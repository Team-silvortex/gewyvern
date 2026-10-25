use super::*;
use crate::{EffectErrorClass, FailedEffect, RetrySchedule};

// Frozen report_error bodies from the pre-extraction tree (HEAD 49648886).
// Native codecs/storage, lease primitives and group finalization remain shared.
impl EphemeralJournal {
    fn frozen_report_error(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        disposition: &RetryDisposition,
    ) -> Result<RetryDisposition, Fault> {
        let token = &lease.request.continuation.token;
        let Some(dispatch) = self.dispatches.get_mut(token) else {
            return Err(journal_fault("LSV4021", "dispatch lease is unknown"));
        };
        validate_ephemeral_lease(dispatch, lease, now_ms)?;
        match disposition {
            RetryDisposition::Scheduled(schedule) => {
                validate_proposed_retry(dispatch.retry_count, schedule, now_ms)?;
                dispatch.retry_count = schedule.retry_count;
                dispatch.ready_at_ms = schedule.ready_at_ms;
                dispatch.last_error = Some(schedule.error.clone());
                dispatch.lease_expires_at_ms = None;
            }
            RetryDisposition::Terminal(step) => {
                validate_terminal_step(step)?;
                dispatch.acknowledged = true;
                dispatch.lease_expires_at_ms = None;
                dispatch.terminal_step = Some(step.clone());
            }
        }
        if matches!(disposition, RetryDisposition::Terminal(_)) {
            self.finalize_merge_group(token)?;
        }
        Ok(disposition.clone())
    }
}
#[cfg(feature = "sqlite-journal")]
impl SqliteJournal {
    fn frozen_report_error(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        disposition: &RetryDisposition,
    ) -> Result<RetryDisposition, Fault> {
        validate_effect_request(&lease.request)?;
        let image = &lease.request.continuation;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4026", "failed to lock retry journal", error))?;
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
            let authoritative: Step = decode_bounded(
                &existing.terminal_step.ok_or_else(|| {
                    journal_fault("LSV4007", "completed journal record has no terminal step")
                })?,
                MAX_JOURNAL_ENTRY_BYTES,
            )?;
            validate_terminal_step(&authoritative)?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4026", "failed to close retry transaction", error)
            })?;
            return Ok(RetryDisposition::Terminal(authoritative));
        }
        let Some(dispatch) = load_dispatch(&transaction, image.token.as_str())? else {
            return Err(journal_fault("LSV4021", "dispatch lease is unknown"));
        };
        let durable_request: EffectRequest =
            decode_bounded(&dispatch.request, MAX_JOURNAL_ENTRY_BYTES)?;
        validate_stored_lease(&dispatch, lease, now_ms, durable_request == lease.request)?;

        match disposition {
            RetryDisposition::Scheduled(schedule) => {
                validate_proposed_retry(dispatch.retry_count, schedule, now_ms)?;
                let error_bytes = encode_json_capped(&schedule.error, 2_048, "effect error")?;
                let ready_at_ms = i64::try_from(schedule.ready_at_ms)
                    .map_err(|_| journal_fault("LSV4026", "retry clock is invalid"))?;
                ensure_growth(&transaction, error_bytes.len())?;
                transaction
                    .execute(
                        "UPDATE vm_dispatches
                         SET state = 'ready', retry_count = ?2, ready_at_ms = ?3,
                             last_error = ?4, lease_expires_at_ms = NULL
                         WHERE token = ?1",
                        params![
                            image.token.as_str(),
                            i64::from(schedule.retry_count),
                            ready_at_ms,
                            error_bytes
                        ],
                    )
                    .map_err(|error| {
                        journal_error("LSV4026", "failed to schedule effect retry", error)
                    })?;
            }
            RetryDisposition::Terminal(step) => {
                validate_terminal_step(step)?;
                let step_bytes =
                    encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
                ensure_growth(&transaction, step_bytes.len())?;
                complete_terminal_record(&transaction, image.token.as_str(), &step_bytes)?;
                finalize_merge_group(&transaction, image.token.as_str())?;
            }
        }
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4026", "failed to commit effect retry", error))?;
        Ok(disposition.clone())
    }
}

fn error() -> EffectError {
    EffectError {
        class: EffectErrorClass::Transient,
        code: "busy".into(),
        message: "host busy".into(),
    }
}
fn schedule() -> RetryDisposition {
    RetryDisposition::Scheduled(RetrySchedule {
        retry_count: 1,
        ready_at_ms: 20,
        error: error(),
    })
}
fn terminal() -> RetryDisposition {
    RetryDisposition::Terminal(Step::Failed(FailedEffect {
        error: error(),
        retry_count: 0,
    }))
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
) -> (EphemeralJournal, DispatchLease, u64, RetryDisposition) {
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
    let mut proposal = schedule();
    match kind {
        0 => {}
        1 => proposal = terminal(),
        2 => {
            proposal = RetryDisposition::Terminal(Step::Done(crate::Value::UiFocus {
                node_id: "old".into(),
            }))
        }
        3 => {
            proposal = RetryDisposition::Terminal(deadline_cancellation(&request.continuation, 10))
        }
        4 => proposal = RetryDisposition::Terminal(Step::Yield(request.continuation.clone())),
        5 => {
            let RetryDisposition::Scheduled(s) = &mut proposal else {
                unreachable!()
            };
            s.retry_count = 2;
        }
        6 => now = 20,
        7 => {
            let RetryDisposition::Scheduled(s) = &mut proposal else {
                unreachable!()
            };
            s.error.code.clear();
        }
        8 => {
            let RetryDisposition::Scheduled(s) = &mut proposal else {
                unreachable!()
            };
            s.ready_at_ms = u64::MAX;
        }
        9 => {
            journal.dispatches.clear();
        }
        10 => {
            dispatch.acknowledged = true;
            dispatch.terminal_step = Some(Step::Fault(journal_fault("OLD", "winner")));
        }
        11 => lease.attempt = 2,
        12 => lease.retry_count = 1,
        13 => lease.lease_expires_at_ms = 99,
        14 => now = 101,
        15 => lease.request.required_capability = "other".into(),
        16 => {
            dispatch.retry_count = 1;
            dispatch.last_error = Some(error());
            lease.retry_count = 1;
            let RetryDisposition::Scheduled(s) = &mut proposal else {
                unreachable!()
            };
            s.retry_count = 2;
        }
        17 => {
            dispatch.retry_count = u32::MAX;
            lease.retry_count = u32::MAX;
        }
        18 => dispatch.ready_at_ms = 11,
        19 => proposal = RetryDisposition::Terminal(Step::Fault(journal_fault("", "invalid"))),
        20 => {
            proposal =
                RetryDisposition::Terminal(deadline_cancellation(&request.continuation, u64::MAX))
        }
        21 => {
            lease.attempt = 0;
            proposal = RetryDisposition::Terminal(Step::Yield(request.continuation.clone()));
        }
        _ => unreachable!(),
    }
    (journal, lease, now, proposal)
}

#[test]
fn memory_retry_commit_matches_frozen_policy_for_seventy_operations_and_lease_proposals() {
    let mut cases = 0;
    for request in crate::continuation_contract_tests::requests() {
        for kind in 0..22 {
            let (mut actual, lease, now, proposed) = memory_fixture(&request, kind);
            let (mut original, ..) = memory_fixture(&request, kind);
            let before = memory_snapshot(&actual);
            let result = actual.report_error(&lease, now, &proposed);
            assert_eq!(
                result,
                original.frozen_report_error(&lease, now, &proposed),
                "case {kind}"
            );
            assert_eq!(memory_snapshot(&actual), memory_snapshot(&original));
            assert_eq!(
                result.is_ok(),
                matches!(kind, 0..=3 | 16),
                "case {kind}: {result:?}"
            );
            if result.is_err() {
                assert_eq!(memory_snapshot(&actual), before);
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 1540);
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
fn native_memory_group_failure_preserves_retry_terminal_mutation_without_rollback() {
    let mut cases = 0;
    for source in [
        r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
    ] {
        let (mut actual, lease) = corrupt_memory_group(source);
        let (mut original, _) = corrupt_memory_group(source);
        let before = memory_snapshot(&actual);
        let result = actual.report_error(&lease, 10, &terminal());
        assert_eq!(
            result,
            original.frozen_report_error(&lease, 10, &terminal())
        );
        assert!(result.is_err());
        assert_ne!(memory_snapshot(&actual), before);
        assert_eq!(memory_snapshot(&actual), memory_snapshot(&original));
        let d = actual
            .dispatches
            .get(&lease.request.continuation.token)
            .unwrap();
        assert!(d.acknowledged && d.lease_expires_at_ms.is_none() && d.terminal_step.is_some());
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
    fn completed(journal: &SqliteJournal, lease: &DispatchLease, terminal: Option<Vec<u8>>) {
        journal
            .connection
            .execute(
                "UPDATE vm_effects SET state='completed',terminal_step=?1 WHERE token=?2",
                params![terminal, lease.request.continuation.token.as_str()],
            )
            .unwrap();
    }
    fn mutate(
        journal: &mut SqliteJournal,
        lease: &mut DispatchLease,
        now: &mut u64,
        proposal: &mut RetryDisposition,
        kind: usize,
    ) {
        // Corruption is fixture-only; constraints are restored before the production call.
        journal
            .connection
            .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        match kind {
            0 => {}
            1 => *proposal = terminal(),
            2 | 30 => {
                journal
                    .frozen_report_error(
                        lease,
                        *now,
                        &RetryDisposition::Terminal(Step::Done(crate::Value::UiFocus {
                            node_id: "a".into(),
                        })),
                    )
                    .unwrap();
                if kind == 30 {
                    lease.attempt = 0;
                    *proposal =
                        RetryDisposition::Terminal(Step::Yield(lease.request.continuation.clone()));
                }
            }
            3 => completed(
                journal,
                lease,
                Some(serde_json::to_vec(&Step::Yield(lease.request.continuation.clone())).unwrap()),
            ),
            4 => completed(journal, lease, None),
            5 | 6 => {
                completed(journal, lease, Some(b"{".to_vec()));
                if kind == 6 {
                    lease.request.continuation.fuel_remaining += 1;
                }
            }
            7 => {
                journal
                    .connection
                    .execute(
                        "DELETE FROM vm_effects WHERE token=?1",
                        [lease.request.continuation.token.as_str()],
                    )
                    .unwrap();
            }
            8 => {
                journal
                    .connection
                    .execute(
                        "DELETE FROM vm_dispatches WHERE token=?1",
                        [lease.request.continuation.token.as_str()],
                    )
                    .unwrap();
            }
            9 => {
                journal
                    .connection
                    .execute(
                        "UPDATE vm_dispatches SET request=x'7b' WHERE token=?1",
                        [lease.request.continuation.token.as_str()],
                    )
                    .unwrap();
            }
            10 => {
                let request = crate::continuation_contract_tests::requests()
                    .into_iter()
                    .find(|r| *r != lease.request)
                    .unwrap();
                journal
                    .connection
                    .execute(
                        "UPDATE vm_dispatches SET request=?1 WHERE token=?2",
                        params![
                            serde_json::to_vec(&request).unwrap(),
                            lease.request.continuation.token.as_str()
                        ],
                    )
                    .unwrap();
            }
            11 => lease.attempt = 2,
            12 => *now = lease.lease_expires_at_ms + 1,
            13 => {
                let RetryDisposition::Scheduled(s) = proposal else {
                    unreachable!()
                };
                s.retry_count = 2;
            }
            14 => {
                let RetryDisposition::Scheduled(s) = proposal else {
                    unreachable!()
                };
                s.error.code.clear();
            }
            15 => {
                let RetryDisposition::Scheduled(s) = proposal else {
                    unreachable!()
                };
                s.ready_at_ms = *now;
            }
            16 => {
                journal
                    .connection
                    .execute(
                        "UPDATE vm_effects SET state='unknown' WHERE token=?1",
                        [lease.request.continuation.token.as_str()],
                    )
                    .unwrap();
            }
            17 => {
                journal
                    .connection
                    .execute(
                        "UPDATE vm_dispatches SET state='acknowledged' WHERE token=?1",
                        [lease.request.continuation.token.as_str()],
                    )
                    .unwrap();
            }
            18 => {
                journal
                    .connection
                    .execute(
                        "UPDATE vm_dispatches SET retry_count=1,last_error=NULL WHERE token=?1",
                        [lease.request.continuation.token.as_str()],
                    )
                    .unwrap();
            }
            19 => lease.request.continuation.schema_version = 0,
            20 => {
                *proposal =
                    RetryDisposition::Terminal(Step::Yield(lease.request.continuation.clone()))
            }
            21 | 33 => {
                if kind == 33 {
                    *proposal = terminal();
                }
                journal.connection.execute_batch("CREATE TRIGGER refuse_dispatch BEFORE UPDATE ON vm_dispatches BEGIN SELECT RAISE(ABORT, 'dispatch refused'); END;").unwrap();
            }
            22 => {
                *proposal = terminal();
                journal.connection.execute_batch("CREATE TRIGGER refuse_effect BEFORE UPDATE ON vm_effects BEGIN SELECT RAISE(ABORT, 'effect refused'); END;").unwrap();
            }
            23 => {
                *proposal = terminal();
                journal.connection.execute_batch("CREATE TRIGGER refuse_group BEFORE UPDATE ON vm_merge_groups BEGIN SELECT RAISE(ABORT, 'group refused'); END;").unwrap();
            }
            24 | 32 => {
                if kind == 32 {
                    *proposal = terminal();
                }
                journal.connection.execute_batch("CREATE TABLE commit_parent(id INTEGER PRIMARY KEY); CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER refuse_commit AFTER UPDATE ON vm_dispatches BEGIN INSERT INTO commit_child VALUES(1); END;").unwrap();
            }
            25 => {
                *proposal = terminal();
                journal
                    .connection
                    .execute_batch("UPDATE vm_merge_groups SET plan=x'7b';")
                    .unwrap();
            }
            26 => journal
                .connection
                .execute_batch("DROP TABLE vm_effects;")
                .unwrap(),
            27 => {
                *proposal = RetryDisposition::Terminal(Step::Done(crate::Value::UiFocus {
                    node_id: "a".into(),
                }))
            }
            28 => {
                let RetryDisposition::Scheduled(s) = proposal else {
                    unreachable!()
                };
                s.ready_at_ms = u64::MAX;
            }
            29 => {
                journal
                    .connection
                    .execute(
                        "UPDATE vm_dispatches SET attempt=2 WHERE token=?1",
                        [lease.request.continuation.token.as_str()],
                    )
                    .unwrap();
            }
            31 => {
                lease.attempt = 0;
                *proposal = RetryDisposition::Terminal(Step::Fault(journal_fault("", "invalid")));
            }
            _ => unreachable!(),
        }
        journal
            .connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
    }
    #[test]
    fn sqlite_retry_commit_matches_frozen_lease_replay_write_order_and_rollback() {
        let mut cases = 0;
        let mut group_failures = 0;
        for (source_index, source) in SOURCES.iter().enumerate() {
            for kind in 0..34 {
                let fixture = Fixture::new(source, 0);
                let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                let original_lease = vm.claim_effect(1, 100).unwrap().unwrap();
                drop(vm);
                let other = fixture.0.with_extension("reference.sqlite3");
                std::fs::copy(&fixture.0, &other).unwrap();
                let mut actual = SqliteJournal {
                    connection: Connection::open(&fixture.0).unwrap(),
                };
                let mut original = SqliteJournal {
                    connection: Connection::open(&other).unwrap(),
                };
                let (mut lease, mut now, mut proposed) = (original_lease.clone(), 10, schedule());
                mutate(&mut actual, &mut lease, &mut now, &mut proposed, kind);
                mutate(
                    &mut original,
                    &mut original_lease.clone(),
                    &mut 10,
                    &mut schedule(),
                    kind,
                );
                let before = snapshot(&actual.connection);
                assert_eq!(before, snapshot(&original.connection));
                let result = actual.report_error(&lease, now, &proposed);
                assert_eq!(
                    result,
                    original.frozen_report_error(&lease, now, &proposed),
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
                if matches!(kind, 0..=2 | 16 | 27 | 30) {
                    assert!(result.is_ok(), "{result:?}");
                }
                if matches!(kind, 3..=15 | 17..=22 | 24 | 26 | 28 | 29 | 31..=33) {
                    assert!(result.is_err(), "case {kind}");
                }
                if kind == 10 {
                    assert_eq!(result.as_ref().unwrap_err().code, "LSV4022");
                }
                if kind == 23 && result.is_err() {
                    group_failures += 1;
                }
                if kind == 25 {
                    assert_eq!(result.is_err(), source_index != 0);
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
        assert_eq!(cases, 170);
        assert!(group_failures > 0);
    }
    #[test]
    fn retry_reopens_with_original_counters_and_rejects_old_lease_without_duplicate_work() {
        let mut cases = 0;
        for source in SOURCES {
            for stop in [false, true] {
                let fixture = Fixture::new(source, 0);
                let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                let lease = vm.claim_effect(1, 100).unwrap().unwrap();
                let mut failure = error();
                if stop {
                    failure.class = EffectErrorClass::Permanent;
                }
                let policy = crate::RetryPolicy {
                    max_retries: 3,
                    base_delay_ms: 10,
                    max_delay_ms: 10,
                };
                let outcome = vm
                    .report_effect_error(&lease, 10, failure.clone(), &policy)
                    .unwrap();
                drop(vm);
                let mut reopened = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                if stop {
                    assert_eq!(
                        reopened
                            .report_effect_error(&lease, 11, failure, &policy)
                            .unwrap(),
                        outcome
                    );
                    for _ in 0..3 {
                        if let Some(next) = reopened.claim_effect(20, 100).unwrap() {
                            assert_ne!(
                                next.request.continuation.token,
                                lease.request.continuation.token
                            );
                        }
                    }
                } else {
                    assert_eq!(
                        reopened
                            .report_effect_error(&lease, 11, failure, &policy)
                            .unwrap_err()
                            .code,
                        "LSV4022"
                    );
                    if let Some(next) = reopened.claim_effect(19, 100).unwrap() {
                        assert_ne!(
                            next.request.continuation.token,
                            lease.request.continuation.token
                        );
                    }
                    let mut found = false;
                    for _ in 0..3 {
                        if let Some(next) = reopened.claim_effect(20, 100).unwrap()
                            && next.request.continuation.token == lease.request.continuation.token
                        {
                            assert!(!found);
                            found = true;
                            assert_eq!((next.retry_count, next.attempt), (1, 2));
                        }
                    }
                    assert!(found);
                }
                cases += 1;
            }
        }
        assert_eq!(cases, 10);
    }
}
