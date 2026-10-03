use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::computation::Computation;
use leselang_hir::result_field::ResultField;
use leselang_hir::{UiFormRequirementState, UiSelectionState, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectOperation, EffectRequest, EffectResult, PresentationOperation, PresentationResult,
    ScalarValue, Step, Value, Vm, decode_continuation, encode_continuation,
};
use rusqlite::{Connection, params};

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-boolean-projections-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root.join("flow.sqlite3"))
    }
}
impl Drop for JournalPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}
fn start(vm: &mut Vm, source: &str) -> Step {
    vm.start_timed(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        100,
        100,
    )
}
fn effect(step: Step) -> EffectRequest {
    match step {
        Step::Effect(request) => *request,
        other => panic!("expected effect: {other:?}"),
    }
}
fn done(value: bool) -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::Boolean(value),
    })
}
fn selection(node: &str, selected: bool) -> EffectResult {
    EffectResult::Presentation(PresentationResult::SetSelection {
        node_id: node.into(),
        state: if selected {
            UiSelectionState::Selected
        } else {
            UiSelectionState::Unselected
        },
    })
}
fn requirement() -> EffectResult {
    EffectResult::Presentation(PresentationResult::WaitFormFieldRequired {
        node_id: "form".into(),
        field: "name".into(),
        state: UiFormRequirementState::Optional,
        timeout_ms: leselang_hir::UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS,
    })
}
fn focus(node: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: node.into(),
    })
}
fn fields_wire(request: &EffectRequest) -> serde_json::Value {
    serde_json::to_value(&request.continuation).unwrap()
}
fn downgrade_frame(wire: &mut serde_json::Value, index: usize) {
    let frame = &mut wire["result_binding"]["results"][index]["result"];
    frame.as_object_mut().unwrap().remove("projection_version");
    frame["fields"]
        .as_array_mut()
        .unwrap()
        .retain(|field| field["field"] != "selected" && field["field"] != "required");
}
fn write_request(connection: &Connection, request: &EffectRequest) {
    connection
        .execute(
            "UPDATE vm_effects SET image = ?2 WHERE token = ?1",
            params![
                request.continuation.token.as_str(),
                serde_json::to_vec(&request.continuation).unwrap()
            ],
        )
        .unwrap();
    connection
        .execute(
            "UPDATE vm_dispatches SET request = ?2 WHERE token = ?1",
            params![
                request.continuation.token.as_str(),
                serde_json::to_vec(request).unwrap()
            ],
        )
        .unwrap();
}

const FLOW: &str = r#"fn label(value: boolean) = choose(when: value, then: "chosen", otherwise: "skipped")
fn main() = bind(first: ui.set_selection(node_id: "toggle", state: "selected"), body:
    bind(alias: first, body:
        bind(second: ui.wait_form_field_required(node_id: "form", field: "name", state: "optional"), body:
            bind(third: ui.focus(node_id: label(value: field(value: alias, name: "selected"))), body:
                and(left: field(value: first, name: "selected"),
                    right: not(value: field(value: second, name: "required")))))))"#;

#[test]
fn all_exporting_operations_project_both_boolean_polarities_without_text_coercion() {
    for positive in [false, true] {
        for operation in [
            "set_selection",
            "assert_selection",
            "wait_selection",
            "assert_form_field_required",
            "wait_form_field_required",
        ] {
            let required = operation.ends_with("required");
            let state = match (required, positive) {
                (true, true) => "required",
                (true, false) => "optional",
                (false, true) => "selected",
                (false, false) => "unselected",
            };
            let extra = if required { r#"field: "name", "# } else { "" };
            let field = if required { "required" } else { "selected" };
            let source = format!(
                r#"fn invert(value: boolean) = not(value: value)
                fn main() = bind(r: ui.{operation}(node_id: "a", {extra}state: "{state}"),
                    body: invert(value: field(value: r, name: "{field}")))"#
            );
            let path = JournalPath::new();
            for mut vm in [Vm::new(200), Vm::open_journal(&path.0, 200).unwrap()] {
                let request = effect(start(&mut vm, &source));
                assert_eq!(request.continuation.schema_version, 2);
                let selected = if positive {
                    UiSelectionState::Selected
                } else {
                    UiSelectionState::Unselected
                };
                let requirement = if positive {
                    UiFormRequirementState::Required
                } else {
                    UiFormRequirementState::Optional
                };
                let result = match operation {
                    "set_selection" => PresentationResult::SetSelection {
                        node_id: "a".into(),
                        state: selected,
                    },
                    "assert_selection" => PresentationResult::AssertSelection {
                        node_id: "a".into(),
                        state: selected,
                    },
                    "wait_selection" => PresentationResult::WaitSelection {
                        node_id: "a".into(),
                        state: selected,
                        timeout_ms: leselang_hir::UI_WAIT_SELECTION_TIMEOUT_MS,
                    },
                    "assert_form_field_required" => PresentationResult::AssertFormFieldRequired {
                        node_id: "a".into(),
                        field: "name".into(),
                        state: requirement,
                    },
                    "wait_form_field_required" => PresentationResult::WaitFormFieldRequired {
                        node_id: "a".into(),
                        field: "name".into(),
                        state: requirement,
                        timeout_ms: leselang_hir::UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS,
                    },
                    _ => unreachable!(),
                };
                assert_eq!(
                    vm.resume_at(
                        &request.continuation,
                        101,
                        EffectResult::Presentation(result)
                    ),
                    done(!positive)
                );
                assert_eq!(
                    vm.resume_at(&request.continuation, 102, focus("ignored")),
                    done(!positive)
                );
            }
        }
    }
}

#[test]
fn boolean_frames_and_helpers_survive_each_restart_with_original_authority_and_budgets() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, FLOW));
    let second = effect(vm.resume_at(&first.continuation, 101, selection("toggle", true)));
    let frame = &second.continuation.result_binding.as_ref().unwrap().results[0].result;
    assert_eq!(frame.projection_version, 2);
    assert_eq!(frame.fields[1].value, ScalarValue::Boolean(true));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        effect(vm.resume_at(&first.continuation, 102, selection("toggle", false))),
        second
    );
    let third = effect(vm.resume_at(&second.continuation, 102, requirement()));
    assert!(
        matches!(&third.operation, EffectOperation::Presentation(envelope)
        if matches!(&envelope.operation, PresentationOperation::Focus { node_id } if node_id == "chosen"))
    );
    for request in [&second, &third] {
        assert_eq!(request.continuation.expected_revision, Some(Revision(7)));
        assert_eq!(request.budget.deadline_at_ms, Some(200));
        assert_eq!(
            request.budget.max_output_items,
            first.budget.max_output_items
        );
        let EffectOperation::Presentation(envelope) = &request.operation else {
            panic!()
        };
        let EffectOperation::Presentation(original) = &first.operation else {
            panic!()
        };
        assert_eq!(envelope.principal, original.principal);
        assert_eq!(envelope.capabilities, original.capabilities);
        assert_eq!(
            decode_continuation(&encode_continuation(&request.continuation).unwrap()).unwrap(),
            request.continuation
        );
    }
    assert!(first.budget.fuel_remaining > second.budget.fuel_remaining);
    assert!(second.budget.fuel_remaining > third.budget.fuel_remaining);
    let frames = &third.continuation.result_binding.as_ref().unwrap().results;
    assert_eq!(frames.len(), 3);
    assert!(
        frames
            .iter()
            .all(|saved| saved.result.projection_version
                == if saved.name == "second" { 3 } else { 2 })
    );
    assert_eq!(frames[0].result, frames[1].result);
    assert_eq!(
        frames[2].result.fields[1].value,
        ScalarValue::Boolean(false)
    );
    assert_eq!(frames[2].result.fields[2].field, ResultField::Field);
    assert_eq!(
        frames[2].result.fields[2].value,
        ScalarValue::String("name".into())
    );
    let wire = serde_json::to_string(&frames[2].result).unwrap();
    assert!(!wire.contains("\"state\":") && !wire.contains("\"raw\":"));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume_at(&third.continuation, 103, focus("chosen")),
        done(true)
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume_at(&first.continuation, 201, selection("toggle", false)),
        done(true)
    );
    assert_eq!(vm.pending_count(), 0);
    assert!(vm.claim_effect(201, 100).unwrap().is_none());
}

#[test]
fn named_group_booleans_work_in_parallel_and_sequential_recovery() {
    for group in ["all", "seq"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let source = format!(
            r#"fn combine(a: boolean, b: boolean) = and(left: a, right: not(value: b))
            fn main() = bind(g: {group}(
                selection: ui.set_selection(node_id: "toggle", state: "selected"),
                requirement: ui.wait_form_field_required(node_id: "form", field: "name", state: "optional")),
                body: combine(a: field(value: member(value: g, name: "selection"), name: "selected"),
                    b: field(value: member(value: g, name: "requirement"), name: "required")))"#
        );
        let first = match start(&mut vm, &source) {
            Step::Effect(request) => *request,
            Step::Effects(batch) => batch.branches[0].request.clone(),
            other => panic!("{other:?}"),
        };
        let next = vm.resume_at(&first.continuation, 101, selection("toggle", true));
        assert!(matches!(next, Step::Effect(_) | Step::Waiting(_)));
        let second = vm
            .pending_continuations()
            .into_iter()
            .find(|image| {
                matches!(
                    image.pending_effect,
                    leselang_hir::Effect::UiWaitFormFieldRequired { .. }
                )
            })
            .unwrap();
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(vm.resume_at(&second, 102, requirement()), done(true));
        assert_eq!(
            vm.resume_at(&first.continuation, 103, selection("toggle", false)),
            done(true)
        );
    }
}

#[test]
fn wrong_host_state_cannot_be_projected_or_admit_a_successor() {
    for result in [
        selection("toggle", false),
        selection("other", true),
        focus("toggle"),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(start(&mut vm, FLOW));
        assert!(matches!(
            vm.resume_at(&first.continuation, 101, result),
            Step::Fault(_)
        ));
        assert_eq!(vm.pending_count(), 0);
        let count: i64 = Connection::open(&path.0)
            .unwrap()
            .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1);
    }
}

#[test]
fn version_type_order_and_closed_field_set_corruption_are_rejected_before_restore() {
    let mut vm = Vm::new(500);
    let first = effect(start(&mut vm, FLOW));
    let second = effect(vm.resume_at(&first.continuation, 101, selection("toggle", true)));
    let original = fields_wire(&second);
    for corruption in [
        "version",
        "missing_version",
        "type",
        "missing",
        "duplicate",
        "order",
        "extra",
    ] {
        let mut wire = original.clone();
        let frame = &mut wire["result_binding"]["results"][0]["result"];
        match corruption {
            "version" => frame["projection_version"] = 6.into(),
            "missing_version" => {
                frame.as_object_mut().unwrap().remove("projection_version");
            }
            "type" => {
                frame["fields"][1]["value"] =
                    serde_json::json!({"kind":"string","value":"selected"})
            }
            "missing" => {
                frame["fields"].as_array_mut().unwrap().pop();
            }
            "duplicate" => {
                let duplicate = frame["fields"][0].clone();
                frame["fields"][1] = duplicate;
            }
            "order" => frame["fields"].as_array_mut().unwrap().swap(0, 1),
            "extra" => frame["raw_state"] = "selected".into(),
            _ => unreachable!(),
        }
        assert!(
            decode_continuation(&serde_json::to_vec(&wire).unwrap()).is_err(),
            "{corruption}"
        );
    }
    let mut legacy = original;
    downgrade_frame(&mut legacy, 0);
    downgrade_frame(&mut legacy, 1);
    // The original body accesses an alias of this absent field, including helper expansion.
    assert_eq!(
        decode_continuation(&serde_json::to_vec(&legacy).unwrap())
            .unwrap_err()
            .code,
        "LSV1405"
    );
}

#[test]
fn stored_boolean_rebinding_is_rejected_against_the_committed_host_result() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, FLOW));
    let mut second = effect(vm.resume_at(&first.continuation, 101, selection("toggle", true)));
    for saved in &mut second.continuation.result_binding.as_mut().unwrap().results {
        saved.result.fields[1].value = ScalarValue::Boolean(false);
    }
    // The altered frame is structurally typed, but not the winner of the first commit.
    assert!(encode_continuation(&second.continuation).is_ok());
    drop(vm);
    write_request(&Connection::open(&path.0).unwrap(), &second);
    assert!(Vm::open_journal(&path.0, 500).is_err());
}

#[test]
fn unversioned_v1_journal_frames_replay_byte_exactly_and_coexist_with_new_v2_frames() {
    let source = r#"fn main() = bind(first: ui.set_selection(node_id: "a", state: "selected"), body:
        bind(second: ui.assert_selection(node_id: "b", state: "unselected"), body:
            bind(third: ui.focus(node_id: field(value: first, name: "node_id")),
                body: eq(left: field(value: first, name: "node_id"), right: field(value: third, name: "node_id")))))"#;
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, source));
    let mut second = effect(vm.resume_at(&first.continuation, 101, selection("a", true)));
    let mut legacy = fields_wire(&second);
    downgrade_frame(&mut legacy, 0);
    let bytes = serde_json::to_vec(&legacy).unwrap();
    second.continuation = decode_continuation(&bytes).unwrap();
    let encoded = encode_continuation(&second.continuation).unwrap();
    assert_eq!(
        encode_continuation(&decode_continuation(&encoded).unwrap()).unwrap(),
        encoded
    );
    assert_eq!(
        serde_json::to_string(
            &second.continuation.result_binding.as_ref().unwrap().results[0].result
        )
        .unwrap(),
        r#"{"operation":"ui.set_selection","fields":[{"field":"node_id","value":{"kind":"string","value":"a"}}]}"#
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(
            &encode_continuation(&second.continuation).unwrap()
        )
        .unwrap(),
        legacy
    );
    assert!(
        !String::from_utf8(encode_continuation(&second.continuation).unwrap())
            .unwrap()
            .contains("projection_version")
    );
    drop(vm);
    write_request(&Connection::open(&path.0).unwrap(), &second);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        effect(vm.resume_at(&first.continuation, 102, selection("a", false))),
        second
    );
    let third = effect(vm.resume_at(
        &second.continuation,
        102,
        EffectResult::Presentation(PresentationResult::AssertSelection {
            node_id: "b".into(),
            state: UiSelectionState::Unselected,
        }),
    ));
    let saved = &third.continuation.result_binding.as_ref().unwrap().results;
    assert_eq!(saved[0].result.projection_version, 1);
    assert_eq!(saved[0].result.fields.len(), 1);
    assert_eq!(saved[1].result.projection_version, 2);
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume_at(&third.continuation, 103, focus("a")),
        done(true)
    );
    assert_eq!(
        vm.resume_at(&first.continuation, 104, selection("a", false)),
        done(true)
    );
}

#[test]
fn unaffected_operations_keep_the_unversioned_v1_projection_wire_shape() {
    let source = r#"fn main() = bind(first: ui.focus(node_id: "a"), body:
        bind(second: ui.focus(node_id: "b"), body: field(value: first, name: "node_id")))"#;
    let mut vm = Vm::new(100);
    let first = effect(start(&mut vm, source));
    let second = effect(vm.resume_at(&first.continuation, 101, focus("a")));
    let frame = &fields_wire(&second)["result_binding"]["results"][0]["result"];
    assert_eq!(
        *frame,
        serde_json::json!({
            "operation":"ui.focus",
            "fields":[{"field":"node_id","value":{"kind":"string","value":"a"}}]
        })
    );
}

#[test]
fn absent_legacy_fields_are_rejected_through_conditional_aliases_and_cold_paths() {
    let mut vm = Vm::new(500);
    let first = effect(start(&mut vm, FLOW));
    let mut second = effect(vm.resume_at(&first.continuation, 101, selection("toggle", true)));
    for saved in &mut second.continuation.result_binding.as_mut().unwrap().results {
        saved.result.projection_version = 1;
        saved
            .result
            .fields
            .retain(|field| ResultField::V1.contains(&field.field));
    }
    let local = |name: &str| Box::new(Computation::Local { name: name.into() });
    second.continuation.result_binding.as_mut().unwrap().body = Computation::Bind {
        name: "later_alias".into(),
        value: Box::new(Computation::Choose {
            when: Box::new(Computation::Literal {
                value: ScalarValue::Boolean(true),
            }),
            then: local("first"),
            otherwise: local("alias"),
        }),
        body: Box::new(Computation::Choose {
            when: Box::new(Computation::Literal {
                value: ScalarValue::Boolean(true),
            }),
            then: Box::new(Computation::Literal {
                value: ScalarValue::Boolean(false),
            }),
            otherwise: Box::new(Computation::Field {
                value: local("later_alias"),
                field: ResultField::Selected,
            }),
        }),
    };
    assert_eq!(
        encode_continuation(&second.continuation).unwrap_err().code,
        "LSV1405"
    );
    assert_eq!(
        decode_continuation(&serde_json::to_vec(&second.continuation).unwrap())
            .unwrap_err()
            .code,
        "LSV1405"
    );
    assert!(Vm::new(500).restore_request(second).is_err());
}

#[test]
fn legacy_reader_shape_rejects_versioned_frames_and_new_field_downgrades() {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "snake_case")]
    enum LegacyField {
        Revision,
        Count,
        NodeId,
        FocusedNodeId,
        MaxLength,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LegacyProjection {
        #[serde(rename = "field")]
        _field: LegacyField,
        #[serde(rename = "value")]
        _value: ScalarValue,
    }
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct LegacyFrame {
        #[serde(rename = "operation")]
        _operation: String,
        #[serde(rename = "fields")]
        _fields: Vec<LegacyProjection>,
    }
    let mut vm = Vm::new(500);
    let first = effect(start(&mut vm, FLOW));
    let second = effect(vm.resume_at(&first.continuation, 101, selection("toggle", true)));
    let mut frame = fields_wire(&second)["result_binding"]["results"][0]["result"].clone();
    assert!(serde_json::from_value::<LegacyFrame>(frame.clone()).is_err());
    frame.as_object_mut().unwrap().remove("projection_version");
    assert!(serde_json::from_value::<LegacyFrame>(frame.clone()).is_err());
    frame["fields"]
        .as_array_mut()
        .unwrap()
        .retain(|field| field["field"] == "node_id");
    assert!(serde_json::from_value::<LegacyFrame>(frame).is_ok());
}
