use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use leselang_hir::{UiFocusNavigationDirection, lower};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectOperation, EffectRequest, EffectResult, PresentationOperation, PresentationResult,
    RetentionPolicy, ScalarValue, Step, Value, Vm, decode_continuation, encode_continuation,
};
use rusqlite::{Connection, params};

const FLOW: &str = r#"fn main() = bind(prefix: "target-", body:
    bind(first: ui.navigate_focus(node_id: "a", direction: "next"), body:
        bind(alias: first, body:
            bind(second: ui.focus(node_id: field(value: first, name: "focused_node_id")), body:
                bind(third: ui.focus(node_id: concat(left: prefix, right: field(value: second, name: "node_id"))), body:
                    and(left: eq(left: field(value: alias, name: "focused_node_id"), right: field(value: second, name: "node_id")),
                        right: eq(left: field(value: third, name: "node_id"), right: concat(left: prefix, right: field(value: first, name: "focused_node_id")))))))))"#;

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-dataflow-{}-{}",
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
    vm.start(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation", "runtime.read"]),
        Some(Revision(7)),
    )
}
fn effect(step: Step) -> EffectRequest {
    match step {
        Step::Effect(request) => *request,
        other => panic!("expected effect: {other:?}"),
    }
}
fn navigate(destination: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::NavigateFocus {
        node_id: "a".into(),
        direction: UiFocusNavigationDirection::Next,
        focused_node_id: destination.into(),
    })
}
fn focus(id: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus { node_id: id.into() })
}
fn node(request: &EffectRequest) -> &str {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!("presentation expected")
    };
    let PresentationOperation::Focus { node_id } = &envelope.operation else {
        panic!("focus expected")
    };
    node_id
}
fn done() -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::Boolean(true),
    })
}
fn scalar(value: u64) -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::Integer(value),
    })
}
fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

#[test]
fn three_results_and_aliases_survive_reentry_without_fresh_authority_or_fuel() {
    let path = JournalPath::new();
    for mut vm in [Vm::new(500), Vm::open_journal(&path.0, 500).unwrap()] {
        let first = effect(start(&mut vm, FLOW));
        assert_eq!(first.continuation.schema_version, 4);
        assert!(
            first
                .continuation
                .result_binding
                .as_ref()
                .unwrap()
                .results
                .is_empty()
        );
        let second = effect(vm.resume(&first.continuation, navigate("b")));
        assert_eq!(node(&second), "b");
        let third = effect(vm.resume(&second.continuation, focus("b")));
        assert_eq!(node(&third), "target-b");
        for request in [&second, &third] {
            assert_eq!(request.continuation.schema_version, 4);
            assert_eq!(
                decode_continuation(&encode_continuation(&request.continuation).unwrap()).unwrap(),
                request.continuation
            );
            assert_eq!(
                request.continuation.expected_revision,
                first.continuation.expected_revision
            );
            assert_eq!(request.budget.deadline_ms, first.budget.deadline_ms);
            let (EffectOperation::Presentation(a), EffectOperation::Presentation(b)) =
                (&request.operation, &first.operation)
            else {
                panic!("presentation expected")
            };
            assert_eq!(a.principal, b.principal);
            assert_eq!(a.capabilities, b.capabilities);
        }
        assert!(first.budget.fuel_remaining > second.budget.fuel_remaining);
        assert!(second.budget.fuel_remaining > third.budget.fuel_remaining);
        let locals = &third.continuation.result_binding.as_ref().unwrap().results;
        assert_eq!(
            locals
                .iter()
                .map(|saved| saved.name.as_str())
                .collect::<Vec<_>>(),
            ["first", "alias", "second"]
        );
        assert_eq!(locals[0].result, locals[1].result);
        assert_eq!(vm.resume(&third.continuation, focus("target-b")), done());
        for request in [&first, &second, &third] {
            assert_eq!(vm.resume(&request.continuation, focus("ignored")), done());
        }
        assert_eq!(vm.pending_count(), 0);
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
    }
}

#[test]
fn each_suspension_recovers_from_the_journal_and_retains_the_committed_choice() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, FLOW));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let second = effect(vm.resume(&first.continuation, navigate("b")));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        effect(vm.resume(&first.continuation, navigate("changed"))),
        second
    );
    let third = effect(vm.resume(&second.continuation, focus("b")));
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        vm.pending_continuations(),
        std::slice::from_ref(&third.continuation)
    );
    assert_eq!(
        effect(vm.resume(&first.continuation, navigate("changed"))),
        third
    );
    assert_eq!(vm.resume(&third.continuation, focus("target-b")), done());
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(vm.resume(&second.continuation, focus("changed")), done());
    let connection = Connection::open(&path.0).unwrap();
    assert_eq!(count(&connection, "vm_merge_groups"), 1);
    assert_eq!(count(&connection, "vm_effects"), 3);
}

#[test]
fn competing_workers_reconcile_completed_prefixes_without_duplicate_children() {
    let path = JournalPath::new();
    let mut winner = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut winner, FLOW));
    let mut peer = Vm::open_journal(&path.0, 1).unwrap();
    let second = effect(winner.resume(&first.continuation, navigate("winner")));
    assert_eq!(
        effect(peer.resume(&first.continuation, navigate("loser"))),
        second
    );
    let third = effect(winner.resume(&second.continuation, focus("winner")));
    assert_eq!(
        effect(peer.resume(&first.continuation, navigate("again"))),
        third
    );
    assert_eq!(
        peer.pending_continuations(),
        std::slice::from_ref(&third.continuation)
    );
    assert_eq!(
        effect(peer.resume(&second.continuation, focus("wrong"))),
        third
    );
    assert_eq!(
        peer.resume(&third.continuation, focus("target-winner")),
        done()
    );
    assert_eq!(winner.resume(&first.continuation, navigate("late")), done());
    assert_eq!(winner.pending_count(), 0);
    assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 3);
}

#[test]
fn appending_an_effect_rolls_back_the_entire_transition_at_each_write() {
    for (table, operation) in [
        ("vm_merge_groups", "UPDATE"),
        ("vm_effects", "UPDATE"),
        ("vm_dispatches", "UPDATE"),
        ("vm_effects", "INSERT"),
        ("vm_dispatches", "INSERT"),
        ("vm_merge_branches", "INSERT"),
    ] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(start(&mut vm, FLOW));
        let second = effect(vm.resume(&first.continuation, navigate("b")));
        let connection = Connection::open(&path.0).unwrap();
        connection.execute_batch(&format!("CREATE TRIGGER reject_append BEFORE {operation} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
        assert!(matches!(
            vm.resume(&second.continuation, focus("b")),
            Step::Fault(_)
        ));
        assert_eq!(count(&connection, "vm_effects"), 2);
        assert_eq!(count(&connection, "vm_merge_branches"), 2);
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(
            vm.pending_continuations(),
            std::slice::from_ref(&second.continuation)
        );
        connection
            .execute_batch("DROP TRIGGER reject_append;")
            .unwrap();
        let third = effect(vm.resume(&second.continuation, focus("b")));
        assert_eq!(vm.resume(&third.continuation, focus("target-b")), done());
        assert_eq!(count(&connection, "vm_effects"), 3);
    }
}

#[test]
fn schema_four_snapshots_need_authority_and_reject_forged_projection_scopes() {
    let mut vm = Vm::new(500);
    let first = effect(start(&mut vm, FLOW));
    let second = effect(vm.resume(&first.continuation, navigate("b")));
    let mut restored = Vm::new(1);
    assert!(restored.restore(second.continuation.clone()).is_err());
    restored.restore_request(second.clone()).unwrap();
    let third = effect(restored.resume(&second.continuation, focus("b")));
    assert_eq!(
        restored.resume(&third.continuation, focus("target-b")),
        done()
    );
    for corruption in [
        "missing",
        "extra",
        "wrong_type",
        "shadow",
        "unknown_name",
        "legacy_schema",
        "oversized",
    ] {
        let mut forged = second.clone();
        let binding = forged.continuation.result_binding.as_mut().unwrap();
        match corruption {
            "missing" => {
                binding.results[0].result.fields.pop();
            }
            "extra" => {
                let field = binding.results[0].result.fields[0].clone();
                binding.results[0].result.fields.push(field);
            }
            "wrong_type" => binding.results[0].result.fields[0].value = ScalarValue::Integer(1),
            "shadow" => binding.results[0].name = binding.name.clone(),
            "unknown_name" => binding.results[0].name = "unknown".into(),
            "legacy_schema" => forged.continuation.schema_version = 2,
            "oversized" => {
                binding.results[0].result.fields[0].value = ScalarValue::String("x".repeat(4097))
            }
            _ => unreachable!(),
        }
        assert!(
            Vm::default().restore_request(forged).is_err(),
            "{corruption}"
        );
    }
}

#[test]
fn conditional_chains_and_pure_loops_can_reuse_multiple_prior_results() {
    let source = r#"fn main() = bind(a: runtime.list(), body:
        choose(when: eq(left: field(value: a, name: "count"), right: 0),
            then: bind(b: runtime.list(), body: loop(n: field(value: a, name: "revision"),
                while: lt(left: n, right: add(left: field(value: b, name: "revision"), right: 2)),
                next: add(left: n, right: 1), limit: 2)),
            otherwise: bind(b: runtime.list(), body: div(left: 1, right: 0))))"#;
    let result = || {
        EffectResult::Query(leserpent_domain::QueryResult::RuntimeList {
            revision: Revision(7),
            runtimes: vec![],
        })
    };
    let mut vm = Vm::new(500);
    let first = effect(start(&mut vm, source));
    let second = effect(vm.resume(&first.continuation, result()));
    assert_eq!(vm.resume(&second.continuation, result()), scalar(9));
}

#[test]
fn later_faults_cancellation_and_deadlines_close_the_chain_without_new_effects() {
    for outcome in ["bad_result", "cancel", "deadline"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(vm.start_timed(
            &lower(&parse(FLOW)).unwrap(),
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            None,
            100,
            100,
        ));
        let second = effect(vm.resume_at(&first.continuation, 110, navigate("b")));
        let third = effect(vm.resume_at(&second.continuation, 120, focus("b")));
        assert_eq!(third.budget.deadline_at_ms, Some(200));
        let terminal = match outcome {
            "bad_result" => vm.resume_at(&third.continuation, 130, focus("wrong")),
            "cancel" => vm.cancel_effect(&third.continuation, 130),
            "deadline" => vm.resume_at(&third.continuation, 200, focus("target-b")),
            _ => unreachable!(),
        };
        assert!(matches!(terminal, Step::Fault(_) | Step::Cancelled(_)));
        drop(vm);
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        assert_eq!(
            vm.resume_at(&first.continuation, 201, navigate("late")),
            terminal
        );
        assert!(vm.claim_effect(202, 100).unwrap().is_none());
        assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 3);
    }
}

#[test]
fn dynamic_chain_retention_never_prunes_a_live_prefix() {
    let path = JournalPath::new();
    for mut vm in [Vm::new(500), Vm::open_journal(&path.0, 500).unwrap()] {
        let first = effect(start(&mut vm, FLOW));
        let second = effect(vm.resume(&first.continuation, navigate("b")));
        let policy = RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 1,
        };
        assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 0);
        let third = effect(vm.resume(&second.continuation, focus("b")));
        assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 0);
        assert_eq!(vm.resume(&third.continuation, focus("target-b")), done());
        let later = effect(start(&mut vm, "fn main() = runtime.list()"));
        let result = EffectResult::Query(leserpent_domain::QueryResult::RuntimeList {
            revision: Revision(7),
            runtimes: vec![],
        });
        assert!(matches!(
            vm.resume(&later.continuation, result),
            Step::Done(_)
        ));
        assert_eq!(vm.compact_journal(&policy).unwrap().removed_records, 1);
        assert_eq!(vm.completed_count(), 1);
    }
    let connection = Connection::open(&path.0).unwrap();
    assert_eq!(count(&connection, "vm_effects"), 1);
    assert_eq!(count(&connection, "vm_merge_groups"), 0);
}

#[test]
fn recovery_rejects_projection_rebinding_and_truncated_committed_chains() {
    for corruption in ["projection", "budget", "orphan", "truncated"] {
        let path = JournalPath::new();
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let first = effect(start(&mut vm, FLOW));
        let second = effect(vm.resume(&first.continuation, navigate("b")));
        let mut third = effect(vm.resume(&second.continuation, focus("b")));
        drop(vm);
        let connection = Connection::open(&path.0).unwrap();
        match corruption {
            "projection" | "budget" => {
                if corruption == "projection" {
                    third.continuation.result_binding.as_mut().unwrap().results[0]
                        .result
                        .fields[1]
                        .value = ScalarValue::String("forged".into());
                } else {
                    third.continuation.fuel_remaining = second.budget.fuel_remaining;
                    third.budget.fuel_remaining = second.budget.fuel_remaining;
                }
                connection
                    .execute(
                        "UPDATE vm_effects SET image = ?2 WHERE token = ?1",
                        params![
                            third.continuation.token.as_str(),
                            serde_json::to_vec(&third.continuation).unwrap()
                        ],
                    )
                    .unwrap();
                connection
                    .execute(
                        "UPDATE vm_dispatches SET request = ?2 WHERE token = ?1",
                        params![
                            third.continuation.token.as_str(),
                            serde_json::to_vec(&third).unwrap()
                        ],
                    )
                    .unwrap();
            }
            "orphan" => {
                connection
                    .execute("DELETE FROM vm_merge_branches", [])
                    .unwrap();
            }
            "truncated" => {
                connection
                    .execute("DELETE FROM vm_merge_branches WHERE position = 2", [])
                    .unwrap();
                connection
                    .execute(
                        "UPDATE vm_merge_groups SET plan = ?1",
                        [serde_json::to_vec(
                            &serde_json::json!({"order":"dataflow","branches":["step_1","step_2"]}),
                        )
                        .unwrap()],
                    )
                    .unwrap();
            }
            _ => unreachable!(),
        }
        assert!(Vm::open_journal(&path.0, 500).is_err(), "{corruption}");
    }
}

fn inventory(size: usize) -> EffectResult {
    let mut control = leserpent_domain::InMemoryControlPlane::default();
    let runtime = control.register_runtime(
        leselang_host_contract::RuntimeId::new("runtime-a").unwrap(),
        "Private runtime metadata",
        "http://private-runtime",
    );
    let runtimes = (0..size)
        .map(|index| {
            let mut runtime = runtime.clone();
            runtime.id =
                leselang_host_contract::RuntimeId::new(format!("runtime-{index}")).unwrap();
            runtime
        })
        .collect();
    EffectResult::Query(leserpent_domain::QueryResult::RuntimeList {
        revision: Revision(7),
        runtimes,
    })
}

#[test]
fn large_host_objects_are_not_copied_into_resumable_lexical_frames() {
    let source = r#"fn main() = bind(a: runtime.list(), body: bind(b: runtime.list(), body:
        add(left: field(value: a, name: "count"), right: field(value: b, name: "count"))))"#;
    let mut vm = Vm::new(500);
    let first = effect(start(&mut vm, source));
    let large = inventory(1024);
    assert!(serde_json::to_vec(&large).unwrap().len() > leselang_vm::MAX_CONTINUATION_BYTES);
    let second = effect(vm.resume(&first.continuation, large));
    let encoded = encode_continuation(&second.continuation).unwrap();
    assert!(encoded.len() < 4096);
    assert!(
        !String::from_utf8(encoded)
            .unwrap()
            .contains("private-runtime")
    );
    let mut restored = Vm::new(1);
    restored.restore_request(second.clone()).unwrap();
    assert_eq!(
        restored.resume(&second.continuation, inventory(2)),
        scalar(1026)
    );
}

#[test]
fn cumulative_output_exhaustion_stops_before_admitting_another_effect() {
    let source = r#"fn main() = bind(a: runtime.list(), body: bind(b: runtime.list(), body:
        bind(c: runtime.list(), body: field(value: c, name: "count"))))"#;
    let path = JournalPath::new();
    for mut vm in [Vm::new(500), Vm::open_journal(&path.0, 500).unwrap()] {
        let first = effect(start(&mut vm, source));
        let second = effect(vm.resume(&first.continuation, inventory(5001)));
        let terminal = vm.resume(&second.continuation, inventory(5001));
        assert!(
            matches!(&terminal, Step::Fault(fault) if fault.code == "LSV2404"),
            "{terminal:?}"
        );
        assert_eq!(vm.pending_count(), 0);
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
        assert_eq!(vm.resume(&first.continuation, inventory(0)), terminal);
    }
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    assert!(vm.claim_effect(1, 100).unwrap().is_none());
    assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 2);
}

#[test]
fn terminal_scalar_projection_cannot_hide_cumulative_raw_output() {
    for conditional in [false, true] {
        let body = if conditional {
            "choose(when: true, then: 1, otherwise: bind(c: runtime.list(), body: 2))"
        } else {
            "1"
        };
        let source = format!(
            "fn main() = bind(a: runtime.list(), body: bind(b: runtime.list(), body: {body}))"
        );
        for leased in [false, true] {
            let path = JournalPath::new();
            for mut vm in [Vm::new(500), Vm::open_journal(&path.0, 500).unwrap()] {
                let first = effect(start(&mut vm, &source));
                let second = effect(vm.resume(&first.continuation, inventory(5001)));
                let terminal = if leased {
                    let lease = vm.claim_effect(1, 100).unwrap().unwrap();
                    vm.acknowledge_effect(&lease, 2, inventory(5001))
                } else {
                    vm.resume(&second.continuation, inventory(5001))
                };
                assert!(
                    matches!(&terminal, Step::Fault(f) if f.code == "LSV2404"),
                    "{terminal:?}"
                );
                assert_eq!(vm.pending_count(), 0);
                assert_eq!(vm.resume(&first.continuation, inventory(0)), terminal);
            }
            assert_eq!(Vm::open_journal(&path.0, 1).unwrap().pending_count(), 0);
            assert_eq!(count(&Connection::open(&path.0).unwrap(), "vm_effects"), 2);
        }
    }
}

#[test]
fn leased_dataflow_steps_keep_retry_fences_and_do_not_reset_fuel() {
    use leselang_vm::{EffectError, EffectErrorClass, RetryDisposition, RetryPolicy};
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let first = effect(start(&mut vm, FLOW));
    let first_lease = vm.claim_effect(1, 100).unwrap().unwrap();
    let second = effect(vm.acknowledge_effect(&first_lease, 2, navigate("b")));
    let stale = vm.claim_effect(3, 10).unwrap().unwrap();
    assert!(
        matches!(vm.resume(&second.continuation, focus("b")), Step::Fault(f) if f.code == "LSV4024")
    );
    let active = vm.claim_effect(13, 100).unwrap().unwrap();
    assert!(
        matches!(vm.acknowledge_effect(&stale, 14, focus("b")), Step::Fault(f) if f.code == "LSV4022")
    );
    assert!(matches!(
        vm.report_effect_error(
            &active,
            14,
            EffectError {
                class: EffectErrorClass::Transient,
                code: "busy".into(),
                message: "retry".into(),
            },
            &RetryPolicy::default()
        )
        .unwrap(),
        RetryDisposition::Scheduled(_)
    ));
    assert!(vm.claim_effect(100, 100).unwrap().is_none());
    drop(vm);
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    let retry = vm.claim_effect(264, 100).unwrap().unwrap();
    assert_eq!(retry.request, second);
    let third = effect(vm.acknowledge_effect(&retry, 265, focus("b")));
    assert_eq!(
        effect(vm.acknowledge_effect(&active, 266, focus("wrong"))),
        third
    );
    assert!(third.budget.fuel_remaining < second.budget.fuel_remaining);
    let final_lease = vm.claim_effect(267, 100).unwrap().unwrap();
    assert_eq!(
        vm.acknowledge_effect(&final_lease, 268, focus("target-b")),
        done()
    );
    assert_eq!(vm.resume(&first.continuation, navigate("late")), done());
    let mut exhausted = third.clone();
    exhausted.continuation.fuel_remaining = 1;
    exhausted.budget.fuel_remaining = 1;
    let mut restored = Vm::new(1_000_000);
    restored.restore_request(exhausted.clone()).unwrap();
    assert!(
        matches!(restored.resume(&exhausted.continuation, focus("target-b")), Step::Fault(f) if f.code == "LSV1001")
    );
}

#[test]
fn journal_eight_migration_preserves_legacy_two_effect_chains() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 100).unwrap();
    let first = effect(start(
        &mut vm,
        r#"fn main() = bind(r: ui.focus(node_id: "a"), body: ui.focus(node_id: field(value: r, name: "node_id")))"#,
    ));
    assert_eq!(first.continuation.schema_version, 3);
    let second = effect(vm.resume(&first.continuation, focus("a")));
    drop(vm);
    let connection = Connection::open(&path.0).unwrap();
    connection
        .execute_batch("PRAGMA user_version = 8;")
        .unwrap();
    let mut vm = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        connection
            .query_row("PRAGMA user_version", [], |row| row.get::<_, u32>(0))
            .unwrap(),
        leselang_vm::JOURNAL_SCHEMA_VERSION
    );
    assert_eq!(
        effect(vm.resume(&first.continuation, focus("changed"))),
        second
    );
    assert_eq!(
        vm.resume(&second.continuation, focus("a")),
        Step::Done(Value::UiFocus {
            node_id: "a".into()
        })
    );
}
