use std::env;
use std::fs;
use std::io::Write;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

use super::command::{ValidationError, ValidationReport, default_out_dir, repo_root};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RemoteLinuxHostOptions {
    pub host: String,
    pub remote_dir: Option<String>,
    pub build_packages: bool,
    pub keep_remote_dir: bool,
}

impl Default for RemoteLinuxHostOptions {
    fn default() -> Self {
        Self {
            host: env::var("GEWY_REMOTE_HOST").unwrap_or_else(|_| "kyuubiki-lab".to_string()),
            remote_dir: None,
            build_packages: true,
            keep_remote_dir: false,
        }
    }
}

pub fn run_remote_linux_host_validation(
    options: RemoteLinuxHostOptions,
) -> Result<ValidationReport, ValidationError> {
    require_cmd("ssh")?;
    require_cmd("rsync")?;

    let out_dir = default_out_dir("remote-linux-host-validation");
    fs::create_dir_all(&out_dir)?;

    let remote_dir = options
        .remote_dir
        .clone()
        .unwrap_or_else(default_remote_dir);
    let remote_path = remote_workspace_path(&remote_dir);
    let release_line = env::var("GEWY_RELEASE_LINE").unwrap_or_else(|_| "v0.20.x".to_string());

    println!("[remote-host] host: {}", options.host);
    println!("[remote-host] remote workspace: {}", remote_path);

    let result: Result<ValidationReport, ValidationError> = (|| {
        println!("[remote-host] ----------------------------------------");
        println!("[remote-host] creating remote workspace");
        run_ssh_command(
            &options.host,
            &format!("mkdir -p {remote_path}"),
            "failed to create remote workspace",
        )?;

        println!("[remote-host] ----------------------------------------");
        println!("[remote-host] syncing current workspace");
        sync_workspace(&options.host, &remote_path)?;

        let mut checks = vec!["workspace_synced".to_string()];

        if options.build_packages {
            println!("[remote-host] ----------------------------------------");
            println!("[remote-host] building x86_64 packages on remote host");
            run_ssh_command(
                &options.host,
                &format!("cd {remote_path} && ./scripts/packaging/build_packages.sh --format all"),
                "remote package build failed",
            )?;
            checks.push("remote_package_build".to_string());
        } else {
            println!("[remote-host] skipping remote package build");
        }

        println!("[remote-host] ----------------------------------------");
        println!("[remote-host] running remote package smoke");
        run_ssh_script(
            &options.host,
            &format!("cd {remote_path} && bash -s"),
            &remote_package_smoke_script(&release_line),
            "remote package smoke failed",
        )?;
        checks.push("remote_package_smoke".to_string());

        println!("[remote-host] ----------------------------------------");
        println!("[remote-host] running remote runtime smoke");
        run_ssh_script(
            &options.host,
            &format!("cd {remote_path} && bash -s"),
            REMOTE_RUNTIME_SMOKE_SCRIPT,
            "remote runtime smoke failed",
        )?;
        checks.push("remote_runtime_smoke".to_string());

        let summary = format!(
            "host={}\nremote_dir={}\nbuild_packages={}\nkeep_remote_dir={}\nchecks={}\n",
            options.host,
            remote_path,
            options.build_packages,
            options.keep_remote_dir,
            checks.join(",")
        );
        fs::write(out_dir.join("remote-run.txt"), summary)?;

        if options.keep_remote_dir {
            println!("[remote-host] keeping remote workspace: {}", remote_path);
        } else {
            println!("[remote-host] ----------------------------------------");
            println!("[remote-host] removing remote workspace");
            run_ssh_command(
                &options.host,
                &format!("rm -rf {remote_path}"),
                "failed to remove remote workspace",
            )?;
        }

        Ok(ValidationReport {
            name: format!("remote linux host validation ({})", options.host),
            out_dir,
            checks,
        })
    })();

    result.map_err(|err: ValidationError| {
        ValidationError::new(format!(
            "{err}\nremote workspace retained at {}:{}",
            options.host, remote_path
        ))
    })
}

fn default_remote_dir() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!(".kyuubiki-remote-runs/gewyvern-remote-{now}")
}

fn remote_workspace_path(remote_dir: &str) -> String {
    if remote_dir.starts_with('/') {
        remote_dir.to_string()
    } else {
        format!("~/{remote_dir}")
    }
}

fn sync_workspace(host: &str, remote_path: &str) -> Result<(), ValidationError> {
    let root = repo_root();
    let mut command = Command::new("rsync");
    command
        .arg("-az")
        .arg("--delete")
        .arg("--exclude")
        .arg(".git/")
        .arg("--exclude")
        .arg("target/")
        .arg("--exclude")
        .arg("node_modules/")
        .arg("--exclude")
        .arg("**/obj/")
        .arg("--exclude")
        .arg("apps/leserpent/src/Leserpent/data/control-plane-state.json")
        .arg(format!("{}/", root.display()))
        .arg(format!("{host}:{remote_path}/"))
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit());

    let status = command
        .status()
        .map_err(|err| ValidationError::new(format!("failed to launch rsync: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "rsync failed with status {status}"
        )))
    }
}

fn run_ssh_command(host: &str, command: &str, context: &str) -> Result<(), ValidationError> {
    let status = Command::new("ssh")
        .args(["-o", "BatchMode=yes", host, command])
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "{context}: remote command exited with status {status}"
        )))
    }
}

fn run_ssh_script(
    host: &str,
    command: &str,
    script: &str,
    context: &str,
) -> Result<(), ValidationError> {
    let mut child = Command::new("ssh")
        .args(["-o", "BatchMode=yes", host, command])
        .stdin(Stdio::piped())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;

    {
        let stdin = child
            .stdin
            .as_mut()
            .ok_or_else(|| ValidationError::new(format!("{context}: missing ssh stdin")))?;
        stdin
            .write_all(script.as_bytes())
            .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;
    }

    let status = child
        .wait()
        .map_err(|err| ValidationError::new(format!("{context}: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "{context}: remote script exited with status {status}"
        )))
    }
}

fn require_cmd(name: &str) -> Result<(), ValidationError> {
    let status = Command::new("sh")
        .arg("-lc")
        .arg(format!("command -v {name} >/dev/null 2>&1"))
        .status()
        .map_err(|err| ValidationError::new(format!("failed to probe command `{name}`: {err}")))?;
    if status.success() {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "required command not found: {name}"
        )))
    }
}

fn remote_package_smoke_script(release_line: &str) -> String {
    format!(
        r#"set -euo pipefail
DEB=$(find target/packages -maxdepth 1 -name 'gewyvern_*_amd64.deb' | sort | tail -n 1)
RPM=$(find target/packages/rpm -maxdepth 1 -name 'gewyvern-*.x86_64.rpm' | sort | tail -n 1)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
mkdir -p "$TMP/deb" "$TMP/rpm"
dpkg-deb -c "$DEB" > "$TMP/deb-contents.txt"
grep -q './usr/share/doc/gewyvern/LICENSE' "$TMP/deb-contents.txt"
dpkg-deb -x "$DEB" "$TMP/deb"
"$TMP/deb/usr/bin/gewyvern" --list-protocols >/dev/null
"$TMP/deb/usr/bin/gewyc" "$TMP/deb/usr/share/gewyvern/dsl/http_request_path.gewy" --json >/dev/null
test -d "$TMP/deb/usr/share/gewyvern/dsl"
test -d "$TMP/deb/usr/share/gewyvern/protocols"
test -f "$TMP/deb/usr/share/gewyvern/package-compat.toml"
grep -q '^schema_version = 1$' "$TMP/deb/usr/share/gewyvern/package-compat.toml"
grep -q '^release_line = "{release_line}"$' "$TMP/deb/usr/share/gewyvern/package-compat.toml"
test -f "$TMP/deb/usr/share/gewyvern/examples/gewyvern.toml.example"
rpm -qpl "$RPM" > "$TMP/rpm-contents.txt"
grep -q '/usr/share/doc/gewyvern/LICENSE' "$TMP/rpm-contents.txt"
rpm2cpio "$RPM" | (cd "$TMP/rpm" && cpio -idmu --quiet)
"$TMP/rpm/usr/bin/gewyvern" --list-protocols >/dev/null
"$TMP/rpm/usr/bin/gewyc" "$TMP/rpm/usr/share/gewyvern/dsl/http_request_path.gewy" --json >/dev/null
test -d "$TMP/rpm/usr/share/gewyvern/dsl"
test -d "$TMP/rpm/usr/share/gewyvern/protocols"
test -f "$TMP/rpm/usr/share/gewyvern/package-compat.toml"
grep -q '^schema_version = 1$' "$TMP/rpm/usr/share/gewyvern/package-compat.toml"
grep -q '^release_line = "{release_line}"$' "$TMP/rpm/usr/share/gewyvern/package-compat.toml"
test -f "$TMP/rpm/usr/share/gewyvern/examples/gewyvern.toml.example"
echo 'remote package smoke: ok'
"#
    )
}

const REMOTE_RUNTIME_SMOKE_SCRIPT: &str = r#"set -euo pipefail
DEB=$(find target/packages -maxdepth 1 -name 'gewyvern_*_amd64.deb' | sort | tail -n 1)
TMP=$(mktemp -d)
trap 'kill ${TCP_PID:-} ${UDP_PID:-} >/dev/null 2>&1 || true; wait ${TCP_PID:-} ${UDP_PID:-} >/dev/null 2>&1 || true; rm -rf "$TMP"' EXIT
mkdir -p "$TMP/deb"
dpkg-deb -x "$DEB" "$TMP/deb"
GEWY="$TMP/deb/usr/bin/gewyvern"
SEND="$TMP/deb/usr/bin/gewyvern_socket_send"
wait_http() {
  local url="$1" out="$2" frag="${3:-}"
  for _ in $(seq 1 120); do
    if curl -fsS "$url" >"$out" 2>/dev/null; then
      if [ -z "$frag" ] || grep -q "$frag" "$out"; then
        return 0
      fi
    fi
    sleep 0.1
  done
  echo "timed out waiting for $url" >&2
  return 1
}
TCP_SOCKET=127.0.0.1:29090
TCP_API=127.0.0.1:29190
UDP_SOCKET=127.0.0.1:29091
UDP_API=127.0.0.1:29191
"$GEWY" --tcp-socket "$TCP_SOCKET" --template tcp --serve --api-socket "$TCP_API" --json --summary-only >"$TMP/tcp.log" 2>&1 &
TCP_PID=$!
wait_http "http://$TCP_API/health" "$TMP/tcp-health.json"
"$SEND" --tcp-socket "$TCP_SOCKET" --template tcp >/dev/null
wait_http "http://$TCP_API/v1/latest/summary.json" "$TMP/tcp-summary.json" '"primary_module_kind":"connection_establishment"'
grep -q '"operator_guidance_action":"avoid_pid_strong_actions"' "$TMP/tcp-summary.json"
"$SEND" --tcp-socket "$TCP_SOCKET" --raw-line '{"broken":true' >/dev/null || true
wait_http "http://$TCP_API/health" "$TMP/tcp-health-after.json"
grep -q '"ok":true' "$TMP/tcp-health-after.json"
"$SEND" --tcp-socket "$TCP_SOCKET" --template tcp >/dev/null
wait_http "http://$TCP_API/v1/latest/analysis.json" "$TMP/tcp-analysis.json" '"protocol_flows"'
kill "$TCP_PID" >/dev/null 2>&1 || true
wait "$TCP_PID" >/dev/null 2>&1 || true
"$GEWY" --tcp-socket "$UDP_SOCKET" --template udp --serve --api-socket "$UDP_API" --json --summary-only >"$TMP/udp.log" 2>&1 &
UDP_PID=$!
wait_http "http://$UDP_API/health" "$TMP/udp-health.json"
"$SEND" --tcp-socket "$UDP_SOCKET" --template udp >/dev/null
wait_http "http://$UDP_API/v1/latest/summary.json" "$TMP/udp-summary.json" '"primary_module_kind":"datagram_exchange"'
wait_http "http://$UDP_API/v1/latest/analysis.json" "$TMP/udp-analysis.json" '"primary_failure_mode":"none"'
echo 'remote runtime smoke: ok'
"#;
