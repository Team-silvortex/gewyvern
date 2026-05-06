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
    hookpoint_name: &'static str,
) -> Result<Vec<AttachFailure>, LoaderError> {
    use crate::fragment::HookPoint;
    validate_tracepoint_name(hookpoint_name)?;
    let output = run_repo_script("linux_attach_smoke.sh", hookpoint_name)?;

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
    validate_tracepoint_name(hookpoint_name)?;
    let output = run_repo_script("linux_attach_smoke.sh", hookpoint_name)?;

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
    validate_symbol_name(symbol_name)?;
    let output = run_repo_script("linux_kprobe_smoke.sh", symbol_name)?;

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
    validate_netdev_name(dev_name)?;
    let output = run_repo_script("linux_tc_smoke.sh", dev_name)?;

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
pub fn linux_probe_tracepoint_hooks(_plan: &AttachPlan) -> Result<Vec<AttachFailure>, LoaderError> {
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
pub fn linux_probe_kernel_hooks(_plan: &AttachPlan) -> Result<Vec<AttachFailure>, LoaderError> {
    Err(LoaderError::UnsupportedPlatform)
}

#[cfg(target_os = "linux")]
fn run_repo_script(
    script_name: &'static str,
    arg: &'static str,
) -> Result<std::process::Output, LoaderError> {
    let script_path = repo_script_path(script_name)?;
    std::process::Command::new(script_path)
        .arg(arg)
        .output()
        .map_err(|err| LoaderError::LaunchFailed(err.to_string()))
}

#[cfg(target_os = "linux")]
fn repo_script_path(script_name: &'static str) -> Result<std::path::PathBuf, LoaderError> {
    let script_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("scripts")
        .join(script_name);
    let canonical = std::fs::canonicalize(&script_path)
        .map_err(|err| LoaderError::LaunchFailed(err.to_string()))?;
    let scripts_root =
        std::fs::canonicalize(std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("scripts"))
            .map_err(|err| LoaderError::LaunchFailed(err.to_string()))?;
    if !canonical.starts_with(&scripts_root) {
        return Err(LoaderError::LaunchFailed(format!(
            "refusing to execute script outside scripts root: {}",
            canonical.display()
        )));
    }
    Ok(canonical)
}

fn validate_tracepoint_name(name: &str) -> Result<(), LoaderError> {
    validate_probe_target(
        name,
        |ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '/' | '-'),
        "tracepoint",
    )
}

fn validate_symbol_name(name: &str) -> Result<(), LoaderError> {
    validate_probe_target(
        name,
        |ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '.'),
        "symbol",
    )
}

fn validate_netdev_name(name: &str) -> Result<(), LoaderError> {
    validate_probe_target(
        name,
        |ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.'),
        "netdev",
    )
}

fn validate_probe_target<F>(
    value: &str,
    is_allowed: F,
    label: &'static str,
) -> Result<(), LoaderError>
where
    F: Fn(char) -> bool,
{
    if value.is_empty() || value.len() > 128 || value.chars().any(|ch| !is_allowed(ch)) {
        return Err(LoaderError::InvalidProbeTarget(format!(
            "invalid {label} target '{value}'"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{
        LoaderError, validate_netdev_name, validate_symbol_name, validate_tracepoint_name,
    };

    #[test]
    fn tracepoint_validation_rejects_shell_metacharacters() {
        let err = validate_tracepoint_name("syscalls/sys_enter_openat;touch").unwrap_err();
        assert!(matches!(err, LoaderError::InvalidProbeTarget(_)));
    }

    #[test]
    fn symbol_validation_rejects_path_characters() {
        let err = validate_symbol_name("../tcp_v4_connect").unwrap_err();
        assert!(matches!(err, LoaderError::InvalidProbeTarget(_)));
    }

    #[test]
    fn netdev_validation_rejects_whitespace() {
        let err = validate_netdev_name("eth0 prod").unwrap_err();
        assert!(matches!(err, LoaderError::InvalidProbeTarget(_)));
    }
}
