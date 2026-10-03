use leselang_hir::computation::{Computation, OptionalStringValue};
use leselang_hir::lower;
use leselang_hir::result_field::ResultField;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectOperation, EffectRequest, EffectResult, ScalarValue, Step, Value, Vm,
    decode_continuation, encode_continuation,
};
use rusqlite::{Connection, params};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-optional-projections-{}-{}",
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
        other => panic!("{other:?}"),
    }
}
fn done(value: ScalarValue) -> Step {
    Step::Done(Value::Scalar { value })
}
fn optional(value: Option<&str>) -> ScalarValue {
    ScalarValue::OptionalString(OptionalStringValue(value.map(str::to_owned)))
}
fn literal(value: Option<&str>) -> String {
    value.map_or_else(
        || "none".into(),
        |value| serde_json::to_string(value).unwrap(),
    )
}
fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    // These metadata fixtures confirm the concrete request, including nullable text and wait limits.
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
fn downgrade(result: &mut leselang_vm::ProjectedResult, version: u32) {
    let fields: &[ResultField] = match version {
        1 => &ResultField::V1,
        2 => &ResultField::V2,
        3 => &ResultField::V3,
        4 => &ResultField::V4,
        _ => panic!(),
    };
    result.projection_version = version;
    result.fields.retain(|field| fields.contains(&field.field));
}
fn flow(value: Option<&str>) -> String {
    format!(
        r#"fn defaulted(value: optional_string) = value_or(left: value, right: "default")
        fn main() = bind(first: ui.assert_form_field_placeholder(node_id: "form", field: "name", expected: {}), body:
            bind(alias: first, body: bind(second: ui.wait_form_field_placeholder(node_id: "form", field: field(value: alias, name: "field"), expected: field(value: alias, name: "optional_expected")), body:
                bind(third: ui.set_form_value(node_id: "form", field: "label", value: defaulted(value: field(value: second, name: "optional_expected"))), body:
                    eq(left: field(value: third, name: "value"), right: defaulted(value: field(value: first, name: "optional_expected")))))))"#,
        literal(value)
    )
}

#[test]
fn explicit_optional_wire_payloads_distinguish_none_empty_and_text_and_reject_missing_data() {
    for value in [
        None,
        Some(""),
        Some("none"),
        Some("null"),
        Some("quoted \" text"),
    ] {
        let scalar = optional(value);
        let wire = serde_json::to_value(&scalar).unwrap();
        assert_eq!(
            wire,
            serde_json::json!({"kind":"optional_string", "value": value})
        );
        assert_eq!(serde_json::from_value::<ScalarValue>(wire).unwrap(), scalar);
    }
    for wire in [
        r#"{"kind":"optional_string"}"#,
        r#"{"kind":"optional_string","value":false}"#,
        r#"{"kind":"optional_string","value":0}"#,
        r#"{"kind":"optional_string","value":[]}"#,
        r#"{"kind":"optional_string","value":{},"raw":true}"#,
        r#"{"kind":"optional_string","value":null,"raw":true}"#,
    ] {
        assert!(serde_json::from_str::<ScalarValue>(wire).is_err(), "{wire}");
    }
    assert!(
        serde_json::from_value::<ScalarValue>(
            serde_json::json!({"kind":"optional_string","value":"x".repeat(4097)})
        )
        .is_err()
    );
}

#[test]
fn pure_optional_values_helpers_loops_and_lazy_defaults_preserve_exact_semantics() {
    for (body, expected) in [
        ("optional_string(value: none)", optional(None)),
        (r#"optional_string(value: "")"#, optional(Some(""))),
        (
            r#"has_value(value: optional_string(value: ""))"#,
            ScalarValue::Boolean(true),
        ),
        (
            "has_value(value: optional_string(value: none))",
            ScalarValue::Boolean(false),
        ),
        (
            r#"eq(left: optional_string(value: none), right: optional_string(value: ""))"#,
            ScalarValue::Boolean(false),
        ),
        (
            r#"value_or(left: optional_string(value: ""), right: to_string(value: parse_integer(value: "bad")))"#,
            ScalarValue::String("".into()),
        ),
        (
            r#"value_or(left: optional_string(value: none), right: "default")"#,
            ScalarValue::String("default".into()),
        ),
        (
            r#"loop(value: optional_string(value: none), while: not(value: has_value(value: value)), next: optional_string(value: "ready"), limit: 1)"#,
            optional(Some("ready")),
        ),
    ] {
        assert_eq!(
            start(&mut Vm::new(1000), &format!("fn main() = {body}")),
            done(expected)
        );
    }
    assert!(
        matches!(start(&mut Vm::new(1000), r#"fn main() = value_or(left: optional_string(value: none), right: to_string(value: parse_integer(value: "bad")))"#), Step::Fault(fault) if fault.code == "LSV1408")
    );
}

#[test]
fn all_four_metadata_operations_export_none_empty_unicode_and_maximum_expected_text() {
    let maximum = "x".repeat(leselang_hir::MAX_UI_EXPECTED_TEXT_BYTES);
    for (operation, extra) in [
        ("assert_form_field_placeholder", r#"field: "name", "#),
        ("wait_form_field_placeholder", r#"field: "name", "#),
        ("assert_action_unavailable_reason", ""),
        ("wait_action_unavailable_reason", ""),
    ] {
        for value in [
            None,
            Some(""),
            Some("\u{754c}\u{9762}"),
            Some(maximum.as_str()),
        ] {
            let path = JournalPath::new();
            let source = format!(
                r#"fn main() = bind(r: ui.{operation}(node_id: "a", {extra}expected: {}), body: field(value: r, name: "optional_expected"))"#,
                literal(value)
            );
            for mut vm in [Vm::new(1000), Vm::open_journal(&path.0, 1000).unwrap()] {
                let request = effect(start(&mut vm, &source));
                assert_eq!(
                    vm.resume_at(&request.continuation, 101, receipt(&request)),
                    done(optional(value))
                );
                assert_eq!(
                    vm.resume_at(&request.continuation, 102, receipt(&request)),
                    done(optional(value))
                );
            }
        }
    }
}

#[test]
fn optional_projection_aliases_helpers_and_scalar_locals_survive_restarts_with_shared_budgets() {
    for value in [None, Some(""), Some("hint")] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(&mut vm, &flow(value)));
        let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
        let frames = &second.continuation.result_binding.as_ref().unwrap().results;
        assert_eq!(frames[0].result, frames[1].result);
        assert!(
            frames
                .iter()
                .all(|saved| saved.result.projection_version == 5)
        );
        assert_eq!(
            frames[0].result.fields.last().unwrap().value,
            optional(value)
        );
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 102, receipt(&first)),
            Step::Effect(Box::new(second.clone()))
        );
        let third = effect(vm.resume_at(&second.continuation, 103, receipt(&second)));
        assert!(third.continuation.fuel_remaining < second.continuation.fuel_remaining);
        for request in [&second, &third] {
            assert_eq!(request.continuation.expected_revision, Some(Revision(7)));
            assert_eq!(request.budget.deadline_at_ms, Some(200));
            assert_eq!(
                decode_continuation(&encode_continuation(&request.continuation).unwrap()).unwrap(),
                request.continuation
            );
        }
        assert_eq!(
            vm.resume_at(&third.continuation, 104, receipt(&third)),
            done(ScalarValue::Boolean(true))
        );
    }
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let first = effect(start(
        &mut vm,
        r#"fn main() = bind(r: ui.assert_action_unavailable_reason(node_id: "a", expected: none), body: bind(value: field(value: r, name: "optional_expected"), body: bind(s: ui.focus(node_id: "a"), body: value)))"#,
    ));
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    assert_eq!(
        second.continuation.result_binding.as_ref().unwrap().locals[0].value,
        optional(None)
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume_at(&second.continuation, 102, receipt(&second)),
        done(optional(None))
    );
}

#[test]
fn sequential_and_parallel_nullable_group_members_drive_presence_exits_and_successors() {
    for group in ["seq", "all"] {
        for value in [None, Some(""), Some("hint")] {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
            let source = format!(
                r#"fn main() = bind(g: {group}(a: ui.assert_action_unavailable_reason(node_id: "a", expected: {}), b: ui.assert_form_field_placeholder(node_id: "form", field: "name", expected: none)), body: bind(alias: g, body: choose(when: not(value: has_value(value: field(value: member(value: alias, name: "a"), name: "optional_expected"))), then: "missing", otherwise: bind(r: ui.wait_action_unavailable_reason(node_id: "a", expected: field(value: member(value: g, name: "a"), name: "optional_expected")), body: value_or(left: field(value: r, name: "optional_expected"), right: "default")))))"#,
                literal(value)
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
            drop(vm);
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            if value.is_some() {
                let request = effect(step);
                assert!(
                    request
                        .continuation
                        .result_binding
                        .as_ref()
                        .unwrap()
                        .groups
                        .iter()
                        .flat_map(|group| &group.group.members)
                        .all(|member| member.result.projection_version == 5)
                );
                step = vm.resume_at(&request.continuation, 103, receipt(&request));
            }
            assert_eq!(
                step,
                done(ScalarValue::String(value.unwrap_or("missing").into()))
            );
            assert_eq!(
                vm.resume_at(&first.continuation, 104, receipt(&first)),
                step
            );
        }
    }
}

#[test]
fn legacy_v1_through_v4_frames_remain_exact_and_do_not_synthesize_nullable_fields() {
    for version in 1..=4 {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(
            &mut vm,
            r#"fn main() = bind(first: ui.assert_form_field_placeholder(node_id: "form", field: "name", expected: none), body: bind(second: ui.wait_form_field_placeholder(node_id: field(value: first, name: "node_id"), field: "name", expected: optional_string(value: "hint")), body: bind(last: ui.focus(node_id: "form"), body: field(value: first, name: "node_id"))))"#,
        ));
        let mut second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
        downgrade(
            &mut second.continuation.result_binding.as_mut().unwrap().results[0].result,
            version,
        );
        let bytes = encode_continuation(&second.continuation).unwrap();
        assert_eq!(
            encode_continuation(&decode_continuation(&bytes).unwrap()).unwrap(),
            bytes
        );
        drop(vm);
        write_request(&Connection::open(&path.0).unwrap(), &second);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 102, receipt(&first)),
            Step::Effect(Box::new(second.clone()))
        );
        let third = effect(vm.resume_at(&second.continuation, 103, receipt(&second)));
        let frames = &third.continuation.result_binding.as_ref().unwrap().results;
        assert_eq!(frames[0].result.projection_version, version);
        assert_eq!(frames[1].result.projection_version, 5);
        assert!(
            !frames[0]
                .result
                .fields
                .iter()
                .any(|field| field.field == ResultField::OptionalExpected)
        );
        assert_eq!(
            vm.resume_at(&third.continuation, 104, receipt(&third)),
            done(ScalarValue::String("form".into()))
        );
    }
}

#[test]
fn closed_v5_frames_reject_missing_payload_wrong_types_domains_versions_and_layout() {
    let mut vm = Vm::new(10000);
    let first = effect(start(&mut vm, &flow(None)));
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    let wire = serde_json::to_value(&second.continuation).unwrap();
    for mutation in 0..11 {
        let mut wire = wire.clone();
        let frame = &mut wire["result_binding"]["results"][0]["result"];
        match mutation {
            0 => frame["projection_version"] = 6.into(),
            1 => frame["projection_version"] = 4.into(),
            2 => {
                frame["fields"].as_array_mut().unwrap().pop();
            }
            3 => frame["fields"][2] = frame["fields"][1].clone(),
            4 => frame["fields"].as_array_mut().unwrap().swap(1, 2),
            5 => frame["fields"][2]["value"] = serde_json::to_value(ScalarValue::None).unwrap(),
            6 => {
                frame["fields"][2]["value"]
                    .as_object_mut()
                    .unwrap()
                    .remove("value");
            }
            7 => frame["fields"][2]["value"]["value"] = true.into(),
            8 => frame["fields"][2]["value"]["value"] = "x".repeat(1025).into(),
            9 => frame["fields"][2]["value"]["value"] = "bad\ntext".into(),
            10 => frame["raw"] = serde_json::json!({"expected":null}),
            _ => unreachable!(),
        }
        assert!(
            decode_continuation(&serde_json::to_vec(&wire).unwrap()).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn missing_legacy_nullable_fields_are_rejected_on_cold_paths_and_conditional_aliases() {
    let mut vm = Vm::new(10000);
    let first = effect(start(&mut vm, &flow(None)));
    let mut second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    let binding = second.continuation.result_binding.as_mut().unwrap();
    downgrade(&mut binding.results[1].result, 4);
    let local = |name: &str| Box::new(Computation::Local { name: name.into() });
    binding.body = Computation::Bind {
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
                value: optional(None),
            }),
            otherwise: Box::new(Computation::Field {
                value: local("chosen"),
                field: ResultField::OptionalExpected,
            }),
        }),
    };
    assert_eq!(
        encode_continuation(&second.continuation).unwrap_err().code,
        "LSV1405"
    );
}

#[test]
fn forged_none_empty_or_text_is_rejected_against_committed_atomic_and_group_receipts() {
    for grouped in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let source = if grouped {
            r#"fn main() = bind(g: seq(a: ui.assert_action_unavailable_reason(node_id: "a", expected: none)), body: bind(r: ui.focus(node_id: "a"), body: field(value: member(value: g, name: "a"), name: "optional_expected")))"#
        } else {
            r#"fn main() = bind(r: ui.assert_action_unavailable_reason(node_id: "a", expected: none), body: bind(s: ui.focus(node_id: "a"), body: field(value: r, name: "optional_expected")))"#
        };
        let first = effect(start(&mut vm, source));
        let mut second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
        let binding = second.continuation.result_binding.as_mut().unwrap();
        let frame = if grouped {
            &mut binding.groups[0].group.members[0].result
        } else {
            &mut binding.results[0].result
        };
        frame
            .fields
            .iter_mut()
            .find(|field| field.field == ResultField::OptionalExpected)
            .unwrap()
            .value = optional(Some(""));
        assert!(encode_continuation(&second.continuation).is_ok());
        drop(vm);
        write_request(&Connection::open(&path.0).unwrap(), &second);
        assert!(Vm::open_journal(&path.0, 1).is_err());
    }
}

#[test]
fn wrong_nullable_acknowledgements_create_no_successor_and_replay_the_committed_fault() {
    for value in [None, Some(""), Some("hint")] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(&mut vm, &flow(value)));
        let EffectResult::Presentation(result) = receipt(&first) else {
            panic!()
        };
        let mut result = serde_json::to_value(result).unwrap();
        result["expected"] = serde_json::json!(if value.is_none() { Some("") } else { None });
        let terminal = vm.resume_at(
            &first.continuation,
            101,
            EffectResult::Presentation(serde_json::from_value(result).unwrap()),
        );
        assert!(matches!(&terminal, Step::Fault(_)));
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(count(&Connection::open(&path.0).unwrap()), 1);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 102, receipt(&first)),
            terminal
        );
    }
}

#[test]
fn nullable_frames_and_next_request_or_scalar_commit_at_every_transaction_boundary() {
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
            let first = effect(start(&mut vm, &flow(None)));
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
            assert_eq!(step, done(ScalarValue::Boolean(true)));
        }
    }
}

#[test]
fn optional_strings_cannot_bypass_scalar_host_or_image_bounds() {
    let maximum = "x".repeat(4096);
    assert_eq!(
        start(
            &mut Vm::new(1000),
            &format!(
                "fn main() = optional_string(value: {})",
                literal(Some(&maximum))
            )
        ),
        done(optional(Some(&maximum)))
    );
    assert!(
        lower(&parse(&format!(
            "fn main() = optional_string(value: {})",
            literal(Some(&"x".repeat(4097)))
        )))
        .is_err()
    );
    let text = "x".repeat(1024);
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let first = effect(start(
        &mut vm,
        &format!(
            r#"fn main() = bind(r: ui.assert_action_unavailable_reason(node_id: "a", expected: "{text}"), body: ui.set_form_value(node_id: "form", field: "name", value: value_or(left: field(value: r, name: "optional_expected"), right: "default")))"#
        ),
    ));
    assert!(
        matches!(vm.resume_at(&first.continuation, 101, receipt(&first)), Step::Fault(fault) if fault.code == "LSV1404")
    );
    assert_eq!(count(&Connection::open(&path.0).unwrap()), 1);
    for exiting in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 100000).unwrap();
        let members = (0..8).map(|index| format!(r#"m{index}: ui.assert_form_field_placeholder(node_id: "form", field: "name", expected: "{text}")"#)).collect::<Vec<_>>().join(", ");
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
            assert_eq!(terminal, done(ScalarValue::String("done".into())));
        } else {
            assert!(
                matches!(&terminal, Step::Fault(fault) if fault.code == "LSV3002"),
                "{terminal:?}"
            );
        }
        assert_eq!(count(&Connection::open(&path.0).unwrap()), 9);
    }
}

#[test]
fn optional_restoration_does_not_refill_fuel_or_extend_deadline() {
    let mut vm = Vm::new(10000);
    let first = effect(start(&mut vm, &flow(None)));
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    let mut exhausted = second.clone();
    exhausted.continuation.fuel_remaining = 1;
    exhausted.budget.fuel_remaining = 1;
    let mut restored = Vm::new(1_000_000);
    restored.restore_request(exhausted.clone()).unwrap();
    assert!(
        matches!(restored.resume_at(&exhausted.continuation, 102, receipt(&exhausted)), Step::Fault(fault) if fault.code == "LSV1001")
    );
    let mut restored = Vm::new(1_000_000);
    restored.restore_request(second.clone()).unwrap();
    assert!(matches!(
        restored.resume_at(&second.continuation, 200, receipt(&second)),
        Step::Cancelled(_)
    ));
}
