use super::*;

fn audit() -> DebuggerAuditContext {
    DebuggerAuditContext {
        command_id: CommandId::new("cancel-command").unwrap(),
        idempotency_key: IdempotencyKey::new("cancel-key").unwrap(),
        principal: Principal::new("debugger-operator").unwrap(),
        origin: CommandOrigin::Gui,
        session_id: "session-a".into(),
        expected_revision: Revision(7),
    }
}

#[derive(Debug, PartialEq)]
enum Outcome {
    Ordinary(Step),
    Audited(Result<DebuggerAuditRecord, Fault>),
}
fn invoke(
    vm: &mut Vm,
    image: &ContinuationImage,
    now: u64,
    audit: &DebuggerAuditContext,
    audited: bool,
    frozen: bool,
) -> Outcome {
    match (audited, frozen) {
        (false, false) => Outcome::Ordinary(vm.cancel_effect(image, now)),
        (false, true) => Outcome::Ordinary(frozen_cancel_effect(vm, image, now)),
        (true, false) => Outcome::Audited(vm.cancel_effect_audited(image, now, audit)),
        (true, true) => Outcome::Audited(frozen_cancel_effect_audited(vm, image, now, audit)),
    }
}
fn wire(outcome: &Outcome) -> Vec<u8> {
    match outcome {
        Outcome::Ordinary(step) => serde_json::to_vec(step).unwrap(),
        Outcome::Audited(record) => serde_json::to_vec(record).unwrap(),
    }
}
fn assert_state(actual: &Vm, expected: &Vm) {
    assert_eq!(actual.pending, expected.pending);
    assert_eq!(actual.completed, expected.completed);
    assert_eq!(actual.next_effect_id, expected.next_effect_id);
    assert_eq!(
        actual.scheduler_pressure(12),
        expected.scheduler_pressure(12)
    );
}

#[test]
fn cancellation_entries_match_frozen_vm_for_seventy_operations_and_competing_outcomes() {
    let mut comparisons = 0;
    let mut baseline_completions = 0;
    for (original, reply) in crate::reply_contract_tests::cases() {
        for audited in [false, true] {
            for defect in 0..16 {
                let mut request = original.clone();
                if defect == 5 {
                    request.continuation.deadline_at_ms = Some(10);
                    request.budget = request.continuation.resource_budget();
                }
                let mut actual = Vm::new(1);
                let mut expected = Vm::new(1);
                actual.restore_request(request.clone()).unwrap();
                expected.restore_request(request.clone()).unwrap();
                let mut image = request.continuation.clone();
                let mut context = audit();
                let mut now = 12;
                if matches!(defect, 6 | 15) {
                    assert_eq!(
                        frozen_cancel_effect(&mut actual, &image, 2),
                        frozen_cancel_effect(&mut expected, &image, 2)
                    );
                }
                if matches!(defect, 7 | 8 | 13 | 14) {
                    assert_eq!(
                        frozen_cancel_effect_audited(&mut actual, &image, 2, &context),
                        frozen_cancel_effect_audited(&mut expected, &image, 2, &context)
                    );
                }
                if matches!(defect, 10 | 11) {
                    let lease = actual.claim_effect(1, 100).unwrap().unwrap();
                    assert_eq!(expected.claim_effect(1, 100).unwrap().unwrap(), lease);
                    if defect == 10 {
                        assert_eq!(
                            actual.acknowledge_effect(&lease, 2, reply.clone()),
                            expected.acknowledge_effect(&lease, 2, reply.clone())
                        );
                    }
                }
                if defect == 12 {
                    let mut other = request.clone();
                    other.continuation.token =
                        ContinuationToken::from_wire("continuation-9000".into());
                    other.effect_id = "effect-9000".into();
                    match &mut other.operation {
                        EffectOperation::Command(e) => {
                            e.command_id = CommandId::new("leselang-command-9000").unwrap();
                            e.idempotency_key =
                                IdempotencyKey::new("leselang-effect-9000").unwrap();
                        }
                        EffectOperation::Query(_) | EffectOperation::Presentation(_) => {}
                    }
                    actual.restore_request(other.clone()).unwrap();
                    expected.restore_request(other.clone()).unwrap();
                    assert_eq!(
                        frozen_cancel_effect_audited(&mut actual, &other.continuation, 2, &context),
                        frozen_cancel_effect_audited(
                            &mut expected,
                            &other.continuation,
                            2,
                            &context
                        )
                    );
                }
                match defect {
                    1 => image.token = ContinuationToken::from_wire("continuation-9001".into()),
                    2 => image.fuel_remaining -= 1,
                    3 | 14 => image.schema_version = 0,
                    4 => now = u64::MAX,
                    8 => context.principal = Principal::new("another-operator").unwrap(),
                    9 => {
                        context.session_id = "invalid session".into();
                        now = u64::MAX;
                    }
                    13 => {
                        context.command_id = CommandId::new("new-command").unwrap();
                        context.idempotency_key = IdempotencyKey::new("new-key").unwrap();
                    }
                    15 => context.session_id = "invalid session".into(),
                    _ => {}
                }
                let before = serde_json::to_vec(&(&image, &context)).unwrap();
                let want = invoke(&mut expected, &image, now, &context, audited, true);
                let got = invoke(&mut actual, &image, now, &context, audited, false);
                assert_eq!(
                    got, want,
                    "audited={audited}, defect={defect}, type={:?}",
                    original.continuation.result_type
                );
                assert_eq!(wire(&got), wire(&want));
                assert_eq!(serde_json::to_vec(&(&image, &context)).unwrap(), before);
                if defect == 0 {
                    assert!(matches!(
                        got,
                        Outcome::Ordinary(Step::Cancelled(_)) | Outcome::Audited(Ok(_))
                    ));
                    baseline_completions += 1;
                }
                if audited && defect == 6 {
                    assert!(matches!(got, Outcome::Audited(Err(ref e)) if e.code == "LSV4033"));
                }
                if audited && matches!(defect, 5 | 10) {
                    assert!(matches!(got, Outcome::Audited(Err(ref e)) if e.code == "LSV2014"));
                }
                assert_eq!(
                    actual.debugger_audit(&context.command_id),
                    expected.debugger_audit(&context.command_id)
                );
                assert_state(&actual, &expected);
                assert_eq!(
                    actual.claim_effect(201, 100),
                    expected.claim_effect(201, 100)
                );
                assert_eq!(actual.allocate_sequence(), expected.allocate_sequence());
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 2240);
    assert_eq!(baseline_completions, 140);
}

#[cfg(feature = "sqlite-journal")]
#[test]
fn cancellation_entries_sqlite_match_frozen_failures_peer_races_and_restart() {
    use rusqlite::{Connection, types::Value as SqlValue};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn snapshot(path: &std::path::Path) -> Vec<(String, Vec<Vec<SqlValue>>)> {
        let db = Connection::open(path).unwrap();
        let tables = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'vm_%' ORDER BY name").unwrap().query_map([], |row| row.get::<_, String>(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
        tables
            .into_iter()
            .map(|table| {
                let mut stmt = db
                    .prepare(&format!(
                        "SELECT * FROM \"{}\" ORDER BY rowid",
                        table.replace('"', "\"\"")
                    ))
                    .unwrap();
                let columns = stmt.column_count();
                let rows = stmt
                    .query_map([], |row| {
                        (0..columns)
                            .map(|i| row.get(i))
                            .collect::<Result<Vec<SqlValue>, _>>()
                    })
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                (table, rows)
            })
            .collect()
    }

    let mut comparisons = 0;
    for source in [
        r#"ui.focus(node_id: "a")"#,
        r#"bind(r: ui.focus(node_id: "a"), body: 7)"#,
        r#"bind(r: ui.focus(node_id: "a"), body: ui.focus(node_id: "b"))"#,
        r#"bind(r: ui.focus(node_id: "a"), body: bind(s: ui.focus(node_id: "b"), body: 7))"#,
        r#"seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"bind(r: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"bind(r: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: bind(s: ui.focus(node_id: "c"), body: 7))"#,
    ] {
        let program =
            leselang_hir::lower(&leselang_syntax::parse(&format!("fn main() = {source}"))).unwrap();
        for audited in [false, true] {
            for defect in 0..6 {
                let scratch = Scratch(std::env::temp_dir().join(format!(
                    "leselang-cancellation-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                )));
                std::fs::create_dir(&scratch.0).unwrap();
                let actual_path = scratch.0.join("actual.sqlite3");
                let expected_path = scratch.0.join("expected.sqlite3");
                let mut actual = Vm::open_journal(&actual_path, 100).unwrap();
                let mut expected = Vm::open_journal(&expected_path, 100).unwrap();
                let start = |vm: &mut Vm| {
                    vm.start(
                        &program,
                        Principal::new("operator").unwrap(),
                        CapabilitySet::new([CAPABILITY_UI_PRESENTATION]),
                        None,
                    )
                };
                assert_eq!(start(&mut actual), start(&mut expected));
                let mut peer_actual = Vm::open_journal(&actual_path, 1).unwrap();
                let mut peer_expected = Vm::open_journal(&expected_path, 1).unwrap();
                let lease = actual.claim_effect(1, 100).unwrap().unwrap();
                assert_eq!(expected.claim_effect(1, 100).unwrap().unwrap(), lease);
                let image = lease.request.continuation;
                let context = audit();
                if defect == 5 {
                    assert_eq!(
                        invoke(&mut peer_actual, &image, 2, &context, audited, false),
                        invoke(&mut peer_expected, &image, 2, &context, audited, true)
                    );
                } else if defect > 0 {
                    let (table, action) = match defect {
                        1 => ("vm_effects", "UPDATE"),
                        2 => ("vm_dispatches", "UPDATE"),
                        3 => ("vm_debugger_audit", "INSERT"),
                        _ => ("vm_merge_groups", "UPDATE"),
                    };
                    for path in [&actual_path, &expected_path] {
                        Connection::open(path).unwrap().execute_batch(&format!("CREATE TRIGGER reject_cancellation BEFORE {action} ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
                    }
                }
                let want = invoke(&mut expected, &image, 12, &context, audited, true);
                let got = invoke(&mut actual, &image, 12, &context, audited, false);
                assert_eq!(got, want, "{source}, audited={audited}, defect={defect}");
                assert_eq!(wire(&got), wire(&want));
                if matches!(defect, 0 | 5) {
                    assert!(
                        !matches!(
                            got,
                            Outcome::Ordinary(Step::Fault(_)) | Outcome::Audited(Err(_))
                        ),
                        "{got:?}"
                    );
                }
                assert_state(&actual, &expected);
                assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
                for path in [&actual_path, &expected_path] {
                    Connection::open(path)
                        .unwrap()
                        .execute_batch("DROP TRIGGER IF EXISTS reject_cancellation")
                        .unwrap();
                }
                assert_eq!(
                    invoke(&mut peer_actual, &image, 13, &context, audited, false),
                    invoke(&mut peer_expected, &image, 13, &context, audited, true)
                );
                assert_state(&peer_actual, &peer_expected);
                drop((actual, expected, peer_actual, peer_expected));
                let mut actual = Vm::open_journal(&actual_path, 1).unwrap();
                let mut expected = Vm::open_journal(&expected_path, 1).unwrap();
                assert_state(&actual, &expected);
                assert_eq!(
                    actual.debugger_audit(&context.command_id),
                    expected.debugger_audit(&context.command_id)
                );
                let mut drained = 0;
                for now in [200, 202, 204, 206, 208, 210, 212, 214] {
                    let lease = actual.claim_effect(now, 100).unwrap();
                    assert_eq!(lease, expected.claim_effect(now, 100).unwrap());
                    let Some(lease) = lease else { break };
                    let image = &lease.request.continuation;
                    let got = invoke(&mut actual, image, now + 1, &context, false, false);
                    let want = invoke(&mut expected, image, now + 1, &context, false, true);
                    assert_eq!(got, want);
                    assert!(!matches!(got, Outcome::Ordinary(Step::Fault(_))), "{got:?}");
                    drained += 1;
                }
                assert!(drained < 8);
                assert_eq!(actual.pending_count(), 0);
                assert_state(&actual, &expected);
                assert_eq!(actual.allocate_sequence(), expected.allocate_sequence());
                assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 96);
}

// Frozen complete cancellation entries before product-policy extraction.
fn frozen_cancel_effect(vm: &mut Vm, image: &ContinuationImage, now_ms: u64) -> Step {
    if let Err(error) = validate_scheduler_time(now_ms) {
        return Step::Fault(error);
    }
    if let Err(error) = vm.expire_due(now_ms) {
        return Step::Fault(error);
    }
    match vm.replay_completed(&image.token) {
        Ok(Some(completed)) => return vm.visible_step(&image.token, completed),
        Ok(None) => {}
        Err(error) => return Step::Fault(error),
    }
    let Some(stored) = vm.pending.get(&image.token) else {
        return fault("LSV2004", "unknown continuation token");
    };
    if stored != image {
        return fault("LSV2005", "continuation image does not match pending state");
    }
    frozen_finish_cancellation(vm, image, CancellationReason::Requested, now_ms)
}

fn frozen_finish_cancellation(
    vm: &mut Vm,
    image: &ContinuationImage,
    reason: CancellationReason,
    observed_at_ms: u64,
) -> Step {
    let step = Step::Cancelled(Cancellation {
        continuation: image.token.clone(),
        reason,
        observed_at_ms,
    });
    let authoritative = match vm.journal.cancel(image, &step) {
        Ok(step) => step,
        Err(error) => return Step::Fault(error),
    };
    vm.pending.remove(&image.token);
    vm.completed
        .insert(image.token.clone(), authoritative.clone());
    vm.visible_step(&image.token, authoritative)
}

fn frozen_cancel_effect_audited(
    vm: &mut Vm,
    image: &ContinuationImage,
    now_ms: u64,
    audit: &DebuggerAuditContext,
) -> Result<DebuggerAuditRecord, Fault> {
    validate_debugger_audit(audit)?;
    validate_scheduler_time(now_ms)?;
    vm.expire_due(now_ms)?;
    // Reconcile peer completion/retention just like ordinary cancellation.
    if let Some(completed) = vm.reconcile_completed(&image.token)? {
        if !matches!(
            completed,
            Step::Cancelled(Cancellation {
                reason: CancellationReason::Requested,
                ..
            })
        ) {
            return Err(Fault {
                code: "LSV2014".to_string(),
                message: "completed effect cannot accept debugger cancellation audit".to_string(),
            });
        }
    } else {
        let Some(stored) = vm.pending.get(&image.token) else {
            return Err(Fault {
                code: "LSV2004".to_string(),
                message: "unknown continuation token".to_string(),
            });
        };
        if stored != image {
            return Err(Fault {
                code: "LSV2005".to_string(),
                message: "continuation image does not match pending state".to_string(),
            });
        }
    }
    let step = Step::Cancelled(Cancellation {
        continuation: image.token.clone(),
        reason: CancellationReason::Requested,
        observed_at_ms: now_ms,
    });
    let (authoritative, record) = vm.journal.cancel_audited(image, &step, audit, now_ms)?;
    if matches!(authoritative, Step::Cancelled(_)) {
        vm.pending.remove(&image.token);
        vm.completed
            .insert(image.token.clone(), authoritative.clone());
        if let Step::Fault(error) = vm.visible_step(&image.token, authoritative) {
            return Err(error);
        }
    }
    Ok(record)
}
