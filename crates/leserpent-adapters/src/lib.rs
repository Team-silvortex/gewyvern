use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use leserpent_runtime::{EffectExecution, EffectExecutor, EffectLease};

mod bootstrap;
mod bootstrap_trust;
mod deployment;
mod discovery;
mod gewyvern;
#[cfg(feature = "native-ssh")]
mod native_ssh;
mod provisioning;
mod retirement;
mod secret;

#[cfg(feature = "native-ssh")]
pub use bootstrap::NativeSshBootstrapTransport;
pub use bootstrap::{
    BootstrapArtifact, DAEMON_RETIREMENT_EFFECT_KIND, HOST_BOOTSTRAP_EFFECT_KIND,
    MAX_BOOTSTRAP_ARTIFACT_BYTES, SshBootstrapAdapter, SshBootstrapHostPolicy, SshBootstrapJob,
    SshBootstrapOutcome, SshBootstrapRetirementJob, SshBootstrapRetirementTransport,
    SshBootstrapRetirementTransportError, SshBootstrapTransport, SshBootstrapTransportError,
    SshDaemonRetirementAdapter,
};
pub use bootstrap_trust::{
    BootstrapTrustError, BootstrapTrustRecord, BootstrapTrustStore, FileBootstrapTrustStore,
};
pub use deployment::{
    GEWYVERN_DEPLOYMENT_EFFECT_KIND, GewyvernDeploymentAdapter, GewyvernDeploymentRequest,
    GewyvernDeploymentResponse,
};
pub use discovery::{
    GEWYVERN_DISCOVERY_EFFECT_KIND, GewyvernCapabilityObservation, GewyvernDiscoveryAdapter,
    GewyvernDiscoveryRequest,
};
pub use gewyvern::{
    GEWYVERN_HEALTH_EFFECT_KIND, GEWYVERN_STATUS_REFRESH_EFFECT_KIND, GewyvernHealthAdapter,
    GewyvernStatusObservation, GewyvernStatusRefreshAdapter, GewyvernStatusRefreshRequest,
    GewyvernTarget, GewyvernTargetCatalog, validate_gewyvern_admin_secret,
};
#[cfg(feature = "native-ssh")]
pub use provisioning::NativeSshGewyvernProvisioningTransport;
pub use provisioning::{
    GEWYVERN_PROVISIONING_EFFECT_KIND, GewyvernArtifact, GewyvernProvisioningAdapter,
    GewyvernProvisioningJob, GewyvernProvisioningTransport, GewyvernProvisioningTransportError,
    MAX_GEWYVERN_ARTIFACT_BYTES, SshGewyvernHostPolicy,
};
#[cfg(feature = "native-ssh")]
pub use retirement::NativeSshGewyvernRetirementTransport;
pub use retirement::{
    GEWYVERN_RETIREMENT_EFFECT_KIND, GewyvernRetirementAdapter, GewyvernRetirementJob,
    GewyvernRetirementTransport, GewyvernRetirementTransportError,
};
pub use secret::{
    ConfiguredSecretStore, EmptySecretStore, EnvironmentSecretStore, MAX_SECRET_BYTES,
    MutableSecretStore, PlatformSecretStore, SecretKey, SecretStore, SecretStoreError, SecretValue,
};

pub trait EffectAdapter: Send {
    fn kind(&self) -> &str;
    fn execute(&mut self, payload: &[u8]) -> EffectExecution;

    fn execute_with_context(
        &mut self,
        payload: &[u8],
        _context: &EffectContext<'_>,
    ) -> EffectExecution {
        self.execute(payload)
    }
}

#[derive(Clone, Copy)]
pub struct EffectContext<'a> {
    cancelled: &'a AtomicBool,
}

impl<'a> EffectContext<'a> {
    pub fn new(cancelled: &'a AtomicBool) -> Self {
        Self { cancelled }
    }

    pub fn is_cancelled(self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

type SharedAdapter = Arc<Mutex<Box<dyn EffectAdapter>>>;

#[derive(Clone, Default)]
pub struct AdapterRegistry {
    adapters: BTreeMap<String, SharedAdapter>,
}

impl AdapterRegistry {
    pub fn register(&mut self, adapter: impl EffectAdapter + 'static) -> Result<(), String> {
        let kind = adapter.kind().to_string();
        validate_id("adapter kind", &kind)?;
        if self.adapters.contains_key(&kind) {
            return Err(format!("adapter kind '{kind}' is already registered"));
        }
        self.adapters
            .insert(kind, Arc::new(Mutex::new(Box::new(adapter))));
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.adapters.is_empty()
    }

    pub fn len(&self) -> usize {
        self.adapters.len()
    }

    pub fn contains_kind(&self, kind: &str) -> bool {
        self.adapters.contains_key(kind)
    }

    pub fn execute_lease(
        &self,
        lease: &EffectLease,
        context: &EffectContext<'_>,
    ) -> EffectExecution {
        if context.is_cancelled() {
            return EffectExecution::Retry {
                error: "adapter execution was cancelled".into(),
                after: Duration::ZERO,
            };
        }
        let Some(adapter) = self.adapters.get(&lease.kind) else {
            return EffectExecution::Reject {
                error: format!("no adapter is registered for effect kind '{}'", lease.kind),
            };
        };
        match adapter.lock() {
            Ok(mut adapter) => adapter.execute_with_context(&lease.payload, context),
            Err(_) => EffectExecution::Reject {
                error: "adapter became unavailable after an execution panic".into(),
            },
        }
    }
}

impl EffectExecutor for AdapterRegistry {
    fn execute(&mut self, lease: &EffectLease) -> EffectExecution {
        static NOT_CANCELLED: AtomicBool = AtomicBool::new(false);
        self.execute_lease(lease, &EffectContext::new(&NOT_CANCELLED))
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
