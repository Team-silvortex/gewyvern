use super::*;

// Frozen pre-extraction bodies from the working tree based on 8b792476.
// Storage and the original merge/successor evaluator remain shared, not orchestration.
impl EphemeralJournal {
    fn frozen_finalize_merge_group(
        &mut self,
        branch_token: &ContinuationToken,
    ) -> Result<(), Fault> {
        let group_token = self
            .merge_groups
            .iter()
            .find(|(_, group)| group.branch_tokens.contains(branch_token))
            .map(|(token, _)| token.clone());
        let Some(group_token) = group_token else {
            return Ok(());
        };
        let group = self
            .merge_groups
            .get(&group_token)
            .ok_or_else(|| journal_fault("LSV4033", "located merge group disappeared"))?;
        if group.terminal_step.is_some() {
            return Ok(());
        }
        let plan = group.plan.clone();
        let branch_tokens = group.branch_tokens.clone();
        if plan.order.is_sequential()
            && let Some(stop) = sequence_stop(branch_tokens.iter().filter_map(|token| {
                self.dispatches
                    .get(token)
                    .and_then(|dispatch| dispatch.terminal_step.as_ref())
            }))?
        {
            for token in &branch_tokens {
                if let Some(dispatch) = self.dispatches.get_mut(token)
                    && !dispatch.acknowledged
                {
                    dispatch.acknowledged = true;
                    dispatch.lease_expires_at_ms = None;
                    dispatch.terminal_step = Some(stop.clone());
                }
            }
            self.merge_groups
                .get_mut(&group_token)
                .ok_or_else(|| journal_fault("LSV4007", "sequence disappeared"))?
                .terminal_step = Some(stop);
            return Ok(());
        }
        let mut completions = Vec::with_capacity(branch_tokens.len());
        for (branch, token) in plan.branches.iter().zip(&branch_tokens) {
            let Some(step) = self
                .dispatches
                .get(token)
                .and_then(|dispatch| dispatch.terminal_step.clone())
            else {
                return Ok(());
            };
            completions.push(BranchCompletion {
                branch: branch.clone(),
                outcome: terminal_step_outcome(step)?,
            });
        }
        let merged =
            if group_binding::needs_successor(&plan) || plan.order.captures_group_successor() {
                let prefix = group_binding::merge_prefix(&plan, completions)?;
                let Some(terminal) =
                    group_binding::admit_ephemeral(self, &group_token, &plan, &prefix)?
                else {
                    return Ok(());
                };
                terminal
            } else {
                group_binding::finish(&plan, completions)?
            };
        self.merge_groups
            .get_mut(&group_token)
            .ok_or_else(|| journal_fault("LSV4033", "located merge group disappeared"))?
            .terminal_step = Some(merged);
        Ok(())
    }
}

#[cfg(feature = "sqlite-journal")]
fn frozen_finalize_merge_group(
    transaction: &rusqlite::Transaction<'_>,
    branch_token: &str,
) -> Result<(), Fault> {
    type RawGroup = (String, Vec<u8>, String, Option<Vec<u8>>);
    let group: Option<RawGroup> = transaction
        .query_row(
            "SELECT g.token, g.plan, g.state, g.terminal_step
             FROM vm_merge_groups g
             JOIN vm_merge_branches b ON b.group_token = g.token
             WHERE b.branch_token = ?1",
            [branch_token],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()
        .map_err(|error| journal_error("LSV4033", "failed to locate merge group", error))?;
    let Some((group_token, plan_bytes, state, terminal_bytes)) = group else {
        return Ok(());
    };
    if state == "completed" {
        let terminal: Step = decode_bounded(
            &terminal_bytes.ok_or_else(|| {
                journal_fault("LSV4007", "completed merge group has no terminal step")
            })?,
            MAX_JOURNAL_ENTRY_BYTES,
        )?;
        validate_terminal_step(&terminal)?;
        return Ok(());
    }
    if state != "pending" || terminal_bytes.is_some() {
        return Err(journal_fault(
            "LSV4007",
            "merge group has an invalid durable state",
        ));
    }

    type RawBranch = (String, String, Option<Vec<u8>>);
    let branches = {
        let mut statement = transaction
            .prepare(
                "SELECT b.branch_name, e.state, e.terminal_step
                 FROM vm_merge_branches b
                 JOIN vm_effects e ON e.token = b.branch_token
                 WHERE b.group_token = ?1
                 ORDER BY b.position ASC",
            )
            .map_err(|error| {
                journal_error("LSV4033", "failed to prepare merge completion", error)
            })?;
        statement
            .query_map([&group_token], |row| {
                Ok((row.get(0)?, row.get(1)?, row.get(2)?))
            })
            .map_err(|error| journal_error("LSV4033", "failed to scan merge completion", error))?
            .collect::<Result<Vec<RawBranch>, _>>()
            .map_err(|error| journal_error("LSV4033", "failed to read merge completion", error))?
    };
    let plan: MergePlan = decode_bounded(&plan_bytes, MAX_CONTINUATION_BYTES)?;
    if plan.order.is_sequential() {
        let terminal_steps = branches
            .iter()
            .filter_map(|(_, _, terminal)| terminal.as_ref())
            .map(|bytes| decode_bounded::<Step>(bytes, MAX_JOURNAL_ENTRY_BYTES))
            .collect::<Result<Vec<_>, _>>()?;
        if let Some(stop) = sequence_stop(terminal_steps.iter())? {
            let bytes = encode_json_capped(&stop, MAX_JOURNAL_ENTRY_BYTES, "sequence failure")?;
            let pending = {
                let mut statement = transaction
                    .prepare(
                        "SELECT b.branch_token FROM vm_merge_branches b
                     JOIN vm_effects e ON e.token = b.branch_token
                     WHERE b.group_token = ?1 AND e.state = 'pending'",
                    )
                    .map_err(|error| {
                        journal_error("LSV4033", "failed to prepare sequence stop", error)
                    })?;
                statement
                    .query_map([&group_token], |row| row.get::<_, String>(0))
                    .map_err(|error| {
                        journal_error("LSV4033", "failed to scan stopped sequence", error)
                    })?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|error| journal_error("LSV4033", "invalid stopped sequence", error))?
            };
            ensure_growth(transaction, bytes.len().saturating_mul(pending.len() + 1))?;
            for token in pending {
                complete_terminal_record(transaction, &token, &bytes)?;
            }
            transaction.execute(
                "UPDATE vm_merge_groups SET state = 'completed', terminal_step = ?2 WHERE token = ?1",
                params![group_token, bytes]
            ).map_err(|error| journal_error("LSV4033", "failed to stop sequence", error))?;
            return Ok(());
        }
    }
    if branches.iter().any(|(_, state, _)| state == "pending") {
        return Ok(());
    }
    if branches
        .iter()
        .any(|(_, state, terminal)| state != "completed" || terminal.is_none())
    {
        return Err(journal_fault(
            "LSV4007",
            "merge branch has an invalid durable state",
        ));
    }

    let completions = branches
        .into_iter()
        .map(|(branch, _, terminal)| {
            let terminal = terminal.ok_or_else(|| {
                journal_fault("LSV4007", "completed merge branch has no terminal step")
            })?;
            let step: Step = decode_bounded(&terminal, MAX_JOURNAL_ENTRY_BYTES)?;
            Ok(BranchCompletion {
                branch,
                outcome: terminal_step_outcome(step)?,
            })
        })
        .collect::<Result<Vec<_>, Fault>>()?;
    let merged = if group_binding::needs_successor(&plan) || plan.order.captures_group_successor() {
        let prefix = group_binding::merge_prefix(&plan, completions)?;
        let Some(terminal) = group_binding::admit_successor(
            transaction,
            &ContinuationToken::from_wire(group_token.clone()),
            &plan,
            &prefix,
        )?
        else {
            return Ok(());
        };
        terminal
    } else {
        group_binding::finish(&plan, completions)?
    };
    validate_terminal_step(&merged)?;
    let merged_bytes = encode_json_capped(&merged, MAX_JOURNAL_ENTRY_BYTES, "merge result")?;
    ensure_growth(transaction, merged_bytes.len())?;
    let updated = transaction
        .execute(
            "UPDATE vm_merge_groups
             SET state = 'completed', terminal_step = ?2
             WHERE token = ?1 AND state = 'pending'",
            params![group_token, merged_bytes],
        )
        .map_err(|error| journal_error("LSV4033", "failed to complete merge group", error))?;
    if updated != 1 {
        return Err(journal_fault(
            "LSV4033",
            "merge group completion lost its pending state",
        ));
    }
    Ok(())
}

fn done() -> Step {
    Step::Done(crate::Value::UiFocus {
        node_id: "node".into(),
    })
}
fn stop() -> Step {
    Step::Fault(journal_fault("HOST", "stopped"))
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
fn memory_fixture(input: &EffectRequest, order: ExecutionOrder, kind: usize) -> EphemeralJournal {
    let mut j = EphemeralJournal::default();
    let second = crate::Vm::build_effect_request(
        &leserpent_hir::Effect::UiFocus {
            node_id: "node".into(),
        },
        leserpent_hir::Type::UiFocus,
        500,
        crate::Principal::new("operator").unwrap(),
        crate::CapabilitySet::new(["ui.presentation"]),
        None,
        1000,
        None,
        100,
    )
    .unwrap();
    let first = input.continuation.token.clone();
    let next = second.continuation.token.clone();
    j.record_pending(Some(input), SchedulerLimits::default())
        .unwrap();
    j.record_pending(Some(&second), SchedulerLimits::default())
        .unwrap();
    if kind == 0 {
        return j;
    }
    let mut plan = MergePlan::new(["first", "next"]).unwrap();
    plan.order = order;
    let owner = ContinuationToken::from_wire("merge-600".into());
    j.merge_groups.insert(
        owner.clone(),
        EphemeralMergeGroup {
            plan,
            branch_tokens: vec![first.clone(), next.clone()],
            terminal_step: None,
        },
    );
    if matches!(kind, 2 | 3 | 8 | 12 | 13 | 14) {
        let d = j.dispatches.get_mut(&first).unwrap();
        d.acknowledged = true;
        d.terminal_step = Some(done());
    }
    if matches!(kind, 3 | 9 | 12 | 13 | 14) {
        let d = j.dispatches.get_mut(&next).unwrap();
        d.acknowledged = true;
        d.terminal_step = Some(done());
        d.lease_expires_at_ms = Some(100);
    }
    match kind {
        0..=3 => {}
        4 | 7 | 9 => {
            let d = j.dispatches.get_mut(&first).unwrap();
            d.terminal_step = Some(stop());
            d.acknowledged = true;
        }
        5 => {
            let d = j.dispatches.get_mut(&first).unwrap();
            d.terminal_step = Some(deadline_cancellation(&input.continuation, 10));
            d.acknowledged = true;
        }
        6 | 15 => {
            j.dispatches.get_mut(&first).unwrap().terminal_step =
                Some(Step::Fault(journal_fault("", "invalid")));
        }
        8 => {}
        10 => {
            j.merge_groups.get_mut(&owner).unwrap().terminal_step =
                Some(Step::Yield(input.continuation.clone()))
        }
        11 => {
            j.dispatches.remove(&first);
        }
        12 => j
            .merge_groups
            .get_mut(&owner)
            .unwrap()
            .plan
            .branches
            .clear(),
        13 => j
            .merge_groups
            .get_mut(&owner)
            .unwrap()
            .branch_tokens
            .reverse(),
        14 => {
            j.dispatches.get_mut(&next).unwrap().acknowledged = false;
        }
        _ => unreachable!(),
    }
    if matches!(kind, 7 | 8) {
        j.dispatches.get_mut(&next).unwrap().terminal_step =
            Some(Step::Yield(input.continuation.clone()));
    }
    if kind == 15 {
        j.dispatches.get_mut(&next).unwrap().terminal_step = Some(done());
    }
    j
}
#[test]
fn memory_finalization_matches_frozen_lazy_stop_replay_and_partial_state_for_all_operations() {
    let mut cases = 0;
    for input in crate::continuation_contract_tests::requests() {
        for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
            for kind in 0..16 {
                let mut actual = memory_fixture(&input, order, kind);
                let mut old = memory_fixture(&input, order, kind);
                let result = actual.finalize_merge_group(&input.continuation.token);
                assert_eq!(
                    result,
                    old.frozen_finalize_merge_group(&input.continuation.token),
                    "order {order:?} case {kind}"
                );
                assert_eq!(snapshot_memory(&actual), snapshot_memory(&old));
                if kind == 10 {
                    assert!(result.is_ok());
                }
                if kind == 7 {
                    assert_eq!(result.is_ok(), order.is_sequential());
                }
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 2240);
}
fn bound_memory(order: ExecutionOrder, kind: usize) -> EphemeralJournal {
    let mut vm = crate::Vm::new(500);
    vm.next_effect_id = 500;
    let branches = ["left", "right"].map(|name| leserpent_hir::HirBranch {
        name: name.into(),
        effect: leserpent_hir::Effect::UiFocus {
            node_id: name.into(),
        },
        result_type: leserpent_hir::Type::UiFocus,
    });
    let result = vm.start_group(
        &branches,
        order,
        crate::Principal::new("operator").unwrap(),
        crate::CapabilitySet::new(["ui.presentation"]),
        None,
        1000,
        None,
        100,
        crate::group_start_fixture::binding(kind),
    );
    assert!(!matches!(result, Step::Fault(_)), "{result:?}");
    #[cfg(feature = "sqlite-journal")]
    let Journal::Ephemeral(j) = vm.journal else {
        unreachable!()
    };
    #[cfg(not(feature = "sqlite-journal"))]
    let Journal::Ephemeral(j) = vm.journal;
    j
}
fn complete_memory_member(j: &mut EphemeralJournal, t: &ContinuationToken) {
    let d = j.dispatches.get_mut(t).unwrap();
    d.terminal_step = Some(done());
    d.acknowledged = true;
    d.lease_expires_at_ms = None;
}
#[test]
fn memory_bound_group_shapes_reuse_original_successor_evaluator_through_final_receipt() {
    let mut cases = 0;
    for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
        for kind in 0..6 {
            let mut actual = bound_memory(order, kind);
            let mut old = bound_memory(order, kind);
            let owner = actual.merge_groups.keys().next().unwrap().clone();
            for _ in 0..8 {
                let Some(t) = actual
                    .dispatches
                    .iter()
                    .find(|(_, d)| !d.acknowledged)
                    .map(|(t, _)| t.clone())
                else {
                    break;
                };
                complete_memory_member(&mut actual, &t);
                complete_memory_member(&mut old, &t);
                let result = actual.finalize_merge_group(&t);
                assert_eq!(result, old.frozen_finalize_merge_group(&t));
                result.unwrap();
                assert_eq!(snapshot_memory(&actual), snapshot_memory(&old));
            }
            assert!(matches!(
                actual.merge_groups[&owner].terminal_step,
                Some(Step::Done(_))
            ));
            assert!(actual.dispatches.values().all(|d| d.acknowledged));
            cases += 1;
        }
    }
    assert_eq!(cases, 12);
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
    fn set_terminal(c: &Connection, t: &str, bytes: Option<Vec<u8>>) {
        c.execute(
            "UPDATE vm_effects SET state='completed',terminal_step=?2 WHERE token=?1",
            params![t, bytes],
        )
        .unwrap();
        c.execute(
            "UPDATE vm_dispatches SET state='acknowledged',lease_expires_at_ms=NULL WHERE token=?1",
            [t],
        )
        .unwrap();
    }
    fn mutate(c: &Connection, tokens: &[ContinuationImage], kind: usize) {
        c.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        let first = tokens[0].token.as_str();
        let next = tokens[1].token.as_str();
        if matches!(kind, 1 | 2 | 15 | 16 | 22..=24 | 27 | 30) {
            set_terminal(c, first, Some(serde_json::to_vec(&done()).unwrap()));
        }
        if matches!(kind, 2 | 15 | 16 | 22..=24 | 27 | 30) {
            set_terminal(c, next, Some(serde_json::to_vec(&done()).unwrap()));
        }
        if matches!(kind, 3 | 17..=21) {
            set_terminal(c, first, Some(serde_json::to_vec(&stop()).unwrap()));
        }
        match kind {
            0..=3=>{},
            4=>set_terminal(c,first,Some(serde_json::to_vec(&deadline_cancellation(&tokens[0],10)).unwrap())),
            5..=8|29=>{let bytes=match kind {5=>Some(serde_json::to_vec(&done()).unwrap()),6=>None,7=>Some(b"{".to_vec()),8=>Some(serde_json::to_vec(&Step::Yield(tokens[0].clone())).unwrap()),_=>Some(serde_json::to_vec(&Step::Fault(journal_fault("","invalid"))).unwrap())};c.execute("UPDATE vm_merge_groups SET state='completed',terminal_step=?1,plan=x'7b'",[bytes]).unwrap();},
            9=>{c.execute_batch("UPDATE vm_merge_groups SET state='invalid';").unwrap();},
            10=>{c.execute("UPDATE vm_merge_groups SET terminal_step=?1",[serde_json::to_vec(&done()).unwrap()]).unwrap();},
            11|25=>{c.execute_batch("UPDATE vm_merge_groups SET plan=x'7b';").unwrap();if kind==25 {c.execute_batch("DROP TABLE vm_effects;").unwrap();}},
            12=>set_terminal(c,first,Some(b"{".to_vec())),
            13=>set_terminal(c,first,Some(serde_json::to_vec(&Step::Yield(tokens[0].clone())).unwrap())),
            14=>{c.execute("UPDATE vm_effects SET state='invalid' WHERE token=?1",[first]).unwrap();},
            15=>set_terminal(c,next,None),
            16|18=>set_terminal(c,next,Some(serde_json::to_vec(&Step::Yield(tokens[1].clone())).unwrap())),
            17=>set_terminal(c,next,Some(b"{".to_vec())),
            19=>c.execute_batch("CREATE TRIGGER refuse_effect BEFORE UPDATE ON vm_effects BEGIN SELECT RAISE(ABORT,'effect refused'); END;").unwrap(),
            20=>c.execute_batch("CREATE TRIGGER refuse_dispatch BEFORE UPDATE ON vm_dispatches BEGIN SELECT RAISE(ABORT,'dispatch refused'); END;").unwrap(),
            21|23=>c.execute_batch("CREATE TRIGGER refuse_group BEFORE UPDATE ON vm_merge_groups BEGIN SELECT RAISE(ABORT,'group refused'); END;").unwrap(),
            22=>c.execute_batch("CREATE TRIGGER ignore_group BEFORE UPDATE ON vm_merge_groups BEGIN SELECT RAISE(IGNORE); END;").unwrap(),
            24=>c.execute_batch("CREATE TABLE commit_parent(id INTEGER PRIMARY KEY); CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER refuse_commit AFTER UPDATE ON vm_merge_groups BEGIN INSERT INTO commit_child VALUES(1); END;").unwrap(),
            26=>c.execute_batch("DELETE FROM vm_merge_branches;").unwrap(),
            27=>{c.execute("DELETE FROM vm_merge_branches WHERE branch_token=?1",[next]).unwrap();},
            28=>{c.execute("UPDATE vm_merge_groups SET plan=?1",[vec![b' ';MAX_CONTINUATION_BYTES+1]]).unwrap();},
            30=>c.execute_batch("CREATE TRIGGER refuse_successor BEFORE INSERT ON vm_dispatches BEGIN SELECT RAISE(ABORT,'successor refused'); END;").unwrap(),
            31=>set_terminal(c,first,Some(vec![b' ';MAX_JOURNAL_ENTRY_BYTES+1])),
            _=>unreachable!(),
        }
        c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
    }
    fn finalize(c: &mut Connection, branch: &str, frozen: bool) -> Result<(), Fault> {
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        if frozen {
            frozen_finalize_merge_group(&tx, branch)?;
        } else {
            finalize_merge_group(&tx, branch)?;
        }
        tx.commit()
            .map_err(|e| journal_error("TEST_COMMIT", "outer settlement commit failed", e))
    }
    #[test]
    fn sqlite_finalization_matches_frozen_replay_decode_order_successors_and_atomic_rollback() {
        let mut cases = 0;
        for (source_index, source) in SOURCES.iter().enumerate() {
            for kind in 0..32 {
                let f = Fixture::new(source, 0);
                let vm = crate::Vm::open_journal(&f.0, 500).unwrap();
                let tokens = vm.pending_continuations();
                drop(vm);
                let other = f.0.with_extension("reference.sqlite3");
                std::fs::copy(&f.0, &other).unwrap();
                let mut actual = Connection::open(&f.0).unwrap();
                let mut old = Connection::open(&other).unwrap();
                mutate(&actual, &tokens, kind);
                mutate(&old, &tokens, kind);
                let before = snapshot(&actual);
                assert_eq!(before, snapshot(&old));
                let result = finalize(&mut actual, tokens[0].token.as_str(), false);
                assert_eq!(
                    result,
                    finalize(&mut old, tokens[0].token.as_str(), true),
                    "source {source_index} case {kind}"
                );
                assert_eq!(snapshot(&actual), snapshot(&old));
                assert_eq!(actual.total_changes(), old.total_changes());
                assert!(actual.is_autocommit());
                assert!(old.is_autocommit());
                if result.is_err() {
                    assert_eq!(snapshot(&actual), before);
                }
                if kind == 17 {
                    assert!(result.is_err());
                }
                if matches!(kind, 0 | 1 | 5 | 26) {
                    assert!(
                        result.is_ok(),
                        "source {source_index} case {kind}: {result:?}"
                    );
                }
                cases += 1;
            }
        }
        assert_eq!(cases, 128);
    }
    #[test]
    fn durable_bound_shapes_match_frozen_successor_insertion_and_reopen_receipts() {
        let mut cases = 0;
        for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
            for kind in 0..6 {
                let memory = bound_memory(order, kind);
                let (owner, group) = memory.merge_groups.iter().next().unwrap();
                let graph = group
                    .plan
                    .branches
                    .iter()
                    .zip(&group.branch_tokens)
                    .map(|(name, t)| (name.as_str(), &memory.dispatches[t].request))
                    .collect::<Vec<_>>();
                let f = Fixture::new(SOURCES[0], 0);
                let a = f.0.with_extension("actual.sqlite3");
                let b = f.0.with_extension("frozen.sqlite3");
                let mut actual = SqliteJournal::open(&a).unwrap();
                let mut old = SqliteJournal::open(&b).unwrap();
                actual
                    .record_merge_graph(owner, &group.plan, &graph, SchedulerLimits::default())
                    .unwrap();
                old.record_merge_graph(owner, &group.plan, &graph, SchedulerLimits::default())
                    .unwrap();
                for _ in 0..8 {
                    let next:Option<String>=actual.connection.query_row("SELECT token FROM vm_effects WHERE state='pending' ORDER BY length(token),token LIMIT 1",[],|r|r.get(0)).optional().unwrap();
                    let Some(t) = next else {
                        break;
                    };
                    for c in [&actual.connection, &old.connection] {
                        set_terminal(c, &t, Some(serde_json::to_vec(&done()).unwrap()));
                    }
                    let result = finalize(&mut actual.connection, &t, false);
                    assert_eq!(result, finalize(&mut old.connection, &t, true));
                    result.unwrap();
                    assert_eq!(snapshot(&actual.connection), snapshot(&old.connection));
                }
                assert!(matches!(
                    actual.merge_result(owner).unwrap(),
                    Some(Step::Done(_))
                ));
                let before = snapshot(&actual.connection);
                drop(actual);
                let mut reopened = SqliteJournal::open(&a).unwrap();
                reopened.load().unwrap();
                assert!(matches!(
                    reopened.merge_result(owner).unwrap(),
                    Some(Step::Done(_))
                ));
                assert_eq!(snapshot(&reopened.connection), before);
                assert!(matches!(
                    reopened
                        .claim_dispatch(1, 101, SchedulerLimits::default())
                        .unwrap(),
                    DispatchClaim::Idle
                ));
                cases += 1;
            }
        }
        assert_eq!(cases, 12);
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
    #[test]
    fn public_last_branch_failure_rolls_back_outer_settlement_and_recovers_without_duplicate_successors()
     {
        let mut cases = 0;
        for source in SOURCES {
            for deferred in [false, true] {
                let f = Fixture::new(source, 0);
                let mut vm = crate::Vm::open_journal(&f.0, 500).unwrap();
                let first = vm.claim_effect(1, 100).unwrap().unwrap();
                vm.acknowledge_effect(&first, 2, reply(&first));
                let last = vm.claim_effect(3, 100).unwrap().unwrap();
                let c = Connection::open(&f.0).unwrap();
                let owner: String = c
                    .query_row("SELECT token FROM vm_merge_groups", [], |r| r.get(0))
                    .unwrap();
                c.execute_batch(if deferred {"CREATE TABLE commit_parent(id INTEGER PRIMARY KEY); CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER refuse_finalization AFTER UPDATE ON vm_merge_groups BEGIN INSERT INTO commit_child VALUES(1); END;"} else {"CREATE TRIGGER refuse_finalization BEFORE UPDATE ON vm_merge_groups BEGIN SELECT RAISE(ABORT,'finalization refused'); END;"}).unwrap();
                let before = snapshot(&c);
                let cached = vm.completed_count();
                assert!(matches!(
                    vm.acknowledge_effect(&last, 4, reply(&last)),
                    Step::Fault(_)
                ));
                assert_eq!(snapshot(&c), before);
                assert_eq!(vm.completed_count(), cached);
                assert!(
                    vm.merge_result(&ContinuationToken::from_wire(owner.clone()))
                        .unwrap()
                        .is_none()
                );
                c.execute_batch("DROP TRIGGER refuse_finalization;")
                    .unwrap();
                drop(vm);
                let mut recovered = crate::Vm::open_journal(&f.0, 500).unwrap();
                assert!(!matches!(
                    recovered.acknowledge_effect(&last, 5, reply(&last)),
                    Step::Fault(_)
                ));
                for tick in 6..12 {
                    if let Some(lease) = recovered.claim_effect(tick, 100).unwrap() {
                        assert!(!matches!(
                            recovered.acknowledge_effect(&lease, tick, reply(&lease)),
                            Step::Fault(_)
                        ));
                    }
                }
                let terminal = recovered
                    .merge_result(&ContinuationToken::from_wire(owner.clone()))
                    .unwrap()
                    .unwrap();
                assert!(matches!(terminal, Step::Done(_)));
                drop(recovered);
                let mut reopened = crate::Vm::open_journal(&f.0, 500).unwrap();
                assert!(reopened.claim_effect(30, 100).unwrap().is_none());
                assert_eq!(
                    reopened
                        .merge_result(&ContinuationToken::from_wire(owner))
                        .unwrap(),
                    Some(terminal)
                );
                let records: i64 = c
                    .query_row("SELECT COUNT(*) FROM vm_effects", [], |r| r.get(0))
                    .unwrap();
                assert_eq!(records, if source.contains("bind(") { 3 } else { 2 });
                cases += 1;
            }
        }
        assert_eq!(cases, 8);
    }
}
