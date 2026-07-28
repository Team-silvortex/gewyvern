use std::hint::black_box;
use std::time::Instant;

use leselang_hir::lower;
use leselang_syntax::parse;
use leselang_vm::{Step, Vm};
use leserpent_domain::{CAPABILITY_RUNTIME_READ, CapabilitySet, Principal};
use serde::Serialize;

const BRANCH_COUNT: usize = 64;
const ITERATIONS: usize = 500;

#[derive(Serialize)]
struct BenchmarkReport {
    schema_version: u32,
    workload: Workload,
    metrics: Metrics,
}

#[derive(Serialize)]
struct Workload {
    branch_count: usize,
    iterations: usize,
    source_bytes: usize,
    token_count: usize,
}

#[derive(Serialize)]
struct Metrics {
    parse_p50_ms: f64,
    lower_p50_ms: f64,
    vm_start_p50_ms: f64,
    full_pipeline_p50_ms: f64,
    effect_request_count: usize,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("leselang-language-benchmark: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let source = benchmark_source();
    let syntax = parse(&source);
    if !syntax.diagnostics.is_empty() {
        return Err("benchmark source did not parse".to_string());
    }
    let program = lower(&syntax).map_err(|_| "benchmark source did not lower")?;

    let parse_p50_ms = median_ms(|| {
        let syntax = parse(black_box(&source));
        black_box(syntax);
    });
    let lower_p50_ms = median_ms(|| {
        let program = lower(black_box(&syntax)).expect("validated benchmark syntax");
        black_box(program);
    });
    let vm_start_p50_ms = median_ms(|| {
        black_box(start_program(black_box(&program)));
    });
    let full_pipeline_p50_ms = median_ms(|| {
        let syntax = parse(black_box(&source));
        let program = lower(&syntax).expect("validated benchmark syntax");
        black_box(start_program(&program));
    });

    let effect_request_count = match start_program(&program) {
        Step::Effects(batch) => batch.branches.len(),
        _ => return Err("benchmark program did not produce a structured effect batch".to_string()),
    };
    let report = BenchmarkReport {
        schema_version: 1,
        workload: Workload {
            branch_count: BRANCH_COUNT,
            iterations: ITERATIONS,
            source_bytes: source.len(),
            token_count: syntax.tokens.len(),
        },
        metrics: Metrics {
            parse_p50_ms,
            lower_p50_ms,
            vm_start_p50_ms,
            full_pipeline_p50_ms,
            effect_request_count,
        },
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&report)
            .map_err(|error| format!("failed to encode benchmark report: {error}"))?
    );
    Ok(())
}

fn benchmark_source() -> String {
    let branches = (0..BRANCH_COUNT)
        .map(|index| {
            format!(
                "branch_{index}: runtime.list(environment: \"production\", cluster: none, role: \"edge-{index}\")"
            )
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("fn main() = all({branches})")
}

fn start_program(program: &leselang_hir::HirProgram) -> Step {
    let mut vm = Vm::default();
    vm.start(
        program,
        Principal {
            id: "benchmark".to_string(),
        },
        CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
        None,
    )
}

fn median_ms(mut operation: impl FnMut()) -> f64 {
    operation();
    let mut samples = Vec::with_capacity(ITERATIONS);
    for _ in 0..ITERATIONS {
        let started = Instant::now();
        operation();
        samples.push(started.elapsed().as_nanos());
    }
    samples.sort_unstable();
    samples[samples.len() / 2] as f64 / 1_000_000.0
}
