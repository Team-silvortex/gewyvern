use crate::fragment::AttachFailure;

pub const LINUX_SMOKE_FRAGMENT_ID: &str = "linux_tracepoint_smoke_fragment";

#[derive(Debug, Eq, PartialEq)]
pub enum LoaderError {
    UnsupportedPlatform,
    LaunchFailed(String),
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
