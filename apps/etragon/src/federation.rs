use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FederationMember {
    pub(super) id: String,
    pub(super) targets_url: String,
}

#[derive(Clone, Debug)]
struct FederationRuntimeResult {
    id: String,
    targets_url: String,
    target_count: usize,
    error: Option<String>,
}

pub(super) fn parse_federation_members(input: &str) -> Result<Vec<FederationMember>, String> {
    let runtimes = extract_json_value(input, "runtimes")
        .ok_or_else(|| "federation manifest missing runtimes".to_string())?;
    let mut members = Vec::new();
    for item in split_top_level_json_items(&runtimes) {
        let id = parse_json_string_value(
            &extract_json_value(&item, "id")
                .ok_or_else(|| "federation runtime missing id".to_string())?,
        )
        .ok_or_else(|| "federation runtime id was null".to_string())?;
        let targets_url = parse_json_string_value(
            &extract_json_value(&item, "targets_url")
                .ok_or_else(|| "federation runtime missing targets_url".to_string())?,
        )
        .ok_or_else(|| "federation runtime targets_url was null".to_string())?;
        if id.trim().is_empty() {
            return Err("federation runtime id must not be empty".to_string());
        }
        members.push(FederationMember { id, targets_url });
    }
    if members.is_empty() {
        return Err("federation manifest did not include any runtimes".to_string());
    }
    Ok(members)
}

pub(super) fn analyze_federation_manifest_with_python_worker(
    manifest_json: &str,
    filter_prefix: Option<&str>,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    let members = parse_federation_members(manifest_json)?;
    let mut worker = PythonWorkerClient::spawn(config)?;
    let mut entries = Vec::new();
    let mut runtime_results = Vec::new();
    for member in members {
        let result = analyze_federation_member(&member, filter_prefix, &mut worker, &mut entries);
        runtime_results.push(result);
    }
    Ok(federation_output_json(&runtime_results, &entries))
}

pub(super) fn train_federation_manifest_with_python_worker(
    manifest_json: &str,
    label: &str,
    weight: f64,
    filter_prefix: Option<&str>,
    config: &PythonWorkerConfig,
) -> Result<String, String> {
    let members = parse_federation_members(manifest_json)?;
    let mut worker = PythonWorkerClient::spawn(config)?;
    let mut entries = Vec::new();
    let mut runtime_results = Vec::new();
    for member in members {
        let result = train_federation_member(
            &member,
            label,
            weight,
            filter_prefix,
            &mut worker,
            &mut entries,
        );
        runtime_results.push(result);
    }
    Ok(federation_output_json(&runtime_results, &entries))
}

fn analyze_federation_member(
    member: &FederationMember,
    filter_prefix: Option<&str>,
    worker: &mut PythonWorkerClient,
    entries: &mut Vec<(String, String)>,
) -> FederationRuntimeResult {
    visit_federation_member_targets(member, filter_prefix, entries, |analysis_json| {
        worker.analyze_json(analysis_json)
    })
}

fn train_federation_member(
    member: &FederationMember,
    label: &str,
    weight: f64,
    filter_prefix: Option<&str>,
    worker: &mut PythonWorkerClient,
    entries: &mut Vec<(String, String)>,
) -> FederationRuntimeResult {
    visit_federation_member_targets(member, filter_prefix, entries, |analysis_json| {
        worker.train_json_with_weight(analysis_json, label, weight)
    })
}

fn visit_federation_member_targets<F>(
    member: &FederationMember,
    filter_prefix: Option<&str>,
    entries: &mut Vec<(String, String)>,
    mut analyze: F,
) -> FederationRuntimeResult
where
    F: FnMut(&str) -> Result<String, String>,
{
    let endpoint = match resolve_target_batch_endpoint(
        &member.targets_url,
        "federation targets_url must point at /v1/latest/targets",
        filter_prefix,
    ) {
        Ok(endpoint) => endpoint,
        Err(err) => return federation_member_error(member, err),
    };
    let mut target_count = 0usize;
    for segment in endpoint.segments.clone() {
        target_count += 1;
        let key = federation_target_key(&member.id, &segment);
        match endpoint
            .fetch_analysis_json(&segment)
            .and_then(|json| analyze(&json))
        {
            Ok(output) => entries.push((key, output)),
            Err(err) => entries.push((key, format!("__error__:{err}"))),
        }
    }
    FederationRuntimeResult {
        id: member.id.clone(),
        targets_url: member.targets_url.clone(),
        target_count,
        error: None,
    }
}

fn federation_member_error(member: &FederationMember, error: String) -> FederationRuntimeResult {
    FederationRuntimeResult {
        id: member.id.clone(),
        targets_url: member.targets_url.clone(),
        target_count: 0,
        error: Some(error),
    }
}

fn federation_target_key(runtime_id: &str, path_segment: &str) -> String {
    format!("{runtime_id}/{path_segment}")
}

fn federation_runtime_results_json(results: &[FederationRuntimeResult]) -> String {
    let body = results
        .iter()
        .map(|result| {
            let error = result
                .error
                .as_ref()
                .map(|value| format!("\"{}\"", escape_json_string(value)))
                .unwrap_or_else(|| "null".to_string());
            format!(
                "{{\"id\":\"{}\",\"targets_url\":\"{}\",\"target_count\":{},\"error\":{}}}",
                escape_json_string(&result.id),
                escape_json_string(&result.targets_url),
                result.target_count,
                error
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{}]", body)
}

fn federation_targets_json(entries: &[(String, String)]) -> String {
    let body = entries
        .iter()
        .map(|(key, output)| {
            let (runtime_id, path_segment) = key.split_once('/').unwrap_or((key, ""));
            let output_json = if output.starts_with("__error__:") {
                format!(
                    "{{\"error\":\"{}\"}}",
                    escape_json_string(output.trim_start_matches("__error__:"))
                )
            } else {
                output.clone()
            };
            format!(
                "{{\"runtime_id\":\"{}\",\"path_segment\":\"{}\",\"federated_path_segment\":\"{}\",\"output\":{}}}",
                escape_json_string(runtime_id),
                escape_json_string(path_segment),
                escape_json_string(key),
                output_json
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{}]", body)
}

fn federation_output_json(
    runtime_results: &[FederationRuntimeResult],
    entries: &[(String, String)],
) -> String {
    let failed_runtime_count = runtime_results
        .iter()
        .filter(|result| result.error.is_some())
        .count();
    format!(
        "{{\"kind\":\"etragon_federated_learning_batch\",\"runtime_count\":{},\"target_count\":{},\"failed_runtime_count\":{},\"runtimes\":{},\"recommendation_summary\":{},\"targets\":{}}}",
        runtime_results.len(),
        entries.len(),
        failed_runtime_count,
        federation_runtime_results_json(runtime_results),
        recommendation_overview_json(entries),
        federation_targets_json(entries)
    )
}

pub(super) fn federation_summary_json_from_snapshot(snapshot: &DaemonSnapshot) -> String {
    let source = if snapshot.source == "python-targets-url" {
        "single_upstream_targets"
    } else {
        "single_upstream_latest"
    };
    let entries = snapshot
        .target_outputs
        .iter()
        .map(|target| (target.path_segment.clone(), target.output_json.clone()))
        .collect::<Vec<_>>();
    format!(
        "{{\"kind\":\"etragon_federation_summary\",\"source\":\"{}\",\"runtime_count\":1,\"target_count\":{},\"upstream_url\":\"{}\",\"recommendation_summary\":{},\"resident_training_events\":{},\"target_training_events\":{}}}",
        source,
        snapshot.target_outputs.len(),
        escape_json_string(&snapshot.upstream_url),
        recommendation_overview_json(&entries),
        snapshot.training_history.len(),
        snapshot
            .target_outputs
            .iter()
            .map(|target| target.training_history.len())
            .sum::<usize>()
    )
}
