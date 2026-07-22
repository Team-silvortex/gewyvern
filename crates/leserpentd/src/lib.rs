use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

use leserpent_adapters::HOST_BOOTSTRAP_EFFECT_KIND;
pub use leserpent_adapters::{AdapterRegistry, EffectAdapter, EffectContext};
use leserpent_domain::bootstrap::{BootstrapPhase, DeploymentBootstrapCheckpoint};
use leserpent_protocol::bootstrap::{
    BootstrapResponse, decode_bootstrap_request, decode_bootstrap_response,
};
use leserpent_runtime::{ControlRuntime, EffectExecution, EffectLease, RuntimeError, WorkerStep};

pub mod bootstrap_health;
pub mod bootstrap_install;
#[cfg(unix)]
mod ipc;
#[cfg(unix)]
pub use ipc::IpcServer;
mod remote;
pub use remote::{RemoteServer, load_remote_token_file};
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
            let Some(lease) = self
                .runtime
                .claim_effect(&self.config.worker_id, self.config.lease_duration)?
            else {
                self.record_step(&WorkerStep::Idle);
                return Ok(WorkerStep::Idle);
            };
            let cancelled = AtomicBool::new(false);
            let execution = self
                .registry
                .execute_lease(&lease, &EffectContext::new(&cancelled));
            self.settle_execution(&lease, execution)?
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
                    let fallback_lease = lease.clone();
                    let handle = scope.spawn(move || {
                        let context = EffectContext::new(cancelled);
                        let execution = registry.execute_lease(&lease, &context);
                        (lease, execution)
                    });
                    (fallback_lease, handle)
                })
                .collect::<Vec<_>>();
            handles
                .into_iter()
                .map(|(lease, handle)| {
                    handle.join().unwrap_or_else(|_| {
                        (
                            lease,
                            leserpent_runtime::EffectExecution::Reject {
                                error: "adapter execution panicked".into(),
                            },
                        )
                    })
                })
                .collect::<Vec<_>>()
        });

        let mut steps = Vec::with_capacity(executions.len());
        for (lease, execution) in executions {
            let step = self.settle_execution(&lease, execution)?;
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

    fn settle_execution(
        &mut self,
        lease: &EffectLease,
        execution: EffectExecution,
    ) -> Result<WorkerStep, RuntimeError> {
        let EffectExecution::Complete(outcome) = execution else {
            return self.runtime.settle_effect(lease, execution);
        };
        if lease.kind != HOST_BOOTSTRAP_EFFECT_KIND {
            return self
                .runtime
                .settle_effect(lease, EffectExecution::Complete(outcome));
        }
        let checkpoint = match checkpoint_from_bootstrap_effect(lease, &outcome) {
            Ok(checkpoint) => checkpoint,
            Err(error) => {
                self.runtime
                    .reject_effect(lease, &format!("bootstrap outcome was rejected: {error}"))?;
                return Ok(WorkerStep::Rejected {
                    effect_id: lease.effect_id.clone(),
                    attempt: lease.attempt,
                });
            }
        };
        self.runtime
            .complete_bootstrap_effect(lease, &outcome, &checkpoint)?;
        Ok(WorkerStep::Completed {
            effect_id: lease.effect_id.clone(),
            attempt: lease.attempt,
        })
    }

    fn prepare_tick(&mut self) -> Result<(), RuntimeError> {
        self.runtime.heartbeat()?;
        self.stats.heartbeats += 1;
        if self
            .stats
            .heartbeats
            .is_multiple_of(self.config.maintenance_interval_ticks)
        {
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

fn checkpoint_from_bootstrap_effect(
    lease: &EffectLease,
    outcome: &[u8],
) -> Result<DeploymentBootstrapCheckpoint, String> {
    let request = decode_bootstrap_request(&lease.payload)
        .map_err(|_| "invalid persisted bootstrap request".to_string())?;
    let response =
        decode_bootstrap_response(outcome).map_err(|_| "invalid bootstrap response".to_string())?;
    let BootstrapResponse::State(state) = response.response else {
        return Err("bootstrap adapter returned an error envelope as success".into());
    };
    if state.bootstrap_id != request.request.intent.bootstrap_id
        || state.target != request.request.intent.target
    {
        return Err("bootstrap response identity does not match its request".into());
    }
    let credential_handle = match state.phase {
        BootstrapPhase::Bootstrapped => Some(request.request.intent.credential_handle),
        BootstrapPhase::Failed => None,
        _ => return Err("bootstrap effect stopped before deployment was terminal".into()),
    };
    DeploymentBootstrapCheckpoint::new(1, state, credential_handle)
        .map_err(|error| error.to_string())
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
    use leserpent_adapters::{
        ConfiguredSecretStore, GEWYVERN_DEPLOYMENT_EFFECT_KIND, GEWYVERN_DISCOVERY_EFFECT_KIND,
        GEWYVERN_HEALTH_EFFECT_KIND, GewyvernDeploymentAdapter, GewyvernDiscoveryAdapter,
        GewyvernHealthAdapter, GewyvernTarget, SecretKey, SecretValue,
    };
    use leserpent_domain::bootstrap::{
        BOOTSTRAP_DOMAIN_SCHEMA_VERSION, BOOTSTRAP_SESSION_PROTOCOL_VERSION, BootstrapId,
        BootstrapIntent, BootstrapTarget, BootstrapTransport, CAPABILITY_HOST_BOOTSTRAP,
        CredentialHandle, DaemonBootstrapReceipt, DaemonId, DaemonSessionProof,
        DeploymentBootstrap,
    };
    use leserpent_domain::{CapabilitySet, Principal};
    use leserpent_protocol::bootstrap::{
        BOOTSTRAP_PROTOCOL_SCHEMA_VERSION, BootstrapRequest, BootstrapRequestEnvelope,
        BootstrapResponseEnvelope, encode_bootstrap_request, encode_bootstrap_response,
    };
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

    struct FixedBootstrapAdapter {
        outcome: Vec<u8>,
    }

    impl EffectAdapter for FixedBootstrapAdapter {
        fn kind(&self) -> &str {
            HOST_BOOTSTRAP_EFFECT_KIND
        }

        fn execute(&mut self, _payload: &[u8]) -> EffectExecution {
            EffectExecution::Complete(self.outcome.clone())
        }
    }

    fn bootstrap_request_and_outcome() -> (Vec<u8>, Vec<u8>, BootstrapId) {
        let bootstrap_id = BootstrapId::new("bootstrap-restart-1").unwrap();
        let target = BootstrapTarget {
            transport: BootstrapTransport::Ssh,
            host: "host.example".into(),
            port: 22,
        };
        let intent = BootstrapIntent {
            schema_version: BOOTSTRAP_DOMAIN_SCHEMA_VERSION,
            bootstrap_id: bootstrap_id.clone(),
            target,
            credential_handle: CredentialHandle::new("vault:ssh:host-example").unwrap(),
            requested_by: "operator-a".into(),
            confirmed: true,
        };
        let principal = Principal {
            id: "operator-a".into(),
        };
        let capabilities = CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]);
        let request = encode_bootstrap_request(&BootstrapRequestEnvelope {
            schema_version: BOOTSTRAP_PROTOCOL_SCHEMA_VERSION,
            request: BootstrapRequest {
                principal: principal.clone(),
                capabilities: capabilities.clone(),
                intent: intent.clone(),
            },
        })
        .unwrap();
        let mut bootstrap = DeploymentBootstrap::plan(&principal, &capabilities, intent).unwrap();
        bootstrap.begin().unwrap();
        let state = bootstrap
            .accept_deployed(DaemonBootstrapReceipt {
                bootstrap_id: bootstrap_id.clone(),
                daemon_id: DaemonId::new("daemon-host-example").unwrap(),
                endpoint: "https://host.example:9443/".into(),
                session_credential_handle: CredentialHandle::new("vault:leserpentd:host-example")
                    .unwrap(),
                trust_credential_handle: CredentialHandle::new("vault:leserpent-ca:host-example")
                    .unwrap(),
            })
            .unwrap();
        let outcome = encode_bootstrap_response(&BootstrapResponseEnvelope {
            schema_version: BOOTSTRAP_PROTOCOL_SCHEMA_VERSION,
            response: BootstrapResponse::State(state),
        })
        .unwrap();
        (request, outcome, bootstrap_id)
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
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
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

    #[test]
    fn gewyvern_deployment_runs_through_scheduler_and_daemon_registry() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let read = stream.read(&mut request).unwrap();
            let request = std::str::from_utf8(&request[..read]).unwrap();
            assert!(request.starts_with("POST /v1/deployments HTTP/1.1\r\n"));
            assert!(request.contains("X-Gewyvern-Admin-Token: test-token\r\n"));
            let body = br#"{"deployment_id":"gdep_1","request_id":"deploy-1","pipeline_kind":"http/request","requested_by":"operator","status":"accepted","accepted_unix_ms":1,"target":"pid:42","replayed":false}"#;
            write!(
                stream,
                "HTTP/1.1 202 Accepted\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        });

        let key = SecretKey::new("runtime-a-admin").unwrap();
        let secrets = Arc::new(
            ConfiguredSecretStore::new([(key.clone(), SecretValue::new("test-token").unwrap())])
                .unwrap(),
        );
        let target = GewyvernTarget::loopback(address, Some(key)).unwrap();
        let adapter = GewyvernDeploymentAdapter::with_secret_store(
            [("runtime-a".to_string(), target)],
            secrets,
        )
        .unwrap();
        let path = temp_database("gewyvern-deployment-adapter");
        let runtime = ControlRuntime::open(&path).unwrap();
        let mut registry = AdapterRegistry::default();
        registry.register(adapter).unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        host.runtime_mut()
            .enqueue_effect(
                "effect-deploy-a",
                GEWYVERN_DEPLOYMENT_EFFECT_KIND,
                br#"{"runtime_id":"runtime-a","request_id":"deploy-1","pipeline_kind":"http/request","requested_by":"operator","confirmed":true,"target":"pid:42"}"#,
                3,
            )
            .unwrap();
        assert!(matches!(
            host.tick().unwrap(),
            WorkerStep::Completed { ref effect_id, attempt: 1 }
                if effect_id == "effect-deploy-a"
        ));
        server.join().unwrap();
        drop(host);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn gewyvern_discovery_runs_through_scheduler_without_ambient_targets() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = std::thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).unwrap();
            assert!(
                std::str::from_utf8(&request[..read])
                    .unwrap()
                    .starts_with("GET /v1/capabilities HTTP/1.1\r\n")
            );
            let body = br#"{"service":"gewyvern-api","version":"1.2.0","latest_snapshot":true,"authenticated_deployment":false,"serve_required":true,"external_sidecar_context":true,"target_path_segment_encoding":"percent-encoding","target_direct_path_chars":"A-Z a-z 0-9 . _ ~ :","endpoints":["/v1/capabilities"],"protocol_catalog":true}"#;
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        });

        let target = GewyvernTarget::loopback(address, None).unwrap();
        let adapter = GewyvernDiscoveryAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        let path = temp_database("gewyvern-discovery-adapter");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(
                leserpent_domain::RuntimeId::new("runtime-a").unwrap(),
                "Runtime A",
                "http://127.0.0.1:9411",
            )
            .unwrap();
        let mut registry = AdapterRegistry::default();
        registry.register(adapter).unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        host.runtime_mut()
            .enqueue_effect(
                "effect-discovery-a",
                GEWYVERN_DISCOVERY_EFFECT_KIND,
                br#"{"runtime_id":"runtime-a","expected_revision":1}"#,
                3,
            )
            .unwrap();
        assert!(matches!(
            host.tick().unwrap(),
            WorkerStep::Completed { ref effect_id, attempt: 1 }
                if effect_id == "effect-discovery-a"
        ));
        let (_, runtimes) = host.runtime_mut().runtime_event_state();
        assert_eq!(runtimes[0].revision, leserpent_domain::Revision(2));
        assert_eq!(runtimes[0].capabilities.service, "gewyvern-api");
        assert_eq!(
            runtimes[0].capabilities.extensions.get("protocol_catalog"),
            Some(&true)
        );
        server.join().unwrap();
        drop(host);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn bootstrap_handoff_survives_restart_and_requires_matching_session_proof() {
        let path = temp_database("bootstrap-handoff-restart");
        let (request, outcome, bootstrap_id) = bootstrap_request_and_outcome();
        let runtime = ControlRuntime::open(&path).unwrap();
        let mut registry = AdapterRegistry::default();
        registry
            .register(FixedBootstrapAdapter { outcome })
            .unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        host.runtime_mut()
            .enqueue_effect(
                "bootstrap-effect-1",
                HOST_BOOTSTRAP_EFFECT_KIND,
                &request,
                3,
            )
            .unwrap();
        assert!(matches!(
            host.tick().unwrap(),
            WorkerStep::Completed { attempt: 1, .. }
        ));
        let before_restart = host
            .runtime_mut()
            .bootstrap_checkpoint(&bootstrap_id)
            .unwrap()
            .unwrap();
        assert_eq!(before_restart.state.phase, BootstrapPhase::Bootstrapped);
        assert!(!before_restart.state.mutation_authorized);
        drop(host);

        let bytes = fs::read(&path).unwrap();
        assert!(
            !bytes
                .windows(22)
                .any(|window| window == b"raw-bootstrap-password")
        );
        assert!(
            !bytes
                .windows(17)
                .any(|window| window == b"raw-session-token")
        );

        let mut recovered = ControlRuntime::open(&path).unwrap();
        let recovered_checkpoint = recovered
            .bootstrap_checkpoint(&bootstrap_id)
            .unwrap()
            .unwrap();
        assert_eq!(recovered_checkpoint.revision, 1);
        assert_eq!(
            recovered_checkpoint.state.phase,
            BootstrapPhase::Bootstrapped
        );
        let mut proof = DaemonSessionProof {
            bootstrap_id: bootstrap_id.clone(),
            daemon_id: DaemonId::new("daemon-wrong").unwrap(),
            session_credential_handle: CredentialHandle::new("vault:leserpentd:host-example")
                .unwrap(),
            trust_credential_handle: CredentialHandle::new("vault:leserpent-ca:host-example")
                .unwrap(),
            authority_owned: true,
            protocol_schema_version: BOOTSTRAP_SESSION_PROTOCOL_VERSION,
        };
        assert!(matches!(
            recovered.bind_bootstrap_session(&bootstrap_id, proof.clone()),
            Err(RuntimeError::Bootstrap(
                leserpent_domain::bootstrap::BootstrapError::IdentityMismatch
            ))
        ));
        assert_eq!(
            recovered
                .bootstrap_checkpoint(&bootstrap_id)
                .unwrap()
                .unwrap()
                .revision,
            1
        );

        proof.daemon_id = DaemonId::new("daemon-host-example").unwrap();
        let bound = recovered
            .bind_bootstrap_session(&bootstrap_id, proof.clone())
            .unwrap();
        assert_eq!(bound.phase, BootstrapPhase::SessionBound);
        assert!(bound.mutation_authorized);
        drop(recovered);

        let mut restarted = ControlRuntime::open(&path).unwrap();
        let durable = restarted
            .bootstrap_checkpoint(&bootstrap_id)
            .unwrap()
            .unwrap();
        assert_eq!(durable.revision, 2);
        assert_eq!(durable.state.phase, BootstrapPhase::SessionBound);
        assert!(durable.state.mutation_authorized);
        assert!(durable.bootstrap_credential_handle.is_none());
        let replay = restarted
            .bind_bootstrap_session(&bootstrap_id, proof)
            .unwrap();
        assert_eq!(replay, durable.state);
        assert_eq!(
            restarted
                .bootstrap_checkpoint(&bootstrap_id)
                .unwrap()
                .unwrap()
                .revision,
            2
        );
        drop(restarted);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn malformed_bootstrap_outcome_is_rejected_without_a_handoff() {
        let path = temp_database("bootstrap-handoff-malformed");
        let (request, _, bootstrap_id) = bootstrap_request_and_outcome();
        let runtime = ControlRuntime::open(&path).unwrap();
        let mut registry = AdapterRegistry::default();
        registry
            .register(FixedBootstrapAdapter {
                outcome: br#"{"not":"a bootstrap response"}"#.to_vec(),
            })
            .unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        host.runtime_mut()
            .enqueue_effect(
                "bootstrap-effect-malformed",
                HOST_BOOTSTRAP_EFFECT_KIND,
                &request,
                3,
            )
            .unwrap();
        assert!(matches!(host.tick().unwrap(), WorkerStep::Rejected { .. }));
        assert!(
            host.runtime_mut()
                .bootstrap_checkpoint(&bootstrap_id)
                .unwrap()
                .is_none()
        );
        drop(host);
        fs::remove_file(path).unwrap();
    }
}
