use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};
use std::{env, fs};

use serde_json::Value;

use super::command::{
    ValidationError, ValidationReport, default_out_dir, repo_root, run_cargo_json,
    run_cargo_status, value_at,
};

pub fn run_field_smoke_validation(
    out_dir: Option<PathBuf>,
    include_socket: bool,
    include_scan_all: bool,
) -> Result<ValidationReport, ValidationError> {
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("field-validation-smoke"));
    if out_dir.exists() {
        fs::remove_dir_all(&out_dir)?;
    }
    fs::create_dir_all(&out_dir)?;

    let mut checks = Vec::new();

    check_demo_summary(&out_dir)?;
    checks.push("standalone_demo_summary".to_string());

    check_dsl_summary(&out_dir)?;
    checks.push("standalone_dsl_summary".to_string());

    check_gewyc_explain(&out_dir)?;
    checks.push("gewyc_explain_surface".to_string());

    if include_socket {
        check_socket_roundtrip(&out_dir)?;
        checks.push("socket_roundtrip".to_string());
    }

    if include_scan_all {
        check_scan_all(&out_dir)?;
        checks.push("registry_wide_scan".to_string());
    }

    write_readme(&out_dir, &checks)?;

    Ok(ValidationReport {
        name: "field validation smoke".to_string(),
        out_dir,
        checks,
    })
}

fn check_demo_summary(out_dir: &Path) -> Result<(), ValidationError> {
    let json = run_gewyvern_json(
        &["--demo", "udp", "--json", "--summary-only"],
        &out_dir.join("demo-summary.json"),
    )?;
    require_field(&json, &["primary_failure_mode"])?;
    require_field(&json, &["operator_guidance_action"])
}

fn check_dsl_summary(out_dir: &Path) -> Result<(), ValidationError> {
    let dsl = repo_root().join("dsl/http_request_path.gewy");
    let json = run_gewyvern_json(
        &[
            "--dsl",
            dsl.to_str().unwrap_or_default(),
            "--json",
            "--summary-only",
        ],
        &out_dir.join("dsl-summary.json"),
    )?;
    require_field(&json, &["primary_module_kind"])?;
    require_field(&json, &["process_network_profiles"])
}

fn check_gewyc_explain(out_dir: &Path) -> Result<(), ValidationError> {
    let dsl = repo_root().join("dsl/http_request_path.gewy");
    let args = vec![
        "run".to_string(),
        "--quiet".to_string(),
        "-p".to_string(),
        "gewyc".to_string(),
        "--".to_string(),
        "explain".to_string(),
        dsl.display().to_string(),
        "--json".to_string(),
    ];
    let json = run_cargo_json(&args, &out_dir.join("explain.json"))?;
    require_string(&json, &["surface_id"], "gewyc.explain")?;
    require_field(&json, &["payload", "summary"])?;
    require_field(&json, &["payload", "summary", "next_step"])
}

fn check_socket_roundtrip(out_dir: &Path) -> Result<(), ValidationError> {
    build_socket_binaries(out_dir)?;
    let socket_path = default_field_socket_path();
    let output_path = out_dir.join("socket.json");
    let stdout = fs::File::create(out_dir.join("socket-server.stdout.log"))?;
    let stderr = fs::File::create(out_dir.join("socket-server.stderr.log"))?;
    let _ = fs::remove_file(&socket_path);

    let mut child = Command::new(repo_root().join("target/debug/gewyvern"))
        .current_dir(repo_root())
        .arg("--unix-socket")
        .arg(&socket_path)
        .arg("--template")
        .arg("udp")
        .arg("--json")
        .arg("--out")
        .arg(&output_path)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|err| ValidationError::new(format!("failed to start socket smoke: {err}")))?;

    let result = run_socket_client(&socket_path).and_then(|_| {
        wait_for_child(&mut child)?;
        let body = fs::read_to_string(&output_path)?;
        if !body.contains("\"template_id\"") || !body.contains("\"facts\"") {
            return Err(ValidationError::new(
                "socket roundtrip output missed expected fields",
            ));
        }
        Ok(())
    });

    if result.is_err() {
        kill_child(&mut child);
    }
    let _ = fs::remove_file(&socket_path);
    result
}

fn default_field_socket_path() -> String {
    env::var("GEWY_FIELD_SOCKET_PATH")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| {
            env::temp_dir()
                .join(format!(
                    "gewyvern-field-validation-{}.sock",
                    std::process::id()
                ))
                .to_string_lossy()
                .into_owned()
        })
}

fn run_socket_client(socket_path: &str) -> Result<(), ValidationError> {
    wait_for_socket(socket_path)?;
    let output = Command::new(repo_root().join("target/debug/gewyvern_socket_send"))
        .current_dir(repo_root())
        .arg("--socket")
        .arg(socket_path)
        .arg("--template")
        .arg("udp")
        .output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(ValidationError::new(format!(
        "socket sender failed: {}",
        String::from_utf8_lossy(&output.stderr)
    )))
}

fn check_scan_all(out_dir: &Path) -> Result<(), ValidationError> {
    let json = run_gewyvern_json(
        &["--scan-all", "--json", "--summary-only"],
        &out_dir.join("scan.json"),
    )?;
    require_string(&json, &["kind"], "scan")?;
    require_field(&json, &["target_count"])
}

fn build_socket_binaries(out_dir: &Path) -> Result<(), ValidationError> {
    let args = vec![
        "build".to_string(),
        "--quiet".to_string(),
        "--bin".to_string(),
        "gewyvern".to_string(),
        "--bin".to_string(),
        "gewyvern_socket_send".to_string(),
    ];
    run_cargo_status(&args, &out_dir.join("cargo-build.log"))
}

fn run_gewyvern_json(args: &[&str], output_path: &Path) -> Result<Value, ValidationError> {
    let mut cargo_args = vec!["run".to_string(), "--quiet".to_string(), "--".to_string()];
    cargo_args.extend(args.iter().map(|arg| (*arg).to_string()));
    run_cargo_json(&cargo_args, output_path)
}

fn require_field(value: &Value, path: &[&str]) -> Result<(), ValidationError> {
    value_at(value, path).map(|_| ())
}

fn require_string(value: &Value, path: &[&str], expected: &str) -> Result<(), ValidationError> {
    let actual = value_at(value, path)?.as_str().ok_or_else(|| {
        ValidationError::new(format!("expected string field `{}`", path.join(".")))
    })?;
    if actual == expected {
        Ok(())
    } else {
        Err(ValidationError::new(format!(
            "expected `{}` to be `{expected}`, got `{actual}`",
            path.join(".")
        )))
    }
}

fn wait_for_socket(path: &str) -> Result<(), ValidationError> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if Path::new(path).exists() {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(ValidationError::new(format!(
        "socket did not appear: {path}"
    )))
}

fn wait_for_child(child: &mut Child) -> Result<(), ValidationError> {
    let deadline = Instant::now() + Duration::from_secs(8);
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            return Err(ValidationError::new(format!(
                "socket smoke exited with {status}"
            )));
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(ValidationError::new("socket smoke did not exit in time"))
}

fn kill_child(child: &mut Child) {
    if matches!(child.try_wait(), Ok(None)) {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn write_readme(out_dir: &Path, checks: &[String]) -> Result<(), ValidationError> {
    fs::write(
        out_dir.join("README.txt"),
        format!(
            "gewyvern native field validation smoke\n\
             =====================================\n\n\
             checks={}\n\n\
             Optional socket and scan-all checks can be enabled with --socket and --scan-all.\n",
            checks.len()
        ),
    )?;
    Ok(())
}
