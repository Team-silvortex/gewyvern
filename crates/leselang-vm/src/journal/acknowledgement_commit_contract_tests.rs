use super::*;

// Frozen acknowledgement bodies from the pre-extraction tree (HEAD 49648886).
// Native codecs/storage, lease and group primitives remain shared, not policy.
impl EphemeralJournal {
    fn frozen_acknowledge_dispatch(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        step: &Step,
    ) -> Result<Step, Fault> {
        validate_terminal_step(step)?;
        let token = &lease.request.continuation.token;
        let Some(dispatch) = self.dispatches.get_mut(token) else {
            return Err(journal_fault("LSV4021", "dispatch lease is unknown"));
        };
        if dispatch.acknowledged {
            return Ok(step.clone());
        }
        validate_ephemeral_lease(dispatch, lease, now_ms)?;
        dispatch.acknowledged = true;
        dispatch.lease_expires_at_ms = None;
        dispatch.terminal_step = Some(step.clone());
        self.finalize_merge_group(token)?;
        Ok(step.clone())
    }
}
#[cfg(feature = "sqlite-journal")]
impl SqliteJournal {
    fn frozen_acknowledge_dispatch(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        step: &Step,
    ) -> Result<Step, Fault> {
        validate_effect_request(&lease.request)?;
        validate_terminal_step(step)?;
        if lease.attempt == 0 || lease.attempt > MAX_DISPATCH_ATTEMPTS {
            return Err(journal_fault("LSV4022", "dispatch attempt is invalid"));
        }
        let image = &lease.request.continuation;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        encode_json_capped(&lease.request, MAX_JOURNAL_ENTRY_BYTES, "effect request")?;
        let step_bytes = encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4020", "failed to lock dispatch journal", error))?;
        let Some(existing) = load_record(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4021", "dispatch continuation is unknown"));
        };
        if existing.image != image_bytes {
            return Err(journal_fault(
                "LSV4014",
                "dispatch continuation conflicts with durable journal state",
            ));
        }
        if existing.state == "completed" {
            let bytes = existing.terminal_step.ok_or_else(|| {
                journal_fault("LSV4007", "completed journal record has no terminal step")
            })?;
            let authoritative: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
            validate_terminal_step(&authoritative)?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4020", "failed to close dispatch transaction", error)
            })?;
            return Ok(authoritative);
        }
        let Some(dispatch) = load_dispatch(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4021", "dispatch lease is unknown"));
        };
        let durable_request: EffectRequest =
            decode_bounded(&dispatch.request, MAX_JOURNAL_ENTRY_BYTES)?;
        validate_stored_lease(&dispatch, lease, now_ms, durable_request == lease.request)?;
        ensure_growth(&transaction, step_bytes.len())?;
        transaction
            .execute(
                "UPDATE vm_effects SET state = 'completed', terminal_step = ?2 WHERE token = ?1",
                params![image.token.as_str(), step_bytes],
            )
            .map_err(|error| journal_error("LSV4020", "failed to complete dispatch", error))?;
        transaction
            .execute(
                "UPDATE vm_dispatches
                 SET state = 'acknowledged', lease_expires_at_ms = NULL
                 WHERE token = ?1",
                [image.token.as_str()],
            )
            .map_err(|error| journal_error("LSV4020", "failed to acknowledge dispatch", error))?;
        finalize_merge_group(&transaction, image.token.as_str())?;
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4020", "failed to commit dispatch result", error))?;
        Ok(step.clone())
    }
}

fn done(node: &str) -> Step {
    Step::Done(crate::Value::UiFocus {
        node_id: node.into(),
    })
}
fn reply(wrong: bool) -> crate::EffectResult {
    crate::EffectResult::Presentation(if wrong {
        crate::PresentationResult::Activate {
            node_id: "a".into(),
        }
    } else {
        crate::PresentationResult::Focus {
            node_id: "a".into(),
        }
    })
}
fn memory_snapshot(journal: &EphemeralJournal) -> Vec<u8> {
    serde_json::to_vec(&(
        journal
            .dispatches
            .iter()
            .map(|(t, d)| {
                (
                    t,
                    &d.request,
                    d.attempt,
                    d.retry_count,
                    d.ready_at_ms,
                    d.lease_expires_at_ms,
                    &d.last_error,
                    d.acknowledged,
                    &d.terminal_step,
                )
            })
            .collect::<Vec<_>>(),
        journal
            .merge_groups
            .iter()
            .map(|(t, g)| (t, &g.plan, &g.branch_tokens, &g.terminal_step))
            .collect::<Vec<_>>(),
        journal.debugger_audits.len(),
    ))
    .unwrap()
}
fn memory_fixture(
    request: &EffectRequest,
    kind: usize,
) -> (EphemeralJournal, DispatchLease, u64, Step) {
    let mut journal = EphemeralJournal::default();
    journal
        .record_pending(Some(request), SchedulerLimits::default())
        .unwrap();
    let dispatch = journal
        .dispatches
        .get_mut(&request.continuation.token)
        .unwrap();
    dispatch.attempt = 1;
    dispatch.lease_expires_at_ms = Some(100);
    let mut lease = DispatchLease {
        request: request.clone(),
        attempt: 1,
        retry_count: 0,
        lease_expires_at_ms: 100,
    };
    let mut now = 10;
    let mut proposal = done("new");
    match kind {
        0 => {}
        1 => proposal = Step::Fault(journal_fault("HOST", "failed")),
        2 => proposal = deadline_cancellation(&request.continuation, 10),
        3 => proposal = Step::Yield(request.continuation.clone()),
        4 => proposal = Step::Fault(journal_fault("", "invalid")),
        5 => journal.dispatches.clear(),
        6 => {
            dispatch.acknowledged = true;
            dispatch.terminal_step = Some(done("old"));
        }
        7 => {
            dispatch.acknowledged = true;
            dispatch.terminal_step = None;
            lease.attempt = 0;
            now = u64::MAX;
        }
        8 => lease.attempt = 2,
        9 => now = 101,
        10 => now = 100,
        11 => lease.retry_count = 1,
        12 => lease.lease_expires_at_ms = 99,
        13 => dispatch.ready_at_ms = 11,
        14 => lease.request.required_capability = "other".into(),
        15 | 16 => {
            let attempt = if kind == 15 {
                0
            } else {
                MAX_DISPATCH_ATTEMPTS + 1
            };
            dispatch.attempt = attempt;
            lease.attempt = attempt;
        }
        17 => {
            journal.dispatches.clear();
            proposal = Step::Yield(request.continuation.clone());
        }
        18 => {
            dispatch.acknowledged = true;
            now = u64::MAX;
            proposal = Step::Yield(request.continuation.clone());
        }
        19 => {
            dispatch.acknowledged = true;
            dispatch.terminal_step = Some(Step::Yield(request.continuation.clone()));
            lease.request.required_capability = "other".into();
        }
        _ => unreachable!(),
    }
    (journal, lease, now, proposal)
}
#[test]
fn memory_acknowledgement_matches_frozen_policy_for_all_operations_and_completed_states() {
    let mut cases = 0;
    for request in crate::continuation_contract_tests::requests() {
        for kind in 0..20 {
            let (mut actual, lease, now, proposed) = memory_fixture(&request, kind);
            let (mut original, ..) = memory_fixture(&request, kind);
            let before = memory_snapshot(&actual);
            let result = actual.acknowledge_dispatch(&lease, now, &proposed);
            assert_eq!(
                result,
                original.frozen_acknowledge_dispatch(&lease, now, &proposed),
                "case {kind}"
            );
            assert_eq!(memory_snapshot(&actual), memory_snapshot(&original));
            assert_eq!(
                result.is_ok(),
                matches!(kind, 0..=2 | 6 | 7 | 10 | 15 | 16 | 19),
                "{kind}: {result:?}"
            );
            if result.is_err() || matches!(kind, 6 | 7 | 19) {
                assert_eq!(memory_snapshot(&actual), before);
            }
            if matches!(kind, 17 | 18) {
                assert_eq!(result.unwrap_err().code, "LSV4007");
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 1400);
}
fn corrupt_memory_group(source: &str) -> (EphemeralJournal, DispatchLease) {
    let mut vm = crate::Vm::new(500);
    assert!(matches!(
        vm.start(
            &leselang_hir::lower(&leselang_syntax::parse(source)).unwrap(),
            crate::Principal::new("operator").unwrap(),
            crate::CapabilitySet::new(["ui.presentation"]),
            None
        ),
        Step::Effect(_) | Step::Effects(_)
    ));
    let pending = vm.pending_continuations();
    assert_eq!(pending.len(), 2);
    let mut journal = match vm.journal {
        Journal::Ephemeral(j) => j,
        #[cfg(feature = "sqlite-journal")]
        Journal::Sqlite(_) => unreachable!(),
    };
    let first = journal.dispatches.get_mut(&pending[0].token).unwrap();
    first.acknowledged = true;
    first.terminal_step = Some(Step::Yield(pending[0].clone()));
    let second = journal.dispatches.get_mut(&pending[1].token).unwrap();
    second.attempt = 1;
    second.lease_expires_at_ms = Some(100);
    let lease = DispatchLease {
        request: second.request.clone(),
        attempt: 1,
        retry_count: 0,
        lease_expires_at_ms: 100,
    };
    (journal, lease)
}
#[test]
fn native_memory_finalization_failure_retains_acknowledged_terminal_without_rollback() {
    let mut cases = 0;
    for source in [
        r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
    ] {
        let (mut actual, lease) = corrupt_memory_group(source);
        let (mut original, _) = corrupt_memory_group(source);
        let before = memory_snapshot(&actual);
        let result = actual.acknowledge_dispatch(&lease, 10, &done("b"));
        assert_eq!(
            result,
            original.frozen_acknowledge_dispatch(&lease, 10, &done("b"))
        );
        assert!(result.is_err());
        assert_ne!(memory_snapshot(&actual), before);
        assert_eq!(memory_snapshot(&actual), memory_snapshot(&original));
        let d = actual
            .dispatches
            .get(&lease.request.continuation.token)
            .unwrap();
        assert!(d.acknowledged && d.lease_expires_at_ms.is_none());
        assert_eq!(d.terminal_step, Some(done("b")));
        cases += 1;
    }
    assert_eq!(cases, 2);
}
#[test]
fn public_memory_acknowledgement_replays_winner_despite_low_level_proposal_semantics() {
    let mut cases = 0;
    for wrong in [false, true] {
        let mut vm = crate::Vm::new(500);
        vm.start(
            &leselang_hir::lower(&leselang_syntax::parse(
                r#"fn main() = ui.focus(node_id: "a")"#,
            ))
            .unwrap(),
            crate::Principal::new("operator").unwrap(),
            crate::CapabilitySet::new(["ui.presentation"]),
            None,
        );
        let lease = vm.claim_effect(1, 100).unwrap().unwrap();
        let original = vm.acknowledge_effect(&lease, 10, reply(wrong));
        assert_eq!(matches!(original, Step::Fault(_)), wrong);
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.acknowledge_effect(&lease, 11, reply(!wrong)), original);
        assert!(vm.claim_effect(12, 100).unwrap().is_none());
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
    fn snapshot(connection: &Connection) -> Vec<(String, Vec<Vec<rusqlite::types::Value>>)> {
        let mut tables = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        tables
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|table| {
                let table = table.unwrap();
                let mut query = connection
                    .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
                    .unwrap();
                let columns = query.column_count();
                let rows = query
                    .query_map([], |r| {
                        (0..columns)
                            .map(|i| r.get(i))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap();
                (table, rows)
            })
            .collect()
    }
    fn completed(journal: &SqliteJournal, lease: &DispatchLease, bytes: Option<Vec<u8>>) {
        journal
            .connection
            .execute(
                "UPDATE vm_effects SET state='completed',terminal_step=?1 WHERE token=?2",
                params![bytes, lease.request.continuation.token.as_str()],
            )
            .unwrap();
    }
    fn mutate(
        journal: &mut SqliteJournal,
        lease: &mut DispatchLease,
        now: &mut u64,
        step: &mut Step,
        kind: usize,
    ) {
        // Only disposable fixtures bypass constraints; production calls restore them.
        journal
            .connection
            .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        match kind {
            0 => {}, 1 => *step = Step::Fault(journal_fault("HOST", "failed")),
            2 => *step = deadline_cancellation(&lease.request.continuation, 10),
            3 | 28..=30 | 34 => { journal.frozen_acknowledge_dispatch(lease, *now, &done("old")).unwrap(); match kind { 28 => *step = Step::Yield(lease.request.continuation.clone()), 29 => lease.attempt = 0, 30 => { lease.attempt = 2; *now = u64::MAX; }, 34 => lease.request.continuation.schema_version = 0, _ => {} } },
            4 => completed(journal, lease, None),
            5 => completed(journal, lease, Some(b"{".to_vec())),
            6 => completed(journal, lease, Some(serde_json::to_vec(&Step::Yield(lease.request.continuation.clone())).unwrap())),
            7 => { lease.request.continuation.fuel_remaining += 1; lease.request.budget = lease.request.continuation.resource_budget(); },
            8 => { journal.connection.execute("DELETE FROM vm_effects WHERE token=?1", [lease.request.continuation.token.as_str()]).unwrap(); },
            9 => { journal.connection.execute("DELETE FROM vm_dispatches WHERE token=?1", [lease.request.continuation.token.as_str()]).unwrap(); },
            10 => { journal.connection.execute("UPDATE vm_dispatches SET request=x'7b' WHERE token=?1", [lease.request.continuation.token.as_str()]).unwrap(); },
            11 => { let request = crate::continuation_contract_tests::requests().into_iter().find(|r| *r != lease.request).unwrap(); journal.connection.execute("UPDATE vm_dispatches SET request=?1 WHERE token=?2", params![serde_json::to_vec(&request).unwrap(), lease.request.continuation.token.as_str()]).unwrap(); },
            12 => lease.attempt = 2, 13 => lease.attempt = 0, 14 => lease.attempt = MAX_DISPATCH_ATTEMPTS + 1,
            15 => *now = lease.lease_expires_at_ms + 1, 16 => *now = lease.lease_expires_at_ms,
            17 => lease.retry_count = 1, 18 => lease.request.continuation.schema_version = 0,
            19 | 20 => { *step = Step::Yield(lease.request.continuation.clone()); if kind == 20 { lease.attempt = 0; } },
            21 => journal.connection.execute_batch("CREATE TRIGGER refuse_effect BEFORE UPDATE ON vm_effects BEGIN SELECT RAISE(ABORT, 'effect refused'); END;").unwrap(),
            22 => journal.connection.execute_batch("CREATE TRIGGER refuse_dispatch BEFORE UPDATE ON vm_dispatches BEGIN SELECT RAISE(ABORT, 'dispatch refused'); END;").unwrap(),
            23 => { *step = Step::Fault(journal_fault("HOST", "stop sequence")); journal.connection.execute_batch("CREATE TRIGGER refuse_group BEFORE UPDATE ON vm_merge_groups BEGIN SELECT RAISE(ABORT, 'group refused'); END;").unwrap(); },
            24 => journal.connection.execute_batch("CREATE TABLE commit_parent(id INTEGER PRIMARY KEY); CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER refuse_commit AFTER UPDATE ON vm_dispatches BEGIN INSERT INTO commit_child VALUES(1); END;").unwrap(),
            25 => journal.connection.execute_batch("UPDATE vm_merge_groups SET plan=x'7b';").unwrap(),
            26 => journal.connection.execute_batch("DROP TABLE vm_effects;").unwrap(),
            27 => { journal.connection.execute("UPDATE vm_effects SET state='unknown' WHERE token=?1", [lease.request.continuation.token.as_str()]).unwrap(); },
            31 => { journal.connection.execute("UPDATE vm_dispatches SET ready_at_ms=11 WHERE token=?1", [lease.request.continuation.token.as_str()]).unwrap(); },
            32 => { journal.connection.execute("UPDATE vm_dispatches SET retry_count=1,last_error=NULL WHERE token=?1", [lease.request.continuation.token.as_str()]).unwrap(); },
            33 => completed(journal, lease, Some(serde_json::to_vec(&Step::Fault(journal_fault("", "invalid"))).unwrap())),
            35 => completed(journal, lease, Some(vec![b' '; MAX_JOURNAL_ENTRY_BYTES + 1])),
            _ => unreachable!(),
        }
        journal
            .connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
    }
    #[test]
    fn sqlite_acknowledgement_matches_frozen_admission_replay_and_transaction_rollback() {
        let mut cases = 0;
        let mut group_failures = 0;
        for (source_index, source) in SOURCES.iter().enumerate() {
            for kind in 0..36 {
                let fixture = Fixture::new(source, 0);
                let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                let initial_lease = vm.claim_effect(1, 100).unwrap().unwrap();
                drop(vm);
                let other = fixture.0.with_extension("reference.sqlite3");
                std::fs::copy(&fixture.0, &other).unwrap();
                let mut actual = SqliteJournal {
                    connection: Connection::open(&fixture.0).unwrap(),
                };
                let mut original = SqliteJournal {
                    connection: Connection::open(&other).unwrap(),
                };
                let (mut lease, mut now, mut step) = (initial_lease.clone(), 10, done("new"));
                mutate(&mut actual, &mut lease, &mut now, &mut step, kind);
                mutate(
                    &mut original,
                    &mut initial_lease.clone(),
                    &mut 10,
                    &mut done("new"),
                    kind,
                );
                let before = snapshot(&actual.connection);
                assert_eq!(before, snapshot(&original.connection));
                let result = actual.acknowledge_dispatch(&lease, now, &step);
                assert_eq!(
                    result,
                    original.frozen_acknowledge_dispatch(&lease, now, &step),
                    "source {source_index}, case {kind}"
                );
                assert_eq!(snapshot(&actual.connection), snapshot(&original.connection));
                assert_eq!(
                    actual.connection.total_changes(),
                    original.connection.total_changes()
                );
                if result.is_err() {
                    assert_eq!(snapshot(&actual.connection), before);
                }
                if matches!(kind, 0..=3 | 16 | 27 | 30) {
                    assert!(result.is_ok(), "{result:?}");
                }
                if matches!(kind, 4..=15 | 17..=22 | 24 | 26 | 28 | 29 | 31..=35) {
                    assert!(result.is_err(), "case {kind}");
                }
                if kind == 23 && result.is_err() {
                    group_failures += 1;
                }
                if kind == 25 {
                    assert_eq!(result.is_err(), source_index != 0);
                }
                if kind == 7 {
                    assert_eq!(result.as_ref().unwrap_err().code, "LSV4014");
                }
                if kind == 11 {
                    assert_eq!(result.as_ref().unwrap_err().code, "LSV4022");
                }
                if matches!(kind, 20 | 28) {
                    assert_eq!(result.as_ref().unwrap_err().code, "LSV4007");
                }
                if kind == 29 {
                    assert_eq!(result.as_ref().unwrap_err().code, "LSV4022");
                }
                assert!(actual.connection.is_autocommit() && original.connection.is_autocommit());
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
        assert_eq!(cases, 180);
        assert!(group_failures > 0);
    }
    #[test]
    fn acknowledgement_reopens_and_replays_committed_success_or_fault_without_redispatch() {
        let mut cases = 0;
        for source in SOURCES {
            for wrong in [false, true] {
                let fixture = Fixture::new(source, 0);
                let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                let lease = vm.claim_effect(1, 100).unwrap().unwrap();
                let original = vm.acknowledge_effect(&lease, 10, reply(wrong));
                // Group visibility can hide one member's fault until its barrier closes.
                let bytes: Vec<u8> = Connection::open(&fixture.0)
                    .unwrap()
                    .query_row(
                        "SELECT terminal_step FROM vm_effects WHERE token=?1",
                        [lease.request.continuation.token.as_str()],
                        |r| r.get(0),
                    )
                    .unwrap();
                let terminal: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES).unwrap();
                assert_eq!(matches!(terminal, Step::Fault(_)), wrong);
                drop(vm);
                let mut reopened = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                assert_eq!(
                    reopened.acknowledge_effect(&lease, 11, reply(!wrong)),
                    original
                );
                for _ in 0..4 {
                    if let Some(next) = reopened.claim_effect(20, 100).unwrap() {
                        assert_ne!(
                            next.request.continuation.token,
                            lease.request.continuation.token
                        );
                    }
                }
                cases += 1;
            }
        }
        assert_eq!(cases, 10);
    }
}
