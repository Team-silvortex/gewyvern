use super::*;
fn request(n: u64) -> EffectRequest {
    crate::Vm::build_effect_request(
        &leselang_hir::Effect::UiFocus {
            node_id: "node".into(),
        },
        leselang_hir::Type::UiFocus,
        n,
        crate::Principal::new("operator").unwrap(),
        crate::CapabilitySet::new(["ui.presentation"]),
        None,
        1000,
        None,
        100,
    )
    .unwrap()
}
fn done() -> Step {
    Step::Done(crate::Value::UiFocus {
        node_id: "node".into(),
    })
}
fn snapshot_memory(j: &EphemeralJournal) -> Vec<u8> {
    serde_json::to_vec(&(
        j.dispatches
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
        j.merge_groups
            .iter()
            .map(|(t, g)| (t, &g.plan, &g.branch_tokens, &g.terminal_step))
            .collect::<Vec<_>>(),
        j.debugger_audits.len(),
    ))
    .unwrap()
}
fn memory_fixture(input: &EffectRequest, kind: usize) -> (EphemeralJournal, SchedulerLimits) {
    let mut j = EphemeralJournal::default();
    let mut limits = SchedulerLimits::default();
    if kind == 0 {
        return (j, limits);
    }
    j.record_pending(Some(input), limits).unwrap();
    if matches!(kind, 8 | 12..=16) {
        j.record_pending(Some(&request(200)), limits).unwrap();
    }
    let d = j.dispatches.get_mut(&input.continuation.token).unwrap();
    match kind {
        1 => {}
        2 => {
            d.acknowledged = true;
            d.terminal_step = Some(done());
        }
        3 => d.ready_at_ms = 11,
        4 | 9 => {
            d.attempt = 1;
            d.lease_expires_at_ms = Some(11);
            limits.max_active_leases = 1;
        }
        5 => {
            d.attempt = 1;
            d.lease_expires_at_ms = Some(10);
        }
        6 => d.ready_at_ms = 10,
        7 => {
            d.attempt = 3;
            d.lease_expires_at_ms = Some(1);
        }
        8 => limits.max_active_leases = 1,
        10 => d.attempt = 100,
        11 => d.attempt = u32::MAX,
        12 => d.attempt = 2,
        13..=16 => {}
        17 => d.lease_expires_at_ms = Some(0),
        18 => d.request.required_capability = "invalid".into(),
        19 => d.retry_count = u32::MAX,
        _ => unreachable!(),
    }
    if kind == 8 {
        j.dispatches
            .get_mut(&request(200).continuation.token)
            .unwrap()
            .lease_expires_at_ms = Some(11);
    }
    if (13..=16).contains(&kind) {
        let prev = j
            .dispatches
            .get_mut(&request(200).continuation.token)
            .unwrap();
        prev.lease_expires_at_ms = Some(11);
        if kind != 13 {
            prev.acknowledged = true;
            prev.terminal_step = Some(if kind == 15 {
                Step::Fault(Fault {
                    code: "fail".into(),
                    message: "prior".into(),
                })
            } else {
                done()
            });
        }
        let mut plan = MergePlan::new(["prior", "next"]).unwrap();
        plan.order = ExecutionOrder::Sequential;
        j.merge_groups.insert(
            ContinuationToken::from_wire("merge-300".into()),
            EphemeralMergeGroup {
                plan,
                branch_tokens: vec![
                    request(200).continuation.token,
                    input.continuation.token.clone(),
                ],
                terminal_step: (kind == 16).then(done),
            },
        );
    }
    (j, limits)
}
#[test]
fn memory_claim_matches_frozen_selection_pressure_graph_gates_and_counter_failures_for_all_operations()
 {
    let mut cases = 0;
    for input in crate::continuation_contract_tests::requests() {
        for kind in 0..20 {
            let (mut actual, limits) = memory_fixture(&input, kind);
            let (mut original, _) = memory_fixture(&input, kind);
            let before = snapshot_memory(&actual);
            let result = actual.claim_dispatch(10, 20, limits);
            assert_eq!(
                result,
                original.frozen_claim_dispatch(10, 20, limits),
                "case {kind}"
            );
            assert_eq!(snapshot_memory(&actual), snapshot_memory(&original));
            if !matches!(result, Ok(DispatchClaim::Leased(_))) {
                assert_eq!(snapshot_memory(&actual), before);
            }
            if matches!(kind, 0 | 2..=4 | 9 | 13 | 15 | 16) {
                assert!(
                    matches!(result, Ok(DispatchClaim::Idle)),
                    "case {kind}: {result:?}"
                );
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 1400);
}
#[test]
fn public_memory_claim_clock_refusals_leave_dispatch_state_unchanged() {
    let mut cases = 0;
    for (now, duration) in [
        (u64::MAX, 1),
        (1, 0),
        (1, MAX_DISPATCH_LEASE_MS + 1),
        (i64::MAX as u64, 1),
    ] {
        let (j, limits) = memory_fixture(&request(7), 1);
        let before = snapshot_memory(&j);
        let mut journal = Journal::Ephemeral(j);
        assert!(journal.claim_dispatch(now, duration, limits).is_err());
        #[cfg(feature = "sqlite-journal")]
        let Journal::Ephemeral(j) = journal else {
            unreachable!()
        };
        #[cfg(not(feature = "sqlite-journal"))]
        let Journal::Ephemeral(j) = journal;
        assert_eq!(snapshot_memory(&j), before);
        cases += 1;
    }
    assert_eq!(cases, 4);
}

#[cfg(feature = "sqlite-journal")]
mod durable {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let dir = std::env::temp_dir().join(format!(
                "leselang-claim-commit-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&dir).unwrap();
            let f = Self(dir.join("journal.sqlite3"));
            drop(SqliteJournal::open(&f.0).unwrap());
            f
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
        }
    }
    fn snapshot(c: &Connection) -> Vec<(String, Vec<Vec<rusqlite::types::Value>>)> {
        let tables = c
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        tables
            .into_iter()
            .map(|t| {
                let mut q = c
                    .prepare(&format!("SELECT * FROM {t} ORDER BY rowid"))
                    .unwrap();
                let n = q.column_count();
                let rows = q
                    .query_map([], |r| {
                        (0..n)
                            .map(|i| r.get(i))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap()
                    .collect::<rusqlite::Result<_>>()
                    .unwrap();
                (t, rows)
            })
            .collect()
    }
    fn mutate(
        j: &mut SqliteJournal,
        input: &EffectRequest,
        kind: usize,
    ) -> (u64, u64, SchedulerLimits) {
        let (mut now, mut expires) = (10, 20);
        let mut limits = SchedulerLimits::default();
        if kind != 0 {
            j.record_pending(&input.continuation, Some(input), limits)
                .unwrap();
        }
        if matches!(kind, 21 | 26..=30) {
            let other = request(200);
            j.record_pending(&other.continuation, Some(&other), limits)
                .unwrap();
        }
        j.connection
            .execute_batch("PRAGMA foreign_keys=OFF;PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        let t = input.continuation.token.as_str();
        match kind {
            0 | 1 => {}
            2 => {
                j.connection
                    .execute(
                        "UPDATE vm_effects SET state='completed',terminal_step=?1",
                        [serde_json::to_vec(&done()).unwrap()],
                    )
                    .unwrap();
                j.connection
                    .execute_batch("UPDATE vm_dispatches SET state='acknowledged';")
                    .unwrap();
            }
            3 => {
                j.connection
                    .execute_batch("UPDATE vm_dispatches SET ready_at_ms=11;")
                    .unwrap();
            }
            4 | 22 => {
                j.connection
                    .execute_batch(
                        "UPDATE vm_dispatches SET state='leased',attempt=1,lease_expires_at_ms=11;",
                    )
                    .unwrap();
                limits.max_active_leases = 1;
            }
            5 => {
                j.connection
                    .execute_batch(
                        "UPDATE vm_dispatches SET state='leased',attempt=1,lease_expires_at_ms=10;",
                    )
                    .unwrap();
            }
            6 => {
                j.connection
                    .execute_batch(
                        "UPDATE vm_dispatches SET state='leased',attempt=3,lease_expires_at_ms=1;",
                    )
                    .unwrap();
            }
            7 => {
                j.connection
                    .execute_batch("UPDATE vm_dispatches SET request=x'7b';")
                    .unwrap();
            }
            8 => {
                j.connection
                    .execute(
                        "UPDATE vm_dispatches SET request=?1",
                        [serde_json::to_vec(&request(201)).unwrap()],
                    )
                    .unwrap();
            }
            9 => {
                j.connection
                    .execute_batch("UPDATE vm_effects SET image=x'7b';")
                    .unwrap();
            }
            10 => {
                j.connection
                    .execute_batch("UPDATE vm_dispatches SET attempt=-1;")
                    .unwrap();
            }
            11 => {
                j.connection
                    .execute_batch("UPDATE vm_dispatches SET attempt=9223372036854775807;")
                    .unwrap();
            }
            12 => {
                j.connection
                    .execute_batch("UPDATE vm_dispatches SET attempt=100,retry_count=-1;")
                    .unwrap();
            }
            13 => {
                j.connection
                    .execute_batch("UPDATE vm_dispatches SET retry_count=-1;")
                    .unwrap();
            }
            14 => {
                j.connection
                    .execute_batch("UPDATE vm_dispatches SET retry_count=17;")
                    .unwrap();
            }
            15 => {
                j.connection
                    .execute_batch("UPDATE vm_dispatches SET retry_count=9223372036854775807;")
                    .unwrap();
            }
            16 => {
                j.connection.execute_batch("CREATE TRIGGER refuse_lease BEFORE UPDATE ON vm_dispatches BEGIN SELECT RAISE(ABORT,'lease refused'); END;").unwrap();
            }
            17 => {
                j.connection.execute_batch("CREATE TABLE commit_parent(id INTEGER PRIMARY KEY);CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED);CREATE TRIGGER refuse_commit AFTER UPDATE ON vm_dispatches BEGIN INSERT INTO commit_child VALUES(1); END;").unwrap();
            }
            18 => {
                j.connection
                    .execute_batch("DROP TABLE vm_dispatches;")
                    .unwrap();
            }
            19 => now = u64::MAX,
            20 => expires = u64::MAX,
            21 => {
                limits.max_active_leases = 1;
                j.connection
                    .execute("UPDATE vm_dispatches SET request=x'7b' WHERE token=?1", [t])
                    .unwrap();
                j.connection.execute_batch("UPDATE vm_dispatches SET state='leased',attempt=1,lease_expires_at_ms=11 WHERE token='continuation-200';").unwrap();
            }
            23 => limits.max_active_leases = 0,
            24 => limits.max_active_leases = usize::MAX,
            25 => {
                j.connection
                    .execute_batch("UPDATE vm_dispatches SET ready_at_ms=11,request=x'7b';")
                    .unwrap();
            }
            26..=29 => {
                let mut p = MergePlan::new(["prior", "next"]).unwrap();
                p.order = ExecutionOrder::Sequential;
                j.connection.execute("INSERT INTO vm_merge_groups(token,plan,state,execution_order) VALUES('merge-500',?1,'pending','sequential')",[serde_json::to_vec(&p).unwrap()]).unwrap();
                j.connection.execute("INSERT INTO vm_merge_branches(group_token,branch_token,branch_name,position) VALUES('merge-500','continuation-200','prior',0),('merge-500',?1,'next',1)",[t]).unwrap();
                j.connection.execute_batch("UPDATE vm_dispatches SET state='leased',attempt=1,lease_expires_at_ms=11 WHERE token='continuation-200';").unwrap();
                if kind >= 27 {
                    let step = if kind == 28 {
                        Step::Fault(Fault {
                            code: "prior".into(),
                            message: "failed".into(),
                        })
                    } else {
                        done()
                    };
                    j.connection.execute("UPDATE vm_effects SET state='completed',terminal_step=?1 WHERE token='continuation-200'",[serde_json::to_vec(&step).unwrap()]).unwrap();
                    j.connection.execute_batch("UPDATE vm_dispatches SET state='acknowledged',lease_expires_at_ms=NULL WHERE token='continuation-200';").unwrap();
                }
                if kind == 29 {
                    j.connection
                        .execute(
                            "UPDATE vm_merge_groups SET state='completed',terminal_step=?1",
                            [serde_json::to_vec(&done()).unwrap()],
                        )
                        .unwrap();
                }
            }
            30 => {
                j.connection
                    .execute("UPDATE vm_dispatches SET attempt=2 WHERE token=?1", [t])
                    .unwrap();
            }
            31 => {
                let mut r = input.clone();
                r.required_capability = "invalid".into();
                j.connection
                    .execute(
                        "UPDATE vm_dispatches SET request=?1",
                        [serde_json::to_vec(&r).unwrap()],
                    )
                    .unwrap();
            }
            32 => {
                j.connection
                    .execute_batch("UPDATE vm_dispatches SET ready_at_ms=10;")
                    .unwrap();
            }
            33 => {
                j.connection
                    .execute_batch("UPDATE vm_effects SET state='invalid';")
                    .unwrap();
            }
            34 => {
                j.connection
                    .execute_batch("UPDATE vm_dispatches SET attempt=-1,request=x'7b';")
                    .unwrap();
            }
            35 => {
                j.connection.execute_batch("UPDATE vm_effects SET image=x'7b';UPDATE vm_dispatches SET retry_count=-1;").unwrap();
            }
            _ => unreachable!(),
        }
        j.connection
            .execute_batch("PRAGMA foreign_keys=ON;PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
        (now, expires, limits)
    }
    #[test]
    fn sqlite_claim_matches_frozen_busy_cold_path_decode_precedence_and_atomic_lease_updates() {
        let inputs = crate::continuation_contract_tests::requests();
        let mut cases = 0;
        for index in [0, 35, 69] {
            for kind in 0..36 {
                let f = Fixture::new();
                let other = f.0.with_extension("reference.sqlite3");
                std::fs::copy(&f.0, &other).unwrap();
                let mut actual = SqliteJournal {
                    connection: Connection::open(&f.0).unwrap(),
                };
                let mut original = SqliteJournal {
                    connection: Connection::open(&other).unwrap(),
                };
                let (now, expires, limits) = mutate(&mut actual, &inputs[index], kind);
                mutate(&mut original, &inputs[index], kind);
                let before = snapshot(&actual.connection);
                assert_eq!(before, snapshot(&original.connection));
                let result = actual.claim_dispatch(now, expires, limits);
                assert_eq!(
                    result,
                    original.frozen_claim_dispatch(now, expires, limits),
                    "operation {index}, case {kind}"
                );
                assert_eq!(snapshot(&actual.connection), snapshot(&original.connection));
                assert_eq!(
                    actual.connection.total_changes(),
                    original.connection.total_changes()
                );
                assert!(actual.connection.is_autocommit());
                assert!(original.connection.is_autocommit());
                if !matches!(result, Ok(DispatchClaim::Leased(_))) {
                    assert_eq!(snapshot(&actual.connection), before);
                }
                if matches!(kind, 21 | 23) {
                    assert!(matches!(result, Ok(DispatchClaim::Backpressured(_))));
                }
                if matches!(kind, 0 | 2..=4 | 22 | 25 | 26 | 33) {
                    assert!(
                        matches!(result, Ok(DispatchClaim::Idle)),
                        "case {kind}: {result:?}"
                    );
                }
                if matches!(kind, 7..=20 | 24 | 31 | 34 | 35) {
                    assert!(result.is_err(), "case {kind}: {result:?}");
                }
                // In this inconsistent raw fixture the original SQL ignores a completed
                // group. This is query parity, not permission to recover that graph.
                if kind == 29 {
                    assert!(matches!(result, Ok(DispatchClaim::Leased(_))));
                }
                cases += 1;
            }
        }
        assert_eq!(cases, 108);
    }
    #[test]
    fn public_claim_failure_reopen_and_shared_owners_preserve_one_current_lease_and_cache_publication()
     {
        let mut cases = 0;
        for n in [7, 70, 200] {
            for deferred in [false, true] {
                let f = Fixture::new();
                let mut local = crate::Vm::open_journal(&f.0, 500).unwrap();
                let mut peer = crate::Vm::open_journal(&f.0, 500).unwrap();
                let input = request(n);
                peer.restore_request(input.clone()).unwrap();
                assert_eq!(local.pending_count(), 0);
                let c = Connection::open(&f.0).unwrap();
                c.execute_batch(if deferred{
                "CREATE TABLE commit_parent(id INTEGER PRIMARY KEY);CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED);CREATE TRIGGER refuse_lease AFTER UPDATE ON vm_dispatches BEGIN INSERT INTO commit_child VALUES(1); END;"
            }else{
                "CREATE TRIGGER refuse_lease BEFORE UPDATE ON vm_dispatches BEGIN SELECT RAISE(ABORT,'lease refused'); END;"
            }).unwrap();
                let before = snapshot(&c);
                assert!(local.claim_effect(1, 10).is_err());
                assert_eq!(local.pending_count(), 0);
                assert_eq!(local.completed_count(), 0);
                assert_eq!(snapshot(&c), before);
                c.execute_batch("DROP TRIGGER refuse_lease;").unwrap();
                let first = local.claim_effect(1, 10).unwrap().unwrap();
                assert_eq!(first.attempt, 1);
                assert_eq!(local.pending_count(), 1);
                assert!(peer.claim_effect(2, 10).unwrap().is_none());
                drop(local);
                let mut recovered = crate::Vm::open_journal(&f.0, 500).unwrap();
                assert!(recovered.claim_effect(10, 10).unwrap().is_none());
                let second = recovered.claim_effect(11, 10).unwrap().unwrap();
                assert_eq!(second.attempt, 2);
                assert_eq!(second.request, input);
                let reply = || {
                    crate::EffectResult::Presentation(crate::PresentationResult::Focus {
                        node_id: "node".into(),
                    })
                };
                assert!(
                    matches!(peer.acknowledge_effect(&first,11,reply()),Step::Fault(Fault{code,..})if code=="LSV4022")
                );
                assert_eq!(recovered.acknowledge_effect(&second, 12, reply()), done());
                drop(recovered);
                let mut reopened = crate::Vm::open_journal(&f.0, 500).unwrap();
                assert!(reopened.claim_effect(22, 10).unwrap().is_none());
                assert_eq!(reopened.resume(&input.continuation, reply()), done());
                let next: i64 = c
                    .query_row("SELECT next_sequence FROM vm_metadata", [], |r| r.get(0))
                    .unwrap();
                assert_eq!(next, n as i64 + 1);
                cases += 1;
            }
        }
        assert_eq!(cases, 6);
    }
}

// Frozen claim bodies from the pre-extraction working tree based on 8b792476.
// Native selection SQL, graph/capacity observations, codecs and storage remain shared.
impl EphemeralJournal {
    fn frozen_claim_dispatch(
        &mut self,
        now_ms: u64,
        lease_expires_at_ms: u64,
        limits: SchedulerLimits,
    ) -> Result<DispatchClaim, Fault> {
        let mut eligible: Option<(&ContinuationToken, &EphemeralDispatch)> = None;
        for (token, dispatch) in &self.dispatches {
            if dispatch.acknowledged
                || dispatch.ready_at_ms > now_ms
                || dispatch
                    .lease_expires_at_ms
                    .is_some_and(|expires_at| expires_at > now_ms)
            {
                continue;
            }
            let already_preferred = eligible.is_some_and(|(best_token, best)| {
                best.attempt
                    .cmp(&dispatch.attempt)
                    .then_with(|| continuation_age_order(best_token, token))
                    .is_le()
            });
            // Scan graph dependencies only for candidates that can beat the current minimum.
            if !already_preferred && self.sequence_ready(token) {
                eligible = Some((token, dispatch));
            }
        }
        let Some(token) = eligible.map(|(token, _)| token.clone()) else {
            return Ok(DispatchClaim::Idle);
        };
        let pressure = self.scheduler_pressure(now_ms, limits);
        if pressure.lease_capacity_exhausted() {
            return Ok(DispatchClaim::Backpressured(pressure));
        }
        let dispatch = self
            .dispatches
            .get_mut(&token)
            .ok_or_else(|| journal_fault("LSV4007", "dispatch disappeared"))?;
        dispatch.attempt = next_attempt(dispatch.attempt)?;
        dispatch.lease_expires_at_ms = Some(lease_expires_at_ms);
        Ok(DispatchClaim::Leased(Box::new(DispatchLease {
            request: dispatch.request.clone(),
            attempt: dispatch.attempt,
            retry_count: dispatch.retry_count,
            lease_expires_at_ms,
        })))
    }
}
#[cfg(feature = "sqlite-journal")]
impl SqliteJournal {
    fn frozen_claim_dispatch(
        &mut self,
        now_ms: u64,
        expires_at: u64,
        limits: SchedulerLimits,
    ) -> Result<DispatchClaim, Fault> {
        let now = i64::try_from(now_ms)
            .map_err(|_| journal_fault("LSV4015", "dispatch clock is out of range"))?;
        let expires_at_sql = i64::try_from(expires_at)
            .map_err(|_| journal_fault("LSV4015", "dispatch lease expiration is out of range"))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4016", "failed to lock dispatch journal", error))?;
        // Busy workers only inspect eligibility; they do not materialize a queued request/image.
        if sql_lease_capacity_exhausted(&transaction, now, limits.max_active_leases)? {
            let eligible: bool = transaction
                .query_row(
                    &format!("SELECT EXISTS({CLAIM_DISPATCH_SQL})"),
                    [now],
                    |row| row.get(0),
                )
                .map_err(|error| {
                    journal_error("LSV4016", "failed to inspect dispatch readiness", error)
                })?;
            let claim = if eligible {
                DispatchClaim::Backpressured(sql_scheduler_pressure(&transaction, now_ms, limits)?)
            } else {
                DispatchClaim::Idle
            };
            transaction.commit().map_err(|error| {
                journal_error(
                    "LSV4016",
                    "failed to close capacity-limited dispatch transaction",
                    error,
                )
            })?;
            return Ok(claim);
        }
        let candidate = transaction
            .query_row(CLAIM_DISPATCH_SQL, [now], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, Vec<u8>>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Vec<u8>>(4)?,
                ))
            })
            .optional()
            .map_err(|error| journal_error("LSV4016", "failed to select effect dispatch", error))?;
        let Some((token, request_bytes, attempt, retry_count, image_bytes)) = candidate else {
            transaction.commit().map_err(|error| {
                journal_error("LSV4016", "failed to close dispatch transaction", error)
            })?;
            return Ok(DispatchClaim::Idle);
        };
        let request: EffectRequest = decode_bounded(&request_bytes, MAX_JOURNAL_ENTRY_BYTES)?;
        validate_effect_request(&request)?;
        if request.continuation.token.as_str() != token {
            return Err(journal_fault(
                "LSV4007",
                "dispatch token does not match effect request",
            ));
        }
        if image_bytes
            != encode_json_capped(
                &request.continuation,
                MAX_CONTINUATION_BYTES,
                "continuation",
            )?
        {
            return Err(journal_fault(
                "LSV4014",
                "dispatch continuation conflicts with durable journal state",
            ));
        }
        let attempt = u32::try_from(attempt)
            .map_err(|_| journal_fault("LSV4007", "dispatch attempt is invalid"))?;
        let attempt = next_attempt(attempt)?;
        let retry_count = u32::try_from(retry_count)
            .map_err(|_| journal_fault("LSV4007", "dispatch retry count is invalid"))?;
        if retry_count > MAX_SEMANTIC_RETRIES {
            return Err(journal_fault(
                "LSV4007",
                "dispatch retry count exceeds runtime limit",
            ));
        }
        transaction
            .execute(
                "UPDATE vm_dispatches
                 SET state = 'leased', attempt = ?2, lease_expires_at_ms = ?3
                 WHERE token = ?1",
                params![token, i64::from(attempt), expires_at_sql],
            )
            .map_err(|error| journal_error("LSV4016", "failed to lease effect dispatch", error))?;
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4016", "failed to commit effect lease", error))?;
        Ok(DispatchClaim::Leased(Box::new(DispatchLease {
            request,
            attempt,
            retry_count,
            lease_expires_at_ms: expires_at,
        })))
    }
}
