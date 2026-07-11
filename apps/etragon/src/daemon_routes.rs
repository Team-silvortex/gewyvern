use super::*;

pub(super) fn handle_daemon_client(
    mut stream: TcpStream,
    remote_ip: IpAddr,
    access_policy: &DaemonAccessPolicy,
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    config: &PythonWorkerConfig,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> Result<(), String> {
    let request_text = match read_daemon_request(&mut stream)? {
        DaemonRequestRead::Complete(request_text) => request_text,
        DaemonRequestRead::TooLarge => {
            let response =
                daemon_error_response("HTTP/1.1 413 Payload Too Large", "daemon_request_too_large");
            stream
                .write_all(response.as_bytes())
                .map_err(|err| format!("failed to write daemon request limit response: {err}"))?;
            return Ok(());
        }
        DaemonRequestRead::Invalid => {
            let response =
                daemon_error_response("HTTP/1.1 400 Bad Request", "daemon_request_invalid");
            stream
                .write_all(response.as_bytes())
                .map_err(|err| format!("failed to write daemon invalid request response: {err}"))?;
            return Ok(());
        }
    };
    let (method, path) = request_text
        .lines()
        .next()
        .and_then(|line| {
            let mut parts = line.split_whitespace();
            Some((parts.next()?, parts.next()?))
        })
        .ok_or_else(|| "invalid daemon HTTP request".to_string())?;
    if !daemon_request_is_authorized(remote_ip, &request_text, access_policy) {
        write_daemon_access_denied(&mut stream)?;
        return Ok(());
    }
    let snapshot = latest
        .lock()
        .map_err(|_| "daemon snapshot lock poisoned".to_string())?
        .clone();
    let response = match path {
        "/health" => daemon_json_ok("{\"status\":\"ok\"}"),
        "/v1/training-labels.json" => daemon_json_ok(&training_labels_json()),
        "/v1/memory-state.json" => memory_state_route_response(config, snapshot.as_ref()),
        "/v1/memory-model.json" => memory_model_route_response(config),
        "/v1/memory-versions.json" => memory_versions_route_response(config),
        "/v1/memory-snapshot.json" => memory_snapshot_route_response(config),
        "/v1/protocol-capabilities.json" => {
            daemon_gateway_json_response(protocol_capabilities_json(config))
        }
        "/v1/memory-admin/save" if method == "POST" => {
            save_memory_slot_route_response(config, &request_text)
        }
        "/v1/memory-admin/clear" if method == "POST" => {
            clear_memory_route_response(config, latest, daemon_state_file, invalidation_epoch)
        }
        "/v1/memory-admin/load" if method == "POST" => load_memory_route_response(
            config,
            &request_text,
            latest,
            daemon_state_file,
            invalidation_epoch,
        ),
        "/v1/memory-admin/delete" if method == "POST" => {
            delete_memory_slot_route_response(config, &request_text)
        }
        "/v1/latest/status" => daemon_json_ok(&daemon_status_json(snapshot.as_ref())),
        "/v1/latest/meta" => daemon_json_ok(&daemon_meta_json(
            snapshot.as_ref(),
            Some(&daemon_meta_worker_state_json(snapshot.as_ref())),
        )),
        "/v1/latest/recommendation-summary.json" => match snapshot {
            Some(snapshot) => daemon_json_ok(&snapshot.latest_recommendation_summary_json),
            None => no_snapshot_response(),
        },
        "/v1/latest/federation-summary.json" => match snapshot {
            Some(snapshot) => daemon_json_ok(&federation_summary_json_from_snapshot(&snapshot)),
            None => no_snapshot_response(),
        },
        "/v1/latest/learning-summary.json" => latest_route_response(snapshot.as_ref(), "learning"),
        "/v1/latest/evidence-chain-enrichment.json" => {
            latest_route_response(snapshot.as_ref(), "evidence_chain_enrichment")
        }
        "/v1/latest/diagnostic-opinion.json" => {
            latest_route_response(snapshot.as_ref(), "diagnostic_opinion")
        }
        "/v1/latest/handoff-summary.json" => latest_route_response(snapshot.as_ref(), "handoff"),
        "/v1/latest/output.json" => match snapshot {
            Some(snapshot) => daemon_json_ok(&daemon_snapshot_json(&snapshot)),
            None => no_snapshot_response(),
        },
        "/v1/train/latest" if method == "POST" => train_latest_route_response(
            snapshot.as_ref(),
            &request_text,
            config,
            latest,
            daemon_state_file,
            invalidation_epoch,
        )?,
        "/v1/latest/targets" => match snapshot {
            Some(snapshot) => daemon_json_ok(&daemon_target_index_json(&snapshot)),
            None => no_snapshot_response(),
        },
        _ if path.starts_with("/v1/latest/targets/") => {
            target_route_response(snapshot.as_ref(), path)
        }
        _ if method == "POST" && path.starts_with("/v1/train/targets/") => {
            train_target_route_response(
                snapshot.as_ref(),
                path,
                &request_text,
                config,
                latest,
                daemon_state_file,
                invalidation_epoch,
            )?
        }
        _ => not_found_response(),
    };
    stream
        .write_all(response.as_bytes())
        .map_err(|err| format!("failed to write daemon response: {err}"))?;
    Ok(())
}

struct DaemonTrainingRefresh {
    training_json: String,
    analysis_json: String,
    recommendation_summary_json: String,
}

fn train_and_refresh_daemon_input(
    input_json: &str,
    label: &str,
    weight: f64,
    config: &PythonWorkerConfig,
) -> Result<DaemonTrainingRefresh, String> {
    let mut worker = PythonWorkerClient::spawn(config)?;
    let training_json = worker.train_json_with_weight(input_json, label, weight)?;
    let analysis_json = worker.analyze_json(input_json)?;
    let recommendation_summary_json = single_output_recommendation_summary(&analysis_json);
    Ok(DaemonTrainingRefresh {
        training_json,
        analysis_json,
        recommendation_summary_json,
    })
}

fn train_latest_route_response(
    snapshot: Option<&DaemonSnapshot>,
    request_text: &str,
    config: &PythonWorkerConfig,
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> Result<String, String> {
    let Some(snapshot) = snapshot else {
        return Ok(no_snapshot_response());
    };
    let Some(input_json) = snapshot.latest_input_json.as_deref() else {
        return Ok(no_trainable_snapshot_response());
    };
    let (label, weight) = match parse_training_feedback(request_text) {
        Ok(feedback) => feedback,
        Err(err) => return Ok(bad_request_response(&err)),
    };
    Ok(complete_training_route(
        input_json,
        &label,
        weight,
        config,
        |refresh| apply_latest_training_refresh(latest, &label, weight, refresh),
        daemon_state_file,
        invalidation_epoch,
    )?)
}

fn train_target_route_response(
    snapshot: Option<&DaemonSnapshot>,
    path: &str,
    request_text: &str,
    config: &PythonWorkerConfig,
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> Result<String, String> {
    let Some(snapshot) = snapshot else {
        return Ok(no_snapshot_response());
    };
    let segment = path
        .trim_start_matches("/v1/train/targets/")
        .trim_end_matches('/');
    let Some(target) = snapshot
        .target_outputs
        .iter()
        .find(|target| target.path_segment == segment)
    else {
        return Ok(target_not_found_response());
    };
    let Some(input_json) = target.input_json.as_deref() else {
        return Ok(no_trainable_snapshot_response());
    };
    let (label, weight) = match parse_training_feedback(request_text) {
        Ok(feedback) => feedback,
        Err(err) => return Ok(bad_request_response(&err)),
    };
    Ok(complete_training_route(
        input_json,
        &label,
        weight,
        config,
        |refresh| apply_target_training_refresh(latest, segment, &label, weight, refresh),
        daemon_state_file,
        invalidation_epoch,
    )?)
}

fn complete_training_route<F>(
    input_json: &str,
    label: &str,
    weight: f64,
    config: &PythonWorkerConfig,
    apply_refresh: F,
    daemon_state_file: Option<&Path>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> Result<String, String>
where
    F: FnOnce(&DaemonTrainingRefresh) -> Result<Option<DaemonSnapshot>, String>,
{
    let refresh = match train_and_refresh_daemon_input(input_json, label, weight, config) {
        Ok(refresh) => refresh,
        Err(err) => return Ok(bad_gateway_response(&err)),
    };
    let snapshot_to_persist = apply_refresh(&refresh)?;
    persist_snapshot_and_invalidate(
        daemon_state_file,
        snapshot_to_persist.as_ref(),
        invalidation_epoch,
    )?;
    Ok(daemon_json_ok(&refresh.training_json))
}

fn apply_latest_training_refresh(
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    label: &str,
    weight: f64,
    refresh: &DaemonTrainingRefresh,
) -> Result<Option<DaemonSnapshot>, String> {
    let mut guard = latest
        .lock()
        .map_err(|_| "daemon snapshot lock poisoned".to_string())?;
    let Some(snapshot) = guard.as_mut() else {
        return Ok(None);
    };
    let trained_unix_ms = now_unix_ms().unwrap_or(snapshot.updated_unix_ms);
    push_training_event(
        &mut snapshot.training_history,
        TrainingEvent {
            label: label.to_string(),
            weight: format!("{}", weight),
            trained_unix_ms,
            scope: "latest".to_string(),
        },
    );
    snapshot.analysis_runs += 1;
    snapshot.updated_unix_ms = trained_unix_ms;
    snapshot.last_success_unix_ms = Some(trained_unix_ms);
    snapshot.last_error = None;
    snapshot.latest_output_json = refresh.analysis_json.clone();
    snapshot.latest_recommendation_summary_json = refresh.recommendation_summary_json.clone();
    snapshot.state_hash = state_hash_for_output(
        &snapshot.latest_output_json,
        &snapshot.latest_recommendation_summary_json,
    );
    Ok(Some(snapshot.clone()))
}

fn apply_target_training_refresh(
    latest: &Arc<Mutex<Option<DaemonSnapshot>>>,
    segment: &str,
    label: &str,
    weight: f64,
    refresh: &DaemonTrainingRefresh,
) -> Result<Option<DaemonSnapshot>, String> {
    let mut guard = latest
        .lock()
        .map_err(|_| "daemon snapshot lock poisoned".to_string())?;
    let Some(snapshot) = guard.as_mut() else {
        return Ok(None);
    };
    let trained_unix_ms = now_unix_ms().unwrap_or(snapshot.updated_unix_ms);
    push_training_event(
        &mut snapshot.training_history,
        TrainingEvent {
            label: label.to_string(),
            weight: format!("{}", weight),
            trained_unix_ms,
            scope: format!("target:{segment}"),
        },
    );
    if let Some(target) = snapshot
        .target_outputs
        .iter_mut()
        .find(|target| target.path_segment == segment)
    {
        target.output_json = refresh.analysis_json.clone();
        target.recommendation_summary_json = refresh.recommendation_summary_json.clone();
        target.updated_unix_ms = trained_unix_ms;
        target.last_success_unix_ms = Some(trained_unix_ms);
        target.last_error = None;
        target.state_hash =
            state_hash_for_output(&target.output_json, &target.recommendation_summary_json);
        push_training_event(
            &mut target.training_history,
            TrainingEvent {
                label: label.to_string(),
                weight: format!("{}", weight),
                trained_unix_ms,
                scope: "target".to_string(),
            },
        );
    }
    refresh_snapshot_from_targets(snapshot, trained_unix_ms);
    Ok(Some(snapshot.clone()))
}

fn refresh_snapshot_from_targets(snapshot: &mut DaemonSnapshot, trained_unix_ms: u128) {
    let target_entries = snapshot
        .target_outputs
        .iter()
        .map(|target| (target.path_segment.clone(), target.output_json.clone()))
        .collect::<Vec<_>>();
    snapshot.analysis_runs += 1;
    snapshot.target_count = snapshot.target_outputs.len();
    snapshot.updated_unix_ms = trained_unix_ms;
    snapshot.last_success_unix_ms = Some(trained_unix_ms);
    snapshot.last_error = None;
    snapshot.latest_output_json = batch_output_json(&target_entries);
    snapshot.latest_recommendation_summary_json = recommendation_overview_json(&target_entries);
    snapshot.state_hash = state_hash_for_output(
        &snapshot.latest_output_json,
        &snapshot.latest_recommendation_summary_json,
    );
}

fn persist_snapshot_and_invalidate(
    daemon_state_file: Option<&Path>,
    snapshot: Option<&DaemonSnapshot>,
    invalidation_epoch: &Arc<AtomicU64>,
) -> Result<(), String> {
    if let (Some(path), Some(snapshot)) = (daemon_state_file, snapshot) {
        write_daemon_state(path, snapshot)?;
    }
    invalidation_epoch.fetch_add(1, Ordering::Relaxed);
    Ok(())
}

fn bad_request_response(err: &str) -> String {
    daemon_bad_request_response(err)
}

fn bad_gateway_response(err: &str) -> String {
    daemon_bad_gateway_response(err)
}

fn not_found_response() -> String {
    daemon_error_response("HTTP/1.1 404 Not Found", "not_found")
}

fn latest_route_response(snapshot: Option<&DaemonSnapshot>, route: &str) -> String {
    let Some(snapshot) = snapshot else {
        return no_snapshot_response();
    };
    let queue_summary_override = latest_queue_summary_override(snapshot);
    match route {
        "learning" => daemon_json_ok(&learning_summary_json_from_output_and_history(
            &snapshot.latest_output_json,
            &snapshot.latest_recommendation_summary_json,
            &snapshot.training_history,
            queue_summary_override.as_deref(),
        )),
        "evidence_chain_enrichment" => daemon_json_ok(&learning_summary_field_json(
            &snapshot.latest_output_json,
            &snapshot.latest_recommendation_summary_json,
            &snapshot.training_history,
            queue_summary_override.as_deref(),
            "evidence_chain_enrichment",
            "latest",
        )),
        "diagnostic_opinion" => daemon_json_ok(&learning_summary_field_json(
            &snapshot.latest_output_json,
            &snapshot.latest_recommendation_summary_json,
            &snapshot.training_history,
            queue_summary_override.as_deref(),
            "diagnostic_opinion",
            "latest",
        )),
        "handoff" => daemon_json_ok(&handoff_summary_json(
            &snapshot.latest_output_json,
            &snapshot.latest_recommendation_summary_json,
            &snapshot.training_history,
            queue_summary_override.as_deref(),
            "latest",
        )),
        _ => not_found_response(),
    }
}

fn latest_queue_summary_override(snapshot: &DaemonSnapshot) -> Option<String> {
    (!snapshot.target_outputs.is_empty())
        .then(|| queue_summary_json_from_targets(&snapshot.target_outputs))
}

fn target_route_response(snapshot: Option<&DaemonSnapshot>, path: &str) -> String {
    let Some(snapshot) = snapshot else {
        return no_snapshot_response();
    };
    if let Some(target) = target_route_target(snapshot, path, "/output.json") {
        return daemon_json_ok(&target.output_json);
    }
    if let Some(target) = target_route_target(snapshot, path, "/recommendation-summary.json") {
        return daemon_json_ok(&target.recommendation_summary_json);
    }
    if let Some(target) = target_route_target(snapshot, path, "/learning-summary.json") {
        return daemon_json_ok(&learning_summary_json_from_output_and_history_with_scope(
            &target.output_json,
            &target.recommendation_summary_json,
            &target.training_history,
            None,
            "target",
        ));
    }
    if let Some(target) = target_route_target(snapshot, path, "/evidence-chain-enrichment.json") {
        return daemon_json_ok(&learning_summary_field_json(
            &target.output_json,
            &target.recommendation_summary_json,
            &target.training_history,
            None,
            "evidence_chain_enrichment",
            "target",
        ));
    }
    if let Some(target) = target_route_target(snapshot, path, "/diagnostic-opinion.json") {
        return daemon_json_ok(&learning_summary_field_json(
            &target.output_json,
            &target.recommendation_summary_json,
            &target.training_history,
            None,
            "diagnostic_opinion",
            "target",
        ));
    }
    if let Some(target) = target_route_target(snapshot, path, "/handoff-summary.json") {
        return daemon_json_ok(&handoff_summary_json(
            &target.output_json,
            &target.recommendation_summary_json,
            &target.training_history,
            None,
            "target",
        ));
    }
    if let Some(target) = target_route_target(snapshot, path, "/meta.json") {
        return daemon_json_ok(&target_daemon_meta_json(target, snapshot.interval_ms));
    }
    target_not_found_response()
}

fn target_route_target<'a>(
    snapshot: &'a DaemonSnapshot,
    path: &str,
    suffix: &str,
) -> Option<&'a TargetDaemonOutput> {
    let segment = target_route_segment(path, suffix)?;
    snapshot
        .target_outputs
        .iter()
        .find(|target| target.path_segment == segment)
}

fn target_route_segment<'a>(path: &'a str, suffix: &str) -> Option<&'a str> {
    path.strip_prefix("/v1/latest/targets/")?
        .strip_suffix(suffix)
        .map(|segment| segment.trim_end_matches('/'))
}

fn no_snapshot_response() -> String {
    daemon_error_response("HTTP/1.1 503 Service Unavailable", "no_snapshot_available")
}

fn no_trainable_snapshot_response() -> String {
    daemon_error_response("HTTP/1.1 409 Conflict", "no_trainable_snapshot_available")
}

fn target_not_found_response() -> String {
    daemon_error_response("HTTP/1.1 404 Not Found", "target_not_found")
}
