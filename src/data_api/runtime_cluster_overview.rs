use std::collections::BTreeMap;

use crate::render_utils::append_json_string;

use super::{ApiSnapshot, ApiTargetSnapshot};

pub(super) fn api_runtime_cluster_overview_json(snapshot: &ApiSnapshot) -> String {
    let overview = build_overview(snapshot);
    let mut json = String::with_capacity(4096);
    json.push_str("{\"surface\":\"runtime_cluster_overview\",\"kind\":");
    append_json_string(&mut json, &snapshot.kind);
    json.push_str(",\"updated_unix_ms\":");
    json.push_str(&snapshot.updated_unix_ms.to_string());
    json.push_str(",\"target_count\":");
    json.push_str(&snapshot.target_names.len().to_string());
    json.push_str(",\"cluster_count\":");
    json.push_str(&overview.clusters.len().to_string());
    json.push_str(",\"unclustered_target_count\":");
    json.push_str(&overview.unclustered_targets.len().to_string());
    json.push_str(",\"clusters\":[");
    for (index, cluster) in overview.clusters.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('{');
        json.push_str("\"key\":");
        append_json_string(&mut json, &cluster.key);
        json.push_str(",\"label\":");
        append_json_string(&mut json, &cluster.label);
        json.push_str(",\"operator_hint\":");
        append_json_string(&mut json, &cluster.operator_hint);
        json.push_str(",\"target_count\":");
        json.push_str(&cluster.targets.len().to_string());
        json.push_str(",\"sidecar_context_count\":");
        json.push_str(&cluster.sidecar_context_count.to_string());
        json.push_str(",\"capability_profile_count\":");
        json.push_str(&cluster.capability_profile_count.to_string());
        json.push_str(",\"targets\":[");
        for (target_index, target) in cluster.targets.iter().enumerate() {
            if target_index > 0 {
                json.push(',');
            }
            append_target_json(&mut json, target);
        }
        json.push_str("]}");
    }
    json.push_str("],\"unclustered_targets\":[");
    for (index, target) in overview.unclustered_targets.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_target_json(&mut json, target);
    }
    json.push_str("]}");
    json
}

#[derive(Default)]
struct Overview {
    clusters: Vec<ClusterOverview>,
    unclustered_targets: Vec<TargetOverview>,
}

struct ClusterOverview {
    key: String,
    label: String,
    operator_hint: String,
    sidecar_context_count: usize,
    capability_profile_count: usize,
    targets: Vec<TargetOverview>,
}

#[derive(Clone)]
struct TargetOverview {
    name: String,
    protocol: Option<String>,
    entry: Option<String>,
    primary_module_family: Option<String>,
    evidence_posture: Option<String>,
    automation_outcome: Option<String>,
    has_external_sidecar_context: bool,
    has_external_capability_profile: bool,
    external_capability_status: Option<String>,
    external_context_status: Option<String>,
    external_sidecar_trust_level: Option<String>,
}

fn build_overview(snapshot: &ApiSnapshot) -> Overview {
    let mut grouped =
        BTreeMap::<String, (String, String, usize, usize, Vec<TargetOverview>)>::new();
    let mut unclustered_targets = Vec::new();

    for name in &snapshot.target_names {
        let Some(target) = snapshot.target_snapshots.get(name) else {
            continue;
        };
        let rendered = target_overview(name, target);
        let Some(surface) = target.protocol_surface.as_ref() else {
            unclustered_targets.push(rendered);
            continue;
        };
        let Some(cluster) = surface.cluster_hint.as_ref() else {
            unclustered_targets.push(rendered);
            continue;
        };
        let bucket = grouped.entry(cluster.key.clone()).or_insert_with(|| {
            (
                cluster.label.clone(),
                cluster.operator_hint.clone(),
                0usize,
                0usize,
                Vec::new(),
            )
        });
        if target.has_external_sidecar_context {
            bucket.2 += 1;
        }
        if target.has_external_capability_profile {
            bucket.3 += 1;
        }
        bucket.4.push(rendered);
    }

    Overview {
        clusters: grouped
            .into_iter()
            .map(
                |(key, (label, operator_hint, sidecar_context_count, capability_profile_count, mut targets))| {
                    targets.sort_by(|left, right| left.name.cmp(&right.name));
                    ClusterOverview {
                        key,
                        label,
                        operator_hint,
                        sidecar_context_count,
                        capability_profile_count,
                        targets,
                    }
                },
            )
            .collect(),
        unclustered_targets,
    }
}

fn target_overview(name: &str, target: &ApiTargetSnapshot) -> TargetOverview {
    TargetOverview {
        name: name.to_string(),
        protocol: target.protocol_surface.as_ref().map(|surface| surface.protocol.clone()),
        entry: target.protocol_surface.as_ref().map(|surface| surface.entry.clone()),
        primary_module_family: target.primary_module_family.clone(),
        evidence_posture: target.evidence_posture.clone(),
        automation_outcome: target.automation_outcome.clone(),
        has_external_sidecar_context: target.has_external_sidecar_context,
        has_external_capability_profile: target.has_external_capability_profile,
        external_capability_status: target.external_capability_status.clone(),
        external_context_status: target.external_context_status.clone(),
        external_sidecar_trust_level: target.external_sidecar_trust_level.clone(),
    }
}

fn append_target_json(target: &mut String, rendered: &TargetOverview) {
    target.push('{');
    target.push_str("\"name\":");
    append_json_string(target, &rendered.name);
    target.push_str(",\"protocol\":");
    append_optional_string_json(target, rendered.protocol.as_deref());
    target.push_str(",\"entry\":");
    append_optional_string_json(target, rendered.entry.as_deref());
    target.push_str(",\"primary_module_family\":");
    append_optional_string_json(target, rendered.primary_module_family.as_deref());
    target.push_str(",\"evidence_posture\":");
    append_optional_string_json(target, rendered.evidence_posture.as_deref());
    target.push_str(",\"automation_outcome\":");
    append_optional_string_json(target, rendered.automation_outcome.as_deref());
    target.push_str(",\"has_external_sidecar_context\":");
    target.push_str(if rendered.has_external_sidecar_context { "true" } else { "false" });
    target.push_str(",\"has_external_capability_profile\":");
    target.push_str(if rendered.has_external_capability_profile { "true" } else { "false" });
    target.push_str(",\"external_capability_status\":");
    append_optional_string_json(target, rendered.external_capability_status.as_deref());
    target.push_str(",\"external_context_status\":");
    append_optional_string_json(target, rendered.external_context_status.as_deref());
    target.push_str(",\"external_sidecar_trust_level\":");
    append_optional_string_json(target, rendered.external_sidecar_trust_level.as_deref());
    target.push('}');
}

fn append_optional_string_json(target: &mut String, value: Option<&str>) {
    if let Some(value) = value {
        append_json_string(target, value);
    } else {
        target.push_str("null");
    }
}
