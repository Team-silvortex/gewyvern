use std::hint::black_box;
use std::time::Instant;

use leselang_ui::{apply_patch, decode_document, diff, encode_document, fleet_document};
use leserpent_domain::{
    QueryResult, RefreshStatus, Revision, RuntimeId, RuntimeProjection, RuntimeStatusSnapshot,
    RuntimeTags,
};
use serde::Serialize;

const RUNTIME_COUNT: usize = 256;
const ITERATIONS: usize = 100;

#[derive(Serialize)]
struct BenchmarkEvidence {
    schema_version: u32,
    workload: Workload,
    metrics: Metrics,
}

#[derive(Serialize)]
struct Workload {
    runtime_count: usize,
    ui_node_count: usize,
    iterations: usize,
}

#[derive(Serialize)]
struct Metrics {
    document_p50_ms: f64,
    patch_p50_ms: f64,
    codec_p50_ms: f64,
    encoded_document_bytes: usize,
    patch_operations: usize,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("leselang-ui-benchmark: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let before_result = fleet_result(Revision(1), false)?;
    let after_result = fleet_result(Revision(2), true)?;
    let before = fleet_document(&before_result).map_err(|error| format!("{error:?}"))?;
    let after = fleet_document(&after_result).map_err(|error| format!("{error:?}"))?;
    let patch = diff(&before, &after).map_err(|error| format!("{error:?}"))?;
    if apply_patch(&before, &patch).map_err(|error| format!("{error:?}"))? != after {
        return Err("UI patch did not converge".into());
    }

    let document_p50_ms = measure(|| {
        black_box(fleet_document(&before_result).expect("validated benchmark fixture"));
    });
    let patch_p50_ms = measure(|| {
        let patch = diff(&before, &after).expect("validated benchmark fixture");
        black_box(apply_patch(&before, &patch).expect("validated benchmark patch"));
    });
    let codec_p50_ms = measure(|| {
        let encoded = encode_document(&before).expect("validated benchmark fixture");
        black_box(decode_document(&encoded).expect("validated benchmark encoding"));
    });
    let encoded_document_bytes = encode_document(&before)
        .map_err(|error| format!("{error:?}"))?
        .len();
    let evidence = BenchmarkEvidence {
        schema_version: 1,
        workload: Workload {
            runtime_count: RUNTIME_COUNT,
            ui_node_count: count_nodes(&before.root),
            iterations: ITERATIONS,
        },
        metrics: Metrics {
            document_p50_ms,
            patch_p50_ms,
            codec_p50_ms,
            encoded_document_bytes,
            patch_operations: patch.operations.len(),
        },
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn fleet_result(revision: Revision, changed: bool) -> Result<QueryResult, String> {
    let runtimes = (0..RUNTIME_COUNT)
        .map(|index| {
            Ok(RuntimeProjection {
                id: RuntimeId::new(format!("runtime-{index:04}"))
                    .map_err(|error| error.to_string())?,
                name: format!("Runtime {index:04}"),
                endpoint: format!("http://127.0.0.1:{}", 10_000 + index),
                revision,
                refresh_count: u64::from(changed && index == 0),
                refresh_status: if changed && index == 0 {
                    RefreshStatus::Ready
                } else {
                    RefreshStatus::NeverRequested
                },
                tags: RuntimeTags::default(),
                status: RuntimeStatusSnapshot::default(),
                capabilities: Default::default(),
                capabilities_observed_for_revision: None,
            })
        })
        .collect::<Result<Vec<_>, String>>()?;
    Ok(QueryResult::RuntimeList { revision, runtimes })
}

fn measure(mut workload: impl FnMut()) -> f64 {
    let mut samples = Vec::with_capacity(ITERATIONS);
    for _ in 0..ITERATIONS {
        let started = Instant::now();
        workload();
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    samples[(samples.len() - 1) / 2] as f64 / 1_000_000.0
}

fn count_nodes(node: &leselang_ui::UiNode) -> usize {
    1 + node.children.iter().map(count_nodes).sum::<usize>()
}
