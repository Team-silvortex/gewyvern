use std::collections::{BTreeMap, BTreeSet};

use crate::render_utils::{append_json_string, append_string_list_json};

use super::ApiSnapshot;

pub(super) fn api_runtime_capability_digest_json(snapshot: &ApiSnapshot) -> String {
    let digest = build_digest(snapshot);
    let mut json = String::with_capacity(4096);
    json.push_str("{\"surface\":\"runtime_capability_digest\",\"kind\":");
    append_json_string(&mut json, &snapshot.kind);
    json.push_str(",\"updated_unix_ms\":");
    json.push_str(&snapshot.updated_unix_ms.to_string());
    json.push_str(",\"target_count\":");
    json.push_str(&snapshot.target_names.len().to_string());
    json.push_str(",\"targets_with_protocol_surface\":");
    json.push_str(&digest.targets_with_protocol_surface.to_string());
    json.push_str(",\"targets_without_protocol_surface\":");
    json.push_str(&digest.targets_without_protocol_surface.to_string());
    json.push_str(",\"cluster_count\":");
    json.push_str(&digest.clusters.len().to_string());
    json.push_str(",\"protocol_count\":");
    json.push_str(&digest.protocol_count.to_string());
    json.push_str(",\"clusters\":[");
    for (index, cluster) in digest.clusters.iter().enumerate() {
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
        json.push_str(",\"sibling_protocols\":");
        append_string_list_json(&mut json, &cluster.sibling_protocols);
        json.push_str(",\"observed_protocol_count\":");
        json.push_str(&cluster.protocols.len().to_string());
        json.push_str(",\"protocols\":[");
        for (protocol_index, protocol) in cluster.protocols.iter().enumerate() {
            if protocol_index > 0 {
                json.push(',');
            }
            json.push('{');
            json.push_str("\"protocol\":");
            append_json_string(&mut json, &protocol.protocol);
            json.push_str(",\"entry_count\":");
            json.push_str(&protocol.entries.len().to_string());
            json.push_str(",\"entries\":");
            append_string_list_json(&mut json, &protocol.entries);
            json.push('}');
        }
        json.push_str("]}");
    }
    json.push_str("]}");
    json
}

#[derive(Default)]
struct Digest {
    targets_with_protocol_surface: usize,
    targets_without_protocol_surface: usize,
    protocol_count: usize,
    clusters: Vec<ClusterDigest>,
}

struct ClusterDigest {
    key: String,
    label: String,
    operator_hint: String,
    sibling_protocols: Vec<String>,
    protocols: Vec<ProtocolDigest>,
}

struct ProtocolDigest {
    protocol: String,
    entries: Vec<String>,
}

fn build_digest(snapshot: &ApiSnapshot) -> Digest {
    let mut by_cluster =
        BTreeMap::<String, (String, String, Vec<String>, BTreeMap<String, BTreeSet<String>>)>::new();
    let mut protocol_names = BTreeSet::new();
    let mut targets_with_protocol_surface = 0usize;
    let mut targets_without_protocol_surface = 0usize;

    for name in &snapshot.target_names {
        let Some(target) = snapshot.target_snapshots.get(name) else {
            targets_without_protocol_surface += 1;
            continue;
        };
        let Some(surface) = target.protocol_surface.as_ref() else {
            targets_without_protocol_surface += 1;
            continue;
        };
        targets_with_protocol_surface += 1;
        protocol_names.insert(surface.protocol.clone());
        let Some(cluster) = surface.cluster_hint.as_ref() else {
            continue;
        };
        let (_, _, _, protocols) = by_cluster.entry(cluster.key.clone()).or_insert_with(|| {
            (
                cluster.label.clone(),
                cluster.operator_hint.clone(),
                cluster.sibling_protocols.clone(),
                BTreeMap::new(),
            )
        });
        protocols
            .entry(surface.protocol.clone())
            .or_default()
            .insert(surface.entry.clone());
    }

    let clusters = by_cluster
        .into_iter()
        .map(
            |(key, (label, operator_hint, sibling_protocols, protocols))| ClusterDigest {
                key,
                label,
                operator_hint,
                sibling_protocols,
                protocols: protocols
                    .into_iter()
                    .map(|(protocol, entries)| ProtocolDigest {
                        protocol,
                        entries: entries.into_iter().collect(),
                    })
                    .collect(),
            },
        )
        .collect();

    Digest {
        targets_with_protocol_surface,
        targets_without_protocol_surface,
        protocol_count: protocol_names.len(),
        clusters,
    }
}
