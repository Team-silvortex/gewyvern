use std::fs;
use std::hint::black_box;
use std::path::PathBuf;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use leserpent_domain::{
    CAPABILITY_RUNTIME_READ, COMMAND_PLAN_SCHEMA_VERSION, CapabilitySet, PlannedOperation,
    Principal, Query, QueryEnvelope, RuntimeId, RuntimeListFilter,
};
use leserpent_runtime::{ControlRuntime, EffectEnqueue, PlanResult};
use serde::Serialize;

const COLD_OPEN_SAMPLES: usize = 16;
const RUNTIME_COUNT: usize = 256;
const QUERY_ITERATIONS: usize = 2_000;
const EFFECT_COUNT: usize = 10_000;
const EFFECT_BATCH_SIZE: usize = 100;

#[derive(Serialize)]
struct BenchmarkEvidence {
    schema_version: u32,
    workload: Workload,
    metrics: Metrics,
}

#[derive(Serialize)]
struct Workload {
    cold_open_samples: usize,
    runtime_count: usize,
    query_iterations: usize,
    effect_count: usize,
    effect_batch_size: usize,
}

#[derive(Serialize)]
struct Metrics {
    cold_open_p50_ms: f64,
    cold_open_p95_ms: f64,
    runtime_list_p50_ms: f64,
    effect_enqueue_ms: f64,
    effect_enqueue_per_second: f64,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("leserpent-runtime-benchmark: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let cold_open = benchmark_cold_open()?;
    let runtime_list_p50_ms = benchmark_runtime_list()?;
    let (effect_enqueue_ms, effect_enqueue_per_second) = benchmark_effect_enqueue()?;
    let evidence = BenchmarkEvidence {
        schema_version: 1,
        workload: Workload {
            cold_open_samples: COLD_OPEN_SAMPLES,
            runtime_count: RUNTIME_COUNT,
            query_iterations: QUERY_ITERATIONS,
            effect_count: EFFECT_COUNT,
            effect_batch_size: EFFECT_BATCH_SIZE,
        },
        metrics: Metrics {
            cold_open_p50_ms: percentile_ms(&cold_open, 50),
            cold_open_p95_ms: percentile_ms(&cold_open, 95),
            runtime_list_p50_ms,
            effect_enqueue_ms,
            effect_enqueue_per_second,
        },
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn benchmark_cold_open() -> Result<Vec<u128>, String> {
    let mut samples = Vec::with_capacity(COLD_OPEN_SAMPLES);
    for index in 0..COLD_OPEN_SAMPLES {
        let path = temp_database(&format!("cold-{index}"));
        let started = Instant::now();
        let runtime = ControlRuntime::open(&path).map_err(|error| error.to_string())?;
        samples.push(started.elapsed().as_nanos());
        drop(runtime);
        fs::remove_file(path).map_err(|error| error.to_string())?;
    }
    Ok(samples)
}

fn benchmark_runtime_list() -> Result<f64, String> {
    let path = temp_database("query");
    let mut runtime = ControlRuntime::open(&path).map_err(|error| error.to_string())?;
    for index in 0..RUNTIME_COUNT {
        runtime
            .register_runtime(
                RuntimeId::new(format!("runtime-{index:04}")).map_err(|error| error.to_string())?,
                format!("Runtime {index:04}"),
                format!("http://127.0.0.1:{}", 10_000 + index),
            )
            .map_err(|error| error.to_string())?;
    }
    let plan = leserpent_domain::CommandPlan {
        schema_version: COMMAND_PLAN_SCHEMA_VERSION,
        required_capability: CAPABILITY_RUNTIME_READ.into(),
        operation: PlannedOperation::Query(QueryEnvelope {
            schema_version: leserpent_domain::DOMAIN_SCHEMA_VERSION,
            principal: Principal {
                id: "benchmark".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            query: Query::RuntimeList {
                filter: RuntimeListFilter::default(),
            },
        }),
    };
    let mut samples = Vec::with_capacity(QUERY_ITERATIONS);
    for _ in 0..QUERY_ITERATIONS {
        let started = Instant::now();
        let result = runtime
            .execute_plan(plan.clone())
            .map_err(|error| error.to_string())?;
        samples.push(started.elapsed().as_nanos());
        let PlanResult::Query(result) = result else {
            return Err("runtime list returned a command result".into());
        };
        black_box(result);
    }
    drop(runtime);
    fs::remove_file(path).map_err(|error| error.to_string())?;
    Ok(percentile_ms(&samples, 50))
}

fn benchmark_effect_enqueue() -> Result<(f64, f64), String> {
    let path = temp_database("effects");
    let mut runtime = ControlRuntime::open(&path).map_err(|error| error.to_string())?;
    let started = Instant::now();
    for start in (0..EFFECT_COUNT).step_by(EFFECT_BATCH_SIZE) {
        let batch = (start..start + EFFECT_BATCH_SIZE)
            .map(|index| EffectEnqueue {
                effect_id: format!("benchmark-effect-{index}"),
                kind: "benchmark.effect".into(),
                payload: b"bounded-payload".to_vec(),
                max_attempts: 1,
            })
            .collect::<Vec<_>>();
        let inserted = runtime
            .enqueue_effect_batch(&batch)
            .map_err(|error| error.to_string())?;
        if inserted != EFFECT_BATCH_SIZE as u64 {
            return Err(format!("effect batch inserted {inserted} entries"));
        }
    }
    let elapsed = started.elapsed();
    let stats = runtime
        .effect_queue_stats()
        .map_err(|error| error.to_string())?;
    if stats.active() != EFFECT_COUNT as u64 {
        return Err(format!(
            "effect queue contains {} active entries",
            stats.active()
        ));
    }
    drop(runtime);
    fs::remove_file(path).map_err(|error| error.to_string())?;
    let elapsed_ms = elapsed.as_secs_f64() * 1_000.0;
    Ok((elapsed_ms, EFFECT_COUNT as f64 / elapsed.as_secs_f64()))
}

fn percentile_ms(samples: &[u128], percentile: usize) -> f64 {
    let mut samples = samples.to_vec();
    samples.sort_unstable();
    let index = (samples.len() - 1) * percentile / 100;
    samples[index] as f64 / 1_000_000.0
}

fn temp_database(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "leserpent-runtime-benchmark-{label}-{}-{unique}.sqlite",
        std::process::id()
    ))
}
