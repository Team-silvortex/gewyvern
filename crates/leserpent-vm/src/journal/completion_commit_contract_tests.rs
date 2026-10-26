use super::*;

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
const SOURCES: [&str; 5] = [
    r#"fn main() = ui.focus(node_id: "a")"#,
    r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
    r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
    r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
    r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
];
fn start(source: &str) -> crate::Vm {
    let mut vm = crate::Vm::new(500);
    assert!(matches!(
        vm.start(
            &leserpent_hir::lower(&leselang_syntax::parse(source)).unwrap(),
            crate::Principal::new("operator").unwrap(),
            crate::CapabilitySet::new(["ui.presentation"]),
            None
        ),
        Step::Effect(_) | Step::Effects(_)
    ));
    vm
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
fn memory_fixture(
    request: &EffectRequest,
    kind: usize,
) -> (EphemeralJournal, ContinuationImage, Step) {
    let mut j = EphemeralJournal::default();
    j.record_pending(Some(request), SchedulerLimits::default())
        .unwrap();
    let d = j.dispatches.get_mut(&request.continuation.token).unwrap();
    let mut image = request.continuation.clone();
    let mut step = done("new");
    match kind {
        0 => {}
        1 => step = Step::Fault(journal_fault("HOST", "failure")),
        2 => step = deadline_cancellation(&image, 10),
        3 => step = Step::Yield(image.clone()),
        4 => d.lease_expires_at_ms = Some(100),
        5 => {
            d.acknowledged = true;
            d.lease_expires_at_ms = Some(100);
            d.terminal_step = Some(done("old"));
        }
        6 => {
            d.acknowledged = true;
            d.terminal_step = Some(Step::Yield(image.clone()));
        }
        7 => j.dispatches.clear(),
        8 => image.schema_version = 0,
        9 => {
            j.dispatches.clear();
            step = Step::Yield(image.clone());
        }
        _ => unreachable!(),
    }
    (j, image, step)
}
#[test]
fn memory_completion_matches_frozen_primitive_for_all_operations() {
    let mut cases = 0;
    for request in crate::continuation_contract_tests::requests() {
        for kind in 0..10 {
            let (mut actual, image, step) = memory_fixture(&request, kind);
            let (mut original, ..) = memory_fixture(&request, kind);
            let before = memory_snapshot(&actual);
            let result = actual.record_completed(&image, &step);
            assert_eq!(result, original.frozen_record_completed(&image, &step));
            assert_eq!(memory_snapshot(&actual), memory_snapshot(&original));
            assert_eq!(result.is_err(), kind == 4);
            if kind == 4 {
                assert_eq!(memory_snapshot(&actual), before);
            }
            if kind == 5 {
                assert_eq!(
                    actual.dispatches[&image.token].lease_expires_at_ms,
                    Some(100)
                );
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 700);
}
fn group_fixture(source: &str, mode: usize) -> (EphemeralJournal, ContinuationImage) {
    let vm = start(source);
    let pending = vm.pending_continuations();
    #[cfg(feature = "sqlite-journal")]
    let mut j = match vm.journal {
        Journal::Ephemeral(j) => j,
        Journal::Sqlite(_) => unreachable!(),
    };
    #[cfg(not(feature = "sqlite-journal"))]
    let Journal::Ephemeral(mut j) = vm.journal;
    if mode == 0 {
        j.dispatches
            .get_mut(&pending[1].token)
            .unwrap()
            .lease_expires_at_ms = Some(100);
        (j, pending[1].clone())
    } else {
        let other = j.dispatches.get_mut(&pending[1].token).unwrap();
        other.acknowledged = true;
        other.terminal_step = Some(Step::Yield(pending[1].clone()));
        (j, pending[0].clone())
    }
}
#[test]
fn native_memory_sequence_precedence_and_partial_finalization_match_original() {
    let mut cases = 0;
    for source in &SOURCES[1..3] {
        for mode in 0..2 {
            let (mut actual, image) = group_fixture(source, mode);
            let (mut original, _) = group_fixture(source, mode);
            let before = memory_snapshot(&actual);
            let result = actual.record_completed(&image, &done("new"));
            assert_eq!(
                result,
                original.frozen_record_completed(&image, &done("new"))
            );
            assert_eq!(memory_snapshot(&actual), memory_snapshot(&original));
            assert!(result.is_err());
            if mode == 0 {
                assert_eq!(memory_snapshot(&actual), before);
                assert_eq!(
                    result.unwrap_err().code,
                    if source.contains("seq(") {
                        "LSV2410"
                    } else {
                        "LSV4024"
                    }
                );
            } else {
                assert_ne!(memory_snapshot(&actual), before);
                assert!(actual.dispatches[&image.token].acknowledged);
                assert_eq!(
                    actual.dispatches[&image.token].terminal_step,
                    Some(done("new"))
                );
            }
            cases += 1;
        }
    }
    assert_eq!(cases, 4);
}
#[test]
fn public_memory_resume_replays_original_winner_without_redispatch() {
    let mut cases = 0;
    for wrong in [false, true] {
        let mut vm = start(SOURCES[0]);
        let image = vm.pending_continuations().remove(0);
        let original = vm.resume(&image, reply(wrong));
        assert_eq!(matches!(original, Step::Fault(_)), wrong);
        assert_eq!(vm.resume(&image, reply(!wrong)), original);
        assert!(vm.claim_effect(1, 100).unwrap().is_none());
        cases += 1;
    }
    assert_eq!(cases, 2);
}

#[cfg(feature = "sqlite-journal")]
mod durable {
    use super::super::group_recovery_contract_tests::Fixture;
    use super::*;
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
    fn completed(j: &SqliteJournal, image: &ContinuationImage, bytes: Option<Vec<u8>>) {
        j.connection
            .execute(
                "UPDATE vm_effects SET state='completed', terminal_step=?1 WHERE token=?2",
                params![bytes, image.token.as_str()],
            )
            .unwrap();
    }
    fn mutate(j: &mut SqliteJournal, image: &mut ContinuationImage, step: &mut Step, kind: usize) {
        // Deliberately corrupt only disposable fixtures; restore checks before production calls.
        j.connection
            .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        let token = image.token.as_str();
        match kind {
            0 | 21 => {},
            1 => *step = Step::Fault(journal_fault("HOST", "failure")),
            2 => *step = deadline_cancellation(image, 10),
            3 | 24 | 25 | 29 => {
                completed(j, image, Some(serde_json::to_vec(&done("old")).unwrap()));
                if kind == 24 { *step = Step::Yield(image.clone()); }
                if kind == 25 { j.connection.execute("DELETE FROM vm_dispatches WHERE token=?1", [token]).unwrap(); }
                if kind == 29 { j.connection.execute_batch("UPDATE vm_merge_groups SET plan=x'7b';").unwrap(); }
            },
            4 => completed(j, image, None),
            5 => completed(j, image, Some(b"{".to_vec())),
            6 => completed(j, image, Some(serde_json::to_vec(&Step::Yield(image.clone())).unwrap())),
            7 => image.fuel_remaining += 1,
            8 => { j.connection.execute("DELETE FROM vm_effects WHERE token=?1", [token]).unwrap(); },
            9 => { j.connection.execute("DELETE FROM vm_dispatches WHERE token=?1", [token]).unwrap(); },
            10 => { j.connection.execute("UPDATE vm_dispatches SET request=x'7b' WHERE token=?1", [token]).unwrap(); },
            11 | 22 => { j.connection.execute("UPDATE vm_dispatches SET state='leased', attempt=1, lease_expires_at_ms=100 WHERE token=?1", [token]).unwrap(); },
            12 => image.schema_version = 0,
            13 => *step = Step::Yield(image.clone()),
            14 => j.connection.execute_batch("CREATE TRIGGER refuse_effect BEFORE UPDATE ON vm_effects BEGIN SELECT RAISE(ABORT, 'effect refused'); END;").unwrap(),
            15 => j.connection.execute_batch("CREATE TRIGGER refuse_dispatch BEFORE UPDATE ON vm_dispatches BEGIN SELECT RAISE(ABORT, 'dispatch refused'); END;").unwrap(),
            16 => { *step = Step::Fault(journal_fault("HOST", "stop sequence")); j.connection.execute_batch("CREATE TRIGGER refuse_group BEFORE UPDATE ON vm_merge_groups BEGIN SELECT RAISE(ABORT, 'group refused'); END;").unwrap(); },
            17 => j.connection.execute_batch("CREATE TABLE commit_parent(id INTEGER PRIMARY KEY); CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER refuse_commit AFTER UPDATE ON vm_dispatches BEGIN INSERT INTO commit_child VALUES(1); END;").unwrap(),
            18 => j.connection.execute_batch("UPDATE vm_merge_groups SET plan=x'7b';").unwrap(),
            19 => j.connection.execute_batch("DROP TABLE vm_effects;").unwrap(),
            20 => { j.connection.execute("UPDATE vm_effects SET state='unknown' WHERE token=?1", [token]).unwrap(); },
            23 => { j.connection.execute("UPDATE vm_dispatches SET retry_count=1,last_error=NULL WHERE token=?1", [token]).unwrap(); },
            26 => completed(j, image, Some(serde_json::to_vec(&Step::Fault(journal_fault("", "invalid"))).unwrap())),
            27 => completed(j, image, Some(vec![b' '; MAX_JOURNAL_ENTRY_BYTES + 1])),
            28 => { j.connection.execute("UPDATE vm_dispatches SET state='acknowledged',lease_expires_at_ms=100 WHERE token=?1", [token]).unwrap(); },
            _ => unreachable!(),
        }
        j.connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
    }
    #[test]
    fn sqlite_completion_matches_frozen_sequence_replay_and_all_table_rollback() {
        let mut cases = 0;
        let mut group_failures = 0;
        for (source_index, source) in SOURCES.iter().enumerate() {
            for kind in 0..30 {
                let fixture = Fixture::new(source, 0);
                let vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                let pending = vm.pending_continuations();
                let initial = pending[if matches!(kind, 21 | 22) && pending.len() > 1 {
                    1
                } else {
                    0
                }]
                .clone();
                drop(vm);
                let other = fixture.0.with_extension("reference.sqlite3");
                std::fs::copy(&fixture.0, &other).unwrap();
                let mut actual = SqliteJournal {
                    connection: Connection::open(&fixture.0).unwrap(),
                };
                let mut original = SqliteJournal {
                    connection: Connection::open(&other).unwrap(),
                };
                let (mut image, mut step) = (initial.clone(), done("new"));
                mutate(&mut actual, &mut image, &mut step, kind);
                mutate(&mut original, &mut initial.clone(), &mut done("new"), kind);
                let before = snapshot(&actual.connection);
                assert_eq!(before, snapshot(&original.connection));
                let result = actual.record_completed(&image, &step);
                assert_eq!(
                    result,
                    original.frozen_record_completed(&image, &step),
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
                if matches!(kind, 0..=3 | 9 | 10 | 20 | 25 | 28 | 29) {
                    assert!(result.is_ok(), "case {kind}: {result:?}");
                }
                if matches!(kind, 4..=8 | 11..=15 | 17 | 19 | 22..=24 | 26 | 27) {
                    assert!(result.is_err(), "case {kind}");
                }
                if kind == 7 {
                    assert_eq!(result.as_ref().unwrap_err().code, "LSV4014");
                }
                if matches!(kind, 11 | 22) {
                    assert_eq!(result.as_ref().unwrap_err().code, "LSV4024");
                }
                if kind == 24 {
                    assert_eq!(result.as_ref().unwrap_err().code, "LSV4007");
                }
                if kind == 16 && result.is_err() {
                    group_failures += 1;
                }
                if kind == 18 {
                    assert_eq!(result.is_err(), source_index != 0);
                }
                if kind == 21 {
                    assert_eq!(result.is_err(), matches!(source_index, 1 | 3));
                }
                if kind == 28 {
                    let expires: Option<i64> = actual
                        .connection
                        .query_row(
                            "SELECT lease_expires_at_ms FROM vm_dispatches WHERE token=?1",
                            [image.token.as_str()],
                            |r| r.get(0),
                        )
                        .unwrap();
                    assert_eq!(expires, None);
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
        assert_eq!(cases, 150);
        assert!(group_failures > 0);
    }
    #[test]
    fn public_resume_reopens_and_replays_member_result_without_redispatch() {
        let mut cases = 0;
        for source in SOURCES {
            for wrong in [false, true] {
                let fixture = Fixture::new(source, 0);
                let mut vm = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                let image = vm.pending_continuations().remove(0);
                let original = vm.resume(&image, reply(wrong));
                // Member terminal persistence and group-level visibility are distinct.
                let bytes: Vec<u8> = Connection::open(&fixture.0)
                    .unwrap()
                    .query_row(
                        "SELECT terminal_step FROM vm_effects WHERE token=?1",
                        [image.token.as_str()],
                        |r| r.get(0),
                    )
                    .unwrap();
                let terminal: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES).unwrap();
                assert_eq!(matches!(terminal, Step::Fault(_)), wrong);
                drop(vm);
                let mut reopened = crate::Vm::open_journal(&fixture.0, 500).unwrap();
                assert_eq!(reopened.resume(&image, reply(!wrong)), original);
                for _ in 0..4 {
                    if let Some(next) = reopened.claim_effect(20, 100).unwrap() {
                        assert_ne!(next.request.continuation.token, image.token);
                    }
                }
                cases += 1;
            }
        }
        assert_eq!(cases, 10);
    }
}

// Frozen ordinary-completion bodies from the pre-extraction working tree.
// Native storage/codecs and group primitives are shared, not orchestration.
impl EphemeralJournal {
    fn frozen_record_completed(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
    ) -> Result<Step, Fault> {
        if !self.sequence_ready(&image.token) {
            return Err(journal_fault(
                "LSV2410",
                "sequential predecessor has not completed",
            ));
        }
        if let Some(dispatch) = self.dispatches.get_mut(&image.token) {
            if dispatch.lease_expires_at_ms.is_some() && !dispatch.acknowledged {
                return Err(journal_fault(
                    "LSV4024",
                    "leased effect must be completed through dispatch acknowledgement",
                ));
            }
            dispatch.acknowledged = true;
            dispatch.terminal_step = Some(step.clone());
        }
        self.finalize_merge_group(&image.token)?;
        Ok(step.clone())
    }
}
#[cfg(feature = "sqlite-journal")]
impl SqliteJournal {
    fn frozen_record_completed(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
    ) -> Result<Step, Fault> {
        validate_image(image)?;
        validate_terminal_step(step)?;
        let image_bytes = encode_json_capped(image, MAX_CONTINUATION_BYTES, "continuation")?;
        let step_bytes = encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4012", "failed to lock journal", error))?;
        let Some(existing) = load_record(&transaction, image.token.as_str())? else {
            return Err(journal_fault(
                "LSV4013",
                "pending continuation is missing from durable journal",
            ));
        };
        if existing.image != image_bytes {
            return Err(journal_fault(
                "LSV4014",
                "continuation image conflicts with durable journal state",
            ));
        }
        if existing.state == "completed" {
            let bytes = existing.terminal_step.ok_or_else(|| {
                journal_fault("LSV4007", "completed journal record has no terminal step")
            })?;
            let authoritative: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
            validate_terminal_step(&authoritative)?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4012", "failed to close journal transaction", error)
            })?;
            return Ok(authoritative);
        }
        if let Some(dispatch) = load_dispatch(&transaction, image.token.as_str())?
            && dispatch.state == "leased"
        {
            return Err(journal_fault(
                "LSV4024",
                "leased effect must be completed through dispatch acknowledgement",
            ));
        }
        ensure_sequence_ready(&transaction, image.token.as_str())?;
        ensure_growth(&transaction, step_bytes.len())?;
        transaction
            .execute(
                "UPDATE vm_effects SET state = 'completed', terminal_step = ?2 WHERE token = ?1",
                params![image.token.as_str(), step_bytes],
            )
            .map_err(|error| journal_error("LSV4012", "failed to complete effect", error))?;
        transaction
            .execute(
                "UPDATE vm_dispatches
                 SET state = 'acknowledged', lease_expires_at_ms = NULL
                 WHERE token = ?1",
                [image.token.as_str()],
            )
            .map_err(|error| journal_error("LSV4012", "failed to acknowledge dispatch", error))?;
        finalize_merge_group(&transaction, image.token.as_str())?;
        transaction.commit().map_err(|error| {
            journal_error("LSV4012", "failed to commit completed effect", error)
        })?;
        Ok(step.clone())
    }
}
