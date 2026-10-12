use std::{
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};

use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_syntax::parse;
use leselang_vm::{
    EffectError, EffectErrorClass, EffectOperation, EffectResult, PresentationOperation,
    PresentationResult, RetryDisposition, RetryPolicy, ScalarValue, Step, Value, Vm,
};
use rusqlite::Connection;

const ALL: &str =
    r#"fn main() = all(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#;
const SEQ: &str =
    r#"fn main() = seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#;
const CHAIN: &str = r#"fn main() = bind(r: ui.focus(node_id: "a"),
    body: ui.focus(node_id: field(value: r, name: "node_id")))"#;
const DATAFLOW: &str = r#"fn main() = bind(a: ui.focus(node_id: "a"),
    body: bind(b: ui.focus(node_id: field(value: a, name: "node_id")),
        body: bind(c: ui.focus(node_id: field(value: b, name: "node_id")),
            body: eq(left: field(value: c, name: "node_id"), right: "a"))))"#;

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-group-receipts-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root.join("receipt.sqlite3"))
    }
}
impl Drop for JournalPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}

#[derive(Clone, Copy)]
enum Outcome {
    Success,
    Cancelled,
    Failed,
    Fault,
}

fn completed(path: &JournalPath, source: &str, outcome: Outcome) -> Step {
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let started = vm.start(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
    );
    assert!(matches!(started, Step::Effect(_) | Step::Effects(_)));
    let first = vm.claim_effect(1, 100).unwrap().unwrap();
    match outcome {
        Outcome::Success => {
            vm.acknowledge_effect(&first, 2, focus_result(&first.request.operation));
        }
        Outcome::Cancelled => {
            assert!(matches!(
                vm.cancel_effect(&first.request.continuation, 2),
                Step::Cancelled(_) | Step::Waiting(_)
            ));
        }
        Outcome::Failed => {
            assert!(matches!(
                vm.report_effect_error(
                    &first,
                    2,
                    EffectError {
                        class: EffectErrorClass::Permanent,
                        code: "adapter_failed".into(),
                        message: "original failure".into(),
                    },
                    &RetryPolicy::default(),
                )
                .unwrap(),
                RetryDisposition::Terminal(_)
            ));
        }
        Outcome::Fault => {
            let fault = vm.acknowledge_effect(
                &first,
                2,
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "wrong-target".into(),
                }),
            );
            assert!(matches!(fault, Step::Fault(_) | Step::Waiting(_)));
        }
    }
    let mut now = 3;
    while let Some(lease) = vm.claim_effect(now, 100).unwrap() {
        vm.acknowledge_effect(&lease, now + 1, focus_result(&lease.request.operation));
        now += 2;
        assert!(now < 20);
    }
    assert_eq!(vm.pending_count(), 0);
    drop(vm);
    let connection = Connection::open(&path.0).unwrap();
    let (plan, state, bytes): (Vec<u8>, String, Vec<u8>) = connection
        .query_row(
            "SELECT plan, state, terminal_step FROM vm_merge_groups",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(state, "completed");
    assert!(
        serde_json::from_slice::<serde_json::Value>(&plan).unwrap()["result_binding"].is_null()
    );
    let receipt: Step = serde_json::from_slice(&bytes).unwrap();
    let mut recovered = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(
        recovered.resume(
            &first.request.continuation,
            focus_result(&first.request.operation)
        ),
        receipt
    );
    receipt
}

fn focus_result(operation: &EffectOperation) -> EffectResult {
    let EffectOperation::Presentation(envelope) = operation else {
        panic!("expected presentation");
    };
    let PresentationOperation::Focus { node_id } = &envelope.operation else {
        panic!("expected focus");
    };
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: node_id.clone(),
    })
}

fn assert_refusal_then_repair(path: &JournalPath, changed: &Step, original: &Step) {
    let connection = Connection::open(&path.0).unwrap();
    let watermark: i64 = connection
        .query_row("SELECT next_sequence FROM vm_metadata", [], |r| r.get(0))
        .unwrap();
    connection
        .execute("UPDATE vm_metadata SET next_sequence = 1", [])
        .unwrap();
    let bytes = serde_json::to_vec(changed).unwrap();
    connection
        .execute("UPDATE vm_merge_groups SET terminal_step = ?1", [&bytes])
        .unwrap();
    let before = durable_snapshot(&connection);
    let error = Vm::open_journal(&path.0, 1)
        .err()
        .expect("forged group receipt must be rejected");
    assert_eq!(error.code, "LSV4007");
    assert_eq!(
        error.message,
        "merge group receipt does not match original branch results"
    );
    assert_eq!(durable_snapshot(&connection), before);
    assert_eq!(
        connection
            .query_row("SELECT next_sequence FROM vm_metadata", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        1
    );
    assert_eq!(
        connection
            .query_row("SELECT terminal_step FROM vm_merge_groups", [], |r| r
                .get::<_, Vec<u8>>(0))
            .unwrap(),
        bytes
    );
    connection
        .execute(
            "UPDATE vm_merge_groups SET terminal_step = ?1",
            [serde_json::to_vec(original).unwrap()],
        )
        .unwrap();
    let mut recovered = Vm::open_journal(&path.0, 1).unwrap();
    assert_eq!(recovered.pending_count(), 0);
    assert!(recovered.claim_effect(100, 100).unwrap().is_none());
    assert_eq!(
        connection
            .query_row("SELECT next_sequence FROM vm_metadata", [], |r| r
                .get::<_, i64>(0))
            .unwrap(),
        watermark
    );
}

fn durable_snapshot(connection: &Connection) -> Vec<Vec<Vec<rusqlite::types::Value>>> {
    [
        ("vm_metadata", "singleton"),
        ("vm_effects", "token"),
        ("vm_dispatches", "token"),
        ("vm_merge_groups", "token"),
        ("vm_merge_branches", "group_token, position"),
    ]
    .into_iter()
    .map(|(table, order)| {
        let mut statement = connection
            .prepare(&format!("SELECT * FROM {table} ORDER BY {order}"))
            .unwrap();
        let columns = statement.column_count();
        statement
            .query_map([], |row| {
                (0..columns)
                    .map(|column| row.get::<_, rusqlite::types::Value>(column))
                    .collect::<Result<Vec<_>, _>>()
            })
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    })
    .collect()
}

#[test]
fn bound_output_precedence_receipts_cannot_replace_aggregate_or_first_stop() {
    for (parallel, fail_first) in [(true, true), (false, false), (false, true)] {
        let path = JournalPath::new();
        let group = if parallel { "all" } else { "seq" };
        let source = format!(
            "fn main() = bind(g: {group}(first: runtime.list(), second: runtime.list(), third: runtime.list()), body: true)"
        );
        let mut vm = Vm::open_journal(&path.0, 500).unwrap();
        let started = vm.start(
            &lower(&parse(&source)).unwrap(),
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["runtime.read"]),
            Some(leselang_host_contract::Revision(7)),
        );
        assert!(matches!(started, Step::Effect(_) | Step::Effects(_)));
        let mut control = leserpent_domain::InMemoryControlPlane::default();
        let runtime = control.register_runtime(
            leselang_host_contract::RuntimeId::new("runtime-a").unwrap(),
            "runtime",
            "http://runtime.local",
        );
        let mut now = 1;
        while let Some(lease) = vm.claim_effect(now, 100).unwrap() {
            if now == 1 && fail_first {
                vm.report_effect_error(
                    &lease,
                    now + 1,
                    EffectError {
                        class: EffectErrorClass::Permanent,
                        code: "original_failure".into(),
                        message: "original first failure".into(),
                    },
                    &RetryPolicy::default(),
                )
                .unwrap();
            } else {
                let runtimes = (0..6000)
                    .map(|index| {
                        let mut item = runtime.clone();
                        item.id =
                            leselang_host_contract::RuntimeId::new(format!("runtime-{index}"))
                                .unwrap();
                        item
                    })
                    .collect();
                vm.acknowledge_effect(
                    &lease,
                    now + 1,
                    EffectResult::Query(leserpent_domain::QueryResult::RuntimeList {
                        revision: leselang_host_contract::Revision(7),
                        runtimes,
                    }),
                );
            }
            now += 2;
            assert!(now < 10);
        }
        assert_eq!(vm.pending_count(), 0);
        drop(vm);
        let connection = Connection::open(&path.0).unwrap();
        let (plan, bytes): (Vec<u8>, Vec<u8>) = connection
            .query_row(
                "SELECT plan, terminal_step FROM vm_merge_groups",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .unwrap();
        assert!(
            !serde_json::from_slice::<serde_json::Value>(&plan).unwrap()["result_binding"]
                .is_null()
        );
        let original: Step = serde_json::from_slice(&bytes).unwrap();
        let forged = if parallel {
            assert!(matches!(&original, Step::Fault(fault) if fault.code == "LSV2404"));
            let failed: Vec<u8> = connection.query_row("SELECT terminal_step FROM vm_effects WHERE token = (SELECT branch_token FROM vm_merge_branches WHERE position = 0)", [], |row| row.get(0)).unwrap();
            serde_json::from_slice::<Step>(&failed).unwrap()
        } else if fail_first {
            assert!(matches!(original, Step::Failed(_)));
            Step::Fault(leselang_vm::Fault {
                code: "LSV2404".into(),
                message: "sequential output exceeds runtime bounds".into(),
            })
        } else {
            assert!(matches!(&original, Step::Fault(fault) if fault.code == "LSV2404"));
            Step::Done(Value::Scalar {
                value: ScalarValue::Boolean(true),
            })
        };
        let watermark: i64 = connection
            .query_row("SELECT next_sequence FROM vm_metadata", [], |row| {
                row.get(0)
            })
            .unwrap();
        connection
            .execute("UPDATE vm_metadata SET next_sequence = 1", [])
            .unwrap();
        connection
            .execute(
                "UPDATE vm_merge_groups SET terminal_step = ?1",
                [serde_json::to_vec(&forged).unwrap()],
            )
            .unwrap();
        let before = durable_snapshot(&connection);
        assert_eq!(Vm::open_journal(&path.0, 1).err().unwrap().code, "LSV1409");
        assert_eq!(durable_snapshot(&connection), before);
        connection
            .execute("UPDATE vm_merge_groups SET terminal_step = ?1", [&bytes])
            .unwrap();
        let mut recovered = Vm::open_journal(&path.0, 1).unwrap();
        assert_eq!(recovered.pending_count(), 0);
        assert!(recovered.claim_effect(100, 100).unwrap().is_none());
        assert_eq!(
            connection
                .query_row("SELECT next_sequence FROM vm_metadata", [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            watermark
        );
        let repaired: Vec<u8> = connection
            .query_row("SELECT terminal_step FROM vm_merge_groups", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(repaired, bytes);
    }
}

#[test]
fn unbound_completed_receipt_must_match_original_members_before_watermark_repair() {
    for source in [ALL, SEQ, CHAIN, DATAFLOW] {
        let path = JournalPath::new();
        let original = completed(&path, source, Outcome::Success);
        assert_refusal_then_repair(
            &path,
            &Step::Done(Value::Scalar {
                value: ScalarValue::Integer(999),
            }),
            &original,
        );
    }
}

#[test]
fn original_field_values_names_and_declared_order_are_not_interchangeable() {
    for source in [ALL, SEQ] {
        for mutation in 0..3 {
            let path = JournalPath::new();
            let original = completed(&path, source, Outcome::Success);
            let mut changed = original.clone();
            let Step::Done(Value::Structured { fields }) = &mut changed else {
                panic!()
            };
            match mutation {
                0 => fields.reverse(),
                1 => fields[0].name = "other".into(),
                _ => {
                    *fields[0].value = Value::UiFocus {
                        node_id: "forged".into(),
                    }
                }
            }
            assert_refusal_then_repair(&path, &changed, &original);
        }
    }
}

#[test]
fn structurally_valid_fault_cannot_replace_success_or_change_original_refusal() {
    for source in [ALL, SEQ] {
        for outcome in [Outcome::Success, Outcome::Fault] {
            let path = JournalPath::new();
            let original = completed(&path, source, outcome);
            let changed = Step::Fault(leselang_vm::Fault {
                code: "LSV1001".into(),
                message: "forged refusal".into(),
            });
            assert_refusal_then_repair(&path, &changed, &original);
        }
    }
}

#[test]
fn original_cancellation_and_failure_metadata_are_part_of_receipt_correspondence() {
    for source in [ALL, SEQ] {
        for outcome in [Outcome::Cancelled, Outcome::Failed] {
            let path = JournalPath::new();
            let original = completed(&path, source, outcome);
            let mut changed = original.clone();
            match &mut changed {
                Step::Cancelled(value) => value.observed_at_ms += 1,
                Step::Failed(value) => value.error.message = "forged failure".into(),
                other => panic!("expected terminal refusal: {other:?}"),
            }
            assert_refusal_then_repair(&path, &changed, &original);
        }
    }
}

#[test]
fn parallel_failure_choice_follows_declared_order_not_completion_chronology() {
    let path = JournalPath::new();
    let mut vm = Vm::open_journal(&path.0, 500).unwrap();
    let Step::Effects(batch) = vm.start(
        &lower(&parse(ALL)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
    ) else {
        panic!()
    };
    let left = &batch.branches[0].request;
    let right = &batch.branches[1].request;
    assert!(matches!(
        vm.resume(
            &right.continuation,
            EffectResult::Presentation(PresentationResult::Focus {
                node_id: "wrong-target".into()
            })
        ),
        Step::Waiting(_)
    ));
    let original = vm.cancel_effect(&left.continuation, 2);
    assert!(matches!(original, Step::Cancelled(_)));
    drop(vm);
    let connection = Connection::open(&path.0).unwrap();
    let bytes: Vec<u8> = connection
        .query_row(
            "SELECT terminal_step FROM vm_effects WHERE token = ?1",
            [right.continuation.token.as_str()],
            |row| row.get(0),
        )
        .unwrap();
    let changed: Step = serde_json::from_slice(&bytes).unwrap();
    assert!(matches!(changed, Step::Fault(_)));
    assert_refusal_then_repair(&path, &changed, &original);
}
