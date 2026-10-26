use super::*;
use crate::ExecutionOrder;
use leserpent_hir::computation::Computation;

fn invalid() -> Fault {
    Fault {
        code: "LSV1409".into(),
        message: "invalid group-result binding or missing complete group journal".into(),
    }
}

pub(crate) fn frozen_validate_group(
    binding: &GroupResultBinding,
    plan: &MergePlan,
) -> Result<Type, Fault> {
    FrozenGroup(binding).validate(plan)
}

fn fixture(parallel: bool, kind: usize) -> (GroupResultBinding, MergePlan) {
    let pure = Computation::Literal {
        value: crate::ScalarValue::None,
    };
    let host = Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "next".into(),
        }),
    };
    let capture = Computation::Bind {
        name: "next".into(),
        value: Box::new(host.clone()),
        body: Box::new(pure.clone()),
    };
    let (body, order) = match (parallel, kind) {
        (p, 0) => (
            pure.clone(),
            if p {
                ExecutionOrder::BoundParallel
            } else {
                ExecutionOrder::BoundSequential
            },
        ),
        (p, 1) => (
            host.clone(),
            if p {
                ExecutionOrder::ParallelGroupTail
            } else {
                ExecutionOrder::GroupTail
            },
        ),
        (p, 2) => (
            capture.clone(),
            if p {
                ExecutionOrder::ParallelGroupCapture
            } else {
                ExecutionOrder::GroupCapture
            },
        ),
        (p, 3) => (
            Computation::Bind {
                name: "first".into(),
                value: Box::new(host),
                body: Box::new(capture.clone()),
            },
            if p {
                ExecutionOrder::ParallelGroupDataflow
            } else {
                ExecutionOrder::GroupDataflow
            },
        ),
        (p, 4) => (
            Computation::Choose {
                when: Box::new(Computation::Literal {
                    value: crate::ScalarValue::Boolean(false),
                }),
                then: Box::new(capture),
                otherwise: Box::new(pure),
            },
            if p {
                ExecutionOrder::ParallelGroupConditional
            } else {
                ExecutionOrder::GroupConditional
            },
        ),
        _ => unreachable!(),
    };
    let branches = ["a", "b"]
        .into_iter()
        .map(|name| HirBranch {
            name: name.into(),
            result_type: Type::UiFocus,
            effect: Effect::UiFocus {
                node_id: name.into(),
            },
        })
        .collect();
    let binding = GroupResultBinding {
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
            body,
        }),
        fuel_remaining: 100,
        successor_sequence: (kind != 0).then_some(100),
        additional_successor_sequences: if kind == 3 { vec![101] } else { vec![] },
    };
    let plan = MergePlan {
        branches: vec!["a".into(), "b".into()],
        order,
        result_binding: Some(Box::new(binding.clone())),
    };
    (binding, plan)
}

fn mutate(binding: &mut GroupResultBinding, plan: &mut MergePlan, defect: usize) {
    match defect {
        0 => {}
        1 => {
            plan.branches.pop();
        }
        2 => plan.branches.push("wrong-successor".into()),
        3 => plan.branches.reverse(),
        4 => binding.fuel_remaining = crate::MAX_EXECUTION_FUEL,
        5 => binding.fuel_remaining = crate::MAX_EXECUTION_FUEL + 1,
        6 => binding.fuel_remaining = 0,
        7 => binding.successor_sequence = None,
        8 => binding.successor_sequence = Some(0),
        9 => binding.successor_sequence = Some(crate::MAX_EFFECT_SEQUENCE),
        10 => binding.successor_sequence = Some(crate::MAX_EFFECT_SEQUENCE + 1),
        11 => binding.additional_successor_sequences = vec![100],
        12 => binding.additional_successor_sequences = vec![99],
        13 => binding.additional_successor_sequences = vec![101, 102],
        14 => {
            binding.pending = Effect::UiFocus {
                node_id: "not-a-group".into(),
            }
        }
        15 => binding.pending = Effect::All { branches: vec![] },
        16 | 17 | 22 => {
            let branches = match &mut binding.pending {
                Effect::All { branches } | Effect::Sequence { steps: branches } => branches,
                _ => unreachable!(),
            };
            match defect {
                16 => branches[0].result_type = Type::UiAssertVisible,
                17 => branches[0].effect = Effect::Sequence { steps: vec![] },
                22 => {
                    branches[0].name = "successor".into();
                    plan.branches[0] = "successor".into();
                    plan.branches.push("successor_1".into());
                }
                _ => unreachable!(),
            }
        }
        18 => binding.binding.name = "invalid name".into(),
        19 => {
            binding.binding.body = Computation::Local {
                name: "missing".into(),
            }
        }
        20 | 21 => {
            let saved = leselang_command::ProjectedBinding {
                name: "saved".into(),
                result: leselang_command::ProjectedResult::capture(
                    HostOperation::UiFocus,
                    &Value::UiFocus {
                        node_id: "saved".into(),
                    },
                )
                .unwrap(),
            };
            if defect == 20 {
                binding.binding.results.push(saved);
            } else {
                binding
                    .binding
                    .groups
                    .push(leselang_command::ProjectedGroupBinding {
                        name: "saved_group".into(),
                        group: leselang_command::ProjectedGroup {
                            members: vec![saved],
                        },
                    });
            }
        }
        23 => plan
            .branches
            .extend(["successor".into(), "successor_1".into()]),
        _ => unreachable!(),
    }
    plan.result_binding = Some(Box::new(binding.clone()));
}

#[test]
fn group_plan_keeps_frozen_full_admission_across_orders_flows_and_corruptions() {
    use ExecutionOrder::*;
    let orders = [
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
    ];
    let mut comparisons = 0;
    for parallel in [false, true] {
        for kind in 0..5 {
            let (binding, plan) = fixture(parallel, kind);
            assert!(
                binding.validate(&plan).is_ok(),
                "valid fixture {parallel}/{kind}"
            );
            for order in orders {
                for defect in 0..24 {
                    let mut binding = binding.clone();
                    let mut plan = plan.clone();
                    plan.order = order;
                    mutate(&mut binding, &mut plan, defect);
                    let before = serde_json::to_vec(&plan).unwrap();
                    assert_eq!(
                        binding.validate(&plan),
                        FrozenGroup(&binding).validate(&plan),
                        "parallel={parallel} kind={kind} order={order:?} defect={defect}"
                    );
                    assert_eq!(
                        leselang_command::reference_merge::validate_group_binding(&binding, &plan),
                        FrozenGroup(&binding).validate(&plan),
                        "adapter parallel={parallel} kind={kind} order={order:?} defect={defect}"
                    );
                    assert_eq!(serde_json::to_vec(&plan).unwrap(), before);
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 3360);
}

#[test]
fn group_plan_keeps_structure_then_correspondence_then_size_then_type_faults() {
    let (mut binding, mut plan) = fixture(false, 0);
    // A valid lexical shape with an unresolved name reaches the final type gate.
    binding.binding.body = Computation::Local {
        name: "missing".into(),
    };
    let mut large = binding.clone();
    if let Effect::Sequence { steps } = &mut large.pending {
        steps[0].effect = Effect::UiFocus {
            node_id: "x".repeat(crate::MAX_CONTINUATION_BYTES),
        };
    }
    plan.result_binding = Some(Box::new(large));
    let check = |binding: &GroupResultBinding, plan: &MergePlan, code: &str| {
        let expected = FrozenGroup(binding).validate(plan);
        assert_eq!(expected.as_ref().unwrap_err().code, code);
        assert_eq!(binding.validate(plan), expected);
        assert_eq!(
            leselang_command::reference_merge::validate_group_binding(binding, plan),
            expected
        );
    };
    check(&binding, &plan, "LSV3002");
    binding.fuel_remaining = crate::MAX_EXECUTION_FUEL + 1;
    check(&binding, &plan, "LSV1409");
    binding.binding.name = "x".repeat(leserpent_hir::MAX_BRANCH_NAME_BYTES + 1);
    check(&binding, &plan, "LSV1405");
    binding.binding.name = "group".into();
    binding.fuel_remaining = 100;
    plan.result_binding = Some(Box::new(binding.clone()));
    check(&binding, &plan, "LSV1405");
}

#[test]
fn group_plan_successor_names_preserve_collisions_and_fixed_search_exhaustion() {
    let (mut binding, _) = fixture(false, 3);
    for used in 0..=crate::MAX_MERGE_BRANCHES {
        let branches = (0..used)
            .map(|i| HirBranch {
                name: if i == 0 {
                    "successor".into()
                } else {
                    format!("successor_{i}")
                },
                result_type: Type::UiFocus,
                effect: Effect::UiFocus {
                    node_id: "node".into(),
                },
            })
            .collect();
        binding.pending = Effect::Sequence { steps: branches };
        for position in [0, 1, 63, 64, usize::MAX] {
            assert_eq!(
                binding.successor_name_at(position),
                FrozenGroup(&binding).successor_name_at(position)
            );
        }
    }
}

// Frozen before the group-plan correspondence move; never delegates to the new policy.
struct FrozenGroup<'a>(&'a GroupResultBinding);

impl std::ops::Deref for FrozenGroup<'_> {
    type Target = GroupResultBinding;
    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl FrozenGroup<'_> {
    fn branches(&self) -> Result<&[HirBranch], Fault> {
        match &self.pending {
            Effect::All { branches } | Effect::Sequence { steps: branches } => Ok(branches),
            _ => Err(invalid()),
        }
    }

    fn successor_name_at(&self, position: usize) -> Result<String, Fault> {
        let branches = self.branches()?;
        let mut available = 0;
        for index in 0..crate::MAX_MERGE_BRANCHES {
            let name = if index == 0 {
                "successor".into()
            } else {
                format!("successor_{index}")
            };
            if !branches.iter().any(|branch| branch.name == name) {
                if available == position {
                    return Ok(name);
                }
                available += 1;
            }
        }
        Err(invalid())
    }

    fn validate(&self, plan: &MergePlan) -> Result<Type, Fault> {
        self.binding.validate_structure()?;
        let branches = self.branches()?;
        let tail = self.successor_sequence.is_some();
        let bound = self.binding.body.atomic_flow_bound().ok_or_else(invalid)?;
        if plan.branches.len() < branches.len()
            || plan.branches.len() > branches.len().saturating_add(bound)
            || !self.binding.results.is_empty()
            || !self.binding.groups.is_empty()
            || if tail {
                if plan.order.captures_group_successor() {
                    !self.captures_successor()
                } else {
                    !self.binding.body.is_atomic_tail()
                }
            } else {
                !self.binding.body.is_pure()
            }
            || self.fuel_remaining > crate::MAX_EXECUTION_FUEL
            || !matches!(
                (&self.pending, plan.order),
                (Effect::All { .. }, ExecutionOrder::BoundParallel)
                    | (Effect::Sequence { .. }, ExecutionOrder::BoundSequential)
                    | (Effect::Sequence { .. }, ExecutionOrder::GroupTail)
                    | (Effect::Sequence { .. }, ExecutionOrder::GroupCapture)
                    | (Effect::All { .. }, ExecutionOrder::ParallelGroupTail)
                    | (Effect::All { .. }, ExecutionOrder::ParallelGroupCapture)
                    | (Effect::Sequence { .. }, ExecutionOrder::GroupDataflow)
                    | (Effect::All { .. }, ExecutionOrder::ParallelGroupDataflow)
                    | (Effect::Sequence { .. }, ExecutionOrder::GroupConditional)
                    | (Effect::All { .. }, ExecutionOrder::ParallelGroupConditional)
            )
            || matches!(
                plan.order,
                ExecutionOrder::GroupDataflow | ExecutionOrder::ParallelGroupDataflow
            ) != self.is_dataflow()
            || matches!(
                plan.order,
                ExecutionOrder::GroupConditional | ExecutionOrder::ParallelGroupConditional
            ) != self.is_conditional()
            || plan.order.has_group_successor() != tail
            || self.reserved_sequences().count() != bound
            || self
                .reserved_sequences()
                .any(|sequence| sequence == 0 || sequence > crate::MAX_EFFECT_SEQUENCE)
            || self
                .reserved_sequences()
                .zip(self.reserved_sequences().skip(1))
                .any(|(left, right)| left >= right)
            || branches.len().saturating_add(bound) > crate::MAX_MERGE_BRANCHES
            || plan
                .branches
                .iter()
                .skip(branches.len())
                .enumerate()
                .any(|(index, name)| self.successor_name_at(index).as_ref() != Ok(name))
            || branches.iter().zip(&plan.branches).any(|(branch, name)| {
                branch.name != *name
                    || HostOperation::for_effect(&branch.effect)
                        .is_none_or(|operation| operation.result_type() != branch.result_type)
            })
        {
            return Err(invalid());
        }
        crate::validate_json_size_capped(plan, crate::MAX_CONTINUATION_BYTES, "group-result plan")?;
        let ty = self.binding.validate(&self.pending)?;
        if (!tail || self.captures_successor()) && !matches!(ty, Type::Scalar(_)) {
            return Err(invalid());
        }
        Ok(ty)
    }
}
