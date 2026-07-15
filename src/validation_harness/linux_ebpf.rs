use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::json;

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
    write_linux_smoke_evidence(
        &out_dir,
        "linux-attach-smoke",
        &[("hookpoint", hookpoint_name)],
        None,
    )?;

    run_tracepoint_attach_smoke(hookpoint_name, Some(&transcript)).map_err(map_smoke_error)?;

    Ok(ValidationReport {
        name: format!("linux attach smoke ({hookpoint_name})"),
        out_dir,
        checks: vec![
            "tracepoint_attach".to_string(),
            "linux_environment_evidence".to_string(),
        ],
    })
}

pub fn run_linux_kprobe_smoke(
    symbol_name: &str,
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("linux-kprobe-smoke"));
    fs::create_dir_all(&out_dir)?;
    let transcript = out_dir.join("run.log");
    write_linux_smoke_evidence(
        &out_dir,
        "linux-kprobe-smoke",
        &[("symbol", symbol_name)],
        None,
    )?;

    run_kprobe_attach_smoke(symbol_name, Some(&transcript)).map_err(map_smoke_error)?;

    Ok(ValidationReport {
        name: format!("linux kprobe smoke ({symbol_name})"),
        out_dir,
        checks: vec![
            "kprobe_attach".to_string(),
            "linux_environment_evidence".to_string(),
        ],
    })
}

pub fn run_linux_tc_smoke(
    dev_name: &str,
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("linux-tc-smoke"));
    fs::create_dir_all(&out_dir)?;
    let transcript = out_dir.join("run.log");
    write_linux_smoke_evidence(
        &out_dir,
        "linux-tc-smoke",
        &[("device", dev_name)],
        Some(dev_name),
    )?;

    run_tc_attach_smoke(dev_name, Some(&transcript)).map_err(map_smoke_error)?;

    Ok(ValidationReport {
        name: format!("linux tc smoke ({dev_name})"),
        out_dir,
        checks: vec![
            "tc_ingress_attach".to_string(),
            "linux_environment_evidence".to_string(),
        ],
    })
}

fn write_linux_smoke_evidence(
    out_dir: &Path,
    command: &str,
    target_fields: &[(&str, &str)],
    netdev: Option<&str>,
) -> Result<(), ValidationError> {
    fs::write(
        out_dir.join("target.txt"),
        render_key_value_lines(target_fields),
    )?;
    fs::write(
        out_dir.join("environment.txt"),
        render_linux_environment(target_fields),
    )?;

    let mut files = vec!["target.txt", "environment.txt", "run.log"];
    if let Some(name) = netdev {
        fs::write(out_dir.join("netdev.txt"), render_netdev_context(name))?;
        files.push("netdev.txt");
    }

    let index = json!({
        "schema_version": 1,
        "command": command,
        "files": files,
    });
    fs::write(
        out_dir.join("evidence-index.json"),
        serde_json::to_string_pretty(&index)?,
    )?;
    Ok(())
}

fn render_linux_environment(target_fields: &[(&str, &str)]) -> String {
    let mut lines = Vec::new();
    lines.push(format!("os={}", env::consts::OS));
    lines.push(format!("arch={}", env::consts::ARCH));
    lines.push(format!("workspace={}", env!("CARGO_MANIFEST_DIR")));
    lines.push(format!(
        "kernel_release={}",
        read_trimmed("/proc/sys/kernel/osrelease").unwrap_or_else(|| "unknown".to_string())
    ));
    lines.push(format!(
        "kernel_version={}",
        read_trimmed("/proc/version").unwrap_or_else(|| "unknown".to_string())
    ));
    lines.push(format!(
        "hostname={}",
        read_trimmed("/proc/sys/kernel/hostname").unwrap_or_else(|| "unknown".to_string())
    ));
    lines.push(format!(
        "uid={}",
        read_status_value("Uid").unwrap_or_else(|| "unknown".to_string())
    ));
    lines.push(format!(
        "gid={}",
        read_status_value("Gid").unwrap_or_else(|| "unknown".to_string())
    ));
    lines.push(format!(
        "cap_eff={}",
        read_status_value("CapEff").unwrap_or_else(|| "unknown".to_string())
    ));
    lines.push(format!(
        "bpf_fs_present={}",
        Path::new("/sys/fs/bpf").exists()
    ));
    lines.push(format!(
        "tracefs_present={}",
        Path::new("/sys/kernel/tracing").exists()
            || Path::new("/sys/kernel/debug/tracing").exists()
    ));
    lines.push(format!(
        "btf_vmlinux_present={}",
        Path::new("/sys/kernel/btf/vmlinux").exists()
    ));
    lines.push(format!(
        "unprivileged_bpf_disabled={}",
        read_trimmed("/proc/sys/kernel/unprivileged_bpf_disabled")
            .unwrap_or_else(|| "unknown".to_string())
    ));
    lines.push(format!("clang_in_path={}", command_in_path("clang")));
    lines.push(format!("cc_in_path={}", command_in_path("cc")));
    lines.push(format!("tc_in_path={}", command_in_path("tc")));
    lines.push(format!("bpftool_in_path={}", command_in_path("bpftool")));
    lines.extend(
        target_fields
            .iter()
            .map(|(key, value)| format!("target_{key}={value}")),
    );
    lines.join("\n") + "\n"
}

fn render_netdev_context(name: &str) -> String {
    let root = Path::new("/sys/class/net").join(name);
    let mut lines = vec![format!("device={name}")];
    for (label, path) in [
        ("ifindex", root.join("ifindex")),
        ("mtu", root.join("mtu")),
        ("operstate", root.join("operstate")),
        ("carrier", root.join("carrier")),
        ("type", root.join("type")),
    ] {
        lines.push(format!(
            "{label}={}",
            read_trimmed_path(&path).unwrap_or_else(|| "unknown".to_string())
        ));
    }
    lines.join("\n") + "\n"
}

fn render_key_value_lines(fields: &[(&str, &str)]) -> String {
    fields
        .iter()
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

fn read_trimmed(path: &str) -> Option<String> {
    read_trimmed_path(Path::new(path))
}

fn read_trimmed_path(path: &Path) -> Option<String> {
    fs::read_to_string(path)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn read_status_value(field: &str) -> Option<String> {
    let prefix = format!("{field}:");
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        line.strip_prefix(&prefix).and_then(|rest| {
            rest.split_whitespace()
                .next()
                .map(|value| value.to_string())
        })
    })
}

fn command_in_path(command: &str) -> bool {
    env::var_os("PATH")
        .map(|paths| env::split_paths(&paths).any(|dir| dir.join(command).exists()))
        .unwrap_or(false)
}

fn map_smoke_error(err: LinuxEbpfSmokeError) -> ValidationError {
    match err {
        LinuxEbpfSmokeError::UnsupportedPlatform => {
            ValidationError::new("linux eBPF smoke requires a Linux environment")
        }
        LinuxEbpfSmokeError::InvalidTarget(message)
        | LinuxEbpfSmokeError::UnsafeHostState(message)
        | LinuxEbpfSmokeError::Io(message)
        | LinuxEbpfSmokeError::CommandFailed(message) => ValidationError::new(format!(
            "{message}\nlinux eBPF smoke requires Linux kernel support and BPF attach privileges; unprivileged runs may fail with `Operation not permitted`"
        )),
    }
}
