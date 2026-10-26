use leselang_command::reference_group_binding::{validate_member, validate_saved_capture};
use leselang_command::reference_merge::invalid_group_binding as invalid;
use leselang_command::reference_request::{continuation_admission_fault, result_binding_limits};
use leselang_command::{
    CaptureReplayError, ContinuationAdmissionError, ContinuationImage, ContinuationToken,
    EffectOperation, EffectRequest, ExecutionOrder, GroupResultBinding, MAX_MERGE_BRANCHES,
    MergePlan, PresentationEnvelope, PresentationOperation, ProjectedBinding, ProjectedGroup,
    ProjectedGroupBinding, ProjectedGroupExt, ProjectedResult, ResultBinding, StructuredField,
    Value,
};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::{CaptureCorrespondenceLimits, Fault, ScalarValue};
use leserpent_hir::{
    Effect, HirBranch, Type, computation::Computation, host_call::HostOperation,
    result_field::ResultField,
};

fn focus(name: &str) -> Value {
    Value::UiFocus {
        node_id: name.into(),
    }
}
fn projection_fault() -> Fault {
    continuation_admission_fault(ContinuationAdmissionError::Binding)
}
fn valid_merge_branch_name(name: &str) -> bool {
    leselang_command::valid_merge_branch_name(name, leselang_command::MAX_MERGE_BRANCH_NAME_BYTES)
}

// Frozen VM member policy, substituting only its existing private forwarding helpers.
fn frozen_member(group: &GroupResultBinding, name: &str, value: &Value) -> Result<(), Fault> {
    if let Some(index) = (0..group.reserved_sequences().count()).find(|index| {
        group
            .plan_successor_name_at(*index, MAX_MERGE_BRANCHES)
            .is_ok_and(|candidate| candidate == name)
    }) {
        let accepted = if group.captures_successor() {
            group.preflighted_capture_accepts_type_at(index, value.result_type())
        } else {
            value.result_type()
                == leselang_command::validate_result_binding(
                    &group.binding,
                    &group.pending,
                    result_binding_limits(),
                    valid_merge_branch_name,
                )
                .map_err(|_| projection_fault())?
        };
        return if accepted { Ok(()) } else { Err(invalid()) };
    }
    let branch = group
        .plan_branches()
        .map_err(|_| invalid())?
        .iter()
        .find(|branch| branch.name == name)
        .ok_or_else(invalid)?;
    if value.result_type() != branch.result_type {
        return Err(invalid());
    }
    Ok(())
}

// Frozen VM capture gate over the unchanged lower-level correspondence engine.
fn frozen_capture(
    group: &GroupResultBinding,
    request: &EffectRequest,
    fields: &[StructuredField],
    operations: &[HostOperation],
) -> Result<(), Fault> {
    let branches = group.plan_branches().map_err(|_| invalid())?;
    if fields.len() < branches.len() || fields.len() != operations.len() {
        return Err(invalid());
    }
    let binding = request
        .continuation
        .result_binding
        .as_ref()
        .ok_or_else(invalid)?;
    leselang_command::validate_group_capture_correspondence(
        binding,
        branches,
        fields,
        operations,
        CaptureCorrespondenceLimits {
            max_saved_bindings: leserpent_hir::MAX_EFFECT_NESTING_DEPTH,
            max_original_rows: MAX_MERGE_BRANCHES,
        },
        result_binding_limits().groups,
        valid_merge_branch_name,
    )
    .map_err(|error| match error {
        CaptureReplayError::InvalidProjection => projection_fault(),
        CaptureReplayError::Mismatch => invalid(),
    })
}

#[test]
fn member_policy_preserves_frozen_label_precedence_types_and_lazy_binding_faults() {
    let mut comparisons = 0;
    for parallel in [false, true] {
        for kind in 0..5 {
            for defect in 0..8 {
                let (mut binding, _) = group_plan_fixture(parallel, kind);
                match defect {
                    0 => {}
                    1 => match &mut binding.pending {
                        Effect::Sequence { steps } | Effect::All { branches: steps } => {
                            steps[0].name = "successor".into()
                        }
                        _ => unreachable!(),
                    },
                    2 => {
                        binding.successor_sequence = None;
                        binding.additional_successor_sequences.clear();
                    }
                    3 => binding.additional_successor_sequences = vec![101, 102],
                    4 => binding.binding.name = "bad name".into(),
                    5 => {
                        binding.pending = Effect::UiFocus {
                            node_id: "a".into(),
                        }
                    }
                    6 => match &mut binding.pending {
                        Effect::Sequence { steps } | Effect::All { branches: steps } => {
                            let mut duplicate = steps[0].clone();
                            duplicate.result_type =
                                Type::Scalar(leselang_runtime_core::ScalarType::Boolean);
                            steps.push(duplicate);
                        }
                        _ => unreachable!(),
                    },
                    7 => {
                        binding.pending = Effect::Sequence {
                            steps: (0..64)
                                .map(|i| HirBranch {
                                    name: if i == 0 {
                                        "successor".into()
                                    } else {
                                        format!("successor_{i}")
                                    },
                                    result_type: Type::UiFocus,
                                    effect: Effect::UiFocus {
                                        node_id: "a".into(),
                                    },
                                })
                                .collect(),
                        }
                    }
                    _ => unreachable!(),
                }
                let names = [
                    "a".into(),
                    "b".into(),
                    binding
                        .plan_successor_name_at(0, 64)
                        .unwrap_or_else(|_| "successor".into()),
                    binding
                        .plan_successor_name_at(1, 64)
                        .unwrap_or_else(|_| "successor_1".into()),
                    "unknown".into(),
                    String::new(),
                    "successor_63".into(),
                ];
                let values = [
                    focus("a"),
                    focus("bad raw node"),
                    Value::Scalar {
                        value: ScalarValue::None,
                    },
                    Value::Scalar {
                        value: ScalarValue::Boolean(true),
                    },
                    Value::Scalar {
                        value: ScalarValue::Integer(7),
                    },
                    Value::Structured { fields: vec![] },
                ];
                for name in names {
                    for value in &values {
                        let before = serde_json::to_vec(&(&binding, &name, value)).unwrap();
                        let actual = validate_member(&binding, &name, value);
                        assert_eq!(
                            actual,
                            frozen_member(&binding, &name, value),
                            "parallel={parallel} kind={kind} defect={defect} name={name}"
                        );
                        if defect == 0 && name == "a" && value == &focus("a") {
                            assert_eq!(actual, Ok(()));
                        }
                        assert_eq!(
                            serde_json::to_vec(&(&binding, &name, value)).unwrap(),
                            before
                        );
                        comparisons += 1;
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 3360);
}

fn capture_fixture(
    parallel: bool,
    version: usize,
) -> (
    GroupResultBinding,
    EffectRequest,
    Vec<StructuredField>,
    Vec<HostOperation>,
) {
    let (binding, _) = group_plan_fixture(parallel, 2);
    let fields = ["a", "b"]
        .into_iter()
        .map(|name| StructuredField {
            name: name.into(),
            value: Box::new(focus(name)),
        })
        .collect::<Vec<_>>();
    let mut request = request();
    let mut saved = ResultBinding {
        name: "next".into(),
        locals: vec![],
        results: vec![ProjectedBinding {
            name: "alias".into(),
            result: ProjectedResult::capture(HostOperation::UiFocus, &focus("a")).unwrap(),
        }],
        groups: vec![ProjectedGroupBinding {
            name: "group".into(),
            group: ProjectedGroup::capture(
                binding.plan_branches().unwrap(),
                &Value::Structured {
                    fields: fields.clone(),
                },
                result_binding_limits().groups,
                valid_merge_branch_name,
            )
            .unwrap(),
        }],
        body: Computation::Literal {
            value: ScalarValue::None,
        },
    };
    let vocabularies: [&[ResultField]; 5] = [
        &ResultField::V1,
        &ResultField::V2,
        &ResultField::V3,
        &ResultField::V4,
        &ResultField::V5,
    ];
    for result in saved
        .results
        .iter_mut()
        .chain(saved.groups[0].group.members.iter_mut())
    {
        result.result.projection_version = version as u32 + 1;
        result
            .result
            .fields
            .retain(|field| vocabularies[version].contains(&field.field));
    }
    request.continuation.result_binding = Some(Box::new(saved));
    (binding, request, fields, vec![HostOperation::UiFocus; 2])
}

#[test]
fn capture_policy_preserves_frozen_versions_shape_and_competing_fault_precedence() {
    let mut comparisons = 0;
    for parallel in [false, true] {
        for version in 0..5 {
            for defect in 0..16 {
                for guard in 0..4 {
                    let (mut group, mut request, mut fields, mut operations) =
                        capture_fixture(parallel, version);
                    let saved = request.continuation.result_binding.as_mut().unwrap();
                    match defect {
                        0 => {}
                        1 => saved.groups[0].group.members[0].name = "changed".into(),
                        2 => saved.groups[0].group.members[0].result.projection_version = 99,
                        3 => saved.results[0].result.projection_version = 99,
                        4 => {
                            saved.results[0].result = ProjectedResult::capture(
                                HostOperation::UiFocus,
                                &focus("unmatched"),
                            )
                            .unwrap()
                        }
                        5 => fields.swap(0, 1),
                        6 => operations[0] = HostOperation::UiNavigateFocus,
                        7 => *fields[0].value = focus("changed"),
                        8 => {
                            fields.push(StructuredField {
                                name: "late".into(),
                                value: Box::new(Value::Scalar {
                                    value: ScalarValue::None,
                                }),
                            });
                            operations.push(HostOperation::UiFocus);
                        }
                        9 => saved.groups.push(saved.groups[0].clone()),
                        10 => saved.results.push(saved.results[0].clone()),
                        11 => {
                            saved.groups[0].group.members[0].name = "changed".into();
                            saved.results[0].result.projection_version = 99;
                        }
                        12 => {
                            saved.results.push(saved.results[0].clone());
                            saved.results[1].result.projection_version = 99;
                        }
                        13 => {
                            saved.body = Computation::Local {
                                name: "not_evaluated_or_typed_here".into(),
                            }
                        }
                        14 => {
                            operations.pop();
                        }
                        15 => {
                            fields.pop();
                            operations.pop();
                        }
                        _ => unreachable!(),
                    }
                    match guard {
                        0 => {}
                        1 => fields.clear(),
                        2 => request.continuation.result_binding = None,
                        3 => {
                            group.pending = Effect::UiFocus {
                                node_id: "a".into(),
                            }
                        }
                        _ => unreachable!(),
                    }
                    let before =
                        serde_json::to_vec(&(&group, &request, &fields, &operations)).unwrap();
                    let actual = validate_saved_capture(&group, &request, &fields, &operations);
                    assert_eq!(
                        actual,
                        frozen_capture(&group, &request, &fields, &operations),
                        "parallel={parallel} version={version} defect={defect} guard={guard}"
                    );
                    if defect == 0 && guard == 0 {
                        assert_eq!(actual, Ok(()));
                    }
                    if guard != 0 {
                        assert_eq!(actual.unwrap_err().code, "LSV1409");
                    }
                    assert_eq!(
                        serde_json::to_vec(&(&group, &request, &fields, &operations)).unwrap(),
                        before
                    );
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 640);
}

#[test]
fn capture_limits_are_inclusive_and_correspondence_never_certifies_raw_values_or_requests() {
    for collection in ["groups", "results", "rows"] {
        let (group, mut request, mut fields, mut operations) = capture_fixture(false, 4);
        let saved = request.continuation.result_binding.as_mut().unwrap();
        let limit = if collection == "rows" {
            MAX_MERGE_BRANCHES
        } else {
            leserpent_hir::MAX_EFFECT_NESTING_DEPTH
        };
        match collection {
            "groups" => saved
                .groups
                .resize(limit - saved.results.len(), saved.groups[0].clone()),
            "results" => saved
                .results
                .resize(limit - saved.groups.len(), saved.results[0].clone()),
            _ => {
                fields.resize(limit, fields[0].clone());
                operations.resize(limit, HostOperation::UiFocus);
            }
        }
        assert_eq!(
            validate_saved_capture(&group, &request, &fields, &operations),
            Ok(())
        );
        let saved = request.continuation.result_binding.as_mut().unwrap();
        match collection {
            "groups" => saved.groups.push(saved.groups[0].clone()),
            "results" => saved.results.push(saved.results[0].clone()),
            _ => {
                fields.push(fields[0].clone());
                operations.push(HostOperation::UiFocus);
            }
        }
        assert_eq!(
            validate_saved_capture(&group, &request, &fields, &operations)
                .unwrap_err()
                .code,
            "LSV1409"
        );
    }
    let (group, mut request, fields, operations) = capture_fixture(false, 4);
    request.effect_id = "forged".into();
    assert_eq!(
        validate_saved_capture(&group, &request, &fields, &operations),
        Ok(())
    );
    assert!(leselang_command::reference_request::validate_effect_request(&request).is_err());
    let invalid_value = focus("invalid node id");
    assert_eq!(validate_member(&group, "a", &invalid_value), Ok(()));
    assert!(leselang_command::reference_value::validate_value(&invalid_value, 0).is_err());
}

fn request() -> EffectRequest {
    let continuation = ContinuationImage {
        schema_version: 1,
        token: ContinuationToken::from_wire("continuation-7".into()),
        program_counter: 1,
        expected_revision: None,
        result_type: Type::UiFocus,
        pending_effect: Effect::UiFocus {
            node_id: "a".into(),
        },
        fuel_remaining: 99,
        deadline_ms: 30_000,
        deadline_at_ms: Some(40_000),
        max_output_items: 10_000,
        result_binding: None,
        group_result: None,
    };
    EffectRequest {
        effect_id: "effect-7".into(),
        required_capability: "ui.presentation".into(),
        operation: EffectOperation::Presentation(PresentationEnvelope {
            schema_version: 1,
            principal: Principal::new("operator").unwrap(),
            capabilities: CapabilitySet::new(["ui.presentation"]),
            operation: PresentationOperation::Focus {
                node_id: "a".into(),
            },
        }),
        budget: continuation.resource_budget(),
        continuation,
    }
}

fn group_plan_fixture(parallel: bool, kind: usize) -> (GroupResultBinding, MergePlan) {
    let pure = Computation::Literal {
        value: ScalarValue::None,
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
                    value: ScalarValue::Boolean(false),
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
