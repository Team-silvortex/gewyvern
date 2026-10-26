use super::*;

use leselang_command::reference_group_start::{
    GroupStartContext, ReferenceGroupStartHost, prepare_group_start,
};
use leselang_command::reference_publication::ReferenceRootPublicationHost;
use rusqlite::{Connection, types::Value as SqlValue};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "leselang-publication-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn path(&self, name: &str) -> PathBuf {
        self.0.join(name)
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn snapshot(path: &Path) -> Vec<(String, Vec<Vec<SqlValue>>)> {
    let db = Connection::open(path).unwrap();
    let tables = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name").unwrap().query_map([], |row| row.get::<_, String>(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
    tables
        .into_iter()
        .map(|table| {
            let mut stmt = db
                .prepare(&format!(
                    "SELECT * FROM \"{}\" ORDER BY rowid",
                    table.replace('"', "\"\"")
                ))
                .unwrap();
            let columns = stmt.column_count();
            let rows = stmt
                .query_map([], |row| {
                    (0..columns)
                        .map(|i| row.get(i))
                        .collect::<Result<Vec<_>, _>>()
                })
                .unwrap()
                .collect::<Result<Vec<_>, _>>()
                .unwrap();
            (table, rows)
        })
        .collect()
}
fn compare(actual: &Vm, expected: &Vm) {
    assert_eq!(actual.pending, expected.pending);
    assert_eq!(actual.completed, expected.completed);
    assert_eq!(actual.next_effect_id, expected.next_effect_id);
    assert_eq!(
        actual.scheduler_pressure(100),
        expected.scheduler_pressure(100)
    );
}
fn context() -> GroupStartContext {
    GroupStartContext {
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: Some(Revision(7)),
        deadline_ms: 30_000,
        deadline_at_ms: Some(31_100),
        fuel_remaining: 100,
    }
}
fn branches() -> [leserpent_hir::HirBranch; 2] {
    ["a", "b"].map(|name| leserpent_hir::HirBranch {
        name: name.into(),
        effect: Effect::UiFocus {
            node_id: name.into(),
        },
        result_type: Type::UiFocus,
    })
}
fn atomic_request(vm: &mut Vm) -> EffectRequest {
    let c = context();
    let sequence = vm.allocate_sequence().unwrap();
    Vm::build_effect_request(
        &branches()[0].effect,
        Type::UiFocus,
        sequence,
        c.principal,
        c.capabilities,
        c.expected_revision,
        c.deadline_ms,
        c.deadline_at_ms,
        c.fuel_remaining,
    )
    .unwrap()
}
fn run(vm: &mut Vm, actual: bool, shape: Option<(bool, usize)>) -> Step {
    if let Some((sequential, kind)) = shape {
        let c = context();
        let order = if sequential {
            ExecutionOrder::Sequential
        } else {
            ExecutionOrder::Parallel
        };
        if actual {
            vm.start_group(
                &branches(),
                order,
                c.principal,
                c.capabilities,
                c.expected_revision,
                c.deadline_ms,
                c.deadline_at_ms,
                c.fuel_remaining,
                group_start_fixture::binding(kind),
            )
        } else {
            group_start_contract_tests::frozen_start_group(
                vm,
                &branches(),
                order,
                c.principal,
                c.capabilities,
                c.expected_revision,
                c.deadline_ms,
                c.deadline_at_ms,
                c.fuel_remaining,
                group_start_fixture::binding(kind),
            )
        }
    } else {
        let request = atomic_request(vm);
        if actual {
            leselang_command::reference_start::ReferenceStartHost::commit_atomic(
                &mut ReferenceStartOwner(vm),
                request,
            )
        } else {
            frozen_atomic(vm, request)
        }
    }
}
fn shapes() -> Vec<Option<(bool, usize)>> {
    std::iter::once(None)
        .chain(
            [false, true]
                .into_iter()
                .flat_map(|sequential| (0..6).map(move |kind| Some((sequential, kind)))),
        )
        .collect()
}
fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}

#[test]
fn publication_sqlite_late_write_and_commit_failures_match_frozen_cache_and_restart() {
    let mut cases = 0;
    for shape in shapes() {
        let failures = if shape.is_some() {
            &['o', 'e', 'd', 'b', 'c'][..]
        } else {
            &['o', 'e', 'd', 'c'][..]
        };
        for &failure in failures {
            let scratch = Scratch::new();
            let a = scratch.path("actual");
            let b = scratch.path("frozen");
            let mut actual = Vm::open_journal(&a, 100).unwrap();
            let mut expected = Vm::open_journal(&b, 100).unwrap();
            if failure != 'o' {
                for path in [&a, &b] {
                    let db = Connection::open(path).unwrap();
                    if failure == 'c' {
                        db.execute_batch("CREATE TABLE failure_parent(id INTEGER PRIMARY KEY); CREATE TABLE failure_child(id INTEGER REFERENCES failure_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER reject_publication AFTER INSERT ON vm_dispatches BEGIN INSERT INTO failure_child VALUES(1); END;").unwrap();
                    } else {
                        let table = match failure {
                            'e' => "vm_effects",
                            'd' => "vm_dispatches",
                            'b' => "vm_merge_branches",
                            _ => unreachable!(),
                        };
                        let condition = if shape.is_none() {
                            "1"
                        } else if failure == 'b' {
                            "NEW.position=1"
                        } else {
                            "NEW.token='continuation-3'"
                        };
                        db.execute_batch(&format!("CREATE TRIGGER reject_publication BEFORE INSERT ON {table} WHEN {condition} BEGIN SELECT RAISE(ABORT, 'late publication failure'); END;")).unwrap();
                    }
                }
            }
            let want = run(&mut expected, false, shape);
            let got = run(&mut actual, true, shape);
            assert_eq!(got, want, "{shape:?} {failure}");
            assert_eq!(
                serde_json::to_vec(&got).unwrap(),
                serde_json::to_vec(&want).unwrap()
            );
            assert_eq!(matches!(got, Step::Fault(_)), failure != 'o');
            compare(&actual, &expected);
            assert_eq!(snapshot(&a), snapshot(&b));
            if failure != 'o' {
                assert!(actual.pending.is_empty());
                assert!(actual.completed.is_empty());
                assert!(actual.next_effect_id > 1);
                for path in [&a, &b] {
                    let db = Connection::open(path).unwrap();
                    for table in [
                        "vm_effects",
                        "vm_dispatches",
                        "vm_merge_groups",
                        "vm_merge_branches",
                    ] {
                        assert_eq!(
                            db.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| row
                                .get::<_, i64>(
                                0
                            ))
                            .unwrap(),
                            0
                        );
                    }
                    db.execute_batch("DROP TRIGGER reject_publication").unwrap();
                }
            }
            drop((actual, expected));
            let mut actual = Vm::open_journal(&a, 100).unwrap();
            let mut expected = Vm::open_journal(&b, 100).unwrap();
            compare(&actual, &expected);
            if failure != 'o' {
                assert_eq!(
                    run(&mut actual, true, shape),
                    run(&mut expected, false, shape)
                );
            }
            for now in 100..108 {
                let got = actual.claim_effect(now, 100).unwrap();
                let want = expected.claim_effect(now, 100).unwrap();
                assert_eq!(got, want);
                let Some(lease) = got else { break };
                let step = actual.acknowledge_effect(&lease, now, receipt(&lease.request));
                assert_eq!(
                    step,
                    expected.acknowledge_effect(&lease, now, receipt(&lease.request))
                );
            }
            compare(&actual, &expected);
            assert_eq!(snapshot(&a), snapshot(&b));
            cases += 1;
        }
    }
    assert_eq!(cases, 64);
}

// Inject a real second owner only after the first owner has completed preflight
// and all reservations, immediately before its native transaction rechecks limits.
struct RacingOwner<'a> {
    vm: &'a mut Vm,
    peer: &'a mut Vm,
    peer_runs: usize,
}
impl RacingOwner<'_> {
    fn admit_peer(&mut self) {
        self.peer_runs += 1;
        assert_eq!(self.peer_runs, 1);
        assert!(matches!(run(self.peer, true, None), Step::Effect(_)));
    }
}
impl ReferenceGroupStartHost for RacingOwner<'_> {
    fn check_admission(&mut self, additional: usize) -> Result<(), Fault> {
        ReferenceStartOwner(self.vm).check_admission(additional)
    }
    fn allocate_sequence(&mut self) -> Result<u64, Fault> {
        self.vm.allocate_sequence()
    }
}
impl ReferenceRootPublicationHost for RacingOwner<'_> {
    fn commit_atomic(&mut self, request: &EffectRequest) -> Result<(), Fault> {
        self.admit_peer();
        ReferenceRootPublicationHost::commit_atomic(&mut ReferenceStartOwner(self.vm), request)
    }
    fn commit_group(
        &mut self,
        token: &ContinuationToken,
        plan: &MergePlan,
        graph: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault> {
        self.admit_peer();
        ReferenceStartOwner(self.vm).commit_group(token, plan, graph)
    }
    fn publish_pending(&mut self, image: &ContinuationImage) {
        ReferenceStartOwner(self.vm).publish_pending(image);
    }
}

#[test]
fn publication_shared_journal_rechecks_capacity_after_peer_admission_without_refunding_ids() {
    let mut cases = 0;
    for shape in shapes() {
        let scratch = Scratch::new();
        let a = scratch.path("actual");
        let b = scratch.path("frozen");
        let limits = SchedulerLimits {
            max_pending_dispatches: if shape.is_some() { 2 } else { 1 },
            max_active_leases: 1,
        };
        let mut actual = Vm::open_journal_with_limits(&a, 100, limits).unwrap();
        let mut expected = Vm::open_journal_with_limits(&b, 100, limits).unwrap();
        let mut actual_peer = Vm::open_journal_with_limits(&a, 100, limits).unwrap();
        let mut expected_peer = Vm::open_journal_with_limits(&b, 100, limits).unwrap();
        let (got, want) = if let Some((sequential, kind)) = shape {
            let order = if sequential {
                ExecutionOrder::Sequential
            } else {
                ExecutionOrder::Parallel
            };
            let got = leselang_command::reference_group_start::start_group(
                &branches(),
                order,
                context(),
                group_start_fixture::binding(kind),
                &mut RacingOwner {
                    vm: &mut actual,
                    peer: &mut actual_peer,
                    peer_runs: 0,
                },
            );
            let prepared = prepare_group_start(
                &branches(),
                order,
                context(),
                group_start_fixture::binding(kind),
                &mut ReferenceStartOwner(&mut expected),
            )
            .unwrap();
            assert!(matches!(
                run(&mut expected_peer, true, None),
                Step::Effect(_)
            ));
            (got, frozen_group_publication(&mut expected, prepared))
        } else {
            actual.journal.check_admission(1, limits).unwrap();
            expected.journal.check_admission(1, limits).unwrap();
            let request = atomic_request(&mut actual);
            let got = leselang_command::reference_publication::publish_atomic(
                request,
                &mut RacingOwner {
                    vm: &mut actual,
                    peer: &mut actual_peer,
                    peer_runs: 0,
                },
            );
            let request = atomic_request(&mut expected);
            assert!(matches!(
                run(&mut expected_peer, true, None),
                Step::Effect(_)
            ));
            (got, frozen_atomic(&mut expected, request))
        };
        assert_eq!(got, want);
        assert!(matches!(got, Step::Fault(ref f) if f.code == "LSV2501"));
        compare(&actual, &expected);
        compare(&actual_peer, &expected_peer);
        assert!(actual.pending.is_empty());
        assert_eq!(actual_peer.pending.len(), 1);
        assert!(actual.next_effect_id > 1);
        assert_eq!(snapshot(&a), snapshot(&b));
        drop((actual, expected, actual_peer, expected_peer));
        let mut actual = Vm::open_journal_with_limits(&a, 100, limits).unwrap();
        let mut expected = Vm::open_journal_with_limits(&b, 100, limits).unwrap();
        compare(&actual, &expected);
        assert_eq!(actual.pending.len(), 1, "only the peer survived admission");
        let lease = actual.claim_effect(100, 100).unwrap().unwrap();
        assert_eq!(
            Some(lease.clone()),
            expected.claim_effect(100, 100).unwrap()
        );
        assert_eq!(
            actual.acknowledge_effect(&lease, 101, receipt(&lease.request)),
            expected.acknowledge_effect(&lease, 101, receipt(&lease.request))
        );
        assert_eq!(
            run(&mut actual, true, shape),
            run(&mut expected, false, shape)
        );
        assert_eq!(snapshot(&a), snapshot(&b));
        cases += 1;
    }
    assert_eq!(cases, 13);
}

// Frozen pre-extraction publication. The peer-race oracle shares preparation,
// while the existing whole-VM group/start oracles retain independent preparation.
fn frozen_atomic(vm: &mut Vm, request: EffectRequest) -> Step {
    if let Err(error) =
        vm.journal
            .record_pending(&request.continuation, Some(&request), vm.scheduler_limits)
    {
        return Step::Fault(error);
    }
    vm.pending.insert(
        request.continuation.token.clone(),
        request.continuation.clone(),
    );
    Step::Effect(Box::new(request))
}
fn frozen_group_publication(
    vm: &mut Vm,
    prepared: leselang_command::reference_group_start::PreparedGroupStart,
) -> Step {
    let leselang_command::reference_group_start::PreparedGroupStart {
        merge_token,
        plan,
        branches: mut named,
    } = prepared;
    let order = plan.order;
    let graph = named
        .iter()
        .map(|branch| (branch.branch.as_str(), &branch.request))
        .collect::<Vec<_>>();
    if let Err(error) =
        vm.journal
            .record_merge_graph_with_limits(&merge_token, &plan, &graph, vm.scheduler_limits)
    {
        return Step::Fault(error);
    }
    for branch in &named {
        vm.pending.insert(
            branch.request.continuation.token.clone(),
            branch.request.continuation.clone(),
        );
    }
    if order.is_sequential() {
        return Step::Effect(Box::new(named.remove(0).request));
    }
    Step::Effects(Box::new(StructuredEffectBatch {
        merge_token,
        branches: named,
    }))
}
