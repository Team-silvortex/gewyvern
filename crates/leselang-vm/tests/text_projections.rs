use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::computation::Computation;
use leselang_hir::lower;
use leselang_hir::result_field::ResultField;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectOperation, EffectRequest, EffectResult, PresentationResult, ScalarValue, Step, Value, Vm,
    decode_continuation, encode_continuation,
};
use rusqlite::{Connection, params};

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-text-projections-{}-{}",
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
fn done(value: &str) -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::String(value.into()),
    })
}
fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    // These assertion/set fixtures acknowledge the exact requested operation, including wait limits.
    EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}
fn count(connection: &Connection) -> i64 {
    connection
        .query_row("SELECT COUNT(*) FROM vm_effects", [], |row| row.get(0))
        .unwrap()
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
fn downgrade(request: &mut EffectRequest, version: u32) {
    let fields = if version == 1 {
        &ResultField::V1[..]
    } else {
        &ResultField::V2[..]
    };
    for saved in &mut request
        .continuation
        .result_binding
        .as_mut()
        .unwrap()
        .results
    {
        saved.result.projection_version = version;
        saved
            .result
            .fields
            .retain(|field| fields.contains(&field.field));
    }
}

const FLOW: &str = r#"fn copy(text: string) = concat(left: text, right: "-value")
fn main() = bind(first: ui.assert_text(node_id: "a", expected: "alpha"), body:
    bind(alias: first, body:
        bind(second: ui.set_form_value(node_id: "form", field: "name", value: copy(text: field(value: alias, name: "expected"))), body:
            bind(third: ui.wait_form_value(node_id: "form", field: field(value: second, name: "field"), expected: field(value: second, name: "value")), body:
                choose(when: eq(left: field(value: third, name: "expected"), right: copy(text: field(value: first, name: "expected"))), then: field(value: second, name: "value"), otherwise: "mismatch")))))"#;

#[test]
fn every_exporting_text_operation_projects_confirmed_strings_in_both_journals() {
    let expected_ops = [
        "assert_text",
        "wait_text",
        "assert_automation_id",
        "wait_automation_id",
        "assert_action_label",
        "wait_action_label",
        "assert_form_value",
        "wait_form_value",
        "assert_form_field",
        "wait_form_field",
        "assert_accessible_name",
        "wait_accessible_name",
        "assert_accessible_description",
        "wait_accessible_description",
    ];
    let field_ops = [
        ("set_form_value", r#"value: "confirmed""#),
        ("assert_form_value", r#"expected: "confirmed""#),
        ("wait_form_value", r#"expected: "confirmed""#),
        ("assert_form_field", r#"expected: "Name""#),
        ("wait_form_field", r#"expected: "Name""#),
        ("assert_form_field_input_kind", r#"kind: "trimmed_text""#),
        ("wait_form_field_input_kind", r#"kind: "trimmed_text""#),
        ("assert_form_field_required", r#"state: "required""#),
        ("wait_form_field_required", r#"state: "optional""#),
        ("assert_form_field_max_length", r#"max_length: "7""#),
        ("wait_form_field_max_length", r#"max_length: "7""#),
        ("assert_form_field_placeholder", "expected: none"),
        ("wait_form_field_placeholder", "expected: none"),
    ];
    let mut fixtures = expected_ops
        .into_iter()
        .map(|operation| {
            let extra = if operation.contains("form_") {
                r#"field: "name", "#
            } else {
                ""
            };
            (
                operation,
                format!(r#"{extra}expected: "confirmed""#),
                "expected",
                "confirmed",
            )
        })
        .collect::<Vec<_>>();
    fixtures.extend(field_ops.into_iter().map(|(operation, argument)| {
        (
            operation,
            format!(r#"field: "name", {argument}"#),
            "field",
            "name",
        )
    }));
    fixtures.push((
        "set_form_value",
        r#"field: "name", value: "confirmed""#.into(),
        "value",
        "confirmed",
    ));
    for (operation, arguments, field, value) in fixtures {
        let path = JournalPath::new();
        let source = format!(
            r#"fn main() = bind(r: ui.{operation}(node_id: "a", {arguments}), body: field(value: r, name: "{field}"))"#
        );
        for mut vm in [Vm::new(1000), Vm::open_journal(&path.0, 1000).unwrap()] {
            let request = effect(start(&mut vm, &source));
            assert_eq!(
                vm.resume_at(&request.continuation, 101, receipt(&request)),
                done(value)
            );
            assert_eq!(
                vm.resume_at(&request.continuation, 102, receipt(&request)),
                done(value)
            );
        }
    }
}

#[test]
fn text_and_high_bit_field_value_projections_survive_aliases_restarts_and_helpers() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let first = effect(start(&mut vm, FLOW));
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    let frames = &second.continuation.result_binding.as_ref().unwrap().results;
    assert_eq!(frames.len(), 2);
    assert_eq!(frames[0].result, frames[1].result);
    assert!(
        frames
            .iter()
            .all(|saved| saved.result.projection_version == 3)
    );
    assert_eq!(
        frames[0]
            .result
            .fields
            .iter()
            .map(|field| field.field)
            .collect::<Vec<_>>(),
        [ResultField::NodeId, ResultField::Expected]
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume_at(&first.continuation, 102, receipt(&first)),
        Step::Effect(Box::new(second.clone()))
    );
    let third = effect(vm.resume_at(&second.continuation, 102, receipt(&second)));
    let frames = &third.continuation.result_binding.as_ref().unwrap().results;
    assert_eq!(
        frames[2]
            .result
            .fields
            .iter()
            .map(|field| field.field)
            .collect::<Vec<_>>(),
        [ResultField::NodeId, ResultField::Field, ResultField::Value]
    );
    assert_eq!(
        frames[2].result.fields[2].value,
        ScalarValue::String("alpha-value".into())
    );
    assert_eq!(third.continuation.schema_version, 4);
    for request in [&second, &third] {
        assert_eq!(request.continuation.expected_revision, Some(Revision(7)));
        assert_eq!(request.budget.deadline_at_ms, Some(200));
        assert_eq!(
            request.budget.max_output_items,
            first.budget.max_output_items
        );
        assert_eq!(
            decode_continuation(&encode_continuation(&request.continuation).unwrap()).unwrap(),
            request.continuation
        );
    }
    assert!(
        first.budget.fuel_remaining > second.budget.fuel_remaining
            && second.budget.fuel_remaining > third.budget.fuel_remaining
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume_at(&third.continuation, 103, receipt(&third)),
        done("alpha-value")
    );
    assert_eq!(
        vm.resume_at(&first.continuation, 104, receipt(&first)),
        done("alpha-value")
    );
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn sequential_and_parallel_group_text_frames_drive_scalar_exits_and_captured_successors() {
    for group in ["seq", "all"] {
        for exiting in [false, true] {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
            let source = format!(
                r#"fn main() = bind(g: {group}(text: ui.assert_text(node_id: "a", expected: "alpha"), form: ui.set_form_value(node_id: "form", field: "name", value: "beta")), body:
                bind(alias: g, body: choose(when: {exiting}, then: field(value: member(value: alias, name: "form"), name: "value"), otherwise:
                    bind(r: ui.wait_form_value(node_id: "form", field: field(value: member(value: g, name: "form"), name: "field"), expected: field(value: member(value: alias, name: "form"), name: "value")), body:
                        choose(when: eq(left: field(value: member(value: g, name: "text"), name: "expected"), right: "alpha"), then: field(value: r, name: "expected"), otherwise: "mismatch")))))"#
            );
            let (first, mut step) = match start(&mut vm, &source) {
                Step::Effect(first) => {
                    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
                    let step = vm.resume_at(&second.continuation, 102, receipt(&second));
                    (*first, step)
                }
                Step::Effects(batch) => {
                    assert!(matches!(
                        vm.resume_at(
                            &batch.branches[1].request.continuation,
                            101,
                            receipt(&batch.branches[1].request)
                        ),
                        Step::Waiting(_)
                    ));
                    let first = batch.branches[0].request.clone();
                    let step = vm.resume_at(&first.continuation, 102, receipt(&first));
                    (first, step)
                }
                other => panic!("{other:?}"),
            };
            assert_eq!(first.continuation.schema_version, 11);
            drop(vm);
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            if !exiting {
                let current = effect(step);
                let groups = &current.continuation.result_binding.as_ref().unwrap().groups;
                assert_eq!(groups.len(), 2);
                assert!(
                    groups
                        .iter()
                        .flat_map(|group| &group.group.members)
                        .all(|member| member.result.projection_version == 3)
                );
                step = vm.resume_at(&current.continuation, 103, receipt(&current));
            }
            assert_eq!(step, done("beta"));
            assert_eq!(
                vm.resume_at(&first.continuation, 104, receipt(&first)),
                step
            );
            assert_eq!(
                count(&Connection::open(&path.0).unwrap()),
                if exiting { 2 } else { 3 }
            );
        }
    }
}

#[test]
fn legacy_v1_and_v2_frames_remain_exact_and_coexist_with_new_v3_captures() {
    for version in [1, 2] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let operation = if version == 1 {
            r#"ui.assert_text(node_id: "a", expected: "alpha")"#
        } else {
            r#"ui.wait_form_field_required(node_id: "a", field: "name", state: "optional")"#
        };
        let final_body = if version == 1 {
            r#"concat(left: field(value: previous, name: "node_id"), right: field(value: second, name: "value"))"#
        } else {
            r#"choose(when: field(value: previous, name: "required"), then: "required", otherwise: field(value: second, name: "value"))"#
        };
        let source = format!(
            r#"fn main() = bind(first: {operation}, body: bind(previous: first, body: bind(second: ui.set_form_value(node_id: "form", field: "name", value: "written"), body: bind(third: ui.focus(node_id: "a"), body: {final_body}))))"#
        );
        let first = effect(start(&mut vm, &source));
        let mut second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
        downgrade(&mut second, version);
        let legacy = encode_continuation(&second.continuation).unwrap();
        assert_eq!(
            encode_continuation(&decode_continuation(&legacy).unwrap()).unwrap(),
            legacy
        );
        for saved in &second.continuation.result_binding.as_ref().unwrap().results {
            assert_eq!(saved.result.projection_version, version);
            assert!(saved.result.fields.iter().all(|field| {
                ![
                    ResultField::Expected,
                    ResultField::Field,
                    ResultField::Value,
                ]
                .contains(&field.field)
            }));
        }
        drop(vm);
        write_request(&Connection::open(&path.0).unwrap(), &second);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 102, receipt(&first)),
            Step::Effect(Box::new(second.clone()))
        );
        let third = effect(vm.resume_at(&second.continuation, 102, receipt(&second)));
        let frames = &third.continuation.result_binding.as_ref().unwrap().results;
        assert_eq!(frames[0].result.projection_version, version);
        assert_eq!(frames[1].result.projection_version, version);
        assert_eq!(frames[2].result.projection_version, 3);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&third.continuation, 103, receipt(&third)),
            done(if version == 1 { "awritten" } else { "written" })
        );
    }
}

#[test]
fn v3_version_fields_types_order_and_size_corruption_fail_closed() {
    let mut vm = Vm::new(10000);
    let first = effect(start(
        &mut vm,
        r#"fn main() = bind(first: ui.assert_form_value(node_id: "form", field: "name", expected: "written"), body: bind(second: ui.focus(node_id: "a"), body: field(value: first, name: "expected")))"#,
    ));
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    let wire = serde_json::to_value(&second.continuation).unwrap();
    for mutation in 0..9 {
        let mut wire = wire.clone();
        let frame = &mut wire["result_binding"]["results"][0]["result"];
        match mutation {
            0 => frame["projection_version"] = 6.into(),
            1 => frame["projection_version"] = 2.into(),
            2 => {
                frame.as_object_mut().unwrap().remove("projection_version");
            }
            3 => {
                frame["fields"].as_array_mut().unwrap().pop();
            }
            4 => frame["fields"][1] = frame["fields"][0].clone(),
            5 => frame["fields"].as_array_mut().unwrap().swap(1, 2),
            6 => {
                frame["fields"][1]["value"] =
                    serde_json::to_value(ScalarValue::Boolean(true)).unwrap()
            }
            7 => {
                frame["fields"][1]["value"] =
                    serde_json::to_value(ScalarValue::String("x".repeat(4097))).unwrap()
            }
            8 => frame["raw"] = serde_json::json!({"password":"not-a-projection"}),
            _ => unreachable!(),
        }
        assert!(
            decode_continuation(&serde_json::to_vec(&wire).unwrap()).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn missing_legacy_fields_cannot_be_synthesized_through_aliases_or_cold_choices() {
    for field in [
        ResultField::Expected,
        ResultField::Field,
        ResultField::Value,
    ] {
        let mut vm = Vm::new(10000);
        let operation = if field == ResultField::Expected {
            r#"ui.assert_form_value(node_id: "form", field: "name", expected: "written")"#
        } else {
            r#"ui.set_form_value(node_id: "form", field: "name", value: "written")"#
        };
        let source = format!(
            r#"fn main() = bind(first: {operation}, body: bind(alias: first, body: bind(second: ui.focus(node_id: "a"), body: field(value: first, name: "node_id"))))"#
        );
        let first = effect(start(&mut vm, &source));
        let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
        let mut mixed = second.clone();
        let saved = &mut mixed.continuation.result_binding.as_mut().unwrap().results[1].result;
        saved.projection_version = 2;
        saved
            .fields
            .retain(|field| ResultField::V2.contains(&field.field));
        let local = |name: &str| Box::new(Computation::Local { name: name.into() });
        mixed.continuation.result_binding.as_mut().unwrap().body = Computation::Bind {
            name: "chosen".into(),
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
                    value: ScalarValue::String("".into()),
                }),
                otherwise: Box::new(Computation::Field {
                    value: local("chosen"),
                    field,
                }),
            }),
        };
        assert_eq!(
            encode_continuation(&mixed.continuation).unwrap_err().code,
            "LSV1405"
        );
        assert!(decode_continuation(&serde_json::to_vec(&mixed.continuation).unwrap()).is_err());
    }
}

#[test]
fn missing_group_text_fields_cannot_be_read_through_cold_group_aliases() {
    let mut vm = Vm::new(10000);
    let source = r#"fn main() = bind(g: seq(a: ui.set_form_value(node_id: "form", field: "name", value: "written")), body: bind(alias: g, body: bind(r: ui.focus(node_id: "a"), body: choose(when: true, then: "", otherwise: field(value: member(value: alias, name: "a"), name: "value")))))"#;
    let prefix = effect(start(&mut vm, source));
    let mut current = effect(vm.resume_at(&prefix.continuation, 101, receipt(&prefix)));
    for group in &mut current.continuation.result_binding.as_mut().unwrap().groups {
        for member in &mut group.group.members {
            member.result.projection_version = 2;
            member
                .result
                .fields
                .retain(|field| ResultField::V2.contains(&field.field));
        }
    }
    assert_eq!(
        encode_continuation(&current.continuation).unwrap_err().code,
        "LSV1405"
    );
}

#[test]
fn changed_durable_text_and_field_frames_are_rejected_against_raw_receipts() {
    for field in [ResultField::Field, ResultField::Value] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(
            &mut vm,
            r#"fn main() = bind(first: ui.set_form_value(node_id: "form", field: "name", value: "written"), body: bind(second: ui.focus(node_id: "a"), body: field(value: first, name: "value")))"#,
        ));
        let mut second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
        let frame = &mut second.continuation.result_binding.as_mut().unwrap().results[0].result;
        frame
            .fields
            .iter_mut()
            .find(|saved| saved.field == field)
            .unwrap()
            .value = ScalarValue::String("forged".into());
        assert!(encode_continuation(&second.continuation).is_ok());
        drop(vm);
        write_request(&Connection::open(&path.0).unwrap(), &second);
        assert!(Vm::open_journal(&path.0, 1).is_err());
    }
}

#[test]
fn wrong_expected_field_or_value_acknowledgements_never_create_successors() {
    for mutation in ["node_id", "field", "value"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(
            &mut vm,
            r#"fn main() = bind(r: ui.set_form_value(node_id: "form", field: "name", value: "written"), body: bind(s: ui.focus(node_id: field(value: r, name: "value")), body: "done"))"#,
        ));
        let mut result = serde_json::to_value(match receipt(&first) {
            EffectResult::Presentation(result) => result,
            _ => unreachable!(),
        })
        .unwrap();
        result[mutation] = "wrong".into();
        let step = vm.resume_at(
            &first.continuation,
            101,
            EffectResult::Presentation(serde_json::from_value(result).unwrap()),
        );
        assert!(matches!(&step, Step::Fault(_)));
        assert_eq!(count(&Connection::open(&path.0).unwrap()), 1);
        assert_eq!(vm.pending_count(), 0);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 102, receipt(&first)),
            step
        );
    }
    let mut vm = Vm::new(10000);
    let first = effect(start(&mut vm, FLOW));
    assert!(matches!(
        vm.resume_at(
            &first.continuation,
            101,
            EffectResult::Presentation(PresentationResult::AssertText {
                node_id: "a".into(),
                expected: "changed".into()
            })
        ),
        Step::Fault(_)
    ));
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn text_frames_and_next_request_or_scalar_commit_at_every_transaction_boundary() {
    for finishing in [false, true] {
        let writes: &[(&str, &str)] = if finishing {
            &[
                ("vm_effects", "UPDATE"),
                ("vm_dispatches", "UPDATE"),
                ("vm_merge_groups", "UPDATE"),
            ]
        } else {
            &[
                ("vm_effects", "UPDATE"),
                ("vm_dispatches", "UPDATE"),
                ("vm_effects", "INSERT"),
                ("vm_dispatches", "INSERT"),
                ("vm_merge_groups", "UPDATE"),
                ("vm_merge_branches", "INSERT"),
            ]
        };
        for (table, operation) in writes {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
            let first = effect(start(&mut vm, FLOW));
            let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
            let current = if finishing {
                effect(vm.resume_at(&second.continuation, 102, receipt(&second)))
            } else {
                second
            };
            let connection = Connection::open(&path.0).unwrap();
            let records = count(&connection);
            connection.execute_batch(&format!("CREATE TRIGGER reject_transition BEFORE {operation} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
            assert!(matches!(
                vm.resume_at(&current.continuation, 103, receipt(&current)),
                Step::Fault(_)
            ));
            assert_eq!(count(&connection), records);
            let state: String = connection
                .query_row(
                    "SELECT state FROM vm_effects WHERE token = ?1",
                    [current.continuation.token.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(state, "pending");
            drop(vm);
            connection
                .execute_batch("DROP TRIGGER reject_transition")
                .unwrap();
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            let mut step = vm.resume_at(&current.continuation, 104, receipt(&current));
            if !finishing {
                let next = effect(step);
                step = vm.resume_at(&next.continuation, 105, receipt(&next));
            }
            assert_eq!(step, done("alpha-value"));
        }
    }
}

#[test]
fn text_boundaries_preserve_empty_unicode_and_maximum_form_values_without_coercion() {
    for value in [
        String::new(),
        "quote\" slash\\ data".into(),
        "\u{754c}\u{9762}".into(),
        "x".repeat(leselang_hir::MAX_UI_FORM_FIELD_MAX_LENGTH),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100000).unwrap();
        let source = format!(
            r#"fn main() = bind(r: ui.set_form_value(node_id: "form", field: "name", value: {}), body: bind(s: ui.assert_form_value(node_id: "form", field: field(value: r, name: "field"), expected: field(value: r, name: "value")), body: field(value: s, name: "expected")))"#,
            serde_json::to_string(&value).unwrap()
        );
        let first = effect(start(&mut vm, &source));
        let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
        assert!(encode_continuation(&second.continuation).unwrap().len() <= 64 * 1024);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&second.continuation, 102, receipt(&second)),
            done(&value)
        );
    }
}

#[test]
fn maximum_verified_text_is_projected_but_cannot_bypass_a_narrower_host_argument_domain() {
    for narrowing in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100000).unwrap();
        let text = "x".repeat(leselang_hir::MAX_UI_EXPECTED_TEXT_BYTES);
        let tail = if narrowing {
            r#"bind(s: ui.set_form_value(node_id: "form", field: "name", value: field(value: r, name: "expected")), body: "done")"#
        } else {
            r#"bind(s: ui.wait_text(node_id: "a", expected: field(value: r, name: "expected")), body: field(value: s, name: "expected"))"#
        };
        let source = format!(
            r#"fn main() = bind(r: ui.assert_text(node_id: "a", expected: "{text}"), body: {tail})"#
        );
        let first = effect(start(&mut vm, &source));
        let mut terminal = vm.resume_at(&first.continuation, 101, receipt(&first));
        if narrowing {
            assert!(
                matches!(&terminal, Step::Fault(fault) if fault.code == "LSV1404"),
                "{terminal:?}"
            );
            assert_eq!(count(&Connection::open(&path.0).unwrap()), 1);
        } else {
            let second = effect(terminal);
            terminal = vm.resume_at(&second.continuation, 102, receipt(&second));
            assert_eq!(terminal, done(&text));
        }
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 103, receipt(&first)),
            terminal
        );
    }
}

#[test]
fn selected_large_group_text_aliases_hit_image_limits_without_phantom_work() {
    for exiting in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100000).unwrap();
        let text = "x".repeat(leselang_hir::MAX_UI_EXPECTED_TEXT_BYTES);
        let members = (0..8)
            .map(|index| format!(r#"m{index}: ui.assert_text(node_id: "a", expected: "{text}")"#))
            .collect::<Vec<_>>()
            .join(", ");
        let mut cold = r#"bind(s: ui.focus(node_id: "c"), body: "done")"#.to_string();
        for index in 0..8 {
            cold = format!("bind(alias{index}: g, body: {cold})");
        }
        let source = format!(
            r#"fn main() = bind(g: seq({members}), body: bind(r: ui.focus(node_id: "a"), body: choose(when: {exiting}, then: "done", otherwise: {cold})))"#
        );
        let prefix = effect(start(&mut vm, &source));
        let mut step = Step::Effect(Box::new(prefix.clone()));
        for _ in 0..8 {
            let current = effect(step);
            step = vm.resume_at(&current.continuation, 101, receipt(&current));
        }
        let current = effect(step);
        let terminal = vm.resume_at(&current.continuation, 103, receipt(&current));
        if exiting {
            assert_eq!(terminal, done("done"));
        } else {
            assert!(
                matches!(&terminal, Step::Fault(fault) if fault.code == "LSV3002"),
                "{terminal:?}"
            );
        }
        assert_eq!(count(&Connection::open(&path.0).unwrap()), 9);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&prefix.continuation, 104, receipt(&prefix)),
            terminal
        );
    }
}
