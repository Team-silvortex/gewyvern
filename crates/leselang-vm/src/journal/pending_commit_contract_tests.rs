use super::*;

fn request(sequence: u64) -> EffectRequest {
    crate::Vm::build_effect_request(
        &leselang_hir::Effect::UiFocus {
            node_id: "node".into(),
        },
        leselang_hir::Type::UiFocus,
        sequence,
        crate::Principal::new("operator").unwrap(),
        crate::CapabilitySet::new(["ui.presentation"]),
        None,
        1000,
        None,
        100,
    )
    .unwrap()
}
fn reply() -> crate::EffectResult {
    crate::EffectResult::Presentation(crate::PresentationResult::Focus {
        node_id: "node".into(),
    })
}
fn done() -> Step {
    Step::Done(crate::Value::UiFocus {
        node_id: "node".into(),
    })
}
fn memory_snapshot(j: &EphemeralJournal) -> Vec<u8> {
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
        j.merge_groups.len(),
        j.debugger_audits.len(),
    ))
    .unwrap()
}
fn memory_fixture(
    input: &EffectRequest,
    kind: usize,
) -> (EphemeralJournal, Option<EffectRequest>, SchedulerLimits) {
    let mut j = EphemeralJournal::default();
    let mut proposed = Some(input.clone());
    let mut limits = SchedulerLimits::default();
    if matches!(kind, 4..=8 | 11) {
        j.frozen_record_pending(Some(input), limits).unwrap();
    }
    match kind {
        0 => proposed = None,
        1 => {}
        2 => limits.max_pending_dispatches = 0,
        3 => {
            j.frozen_record_pending(Some(&request(1000)), limits)
                .unwrap();
            limits.max_pending_dispatches = 1;
        }
        4 => limits.max_pending_dispatches = 0,
        5 => {
            let d = j.dispatches.get_mut(&input.continuation.token).unwrap();
            d.acknowledged = true;
            d.terminal_step = Some(done());
            limits.max_pending_dispatches = 0;
        }
        6 => {
            let d = j.dispatches.get_mut(&input.continuation.token).unwrap();
            d.attempt = 2;
            d.lease_expires_at_ms = Some(100);
            d.ready_at_ms = 10;
        }
        7 => {
            j.dispatches
                .get_mut(&input.continuation.token)
                .unwrap()
                .request = request(1000)
        }
        8 => {
            let r = proposed.as_mut().unwrap();
            r.required_capability = "invalid".into();
            j.dispatches
                .get_mut(&input.continuation.token)
                .unwrap()
                .request = r.clone();
        }
        9 => {
            proposed.as_mut().unwrap().continuation.schema_version = 0;
            limits.max_pending_dispatches = 0;
        }
        10 => {
            j.frozen_record_pending(Some(&request(1000)), limits)
                .unwrap();
            proposed = None;
            limits.max_pending_dispatches = 0;
        }
        11 => {
            j.dispatches
                .get_mut(&input.continuation.token)
                .unwrap()
                .acknowledged = true
        }
        _ => unreachable!(),
    }
    (j, proposed, limits)
}
#[test]
fn memory_registration_matches_frozen_admission_replay_and_capacity_for_all_operations() {
    let mut cases = 0;
    for input in crate::continuation_contract_tests::requests() {
        for kind in 0..12 {
            let (mut actual, proposed, limits) = memory_fixture(&input, kind);
            let (mut original, _, _) = memory_fixture(&input, kind);
            let before = memory_snapshot(&actual);
            let result = actual.record_pending(proposed.as_ref(), limits);
            assert_eq!(
                result,
                original.frozen_record_pending(proposed.as_ref(), limits)
            );
            assert_eq!(memory_snapshot(&actual), memory_snapshot(&original));
            assert_eq!(result.is_err(), matches!(kind, 2 | 3 | 7..=9));
            if kind != 1 {
                assert_eq!(memory_snapshot(&actual), before);
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 840);
}
#[test]
fn public_memory_import_replay_preserves_leases_and_completed_winners() {
    let mut cases = 0;
    for full in [false, true] {
        let input = request(7);
        let mut vm = crate::Vm::new(500);
        if full {
            vm.restore_request(input.clone()).unwrap();
        } else {
            vm.restore(input.continuation.clone()).unwrap();
        }
        let lease = vm.claim_effect(1, 100).unwrap();
        assert_eq!(lease.is_some(), full);
        if full {
            vm.restore_request(input.clone()).unwrap();
        } else {
            vm.restore(input.continuation.clone()).unwrap();
        }
        assert_eq!(vm.pending_count(), 1);
        assert!(vm.claim_effect(2, 100).unwrap().is_none());
        let terminal = if let Some(lease) = lease {
            vm.acknowledge_effect(&lease, 3, reply())
        } else {
            vm.resume(&input.continuation, reply())
        };
        assert_eq!(terminal, done());
        assert_eq!(vm.resume(&input.continuation, reply()), terminal);
        assert_eq!(vm.pending_count(), 0);
        cases += 1;
    }
    assert_eq!(cases, 2);
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
                "leselang-pending-{}-{}",
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
        let mut tables = c
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        tables
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|t| {
                let t = t.unwrap();
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
    ) -> (ContinuationImage, EffectRequest, SchedulerLimits) {
        let mut image = input.continuation.clone();
        let mut proposed = input.clone();
        let mut limits = SchedulerLimits::default();
        if matches!(kind,1..=10|14|23..=25|30|31) {
            j.frozen_record_pending(&image, Some(input), limits)
                .unwrap();
        }
        // Corrupt only disposable fixtures; these are not admitted checkpoints.
        j.connection
            .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        let token = image.token.as_str();
        match kind {
            0|1=>{},
            2=>{j.connection.execute("UPDATE vm_effects SET state='completed',terminal_step=?1 WHERE token=?2",params![serde_json::to_vec(&done()).unwrap(),token]).unwrap();},
            3=>{j.connection.execute("UPDATE vm_effects SET image=x'7b' WHERE token=?1",[token]).unwrap();},
            4=>{j.connection.execute("UPDATE vm_effects SET state='unknown' WHERE token=?1",[token]).unwrap();},
            5=>{j.connection.execute("DELETE FROM vm_dispatches WHERE token=?1",[token]).unwrap();},
            6=>{j.connection.execute("UPDATE vm_dispatches SET request=x'7b' WHERE token=?1",[token]).unwrap();},
            7=>{j.connection.execute("UPDATE vm_dispatches SET request=?1 WHERE token=?2",params![serde_json::to_vec(&request(1000)).unwrap(),token]).unwrap();},
            8=>{let mut r=input.clone();r.budget.fuel_remaining+=1;j.connection.execute("UPDATE vm_dispatches SET request=?1 WHERE token=?2",params![serde_json::to_vec(&r).unwrap(),token]).unwrap();},
            9=>{j.connection.execute("UPDATE vm_dispatches SET state='leased',attempt=2,lease_expires_at_ms=100 WHERE token=?1",[token]).unwrap();},
            10=>{j.connection.execute("UPDATE vm_dispatches SET retry_count=1,last_error=NULL WHERE token=?1",[token]).unwrap();},
            11=>image.schema_version=0,
            12=>image.fuel_remaining+=1,
            13=>proposed.required_capability="invalid".into(),
            14|15=>limits.max_pending_dispatches=0,
            16=>{let other=request(1000);j.frozen_record_pending(&other.continuation,Some(&other),limits).unwrap();limits.max_pending_dispatches=1;},
            17=>j.connection.execute_batch("DROP TABLE vm_effects;").unwrap(),
            18=>j.connection.execute_batch("CREATE TRIGGER refuse_effect BEFORE INSERT ON vm_effects BEGIN SELECT RAISE(ABORT,'effect refused'); END;").unwrap(),
            19=>j.connection.execute_batch("CREATE TRIGGER refuse_dispatch BEFORE INSERT ON vm_dispatches BEGIN SELECT RAISE(ABORT,'dispatch refused'); END;").unwrap(),
            20=>j.connection.execute_batch("CREATE TRIGGER refuse_watermark BEFORE UPDATE ON vm_metadata BEGIN SELECT RAISE(ABORT,'watermark refused'); END;").unwrap(),
            21=>j.connection.execute_batch("CREATE TABLE commit_parent(id INTEGER PRIMARY KEY); CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER refuse_commit AFTER INSERT ON vm_effects BEGIN INSERT INTO commit_child VALUES(1); END;").unwrap(),
            22=>j.connection.execute_batch("UPDATE vm_metadata SET next_sequence=-1;").unwrap(),
            23=>j.connection.execute_batch("UPDATE vm_metadata SET next_sequence=1;").unwrap(),
            24=>j.connection.execute_batch("UPDATE vm_metadata SET next_sequence=1;CREATE TRIGGER refuse_watermark BEFORE UPDATE ON vm_metadata BEGIN SELECT RAISE(ABORT,'watermark refused'); END;").unwrap(),
            25=>j.connection.execute_batch("UPDATE vm_metadata SET next_sequence=1;CREATE TABLE commit_parent(id INTEGER PRIMARY KEY); CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER refuse_commit AFTER UPDATE ON vm_metadata BEGIN INSERT INTO commit_child VALUES(1); END;").unwrap(),
            26=>j.connection.execute_batch(&format!("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<{MAX_JOURNAL_RECORDS}) INSERT INTO vm_effects(token,state,image) SELECT 'filler-'||x,'pending',x'7b' FROM n;")).unwrap(),
            27=>j.connection.execute_batch(&format!("ALTER TABLE vm_merge_groups RENAME TO saved_groups;CREATE VIEW vm_merge_groups AS SELECT zeroblob({MAX_JOURNAL_TOTAL_BYTES}) AS plan,NULL AS terminal_step;")).unwrap(),
            28=>j.connection.execute_batch("DROP TABLE vm_dispatches;").unwrap(),
            29=>{image.schema_version=0;j.connection.execute_batch("DROP TABLE vm_effects;").unwrap();},
            30=>{j.connection.execute("UPDATE vm_effects SET terminal_step=x'7b' WHERE token=?1",[token]).unwrap();},
            31=>j.connection.execute_batch("UPDATE vm_metadata SET next_sequence=9999;").unwrap(),
            _=>unreachable!(),
        }
        j.connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
        (image, proposed, limits)
    }
    #[test]
    fn sqlite_registration_matches_frozen_replay_conflicts_limits_and_transaction_rollback() {
        let inputs = crate::continuation_contract_tests::requests();
        let mut cases = 0;
        for index in [0, 35, 69] {
            for full in [false, true] {
                for kind in 0..32 {
                    let fixture = Fixture::new();
                    let other = fixture.0.with_extension("reference.sqlite3");
                    std::fs::copy(&fixture.0, &other).unwrap();
                    let mut actual = SqliteJournal {
                        connection: Connection::open(&fixture.0).unwrap(),
                    };
                    let mut original = SqliteJournal {
                        connection: Connection::open(&other).unwrap(),
                    };
                    let (image, proposed, limits) = mutate(&mut actual, &inputs[index], kind);
                    mutate(&mut original, &inputs[index], kind);
                    let before = snapshot(&actual.connection);
                    assert_eq!(before, snapshot(&original.connection));
                    let request = full.then_some(&proposed);
                    let result = actual.record_pending(&image, request, limits);
                    assert_eq!(
                        result,
                        original.frozen_record_pending(&image, request, limits),
                        "operation {index}, full {full}, case {kind}"
                    );
                    assert_eq!(snapshot(&actual.connection), snapshot(&original.connection));
                    assert_eq!(
                        actual.connection.total_changes(),
                        original.connection.total_changes()
                    );
                    if result.is_err() {
                        assert_eq!(snapshot(&actual.connection), before);
                    }
                    let fails = matches!(kind,2..=4|11|17|18|20..=22|24..=29)
                        || (full && matches!(kind, 5..=8 | 10 | 12 | 13 | 15 | 16 | 19));
                    assert_eq!(
                        result.is_err(),
                        fails,
                        "case {kind}, full {full}: {result:?}"
                    );
                    if matches!(kind, 26 | 27) {
                        assert_eq!(result.as_ref().unwrap_err().code, "LSV4006");
                    }
                    if matches!(kind, 1 | 9 | 14 | 30 | 31) {
                        assert_eq!(snapshot(&actual.connection), before);
                    }
                    if kind == 23 {
                        let next: i64 = actual
                            .connection
                            .query_row("SELECT next_sequence FROM vm_metadata", [], |r| r.get(0))
                            .unwrap();
                        assert_eq!(
                            next,
                            crate::canonical_continuation_sequence(&image.token).unwrap() as i64
                                + 1
                        );
                    }
                    assert!(
                        actual.connection.is_autocommit() && original.connection.is_autocommit()
                    );
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
        }
        assert_eq!(cases, 192);
    }
    #[test]
    fn public_imports_reopen_with_original_leases_watermark_and_settled_results() {
        let mut cases = 0;
        for full in [false, true] {
            for sequence in [7, 20, 100] {
                let fixture = Fixture::new();
                let input = request(sequence);
                let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                if full {
                    vm.restore_request(input.clone()).unwrap();
                } else {
                    vm.restore(input.continuation.clone()).unwrap();
                }
                let lease = vm.claim_effect(1, 10).unwrap();
                assert_eq!(lease.is_some(), full);
                if full {
                    vm.restore_request(input.clone()).unwrap();
                } else {
                    vm.restore(input.continuation.clone()).unwrap();
                }
                drop(vm);
                let mut reopened = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                assert_eq!(reopened.next_effect_id, sequence + 1);
                assert!(reopened.claim_effect(2, 10).unwrap().is_none());
                let terminal = if full {
                    let next = reopened.claim_effect(12, 10).unwrap().unwrap();
                    assert_eq!(next.attempt, 2);
                    assert_eq!(next.request, input);
                    reopened.acknowledge_effect(&next, 13, reply())
                } else {
                    reopened.resume(&input.continuation, reply())
                };
                assert_eq!(terminal, done());
                drop(reopened);
                let mut reopened = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                assert_eq!(reopened.resume(&input.continuation, reply()), terminal);
                assert!(reopened.claim_effect(20, 10).unwrap().is_none());
                cases += 1;
            }
        }
        assert_eq!(cases, 6);
    }
    #[test]
    fn native_commit_failure_keeps_fresh_and_replayed_imports_out_of_public_caches() {
        let mut cases = 0;
        for full in [false, true] {
            for replay in [false, true] {
                let fixture = Fixture::new();
                let input = request(7);
                let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                if replay {
                    let mut peer = SqliteJournal::open(&fixture.0).unwrap();
                    peer.frozen_record_pending(
                        &input.continuation,
                        full.then_some(&input),
                        SchedulerLimits::default(),
                    )
                    .unwrap();
                }
                let db = Connection::open(&fixture.0).unwrap();
                db.execute_batch("UPDATE vm_metadata SET next_sequence=1;CREATE TABLE commit_parent(id INTEGER PRIMARY KEY);CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED);").unwrap();
                db.execute_batch(if replay {"CREATE TRIGGER refuse_commit AFTER UPDATE ON vm_metadata BEGIN INSERT INTO commit_child VALUES(1); END;"}else{"CREATE TRIGGER refuse_commit AFTER INSERT ON vm_effects BEGIN INSERT INTO commit_child VALUES(1); END;"}).unwrap();
                let before = snapshot(&db);
                let pending = vm.pending.clone();
                let completed = vm.completed.clone();
                let next = vm.next_effect_id;
                let result = if full {
                    vm.restore_request(input.clone())
                } else {
                    vm.restore(input.continuation.clone())
                };
                assert!(result.is_err());
                assert_eq!(vm.pending, pending);
                assert_eq!(vm.completed, completed);
                assert_eq!(vm.next_effect_id, next);
                assert_eq!(snapshot(&db), before);
                cases += 1;
            }
        }
        assert_eq!(cases, 4);
    }
}

// Frozen registration bodies from the pre-extraction working tree (ca6b738d).
// Native admission/codecs/storage and watermark primitives remain shared, not policy.
impl EphemeralJournal {
    fn frozen_record_pending(
        &mut self,
        request: Option<&EffectRequest>,
        limits: SchedulerLimits,
    ) -> Result<(), Fault> {
        let Some(request) = request else {
            return Ok(());
        };
        validate_effect_request(request)?;
        validate_continuation_encoding_size(&request.continuation)?;
        let token = request.continuation.token.clone();
        if let Some(existing) = self.dispatches.get(&token) {
            return if existing.request == *request {
                Ok(())
            } else {
                Err(journal_fault(
                    "LSV4011",
                    "effect request conflicts with pending state",
                ))
            };
        }
        limits.check_admission(self.scheduler_pressure(0, limits).pending_dispatches, 1)?;
        self.dispatches.insert(
            token,
            EphemeralDispatch {
                request: request.clone(),
                attempt: 0,
                lease_expires_at_ms: None,
                ready_at_ms: 0,
                retry_count: 0,
                last_error: None,
                acknowledged: false,
                terminal_step: None,
            },
        );
        Ok(())
    }
}
#[cfg(feature = "sqlite-journal")]
impl SqliteJournal {
    fn frozen_record_pending(
        &mut self,
        image: &ContinuationImage,
        request: Option<&EffectRequest>,
        limits: SchedulerLimits,
    ) -> Result<(), Fault> {
        validate_image(image)?;
        if let Some(request) = request {
            validate_effect_request(request)?;
            if &request.continuation != image {
                return Err(journal_fault(
                    "LSV4011",
                    "effect request continuation does not match pending image",
                ));
            }
        }
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let sequence = crate::canonical_continuation_sequence(&image.token)
            .ok_or_else(|| journal_fault("LSV4007", "journal continuation sequence is invalid"))?;
        let request_bytes = request
            .map(|request| encode_json_capped(request, MAX_JOURNAL_ENTRY_BYTES, "effect request"))
            .transpose()?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4010", "failed to lock journal", error))?;
        let existing = load_record(&transaction, image.token.as_str())?;
        if let Some(existing) = existing {
            if existing.state == "pending" && existing.image == image_bytes {
                if let Some(request) = request {
                    frozen_ensure_dispatch_matches(&transaction, image.token.as_str(), request)?;
                }
                advance_sequence_watermark(&transaction, sequence)?;
                return transaction.commit().map_err(|error| {
                    journal_error("LSV4010", "failed to commit pending journal record", error)
                });
            }
            return Err(journal_fault(
                "LSV4011",
                "continuation token conflicts with durable journal state",
            ));
        }
        let count = journal_record_count(&transaction)?;
        if count >= MAX_JOURNAL_RECORDS {
            return Err(journal_fault(
                "LSV4006",
                format!("journal record limit {MAX_JOURNAL_RECORDS} reached"),
            ));
        }
        if request.is_some() {
            limits.check_admission(
                sql_scheduler_pressure(&transaction, 0, limits)?.pending_dispatches,
                1,
            )?;
        }
        ensure_growth(
            &transaction,
            image_bytes.len() + request_bytes.as_ref().map_or(0, Vec::len),
        )?;
        transaction
            .execute(
                "INSERT INTO vm_effects(token, state, image, deadline_at_ms, terminal_step)
                 VALUES (?1, 'pending', ?2, ?3, NULL)",
                params![
                    image.token.as_str(),
                    image_bytes,
                    image
                        .deadline_at_ms
                        .map(i64::try_from)
                        .transpose()
                        .map_err(|_| {
                            journal_fault("LSV4010", "effect deadline is out of range")
                        })?
                ],
            )
            .map_err(|error| journal_error("LSV4010", "failed to store pending effect", error))?;
        if let Some(request_bytes) = request_bytes {
            transaction
                .execute(
                    "INSERT INTO vm_dispatches(token, request, state, attempt, lease_expires_at_ms)
                     VALUES (?1, ?2, 'ready', 0, NULL)",
                    params![image.token.as_str(), request_bytes],
                )
                .map_err(|error| {
                    journal_error("LSV4010", "failed to store effect dispatch", error)
                })?;
        }
        advance_sequence_watermark(&transaction, sequence)?;
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4010", "failed to commit pending effect", error))
    }
}
#[cfg(feature = "sqlite-journal")]
fn frozen_ensure_dispatch_matches(
    transaction: &rusqlite::Transaction<'_>,
    token: &str,
    request: &EffectRequest,
) -> Result<(), Fault> {
    let Some(existing) = load_dispatch(transaction, token)? else {
        return Err(journal_fault(
            "LSV4011",
            "pending effect is missing its durable dispatch",
        ));
    };
    let durable_request: EffectRequest =
        decode_bounded(&existing.request, MAX_JOURNAL_ENTRY_BYTES)?;
    if durable_request != *request {
        return Err(journal_fault(
            "LSV4011",
            "effect request conflicts with durable dispatch state",
        ));
    }
    Ok(())
}
