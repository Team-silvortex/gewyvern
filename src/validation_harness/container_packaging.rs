// Keep validation tests adjacent to the package helpers they specify.
#![allow(clippy::items_after_test_module)]

use std::env;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use ring::digest::{Context, SHA256};
use serde_json::json;

use super::command::{
    ValidationError, ValidationReport, default_out_dir, repo_root, validation_command_stdout,
    validation_log,
};
use super::release_gate::ReleaseCheckMode;

const DEFAULT_DEB_PROTOCOL_IMAGE: &str = "ubuntu:24.04";
const DEFAULT_RPM_PROTOCOL_IMAGE: &str = "fedora:41";
const DEFAULT_DEB_OPERATOR_IMAGE: &str = "ubuntu:24.04";
const DEFAULT_RPM_OPERATOR_IMAGE: &str = "fedora:41";
const DEFAULT_DEB_RUNTIME_IMAGE: &str = "ubuntu:24.04";
const DEFAULT_RPM_RUNTIME_IMAGE: &str = "fedora:41";
const DEFAULT_DEB_SMOKE_IMAGE: &str = "ubuntu:24.04";
const DEFAULT_RPM_SMOKE_IMAGE: &str = "fedora:41";

pub fn run_package_install_smoke(
    mode: ReleaseCheckMode,
) -> Result<ValidationReport, ValidationError> {
    let cfg = ContainerValidationConfig::new(
        "install-smoke",
        "GEWY_DEB_SMOKE_IMAGE",
        DEFAULT_DEB_SMOKE_IMAGE,
        "GEWY_RPM_SMOKE_IMAGE",
        DEFAULT_RPM_SMOKE_IMAGE,
    )?;
    run_packaged_validation(
        "package-install-smoke",
        "package install smoke",
        cfg,
        mode,
        package_install_smoke_deb_body(),
        package_install_smoke_rpm_body(),
        "deb_install_smoke",
        "rpm_install_smoke",
    )
}

fn prepare_container_evidence(out_dir: &Path) -> Result<(), ValidationError> {
    fs::create_dir_all(out_dir)?;
    for name in [
        "deb.json",
        "rpm.json",
        "summary.json",
        "evidence-index.json",
    ] {
        let path = out_dir.join(name);
        if path.exists() || path.is_symlink() {
            fs::remove_file(path)?;
        }
    }
    Ok(())
}

fn write_package_stage_evidence(
    out_dir: &Path,
    family: &str,
    image: &str,
    package: &Path,
) -> Result<(), ValidationError> {
    let artifact = package
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ValidationError::new("invalid package artifact filename"))?;
    let payload = json!({
        "schema_version": 1,
        "family": family,
        "status": "ok",
        "image": image,
        "artifact": artifact,
        "artifact_bytes": fs::metadata(package)?.len(),
        "artifact_sha256": sha256_file(package)?,
    });
    fs::write(
        out_dir.join(format!("{family}.json")),
        format!("{}\n", serde_json::to_string_pretty(&payload)?),
    )?;
    Ok(())
}

fn write_container_summary(
    out_dir: &Path,
    command: &str,
    mode: ReleaseCheckMode,
    checks: &[String],
    evidence_files: &mut Vec<String>,
) -> Result<(), ValidationError> {
    let summary = json!({
        "schema_version": 1,
        "command": command,
        "status": "ok",
        "mode": mode.label(),
        "checks": checks,
    });
    fs::write(
        out_dir.join("summary.json"),
        format!("{}\n", serde_json::to_string_pretty(&summary)?),
    )?;
    evidence_files.push("summary.json".to_string());
    let index = json!({
        "schema_version": 1,
        "command": command,
        "files": evidence_files,
    });
    fs::write(
        out_dir.join("evidence-index.json"),
        format!("{}\n", serde_json::to_string_pretty(&index)?),
    )?;
    Ok(())
}

fn sha256_file(path: &Path) -> Result<String, ValidationError> {
    let mut file = fs::File::open(path)?;
    let mut context = Context::new(&SHA256);
    let mut buffer = [0_u8; 64 * 1024];
    loop {
        let count = file.read(&mut buffer)?;
        if count == 0 {
            break;
        }
        context.update(&buffer[..count]);
    }
    Ok(context
        .finish()
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

pub fn run_container_runtime_validation(
    mode: ReleaseCheckMode,
) -> Result<ValidationReport, ValidationError> {
    let cfg = ContainerValidationConfig::new(
        "runtime",
        "GEWY_DEB_RUNTIME_IMAGE",
        DEFAULT_DEB_RUNTIME_IMAGE,
        "GEWY_RPM_RUNTIME_IMAGE",
        DEFAULT_RPM_RUNTIME_IMAGE,
    )?;
    run_packaged_validation(
        "container-runtime-validation",
        "container runtime validation",
        cfg,
        mode,
        runtime_validation_body(),
        runtime_validation_body(),
        "deb_runtime_validation",
        "rpm_runtime_validation",
    )
}

pub fn run_container_protocol_validation(
    mode: ReleaseCheckMode,
) -> Result<ValidationReport, ValidationError> {
    let cfg = ContainerValidationConfig::new(
        "protocol",
        "GEWY_DEB_PROTOCOL_IMAGE",
        DEFAULT_DEB_PROTOCOL_IMAGE,
        "GEWY_RPM_PROTOCOL_IMAGE",
        DEFAULT_RPM_PROTOCOL_IMAGE,
    )?;
    run_packaged_validation(
        "container-protocol-validation",
        "container protocol validation",
        cfg,
        mode,
        protocol_validation_body(),
        protocol_validation_body(),
        "deb_protocol_validation",
        "rpm_protocol_validation",
    )
}

pub fn run_container_operator_path_validation(
    mode: ReleaseCheckMode,
) -> Result<ValidationReport, ValidationError> {
    let cfg = ContainerValidationConfig::new(
        "operator-path",
        "GEWY_DEB_OPERATOR_IMAGE",
        DEFAULT_DEB_OPERATOR_IMAGE,
        "GEWY_RPM_OPERATOR_IMAGE",
        DEFAULT_RPM_OPERATOR_IMAGE,
    )?;
    run_packaged_validation(
        "container-operator-path-validation",
        "container operator path validation",
        cfg,
        mode,
        operator_validation_body(),
        operator_validation_body(),
        "deb_operator_path_validation",
        "rpm_operator_path_validation",
    )
}

pub fn run_container_validation_summary(
    mode: ReleaseCheckMode,
) -> Result<ValidationReport, ValidationError> {
    validation_log(format!(
        "[summary] starting packaged container validation ({})",
        mode.label()
    ));

    validation_log("[summary] ----------------------------------------");
    validation_log("[summary] running packaged protocol validation");
    let protocol = run_container_protocol_validation(mode)?;

    validation_log("[summary] ----------------------------------------");
    validation_log("[summary] running packaged operator-path validation");
    let operator = run_container_operator_path_validation(mode)?;

    validation_log("[summary] ----------------------------------------");
    validation_log(format!(
        "[summary] packaged container validation: ok ({})",
        mode.label()
    ));

    let mut checks = protocol.checks;
    checks.extend(operator.checks);
    let out_dir = default_out_dir("container-validation-summary");
    prepare_container_evidence(&out_dir)?;
    write_composite_evidence(
        &out_dir,
        "container-validation-summary",
        mode,
        &checks,
        &[
            ("protocol", &protocol.out_dir),
            ("operator_path", &operator.out_dir),
        ],
    )?;
    Ok(ValidationReport {
        name: format!("packaged container validation summary ({})", mode.label()),
        out_dir,
        checks,
    })
}

fn write_composite_evidence(
    out_dir: &Path,
    command: &str,
    mode: ReleaseCheckMode,
    checks: &[String],
    components: &[(&str, &Path)],
) -> Result<(), ValidationError> {
    let components = components
        .iter()
        .map(|(name, path)| {
            let evidence_dir = path
                .file_name()
                .and_then(|value| value.to_str())
                .ok_or_else(|| ValidationError::new("invalid component evidence directory"))?;
            Ok(json!({
                "name": name,
                "evidence_dir": format!("../{evidence_dir}"),
                "status": "ok",
            }))
        })
        .collect::<Result<Vec<_>, ValidationError>>()?;
    let summary = json!({
        "schema_version": 1,
        "command": command,
        "status": "ok",
        "mode": mode.label(),
        "checks": checks,
        "components": components,
    });
    fs::write(
        out_dir.join("summary.json"),
        format!("{}\n", serde_json::to_string_pretty(&summary)?),
    )?;
    let index = json!({
        "schema_version": 1,
        "command": command,
        "files": ["summary.json"],
    });
    fs::write(
        out_dir.join("evidence-index.json"),
        format!("{}\n", serde_json::to_string_pretty(&index)?),
    )?;
    Ok(())
}

struct ContainerValidationConfig {
    validation_name: &'static str,
    deb_image: String,
    rpm_image: String,
}

impl ContainerValidationConfig {
    fn new(
        validation_name: &'static str,
        deb_env: &str,
        deb_default: &str,
        rpm_env: &str,
        rpm_default: &str,
    ) -> Result<Self, ValidationError> {
        Ok(Self {
            validation_name,
            deb_image: validate_container_image(
                deb_env,
                &env::var(deb_env).unwrap_or_else(|_| deb_default.to_string()),
            )?,
            rpm_image: validate_container_image(
                rpm_env,
                &env::var(rpm_env).unwrap_or_else(|_| rpm_default.to_string()),
            )?,
        })
    }
}

#[allow(clippy::too_many_arguments)]
fn run_packaged_validation(
    command: &str,
    report_name: &str,
    cfg: ContainerValidationConfig,
    mode: ReleaseCheckMode,
    deb_body: &'static str,
    rpm_body: &'static str,
    deb_check: &str,
    rpm_check: &str,
) -> Result<ValidationReport, ValidationError> {
    require_cmd("docker")?;
    ensure_docker_reachable()?;

    let packages_dir = repo_root().join("target").join("packages");
    let out_dir = default_out_dir(command);
    prepare_container_evidence(&out_dir)?;
    let mut checks = Vec::new();
    let mut evidence_files = Vec::new();

    if matches!(mode, ReleaseCheckMode::Deb | ReleaseCheckMode::DebAndRpm) {
        let package = run_deb_validation(&cfg, &packages_dir, deb_body)?;
        checks.push(deb_check.to_string());
        write_package_stage_evidence(&out_dir, "deb", &cfg.deb_image, &package)?;
        evidence_files.push("deb.json".to_string());
    }

    if matches!(mode, ReleaseCheckMode::Rpm | ReleaseCheckMode::DebAndRpm) {
        let package = run_rpm_validation(&cfg, &packages_dir, rpm_body)?;
        checks.push(rpm_check.to_string());
        write_package_stage_evidence(&out_dir, "rpm", &cfg.rpm_image, &package)?;
        evidence_files.push("rpm.json".to_string());
    }

    write_container_summary(&out_dir, command, mode, &checks, &mut evidence_files)?;
    validation_log(format!("{report_name}: ok"));
    Ok(ValidationReport {
        name: format!("{report_name} ({})", mode.label()),
        out_dir,
        checks,
    })
}

fn run_deb_validation(
    cfg: &ContainerValidationConfig,
    packages_dir: &Path,
    body: &'static str,
) -> Result<PathBuf, ValidationError> {
    let deb_path = package_from_manifest(packages_dir, "deb", "deb")?;
    let package_name = deb_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ValidationError::new("invalid deb package filename"))?;
    let package_file = shell_single_quote(&format!("/packages/{package_name}"));

    let script = format!(
        "set -euo pipefail\n\
if [ -n \"${{GEWY_DEB_APT_MIRROR:-}}\" ]; then\n\
  sed -i \"s|http://archive.ubuntu.com/ubuntu|${{GEWY_DEB_APT_MIRROR}}|g; s|http://security.ubuntu.com/ubuntu|${{GEWY_DEB_APT_MIRROR}}|g\" /etc/apt/sources.list /etc/apt/sources.list.d/*.sources 2>/dev/null || true\n\
fi\n\
GEWY_PACKAGE_FILE={package_file}\n\
if ! dpkg -i \"${{GEWY_PACKAGE_FILE}}\" >/tmp/gewyvern-dpkg-install.log 2>&1; then\n\
  apt-get update >/dev/null\n\
  apt-get install -y \"${{GEWY_PACKAGE_FILE}}\" >/dev/null\n\
fi\n\
{body}\n"
    );

    run_docker_script(
        cfg.validation_name,
        "deb",
        packages_dir,
        &cfg.deb_image,
        &script,
    )?;
    validation_log(format!(
        "deb {} validation: ok ({})",
        cfg.validation_name,
        deb_path.display()
    ));
    Ok(deb_path)
}

fn run_rpm_validation(
    cfg: &ContainerValidationConfig,
    packages_dir: &Path,
    body: &'static str,
) -> Result<PathBuf, ValidationError> {
    let rpm_dir = packages_dir.join("rpm");
    let rpm_path = package_from_manifest(packages_dir, "rpm", "rpm")?;
    let package_name = rpm_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ValidationError::new("invalid rpm package filename"))?;
    let package_file = shell_single_quote(&format!("/packages/{package_name}"));

    let install_line =
        format!("rpm -Uvh {package_file} >/dev/null || dnf install -y {package_file} >/dev/null");

    let script = format!(
        "set -euo pipefail\n\
if [ -n \"${{GEWY_RPM_DNF_MIRROR:-}}\" ]; then\n\
  sed -i \"s|^metalink=|#metalink=|g; s|^mirrorlist=|#mirrorlist=|g; s|^#baseurl=http://download.example/pub/fedora/linux|baseurl=${{GEWY_RPM_DNF_MIRROR}}|g; s|^#baseurl=https://download.example/pub/fedora/linux|baseurl=${{GEWY_RPM_DNF_MIRROR}}|g\" /etc/yum.repos.d/*.repo 2>/dev/null || true\n\
fi\n\
GEWY_PACKAGE_FILE={package_file}\n\
{install_line}\n\
{body}\n"
    );

    run_docker_script(
        cfg.validation_name,
        "rpm",
        &rpm_dir,
        &cfg.rpm_image,
        &script,
    )?;
    validation_log(format!(
        "rpm {} validation: ok ({})",
        cfg.validation_name,
        rpm_path.display()
    ));
    Ok(rpm_path)
}

fn run_docker_script(
    validation_name: &str,
    mode_label: &str,
    mount_source: &Path,
    image: &str,
    script: &str,
) -> Result<(), ValidationError> {
    let timeout_seconds =
        env::var("GEWY_CONTAINER_VALIDATION_TIMEOUT_SECONDS").unwrap_or_else(|_| "900".to_string());
    let timeout_seconds = validate_positive_u16_timeout(&timeout_seconds)?;
    let container_name = format!(
        "gewyvern-{validation_name}-{mode_label}-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
    );
    let mount = format!("{}:/packages:ro", mount_source.display());

    let args = [
        "run",
        "--name",
        &container_name,
        "--rm",
        "-v",
        &mount,
        image,
        "bash",
        "-lc",
        script,
    ];

    let status = if has_command("timeout") {
        let mut cmd = Command::new("timeout");
        cmd.arg(&timeout_seconds);
        cmd.arg("docker");
        cmd.args(args);
        cmd.stdin(Stdio::null())
            .stdout(validation_command_stdout())
            .stderr(Stdio::inherit());
        let status = cmd.status().map_err(|err| {
            ValidationError::new(format!(
                "failed to launch timed container validation `{validation_name}`: {err}"
            ))
        })?;
        if status.code() == Some(124) {
            let _ = Command::new("docker")
                .args(["rm", "-f", &container_name])
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status();
            return Err(ValidationError::new(format!(
                "container validation timed out after {timeout_seconds}s: {validation_name}"
            )));
        }
        status
    } else {
        let mut cmd = Command::new("docker");
        cmd.args(args);
        cmd.stdin(Stdio::null())
            .stdout(validation_command_stdout())
            .stderr(Stdio::inherit());
        cmd.status().map_err(|err| {
            ValidationError::new(format!(
                "failed to launch container validation `{validation_name}`: {err}"
            ))
        })?
    };

    if !status.success() {
        return Err(ValidationError::new(format!(
            "container validation `{validation_name}` failed with status {status}"
        )));
    }

    Ok(())
}

fn package_from_manifest(
    packages_dir: &Path,
    key: &str,
    extension: &str,
) -> Result<PathBuf, ValidationError> {
    let manifest = packages_dir.join("build-manifest.txt");
    let metadata = fs::symlink_metadata(&manifest).map_err(|error| {
        ValidationError::new(format!(
            "package build manifest is unavailable: {}: {error}",
            manifest.display()
        ))
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(ValidationError::new(format!(
            "package build manifest is not a regular file: {}",
            manifest.display()
        )));
    }
    let body = fs::read_to_string(&manifest)?;
    let mut values = Vec::new();
    for (index, line) in body.lines().enumerate() {
        let (observed, value) = line.split_once('=').ok_or_else(|| {
            ValidationError::new(format!(
                "package build manifest line {} is malformed",
                index + 1
            ))
        })?;
        if observed.is_empty() || value.is_empty() {
            return Err(ValidationError::new(format!(
                "package build manifest line {} contains an empty key or value",
                index + 1
            )));
        }
        if observed == key {
            values.push(value);
        }
    }
    let value = values.first().copied().ok_or_else(|| {
        ValidationError::new(format!(
            "package build manifest does not contain a {key} artifact"
        ))
    })?;
    if values.len() != 1 {
        return Err(ValidationError::new(format!(
            "package build manifest contains duplicate {key} artifacts"
        )));
    }
    let declared_path = PathBuf::from(value);
    let path = if declared_path.is_absolute() {
        declared_path
    } else {
        packages_dir.join(declared_path)
    };
    let file_type = fs::symlink_metadata(&path)
        .map_err(|error| {
            ValidationError::new(format!(
                "package build manifest {key} artifact is unavailable: {}: {error}",
                path.display()
            ))
        })?
        .file_type();
    if file_type.is_symlink() || !file_type.is_file() {
        return Err(ValidationError::new(format!(
            "package candidate is not a regular file: {}",
            path.display()
        )));
    }
    let package_root = fs::canonicalize(packages_dir)?;
    let path = fs::canonicalize(&path)?;
    if path
        .strip_prefix(&package_root)
        .ok()
        .is_none_or(|relative| relative.as_os_str().is_empty())
    {
        return Err(ValidationError::new(format!(
            "package build manifest {key} artifact escapes the package root: {}",
            path.display()
        )));
    }
    if path.extension().and_then(|ext| ext.to_str()) != Some(extension) {
        return Err(ValidationError::new(format!(
            "package build manifest {key} artifact has the wrong extension: {}",
            path.display()
        )));
    }
    Ok(path)
}

fn validate_container_image(name: &str, value: &str) -> Result<String, ValidationError> {
    let trimmed = value.trim();
    if trimmed != value {
        return Err(ValidationError::new(format!(
            "{name} must not have surrounding whitespace"
        )));
    }
    if trimmed.is_empty() {
        return Err(ValidationError::new(format!("{name} must not be empty")));
    }
    if value.chars().any(|ch| ch.is_ascii_control()) {
        return Err(ValidationError::new(format!(
            "{name} must not contain control characters"
        )));
    }
    if value.chars().any(char::is_whitespace) {
        return Err(ValidationError::new(format!(
            "{name} must not contain whitespace"
        )));
    }
    if value.starts_with('-') {
        return Err(ValidationError::new(format!(
            "{name} must not start with option prefix: {value}"
        )));
    }
    Ok(value.to_string())
}

fn validate_positive_u16_timeout(value: &str) -> Result<String, ValidationError> {
    if value.is_empty() {
        return Err(ValidationError::new(
            "GEWY_CONTAINER_VALIDATION_TIMEOUT_SECONDS must not be empty".to_string(),
        ));
    }
    if !value.chars().all(|ch| ch.is_ascii_digit()) {
        return Err(ValidationError::new(
            "GEWY_CONTAINER_VALIDATION_TIMEOUT_SECONDS must be a positive integer".to_string(),
        ));
    }
    let parsed = value.parse::<u16>().map_err(|_| {
        ValidationError::new("GEWY_CONTAINER_VALIDATION_TIMEOUT_SECONDS must fit u16".to_string())
    })?;
    if parsed == 0 {
        return Err(ValidationError::new(
            "GEWY_CONTAINER_VALIDATION_TIMEOUT_SECONDS must be greater than zero".to_string(),
        ));
    }
    Ok(parsed.to_string())
}

fn shell_single_quote(value: &str) -> String {
    let escaped = value.replace('\'', "'\"'\"'");
    format!("'{escaped}'")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_manifest(root: &Path, lines: &[(&str, &Path)]) {
        let body = lines
            .iter()
            .map(|(key, path)| format!("{key}={}", path.display()))
            .collect::<Vec<_>>()
            .join("\n");
        fs::write(root.join("build-manifest.txt"), format!("{body}\n")).unwrap();
    }

    #[test]
    fn package_discovery_uses_the_manifest_instead_of_filename_order() {
        let root = env::temp_dir().join(format!(
            "gewyvern-package-discovery-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&root).unwrap();
        let current = root.join("gewyvern_1.16.0-1_amd64.deb");
        fs::write(&current, b"current-package").unwrap();
        fs::write(root.join("gewyvern_1.9.0-1_amd64.deb"), b"stale-package").unwrap();
        write_manifest(&root, &[("deb", Path::new("gewyvern_1.16.0-1_amd64.deb"))]);
        assert_eq!(
            package_from_manifest(&root, "deb", "deb").unwrap(),
            fs::canonicalize(&current).unwrap()
        );

        let duplicate = root.join("gewyvern_1.16.0-2_amd64.deb");
        fs::write(&duplicate, b"duplicate-package").unwrap();
        write_manifest(&root, &[("deb", &current), ("deb", &duplicate)]);
        assert!(package_from_manifest(&root, "deb", "deb").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn package_discovery_rejects_non_file_and_out_of_root_candidates() {
        let root = env::temp_dir().join(format!(
            "gewyvern-package-boundary-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        fs::create_dir_all(&root).unwrap();
        let fake = root.join("fake.deb");
        fs::create_dir(&fake).unwrap();
        write_manifest(&root, &[("deb", &fake)]);
        assert!(package_from_manifest(&root, "deb", "deb").is_err());

        let outside = root.with_extension("deb");
        fs::write(&outside, b"outside-package").unwrap();
        write_manifest(&root, &[("deb", &outside)]);
        assert!(package_from_manifest(&root, "deb", "deb").is_err());

        let traversal = PathBuf::from("..").join(outside.file_name().unwrap());
        write_manifest(&root, &[("deb", &traversal)]);
        assert!(package_from_manifest(&root, "deb", "deb").is_err());
        fs::remove_file(outside).unwrap();
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn package_install_smoke_persists_complete_machine_evidence() {
        let root = env::temp_dir().join(format!(
            "gewyvern-package-evidence-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let out_dir = root.join("evidence");
        fs::create_dir_all(&root).unwrap();
        let deb = root.join("gewyvern.deb");
        let rpm = root.join("gewyvern.rpm");
        fs::write(&deb, b"deb-package").unwrap();
        fs::write(&rpm, b"rpm-package").unwrap();

        prepare_container_evidence(&out_dir).unwrap();
        write_package_stage_evidence(&out_dir, "deb", "ubuntu:test", &deb).unwrap();
        write_package_stage_evidence(&out_dir, "rpm", "fedora:test", &rpm).unwrap();
        let checks = vec![
            "deb_install_smoke".to_string(),
            "rpm_install_smoke".to_string(),
        ];
        let mut files = vec!["deb.json".to_string(), "rpm.json".to_string()];
        write_container_summary(
            &out_dir,
            "package-install-smoke",
            ReleaseCheckMode::DebAndRpm,
            &checks,
            &mut files,
        )
        .unwrap();

        let deb_evidence: serde_json::Value =
            serde_json::from_slice(&fs::read(out_dir.join("deb.json")).unwrap()).unwrap();
        assert_eq!(deb_evidence["status"], "ok");
        assert_eq!(deb_evidence["image"], "ubuntu:test");
        assert_eq!(deb_evidence["artifact_bytes"], 11);
        assert_eq!(deb_evidence["artifact_sha256"].as_str().unwrap().len(), 64);

        let summary: serde_json::Value =
            serde_json::from_slice(&fs::read(out_dir.join("summary.json")).unwrap()).unwrap();
        assert_eq!(summary["mode"], "deb+rpm");
        assert_eq!(summary["checks"].as_array().unwrap().len(), 2);

        let index: serde_json::Value =
            serde_json::from_slice(&fs::read(out_dir.join("evidence-index.json")).unwrap())
                .unwrap();
        assert_eq!(index["files"].as_array().unwrap().len(), 3);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn container_validation_rejects_unsafe_image_values() {
        assert!(validate_container_image("GEWY_IMAGE", "").is_err());
        assert!(validate_container_image("GEWY_IMAGE", "linux \n").is_err());
        assert!(validate_container_image("GEWY_IMAGE", "  ubuntu:24.04").is_err());
        assert!(validate_container_image("GEWY_IMAGE", "ubuntu:24.04").is_ok());
    }

    #[test]
    fn container_validation_timeout_requires_positive_numeric_value() {
        assert!(validate_positive_u16_timeout("0").is_err());
        assert!(validate_positive_u16_timeout("abc").is_err());
        assert!(validate_positive_u16_timeout("30").is_ok());
    }
}

fn require_cmd(name: &str) -> Result<(), ValidationError> {
    if has_command(name) {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "required command not found: {name}"
        )))
    }
}

fn has_command(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    let candidate = Path::new(name);
    if candidate.components().count() > 1 {
        return is_executable_file(candidate);
    }
    env::var_os("PATH")
        .map(|path| {
            env::split_paths(&path).any(|dir| {
                let base = dir.join(name);
                command_probe_candidates(&base)
                    .into_iter()
                    .any(|candidate| is_executable_file(&candidate))
            })
        })
        .unwrap_or(false)
}

fn command_probe_candidates(base: &Path) -> Vec<PathBuf> {
    #[cfg(windows)]
    {
        let mut candidates = Vec::new();
        let has_extension = base.extension().is_some();
        candidates.push(base.to_path_buf());
        if !has_extension {
            if let Some(path_ext) = env::var_os("PATHEXT") {
                for suffix in path_ext.to_string_lossy().split(';') {
                    let trimmed = suffix.trim();
                    if trimmed.is_empty() {
                        continue;
                    }
                    let suffix = trimmed.trim_start_matches('.');
                    candidates.push(base.with_extension(suffix));
                }
            }
        }
        candidates
    }
    #[cfg(not(windows))]
    {
        vec![base.to_path_buf()]
    }
}

fn is_executable_file(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if !metadata.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        true
    }
}

fn ensure_docker_reachable() -> Result<(), ValidationError> {
    let status = Command::new("docker")
        .arg("info")
        .stdout(validation_command_stdout())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| ValidationError::new(format!("failed to query docker: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(ValidationError::new(
            "docker daemon is not reachable; start Docker Desktop or another local daemon and retry",
        ))
    }
}

fn protocol_validation_body() -> &'static str {
    r#"
expect_contains() {
  local file="$1"
  local needle="$2"
  if ! grep -q "$needle" "$file"; then
    echo "expected to find '${needle}' in ${file}" >&2
    exit 1
  fi
}

echo "[protocol] validating protocol registry visibility"

gewyvern --list-protocols >/tmp/list-protocols.txt
expect_contains /tmp/list-protocols.txt 'dns (default: udp)'
expect_contains /tmp/list-protocols.txt 'http (default: request)'
expect_contains /tmp/list-protocols.txt 'tls (default: client)'
expect_contains /tmp/list-protocols.txt 'http3 (default: request)'
expect_contains /tmp/list-protocols.txt 'quic (default: initial)'
expect_contains /tmp/list-protocols.txt 'ssh (default: session)'
expect_contains /tmp/list-protocols.txt 'socks5 (default: session)'
expect_contains /tmp/list-protocols.txt 'mysql (default: session)'
expect_contains /tmp/list-protocols.txt 'postgres (default: query)'
expect_contains /tmp/list-protocols.txt 'smtp (default: session)'
expect_contains /tmp/list-protocols.txt 'ldap (default: sync)'
expect_contains /tmp/list-protocols.txt 'redis (default: ping)'
expect_contains /tmp/list-protocols.txt 'mqtt (default: connect)'
expect_contains /tmp/list-protocols.txt 'amqp (default: session)'
expect_contains /tmp/list-protocols.txt 'radius (default: access)'
expect_contains /tmp/list-protocols.txt 'snmp (default: get)'
expect_contains /tmp/list-protocols.txt 'ftp (default: session)'
expect_contains /tmp/list-protocols.txt 'imap (default: auth)'
expect_contains /tmp/list-protocols.txt 'pop3 (default: auth)'
expect_contains /tmp/list-protocols.txt 'kerberos (default: as)'
expect_contains /tmp/list-protocols.txt 'rtsp (default: options)'

echo "[protocol] validating resolution, web, and secure transport families"

gewyvern --protocol dns --entry udp --json --summary-only >/tmp/dns.json
gewyvern --protocol http --entry request --json --summary-only >/tmp/http.json
gewyvern --protocol tls --entry client --json --summary-only >/tmp/tls.json
gewyvern --protocol http3 --entry request --json --summary-only >/tmp/http3.json
gewyvern --protocol quic --entry initial --json --summary-only >/tmp/quic.json

expect_contains /tmp/dns.json '"primary_module_kind":"name_resolution"'
expect_contains /tmp/dns.json '"operator_guidance_action":"manual_review"'
expect_contains /tmp/http.json '"primary_module_kind":"http_request_response"'
expect_contains /tmp/http.json '"operator_guidance_action":"manual_review"'
expect_contains /tmp/tls.json '"primary_module_kind":"tls_handshake"'
expect_contains /tmp/tls.json '"operator_guidance_action":"manual_review"'
expect_contains /tmp/http3.json '"primary_module_kind":"http3_request_response"'
expect_contains /tmp/http3.json '"operator_guidance_action":"safe_to_escalate_protocol_signal"'
expect_contains /tmp/quic.json '"primary_module_kind":"quic_handshake"'
expect_contains /tmp/quic.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "[protocol] validating remote access and proxy families"

gewyvern --protocol ssh --entry session --json --summary-only >/tmp/ssh.json
gewyvern --protocol socks5 --entry auth --json --summary-only >/tmp/socks5.json

expect_contains /tmp/ssh.json '"primary_module_kind":"remote_access_session"'
expect_contains /tmp/ssh.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/socks5.json '"primary_module_kind":"proxy_authentication"'
expect_contains /tmp/socks5.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "[protocol] validating database, messaging, and directory families"

gewyvern --protocol mysql --entry session --json --summary-only >/tmp/mysql.json
gewyvern --protocol mysql --entry query --json --summary-only >/tmp/mysql-query.json
gewyvern --protocol postgres --entry query --json --summary-only >/tmp/postgres.json
gewyvern --protocol smtp --entry session --json --summary-only >/tmp/smtp.json
gewyvern --protocol ldap --entry sync --json --summary-only >/tmp/ldap.json

expect_contains /tmp/mysql.json '"primary_module_kind":"database_query"'
expect_contains /tmp/mysql.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/mysql-query.json '"primary_module_kind":"database_query"'
expect_contains /tmp/mysql-query.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/postgres.json '"primary_module_kind":"database_query"'
expect_contains /tmp/postgres.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/smtp.json '"primary_module_kind":"mail_session"'
expect_contains /tmp/smtp.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/ldap.json '"primary_module_kind":"directory_sync"'
expect_contains /tmp/ldap.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "[protocol] validating cache, broker, auth, management, and signaling families"

gewyvern --protocol redis --entry ping --json --summary-only >/tmp/redis.json
gewyvern --protocol mqtt --entry connect --json --summary-only >/tmp/mqtt.json
gewyvern --protocol amqp --entry start --json --summary-only >/tmp/amqp.json
gewyvern --protocol radius --entry access --json --summary-only >/tmp/radius.json
gewyvern --protocol snmp --entry get --json --summary-only >/tmp/snmp.json
gewyvern --protocol ftp --entry session --json --summary-only >/tmp/ftp.json
gewyvern --protocol imap --entry auth --json --summary-only >/tmp/imap.json
gewyvern --protocol pop3 --entry auth --json --summary-only >/tmp/pop3.json
gewyvern --protocol kerberos --entry as --json --summary-only >/tmp/kerberos.json
gewyvern --protocol rtsp --entry describe --json --summary-only >/tmp/rtsp.json

expect_contains /tmp/redis.json '"primary_module_kind":"cache_access"'
expect_contains /tmp/redis.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/mqtt.json '"primary_module_kind":"message_session"'
expect_contains /tmp/mqtt.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/amqp.json '"primary_module_kind":"message_session"'
expect_contains /tmp/amqp.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/radius.json '"primary_module_kind":"authentication_exchange"'
expect_contains /tmp/radius.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/snmp.json '"primary_module_kind":"management_query"'
expect_contains /tmp/snmp.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/ftp.json '"primary_module_kind":"authentication_exchange"'
expect_contains /tmp/ftp.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/imap.json '"primary_module_kind":"authentication_exchange"'
expect_contains /tmp/imap.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/pop3.json '"primary_module_kind":"authentication_exchange"'
expect_contains /tmp/pop3.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/kerberos.json '"primary_module_kind":"authentication_exchange"'
expect_contains /tmp/kerberos.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/rtsp.json '"primary_module_kind":"signaling_session"'
expect_contains /tmp/rtsp.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "[protocol] validating full packaged registry sweep"

gewyvern --scan-all --json --summary-only >/tmp/scan-all.json
expect_contains /tmp/scan-all.json '"total_targets":'

echo "container protocol validation: ok"
"#
}

fn operator_validation_body() -> &'static str {
    r#"
expect_contains() {
  local file="$1"
  local needle="$2"
  if ! grep -q "$needle" "$file"; then
    echo "expected to find '${needle}' in ${file}" >&2
    exit 1
  fi
}

echo "[operator-path] validating advisory resolution and application paths"

gewyvern --protocol dns --entry udp --json --summary-only >/tmp/path-dns.json
gewyvern --protocol quic --entry initial --json --summary-only >/tmp/path-quic.json
gewyvern --protocol http3 --entry request --json --summary-only >/tmp/path-http3.json

expect_contains /tmp/path-dns.json '"primary_module_kind":"name_resolution"'
expect_contains /tmp/path-dns.json '"operator_guidance_action":"manual_review"'
expect_contains /tmp/path-quic.json '"primary_module_kind":"quic_handshake"'
expect_contains /tmp/path-quic.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-quic.json '"operator_guidance_action":"collect_more_runtime_evidence"'
expect_contains /tmp/path-http3.json '"primary_module_kind":"http3_request_response"'
expect_contains /tmp/path-http3.json '"operator_guidance_action":"safe_to_escalate_protocol_signal"'

echo "[operator-path] validating secure transport and tunnel paths"

gewyvern --protocol tls --entry client --json --summary-only >/tmp/path-tls.json
gewyvern --protocol https --entry connect --json --summary-only >/tmp/path-https-connect.json

expect_contains /tmp/path-tls.json '"primary_module_kind":"tls_handshake"'
expect_contains /tmp/path-tls.json '"operator_guidance_action":"manual_review"'
expect_contains /tmp/path-https-connect.json '"primary_module_kind":"network_module"'
expect_contains /tmp/path-https-connect.json '"operator_guidance_action":"manual_review"'

gewyvern --protocol socks5 --entry auth --json --summary-only >/tmp/path-socks5-auth.json

expect_contains /tmp/path-socks5-auth.json '"primary_module_kind":"proxy_authentication"'
expect_contains /tmp/path-socks5-auth.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-socks5-auth.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "[operator-path] validating secure database and mail paths"

gewyvern --protocol postgres --entry query --json --summary-only >/tmp/path-postgres.json

expect_contains /tmp/path-postgres.json '"primary_module_kind":"database_query"'
expect_contains /tmp/path-postgres.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-postgres.json '"operator_guidance_action":"collect_more_runtime_evidence"'

gewyvern --protocol mysql --entry session --json --summary-only >/tmp/path-mysql.json

expect_contains /tmp/path-mysql.json '"primary_module_kind":"database_query"'
expect_contains /tmp/path-mysql.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-mysql.json '"operator_guidance_action":"collect_more_runtime_evidence"'

gewyvern --protocol smtp --entry auth --json --summary-only >/tmp/path-smtp-auth.json

expect_contains /tmp/path-smtp-auth.json '"primary_module_kind":"authentication_exchange"'
expect_contains /tmp/path-smtp-auth.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-smtp-auth.json '"operator_guidance_action":"collect_more_runtime_evidence"'

gewyvern --protocol smtp --entry session --json --summary-only >/tmp/path-smtp.json

expect_contains /tmp/path-smtp.json '"primary_module_kind":"mail_session"'
expect_contains /tmp/path-smtp.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-smtp.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "[operator-path] validating conservative negative-path guard"

gewyvern --protocol socks5 --entry auth-denied --json --summary-only >/tmp/path-socks5-auth-denied.json

expect_contains /tmp/path-socks5-auth-denied.json '"primary_module_kind":"proxy_authentication"'
expect_contains /tmp/path-socks5-auth-denied.json '"primary_failure_basis":"missing_transition"'
expect_contains /tmp/path-socks5-auth-denied.json '"operator_guidance_action":"collect_more_runtime_evidence"'

echo "container operator path validation: ok"
"#
}

fn runtime_validation_body() -> &'static str {
    r#"
TCP_SOCKET="127.0.0.1:19090"
TCP_API="127.0.0.1:19190"
TCP_SUMMARY="/tmp/tcp-summary.json"
TCP_ANALYSIS="/tmp/tcp-analysis.json"
TCP_EXPORT="/tmp/tcp-export.json"

UDP_SOCKET="127.0.0.1:19091"
UDP_API="127.0.0.1:19191"
UDP_SUMMARY="/tmp/udp-summary.json"
UDP_ANALYSIS="/tmp/udp-analysis.json"

expect_contains() {
  local file="$1"
  local needle="$2"
  if ! grep -q "$needle" "$file"; then
    echo "expected to find '${needle}' in ${file}" >&2
    exit 1
  fi
}

wait_for_http_body() {
  local url="$1"
  local out="$2"
  local fragment="${3:-}"
  local address="${url#http://}"
  local authority="${address%%/*}"
  local path="/${address#*/}"
  local host="${authority%:*}"
  local port="${authority##*:}"
  local response="${out}.http-response"
  if [ "${address}" = "${authority}" ]; then
    path="/"
  fi
  for _ in $(seq 1 120); do
    local http_ok=false
    if { exec 3<>"/dev/tcp/${host}/${port}"; } 2>/dev/null; then
      printf 'GET %s HTTP/1.1\r\nHost: %s:%s\r\nAccept: application/json\r\nConnection: close\r\n\r\n' \
        "${path}" "${host}" "${port}" >&3
      timeout 2 cat <&3 >"${response}" 2>/dev/null || true
      exec 3>&-
      exec 3<&-
      if grep -q '^HTTP/1\.[01] 200 ' "${response}"; then
        sed '1,/^\r$/d' "${response}" >"$out"
        http_ok=true
      else
        : >"$out"
      fi
      rm -f "${response}"
      if [ "${http_ok}" = true ] && { [ -z "${fragment}" ] || grep -q "${fragment}" "$out"; }; then
        return 0
      fi
    fi
    sleep 0.1
  done
  echo "timed out waiting for ${url}" >&2
  exit 1
}

start_server() {
  local template="$1"
  local socket_addr="$2"
  local api_addr="$3"
  local log_path="$4"
  gewyvern --tcp-socket "${socket_addr}" --template "${template}" --serve --api-socket "${api_addr}" --json --summary-only >"${log_path}" 2>&1 &
  echo $!
}

stop_server() {
  local pid="$1"
  kill "${pid}" >/dev/null 2>&1 || true
  wait "${pid}" >/dev/null 2>&1 || true
}

send_template() {
  local socket_addr="$1"
  local template="$2"
  gewyvern_socket_send --tcp-socket "${socket_addr}" --template "${template}"
}

send_invalid_session() {
  local socket_addr="$1"
  gewyvern_socket_send --tcp-socket "${socket_addr}" --raw-line '{"broken":true'
}

TCP_PID="$(start_server tcp "${TCP_SOCKET}" "${TCP_API}" /tmp/tcp-serve.log)"
trap 'stop_server "${TCP_PID:-}"; stop_server "${UDP_PID:-}"' EXIT

wait_for_http_body "http://${TCP_API}/health" /tmp/tcp-health.txt
send_template "${TCP_SOCKET}" tcp
wait_for_http_body "http://${TCP_API}/v1/latest/summary.json" "${TCP_SUMMARY}" '"primary_module_kind":"connection_establishment"'
wait_for_http_body "http://${TCP_API}/v1/latest/export.json" "${TCP_EXPORT}" '"template_id":"handshake_debug"'
expect_contains "${TCP_SUMMARY}" '"primary_module_kind":"connection_establishment"'
expect_contains "${TCP_SUMMARY}" '"operator_guidance_action":"avoid_pid_strong_actions"'
expect_contains "${TCP_EXPORT}" '"template_id":"handshake_debug"'

send_template "${TCP_SOCKET}" tcp
wait_for_http_body "http://${TCP_API}/v1/latest/summary.json" "${TCP_SUMMARY}" '"accepted_facts":3'
expect_contains "${TCP_SUMMARY}" '"accepted_facts":3'

send_invalid_session "${TCP_SOCKET}"
wait_for_http_body "http://${TCP_API}/health" /tmp/tcp-health-after-bad.txt
expect_contains /tmp/tcp-health-after-bad.txt '"ok":true'

send_template "${TCP_SOCKET}" tcp
wait_for_http_body "http://${TCP_API}/v1/latest/analysis.json" "${TCP_ANALYSIS}" '"protocol_flows"'
expect_contains "${TCP_ANALYSIS}" '"protocol_flows"'
stop_server "${TCP_PID}"

UDP_PID="$(start_server udp "${UDP_SOCKET}" "${UDP_API}" /tmp/udp-serve.log)"
wait_for_http_body "http://${UDP_API}/health" /tmp/udp-health.txt
send_template "${UDP_SOCKET}" udp
wait_for_http_body "http://${UDP_API}/v1/latest/summary.json" "${UDP_SUMMARY}" '"primary_module_kind":"datagram_exchange"'
wait_for_http_body "http://${UDP_API}/v1/latest/analysis.json" "${UDP_ANALYSIS}" '"primary_failure_mode":"none"'
expect_contains "${UDP_SUMMARY}" '"primary_module_kind":"datagram_exchange"'
expect_contains "${UDP_SUMMARY}" '"operator_guidance_action":"avoid_pid_strong_actions"'
expect_contains "${UDP_ANALYSIS}" '"primary_failure_mode":"none"'
stop_server "${UDP_PID}"

echo "container runtime validation: ok"
"#
}

fn package_install_smoke_deb_body() -> &'static str {
    r#"
PRODUCT_VERSION="$(gewyvern --version)"
PRODUCT_VERSION="${PRODUCT_VERSION#gewyvern }"
RELEASE_LINE="${GEWY_RELEASE_LINE:-v${PRODUCT_VERSION}}"

dpkg-deb -c "${GEWY_PACKAGE_FILE}" >/tmp/gewyvern-package-contents.txt
grep -q './usr/share/doc/gewyvern/LICENSE' /tmp/gewyvern-package-contents.txt
command -v gewyvern >/dev/null
command -v gewyc >/dev/null
command -v gewyvern_socket_send >/dev/null
gewyvern --list-protocols >/dev/null
gewyc /usr/share/gewyvern/dsl/http_request_path.gewy --json >/dev/null
test -d /usr/share/gewyvern/dsl
test -d /usr/share/gewyvern/protocols
test -f /usr/share/gewyvern/package-compat.toml
grep -q '^schema_version = 1$' /usr/share/gewyvern/package-compat.toml
grep -q "^release_line = \"${RELEASE_LINE}\"$" /usr/share/gewyvern/package-compat.toml
test -f /usr/share/gewyvern/examples/gewyvern.toml.example
test -x /usr/libexec/gewyvern-ebpf-helper
test -x /usr/sbin/gewyvern-ebpf-provision
test -f /usr/share/gewyvern/examples/ebpf-helper.conf.example
test -f /usr/share/gewyvern/examples/gewyvern-ebpf-validation.sudoers.example
"#
}

fn package_install_smoke_rpm_body() -> &'static str {
    r#"
PRODUCT_VERSION="$(gewyvern --version)"
PRODUCT_VERSION="${PRODUCT_VERSION#gewyvern }"
RELEASE_LINE="${GEWY_RELEASE_LINE:-v${PRODUCT_VERSION}}"

rpm -qpl "${GEWY_PACKAGE_FILE}" >/tmp/gewyvern-package-contents.txt
grep -q '/usr/share/doc/gewyvern/LICENSE' /tmp/gewyvern-package-contents.txt
command -v gewyvern >/dev/null
command -v gewyc >/dev/null
command -v gewyvern_socket_send >/dev/null
gewyvern --list-protocols >/dev/null
gewyc /usr/share/gewyvern/dsl/http_request_path.gewy --json >/dev/null
test -d /usr/share/gewyvern/dsl
test -d /usr/share/gewyvern/protocols
test -f /usr/share/gewyvern/package-compat.toml
grep -q '^schema_version = 1$' /usr/share/gewyvern/package-compat.toml
grep -q "^release_line = \"${RELEASE_LINE}\"$" /usr/share/gewyvern/package-compat.toml
test -f /usr/share/gewyvern/examples/gewyvern.toml.example
test -x /usr/libexec/gewyvern-ebpf-helper
test -x /usr/sbin/gewyvern-ebpf-provision
test -f /usr/share/gewyvern/examples/ebpf-helper.conf.example
test -f /usr/share/gewyvern/examples/gewyvern-ebpf-validation.sudoers.example
"#
}
