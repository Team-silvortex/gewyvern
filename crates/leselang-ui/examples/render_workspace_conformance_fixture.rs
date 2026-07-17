use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use leselang_ui::{UiDocument, UiPatch, diff, runtime_workspace_document};
use leserpent_domain::{
    CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, CapabilitySet, Command, CommandEnvelope,
    CommandId, CommandOrigin, Confirmation, DOMAIN_SCHEMA_VERSION, IdempotencyKey,
    InMemoryControlPlane, Principal, Query, QueryEnvelope, QueryResult, Revision,
    RuntimeCapabilitySnapshot, RuntimeId,
};
use serde::Serialize;

#[derive(Serialize)]
struct Fixture<'a> {
    schema_version: u32,
    previous: &'a UiDocument,
    patch: &'a UiPatch,
    next: &'a UiDocument,
}

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: render_workspace_conformance_fixture OUTPUT");
    let mut control = InMemoryControlPlane::default();
    let runtime_id = RuntimeId::new("runtime-history").unwrap();
    control.register_runtime(
        runtime_id.clone(),
        "History Runtime",
        "fixture-workspace-endpoint",
    );
    for index in 0..32 {
        refresh(&mut control, &runtime_id, index, Revision(index + 1));
    }
    control
        .complete_runtime_capability_refresh(
            &runtime_id,
            Revision(33),
            RuntimeCapabilitySnapshot {
                source: "gewyvern-api".into(),
                service: "gewyvern-api".into(),
                version: "1.2.0".into(),
                latest_snapshot: true,
                authenticated_deployment: true,
                serve_required: true,
                external_sidecar_context: true,
                target_path_segment_encoding: "percent-encoding".into(),
                target_direct_path_chars: "A-Z a-z 0-9 . _ ~ :".into(),
                endpoints: vec!["/v1/capabilities".into(), "/v1/deployments".into()],
                extensions: BTreeMap::from([("protocol_catalog".into(), true)]),
            },
        )
        .unwrap();
    let previous = workspace(&control, &runtime_id);
    refresh(&mut control, &runtime_id, 32, Revision(34));
    let next = workspace(&control, &runtime_id);
    let patch = diff(&previous, &next).unwrap();
    let bytes = serde_json::to_vec_pretty(&Fixture {
        schema_version: 1,
        previous: &previous,
        patch: &patch,
        next: &next,
    })
    .unwrap();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(output, bytes).unwrap();
}

fn refresh(
    control: &mut InMemoryControlPlane,
    runtime_id: &RuntimeId,
    index: u64,
    expected_revision: Revision,
) {
    control
        .execute(CommandEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            command_id: CommandId::new(format!("history-command-{index}")).unwrap(),
            idempotency_key: IdempotencyKey::new(format!("history-effect-{index}")).unwrap(),
            expected_revision: Some(expected_revision),
            principal: Principal {
                id: "fixture".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
            origin: CommandOrigin::Gui,
            confirmation: Confirmation::Confirmed,
            dry_run: false,
            command: Command::RuntimeRefresh {
                runtime_id: runtime_id.clone(),
            },
        })
        .unwrap();
}

fn workspace(control: &InMemoryControlPlane, runtime_id: &RuntimeId) -> UiDocument {
    let query = |query| {
        control
            .query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "fixture".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query,
            })
            .unwrap()
    };
    let inspect = query(Query::RuntimeInspect {
        runtime_id: runtime_id.clone(),
    });
    let history = query(Query::RuntimeHistory {
        runtime_id: runtime_id.clone(),
    });
    assert!(matches!(inspect, QueryResult::RuntimeInspect { .. }));
    assert!(matches!(history, QueryResult::RuntimeHistory { .. }));
    runtime_workspace_document(&inspect, &history).unwrap()
}
