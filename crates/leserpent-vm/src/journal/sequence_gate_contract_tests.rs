use super::*;

fn request(n: u64) -> EffectRequest {
    crate::Vm::build_effect_request(
        &leselang_hir::Effect::UiFocus {
            node_id: format!("node-{n}"),
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
const ORDERS: [ExecutionOrder; 14] = [
    ExecutionOrder::Parallel,
    ExecutionOrder::Sequential,
    ExecutionOrder::ResultChain,
    ExecutionOrder::Dataflow,
    ExecutionOrder::BoundParallel,
    ExecutionOrder::BoundSequential,
    ExecutionOrder::GroupTail,
    ExecutionOrder::GroupCapture,
    ExecutionOrder::ParallelGroupTail,
    ExecutionOrder::ParallelGroupCapture,
    ExecutionOrder::GroupDataflow,
    ExecutionOrder::ParallelGroupDataflow,
    ExecutionOrder::GroupConditional,
    ExecutionOrder::ParallelGroupConditional,
];
#[test]
fn memory_gate_matches_frozen_for_all_order_labels_prefixes_and_terminal_classes() {
    let mut cases = 0;
    let requests = [request(1), request(2), request(3)];
    let tokens = requests
        .iter()
        .map(|r| r.continuation.token.clone())
        .collect::<Vec<_>>();
    for order in ORDERS {
        for closed in [false, true] {
            for state in 0..9 {
                for position in 0..5 {
                    for acknowledged in [false, true] {
                        let mut j = EphemeralJournal::default();
                        for r in &requests {
                            j.record_pending(Some(r), SchedulerLimits::default())
                                .unwrap();
                        }
                        let step = match state {
                            0 | 1 => None,
                            2 => Some(done()),
                            3 => Some(Step::Fault(journal_fault("HOST", "failed"))),
                            4 => Some(deadline_cancellation(&requests[0].continuation, 1)),
                            5 => Some(Step::Yield(requests[0].continuation.clone())),
                            6 => Some(Step::Waiting(crate::MergeWait {
                                merge_token: tokens[0].clone(),
                                completed_branches: 0,
                                total_branches: 2,
                            })),
                            7 => Some(Step::Effect(Box::new(requests[0].clone()))),
                            8 => Some(Step::Done(crate::Value::UiFocus {
                                node_id: String::new(),
                            })),
                            _ => unreachable!(),
                        };
                        let first = j.dispatches.get_mut(&tokens[0]).unwrap();
                        first.acknowledged = acknowledged;
                        first.terminal_step = step;
                        if state == 0 {
                            j.dispatches.remove(&tokens[0]);
                        }
                        j.dispatches.get_mut(&tokens[1]).unwrap().terminal_step = Some(done());
                        let mut plan = MergePlan::new(["a", "b", "c"]).unwrap();
                        plan.order = order;
                        let branches = if position == 4 {
                            vec![tokens[0].clone(), tokens[1].clone(), tokens[0].clone()]
                        } else {
                            tokens.clone()
                        };
                        j.merge_groups.insert(
                            ContinuationToken::from_wire("merge-9".into()),
                            EphemeralMergeGroup {
                                plan,
                                branch_tokens: branches,
                                terminal_step: closed.then(done),
                            },
                        );
                        let target = match position {
                            0 => ContinuationToken::from_wire("missing".into()),
                            1 | 4 => tokens[0].clone(),
                            2 => tokens[1].clone(),
                            _ => tokens[2].clone(),
                        };
                        let before = memory_snapshot(&j);
                        let ready = j.sequence_ready(&target);
                        assert_eq!(
                            ready,
                            j.frozen_sequence_ready(&target),
                            "{order:?} closed={closed} state={state} position={position} ack={acknowledged}"
                        );
                        let expected = !order.is_sequential()
                            || position == 0
                            || (!closed && (matches!(position, 1 | 4) || matches!(state, 2 | 8)));
                        assert_eq!(ready, expected);
                        assert_eq!(memory_snapshot(&j), before);
                        cases += 1;
                    }
                }
            }
        }
    }
    assert_eq!(cases, 2520);
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
fn reply(request: &EffectRequest) -> crate::EffectResult {
    let crate::EffectOperation::Presentation(e) = &request.operation else {
        panic!()
    };
    let crate::PresentationOperation::Focus { node_id } = &e.operation else {
        panic!()
    };
    crate::EffectResult::Presentation(crate::PresentationResult::Focus {
        node_id: node_id.clone(),
    })
}
fn compare_gates(vm: &crate::Vm) {
    match &vm.journal {
        Journal::Ephemeral(j) => {
            for g in j.merge_groups.values() {
                for t in &g.branch_tokens {
                    assert_eq!(j.sequence_ready(t), j.frozen_sequence_ready(t));
                }
            }
        }
        #[cfg(feature = "sqlite-journal")]
        Journal::Sqlite(j) => {
            let tokens = j
                .connection
                .prepare("SELECT branch_token FROM vm_merge_branches ORDER BY position")
                .unwrap()
                .query_map([], |r| r.get::<_, String>(0))
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            for t in tokens {
                assert_eq!(
                    ensure_sequence_ready(&j.connection, &t),
                    frozen_ensure_sequence_ready(&j.connection, &t)
                );
            }
        }
    }
}
fn drive(
    mut vm: crate::Vm,
    order: ExecutionOrder,
    kind: usize,
    reopen: impl FnOnce(crate::Vm) -> crate::Vm,
) {
    start_group(&mut vm, order, kind);
    compare_gates(&vm);
    if order.is_sequential() {
        let later = vm.pending_continuations()[1].clone();
        let result = crate::EffectResult::Presentation(crate::PresentationResult::Focus {
            node_id: "right".into(),
        });
        assert!(
            matches!(vm.resume(&later, result), Step::Fault(Fault { code, .. }) if code == "LSV2410")
        );
        assert_eq!(vm.pending_count(), 2);
    }
    let first = vm.claim_effect(1, 100).unwrap().unwrap();
    if order.is_sequential() {
        assert!(vm.claim_effect(2, 100).unwrap().is_none());
    }
    assert!(!matches!(
        vm.acknowledge_effect(&first, 3, reply(&first.request)),
        Step::Fault(_)
    ));
    vm = reopen(vm);
    for i in 0..8 {
        compare_gates(&vm);
        if vm.pending_count() == 0 {
            assert!(vm.claim_effect(30, 100).unwrap().is_none());
            return;
        }
        let lease = vm.claim_effect(4 + i * 2, 100).unwrap().unwrap();
        assert!(!matches!(
            vm.acknowledge_effect(&lease, 5 + i * 2, reply(&lease.request)),
            Step::Fault(_)
        ));
    }
    panic!("group did not finish");
}
#[test]
fn memory_public_claim_and_completion_keep_sequence_gates_for_twelve_group_shapes() {
    let mut cases = 0;
    for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
        for kind in 0..6 {
            drive(crate::Vm::new(500), order, kind, |vm| vm);
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
    fn mutate(c: &Connection, first: &str, kind: usize) {
        c.execute_batch("PRAGMA foreign_keys=OFF; PRAGMA ignore_check_constraints=ON;")
            .unwrap();
        match kind {
            0 => {}
            1 => c
                .execute_batch("UPDATE vm_merge_groups SET execution_order='unknown';")
                .unwrap(),
            2 => c
                .execute_batch("UPDATE vm_merge_groups SET plan=x'7b';")
                .unwrap(),
            3 => c
                .execute_batch("UPDATE vm_merge_groups SET terminal_step=x'7b';")
                .unwrap(),
            4 => {
                c.execute(
                    "UPDATE vm_effects SET state='completed',terminal_step=x'7b' WHERE token=?1",
                    [first],
                )
                .unwrap();
            }
            5 => {
                c.execute("DELETE FROM vm_effects WHERE token=?1", [first])
                    .unwrap();
            }
            6 => c.execute_batch("DELETE FROM vm_merge_branches;").unwrap(),
            7 => {
                c.execute(
                    "UPDATE vm_effects SET state='invalid' WHERE token=?1",
                    [first],
                )
                .unwrap();
            }
            8 => c
                .execute_batch("UPDATE vm_merge_groups SET state='completed',terminal_step=x'7b';")
                .unwrap(),
            9 => c.execute_batch("DROP TABLE vm_merge_groups;").unwrap(),
            10 => c.execute_batch("DROP TABLE vm_effects;").unwrap(),
            11 => c.execute_batch("DROP TABLE vm_merge_branches;").unwrap(),
            _ => unreachable!(),
        }
        c.execute_batch("PRAGMA foreign_keys=ON; PRAGMA ignore_check_constraints=OFF;")
            .unwrap();
    }
    #[test]
    fn sqlite_gate_matches_frozen_state_queries_and_native_faults_without_writes() {
        let mut cases = 0;
        for source in SOURCES {
            for stage in 0..3 {
                for kind in 0..12 {
                    let f = Fixture::new(source, stage);
                    let c = Connection::open(&f.0).unwrap();
                    let mut tokens = c
                        .prepare(
                            "SELECT branch_token FROM vm_merge_branches ORDER BY position LIMIT 2",
                        )
                        .unwrap()
                        .query_map([], |r| r.get::<_, String>(0))
                        .unwrap()
                        .collect::<Result<Vec<_>, _>>()
                        .unwrap();
                    tokens.push("missing".into());
                    mutate(&c, &tokens[0], kind);
                    let before = snapshot(&c);
                    let changes = c.total_changes();
                    for t in tokens {
                        assert_eq!(
                            ensure_sequence_ready(&c, &t),
                            frozen_ensure_sequence_ready(&c, &t),
                            "{source} stage {stage} kind {kind} token {t}"
                        );
                        assert_eq!(snapshot(&c), before);
                        assert_eq!(c.total_changes(), changes);
                        assert!(c.is_autocommit());
                        cases += 1;
                    }
                }
            }
        }
        assert_eq!(cases, 432);
    }
    #[test]
    fn sequence_observation_borrows_outer_transaction_without_commit_or_repair() {
        let f = Fixture::new(SOURCES[0], 0);
        let mut c = Connection::open(&f.0).unwrap();
        let tokens = c
            .prepare("SELECT branch_token FROM vm_merge_branches ORDER BY position")
            .unwrap()
            .query_map([], |r| r.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();
        let before = snapshot(&c);
        assert_eq!(
            ensure_sequence_ready(&c, &tokens[1]).unwrap_err().code,
            "LSV2410"
        );
        let tx = c
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .unwrap();
        tx.execute(
            "UPDATE vm_effects SET state='completed',terminal_step=?2 WHERE token=?1",
            params![tokens[0], serde_json::to_vec(&done()).unwrap()],
        )
        .unwrap();
        let changed = snapshot(&tx);
        let changes = tx.total_changes();
        assert_eq!(ensure_sequence_ready(&tx, &tokens[1]), Ok(()));
        assert_eq!(snapshot(&tx), changed);
        assert_eq!(tx.total_changes(), changes);
        assert!(!tx.is_autocommit());
        tx.rollback().unwrap();
        assert_eq!(snapshot(&c), before);
        assert_eq!(
            ensure_sequence_ready(&c, &tokens[1]).unwrap_err().code,
            "LSV2410"
        );
    }
    #[test]
    fn durable_public_claim_completion_and_restart_keep_gates_for_twelve_group_shapes() {
        let mut cases = 0;
        for order in [ExecutionOrder::Parallel, ExecutionOrder::Sequential] {
            for kind in 0..6 {
                let f = Fixture::new(SOURCES[0], 0);
                let path = f.0.with_extension("public.sqlite3");
                drive(
                    crate::Vm::open_journal(&path, 500).unwrap(),
                    order,
                    kind,
                    |vm| {
                        drop(vm);
                        crate::Vm::open_journal(&path, 500).unwrap()
                    },
                );
                cases += 1;
            }
        }
        assert_eq!(cases, 12);
    }
}

// Frozen before extraction at 63df912e; no migrated gate policy is shared.
impl EphemeralJournal {
    fn frozen_sequence_ready(&self, token: &ContinuationToken) -> bool {
        self.merge_groups.values().all(|group| {
            if !group.plan.order.is_sequential() {
                return true;
            }
            let Some(position) = group
                .branch_tokens
                .iter()
                .position(|branch| branch == token)
            else {
                return true;
            };
            group.terminal_step.is_none()
                && group.branch_tokens[..position].iter().all(|token| {
                    self.dispatches.get(token).is_some_and(|dispatch| {
                        matches!(dispatch.terminal_step, Some(Step::Done(_)))
                    })
                })
        })
    }
}

#[cfg(feature = "sqlite-journal")]
fn frozen_ensure_sequence_ready(connection: &Connection, token: &str) -> Result<(), Fault> {
    let blocked: bool = connection.query_row(
        "SELECT EXISTS (
           SELECT 1 FROM vm_merge_branches own
           JOIN vm_merge_groups g ON g.token = own.group_token
           JOIN vm_merge_branches prior ON prior.group_token = own.group_token AND prior.position < own.position
           JOIN vm_effects e ON e.token = prior.branch_token
           WHERE own.branch_token = ?1 AND g.execution_order = 'sequential' AND e.state != 'completed'
         )", [token], |row| row.get(0)
    ).map_err(|error| journal_error("LSV4033", "failed to check sequential predecessor", error))?;
    if blocked {
        return Err(journal_fault(
            "LSV2410",
            "sequential predecessor has not completed",
        ));
    }
    Ok(())
}
