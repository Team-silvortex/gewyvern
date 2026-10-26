use super::*;

// Frozen registration bodies from the pre-extraction working tree at 8b792476.
// Original loading/next-plan bodies are retained; codecs, evaluator and graph proof
// remain shared, not registration coordination.
fn frozen_admit_ephemeral(
    journal: &mut EphemeralJournal,
    owner: &ContinuationToken,
    plan: &MergePlan,
    prefix: &Step,
) -> Result<Option<Step>, Fault> {
    let Step::Done(value) = prefix else {
        return Ok(Some(prefix.clone()));
    };
    let group = journal.merge_groups.get(owner).ok_or_else(invalid)?;
    let mut requests = group
        .branch_tokens
        .iter()
        .map(|token| {
            journal
                .dispatches
                .get(token)
                .map(|dispatch| dispatch.request.clone())
                .ok_or_else(invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let request = match prepare_admission(owner, plan, &requests, value) {
        Ok(crate::group_binding::CaptureProgress::Pending(request)) => *request,
        Ok(crate::group_binding::CaptureProgress::Done(value)) => {
            return Ok(Some(Step::Done(crate::Value::Scalar { value })));
        }
        Err(error) => return Ok(Some(Step::Fault(error))),
    };
    let next = successor_plan(plan)?;
    requests.push(request.clone());
    let graph = next
        .branches
        .iter()
        .zip(&requests)
        .map(|(name, request)| (name.as_str(), request))
        .collect::<Vec<_>>();
    validate_merge_graph_input(owner, &next, &graph)?;
    let token = request.continuation.token.clone();
    if journal.dispatches.contains_key(&token) || journal.merge_groups.contains_key(&token) {
        return Err(invalid());
    }
    encode_json_capped(&request, MAX_JOURNAL_ENTRY_BYTES, "group successor request")?;
    journal.dispatches.insert(
        token.clone(),
        EphemeralDispatch {
            request,
            attempt: 0,
            lease_expires_at_ms: None,
            ready_at_ms: 0,
            retry_count: 0,
            last_error: None,
            acknowledged: false,
            terminal_step: None,
        },
    );
    let group = journal.merge_groups.get_mut(owner).ok_or_else(invalid)?;
    group.plan = next;
    group.branch_tokens.push(token);
    Ok(None)
}

#[cfg(feature = "sqlite-journal")]
fn frozen_admit_successor(
    transaction: &rusqlite::Transaction<'_>,
    owner: &ContinuationToken,
    plan: &MergePlan,
    prefix: &Step,
) -> Result<Option<Step>, Fault> {
    let Step::Done(value) = prefix else {
        return Ok(Some(prefix.clone()));
    };
    let mut statement = transaction.prepare(
        "SELECT d.request FROM vm_merge_branches b JOIN vm_dispatches d ON d.token = b.branch_token
         WHERE b.group_token = ?1 ORDER BY b.position"
    ).map_err(|error| journal_error("LSV4033", "failed to prepare group successor", error))?;
    let mut requests = statement
        .query_map([owner.as_str()], |row| row.get::<_, Vec<u8>>(0))
        .map_err(|error| journal_error("LSV4033", "failed to load group authority", error))?
        .map(|row| {
            decode_bounded::<EffectRequest>(
                &row.map_err(|error| journal_error("LSV4033", "invalid group authority", error))?,
                MAX_JOURNAL_ENTRY_BYTES,
            )
        })
        .collect::<Result<Vec<_>, Fault>>()?;
    drop(statement);
    let request = match prepare_admission(owner, plan, &requests, value) {
        Ok(crate::group_binding::CaptureProgress::Pending(request)) => *request,
        Ok(crate::group_binding::CaptureProgress::Done(value)) => {
            return Ok(Some(Step::Done(crate::Value::Scalar { value })));
        }
        Err(error) => return Ok(Some(Step::Fault(error))),
    };
    let next = successor_plan(plan)?;
    requests.push(request.clone());
    let graph = next
        .branches
        .iter()
        .zip(&requests)
        .map(|(name, request)| (name.as_str(), request))
        .collect::<Vec<_>>();
    validate_merge_graph_input(owner, &next, &graph)?;
    let records: i64 = transaction
        .query_row(
            "SELECT (SELECT COUNT(*) FROM vm_effects) + (SELECT COUNT(*) FROM vm_merge_groups)",
            [],
            |row| row.get(0),
        )
        .map_err(|error| journal_error("LSV4033", "failed to bound group successor", error))?;
    if records >= MAX_JOURNAL_RECORDS as i64 {
        return Err(journal_fault("LSV4008", "journal record limit exceeded"));
    }
    let image = encode_json_capped(
        &request.continuation,
        MAX_CONTINUATION_BYTES,
        "group successor image",
    )?;
    let request_bytes =
        encode_json_capped(&request, MAX_JOURNAL_ENTRY_BYTES, "group successor request")?;
    let plan_bytes = encode_json_capped(&next, MAX_CONTINUATION_BYTES, "group successor plan")?;
    let previous_plan_bytes =
        encode_json_capped(plan, MAX_CONTINUATION_BYTES, "group prefix plan")?;
    let deadline = request
        .continuation
        .deadline_at_ms
        .map(i64::try_from)
        .transpose()
        .map_err(|_| invalid())?;
    ensure_growth(
        transaction,
        image
            .len()
            .saturating_add(request_bytes.len())
            .saturating_add(plan_bytes.len().saturating_sub(previous_plan_bytes.len())),
    )?;
    transaction.execute(
        "INSERT INTO vm_effects(token, state, image, deadline_at_ms, terminal_step) VALUES (?1, 'pending', ?2, ?3, NULL)",
        params![request.continuation.token.as_str(), image, deadline]
    ).map_err(|error| journal_error("LSV4033", "failed to create group successor", error))?;
    transaction.execute(
        "INSERT INTO vm_dispatches(token, request, state, attempt, lease_expires_at_ms) VALUES (?1, ?2, 'ready', 0, NULL)",
        params![request.continuation.token.as_str(), request_bytes]
    ).map_err(|error| journal_error("LSV4033", "failed to create group successor dispatch", error))?;
    let updated = transaction
        .execute(
            "UPDATE vm_merge_groups SET plan = ?2 WHERE token = ?1 AND state = 'pending'",
            params![owner.as_str(), plan_bytes],
        )
        .map_err(|error| {
            journal_error("LSV4033", "failed to append group successor plan", error)
        })?;
    if updated != 1 {
        return Err(journal_fault(
            "LSV4033",
            "group successor lost its pending owner",
        ));
    }
    transaction.execute(
        "INSERT INTO vm_merge_branches(group_token, branch_token, branch_name, position) VALUES (?1, ?2, ?3, ?4)",
        params![owner.as_str(), request.continuation.token.as_str(), next.branches.last().ok_or_else(invalid)?, plan.branches.len() as i64]
    ).map_err(|error| journal_error("LSV4033", "failed to link group successor", error))?;
    Ok(None)
}

fn done() -> Step {
    Step::Done(crate::Value::UiFocus {
        node_id: "node".into(),
    })
}
fn failure() -> Step {
    Step::Fault(journal_fault("HOST", "stopped"))
}
fn memory(
    order: ExecutionOrder,
    kind: usize,
) -> (EphemeralJournal, ContinuationToken, MergePlan, Step) {
    let mut vm = crate::Vm::new(500);
    vm.next_effect_id = 500;
    let branches = ["left", "right"].map(|name| leserpent_hir::HirBranch {
        name: name.into(),
        effect: leserpent_hir::Effect::UiFocus {
            node_id: name.into(),
        },
        result_type: leserpent_hir::Type::UiFocus,
    });
    let step = vm.start_group(
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
    assert!(!matches!(step, Step::Fault(_)));
    #[cfg(feature = "sqlite-journal")]
    let Journal::Ephemeral(j) = vm.journal else {
        unreachable!()
    };
    #[cfg(not(feature = "sqlite-journal"))]
    let Journal::Ephemeral(j) = vm.journal;
    let (owner, g) = j.merge_groups.iter().next().unwrap();
    let p = g.plan.clone();
    let owner = owner.clone();
    let prefix = prefix(&p);
    (j, owner, p, prefix)
}
fn completions(p: &MergePlan) -> Vec<BranchCompletion> {
    p.branches
        .iter()
        .map(|name| BranchCompletion {
            branch: name.clone(),
            outcome: terminal_step_outcome(done()).unwrap(),
        })
        .collect()
}
fn prefix(p: &MergePlan) -> Step {
    merge_prefix(p, completions(p)).unwrap()
}
fn candidate(
    j: &EphemeralJournal,
    owner: &ContinuationToken,
    p: &MergePlan,
    prefix: &Step,
) -> Option<EffectRequest> {
    let Step::Done(value) = prefix else {
        return None;
    };
    let g = j.merge_groups.get(owner)?;
    let requests = g
        .branch_tokens
        .iter()
        .map(|t| j.dispatches[t].request.clone())
        .collect::<Vec<_>>();
    if let Ok(crate::group_binding::CaptureProgress::Pending(r)) =
        prepare_admission(owner, p, &requests, value)
    {
        Some(*r)
    } else {
        None
    }
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
fn mutate_memory(
    j: &mut EphemeralJournal,
    owner: &ContinuationToken,
    p: &mut MergePlan,
    prefix: &mut Step,
    mode: usize,
) {
    let tokens = j.merge_groups[owner].branch_tokens.clone();
    let next = candidate(j, owner, p, prefix);
    match mode {
        0 => {}
        1 => *prefix = failure(),
        2 => *prefix = Step::Yield(j.dispatches[&tokens[0]].request.continuation.clone()),
        3 => {
            j.merge_groups.remove(owner);
        }
        4 => {
            j.dispatches.remove(&tokens[0]);
        }
        5 => {
            j.dispatches
                .get_mut(&tokens[0])
                .unwrap()
                .request
                .required_capability = "invalid".into()
        }
        6 => p.result_binding = None,
        7 => {
            if let Some(b) = &mut p.result_binding {
                b.fuel_remaining = 0;
            }
        }
        8 => {
            if let Some(r) = next {
                j.record_pending(Some(&r), SchedulerLimits::default())
                    .unwrap();
            }
        }
        9 => {
            if let Some(r) = next {
                j.merge_groups.insert(
                    r.continuation.token,
                    EphemeralMergeGroup {
                        plan: p.clone(),
                        branch_tokens: vec![],
                        terminal_step: None,
                    },
                );
            }
        }
        10 => j.merge_groups.get_mut(owner).unwrap().terminal_step = Some(failure()),
        11 => j
            .merge_groups
            .get_mut(owner)
            .unwrap()
            .branch_tokens
            .reverse(),
        12 => {
            *prefix = Step::Done(crate::Value::Scalar {
                value: leselang_runtime_core::ScalarValue::None,
            })
        }
        13 => p.branches[1] = "invalid branch".into(),
        _ => unreachable!(),
    }
}
#[test]
fn memory_successor_registration_matches_frozen_prefix_error_conflict_and_graph_policy() {
    let mut cases = 0;
    for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
        for kind in 0..6 {
            for mode in 0..14 {
                let (mut actual, owner, mut p, mut step) = memory(order, kind);
                let (mut old, _, mut old_p, mut old_step) = memory(order, kind);
                mutate_memory(&mut actual, &owner, &mut p, &mut step, mode);
                mutate_memory(&mut old, &owner, &mut old_p, &mut old_step, mode);
                let before = snapshot_memory(&actual);
                let r = admit_ephemeral(&mut actual, &owner, &p, &step);
                assert_eq!(
                    r,
                    frozen_admit_ephemeral(&mut old, &owner, &old_p, &old_step),
                    "{order:?} kind {kind} mode {mode}"
                );
                assert_eq!(snapshot_memory(&actual), snapshot_memory(&old));
                if r.is_err() || matches!(r, Ok(Some(_))) {
                    assert_eq!(snapshot_memory(&actual), before);
                }
                if matches!(mode, 1 | 2) {
                    assert_eq!(r, Ok(Some(step)));
                }
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 168);
}
#[test]
fn memory_successor_capture_stages_match_original_until_the_final_receipt() {
    let mut cases = 0;
    for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
        for kind in 0..6 {
            let (mut actual, owner, _, _) = memory(order, kind);
            let (mut old, ..) = memory(order, kind);
            for _ in 0..8 {
                let p = actual.merge_groups[&owner].plan.clone();
                for j in [&mut actual, &mut old] {
                    for t in j.merge_groups[&owner].branch_tokens.clone() {
                        let d = j.dispatches.get_mut(&t).unwrap();
                        d.acknowledged = true;
                        d.terminal_step = Some(done());
                    }
                }
                let result = if needs_successor(&p) || p.order.captures_group_successor() {
                    let v = prefix(&p);
                    let result = admit_ephemeral(&mut actual, &owner, &p, &v);
                    assert_eq!(result, frozen_admit_ephemeral(&mut old, &owner, &p, &v));
                    result.unwrap()
                } else {
                    Some(finish(&p, completions(&p)).unwrap())
                };
                assert_eq!(snapshot_memory(&actual), snapshot_memory(&old));
                if let Some(terminal) = result {
                    assert!(matches!(terminal, Step::Done(_)));
                    actual.merge_groups.get_mut(&owner).unwrap().terminal_step = Some(terminal);
                    break;
                }
            }
            assert!(actual.merge_groups[&owner].terminal_step.is_some());
            cases += 1;
        }
    }
    assert_eq!(cases, 12);
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
            let p = std::env::temp_dir().join(format!(
                "leselang-group-successor-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir(&p).unwrap();
            Self(p)
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn initialize(
        path: &Path,
        m: &EphemeralJournal,
        owner: &ContinuationToken,
        p: &MergePlan,
    ) -> Connection {
        let mut j = SqliteJournal::open(path).unwrap();
        let graph = p
            .branches
            .iter()
            .zip(&m.merge_groups[owner].branch_tokens)
            .map(|(n, t)| (n.as_str(), &m.dispatches[t].request))
            .collect::<Vec<_>>();
        j.record_merge_graph(owner, p, &graph, SchedulerLimits::default())
            .unwrap();
        j.connection
    }
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
                let count = q.column_count();
                let rows = q
                    .query_map([], |r| {
                        (0..count)
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
    fn mutate(
        c: &Connection,
        m: &EphemeralJournal,
        owner: &ContinuationToken,
        p: &mut MergePlan,
        step: &mut Step,
        mode: usize,
    ) {
        let first = &m.merge_groups[owner].branch_tokens[0];
        let last = &m.merge_groups[owner].branch_tokens[1];
        let next = candidate(m, owner, p, step);
        c.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        match mode{
            0=>{},1=>*step=failure(),2=>*step=Step::Yield(m.dispatches[first].request.continuation.clone()),
            3=>{c.execute("DELETE FROM vm_merge_groups WHERE token=?1",[owner.as_str()]).unwrap();},
            4=>{c.execute("DELETE FROM vm_dispatches WHERE token=?1",[first.as_str()]).unwrap();},
            5|18=>{
                c.execute("UPDATE vm_dispatches SET request=x'7b' WHERE token=?1",[first.as_str()]).unwrap();
                if mode==18 {
                    // STRICT tables reject wrong storage classes even with CHECKs disabled.
                    // A disposable read view probes lazy decoding without weakening production.
                    c.execute_batch(&format!("ALTER TABLE vm_dispatches RENAME TO fixture_dispatches;
                        CREATE VIEW vm_dispatches AS SELECT token, CASE WHEN token='{}' THEN 42 ELSE request END AS request FROM fixture_dispatches;",last.as_str())).unwrap();
                }
            },
            6=>{let mut r=m.dispatches[first].request.clone();r.required_capability="invalid".into();c.execute("UPDATE vm_dispatches SET request=?2 WHERE token=?1",params![first.as_str(),serde_json::to_vec(&r).unwrap()]).unwrap();},
            7=>p.result_binding=None,8=>{if let Some(b)=&mut p.result_binding{b.fuel_remaining=0;}},
            9=>{if let Some(r)=next{c.execute("INSERT INTO vm_effects(token,state,image,deadline_at_ms) VALUES(?1,'pending',?2,NULL)",params![r.continuation.token.as_str(),serde_json::to_vec(&r.continuation).unwrap()]).unwrap();}},
            10=>{if let Some(r)=next{c.execute("INSERT INTO vm_dispatches(token,request,state,attempt) VALUES(?1,?2,'ready',0)",params![r.continuation.token.as_str(),serde_json::to_vec(&r).unwrap()]).unwrap();}},
            11=>{c.execute("UPDATE vm_merge_groups SET state='completed',terminal_step=?2 WHERE token=?1",params![owner.as_str(),serde_json::to_vec(&failure()).unwrap()]).unwrap();},
            12=>c.execute_batch("CREATE TRIGGER fail_effect BEFORE INSERT ON vm_effects BEGIN SELECT RAISE(ABORT,'effect refused'); END;").unwrap(),
            13=>c.execute_batch("CREATE TRIGGER fail_dispatch BEFORE INSERT ON vm_dispatches BEGIN SELECT RAISE(ABORT,'dispatch refused'); END;").unwrap(),
            14=>c.execute_batch("CREATE TRIGGER fail_plan BEFORE UPDATE ON vm_merge_groups BEGIN SELECT RAISE(ABORT,'plan refused'); END;").unwrap(),
            15=>c.execute_batch("CREATE TRIGGER fail_link BEFORE INSERT ON vm_merge_branches BEGIN SELECT RAISE(ABORT,'link refused'); END;").unwrap(),
            16=>c.execute_batch("CREATE TABLE commit_parent(id INTEGER PRIMARY KEY);CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED);CREATE TRIGGER fail_commit AFTER INSERT ON vm_merge_branches BEGIN INSERT INTO commit_child VALUES(1);END;").unwrap(),
            17=>c.execute_batch("CREATE TRIGGER ignore_plan BEFORE UPDATE ON vm_merge_groups BEGIN SELECT RAISE(IGNORE); END;").unwrap(),
            19=>{c.execute("UPDATE vm_dispatches SET request=?2 WHERE token=?1",params![first.as_str(),vec![b' ';MAX_JOURNAL_ENTRY_BYTES+1]]).unwrap();},
            20=>*step=Step::Done(crate::Value::Scalar{value:leselang_runtime_core::ScalarValue::None}),
            21=>p.branches[1]="invalid branch".into(),
            22|23=>{c.execute_batch("DROP TABLE vm_dispatches;").unwrap();if mode==23{*step=failure();}},
            _=>unreachable!(),
        }
        c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
    }
    fn admit(
        c: &mut Connection,
        owner: &ContinuationToken,
        p: &MergePlan,
        step: &Step,
        frozen: bool,
    ) -> Result<Option<Step>, Fault> {
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        let r = if frozen {
            frozen_admit_successor(&tx, owner, p, step)?
        } else {
            admit_successor(&tx, owner, p, step)?
        };
        tx.commit()
            .map_err(|e| journal_error("TEST_COMMIT", "outer commit failed", e))?;
        Ok(r)
    }
    #[test]
    fn sqlite_successor_registration_matches_frozen_streaming_decode_writes_and_outer_rollback() {
        let mut cases = 0;
        for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
            for kind in 0..6 {
                for mode in 0..24 {
                    let f = Fixture::new();
                    let (m, owner, p, step) = memory(order, kind);
                    let mut actual = initialize(&f.0.join("actual.sqlite3"), &m, &owner, &p);
                    let mut old = initialize(&f.0.join("old.sqlite3"), &m, &owner, &p);
                    let (mut p1, mut p2, mut s1, mut s2) = (p.clone(), p, step.clone(), step);
                    mutate(&actual, &m, &owner, &mut p1, &mut s1, mode);
                    mutate(&old, &m, &owner, &mut p2, &mut s2, mode);
                    let before = snapshot(&actual);
                    let r = admit(&mut actual, &owner, &p1, &s1, false);
                    assert_eq!(
                        r,
                        admit(&mut old, &owner, &p2, &s2, true),
                        "{order:?} kind {kind} mode {mode}"
                    );
                    assert_eq!(snapshot(&actual), snapshot(&old));
                    assert_eq!(actual.total_changes(), old.total_changes());
                    assert!(actual.is_autocommit());
                    if r.is_err() || matches!(r, Ok(Some(_))) {
                        assert_eq!(snapshot(&actual), before);
                    }
                    if matches!(mode, 1 | 2 | 23) {
                        assert_eq!(r, Ok(Some(s1)));
                    }
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 288);
    }
    #[test]
    fn durable_capture_stages_preserve_reservations_and_reopen_completed_group_receipts() {
        let mut cases = 0;
        for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
            for kind in 0..6 {
                let f = Fixture::new();
                let (m, owner, p, _) = memory(order, kind);
                let a = f.0.join("actual.sqlite3");
                let mut actual = initialize(&a, &m, &owner, &p);
                let mut old = initialize(&f.0.join("old.sqlite3"), &m, &owner, &p);
                let watermark: i64 = actual
                    .query_row("SELECT next_sequence FROM vm_metadata", [], |r| r.get(0))
                    .unwrap();
                let mut complete = false;
                for _ in 0..8 {
                    let bytes: Vec<u8> = actual
                        .query_row(
                            "SELECT plan FROM vm_merge_groups WHERE token=?1",
                            [owner.as_str()],
                            |r| r.get(0),
                        )
                        .unwrap();
                    let p: MergePlan = serde_json::from_slice(&bytes).unwrap();
                    for c in [&actual, &old] {
                        c.execute(
                            "UPDATE vm_effects SET state='completed',terminal_step=?1",
                            [serde_json::to_vec(&done()).unwrap()],
                        )
                        .unwrap();
                        c.execute_batch("UPDATE vm_dispatches SET state='acknowledged',lease_expires_at_ms=NULL;").unwrap();
                    }
                    let result = if needs_successor(&p) || p.order.captures_group_successor() {
                        let v = prefix(&p);
                        let r = admit(&mut actual, &owner, &p, &v, false);
                        assert_eq!(r, admit(&mut old, &owner, &p, &v, true));
                        r.unwrap()
                    } else {
                        Some(finish(&p, completions(&p)).unwrap())
                    };
                    assert_eq!(snapshot(&actual), snapshot(&old));
                    if let Some(step) = result {
                        assert!(matches!(step, Step::Done(_)));
                        actual
                            .execute(
                                "UPDATE vm_merge_groups SET state='completed',terminal_step=?1",
                                [serde_json::to_vec(&step).unwrap()],
                            )
                            .unwrap();
                        complete = true;
                        break;
                    }
                }
                assert!(complete);
                assert_eq!(
                    actual
                        .query_row("SELECT next_sequence FROM vm_metadata", [], |r| r
                            .get::<_, i64>(0))
                        .unwrap(),
                    watermark
                );
                let before = snapshot(&actual);
                drop(actual);
                let mut reopened = SqliteJournal::open(&a).unwrap();
                reopened.load().unwrap();
                assert_eq!(snapshot(&reopened.connection), before);
                assert!(matches!(
                    reopened.merge_result(&owner).unwrap(),
                    Some(Step::Done(_))
                ));
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
    #[test]
    fn public_successor_write_failures_roll_back_last_member_and_allow_one_explicit_retry() {
        let mut cases = 0;
        for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
            for phase in 0..5 {
                let f = Fixture::new();
                let path = f.0.join("public.sqlite3");
                let mut vm = crate::Vm::open_journal(&path, 500).unwrap();
                let branches = ["left", "right"].map(|name| leserpent_hir::HirBranch {
                    name: name.into(),
                    effect: leserpent_hir::Effect::UiFocus {
                        node_id: name.into(),
                    },
                    result_type: leserpent_hir::Type::UiFocus,
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
                        crate::group_start_fixture::binding(4)
                    ),
                    Step::Fault(_)
                ));
                let reply = |lease: &DispatchLease| {
                    let crate::EffectOperation::Presentation(e) = &lease.request.operation else {
                        panic!()
                    };
                    let crate::PresentationOperation::Focus { node_id } = &e.operation else {
                        panic!()
                    };
                    crate::EffectResult::Presentation(crate::PresentationResult::Focus {
                        node_id: node_id.clone(),
                    })
                };
                let first = vm.claim_effect(1, 100).unwrap().unwrap();
                vm.acknowledge_effect(&first, 2, reply(&first));
                let last = vm.claim_effect(3, 100).unwrap().unwrap();
                let c = Connection::open(&path).unwrap();
                c.execute_batch(match phase{0=>"CREATE TRIGGER fail_successor BEFORE INSERT ON vm_effects BEGIN SELECT RAISE(ABORT,'effect refused');END;",1=>"CREATE TRIGGER fail_successor BEFORE INSERT ON vm_dispatches BEGIN SELECT RAISE(ABORT,'dispatch refused');END;",2=>"CREATE TRIGGER fail_successor BEFORE UPDATE ON vm_merge_groups BEGIN SELECT RAISE(ABORT,'plan refused');END;",3=>"CREATE TRIGGER fail_successor BEFORE INSERT ON vm_merge_branches BEGIN SELECT RAISE(ABORT,'link refused');END;",_=>"CREATE TABLE commit_parent(id INTEGER PRIMARY KEY);CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED);CREATE TRIGGER fail_successor AFTER INSERT ON vm_merge_branches BEGIN INSERT INTO commit_child VALUES(1);END;"}).unwrap();
                let before = snapshot(&c);
                let cached = vm.completed_count();
                assert!(matches!(
                    vm.acknowledge_effect(&last, 4, reply(&last)),
                    Step::Fault(_)
                ));
                assert_eq!(snapshot(&c), before);
                assert_eq!(vm.completed_count(), cached);
                c.execute_batch("DROP TRIGGER fail_successor;").unwrap();
                drop(vm);
                let mut recovered = crate::Vm::open_journal(&path, 500).unwrap();
                assert!(!matches!(
                    recovered.acknowledge_effect(&last, 5, reply(&last)),
                    Step::Fault(_)
                ));
                for now in 6..12 {
                    if let Some(lease) = recovered.claim_effect(now, 100).unwrap() {
                        assert!(!matches!(
                            recovered.acknowledge_effect(&lease, now, reply(&lease)),
                            Step::Fault(_)
                        ));
                    }
                }
                let owner: String = c
                    .query_row("SELECT token FROM vm_merge_groups", [], |r| r.get(0))
                    .unwrap();
                assert!(matches!(
                    recovered
                        .merge_result(&ContinuationToken::from_wire(owner.clone()))
                        .unwrap(),
                    Some(Step::Done(_))
                ));
                drop(recovered);
                let mut reopened = crate::Vm::open_journal(&path, 500).unwrap();
                assert!(reopened.claim_effect(30, 100).unwrap().is_none());
                assert_eq!(
                    c.query_row("SELECT COUNT(*) FROM vm_effects", [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    4
                );
                cases += 1;
            }
        }
        assert_eq!(cases, 10);
    }
}
