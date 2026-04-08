use crate::fragment::AttachFailure;
use crate::fragment::AttachPlan;

pub const LINUX_SMOKE_FRAGMENT_ID: &str = "linux_tracepoint_smoke_fragment";

#[derive(Debug, Eq, PartialEq)]
pub enum LoaderError {
    UnsupportedPlatform,
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
    hookpoint_name: &'static str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    use crate::fragment::HookPoint;
    use std::process::Command;

    let output = Command::new("/bin/bash")
        .arg("scripts/linux_attach_smoke.sh")
        .arg(hookpoint_name)
        .output()
        .map_err(|err| LoaderError::LaunchFailed(err.to_string()))?;

    if output.status.success() {
        return Ok(Vec::new());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let message = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("smoke attach exited with {}", output.status)
    };

    Ok(vec![AttachFailure {
        fragment_id: LINUX_SMOKE_FRAGMENT_ID,
        hookpoint: HookPoint::TracePoint(hookpoint_name),
        error: message,
    }])
}

#[cfg(not(target_os = "linux"))]
pub fn linux_tracepoint_smoke_failures(
    _hookpoint_name: &'static str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    Err(LoaderError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
pub fn linux_probe_tracepoint_hook(
    fragment_id: &'static str,
    hookpoint_name: &'static str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    use crate::fragment::HookPoint;

    let output = std::process::Command::new("/bin/bash")
        .arg("scripts/linux_attach_smoke.sh")
        .arg(hookpoint_name)
        .output()
        .map_err(|err| LoaderError::LaunchFailed(err.to_string()))?;

    if output.status.success() {
        return Ok(Vec::new());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let message = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("probe attach exited with {}", output.status)
    };

    Ok(vec![AttachFailure {
        fragment_id,
        hookpoint: HookPoint::TracePoint(hookpoint_name),
        error: message,
    }])
}

#[cfg(not(target_os = "linux"))]
pub fn linux_probe_tracepoint_hook(
    _fragment_id: &'static str,
    _hookpoint_name: &'static str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    Err(LoaderError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
pub fn linux_probe_kprobe_hook(
    fragment_id: &'static str,
    symbol_name: &'static str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    use crate::fragment::HookPoint;

    let output = std::process::Command::new("/bin/bash")
        .arg("scripts/linux_kprobe_smoke.sh")
        .arg(symbol_name)
        .output()
        .map_err(|err| LoaderError::LaunchFailed(err.to_string()))?;

    if output.status.success() {
        return Ok(Vec::new());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let message = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("probe attach exited with {}", output.status)
    };

    Ok(vec![AttachFailure {
        fragment_id,
        hookpoint: HookPoint::KProbe(symbol_name),
        error: message,
    }])
}

#[cfg(not(target_os = "linux"))]
pub fn linux_probe_kprobe_hook(
    _fragment_id: &'static str,
    _symbol_name: &'static str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    Err(LoaderError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
pub fn linux_probe_tc_ingress_hook(
    fragment_id: &'static str,
    dev_name: &'static str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    use crate::fragment::HookPoint;

    let output = std::process::Command::new("/bin/bash")
        .arg("scripts/linux_tc_smoke.sh")
        .arg(dev_name)
        .output()
        .map_err(|err| LoaderError::LaunchFailed(err.to_string()))?;

    if output.status.success() {
        return Ok(Vec::new());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let message = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        format!("probe attach exited with {}", output.status)
    };

    Ok(vec![AttachFailure {
        fragment_id,
        hookpoint: HookPoint::TCIngress,
        error: message,
    }])
}

#[cfg(not(target_os = "linux"))]
pub fn linux_probe_tc_ingress_hook(
    _fragment_id: &'static str,
    _dev_name: &'static str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    Err(LoaderError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
pub fn linux_probe_tracepoint_hooks(plan: &AttachPlan) -> Result<Vec<AttachFailure>, LoaderError> {
    use crate::fragment::HookPoint;

    let mut failures = Vec::new();

    for binding in &plan.hook_graph {
        if let HookPoint::TracePoint(name) = binding.hookpoint {
            failures.extend(linux_probe_tracepoint_hook(binding.fragment_id, name)?);
        }
    }

    Ok(failures)
}

#[cfg(not(target_os = "linux"))]
pub fn linux_probe_tracepoint_hooks(
    _plan: &AttachPlan,
) -> Result<Vec<AttachFailure>, LoaderError> {
    Err(LoaderError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
pub fn linux_probe_kernel_hooks(plan: &AttachPlan) -> Result<Vec<AttachFailure>, LoaderError> {
    use crate::fragment::HookPoint;

    let mut failures = Vec::new();

    for binding in &plan.hook_graph {
        match binding.hookpoint {
            HookPoint::TracePoint(name) => {
                failures.extend(linux_probe_tracepoint_hook(binding.fragment_id, name)?);
            }
            HookPoint::KProbe(name) => {
                failures.extend(linux_probe_kprobe_hook(binding.fragment_id, name)?);
            }
            HookPoint::TCIngress => {
                failures.extend(linux_probe_tc_ingress_hook(binding.fragment_id, "eth0")?);
            }
            HookPoint::TCEgress => {}
        }
    }

    Ok(failures)
}

#[cfg(not(target_os = "linux"))]
pub fn linux_probe_kernel_hooks(
    _plan: &AttachPlan,
) -> Result<Vec<AttachFailure>, LoaderError> {
    Err(LoaderError::UnsupportedPlatform)
}
