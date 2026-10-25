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
fn owner() -> ContinuationToken {
    ContinuationToken::from_wire("merge-100".into())
}
fn plan(order: ExecutionOrder) -> MergePlan {
    let mut p = MergePlan::new(["left", "right"]).unwrap();
    p.order = order;
    p
}
fn graph(requests: &[EffectRequest; 2]) -> [(&str, &EffectRequest); 2] {
    [("left", &requests[0]), ("right", &requests[1])]
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
fn memory_fixture(
    input: &EffectRequest,
    p: &MergePlan,
    kind: usize,
) -> (EphemeralJournal, [EffectRequest; 2], SchedulerLimits) {
    let mut j = EphemeralJournal::default();
    let mut requests = [input.clone(), request(200)];
    let mut limits = SchedulerLimits::default();
    if matches!(kind, 1 | 10 | 11) {
        j.frozen_record_merge_graph(&owner(), p, &graph(&requests), limits)
            .unwrap();
    }
    match kind {
        0 | 1 => {}
        2 | 3 | 5 => {
            j.record_pending(Some(&request(300)), limits).unwrap();
            let d = j
                .dispatches
                .remove(&request(300).continuation.token)
                .unwrap();
            let key = if kind == 2 {
                owner()
            } else {
                requests[usize::from(kind == 5)].continuation.token.clone()
            };
            j.dispatches.insert(key, d);
        }
        4 | 6 => {
            let key = requests[usize::from(kind == 6)].continuation.token.clone();
            j.merge_groups.insert(
                key,
                EphemeralMergeGroup {
                    plan: p.clone(),
                    branch_tokens: vec![],
                    terminal_step: None,
                },
            );
        }
        7 => limits.max_pending_dispatches = 1,
        8 | 9 => {
            j.record_pending(Some(&request(300)), limits).unwrap();
            if kind == 9 {
                let d = j
                    .dispatches
                    .get_mut(&request(300).continuation.token)
                    .unwrap();
                d.acknowledged = true;
                d.terminal_step = Some(done());
                d.attempt = 2;
            }
            limits.max_pending_dispatches = 2;
        }
        10 => {
            requests[1].continuation.pending_effect = leselang_hir::Effect::UiFocus {
                node_id: "x".repeat(MAX_CONTINUATION_BYTES),
            }
        }
        11 => limits.max_pending_dispatches = 0,
        _ => unreachable!(),
    }
    (j, requests, limits)
}
#[test]
fn memory_graph_registration_matches_frozen_order_conflicts_and_capacity_for_all_operations() {
    let mut cases = 0;
    for input in crate::continuation_contract_tests::requests() {
        for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
            let p = plan(order);
            for kind in 0..12 {
                let (mut actual, requests, limits) = memory_fixture(&input, &p, kind);
                let (mut original, _, _) = memory_fixture(&input, &p, kind);
                let before = memory_snapshot(&actual);
                let result = actual.record_merge_graph(&owner(), &p, &graph(&requests), limits);
                assert_eq!(
                    result,
                    original.frozen_record_merge_graph(&owner(), &p, &graph(&requests), limits),
                    "case {kind}, order {order:?}"
                );
                assert_eq!(memory_snapshot(&actual), memory_snapshot(&original));
                assert_eq!(result.is_ok(), matches!(kind, 0 | 9));
                if result.is_err() {
                    assert_eq!(memory_snapshot(&actual), before);
                }
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 1680);
}
#[test]
fn public_graph_admission_refuses_invalid_late_members_before_memory_mutation() {
    let mut cases = 0;
    for defect in 0..5 {
        let mut requests = [request(7), request(8)];
        let mut p = plan(ExecutionOrder::Parallel);
        let mut t = owner();
        match defect {
            0 => requests[1].required_capability = "invalid".into(),
            1 => requests[1] = requests[0].clone(),
            2 => p.branches.swap(0, 1),
            3 => t = requests[0].continuation.token.clone(),
            4 => requests[1].continuation.schema_version = 0,
            _ => unreachable!(),
        }
        let mut journal = Journal::Ephemeral(EphemeralJournal::default());
        assert!(
            journal
                .record_merge_graph_with_limits(
                    &t,
                    &p,
                    &graph(&requests),
                    SchedulerLimits::default()
                )
                .is_err()
        );
        #[cfg(feature = "sqlite-journal")]
        let Journal::Ephemeral(j) = journal else {
            unreachable!()
        };
        #[cfg(not(feature = "sqlite-journal"))]
        let Journal::Ephemeral(j) = journal;
        assert!(j.dispatches.is_empty());
        assert!(j.merge_groups.is_empty());
        cases += 1;
    }
    assert_eq!(cases, 5);
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
                "leselang-graph-{}-{}",
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
        order: ExecutionOrder,
        kind: usize,
    ) -> (
        ContinuationToken,
        MergePlan,
        [EffectRequest; 2],
        SchedulerLimits,
    ) {
        let mut requests = [input.clone(), request(200)];
        let mut p = plan(order);
        let mut t = owner();
        let mut limits = SchedulerLimits::default();
        if matches!(kind, 1 | 5 | 26) {
            j.frozen_record_merge_graph(&t, &p, &graph(&requests), limits)
                .unwrap();
        }
        if matches!(kind, 2..=4 | 7 | 8) {
            let r = match kind {
                3 => requests[0].clone(),
                4 => requests[1].clone(),
                _ => request(300),
            };
            j.record_pending(&r.continuation, Some(&r), limits).unwrap();
            if kind == 2 {
                t = r.continuation.token;
            }
            if kind == 8 {
                j.connection
                    .execute(
                        "UPDATE vm_effects SET state='completed',terminal_step=?1",
                        [serde_json::to_vec(&done()).unwrap()],
                    )
                    .unwrap();
                j.connection
                    .execute("UPDATE vm_dispatches SET state='acknowledged'", [])
                    .unwrap();
            }
        }
        j.connection
            .execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        match kind {
            0..=4=>{},
            5=>limits.max_pending_dispatches=0,
            6=>limits.max_pending_dispatches=1,
            7|8=>limits.max_pending_dispatches=2,
            9=>j.connection.execute_batch("UPDATE vm_metadata SET next_sequence=-1;").unwrap(),
            10|30=>j.connection.execute_batch("UPDATE vm_metadata SET next_sequence=9999;").unwrap(),
            11=>j.connection.execute_batch("CREATE TRIGGER refuse_group BEFORE INSERT ON vm_merge_groups BEGIN SELECT RAISE(ABORT,'group refused'); END;").unwrap(),
            12=>j.connection.execute_batch("CREATE TRIGGER refuse_effect BEFORE INSERT ON vm_effects WHEN NEW.token='continuation-200' BEGIN SELECT RAISE(ABORT,'late effect refused'); END;").unwrap(),
            13=>j.connection.execute_batch("CREATE TRIGGER refuse_dispatch BEFORE INSERT ON vm_dispatches WHEN NEW.token='continuation-200' BEGIN SELECT RAISE(ABORT,'late dispatch refused'); END;").unwrap(),
            14=>j.connection.execute_batch("CREATE TRIGGER refuse_link BEFORE INSERT ON vm_merge_branches WHEN NEW.position=1 BEGIN SELECT RAISE(ABORT,'late link refused'); END;").unwrap(),
            15=>j.connection.execute_batch("CREATE TRIGGER refuse_watermark BEFORE UPDATE ON vm_metadata BEGIN SELECT RAISE(ABORT,'watermark refused'); END;").unwrap(),
            16=>j.connection.execute_batch("CREATE TABLE commit_parent(id INTEGER PRIMARY KEY);CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED);CREATE TRIGGER refuse_commit AFTER INSERT ON vm_merge_branches WHEN NEW.position=1 BEGIN INSERT INTO commit_child VALUES(1); END;").unwrap(),
            17=>requests[1].continuation.deadline_at_ms=Some(u64::MAX),
            18|19=>{
                let n=MAX_JOURNAL_RECORDS-if kind==18 {3}else{2};
                j.connection.execute_batch(&format!("WITH RECURSIVE n(x) AS (VALUES(1) UNION ALL SELECT x+1 FROM n WHERE x<{n}) INSERT INTO vm_effects(token,state,image) SELECT 'filler-'||x,'pending',x'7b' FROM n;")).unwrap();
            },
            20=>j.connection.execute_batch(&format!("ALTER TABLE vm_merge_groups RENAME TO saved_groups;CREATE VIEW vm_merge_groups AS SELECT zeroblob({MAX_JOURNAL_TOTAL_BYTES}) AS plan,NULL AS terminal_step;")).unwrap(),
            21=>j.connection.execute_batch("DROP TABLE vm_effects;").unwrap(),
            22=>j.connection.execute_batch("DROP TABLE vm_dispatches;").unwrap(),
            23=>j.connection.execute_batch("ALTER TABLE vm_merge_groups RENAME TO saved_groups;CREATE VIEW vm_merge_groups AS SELECT x'00' AS plan,NULL AS terminal_step;").unwrap(),
            24=>p.branches[1]="x".repeat(MAX_CONTINUATION_BYTES),
            25=>requests[1].continuation.pending_effect=leselang_hir::Effect::UiFocus { node_id:"x".repeat(MAX_CONTINUATION_BYTES) },
            26=>{j.connection.execute("UPDATE vm_merge_groups SET state='completed',terminal_step=?1",[serde_json::to_vec(&done()).unwrap()]).unwrap();},
            27=>t=ContinuationToken::from_wire("merge-500".into()),
            28=>t=ContinuationToken::from_wire("continuation-600".into()),
            29=>t=ContinuationToken::from_wire("legacy-owner".into()),
            31=>{},
            _=>unreachable!(),
        }
        j.connection
            .execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
        (t, p, requests, limits)
    }
    #[test]
    fn sqlite_graph_registration_matches_frozen_order_limits_conflicts_and_late_rollback() {
        let inputs = crate::continuation_contract_tests::requests();
        let mut cases = 0;
        for index in [0, 35, 69] {
            for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
                for kind in 0..32 {
                    let f = Fixture::new();
                    let path = f.0.with_extension("reference.sqlite3");
                    std::fs::copy(&f.0, &path).unwrap();
                    let mut actual = SqliteJournal {
                        connection: Connection::open(&f.0).unwrap(),
                    };
                    let mut original = SqliteJournal {
                        connection: Connection::open(&path).unwrap(),
                    };
                    let (t, p, requests, limits) = mutate(&mut actual, &inputs[index], order, kind);
                    mutate(&mut original, &inputs[index], order, kind);
                    let before = snapshot(&actual.connection);
                    assert_eq!(before, snapshot(&original.connection));
                    let mut graph = graph(&requests);
                    if kind == 31 {
                        graph[1].0 = "left";
                    }
                    let result = actual.record_merge_graph(&t, &p, &graph, limits);
                    assert_eq!(
                        result,
                        original.frozen_record_merge_graph(&t, &p, &graph, limits),
                        "operation {index}, order {order:?}, case {kind}"
                    );
                    assert_eq!(snapshot(&actual.connection), snapshot(&original.connection));
                    assert_eq!(
                        actual.connection.total_changes(),
                        original.connection.total_changes()
                    );
                    assert!(actual.connection.is_autocommit());
                    assert!(original.connection.is_autocommit());
                    assert_eq!(
                        result.is_ok(),
                        matches!(kind, 0 | 8 | 10 | 18 | 27..=30),
                        "case {kind}: {result:?}"
                    );
                    if result.is_err() {
                        assert_eq!(snapshot(&actual.connection), before);
                    }
                    cases += 1;
                }
            }
        }
        assert_eq!(cases, 192);
    }
    #[test]
    fn graph_reopen_keeps_dispatch_leases_order_watermarks_and_duplicate_refusal() {
        let mut cases = 0;
        for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
            for name in ["merge-500", "continuation-600", "legacy-owner"] {
                let f = Fixture::new();
                let p = plan(order);
                let t = ContinuationToken::from_wire(name.into());
                let requests = [request(7), request(200)];
                let mut j = SqliteJournal::open(&f.0).unwrap();
                validate_merge_graph_input(&t, &p, &graph(&requests)).unwrap();
                j.record_merge_graph(&t, &p, &graph(&requests), SchedulerLimits::default())
                    .unwrap();
                let claim = j
                    .claim_dispatch(1, 101, SchedulerLimits::default())
                    .unwrap();
                drop(j);
                let mut j = SqliteJournal::open(&f.0).unwrap();
                let loaded = j.load().unwrap();
                assert_eq!(loaded.pending.len(), 2);
                assert_eq!(
                    loaded.next_sequence,
                    match name {
                        "merge-500" => 501,
                        "continuation-600" => 601,
                        _ => 201,
                    }
                );
                let before = snapshot(&j.connection);
                assert_eq!(
                    j.record_merge_graph(&t, &p, &graph(&requests), SchedulerLimits::default())
                        .unwrap_err()
                        .code,
                    "LSV4011"
                );
                assert_eq!(snapshot(&j.connection), before);
                let bytes = serde_json::to_vec(&claim).unwrap();
                assert!(String::from_utf8(bytes).unwrap().contains("101"));
                let lease:(i64,i64)=j.connection.query_row("SELECT attempt,lease_expires_at_ms FROM vm_dispatches WHERE state='leased'",[],|r|Ok((r.get(0)?,r.get(1)?))).unwrap();
                assert_eq!(lease, (1, 101));
                assert_eq!(j.allocate_sequence(1).unwrap(), loaded.next_sequence);
                cases += 1;
            }
        }
        assert_eq!(cases, 6);
    }
    #[test]
    fn public_group_start_commit_failures_do_not_publish_partial_caches() {
        let mut cases = 0;
        for mode in ["all", "seq"] {
            for failure in 0..3 {
                let f = Fixture::new();
                let mut vm = crate::Vm::open_journal(&f.0, 500).unwrap();
                let c = Connection::open(&f.0).unwrap();
                c.execute_batch(match failure {
                    0=>"CREATE TRIGGER refuse_graph BEFORE INSERT ON vm_effects WHEN (SELECT COUNT(*) FROM vm_effects)=1 BEGIN SELECT RAISE(ABORT,'late effect refused'); END;",
                    1=>"CREATE TRIGGER refuse_graph BEFORE INSERT ON vm_merge_branches WHEN NEW.position=1 BEGIN SELECT RAISE(ABORT,'late link refused'); END;",
                    _=>"CREATE TABLE commit_parent(id INTEGER PRIMARY KEY);CREATE TABLE commit_child(parent INTEGER REFERENCES commit_parent(id) DEFERRABLE INITIALLY DEFERRED);CREATE TRIGGER refuse_graph AFTER INSERT ON vm_merge_branches WHEN NEW.position=1 BEGIN INSERT INTO commit_child VALUES(1); END;",
                }).unwrap();
                let source = format!(
                    "fn main() = {mode}(left: ui.focus(node_id: \"a\"), right: ui.focus(node_id: \"b\"))"
                );
                let program = leselang_hir::lower(&leselang_syntax::parse(&source)).unwrap();
                let result = vm.start(
                    &program,
                    crate::Principal::new("operator").unwrap(),
                    crate::CapabilitySet::new(["ui.presentation"]),
                    None,
                );
                assert!(matches!(result, Step::Fault(_)), "{result:?}");
                assert!(vm.pending.is_empty());
                assert!(vm.completed.is_empty());
                for table in [
                    "vm_effects",
                    "vm_dispatches",
                    "vm_merge_groups",
                    "vm_merge_branches",
                ] {
                    let n: i64 = c
                        .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |r| r.get(0))
                        .unwrap();
                    assert_eq!(n, 0);
                }
                // Root sequence reservations precede this transaction and are not refunded.
                let next: i64 = c
                    .query_row("SELECT next_sequence FROM vm_metadata", [], |r| r.get(0))
                    .unwrap();
                assert!(next > 1);
                c.execute_batch("DROP TRIGGER refuse_graph;").unwrap();
                let result = vm.start(
                    &program,
                    crate::Principal::new("operator").unwrap(),
                    crate::CapabilitySet::new(["ui.presentation"]),
                    None,
                );
                assert!(!matches!(result, Step::Fault(_)), "{result:?}");
                assert_eq!(vm.pending_count(), 2);
                drop(vm);
                assert_eq!(
                    crate::Vm::open_journal(&f.0, 500).unwrap().pending_count(),
                    2
                );
                cases += 1;
            }
        }
        assert_eq!(cases, 6);
    }

    #[test]
    fn bound_graph_registration_preserves_all_group_start_shapes_and_reserved_successors() {
        let mut cases = 0;
        for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
            for kind in 0..6 {
                let mut vm = crate::Vm::new(500);
                vm.next_effect_id = 500;
                let branches = ["left", "right"].map(|name| leselang_hir::HirBranch {
                    name: name.into(),
                    effect: leselang_hir::Effect::UiFocus {
                        node_id: name.into(),
                    },
                    result_type: leselang_hir::Type::UiFocus,
                });
                let started = vm.start_group(
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
                assert!(!matches!(started, Step::Fault(_)), "{started:?}");
                let Journal::Ephemeral(memory) = &vm.journal else {
                    unreachable!()
                };
                let (token, group) = memory.merge_groups.iter().next().unwrap();
                let graph = group
                    .plan
                    .branches
                    .iter()
                    .zip(&group.branch_tokens)
                    .map(|(name, t)| (name.as_str(), &memory.dispatches[t].request))
                    .collect::<Vec<_>>();
                validate_merge_graph_input(token, &group.plan, &graph).unwrap();
                let f = Fixture::new();
                let mut actual = SqliteJournal::open(&f.0).unwrap();
                let mut original =
                    SqliteJournal::open(&f.0.with_extension("reference.sqlite3")).unwrap();
                actual
                    .record_merge_graph(token, &group.plan, &graph, SchedulerLimits::default())
                    .unwrap();
                original
                    .frozen_record_merge_graph(
                        token,
                        &group.plan,
                        &graph,
                        SchedulerLimits::default(),
                    )
                    .unwrap();
                assert_eq!(snapshot(&actual.connection), snapshot(&original.connection));
                assert_eq!(actual.load().unwrap().next_sequence, vm.next_effect_id);
                cases += 1;
            }
        }
        assert_eq!(cases, 12);
    }
}

// Frozen graph-registration bodies from HEAD 8b792476. Native admission,
// codecs/storage and watermark primitives remain shared, not coordination.
impl EphemeralJournal {
    fn frozen_record_merge_graph(
        &mut self,
        group_token: &ContinuationToken,
        plan: &MergePlan,
        branches: &[(&str, &EffectRequest)],
        limits: SchedulerLimits,
    ) -> Result<(), Fault> {
        for (_, request) in branches {
            validate_continuation_encoding_size(&request.continuation)?;
        }
        if self.merge_groups.contains_key(group_token)
            || self.dispatches.contains_key(group_token)
            || branches.iter().any(|(_, request)| {
                self.dispatches.contains_key(&request.continuation.token)
                    || self.merge_groups.contains_key(&request.continuation.token)
            })
        {
            return Err(journal_fault(
                "LSV4011",
                "merge graph token conflicts with pending state",
            ));
        }
        limits.check_admission(
            self.scheduler_pressure(0, limits).pending_dispatches,
            branches.len(),
        )?;

        let branch_tokens = branches
            .iter()
            .map(|(_, request)| request.continuation.token.clone())
            .collect::<Vec<_>>();
        for (_, request) in branches {
            self.dispatches.insert(
                request.continuation.token.clone(),
                EphemeralDispatch {
                    request: (*request).clone(),
                    attempt: 0,
                    lease_expires_at_ms: None,
                    ready_at_ms: 0,
                    retry_count: 0,
                    last_error: None,
                    acknowledged: false,
                    terminal_step: None,
                },
            );
        }
        self.merge_groups.insert(
            group_token.clone(),
            EphemeralMergeGroup {
                plan: plan.clone(),
                branch_tokens,
                terminal_step: None,
            },
        );
        Ok(())
    }
}
#[cfg(feature = "sqlite-journal")]
impl SqliteJournal {
    fn frozen_record_merge_graph(
        &mut self,
        group_token: &ContinuationToken,
        plan: &MergePlan,
        branches: &[(&str, &EffectRequest)],
        limits: SchedulerLimits,
    ) -> Result<(), Fault> {
        let plan_bytes = encode_json_capped(plan, MAX_CONTINUATION_BYTES, "merge plan")?;
        let encoded = branches
            .iter()
            .map(|(name, request)| {
                Ok((
                    *name,
                    encode_json_capped(
                        &request.continuation,
                        MAX_CONTINUATION_BYTES,
                        "continuation",
                    )?,
                    encode_json_capped(request, MAX_JOURNAL_ENTRY_BYTES, "effect request")?,
                ))
            })
            .collect::<Result<Vec<_>, Fault>>()?;
        let additional_bytes = plan_bytes.len()
            + encoded
                .iter()
                .map(|(_, image, request)| image.len().saturating_add(request.len()))
                .sum::<usize>();
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4032", "failed to lock merge graph", error))?;
        let current_count = journal_record_count(&transaction)?;
        let added_records = branches.len().saturating_add(1);
        if current_count.saturating_add(added_records) > MAX_JOURNAL_RECORDS {
            return Err(journal_fault(
                "LSV4006",
                format!("journal record limit {MAX_JOURNAL_RECORDS} reached"),
            ));
        }
        limits.check_admission(
            sql_scheduler_pressure(&transaction, 0, limits)?.pending_dispatches,
            branches.len(),
        )?;
        ensure_growth(&transaction, additional_bytes)?;

        let group_conflict: bool = transaction
            .query_row(
                "SELECT EXISTS(
                   SELECT 1 FROM vm_merge_groups WHERE token = ?1
                   UNION ALL
                   SELECT 1 FROM vm_effects WHERE token = ?1
                 )",
                [group_token.as_str()],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error("LSV4032", "failed to validate merge group token", error)
            })?;
        if group_conflict {
            return Err(journal_fault(
                "LSV4011",
                "merge graph token conflicts with durable state",
            ));
        }

        transaction
            .execute(
                "INSERT INTO vm_merge_groups(token, plan, state, terminal_step, execution_order)
                 VALUES (?1, ?2, 'pending', NULL, ?3)",
                params![
                    group_token.as_str(),
                    plan_bytes,
                    execution_order_label(plan.order)
                ],
            )
            .map_err(|error| journal_error("LSV4032", "failed to store merge group", error))?;
        for (position, ((_, request), (name, image_bytes, request_bytes))) in
            branches.iter().zip(encoded.iter()).enumerate()
        {
            let position = i64::try_from(position)
                .map_err(|_| journal_fault("LSV4032", "merge branch position is invalid"))?;
            transaction
                .execute(
                    "INSERT INTO vm_effects(token, state, image, deadline_at_ms, terminal_step)
                     VALUES (?1, 'pending', ?2, ?3, NULL)",
                    params![
                        request.continuation.token.as_str(),
                        image_bytes,
                        request
                            .continuation
                            .deadline_at_ms
                            .map(i64::try_from)
                            .transpose()
                            .map_err(|_| {
                                journal_fault("LSV4032", "effect deadline is out of range")
                            })?
                    ],
                )
                .map_err(|error| {
                    journal_error("LSV4032", "failed to store merge branch effect", error)
                })?;
            transaction
                .execute(
                    "INSERT INTO vm_dispatches(token, request, state, attempt, lease_expires_at_ms)
                     VALUES (?1, ?2, 'ready', 0, NULL)",
                    params![request.continuation.token.as_str(), request_bytes],
                )
                .map_err(|error| {
                    journal_error("LSV4032", "failed to store merge branch dispatch", error)
                })?;
            transaction
                .execute(
                    "INSERT INTO vm_merge_branches(
                       group_token, branch_token, branch_name, position
                     ) VALUES (?1, ?2, ?3, ?4)",
                    params![
                        group_token.as_str(),
                        request.continuation.token.as_str(),
                        name,
                        position
                    ],
                )
                .map_err(|error| journal_error("LSV4032", "failed to link merge branch", error))?;
        }
        let highest_sequence = branches
            .iter()
            .filter_map(|(_, request)| {
                crate::canonical_continuation_sequence(&request.continuation.token)
            })
            .fold(merge_reserved_sequence(group_token, plan), u64::max);
        advance_sequence_watermark(&transaction, highest_sequence)?;
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4032", "failed to commit merge graph", error))
    }
}
