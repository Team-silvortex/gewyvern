use super::*;

fn value(result: Result<MergeProgress, Fault>) -> serde_json::Value {
    use serde_json::json;
    match result {
        Err(e) => json!({"error": e}),
        Ok(MergeProgress::Standalone) => json!({"standalone": true}),
        Ok(MergeProgress::Next {
            request,
            completed_tokens,
        }) => json!({"next": request, "completed": completed_tokens}),
        Ok(MergeProgress::Pending {
            merge_token,
            completed_branches,
            total_branches,
        }) => {
            json!({"pending": merge_token, "completed": completed_branches, "total": total_branches})
        }
        Ok(MergeProgress::Completed {
            merge_token,
            step,
            branch_tokens,
        }) => json!({"owner": merge_token, "step": step, "branches": branch_tokens}),
    }
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
        j.merge_groups
            .iter()
            .map(|(t, g)| (t, &g.plan, &g.branch_tokens, &g.terminal_step))
            .collect::<Vec<_>>(),
        j.debugger_audits.len(),
    ))
    .unwrap()
}
#[test]
fn memory_projection_matches_frozen_for_all_operations_missing_rows_and_partial_states() {
    let mut cases = 0;
    for input in crate::continuation_contract_tests::requests() {
        for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
            for kind in 0..12 {
                let mut j = EphemeralJournal::default();
                let second = crate::Vm::build_effect_request(
                    &leselang_hir::Effect::UiFocus {
                        node_id: "second".into(),
                    },
                    leselang_hir::Type::UiFocus,
                    500,
                    crate::Principal::new("operator").unwrap(),
                    crate::CapabilitySet::new(["ui.presentation"]),
                    None,
                    1000,
                    None,
                    100,
                )
                .unwrap();
                for r in [&input, &second] {
                    j.record_pending(Some(r), SchedulerLimits::default())
                        .unwrap();
                }
                let first = input.continuation.token.clone();
                let next = second.continuation.token.clone();
                let owner = ContinuationToken::from_wire("merge-600".into());
                let mut plan = MergePlan::new(["a", "b"]).unwrap();
                plan.order = order;
                if kind != 0 {
                    j.merge_groups.insert(
                        owner.clone(),
                        EphemeralMergeGroup {
                            plan: plan.clone(),
                            branch_tokens: vec![first.clone(), next.clone()],
                            terminal_step: None,
                        },
                    );
                }
                match kind {
                    0 | 1 => {}
                    2 => j.dispatches.get_mut(&first).unwrap().acknowledged = true,
                    3 => j.dispatches.get_mut(&first).unwrap().terminal_step = Some(done()),
                    4 => {
                        let d = j.dispatches.get_mut(&first).unwrap();
                        d.acknowledged = true;
                        d.terminal_step = Some(done());
                    }
                    5 => {
                        for d in j.dispatches.values_mut() {
                            d.acknowledged = true;
                            d.terminal_step = Some(done());
                        }
                    }
                    6 => {
                        j.dispatches.remove(&first);
                    }
                    7 => {
                        j.merge_groups.get_mut(&owner).unwrap().terminal_step =
                            Some(Step::Yield(input.continuation.clone()))
                    }
                    8 => j
                        .merge_groups
                        .get_mut(&owner)
                        .unwrap()
                        .branch_tokens
                        .push(first.clone()),
                    9 => j.dispatches.clear(),
                    10 => {
                        j.merge_groups.insert(
                            ContinuationToken::from_wire("merge-1".into()),
                            EphemeralMergeGroup {
                                plan,
                                branch_tokens: vec![first.clone()],
                                terminal_step: Some(done()),
                            },
                        );
                    }
                    11 => {
                        j.dispatches
                            .get_mut(&first)
                            .unwrap()
                            .request
                            .continuation
                            .token = ContinuationToken::from_wire("invalid".into())
                    }
                    _ => unreachable!(),
                }
                let before = memory_snapshot(&j);
                assert_eq!(
                    value(Ok(j.merge_progress(&first))),
                    value(Ok(j.frozen_merge_progress(&first))),
                    "{order:?} case {kind}"
                );
                assert_eq!(memory_snapshot(&j), before);
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 1680);
}
fn start_group(vm: &mut crate::Vm, order: ExecutionOrder, kind: usize) {
    let branches = ["left", "right"].map(|name| leselang_hir::HirBranch {
        name: name.into(),
        effect: leselang_hir::Effect::UiFocus {
            node_id: name.into(),
        },
        result_type: leselang_hir::Type::UiFocus,
    });
    assert!(!matches!(
        vm.start_group(
            &branches,
            order,
            crate::Principal::new("operator").unwrap(),
            crate::CapabilitySet::new(["ui.presentation"]),
            None,
            1000,
            None,
            100,
            crate::group_start_fixture::binding(kind)
        ),
        Step::Fault(_)
    ));
}
fn reply(lease: &DispatchLease) -> crate::EffectResult {
    let crate::EffectOperation::Presentation(e) = &lease.request.operation else {
        panic!()
    };
    let crate::PresentationOperation::Focus { node_id } = &e.operation else {
        panic!()
    };
    crate::EffectResult::Presentation(crate::PresentationResult::Focus {
        node_id: node_id.clone(),
    })
}
fn drain_and_compare(vm: &mut crate::Vm) {
    let branch = vm.pending_continuations()[0].token.clone();
    for i in 0..8 {
        let old = match &vm.journal {
            Journal::Ephemeral(j) => Ok(j.frozen_merge_progress(&branch)),
            #[cfg(feature = "sqlite-journal")]
            Journal::Sqlite(j) => j.frozen_merge_progress(&branch),
        };
        assert_eq!(value(vm.journal.merge_progress(&branch)), value(old));
        if vm.pending_count() == 0 {
            assert!(matches!(
                vm.journal.merge_progress(&branch).unwrap(),
                MergeProgress::Completed { .. }
            ));
            return;
        }
        let lease = vm.claim_effect(i * 2 + 1, 100).unwrap().unwrap();
        assert!(!matches!(
            vm.acknowledge_effect(&lease, i * 2 + 2, reply(&lease)),
            Step::Fault(_)
        ));
    }
    panic!("group did not finish");
}
#[test]
fn memory_bound_groups_match_frozen_through_public_acknowledgement() {
    let mut cases = 0;
    for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
        for kind in 0..6 {
            let mut vm = crate::Vm::new(500);
            start_group(&mut vm, order, kind);
            drain_and_compare(&mut vm);
            cases += 1;
        }
    }
    assert_eq!(cases, 12);
}

#[cfg(feature = "sqlite-journal")]
mod durable {
    use super::super::group_recovery_contract_tests::Fixture;
    use super::*;
    use leselang_command::reference_group_progress::{DurableGroupProgress, StoredGroupProgress};
    const SOURCES: [&str; 4] = [
        r#"fn main() = seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
        r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")), body: ui.focus(node_id: "c"))"#,
    ];
    fn snapshot(c: &Connection) -> Vec<(String, Vec<Vec<rusqlite::types::Value>>)> {
        let names = c
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        names
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
    fn mutate(c: &Connection, kind: usize) {
        c.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        match kind {
            0 | 1 => {}
            2 => c
                .execute_batch("UPDATE vm_merge_groups SET plan=x'7b';")
                .unwrap(),
            3 => c
                .execute_batch("UPDATE vm_dispatches SET request=x'7b';")
                .unwrap(),
            4 => {
                c.execute(
                    "UPDATE vm_dispatches SET request=?1",
                    [vec![b' '; MAX_JOURNAL_ENTRY_BYTES + 1]],
                )
                .unwrap();
            }
            5 => {
                let bytes: Vec<u8> = c
                    .query_row("SELECT request FROM vm_dispatches LIMIT 1", [], |r| {
                        r.get(0)
                    })
                    .unwrap();
                let mut r: EffectRequest = serde_json::from_slice(&bytes).unwrap();
                r.continuation.token = ContinuationToken::from_wire("invalid".into());
                c.execute(
                    "UPDATE vm_dispatches SET request=?1",
                    [serde_json::to_vec(&r).unwrap()],
                )
                .unwrap();
            }
            6 => c
                .execute_batch("UPDATE vm_merge_groups SET state='invalid';")
                .unwrap(),
            7 => {
                c.execute(
                    "UPDATE vm_merge_groups SET state='pending',terminal_step=?1",
                    [serde_json::to_vec(&done()).unwrap()],
                )
                .unwrap();
            }
            8 => c
                .execute_batch("UPDATE vm_merge_groups SET state='completed',terminal_step=NULL;")
                .unwrap(),
            9 => {
                let bytes: Vec<u8> = c
                    .query_row("SELECT image FROM vm_effects LIMIT 1", [], |r| r.get(0))
                    .unwrap();
                let image: ContinuationImage = serde_json::from_slice(&bytes).unwrap();
                c.execute(
                    "UPDATE vm_merge_groups SET state='completed',terminal_step=?1",
                    [serde_json::to_vec(&Step::Yield(image)).unwrap()],
                )
                .unwrap();
            }
            10 => c.execute_batch("DELETE FROM vm_merge_branches;").unwrap(),
            11 => c.execute_batch("DELETE FROM vm_effects;").unwrap(),
            12 => c.execute_batch("DROP TABLE vm_dispatches;").unwrap(),
            13 => c
                .execute_batch("UPDATE vm_merge_groups SET execution_order='unknown';")
                .unwrap(),
            14 => {
                c.execute(
                    "UPDATE vm_merge_groups SET plan=?1",
                    [vec![b' '; MAX_CONTINUATION_BYTES + 1]],
                )
                .unwrap();
            }
            15 => c.execute_batch("DROP TABLE vm_merge_groups;").unwrap(),
            _ => unreachable!(),
        }
        c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
    }
    #[test]
    fn sqlite_progress_matches_frozen_decode_faults_and_is_read_only() {
        let mut cases = 0;
        for source in SOURCES {
            for stage in 0..3 {
                for kind in 0..16 {
                    let f = Fixture::new(source, stage);
                    let j = SqliteJournal {
                        connection: Connection::open(&f.0).unwrap(),
                    };
                    let branch = if kind == 1 {
                        ContinuationToken::from_wire("continuation-999".into())
                    } else {
                        ContinuationToken::from_wire(j.connection.query_row("SELECT branch_token FROM vm_merge_branches ORDER BY position LIMIT 1", [], |r| r.get(0)).unwrap())
                    };
                    mutate(&j.connection, kind);
                    let before = snapshot(&j.connection);
                    let changes = j.connection.total_changes();
                    assert_eq!(
                        value(j.merge_progress(&branch)),
                        value(j.frozen_merge_progress(&branch)),
                        "{source} stage {stage} case {kind}"
                    );
                    assert_eq!(snapshot(&j.connection), before);
                    assert_eq!(j.connection.total_changes(), changes);
                    assert!(j.connection.is_autocommit());
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 192);
    }
    #[test]
    fn durable_bound_groups_match_frozen_through_public_acknowledgement_and_reopen() {
        let mut cases = 0;
        for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
            for kind in 0..6 {
                let f = Fixture::new(SOURCES[0], 0);
                let path = f.0.with_extension("bound.sqlite3");
                let mut vm = crate::Vm::open_journal(&path, 500).unwrap();
                start_group(&mut vm, order, kind);
                let branch = vm.pending_continuations()[0].token.clone();
                drain_and_compare(&mut vm);
                let before = value(vm.journal.merge_progress(&branch));
                drop(vm);
                let vm = crate::Vm::open_journal(&path, 500).unwrap();
                assert_eq!(value(vm.journal.merge_progress(&branch)), before);
                cases += 1;
            }
        }
        assert_eq!(cases, 12);
    }
    struct Interleaved<'a, 'connection, F> {
        native: group_progress::SqliteGroupProgress<'a, 'connection>,
        after_locate: Option<F>,
    }
    impl<F: FnOnce()> DurableGroupProgress for Interleaved<'_, '_, F> {
        fn locate(&mut self, branch: &str) -> Result<Option<StoredGroupProgress>, Fault> {
            let r = self.native.locate(branch)?;
            self.after_locate.take().unwrap()();
            Ok(r)
        }
        fn plan(&mut self, t: &str) -> Result<Vec<u8>, Fault> {
            self.native.plan(t)
        }
        fn next_request(&mut self, t: &str) -> Result<Vec<u8>, Fault> {
            self.native.next_request(t)
        }
        fn completed_tokens(&mut self, t: &str) -> Result<Vec<ContinuationToken>, Fault> {
            self.native.completed_tokens(t)
        }
        fn branch_tokens(&mut self, t: &str) -> Result<Vec<ContinuationToken>, Fault> {
            self.native.branch_tokens(t)
        }
    }
    #[test]
    fn wal_snapshot_keeps_progress_request_and_prefix_on_one_commit() {
        let f = Fixture::new(SOURCES[0], 0);
        let j = SqliteJournal {
            connection: Connection::open(&f.0).unwrap(),
        };
        let mut writer = crate::Vm::open_journal(&f.0, 500).unwrap();
        j.connection
            .pragma_update(None, "journal_mode", "WAL")
            .unwrap();
        let Journal::Sqlite(w) = &writer.journal else {
            panic!()
        };
        w.connection
            .pragma_update(None, "journal_mode", "WAL")
            .unwrap();
        let branch = writer.pending_continuations()[0].token.clone();
        let before = value(j.merge_progress(&branch));
        let tx = j.connection.unchecked_transaction().unwrap();
        let mut host = Interleaved {
            native: group_progress::SqliteGroupProgress(&tx),
            after_locate: Some(|| {
                let lease = writer.claim_effect(1, 100).unwrap().unwrap();
                assert!(matches!(
                    writer.acknowledge_effect(&lease, 2, reply(&lease)),
                    Step::Effect(_)
                ));
            }),
        };
        assert_eq!(
            value(leselang_command::reference_group_progress::read_durable(
                &mut host, &branch
            )),
            before
        );
        assert!(!j.connection.is_autocommit());
        drop(tx);
        assert!(j.connection.is_autocommit());
        let after = value(j.merge_progress(&branch));
        assert_ne!(after, before);
        assert_eq!(after["completed"], serde_json::json!([branch]));
        assert_eq!(j.connection.total_changes(), 0);
    }
}

// Frozen pre-extraction bodies from the working tree based on 8b792476.
impl EphemeralJournal {
    fn frozen_merge_progress(&self, branch_token: &ContinuationToken) -> MergeProgress {
        let Some((merge_token, group)) = self
            .merge_groups
            .iter()
            .find(|(_, group)| group.branch_tokens.contains(branch_token))
        else {
            return MergeProgress::Standalone;
        };
        if let Some(step) = &group.terminal_step {
            return MergeProgress::Completed {
                merge_token: merge_token.clone(),
                step: Box::new(step.clone()),
                branch_tokens: group.branch_tokens.clone(),
            };
        }
        if (group.plan.order.is_sequential()
            || group
                .plan
                .result_binding
                .as_ref()
                .is_some_and(|binding| binding.has_successor(&group.plan)))
            && let Some(dispatch) = group
                .branch_tokens
                .iter()
                .filter_map(|token| self.dispatches.get(token))
                .find(|dispatch| !dispatch.acknowledged)
        {
            return MergeProgress::Next {
                request: Box::new(dispatch.request.clone()),
                completed_tokens: group
                    .branch_tokens
                    .iter()
                    .filter(|token| {
                        self.dispatches
                            .get(*token)
                            .is_some_and(|dispatch| dispatch.acknowledged)
                    })
                    .cloned()
                    .collect(),
            };
        }
        MergeProgress::Pending {
            merge_token: merge_token.clone(),
            completed_branches: group
                .branch_tokens
                .iter()
                .filter(|token| {
                    self.dispatches
                        .get(*token)
                        .is_some_and(|dispatch| dispatch.terminal_step.is_some())
                })
                .count(),
            total_branches: group.branch_tokens.len(),
        }
    }
}

#[cfg(feature = "sqlite-journal")]
impl SqliteJournal {
    fn frozen_merge_progress(
        &self,
        branch_token: &ContinuationToken,
    ) -> Result<MergeProgress, Fault> {
        type RawProgress = (String, String, Option<Vec<u8>>, i64, i64, String);
        // Progress, current request and completed prefix must describe the same commit.
        let transaction = self.connection.unchecked_transaction().map_err(|error| {
            journal_error("LSV4033", "failed to snapshot merge progress", error)
        })?;
        let progress: Option<RawProgress> = transaction
            .query_row(
                "SELECT g.token, g.state, g.terminal_step,
                        SUM(CASE WHEN e.state = 'completed' THEN 1 ELSE 0 END),
                        COUNT(*), g.execution_order
                 FROM vm_merge_groups g
                 JOIN vm_merge_branches own ON own.group_token = g.token
                 JOIN vm_merge_branches branches ON branches.group_token = g.token
                 JOIN vm_effects e ON e.token = branches.branch_token
                 WHERE own.branch_token = ?1
                 GROUP BY g.token, g.state, g.terminal_step",
                [branch_token.as_str()],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                        row.get(5)?,
                    ))
                },
            )
            .optional()
            .map_err(|error| journal_error("LSV4033", "failed to read merge progress", error))?;
        let Some((merge_token, state, terminal, completed, total, order)) = progress else {
            return Ok(MergeProgress::Standalone);
        };
        let merge_token = ContinuationToken::from_wire(merge_token);
        let completed_branches = usize::try_from(completed)
            .map_err(|_| journal_fault("LSV4007", "merge completion count is invalid"))?;
        let total_branches = usize::try_from(total)
            .map_err(|_| journal_fault("LSV4007", "merge branch count is invalid"))?;
        match (state.as_str(), terminal) {
            ("pending", None) if completed_branches < total_branches => {
                // One pending request may be the last parallel member or its admitted successor.
                let successor_ready = if order == "parallel"
                    && completed_branches + 1 == total_branches
                {
                    let bytes: Vec<u8> = transaction
                        .query_row(
                            "SELECT plan FROM vm_merge_groups WHERE token = ?1",
                            [merge_token.as_str()],
                            |row| row.get(0),
                        )
                        .map_err(|error| {
                            journal_error("LSV4033", "failed to inspect parallel successor", error)
                        })?;
                    let plan: MergePlan = decode_bounded(&bytes, MAX_CONTINUATION_BYTES)?;
                    plan.order.has_group_successor()
                        && plan
                            .result_binding
                            .as_ref()
                            .is_some_and(|binding| binding.has_successor(&plan))
                } else {
                    false
                };
                if order == "sequential" || successor_ready {
                    let bytes: Vec<u8> = transaction
                        .query_row(
                            "SELECT d.request FROM vm_merge_branches b
                         JOIN vm_effects e ON e.token = b.branch_token
                         JOIN vm_dispatches d ON d.token = b.branch_token
                         WHERE b.group_token = ?1 AND e.state = 'pending'
                         ORDER BY b.position LIMIT 1",
                            [merge_token.as_str()],
                            |row| row.get(0),
                        )
                        .map_err(|error| {
                            journal_error("LSV4033", "failed to read next sequence step", error)
                        })?;
                    let request: EffectRequest = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
                    validate_effect_request(&request)?;
                    let mut statement = transaction.prepare(
                        "SELECT b.branch_token FROM vm_merge_branches b JOIN vm_effects e ON e.token = b.branch_token
                         WHERE b.group_token = ?1 AND e.state = 'completed' ORDER BY b.position",
                    ).map_err(|error| journal_error("LSV4033", "failed to inspect completed prefix", error))?;
                    let completed_tokens = statement
                        .query_map([merge_token.as_str()], |row| {
                            Ok(ContinuationToken::from_wire(row.get(0)?))
                        })
                        .map_err(|error| {
                            journal_error("LSV4033", "failed to load completed prefix", error)
                        })?
                        .collect::<Result<Vec<_>, _>>()
                        .map_err(|error| {
                            journal_error("LSV4033", "invalid completed prefix", error)
                        })?;
                    return Ok(MergeProgress::Next {
                        request: Box::new(request),
                        completed_tokens,
                    });
                }
                Ok(MergeProgress::Pending {
                    merge_token,
                    completed_branches,
                    total_branches,
                })
            }
            ("completed", Some(bytes)) if completed_branches == total_branches => {
                let step: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
                validate_terminal_step(&step)?;
                let mut statement = transaction.prepare(
                    "SELECT branch_token FROM vm_merge_branches WHERE group_token = ?1 ORDER BY position"
                ).map_err(|error| journal_error("LSV4033", "failed to read completed group", error))?;
                let branch_tokens = statement
                    .query_map([merge_token.as_str()], |row| {
                        Ok(ContinuationToken::from_wire(row.get(0)?))
                    })
                    .map_err(|error| {
                        journal_error("LSV4033", "failed to read completed branches", error)
                    })?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| {
                        journal_error("LSV4033", "invalid completed branches", error)
                    })?;
                Ok(MergeProgress::Completed {
                    merge_token,
                    step: Box::new(step),
                    branch_tokens,
                })
            }
            _ => Err(journal_fault(
                "LSV4007",
                "merge progress has an invalid durable state",
            )),
        }
    }
}
