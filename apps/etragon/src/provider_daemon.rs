use super::*;
use etragon::provider::{
    AdvisoryRequest, AnalysisSource, ChatCompletionsProvider, ProviderConfig, ProviderError,
};
use serde_json::{Value, json};
use std::net::SocketAddr;

pub(super) struct ProviderDaemonOptions {
    pub bind: SocketAddr,
    pub interval_ms: u64,
    pub config: ProviderConfig,
    pub source_ca: Option<PathBuf>,
}

pub(super) fn parse_provider_daemon_options(
    args: &[String],
) -> Result<ProviderDaemonOptions, String> {
    let mut provider_path = None;
    let mut source_ca = None;
    let mut bind = SocketAddr::from(([127, 0, 0, 1], 4321));
    let mut interval_ms = 1000;
    let mut seen = std::collections::HashSet::new();
    for pair in args.chunks(2) {
        if pair.len() != 2 || !seen.insert(pair[0].as_str()) {
            return Err("etragon_provider_invalid_options".into());
        }
        match pair[0].as_str() {
            "--provider" => provider_path = Some(pair[1].as_str()),
            "--source-ca" => source_ca = Some(PathBuf::from(&pair[1])),
            "--bind" => {
                bind = pair[1]
                    .parse()
                    .map_err(|_| "etragon_provider_invalid_bind")?
            }
            "--interval-ms" => {
                interval_ms = pair[1]
                    .parse()
                    .map_err(|_| "etragon_provider_invalid_interval")?
            }
            _ => return Err("etragon_provider_unsupported_option".into()),
        }
    }
    if !(100..=3_600_000).contains(&interval_ms) {
        return Err("etragon_provider_invalid_interval".into());
    }
    let config = super::cli_provider::load_provider_config(
        provider_path.ok_or("etragon_provider_config_required")?,
    )?;
    Ok(ProviderDaemonOptions {
        bind,
        interval_ms,
        config,
        source_ca,
    })
}

pub(super) fn provider_capabilities(config: &ProviderConfig) -> String {
    provider_capabilities_value(config).to_string()
}

fn provider_capabilities_value(config: &ProviderConfig) -> Value {
    json!({
        "protocol_family":"etragon-resident-protocol", "protocol_version":1,
        "mode":"external-provider", "provider_id":config.id, "model":config.model,
        "input_contracts":["gewyvern-analysis-json-v1"],
        "output_contracts":["etragon-engine-output-v1", "etragon-advisory-v1"],
        "daemon_routes":["/health", "/v1/protocol-capabilities.json", "/v1/latest/status", "/v1/latest/meta", "/v1/latest/output.json"],
        "merge_capabilities":{"safe_automation_hints":[], "operator_review_hints":["augmentations_only"]},
        "handoff_capabilities":{"readiness_levels":["advisory_only"]},
        "context_capabilities":{"published_contexts":[]},
        "compatibility":{"forward_compatibility_rules":["unknown_merge_hints_must_downgrade_to_operator_review"]},
        "training_supported":false, "memory_mutation_supported":false,
        "persistence_supported":false, "hot_reload_supported":false, "multi_target_supported":false,
        "available_modes":["single-source", "target-index"],
        "one_shot_commands":["analyze-json", "analyze-url", "analyze-targets-url"],
        "batch_contract":{"schema_version":"etragon-provider-batch-v1", "max_inferences":8, "max_output_bytes":1048576, "partial_exit_code":2},
        "operator_review_required":true,
        "cache":{"scope":"process-and-startup-config", "entries":1, "key":"canonical-allowlisted-evidence", "checked_before_inference":true},
    })
}

#[derive(Default)]
struct ProviderSnapshot {
    cycle: u64,
    analysis_runs: u64,
    cache_hits: u64,
    updated_unix_ms: Option<u128>,
    source_checked_unix_ms: Option<u128>,
    source_checked_at: Option<std::time::Instant>,
    last_success_unix_ms: Option<u128>,
    last_error: Option<String>,
    consecutive_failures: u32,
    retry_delay_ms: u64,
    status: &'static str,
    output: Option<Value>,
}

impl ProviderSnapshot {
    fn json(&self, config: &ProviderConfig, interval_ms: u64, include_output: bool) -> Value {
        let mut value = json!({
            "source":"external-provider-url", "mode":"external-provider",
            "provider_id":config.id, "model":config.model, "status":self.status,
            "interval_ms":interval_ms, "cycle":self.cycle, "analysis_runs":self.analysis_runs,
            "cache_hits":self.cache_hits, "updated_unix_ms":self.updated_unix_ms,
            "source_checked_unix_ms":self.source_checked_unix_ms,
            "last_success_unix_ms":self.last_success_unix_ms, "last_error":self.last_error,
            "consecutive_failures":self.consecutive_failures, "retry_delay_ms":self.retry_delay_ms,
            "learning_active":false, "learned_routes":0, "target_count":0,
            "operator_review_required":true, "advisory_only":true,
        });
        if include_output {
            value["output"] = self.output.clone().unwrap_or(Value::Null);
        }
        value
    }
}

type SharedSnapshot = Arc<Mutex<ProviderSnapshot>>;

fn update(
    state: &SharedSnapshot,
    change: impl FnOnce(&mut ProviderSnapshot),
) -> Result<(), String> {
    let mut guard = state
        .lock()
        .map_err(|_| "etragon_provider_state_unavailable")?;
    change(&mut guard);
    guard.updated_unix_ms = Some(now_unix_ms()?);
    Ok(())
}

struct ProviderPoller {
    cached: Option<(String, Value)>,
}

impl ProviderPoller {
    fn poll(
        &mut self,
        state: &SharedSnapshot,
        interval_ms: u64,
        fetch: impl FnOnce() -> Result<String, ProviderError>,
        infer: impl FnOnce(&str) -> Result<etragon::EngineOutput, ProviderError>,
    ) -> Result<u64, String> {
        self.poll_with_budget(state, interval_ms, true, fetch, infer)
    }

    fn poll_with_budget(
        &mut self,
        state: &SharedSnapshot,
        interval_ms: u64,
        allow_inference: bool,
        fetch: impl FnOnce() -> Result<String, ProviderError>,
        infer: impl FnOnce(&str) -> Result<etragon::EngineOutput, ProviderError>,
    ) -> Result<u64, String> {
        update(state, |state| {
            state.cycle = state.cycle.saturating_add(1);
            state.status = "checking";
            state.output = None;
            state.retry_delay_ms = 0;
        })?;
        let input = match fetch() {
            Ok(input) => input,
            Err(error) => {
                return self.fail(
                    state,
                    interval_ms,
                    error
                        .to_string()
                        .replace("etragon_provider_", "etragon_source_"),
                );
            }
        };
        let request = match AdvisoryRequest::from_analysis_json(&input) {
            Ok(request) => request,
            Err(error) => return self.fail(state, interval_ms, error.to_string()),
        };
        let key = serde_json::to_string(&request).map_err(|_| "etragon_provider_invalid_input")?;
        let checked_at = now_unix_ms()?;
        update(state, |state| {
            state.source_checked_unix_ms = Some(checked_at);
            state.source_checked_at = Some(std::time::Instant::now());
        })?;
        if let Some((old_key, output)) = &self.cached
            && old_key == &key
        {
            update(state, |state| {
                state.cache_hits = state.cache_hits.saturating_add(1);
                state.status = "ready";
                state.output = Some(output.clone());
                state.last_error = None;
                state.consecutive_failures = 0;
            })?;
            return Ok(interval_ms);
        }
        self.cached = None;
        if !allow_inference {
            update(state, |state| state.status = "queued")?;
            return Ok(interval_ms);
        }
        update(state, |state| {
            state.status = "analyzing";
            state.analysis_runs = state.analysis_runs.saturating_add(1);
        })?;
        let output = match infer(&input) {
            Ok(output) => serde_json::from_str::<Value>(&etragon::engine_output_json(&output))
                .map_err(|_| "etragon_provider_invalid_response")?,
            Err(error) => return self.fail(state, interval_ms, error.to_string()),
        };
        let success_time = now_unix_ms()?;
        update(state, |state| {
            state.status = "ready";
            state.output = Some(output.clone());
            state.last_success_unix_ms = Some(success_time);
            state.last_error = None;
            state.consecutive_failures = 0;
        })?;
        self.cached = Some((key, output));
        Ok(interval_ms)
    }

    fn fail(
        &mut self,
        state: &SharedSnapshot,
        interval_ms: u64,
        error: String,
    ) -> Result<u64, String> {
        self.cached = None;
        let mut delay = 0;
        update(state, |state| {
            state.status = "error";
            state.output = None;
            state.last_error = Some(error);
            state.consecutive_failures = state.consecutive_failures.saturating_add(1);
            delay = interval_ms
                .max(1000)
                .saturating_mul(1u64 << state.consecutive_failures.saturating_sub(1).min(6));
            delay = delay.min(60_000).max(interval_ms);
            state.retry_delay_ms = delay;
        })?;
        Ok(delay)
    }
}

struct StopOnDrop(Arc<AtomicBool>);
impl Drop for StopOnDrop {
    fn drop(&mut self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

pub(super) fn serve_provider_url(
    url: &str,
    options: ProviderDaemonOptions,
) -> Result<String, String> {
    let source = AnalysisSource::new(url.to_owned(), options.source_ca.clone())
        .map_err(|e| e.to_string())?;
    let provider =
        ChatCompletionsProvider::new(options.config.clone()).map_err(|e| e.to_string())?;
    let policy = daemon_access_policy_from_env();
    if !options.bind.ip().is_loopback() && policy.admin_token.is_none() {
        return Err("etragon_provider_remote_bind_requires_admin_token".into());
    }
    let listener = TcpListener::bind(options.bind).map_err(|_| "etragon_provider_bind_failed")?;
    run_provider_daemon_until(
        listener,
        &options,
        policy,
        Arc::new(AtomicBool::new(false)),
        || source.fetch_json(),
        |input| provider.analyze_json(input),
    )?;
    Ok(String::new())
}

fn run_provider_daemon_until(
    listener: TcpListener,
    options: &ProviderDaemonOptions,
    policy: DaemonAccessPolicy,
    stop: Arc<AtomicBool>,
    mut fetch: impl FnMut() -> Result<String, ProviderError>,
    mut infer: impl FnMut(&str) -> Result<etragon::EngineOutput, ProviderError>,
) -> Result<(), String> {
    let state = Arc::new(Mutex::new(ProviderSnapshot {
        status: "starting",
        ..Default::default()
    }));
    let server_state = Arc::clone(&state);
    run_provider_service_until(
        listener,
        &stop,
        move |stream, peer| handle_client(stream, peer, &policy, &server_state, options),
        || {
            let mut poller = ProviderPoller { cached: None };
            while !stop.load(Ordering::Relaxed) {
                let delay = poller.poll(&state, options.interval_ms, &mut fetch, &mut infer)?;
                wait_provider_poll(delay, &stop);
            }
            Ok(())
        },
    )
}

fn wait_provider_poll(delay: u64, stop: &AtomicBool) {
    let deadline = std::time::Instant::now() + Duration::from_millis(delay);
    while !stop.load(Ordering::Relaxed) {
        let remaining = deadline.saturating_duration_since(std::time::Instant::now());
        if remaining.is_zero() {
            break;
        }
        thread::sleep(remaining.min(Duration::from_millis(25)));
    }
}

fn run_provider_service_until(
    listener: TcpListener,
    stop: &Arc<AtomicBool>,
    handle: impl Fn(TcpStream, IpAddr) -> Result<(), String> + Send,
    poll: impl FnOnce() -> Result<(), String>,
) -> Result<(), String> {
    listener
        .set_nonblocking(true)
        .map_err(|_| "etragon_provider_listener_failed")?;
    thread::scope(|scope| {
        let guard = StopOnDrop(Arc::clone(stop));
        let server_stop = Arc::clone(stop);
        let server = scope.spawn(move || {
            let _guard = StopOnDrop(Arc::clone(&server_stop));
            while !server_stop.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, peer)) => {
                        if handle(stream, peer.ip()).is_err() {
                            eprintln!("etragon_provider_client_failed");
                        }
                    }
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        thread::sleep(Duration::from_millis(25))
                    }
                    Err(_) => return Err("etragon_provider_listener_failed".into()),
                }
            }
            Ok(())
        });
        let result = poll();
        drop(guard);
        let server_result = server
            .join()
            .map_err(|_| "etragon_provider_listener_panicked")?;
        result.and(server_result)
    })
}

fn handle_client(
    stream: TcpStream,
    peer: IpAddr,
    policy: &DaemonAccessPolicy,
    state: &SharedSnapshot,
    options: &ProviderDaemonOptions,
) -> Result<(), String> {
    handle_provider_request(stream, |request| {
        route(request, peer, policy, state, options)
    })
}

fn handle_provider_request(
    mut stream: TcpStream,
    route: impl FnOnce(&str) -> Result<String, String>,
) -> Result<(), String> {
    let response = match read_daemon_request(&mut stream)? {
        DaemonRequestRead::Invalid => {
            daemon_error_response("HTTP/1.1 400 Bad Request", "daemon_request_invalid")
        }
        DaemonRequestRead::TooLarge => {
            daemon_error_response("HTTP/1.1 413 Payload Too Large", "daemon_request_too_large")
        }
        DaemonRequestRead::Complete(request) => route(&request)?,
    };
    let mut stream =
        silvortex_bounded_io::wrap_tcp_stream_with_io_deadline(stream, Duration::from_secs(1))
            .map_err(|_| "etragon_provider_client_failed")?;
    stream
        .write_all(response.as_bytes())
        .map_err(|_| "etragon_provider_client_failed".into())
}

fn route(
    request: &str,
    peer: IpAddr,
    policy: &DaemonAccessPolicy,
    state: &SharedSnapshot,
    options: &ProviderDaemonOptions,
) -> Result<String, String> {
    let path = match provider_read_path(request, peer, policy) {
        Ok(path) => path,
        Err(response) => return Ok(response),
    };
    match path {
        "/health" => Ok(daemon_json_ok(
            "{\"status\":\"ok\",\"mode\":\"external-provider\"}",
        )),
        "/v1/protocol-capabilities.json" => {
            Ok(daemon_json_ok(&provider_capabilities(&options.config)))
        }
        "/v1/latest/status" | "/v1/latest/meta" | "/v1/latest/output.json" => {
            let state = state
                .lock()
                .map_err(|_| "etragon_provider_state_unavailable")?;
            let include_output = path == "/v1/latest/output.json";
            let body = state.json(&options.config, options.interval_ms, include_output);
            Ok(provider_json_response(
                &body,
                !include_output || state.status == "ready",
            ))
        }
        _ => Ok(daemon_error_response(
            "HTTP/1.1 404 Not Found",
            "etragon_provider_route_unsupported",
        )),
    }
}

fn provider_read_path<'a>(
    request: &'a str,
    peer: IpAddr,
    policy: &DaemonAccessPolicy,
) -> Result<&'a str, String> {
    if !daemon_request_is_authorized(peer, request, policy) {
        return Err(daemon_error_response(
            "HTTP/1.1 403 Forbidden",
            "daemon_access_denied",
        ));
    }
    let parts = request
        .lines()
        .next()
        .unwrap_or_default()
        .split_whitespace()
        .collect::<Vec<_>>();
    if parts.len() != 3 || !["HTTP/1.0", "HTTP/1.1"].contains(&parts[2]) {
        return Err(daemon_error_response(
            "HTTP/1.1 400 Bad Request",
            "daemon_request_invalid",
        ));
    }
    if parts[0] != "GET" {
        return Err(daemon_error_response(
            "HTTP/1.1 405 Method Not Allowed",
            "etragon_provider_read_only",
        ));
    }
    Ok(parts[1])
}

fn provider_json_response(body: &Value, available: bool) -> String {
    let status = if available {
        "200 OK"
    } else {
        "503 Service Unavailable"
    };
    let body = body.to_string();
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nCache-Control: no-store\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
}

pub(super) mod targets;

#[cfg(test)]
mod tests;
