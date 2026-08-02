use std::sync::atomic::{AtomicBool, Ordering};
use std::thread;
use std::time::Duration;

pub use leserpent_adapters::{AdapterRegistry, EffectAdapter, EffectContext};
use leserpent_adapters::{
    DAEMON_RETIREMENT_EFFECT_KIND, GEWYVERN_PROVISIONING_EFFECT_KIND,
    GEWYVERN_RETIREMENT_EFFECT_KIND, HOST_BOOTSTRAP_EFFECT_KIND,
};
use leserpent_domain::bootstrap::{BootstrapPhase, DeploymentBootstrapCheckpoint};
use leserpent_domain::bootstrap_retirement::{DaemonRetirementCheckpoint, DaemonRetirementPhase};
use leserpent_domain::provisioning::{ProvisioningPhase, RuntimeProvisioningCheckpoint};
use leserpent_domain::retirement::{RetirementPhase, RuntimeRetirementCheckpoint};
use leserpent_protocol::bootstrap::{
    BootstrapRequestEnvelope, BootstrapResponse, decode_bootstrap_request,
    decode_bootstrap_response,
};
use leserpent_protocol::bootstrap_retirement_control::{
    DaemonRetirementEffectEnvelope, DaemonRetirementResponse, DaemonRetirementResponseEnvelope,
    decode_daemon_retirement_effect, decode_daemon_retirement_response,
};
use leserpent_protocol::provisioning::{
    ProvisioningRequestEnvelope, ProvisioningResponse, decode_provisioning_request,
    decode_provisioning_response,
};
use leserpent_protocol::retirement::{
    RetirementRequestEnvelope, RetirementResponse, RetirementResponseEnvelope,
    decode_retirement_request, decode_retirement_response,
};
use leserpent_runtime::{ControlRuntime, EffectExecution, EffectLease, RuntimeError, WorkerStep};

pub mod bootstrap_health;
pub mod bootstrap_install;
#[cfg(feature = "native-ssh")]
mod bootstrap_origin;
#[cfg(feature = "native-ssh")]
mod gewyvern_origin;
#[cfg(feature = "native-ssh")]
pub use bootstrap_origin::{BOOTSTRAP_ORIGIN_CONFIG_SCHEMA_VERSION, BootstrapOriginConfig};
#[cfg(feature = "native-ssh")]
pub use gewyvern_origin::{GEWYVERN_ORIGIN_CONFIG_SCHEMA_VERSION, GewyvernOriginConfig};
mod bootstrap_session;
pub use bootstrap_session::NativeBootstrapSessionVerifier;
mod bootstrap_submission;
mod daemon_retirement_submission;
#[cfg(unix)]
mod ipc;
mod provisioning_submission;
mod retirement_submission;
#[cfg(unix)]
pub use ipc::IpcServer;
mod remote;
pub use remote::{RemoteServer, load_remote_token_file};
mod events;
mod wire;
pub use wire::BootstrapSessionVerifier;

pub const MAX_IPC_CONNECTIONS_PER_TICK: usize = 64;

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
            let execution = self.preflight_execution(&lease).unwrap_or_else(|| {
                self.registry
                    .execute_lease(&lease, &EffectContext::new(&cancelled))
            });
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

        let leases = leases
            .into_iter()
            .map(|lease| {
                let preflight = self.preflight_execution(&lease);
                (lease, preflight)
            })
            .collect::<Vec<_>>();
        let registry = self.registry.clone();
        let executions = thread::scope(|scope| {
            let handles = leases
                .into_iter()
                .map(|(lease, preflight)| {
                    let registry = registry.clone();
                    let fallback_lease = lease.clone();
                    let handle = scope.spawn(move || {
                        if let Some(execution) = preflight {
                            return (lease, execution);
                        }
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

    fn preflight_execution(&self, lease: &EffectLease) -> Option<EffectExecution> {
        match lease.kind.as_str() {
            GEWYVERN_PROVISIONING_EFFECT_KIND => {
                let request = match decode_provisioning_request(&lease.payload) {
                    Ok(request) => request,
                    Err(_) => {
                        return Some(EffectExecution::Reject {
                            error: "provisioning persisted request was rejected".into(),
                        });
                    }
                };
                self.runtime
                    .runtime_projection(&request.request.intent.runtime_id)
                    .is_some()
                    .then(|| EffectExecution::Reject {
                        error: "provisioning runtime identity became unavailable before dispatch"
                            .into(),
                    })
            }
            GEWYVERN_RETIREMENT_EFFECT_KIND => {
                let request = match decode_retirement_request(&lease.payload) {
                    Ok(request) => request,
                    Err(_) => {
                        return Some(EffectExecution::Reject {
                            error: "retirement persisted request was rejected".into(),
                        });
                    }
                };
                self.runtime
                    .runtime_projection(&request.request.intent.runtime_id)
                    .is_none()
                    .then(|| EffectExecution::Reject {
                        error: "retirement runtime is no longer registered before dispatch".into(),
                    })
            }
            _ => None,
        }
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

    pub fn submit_retirement(&mut self, bytes: &[u8]) -> RetirementResponseEnvelope {
        let enabled = self.registry.contains_kind(GEWYVERN_RETIREMENT_EFFECT_KIND);
        retirement_submission::decode_and_submit(&mut self.runtime, bytes, enabled)
    }

    pub fn submit_daemon_retirement(&mut self, bytes: &[u8]) -> DaemonRetirementResponseEnvelope {
        let enabled = self.registry.contains_kind(DAEMON_RETIREMENT_EFFECT_KIND);
        daemon_retirement_submission::decode_and_submit(&mut self.runtime, bytes, enabled)
    }

    fn settle_execution(
        &mut self,
        lease: &EffectLease,
        execution: EffectExecution,
    ) -> Result<WorkerStep, RuntimeError> {
        let EffectExecution::Complete(outcome) = execution else {
            return self.runtime.settle_effect(lease, execution);
        };
        if lease.kind == GEWYVERN_PROVISIONING_EFFECT_KIND {
            return self.settle_provisioning_execution(lease, outcome);
        }
        if lease.kind == GEWYVERN_RETIREMENT_EFFECT_KIND {
            return self.settle_retirement_execution(lease, outcome);
        }
        if lease.kind == DAEMON_RETIREMENT_EFFECT_KIND {
            return self.settle_daemon_retirement_execution(lease, outcome);
        }
        if lease.kind != HOST_BOOTSTRAP_EFFECT_KIND {
            return self
                .runtime
                .settle_effect(lease, EffectExecution::Complete(outcome));
        }
        let request = match decode_bootstrap_request(&lease.payload) {
            Ok(request) => request,
            Err(_) => {
                self.runtime
                    .reject_effect(lease, "bootstrap persisted request was rejected")?;
                return Ok(WorkerStep::Rejected {
                    effect_id: lease.effect_id.clone(),
                    attempt: lease.attempt,
                });
            }
        };
        let existing = self
            .runtime
            .bootstrap_checkpoint(&request.request.intent.bootstrap_id)?;
        let checkpoint =
            match checkpoint_from_bootstrap_effect(&request, &outcome, existing.as_ref()) {
                Ok(checkpoint) => checkpoint,
                Err(error) => {
                    self.runtime.reject_effect(
                        lease,
                        &format!("bootstrap outcome was rejected: {error}"),
                    )?;
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

    fn settle_provisioning_execution(
        &mut self,
        lease: &EffectLease,
        outcome: Vec<u8>,
    ) -> Result<WorkerStep, RuntimeError> {
        let request = match decode_provisioning_request(&lease.payload) {
            Ok(request) => request,
            Err(_) => {
                self.runtime
                    .reject_effect(lease, "provisioning persisted request was rejected")?;
                return Ok(WorkerStep::Rejected {
                    effect_id: lease.effect_id.clone(),
                    attempt: lease.attempt,
                });
            }
        };
        let existing = self
            .runtime
            .provisioning_checkpoint(&request.request.intent.provisioning_id)?;
        let checkpoint =
            match checkpoint_from_provisioning_effect(&request, &outcome, existing.as_ref()) {
                Ok(checkpoint) => checkpoint,
                Err(error) => {
                    self.runtime.reject_effect(
                        lease,
                        &format!("provisioning outcome was rejected: {error}"),
                    )?;
                    return Ok(WorkerStep::Rejected {
                        effect_id: lease.effect_id.clone(),
                        attempt: lease.attempt,
                    });
                }
            };
        if checkpoint.state.phase == ProvisioningPhase::ServiceReady {
            self.runtime
                .complete_provisioning_effect_and_register(lease, &outcome, &checkpoint)?;
        } else {
            self.runtime
                .complete_provisioning_effect(lease, &outcome, &checkpoint)?;
        }
        Ok(WorkerStep::Completed {
            effect_id: lease.effect_id.clone(),
            attempt: lease.attempt,
        })
    }

    fn settle_retirement_execution(
        &mut self,
        lease: &EffectLease,
        outcome: Vec<u8>,
    ) -> Result<WorkerStep, RuntimeError> {
        let request = match decode_retirement_request(&lease.payload) {
            Ok(request) => request,
            Err(_) => {
                self.runtime
                    .reject_effect(lease, "retirement persisted request was rejected")?;
                return Ok(WorkerStep::Rejected {
                    effect_id: lease.effect_id.clone(),
                    attempt: lease.attempt,
                });
            }
        };
        let existing = self
            .runtime
            .retirement_checkpoint(&request.request.intent.retirement_id)?;
        let checkpoint =
            match checkpoint_from_retirement_effect(&request, &outcome, existing.as_ref()) {
                Ok(checkpoint) => checkpoint,
                Err(error) => {
                    self.runtime.reject_effect(
                        lease,
                        &format!("retirement outcome was rejected: {error}"),
                    )?;
                    return Ok(WorkerStep::Rejected {
                        effect_id: lease.effect_id.clone(),
                        attempt: lease.attempt,
                    });
                }
            };
        if checkpoint.state.phase == RetirementPhase::ServiceRetired {
            self.runtime
                .complete_retirement_effect_and_unregister(lease, &outcome, &checkpoint)?;
        } else {
            self.runtime
                .complete_retirement_effect(lease, &outcome, &checkpoint)?;
        }
        Ok(WorkerStep::Completed {
            effect_id: lease.effect_id.clone(),
            attempt: lease.attempt,
        })
    }

    fn settle_daemon_retirement_execution(
        &mut self,
        lease: &EffectLease,
        outcome: Vec<u8>,
    ) -> Result<WorkerStep, RuntimeError> {
        let effect = match decode_daemon_retirement_effect(&lease.payload) {
            Ok(effect) => effect,
            Err(_) => {
                self.runtime
                    .reject_effect(lease, "daemon retirement persisted effect was rejected")?;
                return Ok(WorkerStep::Rejected {
                    effect_id: lease.effect_id.clone(),
                    attempt: lease.attempt,
                });
            }
        };
        let existing = self
            .runtime
            .daemon_retirement_checkpoint(&effect.checkpoint.state.retirement_id)?;
        let checkpoint =
            match checkpoint_from_daemon_retirement_effect(&effect, &outcome, existing.as_ref()) {
                Ok(checkpoint) => checkpoint,
                Err(error) => {
                    self.runtime.reject_effect(
                        lease,
                        &format!("daemon retirement outcome was rejected: {error}"),
                    )?;
                    return Ok(WorkerStep::Rejected {
                        effect_id: lease.effect_id.clone(),
                        attempt: lease.attempt,
                    });
                }
            };
        self.runtime
            .complete_daemon_retirement_effect(lease, &outcome, &checkpoint)?;
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
    request: &BootstrapRequestEnvelope,
    outcome: &[u8],
    existing: Option<&DeploymentBootstrapCheckpoint>,
) -> Result<DeploymentBootstrapCheckpoint, String> {
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
    if state.phase == BootstrapPhase::Bootstrapped
        && (state.generation.is_none() || state.install_profile.is_none())
    {
        return Err("bootstrap response omitted retirement authority".into());
    }
    let credential_handle = match state.phase {
        BootstrapPhase::Bootstrapped => Some(request.request.intent.credential_handle.clone()),
        BootstrapPhase::Failed => None,
        _ => return Err("bootstrap effect stopped before deployment was terminal".into()),
    };
    let revision = match existing {
        Some(checkpoint)
            if checkpoint.revision == 1
                && checkpoint.state.phase == BootstrapPhase::Planned
                && checkpoint.state.bootstrap_id == request.request.intent.bootstrap_id
                && checkpoint.state.target == request.request.intent.target
                && checkpoint.bootstrap_credential_handle.as_ref()
                    == Some(&request.request.intent.credential_handle) =>
        {
            2
        }
        Some(_) => return Err("bootstrap planned checkpoint does not match its request".into()),
        None => 1,
    };
    DeploymentBootstrapCheckpoint::new(revision, state, credential_handle)
        .map_err(|error| error.to_string())
}

fn checkpoint_from_provisioning_effect(
    request: &ProvisioningRequestEnvelope,
    outcome: &[u8],
    existing: Option<&RuntimeProvisioningCheckpoint>,
) -> Result<RuntimeProvisioningCheckpoint, String> {
    let response = decode_provisioning_response(outcome)
        .map_err(|_| "invalid provisioning response".to_string())?;
    let ProvisioningResponse::State(state) = response.response else {
        return Err("provisioning adapter returned an error envelope as success".into());
    };
    if state.provisioning_id != request.request.intent.provisioning_id
        || state.runtime_id != request.request.intent.runtime_id
        || state.target != request.request.intent.target
    {
        return Err("provisioning response identity does not match its request".into());
    }
    if !matches!(
        state.phase,
        ProvisioningPhase::ServiceReady | ProvisioningPhase::Failed
    ) {
        return Err("provisioning effect stopped before installation was terminal".into());
    }
    match existing {
        Some(checkpoint)
            if checkpoint.revision == 1
                && checkpoint.state.phase == ProvisioningPhase::Planned
                && checkpoint.state.provisioning_id == request.request.intent.provisioning_id
                && checkpoint.state.runtime_id == request.request.intent.runtime_id
                && checkpoint.state.target == request.request.intent.target
                && checkpoint.install_credential_handle.as_ref()
                    == Some(&request.request.intent.install_credential_handle) => {}
        Some(_) => return Err("provisioning planned checkpoint does not match its request".into()),
        None => return Err("provisioning planned checkpoint is missing".into()),
    }
    RuntimeProvisioningCheckpoint::new(2, state, None).map_err(|error| error.to_string())
}

fn checkpoint_from_retirement_effect(
    request: &RetirementRequestEnvelope,
    outcome: &[u8],
    existing: Option<&RuntimeRetirementCheckpoint>,
) -> Result<RuntimeRetirementCheckpoint, String> {
    let response = decode_retirement_response(outcome)
        .map_err(|_| "invalid retirement response".to_string())?;
    let RetirementResponse::State(state) = response.response else {
        return Err("retirement adapter returned an error envelope as success".into());
    };
    if state.retirement_id != request.request.intent.retirement_id
        || state.provisioning_id != request.request.intent.provisioning_id
        || state.runtime_id != request.request.intent.runtime_id
        || state.target != request.request.intent.target
    {
        return Err("retirement response identity does not match its request".into());
    }
    if !matches!(
        state.phase,
        RetirementPhase::ServiceRetired | RetirementPhase::Failed
    ) {
        return Err("retirement effect stopped before service retirement was terminal".into());
    }
    match existing {
        Some(checkpoint)
            if checkpoint.revision == 1
                && checkpoint.state.phase == RetirementPhase::Planned
                && checkpoint.state.retirement_id == request.request.intent.retirement_id
                && checkpoint.state.provisioning_id == request.request.intent.provisioning_id
                && checkpoint.state.runtime_id == request.request.intent.runtime_id
                && checkpoint.state.target == request.request.intent.target
                && checkpoint.retirement_credential_handle.as_ref()
                    == Some(&request.request.intent.retirement_credential_handle) => {}
        Some(_) => return Err("retirement planned checkpoint does not match its request".into()),
        None => return Err("retirement planned checkpoint is missing".into()),
    }
    RuntimeRetirementCheckpoint::new(2, state, None).map_err(|error| error.to_string())
}

fn checkpoint_from_daemon_retirement_effect(
    effect: &DaemonRetirementEffectEnvelope,
    outcome: &[u8],
    existing: Option<&DaemonRetirementCheckpoint>,
) -> Result<DaemonRetirementCheckpoint, String> {
    let response = decode_daemon_retirement_response(outcome)
        .map_err(|_| "invalid daemon retirement response".to_string())?;
    let DaemonRetirementResponse::State(state) = response.response else {
        return Err("daemon retirement adapter returned an error envelope as success".into());
    };
    let planned = &effect.checkpoint;
    if state.retirement_id != planned.state.retirement_id
        || state.bootstrap_id != planned.state.bootstrap_id
        || state.daemon_id != planned.state.daemon_id
        || state.target != planned.state.target
        || state.generation != planned.state.generation
        || state.install_profile != planned.state.install_profile
    {
        return Err("daemon retirement response identity does not match its effect".into());
    }
    if !matches!(
        state.phase,
        DaemonRetirementPhase::ServiceRetired | DaemonRetirementPhase::Failed
    ) {
        return Err(
            "daemon retirement effect stopped before service retirement was terminal".into(),
        );
    }
    match existing {
        Some(checkpoint) if checkpoint == planned => {}
        Some(_) => {
            return Err("daemon retirement planned checkpoint does not match its effect".into());
        }
        None => return Err("daemon retirement planned checkpoint is missing".into()),
    }
    DaemonRetirementCheckpoint::new(2, state, None).map_err(|error| error.to_string())
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
    use leserpent_domain::bootstrap_retirement::{
        CAPABILITY_HOST_RETIRE, DAEMON_RETIREMENT_DOMAIN_SCHEMA_VERSION, DaemonRetirement,
        DaemonRetirementIntent, DaemonRetirementReceipt,
    };
    use leserpent_domain::provisioning::{
        CAPABILITY_RUNTIME_PROVISION, GewyvernServiceReceipt, PROVISIONING_DOMAIN_SCHEMA_VERSION,
        ProvisioningId, RuntimeProvisioning, RuntimeProvisioningIntent,
    };
    use leserpent_domain::retirement::{
        CAPABILITY_RUNTIME_RETIRE, GewyvernRetirementReceipt, RETIREMENT_DOMAIN_SCHEMA_VERSION,
        RetirementId, RuntimeRetirement, RuntimeRetirementIntent,
    };
    use leserpent_domain::{CapabilitySet, Principal};
    use leserpent_protocol::bootstrap::{
        BOOTSTRAP_PROTOCOL_SCHEMA_VERSION, BootstrapRequest, BootstrapRequestEnvelope,
        BootstrapResponseEnvelope, encode_bootstrap_request, encode_bootstrap_response,
    };
    use leserpent_protocol::bootstrap_retirement_control::{
        DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION, DaemonRetirementRequest,
        DaemonRetirementRequestEnvelope, DaemonRetirementResponseEnvelope,
        encode_daemon_retirement_request, encode_daemon_retirement_response,
    };
    use leserpent_protocol::provisioning::{
        PROVISIONING_PROTOCOL_SCHEMA_VERSION, ProvisioningRequest, ProvisioningResponseEnvelope,
        encode_provisioning_request, encode_provisioning_response,
    };
    use leserpent_protocol::retirement::{
        RETIREMENT_PROTOCOL_SCHEMA_VERSION, RetirementRequest, RetirementRequestEnvelope,
        RetirementResponseEnvelope, encode_retirement_request, encode_retirement_response,
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

    struct FixedProvisioningAdapter {
        outcome: Vec<u8>,
    }

    struct FixedRetirementAdapter {
        outcome: Vec<u8>,
    }

    struct FixedDaemonRetirementAdapter {
        outcome: Vec<u8>,
    }

    struct TrackingProvisioningAdapter {
        called: Arc<AtomicBool>,
    }

    impl EffectAdapter for FixedProvisioningAdapter {
        fn kind(&self) -> &str {
            GEWYVERN_PROVISIONING_EFFECT_KIND
        }

        fn execute(&mut self, _payload: &[u8]) -> EffectExecution {
            EffectExecution::Complete(self.outcome.clone())
        }
    }

    impl EffectAdapter for FixedRetirementAdapter {
        fn kind(&self) -> &str {
            GEWYVERN_RETIREMENT_EFFECT_KIND
        }

        fn execute(&mut self, _payload: &[u8]) -> EffectExecution {
            EffectExecution::Complete(self.outcome.clone())
        }
    }

    impl EffectAdapter for FixedDaemonRetirementAdapter {
        fn kind(&self) -> &str {
            DAEMON_RETIREMENT_EFFECT_KIND
        }

        fn execute(&mut self, _payload: &[u8]) -> EffectExecution {
            EffectExecution::Complete(self.outcome.clone())
        }
    }

    impl EffectAdapter for TrackingProvisioningAdapter {
        fn kind(&self) -> &str {
            GEWYVERN_PROVISIONING_EFFECT_KIND
        }

        fn execute(&mut self, _payload: &[u8]) -> EffectExecution {
            self.called.store(true, Ordering::Release);
            EffectExecution::Reject {
                error: "tracking adapter must not execute".into(),
            }
        }
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
                generation: "a".repeat(64),
                install_profile: "system".into(),
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

    fn provisioning_request_and_outcome() -> (Vec<u8>, Vec<u8>, ProvisioningId) {
        let provisioning_id = ProvisioningId::new("provision-restart-1").unwrap();
        let target = BootstrapTarget {
            transport: BootstrapTransport::Ssh,
            host: "runtime-host.example".into(),
            port: 22,
        };
        let intent = RuntimeProvisioningIntent {
            schema_version: PROVISIONING_DOMAIN_SCHEMA_VERSION,
            provisioning_id: provisioning_id.clone(),
            runtime_id: leserpent_domain::RuntimeId::new("runtime-provisioned-1").unwrap(),
            target,
            install_credential_handle: CredentialHandle::new("vault:ssh:runtime-host").unwrap(),
            requested_by: "operator-a".into(),
            confirmed: true,
        };
        let principal = Principal {
            id: "operator-a".into(),
        };
        let capabilities = CapabilitySet::new([CAPABILITY_RUNTIME_PROVISION]);
        let request = encode_provisioning_request(&ProvisioningRequestEnvelope {
            schema_version: PROVISIONING_PROTOCOL_SCHEMA_VERSION,
            request: ProvisioningRequest {
                principal: principal.clone(),
                capabilities: capabilities.clone(),
                intent: intent.clone(),
            },
        })
        .unwrap();
        let mut provisioning =
            RuntimeProvisioning::plan(&principal, &capabilities, intent).unwrap();
        provisioning.begin().unwrap();
        let state = provisioning
            .accept_service(GewyvernServiceReceipt {
                provisioning_id: provisioning_id.clone(),
                runtime_id: leserpent_domain::RuntimeId::new("runtime-provisioned-1").unwrap(),
                endpoint: "https://runtime-host.example:9443".into(),
                api_credential_handle: CredentialHandle::new("vault:gewyvern:runtime-api").unwrap(),
                trust_credential_handle: CredentialHandle::new("vault:gewyvern:runtime-ca")
                    .unwrap(),
            })
            .unwrap();
        let outcome = encode_provisioning_response(&ProvisioningResponseEnvelope {
            schema_version: PROVISIONING_PROTOCOL_SCHEMA_VERSION,
            response: ProvisioningResponse::State(state),
        })
        .unwrap();
        (request, outcome, provisioning_id)
    }

    fn daemon_retirement_request_and_outcome(
        runtime: &mut ControlRuntime,
        bootstrap_id: &BootstrapId,
    ) -> (Vec<u8>, Vec<u8>, RetirementId) {
        let retirement_id = RetirementId::new("retire-daemon-restart-1").unwrap();
        let request = DaemonRetirementRequestEnvelope {
            schema_version: DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION,
            request: DaemonRetirementRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_HOST_RETIRE]),
                intent: DaemonRetirementIntent {
                    schema_version: DAEMON_RETIREMENT_DOMAIN_SCHEMA_VERSION,
                    retirement_id: retirement_id.clone(),
                    bootstrap_id: bootstrap_id.clone(),
                    retirement_credential_handle: CredentialHandle::new("vault:ssh:host-example")
                        .unwrap(),
                    requested_by: "operator-a".into(),
                    confirmed: true,
                },
            },
        };
        let deployment = runtime.bootstrap_checkpoint(bootstrap_id).unwrap().unwrap();
        let mut retirement = DaemonRetirement::plan(
            &request.request.principal,
            &request.request.capabilities,
            request.request.intent.clone(),
            &deployment,
        )
        .unwrap();
        retirement.begin().unwrap();
        let planned = retirement.snapshot();
        let state = retirement
            .accept_service_retirement(DaemonRetirementReceipt {
                retirement_id: retirement_id.clone(),
                bootstrap_id: bootstrap_id.clone(),
                daemon_id: planned.daemon_id,
                generation: planned.generation,
                service_retired: true,
            })
            .unwrap();
        let request = encode_daemon_retirement_request(&request).unwrap();
        let outcome = encode_daemon_retirement_response(&DaemonRetirementResponseEnvelope {
            schema_version: DAEMON_RETIREMENT_PROTOCOL_SCHEMA_VERSION,
            response: DaemonRetirementResponse::State(state),
        })
        .unwrap();
        (request, outcome, retirement_id)
    }

    fn retirement_request_and_outcome(
        receipt_runtime_id: &str,
    ) -> (Vec<u8>, Vec<u8>, RetirementId) {
        let retirement_id = RetirementId::new("retire-restart-1").unwrap();
        let target = BootstrapTarget {
            transport: BootstrapTransport::Ssh,
            host: "runtime-host.example".into(),
            port: 22,
        };
        let intent = RuntimeRetirementIntent {
            schema_version: RETIREMENT_DOMAIN_SCHEMA_VERSION,
            retirement_id: retirement_id.clone(),
            provisioning_id: ProvisioningId::new("provision-restart-1").unwrap(),
            runtime_id: leserpent_domain::RuntimeId::new("runtime-provisioned-1").unwrap(),
            target,
            retirement_credential_handle: CredentialHandle::new("vault:ssh:runtime-retirement")
                .unwrap(),
            requested_by: "operator-a".into(),
            confirmed: true,
        };
        let principal = Principal {
            id: "operator-a".into(),
        };
        let capabilities = CapabilitySet::new([CAPABILITY_RUNTIME_RETIRE]);
        let request = encode_retirement_request(&RetirementRequestEnvelope {
            schema_version: RETIREMENT_PROTOCOL_SCHEMA_VERSION,
            request: RetirementRequest {
                principal: principal.clone(),
                capabilities: capabilities.clone(),
                intent: intent.clone(),
            },
        })
        .unwrap();
        let mut retirement = RuntimeRetirement::plan(&principal, &capabilities, intent).unwrap();
        retirement.begin().unwrap();
        let mut state = retirement
            .accept_service_retirement(GewyvernRetirementReceipt {
                retirement_id: retirement_id.clone(),
                provisioning_id: ProvisioningId::new("provision-restart-1").unwrap(),
                runtime_id: leserpent_domain::RuntimeId::new("runtime-provisioned-1").unwrap(),
                service_retired: true,
            })
            .unwrap();
        state.runtime_id = leserpent_domain::RuntimeId::new(receipt_runtime_id).unwrap();
        let outcome = encode_retirement_response(&RetirementResponseEnvelope {
            schema_version: RETIREMENT_PROTOCOL_SCHEMA_VERSION,
            response: RetirementResponse::State(state),
        })
        .unwrap();
        (request, outcome, retirement_id)
    }

    fn host_with_registered_runtime(
        path: &PathBuf,
        retirement_outcome: Vec<u8>,
    ) -> (DaemonHost, leserpent_domain::RuntimeId, ProvisioningId) {
        let (request, provisioning_outcome, provisioning_id) = provisioning_request_and_outcome();
        let runtime_id = leserpent_domain::RuntimeId::new("runtime-provisioned-1").unwrap();
        let mut runtime = ControlRuntime::open(path).unwrap();
        let submitted =
            crate::provisioning_submission::decode_and_submit(&mut runtime, &request, true);
        assert!(matches!(
            submitted.response,
            ProvisioningResponse::State(ref state)
                if state.phase == ProvisioningPhase::Planned
        ));
        let mut registry = AdapterRegistry::default();
        registry
            .register(FixedProvisioningAdapter {
                outcome: provisioning_outcome,
            })
            .unwrap();
        registry
            .register(FixedRetirementAdapter {
                outcome: retirement_outcome,
            })
            .unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        assert!(matches!(
            host.tick().unwrap(),
            WorkerStep::Completed { attempt: 1, .. }
        ));
        assert!(host.runtime_mut().runtime_projection(&runtime_id).is_some());
        (host, runtime_id, provisioning_id)
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
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let submitted =
            crate::bootstrap_submission::decode_and_submit(&mut runtime, &request, true);
        assert!(matches!(
            submitted.response,
            BootstrapResponse::State(ref state)
                if state.phase == BootstrapPhase::Planned
                    && state.bootstrap_id == bootstrap_id
        ));
        assert_eq!(
            runtime
                .bootstrap_checkpoint(&bootstrap_id)
                .unwrap()
                .unwrap()
                .revision,
            1
        );
        let replay = crate::bootstrap_submission::decode_and_submit(&mut runtime, &request, true);
        assert_eq!(replay, submitted);
        let mut registry = AdapterRegistry::default();
        registry
            .register(FixedBootstrapAdapter { outcome })
            .unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        assert!(matches!(
            host.tick().unwrap(),
            WorkerStep::Completed { attempt: 1, .. }
        ));
        let before_restart = host
            .runtime_mut()
            .bootstrap_checkpoint(&bootstrap_id)
            .unwrap()
            .unwrap();
        assert_eq!(before_restart.revision, 2);
        assert_eq!(before_restart.state.phase, BootstrapPhase::Bootstrapped);
        assert_eq!(
            before_restart.state.generation.as_deref().unwrap(),
            "a".repeat(64)
        );
        assert_eq!(
            before_restart.state.install_profile.as_deref(),
            Some("system")
        );
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
        assert_eq!(recovered_checkpoint.revision, 2);
        assert_eq!(
            recovered_checkpoint.state.phase,
            BootstrapPhase::Bootstrapped
        );
        assert_eq!(
            recovered_checkpoint.state.generation.as_deref().unwrap(),
            "a".repeat(64)
        );
        assert_eq!(
            recovered_checkpoint.state.install_profile.as_deref(),
            Some("system")
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
            2
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
        assert_eq!(durable.revision, 3);
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
            3
        );
        drop(restarted);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn daemon_retirement_submission_settles_atomically_and_survives_restart() {
        let path = temp_database("daemon-retirement-restart");
        let (bootstrap_request, bootstrap_outcome, bootstrap_id) = bootstrap_request_and_outcome();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let submitted =
            crate::bootstrap_submission::decode_and_submit(&mut runtime, &bootstrap_request, true);
        assert!(matches!(
            submitted.response,
            BootstrapResponse::State(ref state) if state.phase == BootstrapPhase::Planned
        ));
        let lease = runtime
            .claim_effect("bootstrap-seed", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        let request = decode_bootstrap_request(&lease.payload).unwrap();
        let existing = runtime.bootstrap_checkpoint(&bootstrap_id).unwrap();
        let checkpoint =
            checkpoint_from_bootstrap_effect(&request, &bootstrap_outcome, existing.as_ref())
                .unwrap();
        runtime
            .complete_bootstrap_effect(&lease, &bootstrap_outcome, &checkpoint)
            .unwrap();
        runtime
            .bind_bootstrap_session(
                &bootstrap_id,
                DaemonSessionProof {
                    bootstrap_id: bootstrap_id.clone(),
                    daemon_id: DaemonId::new("daemon-host-example").unwrap(),
                    session_credential_handle: CredentialHandle::new(
                        "vault:leserpentd:host-example",
                    )
                    .unwrap(),
                    trust_credential_handle: CredentialHandle::new(
                        "vault:leserpent-ca:host-example",
                    )
                    .unwrap(),
                    authority_owned: true,
                    protocol_schema_version: BOOTSTRAP_SESSION_PROTOCOL_VERSION,
                },
            )
            .unwrap();
        let (retirement_request, retirement_outcome, retirement_id) =
            daemon_retirement_request_and_outcome(&mut runtime, &bootstrap_id);

        let mut registry = AdapterRegistry::default();
        registry
            .register(FixedDaemonRetirementAdapter {
                outcome: retirement_outcome,
            })
            .unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        let submitted = host.submit_daemon_retirement(&retirement_request);
        assert!(matches!(
            submitted.response,
            DaemonRetirementResponse::State(ref state)
                if state.phase == DaemonRetirementPhase::Planned
                    && state.retirement_id == retirement_id
        ));
        assert_eq!(
            host.submit_daemon_retirement(&retirement_request),
            submitted
        );
        assert!(matches!(
            host.tick().unwrap(),
            WorkerStep::Completed { attempt: 1, .. }
        ));
        let terminal = host
            .runtime_mut()
            .daemon_retirement_checkpoint(&retirement_id)
            .unwrap()
            .unwrap();
        assert_eq!(terminal.revision, 2);
        assert_eq!(terminal.state.phase, DaemonRetirementPhase::ServiceRetired);
        assert!(terminal.retirement_credential_handle.is_none());
        drop(host);

        let mut restarted = ControlRuntime::open(&path).unwrap();
        assert_eq!(
            restarted
                .daemon_retirement_checkpoint(&retirement_id)
                .unwrap()
                .unwrap(),
            terminal
        );
        let replay = crate::daemon_retirement_submission::decode_and_submit(
            &mut restarted,
            &retirement_request,
            true,
        );
        assert_eq!(
            replay.response,
            DaemonRetirementResponse::State(terminal.state)
        );
        drop(restarted);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn provisioning_submission_registers_runtime_atomically_and_survives_restart() {
        let path = temp_database("provisioning-runtime-registered-restart");
        let (request, outcome, provisioning_id) = provisioning_request_and_outcome();
        let runtime_id = leserpent_domain::RuntimeId::new("runtime-provisioned-1").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let submitted =
            crate::provisioning_submission::decode_and_submit(&mut runtime, &request, true);
        assert!(matches!(
            submitted.response,
            ProvisioningResponse::State(ref state)
                if state.phase == ProvisioningPhase::Planned
                    && state.provisioning_id == provisioning_id
        ));
        assert_eq!(
            runtime
                .provisioning_checkpoint(&provisioning_id)
                .unwrap()
                .unwrap()
                .revision,
            1
        );
        let replay =
            crate::provisioning_submission::decode_and_submit(&mut runtime, &request, true);
        assert_eq!(replay, submitted);

        let mut registry = AdapterRegistry::default();
        registry
            .register(FixedProvisioningAdapter { outcome })
            .unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        assert!(matches!(
            host.tick().unwrap(),
            WorkerStep::Completed { attempt: 1, .. }
        ));
        let checkpoint = host
            .runtime_mut()
            .provisioning_checkpoint(&provisioning_id)
            .unwrap()
            .unwrap();
        assert_eq!(checkpoint.revision, 3);
        assert_eq!(checkpoint.state.phase, ProvisioningPhase::RuntimeRegistered);
        assert!(checkpoint.install_credential_handle.is_none());
        let projection = host.runtime_mut().runtime_projection(&runtime_id).unwrap();
        assert_eq!(projection.name, "runtime-provisioned-1");
        assert_eq!(projection.endpoint, "https://runtime-host.example:9443");

        drop(host);
        let mut restarted = ControlRuntime::open(&path).unwrap();
        let restored = restarted
            .provisioning_checkpoint(&provisioning_id)
            .unwrap()
            .unwrap();
        assert_eq!(restored, checkpoint);
        assert_eq!(
            restarted.runtime_projection(&runtime_id).unwrap().endpoint,
            "https://runtime-host.example:9443"
        );
        let terminal_replay =
            crate::provisioning_submission::decode_and_submit(&mut restarted, &request, true);
        assert!(matches!(
            terminal_replay.response,
            ProvisioningResponse::State(ref state)
                if state.phase == ProvisioningPhase::RuntimeRegistered
        ));
        drop(restarted);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn retirement_submission_unregisters_runtime_atomically_and_survives_restart() {
        let path = temp_database("retirement-runtime-unregistered-restart");
        let (request, outcome, retirement_id) =
            retirement_request_and_outcome("runtime-provisioned-1");
        let (mut host, runtime_id, _) = host_with_registered_runtime(&path, outcome);

        let submitted = host.submit_retirement(&request);
        assert!(matches!(
            submitted.response,
            RetirementResponse::State(ref state)
                if state.phase == RetirementPhase::Planned
                    && state.retirement_id == retirement_id
        ));
        assert_eq!(host.submit_retirement(&request), submitted);
        assert!(matches!(
            host.tick().unwrap(),
            WorkerStep::Completed { attempt: 1, .. }
        ));
        assert!(host.runtime_mut().runtime_projection(&runtime_id).is_none());
        let checkpoint = host
            .runtime_mut()
            .retirement_checkpoint(&retirement_id)
            .unwrap()
            .unwrap();
        assert_eq!(checkpoint.revision, 3);
        assert_eq!(checkpoint.state.phase, RetirementPhase::RuntimeUnregistered);
        drop(host);

        let mut restarted = ControlRuntime::open(&path).unwrap();
        assert!(restarted.runtime_projection(&runtime_id).is_none());
        assert_eq!(
            restarted
                .retirement_checkpoint(&retirement_id)
                .unwrap()
                .unwrap(),
            checkpoint
        );
        drop(restarted);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn retirement_worker_rejects_forged_receipt_without_unregistering_runtime() {
        let path = temp_database("retirement-forged-receipt");
        let (request, forged_outcome, retirement_id) =
            retirement_request_and_outcome("runtime-forged");
        let (mut host, runtime_id, _) = host_with_registered_runtime(&path, forged_outcome);

        assert!(matches!(
            host.submit_retirement(&request).response,
            RetirementResponse::State(ref state) if state.phase == RetirementPhase::Planned
        ));
        assert!(matches!(
            host.tick().unwrap(),
            WorkerStep::Rejected { attempt: 1, .. }
        ));
        assert!(host.runtime_mut().runtime_projection(&runtime_id).is_some());
        let checkpoint = host
            .runtime_mut()
            .retirement_checkpoint(&retirement_id)
            .unwrap()
            .unwrap();
        assert_eq!(checkpoint.revision, 1);
        assert_eq!(checkpoint.state.phase, RetirementPhase::Planned);
        assert_eq!(host.runtime_mut().effect_queue_stats().unwrap().failed, 1);
        drop(host);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn provisioning_rejects_registered_runtime_identity_before_effect_submission() {
        let path = temp_database("provisioning-runtime-identity-preflight");
        let (request, _, provisioning_id) = provisioning_request_and_outcome();
        let runtime_id = leserpent_domain::RuntimeId::new("runtime-provisioned-1").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(
                runtime_id,
                "existing-runtime",
                "https://existing.example:9443",
            )
            .unwrap();

        let response =
            crate::provisioning_submission::decode_and_submit(&mut runtime, &request, true);
        assert!(matches!(
            response.response,
            ProvisioningResponse::Error(ref error)
                if error.provisioning_id.as_ref() == Some(&provisioning_id)
                    && error.code == "runtime_identity_conflict"
        ));
        assert!(
            runtime
                .provisioning_checkpoint(&provisioning_id)
                .unwrap()
                .is_none()
        );
        assert_eq!(runtime.effect_queue_stats().unwrap().active(), 0);
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn provisioning_worker_rechecks_runtime_identity_before_batch_adapter_dispatch() {
        let path = temp_database("provisioning-runtime-identity-dispatch-race");
        let (request, _, provisioning_id) = provisioning_request_and_outcome();
        let runtime_id = leserpent_domain::RuntimeId::new("runtime-provisioned-1").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let submitted =
            crate::provisioning_submission::decode_and_submit(&mut runtime, &request, true);
        assert!(matches!(submitted.response, ProvisioningResponse::State(_)));
        runtime
            .register_runtime(
                runtime_id,
                "competing-runtime",
                "https://competing.example:9443",
            )
            .unwrap();
        let called = Arc::new(AtomicBool::new(false));
        let mut registry = AdapterRegistry::default();
        registry
            .register(TrackingProvisioningAdapter {
                called: Arc::clone(&called),
            })
            .unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();

        assert!(matches!(
            host.tick_batch(&AtomicBool::new(false)).unwrap().as_slice(),
            [WorkerStep::Rejected { attempt: 1, .. }]
        ));
        assert!(!called.load(Ordering::Acquire));
        let checkpoint = host
            .runtime_mut()
            .provisioning_checkpoint(&provisioning_id)
            .unwrap()
            .unwrap();
        assert_eq!(checkpoint.revision, 1);
        assert_eq!(checkpoint.state.phase, ProvisioningPhase::Planned);
        drop(host);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn provisioning_submission_promotes_a_legacy_ready_checkpoint() {
        let path = temp_database("provisioning-legacy-ready-promotion");
        let (request_bytes, outcome, provisioning_id) = provisioning_request_and_outcome();
        let request = decode_provisioning_request(&request_bytes).unwrap();
        let runtime_id = request.request.intent.runtime_id.clone();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let submitted =
            crate::provisioning_submission::decode_and_submit(&mut runtime, &request_bytes, true);
        assert!(matches!(submitted.response, ProvisioningResponse::State(_)));
        let lease = runtime
            .claim_effect("legacy-worker", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        let planned = runtime
            .provisioning_checkpoint(&provisioning_id)
            .unwrap()
            .unwrap();
        let ready =
            checkpoint_from_provisioning_effect(&request, &outcome, Some(&planned)).unwrap();
        runtime
            .complete_provisioning_effect(&lease, &outcome, &ready)
            .unwrap();
        assert_eq!(
            runtime
                .provisioning_checkpoint(&provisioning_id)
                .unwrap()
                .unwrap()
                .state
                .phase,
            ProvisioningPhase::ServiceReady
        );

        let promoted =
            crate::provisioning_submission::decode_and_submit(&mut runtime, &request_bytes, true);
        assert!(matches!(
            promoted.response,
            ProvisioningResponse::State(ref state)
                if state.phase == ProvisioningPhase::RuntimeRegistered
        ));
        assert_eq!(
            runtime.runtime_projection(&runtime_id).unwrap().endpoint,
            "https://runtime-host.example:9443"
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn provisioning_identity_drift_is_rejected_without_advancing_checkpoint() {
        let path = temp_database("provisioning-identity-drift");
        let (request, outcome, provisioning_id) = provisioning_request_and_outcome();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        let submitted =
            crate::provisioning_submission::decode_and_submit(&mut runtime, &request, true);
        assert!(matches!(submitted.response, ProvisioningResponse::State(_)));

        let mut value: serde_json::Value = serde_json::from_slice(&outcome).unwrap();
        value["response"]["payload"]["runtime_id"] =
            serde_json::Value::String("runtime-confused".into());
        let mut registry = AdapterRegistry::default();
        registry
            .register(FixedProvisioningAdapter {
                outcome: serde_json::to_vec(&value).unwrap(),
            })
            .unwrap();
        let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default()).unwrap();
        assert!(matches!(
            host.tick().unwrap(),
            WorkerStep::Rejected { attempt: 1, .. }
        ));
        let checkpoint = host
            .runtime_mut()
            .provisioning_checkpoint(&provisioning_id)
            .unwrap()
            .unwrap();
        assert_eq!(checkpoint.revision, 1);
        assert_eq!(checkpoint.state.phase, ProvisioningPhase::Planned);
        assert!(checkpoint.install_credential_handle.is_some());
        drop(host);
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

    #[test]
    fn new_bootstrap_outcome_requires_retirement_authority() {
        let (request, outcome, _) = bootstrap_request_and_outcome();
        let request = decode_bootstrap_request(&request).unwrap();
        let mut response = decode_bootstrap_response(&outcome).unwrap();
        let BootstrapResponse::State(state) = &mut response.response else {
            panic!("test bootstrap outcome must be a state");
        };
        state.generation = None;
        state.install_profile = None;
        let outcome = encode_bootstrap_response(&response).unwrap();

        assert_eq!(
            checkpoint_from_bootstrap_effect(&request, &outcome, None),
            Err("bootstrap response omitted retirement authority".into())
        );
    }
}
