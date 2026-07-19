use std::env;
use std::fs;
use std::io::{ErrorKind, Read, Write};
use std::net::{Shutdown, TcpStream};
#[cfg(unix)]
use std::os::unix::fs::FileTypeExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;

use super::command::{
    ValidationError, ValidationReport, assert_eq_str, default_out_dir, repo_root, run_cargo_status,
    value_at,
};

pub fn run_socket_roundtrip_demo(
    socket_target: Option<&str>,
    template: Option<&str>,
    output_path: Option<PathBuf>,
    socket_kind: Option<&str>,
) -> Result<ValidationReport, ValidationError> {
    let socket_kind = socket_kind.unwrap_or("unix");
    let template = template.unwrap_or("udp");
    let out_dir = default_out_dir("socket-roundtrip-demo");
    prepare_dir(&out_dir)?;
    build_socket_binaries(&out_dir)?;

    let default_socket = format!("/tmp/gewyvern-demo-{}.sock", std::process::id());
    let socket_target = socket_target.unwrap_or(&default_socket);
    let output_path = output_path.unwrap_or_else(|| out_dir.join("socket-output.json"));
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent)?;
    }
    if socket_kind == "unix" {
        remove_stale_unix_socket(socket_target)?;
    }
    let _ = fs::remove_file(&output_path);

    let mut server =
        start_socket_server(socket_kind, socket_target, template, &output_path, &out_dir)?;
    let result = run_socket_sender(socket_kind, socket_target, template).and_then(|_| {
        wait_for_child(&mut server, Duration::from_secs(12))?;
        let body = fs::read_to_string(&output_path)?;
        if !body.contains("\"template_id\"") || !body.contains("\"facts\"") {
            return Err(ValidationError::new(
                "socket roundtrip output missed expected fields",
            ));
        }
        Ok(())
    });

    if result.is_err() {
        kill_child(&mut server);
    }
    if socket_kind == "unix" {
        remove_stale_unix_socket(socket_target)?;
    }
    result?;

    Ok(ValidationReport {
        name: "socket roundtrip demo".to_string(),
        out_dir,
        checks: vec![
            format!("{socket_kind}_socket_session_completed"),
            "socket_output_contains_template_and_facts".to_string(),
        ],
    })
}

pub fn run_training_dataset_roundtrip_demo(
    api_addr: Option<&str>,
    out_dir: Option<PathBuf>,
    target_path_segment: Option<&str>,
    limit: Option<usize>,
) -> Result<ValidationReport, ValidationError> {
    let api_addr = api_addr.unwrap_or("127.0.0.1:9910");
    let out_dir = out_dir.unwrap_or_else(|| default_out_dir("training-dataset-roundtrip-demo"));
    prepare_dir(&out_dir)?;
    let manifest_route = target_path_segment
        .filter(|segment| !segment.is_empty())
        .map(|segment| format!("/v1/latest/targets/{segment}/training-dataset.json"))
        .unwrap_or_else(|| "/v1/latest/training-dataset.json".to_string());

    let manifest = wait_for_json(
        api_addr,
        &manifest_route,
        &out_dir.join("training-dataset.json"),
        "\"kind\":\"training_dataset_manifest\"",
    )?;
    assert_eq_str(&manifest, &["kind"], "training_dataset_manifest")?;
    assert_eq_str(
        &manifest,
        &["split_policies", "default"],
        "name_bucket_mod_10",
    )?;

    let Some(samples) = value_at(&manifest, &["samples"])?.as_array() else {
        return Err(ValidationError::new(
            "training manifest samples is not an array",
        ));
    };
    if samples.is_empty() {
        return Err(ValidationError::new("training manifest has no samples"));
    }
    let limit = limit.unwrap_or(0);
    let sample_limit = if limit == 0 {
        samples.len()
    } else {
        limit.min(samples.len())
    };
    for (index, sample) in samples.iter().take(sample_limit).enumerate() {
        let sample_id = value_at(sample, &["sample_id"])?
            .as_str()
            .ok_or_else(|| ValidationError::new("training sample id is not a string"))?;
        let sample_path = value_at(sample, &["sample_path"])?
            .as_str()
            .ok_or_else(|| ValidationError::new("training sample path is not a string"))?;
        let payload = wait_for_json(
            api_addr,
            sample_path,
            &out_dir.join(format!("sample-{index:03}.json")),
            "\"kind\":\"training_example\"",
        )?;
        assert_eq_str(&payload, &["sample_id"], sample_id)?;
    }

    write_training_summary(&out_dir, sample_limit)?;

    Ok(ValidationReport {
        name: "training dataset roundtrip demo".to_string(),
        out_dir,
        checks: vec![
            "training_manifest_loaded".to_string(),
            "sample_ids_verified".to_string(),
        ],
    })
}

// Keep the validation entrypoint source-compatible with the native CLI surface.
#[allow(clippy::too_many_arguments)]
pub fn run_external_engine_roundtrip_demo(
    ingest_addr: Option<&str>,
    api_addr: Option<&str>,
    template: Option<&str>,
    analysis_out: Option<PathBuf>,
    engine_out: Option<PathBuf>,
    target_path_segment: Option<&str>,
    engine_root: Option<PathBuf>,
    engine_cmd: Option<&str>,
) -> Result<ValidationReport, ValidationError> {
    let ingest_addr = ingest_addr.unwrap_or("127.0.0.1:9900");
    let api_addr = api_addr.unwrap_or("127.0.0.1:9910");
    let template = template.unwrap_or("udp");
    let out_dir = default_out_dir("external-engine-roundtrip-demo");
    prepare_dir(&out_dir)?;
    build_socket_binaries(&out_dir)?;

    let analysis_out = analysis_out.unwrap_or_else(|| out_dir.join("gewyvern-analysis.json"));
    let engine_out = engine_out.unwrap_or_else(|| out_dir.join("external-engine-output.json"));
    ensure_parent(&analysis_out)?;
    ensure_parent(&engine_out)?;
    let _ = fs::remove_file(&analysis_out);
    let _ = fs::remove_file(&engine_out);

    let analysis_route = target_path_segment
        .filter(|segment| !segment.is_empty())
        .map(|segment| format!("/v1/latest/targets/{segment}/analysis.json"))
        .unwrap_or_else(|| "/v1/latest/analysis.json".to_string());

    let mut server = start_external_engine_server(ingest_addr, api_addr, &out_dir)?;
    let result = wait_for_http_fragment(api_addr, "/health", "ok")
        .and_then(|_| run_socket_sender("tcp", ingest_addr, template))
        .and_then(|_| {
            wait_for_json(
                api_addr,
                &analysis_route,
                &analysis_out,
                "\"operator_guidance_action\"",
            )
        })
        .and_then(|_| {
            run_external_engine(
                api_addr,
                &analysis_route,
                engine_root,
                engine_cmd,
                &engine_out,
                &out_dir,
            )
        });

    kill_child(&mut server);
    result?;

    Ok(ValidationReport {
        name: "external engine roundtrip demo".to_string(),
        out_dir,
        checks: vec![
            "gewyvern_analysis_published".to_string(),
            "external_engine_analyze_url_completed".to_string(),
        ],
    })
}

fn prepare_dir(out_dir: &Path) -> Result<(), ValidationError> {
    if out_dir.exists() {
        fs::remove_dir_all(out_dir)?;
    }
    fs::create_dir_all(out_dir)?;
    Ok(())
}

fn ensure_parent(path: &Path) -> Result<(), ValidationError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

fn remove_stale_unix_socket(socket_target: &str) -> Result<(), ValidationError> {
    let path = Path::new(socket_target);
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == ErrorKind::NotFound => return Ok(()),
        Err(err) => return Err(err.into()),
    };

    if !is_socket_metadata(&metadata) {
        return Err(ValidationError::new(format!(
            "refusing to remove non-socket path '{}'",
            path.display()
        )));
    }
    fs::remove_file(path)?;
    Ok(())
}

#[cfg(unix)]
fn is_socket_metadata(metadata: &fs::Metadata) -> bool {
    metadata.file_type().is_socket()
}

#[cfg(not(unix))]
fn is_socket_metadata(_metadata: &fs::Metadata) -> bool {
    false
}

fn build_socket_binaries(out_dir: &Path) -> Result<(), ValidationError> {
    run_cargo_status(
        &[
            "build".to_string(),
            "--quiet".to_string(),
            "--bin".to_string(),
            "gewyvern".to_string(),
            "--bin".to_string(),
            "gewyvern_socket_send".to_string(),
        ],
        &out_dir.join("cargo-build.log"),
    )
}

fn start_external_engine_server(
    ingest_addr: &str,
    api_addr: &str,
    out_dir: &Path,
) -> Result<Child, ValidationError> {
    let stdout = fs::File::create(out_dir.join("external-engine-server.stdout.log"))?;
    let stderr = fs::File::create(out_dir.join("external-engine-server.stderr.log"))?;
    Command::new(repo_root().join("target/debug/gewyvern"))
        .current_dir(repo_root())
        .arg("--tcp-socket")
        .arg(ingest_addr)
        .arg("--ingest-mode")
        .arg("local-advisory")
        .arg("--serve")
        .arg("--api-socket")
        .arg(api_addr)
        .arg("--json")
        .arg("--summary-only")
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|err| ValidationError::new(format!("failed to start gewyvern server: {err}")))
}

fn start_socket_server(
    socket_kind: &str,
    socket_target: &str,
    template: &str,
    output_path: &Path,
    out_dir: &Path,
) -> Result<Child, ValidationError> {
    let stdout = fs::File::create(out_dir.join("socket-server.stdout.log"))?;
    let stderr = fs::File::create(out_dir.join("socket-server.stderr.log"))?;
    let mut command = Command::new(repo_root().join("target/debug/gewyvern"));
    command.current_dir(repo_root());
    match socket_kind {
        "tcp" => {
            command.arg("--tcp-socket").arg(socket_target);
            thread::sleep(Duration::from_millis(50));
        }
        "unix" => {
            command.arg("--unix-socket").arg(socket_target);
        }
        other => {
            return Err(ValidationError::new(format!(
                "unsupported socket kind: {other}"
            )));
        }
    }
    command
        .arg("--template")
        .arg(template)
        .arg("--json")
        .arg("--out")
        .arg(output_path)
        .stdout(Stdio::from(stdout))
        .stderr(Stdio::from(stderr))
        .spawn()
        .map_err(|err| ValidationError::new(format!("failed to start socket demo: {err}")))
}

fn run_external_engine(
    api_addr: &str,
    analysis_route: &str,
    engine_root: Option<PathBuf>,
    engine_cmd: Option<&str>,
    engine_out: &Path,
    out_dir: &Path,
) -> Result<(), ValidationError> {
    let engine_root = resolve_engine_root(engine_root)?;
    let url = format!("http://{api_addr}{analysis_route}");
    let custom_cmd = engine_cmd
        .map(str::to_string)
        .or_else(|| env::var("EXTERNAL_ENGINE_CMD").ok());
    let output = if let Some(custom_cmd) = custom_cmd {
        let custom_cmd = validate_external_engine_command(&custom_cmd)?;
        Command::new(custom_cmd)
            .current_dir(&engine_root)
            .arg(&url)
            .output()
    } else {
        Command::new(env::var("CARGO").unwrap_or_else(|_| "cargo".to_string()))
            .current_dir(&engine_root)
            .arg("run")
            .arg("--quiet")
            .arg("--")
            .arg("analyze-url")
            .arg(&url)
            .output()
    }
    .map_err(|err| ValidationError::new(format!("failed to run external engine: {err}")))?;

    fs::write(engine_out, &output.stdout)?;
    fs::write(out_dir.join("external-engine.stderr.log"), &output.stderr)?;
    if !output.status.success() {
        return Err(ValidationError::new(format!(
            "external engine failed with {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    if output.stdout.is_empty()
        || !String::from_utf8_lossy(&output.stdout).contains("augmentations")
    {
        return Err(ValidationError::new(
            "external engine output did not contain augmentation evidence",
        ));
    }
    Ok(())
}

fn validate_external_engine_command(command: &str) -> Result<&str, ValidationError> {
    let trimmed = command.trim();
    if trimmed.is_empty() {
        return Err(ValidationError::new(
            "external engine command must not be empty",
        ));
    }
    if trimmed != command {
        return Err(ValidationError::new(
            "external engine command must be a single executable path without surrounding spaces",
        ));
    }
    if command.chars().any(|ch| {
        ch.is_whitespace() || matches!(ch, ';' | '&' | '|' | '`' | '$' | '<' | '>' | '(' | ')')
    }) {
        return Err(ValidationError::new(
            "--engine-cmd/EXTERNAL_ENGINE_CMD now accepts only one executable path; the analysis URL is passed as argv[1]",
        ));
    }
    Ok(command)
}

fn resolve_engine_root(engine_root: Option<PathBuf>) -> Result<PathBuf, ValidationError> {
    if let Some(root) = engine_root {
        return Ok(root);
    }
    if let Ok(root) = env::var("ENGINE_ROOT") {
        return Ok(PathBuf::from(root));
    }
    if let Ok(root) = env::var("ETRAGON_ROOT") {
        return Ok(PathBuf::from(root));
    }
    let monorepo_root = repo_root().join("apps/etragon");
    if monorepo_root.exists() {
        return Ok(monorepo_root);
    }
    Err(ValidationError::new(
        "external engine root is not set and apps/etragon was not found",
    ))
}

fn run_socket_sender(
    socket_kind: &str,
    socket_target: &str,
    template: &str,
) -> Result<(), ValidationError> {
    if socket_kind == "unix" {
        wait_for_unix_socket(socket_target)?;
    } else {
        thread::sleep(Duration::from_millis(150));
    }
    let mut command = Command::new(repo_root().join("target/debug/gewyvern_socket_send"));
    command.current_dir(repo_root());
    if socket_kind == "tcp" {
        command.arg("--tcp-socket").arg(socket_target);
    } else {
        command.arg("--socket").arg(socket_target);
    }
    let output = command.arg("--template").arg(template).output()?;
    if output.status.success() {
        return Ok(());
    }
    Err(ValidationError::new(format!(
        "socket sender failed: {}",
        String::from_utf8_lossy(&output.stderr)
    )))
}

fn wait_for_http_fragment(
    addr: &str,
    path: &str,
    fragment: &str,
) -> Result<String, ValidationError> {
    let deadline = Instant::now() + Duration::from_secs(16);
    while Instant::now() < deadline {
        if let Ok(body) = http_get(addr, path)
            && body.contains(fragment)
        {
            return Ok(body);
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(ValidationError::new(format!(
        "timed out waiting for {fragment} at http://{addr}{path}"
    )))
}

fn wait_for_unix_socket(path: &str) -> Result<(), ValidationError> {
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

fn wait_for_child(child: &mut Child, timeout: Duration) -> Result<(), ValidationError> {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if let Some(status) = child.try_wait()? {
            if status.success() {
                return Ok(());
            }
            return Err(ValidationError::new(format!(
                "socket demo exited with {status}"
            )));
        }
        thread::sleep(Duration::from_millis(50));
    }
    Err(ValidationError::new("socket demo did not exit in time"))
}

fn kill_child(child: &mut Child) {
    if matches!(child.try_wait(), Ok(None)) {
        let _ = child.kill();
        let _ = child.wait();
    }
}

fn wait_for_json(
    addr: &str,
    path: &str,
    output_path: &Path,
    fragment: &str,
) -> Result<Value, ValidationError> {
    let deadline = Instant::now() + Duration::from_secs(16);
    while Instant::now() < deadline {
        if let Ok(body) = http_get(addr, path)
            && body.contains(fragment)
        {
            let payload = body
                .split_once("\r\n\r\n")
                .map(|(_, payload)| payload)
                .unwrap_or(body.as_str());
            fs::write(output_path, payload)?;
            return serde_json::from_str(payload).map_err(ValidationError::from);
        }
        thread::sleep(Duration::from_millis(100));
    }
    Err(ValidationError::new(format!(
        "timed out waiting for {fragment} at http://{addr}{path}"
    )))
}

fn http_get(addr: &str, path: &str) -> Result<String, ValidationError> {
    let mut stream = TcpStream::connect(addr)?;
    stream.set_read_timeout(Some(Duration::from_secs(2)))?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {addr}\r\nConnection: close\r\n\r\n"
    )?;
    stream.shutdown(Shutdown::Write).ok();

    let mut response = String::new();
    stream.read_to_string(&mut response)?;
    if !response.starts_with("HTTP/1.1 200") && !response.starts_with("HTTP/1.0 200") {
        return Err(ValidationError::new("HTTP endpoint did not return 200"));
    }
    Ok(response)
}

fn write_training_summary(out_dir: &Path, checked: usize) -> Result<(), ValidationError> {
    fs::write(
        out_dir.join("roundtrip-summary.json"),
        format!(
            "{{\n  \"sample_count_checked\": {checked},\n  \"default_split_policy\": \"name_bucket_mod_10\",\n  \"sample_ids_verified\": true,\n  \"output_dir\": \"{}\"\n}}\n",
            out_dir.display()
        ),
    )?;
    Ok(())
}
