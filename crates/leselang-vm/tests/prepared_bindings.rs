use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{Effect, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectOperation, EffectRequest, EffectResult, PresentationResult, ScalarValue, Step, Value, Vm,
    decode_continuation, encode_continuation,
};
use rusqlite::Connection;

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-prepared-binding-{}-{}",
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
fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}
fn done(value: bool) -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::Boolean(value),
    })
}
fn rows(path: &JournalPath, table: &str) -> i64 {
    Connection::open(&path.0)
        .unwrap()
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}
fn chain(ready: bool) -> String {
    let expected = if ready { "ready" } else { "skip" };
    format!(
        r#"fn main() = bind(first: ui.assert_text(node_id: "status", expected: "{expected}"), body:
        bind(selected: bind(target: concat(left: "node-", right: field(value: first, name: "expected")), body:
            choose(when: starts_with(left: target, right: "node-ready"), then: ui.focus(node_id: target), otherwise: ui.focus(node_id: "fallback"))), body:
                choose(when: eq(left: field(value: selected, name: "node_id"), right: "fallback"), then: false, otherwise:
                    bind(written: ui.set_form_value(node_id: "form", field: "target", value: field(value: selected, name: "node_id")), body: true))))"#
    )
}

#[test]
fn prepared_atomic_capture_saves_resolved_effect_not_its_preparation_scope() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
    let source = r#"fn focus(node: string, unused: integer) = ui.focus(node_id: node)
        fn main() = bind(prefix: "outer", body: bind(result: bind(target: concat(left: "node-", right: "a"), body:
            choose(when: true, then: focus(node: target, unused: 1), otherwise: focus(node: "cold", unused: div(left: 1, right: 0)))), body: concat(left: prefix, right: field(value: result, name: "node_id"))))"#;
    let first = effect(start(&mut vm, source));
    assert_eq!(first.continuation.schema_version, 2);
    assert!(
        matches!(&first.continuation.pending_effect, Effect::UiFocus { node_id } if node_id == "node-a")
    );
    let locals = &first.continuation.result_binding.as_ref().unwrap().locals;
    assert_eq!(locals.len(), 1);
    assert_eq!(locals[0].name, "prefix");
    let wire = encode_continuation(&first.continuation).unwrap();
    assert_eq!(decode_continuation(&wire).unwrap(), first.continuation);
    let text = String::from_utf8(wire).unwrap();
    assert!(
        !text.contains("choose")
            && !text.contains("div")
            && !text.contains("cold")
            && !text.contains("_lf")
    );
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let terminal = Step::Done(Value::Scalar {
        value: ScalarValue::String("outernode-a".into()),
    });
    assert_eq!(
        vm.resume_at(&first.continuation, 101, receipt(&first)),
        terminal
    );
    assert_eq!(
        vm.resume_at(&first.continuation, 102, receipt(&first)),
        terminal
    );
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn result_driven_prepared_captures_restart_with_exact_prior_frames_and_early_exits() {
    for ready in [false, true] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let first = effect(start(&mut vm, &chain(ready)));
        assert_eq!(first.continuation.schema_version, 5);
        assert_eq!(
            decode_continuation(&encode_continuation(&first.continuation).unwrap()).unwrap(),
            first.continuation
        );
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        let selected = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
        assert_eq!(selected.continuation.schema_version, 5);
        assert_eq!(
            selected
                .continuation
                .result_binding
                .as_ref()
                .unwrap()
                .results
                .len(),
            1
        );
        assert!(
            selected
                .continuation
                .result_binding
                .as_ref()
                .unwrap()
                .locals
                .iter()
                .all(|local| local.name != "target")
        );
        assert!(
            matches!(&selected.continuation.pending_effect, Effect::UiFocus { node_id } if node_id == if ready { "node-ready" } else { "fallback" })
        );
        assert!(selected.budget.fuel_remaining < first.budget.fuel_remaining);
        assert_eq!(selected.continuation.expected_revision, Some(Revision(7)));
        assert_eq!(selected.continuation.deadline_at_ms, Some(200));
        assert_eq!(
            decode_continuation(&encode_continuation(&selected.continuation).unwrap()).unwrap(),
            selected.continuation
        );
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        let step = vm.resume_at(&selected.continuation, 102, receipt(&selected));
        if ready {
            let written = effect(step);
            assert_eq!(
                written
                    .continuation
                    .result_binding
                    .as_ref()
                    .unwrap()
                    .results
                    .len(),
                2
            );
            assert!(
                matches!(&written.continuation.pending_effect, Effect::UiSetFormValue { value, .. } if value == "node-ready")
            );
            assert!(written.budget.fuel_remaining < selected.budget.fuel_remaining);
            assert_eq!(
                vm.resume_at(&written.continuation, 103, receipt(&written)),
                done(true)
            );
        } else {
            assert_eq!(step, done(false));
            assert_eq!(rows(&path, "vm_effects"), 2);
        }
        assert_eq!(
            vm.resume_at(&first.continuation, 104, receipt(&first)),
            done(ready)
        );
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn group_owned_prepared_captures_keep_reserved_slots_and_parallel_success_barriers() {
    for kind in ["seq", "all"] {
        for conditional in [false, true] {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
            let body = if conditional {
                r#"choose(when: false, then: false, otherwise: bind(written: ui.set_form_value(node_id: "form", field: "target", value: field(value: selected, name: "node_id")), body: true))"#
            } else {
                "true"
            };
            let source = format!(
                r#"fn main() = bind(group: {kind}(first: ui.assert_text(node_id: "a", expected: "ready"), second: ui.focus(node_id: "b")), body:
                bind(selected: bind(target: concat(left: "node-", right: field(value: member(value: group, name: "first"), name: "expected")), body:
                    choose(when: true, then: ui.focus(node_id: target), otherwise: ui.focus(node_id: "cold"))), body: {body}))"#
            );
            let (first, last) = match start(&mut vm, &source) {
                Step::Effect(first) => {
                    let last = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
                    (*first, last)
                }
                Step::Effects(batch) => {
                    let last = batch.branches[0].request.clone();
                    let first = batch.branches[1].request.clone();
                    assert!(matches!(
                        vm.resume_at(&first.continuation, 101, receipt(&first)),
                        Step::Waiting(_)
                    ));
                    (first, last)
                }
                other => panic!("{other:?}"),
            };
            assert_eq!(rows(&path, "vm_effects"), 2);
            assert_eq!(
                first.continuation.schema_version,
                if conditional {
                    11
                } else if kind == "seq" {
                    8
                } else {
                    9
                }
            );
            drop(vm);
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            let selected = effect(vm.resume_at(&last.continuation, 102, receipt(&last)));
            assert!(
                matches!(&selected.continuation.pending_effect, Effect::UiFocus { node_id } if node_id == "node-ready")
            );
            assert_eq!(
                selected
                    .continuation
                    .result_binding
                    .as_ref()
                    .unwrap()
                    .groups
                    .len(),
                1
            );
            assert_eq!(
                decode_continuation(&encode_continuation(&selected.continuation).unwrap()).unwrap(),
                selected.continuation
            );
            let step = vm.resume_at(&selected.continuation, 103, receipt(&selected));
            if conditional {
                let written = effect(step);
                assert_eq!(
                    vm.resume_at(&written.continuation, 104, receipt(&written)),
                    done(true)
                );
            } else {
                assert_eq!(step, done(true));
            }
            assert_eq!(
                vm.resume_at(&first.continuation, 105, receipt(&first)),
                done(true)
            );
            assert_eq!(vm.pending_count(), 0);
        }
    }
}

#[test]
fn atomic_preparation_faults_do_not_allocate_initial_effects_or_escape_host_domains() {
    for (fuel, value, code) in [
        (
            10000,
            r#"bind(target: to_string(value: div(left: 1, right: 0)), body: ui.focus(node_id: target))"#,
            "LSV1401",
        ),
        (
            10000,
            r#"choose(when: eq(left: div(left: 1, right: 0), right: 0), then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: "b"))"#,
            "LSV1401",
        ),
        (
            10000,
            r#"bind(target: concat(left: "bad", right: " node"), body: ui.focus(node_id: target))"#,
            "LSV1404",
        ),
        (
            1,
            r#"choose(when: true, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: "b"))"#,
            "LSV1001",
        ),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, fuel).unwrap();
        let step = start(
            &mut vm,
            &format!("fn main() = bind(result: {value}, body: true)"),
        );
        assert!(
            matches!(&step, Step::Fault(fault) if fault.code == code),
            "{step:?}"
        );
        assert_eq!(rows(&path, "vm_effects"), 0);
        assert_eq!(vm.pending_count(), 0);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        assert_eq!(
            effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#)).effect_id,
            "effect-1"
        );
    }
}

#[test]
fn receipt_successor_and_terminal_sql_boundaries_roll_back_without_partial_progress() {
    for phase in 0..3 {
        let writes: &[(&str, &str)] = if phase == 2 {
            &[("vm_effects", "UPDATE"), ("vm_dispatches", "UPDATE")]
        } else {
            &[
                ("vm_effects", "UPDATE"),
                ("vm_dispatches", "UPDATE"),
                ("vm_effects", "INSERT"),
                ("vm_dispatches", "INSERT"),
            ]
        };
        for (table, operation) in writes {
            let path = JournalPath::new();
            let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
            let first = effect(start(&mut vm, &chain(true)));
            let mut current = first.clone();
            for index in 0..phase {
                current =
                    effect(vm.resume_at(&current.continuation, 101 + index, receipt(&current)));
            }
            let before = rows(&path, "vm_effects");
            let connection = Connection::open(&path.0).unwrap();
            connection.execute_batch(&format!("CREATE TRIGGER reject_transition BEFORE {operation} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
            assert!(matches!(
                vm.resume_at(&current.continuation, 105, receipt(&current)),
                Step::Fault(_)
            ));
            assert_eq!(rows(&path, "vm_effects"), before);
            assert_eq!(
                vm.pending_continuations(),
                std::slice::from_ref(&current.continuation)
            );
            let state: String = connection
                .query_row(
                    "SELECT state FROM vm_effects WHERE token = ?1",
                    [current.continuation.token.as_str()],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(state, "pending");
            connection
                .execute_batch("DROP TRIGGER reject_transition")
                .unwrap();
            drop(vm);
            let mut vm = Vm::open_journal(&path.0, 1).unwrap();
            let mut step = vm.resume_at(&current.continuation, 106, receipt(&current));
            for time in 107..110 {
                let Step::Effect(request) = step else { break };
                step = vm.resume_at(&request.continuation, time, receipt(&request));
            }
            assert_eq!(step, done(true));
            assert_eq!(
                vm.resume_at(&first.continuation, 110, receipt(&first)),
                done(true)
            );
        }
    }
}

#[test]
fn prepared_capture_cold_paths_cannot_synthesize_fields_missing_from_legacy_frames() {
    let source = r#"fn main() = bind(seed: ui.assert_text(node_id: "a", expected: "ready"), body:
        bind(current: ui.focus(node_id: "b"), body: bind(next: choose(when: true, then: ui.focus(node_id: "c"), otherwise: ui.focus(node_id: field(value: seed, name: "expected"))), body: true)))"#;
    let mut vm = Vm::new(10000);
    let first = effect(start(&mut vm, source));
    let current = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    let mut wire = serde_json::to_value(&current.continuation).unwrap();
    let frame = &mut wire["result_binding"]["results"][0]["result"];
    frame.as_object_mut().unwrap().remove("projection_version");
    frame["fields"]
        .as_array_mut()
        .unwrap()
        .retain(|field| field["field"] != "expected");
    assert!(decode_continuation(&serde_json::to_vec(&wire).unwrap()).is_err());
    let safe = source.replace("field(value: seed, name: \"expected\")", "\"c\"");
    let mut vm = Vm::new(10000);
    let first = effect(start(&mut vm, &safe));
    let current = effect(vm.resume_at(&first.continuation, 101, receipt(&first)));
    let mut wire = serde_json::to_value(&current.continuation).unwrap();
    let frame = &mut wire["result_binding"]["results"][0]["result"];
    frame.as_object_mut().unwrap().remove("projection_version");
    frame["fields"]
        .as_array_mut()
        .unwrap()
        .retain(|field| field["field"] != "expected");
    let image = decode_continuation(&serde_json::to_vec(&wire).unwrap()).unwrap();
    assert_eq!(
        image.result_binding.as_ref().unwrap().results[0]
            .result
            .projection_version,
        1
    );
    assert!(
        image.result_binding.as_ref().unwrap().results[0]
            .result
            .fields
            .iter()
            .all(|field| field.field != leselang_hir::result_field::ResultField::Expected)
    );
    let mut forged = wire;
    forged["result_binding"]["body"]["value"]["otherwise"]["effect"] =
        serde_json::to_value(Effect::UiActivate {
            node_id: "c".into(),
        })
        .unwrap();
    assert!(decode_continuation(&serde_json::to_vec(&forged).unwrap()).is_err());
}

#[test]
fn prepared_result_captures_keep_deadline_cancel_and_receipt_correlation_fences() {
    for mode in ["cancel", "deadline", "wrong-receipt"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 10000).unwrap();
        let source = r#"fn main() = bind(result: choose(when: true, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: "b")), body: bind(next: ui.focus(node_id: field(value: result, name: "node_id")), body: true))"#;
        let first = effect(start(&mut vm, source));
        let terminal = match mode {
            "cancel" => vm.cancel_effect(&first.continuation, 101),
            "deadline" => vm.resume_at(&first.continuation, 200, receipt(&first)),
            _ => vm.resume_at(
                &first.continuation,
                101,
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "wrong".into(),
                }),
            ),
        };
        assert!(matches!(&terminal, Step::Cancelled(_) | Step::Fault(_)));
        assert_eq!(rows(&path, "vm_effects"), 1);
        assert_eq!(vm.pending_count(), 0);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 201, receipt(&first)),
            terminal
        );
    }
}

#[test]
fn simultaneous_workers_commit_one_prepared_capture_successor() {
    for _ in 0..3 {
        let path = JournalPath::new();
        let mut left = Vm::open_journal(&path.0, 10000).unwrap();
        let source = r#"fn main() = bind(seed: ui.assert_text(node_id: "status", expected: "ready"), body:
            bind(selected: choose(when: starts_with(left: field(value: seed, name: "expected"), right: "ready"), then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: "b")), body: true))"#;
        let first = effect(start(&mut left, source));
        let mut right = Vm::open_journal(&path.0, 1).unwrap();
        let barrier = std::sync::Barrier::new(2);
        let (a, b) = std::thread::scope(|scope| {
            let a = scope.spawn(|| {
                barrier.wait();
                left.resume_at(&first.continuation, 101, receipt(&first))
            });
            let b = scope.spawn(|| {
                barrier.wait();
                right.resume_at(&first.continuation, 101, receipt(&first))
            });
            (a.join().unwrap(), b.join().unwrap())
        });
        assert_eq!(a, b);
        let selected = effect(a);
        assert!(
            matches!(&selected.continuation.pending_effect, Effect::UiFocus { node_id } if node_id == "a")
        );
        assert_eq!(rows(&path, "vm_effects"), 2);
        assert_eq!(
            left.resume_at(&selected.continuation, 102, receipt(&selected)),
            done(true)
        );
        assert_eq!(
            right.resume_at(&selected.continuation, 103, receipt(&selected)),
            done(true)
        );
    }
}
