use super::*;

const USAGE: &str = "usage: etragon training-labels | etragon python-memory-info [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon python-memory-model-info [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon python-memory-versions [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon python-memory-snapshot [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon clear-python-memory [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon save-python-memory-slot <slot> [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon load-python-memory-slot <slot> [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon delete-python-memory-slot <slot> [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon import-python-memory <path> [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon protocol-capabilities [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon analyze-json <path|-> | etragon analyze-url <http://host[:port]/path> | etragon analyze-targets-url <http://host[:port]/v1/latest/targets> [--filter <path-segment-prefix>] | etragon analyze-python-json <path|-> [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon analyze-python-targets-url <http://host[:port]/v1/latest/targets> [--filter <path-segment-prefix>] [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon analyze-python-federation-json <path|-> [--filter <path-segment-prefix>] [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon train-python-federation-json <path|-> --label <label> [--filter <path-segment-prefix>] [--weight <n>] [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon watch-python-url <http://host[:port]/path> [--interval-ms <ms>] [--cycles <n>] [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon watch-python-targets-url <http://host[:port]/v1/latest/targets> [--interval-ms <ms>] [--cycles <n>] [--filter <path-segment-prefix>] [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] | etragon serve-python-url <http://host[:port]/path> [--bind <host:port>] [--interval-ms <ms>] [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] [--daemon-state <path>] | etragon serve-python-targets-url <http://host[:port]/v1/latest/targets> [--bind <host:port>] [--filter <path-segment-prefix>] [--interval-ms <ms>] [--python-worker <path>] [--python-bin <bin>] [--python-state <path>] [--daemon-state <path>]";

pub(super) fn analyze_input(input: &str) -> Result<String, String> {
    let output = analyze_gewyvern_analysis_json(&MockMlAdvisoryEngine, input)
        .map_err(|err| format!("failed to analyze gewyvern analysis json: {}", err.message))?;
    Ok(engine_output_json(&output))
}

pub(super) fn analyze_input_with_python_worker(
    input: &str,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    let mut worker = PythonWorkerClient::spawn(config)?;
    worker.analyze_json(input)
}

pub(super) fn train_input_with_python_worker(
    input: &str,
    label: &str,
    weight: f64,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    let mut worker = PythonWorkerClient::spawn(config)?;
    worker.train_json_with_weight(input, label, weight)
}

pub(super) fn analyze_targets_url(url: &str) -> Result<String, String> {
    analyze_targets_url_with_filter(url, None)
}

pub(super) fn analyze_targets_url_with_filter(
    url: &str,
    filter_prefix: Option<&str>,
) -> Result<String, String> {
    let (host, port, path) = parse_http_url(url)?;
    if path != "/v1/latest/targets" {
        return Err("analyze-targets-url expects a /v1/latest/targets endpoint".to_string());
    }
    let targets_json = http_get(&host, port, &path)?;
    let segments = extract_target_path_segments(&targets_json)?;
    let mut entries = Vec::new();
    for segment in segments.into_iter().filter(|segment| {
        filter_prefix
            .map(|prefix| segment.starts_with(prefix))
            .unwrap_or(true)
    }) {
        let analysis_path = format!("/v1/latest/targets/{}/analysis.json", segment);
        let analysis_json = http_get(&host, port, &analysis_path)?;
        let output = analyze_input(&analysis_json)?;
        entries.push((segment, output));
    }
    Ok(batch_output_json(&entries))
}

pub(super) fn analyze_targets_url_with_filter_and_python_worker(
    url: &str,
    filter_prefix: Option<&str>,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    let (host, port, path) = parse_http_url(url)?;
    if path != "/v1/latest/targets" {
        return Err("analyze-targets-url expects a /v1/latest/targets endpoint".to_string());
    }
    let targets_json = http_get(&host, port, &path)?;
    let segments = extract_target_path_segments(&targets_json)?;
    let mut worker = PythonWorkerClient::spawn(config)?;
    let mut entries = Vec::new();
    for segment in segments.into_iter().filter(|segment| {
        filter_prefix
            .map(|prefix| segment.starts_with(prefix))
            .unwrap_or(true)
    }) {
        let analysis_path = format!("/v1/latest/targets/{}/analysis.json", segment);
        let analysis_json = http_get(&host, port, &analysis_path)?;
        let output = worker.analyze_json(&analysis_json)?;
        entries.push((segment, output));
    }
    Ok(batch_output_json(&entries))
}

pub(super) fn train_targets_url_with_filter_and_python_worker(
    url: &str,
    label: &str,
    weight: f64,
    filter_prefix: Option<&str>,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    let (host, port, path) = parse_http_url(url)?;
    if path != "/v1/latest/targets" {
        return Err("train-python-targets-url expects a /v1/latest/targets endpoint".to_string());
    }
    let targets_json = http_get(&host, port, &path)?;
    let segments = extract_target_path_segments(&targets_json)?;
    let mut worker = PythonWorkerClient::spawn(config)?;
    let mut entries = Vec::new();
    for segment in segments.into_iter().filter(|segment| {
        filter_prefix
            .map(|prefix| segment.starts_with(prefix))
            .unwrap_or(true)
    }) {
        let analysis_path = format!("/v1/latest/targets/{}/analysis.json", segment);
        let analysis_json = http_get(&host, port, &analysis_path)?;
        let output = worker.train_json_with_weight(&analysis_json, label, weight)?;
        entries.push((segment, output));
    }
    Ok(target_results_json(&entries))
}

pub(super) fn watch_event_json(cycle: usize, source: &str, url: &str, output: &str) -> String {
    format!(
        "{{\"cycle\":{},\"source\":\"{}\",\"url\":\"{}\",\"output\":{}}}",
        cycle,
        escape_json_string(source),
        escape_json_string(url),
        output
    )
}

pub(super) fn execute_watch_loop<F>(
    cycles: usize,
    interval_ms: u64,
    mut tick: F,
) -> Result<String, String>
where
    F: FnMut(usize) -> Result<String, String>,
{
    let infinite = cycles == 0;
    let mut collected = Vec::new();
    let mut cycle = 0usize;
    loop {
        cycle += 1;
        let event = tick(cycle)?;
        if infinite {
            println!("{}", event);
            io::stdout()
                .flush()
                .map_err(|err| format!("failed to flush watch output: {err}"))?;
        } else {
            collected.push(event);
        }
        if !infinite && cycle >= cycles {
            break;
        }
        thread::sleep(Duration::from_millis(interval_ms));
    }
    Ok(collected.join("\n"))
}

pub(super) fn watch_python_url(
    url: &str,
    interval_ms: u64,
    cycles: usize,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    let mut worker = PythonWorkerClient::spawn(config)?;
    execute_watch_loop(cycles, interval_ms, |cycle| {
        let analysis_json = read_url(url)?;
        let output = worker.analyze_json(&analysis_json)?;
        Ok(watch_event_json(cycle, "python-url", url, &output))
    })
}

pub(super) fn watch_python_targets_url(
    url: &str,
    interval_ms: u64,
    cycles: usize,
    filter_prefix: Option<&str>,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    let mut worker = PythonWorkerClient::spawn(config)?;
    execute_watch_loop(cycles, interval_ms, |cycle| {
        let (host, port, path) = parse_http_url(url)?;
        if path != "/v1/latest/targets" {
            return Err(
                "watch-python-targets-url expects a /v1/latest/targets endpoint".to_string(),
            );
        }
        let targets_json = http_get(&host, port, &path)?;
        let segments = extract_target_path_segments(&targets_json)?;
        let mut entries = Vec::new();
        for segment in segments.into_iter().filter(|segment| {
            filter_prefix
                .map(|prefix| segment.starts_with(prefix))
                .unwrap_or(true)
        }) {
            let analysis_path = format!("/v1/latest/targets/{}/analysis.json", segment);
            let analysis_json = http_get(&host, port, &analysis_path)?;
            let output = worker.analyze_json(&analysis_json)?;
            entries.push((segment, output));
        }
        let batch = batch_output_json(&entries);
        Ok(watch_event_json(cycle, "python-targets-url", url, &batch))
    })
}

pub(super) fn serve_python_url(
    url: &str,
    bind_addr: &str,
    interval_ms: u64,
    config: &PythonWorkerConfig,
    daemon_state_file: Option<&Path>,
) -> Result<String, String> {
    run_python_daemon_until(
        bind_addr,
        interval_ms,
        config,
        daemon_state_file,
        "python-url",
        url,
        |_, worker| {
            let analysis_json = read_url(url)?;
            let output_json = worker.analyze_json(&analysis_json)?;
            Ok(PolledDaemonOutput {
                input_fingerprint: analysis_json.clone(),
                latest_input_json: Some(analysis_json),
                recommendation_summary_json: single_output_recommendation_summary(&output_json),
                output_json,
                target_outputs: Vec::new(),
            })
        },
        Arc::new(AtomicBool::new(false)),
    )?;
    Ok(String::new())
}

pub(super) fn serve_python_targets_url(
    url: &str,
    bind_addr: &str,
    interval_ms: u64,
    filter_prefix: Option<&str>,
    config: &PythonWorkerConfig,
    daemon_state_file: Option<&Path>,
) -> Result<String, String> {
    run_python_daemon_until(
        bind_addr,
        interval_ms,
        config,
        daemon_state_file,
        "python-targets-url",
        url,
        |_, worker| {
            let (host, port, path) = parse_http_url(url)?;
            if path != "/v1/latest/targets" {
                return Err(
                    "serve-python-targets-url expects a /v1/latest/targets endpoint".to_string(),
                );
            }
            let targets_json = http_get(&host, port, &path)?;
            let segments = extract_target_path_segments(&targets_json)?;
            let mut entries = Vec::new();
            let mut target_outputs = Vec::new();
            let mut input_fingerprint = targets_json;
            for segment in segments.into_iter().filter(|segment| {
                filter_prefix
                    .map(|prefix| segment.starts_with(prefix))
                    .unwrap_or(true)
            }) {
                let analysis_path = format!("/v1/latest/targets/{}/analysis.json", segment);
                match http_get(&host, port, &analysis_path) {
                    Ok(analysis_json) => {
                        input_fingerprint.push('\n');
                        input_fingerprint.push_str(&segment);
                        input_fingerprint.push('\n');
                        input_fingerprint.push_str(&analysis_json);
                        match worker.analyze_json(&analysis_json) {
                            Ok(output) => {
                                entries.push((segment.clone(), output.clone()));
                                target_outputs.push(TargetDaemonOutput {
                                    path_segment: segment,
                                    input_json: Some(analysis_json),
                                    recommendation_summary_json:
                                        single_output_recommendation_summary(&output),
                                    output_json: output,
                                    updated_unix_ms: 0,
                                    state_hash: String::new(),
                                    last_success_unix_ms: None,
                                    last_error: None,
                                    training_history: Vec::new(),
                                });
                            }
                            Err(err) => {
                                entries.push((segment.clone(), format!("__error__:{err}")));
                                target_outputs.push(TargetDaemonOutput {
                                    path_segment: segment,
                                    input_json: Some(analysis_json),
                                    recommendation_summary_json: "null".to_string(),
                                    output_json: "null".to_string(),
                                    updated_unix_ms: 0,
                                    state_hash: String::new(),
                                    last_success_unix_ms: None,
                                    last_error: Some(err),
                                    training_history: Vec::new(),
                                });
                            }
                        }
                    }
                    Err(err) => {
                        entries.push((segment.clone(), format!("__error__:{err}")));
                        target_outputs.push(TargetDaemonOutput {
                            path_segment: segment,
                            input_json: None,
                            recommendation_summary_json: "null".to_string(),
                            output_json: "null".to_string(),
                            updated_unix_ms: 0,
                            state_hash: String::new(),
                            last_success_unix_ms: None,
                            last_error: Some(err),
                            training_history: Vec::new(),
                        });
                    }
                }
            }
            let recommendation_summary_json = recommendation_overview_json(&entries);
            let output_json = batch_output_json(&entries);
            Ok(PolledDaemonOutput {
                input_fingerprint,
                latest_input_json: None,
                output_json,
                recommendation_summary_json,
                target_outputs,
            })
        },
        Arc::new(AtomicBool::new(false)),
    )?;
    Ok(String::new())
}

pub(super) fn run_cli(args: &[String]) -> Result<String, String> {
    match args {
        [cmd] if cmd == "training-labels" => Ok(training_labels_json()),
        [cmd, path] if cmd == "analyze-json" => analyze_input(&read_input(path)?),
        [cmd, url] if cmd == "analyze-url" => analyze_input(&read_url(url)?),
        [cmd, url] if cmd == "analyze-targets-url" => analyze_targets_url(url),
        [cmd, url, flag, prefix] if cmd == "analyze-targets-url" && flag == "--filter" => {
            analyze_targets_url_with_filter(url, Some(prefix))
        }
        [cmd, rest @ ..] if cmd == "python-memory-info" => {
            let (config, consumed) = parse_python_worker_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for python-memory-info: {}",
                    rest[consumed]
                ));
            }
            python_memory_info(&config)
        }
        [cmd, rest @ ..] if cmd == "python-memory-model-info" => {
            let (config, consumed) = parse_python_worker_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for python-memory-model-info: {}",
                    rest[consumed]
                ));
            }
            python_memory_model_info(&config)
        }
        [cmd, rest @ ..] if cmd == "python-memory-versions" => {
            let (config, consumed) = parse_python_worker_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for python-memory-versions: {}",
                    rest[consumed]
                ));
            }
            python_memory_versions(&config)
        }
        [cmd, rest @ ..] if cmd == "python-memory-snapshot" => {
            let (config, consumed) = parse_python_worker_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for python-memory-snapshot: {}",
                    rest[consumed]
                ));
            }
            python_memory_snapshot(&config)
        }
        [cmd, slot, rest @ ..] if cmd == "save-python-memory-slot" => {
            let (label, note, source, config, consumed) =
                parse_slot_metadata_and_python_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for save-python-memory-slot: {}",
                    rest[consumed]
                ));
            }
            save_python_memory_slot(
                slot,
                label.as_deref(),
                note.as_deref(),
                source.as_deref(),
                &config,
            )
        }
        [cmd, rest @ ..] if cmd == "protocol-capabilities" => {
            let (config, consumed) = parse_python_worker_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for protocol-capabilities: {}",
                    rest[consumed]
                ));
            }
            protocol_capabilities(&config)
        }
        [cmd, rest @ ..] if cmd == "clear-python-memory" => {
            let (config, consumed) = parse_python_worker_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for clear-python-memory: {}",
                    rest[consumed]
                ));
            }
            clear_python_memory(&config)
        }
        [cmd, path, rest @ ..] if cmd == "import-python-memory" => {
            let memory_snapshot_json = read_input(path)?;
            let (strategy, config, consumed) = parse_memory_strategy_and_python_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for import-python-memory: {}",
                    rest[consumed]
                ));
            }
            import_python_memory(&memory_snapshot_json, &strategy, &config)
        }
        [cmd, slot, rest @ ..] if cmd == "load-python-memory-slot" => {
            let (strategy, config, consumed) = parse_memory_strategy_and_python_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for load-python-memory-slot: {}",
                    rest[consumed]
                ));
            }
            load_python_memory_slot(slot, &strategy, &config)
        }
        [cmd, slot, rest @ ..] if cmd == "delete-python-memory-slot" => {
            let (config, consumed) = parse_python_worker_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for delete-python-memory-slot: {}",
                    rest[consumed]
                ));
            }
            delete_python_memory_slot(slot, &config)
        }
        [cmd, path, rest @ ..] if cmd == "analyze-python-json" => {
            let input = read_input(path)?;
            let (config, consumed) = parse_python_worker_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for analyze-python-json: {}",
                    rest[consumed]
                ));
            }
            analyze_input_with_python_worker(&input, &config)
        }
        [cmd, url, rest @ ..] if cmd == "analyze-python-url" => {
            let input = read_url(url)?;
            let (config, consumed) = parse_python_worker_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for analyze-python-url: {}",
                    rest[consumed]
                ));
            }
            analyze_input_with_python_worker(&input, &config)
        }
        [cmd, path, flag, label, rest @ ..] if cmd == "train-python-json" && flag == "--label" => {
            let input = read_input(path)?;
            let (_filter_prefix, weight, config) = parse_train_options(rest)?;
            let canonical_label = normalize_training_label(label)?;
            train_input_with_python_worker(&input, &canonical_label, weight, &config)
        }
        [cmd, url, flag, label, rest @ ..] if cmd == "train-python-url" && flag == "--label" => {
            let input = read_url(url)?;
            let (_filter_prefix, weight, config) = parse_train_options(rest)?;
            let canonical_label = normalize_training_label(label)?;
            train_input_with_python_worker(&input, &canonical_label, weight, &config)
        }
        [cmd, url, rest @ ..] if cmd == "analyze-python-targets-url" => {
            let (interval_ms, cycles, filter_prefix, config) = parse_watch_options(rest)?;
            if interval_ms != 1000 || cycles != 0 {
                return Err(
                    "analyze-python-targets-url only accepts --filter, --python-worker, --python-bin, and --python-state"
                        .to_string(),
                );
            }
            analyze_targets_url_with_filter_and_python_worker(
                url,
                filter_prefix.as_deref(),
                &config,
            )
        }
        [cmd, url, flag, label, rest @ ..]
            if cmd == "train-python-targets-url" && flag == "--label" =>
        {
            let (filter_prefix, weight, config) = parse_train_options(rest)?;
            let canonical_label = normalize_training_label(label)?;
            train_targets_url_with_filter_and_python_worker(
                url,
                &canonical_label,
                weight,
                filter_prefix.as_deref(),
                &config,
            )
        }
        [cmd, path, rest @ ..] if cmd == "analyze-python-federation-json" => {
            let manifest = read_input(path)?;
            let (interval_ms, cycles, filter_prefix, config) = parse_watch_options(rest)?;
            if interval_ms != 1000 || cycles != 0 {
                return Err(
                    "analyze-python-federation-json only accepts --filter, --python-worker, --python-bin, and --python-state"
                        .to_string(),
                );
            }
            analyze_federation_manifest_with_python_worker(
                &manifest,
                filter_prefix.as_deref(),
                &config,
            )
        }
        [cmd, path, flag, label, rest @ ..]
            if cmd == "train-python-federation-json" && flag == "--label" =>
        {
            let manifest = read_input(path)?;
            let (filter_prefix, weight, config) = parse_train_options(rest)?;
            let canonical_label = normalize_training_label(label)?;
            train_federation_manifest_with_python_worker(
                &manifest,
                &canonical_label,
                weight,
                filter_prefix.as_deref(),
                &config,
            )
        }
        [cmd, url, rest @ ..] if cmd == "watch-python-url" => {
            let (interval_ms, cycles, _filter_prefix, config) = parse_watch_options(rest)?;
            watch_python_url(url, interval_ms, cycles, &config)
        }
        [cmd, url, rest @ ..] if cmd == "watch-python-targets-url" => {
            let (interval_ms, cycles, filter_prefix, config) = parse_watch_options(rest)?;
            watch_python_targets_url(url, interval_ms, cycles, filter_prefix.as_deref(), &config)
        }
        [cmd, url, rest @ ..] if cmd == "serve-python-url" => {
            let (bind_addr, interval_ms, _filter_prefix, config, daemon_state_file) =
                parse_daemon_options(rest)?;
            serve_python_url(
                url,
                &bind_addr,
                interval_ms,
                &config,
                daemon_state_file.as_deref(),
            )
        }
        [cmd, url, rest @ ..] if cmd == "serve-python-targets-url" => {
            let (bind_addr, interval_ms, filter_prefix, config, daemon_state_file) =
                parse_daemon_options(rest)?;
            serve_python_targets_url(
                url,
                &bind_addr,
                interval_ms,
                filter_prefix.as_deref(),
                &config,
                daemon_state_file.as_deref(),
            )
        }
        _ => Err(USAGE.to_string()),
    }
}
