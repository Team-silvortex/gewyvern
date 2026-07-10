use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use super::command::{ValidationError, ValidationReport, default_out_dir, repo_root};
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
    require_cmd("docker")?;
    ensure_docker_reachable()?;

    let cfg = ContainerValidationConfig::new(
        "install-smoke",
        "GEWY_DEB_SMOKE_IMAGE",
        DEFAULT_DEB_SMOKE_IMAGE,
        "GEWY_RPM_SMOKE_IMAGE",
        DEFAULT_RPM_SMOKE_IMAGE,
    );
    let packages_dir = repo_root().join("target").join("packages");
    let mut checks = Vec::new();

    if matches!(mode, ReleaseCheckMode::Deb | ReleaseCheckMode::DebAndRpm) {
        run_deb_validation(&cfg, &packages_dir, package_install_smoke_deb_body(), false)?;
        checks.push("deb_install_smoke".to_string());
    }

    if matches!(mode, ReleaseCheckMode::Rpm | ReleaseCheckMode::DebAndRpm) {
        run_rpm_validation(&cfg, &packages_dir, package_install_smoke_rpm_body(), false)?;
        checks.push("rpm_install_smoke".to_string());
    }

    println!("package install smoke: ok");
    Ok(ValidationReport {
        name: format!("package install smoke ({})", mode.label()),
        out_dir: default_out_dir("package-install-smoke"),
        checks,
    })
}

pub fn run_container_runtime_validation(
    mode: ReleaseCheckMode,
) -> Result<ValidationReport, ValidationError> {
    require_cmd("docker")?;
    ensure_docker_reachable()?;

    let cfg = ContainerValidationConfig::new(
        "runtime",
        "GEWY_DEB_RUNTIME_IMAGE",
        DEFAULT_DEB_RUNTIME_IMAGE,
        "GEWY_RPM_RUNTIME_IMAGE",
        DEFAULT_RPM_RUNTIME_IMAGE,
    );
    let packages_dir = repo_root().join("target").join("packages");
    let mut checks = Vec::new();

    if matches!(mode, ReleaseCheckMode::Deb | ReleaseCheckMode::DebAndRpm) {
        run_deb_validation(&cfg, &packages_dir, runtime_validation_body(), true)?;
        checks.push("deb_runtime_validation".to_string());
    }

    if matches!(mode, ReleaseCheckMode::Rpm | ReleaseCheckMode::DebAndRpm) {
        run_rpm_validation(&cfg, &packages_dir, runtime_validation_body(), true)?;
        checks.push("rpm_runtime_validation".to_string());
    }

    println!("container runtime validation: ok");
    Ok(ValidationReport {
        name: format!("container runtime validation ({})", mode.label()),
        out_dir: default_out_dir("container-runtime-validation"),
        checks,
    })
}

pub fn run_container_protocol_validation(
    mode: ReleaseCheckMode,
) -> Result<ValidationReport, ValidationError> {
    require_cmd("docker")?;
    ensure_docker_reachable()?;

    let cfg = ContainerValidationConfig::new(
        "protocol",
        "GEWY_DEB_PROTOCOL_IMAGE",
        DEFAULT_DEB_PROTOCOL_IMAGE,
        "GEWY_RPM_PROTOCOL_IMAGE",
        DEFAULT_RPM_PROTOCOL_IMAGE,
    );
    let packages_dir = repo_root().join("target").join("packages");
    let mut checks = Vec::new();

    if matches!(mode, ReleaseCheckMode::Deb | ReleaseCheckMode::DebAndRpm) {
        run_deb_validation(&cfg, &packages_dir, protocol_validation_body(), false)?;
        checks.push("deb_protocol_validation".to_string());
    }

    if matches!(mode, ReleaseCheckMode::Rpm | ReleaseCheckMode::DebAndRpm) {
        run_rpm_validation(&cfg, &packages_dir, protocol_validation_body(), false)?;
        checks.push("rpm_protocol_validation".to_string());
    }

    println!("container protocol validation: ok");
    Ok(ValidationReport {
        name: format!("container protocol validation ({})", mode.label()),
        out_dir: default_out_dir("container-protocol-validation"),
        checks,
    })
}

pub fn run_container_operator_path_validation(
    mode: ReleaseCheckMode,
) -> Result<ValidationReport, ValidationError> {
    require_cmd("docker")?;
    ensure_docker_reachable()?;

    let cfg = ContainerValidationConfig::new(
        "operator-path",
        "GEWY_DEB_OPERATOR_IMAGE",
        DEFAULT_DEB_OPERATOR_IMAGE,
        "GEWY_RPM_OPERATOR_IMAGE",
        DEFAULT_RPM_OPERATOR_IMAGE,
    );
    let packages_dir = repo_root().join("target").join("packages");
    let mut checks = Vec::new();

    if matches!(mode, ReleaseCheckMode::Deb | ReleaseCheckMode::DebAndRpm) {
        run_deb_validation(&cfg, &packages_dir, operator_validation_body(), false)?;
        checks.push("deb_operator_path_validation".to_string());
    }

    if matches!(mode, ReleaseCheckMode::Rpm | ReleaseCheckMode::DebAndRpm) {
        run_rpm_validation(&cfg, &packages_dir, operator_validation_body(), false)?;
        checks.push("rpm_operator_path_validation".to_string());
    }

    println!("container operator path validation: ok");
    Ok(ValidationReport {
        name: format!("container operator path validation ({})", mode.label()),
        out_dir: default_out_dir("container-operator-path-validation"),
        checks,
    })
}

pub fn run_container_validation_summary(
    mode: ReleaseCheckMode,
) -> Result<ValidationReport, ValidationError> {
    println!(
        "[summary] starting packaged container validation ({})",
        mode.label()
    );

    println!("[summary] ----------------------------------------");
    println!("[summary] running packaged protocol validation");
    let protocol = run_container_protocol_validation(mode)?;

    println!("[summary] ----------------------------------------");
    println!("[summary] running packaged operator-path validation");
    let operator = run_container_operator_path_validation(mode)?;

    println!("[summary] ----------------------------------------");
    println!(
        "[summary] packaged container validation: ok ({})",
        mode.label()
    );

    let mut checks = protocol.checks;
    checks.extend(operator.checks);
    Ok(ValidationReport {
        name: format!("packaged container validation summary ({})", mode.label()),
        out_dir: default_out_dir("container-validation-summary"),
        checks,
    })
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
    ) -> Self {
        Self {
            validation_name,
            deb_image: env::var(deb_env).unwrap_or_else(|_| deb_default.to_string()),
            rpm_image: env::var(rpm_env).unwrap_or_else(|_| rpm_default.to_string()),
        }
    }
}

fn run_deb_validation(
    cfg: &ContainerValidationConfig,
    packages_dir: &Path,
    body: &'static str,
    install_curl: bool,
) -> Result<(), ValidationError> {
    let deb_path = find_latest_package(packages_dir, "deb")?.ok_or_else(|| {
        ValidationError::new(format!(
            "no .deb artifact found under {}",
            packages_dir.display()
        ))
    })?;
    let package_name = deb_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ValidationError::new("invalid deb package filename"))?;

    let install_line = if install_curl {
        format!("apt-get install -y curl /packages/{package_name} >/dev/null")
    } else {
        format!("apt-get install -y /packages/{package_name} >/dev/null")
    };

    let script = format!(
        "set -euo pipefail\n\
if [ -n \"${{GEWY_DEB_APT_MIRROR:-}}\" ]; then\n\
  sed -i \"s|http://archive.ubuntu.com/ubuntu|${{GEWY_DEB_APT_MIRROR}}|g; s|http://security.ubuntu.com/ubuntu|${{GEWY_DEB_APT_MIRROR}}|g\" /etc/apt/sources.list /etc/apt/sources.list.d/*.sources 2>/dev/null || true\n\
fi\n\
GEWY_PACKAGE_FILE=\"/packages/{package_name}\"\n\
apt-get update >/dev/null\n\
{install_line}\n\
{body}\n"
    );

    run_docker_script(
        cfg.validation_name,
        "deb",
        packages_dir,
        &cfg.deb_image,
        &script,
    )?;
    println!(
        "deb {} validation: ok ({})",
        cfg.validation_name,
        deb_path.display()
    );
    Ok(())
}

fn run_rpm_validation(
    cfg: &ContainerValidationConfig,
    packages_dir: &Path,
    body: &'static str,
    install_curl: bool,
) -> Result<(), ValidationError> {
    let rpm_dir = packages_dir.join("rpm");
    let rpm_path = find_latest_package(&rpm_dir, "rpm")?.ok_or_else(|| {
        ValidationError::new(format!(
            "no .rpm artifact found under {}",
            rpm_dir.display()
        ))
    })?;
    let package_name = rpm_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| ValidationError::new("invalid rpm package filename"))?;

    let install_line = if install_curl {
        format!(
            "if ! command -v curl >/dev/null 2>&1; then\n  dnf install -y curl >/dev/null\nfi\nrpm -Uvh /packages/{package_name} >/dev/null || dnf install -y /packages/{package_name} >/dev/null"
        )
    } else {
        format!(
            "rpm -Uvh /packages/{package_name} >/dev/null || dnf install -y /packages/{package_name} >/dev/null"
        )
    };

    let script = format!(
        "set -euo pipefail\n\
if [ -n \"${{GEWY_RPM_DNF_MIRROR:-}}\" ]; then\n\
  sed -i \"s|^metalink=|#metalink=|g; s|^mirrorlist=|#mirrorlist=|g; s|^#baseurl=http://download.example/pub/fedora/linux|baseurl=${{GEWY_RPM_DNF_MIRROR}}|g; s|^#baseurl=https://download.example/pub/fedora/linux|baseurl=${{GEWY_RPM_DNF_MIRROR}}|g\" /etc/yum.repos.d/*.repo 2>/dev/null || true\n\
fi\n\
GEWY_PACKAGE_FILE=\"/packages/{package_name}\"\n\
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
    println!(
        "rpm {} validation: ok ({})",
        cfg.validation_name,
        rpm_path.display()
    );
    Ok(())
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
            .stdout(Stdio::inherit())
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
            .stdout(Stdio::inherit())
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

fn find_latest_package(dir: &Path, extension: &str) -> Result<Option<PathBuf>, ValidationError> {
    if !dir.is_dir() {
        return Ok(None);
    }

    let mut candidates = fs::read_dir(dir)?
        .filter_map(|entry| entry.ok().map(|item| item.path()))
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some(extension))
        .collect::<Vec<_>>();
    candidates.sort();
    Ok(candidates.pop())
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
    Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {name} >/dev/null 2>&1"))
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn ensure_docker_reachable() -> Result<(), ValidationError> {
    let status = Command::new("docker")
        .arg("info")
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
  for _ in $(seq 1 120); do
    if curl -fsS "$url" >"$out" 2>/dev/null; then
      if [ -z "${fragment}" ] || grep -q "${fragment}" "$out"; then
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
RELEASE_LINE="${GEWY_RELEASE_LINE:-v0.20.x}"

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
"#
}

fn package_install_smoke_rpm_body() -> &'static str {
    r#"
RELEASE_LINE="${GEWY_RELEASE_LINE:-v0.20.x}"

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
"#
}
