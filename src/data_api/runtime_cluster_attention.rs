use std::cmp::Reverse;
use std::collections::BTreeMap;

use crate::render_utils::{append_json_string, append_string_list_json};
use gewyvern::certificate_policy::{
    REASON_CERTIFICATE_SHELF_BOOTSTRAP_EMPTY, REASON_CERTIFICATE_STATE_ROOT_MISSING,
    REASON_DISTRUSTED_TRUST_ANCHOR_MATERIAL, REASON_EMPTY_AUTHORITY_ROOT,
    REASON_EXPIRED_CERTIFICATE_MATERIAL, REASON_EXPIRING_CERTIFICATE_MATERIAL,
    REASON_EXPLICIT_REMOTE_TRUST_WITHOUT_ANCHORS, REASON_IDENTITY_CERTIFICATES_WITHOUT_KEYS,
    REASON_IDENTITY_KEYS_WITHOUT_CERTIFICATES, REASON_OVERDUE_CERTIFICATE_ROTATION,
    REASON_PRIVATE_KEYS_PRESENT_IN_TRUST_ROOT, REASON_REVOKED_CERTIFICATE_MATERIAL,
    runtime_certificate_policy,
};

use super::{ApiSnapshot, ApiTargetSnapshot};

const REASON_SPECS: &[AttentionReasonSpec] = &[
    AttentionReasonSpec {
        key: "automation.targeted_escalation",
        label: "Targeted escalation",
        priority: AttentionPriority::Critical,
        note: "Direct protocol or automation logic marked this runtime for immediate operator focus.",
    },
    AttentionReasonSpec {
        key: "automation.manual_review",
        label: "Manual review",
        priority: AttentionPriority::Warning,
        note: "Automation could not safely conclude and wants an operator to inspect the runtime.",
    },
    AttentionReasonSpec {
        key: "automation.collect_more_evidence",
        label: "Collect more evidence",
        priority: AttentionPriority::Warning,
        note: "The runtime needs more evidence before safe automation or confident handoff.",
    },
    AttentionReasonSpec {
        key: "sidecar.unverified",
        label: "Unverified sidecar trust",
        priority: AttentionPriority::Warning,
        note: "A sidecar exists, but its trust chain is not strong enough for confident control-plane use.",
    },
    AttentionReasonSpec {
        key: "sidecar.degraded",
        label: "Degraded sidecar trust",
        priority: AttentionPriority::Warning,
        note: "A sidecar is present, but its declared trust state is degraded.",
    },
    AttentionReasonSpec {
        key: "capability.unavailable",
        label: "Capability unavailable",
        priority: AttentionPriority::Warning,
        note: "The runtime published or expected a capability profile, but it is currently unavailable.",
    },
    AttentionReasonSpec {
        key: "sidecar.context_without_profile",
        label: "Context without profile",
        priority: AttentionPriority::Observe,
        note: "A sidecar context exists, but there is still no capability profile to anchor decisions.",
    },
    AttentionReasonSpec {
        key: REASON_EXPLICIT_REMOTE_TRUST_WITHOUT_ANCHORS,
        label: "Remote trust without anchors",
        priority: AttentionPriority::Warning,
        note: "Remote trust protection is enabled, but the trust shelf does not contain any trust anchors yet.",
    },
    AttentionReasonSpec {
        key: REASON_PRIVATE_KEYS_PRESENT_IN_TRUST_ROOT,
        label: "Trust shelf contains private keys",
        priority: AttentionPriority::Warning,
        note: "Private-key material was found in the trust shelf, which should normally contain anchors only.",
    },
    AttentionReasonSpec {
        key: REASON_IDENTITY_KEYS_WITHOUT_CERTIFICATES,
        label: "Identity keys without certificates",
        priority: AttentionPriority::Warning,
        note: "Identity private keys exist without matching identity certificate material.",
    },
    AttentionReasonSpec {
        key: REASON_IDENTITY_CERTIFICATES_WITHOUT_KEYS,
        label: "Identity certificates without keys",
        priority: AttentionPriority::Observe,
        note: "Identity certificate material exists without matching private keys in the identity shelf.",
    },
    AttentionReasonSpec {
        key: REASON_EMPTY_AUTHORITY_ROOT,
        label: "Empty authority shelf",
        priority: AttentionPriority::Observe,
        note: "The authority shelf exists but still does not contain local authority material.",
    },
    AttentionReasonSpec {
        key: REASON_CERTIFICATE_STATE_ROOT_MISSING,
        label: "Missing certificate state root",
        priority: AttentionPriority::Observe,
        note: "The certificate state root has not been prepared yet.",
    },
    AttentionReasonSpec {
        key: REASON_CERTIFICATE_SHELF_BOOTSTRAP_EMPTY,
        label: "Certificate shelf bootstrap empty",
        priority: AttentionPriority::Observe,
        note: "The certificate shelf is still empty and remains in bootstrap posture.",
    },
    AttentionReasonSpec {
        key: REASON_EXPIRED_CERTIFICATE_MATERIAL,
        label: "Expired certificate material",
        priority: AttentionPriority::Warning,
        note: "Certificate material has already expired and should be rotated before trust or identity workflows continue.",
    },
    AttentionReasonSpec {
        key: REASON_EXPIRING_CERTIFICATE_MATERIAL,
        label: "Expiring certificate material",
        priority: AttentionPriority::Observe,
        note: "Certificate material is approaching expiry and should be rotated soon.",
    },
    AttentionReasonSpec {
        key: REASON_OVERDUE_CERTIFICATE_ROTATION,
        label: "Overdue certificate rotation",
        priority: AttentionPriority::Warning,
        note: "Certificate rotation records show overdue or failed rotations that need operator attention.",
    },
    AttentionReasonSpec {
        key: REASON_REVOKED_CERTIFICATE_MATERIAL,
        label: "Revoked certificate material",
        priority: AttentionPriority::Warning,
        note: "Certificate material has been explicitly revoked and should not be used by runtime workflows.",
    },
    AttentionReasonSpec {
        key: REASON_DISTRUSTED_TRUST_ANCHOR_MATERIAL,
        label: "Distrusted trust anchor",
        priority: AttentionPriority::Warning,
        note: "Trust-anchor material has been explicitly distrusted and should be removed from active use.",
    },
];

pub(super) fn api_runtime_cluster_attention_json(snapshot: &ApiSnapshot) -> String {
    let rollup = build_rollup(snapshot);
    let mut json = String::with_capacity(4096);
    json.push_str("{\"surface\":\"runtime_cluster_attention\",\"kind\":");
    append_json_string(&mut json, &snapshot.kind);
    json.push_str(",\"updated_unix_ms\":");
    json.push_str(&snapshot.updated_unix_ms.to_string());
    json.push_str(",\"attention_cluster_count\":");
    json.push_str(&rollup.clusters.len().to_string());
    json.push_str(",\"attention_target_count\":");
    json.push_str(&rollup.attention_target_count.to_string());
    json.push_str(",\"runtime_policy_attention_count\":");
    json.push_str(&rollup.runtime_policy_reasons.len().to_string());
    json.push_str(",\"clusters\":[");
    for (index, cluster) in rollup.clusters.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('{');
        json.push_str("\"key\":");
        append_json_string(&mut json, &cluster.key);
        json.push_str(",\"label\":");
        append_json_string(&mut json, &cluster.label);
        json.push_str(",\"priority\":");
        append_json_string(&mut json, cluster.priority.label());
        json.push_str(",\"attention_target_count\":");
        json.push_str(&cluster.targets.len().to_string());
        json.push_str(",\"targets\":[");
        for (target_index, target) in cluster.targets.iter().enumerate() {
            if target_index > 0 {
                json.push(',');
            }
            append_attention_target_json(&mut json, target);
        }
        json.push_str("]}");
    }
    json.push_str("],\"unclustered_targets\":[");
    for (index, target) in rollup.unclustered_targets.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        append_attention_target_json(&mut json, target);
    }
    json.push_str("],\"runtime_policy_reasons\":[");
    append_runtime_policy_reasons_json(&mut json, &rollup.runtime_policy_reasons);
    json.push_str("],\"reason_catalog\":");
    append_reason_catalog_json(&mut json);
    json.push('}');
    json
}

pub(super) fn api_runtime_cluster_attention_reasons_json() -> String {
    let runtime_policy_reasons = runtime_policy_attention_reasons();
    let mut json = String::with_capacity(1024);
    json.push_str("{\"surface\":\"runtime_cluster_attention_reasons\",\"count\":");
    json.push_str(&REASON_SPECS.len().to_string());
    json.push_str(",\"runtime_policy_attention_count\":");
    json.push_str(&runtime_policy_reasons.len().to_string());
    json.push_str(",\"reasons\":");
    append_reason_catalog_json(&mut json);
    json.push_str(",\"runtime_policy_reasons\":[");
    append_runtime_policy_reasons_json(&mut json, &runtime_policy_reasons);
    json.push_str("],\"runtime_policy_reason_counts\":[");
    append_runtime_policy_reason_counts_json(&mut json, &runtime_policy_reasons);
    json.push(']');
    json.push('}');
    json
}

pub(super) fn api_runtime_cluster_attention_summary_json(snapshot: &ApiSnapshot) -> String {
    let rollup = build_rollup(snapshot);
    let mut json = String::with_capacity(2048);
    json.push_str("{\"surface\":\"runtime_cluster_attention_summary\",\"kind\":");
    append_json_string(&mut json, &snapshot.kind);
    json.push_str(",\"updated_unix_ms\":");
    json.push_str(&snapshot.updated_unix_ms.to_string());
    json.push_str(",\"attention_cluster_count\":");
    json.push_str(&rollup.clusters.len().to_string());
    json.push_str(",\"attention_target_count\":");
    json.push_str(&rollup.attention_target_count.to_string());
    json.push_str(",\"runtime_policy_attention_count\":");
    json.push_str(&rollup.runtime_policy_reasons.len().to_string());
    json.push_str(",\"clusters\":[");
    for (index, cluster) in rollup.clusters.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        json.push('{');
        json.push_str("\"key\":");
        append_json_string(&mut json, &cluster.key);
        json.push_str(",\"label\":");
        append_json_string(&mut json, &cluster.label);
        json.push_str(",\"priority\":");
        append_json_string(&mut json, cluster.priority.label());
        json.push_str(",\"attention_target_count\":");
        json.push_str(&cluster.targets.len().to_string());
        json.push_str(",\"reason_counts\":[");
        append_reason_counts_json(&mut json, &reason_counts(&cluster.targets));
        json.push_str("]}");
    }
    json.push_str("],\"unclustered_attention_target_count\":");
    json.push_str(&rollup.unclustered_targets.len().to_string());
    json.push_str(",\"unclustered_reason_counts\":[");
    append_reason_counts_json(&mut json, &reason_counts(&rollup.unclustered_targets));
    json.push_str("],\"runtime_policy_reason_counts\":[");
    append_runtime_policy_reason_counts_json(&mut json, &rollup.runtime_policy_reasons);
    json.push_str("]}");
    json
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
struct AttentionReasonSpec {
    key: &'static str,
    label: &'static str,
    priority: AttentionPriority,
    note: &'static str,
}

#[derive(Copy, Clone, Eq, PartialEq, Ord, PartialOrd)]
pub(super) enum AttentionPriority {
    Critical,
    Warning,
    Observe,
    Healthy,
}

impl AttentionPriority {
    fn label(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Warning => "warning",
            Self::Observe => "observe",
            Self::Healthy => "healthy",
        }
    }

    fn rank(self) -> u8 {
        match self {
            Self::Critical => 0,
            Self::Warning => 1,
            Self::Observe => 2,
            Self::Healthy => 3,
        }
    }
}

struct AttentionRollup {
    attention_target_count: usize,
    clusters: Vec<AttentionCluster>,
    unclustered_targets: Vec<AttentionTarget>,
    runtime_policy_reasons: Vec<&'static AttentionReasonSpec>,
}

struct AttentionCluster {
    key: String,
    label: String,
    priority: AttentionPriority,
    targets: Vec<AttentionTarget>,
}

struct AttentionTarget {
    name: String,
    protocol: Option<String>,
    entry: Option<String>,
    priority: AttentionPriority,
    primary_module_family: Option<String>,
    automation_outcome: Option<String>,
    evidence_posture: Option<String>,
    reason_tags: Vec<String>,
}

struct ReasonCount {
    key: String,
    count: usize,
}

fn build_rollup(snapshot: &ApiSnapshot) -> AttentionRollup {
    let mut grouped = BTreeMap::<String, (String, Vec<AttentionTarget>)>::new();
    let mut unclustered_targets = Vec::new();
    let mut attention_target_count = 0usize;
    let runtime_policy_reasons = runtime_policy_attention_reasons();

    for name in &snapshot.target_names {
        let Some(target) = snapshot.target_snapshots.get(name) else {
            continue;
        };
        let rendered = attention_target(name, target);
        if rendered.priority == AttentionPriority::Healthy {
            continue;
        }
        attention_target_count += 1;
        let Some(surface) = target.protocol_surface.as_ref() else {
            unclustered_targets.push(rendered);
            continue;
        };
        let Some(cluster) = surface.cluster_hint.as_ref() else {
            unclustered_targets.push(rendered);
            continue;
        };
        grouped
            .entry(cluster.key.clone())
            .or_insert_with(|| (cluster.label.clone(), Vec::new()))
            .1
            .push(rendered);
    }

    let mut clusters = grouped
        .into_iter()
        .map(|(key, (label, mut targets))| {
            targets.sort_by_key(|target| (target.priority.rank(), target.name.clone()));
            let priority = targets
                .iter()
                .map(|target| target.priority)
                .min()
                .unwrap_or(AttentionPriority::Healthy);
            AttentionCluster {
                key,
                label,
                priority,
                targets,
            }
        })
        .collect::<Vec<_>>();
    clusters.sort_by_key(|cluster| {
        (
            cluster.priority.rank(),
            Reverse(cluster.targets.len()),
            cluster.key.clone(),
        )
    });
    unclustered_targets.sort_by_key(|target| (target.priority.rank(), target.name.clone()));

    AttentionRollup {
        attention_target_count,
        clusters,
        unclustered_targets,
        runtime_policy_reasons,
    }
}

fn attention_target(name: &str, target: &ApiTargetSnapshot) -> AttentionTarget {
    let reasons = attention_reasons(target);
    let priority = reasons
        .iter()
        .map(|reason| reason.priority)
        .min()
        .unwrap_or(AttentionPriority::Healthy);

    AttentionTarget {
        name: name.to_string(),
        protocol: target
            .protocol_surface
            .as_ref()
            .map(|surface| surface.protocol.clone()),
        entry: target
            .protocol_surface
            .as_ref()
            .map(|surface| surface.entry.clone()),
        priority,
        primary_module_family: target.primary_module_family.clone(),
        automation_outcome: target.automation_outcome.clone(),
        evidence_posture: target.evidence_posture.clone(),
        reason_tags: reasons
            .into_iter()
            .map(|reason| reason.key.to_string())
            .collect(),
    }
}

fn attention_reasons(target: &ApiTargetSnapshot) -> Vec<&'static AttentionReasonSpec> {
    let mut reasons = Vec::new();
    match target.automation_outcome.as_deref() {
        Some("targeted_escalation") => {
            reasons.extend(reason_spec("automation.targeted_escalation"));
        }
        Some("manual_review") => {
            reasons.extend(reason_spec("automation.manual_review"));
        }
        Some("collect_more_evidence") => {
            reasons.extend(reason_spec("automation.collect_more_evidence"));
        }
        _ => {}
    }
    match target.external_sidecar_trust_level.as_deref() {
        Some("unverified") => reasons.extend(reason_spec("sidecar.unverified")),
        Some("degraded") => reasons.extend(reason_spec("sidecar.degraded")),
        _ => {}
    }
    if target.external_capability_status.as_deref() == Some("unavailable") {
        reasons.extend(reason_spec("capability.unavailable"));
    }
    if target.has_external_sidecar_context && !target.has_external_capability_profile {
        reasons.extend(reason_spec("sidecar.context_without_profile"));
    }
    reasons
}

fn reason_spec(key: &str) -> Option<&'static AttentionReasonSpec> {
    REASON_SPECS.iter().find(|spec| spec.key == key)
}

fn runtime_policy_attention_reasons() -> Vec<&'static AttentionReasonSpec> {
    runtime_certificate_policy()
        .reasons
        .into_iter()
        .filter_map(|reason| reason_spec(reason.code))
        .collect()
}

fn append_attention_target_json(target: &mut String, rendered: &AttentionTarget) {
    target.push('{');
    target.push_str("\"name\":");
    append_json_string(target, &rendered.name);
    target.push_str(",\"protocol\":");
    append_optional_string_json(target, rendered.protocol.as_deref());
    target.push_str(",\"entry\":");
    append_optional_string_json(target, rendered.entry.as_deref());
    target.push_str(",\"priority\":");
    append_json_string(target, rendered.priority.label());
    target.push_str(",\"primary_module_family\":");
    append_optional_string_json(target, rendered.primary_module_family.as_deref());
    target.push_str(",\"automation_outcome\":");
    append_optional_string_json(target, rendered.automation_outcome.as_deref());
    target.push_str(",\"evidence_posture\":");
    append_optional_string_json(target, rendered.evidence_posture.as_deref());
    target.push_str(",\"reason_tags\":");
    append_string_list_json(target, &rendered.reason_tags);
    target.push('}');
}

fn append_optional_string_json(target: &mut String, value: Option<&str>) {
    if let Some(value) = value {
        append_json_string(target, value);
    } else {
        target.push_str("null");
    }
}

fn append_reason_catalog_json(target: &mut String) {
    target.push('[');
    for (index, reason) in REASON_SPECS.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        target.push('{');
        target.push_str("\"key\":");
        append_json_string(target, reason.key);
        target.push_str(",\"label\":");
        append_json_string(target, reason.label);
        target.push_str(",\"priority\":");
        append_json_string(target, reason.priority.label());
        target.push_str(",\"note\":");
        append_json_string(target, reason.note);
        target.push('}');
    }
    target.push(']');
}

fn append_runtime_policy_reasons_json(
    target: &mut String,
    reasons: &[&'static AttentionReasonSpec],
) {
    for (index, reason) in reasons.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        target.push('{');
        target.push_str("\"key\":");
        append_json_string(target, reason.key);
        target.push_str(",\"priority\":");
        append_json_string(target, reason.priority.label());
        target.push_str(",\"label\":");
        append_json_string(target, reason.label);
        target.push_str(",\"note\":");
        append_json_string(target, reason.note);
        target.push('}');
    }
}

fn append_runtime_policy_reason_counts_json(
    target: &mut String,
    reasons: &[&'static AttentionReasonSpec],
) {
    let mut counts = BTreeMap::<&'static str, (&'static str, usize)>::new();
    for reason in reasons {
        counts
            .entry(reason.key)
            .and_modify(|(_, count)| *count += 1)
            .or_insert((reason.priority.label(), 1));
    }
    let mut first = true;
    for (key, (priority, count)) in counts {
        if !first {
            target.push(',');
        }
        first = false;
        target.push('{');
        target.push_str("\"key\":");
        append_json_string(target, key);
        target.push_str(",\"priority\":");
        append_json_string(target, priority);
        target.push_str(",\"count\":");
        target.push_str(&count.to_string());
        target.push('}');
    }
}

fn reason_counts(targets: &[AttentionTarget]) -> Vec<ReasonCount> {
    let mut counts = BTreeMap::<String, usize>::new();
    for target in targets {
        for reason in &target.reason_tags {
            *counts.entry(reason.clone()).or_default() += 1;
        }
    }
    counts
        .into_iter()
        .map(|(key, count)| ReasonCount { key, count })
        .collect()
}

fn append_reason_counts_json(target: &mut String, counts: &[ReasonCount]) {
    for (index, reason) in counts.iter().enumerate() {
        if index > 0 {
            target.push(',');
        }
        target.push('{');
        target.push_str("\"key\":");
        append_json_string(target, &reason.key);
        target.push_str(",\"priority\":");
        append_json_string(
            target,
            reason_spec(&reason.key)
                .map_or(AttentionPriority::Warning, |spec| spec.priority)
                .label(),
        );
        target.push_str(",\"count\":");
        target.push_str(&reason.count.to_string());
        target.push('}');
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_attention_reason_degrades_to_warning_without_panicking() {
        let mut json = String::new();
        append_reason_counts_json(
            &mut json,
            &[ReasonCount {
                key: "future.policy.reason".into(),
                count: 1,
            }],
        );
        assert_eq!(
            json,
            "{\"key\":\"future.policy.reason\",\"priority\":\"warning\",\"count\":1}"
        );
    }
}
