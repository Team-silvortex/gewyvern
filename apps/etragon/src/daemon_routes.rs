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
            let response = daemon_http_response(
                "HTTP/1.1 413 Payload Too Large",
                "{\"error\":\"daemon_request_too_large\"}",
            );
            stream
                .write_all(response.as_bytes())
                .map_err(|err| format!("failed to write daemon request limit response: {err}"))?;
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
        "/v1/protocol-capabilities.json" => match protocol_capabilities_json(config) {
            Ok(capabilities) => daemon_json_ok(&capabilities),
            Err(err) => daemon_http_response(
                "HTTP/1.1 502 Bad Gateway",
                &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
            ),
        },
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
        "/v1/latest/meta" => match python_worker_memory_state_json(config, snapshot.as_ref()) {
            Ok(memory_state) => {
                daemon_json_ok(&daemon_meta_json(snapshot.as_ref(), Some(&memory_state)))
            }
            Err(_) => daemon_json_ok(&daemon_meta_json(snapshot.as_ref(), None)),
        },
        "/v1/latest/recommendation-summary.json" => match snapshot {
            Some(snapshot) => daemon_json_ok(&snapshot.latest_recommendation_summary_json),
            None => no_snapshot_response(),
        },
        "/v1/latest/federation-summary.json" => match snapshot {
            Some(snapshot) => daemon_json_ok(&federation_summary_json_from_snapshot(&snapshot)),
            None => no_snapshot_response(),
        },
        "/v1/latest/learning-summary.json" => match snapshot {
            Some(snapshot) => {
                let queue_summary_override = if snapshot.target_outputs.is_empty() {
                    None
                } else {
                    Some(queue_summary_json_from_targets(&snapshot.target_outputs))
                };
                daemon_json_ok(&learning_summary_json_from_output_and_history(
                    &snapshot.latest_output_json,
                    &snapshot.latest_recommendation_summary_json,
                    &snapshot.training_history,
                    queue_summary_override.as_deref(),
                ))
            }
            None => no_snapshot_response(),
        },
        "/v1/latest/evidence-chain-enrichment.json" => match snapshot {
            Some(snapshot) => {
                let queue_summary_override = if snapshot.target_outputs.is_empty() {
                    None
                } else {
                    Some(queue_summary_json_from_targets(&snapshot.target_outputs))
                };
                daemon_json_ok(&learning_summary_field_json(
                    &snapshot.latest_output_json,
                    &snapshot.latest_recommendation_summary_json,
                    &snapshot.training_history,
                    queue_summary_override.as_deref(),
                    "evidence_chain_enrichment",
                    "latest",
                ))
            }
            None => no_snapshot_response(),
        },
        "/v1/latest/diagnostic-opinion.json" => match snapshot {
            Some(snapshot) => {
                let queue_summary_override = if snapshot.target_outputs.is_empty() {
                    None
                } else {
                    Some(queue_summary_json_from_targets(&snapshot.target_outputs))
                };
                daemon_json_ok(&learning_summary_field_json(
                    &snapshot.latest_output_json,
                    &snapshot.latest_recommendation_summary_json,
                    &snapshot.training_history,
                    queue_summary_override.as_deref(),
                    "diagnostic_opinion",
                    "latest",
                ))
            }
            None => no_snapshot_response(),
        },
        "/v1/latest/handoff-summary.json" => match snapshot {
            Some(snapshot) => {
                let queue_summary_override = if snapshot.target_outputs.is_empty() {
                    None
                } else {
                    Some(queue_summary_json_from_targets(&snapshot.target_outputs))
                };
                daemon_json_ok(&handoff_summary_json(
                    &snapshot.latest_output_json,
                    &snapshot.latest_recommendation_summary_json,
                    &snapshot.training_history,
                    queue_summary_override.as_deref(),
                    "latest",
                ))
            }
            None => no_snapshot_response(),
        },
        "/v1/latest/output.json" => match snapshot {
            Some(snapshot) => daemon_json_ok(&daemon_snapshot_json(&snapshot)),
            None => no_snapshot_response(),
        },
        "/v1/train/latest" if method == "POST" => match snapshot {
            Some(snapshot) => match (
                &snapshot.latest_input_json,
                parse_training_feedback(&request_text),
            ) {
                (Some(input_json), Ok((label, weight))) => {
                    match train_and_refresh_daemon_input(input_json, &label, weight, config) {
                        Ok(refresh) => {
                            let mut snapshot_to_persist = None;
                            if let Ok(mut guard) = latest.lock() {
                                if let Some(snapshot) = guard.as_mut() {
                                    let trained_unix_ms =
                                        now_unix_ms().unwrap_or(snapshot.updated_unix_ms);
                                    push_training_event(
                                        &mut snapshot.training_history,
                                        TrainingEvent {
                                            label: label.clone(),
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
                                    snapshot.latest_recommendation_summary_json =
                                        refresh.recommendation_summary_json.clone();
                                    snapshot.state_hash = state_hash_for_output(
                                        &snapshot.latest_output_json,
                                        &snapshot.latest_recommendation_summary_json,
                                    );
                                    snapshot_to_persist = Some(snapshot.clone());
                                }
                            }
                            if let (Some(path), Some(snapshot)) =
                                (daemon_state_file, snapshot_to_persist.as_ref())
                            {
                                write_daemon_state(path, snapshot)?;
                            }
                            invalidation_epoch.fetch_add(1, Ordering::Relaxed);
                            daemon_json_ok(&refresh.training_json)
                        }
                        Err(err) => daemon_http_response(
                            "HTTP/1.1 502 Bad Gateway",
                            &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
                        ),
                    }
                }
                (None, _) => daemon_http_response(
                    "HTTP/1.1 409 Conflict",
                    "{\"error\":\"no_trainable_snapshot_available\"}",
                ),
                (_, Err(err)) => daemon_http_response(
                    "HTTP/1.1 400 Bad Request",
                    &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
                ),
            },
            None => no_snapshot_response(),
        },
        "/v1/latest/targets" => match snapshot {
            Some(snapshot) => daemon_json_ok(&daemon_target_index_json(&snapshot)),
            None => no_snapshot_response(),
        },
        _ if path.starts_with("/v1/latest/targets/") && path.ends_with("/output.json") => {
            match snapshot {
                Some(snapshot) => {
                    let segment = path
                        .trim_start_matches("/v1/latest/targets/")
                        .trim_end_matches("/output.json")
                        .trim_end_matches('/');
                    match snapshot
                        .target_outputs
                        .iter()
                        .find(|target| target.path_segment == segment)
                    {
                        Some(target) => daemon_json_ok(&target.output_json),
                        None => target_not_found_response(),
                    }
                }
                None => no_snapshot_response(),
            }
        }
        _ if path.starts_with("/v1/latest/targets/") && path.ends_with("/meta.json") => {
            match snapshot {
                Some(snapshot) => {
                    let segment = path
                        .trim_start_matches("/v1/latest/targets/")
                        .trim_end_matches("/meta.json")
                        .trim_end_matches('/');
                    match snapshot
                        .target_outputs
                        .iter()
                        .find(|target| target.path_segment == segment)
                    {
                        Some(target) => {
                            daemon_json_ok(&target_daemon_meta_json(target, snapshot.interval_ms))
                        }
                        None => target_not_found_response(),
                    }
                }
                None => no_snapshot_response(),
            }
        }
        _ if path.starts_with("/v1/latest/targets/")
            && path.ends_with("/recommendation-summary.json") =>
        {
            match snapshot {
                Some(snapshot) => {
                    let segment = path
                        .trim_start_matches("/v1/latest/targets/")
                        .trim_end_matches("/recommendation-summary.json")
                        .trim_end_matches('/');
                    match snapshot
                        .target_outputs
                        .iter()
                        .find(|target| target.path_segment == segment)
                    {
                        Some(target) => daemon_json_ok(&target.recommendation_summary_json),
                        None => target_not_found_response(),
                    }
                }
                None => no_snapshot_response(),
            }
        }
        _ if path.starts_with("/v1/latest/targets/")
            && path.ends_with("/evidence-chain-enrichment.json") =>
        {
            match snapshot {
                Some(snapshot) => {
                    let segment = path
                        .trim_start_matches("/v1/latest/targets/")
                        .trim_end_matches("/evidence-chain-enrichment.json")
                        .trim_end_matches('/');
                    match snapshot
                        .target_outputs
                        .iter()
                        .find(|target| target.path_segment == segment)
                    {
                        Some(target) => daemon_json_ok(&learning_summary_field_json(
                            &target.output_json,
                            &target.recommendation_summary_json,
                            &target.training_history,
                            None,
                            "evidence_chain_enrichment",
                            "target",
                        )),
                        None => target_not_found_response(),
                    }
                }
                None => no_snapshot_response(),
            }
        }
        _ if path.starts_with("/v1/latest/targets/")
            && path.ends_with("/diagnostic-opinion.json") =>
        {
            match snapshot {
                Some(snapshot) => {
                    let segment = path
                        .trim_start_matches("/v1/latest/targets/")
                        .trim_end_matches("/diagnostic-opinion.json")
                        .trim_end_matches('/');
                    match snapshot
                        .target_outputs
                        .iter()
                        .find(|target| target.path_segment == segment)
                    {
                        Some(target) => daemon_json_ok(&learning_summary_field_json(
                            &target.output_json,
                            &target.recommendation_summary_json,
                            &target.training_history,
                            None,
                            "diagnostic_opinion",
                            "target",
                        )),
                        None => target_not_found_response(),
                    }
                }
                None => no_snapshot_response(),
            }
        }
        _ if path.starts_with("/v1/latest/targets/") && path.ends_with("/handoff-summary.json") => {
            match snapshot {
                Some(snapshot) => {
                    let segment = path
                        .trim_start_matches("/v1/latest/targets/")
                        .trim_end_matches("/handoff-summary.json")
                        .trim_end_matches('/');
                    match snapshot
                        .target_outputs
                        .iter()
                        .find(|target| target.path_segment == segment)
                    {
                        Some(target) => daemon_json_ok(&handoff_summary_json(
                            &target.output_json,
                            &target.recommendation_summary_json,
                            &target.training_history,
                            None,
                            "target",
                        )),
                        None => target_not_found_response(),
                    }
                }
                None => no_snapshot_response(),
            }
        }
        _ if path.starts_with("/v1/latest/targets/")
            && path.ends_with("/learning-summary.json") =>
        {
            match snapshot {
                Some(snapshot) => {
                    let segment = path
                        .trim_start_matches("/v1/latest/targets/")
                        .trim_end_matches("/learning-summary.json")
                        .trim_end_matches('/');
                    match snapshot
                        .target_outputs
                        .iter()
                        .find(|target| target.path_segment == segment)
                    {
                        Some(target) => daemon_json_ok(
                            &learning_summary_json_from_output_and_history_with_scope(
                                &target.output_json,
                                &target.recommendation_summary_json,
                                &target.training_history,
                                None,
                                "target",
                            ),
                        ),
                        None => target_not_found_response(),
                    }
                }
                None => no_snapshot_response(),
            }
        }
        _ if method == "POST" && path.starts_with("/v1/train/targets/") => match snapshot {
            Some(snapshot) => {
                let segment = path
                    .trim_start_matches("/v1/train/targets/")
                    .trim_end_matches('/');
                match snapshot
                    .target_outputs
                    .iter()
                    .find(|target| target.path_segment == segment)
                {
                    Some(target) => {
                        match (&target.input_json, parse_training_feedback(&request_text)) {
                            (Some(input_json), Ok((label, weight))) => {
                                match train_and_refresh_daemon_input(
                                    input_json, &label, weight, config,
                                ) {
                                    Ok(refresh) => {
                                        let mut snapshot_to_persist = None;
                                        if let Ok(mut guard) = latest.lock() {
                                            if let Some(snapshot) = guard.as_mut() {
                                                let trained_unix_ms = now_unix_ms()
                                                    .unwrap_or(snapshot.updated_unix_ms);
                                                push_training_event(
                                                    &mut snapshot.training_history,
                                                    TrainingEvent {
                                                        label: label.clone(),
                                                        weight: format!("{}", weight),
                                                        trained_unix_ms,
                                                        scope: format!("target:{}", segment),
                                                    },
                                                );
                                                if let Some(target) = snapshot
                                                    .target_outputs
                                                    .iter_mut()
                                                    .find(|target| target.path_segment == segment)
                                                {
                                                    target.output_json =
                                                        refresh.analysis_json.clone();
                                                    target.recommendation_summary_json =
                                                        refresh.recommendation_summary_json.clone();
                                                    target.updated_unix_ms = trained_unix_ms;
                                                    target.last_success_unix_ms =
                                                        Some(trained_unix_ms);
                                                    target.last_error = None;
                                                    target.state_hash = state_hash_for_output(
                                                        &target.output_json,
                                                        &target.recommendation_summary_json,
                                                    );
                                                    push_training_event(
                                                        &mut target.training_history,
                                                        TrainingEvent {
                                                            label: label.clone(),
                                                            weight: format!("{}", weight),
                                                            trained_unix_ms,
                                                            scope: "target".to_string(),
                                                        },
                                                    );
                                                }
                                                let target_entries = snapshot
                                                    .target_outputs
                                                    .iter()
                                                    .map(|target| {
                                                        (
                                                            target.path_segment.clone(),
                                                            target.output_json.clone(),
                                                        )
                                                    })
                                                    .collect::<Vec<_>>();
                                                snapshot.analysis_runs += 1;
                                                snapshot.target_count =
                                                    snapshot.target_outputs.len();
                                                snapshot.updated_unix_ms = trained_unix_ms;
                                                snapshot.last_success_unix_ms =
                                                    Some(trained_unix_ms);
                                                snapshot.last_error = None;
                                                snapshot.latest_recommendation_summary_json =
                                                    recommendation_overview_json(&target_entries);
                                                snapshot.state_hash = state_hash_for_output(
                                                    &snapshot.latest_output_json,
                                                    &snapshot.latest_recommendation_summary_json,
                                                );
                                                snapshot_to_persist = Some(snapshot.clone());
                                            }
                                        }
                                        if let (Some(path), Some(snapshot)) =
                                            (daemon_state_file, snapshot_to_persist.as_ref())
                                        {
                                            write_daemon_state(path, snapshot)?;
                                        }
                                        invalidation_epoch.fetch_add(1, Ordering::Relaxed);
                                        daemon_json_ok(&refresh.training_json)
                                    }
                                    Err(err) => daemon_http_response(
                                        "HTTP/1.1 502 Bad Gateway",
                                        &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
                                    ),
                                }
                            }
                            (None, _) => daemon_http_response(
                                "HTTP/1.1 409 Conflict",
                                "{\"error\":\"no_trainable_snapshot_available\"}",
                            ),
                            (_, Err(err)) => daemon_http_response(
                                "HTTP/1.1 400 Bad Request",
                                &format!("{{\"error\":\"{}\"}}", escape_json_string(&err)),
                            ),
                        }
                    }
                    None => target_not_found_response(),
                }
            }
            None => no_snapshot_response(),
        },
        _ => daemon_http_response("HTTP/1.1 404 Not Found", "{\"error\":\"not_found\"}"),
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

fn daemon_json_ok(body: &str) -> String {
    daemon_http_response("HTTP/1.1 200 OK", body)
}

fn no_snapshot_response() -> String {
    daemon_http_response(
        "HTTP/1.1 503 Service Unavailable",
        "{\"error\":\"no_snapshot_available\"}",
    )
}

fn target_not_found_response() -> String {
    daemon_http_response("HTTP/1.1 404 Not Found", "{\"error\":\"target_not_found\"}")
}
