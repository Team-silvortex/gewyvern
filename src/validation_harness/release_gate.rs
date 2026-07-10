use std::process::{Command, Stdio};

use super::command::{ValidationError, ValidationReport, repo_root};
use super::{
    RemoteLinuxHostOptions, run_container_runtime_validation, run_container_validation_summary,
    run_package_install_smoke, run_pathological_container_validation,
    run_remote_linux_host_validation, run_three_module_stack_smoke,
};

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum ReleaseCheckMode {
    Deb,
    Rpm,
    #[default]
    DebAndRpm,
}

impl ReleaseCheckMode {
    pub fn label(self) -> &'static str {
        match self {
            Self::Deb => "deb",
            Self::Rpm => "rpm",
            Self::DebAndRpm => "deb+rpm",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ReleaseGateOptions {
    pub run_build: bool,
    pub run_release_check: bool,
    pub run_stack: bool,
    pub run_pathology: bool,
    pub run_remote_host: bool,
    pub remote_host: String,
    pub remote_dir: Option<String>,
    pub keep_remote_dir: bool,
    pub remote_build_packages: bool,
    pub release_mode: ReleaseCheckMode,
}

impl Default for ReleaseGateOptions {
    fn default() -> Self {
        Self {
            run_build: true,
            run_release_check: true,
            run_stack: true,
            run_pathology: true,
            run_remote_host: false,
            remote_host: std::env::var("GEWY_REMOTE_HOST")
                .unwrap_or_else(|_| "kyuubiki-lab".to_string()),
            remote_dir: None,
            keep_remote_dir: false,
            remote_build_packages: true,
            release_mode: ReleaseCheckMode::DebAndRpm,
        }
    }
}

pub fn run_release_container_check(
    mode: ReleaseCheckMode,
) -> Result<ValidationReport, ValidationError> {
    let mut checks = Vec::new();
    println!(
        "[release-check] starting packaged release validation ({})",
        mode.label()
    );

    println!("[release-check] ----------------------------------------");
    println!("[release-check] running package install smoke");
    run_package_install_smoke(mode)?;
    checks.push("package_install_smoke".to_string());

    println!("[release-check] ----------------------------------------");
    println!("[release-check] running packaged runtime validation");
    run_container_runtime_validation(mode)?;
    checks.push("packaged_runtime_validation".to_string());

    println!("[release-check] ----------------------------------------");
    println!("[release-check] running packaged protocol/operator summary");
    run_container_validation_summary(mode)?;
    checks.push("packaged_protocol_operator_summary".to_string());

    println!("[release-check] ----------------------------------------");
    println!(
        "[release-check] packaged release validation: ok ({})",
        mode.label()
    );

    Ok(ValidationReport {
        name: format!("packaged release validation ({})", mode.label()),
        out_dir: repo_root().join("target").join("validation"),
        checks,
    })
}

pub fn run_release_gate(options: ReleaseGateOptions) -> Result<ValidationReport, ValidationError> {
    let mut checks = Vec::new();

    if options.run_build {
        run_step(
            "release-gate",
            "building fresh native artifacts",
            "scripts/packaging/build_packages_in_container.sh",
            &["--format", "all"],
        )?;
        checks.push("build_packages_in_container".to_string());
    } else {
        println!("[release-gate] skipping package rebuild");
    }

    if options.run_release_check {
        println!("[release-gate] ----------------------------------------");
        match options.release_mode {
            ReleaseCheckMode::DebAndRpm => {
                println!("[release-gate] running packaged release validation");
            }
            mode => {
                println!(
                    "[release-gate] running packaged release validation ({})",
                    mode.label()
                );
            }
        }
        run_release_container_check(options.release_mode)?;
        checks.push("release_container_check".to_string());
    } else {
        println!("[release-gate] skipping packaged release validation");
    }

    if options.run_stack {
        println!("[release-gate] ----------------------------------------");
        println!("[release-gate] running three-module stack smoke");
        run_three_module_stack_smoke()?;
        checks.push("three_module_stack_smoke".to_string());
    } else {
        println!("[release-gate] skipping three-module stack smoke");
    }

    if options.run_pathology {
        println!("[release-gate] ----------------------------------------");
        println!("[release-gate] running pathological container validation");
        run_pathological_container_validation(None)?;
        checks.push("pathological_container_validation".to_string());
    } else {
        println!("[release-gate] skipping pathological container validation");
    }

    if options.run_remote_host {
        println!("[release-gate] ----------------------------------------");
        println!(
            "[release-gate] running remote linux host validation ({})",
            options.remote_host
        );
        let remote_report = run_remote_linux_host_validation(RemoteLinuxHostOptions {
            host: options.remote_host,
            remote_dir: options.remote_dir,
            build_packages: options.remote_build_packages,
            keep_remote_dir: options.keep_remote_dir,
        })?;
        checks.push("remote_linux_host_validation".to_string());
        if remote_report
            .checks
            .iter()
            .any(|check| check == "remote_ebpf_smoke")
        {
            println!("[release-gate] remote Linux eBPF attach evidence: ok");
            checks.push("remote_ebpf_smoke".to_string());
        } else if remote_report
            .checks
            .iter()
            .any(|check| check == "remote_ebpf_smoke_skipped")
        {
            println!(
                "[release-gate] remote Linux eBPF attach evidence: skipped (see remote-ebpf.txt)"
            );
            checks.push("remote_ebpf_smoke_skipped".to_string());
        }
    } else {
        println!("[release-gate] skipping remote linux host validation");
    }

    println!("[release-gate] ----------------------------------------");
    println!("[release-gate] release gate: ok");

    Ok(ValidationReport {
        name: "release gate".to_string(),
        out_dir: repo_root().join("target").join("validation"),
        checks,
    })
}

fn run_step(
    prefix: &str,
    label: &str,
    script_relative_path: &str,
    args: &[&str],
) -> Result<(), ValidationError> {
    println!("[{prefix}] ----------------------------------------");
    println!("[{prefix}] {label}");
    run_repo_script(script_relative_path, args)
}

fn run_repo_script(script_relative_path: &str, args: &[&str]) -> Result<(), ValidationError> {
    let status = Command::new("bash")
        .current_dir(repo_root())
        .arg(repo_root().join(script_relative_path))
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| {
            ValidationError::new(format!("failed to launch `{script_relative_path}`: {err}"))
        })?;

    if !status.success() {
        return Err(ValidationError::new(format!(
            "`{script_relative_path}` exited with status {status}"
        )));
    }

    Ok(())
}
