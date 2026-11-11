use serde_json::{Value, json};
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicU64, Ordering},
};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

struct HttpFixture {
    address: SocketAddr,
    response: Arc<Mutex<(u16, String)>>,
    requests: Arc<Mutex<Vec<String>>>,
    routes: Arc<Mutex<std::collections::BTreeMap<String, (u16, String)>>>,
    stop: Arc<AtomicBool>,
    worker: Option<JoinHandle<()>>,
}

impl HttpFixture {
    fn new(body: String) -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        listener.set_nonblocking(true).unwrap();
        let response = Arc::new(Mutex::new((200, body)));
        let requests = Arc::new(Mutex::new(Vec::new()));
        let routes = Arc::new(Mutex::new(std::collections::BTreeMap::<
            String,
            (u16, String),
        >::new()));
        let routed = Arc::clone(&routes);
        let stop = Arc::new(AtomicBool::new(false));
        let (reply, captured, stopping) = (
            Arc::clone(&response),
            Arc::clone(&requests),
            Arc::clone(&stop),
        );
        let worker = thread::spawn(move || {
            while !stopping.load(Ordering::Relaxed) {
                let mut stream = match listener.accept() {
                    Ok((stream, _)) => stream,
                    Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(5));
                        continue;
                    }
                    Err(error) => panic!("fixture accept: {error}"),
                };
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                stream
                    .set_write_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = Vec::new();
                while !bytes.ends_with(b"\r\n\r\n") {
                    assert!(bytes.len() < 32 * 1024);
                    let mut byte = [0];
                    stream.read_exact(&mut byte).unwrap();
                    bytes.push(byte[0]);
                }
                let header = String::from_utf8(bytes.clone()).unwrap();
                let path = header.split_whitespace().nth(1).unwrap().to_owned();
                let length = header
                    .lines()
                    .find_map(|line| line.strip_prefix("Content-Length: "))
                    .map(|text| text.parse::<usize>().unwrap())
                    .unwrap_or(0);
                assert!(length < 256 * 1024);
                let mut body = vec![0; length];
                stream.read_exact(&mut body).unwrap();
                bytes.extend(body);
                captured
                    .lock()
                    .unwrap()
                    .push(String::from_utf8(bytes).unwrap());
                let (status, body) = routed
                    .lock()
                    .unwrap()
                    .get(&path)
                    .cloned()
                    .unwrap_or_else(|| reply.lock().unwrap().clone());
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status} Fixture\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        Self {
            address,
            response,
            requests,
            routes,
            stop,
            worker: Some(worker),
        }
    }

    fn count(&self) -> usize {
        self.requests.lock().unwrap().len()
    }
}

impl Drop for HttpFixture {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

struct TempConfig(PathBuf);
impl TempConfig {
    fn new(content: &str) -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "etragon-resident-{}-{}.json",
            std::process::id(),
            COUNTER.fetch_add(1, Ordering::Relaxed)
        ));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .unwrap();
        file.write_all(content.as_bytes()).unwrap();
        Self(path)
    }
}
impl Drop for TempConfig {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

fn start(source: &HttpFixture, config: &TempConfig) -> (SocketAddr, Process) {
    start_mode(source, config, "serve-url", &[])
}

fn start_mode(
    source: &HttpFixture,
    config: &TempConfig,
    mode: &str,
    extra: &[&str],
) -> (SocketAddr, Process) {
    let address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let child = Command::new(env!("CARGO_BIN_EXE_etragon"))
        .args([
            mode,
            &format!(
                "http://{}/v1/latest/{}",
                source.address,
                if mode == "serve-targets-url" {
                    "targets"
                } else {
                    "analysis.json"
                }
            ),
            "--provider",
            config.0.to_str().unwrap(),
            "--bind",
            &address.to_string(),
            "--interval-ms",
            "100",
        ])
        .args(extra)
        .env("ETRAGON_SOURCE_ADMIN_TOKEN", "source-test-secret")
        .env("ETRAGON_TEST_PROVIDER_TOKEN", "model-test-secret")
        .env_remove("ETRAGON_ADMIN_TOKEN")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    (address, Process(child))
}

fn get(address: SocketAddr, path: &str) -> Option<(u16, Value)> {
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_millis(200)).ok()?;
    stream.set_read_timeout(Some(Duration::from_secs(2))).ok()?;
    write!(stream, "GET {path} HTTP/1.1\r\nHost: localhost\r\n\r\n").ok()?;
    let mut wire = String::new();
    stream.read_to_string(&mut wire).ok()?;
    let (header, body) = wire.split_once("\r\n\r\n")?;
    Some((
        header.split_whitespace().nth(1)?.parse().ok()?,
        serde_json::from_str(body).ok()?,
    ))
}

fn wait_for(mut test: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(8);
    while !test() {
        assert!(
            Instant::now() < deadline,
            "resident did not reach expected state"
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn wait_for_output(address: SocketAddr, matches: impl Fn(u16, &Value) -> bool) -> (u16, Value) {
    wait_for_path(address, "/v1/latest/output.json", matches)
}

fn wait_for_path(
    address: SocketAddr,
    path: &str,
    matches: impl Fn(u16, &Value) -> bool,
) -> (u16, Value) {
    let mut accepted = None;
    wait_for(|| {
        if let Some((code, body)) = get(address, path)
            && matches(code, &body)
        {
            accepted = Some((code, body));
            return true;
        }
        false
    });
    accepted.unwrap()
}

fn advice() -> String {
    json!({"choices":[{"finish_reason":"stop","message":{"role":"assistant","content":json!({
        "schema_version":"etragon-advisory-v1","findings":[{"summary":"A candidate only.","evidence_refs":["primary_failure_mode"],"recommendation":"Review another capture."}]
    }).to_string()}}]}).to_string()
}

fn one_shot(
    source: &HttpFixture,
    config: &TempConfig,
    mode: &str,
    extra: &[&str],
) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_etragon"))
        .args([
            mode,
            &format!(
                "http://{}/v1/latest/{}",
                source.address,
                if mode == "analyze-targets-url" {
                    "targets"
                } else {
                    "analysis.json"
                }
            ),
            "--provider",
            config.0.to_str().unwrap(),
        ])
        .args(extra)
        .env("ETRAGON_SOURCE_ADMIN_TOKEN", "source-test-secret")
        .env("ETRAGON_TEST_PROVIDER_TOKEN", "model-test-secret")
        .output()
        .unwrap()
}

#[test]
fn one_shot_url_uses_separate_credentials_and_never_masks_source_or_provider_failures() {
    let mut input: Value =
        serde_json::from_str(include_str!("fixtures/missing_transition_analysis.json")).unwrap();
    input["raw_packet"] = json!("PRIVATE_CAPTURE_PAYLOAD");
    let source = HttpFixture::new(input.to_string());
    let model = HttpFixture::new(advice());
    let config = TempConfig::new(
        &json!({"id":"one-shot","model":"fixture",
        "endpoint":format!("http://{}/v1/chat/completions",model.address),
        "api_key_env":"ETRAGON_TEST_PROVIDER_TOKEN"})
        .to_string(),
    );
    let result = one_shot(&source, &config, "analyze-url", &[]);
    assert!(result.status.success());
    assert!(result.stderr.is_empty());
    let body: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(
        body["augmentations"][0]["data"]["operator_review_required"],
        true
    );
    assert_eq!(model.count(), 1);
    let source_request = source.requests.lock().unwrap()[0].clone();
    assert!(source_request.contains("X-Gewyvern-Admin-Token: source-test-secret\r\n"));
    assert!(!source_request.contains("model-test-secret"));
    let model_request = model.requests.lock().unwrap()[0].clone();
    assert!(model_request.contains("Authorization: Bearer model-test-secret\r\n"));
    assert!(!model_request.contains("source-test-secret"));
    assert!(!model_request.contains("PRIVATE_CAPTURE_PAYLOAD"));

    *model.response.lock().unwrap() = (429, "PRIVATE_PROVIDER_ERROR".into());
    let result = one_shot(&source, &config, "analyze-url", &[]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("etragon_provider_http_status: 429")
    );
    *source.response.lock().unwrap() = (503, "PRIVATE_SOURCE_ERROR".into());
    let result = one_shot(&source, &config, "analyze-url", &[]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    let error = String::from_utf8(result.stderr).unwrap();
    assert!(error.contains("etragon_source_http_status: 503"));
    assert!(!error.contains("PRIVATE_"));
    assert_eq!(model.count(), 2);
}

#[test]
fn one_shot_batch_reports_partial_exit_and_never_follows_advertised_urls() {
    let trap = HttpFixture::new("not an authorized destination".into());
    let bad = "scan:a?%2F# space";
    let refs = [bad, "scan:b", "scan:c", "unselected"]
        .iter()
        .map(|name| json!({"path_segment":name,"url_path":format!("http://{}/steal",trap.address)}))
        .collect::<Vec<_>>();
    let index = json!({"target_refs":refs});
    let source = HttpFixture::new(index.to_string());
    let model = HttpFixture::new(advice());
    let raw = include_str!("fixtures/missing_transition_analysis.json").to_string();
    let encoded_path = "/v1/latest/targets/scan:a%3F%252F%23%20space/analysis.json";
    source.routes.lock().unwrap().extend([
        (encoded_path.into(), (503, "PRIVATE_UPSTREAM_BODY".into())),
        (
            "/v1/latest/targets/scan:b/analysis.json".into(),
            (200, raw.clone()),
        ),
        (
            "/v1/latest/targets/scan:c/analysis.json".into(),
            (200, raw.clone()),
        ),
    ]);
    let config = TempConfig::new(
        &json!({"id":"batch","model":"fixture",
        "endpoint":format!("http://{}/v1/chat/completions",model.address),
        "api_key_env":"ETRAGON_TEST_PROVIDER_TOKEN"})
        .to_string(),
    );
    let result = one_shot(
        &source,
        &config,
        "analyze-targets-url",
        &["--filter", "scan:", "--max-targets", "3"],
    );
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stderr.is_empty());
    let body: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(body["schema_version"], "etragon-provider-batch-v1");
    assert_eq!(body["status"], "partial");
    assert_eq!(body["complete"], false);
    assert_eq!(body["failed_count"], 1);
    assert_eq!(body["ready_count"], 1);
    assert_eq!(body["skipped_count"], 1);
    assert_eq!(body["analysis_runs"], 1);
    assert_eq!(body["targets"][0]["error"]["http_status"], 503);
    assert_eq!(
        body["targets"][1]["output"]["augmentations"][0]["data"]["target_path_segment"],
        "scan:b"
    );
    assert_eq!(
        body["targets"][2]["error"]["code"],
        "etragon_provider_inference_budget_exhausted"
    );
    assert!(!body.to_string().contains("PRIVATE_"));
    assert!(
        !source
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|request| request.starts_with("GET /v1/latest/targets/scan:c/"))
    );
    assert_eq!(model.count(), 1);

    source
        .routes
        .lock()
        .unwrap()
        .insert(encoded_path.into(), (200, raw));
    let result = one_shot(
        &source,
        &config,
        "analyze-targets-url",
        &["--filter", "scan:", "--max-inferences", "3"],
    );
    assert!(result.status.success());
    let body: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(body["complete"], true);
    assert_eq!(body["ready_count"], 3);
    assert_eq!(body["analysis_runs"], 3);
    assert_eq!(
        model.count(),
        4,
        "fresh invocations and distinct targets do not share advice"
    );

    *model.response.lock().unwrap() = (429, "PRIVATE_PROVIDER_BODY".into());
    let result = one_shot(
        &source,
        &config,
        "analyze-targets-url",
        &["--filter", "scan:", "--max-inferences", "2"],
    );
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stderr.is_empty());
    let body: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(body["status"], "unavailable");
    assert_eq!(body["failed_count"], 2);
    assert_eq!(body["skipped_count"], 1);
    assert_eq!(body["analysis_runs"], 2);
    assert_eq!(body["targets"][0]["error"]["stage"], "provider");
    assert_eq!(body["targets"][0]["error"]["http_status"], 429);
    assert!(!body.to_string().contains("PRIVATE_"));
    assert_eq!(
        model.count(),
        6,
        "failed calls are not retried or replaced by legacy advice"
    );

    source.response.lock().unwrap().1 =
        json!({"target_refs":[{"path_segment":"a"},{"path_segment":"a"}]}).to_string();
    let result = one_shot(&source, &config, "analyze-targets-url", &[]);
    assert_eq!(result.status.code(), Some(2));
    assert!(result.stdout.is_empty());
    assert!(
        String::from_utf8(result.stderr)
            .unwrap()
            .contains("etragon_provider_batch_rejected")
    );
    assert_eq!(model.count(), 6);
    source.response.lock().unwrap().1 = json!({"target_refs":[]}).to_string();
    let result = one_shot(&source, &config, "analyze-targets-url", &[]);
    assert!(result.status.success());
    let body: Value = serde_json::from_slice(&result.stdout).unwrap();
    assert_eq!(body["target_count"], 0);
    assert_eq!(body["analysis_runs"], 0);
    assert_eq!(body["complete"], true);
    assert_eq!(model.count(), 6);
    assert_eq!(trap.count(), 0);
    for request in source.requests.lock().unwrap().iter() {
        assert!(request.contains("X-Gewyvern-Admin-Token: source-test-secret\r\n"));
        assert!(!request.contains("model-test-secret"));
        assert!(!request.starts_with("GET /v1/latest/targets/unselected/"));
    }
    for request in model.requests.lock().unwrap().iter() {
        assert!(request.contains("Authorization: Bearer model-test-secret\r\n"));
        assert!(!request.contains("source-test-secret"));
    }
}

#[test]
fn one_shot_refuses_legacy_state_without_network_or_file_mutation() {
    let source = HttpFixture::new("{}".into());
    let model = HttpFixture::new(advice());
    let config = TempConfig::new(
        &json!({"id":"test","model":"test",
        "endpoint":format!("http://{}/v1/chat/completions",model.address)})
        .to_string(),
    );
    let legacy = TempConfig::new("legacy-state-must-survive");
    for mode in ["analyze-url", "analyze-targets-url"] {
        let result = one_shot(
            &source,
            &config,
            mode,
            &["--state", legacy.0.to_str().unwrap()],
        );
        assert_eq!(result.status.code(), Some(2));
        assert!(result.stdout.is_empty());
        assert!(
            String::from_utf8(result.stderr)
                .unwrap()
                .contains("etragon_provider_unsupported_option")
        );
    }
    assert_eq!(source.count(), 0);
    assert_eq!(model.count(), 0);
    assert_eq!(
        std::fs::read_to_string(&legacy.0).unwrap(),
        "legacy-state-must-survive"
    );
}

#[test]
fn resident_pipeline_caches_recovers_and_restarts_with_new_provider_identity() {
    let source =
        HttpFixture::new(include_str!("fixtures/missing_transition_analysis.json").to_string());
    let model = HttpFixture::new(advice());
    let mut config = json!({"id":"first-provider","model":"model-one","endpoint":format!("http://{}/v1/chat/completions", model.address),"api_key_env":"ETRAGON_TEST_PROVIDER_TOKEN"});
    let file = TempConfig::new(&config.to_string());
    let capabilities = Command::new(env!("CARGO_BIN_EXE_etragon"))
        .args([
            "protocol-capabilities",
            "--provider",
            file.0.to_str().unwrap(),
        ])
        .env_remove("ETRAGON_TEST_PROVIDER_TOKEN")
        .output()
        .unwrap();
    assert!(capabilities.status.success());
    let caps: Value = serde_json::from_slice(&capabilities.stdout).unwrap();
    assert_eq!(caps["mode"], "external-provider");
    assert_eq!(caps["training_supported"], false);
    assert_eq!(
        caps["batch_contract"]["schema_version"],
        "etragon-provider-batch-v1"
    );
    assert_eq!(caps["batch_contract"]["partial_exit_code"], 2);
    assert!(
        caps["one_shot_commands"]
            .as_array()
            .unwrap()
            .contains(&json!("analyze-targets-url"))
    );
    assert_eq!(model.count(), 0, "capabilities must not invoke a model");

    let (address, process) = start(&source, &file);
    wait_for(|| {
        get(address, "/v1/latest/status")
            .is_some_and(|(_, v)| v["cache_hits"].as_u64().unwrap_or(0) >= 2)
    });
    assert_eq!(
        model.count(),
        1,
        "polling unchanged evidence must not bill again"
    );
    assert!(source.count() >= 3);
    let (_, status) = get(address, "/v1/latest/status").unwrap();
    assert_eq!(status["analysis_runs"], 1);
    assert!(get(address, "/health").unwrap().0 == 200);

    config["id"] = json!("second-provider");
    config["model"] = json!("model-two");
    std::fs::write(&file.0, config.to_string()).unwrap();
    assert_eq!(
        get(address, "/v1/protocol-capabilities.json").unwrap().1["provider_id"],
        "first-provider",
        "running config must not hot-reload"
    );

    *model.response.lock().unwrap() = (429, "PRIVATE_REMOTE_ERROR".into());
    let mut changed: Value = serde_json::from_str(&source.response.lock().unwrap().1).unwrap();
    changed["primary_failure_detail"] = json!("new observation");
    source.response.lock().unwrap().1 = changed.to_string();
    let (code, failed) = wait_for_output(address, |code, v| code == 503 && v["status"] == "error");
    assert_eq!(code, 503);
    assert!(failed["output"].is_null());
    assert!(failed["last_success_unix_ms"].is_number());
    assert!(!failed.to_string().contains("PRIVATE_REMOTE_ERROR"));
    assert_eq!(failed["last_error"], "etragon_provider_http_status: 429");
    *model.response.lock().unwrap() = (200, advice());
    let (_, recovered) = wait_for_output(address, |code, v| {
        code == 200 && v["analysis_runs"].as_u64().unwrap_or(0) >= 3
    });
    assert!(recovered["source_checked_unix_ms"].is_number());
    assert_eq!(
        recovered["output"]["augmentations"][0]["data"]["provider_id"],
        "first-provider"
    );
    assert_eq!(
        recovered["output"]["augmentations"][0]["data"]["operator_review_required"],
        true
    );

    drop(process);
    let before = model.count();
    let (address, _restarted) = start(&source, &file);
    let (_, result) = wait_for_output(address, |code, _| code == 200);
    assert_eq!(result["analysis_runs"], 1);
    assert_eq!(
        result["output"]["augmentations"][0]["data"]["provider_id"],
        "second-provider"
    );
    assert_eq!(
        result["output"]["augmentations"][0]["data"]["model"],
        "model-two"
    );
    assert_eq!(model.count(), before + 1);
    assert!(!result.to_string().contains("test-secret"));

    for request in source.requests.lock().unwrap().iter() {
        assert!(request.starts_with("GET /v1/latest/analysis.json HTTP/1.1\r\n"));
        assert!(request.contains("X-Gewyvern-Admin-Token: source-test-secret\r\n"));
        assert!(!request.contains("model-test-secret"));
    }
    for request in model.requests.lock().unwrap().iter() {
        assert!(request.starts_with("POST /v1/chat/completions HTTP/1.1\r\n"));
        assert!(request.contains("Authorization: Bearer model-test-secret\r\n"));
        assert!(!request.contains("source-test-secret"));
        assert!(
            !request
                .split_once("\r\n\r\n")
                .unwrap()
                .1
                .contains("model-test-secret")
        );
    }
}

#[test]
fn resident_refuses_legacy_persistence_without_touching_the_file() {
    let config = TempConfig::new(
        r#"{"id":"test","model":"test","endpoint":"http://127.0.0.1:1/v1/chat/completions"}"#,
    );
    let legacy = TempConfig::new("legacy-data-must-survive");
    let output = Command::new(env!("CARGO_BIN_EXE_etragon"))
        .args([
            "serve-url",
            "http://127.0.0.1:1/snapshot",
            "--provider",
            config.0.to_str().unwrap(),
            "--daemon-state",
            legacy.0.to_str().unwrap(),
        ])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(
        String::from_utf8(output.stderr)
            .unwrap()
            .contains("etragon_provider_unsupported_option")
    );
    assert_eq!(
        std::fs::read_to_string(&legacy.0).unwrap(),
        "legacy-data-must-survive"
    );
}

#[test]
fn target_resident_isolates_failures_tracks_membership_and_never_follows_index_urls() {
    let trap = HttpFixture::new("must never be contacted".into());
    let bad = "scan:bad?%2F# space";
    let good = "scan:good";
    let make_index = |names: &[&str]| {
        json!({"target_count":names.len(),"target_refs":names.iter().map(|name| json!({
        "path_segment":name,"url_path":format!("http://{}/steal", trap.address)
    })).collect::<Vec<_>>()})
        .to_string()
    };
    let source = HttpFixture::new(make_index(&[bad, good, "unselected"]));
    let model = HttpFixture::new(advice());
    let raw = include_str!("fixtures/missing_transition_analysis.json").to_string();
    source.routes.lock().unwrap().insert(
        "/v1/latest/targets/scan:bad%3F%252F%23%20space/analysis.json".into(),
        (503, "PRIVATE_UPSTREAM_FAILURE".into()),
    );
    source.routes.lock().unwrap().insert(
        "/v1/latest/targets/scan:good/analysis.json".into(),
        (200, raw.clone()),
    );
    let config = TempConfig::new(&json!({"id":"targets-test","model":"fixture","endpoint":format!("http://{}/v1/chat/completions",model.address),"api_key_env":"ETRAGON_TEST_PROVIDER_TOKEN"}).to_string());
    let (address, _process) = start_mode(
        &source,
        &config,
        "serve-targets-url",
        &[
            "--filter",
            "scan:",
            "--max-targets",
            "2",
            "--max-inferences-per-cycle",
            "1",
        ],
    );
    let good_route = "/v1/latest/targets/scan:good/output.json";
    let bad_route = "/v1/latest/targets/scan:bad%3F%252F%23%20space/output.json";
    let (_, good_result) = wait_for_path(address, good_route, |code, body| {
        code == 200 && body["cache_hits"].as_u64().unwrap_or(0) >= 2
    });
    let old_incarnation = good_result["target_incarnation"].clone();
    assert_eq!(model.count(), 1);
    assert_eq!(
        good_result["output"]["augmentations"][0]["data"]["target_path_segment"],
        good
    );
    let (_, partial) = wait_for_output(address, |code, body| {
        code == 503 && body["status"] == "partial"
    });
    assert_eq!(partial["ready_count"], 1);
    assert!(!partial.to_string().contains("PRIVATE_UPSTREAM_FAILURE"));
    assert_eq!(get(address, bad_route).unwrap().0, 503);
    assert_eq!(get(address, "/health").unwrap().0, 200);
    let caps = get(address, "/v1/protocol-capabilities.json").unwrap().1;
    assert_eq!(caps["multi_target_supported"], true);
    assert_eq!(caps["limits"]["max_inferences_per_cycle"], 1);
    assert_eq!(caps["training_supported"], false);

    source.routes.lock().unwrap().insert(
        "/v1/latest/targets/scan:bad%3F%252F%23%20space/analysis.json".into(),
        (200, raw.clone()),
    );
    let (_, recovered) = wait_for_path(address, bad_route, |code, _| code == 200);
    assert_eq!(
        recovered["output"]["augmentations"][0]["data"]["target_path_segment"],
        bad
    );
    assert_eq!(
        model.count(),
        2,
        "identical evidence must still be analyzed separately per target"
    );

    source.response.lock().unwrap().1 = make_index(&[bad]);
    wait_for_path(address, good_route, |code, _| code == 404);
    source.response.lock().unwrap().1 = make_index(&[bad, good]);
    let (_, reappeared) = wait_for_path(address, good_route, |code, _| code == 200);
    assert_ne!(reappeared["target_incarnation"], old_incarnation);
    assert_eq!(reappeared["analysis_runs"], 1);
    assert_eq!(model.count(), 3);

    source.response.lock().unwrap().1 = make_index(&[good, good]);
    let (_, invalid) = wait_for_output(address, |code, body| {
        code == 503 && body["index_status"] == "error"
    });
    assert!(
        invalid["targets"]
            .as_array()
            .unwrap()
            .iter()
            .all(|target| target["output"].is_null())
    );
    assert_eq!(model.count(), 3);
    source.response.lock().unwrap().1 = make_index(&[]);
    let (_, empty) = wait_for_output(address, |code, body| {
        code == 200 && body["target_count"] == 0
    });
    assert_eq!(empty["status"], "ready");
    assert_eq!(trap.count(), 0);
    for request in source.requests.lock().unwrap().iter() {
        assert!(request.contains("X-Gewyvern-Admin-Token: source-test-secret\r\n"));
        assert!(!request.contains("/unselected/"));
        assert!(!request.contains("/steal"));
        assert!(!request.contains("model-test-secret"));
    }
    assert!(source.requests.lock().unwrap().iter().any(|request| {
        request.starts_with("GET /v1/latest/targets/scan:bad%3F%252F%23%20space/analysis.json ")
    }));
    for request in model.requests.lock().unwrap().iter() {
        assert!(request.contains("Authorization: Bearer model-test-secret\r\n"));
        assert!(!request.contains("source-test-secret"));
    }
}
