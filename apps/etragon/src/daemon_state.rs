use super::*;

pub(super) const DAEMON_STATE_FILE_LIMIT: usize = 1024 * 1024;
static DAEMON_STATE_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(super) fn push_training_event(history: &mut Vec<TrainingEvent>, event: TrainingEvent) {
    history.push(event);
    if history.len() > TRAINING_HISTORY_LIMIT {
        let overflow = history.len() - TRAINING_HISTORY_LIMIT;
        history.drain(0..overflow);
    }
}

pub(super) fn training_event_json(event: &TrainingEvent) -> String {
    format!(
        "{{\"label\":\"{}\",\"weight\":{},\"trained_unix_ms\":{},\"scope\":\"{}\"}}",
        escape_json_string(&event.label),
        event.weight,
        event.trained_unix_ms,
        escape_json_string(&event.scope)
    )
}

pub(super) fn training_history_json(history: &[TrainingEvent]) -> String {
    let body = history
        .iter()
        .map(training_event_json)
        .collect::<Vec<_>>()
        .join(",");
    format!("[{}]", body)
}

pub(super) fn compacted_training_history(history: &[TrainingEvent]) -> Vec<TrainingEvent> {
    if history.len() <= TRAINING_HISTORY_LIMIT {
        return history.to_vec();
    }
    history[history.len() - TRAINING_HISTORY_LIMIT..].to_vec()
}

pub(super) fn recent_label_activity_json(history: &[TrainingEvent]) -> String {
    #[derive(Clone)]
    struct LabelActivity {
        label: String,
        event_count: usize,
        total_weight: f64,
        last_trained_unix_ms: u128,
        scopes: Vec<String>,
    }

    let mut activities: Vec<LabelActivity> = Vec::new();
    for event in history {
        let weight = event.weight.parse::<f64>().unwrap_or(1.0);
        if let Some(existing) = activities.iter_mut().find(|item| item.label == event.label) {
            existing.event_count += 1;
            existing.total_weight += weight;
            existing.last_trained_unix_ms =
                existing.last_trained_unix_ms.max(event.trained_unix_ms);
            if !existing.scopes.iter().any(|scope| scope == &event.scope) {
                existing.scopes.push(event.scope.clone());
            }
        } else {
            activities.push(LabelActivity {
                label: event.label.clone(),
                event_count: 1,
                total_weight: weight,
                last_trained_unix_ms: event.trained_unix_ms,
                scopes: vec![event.scope.clone()],
            });
        }
    }

    activities.sort_by(|left, right| {
        right
            .last_trained_unix_ms
            .cmp(&left.last_trained_unix_ms)
            .then(right.event_count.cmp(&left.event_count))
            .then_with(|| right.label.cmp(&left.label))
    });

    let body = activities
        .into_iter()
        .map(|activity| {
            let scopes = activity
                .scopes
                .iter()
                .map(|scope| format!("\"{}\"", escape_json_string(scope)))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"label\":\"{}\",\"event_count\":{},\"total_weight\":{},\"last_trained_unix_ms\":{},\"scopes\":[{}]}}",
                escape_json_string(&activity.label),
                activity.event_count,
                activity.total_weight,
                activity.last_trained_unix_ms,
                scopes
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{}]", body)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DaemonSnapshot {
    pub(super) source: String,
    pub(super) upstream_url: String,
    pub(super) interval_ms: u64,
    pub(super) cycle: usize,
    pub(super) analysis_runs: usize,
    pub(super) cache_hits: usize,
    pub(super) target_count: usize,
    pub(super) updated_unix_ms: u128,
    pub(super) state_hash: String,
    pub(super) latest_output_json: String,
    pub(super) latest_input_json: Option<String>,
    pub(super) latest_recommendation_summary_json: String,
    pub(super) target_outputs: Vec<TargetDaemonOutput>,
    pub(super) last_success_unix_ms: Option<u128>,
    pub(super) last_error: Option<String>,
    pub(super) training_history: Vec<TrainingEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TargetDaemonOutput {
    pub(super) path_segment: String,
    pub(super) output_json: String,
    pub(super) input_json: Option<String>,
    pub(super) recommendation_summary_json: String,
    pub(super) updated_unix_ms: u128,
    pub(super) state_hash: String,
    pub(super) last_success_unix_ms: Option<u128>,
    pub(super) last_error: Option<String>,
    pub(super) training_history: Vec<TrainingEvent>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PolledDaemonOutput {
    pub(super) input_fingerprint: String,
    pub(super) output_json: String,
    pub(super) latest_input_json: Option<String>,
    pub(super) recommendation_summary_json: String,
    pub(super) target_outputs: Vec<TargetDaemonOutput>,
}

pub(super) fn target_daemon_output_persistence_json(target: &TargetDaemonOutput) -> String {
    format!(
        "{{\"path_segment\":\"{}\",\"output_json\":{},\"input_json\":{},\"recommendation_summary_json\":{},\"updated_unix_ms\":{},\"state_hash\":\"{}\",\"last_success_unix_ms\":{},\"last_error\":{},\"training_history\":{}}}",
        escape_json_string(&target.path_segment),
        target.output_json,
        target.input_json.as_deref().unwrap_or("null"),
        target.recommendation_summary_json,
        target.updated_unix_ms,
        escape_json_string(&target.state_hash),
        persistence_optional_u128_json(target.last_success_unix_ms),
        persistence_optional_error_json(target.last_error.as_deref()),
        training_history_json(&target.training_history)
    )
}

pub(super) fn compact_input_json_for_persistence(input_json: Option<&str>) -> Option<String> {
    match input_json {
        Some(value) if value.len() <= DAEMON_STATE_INPUT_JSON_LIMIT => Some(value.to_string()),
        _ => None,
    }
}

pub(super) fn compact_output_json_for_persistence(output_json: &str, limit: usize) -> String {
    if output_json == "null" || output_json.len() <= limit {
        return output_json.to_string();
    }
    if let Some(pattern_memory_state) = extract_json_value(output_json, "pattern_memory_state") {
        return format!("{{\"pattern_memory_state\":{}}}", pattern_memory_state);
    }
    "null".to_string()
}

pub(super) fn target_persistence_priority(target: &TargetDaemonOutput) -> (usize, u128, &str) {
    let (learning_active, learned_routes) =
        learned_route_summary_from_recommendation_summary(&target.recommendation_summary_json);
    let mut score = 0usize;
    if !target.training_history.is_empty() {
        score += 4;
    }
    if learning_active || learned_routes > 0 {
        score += 3;
    }
    if target.last_error.is_some() {
        score += 2;
    }
    if target.output_json != "null" {
        score += 1;
    }
    (score, target.updated_unix_ms, target.path_segment.as_str())
}

pub(super) fn compact_target_daemon_output_for_persistence(
    target: &TargetDaemonOutput,
) -> TargetDaemonOutput {
    TargetDaemonOutput {
        path_segment: target.path_segment.clone(),
        output_json: compact_output_json_for_persistence(
            &target.output_json,
            DAEMON_STATE_TARGET_OUTPUT_JSON_LIMIT,
        ),
        input_json: compact_input_json_for_persistence(target.input_json.as_deref()),
        recommendation_summary_json: target.recommendation_summary_json.clone(),
        updated_unix_ms: target.updated_unix_ms,
        state_hash: target.state_hash.clone(),
        last_success_unix_ms: target.last_success_unix_ms,
        last_error: target.last_error.clone(),
        training_history: compacted_training_history(&target.training_history),
    }
}

pub(super) fn retained_target_outputs_for_persistence(
    target_outputs: &[TargetDaemonOutput],
) -> Vec<TargetDaemonOutput> {
    let mut retained = target_outputs.to_vec();
    retained.sort_by(|left, right| {
        target_persistence_priority(right).cmp(&target_persistence_priority(left))
    });
    retained.truncate(DAEMON_STATE_TARGET_LIMIT);
    retained.sort_by(|left, right| {
        right
            .updated_unix_ms
            .cmp(&left.updated_unix_ms)
            .then_with(|| left.path_segment.cmp(&right.path_segment))
    });
    retained
        .iter()
        .map(compact_target_daemon_output_for_persistence)
        .collect()
}

pub(super) fn batch_entry_for_target_persistence(target: &TargetDaemonOutput) -> (String, String) {
    if target.output_json == "null"
        && let Some(error) = &target.last_error
    {
        return (target.path_segment.clone(), format!("__error__:{}", error));
    }
    (target.path_segment.clone(), target.output_json.clone())
}

pub(super) fn compact_daemon_snapshot_for_persistence(snapshot: &DaemonSnapshot) -> DaemonSnapshot {
    let retained_target_outputs = retained_target_outputs_for_persistence(&snapshot.target_outputs);
    let (latest_output_json, latest_recommendation_summary_json) =
        if snapshot.source.ends_with("targets-url") {
            let entries = retained_target_outputs
                .iter()
                .map(batch_entry_for_target_persistence)
                .collect::<Vec<_>>();
            (
                batch_output_json(&entries),
                recommendation_overview_json(&entries),
            )
        } else {
            (
                compact_output_json_for_persistence(
                    &snapshot.latest_output_json,
                    DAEMON_STATE_LATEST_OUTPUT_JSON_LIMIT,
                ),
                snapshot.latest_recommendation_summary_json.clone(),
            )
        };

    DaemonSnapshot {
        source: snapshot.source.clone(),
        upstream_url: snapshot.upstream_url.clone(),
        interval_ms: snapshot.interval_ms,
        cycle: snapshot.cycle,
        analysis_runs: snapshot.analysis_runs,
        cache_hits: snapshot.cache_hits,
        target_count: snapshot.target_count,
        updated_unix_ms: snapshot.updated_unix_ms,
        state_hash: snapshot.state_hash.clone(),
        latest_output_json,
        latest_input_json: compact_input_json_for_persistence(
            snapshot.latest_input_json.as_deref(),
        ),
        latest_recommendation_summary_json,
        target_outputs: retained_target_outputs,
        last_success_unix_ms: snapshot.last_success_unix_ms,
        last_error: snapshot.last_error.clone(),
        training_history: compacted_training_history(&snapshot.training_history),
    }
}

pub(super) fn daemon_snapshot_persistence_json(snapshot: &DaemonSnapshot) -> String {
    let snapshot = compact_daemon_snapshot_for_persistence(snapshot);
    let target_outputs = persistence_target_outputs_json(&snapshot.target_outputs);
    format!(
        "{{\"source\":\"{}\",\"upstream_url\":\"{}\",\"interval_ms\":{},\"cycle\":{},\"analysis_runs\":{},\"cache_hits\":{},\"target_count\":{},\"updated_unix_ms\":{},\"state_hash\":\"{}\",\"latest_output_json\":{},\"latest_input_json\":{},\"latest_recommendation_summary_json\":{},\"target_outputs\":[{}],\"last_success_unix_ms\":{},\"last_error\":{},\"training_history\":{}}}",
        escape_json_string(&snapshot.source),
        escape_json_string(&snapshot.upstream_url),
        snapshot.interval_ms,
        snapshot.cycle,
        snapshot.analysis_runs,
        snapshot.cache_hits,
        snapshot.target_count,
        snapshot.updated_unix_ms,
        escape_json_string(&snapshot.state_hash),
        snapshot.latest_output_json,
        snapshot.latest_input_json.as_deref().unwrap_or("null"),
        snapshot.latest_recommendation_summary_json,
        target_outputs,
        persistence_optional_u128_json(snapshot.last_success_unix_ms),
        persistence_optional_error_json(snapshot.last_error.as_deref()),
        training_history_json(&snapshot.training_history)
    )
}

fn persistence_target_outputs_json(target_outputs: &[TargetDaemonOutput]) -> String {
    target_outputs
        .iter()
        .map(target_daemon_output_persistence_json)
        .collect::<Vec<_>>()
        .join(",")
}

fn persistence_optional_u128_json(value: Option<u128>) -> String {
    value
        .map(|value| value.to_string())
        .unwrap_or_else(|| "null".to_string())
}

fn persistence_optional_error_json(value: Option<&str>) -> String {
    value
        .map(|value| format!("\"{}\"", escape_json_string(value)))
        .unwrap_or_else(|| "null".to_string())
}

pub(super) fn parse_training_event_from_json(input: &str) -> Result<TrainingEvent, String> {
    Ok(TrainingEvent {
        label: parse_json_string_value(
            &extract_json_value(input, "label")
                .ok_or_else(|| "persisted training event missing label".to_string())?,
        )
        .ok_or_else(|| "persisted training event label was null".to_string())?,
        weight: extract_json_value(input, "weight")
            .ok_or_else(|| "persisted training event missing weight".to_string())?,
        trained_unix_ms: extract_json_value(input, "trained_unix_ms")
            .ok_or_else(|| "persisted training event missing trained_unix_ms".to_string())?
            .parse::<u128>()
            .map_err(|_| "persisted training event has invalid trained_unix_ms".to_string())?,
        scope: parse_json_string_value(
            &extract_json_value(input, "scope")
                .ok_or_else(|| "persisted training event missing scope".to_string())?,
        )
        .ok_or_else(|| "persisted training event scope was null".to_string())?,
    })
}

pub(super) fn parse_training_history_from_json(input: &str) -> Result<Vec<TrainingEvent>, String> {
    split_top_level_json_items(input)
        .into_iter()
        .map(|item| parse_training_event_from_json(&item))
        .collect()
}

pub(super) fn parse_target_daemon_output_from_json(
    input: &str,
) -> Result<TargetDaemonOutput, String> {
    Ok(TargetDaemonOutput {
        path_segment: parse_json_string_value(
            &extract_json_value(input, "path_segment")
                .ok_or_else(|| "persisted target output missing path_segment".to_string())?,
        )
        .ok_or_else(|| "persisted target output path_segment was null".to_string())?,
        output_json: extract_json_value(input, "output_json")
            .ok_or_else(|| "persisted target output missing output_json".to_string())?,
        input_json: extract_json_value(input, "input_json")
            .and_then(|value| if value == "null" { None } else { Some(value) }),
        recommendation_summary_json: extract_json_value(input, "recommendation_summary_json")
            .ok_or_else(|| {
                "persisted target output missing recommendation_summary_json".to_string()
            })?,
        updated_unix_ms: extract_json_value(input, "updated_unix_ms")
            .ok_or_else(|| "persisted target output missing updated_unix_ms".to_string())?
            .parse::<u128>()
            .map_err(|_| "persisted target output has invalid updated_unix_ms".to_string())?,
        state_hash: parse_json_string_value(
            &extract_json_value(input, "state_hash")
                .ok_or_else(|| "persisted target output missing state_hash".to_string())?,
        )
        .ok_or_else(|| "persisted target output state_hash was null".to_string())?,
        last_success_unix_ms: extract_json_value(input, "last_success_unix_ms").and_then(|value| {
            if value == "null" {
                None
            } else {
                value.parse::<u128>().ok()
            }
        }),
        last_error: extract_json_value(input, "last_error")
            .and_then(|value| parse_json_string_value(&value)),
        training_history: extract_json_value(input, "training_history")
            .map(|value| parse_training_history_from_json(&value))
            .transpose()?
            .unwrap_or_default(),
    })
}

pub(super) fn parse_daemon_snapshot_from_json(input: &str) -> Result<DaemonSnapshot, String> {
    let target_outputs = extract_json_value(input, "target_outputs")
        .map(|value| {
            split_top_level_json_items(&value)
                .into_iter()
                .map(|item| parse_target_daemon_output_from_json(&item))
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_default();
    let training_history = extract_json_value(input, "training_history")
        .map(|value| parse_training_history_from_json(&value))
        .transpose()?
        .unwrap_or_default();
    Ok(DaemonSnapshot {
        source: parse_json_string_value(
            &extract_json_value(input, "source")
                .ok_or_else(|| "persisted daemon snapshot missing source".to_string())?,
        )
        .ok_or_else(|| "persisted daemon snapshot source was null".to_string())?,
        upstream_url: parse_json_string_value(
            &extract_json_value(input, "upstream_url")
                .ok_or_else(|| "persisted daemon snapshot missing upstream_url".to_string())?,
        )
        .ok_or_else(|| "persisted daemon snapshot upstream_url was null".to_string())?,
        interval_ms: extract_json_value(input, "interval_ms")
            .ok_or_else(|| "persisted daemon snapshot missing interval_ms".to_string())?
            .parse::<u64>()
            .map_err(|_| "persisted daemon snapshot has invalid interval_ms".to_string())?,
        cycle: extract_json_value(input, "cycle")
            .ok_or_else(|| "persisted daemon snapshot missing cycle".to_string())?
            .parse::<usize>()
            .map_err(|_| "persisted daemon snapshot has invalid cycle".to_string())?,
        analysis_runs: extract_json_value(input, "analysis_runs")
            .ok_or_else(|| "persisted daemon snapshot missing analysis_runs".to_string())?
            .parse::<usize>()
            .map_err(|_| "persisted daemon snapshot has invalid analysis_runs".to_string())?,
        cache_hits: extract_json_value(input, "cache_hits")
            .ok_or_else(|| "persisted daemon snapshot missing cache_hits".to_string())?
            .parse::<usize>()
            .map_err(|_| "persisted daemon snapshot has invalid cache_hits".to_string())?,
        target_count: extract_json_value(input, "target_count")
            .ok_or_else(|| "persisted daemon snapshot missing target_count".to_string())?
            .parse::<usize>()
            .map_err(|_| "persisted daemon snapshot has invalid target_count".to_string())?,
        updated_unix_ms: extract_json_value(input, "updated_unix_ms")
            .ok_or_else(|| "persisted daemon snapshot missing updated_unix_ms".to_string())?
            .parse::<u128>()
            .map_err(|_| "persisted daemon snapshot has invalid updated_unix_ms".to_string())?,
        state_hash: parse_json_string_value(
            &extract_json_value(input, "state_hash")
                .ok_or_else(|| "persisted daemon snapshot missing state_hash".to_string())?,
        )
        .ok_or_else(|| "persisted daemon snapshot state_hash was null".to_string())?,
        latest_output_json: extract_json_value(input, "latest_output_json")
            .ok_or_else(|| "persisted daemon snapshot missing latest_output_json".to_string())?,
        latest_input_json: extract_json_value(input, "latest_input_json")
            .and_then(|value| if value == "null" { None } else { Some(value) }),
        latest_recommendation_summary_json: extract_json_value(
            input,
            "latest_recommendation_summary_json",
        )
        .ok_or_else(|| {
            "persisted daemon snapshot missing latest_recommendation_summary_json".to_string()
        })?,
        target_outputs,
        last_success_unix_ms: extract_json_value(input, "last_success_unix_ms").and_then(|value| {
            if value == "null" {
                None
            } else {
                value.parse::<u128>().ok()
            }
        }),
        last_error: extract_json_value(input, "last_error")
            .and_then(|value| parse_json_string_value(&value)),
        training_history,
    })
}

pub(super) fn write_daemon_state(path: &Path, snapshot: &DaemonSnapshot) -> Result<(), String> {
    reject_unsafe_daemon_state_path(path)?;
    let payload = daemon_snapshot_persistence_json(snapshot);
    if payload.len() > DAEMON_STATE_FILE_LIMIT {
        return Err(format!(
            "daemon state exceeds {} byte limit",
            DAEMON_STATE_FILE_LIMIT
        ));
    }
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create daemon state directory '{}': {err}",
                parent.display()
            )
        })?;
    }
    let tmp_path = daemon_state_temp_path(path)?;
    let result = write_daemon_state_atomically(path, &tmp_path, payload.as_bytes());
    if result.is_err() {
        let _ = fs::remove_file(&tmp_path);
    }
    result
}

pub(super) fn read_daemon_state(path: &Path) -> Result<Option<DaemonSnapshot>, String> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => {
            return Err(format!(
                "failed to inspect daemon state '{}': {err}",
                path.display()
            ));
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(format!(
            "daemon state must be a regular non-symlink file: '{}'",
            path.display()
        ));
    }
    if metadata.len() > DAEMON_STATE_FILE_LIMIT as u64 {
        return Err(format!(
            "daemon state exceeds {} byte limit",
            DAEMON_STATE_FILE_LIMIT
        ));
    }
    let file = fs::File::open(path)
        .map_err(|err| format!("failed to open daemon state '{}': {err}", path.display()))?;
    let opened_metadata = file.metadata().map_err(|err| {
        format!(
            "failed to inspect opened daemon state '{}': {err}",
            path.display()
        )
    })?;
    if !opened_metadata.is_file() || opened_metadata.len() > DAEMON_STATE_FILE_LIMIT as u64 {
        return Err(format!(
            "daemon state changed to an unsafe file while opening: '{}'",
            path.display()
        ));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.dev() != opened_metadata.dev() || metadata.ino() != opened_metadata.ino() {
            return Err(format!(
                "daemon state changed while opening: '{}'",
                path.display()
            ));
        }
    }
    let mut body = String::new();
    file.take((DAEMON_STATE_FILE_LIMIT + 1) as u64)
        .read_to_string(&mut body)
        .map_err(|err| format!("failed to read daemon state '{}': {err}", path.display()))?;
    if body.len() > DAEMON_STATE_FILE_LIMIT {
        return Err(format!(
            "daemon state exceeds {} byte limit",
            DAEMON_STATE_FILE_LIMIT
        ));
    }
    parse_daemon_snapshot_from_json(&body).map(Some)
}

fn reject_unsafe_daemon_state_path(path: &Path) -> Result<(), String> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => Err(format!(
            "daemon state must be a regular non-symlink file: '{}'",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(format!(
            "failed to inspect daemon state '{}': {err}",
            path.display()
        )),
    }
}

fn daemon_state_temp_path(path: &Path) -> Result<PathBuf, String> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| "daemon state path must end in a UTF-8 filename".to_string())?;
    let sequence = DAEMON_STATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let temp_name = format!(".{file_name}.{}.{}.tmp", std::process::id(), sequence);
    Ok(path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."))
        .join(temp_name))
}

fn write_daemon_state_atomically(
    path: &Path,
    tmp_path: &Path,
    payload: &[u8],
) -> Result<(), String> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(tmp_path).map_err(|err| {
        format!(
            "failed to create daemon state temporary file '{}': {err}",
            tmp_path.display()
        )
    })?;
    file.write_all(payload).map_err(|err| {
        format!(
            "failed to write daemon state temporary file '{}': {err}",
            tmp_path.display()
        )
    })?;
    file.sync_all().map_err(|err| {
        format!(
            "failed to sync daemon state temporary file '{}': {err}",
            tmp_path.display()
        )
    })?;
    drop(file);
    fs::rename(tmp_path, path)
        .map_err(|err| format!("failed to replace daemon state '{}': {err}", path.display()))?;
    #[cfg(unix)]
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|err| {
                format!(
                    "failed to sync daemon state directory '{}': {err}",
                    parent.display()
                )
            })?;
    }
    Ok(())
}
