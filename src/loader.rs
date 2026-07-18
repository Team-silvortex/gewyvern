use crate::fragment::AttachFailure;
use crate::fragment::AttachPlan;

pub const LINUX_SMOKE_FRAGMENT_ID: &str = "linux_tracepoint_smoke_fragment";

#[derive(Debug, Eq, PartialEq)]
pub enum LoaderError {
    UnsupportedPlatform,
    InvalidProbeTarget(String),
    LaunchFailed(String),
}

pub trait Loader {
    fn collect_failures(&self, plan: &AttachPlan) -> Result<Vec<AttachFailure>, LoaderError>;
}

#[derive(Clone, Debug, Default)]
pub struct NoopLoader;

impl Loader for NoopLoader {
    fn collect_failures(&self, _plan: &AttachPlan) -> Result<Vec<AttachFailure>, LoaderError> {
        Ok(Vec::new())
    }
}

#[derive(Clone, Debug, Default)]
pub struct StaticFailureLoader {
    pub failures: Vec<AttachFailure>,
}

impl Loader for StaticFailureLoader {
    fn collect_failures(&self, _plan: &AttachPlan) -> Result<Vec<AttachFailure>, LoaderError> {
        Ok(self.failures.clone())
    }
}

#[derive(Clone, Copy, Debug)]
pub enum LinuxProbeMode {
    Kernel,
    TracepointsOnly,
    SingleTracepointSmoke(&'static str),
}

#[derive(Clone, Copy, Debug)]
pub struct LinuxProbeLoader {
    mode: LinuxProbeMode,
}

impl LinuxProbeLoader {
    pub fn kernel() -> Self {
        Self {
            mode: LinuxProbeMode::Kernel,
        }
    }

    pub fn tracepoints_only() -> Self {
        Self {
            mode: LinuxProbeMode::TracepointsOnly,
        }
    }

    pub fn single_tracepoint_smoke(hookpoint_name: &'static str) -> Self {
        Self {
            mode: LinuxProbeMode::SingleTracepointSmoke(hookpoint_name),
        }
    }
}

impl Loader for LinuxProbeLoader {
    fn collect_failures(&self, plan: &AttachPlan) -> Result<Vec<AttachFailure>, LoaderError> {
        match self.mode {
            LinuxProbeMode::Kernel => linux_probe_kernel_hooks(plan),
            LinuxProbeMode::TracepointsOnly => linux_probe_tracepoint_hooks(plan),
            LinuxProbeMode::SingleTracepointSmoke(name) => linux_tracepoint_smoke_failures(name),
        }
    }
}

#[cfg(target_os = "linux")]
pub fn linux_tracepoint_smoke_failures(
    hookpoint_name: &str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    use crate::fragment::HookPoint;
    crate::linux_ebpf_smoke::validate_tracepoint_name(hookpoint_name).map_err(map_smoke_error)?;
    let message = match crate::linux_ebpf_smoke::run_tracepoint_attach_smoke(hookpoint_name, None) {
        Ok(()) => return Ok(Vec::new()),
        Err(err) => err.to_string(),
    };

    Ok(vec![AttachFailure {
        fragment_id: LINUX_SMOKE_FRAGMENT_ID.to_string(),
        hookpoint: HookPoint::TracePoint(hookpoint_name.to_string()),
        error: message,
    }])
}

#[cfg(not(target_os = "linux"))]
pub fn linux_tracepoint_smoke_failures(
    _hookpoint_name: &str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    Err(LoaderError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
pub fn linux_probe_tracepoint_hook(
    fragment_id: &str,
    hookpoint_name: &str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    use crate::fragment::HookPoint;
    crate::linux_ebpf_smoke::validate_tracepoint_name(hookpoint_name).map_err(map_smoke_error)?;
    let message = match crate::linux_ebpf_smoke::run_tracepoint_attach_smoke(hookpoint_name, None) {
        Ok(()) => return Ok(Vec::new()),
        Err(err) => err.to_string(),
    };

    Ok(vec![AttachFailure {
        fragment_id: fragment_id.to_string(),
        hookpoint: HookPoint::TracePoint(hookpoint_name.to_string()),
        error: message,
    }])
}

#[cfg(not(target_os = "linux"))]
pub fn linux_probe_tracepoint_hook(
    _fragment_id: &str,
    _hookpoint_name: &str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    Err(LoaderError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
pub fn linux_probe_kprobe_hook(
    fragment_id: &str,
    symbol_name: &str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    use crate::fragment::HookPoint;
    crate::linux_ebpf_smoke::validate_symbol_name(symbol_name).map_err(map_smoke_error)?;
    let message = match crate::linux_ebpf_smoke::run_kprobe_attach_smoke(symbol_name, None) {
        Ok(()) => return Ok(Vec::new()),
        Err(err) => err.to_string(),
    };

    Ok(vec![AttachFailure {
        fragment_id: fragment_id.to_string(),
        hookpoint: HookPoint::KProbe(symbol_name.to_string()),
        error: message,
    }])
}

#[cfg(not(target_os = "linux"))]
pub fn linux_probe_kprobe_hook(
    _fragment_id: &str,
    _symbol_name: &str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    Err(LoaderError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
pub fn linux_probe_tc_ingress_hook(
    fragment_id: &str,
    dev_name: &str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    use crate::fragment::HookPoint;
    crate::linux_ebpf_smoke::validate_netdev_name(dev_name).map_err(map_smoke_error)?;
    let message = match crate::linux_ebpf_smoke::run_tc_attach_smoke(dev_name, None) {
        Ok(()) => return Ok(Vec::new()),
        Err(err) => err.to_string(),
    };

    Ok(vec![AttachFailure {
        fragment_id: fragment_id.to_string(),
        hookpoint: HookPoint::TCIngress,
        error: message,
    }])
}

#[cfg(not(target_os = "linux"))]
pub fn linux_probe_tc_ingress_hook(
    _fragment_id: &str,
    _dev_name: &str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    Err(LoaderError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
pub fn linux_probe_tracepoint_hooks(plan: &AttachPlan) -> Result<Vec<AttachFailure>, LoaderError> {
    use crate::fragment::HookPoint;

    let mut failures = Vec::new();

    for binding in &plan.hook_graph {
        if let HookPoint::TracePoint(name) = &binding.hookpoint {
            failures.extend(linux_probe_tracepoint_hook(&binding.fragment_id, name)?);
        }
    }

    Ok(failures)
}

#[cfg(not(target_os = "linux"))]
pub fn linux_probe_tracepoint_hooks(_plan: &AttachPlan) -> Result<Vec<AttachFailure>, LoaderError> {
    Err(LoaderError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
pub fn linux_probe_kernel_hooks(plan: &AttachPlan) -> Result<Vec<AttachFailure>, LoaderError> {
    use crate::fragment::HookPoint;

    let mut failures = Vec::new();

    for binding in &plan.hook_graph {
        match &binding.hookpoint {
            HookPoint::TracePoint(name) => {
                failures.extend(linux_probe_tracepoint_hook(&binding.fragment_id, name)?);
            }
            HookPoint::KProbe(name) => {
                failures.extend(linux_probe_kprobe_hook(&binding.fragment_id, name)?);
            }
            HookPoint::TCIngress => {
                failures.extend(linux_probe_tc_ingress_hook(&binding.fragment_id, "eth0")?);
            }
            HookPoint::TCEgress => {}
        }
    }

    Ok(failures)
}

#[cfg(not(target_os = "linux"))]
pub fn linux_probe_kernel_hooks(_plan: &AttachPlan) -> Result<Vec<AttachFailure>, LoaderError> {
    Err(LoaderError::UnsupportedPlatform)
}

#[cfg(any(test, target_os = "linux"))]
fn map_smoke_error(err: crate::linux_ebpf_smoke::LinuxEbpfSmokeError) -> LoaderError {
    match err {
        crate::linux_ebpf_smoke::LinuxEbpfSmokeError::UnsupportedPlatform => {
            LoaderError::UnsupportedPlatform
        }
        crate::linux_ebpf_smoke::LinuxEbpfSmokeError::InvalidTarget(message) => {
            LoaderError::InvalidProbeTarget(message)
        }
        crate::linux_ebpf_smoke::LinuxEbpfSmokeError::UnsafeHostState(message)
        | crate::linux_ebpf_smoke::LinuxEbpfSmokeError::Io(message)
        | crate::linux_ebpf_smoke::LinuxEbpfSmokeError::CommandFailed(message) => {
            LoaderError::LaunchFailed(message)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{
        LoaderError, linux_probe_kprobe_hook, linux_probe_tc_ingress_hook,
        linux_probe_tracepoint_hook,
    };
    use crate::fragment::AttachFailure;
    use crate::linux_ebpf_smoke::{
        validate_netdev_name, validate_symbol_name, validate_tracepoint_name,
    };

    #[test]
    fn tracepoint_validation_rejects_shell_metacharacters() {
        let err = validate_tracepoint_name("syscalls/sys_enter_openat;touch").unwrap_err();
        assert!(matches!(
            super::map_smoke_error(err),
            LoaderError::InvalidProbeTarget(_)
        ));
    }

    #[test]
    fn symbol_validation_rejects_path_characters() {
        let err = validate_symbol_name("../tcp_v4_connect").unwrap_err();
        assert!(matches!(
            super::map_smoke_error(err),
            LoaderError::InvalidProbeTarget(_)
        ));
    }

    #[test]
    fn netdev_validation_rejects_whitespace() {
        let err = validate_netdev_name("eth0 prod").unwrap_err();
        assert!(matches!(
            super::map_smoke_error(err),
            LoaderError::InvalidProbeTarget(_)
        ));
    }

    #[test]
    fn linux_probe_api_accepts_borrowed_plan_strings() {
        type ProbeFn = fn(&str, &str) -> Result<Vec<AttachFailure>, LoaderError>;
        let _: ProbeFn = linux_probe_tracepoint_hook;
        let _: ProbeFn = linux_probe_kprobe_hook;
        let _: ProbeFn = linux_probe_tc_ingress_hook;
    }
}
