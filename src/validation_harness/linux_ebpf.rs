use std::fs;
use std::path::PathBuf;

use crate::linux_ebpf_smoke::{
    LinuxEbpfSmokeError, run_kprobe_attach_smoke, run_tc_attach_smoke, run_tracepoint_attach_smoke,
};

use super::command::{ValidationError, ValidationReport, default_out_dir};

pub fn run_linux_attach_smoke(
    hookpoint_name: &str,
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("linux-attach-smoke"));
    fs::create_dir_all(&out_dir)?;
    let transcript = out_dir.join("run.log");

    run_tracepoint_attach_smoke(hookpoint_name, Some(&transcript)).map_err(map_smoke_error)?;
    fs::write(
        out_dir.join("target.txt"),
        format!("hookpoint={hookpoint_name}\n"),
    )?;

    Ok(ValidationReport {
        name: format!("linux attach smoke ({hookpoint_name})"),
        out_dir,
        checks: vec!["tracepoint_attach".to_string()],
    })
}

pub fn run_linux_kprobe_smoke(
    symbol_name: &str,
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("linux-kprobe-smoke"));
    fs::create_dir_all(&out_dir)?;
    let transcript = out_dir.join("run.log");

    run_kprobe_attach_smoke(symbol_name, Some(&transcript)).map_err(map_smoke_error)?;
    fs::write(
        out_dir.join("target.txt"),
        format!("symbol={symbol_name}\n"),
    )?;

    Ok(ValidationReport {
        name: format!("linux kprobe smoke ({symbol_name})"),
        out_dir,
        checks: vec!["kprobe_attach".to_string()],
    })
}

pub fn run_linux_tc_smoke(
    dev_name: &str,
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("linux-tc-smoke"));
    fs::create_dir_all(&out_dir)?;
    let transcript = out_dir.join("run.log");

    run_tc_attach_smoke(dev_name, Some(&transcript)).map_err(map_smoke_error)?;
    fs::write(out_dir.join("target.txt"), format!("device={dev_name}\n"))?;

    Ok(ValidationReport {
        name: format!("linux tc smoke ({dev_name})"),
        out_dir,
        checks: vec!["tc_ingress_attach".to_string()],
    })
}

fn map_smoke_error(err: LinuxEbpfSmokeError) -> ValidationError {
    match err {
        LinuxEbpfSmokeError::UnsupportedPlatform => {
            ValidationError::new("linux eBPF smoke requires a Linux environment")
        }
        LinuxEbpfSmokeError::InvalidTarget(message)
        | LinuxEbpfSmokeError::Io(message)
        | LinuxEbpfSmokeError::CommandFailed(message) => ValidationError::new(format!(
            "{message}\nlinux eBPF smoke requires Linux kernel support and BPF attach privileges; unprivileged runs may fail with `Operation not permitted`"
        )),
    }
}
