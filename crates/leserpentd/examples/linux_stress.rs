use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use leserpent_adapters::{AdapterRegistry, EffectAdapter, EffectContext};
use leserpent_runtime::{
    ControlRuntime, EFFECT_QUEUE_CAPACITY, EffectEnqueue, EffectExecution, WorkerStep,
};
use leserpentd::{DaemonConfig, DaemonHost};
use serde::Serialize;

const CHILD_EXIT_CODE: i32 = 86;

#[derive(Serialize)]
struct StressEvidence {
    schema_version: u32,
    passed: bool,
    os: String,
    architecture: String,
    kernel: String,
    concurrency: ConcurrencyEvidence,
    cancellation: CancellationEvidence,
    saturation: SaturationEvidence,
    crash_recovery: CrashRecoveryEvidence,
    total_duration_ms: u128,
}

#[derive(Serialize)]
struct ConcurrencyEvidence {
    effects: u64,
    adapter_kinds: usize,
    max_observed_parallelism: usize,
    completed: u64,
    duration_ms: u128,
}

#[derive(Serialize)]
struct CancellationEvidence {
    retry_scheduled: bool,
    ready_after_cancel: u64,
    leased_after_cancel: u64,
    duration_ms: u128,
}

#[derive(Serialize)]
struct SaturationEvidence {
    capacity: u64,
    active: u64,
    batch_size: usize,
    batches: u64,
    saturated: bool,
    idempotent_replay_at_capacity: bool,
    mixed_overflow_batch_rejected: bool,
    overflow_rejected: bool,
    duration_ms: u128,
}

#[derive(Serialize)]
struct CrashRecoveryEvidence {
    child_exit_code: i32,
    immediate_owner_fenced: bool,
    redelivered_attempt: u32,
    completed_after_recovery: bool,
    owner_lease_wait_ms: u128,
    duration_ms: u128,
}

struct StressAdapter {
    kind: String,
    active: Arc<AtomicUsize>,
    maximum: Arc<AtomicUsize>,
}

impl EffectAdapter for StressAdapter {
    fn kind(&self) -> &str {
        &self.kind
    }

    fn execute(&mut self, payload: &[u8]) -> EffectExecution {
        let active = self.active.fetch_add(1, Ordering::AcqRel) + 1;
        self.maximum.fetch_max(active, Ordering::AcqRel);
        thread::sleep(Duration::from_millis(2));
        self.active.fetch_sub(1, Ordering::AcqRel);
        EffectExecution::Complete(payload.to_vec())
    }
}

struct CancellationAdapter {
    started: mpsc::Sender<()>,
}

impl EffectAdapter for CancellationAdapter {
    fn kind(&self) -> &str {
        "stress.cancel"
    }

    fn execute(&mut self, _payload: &[u8]) -> EffectExecution {
        EffectExecution::Reject {
            error: "cancellation context was not supplied".into(),
        }
    }

    fn execute_with_context(
        &mut self,
        _payload: &[u8],
        context: &EffectContext<'_>,
    ) -> EffectExecution {
        let _ = self.started.send(());
        while !context.is_cancelled() {
            thread::sleep(Duration::from_millis(1));
        }
        EffectExecution::Retry {
            error: "stress cancellation".into(),
            after: Duration::ZERO,
        }
    }
}

fn main() {
    if let Err(error) = run() {
        eprintln!("leserpentd-linux-stress: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = std::env::args_os().collect::<Vec<_>>();
    if arguments
        .get(1)
        .is_some_and(|value| value == "--crash-child")
    {
        let path = arguments
            .get(2)
            .map(PathBuf::from)
            .ok_or_else(|| "crash child requires a database path".to_string())?;
        return crash_child(&path);
    }

    let started = Instant::now();
    let concurrency = concurrency_stress()?;
    let cancellation = cancellation_stress()?;
    let saturation = saturation_stress()?;
    let crash_recovery = crash_recovery_stress()?;
    let evidence = StressEvidence {
        schema_version: 1,
        passed: true,
        os: std::env::consts::OS.into(),
        architecture: std::env::consts::ARCH.into(),
        kernel: kernel_release(),
        concurrency,
        cancellation,
        saturation,
        crash_recovery,
        total_duration_ms: started.elapsed().as_millis(),
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&evidence).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn concurrency_stress() -> Result<ConcurrencyEvidence, String> {
    const KINDS: usize = 8;
    const EFFECTS: u64 = 256;
    let started = Instant::now();
    let path = temp_database("concurrency");
    let runtime = ControlRuntime::open(&path).map_err(|error| error.to_string())?;
    let active = Arc::new(AtomicUsize::new(0));
    let maximum = Arc::new(AtomicUsize::new(0));
    let mut registry = AdapterRegistry::default();
    for index in 0..KINDS {
        registry.register(StressAdapter {
            kind: format!("stress.kind-{index}"),
            active: Arc::clone(&active),
            maximum: Arc::clone(&maximum),
        })?;
    }
    let config = DaemonConfig {
        max_in_flight: KINDS,
        idle_interval: Duration::from_millis(1),
        ..DaemonConfig::default()
    };
    let mut host = DaemonHost::new(runtime, registry, config)?;
    for index in 0..EFFECTS {
        host.runtime_mut()
            .enqueue_effect(
                &format!("stress-effect-{index}"),
                &format!("stress.kind-{}", index % KINDS as u64),
                b"stress",
                3,
            )
            .map_err(|error| error.to_string())?;
    }
    while host
        .runtime_mut()
        .effect_queue_stats()
        .map_err(|error| error.to_string())?
        .active()
        > 0
    {
        host.run_steps(1).map_err(|error| error.to_string())?;
    }
    let completed = host.stats().completed;
    let observed = maximum.load(Ordering::Acquire);
    if completed != EFFECTS || !(2..=KINDS).contains(&observed) {
        return Err(format!(
            "concurrency stress diverged: completed={completed} observed={observed}"
        ));
    }
    drop(host);
    remove_database(&path)?;
    Ok(ConcurrencyEvidence {
        effects: EFFECTS,
        adapter_kinds: KINDS,
        max_observed_parallelism: observed,
        completed,
        duration_ms: started.elapsed().as_millis(),
    })
}

fn cancellation_stress() -> Result<CancellationEvidence, String> {
    let started = Instant::now();
    let path = temp_database("cancellation");
    let runtime = ControlRuntime::open(&path).map_err(|error| error.to_string())?;
    let (started_tx, started_rx) = mpsc::channel();
    let mut registry = AdapterRegistry::default();
    registry.register(CancellationAdapter {
        started: started_tx,
    })?;
    let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default())?;
    host.runtime_mut()
        .enqueue_effect("stress-cancel", "stress.cancel", b"cancel", 3)
        .map_err(|error| error.to_string())?;
    let cancelled = AtomicBool::new(false);
    let steps = thread::scope(|scope| -> Result<Vec<WorkerStep>, String> {
        let handle = scope.spawn(|| host.tick_batch(&cancelled));
        started_rx
            .recv_timeout(Duration::from_secs(2))
            .map_err(|_| "cancellation adapter did not start".to_string())?;
        cancelled.store(true, Ordering::Release);
        handle
            .join()
            .map_err(|_| "cancellation worker panicked".to_string())?
            .map_err(|error| error.to_string())
    })?;
    let stats = host
        .runtime_mut()
        .effect_queue_stats()
        .map_err(|error| error.to_string())?;
    let retry_scheduled = matches!(steps.as_slice(), [WorkerStep::RetryScheduled { .. }]);
    if !retry_scheduled || stats.ready != 1 || stats.leased != 0 {
        return Err("cancelled effect was not safely requeued".into());
    }
    drop(host);
    remove_database(&path)?;
    Ok(CancellationEvidence {
        retry_scheduled,
        ready_after_cancel: stats.ready,
        leased_after_cancel: stats.leased,
        duration_ms: started.elapsed().as_millis(),
    })
}

fn saturation_stress() -> Result<SaturationEvidence, String> {
    const BATCH_SIZE: usize = 100;
    let started = Instant::now();
    let path = temp_database("saturation");
    let mut runtime = ControlRuntime::open(&path).map_err(|error| error.to_string())?;
    let mut batches = 0_u64;
    for start in (0..EFFECT_QUEUE_CAPACITY).step_by(BATCH_SIZE) {
        let batch = (start..start + BATCH_SIZE as u64)
            .map(|index| EffectEnqueue {
                effect_id: format!("saturation-{index}"),
                kind: "stress.saturation".into(),
                payload: b"saturation".to_vec(),
                max_attempts: 1,
            })
            .collect::<Vec<_>>();
        let inserted = runtime
            .enqueue_effect_batch(&batch)
            .map_err(|error| error.to_string())?;
        if inserted != BATCH_SIZE as u64 {
            return Err(format!("saturation batch inserted only {inserted} tasks"));
        }
        batches += 1;
    }
    let overflow_rejected = runtime
        .enqueue_effect("saturation-overflow", "stress.saturation", b"overflow", 1)
        .is_err();
    let existing = EffectEnqueue {
        effect_id: "saturation-0".into(),
        kind: "stress.saturation".into(),
        payload: b"saturation".to_vec(),
        max_attempts: 1,
    };
    let idempotent_replay_at_capacity = runtime
        .enqueue_effect_batch(std::slice::from_ref(&existing))
        .is_ok_and(|inserted| inserted == 0);
    let mixed_overflow_batch_rejected = runtime
        .enqueue_effect_batch(&[
            existing,
            EffectEnqueue {
                effect_id: "saturation-batch-overflow".into(),
                kind: "stress.saturation".into(),
                payload: b"overflow".to_vec(),
                max_attempts: 1,
            },
        ])
        .is_err();
    let stats = runtime
        .effect_queue_stats()
        .map_err(|error| error.to_string())?;
    if !overflow_rejected
        || !idempotent_replay_at_capacity
        || !mixed_overflow_batch_rejected
        || !stats.saturated()
        || stats.active() != EFFECT_QUEUE_CAPACITY
    {
        return Err("effect queue did not enforce its active capacity".into());
    }
    drop(runtime);
    remove_database(&path)?;
    Ok(SaturationEvidence {
        capacity: stats.capacity,
        active: stats.active(),
        batch_size: BATCH_SIZE,
        batches,
        saturated: stats.saturated(),
        idempotent_replay_at_capacity,
        mixed_overflow_batch_rejected,
        overflow_rejected,
        duration_ms: started.elapsed().as_millis(),
    })
}

fn crash_recovery_stress() -> Result<CrashRecoveryEvidence, String> {
    let started = Instant::now();
    let path = temp_database("crash-recovery");
    let child = Command::new(std::env::current_exe().map_err(|error| error.to_string())?)
        .arg("--crash-child")
        .arg(&path)
        .status()
        .map_err(|error| error.to_string())?;
    let child_exit_code = child.code().unwrap_or_default();
    if child_exit_code != CHILD_EXIT_CODE {
        return Err(format!("crash child exited with {child_exit_code}"));
    }
    let immediate_owner_fenced = ControlRuntime::open(&path).is_err();
    if !immediate_owner_fenced {
        return Err("crashed owner was not fenced before lease expiry".into());
    }
    let wait_started = Instant::now();
    thread::sleep(Duration::from_secs(31));
    let mut recovered = ControlRuntime::open(&path).map_err(|error| error.to_string())?;
    let lease = recovered
        .claim_effect("recovery-worker", Duration::from_secs(5))
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "crashed effect was not redelivered".to_string())?;
    let redelivered_attempt = lease.attempt;
    recovered
        .complete_effect(&lease, b"recovered")
        .map_err(|error| error.to_string())?;
    let completed_after_recovery = recovered
        .effect_queue_stats()
        .map_err(|error| error.to_string())?
        .completed
        == 1;
    if redelivered_attempt != 2 || !completed_after_recovery {
        return Err("crashed effect recovery did not preserve attempt fencing".into());
    }
    drop(recovered);
    remove_database(&path)?;
    Ok(CrashRecoveryEvidence {
        child_exit_code,
        immediate_owner_fenced,
        redelivered_attempt,
        completed_after_recovery,
        owner_lease_wait_ms: wait_started.elapsed().as_millis(),
        duration_ms: started.elapsed().as_millis(),
    })
}

fn crash_child(path: &Path) -> Result<(), String> {
    let mut runtime = ControlRuntime::open(path).map_err(|error| error.to_string())?;
    runtime
        .enqueue_effect("crash-effect", "stress.crash", b"crash", 3)
        .map_err(|error| error.to_string())?;
    runtime
        .claim_effect("crash-worker", Duration::from_secs(1))
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "crash child could not claim effect".to_string())?;
    std::process::exit(CHILD_EXIT_CODE);
}

fn temp_database(label: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    std::env::temp_dir().join(format!(
        "leserpentd-linux-stress-{label}-{}-{unique}.sqlite",
        std::process::id()
    ))
}

fn remove_database(path: &Path) -> Result<(), String> {
    std::fs::remove_file(path).map_err(|error| format!("remove {}: {error}", path.display()))
}

fn kernel_release() -> String {
    Command::new("uname")
        .args(["-s", "-r"])
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|value| value.trim().to_string())
        .unwrap_or_else(|| "unavailable".into())
}
