use super::*;

const USAGE: &str = "usage: etragon analyze-json <path|-> [--state <path>] | etragon train-json <path|-> --label <label> [--weight <n>] [--state <path>] | etragon memory-info|memory-model-info|memory-versions|memory-snapshot|clear-memory [--state <path>] | etragon save-memory-slot|load-memory-slot|delete-memory-slot <slot> [options] [--state <path>] | etragon memory-transfer-plan|import-memory <path> [--merge|--replace] [--state <path>] | etragon analyze-url|watch-url|serve-url <url> [options] | etragon analyze-targets-url|train-targets-url|watch-targets-url|serve-targets-url <url> [options] | etragon analyze-federation-json <path> [options] | etragon train-federation-json <path> --label <label> [options] | etragon training-labels | etragon protocol-capabilities; Python compatibility commands: analyze-python-json, train-python-json, python-memory-info, python-memory-model-info, python-protocol-capabilities, watch-python-url, watch-python-targets-url, serve-python-url, serve-python-targets-url";

pub(super) fn analyze_input_with_native_backend(
    input: &str,
    config: NativeLearningConfig,
) -> Result<String, String> {
    NativeLearningBackend::open(config)?.analyze_json(input)
}

pub(super) fn train_input_with_native_backend(
    input: &str,
    label: &str,
    weight: f64,
    config: NativeLearningConfig,
) -> Result<String, String> {
    NativeLearningBackend::open(config)?.train_json_with_weight(input, label, weight)
}

pub(super) fn analyze_input_with_python_worker(
    input: &str,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    with_python_worker(config, |worker| worker.analyze_json(input))
}

pub(super) fn train_input_with_python_worker(
    input: &str,
    label: &str,
    weight: f64,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    with_python_worker(config, |worker| {
        worker.train_json_with_weight(input, label, weight)
    })
}

pub(super) fn analyze_targets_url_with_filter_and_native_backend(
    url: &str,
    filter_prefix: Option<&str>,
    config: NativeLearningConfig,
) -> Result<String, String> {
    let endpoint = resolve_target_batch_endpoint(
        url,
        "analyze-targets-url expects a /v1/latest/targets endpoint",
        filter_prefix,
    )?;
    with_learning_backend(&LearningBackendConfig::Native(config), |backend| {
        let mut entries = Vec::new();
        for segment in endpoint.segments.clone() {
            let analysis_json = endpoint.fetch_analysis_json(&segment)?;
            let output = backend.analyze_json(&analysis_json)?;
            entries.push((segment, output));
        }
        Ok(batch_output_json(&entries))
    })
}

pub(super) fn train_targets_url_with_native_backend(
    url: &str,
    label: &str,
    weight: f64,
    filter_prefix: Option<&str>,
    config: NativeLearningConfig,
) -> Result<String, String> {
    let endpoint = resolve_target_batch_endpoint(
        url,
        "train-targets-url expects a /v1/latest/targets endpoint",
        filter_prefix,
    )?;
    with_learning_backend(&LearningBackendConfig::Native(config), |backend| {
        let mut entries = Vec::new();
        for segment in endpoint.segments.clone() {
            let analysis_json = endpoint.fetch_analysis_json(&segment)?;
            let output = backend.train_json_with_weight(&analysis_json, label, weight)?;
            entries.push((segment, output));
        }
        Ok(target_results_json(&entries))
    })
}

pub(super) fn analyze_targets_url_with_filter_and_python_worker(
    url: &str,
    filter_prefix: Option<&str>,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    let endpoint = resolve_target_batch_endpoint(
        url,
        "analyze-targets-url expects a /v1/latest/targets endpoint",
        filter_prefix,
    )?;
    with_python_worker(config, |worker| {
        let mut entries = Vec::new();
        for segment in endpoint.segments.clone() {
            let analysis_json = endpoint.fetch_analysis_json(&segment)?;
            let output = worker.analyze_json(&analysis_json)?;
            entries.push((segment, output));
        }
        Ok(batch_output_json(&entries))
    })
}

pub(super) fn train_targets_url_with_filter_and_python_worker(
    url: &str,
    label: &str,
    weight: f64,
    filter_prefix: Option<&str>,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    let endpoint = resolve_target_batch_endpoint(
        url,
        "train-python-targets-url expects a /v1/latest/targets endpoint",
        filter_prefix,
    )?;
    with_python_worker(config, |worker| {
        let mut entries = Vec::new();
        for segment in endpoint.segments.clone() {
            let analysis_json = endpoint.fetch_analysis_json(&segment)?;
            let output = worker.train_json_with_weight(&analysis_json, label, weight)?;
            entries.push((segment, output));
        }
        Ok(target_results_json(&entries))
    })
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
    watch_url_with_backend(
        url,
        interval_ms,
        cycles,
        &LearningBackendConfig::Python(config.clone()),
        "python-url",
    )
}

pub(super) fn watch_native_url(
    url: &str,
    interval_ms: u64,
    cycles: usize,
    config: NativeLearningConfig,
) -> Result<String, String> {
    watch_url_with_backend(
        url,
        interval_ms,
        cycles,
        &LearningBackendConfig::Native(config),
        "native-url",
    )
}

fn watch_url_with_backend(
    url: &str,
    interval_ms: u64,
    cycles: usize,
    config: &LearningBackendConfig,
    source: &str,
) -> Result<String, String> {
    with_learning_backend(config, |worker| {
        execute_watch_loop(cycles, interval_ms, |cycle| {
            let analysis_json = read_url(url)?;
            let output = worker.analyze_json(&analysis_json)?;
            Ok(watch_event_json(cycle, source, url, &output))
        })
    })
}

pub(super) fn watch_python_targets_url(
    url: &str,
    interval_ms: u64,
    cycles: usize,
    filter_prefix: Option<&str>,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    watch_targets_url_with_backend(
        url,
        interval_ms,
        cycles,
        filter_prefix,
        &LearningBackendConfig::Python(config.clone()),
        "python-targets-url",
    )
}

pub(super) fn watch_native_targets_url(
    url: &str,
    interval_ms: u64,
    cycles: usize,
    filter_prefix: Option<&str>,
    config: NativeLearningConfig,
) -> Result<String, String> {
    watch_targets_url_with_backend(
        url,
        interval_ms,
        cycles,
        filter_prefix,
        &LearningBackendConfig::Native(config),
        "native-targets-url",
    )
}

fn watch_targets_url_with_backend(
    url: &str,
    interval_ms: u64,
    cycles: usize,
    filter_prefix: Option<&str>,
    config: &LearningBackendConfig,
    source: &str,
) -> Result<String, String> {
    with_learning_backend(config, |worker| {
        execute_watch_loop(cycles, interval_ms, |cycle| {
            let endpoint = resolve_target_batch_endpoint(
                url,
                "watch-targets-url expects a /v1/latest/targets endpoint",
                filter_prefix,
            )?;
            let mut entries = Vec::new();
            for segment in endpoint.segments.clone() {
                let analysis_json = endpoint.fetch_analysis_json(&segment)?;
                let output = worker.analyze_json(&analysis_json)?;
                entries.push((segment, output));
            }
            let batch = batch_output_json(&entries);
            Ok(watch_event_json(cycle, source, url, &batch))
        })
    })
}

pub(super) fn serve_python_url(
    url: &str,
    bind_addr: &str,
    interval_ms: u64,
    config: &PythonWorkerConfig,
    daemon_state_file: Option<&Path>,
) -> Result<String, String> {
    serve_url_with_backend(
        url,
        bind_addr,
        interval_ms,
        daemon_state_file,
        &LearningBackendConfig::Python(config.clone()),
        "python-url",
    )
}

pub(super) fn serve_native_url(
    url: &str,
    bind_addr: &str,
    interval_ms: u64,
    config: NativeLearningConfig,
    daemon_state_file: Option<&Path>,
) -> Result<String, String> {
    serve_url_with_backend(
        url,
        bind_addr,
        interval_ms,
        daemon_state_file,
        &LearningBackendConfig::Native(config),
        "native-url",
    )
}

fn serve_url_with_backend(
    url: &str,
    bind_addr: &str,
    interval_ms: u64,
    daemon_state_file: Option<&Path>,
    config: &LearningBackendConfig,
    source: &str,
) -> Result<String, String> {
    run_learning_daemon_until(
        bind_addr,
        interval_ms,
        config,
        daemon_state_file,
        source,
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
    serve_targets_url_with_backend(
        url,
        bind_addr,
        interval_ms,
        filter_prefix,
        daemon_state_file,
        &LearningBackendConfig::Python(config.clone()),
        "python-targets-url",
    )
}

pub(super) fn serve_native_targets_url(
    url: &str,
    bind_addr: &str,
    interval_ms: u64,
    filter_prefix: Option<&str>,
    config: NativeLearningConfig,
    daemon_state_file: Option<&Path>,
) -> Result<String, String> {
    serve_targets_url_with_backend(
        url,
        bind_addr,
        interval_ms,
        filter_prefix,
        daemon_state_file,
        &LearningBackendConfig::Native(config),
        "native-targets-url",
    )
}

fn serve_targets_url_with_backend(
    url: &str,
    bind_addr: &str,
    interval_ms: u64,
    filter_prefix: Option<&str>,
    daemon_state_file: Option<&Path>,
    config: &LearningBackendConfig,
    source: &str,
) -> Result<String, String> {
    run_learning_daemon_until(
        bind_addr,
        interval_ms,
        config,
        daemon_state_file,
        source,
        url,
        |_, worker| {
            let endpoint = resolve_target_batch_endpoint(
                url,
                "serve-targets-url expects a /v1/latest/targets endpoint",
                filter_prefix,
            )?;
            let segments = endpoint.segments.clone();
            let mut entries = Vec::new();
            let mut target_outputs = Vec::new();
            let mut input_fingerprint = segments.join("\n");
            for segment in segments {
                match endpoint.fetch_analysis_json(&segment) {
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
        [cmd, path, rest @ ..] if cmd == "analyze-json" => {
            let (config, consumed) = parse_native_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for analyze-json: {}",
                    rest[consumed]
                ));
            }
            analyze_input_with_native_backend(&read_input(path)?, config)
        }
        [cmd, path, flag, label, rest @ ..] if cmd == "train-json" && flag == "--label" => {
            let (weight, config) = parse_native_train_options(rest)?;
            let canonical_label = normalize_training_label(label)?;
            train_input_with_native_backend(&read_input(path)?, &canonical_label, weight, config)
        }
        [cmd, url, rest @ ..] if cmd == "analyze-url" => {
            let (config, consumed) = parse_native_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for analyze-url: {}",
                    rest[consumed]
                ));
            }
            analyze_input_with_native_backend(&read_url(url)?, config)
        }
        [cmd, url, rest @ ..] if cmd == "analyze-targets-url" => {
            let (interval_ms, cycles, filter_prefix, config) = parse_native_watch_options(rest)?;
            if interval_ms != 1000 || cycles != 0 {
                return Err("analyze-targets-url only accepts --filter and --state".to_string());
            }
            analyze_targets_url_with_filter_and_native_backend(
                url,
                filter_prefix.as_deref(),
                config,
            )
        }
        [cmd, url, flag, label, rest @ ..] if cmd == "train-targets-url" && flag == "--label" => {
            let (filter_prefix, weight, config) = parse_native_batch_train_options(rest)?;
            let canonical_label = normalize_training_label(label)?;
            train_targets_url_with_native_backend(
                url,
                &canonical_label,
                weight,
                filter_prefix.as_deref(),
                config,
            )
        }
        [cmd, rest @ ..] if cmd == "memory-info" => {
            let (config, consumed) = parse_native_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for memory-info: {}",
                    rest[consumed]
                ));
            }
            native_memory_info(config)
        }
        [cmd, rest @ ..] if cmd == "memory-model-info" => {
            let (config, consumed) = parse_native_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for memory-model-info: {}",
                    rest[consumed]
                ));
            }
            native_memory_model_info(config)
        }
        [cmd, rest @ ..] if cmd == "memory-versions" => {
            let (config, consumed) = parse_native_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for memory-versions: {}",
                    rest[consumed]
                ));
            }
            native_memory_versions(config)
        }
        [cmd, rest @ ..] if cmd == "memory-snapshot" => {
            let (config, consumed) = parse_native_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for memory-snapshot: {}",
                    rest[consumed]
                ));
            }
            native_memory_snapshot(config)
        }
        [cmd, rest @ ..] if cmd == "clear-memory" => {
            let (config, consumed) = parse_native_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for clear-memory: {}",
                    rest[consumed]
                ));
            }
            clear_native_memory(config)
        }
        [cmd, path, rest @ ..] if cmd == "import-memory" => {
            let (strategy, config, consumed) = parse_native_memory_strategy(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for import-memory: {}",
                    rest[consumed]
                ));
            }
            import_native_memory(&read_input(path)?, &strategy, config)
        }
        [cmd, path, rest @ ..] if cmd == "memory-transfer-plan" => {
            let (strategy, config, consumed) = parse_native_memory_strategy(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for memory-transfer-plan: {}",
                    rest[consumed]
                ));
            }
            plan_native_memory_transfer(&read_input(path)?, &strategy, config)
        }
        [cmd, slot, rest @ ..] if cmd == "save-memory-slot" => {
            let (label, note, source, config, consumed) = parse_native_slot_metadata(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for save-memory-slot: {}",
                    rest[consumed]
                ));
            }
            save_native_memory_slot(
                slot,
                label.as_deref(),
                note.as_deref(),
                source.as_deref(),
                config,
            )
        }
        [cmd, slot, rest @ ..] if cmd == "load-memory-slot" => {
            let (strategy, config, consumed) = parse_native_memory_strategy(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for load-memory-slot: {}",
                    rest[consumed]
                ));
            }
            load_native_memory_slot(slot, &strategy, config)
        }
        [cmd, slot, rest @ ..] if cmd == "delete-memory-slot" => {
            let (config, consumed) = parse_native_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for delete-memory-slot: {}",
                    rest[consumed]
                ));
            }
            delete_native_memory_slot(slot, config)
        }
        [cmd, path, rest @ ..] if cmd == "analyze-federation-json" => {
            let manifest = read_input(path)?;
            let (interval_ms, cycles, filter_prefix, config) = parse_native_watch_options(rest)?;
            if interval_ms != 1000 || cycles != 0 {
                return Err("analyze-federation-json only accepts --filter and --state".to_string());
            }
            analyze_federation_manifest_with_backend(
                &manifest,
                filter_prefix.as_deref(),
                &LearningBackendConfig::Native(config),
            )
        }
        [cmd, path, flag, label, rest @ ..]
            if cmd == "train-federation-json" && flag == "--label" =>
        {
            let manifest = read_input(path)?;
            let (filter_prefix, weight, config) = parse_native_batch_train_options(rest)?;
            let canonical_label = normalize_training_label(label)?;
            train_federation_manifest_with_backend(
                &manifest,
                &canonical_label,
                weight,
                filter_prefix.as_deref(),
                &LearningBackendConfig::Native(config),
            )
        }
        [cmd, url, rest @ ..] if cmd == "watch-url" => {
            let (interval_ms, cycles, _filter_prefix, config) = parse_native_watch_options(rest)?;
            watch_native_url(url, interval_ms, cycles, config)
        }
        [cmd, url, rest @ ..] if cmd == "watch-targets-url" => {
            let (interval_ms, cycles, filter_prefix, config) = parse_native_watch_options(rest)?;
            watch_native_targets_url(url, interval_ms, cycles, filter_prefix.as_deref(), config)
        }
        [cmd, url, rest @ ..] if cmd == "serve-url" => {
            let (bind_addr, interval_ms, _filter_prefix, config, daemon_state_file) =
                parse_native_daemon_options(rest)?;
            serve_native_url(
                url,
                &bind_addr,
                interval_ms,
                config,
                daemon_state_file.as_deref(),
            )
        }
        [cmd, url, rest @ ..] if cmd == "serve-targets-url" => {
            let (bind_addr, interval_ms, filter_prefix, config, daemon_state_file) =
                parse_native_daemon_options(rest)?;
            serve_native_targets_url(
                url,
                &bind_addr,
                interval_ms,
                filter_prefix.as_deref(),
                config,
                daemon_state_file.as_deref(),
            )
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
            let (config, consumed) = parse_native_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for protocol-capabilities: {}",
                    rest[consumed]
                ));
            }
            native_protocol_capabilities(config)
        }
        [cmd, rest @ ..] if cmd == "python-protocol-capabilities" => {
            let (config, consumed) = parse_python_worker_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for python-protocol-capabilities: {}",
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
        [cmd, path, rest @ ..] if cmd == "python-memory-transfer-plan" => {
            let memory_snapshot_json = read_input(path)?;
            let (strategy, config, consumed) = parse_memory_strategy_and_python_options(rest)?;
            if consumed != rest.len() {
                return Err(format!(
                    "unknown option for python-memory-transfer-plan: {}",
                    rest[consumed]
                ));
            }
            plan_python_memory_transfer(&memory_snapshot_json, &strategy, &config)
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
