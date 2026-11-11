use super::*;
use etragon::provider::targets::{
    MAX_TARGETS, TargetIndexSource, encoded_target_segment, parse_target_index, stamp_target_output,
};
use std::{collections::BTreeMap, time::Instant};

const MAX_VISITS_PER_CYCLE: usize = 8;
const MAX_AGGREGATE_BYTES: usize = 1024 * 1024;

pub(crate) struct TargetOptions {
    base: ProviderDaemonOptions,
    filter: Option<String>,
    max_targets: usize,
    max_inferences: usize,
}

pub(crate) fn parse_options(args: &[String]) -> Result<TargetOptions, String> {
    let mut common = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut filter = None;
    let mut max_targets = 24;
    let mut max_inferences = 1;
    for pair in args.chunks(2) {
        if pair.len() != 2 || !seen.insert(pair[0].as_str()) {
            return Err("etragon_provider_invalid_options".into());
        }
        match pair[0].as_str() {
            "--filter" => {
                if pair[1].is_empty()
                    || pair[1].len() > 256
                    || pair[1].chars().any(char::is_control)
                {
                    return Err("etragon_provider_invalid_filter".into());
                }
                filter = Some(pair[1].clone());
            }
            "--max-targets" => {
                max_targets = pair[1]
                    .parse()
                    .map_err(|_| "etragon_provider_invalid_target_limit")?
            }
            "--max-inferences-per-cycle" => {
                max_inferences = pair[1]
                    .parse()
                    .map_err(|_| "etragon_provider_invalid_inference_limit")?
            }
            _ => common.extend_from_slice(pair),
        }
    }
    if !(1..=MAX_TARGETS).contains(&max_targets) {
        return Err("etragon_provider_invalid_target_limit".into());
    }
    if !(1..=MAX_VISITS_PER_CYCLE.min(max_targets)).contains(&max_inferences) {
        return Err("etragon_provider_invalid_inference_limit".into());
    }
    Ok(TargetOptions {
        base: parse_provider_daemon_options(&common)?,
        filter,
        max_targets,
        max_inferences,
    })
}

fn capabilities(options: &TargetOptions) -> Value {
    let mut value = provider_capabilities_value(&options.base.config);
    value["mode"] = json!("external-provider-targets");
    value["multi_target_supported"] = json!(true);
    value["cache"]["entries"] = json!(options.max_targets);
    value["cache"]["scope"] = json!("process-startup-config-and-target-incarnation");
    value["limits"] = json!({"max_targets":options.max_targets,"max_target_visits_per_cycle":MAX_VISITS_PER_CYCLE,"max_inferences_per_cycle":options.max_inferences,"max_aggregate_bytes":MAX_AGGREGATE_BYTES});
    value["daemon_routes"] = json!([
        "/health",
        "/v1/protocol-capabilities.json",
        "/v1/latest/status",
        "/v1/latest/meta",
        "/v1/latest/output.json",
        "/v1/latest/targets",
        "/v1/latest/targets/<encoded-segment>/meta.json",
        "/v1/latest/targets/<encoded-segment>/output.json"
    ]);
    value
}

#[derive(Clone)]
struct TargetRecord {
    incarnation: u64,
    state: SharedSnapshot,
}

#[derive(Default)]
struct IndexState {
    cycle: u64,
    analysis_runs: u64,
    cache_hits: u64,
    last_cycle_visits: usize,
    last_cycle_inferences: usize,
    status: &'static str,
    last_error: Option<String>,
    checked_unix_ms: Option<u128>,
    retry_delay_ms: u64,
    targets: BTreeMap<String, TargetRecord>,
}

type SharedIndex = Arc<Mutex<IndexState>>;

struct TargetWorker {
    record: TargetRecord,
    poller: ProviderPoller,
    due: Instant,
}

#[derive(Default)]
struct TargetPoller {
    workers: BTreeMap<String, TargetWorker>,
    cursor: Option<String>,
    next_incarnation: u64,
    index_failures: u32,
}

impl TargetPoller {
    fn poll(
        &mut self,
        state: &SharedIndex,
        options: &TargetOptions,
        stop: &AtomicBool,
        mut index: impl FnMut() -> Result<String, ProviderError>,
        mut fetch: impl FnMut(&str) -> Result<String, ProviderError>,
        mut infer: impl FnMut(&str) -> Result<etragon::EngineOutput, ProviderError>,
    ) -> Result<u64, String> {
        {
            let mut view = state
                .lock()
                .map_err(|_| "etragon_provider_state_unavailable")?;
            view.cycle = view.cycle.saturating_add(1);
            view.status = "checking";
            view.last_cycle_visits = 0;
            view.last_cycle_inferences = 0;
        }
        let names = match index().and_then(|input| {
            parse_target_index(&input, options.filter.as_deref(), options.max_targets)
        }) {
            Ok(names) => names,
            Err(error) => return self.index_failed(state, options, error),
        };
        self.index_failures = 0;
        self.workers
            .retain(|name, _| names.binary_search(name).is_ok());
        for name in &names {
            if !self.workers.contains_key(name) {
                self.next_incarnation = self
                    .next_incarnation
                    .checked_add(1)
                    .ok_or("etragon_provider_incarnation_limit")?;
                self.workers.insert(
                    name.clone(),
                    TargetWorker {
                        record: TargetRecord {
                            incarnation: self.next_incarnation,
                            state: Arc::new(Mutex::new(ProviderSnapshot {
                                status: "starting",
                                ..Default::default()
                            })),
                        },
                        poller: ProviderPoller { cached: None },
                        due: Instant::now(),
                    },
                );
            }
        }
        {
            let mut view = state
                .lock()
                .map_err(|_| "etragon_provider_state_unavailable")?;
            view.targets = self
                .workers
                .iter()
                .map(|(name, worker)| (name.clone(), worker.record.clone()))
                .collect();
            view.status = "ready";
            view.last_error = None;
            view.checked_unix_ms = Some(now_unix_ms()?);
            view.retry_delay_ms = 0;
        }
        let start = self
            .cursor
            .as_ref()
            .map(|cursor| names.partition_point(|name| name <= cursor))
            .unwrap_or(0);
        let order = names[start..].iter().chain(names[..start].iter());
        let mut visits = 0;
        let mut calls = 0;
        let mut last_visited = None;
        let mut last_inferred = None;
        for name in order {
            if stop.load(Ordering::Relaxed) || visits == MAX_VISITS_PER_CYCLE {
                break;
            }
            let worker = self
                .workers
                .get_mut(name)
                .ok_or("etragon_provider_target_missing")?;
            if Instant::now() < worker.due {
                continue;
            }
            visits += 1;
            state
                .lock()
                .map_err(|_| "etragon_provider_state_unavailable")?
                .last_cycle_visits = visits;
            last_visited = Some(name.clone());
            let mut called = false;
            let incarnation = worker.record.incarnation;
            let delay = worker.poller.poll_with_budget(
                &worker.record.state,
                options.base.interval_ms,
                calls < options.max_inferences,
                || fetch(name),
                |input| {
                    called = true;
                    {
                        let mut view = state.lock().map_err(|_| ProviderError::Transport)?;
                        view.analysis_runs = view.analysis_runs.saturating_add(1);
                        view.last_cycle_inferences += 1;
                    }
                    let mut output = infer(input)?;
                    stamp_target_output(&mut output, name, Some(incarnation))?;
                    Ok(output)
                },
            )?;
            worker.due = Instant::now() + Duration::from_millis(delay);
            let cache_hit = !called
                && worker
                    .record
                    .state
                    .lock()
                    .map_err(|_| "etragon_provider_state_unavailable")?
                    .status
                    == "ready";
            if called {
                calls += 1;
                last_inferred = Some(name.clone());
            } else if cache_hit {
                let mut view = state
                    .lock()
                    .map_err(|_| "etragon_provider_state_unavailable")?;
                view.cache_hits = view.cache_hits.saturating_add(1);
            }
        }
        // Budget-deferred targets must lead the next round, even if later visits hit cache.
        if let Some(cursor) = last_inferred.or(last_visited) {
            self.cursor = Some(cursor);
        }
        Ok(options.base.interval_ms)
    }

    fn index_failed(
        &mut self,
        state: &SharedIndex,
        options: &TargetOptions,
        error: ProviderError,
    ) -> Result<u64, String> {
        self.workers.clear();
        self.index_failures = self.index_failures.saturating_add(1);
        let delay = options
            .base
            .interval_ms
            .max(1000)
            .saturating_mul(1u64 << self.index_failures.saturating_sub(1).min(6))
            .min(60_000)
            .max(options.base.interval_ms);
        let message = format!("etragon_target_index: {error}");
        let mut view = state
            .lock()
            .map_err(|_| "etragon_provider_state_unavailable")?;
        view.status = "error";
        view.last_error = Some(message.clone());
        view.retry_delay_ms = delay;
        for record in view.targets.values() {
            update(&record.state, |state| {
                state.status = "index-unavailable";
                state.output = None;
                state.last_error = Some(message.clone());
            })?;
        }
        Ok(delay)
    }
}

fn target_view(
    name: &str,
    record: &TargetRecord,
    index_available: bool,
    options: &TargetOptions,
    include_output: bool,
) -> Result<Value, String> {
    let snapshot = record
        .state
        .lock()
        .map_err(|_| "etragon_provider_state_unavailable")?;
    let freshness_ms = options.base.interval_ms.saturating_mul(2).max(60_000);
    let stale = snapshot.status == "ready"
        && snapshot
            .source_checked_at
            .is_none_or(|checked| checked.elapsed() > Duration::from_millis(freshness_ms));
    let mut value = snapshot.json(
        &options.base.config,
        options.base.interval_ms,
        include_output,
    );
    value["source"] = json!("external-provider-targets");
    value["mode"] = json!("external-provider-targets");
    value["path_segment"] = json!(name);
    value["target_incarnation"] = json!(record.incarnation);
    value["stale_after_ms"] = json!(freshness_ms);
    value["stale"] = json!(stale);
    let encoded = encoded_target_segment(name).map_err(|_| "etragon_provider_invalid_target")?;
    value["url_path"] = json!(format!("/v1/latest/targets/{encoded}/output.json"));
    value["meta_url_path"] = json!(format!("/v1/latest/targets/{encoded}/meta.json"));
    if !index_available || stale {
        value["status"] = json!(if !index_available {
            "index-unavailable"
        } else {
            "stale"
        });
        if include_output {
            value["output"] = Value::Null;
        }
    }
    Ok(value)
}

fn index_view(
    state: &IndexState,
    options: &TargetOptions,
    include_output: bool,
) -> Result<Value, String> {
    let targets = state
        .targets
        .iter()
        .map(|(name, record)| {
            target_view(
                name,
                record,
                state.status == "ready",
                options,
                include_output,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let ready = targets
        .iter()
        .filter(|target| target["status"] == "ready")
        .count();
    let status = if state.status != "ready" {
        state.status
    } else if ready == targets.len() {
        "ready"
    } else {
        "partial"
    };
    Ok(json!({
        "mode":"external-provider-targets", "status":status, "index_status":state.status,
        "provider_id":options.base.config.id, "model":options.base.config.model,
        "cycle":state.cycle, "target_count":targets.len(), "ready_count":ready,
        "analysis_runs":state.analysis_runs, "cache_hits":state.cache_hits,
        "last_cycle_target_visits":state.last_cycle_visits, "last_cycle_inferences":state.last_cycle_inferences,
        "index_checked_unix_ms":state.checked_unix_ms, "last_error":state.last_error,
        "retry_delay_ms":state.retry_delay_ms, "targets":targets,
        "operator_review_required":true, "advisory_only":true,
    }))
}

fn route(
    request: &str,
    peer: IpAddr,
    policy: &DaemonAccessPolicy,
    state: &SharedIndex,
    options: &TargetOptions,
) -> Result<String, String> {
    let path = match provider_read_path(request, peer, policy) {
        Ok(path) => path,
        Err(response) => return Ok(response),
    };
    match path {
        "/health" => Ok(daemon_json_ok(
            "{\"status\":\"ok\",\"mode\":\"external-provider-targets\"}",
        )),
        "/v1/protocol-capabilities.json" => {
            Ok(provider_json_response(&capabilities(options), true))
        }
        "/v1/latest/status"
        | "/v1/latest/meta"
        | "/v1/latest/targets"
        | "/v1/latest/output.json" => {
            let state = state
                .lock()
                .map_err(|_| "etragon_provider_state_unavailable")?;
            let include_output = path == "/v1/latest/output.json";
            let body = index_view(&state, options, include_output)?;
            if include_output && body.to_string().len() > MAX_AGGREGATE_BYTES {
                return Ok(daemon_error_response(
                    "HTTP/1.1 413 Payload Too Large",
                    "etragon_provider_use_target_output_routes",
                ));
            }
            Ok(provider_json_response(
                &body,
                !include_output || body["status"] == "ready",
            ))
        }
        _ => {
            let state = state
                .lock()
                .map_err(|_| "etragon_provider_state_unavailable")?;
            for (name, record) in &state.targets {
                let encoded =
                    encoded_target_segment(name).map_err(|_| "etragon_provider_invalid_target")?;
                let include_output = path == format!("/v1/latest/targets/{encoded}/output.json");
                if include_output || path == format!("/v1/latest/targets/{encoded}/meta.json") {
                    let body = target_view(
                        name,
                        record,
                        state.status == "ready",
                        options,
                        include_output,
                    )?;
                    return Ok(provider_json_response(
                        &body,
                        !include_output || body["status"] == "ready",
                    ));
                }
            }
            Ok(daemon_error_response(
                "HTTP/1.1 404 Not Found",
                "etragon_provider_target_not_found",
            ))
        }
    }
}

pub(crate) fn serve(url: &str, options: TargetOptions) -> Result<String, String> {
    let source = TargetIndexSource::new(url.into(), options.base.source_ca.clone())
        .map_err(|error| error.to_string())?;
    let provider = ChatCompletionsProvider::new(options.base.config.clone())
        .map_err(|error| error.to_string())?;
    let policy = daemon_access_policy_from_env();
    if !options.base.bind.ip().is_loopback() && policy.admin_token.is_none() {
        return Err("etragon_provider_remote_bind_requires_admin_token".into());
    }
    let listener =
        TcpListener::bind(options.base.bind).map_err(|_| "etragon_provider_bind_failed")?;
    let state = Arc::new(Mutex::new(IndexState {
        status: "starting",
        ..Default::default()
    }));
    let stop = Arc::new(AtomicBool::new(false));
    let server_state = Arc::clone(&state);
    let server_options = &options;
    run_provider_service_until(
        listener,
        &stop,
        move |stream, peer| {
            handle_provider_request(stream, |request| {
                route(request, peer, &policy, &server_state, server_options)
            })
        },
        || {
            let mut poller = TargetPoller::default();
            while !stop.load(Ordering::Relaxed) {
                let delay = poller.poll(
                    &state,
                    &options,
                    &stop,
                    || source.fetch_index(),
                    |name| source.fetch_target(name),
                    |input| provider.analyze_json(input),
                )?;
                wait_provider_poll(delay, &stop);
            }
            Ok(())
        },
    )?;
    Ok(String::new())
}

#[cfg(test)]
mod tests;
