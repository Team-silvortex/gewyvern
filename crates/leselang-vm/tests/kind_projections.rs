use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::computation::Computation;
use leselang_hir::lower;
use leselang_hir::result_field::ResultField;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectOperation, EffectRequest, EffectResult, ScalarValue, Step, Value, Vm,
    decode_continuation, encode_continuation,
};
use rusqlite::{Connection, params};

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-kind-projections-{}-{}",
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
fn done(token: &str) -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::String(token.into()),
    })
}
fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    // Assertion/wait fixtures acknowledge the concrete request, including its wait limits.
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
        _ => panic!(),
    };
    result.projection_version = version;
    result.fields.retain(|field| fields.contains(&field.field));
}

const FLOW: &str = r#"fn identity(token: string) = token
fn main() = bind(first: ui.assert_form_field_input_kind(node_id: "form", field: "name", kind: "path_token"), body:
    bind(alias: first, body:
        bind(second: ui.wait_form_field_input_kind(node_id: field(value: first, name: "node_id"), field: field(value: alias, name: "field"), kind: identity(token: field(value: alias, name: "kind"))), body:
            bind(third: ui.set_form_value(node_id: "form", field: "label", value: field(value: second, name: "kind")), body:
                choose(when: eq(left: field(value: third, name: "value"), right: "path_token"), then: field(value: first, name: "kind"), otherwise: "mismatch")))))"#;

#[test]
fn all_canonical_tokens_project_from_both_assert_and_wait_receipts_in_both_journals() {
    let cases: &[(&str, &[&str])] = &[
        (
            "node_kind",
            &[
                "column",
                "heading",
                "text",
                "runtime_card",
                "runtime_workspace",
                "section",
                "history_entry",
                "log_entry",
                "debugger_workspace",
                "debugger_frame",
                "action",
            ],
        ),
        (
            "action_kind",
            &[
                "runtime_inspect",
                "runtime_refresh",
                "runtime_capabilities_refresh",
                "runtime_deploy",
                "debugger_cancel",
            ],
        ),
        ("form_field_input_kind", &["path_token", "trimmed_text"]),
    ];
    for (kind, tokens) in cases {
        for prefix in ["assert", "wait"] {
            for token in *tokens {
                let extra = if *kind == "form_field_input_kind" {
                    r#"field: "name", "#
                } else {
                    ""
                };
                let source = format!(
                    r#"fn main() = bind(r: ui.{prefix}_{kind}(node_id: "a", {extra}kind: "{token}"), body: field(value: r, name: "kind"))"#
                );
                let path = JournalPath::new();
                for mut vm in [Vm::new(1000), Vm::open_journal(&path.0, 1000).unwrap()] {
                    let request = effect(start(&mut vm, &source));
                    assert_eq!(
                        vm.resume_at(&request.continuation, 101, receipt(&request)),
                        done(token)
                    );
                    assert_eq!(
                        vm.resume_at(&request.continuation, 102, receipt(&request)),
                        done(token)
                    );
                    assert_eq!(vm.pending_count(), 0);
                }
            }
        }
    }
}

#[test]
fn high_bit_kind_aliases_helpers_and_text_frames_survive_restart_with_original_budgets() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let first = effect(start(&mut vm, FLOW));
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    let frames = &second.continuation.result_binding.as_ref().unwrap().results;
    assert_eq!(frames[0].result, frames[1].result);
    assert_eq!(frames[0].result.projection_version, 4);
    assert_eq!(
        frames[0]
            .result
            .fields
            .iter()
            .map(|field| field.field)
            .collect::<Vec<_>>(),
        [ResultField::NodeId, ResultField::Field, ResultField::Kind]
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.resume_at(&first.continuation, 102, receipt(&first)),
        Step::Effect(Box::new(second.clone()))
    );
    let third = effect(vm.resume_at(&second.continuation, 103, receipt(&second)));
    let EffectOperation::Presentation(original) = &first.operation else {
        panic!()
    };
    for request in [&second, &third] {
        assert_eq!(request.continuation.expected_revision, Some(Revision(7)));
        let EffectOperation::Presentation(envelope) = &request.operation else {
            panic!()
        };
        assert_eq!(envelope.principal, original.principal);
        assert_eq!(envelope.capabilities, original.capabilities);
        assert_eq!(request.budget.deadline_at_ms, Some(200));
        assert_eq!(
            decode_continuation(&encode_continuation(&request.continuation).unwrap()).unwrap(),
            request.continuation
        );
    }
    assert!(third.continuation.fuel_remaining < second.continuation.fuel_remaining);
    assert_eq!(
        vm.resume_at(&third.continuation, 104, receipt(&third)),
        done("path_token")
    );
    assert_eq!(
        vm.resume_at(&first.continuation, 105, receipt(&first)),
        done("path_token")
    );
}

#[test]
fn sequential_and_parallel_group_kind_frames_drive_early_exits_or_captured_successors() {
    for group in ["seq", "all"] {
        for exiting in [false, true] {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
            let source = format!(
                r#"fn main() = bind(g: {group}(node: ui.assert_node_kind(node_id: "a", kind: "heading"), input: ui.assert_form_field_input_kind(node_id: "form", field: "name", kind: "path_token")), body:
                bind(alias: g, body: choose(when: {exiting}, then: field(value: member(value: alias, name: "node"), name: "kind"), otherwise:
                    bind(r: ui.wait_form_field_input_kind(node_id: "form", field: field(value: member(value: g, name: "input"), name: "field"), kind: field(value: member(value: alias, name: "input"), name: "kind")), body:
                        choose(when: eq(left: field(value: r, name: "kind"), right: "path_token"), then: field(value: member(value: g, name: "node"), name: "kind"), otherwise: "mismatch")))))"#
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
                        .all(|member| member.result.projection_version == 4)
                );
                step = vm.resume_at(&current.continuation, 103, receipt(&current));
            }
            assert_eq!(step, done("heading"));
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
fn legacy_v1_v2_v3_frames_stay_exact_and_mix_with_new_v4_and_v3_results() {
    for version in [1, 2, 3] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(
            &mut vm,
            r#"fn main() = bind(first: ui.assert_form_field_input_kind(node_id: "form", field: "name", kind: "path_token"), body:
            bind(alias: first, body: bind(second: ui.wait_node_kind(node_id: field(value: alias, name: "node_id"), kind: "heading"), body:
                bind(third: ui.set_form_value(node_id: "form", field: "label", value: field(value: second, name: "kind")), body:
                    bind(last: ui.focus(node_id: "form"), body: field(value: first, name: "node_id"))))))"#,
        ));
        let mut second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
        for saved in &mut second.continuation.result_binding.as_mut().unwrap().results {
            downgrade(&mut saved.result, version);
        }
        let bytes = encode_continuation(&second.continuation).unwrap();
        assert_eq!(
            encode_continuation(&decode_continuation(&bytes).unwrap()).unwrap(),
            bytes
        );
        if version == 1 {
            assert!(
                !String::from_utf8(bytes)
                    .unwrap()
                    .contains("projection_version")
            );
        }
        drop(vm);
        write_request(&Connection::open(&path.0).unwrap(), &second);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 102, receipt(&first)),
            Step::Effect(Box::new(second.clone()))
        );
        let third = effect(vm.resume_at(&second.continuation, 103, receipt(&second)));
        let fourth = effect(vm.resume_at(&third.continuation, 104, receipt(&third)));
        let frames = &fourth.continuation.result_binding.as_ref().unwrap().results;
        assert_eq!(
            frames
                .iter()
                .map(|frame| frame.result.projection_version)
                .collect::<Vec<_>>(),
            [version, version, 4, 3]
        );
        assert!(frames[..2].iter().all(|saved| {
            !saved
                .result
                .fields
                .iter()
                .any(|field| field.field == ResultField::Kind)
        }));
        assert_eq!(
            vm.resume_at(&fourth.continuation, 105, receipt(&fourth)),
            done("form")
        );
    }
}

#[test]
fn closed_v4_frames_reject_invalid_tokens_types_versions_and_field_layouts() {
    let mut vm = Vm::new(10000);
    let first = effect(start(
        &mut vm,
        r#"fn main() = bind(r: ui.assert_form_field_input_kind(node_id: "form", field: "name", kind: "path_token"), body: bind(s: ui.focus(node_id: "a"), body: field(value: r, name: "kind")))"#,
    ));
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    let wire = serde_json::to_value(&second.continuation).unwrap();
    for mutation in 0..11 {
        let mut wire = wire.clone();
        let frame = &mut wire["result_binding"]["results"][0]["result"];
        match mutation {
            0 => frame["projection_version"] = 6.into(),
            1 => frame["projection_version"] = 3.into(),
            2 => {
                frame["fields"].as_array_mut().unwrap().pop();
            }
            3 => frame["fields"][2] = frame["fields"][0].clone(),
            4 => frame["fields"].as_array_mut().unwrap().swap(1, 2),
            5 => {
                frame["fields"][2]["value"] =
                    serde_json::to_value(ScalarValue::Boolean(true)).unwrap()
            }
            6..=9 => {
                frame["fields"][2]["value"] = serde_json::to_value(ScalarValue::String(
                    ["heading", "PathToken", "path_token ", ""][mutation - 6].into(),
                ))
                .unwrap()
            }
            10 => frame["raw"] = serde_json::json!({"kind":"path_token"}),
            _ => unreachable!(),
        }
        assert!(
            decode_continuation(&serde_json::to_vec(&wire).unwrap()).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn legacy_kind_absence_is_fenced_in_conditional_aliases_and_cold_group_members() {
    let mut vm = Vm::new(10000);
    let first = effect(start(
        &mut vm,
        r#"fn main() = bind(first: ui.assert_node_kind(node_id: "a", kind: "heading"), body: bind(alias: first, body: bind(next: ui.focus(node_id: "a"), body: "done")))"#,
    ));
    let mut next = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    let binding = next.continuation.result_binding.as_mut().unwrap();
    downgrade(&mut binding.results[1].result, 3);
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
                value: ScalarValue::String("done".into()),
            }),
            otherwise: Box::new(Computation::Field {
                value: local("chosen"),
                field: ResultField::Kind,
            }),
        }),
    };
    assert_eq!(
        encode_continuation(&next.continuation).unwrap_err().code,
        "LSV1405"
    );
    let prefix = effect(start(
        &mut vm,
        r#"fn main() = bind(g: seq(a: ui.assert_node_kind(node_id: "a", kind: "heading")), body: bind(alias: g, body: bind(r: ui.focus(node_id: "a"), body: choose(when: true, then: "done", otherwise: field(value: member(value: alias, name: "a"), name: "kind")))))"#,
    ));
    let mut next = effect(vm.resume_at(&prefix.continuation, 102, receipt(&prefix)));
    for group in &mut next.continuation.result_binding.as_mut().unwrap().groups {
        for member in &mut group.group.members {
            downgrade(&mut member.result, 3);
        }
    }
    assert_eq!(
        encode_continuation(&next.continuation).unwrap_err().code,
        "LSV1405"
    );
}

#[test]
fn same_domain_forged_kind_frames_are_rejected_against_committed_raw_receipts() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let first = effect(start(
        &mut vm,
        r#"fn main() = bind(r: ui.assert_node_kind(node_id: "a", kind: "heading"), body: bind(s: ui.focus(node_id: "a"), body: field(value: r, name: "kind")))"#,
    ));
    let mut second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    second.continuation.result_binding.as_mut().unwrap().results[0]
        .result
        .fields
        .iter_mut()
        .find(|field| field.field == ResultField::Kind)
        .unwrap()
        .value = ScalarValue::String("section".into());
    assert!(encode_continuation(&second.continuation).is_ok());
    drop(vm);
    write_request(&Connection::open(&path.0).unwrap(), &second);
    assert!(Vm::open_journal(&path.0, 1).is_err());
}

#[test]
fn cross_domain_kind_arguments_and_mismatched_receipts_create_no_successor() {
    for (first_call, next_call) in [
        (
            r#"ui.assert_node_kind(node_id: "a", kind: "heading")"#,
            r#"ui.wait_action_kind(node_id: "a", kind: field(value: r, name: "kind"))"#,
        ),
        (
            r#"ui.assert_action_kind(node_id: "a", kind: "runtime_refresh")"#,
            r#"ui.wait_form_field_input_kind(node_id: "form", field: "name", kind: field(value: r, name: "kind"))"#,
        ),
        (
            r#"ui.assert_form_field_input_kind(node_id: "form", field: "name", kind: "path_token")"#,
            r#"ui.wait_node_kind(node_id: "a", kind: field(value: r, name: "kind"))"#,
        ),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(
            &mut vm,
            &format!(r#"fn main() = bind(r: {first_call}, body: {next_call})"#),
        ));
        let fault = vm.resume_at(&first.continuation, 101, receipt(&first));
        assert!(matches!(&fault, Step::Fault(fault) if fault.code == "LSV1404"));
        assert_eq!(count(&Connection::open(&path.0).unwrap()), 1);
        assert_eq!(vm.pending_count(), 0);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 102, receipt(&first)),
            fault
        );
    }
    for mutation in ["node_id", "expected_kind"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(
            &mut vm,
            r#"fn main() = bind(r: ui.assert_node_kind(node_id: "a", kind: "heading"), body: ui.focus(node_id: field(value: r, name: "kind")))"#,
        ));
        let EffectResult::Presentation(result) = receipt(&first) else {
            panic!()
        };
        let mut result = serde_json::to_value(result).unwrap();
        result[mutation] = if mutation == "node_id" {
            "wrong"
        } else {
            "section"
        }
        .into();
        let fault = vm.resume_at(
            &first.continuation,
            101,
            EffectResult::Presentation(serde_json::from_value(result).unwrap()),
        );
        assert!(matches!(&fault, Step::Fault(_)));
        assert_eq!(count(&Connection::open(&path.0).unwrap()), 1);
        assert_eq!(vm.pending_count(), 0);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 102, receipt(&first)),
            fault
        );
    }
}

#[test]
fn kind_frames_and_next_request_or_scalar_commit_atomically_at_each_write_boundary() {
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
            assert_eq!(step, done("path_token"));
        }
    }
}

#[test]
fn restoring_kind_frames_does_not_refill_shared_fuel_or_grant_missing_authority() {
    let program = lower(&parse(FLOW)).unwrap();
    let mut vm = Vm::new(10000);
    assert!(matches!(
        vm.start(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::default(),
            None
        ),
        Step::Fault(_)
    ));
    assert_eq!(vm.pending_count(), 0);
    let first = effect(start(&mut vm, FLOW));
    let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    for mut request in [first, second] {
        request.continuation.fuel_remaining = 1;
        request.budget.fuel_remaining = 1;
        let mut restored = Vm::new(1_000_000);
        restored.restore_request(request.clone()).unwrap();
        assert!(
            matches!(restored.resume_at(&request.continuation, 102, receipt(&request)),
            Step::Fault(fault) if fault.code == "LSV1001")
        );
        assert_eq!(restored.pending_count(), 0);
    }
}

#[test]
fn kind_chains_retain_absolute_deadline_and_current_effect_cancellation() {
    for cancel in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(&mut vm, FLOW));
        let second = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
        let terminal = if cancel {
            vm.cancel_effect(&second.continuation, 102)
        } else {
            vm.resume_at(&second.continuation, 200, receipt(&second))
        };
        assert!(matches!(&terminal, Step::Fault(_) | Step::Cancelled(_)));
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(count(&Connection::open(&path.0).unwrap()), 2);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 201, receipt(&first)),
            terminal
        );
        assert!(vm.claim_effect(202, 100).unwrap().is_none());
    }
}
