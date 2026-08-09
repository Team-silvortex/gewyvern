use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use leselang_hir::{
    UI_WAIT_ACCESSIBLE_DESCRIPTION_TIMEOUT_MS, UI_WAIT_ACCESSIBLE_NAME_TIMEOUT_MS,
    UI_WAIT_ACTION_AVAILABLE_TIMEOUT_MS, UI_WAIT_ACTION_KIND_TIMEOUT_MS,
    UI_WAIT_ACTION_LABEL_TIMEOUT_MS, UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS,
    UI_WAIT_CHILD_COUNT_TIMEOUT_MS, UI_WAIT_ENABLED_TIMEOUT_MS, UI_WAIT_FOCUSED_TIMEOUT_MS,
    UI_WAIT_FORM_FIELD_INPUT_KIND_TIMEOUT_MS, UI_WAIT_FORM_FIELD_MAX_LENGTH_TIMEOUT_MS,
    UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS, UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS,
    UI_WAIT_FORM_FIELD_TIMEOUT_MS, UI_WAIT_NODE_KIND_TIMEOUT_MS, UI_WAIT_REALIZED_TIMEOUT_MS,
    UI_WAIT_SELECTION_TIMEOUT_MS, UI_WAIT_TEXT_TIMEOUT_MS, UI_WAIT_UNFOCUSED_TIMEOUT_MS,
    UI_WAIT_VISIBLE_TIMEOUT_MS, UI_WAIT_WINDOW_CLOSED_TIMEOUT_MS, UI_WAIT_WINDOW_OPEN_TIMEOUT_MS,
    UiFocusNavigationDirection, UiSelectionState,
};
use leselang_ui::{
    NodeId, UiActionKind, UiAdapterBindingKind, UiAdapterManifest, UiDocument, UiFormInputKind,
    UiPatch, UiPresentationOperation, complete_ui_adapter_manifest, diff, fleet_document,
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
    activate_operation: &'a UiPresentationOperation,
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
    window_open_operation: &'a UiPresentationOperation,
    window_close_operation: &'a UiPresentationOperation,
    window_open_assert_operation: &'a UiPresentationOperation,
    window_open_wait_operation: &'a UiPresentationOperation,
    window_closed_assert_operation: &'a UiPresentationOperation,
    window_closed_wait_operation: &'a UiPresentationOperation,
    focused_wait_operation: &'a UiPresentationOperation,
    focused_assert_operation: &'a UiPresentationOperation,
    unfocused_wait_operation: &'a UiPresentationOperation,
    unfocused_assert_operation: &'a UiPresentationOperation,
    enabled_assert_operation: &'a UiPresentationOperation,
    disabled_assert_operation: &'a UiPresentationOperation,
    selection_assert_operation: &'a UiPresentationOperation,
    selection_wait_operation: &'a UiPresentationOperation,
    child_count_assert_operation: &'a UiPresentationOperation,
    child_count_wait_operation: &'a UiPresentationOperation,
    text_assert_operation: &'a UiPresentationOperation,
    text_wait_operation: &'a UiPresentationOperation,
    automation_id_assert_operation: &'a UiPresentationOperation,
    node_kind_assert_operation: &'a UiPresentationOperation,
    node_kind_wait_operation: &'a UiPresentationOperation,
    action_kind_assert_operation: &'a UiPresentationOperation,
    action_kind_wait_operation: &'a UiPresentationOperation,
    action_label_assert_operation: &'a UiPresentationOperation,
    action_label_wait_operation: &'a UiPresentationOperation,
    action_available_assert_operation: &'a UiPresentationOperation,
    action_available_wait_operation: &'a UiPresentationOperation,
    action_unavailable_reason_assert_operation: &'a UiPresentationOperation,
    action_unavailable_reason_wait_operation: &'a UiPresentationOperation,
    form_field_assert_operation: &'a UiPresentationOperation,
    form_field_input_kind_assert_operation: &'a UiPresentationOperation,
    form_field_required_assert_operation: &'a UiPresentationOperation,
    form_field_max_length_assert_operation: &'a UiPresentationOperation,
    form_field_placeholder_assert_operation: &'a UiPresentationOperation,
    form_field_wait_operation: &'a UiPresentationOperation,
    form_field_input_kind_wait_operation: &'a UiPresentationOperation,
    form_field_required_wait_operation: &'a UiPresentationOperation,
    form_field_max_length_wait_operation: &'a UiPresentationOperation,
    form_field_placeholder_wait_operation: &'a UiPresentationOperation,
    accessible_name_assert_operation: &'a UiPresentationOperation,
    accessible_name_wait_operation: &'a UiPresentationOperation,
    accessible_description_assert_operation: &'a UiPresentationOperation,
    accessible_description_wait_operation: &'a UiPresentationOperation,
    adapter_manifest: &'a UiAdapterManifest,
    generated_adapter_manifest: &'a UiAdapterManifest,
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
    let activate_operation = UiPresentationOperation::Activate {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
    };
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
    let window_open_operation = UiPresentationOperation::OpenWindow {
        node_id: NodeId::new("fleet-title").unwrap(),
    };
    let window_close_operation = UiPresentationOperation::CloseWindow {
        node_id: NodeId::new("fleet-title").unwrap(),
    };
    let window_open_assert_operation = UiPresentationOperation::AssertWindowOpen {
        node_id: NodeId::new("fleet-title").unwrap(),
    };
    let window_open_wait_operation = UiPresentationOperation::WaitWindowOpen {
        node_id: NodeId::new("fleet-title").unwrap(),
        timeout_ms: UI_WAIT_WINDOW_OPEN_TIMEOUT_MS,
    };
    let window_closed_assert_operation = UiPresentationOperation::AssertWindowClosed {
        node_id: NodeId::new("fleet-title").unwrap(),
    };
    let window_closed_wait_operation = UiPresentationOperation::WaitWindowClosed {
        node_id: NodeId::new("fleet-title").unwrap(),
        timeout_ms: UI_WAIT_WINDOW_CLOSED_TIMEOUT_MS,
    };
    let focused_wait_operation = UiPresentationOperation::WaitFocused {
        node_id: NodeId::new("runtime-runtime-a-inspect").unwrap(),
        timeout_ms: UI_WAIT_FOCUSED_TIMEOUT_MS,
    };
    let focused_assert_operation = UiPresentationOperation::AssertFocused {
        node_id: NodeId::new("runtime-runtime-a-inspect").unwrap(),
    };
    let unfocused_wait_operation = UiPresentationOperation::WaitUnfocused {
        node_id: NodeId::new("runtime-runtime-a-inspect").unwrap(),
        timeout_ms: UI_WAIT_UNFOCUSED_TIMEOUT_MS,
    };
    let unfocused_assert_operation = UiPresentationOperation::AssertUnfocused {
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
    let child_count_assert_operation = UiPresentationOperation::AssertChildCount {
        node_id: NodeId::new("fleet-root").unwrap(),
        count: 3,
    };
    let child_count_wait_operation = UiPresentationOperation::WaitChildCount {
        node_id: NodeId::new("fleet-root").unwrap(),
        count: 4,
        timeout_ms: UI_WAIT_CHILD_COUNT_TIMEOUT_MS,
    };
    let text_assert_operation = UiPresentationOperation::AssertText {
        node_id: NodeId::new("fleet-title").unwrap(),
        expected: "Runtime fleet".into(),
    };
    let text_wait_operation = UiPresentationOperation::WaitText {
        node_id: NodeId::new("fleet-title").unwrap(),
        expected: "Runtime fleet".into(),
        timeout_ms: UI_WAIT_TEXT_TIMEOUT_MS,
    };
    let automation_id_assert_operation = UiPresentationOperation::AssertAutomationId {
        node_id: NodeId::new("fleet-title").unwrap(),
        expected: "fleet-title".into(),
    };
    let node_kind_assert_operation = UiPresentationOperation::AssertNodeKind {
        node_id: NodeId::new("fleet-title").unwrap(),
        expected_kind: leselang_ui::UiNodeKind::Heading,
    };
    let node_kind_wait_operation = UiPresentationOperation::WaitNodeKind {
        node_id: NodeId::new("fleet-title").unwrap(),
        expected_kind: leselang_ui::UiNodeKind::Heading,
        timeout_ms: UI_WAIT_NODE_KIND_TIMEOUT_MS,
    };
    let action_kind_assert_operation = UiPresentationOperation::AssertActionKind {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
        expected_action_kind: UiActionKind::RuntimeRefresh,
    };
    let action_kind_wait_operation = UiPresentationOperation::WaitActionKind {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
        expected_action_kind: UiActionKind::RuntimeRefresh,
        timeout_ms: UI_WAIT_ACTION_KIND_TIMEOUT_MS,
    };
    let action_label_assert_operation = UiPresentationOperation::AssertActionLabel {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
        expected: "Refresh runtime".into(),
    };
    let action_label_wait_operation = UiPresentationOperation::WaitActionLabel {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
        expected: "Refresh runtime".into(),
        timeout_ms: UI_WAIT_ACTION_LABEL_TIMEOUT_MS,
    };
    let action_available_assert_operation = UiPresentationOperation::AssertActionAvailable {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
    };
    let action_available_wait_operation = UiPresentationOperation::WaitActionAvailable {
        node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
        timeout_ms: UI_WAIT_ACTION_AVAILABLE_TIMEOUT_MS,
    };
    let action_unavailable_reason_assert_operation =
        UiPresentationOperation::AssertActionUnavailableReason {
            node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
            expected: Some("Verification action is temporarily unavailable".into()),
        };
    let action_unavailable_reason_wait_operation =
        UiPresentationOperation::WaitActionUnavailableReason {
            node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
            expected: Some("Verification action is temporarily unavailable".into()),
            timeout_ms: UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS,
        };
    let form_field_assert_operation = UiPresentationOperation::AssertFormField {
        node_id: NodeId::new("runtime-runtime-a-deploy").unwrap(),
        field: "pipeline_kind".into(),
        expected: "Pipeline kind".into(),
    };
    let form_field_input_kind_assert_operation =
        UiPresentationOperation::AssertFormFieldInputKind {
            node_id: NodeId::new("runtime-runtime-a-deploy").unwrap(),
            field: "pipeline_kind".into(),
            input_kind: UiFormInputKind::PathToken,
        };
    let form_field_required_assert_operation = UiPresentationOperation::AssertFormFieldRequired {
        node_id: NodeId::new("runtime-runtime-a-deploy").unwrap(),
        field: "pipeline_kind".into(),
        required: true,
    };
    let form_field_max_length_assert_operation =
        UiPresentationOperation::AssertFormFieldMaxLength {
            node_id: NodeId::new("runtime-runtime-a-deploy").unwrap(),
            field: "pipeline_kind".into(),
            max_length: 128,
        };
    let form_field_placeholder_assert_operation =
        UiPresentationOperation::AssertFormFieldPlaceholder {
            node_id: NodeId::new("runtime-runtime-a-deploy").unwrap(),
            field: "pipeline_kind".into(),
            expected: Some("http/request".into()),
        };
    let form_field_wait_operation = UiPresentationOperation::WaitFormField {
        node_id: NodeId::new("runtime-runtime-a-deploy").unwrap(),
        field: "pipeline_kind".into(),
        expected: "Pipeline kind".into(),
        timeout_ms: UI_WAIT_FORM_FIELD_TIMEOUT_MS,
    };
    let form_field_input_kind_wait_operation = UiPresentationOperation::WaitFormFieldInputKind {
        node_id: NodeId::new("runtime-runtime-a-deploy").unwrap(),
        field: "pipeline_kind".into(),
        input_kind: UiFormInputKind::PathToken,
        timeout_ms: UI_WAIT_FORM_FIELD_INPUT_KIND_TIMEOUT_MS,
    };
    let form_field_required_wait_operation = UiPresentationOperation::WaitFormFieldRequired {
        node_id: NodeId::new("runtime-runtime-a-deploy").unwrap(),
        field: "pipeline_kind".into(),
        required: true,
        timeout_ms: UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS,
    };
    let form_field_max_length_wait_operation = UiPresentationOperation::WaitFormFieldMaxLength {
        node_id: NodeId::new("runtime-runtime-a-deploy").unwrap(),
        field: "pipeline_kind".into(),
        max_length: 128,
        timeout_ms: UI_WAIT_FORM_FIELD_MAX_LENGTH_TIMEOUT_MS,
    };
    let form_field_placeholder_wait_operation = UiPresentationOperation::WaitFormFieldPlaceholder {
        node_id: NodeId::new("runtime-runtime-a-deploy").unwrap(),
        field: "pipeline_kind".into(),
        expected: Some("http/request".into()),
        timeout_ms: UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS,
    };
    let accessible_name_assert_operation = UiPresentationOperation::AssertAccessibleName {
        node_id: NodeId::new("fleet-title").unwrap(),
        expected: "Runtime fleet".into(),
    };
    let accessible_name_wait_operation = UiPresentationOperation::WaitAccessibleName {
        node_id: NodeId::new("fleet-title").unwrap(),
        expected: "Runtime fleet".into(),
        timeout_ms: UI_WAIT_ACCESSIBLE_NAME_TIMEOUT_MS,
    };
    let accessible_description_assert_operation =
        UiPresentationOperation::AssertAccessibleDescription {
            node_id: NodeId::new("runtime-runtime-a-inspect").unwrap(),
            expected: "Open the read-only runtime workspace".into(),
        };
    let accessible_description_wait_operation =
        UiPresentationOperation::WaitAccessibleDescription {
            node_id: NodeId::new("runtime-runtime-a-inspect").unwrap(),
            expected: "Open the read-only runtime workspace".into(),
            timeout_ms: UI_WAIT_ACCESSIBLE_DESCRIPTION_TIMEOUT_MS,
        };
    let adapter_manifest = complete_ui_adapter_manifest(
        "avalonia-renderer",
        "Avalonia",
        UiAdapterBindingKind::DeveloperOwnedAdapter,
    )
    .unwrap();
    let generated_adapter_manifest = complete_ui_adapter_manifest(
        "web-generated-binding",
        "TypeScript web",
        UiAdapterBindingKind::GeneratedFrameworkBinding,
    )
    .unwrap();
    validate_presentation_operation(&next, &activate_operation).unwrap();
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
    validate_presentation_operation(&next, &window_open_operation).unwrap();
    validate_presentation_operation(&next, &window_close_operation).unwrap();
    validate_presentation_operation(&next, &window_open_assert_operation).unwrap();
    validate_presentation_operation(&next, &window_open_wait_operation).unwrap();
    validate_presentation_operation(&next, &window_closed_assert_operation).unwrap();
    validate_presentation_operation(&next, &window_closed_wait_operation).unwrap();
    validate_presentation_operation(&next, &focused_wait_operation).unwrap();
    validate_presentation_operation(&next, &focused_assert_operation).unwrap();
    validate_presentation_operation(&next, &unfocused_wait_operation).unwrap();
    validate_presentation_operation(&next, &unfocused_assert_operation).unwrap();
    validate_presentation_operation(&next, &enabled_assert_operation).unwrap();
    validate_presentation_operation(&next, &disabled_assert_operation).unwrap();
    validate_presentation_operation(&next, &selection_assert_operation).unwrap();
    validate_presentation_operation(&next, &selection_wait_operation).unwrap();
    validate_presentation_operation(&previous, &child_count_assert_operation).unwrap();
    validate_presentation_operation(&next, &child_count_wait_operation).unwrap();
    validate_presentation_operation(&next, &text_assert_operation).unwrap();
    validate_presentation_operation(&next, &text_wait_operation).unwrap();
    validate_presentation_operation(&next, &automation_id_assert_operation).unwrap();
    validate_presentation_operation(&next, &node_kind_assert_operation).unwrap();
    validate_presentation_operation(&next, &action_kind_assert_operation).unwrap();
    validate_presentation_operation(&next, &action_label_assert_operation).unwrap();
    validate_presentation_operation(&next, &action_label_wait_operation).unwrap();
    validate_presentation_operation(&next, &action_available_assert_operation).unwrap();
    validate_presentation_operation(&next, &action_available_wait_operation).unwrap();
    validate_presentation_operation(&next, &action_unavailable_reason_assert_operation).unwrap();
    validate_presentation_operation(&next, &action_unavailable_reason_wait_operation).unwrap();
    validate_presentation_operation(&next, &form_field_assert_operation).unwrap();
    validate_presentation_operation(&next, &form_field_input_kind_assert_operation).unwrap();
    validate_presentation_operation(&next, &form_field_required_assert_operation).unwrap();
    validate_presentation_operation(&next, &form_field_max_length_assert_operation).unwrap();
    validate_presentation_operation(&next, &form_field_placeholder_assert_operation).unwrap();
    validate_presentation_operation(&next, &form_field_wait_operation).unwrap();
    validate_presentation_operation(&next, &form_field_input_kind_wait_operation).unwrap();
    validate_presentation_operation(&next, &form_field_required_wait_operation).unwrap();
    validate_presentation_operation(&next, &form_field_max_length_wait_operation).unwrap();
    validate_presentation_operation(&next, &form_field_placeholder_wait_operation).unwrap();
    validate_presentation_operation(&next, &accessible_name_assert_operation).unwrap();
    validate_presentation_operation(&next, &accessible_name_wait_operation).unwrap();
    validate_presentation_operation(&next, &accessible_description_assert_operation).unwrap();
    validate_presentation_operation(&next, &accessible_description_wait_operation).unwrap();
    let mut bytes = serde_json::to_vec_pretty(&Fixture {
        schema_version: 1,
        previous: &previous,
        patch: &patch,
        next: &next,
        activate_operation: &activate_operation,
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
        window_open_operation: &window_open_operation,
        window_close_operation: &window_close_operation,
        window_open_assert_operation: &window_open_assert_operation,
        window_open_wait_operation: &window_open_wait_operation,
        window_closed_assert_operation: &window_closed_assert_operation,
        window_closed_wait_operation: &window_closed_wait_operation,
        focused_wait_operation: &focused_wait_operation,
        focused_assert_operation: &focused_assert_operation,
        unfocused_wait_operation: &unfocused_wait_operation,
        unfocused_assert_operation: &unfocused_assert_operation,
        enabled_assert_operation: &enabled_assert_operation,
        disabled_assert_operation: &disabled_assert_operation,
        selection_assert_operation: &selection_assert_operation,
        selection_wait_operation: &selection_wait_operation,
        child_count_assert_operation: &child_count_assert_operation,
        child_count_wait_operation: &child_count_wait_operation,
        text_assert_operation: &text_assert_operation,
        text_wait_operation: &text_wait_operation,
        automation_id_assert_operation: &automation_id_assert_operation,
        node_kind_assert_operation: &node_kind_assert_operation,
        node_kind_wait_operation: &node_kind_wait_operation,
        action_kind_assert_operation: &action_kind_assert_operation,
        action_kind_wait_operation: &action_kind_wait_operation,
        action_label_assert_operation: &action_label_assert_operation,
        action_label_wait_operation: &action_label_wait_operation,
        action_available_assert_operation: &action_available_assert_operation,
        action_available_wait_operation: &action_available_wait_operation,
        action_unavailable_reason_assert_operation: &action_unavailable_reason_assert_operation,
        action_unavailable_reason_wait_operation: &action_unavailable_reason_wait_operation,
        form_field_assert_operation: &form_field_assert_operation,
        form_field_input_kind_assert_operation: &form_field_input_kind_assert_operation,
        form_field_required_assert_operation: &form_field_required_assert_operation,
        form_field_max_length_assert_operation: &form_field_max_length_assert_operation,
        form_field_placeholder_assert_operation: &form_field_placeholder_assert_operation,
        form_field_wait_operation: &form_field_wait_operation,
        form_field_input_kind_wait_operation: &form_field_input_kind_wait_operation,
        form_field_required_wait_operation: &form_field_required_wait_operation,
        form_field_max_length_wait_operation: &form_field_max_length_wait_operation,
        form_field_placeholder_wait_operation: &form_field_placeholder_wait_operation,
        accessible_name_assert_operation: &accessible_name_assert_operation,
        accessible_name_wait_operation: &accessible_name_wait_operation,
        accessible_description_assert_operation: &accessible_description_assert_operation,
        accessible_description_wait_operation: &accessible_description_wait_operation,
        adapter_manifest: &adapter_manifest,
        generated_adapter_manifest: &generated_adapter_manifest,
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
