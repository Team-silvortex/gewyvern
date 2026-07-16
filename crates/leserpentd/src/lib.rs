use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub use leserpent_adapters::{AdapterRegistry, EffectAdapter, EffectContext};
use leserpent_runtime::{ControlRuntime, RuntimeError, WorkerStep};

#[cfg(unix)]
mod ipc;
#[cfg(unix)]
pub use ipc::IpcServer;
mod remote;
pub use remote::RemoteServer;
mod events;
mod wire;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaemonConfig {
    pub worker_id: String,
    pub lease_duration: Duration,
    pub idle_interval: Duration,
    pub maintenance_interval_ticks: u64,
    pub terminal_effect_retention: u64,
    pub retention_batch_limit: u64,
    pub max_in_flight: usize,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            worker_id: format!("leserpentd-{}", std::process::id()),
            lease_duration: Duration::from_secs(30),
            idle_interval: Duration::from_millis(100),
            maintenance_interval_ticks: 256,
            terminal_effect_retention: 8_192,
            retention_batch_limit: 100,
            max_in_flight: 4,
        }
    }
}

impl DaemonConfig {
    pub fn validate(&self) -> Result<(), String> {
        validate_id("worker_id", &self.worker_id)?;
        if self.lease_duration.is_zero() || self.lease_duration > Duration::from_secs(300) {
            return Err("lease_duration must be between 1 millisecond and 300 seconds".into());
        }
        if self.idle_interval.is_zero() || self.idle_interval > Duration::from_secs(60) {
            return Err("idle_interval must be between 1 millisecond and 60 seconds".into());
        }
        if self.maintenance_interval_ticks == 0 || self.maintenance_interval_ticks > 1_000_000 {
            return Err("maintenance_interval_ticks must be between 1 and 1000000".into());
        }
        if self.retention_batch_limit == 0 || self.retention_batch_limit > 1_000 {
            return Err("retention_batch_limit must be between 1 and 1000".into());
        }
        if self.max_in_flight == 0 || self.max_in_flight > 32 {
            return Err("max_in_flight must be between 1 and 32".into());
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DaemonStats {
    pub heartbeats: u64,
    pub completed: u64,
    pub retries: u64,
    pub rejected: u64,
    pub idle: u64,
    pub pruned_terminal: u64,
}

pub struct DaemonHost {
    runtime: ControlRuntime,
    registry: AdapterRegistry,
    config: DaemonConfig,
    stats: DaemonStats,
}

impl DaemonHost {
    pub fn new(
        runtime: ControlRuntime,
        registry: AdapterRegistry,
        config: DaemonConfig,
    ) -> Result<Self, String> {
        config.validate()?;
        Ok(Self {
            runtime,
            registry,
            config,
            stats: DaemonStats::default(),
        })
    }

    pub fn tick(&mut self) -> Result<WorkerStep, RuntimeError> {
        self.prepare_tick()?;
        let step = if self.registry.is_empty() {
            WorkerStep::Idle
        } else {
            self.runtime.run_effect_once(
                &self.config.worker_id,
                self.config.lease_duration,
                &mut self.registry,
            )?
        };
        self.record_step(&step);
        Ok(step)
    }

    pub fn tick_batch(&mut self, cancelled: &AtomicBool) -> Result<Vec<WorkerStep>, RuntimeError> {
        self.prepare_tick()?;
        if self.registry.is_empty() || cancelled.load(Ordering::Acquire) {
            let steps = vec![WorkerStep::Idle];
            self.record_step(&steps[0]);
            return Ok(steps);
        }
        let mut leases = Vec::with_capacity(self.config.max_in_flight);
        let mut selected_kinds = Vec::with_capacity(self.config.max_in_flight);
        for _ in 0..self.config.max_in_flight {
            let Some(lease) = self.runtime.claim_effect_excluding(
                &self.config.worker_id,
                self.config.lease_duration,
                &selected_kinds,
            )?
            else {
                break;
            };
            selected_kinds.push(lease.kind.clone());
            leases.push(lease);
        }
        if leases.is_empty() {
            let steps = vec![WorkerStep::Idle];
            self.record_step(&steps[0]);
            return Ok(steps);
        }

        let registry = self.registry.clone();
        let executions = thread::scope(|scope| {
            let handles = leases
                .into_iter()
                .map(|lease| {
                    let registry = registry.clone();
                    scope.spawn(move || {
                        let context = EffectContext::new(cancelled);
                        let execution = catch_unwind(AssertUnwindSafe(|| {
                            registry.execute_lease(&lease, &context)
                        }))
                        .unwrap_or_else(|_| {
                            leserpent_runtime::EffectExecution::Reject {
                                error: "adapter execution panicked".into(),
                            }
                        });
                        (lease, execution)
                    })
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|handle| handle.join().expect("adapter panic is isolated"))
                .collect::<Vec<_>>()
        });

        let mut steps = Vec::with_capacity(executions.len());
        for (lease, execution) in executions {
            let step = self.runtime.settle_effect(&lease, execution)?;
            self.record_step(&step);
            steps.push(step);
        }
        Ok(steps)
    }

    pub fn run_steps(&mut self, steps: u64) -> Result<DaemonStats, RuntimeError> {
        let cancelled = AtomicBool::new(false);
        self.run_steps_until(steps, &cancelled)
    }

    pub fn run_steps_until(
        &mut self,
        steps: u64,
        cancelled: &AtomicBool,
    ) -> Result<DaemonStats, RuntimeError> {
        for _ in 0..steps {
            let batch = self.tick_batch(cancelled)?;
            if batch == [WorkerStep::Idle] {
                thread::sleep(self.config.idle_interval);
            }
        }
        Ok(self.stats)
    }

    pub fn run_until(&mut self, stop: &AtomicBool) -> Result<DaemonStats, RuntimeError> {
        while !stop.load(Ordering::Acquire) {
            let batch = self.tick_batch(stop)?;
            if batch == [WorkerStep::Idle] {
                sleep_until_stop(self.config.idle_interval, stop);
            }
        }
        Ok(self.stats)
    }

    pub fn runtime_mut(&mut self) -> &mut ControlRuntime {
        &mut self.runtime
    }

    pub fn stats(&self) -> DaemonStats {
        self.stats
    }

    fn prepare_tick(&mut self) -> Result<(), RuntimeError> {
        self.runtime.heartbeat()?;
        self.stats.heartbeats += 1;
        if self.stats.heartbeats % self.config.maintenance_interval_ticks == 0 {
            let pruned = self.runtime.prune_terminal_effects(
                self.config.terminal_effect_retention,
                self.config.retention_batch_limit,
            )?;
            self.stats.pruned_terminal = self.stats.pruned_terminal.saturating_add(pruned);
        }
        Ok(())
    }

    fn record_step(&mut self, step: &WorkerStep) {
        match step {
            WorkerStep::Idle => self.stats.idle += 1,
            WorkerStep::Completed { .. } => self.stats.completed += 1,
            WorkerStep::RetryScheduled { .. } => self.stats.retries += 1,
            WorkerStep::Rejected { .. } => self.stats.rejected += 1,
        }
    }
}

fn sleep_until_stop(duration: Duration, stop: &AtomicBool) {
    let slice = Duration::from_millis(25);
    let mut remaining = duration;
    while !remaining.is_zero() && !stop.load(Ordering::Acquire) {
        let current = remaining.min(slice);
        thread::sleep(current);
        remaining = remaining.saturating_sub(current);
    }
}

fn validate_id(label: &str, value: &str) -> Result<(), String> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'));
    valid
        .then_some(())
        .ok_or_else(|| format!("invalid {label}"))
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::sync::mpsc;
    use std::sync::{Arc, Barrier};
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use leserpent_adapters::{GEWYVERN_HEALTH_EFFECT_KIND, GewyvernHealthAdapter, GewyvernTarget};
    use leserpent_runtime::EffectExecution;

    fn temp_database(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "leserpentd-{label}-{}-{unique}.sqlite",
            std::process::id()
        ))
    }

    struct EchoAdapter;

    impl EffectAdapter for EchoAdapter {
        fn kind(&self) -> &str {
            "test.echo"
        }

        fn execute(&mut self, payload: &[u8]) -> EffectExecution {
            EffectExecution::Complete(payload.to_vec())
        }
    }

    struct BarrierAdapter {
        kind: &'static str,
        barrier: Arc<Barrier>,
    }

    impl EffectAdapter for BarrierAdapter {
        fn kind(&self) -> &str {
            self.kind
        }

        fn execute(&mut self, payload: &[u8]) -> EffectExecution {
            self.barrier.wait();
            EffectExecution::Complete(payload.to_vec())
        }
    }

    struct PanicAdapter;

    impl EffectAdapter for PanicAdapter {
        fn kind(&self) -> &str {
            "test.panic"
        }

        fn execute(&mut self, _payload: &[u8]) -> EffectExecution {
            panic!("intentional adapter panic")
        }
    }

    struct CancelAdapter {
        started: mpsc::Sender<()>,
    }

    impl EffectAdapter for CancelAdapter {
        fn kind(&self) -> &str {
            "test.cancel"
        }

        fn execute(&mut self, _payload: &[u8]) -> EffectExecution {
            panic!("context-aware execution must be used")
        }

        fn execute_with_context(
            &mut self,
            _payload: &[u8],
            context: &EffectContext<'_>,
        ) -> EffectExecution {
            self.started.send(()).unwrap();
            while !context.is_cancelled() {
                thread::sleep(Duration::from_millis(1));
            }
            EffectExecution::Retry {
                error: "cancelled".into(),
                after: Duration::ZERO,
            }
        }
    }

    #[test]
    fn empty_registry_heartbeats_without_claiming_tasks() {
        let path = temp_database("heartbeat-only");
        let runtime = ControlRuntime::open(&path).unwrap();
        let mut host =
            DaemonHost::new(runtime, AdapterRegistry::default(), DaemonConfig::default()).unwrap();
        host.runtime_mut()
            .enqueue_effect("effect-a", "test.echo", b"hello", 3)
            .unwrap();
        assert_eq!(host.tick().unwrap(), WorkerStep::Idle);
        assert_eq!(host.stats().heartbeats, 1);
        assert!(
            host.runtime_mut()
                .claim_effect("probe", Duration::from_secs(30))
                .unwrap()
                .is_some()
        );
        drop(host);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn bounded_batch_runs_distinct_adapter_kinds_concurrently() {
        let path = temp_database("parallel-batch");
        let runtime = ControlRuntime::open(&path).unwrap();
        let barrier = Arc::new(Barrier::new(2));
        let mut registry = AdapterRegistry::default();
        registry
            .register(BarrierAdapter {
                kind: "test.parallel-a",
                barrier: Arc::clone(&barrier),
            })
            .unwrap();
        registry
            .register(BarrierAdapter {
                kind: "test.parallel-b",
                barrier,
            })
            .unwrap();
        let config = DaemonConfig {
            max_in_flight: 2,
            ..DaemonConfig::default()
        };
        let mut host = DaemonHost::new(runtime, registry, config).unwrap();
        host.runtime_mut()
            .enqueue_effect("parallel-a", "test.parallel-a", b"a", 3)
            .unwrap();
        host.runtime_mut()
            .enqueue_effect("parallel-b", "test.parallel-b", b"b", 3)
            .unwrap();
        let cancelled = AtomicBool::new(false);
        let steps = host.tick_batch(&cancelled).unwrap();
        assert_eq!(steps.len(), 2);
        assert!(
            steps
                .iter()
                .all(|step| matches!(step, WorkerStep::Completed { .. }))
        );
        assert_eq!(host.stats().completed, 2);
        drop(host);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn bounded_batch_leaves_duplicate_adapter_kind_ready() {
        let path = temp_database("same-kind-batch");
        let runtime = ControlRuntime::open(&path).unwrap();
        let mut registry = AdapterRegistry::default();
        registry.register(EchoAdapter).unwrap();
        let config = DaemonConfig {
            max_in_flight: 4,
            ..DaemonConfig::default()
        };
        let mut host = DaemonHost::new(runtime, registry, config).unwrap();
        host.runtime_mut()
            .enqueue_effect("echo-a", "test.echo", b"a", 3)
            .unwrap();
        host.runtime_mut()
            .enqueue_effect("echo-b", "test.echo", b"b", 3)
            .unwrap();
        let cancelled = AtomicBool::new(false);
        assert_eq!(host.tick_batch(&cancelled).unwrap().len(), 1);
        let stats = host.runtime_mut().effect_queue_stats().unwrap();
        assert_eq!(stats.ready, 1);
        assert_eq!(stats.leased, 0);
        assert_eq!(host.tick_batch(&cancelled).unwrap().len(), 1);
        assert_eq!(host.stats().completed, 2);
        drop(host);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn cooperative_cancellation_requeues_claimed_effect() {
        let path = temp_database("cancel-batch");
        let runtime = ControlRuntime::open(&path).unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let mut registry = AdapterRegistry::default();
        registry
            .register(CancelAdapter {
                started: started_tx,
            })
            .unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        host.runtime_mut()
            .enqueue_effect("cancel-a", "test.cancel", b"payload", 3)
            .unwrap();
        let cancelled = AtomicBool::new(false);
        thread::scope(|scope| {
            let handle = scope.spawn(|| host.tick_batch(&cancelled));
            started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
            cancelled.store(true, Ordering::Release);
            let steps = handle.join().unwrap().unwrap();
            assert!(matches!(
                steps.as_slice(),
                [WorkerStep::RetryScheduled { .. }]
            ));
        });
        let stats = host.runtime_mut().effect_queue_stats().unwrap();
        assert_eq!(stats.ready, 1);
        assert_eq!(stats.leased, 0);
        drop(host);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn adapter_panic_rejects_only_the_claimed_effect() {
        let path = temp_database("panic-isolation");
        let runtime = ControlRuntime::open(&path).unwrap();
        let mut registry = AdapterRegistry::default();
        registry.register(PanicAdapter).unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        host.runtime_mut()
            .enqueue_effect("panic-a", "test.panic", b"payload", 3)
            .unwrap();
        let cancelled = AtomicBool::new(false);
        assert!(matches!(
            host.tick_batch(&cancelled).unwrap().as_slice(),
            [WorkerStep::Rejected { .. }]
        ));
        assert_eq!(host.tick_batch(&cancelled).unwrap(), [WorkerStep::Idle]);
        assert_eq!(host.stats().rejected, 1);
        drop(host);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn maintenance_prunes_terminal_tasks_in_bounded_batches() {
        let path = temp_database("retention");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .enqueue_effect("effect-terminal", "test.echo", b"hello", 3)
            .unwrap();
        let lease = runtime
            .claim_effect("setup-worker", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        runtime.complete_effect(&lease, b"done").unwrap();
        let config = DaemonConfig {
            maintenance_interval_ticks: 1,
            terminal_effect_retention: 0,
            retention_batch_limit: 1,
            ..DaemonConfig::default()
        };
        let mut host = DaemonHost::new(runtime, AdapterRegistry::default(), config).unwrap();
        assert_eq!(host.tick().unwrap(), WorkerStep::Idle);
        assert_eq!(host.stats().pruned_terminal, 1);
        assert_eq!(host.runtime_mut().effect_queue_stats().unwrap().total(), 0);
        drop(host);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn registered_adapter_executes_one_bounded_effect() {
        let path = temp_database("adapter");
        let runtime = ControlRuntime::open(&path).unwrap();
        let mut registry = AdapterRegistry::default();
        registry.register(EchoAdapter).unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        host.runtime_mut()
            .enqueue_effect("effect-a", "test.echo", b"hello", 3)
            .unwrap();
        assert_eq!(
            host.tick().unwrap(),
            WorkerStep::Completed {
                effect_id: "effect-a".into(),
                attempt: 1,
            }
        );
        assert_eq!(host.stats().completed, 1);
        drop(host);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn unknown_kind_is_rejected_when_workers_are_enabled() {
        let path = temp_database("unknown-kind");
        let runtime = ControlRuntime::open(&path).unwrap();
        let mut registry = AdapterRegistry::default();
        registry.register(EchoAdapter).unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        host.runtime_mut()
            .enqueue_effect("effect-a", "test.missing", b"hello", 3)
            .unwrap();
        assert!(matches!(host.tick().unwrap(), WorkerStep::Rejected { .. }));
        drop(host);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn gewyvern_health_effect_runs_through_scheduler_and_daemon_registry() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).unwrap();
            assert!(
                std::str::from_utf8(&request[..read])
                    .unwrap()
                    .starts_with("GET /health HTTP/1.1\r\n")
            );
            let body = br#"{"ok":true,"has_snapshot":true}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        });

        let path = temp_database("gewyvern-adapter");
        let runtime = ControlRuntime::open(&path).unwrap();
        let target = GewyvernTarget::loopback(address, None).unwrap();
        let adapter = GewyvernHealthAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        let mut registry = AdapterRegistry::default();
        registry.register(adapter).unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        host.runtime_mut()
            .enqueue_effect(
                "effect-health-a",
                GEWYVERN_HEALTH_EFFECT_KIND,
                br#"{"runtime_id":"runtime-a"}"#,
                3,
            )
            .unwrap();
        assert!(matches!(
            host.tick().unwrap(),
            WorkerStep::Completed { ref effect_id, attempt: 1 }
                if effect_id == "effect-health-a"
        ));
        server.join().unwrap();
        drop(host);
        fs::remove_file(path).unwrap();
    }
}
