use std::fs;
use std::path::PathBuf;

use leselang_ui::{
    NodeId, UiDocument, UiPatch, UiPresentationOperation, diff, fleet_document,
    validate_presentation_operation,
};
use leserpent_domain::{
    CAPABILITY_RUNTIME_READ, CapabilitySet, InMemoryControlPlane, Principal, Query, QueryEnvelope,
    QueryResult, RuntimeId, RuntimeListFilter,
};
use serde::Serialize;

#[derive(Serialize)]
struct Fixture<'a> {
    schema_version: u32,
    previous: &'a UiDocument,
    patch: &'a UiPatch,
    next: &'a UiDocument,
    presentation_operation: &'a UiPresentationOperation,
    scroll_operation: &'a UiPresentationOperation,
    assert_operation: &'a UiPresentationOperation,
    focused_assert_operation: &'a UiPresentationOperation,
    enabled_assert_operation: &'a UiPresentationOperation,
    text_assert_operation: &'a UiPresentationOperation,
    accessible_name_assert_operation: &'a UiPresentationOperation,
}

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: render_presentation_conformance_fixture OUTPUT");
    let previous = fleet_document(&fleet(&[("runtime-a", "Runtime A")])).unwrap();
    let next = fleet_document(&fleet(&[
        ("runtime-a", "Runtime A"),
        ("runtime-b", "Runtime B"),
    ]))
    .unwrap();
    let patch = diff(&previous, &next).unwrap();
    let presentation_operation = UiPresentationOperation::Focus {
        node_id: NodeId::new("runtime-runtime-a-inspect").unwrap(),
    };
    let scroll_operation = UiPresentationOperation::ScrollIntoView {
        node_id: NodeId::new("fleet-title").unwrap(),
    };
    let assert_operation = UiPresentationOperation::AssertVisible {
        node_id: NodeId::new("fleet-title").unwrap(),
    };
    let focused_assert_operation = UiPresentationOperation::AssertFocused {
        node_id: NodeId::new("runtime-runtime-a-inspect").unwrap(),
    };
    let enabled_assert_operation = UiPresentationOperation::AssertEnabled {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
    };
    let text_assert_operation = UiPresentationOperation::AssertText {
        node_id: NodeId::new("fleet-title").unwrap(),
        expected: "Runtime fleet".into(),
    };
    let accessible_name_assert_operation = UiPresentationOperation::AssertAccessibleName {
        node_id: NodeId::new("fleet-title").unwrap(),
        expected: "Runtime fleet".into(),
    };
    validate_presentation_operation(&next, &presentation_operation).unwrap();
    validate_presentation_operation(&next, &scroll_operation).unwrap();
    validate_presentation_operation(&next, &assert_operation).unwrap();
    validate_presentation_operation(&next, &focused_assert_operation).unwrap();
    validate_presentation_operation(&next, &enabled_assert_operation).unwrap();
    validate_presentation_operation(&next, &text_assert_operation).unwrap();
    validate_presentation_operation(&next, &accessible_name_assert_operation).unwrap();
    let bytes = serde_json::to_vec_pretty(&Fixture {
        schema_version: 1,
        previous: &previous,
        patch: &patch,
        next: &next,
        presentation_operation: &presentation_operation,
        scroll_operation: &scroll_operation,
        assert_operation: &assert_operation,
        focused_assert_operation: &focused_assert_operation,
        enabled_assert_operation: &enabled_assert_operation,
        text_assert_operation: &text_assert_operation,
        accessible_name_assert_operation: &accessible_name_assert_operation,
    })
    .unwrap();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(output, bytes).unwrap();
}

fn fleet(runtimes: &[(&str, &str)]) -> QueryResult {
    let mut control = InMemoryControlPlane::default();
    for (id, name) in runtimes {
        control.register_runtime(RuntimeId::new(*id).unwrap(), *name, "fixture-endpoint");
    }
    control
        .query(QueryEnvelope {
            schema_version: leserpent_domain::DOMAIN_SCHEMA_VERSION,
            principal: Principal {
                id: "presentation-fixture".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            query: Query::RuntimeList {
                filter: RuntimeListFilter::default(),
            },
        })
        .unwrap()
}
