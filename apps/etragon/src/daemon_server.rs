use super::*;

pub(super) const ETRAGON_ADMIN_TOKEN_ENV: &str = "ETRAGON_ADMIN_TOKEN";
pub(super) const ETRAGON_ADMIN_TOKEN_HEADER: &str = "X-Etragon-Admin-Token";

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct DaemonAccessPolicy {
    pub(super) admin_token: Option<String>,
}

pub(super) fn run_python_daemon_until<F>(
    bind_addr: &str,
    interval_ms: u64,
    config: &PythonWorkerConfig,
    daemon_state_file: Option<&Path>,
    source: &str,
    upstream_url: &str,
    mut poll_output: F,
    stop: Arc<AtomicBool>,
) -> Result<(), String>
where
    F: FnMut(usize, &mut PythonWorkerClient) -> Result<PolledDaemonOutput, String>,
{
    let restored_snapshot = daemon_state_file
        .map(read_daemon_state)
        .transpose()?
        .flatten()
        .map(|mut snapshot| {
            snapshot.source = source.to_string();
            snapshot.upstream_url = upstream_url.to_string();
            snapshot.interval_ms = interval_ms;
            snapshot
        });
    let latest = Arc::new(Mutex::new(restored_snapshot.clone()));
    let mut worker = PythonWorkerClient::spawn(config)?;
    let mut worker_epoch = 0u64;
    let invalidation_epoch = Arc::new(AtomicU64::new(0));
    let mut last_input_fingerprint = None::<String>;
    let mut last_cache_epoch = 0u64;
    let mut last_output_json = None::<String>;
    let mut last_latest_input_json = None::<Option<String>>;
    let mut last_recommendation_summary_json = None::<String>;
    let mut last_target_outputs = None::<Vec<TargetDaemonOutput>>;
    let mut analysis_runs = restored_snapshot
        .as_ref()
        .map(|snapshot| snapshot.analysis_runs)
        .unwrap_or(0);
    let mut cache_hits = restored_snapshot
        .as_ref()
        .map(|snapshot| snapshot.cache_hits)
        .unwrap_or(0);

    let access_policy = daemon_access_policy_from_env();
    validate_daemon_bind_addr(bind_addr, &access_policy)?;
    let listener = TcpListener::bind(bind_addr)
        .map_err(|err| format!("failed to bind daemon listener on {}: {err}", bind_addr))?;
    listener
        .set_nonblocking(true)
        .map_err(|err| format!("failed to configure daemon listener: {err}"))?;

    let latest_for_server = Arc::clone(&latest);
    let stop_for_server = Arc::clone(&stop);
    let invalidation_epoch_for_server = Arc::clone(&invalidation_epoch);
    let config_for_server = config.clone();
    let daemon_state_file_for_server = daemon_state_file.map(PathBuf::from);
    let access_policy_for_server = access_policy.clone();
    let server = thread::spawn(move || -> Result<(), String> {
        while !stop_for_server.load(Ordering::Relaxed) {
            match listener.accept() {
                Ok((stream, remote_addr)) => {
                    handle_daemon_client(
                        stream,
                        remote_addr.ip(),
                        &access_policy_for_server,
                        &latest_for_server,
                        &config_for_server,
                        daemon_state_file_for_server.as_deref(),
                        &invalidation_epoch_for_server,
                    )?;
                }
                Err(err) if err.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(25));
                }
                Err(err) => return Err(format!("daemon listener accept failed: {err}")),
            }
        }
        Ok(())
    });

    let mut cycle = 0usize;
    while !stop.load(Ordering::Relaxed) {
        cycle += 1;
        let current_epoch = invalidation_epoch.load(Ordering::Relaxed);
        if current_epoch != worker_epoch {
            worker = PythonWorkerClient::spawn(config)?;
            worker_epoch = current_epoch;
        }
        let polled = match poll_output(cycle, &mut worker) {
            Ok(polled) => polled,
            Err(err) => {
                let updated_unix_ms = now_unix_ms()?;
                let mut guard = latest
                    .lock()
                    .map_err(|_| "daemon snapshot lock poisoned".to_string())?;
                let mut snapshot = guard.clone().unwrap_or(DaemonSnapshot {
                    source: source.to_string(),
                    upstream_url: upstream_url.to_string(),
                    interval_ms,
                    cycle,
                    analysis_runs,
                    cache_hits,
                    target_count: 0,
                    updated_unix_ms,
                    state_hash: String::new(),
                    latest_output_json: "null".to_string(),
                    latest_input_json: None,
                    latest_recommendation_summary_json: "null".to_string(),
                    target_outputs: Vec::new(),
                    last_success_unix_ms: None,
                    last_error: None,
                    training_history: Vec::new(),
                });
                snapshot.cycle = cycle;
                snapshot.analysis_runs = analysis_runs;
                snapshot.cache_hits = cache_hits;
                snapshot.updated_unix_ms = updated_unix_ms;
                snapshot.last_error = Some(err);
                *guard = Some(snapshot);
                let snapshot_to_persist = guard.clone();
                drop(guard);
                if let (Some(path), Some(snapshot)) =
                    (daemon_state_file, snapshot_to_persist.as_ref())
                {
                    write_daemon_state(path, snapshot)?;
                }
                let mut slept = 0u64;
                while slept < interval_ms && !stop.load(Ordering::Relaxed) {
                    let step = (interval_ms - slept).min(25);
                    thread::sleep(Duration::from_millis(step));
                    slept += step;
                }
                continue;
            }
        };
        let (output, latest_input_json, recommendation_summary_json, target_outputs) =
            if last_input_fingerprint.as_deref() == Some(polled.input_fingerprint.as_str())
                && last_cache_epoch == current_epoch
            {
                cache_hits += 1;
                (
                    last_output_json
                        .clone()
                        .ok_or_else(|| "daemon cache hit without cached output".to_string())?,
                    last_latest_input_json
                        .clone()
                        .ok_or_else(|| "daemon cache hit without cached input".to_string())?,
                    last_recommendation_summary_json.clone().ok_or_else(|| {
                        "daemon cache hit without cached recommendation summary".to_string()
                    })?,
                    last_target_outputs.clone().ok_or_else(|| {
                        "daemon cache hit without cached target outputs".to_string()
                    })?,
                )
            } else {
                analysis_runs += 1;
                last_input_fingerprint = Some(polled.input_fingerprint);
                last_cache_epoch = current_epoch;
                last_output_json = Some(polled.output_json.clone());
                last_latest_input_json = Some(polled.latest_input_json.clone());
                last_recommendation_summary_json = Some(polled.recommendation_summary_json.clone());
                last_target_outputs = Some(polled.target_outputs.clone());
                (
                    polled.output_json,
                    polled.latest_input_json,
                    polled.recommendation_summary_json,
                    polled.target_outputs,
                )
            };
        let updated_unix_ms = now_unix_ms()?;
        let state_hash = state_hash_for_output(&output, &recommendation_summary_json);
        let mut guard = latest
            .lock()
            .map_err(|_| "daemon snapshot lock poisoned".to_string())?;
        let previous_snapshot = guard.as_ref().cloned();
        let target_outputs = enrich_target_outputs(target_outputs, updated_unix_ms);
        let target_outputs = if let Some(previous) = &previous_snapshot {
            target_outputs
                .into_iter()
                .map(|mut target| {
                    if let Some(old_target) = previous
                        .target_outputs
                        .iter()
                        .find(|old| old.path_segment == target.path_segment)
                    {
                        target.training_history = old_target.training_history.clone();
                    }
                    target
                })
                .collect()
        } else {
            target_outputs
        };
        *guard = Some(DaemonSnapshot {
            source: source.to_string(),
            upstream_url: upstream_url.to_string(),
            interval_ms,
            cycle,
            analysis_runs,
            cache_hits,
            target_count: target_outputs.len(),
            updated_unix_ms,
            state_hash,
            latest_output_json: output,
            latest_input_json,
            latest_recommendation_summary_json: recommendation_summary_json,
            target_outputs,
            last_success_unix_ms: Some(updated_unix_ms),
            last_error: None,
            training_history: previous_snapshot
                .map(|snapshot| snapshot.training_history)
                .unwrap_or_default(),
        });
        let snapshot_to_persist = guard.clone();
        drop(guard);
        if let (Some(path), Some(snapshot)) = (daemon_state_file, snapshot_to_persist.as_ref()) {
            write_daemon_state(path, snapshot)?;
        }
        let mut slept = 0u64;
        while slept < interval_ms && !stop.load(Ordering::Relaxed) {
            let step = (interval_ms - slept).min(25);
            thread::sleep(Duration::from_millis(step));
            slept += step;
        }
    }

    server
        .join()
        .map_err(|_| "daemon server thread panicked".to_string())?
}

pub(super) fn daemon_access_policy_from_env() -> DaemonAccessPolicy {
    DaemonAccessPolicy {
        admin_token: std::env::var(ETRAGON_ADMIN_TOKEN_ENV)
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
    }
}

pub(super) fn validate_daemon_bind_addr(
    bind_addr: &str,
    access_policy: &DaemonAccessPolicy,
) -> Result<(), String> {
    let resolved = bind_addr
        .to_socket_addrs()
        .map_err(|err| {
            format!(
                "failed to resolve daemon bind address '{}': {err}",
                bind_addr
            )
        })?
        .collect::<Vec<_>>();
    if resolved.is_empty() {
        return Err(format!(
            "daemon bind address '{}' did not resolve to any socket addresses",
            bind_addr
        ));
    }
    if resolved
        .iter()
        .any(|socket_addr| !daemon_client_is_loopback(socket_addr.ip()))
    {
        if access_policy.admin_token.is_none() {
            return Err(format!(
                "daemon bind address '{}' is not loopback-only; bind etragon daemons to 127.0.0.1 or ::1, or set {} for explicit remote access",
                bind_addr, ETRAGON_ADMIN_TOKEN_ENV,
            ));
        }
    }
    Ok(())
}

pub(super) fn daemon_client_is_loopback(address: IpAddr) -> bool {
    address.is_loopback()
}

pub(super) fn write_daemon_access_denied(stream: &mut TcpStream) -> Result<(), String> {
    let body = "{\"error\":\"daemon_access_denied\",\"reason\":\"etragon daemon requires loopback access or a valid admin token\"}";
    let response = format!(
        "HTTP/1.1 403 Forbidden\r\nContent-Type: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n{}",
        body.len(),
        body
    );
    stream
        .write_all(response.as_bytes())
        .map_err(|err| format!("failed to write daemon access denial: {err}"))
}
