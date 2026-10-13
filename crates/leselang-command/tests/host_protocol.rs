use leselang_command::{
    EffectOperation, LoweringContext, LoweringError, PresentationEnvelope, PresentationOperation,
    PresentationResult, lower_effect,
};
use leselang_hir::{
    Effect, UiFocusNavigationDirection, UiFormInputKind, UiFormRequirementState, UiSelectionState,
    UiSemanticActionKind, UiSemanticNodeKind, lower,
};
use leselang_host_contract::{CapabilitySet, CommandOrigin, Confirmation, Principal};
use leselang_syntax::parse;
use leserpent_domain::{CommandId, IdempotencyKey};
use serde_json::{Value, json};

struct Case {
    operation: PresentationOperation,
    reply: PresentationResult,
    operation_wire: Value,
    reply_wire: Value,
}

// A closed wire matrix: adding a protocol variant must update both exhaustive matches.
macro_rules! protocol_cases {
    ($($variant:ident => $kind:literal {
        $($field:ident: ($native:expr, $wire:expr)),* $(,)?
    } $(reply { $($extra:ident: ($extra_native:expr, $extra_wire:expr)),* })?),* $(,)?) => {
        fn cases() -> Vec<Case> {
            vec![$(Case {
                operation: PresentationOperation::$variant { $($field: $native),* },
                reply: PresentationResult::$variant { $($field: $native,)* $($($extra: $extra_native),*)? },
                operation_wire: json!({"kind": $kind, $(stringify!($field): $wire),*}),
                reply_wire: json!({"kind": $kind, $(stringify!($field): $wire,)* $($(stringify!($extra): $extra_wire),*)?}),
            }),*]
        }
        fn operation_name(operation: &PresentationOperation) -> &'static str {
            match operation { $(PresentationOperation::$variant { .. } => $kind),* }
        }
        fn reply_name(reply: &PresentationResult) -> &'static str {
            match reply { $(PresentationResult::$variant { .. } => $kind),* }
        }
    };
}

protocol_cases! {
    Activate => "activate" {
        node_id: ("node-a".into(), "node-a")
    },
    Focus => "focus" {
        node_id: ("node-a".into(), "node-a")
    },
    NavigateFocus => "navigate_focus" {
        node_id: ("node-a".into(), "node-a"),
        direction: (UiFocusNavigationDirection::Next, "next")
    } reply { focused_node_id: ("child-a".into(), "child-a") },
    ScrollIntoView => "scroll_into_view" {
        node_id: ("node-a".into(), "node-a")
    },
    AssertVisible => "assert_visible" {
        node_id: ("node-a".into(), "node-a")
    },
    AssertHidden => "assert_hidden" {
        node_id: ("node-a".into(), "node-a")
    },
    WaitHidden => "wait_hidden" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    AssertRealized => "assert_realized" {
        node_id: ("node-a".into(), "node-a")
    },
    WaitRealized => "wait_realized" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    WaitVisible => "wait_visible" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    WaitEnabled => "wait_enabled" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    WaitDisabled => "wait_disabled" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    OpenWindow => "open_window" {
        node_id: ("node-a".into(), "node-a")
    },
    CloseWindow => "close_window" {
        node_id: ("node-a".into(), "node-a")
    },
    AssertWindowOpen => "assert_window_open" {
        node_id: ("node-a".into(), "node-a")
    },
    WaitWindowOpen => "wait_window_open" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    AssertWindowClosed => "assert_window_closed" {
        node_id: ("node-a".into(), "node-a")
    },
    WaitWindowClosed => "wait_window_closed" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    WaitFocused => "wait_focused" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    AssertFocused => "assert_focused" {
        node_id: ("node-a".into(), "node-a")
    },
    WaitUnfocused => "wait_unfocused" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    AssertUnfocused => "assert_unfocused" {
        node_id: ("node-a".into(), "node-a")
    },
    AssertEnabled => "assert_enabled" {
        node_id: ("node-a".into(), "node-a")
    },
    AssertDisabled => "assert_disabled" {
        node_id: ("node-a".into(), "node-a")
    },
    AssertChildCount => "assert_child_count" {
        node_id: ("node-a".into(), "node-a"),
        count: (3, 3)
    },
    WaitChildCount => "wait_child_count" {
        node_id: ("node-a".into(), "node-a"),
        count: (3, 3),
        timeout_ms: (5000, 5000)
    },
    SetSelection => "set_selection" {
        node_id: ("node-a".into(), "node-a"),
        state: (UiSelectionState::Selected, "selected")
    },
    AssertSelection => "assert_selection" {
        node_id: ("node-a".into(), "node-a"),
        state: (UiSelectionState::Selected, "selected")
    },
    WaitSelection => "wait_selection" {
        node_id: ("node-a".into(), "node-a"),
        state: (UiSelectionState::Selected, "selected"),
        timeout_ms: (5000, 5000)
    },
    AssertText => "assert_text" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready")
    },
    WaitText => "wait_text" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertAutomationId => "assert_automation_id" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready")
    },
    WaitAutomationId => "wait_automation_id" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertNodeKind => "assert_node_kind" {
        node_id: ("node-a".into(), "node-a"),
        expected_kind: (UiSemanticNodeKind::RuntimeCard, "runtime_card")
    },
    WaitNodeKind => "wait_node_kind" {
        node_id: ("node-a".into(), "node-a"),
        expected_kind: (UiSemanticNodeKind::RuntimeCard, "runtime_card"),
        timeout_ms: (5000, 5000)
    },
    AssertActionKind => "assert_action_kind" {
        node_id: ("node-a".into(), "node-a"),
        expected_kind: (UiSemanticActionKind::RuntimeInspect, "runtime_inspect")
    },
    WaitActionKind => "wait_action_kind" {
        node_id: ("node-a".into(), "node-a"),
        expected_kind: (UiSemanticActionKind::RuntimeInspect, "runtime_inspect"),
        timeout_ms: (5000, 5000)
    },
    AssertActionLabel => "assert_action_label" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready")
    },
    WaitActionLabel => "wait_action_label" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertActionAvailable => "assert_action_available" {
        node_id: ("node-a".into(), "node-a")
    },
    WaitActionAvailable => "wait_action_available" {
        node_id: ("node-a".into(), "node-a"),
        timeout_ms: (5000, 5000)
    },
    AssertActionUnavailableReason => "assert_action_unavailable_reason" {
        node_id: ("node-a".into(), "node-a"),
        expected: (Some("ready".into()), "ready")
    },
    WaitActionUnavailableReason => "wait_action_unavailable_reason" {
        node_id: ("node-a".into(), "node-a"),
        expected: (Some("ready".into()), "ready"),
        timeout_ms: (5000, 5000)
    },
    SubmitForm => "submit_form" {
        node_id: ("node-a".into(), "node-a")
    },
    CancelForm => "cancel_form" {
        node_id: ("node-a".into(), "node-a")
    },
    SetFormValue => "set_form_value" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        value: ("ready".into(), "ready")
    },
    AssertFormValue => "assert_form_value" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        expected: ("ready".into(), "ready")
    },
    WaitFormValue => "wait_form_value" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertFormField => "assert_form_field" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        expected: ("ready".into(), "ready")
    },
    WaitFormField => "wait_form_field" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertFormFieldInputKind => "assert_form_field_input_kind" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        input_kind: (UiFormInputKind::TrimmedText, "trimmed_text")
    },
    WaitFormFieldInputKind => "wait_form_field_input_kind" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        input_kind: (UiFormInputKind::TrimmedText, "trimmed_text"),
        timeout_ms: (5000, 5000)
    },
    AssertFormFieldRequired => "assert_form_field_required" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        state: (UiFormRequirementState::Required, "required")
    },
    WaitFormFieldRequired => "wait_form_field_required" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        state: (UiFormRequirementState::Required, "required"),
        timeout_ms: (5000, 5000)
    },
    AssertFormFieldMaxLength => "assert_form_field_max_length" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        max_length: (3, 3)
    },
    WaitFormFieldMaxLength => "wait_form_field_max_length" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        max_length: (3, 3),
        timeout_ms: (5000, 5000)
    },
    AssertFormFieldPlaceholder => "assert_form_field_placeholder" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        expected: (Some("ready".into()), "ready")
    },
    WaitFormFieldPlaceholder => "wait_form_field_placeholder" {
        node_id: ("node-a".into(), "node-a"),
        field: ("ready".into(), "ready"),
        expected: (Some("ready".into()), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertAccessibleName => "assert_accessible_name" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready")
    },
    WaitAccessibleName => "wait_accessible_name" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
    AssertAccessibleDescription => "assert_accessible_description" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready")
    },
    WaitAccessibleDescription => "wait_accessible_description" {
        node_id: ("node-a".into(), "node-a"),
        expected: ("ready".into(), "ready"),
        timeout_ms: (5000, 5000)
    },
}

#[test]
fn all_presentation_operations_and_replies_preserve_the_closed_wire_contract() {
    let cases = cases();
    assert_eq!(cases.len(), 62);
    for case in cases {
        assert_eq!(
            serde_json::to_value(&case.operation).unwrap(),
            case.operation_wire
        );
        assert_eq!(serde_json::to_value(&case.reply).unwrap(), case.reply_wire);
        assert_eq!(operation_name(&case.operation), reply_name(&case.reply));
        let operation: PresentationOperation = serde_json::from_value(case.operation_wire).unwrap();
        let reply: PresentationResult = serde_json::from_value(case.reply_wire).unwrap();
        assert_eq!(operation, case.operation);
        assert_eq!(reply, case.reply);
        assert_eq!(
            serde_json::to_vec(&operation).unwrap(),
            serde_json::to_vec(&case.operation).unwrap()
        );
        assert_eq!(
            serde_json::to_vec(&reply).unwrap(),
            serde_json::to_vec(&case.reply).unwrap()
        );
    }
}

#[test]
fn all_presentation_operations_and_replies_reject_unknown_fields_and_wrong_field_types() {
    for case in cases() {
        for (original, operation) in [(case.operation_wire, true), (case.reply_wire, false)] {
            let rejected = |wire: Value| {
                if operation {
                    assert!(serde_json::from_value::<PresentationOperation>(wire).is_err());
                } else {
                    assert!(serde_json::from_value::<PresentationResult>(wire).is_err());
                }
            };
            let mut extra = original.clone();
            extra["unexpected"] = json!("not-allowed");
            rejected(extra);
            for key in original.as_object().unwrap().keys() {
                let mut wrong = original.clone();
                wrong[key] = json!(false);
                rejected(wrong);
            }
            let mut missing = original.clone();
            missing.as_object_mut().unwrap().remove("node_id");
            rejected(missing);
        }
    }
}

#[test]
fn optional_text_and_focus_navigation_keep_their_distinct_reply_shapes() {
    for kind in [
        "assert_action_unavailable_reason",
        "wait_action_unavailable_reason",
        "assert_form_field_placeholder",
        "wait_form_field_placeholder",
    ] {
        let mut case = cases()
            .into_iter()
            .find(|case| operation_name(&case.operation) == kind)
            .unwrap();
        for wire in [&mut case.operation_wire, &mut case.reply_wire] {
            wire["expected"] = Value::Null;
        }
        let operation: PresentationOperation =
            serde_json::from_value(case.operation_wire.clone()).unwrap();
        let reply: PresentationResult = serde_json::from_value(case.reply_wire.clone()).unwrap();
        assert_eq!(
            serde_json::to_value(operation).unwrap(),
            case.operation_wire
        );
        assert_eq!(serde_json::to_value(reply).unwrap(), case.reply_wire);
    }
    let case = cases()
        .into_iter()
        .find(|case| operation_name(&case.operation) == "navigate_focus")
        .unwrap();
    assert!(serde_json::from_value::<PresentationResult>(case.operation_wire).is_err());
    assert!(serde_json::from_value::<PresentationOperation>(case.reply_wire).is_err());
}

fn effect_from_wire(operation: &Value) -> Effect {
    let mut effect = operation.clone();
    effect["kind"] = json!(format!("ui_{}", operation["kind"].as_str().unwrap()));
    effect.as_object_mut().unwrap().remove("timeout_ms");
    serde_json::from_value(effect).unwrap()
}

#[test]
fn all_presentation_effects_map_without_a_vm_and_keep_the_fixed_wait_policy() {
    let context = LoweringContext {
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: None,
        command_id: CommandId::new("command-a").unwrap(),
        idempotency_key: IdempotencyKey::new("effect-a").unwrap(),
        origin: CommandOrigin::Leselang,
        confirmation: Confirmation::NotRequired,
        dry_run: false,
    };
    for case in cases() {
        let mut expected = case.operation_wire;
        for (field, value) in [
            ("node_id", "node-a"),
            ("field", "field-a"),
            ("expected", "expected-a"),
            ("value", "value-a"),
        ] {
            if expected.get(field).is_some() {
                expected[field] = json!(value);
            }
        }
        let effect = effect_from_wire(&expected);
        let before = serde_json::to_vec(&effect).unwrap();
        if expected.get("timeout_ms").is_some() {
            expected["timeout_ms"] = json!(2000);
        }
        let mapped = PresentationOperation::from_effect(&effect).unwrap();
        assert_eq!(serde_json::to_value(&mapped).unwrap(), expected);
        assert_eq!(serde_json::to_vec(&effect).unwrap(), before);
        assert_eq!(PresentationOperation::from_effect(&effect), Some(mapped));
        // Moving the mapper does not admit GUI operations to the daemon command path.
        assert_eq!(
            lower_effect(&effect, &context),
            Err(LoweringError::FrontendLocalEffect)
        );
    }
}

#[test]
fn presentation_mapping_preserves_absent_optional_text_without_default_substitution() {
    for case in cases() {
        if !matches!(
            operation_name(&case.operation),
            "assert_action_unavailable_reason"
                | "wait_action_unavailable_reason"
                | "assert_form_field_placeholder"
                | "wait_form_field_placeholder"
        ) {
            continue;
        }
        for expected in [Value::Null, json!(""), json!("ready")] {
            let mut wire = case.operation_wire.clone();
            wire["expected"] = expected;
            let effect = effect_from_wire(&wire);
            if wire.get("timeout_ms").is_some() {
                wire["timeout_ms"] = json!(2000);
            }
            assert_eq!(
                serde_json::to_value(PresentationOperation::from_effect(&effect).unwrap()).unwrap(),
                wire
            );
        }
    }
}

#[test]
fn presentation_mapping_does_not_expand_computation_groups_or_control_plane_effects() {
    let sources = [
        "runtime.list()",
        r#"runtime.inspect(runtime_id: "runtime-a")"#,
        r#"runtime.history(runtime_id: "runtime-a")"#,
        r#"runtime.logs(runtime_id: "runtime-a")"#,
        r#"runtime.refresh(runtime_id: "runtime-a")"#,
        r#"runtime.refresh_capabilities(runtime_id: "runtime-a")"#,
        r#"runtime.deploy(runtime_id: "runtime-a", pipeline_kind: "http/request", target: none)"#,
        r#"debugger.cancel(session_id: "session-a")"#,
        r#"seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#,
        r#"all(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#,
        "add(left: 1, right: 2)",
    ];
    for source in sources {
        let effect = lower(&parse(&format!("fn main() = {source}")))
            .unwrap()
            .function
            .effect;
        assert_eq!(
            PresentationOperation::from_effect(&effect),
            None,
            "{source}"
        );
        for case in reply_cases() {
            assert!(!case.operation.matches_effect(&effect), "{source}");
        }
    }
}

#[test]
fn presentation_mapping_is_data_conversion_not_a_validation_certificate() {
    let effect = Effect::UiFocus {
        node_id: "bad node".into(),
    };
    assert_eq!(
        PresentationOperation::from_effect(&effect),
        Some(PresentationOperation::Focus {
            node_id: "bad node".into(),
        })
    );
    assert!(!leselang_hir::validate_ui_node_id("bad node"));
}

fn reply_cases() -> Vec<Case> {
    cases()
        .into_iter()
        .map(|mut case| {
            for wire in [&mut case.operation_wire, &mut case.reply_wire] {
                if wire.get("timeout_ms").is_some() {
                    wire["timeout_ms"] = json!(2000);
                }
                for (field, value) in [
                    ("field", "field-a"),
                    ("expected", "expected-a"),
                    ("value", "value-a"),
                ] {
                    if wire.get(field).is_some() {
                        wire[field] = json!(value);
                    }
                }
            }
            case.operation = serde_json::from_value(case.operation_wire.clone()).unwrap();
            case.reply = serde_json::from_value(case.reply_wire.clone()).unwrap();
            case
        })
        .collect()
}

fn envelope(operation: PresentationOperation) -> EffectOperation {
    EffectOperation::Presentation(PresentationEnvelope {
        schema_version: 1,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        operation,
    })
}

#[test]
fn all_presentation_replies_project_to_the_released_value_wire_and_type() {
    for case in reply_cases() {
        let mut expected = case.reply_wire;
        let kind = format!("ui_{}", expected["kind"].as_str().unwrap());
        expected["kind"] = json!(kind);
        expected.as_object_mut().unwrap().remove("timeout_ms");
        let value = leselang_command::Value::from(case.reply);
        assert_eq!(
            serde_json::to_value(value.result_type()).unwrap(),
            json!(kind)
        );
        assert_eq!(serde_json::to_value(&value).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<leselang_command::Value>(expected).unwrap(),
            value
        );
    }
}

#[test]
fn presentation_value_projection_preserves_optional_text_and_owned_buffers() {
    for case in reply_cases().into_iter().filter(|case| {
        matches!(
            operation_name(&case.operation),
            "assert_action_unavailable_reason"
                | "wait_action_unavailable_reason"
                | "assert_form_field_placeholder"
                | "wait_form_field_placeholder"
        )
    }) {
        for expected in [Value::Null, json!(""), json!("present")] {
            let mut wire = case.reply_wire.clone();
            wire["expected"] = expected.clone();
            let reply: PresentationResult = serde_json::from_value(wire).unwrap();
            let value = leselang_command::Value::from(reply);
            assert_eq!(serde_json::to_value(&value).unwrap()["expected"], expected);
        }
    }
    let (node_id, field, value) = (
        String::from("node-a"),
        String::from("field-a"),
        String::from("value-a"),
    );
    let pointers = (node_id.as_ptr(), field.as_ptr(), value.as_ptr());
    let projected = leselang_command::Value::from(PresentationResult::SetFormValue {
        node_id,
        field,
        value,
    });
    let leselang_command::Value::UiSetFormValue {
        node_id,
        field,
        value,
    } = projected
    else {
        panic!()
    };
    assert_eq!((node_id.as_ptr(), field.as_ptr(), value.as_ptr()), pointers);
}

#[test]
fn presentation_value_conversion_is_not_validation_or_admission() {
    let reply = PresentationResult::WaitHidden {
        node_id: "bad node".into(),
        timeout_ms: 1,
    };
    assert!(!reply.matches_effect(
        &Effect::UiWaitHidden {
            node_id: "bad node".into()
        },
        None
    ));
    let value = leselang_command::Value::from(reply);
    assert_eq!(value.result_type(), leselang_hir::Type::UiWaitHidden);
    assert_eq!(
        serde_json::to_value(value).unwrap(),
        json!({"kind":"ui_wait_hidden","node_id":"bad node"})
    );
}

#[test]
fn product_value_types_include_scalars_domain_results_and_ordered_structures() {
    use leselang_command::{DebuggerCancelResult, StructuredField, Value as ProductValue};
    use leselang_hir::{
        Type,
        computation::{OptionalStringValue, ScalarType, ScalarValue, StringListValue},
    };
    use leselang_host_contract::{Revision, RuntimeId};
    use leserpent_domain::{CommandResult, CommandStatus, RefreshStatus, RuntimeProjection};

    let runtime = RuntimeProjection {
        id: RuntimeId::new("runtime-a").unwrap(),
        name: "runtime-a".into(),
        endpoint: "http://127.0.0.1:8080".into(),
        sidecar_endpoint: None,
        registered_at_unix_ms: None,
        updated_at_unix_ms: None,
        revision: Revision(7),
        refresh_count: 0,
        refresh_status: RefreshStatus::NeverRequested,
        tags: Default::default(),
        status: Default::default(),
        sidecar_status: None,
        capabilities: Default::default(),
        capabilities_observed_for_revision: None,
    };
    let command = CommandResult {
        command_id: CommandId::new("command-a").unwrap(),
        status: CommandStatus::Applied,
        runtime: runtime.clone(),
        events: vec![],
    };
    for count in [0, 1, 2] {
        for value in [
            ProductValue::RuntimeList {
                revision: Revision(7),
                runtimes: vec![runtime.clone(); count],
            },
            ProductValue::RuntimeHistory {
                revision: Revision(7),
                entries: vec![command.clone(); count],
            },
        ] {
            assert_eq!(value.bounded_leaf_items(1), (count <= 1).then_some(count));
            assert_eq!(value.bounded_leaf_items(0), (count == 0).then_some(0));
            assert_eq!(value.bounded_leaf_items(2), Some(count));
        }
    }
    let cases = [
        (
            ProductValue::RuntimeList {
                revision: Revision(7),
                runtimes: vec![runtime.clone()],
            },
            Type::RuntimeList,
        ),
        (
            ProductValue::RuntimeInspect {
                revision: Revision(7),
                runtime: Box::new(runtime),
            },
            Type::RuntimeInspect,
        ),
        (
            ProductValue::RuntimeHistory {
                revision: Revision(7),
                entries: vec![command.clone()],
            },
            Type::RuntimeHistory,
        ),
        (
            ProductValue::RuntimeLogs {
                revision: Revision(7),
                runtime_id: RuntimeId::new("runtime-a").unwrap(),
                runtime_name: "runtime-a".into(),
                entries: vec![],
            },
            Type::RuntimeLogs,
        ),
        (
            ProductValue::RuntimeRefresh {
                result: Box::new(command.clone()),
            },
            Type::RuntimeRefresh,
        ),
        (
            ProductValue::RuntimeCapabilitiesRefresh {
                result: Box::new(command.clone()),
            },
            Type::RuntimeCapabilitiesRefresh,
        ),
        (
            ProductValue::RuntimeDeploy {
                result: Box::new(command),
            },
            Type::RuntimeDeploy,
        ),
        (
            ProductValue::DebuggerCancel {
                result: DebuggerCancelResult {
                    command_id: CommandId::new("command-a").unwrap(),
                    session_id: "session-a".into(),
                    observed_at_ms: 9,
                },
            },
            Type::DebuggerCancel,
        ),
        (
            ProductValue::Structured {
                fields: vec![
                    StructuredField {
                        name: "z".into(),
                        value: Box::new(ProductValue::UiFocus {
                            node_id: "a".into(),
                        }),
                    },
                    StructuredField {
                        name: "a".into(),
                        value: Box::new(ProductValue::Scalar {
                            value: ScalarValue::Integer(7),
                        }),
                    },
                ],
            },
            Type::Structured,
        ),
    ];
    for (value, expected) in cases {
        assert_eq!(value.result_type(), expected);
        let count = match expected {
            Type::Structured => None,
            Type::RuntimeLogs => Some(0),
            _ => Some(1),
        };
        assert_eq!(value.bounded_leaf_items(1), count);
        let wire = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            serde_json::from_slice::<ProductValue>(&wire).unwrap(),
            value
        );
    }
    for (value, expected) in [
        (ScalarValue::Integer(u64::MAX), ScalarType::Integer),
        (ScalarValue::Boolean(false), ScalarType::Boolean),
        (ScalarValue::String("text".into()), ScalarType::String),
        (ScalarValue::None, ScalarType::None),
        (
            ScalarValue::OptionalString(OptionalStringValue(None)),
            ScalarType::OptionalString,
        ),
        (
            ScalarValue::StringList(StringListValue(vec!["z".into(), "a".into()])),
            ScalarType::StringList,
        ),
    ] {
        assert_eq!(
            ProductValue::Scalar { value }.result_type(),
            Type::Scalar(expected)
        );
    }
    let wire = r#"{"kind":"structured","fields":[{"name":"z","value":{"kind":"ui_focus","node_id":"a"}},{"name":"a","value":{"kind":"scalar","value":{"kind":"integer","value":7}}}]}"#;
    let value: ProductValue = serde_json::from_str(wire).unwrap();
    assert_eq!(serde_json::to_string(&value).unwrap(), wire);
}

#[test]
fn all_presentation_payload_and_leaf_bounds_preserve_identifiers_text_and_counts() {
    use leselang_hir::{
        MAX_UI_CHILD_COUNT, MAX_UI_EXPECTED_TEXT_BYTES, MAX_UI_FORM_FIELD_KEY_BYTES,
        MAX_UI_FORM_FIELD_MAX_LENGTH, MAX_UI_NODE_ID_BYTES,
    };

    let cases = reply_cases();
    assert_eq!(cases.len(), 62);
    for case in cases {
        let value = leselang_command::Value::from(case.reply);
        let wire = serde_json::to_value(&value).unwrap();
        let check = |field: &str, candidate: Value, expected| {
            if case.operation_wire.get(field).is_some() {
                let mut operation_wire = case.operation_wire.clone();
                operation_wire[field] = candidate.clone();
                let operation: PresentationOperation =
                    serde_json::from_value(operation_wire.clone()).unwrap();
                let effect = effect_from_wire(&operation_wire);
                assert_eq!(
                    operation.matches_effect(&effect),
                    expected == Some(1),
                    "{operation_wire}"
                );
                assert_eq!(serde_json::to_value(&operation).unwrap(), operation_wire);
            }
            let mut changed = wire.clone();
            changed[field] = candidate;
            let value: leselang_command::Value = serde_json::from_value(changed.clone()).unwrap();
            // Query ceiling zero is not the separate per-execution output budget.
            assert_eq!(value.bounded_leaf_items(0), expected, "{changed}");
            assert_eq!(serde_json::to_value(&value).unwrap(), changed);
        };
        assert_eq!(value.bounded_leaf_items(0), Some(1), "{wire}");
        for (field, max) in [
            ("node_id", MAX_UI_NODE_ID_BYTES),
            ("focused_node_id", MAX_UI_NODE_ID_BYTES),
            ("field", MAX_UI_FORM_FIELD_KEY_BYTES),
        ] {
            if wire.get(field).is_none() {
                continue;
            }
            check(field, json!("a".repeat(max)), Some(1));
            for invalid in [
                "".into(),
                "bad id".into(),
                "a\n".into(),
                "a".repeat(max + 1),
            ] {
                check(field, json!(invalid), None);
            }
            check(field, json!("a.b_c-d"), Some(1));
            check(field, json!("a:b"), (field != "field").then_some(1));
        }
        for field in ["expected", "value"] {
            if wire.get(field).is_none() {
                continue;
            }
            let name = operation_name(&case.operation);
            let automation_id = name.ends_with("automation_id");
            let max = if automation_id {
                MAX_UI_NODE_ID_BYTES
            } else if name.ends_with("form_value") {
                MAX_UI_FORM_FIELD_MAX_LENGTH
            } else {
                MAX_UI_EXPECTED_TEXT_BYTES
            };
            check(field, json!("a".repeat(max)), Some(1));
            check(field, json!("a".repeat(max + 1)), None);
            check(field, json!(""), (!automation_id).then_some(1));
            for invalid in ["\n", "\0", "\u{85}"] {
                check(field, json!(invalid), None);
            }
            if !automation_id {
                check(field, json!("\u{e9}".repeat(max / 2)), Some(1));
                check(field, json!("\u{e9}".repeat(max / 2 + 1)), None);
            }
            if name.ends_with("unavailable_reason") || name.ends_with("placeholder") {
                check(field, Value::Null, Some(1));
            }
        }
        if wire.get("count").is_some() {
            check("count", json!(MAX_UI_CHILD_COUNT), Some(1));
            check("count", json!(MAX_UI_CHILD_COUNT + 1), None);
        }
        if wire.get("max_length").is_some() {
            check("max_length", json!(usize::MAX), Some(1));
        }
        if wire.get("focused_node_id").is_some() {
            check("focused_node_id", wire["node_id"].clone(), Some(1));
        }
    }
}

#[test]
fn product_scalar_leaf_bounds_preserve_empty_lists_and_byte_budgets() {
    use leselang_command::Value as ProductValue;
    use leselang_hir::computation::{
        MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS, OptionalStringValue, ScalarValue,
        StringListValue,
    };

    let check = |value, expected| {
        let value = ProductValue::Scalar { value };
        let before = serde_json::to_vec(&value).unwrap();
        assert_eq!(value.bounded_leaf_items(0), expected);
        assert_eq!(serde_json::to_vec(&value).unwrap(), before);
    };
    for value in [
        ScalarValue::None,
        ScalarValue::Integer(u64::MAX),
        ScalarValue::Boolean(false),
    ] {
        check(value, Some(1));
    }
    check(
        ScalarValue::OptionalString(OptionalStringValue(None)),
        Some(1),
    );
    for bytes in [0, MAX_SCALAR_STRING_BYTES, MAX_SCALAR_STRING_BYTES + 1] {
        let expected = (bytes <= MAX_SCALAR_STRING_BYTES).then_some(1);
        check(ScalarValue::String("a".repeat(bytes)), expected);
        check(
            ScalarValue::OptionalString(OptionalStringValue(Some("a".repeat(bytes)))),
            expected,
        );
    }
    for count in [0, 1, MAX_STRING_LIST_ITEMS, MAX_STRING_LIST_ITEMS + 1] {
        check(
            ScalarValue::StringList(StringListValue(vec![String::new(); count])),
            (count <= MAX_STRING_LIST_ITEMS).then_some(count.max(1)),
        );
    }
    let part = "\u{e9}".repeat(MAX_SCALAR_STRING_BYTES / 4);
    check(
        ScalarValue::StringList(StringListValue(vec![part.clone(), part.clone()])),
        Some(2),
    );
    check(
        ScalarValue::StringList(StringListValue(vec![part.clone(), format!("{part}a")])),
        None,
    );
    // Scalar text is not presentation text: control characters retain their old semantics.
    check(ScalarValue::String("\n\0".into()), Some(1));
    assert_eq!(
        ProductValue::Structured { fields: vec![] }.bounded_leaf_items(1),
        None
    );
}

#[test]
fn product_log_and_debugger_leaf_bounds_keep_their_distinct_limits() {
    use leselang_command::{DebuggerCancelResult, Value as ProductValue};
    use leselang_host_contract::{Revision, RuntimeId};
    use leserpent_domain::{MAX_RUNTIME_LOG_QUERY_ENTRIES, RuntimeLogLevel, RuntimeLogRecord};

    let max = usize::from(MAX_RUNTIME_LOG_QUERY_ENTRIES);
    for count in [0, 1, max, max + 1] {
        let value = ProductValue::RuntimeLogs {
            revision: Revision(7),
            runtime_id: RuntimeId::new("runtime-a").unwrap(),
            runtime_name: String::new(),
            entries: vec![
                RuntimeLogRecord {
                    sequence: 0,
                    level: RuntimeLogLevel::Info,
                    message: String::new()
                };
                count
            ],
        };
        for query_ceiling in [0, 1, usize::MAX] {
            assert_eq!(
                value.bounded_leaf_items(query_ceiling),
                (count <= max).then_some(count)
            );
        }
    }
    for (session_id, observed_at_ms, expected) in [
        ("session-a", 0, Some(1)),
        ("session-a", i64::MAX as u64, Some(1)),
        ("session-a", i64::MAX as u64 + 1, None),
        ("", 0, None),
        ("bad session", 0, None),
    ] {
        let value = ProductValue::DebuggerCancel {
            result: DebuggerCancelResult {
                command_id: CommandId::new("command-a").unwrap(),
                session_id: session_id.into(),
                observed_at_ms,
            },
        };
        assert_eq!(value.bounded_leaf_items(0), expected);
    }
}

#[test]
fn control_plane_requests_match_only_their_original_effect_and_retain_bytes() {
    let cases = control_plane_cases();
    assert_eq!(cases.len(), 8);
    for (index, (effect, operation)) in cases.iter().enumerate() {
        let before = serde_json::to_vec(&(effect, operation)).unwrap();
        for (other_index, (_, other)) in cases.iter().enumerate() {
            assert_eq!(control_plane_matches(other, effect), index == other_index);
        }
        assert_eq!(serde_json::to_vec(&(effect, operation)).unwrap(), before);
        for gui in reply_cases() {
            assert!(!control_plane_matches(
                operation,
                &effect_from_wire(&gui.operation_wire)
            ));
        }
        for source in [
            r#"seq(first: runtime.list(), second: runtime.list())"#,
            r#"all(first: runtime.list(), second: runtime.list())"#,
            "add(left: 1, right: 2)",
        ] {
            let effect = lower(&parse(&format!("fn main() = {source}")))
                .unwrap()
                .function
                .effect;
            assert!(!control_plane_matches(operation, &effect));
        }
    }
}

fn control_plane_reply_cases() -> Vec<(Effect, EffectOperation, leselang_command::EffectResult)> {
    use leselang_command::{DebuggerCancelResult, EffectResult, PlannedOperation};
    use leselang_host_contract::{Revision, RuntimeId};
    use leserpent_domain::{
        CommandResult, CommandStatus, QueryResult, RefreshStatus, RuntimeProjection,
    };
    let runtime = RuntimeProjection {
        id: RuntimeId::new("runtime-a").unwrap(),
        name: "runtime-a".into(),
        endpoint: "http://127.0.0.1:8080".into(),
        sidecar_endpoint: None,
        registered_at_unix_ms: None,
        updated_at_unix_ms: None,
        revision: Revision(7),
        refresh_count: 0,
        refresh_status: RefreshStatus::NeverRequested,
        tags: Default::default(),
        status: Default::default(),
        sidecar_status: None,
        capabilities: Default::default(),
        capabilities_observed_for_revision: None,
    };
    let command = CommandResult {
        command_id: CommandId::new("command-a").unwrap(),
        status: CommandStatus::Applied,
        runtime: runtime.clone(),
        events: vec![],
    };
    control_plane_cases()
        .into_iter()
        .enumerate()
        .map(|(index, (effect, operation))| {
            let operation = match operation {
                PlannedOperation::Query(query) => EffectOperation::Query(query),
                PlannedOperation::Command(command) => EffectOperation::Command(command),
            };
            let reply = match index {
                0 => EffectResult::Query(QueryResult::RuntimeList {
                    revision: Revision(7),
                    runtimes: vec![runtime.clone()],
                }),
                1 => EffectResult::Query(QueryResult::RuntimeInspect {
                    revision: Revision(7),
                    runtime: runtime.clone(),
                }),
                2 => EffectResult::Query(QueryResult::RuntimeHistory {
                    revision: Revision(7),
                    entries: vec![command.clone()],
                }),
                3 => EffectResult::Query(QueryResult::RuntimeLogs {
                    revision: Revision(7),
                    runtime_id: runtime.id.clone(),
                    runtime_name: "runtime-a".into(),
                    entries: vec![],
                }),
                4..=6 => EffectResult::Command(Box::new(command.clone())),
                7 => EffectResult::DebuggerCancel(DebuggerCancelResult {
                    command_id: command.command_id.clone(),
                    session_id: "session-a".into(),
                    observed_at_ms: 11,
                }),
                _ => unreachable!(),
            };
            (effect, operation, reply)
        })
        .collect()
}

#[test]
fn control_plane_replies_match_original_ids_without_copying_or_consuming_payloads() {
    let cases = control_plane_reply_cases();
    for (index, (effect, operation, reply)) in cases.iter().enumerate() {
        let before = serde_json::to_vec(reply).unwrap();
        for (other_index, (_, _, other)) in cases.iter().enumerate() {
            // Command replies have no operation tag; this fixture shares the same saved ID.
            let expected = index == other_index
                || ((4..=6).contains(&index) && (4..=6).contains(&other_index));
            assert_eq!(
                other.matches_control_plane_effect(effect, Some(operation)),
                expected
            );
        }
        assert_eq!(reply.matches_control_plane_effect(effect, None), index < 4);
        let wrong_category = EffectOperation::Presentation(PresentationEnvelope {
            schema_version: 1,
            principal: Principal::new("operator").unwrap(),
            capabilities: CapabilitySet::default(),
            operation: PresentationOperation::Focus {
                node_id: "node-a".into(),
            },
        });
        assert_eq!(
            reply.matches_control_plane_effect(effect, Some(&wrong_category)),
            index < 4
        );
        assert_eq!(serde_json::to_vec(reply).unwrap(), before);
        let decoded: leselang_command::EffectResult = serde_json::from_slice(&before).unwrap();
        assert!(decoded.matches_control_plane_effect(effect, Some(operation)));
    }
}

#[test]
fn control_plane_replies_reject_torn_ids_statuses_and_out_of_range_cancel_time() {
    use leselang_command::EffectResult;
    use leselang_host_contract::RuntimeId;
    use leserpent_domain::{CommandStatus, QueryResult};
    for (effect, operation, reply) in control_plane_reply_cases() {
        let mut torn = reply.clone();
        match &mut torn {
            EffectResult::Query(QueryResult::RuntimeList { .. }) => continue,
            EffectResult::Query(QueryResult::RuntimeInspect { runtime, .. }) => {
                runtime.id = RuntimeId::new("other").unwrap()
            }
            EffectResult::Query(QueryResult::RuntimeHistory { entries, .. }) => {
                entries.push(entries[0].clone());
                entries[1].runtime.id = RuntimeId::new("other").unwrap();
            }
            EffectResult::Query(QueryResult::RuntimeLogs { runtime_id, .. }) => {
                *runtime_id = RuntimeId::new("other").unwrap()
            }
            EffectResult::Command(result) => result.runtime.id = RuntimeId::new("other").unwrap(),
            EffectResult::DebuggerCancel(result) => result.session_id = "other".into(),
            EffectResult::Presentation(_) => unreachable!(),
        }
        assert!(!torn.matches_control_plane_effect(&effect, Some(&operation)));
        if let EffectResult::Command(result) = &reply {
            let mut changed = result.clone();
            changed.status = CommandStatus::Planned;
            assert!(
                !EffectResult::Command(changed)
                    .matches_control_plane_effect(&effect, Some(&operation))
            );
            let mut changed = result.clone();
            changed.command_id = CommandId::new("other").unwrap();
            assert!(
                !EffectResult::Command(changed)
                    .matches_control_plane_effect(&effect, Some(&operation))
            );
        }
        if let EffectResult::DebuggerCancel(result) = &reply {
            for (timestamp, expected) in [
                (0, true),
                (i64::MAX as u64, true),
                (i64::MAX as u64 + 1, false),
                (u64::MAX, false),
            ] {
                let mut changed = result.clone();
                changed.observed_at_ms = timestamp;
                assert_eq!(
                    EffectResult::DebuggerCancel(changed)
                        .matches_control_plane_effect(&effect, Some(&operation)),
                    expected
                );
            }
            let mut changed = result.clone();
            changed.command_id = CommandId::new("other").unwrap();
            assert!(
                !EffectResult::DebuggerCancel(changed)
                    .matches_control_plane_effect(&effect, Some(&operation))
            );
        }
    }
}

#[test]
fn control_plane_reply_correspondence_is_not_authority_revision_bounds_or_domain_validation() {
    use leselang_command::EffectResult;
    use leselang_host_contract::Revision;
    use leserpent_domain::QueryResult;
    for (effect, mut operation, mut reply) in control_plane_reply_cases() {
        match &mut operation {
            EffectOperation::Command(command) => {
                command.schema_version = 99;
                command.principal.id.clear();
                command.capabilities = CapabilitySet::default();
                command.dry_run = true;
                command.confirmation = Confirmation::NotRequired;
                command.expected_revision = Some(Revision(999));
                assert!(reply.matches_control_plane_effect(&effect, Some(&operation)));
            }
            EffectOperation::Query(_) => {
                match &mut reply {
                    EffectResult::Query(QueryResult::RuntimeList { revision, runtimes }) => {
                        *revision = Revision(999);
                        runtimes[0].name.clear();
                        runtimes.resize(10_001, runtimes[0].clone());
                    }
                    EffectResult::Query(QueryResult::RuntimeInspect { revision, runtime }) => {
                        *revision = Revision(999);
                        runtime.name.clear();
                    }
                    EffectResult::Query(QueryResult::RuntimeHistory { revision, entries }) => {
                        *revision = Revision(999);
                        entries.clear();
                    }
                    EffectResult::Query(QueryResult::RuntimeLogs {
                        revision,
                        runtime_name,
                        entries,
                        ..
                    }) => {
                        *revision = Revision(999);
                        runtime_name.clear();
                        entries.resize(
                            257,
                            leserpent_domain::RuntimeLogRecord {
                                sequence: 0,
                                level: leserpent_domain::RuntimeLogLevel::Info,
                                message: String::new(),
                            },
                        );
                    }
                    _ => unreachable!(),
                }
                assert!(reply.matches_control_plane_effect(&effect, None));
            }
            _ => unreachable!(),
        }
    }
}

#[test]
fn control_plane_reply_matcher_rejects_all_gui_and_structured_operations() {
    for (effect, operation, reply) in control_plane_reply_cases() {
        for gui in reply_cases() {
            let gui_effect = effect_from_wire(&gui.operation_wire);
            assert!(!reply.matches_control_plane_effect(&gui_effect, Some(&operation)));
            assert!(
                !leselang_command::EffectResult::Presentation(gui.reply)
                    .matches_control_plane_effect(&effect, Some(&operation))
            );
        }
        for non_atomic in [
            Effect::All { branches: vec![] },
            Effect::Sequence { steps: vec![] },
            lower(&parse("fn main() = 1")).unwrap().function.effect,
        ] {
            assert!(!reply.matches_control_plane_effect(&non_atomic, Some(&operation)));
        }
    }
}

fn control_plane_value_types() -> [(leselang_hir::Type, &'static str); 8] {
    use leselang_hir::Type;
    [
        (Type::RuntimeList, "runtime_list"),
        (Type::RuntimeInspect, "runtime_inspect"),
        (Type::RuntimeHistory, "runtime_history"),
        (Type::RuntimeLogs, "runtime_logs"),
        (Type::RuntimeRefresh, "runtime_refresh"),
        (
            Type::RuntimeCapabilitiesRefresh,
            "runtime_capabilities_refresh",
        ),
        (Type::RuntimeDeploy, "runtime_deploy"),
        (Type::DebuggerCancel, "debugger_cancel"),
    ]
}

#[test]
fn scalar_projection_covers_all_seventy_seven_types_and_twelve_explicit_fields() {
    use leselang_command::Value as ProductValue;
    use leselang_hir::computation::{OptionalStringValue, ScalarValue, StringListValue};
    use leselang_hir::result_field::ResultField;

    let mut values: Vec<ProductValue> = reply_cases()
        .into_iter()
        .map(|case| ProductValue::from(case.reply))
        .collect();
    for ((_, _, reply), (ty, _)) in control_plane_reply_cases()
        .into_iter()
        .zip(control_plane_value_types())
    {
        values.push(reply.into_control_plane_value(ty).unwrap());
    }
    values.extend(
        [
            ScalarValue::Integer(u64::MAX),
            ScalarValue::Boolean(false),
            ScalarValue::String("scalar".into()),
            ScalarValue::None,
            ScalarValue::OptionalString(OptionalStringValue(None)),
            ScalarValue::StringList(StringListValue(vec!["item".into()])),
        ]
        .into_iter()
        .map(|value| ProductValue::Scalar { value }),
    );
    values.push(ProductValue::Structured { fields: vec![] });
    assert_eq!(values.len(), 77);
    assert_eq!(ResultField::ALL.len(), 12);

    for value in values {
        let before = serde_json::to_value(&value).unwrap();
        for field in ResultField::ALL {
            // The HIR type catalog and released wire fields are independent of
            // the adapter's native enum match, including renamed kind/state fields.
            let expected = field.result_type(value.result_type()).map(|ty| {
                let scalar = match field {
                    ResultField::Revision => {
                        ScalarValue::Integer(before["revision"].as_u64().unwrap())
                    }
                    ResultField::Count => ScalarValue::Integer(
                        before
                            .get("runtimes")
                            .or_else(|| before.get("entries"))
                            .map_or_else(
                                || before["count"].as_u64().unwrap(),
                                |items| items.as_array().unwrap().len() as u64,
                            ),
                    ),
                    ResultField::MaxLength => {
                        ScalarValue::Integer(before["max_length"].as_u64().unwrap())
                    }
                    ResultField::Selected => ScalarValue::Boolean(before["state"] == "selected"),
                    ResultField::Required => ScalarValue::Boolean(before["state"] == "required"),
                    ResultField::Kind => ScalarValue::String(
                        before
                            .get("expected_kind")
                            .or_else(|| before.get("input_kind"))
                            .unwrap()
                            .as_str()
                            .unwrap()
                            .into(),
                    ),
                    ResultField::OptionalExpected => ScalarValue::OptionalString(
                        OptionalStringValue(before["expected"].as_str().map(str::to_owned)),
                    ),
                    ResultField::NodeId
                    | ResultField::FocusedNodeId
                    | ResultField::Expected
                    | ResultField::Field
                    | ResultField::Value => {
                        ScalarValue::String(before[field.name()].as_str().unwrap().into())
                    }
                };
                assert_eq!(scalar.scalar_type(), ty);
                scalar
            });
            assert_eq!(
                value.project_scalar(field),
                expected,
                "{:?}/{field:?}",
                value.result_type()
            );
        }
        assert_eq!(serde_json::to_value(&value).unwrap(), before);
    }
}

#[test]
fn scalar_projection_keeps_optional_states_and_returns_independent_owned_strings() {
    use leselang_command::Value as ProductValue;
    use leselang_hir::computation::{OptionalStringValue, ScalarValue};
    use leselang_hir::result_field::ResultField;

    for expected in [None, Some(String::new()), Some("ready".into())] {
        for value in [
            ProductValue::UiAssertFormFieldPlaceholder {
                node_id: "a".into(),
                field: "name".into(),
                expected: expected.clone(),
            },
            ProductValue::UiWaitFormFieldPlaceholder {
                node_id: "a".into(),
                field: "name".into(),
                expected: expected.clone(),
            },
            ProductValue::UiAssertActionUnavailableReason {
                node_id: "a".into(),
                expected: expected.clone(),
            },
            ProductValue::UiWaitActionUnavailableReason {
                node_id: "a".into(),
                expected: expected.clone(),
            },
        ] {
            assert_eq!(
                value.project_scalar(ResultField::OptionalExpected),
                Some(ScalarValue::OptionalString(OptionalStringValue(
                    expected.clone()
                )))
            );
            assert_eq!(value.project_scalar(ResultField::Expected), None);
        }
    }
    let value = ProductValue::UiSetFormValue {
        node_id: "node-a".into(),
        field: "name".into(),
        value: "ready".into(),
    };
    for (field, text) in [
        (ResultField::NodeId, "node-a"),
        (ResultField::Field, "name"),
        (ResultField::Value, "ready"),
    ] {
        let Some(ScalarValue::String(mut owned)) = value.project_scalar(field) else {
            panic!("missing string");
        };
        let Some(ScalarValue::String(second)) = value.project_scalar(field) else {
            panic!("missing string");
        };
        assert_ne!(owned.as_ptr(), second.as_ptr());
        owned.clear();
        assert_eq!(second, text);
        assert_eq!(
            value.project_scalar(field),
            Some(ScalarValue::String(text.into()))
        );
    }
}

#[test]
fn scalar_projection_is_not_bounds_validation_or_execution_admission() {
    use leselang_command::Value as ProductValue;
    use leselang_hir::computation::ScalarValue;
    use leselang_hir::result_field::ResultField;
    use leselang_host_contract::Revision;

    for count in [0, 1, usize::MAX] {
        let value = ProductValue::UiAssertChildCount {
            node_id: "".into(),
            count,
        };
        assert_eq!(
            value.project_scalar(ResultField::Count),
            Some(ScalarValue::Integer(count as u64))
        );
    }
    let oversized = "x".repeat(leselang_hir::computation::MAX_SCALAR_STRING_BYTES + 1);
    let value = ProductValue::UiAssertText {
        node_id: "".into(),
        expected: oversized.clone(),
    };
    assert_eq!(value.bounded_leaf_items(1), None);
    assert_eq!(
        value.project_scalar(ResultField::Expected),
        Some(ScalarValue::String(oversized))
    );
    let value = ProductValue::RuntimeList {
        revision: Revision(u64::MAX),
        runtimes: vec![],
    };
    assert_eq!(
        value.project_scalar(ResultField::Revision),
        Some(ScalarValue::Integer(u64::MAX))
    );
    assert_eq!(
        value.project_scalar(ResultField::Count),
        Some(ScalarValue::Integer(0))
    );
    for (state, expected) in [
        (UiSelectionState::Selected, true),
        (UiSelectionState::Unselected, false),
    ] {
        let value = ProductValue::UiSetSelection {
            node_id: "".into(),
            state,
        };
        assert_eq!(
            value.project_scalar(ResultField::Selected),
            Some(ScalarValue::Boolean(expected))
        );
    }
    for (state, expected) in [
        (UiFormRequirementState::Required, true),
        (UiFormRequirementState::Optional, false),
    ] {
        let value = ProductValue::UiAssertFormFieldRequired {
            node_id: "".into(),
            field: "".into(),
            state,
        };
        assert_eq!(
            value.project_scalar(ResultField::Required),
            Some(ScalarValue::Boolean(expected))
        );
    }
}

fn expected_control_plane_value_wire(reply: &leselang_command::EffectResult, tag: &str) -> Value {
    let payload = serde_json::to_value(reply).unwrap()["payload"].clone();
    if matches!(reply, leselang_command::EffectResult::Query(_)) {
        let mut expected = payload;
        expected["kind"] = json!(tag);
        expected
    } else {
        json!({"kind": tag, "result": payload})
    }
}

#[test]
fn control_plane_value_projection_keeps_closed_type_matrix_and_released_wire() {
    use leselang_command::{EffectResult, Value as ProductValue};
    use leselang_hir::{Type, computation::ScalarType};
    let accepted = control_plane_value_types();
    let mut rejected = vec![
        Type::Structured,
        Type::Scalar(ScalarType::Integer),
        Type::Scalar(ScalarType::Boolean),
        Type::Scalar(ScalarType::String),
        Type::Scalar(ScalarType::None),
        Type::Scalar(ScalarType::OptionalString),
        Type::Scalar(ScalarType::StringList),
    ];
    for gui in reply_cases() {
        let ty = ProductValue::from(gui.reply.clone()).result_type();
        rejected.push(ty);
        for (target, _) in accepted {
            assert!(
                EffectResult::Presentation(gui.reply.clone())
                    .into_control_plane_value(target)
                    .is_none()
            );
        }
        assert!(
            EffectResult::Presentation(gui.reply)
                .into_control_plane_value(ty)
                .is_none()
        );
    }
    assert_eq!(accepted.len() + rejected.len(), 77);
    for (index, (_, _, reply)) in control_plane_reply_cases().into_iter().enumerate() {
        for (target_index, (ty, tag)) in accepted.iter().copied().enumerate() {
            let value = reply.clone().into_control_plane_value(ty);
            let matches = index == target_index
                || ((4..=6).contains(&index) && (4..=6).contains(&target_index));
            if matches {
                let value = value.unwrap();
                let expected = expected_control_plane_value_wire(&reply, tag);
                assert_eq!(serde_json::to_value(&value).unwrap(), expected);
                assert_eq!(value.result_type(), ty);
                assert_eq!(
                    serde_json::from_value::<ProductValue>(expected).unwrap(),
                    value
                );
            } else {
                assert!(value.is_none(), "wrong projection {index} as {ty:?}");
            }
        }
        for &ty in &rejected {
            assert!(reply.clone().into_control_plane_value(ty).is_none());
        }
    }
}

#[test]
fn control_plane_value_projection_moves_buffers_and_keeps_inspect_boxing() {
    use leselang_command::{EffectResult, Value as ProductValue};
    use leserpent_domain::{CommandResult, QueryResult};
    for (index, (_, _, mut reply)) in control_plane_reply_cases().into_iter().enumerate() {
        if let EffectResult::Query(QueryResult::RuntimeLogs { entries, .. }) = &mut reply {
            entries.push(leserpent_domain::RuntimeLogRecord {
                sequence: 1,
                level: leserpent_domain::RuntimeLogLevel::Info,
                message: "ready".into(),
            });
        }
        let before: Vec<usize> = match &reply {
            EffectResult::Query(QueryResult::RuntimeList { runtimes, .. }) => {
                vec![runtimes.as_ptr() as usize]
            }
            EffectResult::Query(QueryResult::RuntimeInspect { runtime, .. }) => vec![
                runtime.name.as_ptr() as usize,
                runtime.endpoint.as_ptr() as usize,
            ],
            EffectResult::Query(QueryResult::RuntimeHistory { entries, .. }) => {
                vec![entries.as_ptr() as usize]
            }
            EffectResult::Query(QueryResult::RuntimeLogs {
                runtime_name,
                entries,
                ..
            }) => vec![runtime_name.as_ptr() as usize, entries.as_ptr() as usize],
            EffectResult::Command(result) => vec![&**result as *const CommandResult as usize],
            EffectResult::DebuggerCancel(result) => vec![
                result.session_id.as_ptr() as usize,
                result.command_id.as_str().as_ptr() as usize,
            ],
            _ => unreachable!(),
        };
        let value = reply
            .into_control_plane_value(control_plane_value_types()[index].0)
            .unwrap();
        let after: Vec<usize> = match &value {
            ProductValue::RuntimeList { runtimes, .. } => vec![runtimes.as_ptr() as usize],
            ProductValue::RuntimeInspect { runtime, .. } => vec![
                runtime.name.as_ptr() as usize,
                runtime.endpoint.as_ptr() as usize,
            ],
            ProductValue::RuntimeHistory { entries, .. } => vec![entries.as_ptr() as usize],
            ProductValue::RuntimeLogs {
                runtime_name,
                entries,
                ..
            } => vec![runtime_name.as_ptr() as usize, entries.as_ptr() as usize],
            ProductValue::RuntimeRefresh { result }
            | ProductValue::RuntimeCapabilitiesRefresh { result }
            | ProductValue::RuntimeDeploy { result } => {
                vec![&**result as *const CommandResult as usize]
            }
            ProductValue::DebuggerCancel { result } => vec![
                result.session_id.as_ptr() as usize,
                result.command_id.as_str().as_ptr() as usize,
            ],
            _ => unreachable!(),
        };
        assert_eq!(after, before);
    }
}

#[test]
fn control_plane_value_projection_does_not_certify_correspondence_status_time_or_bounds() {
    use leselang_command::EffectResult;
    use leselang_host_contract::{Revision, RuntimeId};
    use leserpent_domain::{CommandStatus, QueryResult};
    for (index, (_, _, mut reply)) in control_plane_reply_cases().into_iter().enumerate() {
        match &mut reply {
            EffectResult::Query(QueryResult::RuntimeList { revision, runtimes }) => {
                *revision = Revision(999);
                runtimes[0].name.clear();
                runtimes.resize(10_001, runtimes[0].clone());
            }
            EffectResult::Query(QueryResult::RuntimeInspect { revision, runtime }) => {
                *revision = Revision(999);
                runtime.id = RuntimeId::new("other").unwrap();
            }
            EffectResult::Query(QueryResult::RuntimeHistory { entries, .. }) => {
                entries[0].runtime.id = RuntimeId::new("other").unwrap();
            }
            EffectResult::Query(QueryResult::RuntimeLogs {
                runtime_id,
                runtime_name,
                ..
            }) => {
                *runtime_id = RuntimeId::new("other").unwrap();
                runtime_name.clear();
            }
            EffectResult::Command(result) => {
                result.status = CommandStatus::Planned;
                result.command_id = CommandId::new("other").unwrap();
            }
            EffectResult::DebuggerCancel(result) => {
                result.observed_at_ms = u64::MAX;
                result.session_id.clear();
            }
            _ => unreachable!(),
        }
        let (ty, tag) = control_plane_value_types()[index];
        let expected = expected_control_plane_value_wire(&reply, tag);
        let value = reply.into_control_plane_value(ty).unwrap();
        assert_eq!(serde_json::to_value(value).unwrap(), expected);
    }
}

fn control_plane_cases() -> Vec<(Effect, leselang_command::PlannedOperation)> {
    [
        "runtime.list()",
        r#"runtime.inspect(runtime_id: "runtime-a")"#,
        r#"runtime.history(runtime_id: "runtime-a")"#,
        r#"runtime.logs(runtime_id: "runtime-a")"#,
        r#"runtime.refresh(runtime_id: "runtime-a")"#,
        r#"runtime.refresh_capabilities(runtime_id: "runtime-a")"#,
        r#"runtime.deploy(runtime_id: "runtime-a", pipeline_kind: "http/request", target: none)"#,
        r#"debugger.cancel(session_id: "session-a")"#,
    ]
    .into_iter()
    .map(|source| {
        let effect = lower(&parse(&format!("fn main() = {source}")))
            .unwrap()
            .function
            .effect;
        let context = LoweringContext {
            principal: Principal::new("operator").unwrap(),
            capabilities: CapabilitySet::new([
                "runtime.read",
                "runtime.refresh",
                "runtime.deploy",
                "debugger.control",
            ]),
            expected_revision: Some(leselang_host_contract::Revision(7)),
            command_id: CommandId::new("command-a").unwrap(),
            idempotency_key: IdempotencyKey::new("effect-a").unwrap(),
            origin: CommandOrigin::Leselang,
            confirmation: if matches!(
                effect,
                Effect::RuntimeDeploy { .. } | Effect::DebuggerCancel { .. }
            ) {
                Confirmation::Confirmed
            } else {
                Confirmation::NotRequired
            },
            dry_run: false,
        };
        let operation = lower_effect(&effect, &context).unwrap().operation;
        (effect, operation)
    })
    .collect()
}

fn control_plane_matches(operation: &leselang_command::PlannedOperation, effect: &Effect) -> bool {
    match operation {
        leselang_command::PlannedOperation::Query(query) => {
            leselang_command::query_matches_effect(&query.query, effect)
        }
        leselang_command::PlannedOperation::Command(command) => {
            leselang_command::command_matches_effect(
                command,
                effect,
                "command-a",
                "effect-a",
                Some(leselang_host_contract::Revision(7)),
            )
        }
    }
}

#[test]
fn control_plane_requests_reject_torn_payloads_and_command_metadata() {
    use leselang_command::PlannedOperation;
    for (effect, operation) in control_plane_cases() {
        let wire = serde_json::to_value(&operation).unwrap();
        let payload = if matches!(operation, PlannedOperation::Query(_)) {
            "query"
        } else {
            "command"
        };
        for (field, _) in wire["payload"][payload].as_object().unwrap() {
            if field == "kind" {
                continue;
            }
            let mutations = match field.as_str() {
                "filter" => ["environment", "cluster", "role"]
                    .into_iter()
                    .map(|field| {
                        let mut filter = wire["payload"][payload]["filter"].clone();
                        filter[field] = json!("other");
                        filter
                    })
                    .collect(),
                "after_sequence" => vec![json!(0), json!(u64::MAX)],
                "limit" => vec![json!(0), json!(255), json!(257), json!(u16::MAX)],
                "runtime_id" | "pipeline_kind" | "target" | "session_id" => vec![json!("other")],
                _ => panic!("add a payload mutation for {field}"),
            };
            for value in mutations {
                let mut changed = wire.clone();
                changed["payload"][payload][field] = value;
                let changed: PlannedOperation = serde_json::from_value(changed).unwrap();
                assert!(
                    !control_plane_matches(&changed, &effect),
                    "{payload}.{field}"
                );
            }
        }
        let PlannedOperation::Command(command) = &operation else {
            continue;
        };
        let confirmation = if command.confirmation == Confirmation::Confirmed {
            "not_required"
        } else {
            "confirmed"
        };
        for (field, value) in [
            ("command_id", json!("other-command")),
            ("idempotency_key", json!("other-key")),
            ("expected_revision", Value::Null),
            ("expected_revision", json!(8)),
            ("origin", json!("gui")),
            ("origin", json!("cli")),
            ("origin", json!("model")),
            ("origin", json!("compatibility_adapter")),
            ("confirmation", json!(confirmation)),
            ("dry_run", json!(true)),
        ] {
            let mut changed = wire.clone();
            changed["payload"][field] = value;
            let changed: PlannedOperation = serde_json::from_value(changed).unwrap();
            assert!(!control_plane_matches(&changed, &effect), "{field}");
        }
        assert!(!leselang_command::command_matches_effect(
            command,
            &effect,
            "other",
            "effect-a",
            Some(leselang_host_contract::Revision(7))
        ));
        assert!(!leselang_command::command_matches_effect(
            command,
            &effect,
            "command-a",
            "other",
            Some(leselang_host_contract::Revision(7))
        ));
    }
}

#[test]
fn control_plane_correspondence_is_not_authority_canonicalization_or_identity_generation() {
    use leselang_command::{PlannedOperation, command_matches_effect, query_matches_effect};
    use leselang_host_contract::{Revision, RuntimeListFilter};
    use leserpent_domain::{Command, Query};

    for (effect, operation) in control_plane_cases() {
        let PlannedOperation::Command(mut command) = operation else {
            continue;
        };
        command.schema_version = 9;
        command.principal.id.clear();
        command.capabilities = CapabilitySet::new([] as [&str; 0]);
        command.command_id = CommandId::new("caller-command").unwrap();
        command.idempotency_key = IdempotencyKey::new("caller-key").unwrap();
        for revision in [None, Some(Revision(0)), Some(Revision(u64::MAX))] {
            command.expected_revision = revision;
            assert!(command_matches_effect(
                &command,
                &effect,
                "caller-command",
                "caller-key",
                revision
            ));
        }
        if let Effect::RuntimeDeploy { runtime_id, .. } = effect {
            let effect = Effect::RuntimeDeploy {
                runtime_id: runtime_id.clone(),
                pipeline_kind: String::new(),
                target: Some(String::new()),
            };
            command.command = Command::RuntimeDeploy {
                runtime_id,
                pipeline_kind: String::new(),
                target: Some(String::new()),
            };
            assert!(command_matches_effect(
                &command,
                &effect,
                "caller-command",
                "caller-key",
                command.expected_revision
            ));
            assert!(leselang_hir::canonical_source(&effect).is_err());
        }
    }
    let filter = RuntimeListFilter {
        environment: Some(String::new()),
        cluster: None,
        role: None,
    };
    let effect = Effect::RuntimeList {
        filter: filter.clone(),
    };
    let query = Query::RuntimeList { filter };
    assert!(query_matches_effect(&query, &effect));
    assert!(!query_matches_effect(
        &query,
        &Effect::RuntimeList {
            filter: Default::default()
        }
    ));
    assert!(leselang_hir::canonical_source(&effect).is_err());
}

#[test]
fn control_plane_correspondence_rejects_non_language_registration_commands() {
    use leselang_command::{PlannedOperation, command_matches_effect};
    use leselang_host_contract::{Revision, RuntimeId};
    use leserpent_domain::Command;
    let runtime_id = RuntimeId::new("runtime-a").unwrap();
    let commands = [
        Command::RuntimeRegister {
            runtime_id: runtime_id.clone(),
            name: "name".into(),
            endpoint: "http://127.0.0.1:8080".into(),
            sidecar_endpoint: None,
            tags: Default::default(),
        },
        Command::RuntimeRegistrationUpdate {
            runtime_id: runtime_id.clone(),
            name: "name".into(),
            endpoint: "http://127.0.0.1:8080".into(),
            sidecar_endpoint: None,
            tags: Default::default(),
        },
        Command::RuntimeDiscoveryIntake {
            runtime_id,
            capabilities: None,
            status: None,
            sidecar_status: None,
        },
    ];
    for (effect, operation) in control_plane_cases() {
        let PlannedOperation::Command(mut envelope) = operation else {
            continue;
        };
        for command in &commands {
            envelope.command = command.clone();
            assert!(!command_matches_effect(
                &envelope,
                &effect,
                "command-a",
                "effect-a",
                Some(Revision(7))
            ));
        }
    }
}

#[test]
fn all_presentation_requests_match_only_their_own_effect_without_mutation() {
    let cases = reply_cases();
    assert_eq!(cases.len(), 62);
    for (index, case) in cases.iter().enumerate() {
        let effect = effect_from_wire(&case.operation_wire);
        let before = serde_json::to_vec(&(&effect, &case.operation)).unwrap();
        assert_eq!(
            leselang_hir::host_call::HostOperation::for_effect(&effect)
                .unwrap()
                .required_capability(),
            "ui.presentation"
        );
        for (other_index, other) in cases.iter().enumerate() {
            assert_eq!(
                other.operation.matches_effect(&effect),
                index == other_index
            );
        }
        assert_eq!(
            serde_json::to_vec(&(&effect, &case.operation)).unwrap(),
            before
        );
    }
}

#[test]
fn presentation_requests_reject_each_torn_field_and_preserve_optional_distinctions() {
    for case in reply_cases() {
        let effect = effect_from_wire(&case.operation_wire);
        for (field, value) in case.operation_wire.as_object().unwrap() {
            if field == "kind" {
                continue;
            }
            let mut changed = case.operation_wire.clone();
            changed[field] = different_field(field, value);
            let operation: PresentationOperation = serde_json::from_value(changed.clone()).unwrap();
            assert!(!operation.matches_effect(&effect), "{changed}");
            if field != "timeout_ms" {
                assert!(
                    !case.operation.matches_effect(&effect_from_wire(&changed)),
                    "{changed}"
                );
            }
        }
        if case.operation_wire.get("timeout_ms").is_some() {
            for timeout in [0, 1, 1999, 2000, 2001, u64::MAX] {
                let mut changed = case.operation_wire.clone();
                changed["timeout_ms"] = json!(timeout);
                let operation: PresentationOperation = serde_json::from_value(changed).unwrap();
                assert_eq!(operation.matches_effect(&effect), timeout == 2000);
            }
        }
        let name = operation_name(&case.operation);
        if name.ends_with("unavailable_reason") || name.ends_with("placeholder") {
            for expected in [Value::Null, json!(""), json!("present")] {
                let mut pending = case.operation_wire.clone();
                pending["expected"] = expected.clone();
                let effect = effect_from_wire(&pending);
                for actual in [Value::Null, json!(""), json!("present")] {
                    let mut operation = pending.clone();
                    operation["expected"] = actual.clone();
                    let operation: PresentationOperation =
                        serde_json::from_value(operation).unwrap();
                    assert_eq!(operation.matches_effect(&effect), actual == expected);
                }
            }
        }
    }
}

#[test]
fn all_presentation_replies_match_only_their_own_effect_and_saved_operation() {
    let cases = reply_cases();
    assert_eq!(cases.len(), 62);
    for (index, case) in cases.iter().enumerate() {
        let effect = effect_from_wire(&case.operation_wire);
        let operation = envelope(case.operation.clone());
        let before = serde_json::to_vec(&(&effect, &operation, &case.reply)).unwrap();
        assert!(case.reply.matches_effect(&effect, None));
        assert!(case.reply.matches_effect(&effect, Some(&operation)));
        for (other_index, other) in cases.iter().enumerate() {
            let expected = index == other_index;
            assert_eq!(other.reply.matches_effect(&effect, None), expected);
            assert_eq!(
                other.reply.matches_effect(&effect, Some(&operation)),
                expected
            );
            assert_eq!(
                case.reply
                    .matches_effect(&effect, Some(&envelope(other.operation.clone()))),
                expected
            );
            assert_eq!(
                case.reply
                    .matches_effect(&effect_from_wire(&other.operation_wire), Some(&operation)),
                expected
            );
        }
        assert_eq!(
            serde_json::to_vec(&(&effect, &operation, &case.reply)).unwrap(),
            before
        );
    }
}

fn different_field(field: &str, value: &Value) -> Value {
    match field {
        "node_id" | "field" | "value" | "expected" => json!("different"),
        "timeout_ms" | "count" | "max_length" => json!(value.as_u64().unwrap() + 1),
        "direction" => json!("previous"),
        "state" if value == "selected" => json!("unselected"),
        "state" => json!("optional"),
        "expected_kind" if value == "runtime_card" => json!("column"),
        "expected_kind" => json!("runtime_refresh"),
        "input_kind" => json!("path_token"),
        _ => panic!("add a typed mutation for {field}"),
    }
}

#[test]
fn all_presentation_replies_reject_each_torn_payload_field() {
    for case in reply_cases() {
        let effect = effect_from_wire(&case.operation_wire);
        let operation = envelope(case.operation.clone());
        for (field, value) in case.operation_wire.as_object().unwrap() {
            if field == "kind" {
                continue;
            }
            let changed = different_field(field, value);
            assert_ne!(changed, *value);
            let mut reply_wire = case.reply_wire.clone();
            reply_wire[field] = changed.clone();
            let reply: PresentationResult = serde_json::from_value(reply_wire).unwrap();
            assert!(
                !reply.matches_effect(&effect, None),
                "reply {} {field}",
                reply_name(&reply)
            );
            assert!(!reply.matches_effect(&effect, Some(&operation)));
            let mut operation_wire = case.operation_wire.clone();
            operation_wire[field] = changed;
            let torn = envelope(serde_json::from_value(operation_wire.clone()).unwrap());
            assert!(
                !case.reply.matches_effect(&effect, Some(&torn)),
                "operation {} {field}",
                reply_name(&case.reply)
            );
            // A reply and envelope agreeing with each other is not enough.
            assert!(!reply.matches_effect(&effect, Some(&torn)));
            if field != "timeout_ms" {
                let torn_effect = effect_from_wire(&operation_wire);
                assert!(!case.reply.matches_effect(&torn_effect, None));
                assert!(!case.reply.matches_effect(&torn_effect, Some(&operation)));
            }
        }
    }
}

#[test]
fn presentation_reply_optional_text_keeps_none_empty_and_present_distinct() {
    for case in reply_cases().into_iter().filter(|case| {
        matches!(
            operation_name(&case.operation),
            "assert_action_unavailable_reason"
                | "wait_action_unavailable_reason"
                | "assert_form_field_placeholder"
                | "wait_form_field_placeholder"
        )
    }) {
        let states = [Value::Null, json!(""), json!("ready")];
        for expected in &states {
            let mut operation_wire = case.operation_wire.clone();
            operation_wire["expected"] = expected.clone();
            let effect = effect_from_wire(&operation_wire);
            let operation = envelope(serde_json::from_value(operation_wire).unwrap());
            for actual in &states {
                let mut reply_wire = case.reply_wire.clone();
                reply_wire["expected"] = actual.clone();
                let reply: PresentationResult = serde_json::from_value(reply_wire).unwrap();
                assert_eq!(reply.matches_effect(&effect, None), actual == expected);
                assert_eq!(
                    reply.matches_effect(&effect, Some(&operation)),
                    actual == expected
                );
            }
        }
    }
}

#[test]
fn presentation_reply_navigation_and_count_boundaries_remain_product_rules() {
    for case in reply_cases().into_iter().filter(|case| {
        matches!(
            operation_name(&case.operation),
            "navigate_focus" | "assert_child_count" | "wait_child_count"
        )
    }) {
        for (value, accepted) in if operation_name(&case.operation) == "navigate_focus" {
            vec![
                (json!("child-a"), true),
                (json!("node-a"), false),
                (json!(""), false),
                (json!("bad node"), false),
                (
                    json!("x".repeat(leselang_hir::MAX_UI_NODE_ID_BYTES + 1)),
                    false,
                ),
            ]
        } else {
            vec![
                (json!(0), true),
                (json!(leselang_hir::MAX_UI_CHILD_COUNT), true),
                (json!(leselang_hir::MAX_UI_CHILD_COUNT + 1), false),
            ]
        } {
            let mut operation_wire = case.operation_wire.clone();
            let mut reply_wire = case.reply_wire.clone();
            if operation_name(&case.operation) == "navigate_focus" {
                reply_wire["focused_node_id"] = value;
            } else {
                operation_wire["count"] = value.clone();
                reply_wire["count"] = value;
            }
            let effect = effect_from_wire(&operation_wire);
            let operation = envelope(serde_json::from_value(operation_wire).unwrap());
            let reply: PresentationResult = serde_json::from_value(reply_wire).unwrap();
            assert_eq!(reply.matches_effect(&effect, None), accepted);
            assert_eq!(reply.matches_effect(&effect, Some(&operation)), accepted);
        }
    }
}

#[test]
fn presentation_reply_correspondence_is_not_an_authority_certificate() {
    let case = reply_cases().remove(1);
    let effect = effect_from_wire(&case.operation_wire);
    let mut operation = envelope(case.operation);
    let EffectOperation::Presentation(saved) = &mut operation else {
        panic!()
    };
    saved.schema_version = 0;
    saved.principal.id = "bad principal".into();
    saved.capabilities = CapabilitySet::default();
    assert!(case.reply.matches_effect(&effect, Some(&operation)));
    let foreign = EffectOperation::Query(leserpent_domain::QueryEnvelope {
        schema_version: 1,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["runtime.read"]),
        query: leserpent_domain::Query::RuntimeList {
            filter: Default::default(),
        },
    });
    assert!(!case.reply.matches_effect(&effect, Some(&foreign)));
    for source in [
        "runtime.list()",
        "add(left: 1, right: 2)",
        r#"seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#,
    ] {
        let effect = lower(&parse(&format!("fn main() = {source}")))
            .unwrap()
            .function
            .effect;
        assert!(!case.reply.matches_effect(&effect, None));
        assert!(!case.reply.matches_effect(&effect, Some(&operation)));
    }
}
