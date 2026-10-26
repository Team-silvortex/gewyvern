use super::*;
use crate::{EffectOperation, MAX_EFFECT_SEQUENCE, ScalarValue, Vm};
use leselang_command::{ProjectedBinding, ProjectedGroup, ProjectedGroupBinding, ProjectedResult};
use leselang_hir::computation::Computation;

fn frozen_build(
    group: &GroupResultBinding,
    owner: &ContinuationToken,
    first: &EffectRequest,
    effect: &Effect,
    binding: Option<Box<ResultBinding>>,
    fuel: u64,
    position: usize,
) -> Result<EffectRequest, Fault> {
    let operation = HostOperation::for_effect(effect).ok_or_else(invalid)?;
    let (principal, capabilities) = crate::successor::authority(&first.operation);
    let image = &first.continuation;
    let mut request = Vm::build_effect_request(
        effect,
        operation.result_type(),
        group
            .reserved_sequences()
            .nth(position)
            .ok_or_else(invalid)?,
        principal.clone(),
        capabilities.clone(),
        image.expected_revision,
        image.deadline_ms,
        image.deadline_at_ms,
        fuel,
    )?;
    request.continuation.schema_version = group.continuation_schema();
    request.continuation.group_result = Some(owner.clone());
    request.continuation.result_binding = binding;
    request.continuation.max_output_items = image.max_output_items;
    request.budget.max_output_items = image.max_output_items;
    crate::validate_effect_request(&request)?;
    crate::validate_continuation_size(&request.continuation)?;
    Ok(request)
}

#[test]
fn group_successor_assembly_preserves_seventy_operations_schemas_original_fuel_and_faults() {
    let mut comparisons = 0;
    let mut successes = 0;
    for original in crate::continuation_contract_tests::requests() {
        for parallel in [false, true] {
            for kind in 1..=4 {
                for defect in 0..16 {
                    let (mut group, mut saved) = fixture(parallel, kind);
                    let mut first = original.clone();
                    let mut effect = original.continuation.pending_effect.clone();
                    let mut owner = ContinuationToken::from_wire("merge-1".into());
                    let mut fuel = 23;
                    let mut position = 0;
                    match defect {
                        0 => {}
                        1 => fuel = 0,
                        2 => fuel = 1,
                        3 => position = 1,
                        4 => group.successor_sequence = Some(0),
                        5 => group.successor_sequence = Some(MAX_EFFECT_SEQUENCE + 1),
                        6 | 7 => {
                            let (principal, capabilities) = match &mut first.operation {
                                EffectOperation::Query(value) => {
                                    (&mut value.principal, &mut value.capabilities)
                                }
                                EffectOperation::Command(value) => {
                                    (&mut value.principal, &mut value.capabilities)
                                }
                                EffectOperation::Presentation(value) => {
                                    (&mut value.principal, &mut value.capabilities)
                                }
                            };
                            if defect == 6 {
                                principal.id.clear();
                            } else {
                                *capabilities = crate::CapabilitySet::new([] as [&str; 0]);
                            }
                        }
                        8 => first.continuation.deadline_ms = 0,
                        9 => first.continuation.max_output_items = 0,
                        10 => first.continuation.deadline_at_ms = Some(i64::MAX as u64 + 1),
                        11 => {
                            saved.get_or_insert_with(|| binding(scalar())).name = "bad name".into();
                        }
                        12 => owner = ContinuationToken::from_wire("merge-0".into()),
                        13 => position = usize::MAX,
                        14 => effect = Effect::Sequence { steps: vec![] },
                        15 => {
                            effect = Effect::All { branches: vec![] };
                            fuel = 0;
                            position = usize::MAX;
                        }
                        _ => unreachable!(),
                    }
                    let before = serde_json::to_vec(&(&group, &owner, &first, &effect)).unwrap();
                    let actual = group.build_successor(
                        &owner,
                        &first,
                        &effect,
                        saved.clone(),
                        fuel,
                        position,
                    );
                    let expected =
                        frozen_build(&group, &owner, &first, &effect, saved, fuel, position);
                    assert_eq!(
                        actual, expected,
                        "parallel={parallel}, kind={kind}, defect={defect}, effect={effect:?}"
                    );
                    assert_eq!(
                        serde_json::to_vec(&actual).unwrap(),
                        serde_json::to_vec(&expected).unwrap()
                    );
                    assert_eq!(
                        serde_json::to_vec(&(&group, &owner, &first, &effect)).unwrap(),
                        before
                    );
                    if defect == 0 {
                        let next = actual.unwrap();
                        assert_eq!(next.continuation.fuel_remaining, 22);
                        assert_eq!(next.budget.fuel_remaining, 22);
                        assert_eq!(next.continuation.group_result, Some(owner));
                        assert_eq!(
                            next.continuation.schema_version,
                            group.continuation_schema()
                        );
                        assert_eq!(next.operation.authority(), first.operation.authority());
                        successes += 1;
                    }
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 8960);
    assert_eq!(successes, 560);
}

fn scalar() -> Computation {
    Computation::Literal {
        value: ScalarValue::None,
    }
}

fn binding(body: Computation) -> Box<ResultBinding> {
    Box::new(ResultBinding {
        name: "next".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body,
    })
}

pub(super) fn fixture(
    parallel: bool,
    kind: usize,
) -> (GroupResultBinding, Option<Box<ResultBinding>>) {
    let host = Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "next".into(),
        }),
    };
    let capture = Computation::Bind {
        name: "next".into(),
        value: Box::new(host.clone()),
        body: Box::new(scalar()),
    };
    let body = match kind {
        1 => host.clone(),
        2 => capture.clone(),
        3 => Computation::Bind {
            name: "first".into(),
            value: Box::new(host),
            body: Box::new(capture.clone()),
        },
        4 => Computation::Choose {
            when: Box::new(Computation::Literal {
                value: ScalarValue::Boolean(true),
            }),
            then: Box::new(capture),
            otherwise: Box::new(scalar()),
        },
        _ => unreachable!(),
    };
    let branches = vec![HirBranch {
        name: "a".into(),
        effect: Effect::UiFocus {
            node_id: "a".into(),
        },
        result_type: Type::UiFocus,
    }];
    let group = GroupResultBinding {
        pending: if parallel {
            Effect::All { branches }
        } else {
            Effect::Sequence { steps: branches }
        },
        binding: binding(body),
        fuel_remaining: 100,
        successor_sequence: Some(100),
        additional_successor_sequences: if kind == 3 { vec![101] } else { vec![] },
    };
    let saved = (kind != 1).then(|| {
        let mut saved = binding(scalar());
        saved.groups.push(ProjectedGroupBinding {
            name: "group".into(),
            group: ProjectedGroup {
                members: vec![ProjectedBinding {
                    name: "a".into(),
                    result: ProjectedResult::capture(
                        HostOperation::UiFocus,
                        &Value::UiFocus {
                            node_id: "a".into(),
                        },
                    )
                    .unwrap(),
                }],
            },
        });
        saved
    });
    (group, saved)
}
