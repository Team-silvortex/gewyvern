use super::*;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

// Frozen released wire vocabulary and full plan admission before adapter migration.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FrozenMergePlan {
    pub branches: Vec<String>,
    #[serde(
        default,
        skip_serializing_if = "FrozenExecutionOrder::is_default_parallel"
    )]
    pub order: FrozenExecutionOrder,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub result_binding: Option<Box<GroupResultBinding>>,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FrozenExecutionOrder {
    #[default]
    Parallel,
    Sequential,
    ResultChain,
    Dataflow,
    BoundParallel,
    BoundSequential,
    GroupTail,
    GroupCapture,
    ParallelGroupTail,
    ParallelGroupCapture,
    GroupDataflow,
    ParallelGroupDataflow,
    GroupConditional,
    ParallelGroupConditional,
}

impl FrozenExecutionOrder {
    fn is_default_parallel(&self) -> bool {
        *self == Self::Parallel
    }

    fn is_parallel(&self) -> bool {
        matches!(
            self,
            Self::Parallel
                | Self::BoundParallel
                | Self::ParallelGroupTail
                | Self::ParallelGroupCapture
                | Self::ParallelGroupDataflow
                | Self::ParallelGroupConditional
        )
    }

    fn is_sequential(&self) -> bool {
        !self.is_parallel()
    }

    fn has_group_successor(&self) -> bool {
        matches!(
            self,
            Self::GroupTail
                | Self::GroupCapture
                | Self::ParallelGroupTail
                | Self::ParallelGroupCapture
                | Self::GroupDataflow
                | Self::ParallelGroupDataflow
                | Self::GroupConditional
                | Self::ParallelGroupConditional
        )
    }

    fn captures_group_successor(&self) -> bool {
        matches!(
            self,
            Self::GroupCapture
                | Self::ParallelGroupCapture
                | Self::GroupDataflow
                | Self::ParallelGroupDataflow
                | Self::GroupConditional
                | Self::ParallelGroupConditional
        )
    }
}

fn frozen_validate_merge_plan(plan: &MergePlan) -> Result<(), Fault> {
    if matches!(
        plan.order,
        ExecutionOrder::BoundParallel
            | ExecutionOrder::BoundSequential
            | ExecutionOrder::GroupTail
            | ExecutionOrder::GroupCapture
            | ExecutionOrder::ParallelGroupTail
            | ExecutionOrder::ParallelGroupCapture
            | ExecutionOrder::GroupDataflow
            | ExecutionOrder::ParallelGroupDataflow
            | ExecutionOrder::GroupConditional
            | ExecutionOrder::ParallelGroupConditional
    ) != plan.result_binding.is_some()
    {
        return Err(Fault {
            code: "LSV1409".into(),
            message: "invalid group-result binding or missing complete group journal".into(),
        });
    }
    if plan.order == ExecutionOrder::ResultChain && plan.branches != ["result", "successor"] {
        return Err(frozen_successor_fault());
    }
    if plan.order == ExecutionOrder::Dataflow
        && plan
            .branches
            .iter()
            .enumerate()
            .any(|(index, name)| *name != format!("step_{}", index + 1))
    {
        return Err(frozen_successor_fault());
    }
    let minimum = if matches!(
        plan.order,
        ExecutionOrder::Sequential
            | ExecutionOrder::BoundSequential
            | ExecutionOrder::GroupTail
            | ExecutionOrder::GroupCapture
            | ExecutionOrder::GroupDataflow
            | ExecutionOrder::GroupConditional
    ) {
        1
    } else {
        2
    };
    if !(minimum..=MAX_MERGE_BRANCHES).contains(&plan.branches.len()) {
        return Err(Fault {
            code: "LSV2401".to_string(),
            message: "structured merge plan is invalid or exceeds runtime bounds".to_string(),
        });
    }
    let mut unique = HashSet::with_capacity(plan.branches.len());
    if plan
        .branches
        .iter()
        .any(|branch| !frozen_valid_merge_branch_name(branch) || !unique.insert(branch.as_str()))
    {
        return Err(Fault {
            code: "LSV2401".to_string(),
            message: "structured merge plan is invalid or exceeds runtime bounds".to_string(),
        });
    }
    if let Some(binding) = &plan.result_binding {
        group_binding::plan_contract_tests::frozen_validate_group(binding, plan)?;
    }
    Ok(())
}

fn frozen_successor_fault() -> Fault {
    Fault {
        code: "LSV1407".into(),
        message: "invalid result-driven successor or missing original effect authority".into(),
    }
}

fn frozen_valid_merge_branch_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_MERGE_BRANCH_NAME_BYTES
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
}

fn orders() -> [ExecutionOrder; 14] {
    use ExecutionOrder::*;
    [
        Parallel,
        Sequential,
        ResultChain,
        Dataflow,
        BoundParallel,
        BoundSequential,
        GroupTail,
        GroupCapture,
        ParallelGroupTail,
        ParallelGroupCapture,
        GroupDataflow,
        ParallelGroupDataflow,
        GroupConditional,
        ParallelGroupConditional,
    ]
}

fn branch_sets() -> Vec<Vec<String>> {
    vec![
        vec![],
        vec!["a".into()],
        vec!["a".into(), "b".into()],
        vec!["result".into(), "successor".into()],
        vec!["step_1".into(), "step_2".into()],
        vec!["step_2".into(), "step_1".into()],
        vec!["a".into(), "a".into()],
        vec!["".into(), "b".into()],
        vec!["a b".into(), "b".into()],
        vec!["\u{e9}".into(), "b".into()],
        vec!["x".repeat(64), "b".into()],
        vec!["x".repeat(65), "b".into()],
        (0..63).map(|i| format!("branch_{i}")).collect(),
        (0..64).map(|i| format!("branch_{i}")).collect(),
        (0..65).map(|i| format!("branch_{i}")).collect(),
        vec!["a-A_0".into(), "a:a".into()],
    ]
}

fn group(parallel: bool) -> GroupResultBinding {
    let branches = ["a", "b"]
        .into_iter()
        .map(|name| leserpent_hir::HirBranch {
            name: name.into(),
            result_type: Type::UiFocus,
            effect: Effect::UiFocus {
                node_id: name.into(),
            },
        })
        .collect();
    GroupResultBinding {
        pending: if parallel {
            Effect::All { branches }
        } else {
            Effect::Sequence { steps: branches }
        },
        binding: Box::new(ResultBinding {
            name: "group".into(),
            locals: vec![],
            results: vec![],
            groups: vec![],
            body: leserpent_hir::computation::Computation::Literal {
                value: ScalarValue::None,
            },
        }),
        fuel_remaining: 100,
        successor_sequence: None,
        additional_successor_sequences: vec![],
    }
}

#[test]
fn merge_plan_preserves_frozen_wire_classification_and_layered_admission() {
    let mut malformed = group(false);
    malformed.binding.name = "not a local".into();
    let mut over_budget = group(false);
    over_budget.fuel_remaining = MAX_EXECUTION_FUEL + 1;
    let bindings = [
        None,
        Some(group(false)),
        Some(group(true)),
        Some(malformed),
        Some(over_budget),
    ];
    let mut comparisons = 0;
    let mut accepted_groups = 0;
    for order in orders() {
        let old_order: FrozenExecutionOrder =
            serde_json::from_value(serde_json::to_value(order).unwrap()).unwrap();
        assert_eq!(order.is_parallel(), old_order.is_parallel());
        assert_eq!(order.is_sequential(), old_order.is_sequential());
        assert_eq!(order.has_group_successor(), old_order.has_group_successor());
        assert_eq!(
            order.captures_group_successor(),
            old_order.captures_group_successor()
        );
        for branches in branch_sets() {
            for binding in &bindings {
                let plan = MergePlan {
                    branches: branches.clone(),
                    order,
                    result_binding: binding.clone().map(Box::new),
                };
                let frozen = FrozenMergePlan {
                    branches: branches.clone(),
                    order: old_order,
                    result_binding: plan.result_binding.clone(),
                };
                let before = serde_json::to_vec(&plan).unwrap();
                assert_eq!(before, serde_json::to_vec(&frozen).unwrap());
                assert_eq!(serde_json::from_slice::<MergePlan>(&before).unwrap(), plan);
                let expected = frozen_validate_merge_plan(&plan);
                accepted_groups += usize::from(expected.is_ok() && binding.is_some());
                assert_eq!(
                    validate_merge_plan(&plan),
                    expected,
                    "order={order:?}, branches={branches:?}"
                );
                assert_eq!(
                    leselang_command::reference_merge::validate_merge_plan(&plan),
                    expected,
                    "adapter order={order:?}, branches={branches:?}"
                );
                assert_eq!(serde_json::to_vec(&plan).unwrap(), before);
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 1120);
    assert!(accepted_groups > 0);
}

#[test]
fn merge_plan_constructor_keeps_legacy_faults_defaults_and_closed_decoding() {
    for branches in branch_sets() {
        let plan = MergePlan {
            branches: branches.clone(),
            order: ExecutionOrder::Parallel,
            result_binding: None,
        };
        let expected = frozen_validate_merge_plan(&plan).map(|()| plan);
        assert_eq!(MergePlan::new(branches), expected);
    }
    let plan = MergePlan::new(["a", "b"]).unwrap();
    assert_eq!(
        serde_json::to_string(&plan).unwrap(),
        r#"{"branches":["a","b"]}"#
    );
    for wire in [
        r#"{"branches":["a","b"],"order":"parallel","result_binding":null}"#,
        r#"{"branches":["a","b"]}"#,
    ] {
        assert_eq!(serde_json::from_str::<MergePlan>(wire).unwrap(), plan);
    }
    for wire in [
        r#"{"branches":[],"unknown":true}"#,
        r#"{"branches":[],"order":"unknown"}"#,
        r#"{"order":"parallel"}"#,
        r#"{"branches":[],"branches":[]}"#,
    ] {
        assert!(serde_json::from_str::<MergePlan>(wire).is_err());
        assert!(serde_json::from_str::<FrozenMergePlan>(wire).is_err());
    }
    // Saved data is still unchecked until explicit plan admission.
    assert!(serde_json::from_str::<MergePlan>(r#"{"branches":[]}"#).is_ok());
}

#[test]
fn static_merge_admission_neither_reserves_ids_nor_restores_or_finishes_live_groups() {
    let binding = group(true);
    let branches = binding.plan_branches().unwrap();
    let plan = MergePlan {
        branches: vec!["a".into(), "b".into()],
        order: ExecutionOrder::BoundParallel,
        result_binding: Some(Box::new(binding.clone())),
    };
    let mut vm = Vm::new(500);
    assert_eq!(
        leselang_command::reference_merge::validate_merge_plan(&plan),
        Ok(())
    );
    let mut malformed = binding.binding.clone();
    malformed.name = "x".repeat(leserpent_hir::MAX_BRANCH_NAME_BYTES + 1);
    let begin = |vm: &mut Vm, saved| {
        vm.start_group(
            branches,
            ExecutionOrder::Parallel,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            Some(Revision(7)),
            1000,
            None,
            100,
            Some(saved),
        )
    };
    assert!(matches!(begin(&mut vm, malformed), Step::Fault(fault) if fault.code == "LSV1405"));
    assert_eq!(vm.next_effect_id, 1);
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.completed_count(), 0);
    assert!(vm.claim_effect(1, 10).unwrap().is_none());

    let Step::Effects(batch) = begin(&mut vm, binding.binding.clone()) else {
        panic!()
    };
    assert_eq!(batch.merge_token.as_str(), "merge-1");
    assert_eq!(batch.branches[0].request.effect_id, "effect-2");
    assert_eq!(batch.branches[1].request.effect_id, "effect-3");
    let pending = vm.pending_continuations();
    assert_eq!(
        leselang_command::reference_merge::validate_merge_plan(&plan),
        Ok(())
    );
    assert_eq!(vm.pending_continuations(), pending);
    assert_eq!(vm.merge_result(&batch.merge_token).unwrap(), None);
    let mut receiver = Vm::new(500);
    assert_eq!(
        receiver.restore(pending[0].clone()).unwrap_err().code,
        "LSV1409"
    );
    assert_eq!(receiver.next_effect_id, 1);
    assert_eq!(receiver.pending_count(), 0);
    let reply = |name: &str| {
        EffectResult::Presentation(PresentationResult::Focus {
            node_id: name.into(),
        })
    };
    let left = &batch.branches[0].request.continuation;
    let right = &batch.branches[1].request.continuation;
    assert!(
        matches!(vm.resume(left, reply("a")), Step::Waiting(wait) if wait.completed_branches == 1)
    );
    let done = Step::Done(Value::Scalar {
        value: ScalarValue::None,
    });
    assert_eq!(vm.resume(right, reply("b")), done);
    assert_eq!(vm.resume(left, reply("ignored")), done);
    assert_eq!(vm.resume(right, reply("ignored")), done);
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.completed_count(), 2);
    assert!(vm.claim_effect(2, 10).unwrap().is_none());
}
