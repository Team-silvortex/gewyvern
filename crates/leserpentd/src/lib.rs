use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub use leserpent_adapters::{AdapterRegistry, EffectAdapter};
use leserpent_runtime::{ControlRuntime, RuntimeError, WorkerStep};

#[cfg(unix)]
mod ipc;
#[cfg(unix)]
pub use ipc::IpcServer;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DaemonConfig {
    pub worker_id: String,
    pub lease_duration: Duration,
    pub idle_interval: Duration,
    pub maintenance_interval_ticks: u64,
    pub terminal_effect_retention: u64,
    pub retention_batch_limit: u64,
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
        self.runtime.heartbeat()?;
        self.stats.heartbeats += 1;
        if self.stats.heartbeats % self.config.maintenance_interval_ticks == 0 {
            let pruned = self.runtime.prune_terminal_effects(
                self.config.terminal_effect_retention,
                self.config.retention_batch_limit,
            )?;
            self.stats.pruned_terminal = self.stats.pruned_terminal.saturating_add(pruned);
        }
        let step = if self.registry.is_empty() {
            WorkerStep::Idle
        } else {
            self.runtime.run_effect_once(
                &self.config.worker_id,
                self.config.lease_duration,
                &mut self.registry,
            )?
        };
        match &step {
            WorkerStep::Idle => self.stats.idle += 1,
            WorkerStep::Completed { .. } => self.stats.completed += 1,
            WorkerStep::RetryScheduled { .. } => self.stats.retries += 1,
            WorkerStep::Rejected { .. } => self.stats.rejected += 1,
        }
        Ok(step)
    }

    pub fn run_steps(&mut self, steps: u64) -> Result<DaemonStats, RuntimeError> {
        for _ in 0..steps {
            let step = self.tick()?;
            if step == WorkerStep::Idle {
                thread::sleep(self.config.idle_interval);
            }
        }
        Ok(self.stats)
    }

    pub fn run_until(&mut self, stop: &AtomicBool) -> Result<DaemonStats, RuntimeError> {
        while !stop.load(Ordering::Acquire) {
            let step = self.tick()?;
            if step == WorkerStep::Idle {
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
