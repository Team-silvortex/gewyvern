use super::*;

use crate::group_start_fixture as fixture;

#[test]
fn group_start_matches_frozen_whole_vm_policy_across_seventy_operations_and_native_state() {
    let mut comparisons = 0;
    let mut valid_shapes = 0;
    for request in crate::continuation_contract_tests::requests() {
        for parallel in [false, true] {
            for kind in 0..6 {
                for defect in 0..18 {
                    let mut branches = ["a", "b"]
                        .map(|name| leselang_hir::HirBranch {
                            name: name.into(),
                            effect: request.continuation.pending_effect.clone(),
                            result_type: request.continuation.result_type,
                        })
                        .to_vec();
                    let mut saved = fixture::binding(kind);
                    let mut principal = Principal::new("operator").unwrap();
                    let mut capabilities = request.operation.authority().1.clone();
                    let mut deadline = 30_000;
                    let mut deadline_at = Some(31_100);
                    let mut fuel = 100;
                    let mut actual_vm = Vm::new(500);
                    let mut expected_vm = Vm::new(500);
                    match defect {
                        1 => fuel = 0,
                        2 => fuel = 1,
                        3 => fuel = 2,
                        4 => fuel = MAX_EXECUTION_FUEL + 2,
                        5 => principal.id.clear(),
                        6 => capabilities = CapabilitySet::default(),
                        7 => branches[0].name = "bad name".into(),
                        8 => branches[1].name = "a".into(),
                        9 => branches[0].effect = Effect::All { branches: vec![] },
                        10 => branches[1].result_type = Type::Structured,
                        11 => deadline = 0,
                        12 => deadline_at = Some(u64::MAX),
                        13 => {
                            let limits = SchedulerLimits {
                                max_pending_dispatches: 1,
                                ..SchedulerLimits::default()
                            };
                            actual_vm.set_scheduler_limits(limits).unwrap();
                            expected_vm.set_scheduler_limits(limits).unwrap();
                        }
                        14 | 15 => {
                            let next = MAX_EFFECT_SEQUENCE - if defect == 15 { 2 } else { 0 };
                            actual_vm.next_effect_id = next;
                            expected_vm.next_effect_id = next;
                        }
                        16 => {
                            if let Some(saved) = &mut saved {
                                saved.name = "bad name".into();
                            }
                        }
                        17 => branches.clear(),
                        _ => {}
                    }
                    let before = serde_json::to_vec(&branches).unwrap();
                    let order = if parallel {
                        ExecutionOrder::Parallel
                    } else {
                        ExecutionOrder::Sequential
                    };
                    let expected = frozen_start_group(
                        &mut expected_vm,
                        &branches,
                        order,
                        principal.clone(),
                        capabilities.clone(),
                        Some(Revision(7)),
                        deadline,
                        deadline_at,
                        fuel,
                        saved.clone(),
                    );
                    let actual = actual_vm.start_group(
                        &branches,
                        order,
                        principal,
                        capabilities,
                        Some(Revision(7)),
                        deadline,
                        deadline_at,
                        fuel,
                        saved,
                    );
                    assert_eq!(
                        actual, expected,
                        "parallel={parallel}, kind={kind}, defect={defect}"
                    );
                    assert_eq!(
                        serde_json::to_vec(&actual).unwrap(),
                        serde_json::to_vec(&expected).unwrap()
                    );
                    assert_eq!(actual_vm.next_effect_id, expected_vm.next_effect_id);
                    assert_eq!(actual_vm.pending, expected_vm.pending);
                    assert_eq!(actual_vm.completed, expected_vm.completed);
                    assert_eq!(
                        actual_vm.scheduler_pressure(1),
                        expected_vm.scheduler_pressure(1)
                    );
                    for _ in 0..3 {
                        assert_eq!(
                            actual_vm.claim_effect(1, 100),
                            expected_vm.claim_effect(1, 100)
                        );
                    }
                    assert_eq!(
                        actual_vm.allocate_sequence(),
                        expected_vm.allocate_sequence()
                    );
                    assert_eq!(serde_json::to_vec(&branches).unwrap(), before);
                    if defect == 0 {
                        assert!(!matches!(actual, Step::Fault(_)), "{actual:?}");
                        valid_shapes += 1;
                    }
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 15120);
    assert_eq!(valid_shapes, 840);
}

#[cfg(feature = "sqlite-journal")]
#[test]
fn group_start_sqlite_publication_and_failed_writes_match_frozen_restart_state() {
    use rusqlite::{Connection, types::Value as SqlValue};
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(1);
    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    fn snapshot(path: &std::path::Path) -> Vec<(String, Vec<Vec<SqlValue>>)> {
        let db = Connection::open(path).unwrap();
        let tables = db.prepare("SELECT name FROM sqlite_master WHERE type='table' AND name LIKE 'vm_%' ORDER BY name").unwrap()
            .query_map([], |row| row.get::<_, String>(0)).unwrap().collect::<Result<Vec<_>, _>>().unwrap();
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
                            .collect::<Result<Vec<SqlValue>, _>>()
                    })
                    .unwrap()
                    .collect::<Result<Vec<_>, _>>()
                    .unwrap();
                (table, rows)
            })
            .collect()
    }
    let mut comparisons = 0;
    for parallel in [false, true] {
        for kind in 0..6 {
            for defect in 0..3 {
                let scratch = Scratch(std::env::temp_dir().join(format!(
                    "leselang-group-start-{}-{}",
                    std::process::id(),
                    NEXT.fetch_add(1, Ordering::Relaxed)
                )));
                std::fs::create_dir(&scratch.0).unwrap();
                let actual_path = scratch.0.join("actual.sqlite3");
                let expected_path = scratch.0.join("expected.sqlite3");
                let mut actual_vm = Vm::open_journal(&actual_path, 500).unwrap();
                let mut expected_vm = Vm::open_journal(&expected_path, 500).unwrap();
                if defect != 0 {
                    let table = if defect == 1 {
                        "vm_merge_groups"
                    } else {
                        "vm_dispatches"
                    };
                    for path in [&actual_path, &expected_path] {
                        Connection::open(path).unwrap().execute_batch(&format!("CREATE TRIGGER reject_group_start BEFORE INSERT ON {table} BEGIN SELECT RAISE(ABORT, 'injected'); END;")).unwrap();
                    }
                }
                let branches = ["a", "b"].map(|name| leselang_hir::HirBranch {
                    name: name.into(),
                    effect: Effect::UiFocus {
                        node_id: name.into(),
                    },
                    result_type: Type::UiFocus,
                });
                let order = if parallel {
                    ExecutionOrder::Parallel
                } else {
                    ExecutionOrder::Sequential
                };
                let expected = frozen_start_group(
                    &mut expected_vm,
                    &branches,
                    order,
                    Principal::new("operator").unwrap(),
                    CapabilitySet::new(["ui.presentation"]),
                    Some(Revision(7)),
                    30_000,
                    Some(31_100),
                    100,
                    fixture::binding(kind),
                );
                let actual = actual_vm.start_group(
                    &branches,
                    order,
                    Principal::new("operator").unwrap(),
                    CapabilitySet::new(["ui.presentation"]),
                    Some(Revision(7)),
                    30_000,
                    Some(31_100),
                    100,
                    fixture::binding(kind),
                );
                assert_eq!(actual, expected);
                assert_eq!(matches!(actual, Step::Fault(_)), defect != 0);
                assert_eq!(actual_vm.pending, expected_vm.pending);
                assert_eq!(actual_vm.next_effect_id, expected_vm.next_effect_id);
                assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
                if defect != 0 {
                    assert_eq!(actual_vm.pending_count(), 0);
                    assert!(
                        actual_vm.next_effect_id > 1,
                        "reservations must not be refunded"
                    );
                }
                drop(actual_vm);
                drop(expected_vm);
                let mut actual_vm = Vm::open_journal(&actual_path, 500).unwrap();
                let mut expected_vm = Vm::open_journal(&expected_path, 500).unwrap();
                assert_eq!(actual_vm.pending, expected_vm.pending);
                assert_eq!(actual_vm.next_effect_id, expected_vm.next_effect_id);
                assert_eq!(
                    actual_vm.claim_effect(1, 100),
                    expected_vm.claim_effect(1, 100)
                );
                assert_eq!(snapshot(&actual_path), snapshot(&expected_path));
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 36);
}

// Frozen pre-migration whole VM group start, including actual journal publication.
#[allow(clippy::too_many_arguments)]
fn frozen_start_group(
    vm: &mut Vm,
    branches: &[leselang_hir::HirBranch],
    order: ExecutionOrder,
    principal: Principal,
    capabilities: CapabilitySet,
    expected_revision: Option<Revision>,
    deadline_ms: u64,
    deadline_at_ms: Option<u64>,
    fuel_remaining: u64,
    result_binding: Option<Box<ResultBinding>>,
) -> Step {
    if branches.iter().any(|branch| {
        matches!(
            branch.effect,
            Effect::All { .. } | Effect::Sequence { .. } | Effect::Compute { .. }
        )
    }) {
        return fault(
            "LSV1002",
            "nested structured execution is not yet supported",
        );
    }
    let (order, result_binding) = if let Some(binding) = result_binding {
        let mut body_fuel = Fuel::new(fuel_remaining);
        if body_fuel.charge(branches.len() as u64).is_err() {
            return fault("LSV1001", "bound group exceeds execution fuel");
        }
        let (order, pending) = if order.is_sequential() {
            (
                ExecutionOrder::BoundSequential,
                Effect::Sequence {
                    steps: branches.to_vec(),
                },
            )
        } else {
            (
                ExecutionOrder::BoundParallel,
                Effect::All {
                    branches: branches.to_vec(),
                },
            )
        };
        (
            order,
            Some(Box::new(GroupResultBinding {
                pending,
                // Only a shape/size marker here; reserve the real identity after all prefix IDs.
                successor_sequence: if binding.body.is_pure() {
                    None
                } else {
                    Some(1)
                },
                additional_successor_sequences: (2..=binding.body.atomic_flow_bound().unwrap_or(0)
                    as u64)
                    .collect(),
                binding,
                fuel_remaining: body_fuel.remaining(),
            })),
        )
    } else {
        (order, None)
    };
    let order = if result_binding
        .as_ref()
        .is_some_and(|binding| binding.successor_sequence.is_some())
    {
        let captures = result_binding
            .as_ref()
            .is_some_and(|binding| binding.binding.body.is_atomic_capture());
        if result_binding
            .as_ref()
            .is_some_and(|binding| binding.is_conditional())
        {
            if order.is_parallel() {
                ExecutionOrder::ParallelGroupConditional
            } else {
                ExecutionOrder::GroupConditional
            }
        } else if result_binding
            .as_ref()
            .is_some_and(|binding| binding.is_dataflow())
        {
            if order.is_parallel() {
                ExecutionOrder::ParallelGroupDataflow
            } else {
                ExecutionOrder::GroupDataflow
            }
        } else {
            match (order.is_parallel(), captures) {
                (false, false) => ExecutionOrder::GroupTail,
                (false, true) => ExecutionOrder::GroupCapture,
                (true, false) => ExecutionOrder::ParallelGroupTail,
                (true, true) => ExecutionOrder::ParallelGroupCapture,
            }
        }
    } else {
        order
    };
    let mut plan = MergePlan {
        branches: branches.iter().map(|branch| branch.name.clone()).collect(),
        order,
        result_binding,
    };
    if let Err(error) = validate_merge_plan(&plan) {
        return Step::Fault(error);
    }
    if order.is_sequential() && branches.len() as u64 > fuel_remaining {
        return fault("LSV1001", "sequential control flow exceeds execution fuel");
    }
    if let Err(error) = vm
        .journal
        .check_admission(branches.len(), vm.scheduler_limits)
    {
        return Step::Fault(error);
    }
    let group_sequence = match vm.allocate_sequence() {
        Ok(sequence) => sequence,
        Err(error) => return Step::Fault(error),
    };
    let merge_token = ContinuationToken::from_wire(format!("merge-{group_sequence}"));
    let mut named = Vec::with_capacity(branches.len());
    for (index, branch) in branches.iter().enumerate() {
        let sequence = match vm.allocate_sequence() {
            Ok(sequence) => sequence,
            Err(error) => return Step::Fault(error),
        };
        let mut request = match Vm::build_effect_request(
            &branch.effect,
            branch.result_type,
            sequence,
            principal.clone(),
            capabilities.clone(),
            expected_revision,
            deadline_ms,
            deadline_at_ms,
            fuel_remaining,
        ) {
            Ok(request) => request,
            Err(error) => return Step::Fault(error),
        };
        if order.is_sequential() {
            let mut fuel = Fuel::new(request.continuation.fuel_remaining);
            if fuel.charge(index as u64).is_err() {
                return fault("LSV1001", "sequential control flow exceeds execution fuel");
            }
            request.continuation.fuel_remaining = fuel.remaining();
            request.budget.fuel_remaining = request.continuation.fuel_remaining;
        }
        if plan.result_binding.is_some() {
            request.continuation.schema_version = match order {
                ExecutionOrder::GroupConditional | ExecutionOrder::ParallelGroupConditional => {
                    GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION
                }
                ExecutionOrder::GroupDataflow | ExecutionOrder::ParallelGroupDataflow => {
                    GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
                }
                ExecutionOrder::ParallelGroupTail | ExecutionOrder::ParallelGroupCapture => {
                    PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION
                }
                ExecutionOrder::GroupCapture => GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION,
                ExecutionOrder::GroupTail => GROUP_TAIL_CONTINUATION_SCHEMA_VERSION,
                _ => GROUP_RESULT_CONTINUATION_SCHEMA_VERSION,
            };
            request.continuation.group_result = Some(merge_token.clone());
        }
        named.push(NamedEffectRequest {
            branch: branch.name.clone(),
            request,
        });
    }
    if order.has_group_successor() {
        let sequence = match vm.allocate_sequence() {
            Ok(sequence) => sequence,
            Err(error) => return Step::Fault(error),
        };
        if let Some(binding) = &mut plan.result_binding {
            binding.successor_sequence = Some(sequence);
            for reserved in &mut binding.additional_successor_sequences {
                *reserved = match vm.allocate_sequence() {
                    Ok(sequence) => sequence,
                    Err(error) => return Step::Fault(error),
                };
            }
        }
    }
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
