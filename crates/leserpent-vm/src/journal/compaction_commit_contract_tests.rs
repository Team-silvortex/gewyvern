use super::*;

fn report(
    result: Result<JournalCompaction, Fault>,
) -> Result<(usize, usize, usize, Vec<ContinuationToken>), Fault> {
    result.map(|r| {
        (
            r.removed_records,
            r.remaining_completed,
            r.reclaimed_logical_bytes,
            r.pruned_tokens,
        )
    })
}
fn policy(keep: usize, batch: usize) -> RetentionPolicy {
    RetentionPolicy {
        max_completed_records: keep,
        max_delete_per_run: batch,
    }
}
fn memory_fixture(kind: usize) -> (EphemeralJournal, Vec<(ContinuationToken, Step)>) {
    let (mut j, mut local) = retention_tests::fixture();
    let group = ContinuationToken::from_wire("finished".into());
    let branch = ContinuationToken::from_wire("continuation-4".into());
    let oversized = || {
        Step::Fault(Fault {
            code: "fixture".into(),
            message: "x".repeat(MAX_JOURNAL_ENTRY_BYTES),
        })
    };
    match kind {
        0 => {}
        1 => local.clear(),
        2 => local.reverse(),
        3 => local.push(local[0].clone()),
        4 => {
            j.dispatches.remove(&branch);
        }
        5 => j
            .merge_groups
            .get_mut(&group)
            .unwrap()
            .branch_tokens
            .clear(),
        6 => j
            .merge_groups
            .get_mut(&ContinuationToken::from_wire("live".into()))
            .unwrap()
            .branch_tokens
            .clear(),
        7 => {
            j.merge_groups.get_mut(&group).unwrap().plan.branches[0] =
                "x".repeat(MAX_CONTINUATION_BYTES)
        }
        8 => j.merge_groups.get_mut(&group).unwrap().terminal_step = Some(oversized()),
        9 => local.iter_mut().find(|(t, _)| t == &branch).unwrap().1 = oversized(),
        10 => {
            j.dispatches.get_mut(&branch).unwrap().request.effect_id =
                "x".repeat(MAX_JOURNAL_ENTRY_BYTES)
        }
        11 => {
            j.dispatches.get_mut(&branch).unwrap().last_error = Some(EffectError {
                code: "fixture".into(),
                message: "x".repeat(MAX_JOURNAL_ENTRY_BYTES),
                class: crate::EffectErrorClass::Transient,
            })
        }
        _ => unreachable!(),
    }
    (j, local)
}
#[test]
fn memory_batch_matches_frozen_selection_accounting_refusals_and_full_map_atomicity() {
    let mut cases = 0;
    for kind in 0..12 {
        for keep in [0, 1, 2, 4, 10_000] {
            for batch in [0, 1, 3, 1_000] {
                let (mut old, old_local) = memory_fixture(kind);
                let (mut new, new_local) = memory_fixture(kind);
                let before = retention_tests::snapshot(&new);
                let locals_before = serde_json::to_vec(&new_local).unwrap();
                let old_result =
                    report(old.frozen_compact_completed(&old_local, &policy(keep, batch)));
                let new_result = report(new.compact_completed(&new_local, &policy(keep, batch)));
                assert_eq!(
                    new_result, old_result,
                    "kind={kind} keep={keep} batch={batch}"
                );
                assert_eq!(
                    retention_tests::snapshot(&new),
                    retention_tests::snapshot(&old)
                );
                assert_eq!(serde_json::to_vec(&new_local).unwrap(), locals_before);
                if new_result.is_err() {
                    assert_eq!(retention_tests::snapshot(&new), before);
                }
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 240);
}

#[cfg(feature = "sqlite-journal")]
mod durable {
    use super::super::group_recovery_contract_tests::Fixture;
    use super::*;
    const SOURCES: [&str; 4] = [
        r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
    ];
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
            .map(|name| {
                let mut q = c
                    .prepare(&format!("SELECT * FROM {name} ORDER BY rowid"))
                    .unwrap();
                let n = q.column_count();
                let rows = q
                    .query_map([], |r| {
                        (0..n)
                            .map(|i| r.get(i))
                            .collect::<rusqlite::Result<Vec<_>>>()
                    })
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                (name, rows)
            })
            .collect()
    }
    fn local(c: &Connection) -> Vec<(ContinuationToken, Step)> {
        let mut local = c.prepare("SELECT token, terminal_step FROM vm_effects WHERE state='completed' ORDER BY rowid").unwrap()
            .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, Vec<u8>>(1)?))).unwrap()
            .map(|r| { let (t, bytes) = r.unwrap(); (ContinuationToken::from_wire(t), serde_json::from_slice(&bytes).unwrap()) }).collect::<Vec<_>>();
        local.push((
            ContinuationToken::from_wire("continuation-999".into()),
            Step::Done(crate::Value::Scalar {
                value: crate::ScalarValue::Integer(1),
            }),
        ));
        local
    }
    fn mutate(c: &Connection, kind: usize) {
        c.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        match kind {
            0 => {}
            1 => c.execute_batch("UPDATE vm_merge_groups SET plan=x'7b';").unwrap(),
            2 => c.execute_batch("UPDATE vm_effects SET terminal_step=NULL WHERE state='completed';").unwrap(),
            3 => c.execute_batch("DELETE FROM vm_merge_branches WHERE position=0;").unwrap(),
            4 => c.execute_batch("UPDATE vm_merge_groups SET state='completed',terminal_step=x'7b';").unwrap(),
            5 => c.execute_batch("CREATE TRIGGER refuse_group BEFORE DELETE ON vm_merge_groups BEGIN SELECT RAISE(ABORT,'fixture group'); END;").unwrap(),
            6 => c.execute_batch("CREATE TRIGGER refuse_branch BEFORE DELETE ON vm_effects WHEN OLD.token=(SELECT MAX(token) FROM vm_effects) BEGIN SELECT RAISE(ABORT,'fixture branch'); END;").unwrap(),
            7 => c.execute_batch("CREATE TRIGGER skip_group BEFORE DELETE ON vm_merge_groups BEGIN SELECT RAISE(IGNORE); END;").unwrap(),
            8 => c.execute_batch("CREATE TRIGGER skip_branch BEFORE DELETE ON vm_effects BEGIN SELECT RAISE(IGNORE); END;").unwrap(),
            9 => c.execute_batch("DROP TABLE vm_merge_branches;").unwrap(),
            _ => unreachable!(),
        }
        c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
    }
    #[test]
    fn sqlite_batch_matches_frozen_rows_reports_errors_and_transaction_rollback() {
        let mut cases = 0;
        for source in SOURCES {
            for stage in 0..3 {
                for kind in 0..10 {
                    for (keep, batch) in [(0, 0), (0, 1), (10_000, 1_000)] {
                        let old_file = Fixture::new(source, stage);
                        let new_file = Fixture::new(source, stage);
                        let mut old = SqliteJournal::open(&old_file.0).unwrap();
                        let mut new = SqliteJournal::open(&new_file.0).unwrap();
                        let old_local = local(&old.connection);
                        let new_local = local(&new.connection);
                        mutate(&old.connection, kind);
                        mutate(&new.connection, kind);
                        let before = snapshot(&new.connection);
                        let expected =
                            report(old.frozen_compact_completed(&old_local, &policy(keep, batch)));
                        let actual =
                            report(new.compact_completed(&new_local, &policy(keep, batch)));
                        assert_eq!(
                            actual, expected,
                            "{source} stage={stage} kind={kind} keep={keep} batch={batch}"
                        );
                        assert_eq!(snapshot(&new.connection), snapshot(&old.connection));
                        assert!(new.connection.is_autocommit());
                        if actual.is_err() {
                            assert_eq!(snapshot(&new.connection), before);
                        }
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 360);
    }

    fn complete(vm: &mut crate::Vm) {
        for n in 0..8 {
            let Some(lease) = vm.claim_effect(1 + n * 2, 100).unwrap() else {
                break;
            };
            let crate::EffectOperation::Presentation(e) = &lease.request.operation else {
                panic!()
            };
            let crate::PresentationOperation::Focus { node_id } = &e.operation else {
                panic!()
            };
            let result = crate::EffectResult::Presentation(crate::PresentationResult::Focus {
                node_id: node_id.clone(),
            });
            assert!(!matches!(
                vm.acknowledge_effect(&lease, 2 + n * 2, result),
                Step::Fault(_)
            ));
        }
    }
    #[test]
    fn public_late_delete_failure_rolls_back_earlier_units_without_evicting_cache() {
        let single = r#"fn main() = ui.focus(node_id: "single")"#;
        let f = Fixture::new(single, 2);
        let mut vm = crate::Vm::open_journal(&f.0, 500).unwrap();
        for source in [SOURCES[0], single] {
            let program = leselang_hir::lower(&leselang_syntax::parse(source)).unwrap();
            assert!(matches!(
                vm.start(
                    &program,
                    crate::Principal::new("operator").unwrap(),
                    crate::CapabilitySet::new(["ui.presentation"]),
                    None
                ),
                Step::Effect(_) | Step::Effects(_)
            ));
            complete(&mut vm);
        }
        let connection = |vm: &crate::Vm| match &vm.journal {
            Journal::Sqlite(j) => snapshot(&j.connection),
            _ => unreachable!(),
        };
        let Journal::Sqlite(j) = &vm.journal else {
            unreachable!()
        };
        let branch: String = j
            .connection
            .query_row(
                "SELECT branch_token FROM vm_merge_branches ORDER BY position DESC LIMIT 1",
                [],
                |r| r.get(0),
            )
            .unwrap();
        j.connection.execute_batch(&format!("CREATE TRIGGER fail_late BEFORE DELETE ON vm_effects WHEN OLD.token='{branch}' BEGIN SELECT RAISE(ABORT,'late deletion'); END;")).unwrap();
        let before = connection(&vm);
        let cached = serde_json::to_vec(&vm.completed).unwrap();
        let error = vm.compact_journal(&policy(1, 10)).unwrap_err();
        assert_eq!(error.code, "LSV4030");
        assert!(error.message.contains("failed to delete merge branch"));
        assert_eq!(connection(&vm), before);
        assert_eq!(serde_json::to_vec(&vm.completed).unwrap(), cached);
        let Journal::Sqlite(j) = &vm.journal else {
            unreachable!()
        };
        assert!(j.connection.is_autocommit());
        j.connection
            .execute_batch("DROP TRIGGER fail_late;")
            .unwrap();
        assert_eq!(
            vm.compact_journal(&policy(1, 10)).unwrap().removed_records,
            2
        );
        assert_eq!(vm.completed_count(), 1);
        drop(vm);
        let mut reopened = crate::Vm::open_journal(&f.0, 500).unwrap();
        assert_eq!(reopened.completed_count(), 1);
        assert!(reopened.claim_effect(100, 100).unwrap().is_none());
    }

    #[test]
    fn public_compaction_reconciles_other_worker_cache_and_reopens_without_reusing_ids() {
        let f = Fixture::new(SOURCES[0], 2);
        let mut owner = crate::Vm::open_journal(&f.0, 500).unwrap();
        let program = leselang_hir::lower(&leselang_syntax::parse(
            r#"fn main() = ui.focus(node_id: "new")"#,
        ))
        .unwrap();
        assert!(matches!(
            owner.start(
                &program,
                crate::Principal::new("operator").unwrap(),
                crate::CapabilitySet::new(["ui.presentation"]),
                None
            ),
            Step::Effect(_)
        ));
        complete(&mut owner);
        let mut peer = crate::Vm::open_journal(&f.0, 500).unwrap();
        let watermark = peer.next_effect_id;
        assert_eq!(peer.completed_count(), 3);
        assert_eq!(
            owner
                .compact_journal(&policy(1, 10))
                .unwrap()
                .removed_records,
            1
        );
        assert_eq!(owner.completed_count(), 1);
        assert_eq!(peer.completed_count(), 3);
        assert_eq!(
            peer.compact_journal(&policy(1, 10))
                .unwrap()
                .removed_records,
            0
        );
        assert_eq!(peer.completed_count(), 1);
        drop(owner);
        drop(peer);
        let mut reopened = crate::Vm::open_journal(&f.0, 500).unwrap();
        assert_eq!(reopened.completed_count(), 1);
        assert_eq!(reopened.next_effect_id, watermark);
        assert!(reopened.claim_effect(100, 100).unwrap().is_none());
        let Step::Effect(next) = reopened.start(
            &program,
            crate::Principal::new("operator").unwrap(),
            crate::CapabilitySet::new(["ui.presentation"]),
            None,
        ) else {
            panic!()
        };
        assert_eq!(
            crate::canonical_continuation_sequence(&next.continuation.token),
            Some(watermark)
        );
    }
}

// Frozen at 63df912e before compaction extraction; unchanged by the preceding sequence-gate slice.
impl EphemeralJournal {
    fn frozen_compact_completed(
        &mut self,
        local_completed: &[(ContinuationToken, Step)],
        policy: &RetentionPolicy,
    ) -> Result<JournalCompaction, Fault> {
        let protected = self
            .merge_groups
            .values()
            .flat_map(|group| group.branch_tokens.iter())
            .collect::<BTreeSet<_>>();
        let mut units = local_completed
            .iter()
            .filter(|(token, _)| !protected.contains(token))
            .map(|(token, _)| EphemeralCompactionUnit::Effect(token.clone()))
            .collect::<Vec<_>>();
        for (token, group) in &self.merge_groups {
            if group.terminal_step.is_some() {
                let age_token = group.branch_tokens.first().ok_or_else(|| {
                    journal_fault("LSV4007", "completed merge group has no branches")
                })?;
                units.push(EphemeralCompactionUnit::Merge {
                    group_token: token.clone(),
                    age_token: age_token.clone(),
                });
            }
        }
        units.sort_by(|left, right| continuation_age_order(left.age_token(), right.age_token()));
        let completed_count = units.len();
        let remove_count = policy.removal_count(completed_count);
        let mut reclaimed_logical_bytes = 0usize;
        let mut removed_tokens = Vec::with_capacity(remove_count);
        // Preflight every selected unit before mutating any map. Counting shares
        // the wire encoder's bounds without materializing disposable JSON buffers.
        for unit in units.iter().take(remove_count) {
            let tokens = match unit {
                EphemeralCompactionUnit::Effect(token) => std::slice::from_ref(token),
                EphemeralCompactionUnit::Merge { group_token, .. } => {
                    let group = self.merge_groups.get(group_token).ok_or_else(|| {
                        journal_fault("LSV4033", "selected merge group disappeared")
                    })?;
                    let terminal_step = group.terminal_step.as_ref().ok_or_else(|| {
                        journal_fault("LSV4007", "selected merge group is not completed")
                    })?;
                    reclaimed_logical_bytes = reclaimed_logical_bytes
                        .saturating_add(measure_json_capped(
                            &group.plan,
                            MAX_CONTINUATION_BYTES,
                            "merge plan",
                        )?)
                        .saturating_add(measure_json_capped(
                            terminal_step,
                            MAX_JOURNAL_ENTRY_BYTES,
                            "merge result",
                        )?);
                    group.branch_tokens.as_slice()
                }
            };
            for token in tokens {
                if let Some((_, step)) = local_completed.iter().find(|(local, _)| local == token) {
                    reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(
                        measure_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "completed step")?,
                    );
                }
                if let Some(dispatch) = self.dispatches.get(token) {
                    reclaimed_logical_bytes =
                        reclaimed_logical_bytes.saturating_add(measure_json_capped(
                            &dispatch.request,
                            MAX_JOURNAL_ENTRY_BYTES,
                            "effect request",
                        )?);
                    if let Some(error) = &dispatch.last_error {
                        reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(
                            measure_json_capped(error, MAX_JOURNAL_ENTRY_BYTES, "effect error")?,
                        );
                    }
                }
                removed_tokens.push(token.clone());
            }
        }
        for unit in units.into_iter().take(remove_count) {
            if let EphemeralCompactionUnit::Merge { group_token, .. } = unit {
                self.merge_groups.remove(&group_token);
            }
        }
        for token in &removed_tokens {
            self.dispatches.remove(token);
        }
        if !removed_tokens.is_empty() {
            // Match SQLite's audit cascade once per batch, retaining live group members.
            self.debugger_audits
                .retain(|_, audit| self.dispatches.contains_key(&audit.token));
        }
        Ok(JournalCompaction {
            removed_records: remove_count,
            pruned_tokens: removed_tokens,
            remaining_completed: completed_count.saturating_sub(remove_count),
            reclaimed_logical_bytes,
        })
    }
}

#[cfg(feature = "sqlite-journal")]
impl SqliteJournal {
    fn frozen_compact_completed(
        &mut self,
        local_completed: &[(ContinuationToken, Step)],
        policy: &RetentionPolicy,
    ) -> Result<JournalCompaction, Fault> {
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| {
                journal_error("LSV4030", "failed to lock journal compaction", error)
            })?;
        let completed_count: i64 = transaction
            .query_row(
                "SELECT
                   (SELECT COUNT(*) FROM vm_effects e
                    WHERE e.state = 'completed'
                      AND NOT EXISTS (
                        SELECT 1 FROM vm_merge_branches b WHERE b.branch_token = e.token
                      )) +
                   (SELECT COUNT(*) FROM vm_merge_groups WHERE state = 'completed')",
                [],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error(
                    "LSV4030",
                    "failed to count completed journal records",
                    error,
                )
            })?;
        let completed_count = usize::try_from(completed_count)
            .map_err(|_| journal_fault("LSV4007", "completed journal count is invalid"))?;
        let remove_count = policy.removal_count(completed_count);
        if remove_count == 0 {
            let pruned_tokens = compaction_records_contract_tests::frozen_stale_local_completed(
                &transaction,
                local_completed,
            )?;
            transaction.commit().map_err(|error| {
                journal_error("LSV4030", "failed to close journal compaction", error)
            })?;
            return Ok(JournalCompaction {
                removed_records: 0,
                pruned_tokens,
                remaining_completed: completed_count,
                reclaimed_logical_bytes: 0,
            });
        }

        let limit = i64::try_from(remove_count)
            .map_err(|_| journal_fault("LSV4030", "journal compaction batch is invalid"))?;
        type RawCandidate = (i64, String);
        let candidates = {
            let mut statement = transaction
                .prepare(
                    "SELECT kind, token FROM (
                       SELECT 0 AS kind, e.token AS token, e.rowid AS age
                       FROM vm_effects e
                       WHERE e.state = 'completed'
                         AND NOT EXISTS (
                           SELECT 1 FROM vm_merge_branches b WHERE b.branch_token = e.token
                         )
                       UNION ALL
                       SELECT 1 AS kind, g.token AS token, MIN(e.rowid) AS age
                       FROM vm_merge_groups g
                       JOIN vm_merge_branches b ON b.group_token = g.token
                       JOIN vm_effects e ON e.token = b.branch_token
                       WHERE g.state = 'completed'
                       GROUP BY g.token
                     ) ORDER BY age ASC, kind ASC, token ASC
                     LIMIT ?1",
                )
                .map_err(|error| {
                    journal_error("LSV4030", "failed to prepare journal compaction", error)
                })?;
            let rows = statement
                .query_map([limit], |row| Ok((row.get(0)?, row.get(1)?)))
                .map_err(|error| {
                    journal_error("LSV4030", "failed to scan journal compaction", error)
                })?;
            rows.collect::<Result<Vec<RawCandidate>, _>>()
                .map_err(|error| {
                    journal_error("LSV4030", "failed to read journal compaction", error)
                })?
        };

        let mut removed_units = 0usize;
        let mut reclaimed_logical_bytes = 0usize;
        for (kind, token) in candidates {
            match kind {
                0 => {
                    let logical_bytes =
                        compaction_records_contract_tests::frozen_completed_effect_logical_bytes(
                            &transaction,
                            &token,
                        )?;
                    let deleted = transaction
                        .execute(
                            "DELETE FROM vm_effects
                             WHERE token = ?1 AND state = 'completed'
                               AND NOT EXISTS (
                                 SELECT 1 FROM vm_merge_branches
                                 WHERE branch_token = vm_effects.token
                               )",
                            [&token],
                        )
                        .map_err(|error| {
                            journal_error(
                                "LSV4030",
                                "failed to delete completed journal record",
                                error,
                            )
                        })?;
                    if deleted != 1 {
                        return Err(journal_fault(
                            "LSV4030",
                            "completed journal record changed during compaction",
                        ));
                    }
                    reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(logical_bytes);
                }
                1 => {
                    reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(
                        compaction_records_contract_tests::frozen_delete_completed_merge_group(
                            &transaction,
                            &token,
                        )?,
                    );
                }
                _ => {
                    return Err(journal_fault(
                        "LSV4007",
                        "journal compaction candidate kind is invalid",
                    ));
                }
            }
            removed_units += 1;
        }
        let pruned_tokens = compaction_records_contract_tests::frozen_stale_local_completed(
            &transaction,
            local_completed,
        )?;
        transaction.commit().map_err(|error| {
            journal_error("LSV4030", "failed to commit journal compaction", error)
        })?;
        Ok(JournalCompaction {
            remaining_completed: completed_count.saturating_sub(removed_units),
            removed_records: removed_units,
            pruned_tokens,
            reclaimed_logical_bytes,
        })
    }
}
