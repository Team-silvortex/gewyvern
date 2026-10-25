use super::*;

fn done() -> Step {
    Step::Done(crate::Value::UiFocus {
        node_id: "old".into(),
    })
}
fn reply() -> crate::EffectResult {
    crate::EffectResult::Presentation(crate::PresentationResult::Focus {
        node_id: "a".into(),
    })
}
const SOURCES: [&str; 5] = [
    r#"fn main() = ui.focus(node_id: "a")"#,
    r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
    r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
    r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
    r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
];
fn start(vm: &mut crate::Vm, source: &str) {
    assert!(matches!(
        vm.start_timed(
            &leselang_hir::lower(&leselang_syntax::parse(source)).unwrap(),
            crate::Principal::new("operator").unwrap(),
            crate::CapabilitySet::new(["ui.presentation"]),
            None,
            0,
            10
        ),
        Step::Effect(_) | Step::Effects(_)
    ));
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
        j.merge_groups
            .iter()
            .map(|(t, g)| (t, &g.plan, &g.branch_tokens, &g.terminal_step))
            .collect::<Vec<_>>(),
        j.debugger_audits.len(),
    ))
    .unwrap()
}
fn memory_fixture(request: &EffectRequest, kind: usize) -> (EphemeralJournal, u64) {
    let mut j = EphemeralJournal::default();
    j.record_pending(Some(request), SchedulerLimits::default())
        .unwrap();
    let d = j.dispatches.get_mut(&request.continuation.token).unwrap();
    d.request.continuation.deadline_at_ms = Some(10);
    let mut now = 10;
    match kind {
        0 => d.request.continuation.deadline_at_ms = None,
        1 => d.request.continuation.deadline_at_ms = Some(11),
        2 => d.request.continuation.deadline_at_ms = Some(9),
        3 => {}
        4 => {
            d.acknowledged = true;
            d.terminal_step = Some(done());
        }
        5 => d.lease_expires_at_ms = Some(100),
        6 => {
            d.request.continuation.deadline_at_ms = Some(u64::MAX);
            now = u64::MAX;
        }
        7 => d.request.continuation.schema_version = 0,
        8 => d.terminal_step = Some(done()),
        9 => j.dispatches.clear(),
        _ => unreachable!(),
    }
    (j, now)
}
#[test]
fn memory_expiration_matches_frozen_selection_and_mutation_across_all_operations() {
    let mut cases = 0;
    for request in crate::continuation_contract_tests::requests() {
        for kind in 0..10 {
            let (mut actual, now) = memory_fixture(&request, kind);
            let (mut original, _) = memory_fixture(&request, kind);
            let result = actual.expire_due(now);
            assert_eq!(result, original.frozen_expire_due(now));
            assert_eq!(memory_snapshot(&actual), memory_snapshot(&original));
            assert_eq!(
                result.unwrap().len(),
                usize::from(matches!(kind, 2 | 3 | 5..=8))
            );
            cases += 1;
        }
    }
    assert_eq!(cases, 700);
}
fn memory_group_failure(source: &str) -> (EphemeralJournal, Vec<ContinuationToken>) {
    let mut vm = crate::Vm::new(500);
    start(&mut vm, source);
    start(&mut vm, SOURCES[0]);
    let pending = vm.pending_continuations();
    assert_eq!(pending.len(), 3);
    #[cfg(feature = "sqlite-journal")]
    let mut j = match vm.journal {
        Journal::Ephemeral(j) => j,
        Journal::Sqlite(_) => unreachable!(),
    };
    #[cfg(not(feature = "sqlite-journal"))]
    let Journal::Ephemeral(mut j) = vm.journal;
    let first = j.dispatches.get_mut(&pending[0].token).unwrap();
    first.acknowledged = true;
    first.terminal_step = Some(Step::Yield(pending[0].clone()));
    (j, pending[1..].iter().map(|i| i.token.clone()).collect())
}
#[test]
fn native_memory_failure_keeps_later_unrelated_expiration_before_first_group_finalization() {
    let mut cases = 0;
    for source in &SOURCES[1..3] {
        let (mut actual, due) = memory_group_failure(source);
        let (mut original, _) = memory_group_failure(source);
        let before = memory_snapshot(&actual);
        let result = actual.expire_due(10);
        assert!(result.is_err());
        assert_eq!(result, original.frozen_expire_due(10));
        assert_eq!(memory_snapshot(&actual), memory_snapshot(&original));
        assert_ne!(memory_snapshot(&actual), before);
        for token in due {
            let d = &actual.dispatches[&token];
            assert!(d.acknowledged && d.lease_expires_at_ms.is_none());
            assert_eq!(
                d.terminal_step,
                Some(deadline_cancellation(&d.request.continuation, 10))
            );
        }
        cases += 1;
    }
    assert_eq!(cases, 2);
}
#[test]
fn public_memory_expiration_publishes_only_successful_batches_and_replays_deadlines() {
    let mut cases = 0;
    for leased in [false, true] {
        let mut vm = crate::Vm::new(500);
        start(&mut vm, SOURCES[0]);
        let image = vm.pending_continuations().remove(0);
        if leased {
            assert!(vm.claim_effect(1, 100).unwrap().is_some());
        }
        assert!(vm.claim_effect(u64::MAX, 100).is_err());
        assert_eq!(vm.pending_count(), 1);
        assert!(vm.claim_effect(10, 100).unwrap().is_none());
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(
            vm.resume_at(&image, 11, reply()),
            deadline_cancellation(&image, 10)
        );
        assert!(vm.claim_effect(11, 100).unwrap().is_none());
        cases += 1;
    }
    assert_eq!(cases, 2);
}

#[test]
fn public_memory_finalization_failure_does_not_publish_partial_cache_results() {
    let mut cases = 0;
    for source in &SOURCES[1..3] {
        let mut vm = crate::Vm::new(500);
        start(&mut vm, source);
        start(&mut vm, SOURCES[0]);
        let (journal, _) = memory_group_failure(source);
        vm.journal = Journal::Ephemeral(journal);
        let pending = vm.pending.clone();
        let completed = vm.completed.clone();
        assert!(vm.expire_due(10).is_err());
        assert_eq!(vm.pending, pending);
        assert_eq!(vm.completed, completed);
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
        fn new(source: &str) -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let dir = std::env::temp_dir().join(format!(
                "leselang-expiration-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&dir).unwrap();
            let fixture = Self(dir.join("journal.sqlite3"));
            let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
            for _ in 0..2 {
                start(&mut vm, source);
            }
            fixture
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
        }
    }
    fn snapshot(connection: &Connection) -> Vec<(String, Vec<Vec<rusqlite::types::Value>>)> {
        let mut tables = connection
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        tables
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .map(|t| {
                let t = t.unwrap();
                let mut q = connection
                    .prepare(&format!("SELECT * FROM {t} ORDER BY rowid"))
                    .unwrap();
                let count = q.column_count();
                let rows = q
                    .query_map([], |r| {
                        (0..count)
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
    fn set_image(j: &SqliteJournal, token: &str, bytes: Vec<u8>) {
        j.connection
            .execute(
                "UPDATE vm_effects SET image=?1 WHERE token=?2",
                params![bytes, token],
            )
            .unwrap();
    }
    fn mutate(j: &mut SqliteJournal, images: &[ContinuationImage], kind: usize) -> u64 {
        j.connection
            .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        let first = images[0].token.as_str();
        let last = images.last().unwrap().token.as_str();
        let mut now = 10;
        match kind {
            0=>now=9,1=>{},2=>now=11,3=>now=i64::MAX as u64,4=>now=u64::MAX,
            5|25|27=>{set_image(j,last,b"{".to_vec());},
            6|8|9=>{let mut i=images.last().unwrap().clone();match kind { 6=>i.schema_version=0,8=>i.deadline_at_ms=None,_=>i.deadline_at_ms=Some(11) } set_image(j,last,serde_json::to_vec(&i).unwrap());},
            7=>set_image(j,last,serde_json::to_vec(&images[0]).unwrap()),
            10=>set_image(j,last,vec![b' ';MAX_CONTINUATION_BYTES+1]),
            11=>j.connection.execute_batch(&format!("CREATE TRIGGER refuse_effect BEFORE UPDATE ON vm_effects WHEN OLD.token='{last}' BEGIN SELECT RAISE(ABORT,'effect refused'); END;")).unwrap(),
            12=>j.connection.execute_batch(&format!("CREATE TRIGGER refuse_dispatch BEFORE UPDATE ON vm_dispatches WHEN OLD.token='{last}' BEGIN SELECT RAISE(ABORT,'dispatch refused'); END;")).unwrap(),
            13=>j.connection.execute_batch("CREATE TRIGGER refuse_group BEFORE UPDATE ON vm_merge_groups BEGIN SELECT RAISE(ABORT,'group refused'); END;").unwrap(),
            14=>j.connection.execute_batch("CREATE TABLE commit_parent(id INTEGER PRIMARY KEY); CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER refuse_commit AFTER UPDATE ON vm_dispatches BEGIN INSERT INTO commit_child VALUES(1); END;").unwrap(),
            15=>j.connection.execute_batch("UPDATE vm_merge_groups SET plan=x'7b';").unwrap(),
            16|23=>{j.connection.execute_batch("DROP TABLE vm_effects;").unwrap();if kind==23 {now=u64::MAX;}},
            17=>{j.connection.execute("DELETE FROM vm_dispatches WHERE token=?1",[last]).unwrap();},
            18=>j.connection.execute_batch("UPDATE vm_dispatches SET state='acknowledged',lease_expires_at_ms=100;").unwrap(),
            19=>{j.connection.execute("UPDATE vm_effects SET state='unknown' WHERE token=?1",[last]).unwrap();},
            20=>{j.connection.execute("UPDATE vm_effects SET deadline_at_ms=NULL WHERE token=?1",[last]).unwrap();},
            21=>{j.connection.execute("UPDATE vm_effects SET deadline_at_ms=11 WHERE token=?1",[last]).unwrap();},
            22=>{j.connection.execute("UPDATE vm_effects SET state='completed',terminal_step=?1,image=x'7b' WHERE token=?2",params![serde_json::to_vec(&deadline_cancellation(images.last().unwrap(),9)).unwrap(),last]).unwrap();},
            24|26=>{
                // STRICT tables reject this value on insertion. A disposable view
                // injects a row-decoding failure without weakening production schema.
                j.connection.execute_batch(&format!("ALTER TABLE vm_effects RENAME TO saved_effects;
                    CREATE VIEW vm_effects AS SELECT token,state,deadline_at_ms,terminal_step,
                    CASE WHEN token='{last}' THEN 42 ELSE image END AS image FROM saved_effects;")).unwrap();
            },
            _=>unreachable!(),
        }
        if kind == 25 {
            j.connection.execute_batch(&format!("CREATE TRIGGER refuse_first BEFORE UPDATE ON vm_effects WHEN OLD.token='{first}' BEGIN SELECT RAISE(ABORT,'first refused'); END;")).unwrap();
        }
        if kind == 26 {
            j.connection.execute_batch(&format!("CREATE TRIGGER refuse_first INSTEAD OF UPDATE ON vm_effects WHEN OLD.token='{first}' BEGIN SELECT RAISE(ABORT,'first refused'); END;")).unwrap();
        }
        if kind == 27 {
            j.connection
                .execute_batch("UPDATE vm_merge_groups SET plan=x'7b';")
                .unwrap();
        }
        j.connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
        now
    }
    #[test]
    fn sqlite_expiration_matches_frozen_batch_order_scan_precedence_and_atomic_rollback() {
        let mut cases = 0;
        let mut group_failures = 0;
        for (source_index, source) in SOURCES.iter().enumerate() {
            for kind in 0..28 {
                let fixture = Fixture::new(source);
                let vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                let images = vm.pending_continuations();
                drop(vm);
                let other = fixture.0.with_extension("reference.sqlite3");
                std::fs::copy(&fixture.0, &other).unwrap();
                let mut actual = SqliteJournal {
                    connection: Connection::open(&fixture.0).unwrap(),
                };
                let mut original = SqliteJournal {
                    connection: Connection::open(&other).unwrap(),
                };
                let now = mutate(&mut actual, &images, kind);
                assert_eq!(now, mutate(&mut original, &images, kind));
                let before = snapshot(&actual.connection);
                assert_eq!(before, snapshot(&original.connection));
                let writes = actual.connection.total_changes();
                let result = actual.expire_due(now);
                assert_eq!(
                    result,
                    original.frozen_expire_due(now),
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
                if matches!(kind, 0..=3 | 17 | 18) {
                    assert!(result.is_ok(), "case {kind}: {result:?}");
                }
                if matches!(kind,4..=12|14|16|23..=27) {
                    assert!(result.is_err(), "case {kind}");
                }
                if kind == 0 {
                    assert!(result.as_ref().unwrap().is_empty());
                    assert_eq!(snapshot(&actual.connection), before);
                }
                if matches!(kind, 1..=3) {
                    let results = result.as_ref().unwrap();
                    assert_eq!(results.len(), images.len());
                    assert_eq!(
                        results.iter().map(|(i, _)| &i.token).collect::<Vec<_>>(),
                        images.iter().map(|i| &i.token).collect::<Vec<_>>()
                    );
                }
                if matches!(kind, 4 | 23) {
                    assert_eq!(result.as_ref().unwrap_err().code, "LSV4015");
                }
                if matches!(kind, 13 | 15) {
                    assert_eq!(result.is_err(), source_index != 0);
                    if result.is_err() {
                        group_failures += 1;
                    }
                }
                if matches!(kind, 24 | 26) {
                    assert!(
                        result
                            .as_ref()
                            .unwrap_err()
                            .message
                            .contains("failed to read timed effect")
                    );
                    assert_eq!(actual.connection.total_changes(), writes);
                }
                if kind == 25 {
                    assert!(
                        result
                            .as_ref()
                            .unwrap_err()
                            .message
                            .contains("failed to cancel effect")
                    );
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
        assert_eq!(cases, 140);
        assert_eq!(group_failures, 8);
    }
    #[test]
    fn public_expiration_reopens_without_redispatch_or_rewriting_original_deadline_results() {
        let mut cases = 0;
        for source in SOURCES {
            for leased in [false, true] {
                let fixture = Fixture::new(source);
                let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                let images = vm.pending_continuations();
                if leased {
                    assert!(vm.claim_effect(1, 100).unwrap().is_some());
                }
                assert!(vm.claim_effect(10, 100).unwrap().is_none());
                assert_eq!(vm.pending_count(), 0);
                let results = images
                    .iter()
                    .map(|i| vm.resume_at(i, 11, reply()))
                    .collect::<Vec<_>>();
                let connection = Connection::open(&fixture.0).unwrap();
                for i in &images {
                    let bytes: Vec<u8> = connection
                        .query_row(
                            "SELECT terminal_step FROM vm_effects WHERE token=?1",
                            [i.token.as_str()],
                            |r| r.get(0),
                        )
                        .unwrap();
                    let step: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES).unwrap();
                    assert_eq!(step, deadline_cancellation(i, 10));
                }
                let before = snapshot(&connection);
                drop(connection);
                drop(vm);
                let mut reopened = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                assert_eq!(reopened.pending_count(), 0);
                for (i, result) in images.iter().zip(results) {
                    assert_eq!(reopened.resume_at(i, 20, reply()), result);
                }
                assert!(reopened.claim_effect(20, 100).unwrap().is_none());
                drop(reopened);
                assert_eq!(snapshot(&Connection::open(&fixture.0).unwrap()), before);
                cases += 1;
            }
        }
        assert_eq!(cases, 10);
    }
}

// Frozen batch-expiration bodies from ca6b738d, before policy extraction.
// Native codecs/storage/group primitives remain shared, not orchestration.
impl EphemeralJournal {
    fn frozen_expire_due(&mut self, now_ms: u64) -> Result<Vec<(ContinuationImage, Step)>, Fault> {
        let mut expired = Vec::new();
        for dispatch in self.dispatches.values_mut() {
            let image = &dispatch.request.continuation;
            if dispatch.acknowledged
                || image
                    .deadline_at_ms
                    .is_none_or(|deadline| deadline > now_ms)
            {
                continue;
            }
            let step = deadline_cancellation(image, now_ms);
            dispatch.acknowledged = true;
            dispatch.lease_expires_at_ms = None;
            dispatch.terminal_step = Some(step.clone());
            expired.push((image.clone(), step));
        }
        for (image, _) in &expired {
            self.finalize_merge_group(&image.token)?;
        }
        Ok(expired)
    }
}
#[cfg(feature = "sqlite-journal")]
impl SqliteJournal {
    fn frozen_expire_due(&mut self, now_ms: u64) -> Result<Vec<(ContinuationImage, Step)>, Fault> {
        let now = i64::try_from(now_ms)
            .map_err(|_| journal_fault("LSV4015", "dispatch clock is out of range"))?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4025", "failed to lock timeout journal", error))?;
        let mut statement = transaction
            .prepare(
                "SELECT token, image FROM vm_effects
                 WHERE state = 'pending' AND deadline_at_ms IS NOT NULL AND deadline_at_ms <= ?1
                 ORDER BY token ASC",
            )
            .map_err(|error| journal_error("LSV4025", "failed to prepare timeout scan", error))?;
        let records = statement
            .query_map([now], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, Vec<u8>>(1)?))
            })
            .map_err(|error| journal_error("LSV4025", "failed to scan timed effects", error))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| journal_error("LSV4025", "failed to read timed effect", error))?;
        drop(statement);

        let mut expired = Vec::with_capacity(records.len());
        for (token, image_bytes) in records {
            let image: ContinuationImage = decode_bounded(&image_bytes, MAX_CONTINUATION_BYTES)?;
            validate_image(&image)?;
            if image.token.as_str() != token
                || image
                    .deadline_at_ms
                    .is_none_or(|deadline| deadline > now_ms)
            {
                return Err(journal_fault(
                    "LSV4007",
                    "timed effect deadline conflicts with continuation image",
                ));
            }
            let step = deadline_cancellation(&image, now_ms);
            let step_bytes = encode_json_capped(&step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
            ensure_growth(&transaction, step_bytes.len())?;
            complete_terminal_record(&transaction, &token, &step_bytes)?;
            finalize_merge_group(&transaction, &token)?;
            expired.push((image, step));
        }
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4025", "failed to commit effect timeouts", error))?;
        Ok(expired)
    }
}
