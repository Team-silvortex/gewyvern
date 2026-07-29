use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use leselang_hir::{
    UI_WAIT_ENABLED_TIMEOUT_MS, UI_WAIT_FOCUSED_TIMEOUT_MS, UI_WAIT_REALIZED_TIMEOUT_MS,
    UI_WAIT_SELECTION_TIMEOUT_MS, UI_WAIT_VISIBLE_TIMEOUT_MS, UI_WAIT_WINDOW_OPEN_TIMEOUT_MS,
    UiFocusNavigationDirection, UiSelectionState,
};
use leselang_ui::{
    NodeId, UiActionKind, UiDocument, UiPatch, UiPresentationOperation, diff, fleet_document,
    validate_presentation_operation,
};
use leserpent_domain::{
    CAPABILITY_RUNTIME_READ, CapabilitySet, InMemoryControlPlane, Principal, Query, QueryEnvelope,
    QueryResult, RuntimeCapabilitySnapshot, RuntimeId, RuntimeListFilter,
};
use serde::Serialize;

#[derive(Serialize)]
struct Fixture<'a> {
    schema_version: u32,
    previous: &'a UiDocument,
    patch: &'a UiPatch,
    next: &'a UiDocument,
    presentation_operation: &'a UiPresentationOperation,
    navigation_operation: &'a UiPresentationOperation,
    navigation_first_operation: &'a UiPresentationOperation,
    navigation_last_operation: &'a UiPresentationOperation,
    scroll_operation: &'a UiPresentationOperation,
    assert_operation: &'a UiPresentationOperation,
    hidden_assert_operation: &'a UiPresentationOperation,
    hidden_wait_operation: &'a UiPresentationOperation,
    realized_assert_operation: &'a UiPresentationOperation,
    realized_wait_operation: &'a UiPresentationOperation,
    visible_wait_operation: &'a UiPresentationOperation,
    enabled_wait_operation: &'a UiPresentationOperation,
    disabled_wait_operation: &'a UiPresentationOperation,
    window_open_assert_operation: &'a UiPresentationOperation,
    window_open_wait_operation: &'a UiPresentationOperation,
    focused_wait_operation: &'a UiPresentationOperation,
    focused_assert_operation: &'a UiPresentationOperation,
    enabled_assert_operation: &'a UiPresentationOperation,
    disabled_assert_operation: &'a UiPresentationOperation,
    selection_assert_operation: &'a UiPresentationOperation,
    selection_wait_operation: &'a UiPresentationOperation,
    text_assert_operation: &'a UiPresentationOperation,
    automation_id_assert_operation: &'a UiPresentationOperation,
    node_kind_assert_operation: &'a UiPresentationOperation,
    action_kind_assert_operation: &'a UiPresentationOperation,
    form_field_assert_operation: &'a UiPresentationOperation,
    accessible_name_assert_operation: &'a UiPresentationOperation,
    accessible_description_assert_operation: &'a UiPresentationOperation,
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
    let navigation_operation = UiPresentationOperation::NavigateFocus {
        node_id: NodeId::new("runtime-runtime-a-inspect").unwrap(),
        direction: UiFocusNavigationDirection::Next,
    };
    let navigation_first_operation = UiPresentationOperation::NavigateFocus {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
        direction: UiFocusNavigationDirection::First,
    };
    let navigation_last_operation = UiPresentationOperation::NavigateFocus {
        node_id: NodeId::new("runtime-runtime-a-inspect").unwrap(),
        direction: UiFocusNavigationDirection::Last,
    };
    let scroll_operation = UiPresentationOperation::ScrollIntoView {
        node_id: NodeId::new("fleet-title").unwrap(),
    };
    let assert_operation = UiPresentationOperation::AssertVisible {
        node_id: NodeId::new("fleet-title").unwrap(),
    };
    let hidden_assert_operation = UiPresentationOperation::AssertHidden {
        node_id: NodeId::new("fleet-title").unwrap(),
    };
    let hidden_wait_operation = UiPresentationOperation::WaitHidden {
        node_id: NodeId::new("fleet-title").unwrap(),
        timeout_ms: UI_WAIT_VISIBLE_TIMEOUT_MS,
    };
    let realized_assert_operation = UiPresentationOperation::AssertRealized {
        node_id: NodeId::new("fleet-title").unwrap(),
    };
    let realized_wait_operation = UiPresentationOperation::WaitRealized {
        node_id: NodeId::new("fleet-title").unwrap(),
        timeout_ms: UI_WAIT_REALIZED_TIMEOUT_MS,
    };
    let visible_wait_operation = UiPresentationOperation::WaitVisible {
        node_id: NodeId::new("fleet-title").unwrap(),
        timeout_ms: UI_WAIT_VISIBLE_TIMEOUT_MS,
    };
    let enabled_wait_operation = UiPresentationOperation::WaitEnabled {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
        timeout_ms: UI_WAIT_ENABLED_TIMEOUT_MS,
    };
    let disabled_wait_operation = UiPresentationOperation::WaitDisabled {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
        timeout_ms: UI_WAIT_ENABLED_TIMEOUT_MS,
    };
    let window_open_assert_operation = UiPresentationOperation::AssertWindowOpen {
        node_id: NodeId::new("fleet-title").unwrap(),
    };
    let window_open_wait_operation = UiPresentationOperation::WaitWindowOpen {
        node_id: NodeId::new("fleet-title").unwrap(),
        timeout_ms: UI_WAIT_WINDOW_OPEN_TIMEOUT_MS,
    };
    let focused_wait_operation = UiPresentationOperation::WaitFocused {
        node_id: NodeId::new("runtime-runtime-a-inspect").unwrap(),
        timeout_ms: UI_WAIT_FOCUSED_TIMEOUT_MS,
    };
    let focused_assert_operation = UiPresentationOperation::AssertFocused {
        node_id: NodeId::new("runtime-runtime-a-inspect").unwrap(),
    };
    let enabled_assert_operation = UiPresentationOperation::AssertEnabled {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
    };
    let disabled_assert_operation = UiPresentationOperation::AssertDisabled {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
    };
    let selection_assert_operation = UiPresentationOperation::AssertSelection {
        node_id: NodeId::new("runtime-runtime-a").unwrap(),
        state: UiSelectionState::Selected,
    };
    let selection_wait_operation = UiPresentationOperation::WaitSelection {
        node_id: NodeId::new("runtime-runtime-b").unwrap(),
        state: UiSelectionState::Unselected,
        timeout_ms: UI_WAIT_SELECTION_TIMEOUT_MS,
    };
    let text_assert_operation = UiPresentationOperation::AssertText {
        node_id: NodeId::new("fleet-title").unwrap(),
        expected: "Runtime fleet".into(),
    };
    let automation_id_assert_operation = UiPresentationOperation::AssertAutomationId {
        node_id: NodeId::new("fleet-title").unwrap(),
        expected: "fleet-title".into(),
    };
    let node_kind_assert_operation = UiPresentationOperation::AssertNodeKind {
        node_id: NodeId::new("fleet-title").unwrap(),
        expected_kind: leselang_ui::UiNodeKind::Heading,
    };
    let action_kind_assert_operation = UiPresentationOperation::AssertActionKind {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
        expected_action_kind: UiActionKind::RuntimeRefresh,
    };
    let form_field_assert_operation = UiPresentationOperation::AssertFormField {
        node_id: NodeId::new("runtime-runtime-a-deploy").unwrap(),
        field: "pipeline_kind".into(),
        expected: "Pipeline kind".into(),
    };
    let accessible_name_assert_operation = UiPresentationOperation::AssertAccessibleName {
        node_id: NodeId::new("fleet-title").unwrap(),
        expected: "Runtime fleet".into(),
    };
    let accessible_description_assert_operation =
        UiPresentationOperation::AssertAccessibleDescription {
            node_id: NodeId::new("runtime-runtime-a-inspect").unwrap(),
            expected: "Open the read-only runtime workspace".into(),
        };
    validate_presentation_operation(&next, &presentation_operation).unwrap();
    validate_presentation_operation(&next, &navigation_operation).unwrap();
    validate_presentation_operation(&next, &navigation_first_operation).unwrap();
    validate_presentation_operation(&next, &navigation_last_operation).unwrap();
    validate_presentation_operation(&next, &scroll_operation).unwrap();
    validate_presentation_operation(&next, &assert_operation).unwrap();
    validate_presentation_operation(&next, &hidden_assert_operation).unwrap();
    validate_presentation_operation(&next, &hidden_wait_operation).unwrap();
    validate_presentation_operation(&next, &realized_assert_operation).unwrap();
    validate_presentation_operation(&next, &realized_wait_operation).unwrap();
    validate_presentation_operation(&next, &visible_wait_operation).unwrap();
    validate_presentation_operation(&next, &enabled_wait_operation).unwrap();
    validate_presentation_operation(&next, &disabled_wait_operation).unwrap();
    validate_presentation_operation(&next, &window_open_assert_operation).unwrap();
    validate_presentation_operation(&next, &window_open_wait_operation).unwrap();
    validate_presentation_operation(&next, &focused_wait_operation).unwrap();
    validate_presentation_operation(&next, &focused_assert_operation).unwrap();
    validate_presentation_operation(&next, &enabled_assert_operation).unwrap();
    validate_presentation_operation(&next, &disabled_assert_operation).unwrap();
    validate_presentation_operation(&next, &selection_assert_operation).unwrap();
    validate_presentation_operation(&next, &selection_wait_operation).unwrap();
    validate_presentation_operation(&next, &text_assert_operation).unwrap();
    validate_presentation_operation(&next, &automation_id_assert_operation).unwrap();
    validate_presentation_operation(&next, &node_kind_assert_operation).unwrap();
    validate_presentation_operation(&next, &action_kind_assert_operation).unwrap();
    validate_presentation_operation(&next, &form_field_assert_operation).unwrap();
    validate_presentation_operation(&next, &accessible_name_assert_operation).unwrap();
    validate_presentation_operation(&next, &accessible_description_assert_operation).unwrap();
    let mut bytes = serde_json::to_vec_pretty(&Fixture {
        schema_version: 1,
        previous: &previous,
        patch: &patch,
        next: &next,
        presentation_operation: &presentation_operation,
        navigation_operation: &navigation_operation,
        navigation_first_operation: &navigation_first_operation,
        navigation_last_operation: &navigation_last_operation,
        scroll_operation: &scroll_operation,
        assert_operation: &assert_operation,
        hidden_assert_operation: &hidden_assert_operation,
        hidden_wait_operation: &hidden_wait_operation,
        realized_assert_operation: &realized_assert_operation,
        realized_wait_operation: &realized_wait_operation,
        visible_wait_operation: &visible_wait_operation,
        enabled_wait_operation: &enabled_wait_operation,
        disabled_wait_operation: &disabled_wait_operation,
        window_open_assert_operation: &window_open_assert_operation,
        window_open_wait_operation: &window_open_wait_operation,
        focused_wait_operation: &focused_wait_operation,
        focused_assert_operation: &focused_assert_operation,
        enabled_assert_operation: &enabled_assert_operation,
        disabled_assert_operation: &disabled_assert_operation,
        selection_assert_operation: &selection_assert_operation,
        selection_wait_operation: &selection_wait_operation,
        text_assert_operation: &text_assert_operation,
        automation_id_assert_operation: &automation_id_assert_operation,
        node_kind_assert_operation: &node_kind_assert_operation,
        action_kind_assert_operation: &action_kind_assert_operation,
        form_field_assert_operation: &form_field_assert_operation,
        accessible_name_assert_operation: &accessible_name_assert_operation,
        accessible_description_assert_operation: &accessible_description_assert_operation,
    })
    .unwrap();
    bytes.push(b'\n');
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(output, bytes).unwrap();
}

fn fleet(runtimes: &[(&str, &str)]) -> QueryResult {
    let mut control = InMemoryControlPlane::default();
    for (id, name) in runtimes {
        let runtime_id = RuntimeId::new(*id).unwrap();
        let projection = control.register_runtime(runtime_id.clone(), *name, "fixture-endpoint");
        if *id == "runtime-a" {
            control
                .complete_runtime_capability_refresh(
                    &runtime_id,
                    projection.revision,
                    deployment_capabilities(),
                )
                .unwrap();
        }
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

fn deployment_capabilities() -> RuntimeCapabilitySnapshot {
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
    }
}
