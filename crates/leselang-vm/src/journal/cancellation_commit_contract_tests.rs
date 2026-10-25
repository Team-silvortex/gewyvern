use super::*;

// Frozen ordinary cancellation bodies from 49648886. Native storage, input/codec
// admission and group finalization are shared, not the extracted orchestration.
impl EphemeralJournal {
    fn frozen_cancel(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        validate_cancellation_step(image, step)?;
        if let Some(dispatch) = self.dispatches.get_mut(&image.token) {
            dispatch.acknowledged = true;
            dispatch.lease_expires_at_ms = None;
            dispatch.terminal_step = Some(step.clone());
        }
        self.finalize_merge_group(&image.token)?;
        Ok(step.clone())
    }
}
#[cfg(feature = "sqlite-journal")]
impl SqliteJournal {
    fn frozen_cancel(&mut self, image: &ContinuationImage, step: &Step) -> Result<Step, Fault> {
        validate_image(image)?;
        validate_cancellation_step(image, step)?;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let step_bytes = encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                journal_error("LSV4025", "failed to lock cancellation journal", error)
            })?;
        let Some(existing) = load_record(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4013", "pending continuation is missing"));
        };
        if existing.image != image_bytes {
            return Err(journal_fault(
                "LSV4014",
                "continuation image conflicts with durable journal state",
            ));
        }
        if existing.state == "completed" {
            let authoritative: Step = decode_bounded(
                &existing.terminal_step.ok_or_else(|| {
                    journal_fault("LSV4007", "completed journal record has no terminal step")
                })?,
                MAX_JOURNAL_ENTRY_BYTES,
            )?;
            validate_terminal_step(&authoritative)?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4025", "failed to close cancellation transaction", error)
            })?;
            return Ok(authoritative);
        }
        ensure_growth(&transaction, step_bytes.len())?;
        complete_terminal_record(&transaction, image.token.as_str(), &step_bytes)?;
        finalize_merge_group(&transaction, image.token.as_str())?;
        transaction.commit().map_err(|error| {
            journal_error("LSV4025", "failed to commit effect cancellation", error)
        })?;
        Ok(step.clone())
    }
}

fn proposed(image: &ContinuationImage, now: u64) -> Step {
    Step::Cancelled(Cancellation {
        continuation: image.token.clone(),
        reason: CancellationReason::Requested,
        observed_at_ms: now,
    })
}
fn snapshot(journal: &EphemeralJournal) -> Vec<u8> {
    serde_json::to_vec(&(
        journal
            .dispatches
            .iter()
            .map(|(token, d)| {
                (
                    token,
                    &d.request,
                    d.attempt,
                    d.lease_expires_at_ms,
                    d.ready_at_ms,
                    d.retry_count,
                    &d.last_error,
                    d.acknowledged,
                    &d.terminal_step,
                )
            })
            .collect::<Vec<_>>(),
        journal
            .merge_groups
            .iter()
            .map(|(token, g)| (token, &g.plan, &g.branch_tokens, &g.terminal_step))
            .collect::<Vec<_>>(),
        journal
            .debugger_audits
            .iter()
            .map(|(command, a)| {
                (
                    command,
                    &a.token,
                    &a.idempotency_key,
                    &a.principal_id,
                    &a.record,
                )
            })
            .collect::<Vec<_>>(),
    ))
    .unwrap()
}
fn memory_fixture(
    request: &EffectRequest,
    kind: usize,
) -> (EphemeralJournal, ContinuationImage, Step) {
    let mut journal = EphemeralJournal::default();
    journal
        .record_pending(Some(request), SchedulerLimits::default())
        .unwrap();
    let mut image = request.continuation.clone();
    let mut step = proposed(&image, 200);
    match kind {
        0 => {}
        1 => {
            journal
                .frozen_cancel(&image, &proposed(&image, 30))
                .unwrap();
        }
        2 => {
            let d = journal.dispatches.get_mut(&image.token).unwrap();
            d.acknowledged = true;
            d.terminal_step = Some(Step::Fault(journal_fault("HOST", "old winner")));
        }
        3 => journal.dispatches.clear(),
        4 => image.fuel_remaining += 1,
        5 => {
            step = Step::Done(crate::Value::UiFocus {
                node_id: "not-cancellation".into(),
            })
        }
        6 => {
            let Step::Cancelled(c) = &mut step else {
                unreachable!()
            };
            c.continuation = ContinuationToken::from_wire("effect-other".into());
        }
        7 => step = proposed(&image, u64::MAX),
        8 => step = Step::Yield(image.clone()),
        9 => image.schema_version = 0,
        10 => step = deadline_cancellation(&image, 200),
        11 => {
            journal
                .dispatches
                .get_mut(&image.token)
                .unwrap()
                .acknowledged = true
        }
        12 => {
            journal
                .dispatches
                .get_mut(&image.token)
                .unwrap()
                .lease_expires_at_ms = Some(1000)
        }
        13 => {
            image.token = ContinuationToken::from_wire(String::new());
            step = proposed(&image, 200);
        }
        _ => unreachable!(),
    }
    (journal, image, step)
}
#[test]
fn memory_cancellation_matches_frozen_policy_for_all_operations_and_existing_states() {
    let mut cases = 0;
    for request in crate::continuation_contract_tests::requests() {
        for kind in 0..14 {
            let (mut actual, image, step) = memory_fixture(&request, kind);
            let (mut original, ..) = memory_fixture(&request, kind);
            let before = snapshot(&actual);
            let expected = original.frozen_cancel(&image, &step);
            let result = actual.cancel(&image, &step);
            assert_eq!(result, expected, "case {kind}");
            assert_eq!(snapshot(&actual), snapshot(&original));
            assert_eq!(result.is_ok(), !matches!(kind, 5..=8 | 13));
            if result.is_err() {
                assert_eq!(snapshot(&actual), before);
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 980);
}

fn corrupt_group(source: &str) -> (EphemeralJournal, ContinuationImage) {
    let mut vm = crate::Vm::new(500);
    let started = vm.start(
        &leselang_hir::lower(&leselang_syntax::parse(source)).unwrap(),
        crate::Principal::new("operator").unwrap(),
        crate::CapabilitySet::new(["ui.presentation"]),
        None,
    );
    assert!(
        matches!(started, Step::Effect(_) | Step::Effects(_)),
        "{started:?}"
    );
    let pending = vm.pending_continuations();
    assert_eq!(pending.len(), 2);
    let first = &pending[0];
    let second = pending[1].clone();
    let mut journal = match vm.journal {
        Journal::Ephemeral(journal) => journal,
        #[cfg(feature = "sqlite-journal")]
        Journal::Sqlite(_) => unreachable!(),
    };
    let dispatch = journal.dispatches.get_mut(&first.token).unwrap();
    dispatch.acknowledged = true;
    dispatch.terminal_step = Some(Step::Yield(first.clone()));
    (journal, second)
}
#[test]
fn native_memory_group_failure_retains_partial_dispatch_mutation_without_audit_or_retry() {
    let mut cases = 0;
    for source in [
        r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
    ] {
        let (mut actual, image) = corrupt_group(source);
        let (mut original, _) = corrupt_group(source);
        let before = snapshot(&actual);
        let step = proposed(&image, 30);
        let result = actual.cancel(&image, &step);
        assert_eq!(result, original.frozen_cancel(&image, &step));
        assert!(result.is_err());
        assert_ne!(snapshot(&actual), before);
        assert_eq!(snapshot(&actual), snapshot(&original));
        let dispatch = actual.dispatches.get(&image.token).unwrap();
        assert!(dispatch.acknowledged);
        assert_eq!(dispatch.terminal_step.as_ref(), Some(&step));
        assert!(actual.debugger_audits.is_empty());
        cases += 1;
    }
    assert_eq!(cases, 2);
}

#[cfg(feature = "sqlite-journal")]
mod durable {
    use super::super::group_recovery_contract_tests::Fixture;
    use super::*;

    const SOURCES: [&str; 5] = [
        r#"fn main() = ui.focus(node_id: "a")"#,
        r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
    ];
    const TARGETS: [(usize, usize); 9] = [
        (0, 0),
        (1, 0),
        (1, 1),
        (2, 0),
        (2, 1),
        (3, 0),
        (3, 1),
        (4, 0),
        (4, 1),
    ];

    fn snapshot(connection: &Connection) -> Vec<(String, Vec<Vec<rusqlite::types::Value>>)> {
        let mut tables = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        tables
            .query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .map(|table| {
                let table = table.unwrap();
                let mut query = connection
                    .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
                    .unwrap();
                let columns = query.column_count();
                let rows = query
                    .query_map([], |row| {
                        (0..columns)
                            .map(|i| row.get(i))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap();
                (table, rows)
            })
            .collect()
    }
    fn completed(connection: &Connection, image: &ContinuationImage, bytes: Option<Vec<u8>>) {
        connection
            .execute(
                "UPDATE vm_effects SET state='completed',terminal_step=?1 WHERE token=?2",
                params![bytes, image.token.as_str()],
            )
            .unwrap();
        connection.execute("UPDATE vm_dispatches SET state='acknowledged',lease_expires_at_ms=NULL WHERE token=?1", [image.token.as_str()]).unwrap();
    }
    fn mutate(
        journal: &mut SqliteJournal,
        image: &mut ContinuationImage,
        step: &mut Step,
        kind: usize,
    ) {
        // Bypass constraints only while constructing disposable corrupt fixtures.
        // Enforcement is restored before the production transaction starts.
        journal
            .connection
            .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        match kind {
            0 => {},
            1 => { journal.frozen_cancel(image, &proposed(image, 30)).unwrap(); },
            2 => completed(&journal.connection, image, Some(serde_json::to_vec(&Step::Done(crate::Value::UiFocus { node_id: "original".into() })).unwrap())),
            3 => completed(&journal.connection, image, Some(serde_json::to_vec(&Step::Fault(journal_fault("HOST", "original fault"))).unwrap())),
            4 => { completed(&journal.connection, image, Some(b"{".to_vec())); image.fuel_remaining += 1; },
            5 => { journal.connection.execute("DELETE FROM vm_effects WHERE token=?1", [image.token.as_str()]).unwrap(); },
            6 => completed(&journal.connection, image, None),
            7 => completed(&journal.connection, image, Some(b"{".to_vec())),
            8 => completed(&journal.connection, image, Some(serde_json::to_vec(&Step::Yield(image.clone())).unwrap())),
            9 => completed(&journal.connection, image, Some(serde_json::to_vec(&proposed(image, u64::MAX)).unwrap())),
            10 => *step = Step::Fault(journal_fault("HOST", "not a cancellation")),
            11 => { let Step::Cancelled(c) = step else { unreachable!() }; c.continuation = ContinuationToken::from_wire("other".into()); },
            12 => *step = proposed(image, u64::MAX),
            13 => image.schema_version = 0,
            14 => { image.schema_version = 0; *step = Step::Yield(image.clone()); },
            15 => journal.connection.execute_batch("CREATE TRIGGER refuse_effect BEFORE UPDATE ON vm_effects BEGIN SELECT RAISE(ABORT, 'effect refused'); END;").unwrap(),
            16 => journal.connection.execute_batch("CREATE TRIGGER refuse_dispatch BEFORE UPDATE ON vm_dispatches BEGIN SELECT RAISE(ABORT, 'dispatch refused'); END;").unwrap(),
            17 => journal.connection.execute_batch("CREATE TRIGGER refuse_group BEFORE UPDATE ON vm_merge_groups BEGIN SELECT RAISE(ABORT, 'group refused'); END;").unwrap(),
            18 => journal.connection.execute_batch("CREATE TABLE commit_parent(id INTEGER PRIMARY KEY); CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER refuse_commit AFTER UPDATE ON vm_effects BEGIN INSERT INTO commit_child VALUES(1); END;").unwrap(),
            19 => journal.connection.execute_batch("UPDATE vm_merge_groups SET plan=x'7b';").unwrap(),
            20 => { journal.connection.execute("UPDATE vm_effects SET state='unknown' WHERE token=?1", [image.token.as_str()]).unwrap(); },
            21 => *step = deadline_cancellation(image, 200),
            22 => journal.connection.execute_batch("DROP TABLE vm_effects;").unwrap(),
            23 => completed(&journal.connection, image, Some(serde_json::to_vec(&Step::Cancelled(Cancellation { continuation: ContinuationToken::from_wire("other".into()), reason: CancellationReason::Requested, observed_at_ms: 30 })).unwrap())),
            _ => unreachable!(),
        }
        journal
            .connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
    }

    #[test]
    fn sqlite_cancellation_matches_frozen_replay_refusals_writes_and_atomic_rollback() {
        let mut cases = 0;
        let mut group_write_refusals = 0;
        for (source, member) in TARGETS {
            for kind in 0..24 {
                let fixture = Fixture::new(SOURCES[source], 0);
                let vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                let original_image = vm.pending_continuations().remove(member);
                drop(vm);
                let other = fixture.0.with_extension("reference.sqlite3");
                std::fs::copy(&fixture.0, &other).unwrap();
                let mut actual = SqliteJournal {
                    connection: Connection::open(&fixture.0).unwrap(),
                };
                let mut original = SqliteJournal {
                    connection: Connection::open(&other).unwrap(),
                };
                let (mut image, mut step) =
                    (original_image.clone(), proposed(&original_image, 200));
                mutate(&mut actual, &mut image, &mut step, kind);
                mutate(
                    &mut original,
                    &mut original_image.clone(),
                    &mut proposed(&original_image, 200),
                    kind,
                );
                let before = snapshot(&actual.connection);
                assert_eq!(before, snapshot(&original.connection));
                let expected = original.frozen_cancel(&image, &step);
                let result = actual.cancel(&image, &step);
                assert_eq!(
                    result, expected,
                    "source {source}, member {member}, case {kind}"
                );
                assert_eq!(snapshot(&actual.connection), snapshot(&original.connection));
                assert_eq!(
                    actual.connection.total_changes(),
                    original.connection.total_changes()
                );
                if result.is_err() {
                    assert_eq!(snapshot(&actual.connection), before);
                }
                if matches!(kind, 0..=3 | 20 | 21 | 23) {
                    assert!(result.is_ok(), "{result:?}");
                }
                if matches!(kind, 4..=16 | 18 | 22) {
                    assert!(result.is_err());
                }
                if kind == 17 && result.is_err() {
                    group_write_refusals += 1;
                }
                if kind == 19 {
                    assert_eq!(result.is_err(), source != 0);
                }
                assert!(actual.connection.is_autocommit());
                assert!(original.connection.is_autocommit());
                assert_eq!(
                    actual
                        .connection
                        .query_row("SELECT COUNT(*) FROM vm_debugger_audit", [], |r| r
                            .get::<_, i64>(0))
                        .unwrap(),
                    0
                );
                cases += 1;
            }
        }
        assert_eq!(cases, 216);
        assert!(group_write_refusals > 0);
    }

    #[test]
    fn ordinary_cancellation_reopens_without_new_audit_or_dispatch_of_cancelled_token() {
        let mut cases = 0;
        for (source, member) in TARGETS {
            let fixture = Fixture::new(SOURCES[source], 0);
            let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
            let image = vm.pending_continuations().remove(member);
            let original = vm.cancel_effect(&image, 30);
            assert!(!matches!(original, Step::Fault(_)), "{original:?}");
            drop(vm);
            let mut reopened = crate::Vm::open_journal(&fixture.0, 500).unwrap();
            assert_eq!(reopened.cancel_effect(&image, 200), original);
            for now in 201..205 {
                if let Some(lease) = reopened.claim_effect(now, 100).unwrap() {
                    assert_ne!(lease.request.continuation.token, image.token);
                }
            }
            let connection = Connection::open(&fixture.0).unwrap();
            assert_eq!(
                connection
                    .query_row("SELECT COUNT(*) FROM vm_debugger_audit", [], |r| r
                        .get::<_, i64>(0))
                    .unwrap(),
                0
            );
            cases += 1;
        }
        assert_eq!(cases, 9);
    }
}
