use leselang_command::reference_continuation::validate_continuation_size;
use leselang_command::reference_group_successor::{
    GroupSuccessorContext, ReservedEffectInput, build_group_successor,
};
use leselang_command::reference_merge::invalid_group_binding as invalid;
use leselang_command::reference_request::{DEFAULT_MAX_OUTPUT_ITEMS, validate_effect_request};
use leselang_command::{
    ContinuationImage, ContinuationToken, EffectRequest, GroupResultBinding, ProjectedBinding,
    ProjectedGroup, ProjectedGroupBinding, ProjectedResult, ResultBinding, Value,
};
use leselang_hir::{Effect, HirBranch, Type, computation::Computation, host_call::HostOperation};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::{Fault, Fuel, ScalarValue};
use leserpent_domain::{CommandId, IdempotencyKey};
use std::{cell::RefCell, rc::Rc};

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

fn fixture(parallel: bool, kind: usize) -> (GroupResultBinding, Option<Box<ResultBinding>>) {
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

// Test host, not an alternative production builder. VM parity is tested separately.
fn base(input: ReservedEffectInput<'_>) -> Result<EffectRequest, Fault> {
    let mut fuel = Fuel::new(input.fuel_remaining);
    fuel.charge(1).map_err(|_| Fault {
        code: "LSV1001".into(),
        message: "native charge refused".into(),
    })?;
    let image = ContinuationImage {
        schema_version: 1,
        token: ContinuationToken::from_wire(format!("continuation-{}", input.sequence)),
        program_counter: 1,
        expected_revision: input.expected_revision,
        result_type: input.result_type,
        pending_effect: input.effect.clone(),
        fuel_remaining: fuel.remaining(),
        deadline_ms: input.deadline_ms,
        deadline_at_ms: input.deadline_at_ms,
        max_output_items: DEFAULT_MAX_OUTPUT_ITEMS,
        result_binding: None,
        group_result: None,
    };
    let (required_capability, operation) = leselang_command::lower_effect_operation(
        input.effect,
        input.principal,
        input.capabilities,
        input.expected_revision,
        || {
            Some((
                CommandId::new("native-command").unwrap(),
                IdempotencyKey::new("native-key").unwrap(),
            ))
        },
    )
    .map_err(|_| Fault {
        code: "NATIVE".into(),
        message: "native lowering refused".into(),
    })?;
    let request = EffectRequest {
        effect_id: format!("effect-{}", input.sequence),
        required_capability,
        operation,
        budget: image.resource_budget(),
        continuation: image,
    };
    validate_effect_request(&request)?;
    Ok(request)
}

fn first(effect: &Effect) -> EffectRequest {
    base(ReservedEffectInput {
        effect,
        result_type: HostOperation::for_effect(effect).unwrap().result_type(),
        sequence: 7,
        principal: Principal::new("original-operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation", "runtime.read"]),
        expected_revision: Some(Revision(42)),
        deadline_ms: 30_000,
        deadline_at_ms: Some(31_000),
        fuel_remaining: 89,
    })
    .unwrap()
}

// Frozen group assembly before extraction, with its builder supplied by the test.
fn frozen(
    group: &GroupResultBinding,
    effect: &Effect,
    saved: Option<Box<ResultBinding>>,
    context: GroupSuccessorContext<'_>,
    build: impl FnOnce(ReservedEffectInput<'_>) -> Result<EffectRequest, Fault>,
) -> Result<EffectRequest, Fault> {
    let operation = HostOperation::for_effect(effect).ok_or_else(invalid)?;
    let (principal, capabilities) = context.first.operation.authority();
    let image = &context.first.continuation;
    let mut request = build(ReservedEffectInput {
        effect,
        result_type: operation.result_type(),
        sequence: group
            .reserved_sequences()
            .nth(context.position)
            .ok_or_else(invalid)?,
        principal: principal.clone(),
        capabilities: capabilities.clone(),
        expected_revision: image.expected_revision,
        deadline_ms: image.deadline_ms,
        deadline_at_ms: image.deadline_at_ms,
        fuel_remaining: context.fuel_remaining,
    })?;
    request.continuation.schema_version = group.continuation_schema();
    request.continuation.group_result = Some(context.owner.clone());
    request.continuation.result_binding = saved;
    request.continuation.max_output_items = image.max_output_items;
    request.budget.max_output_items = image.max_output_items;
    validate_effect_request(&request)?;
    validate_continuation_size(&request.continuation)?;
    Ok(request)
}

fn build_observed(
    input: ReservedEffectInput<'_>,
    trace: &RefCell<Vec<Vec<u8>>>,
    mode: usize,
) -> Result<EffectRequest, Fault> {
    trace.borrow_mut().push(
        serde_json::to_vec(&(
            input.effect,
            input.result_type,
            input.sequence,
            &input.principal,
            &input.capabilities,
            input.expected_revision,
            input.deadline_ms,
            input.deadline_at_ms,
            input.fuel_remaining,
        ))
        .unwrap(),
    );
    if mode == 1 {
        return Err(Fault {
            code: "NATIVE".into(),
            message: "original builder refusal".into(),
        });
    }
    let mut result = base(input)?;
    if mode == 2 {
        result.budget.fuel_remaining += 1;
    }
    Ok(result)
}

#[test]
fn group_successor_matches_frozen_assembly_faults_and_once_only_builder_inputs() {
    let mut comparisons = 0;
    for parallel in [false, true] {
        for kind in 1..=4 {
            for defect in 0..16 {
                for mode in 0..3 {
                    let (mut group, mut saved) = fixture(parallel, kind);
                    let mut effect = Effect::UiFocus {
                        node_id: "next".into(),
                    };
                    let mut first = first(&effect);
                    let mut owner = ContinuationToken::from_wire("merge-1".into());
                    let mut fuel = 23;
                    let mut position = 0;
                    match defect {
                        0 => {}
                        1 => position = 1,
                        2 => position = usize::MAX,
                        3 => {
                            group.successor_sequence = None;
                            group.additional_successor_sequences.clear();
                        }
                        4 => group.successor_sequence = Some(0),
                        5 => owner = ContinuationToken::from_wire("merge-0".into()),
                        6 => fuel = 0,
                        7 => fuel = 1,
                        8 => first.continuation.max_output_items = 0,
                        9 => first.continuation.max_output_items = DEFAULT_MAX_OUTPUT_ITEMS + 1,
                        10 => first.continuation.deadline_ms = 0,
                        11 => first.continuation.deadline_at_ms = Some(i64::MAX as u64 + 1),
                        12 => {
                            saved.get_or_insert_with(|| binding(scalar())).name = "bad name".into();
                        }
                        13 => {
                            saved.get_or_insert_with(|| binding(scalar())).body =
                                Computation::Literal {
                                    value: ScalarValue::String("x".repeat(70_000)),
                                };
                        }
                        14 => effect = Effect::Sequence { steps: vec![] },
                        15 => {
                            effect = Effect::All { branches: vec![] };
                            fuel = 0;
                            position = usize::MAX;
                        }
                        _ => unreachable!(),
                    }
                    let before = serde_json::to_vec(&(&group, &first, &effect)).unwrap();
                    let actual_trace = RefCell::new(vec![]);
                    let expected_trace = RefCell::new(vec![]);
                    let context = || GroupSuccessorContext {
                        owner: &owner,
                        first: &first,
                        fuel_remaining: fuel,
                        position,
                    };
                    let actual =
                        build_group_successor(&group, &effect, saved.clone(), context(), |input| {
                            build_observed(input, &actual_trace, mode)
                        });
                    let expected = frozen(&group, &effect, saved, context(), |input| {
                        build_observed(input, &expected_trace, mode)
                    });
                    assert_eq!(
                        actual, expected,
                        "parallel={parallel}, kind={kind}, defect={defect}, mode={mode}"
                    );
                    assert_eq!(
                        serde_json::to_vec(&actual).unwrap(),
                        serde_json::to_vec(&expected).unwrap()
                    );
                    assert_eq!(actual_trace, expected_trace);
                    assert!(actual_trace.borrow().len() <= 1);
                    if defect == 0 && mode == 0 {
                        assert!(actual.is_ok(), "{actual:?}");
                    }
                    assert_eq!(
                        serde_json::to_vec(&(&group, &first, &effect)).unwrap(),
                        before
                    );
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 384);
}

#[test]
fn group_successor_moves_saved_binding_and_preserves_original_context_without_fresh_fuel() {
    let (group, saved) = fixture(false, 2);
    let saved_pointer = &**saved.as_ref().unwrap() as *const ResultBinding;
    let effect = Effect::UiFocus {
        node_id: "next".into(),
    };
    let mut first = first(&effect);
    first.continuation.max_output_items = 17;
    let owner = ContinuationToken::from_wire("merge-1".into());
    let result = build_group_successor(
        &group,
        &effect,
        saved,
        GroupSuccessorContext {
            owner: &owner,
            first: &first,
            fuel_remaining: 23,
            position: 0,
        },
        |input| {
            assert!(std::ptr::eq(input.effect, &effect));
            assert_eq!(input.fuel_remaining, 23);
            assert_eq!(
                (&input.principal, &input.capabilities),
                first.operation.authority()
            );
            base(input)
        },
    )
    .unwrap();
    assert_eq!(
        &**result.continuation.result_binding.as_ref().unwrap() as *const ResultBinding,
        saved_pointer
    );
    assert_eq!(result.continuation.fuel_remaining, 22);
    assert_eq!(result.continuation.expected_revision, Some(Revision(42)));
    assert_eq!(result.continuation.deadline_at_ms, Some(31_000));
    assert_eq!(result.continuation.max_output_items, 17);
    assert_eq!(result.budget.max_output_items, 17);
    assert_eq!(result.continuation.group_result, Some(owner));
    assert_eq!(result.continuation.token.as_str(), "continuation-100");
}

struct LocalDrop(Rc<RefCell<Vec<&'static str>>>);
impl Drop for LocalDrop {
    fn drop(&mut self) {
        self.0.borrow_mut().push("drop");
    }
}

#[test]
fn group_successor_preflight_releases_uncalled_builder_and_unwind_never_retries() {
    for called in [false, true] {
        let trace = Rc::new(RefCell::new(vec![]));
        let guard = LocalDrop(trace.clone());
        let (group, saved) = fixture(false, 1);
        let effect = Effect::UiFocus {
            node_id: "next".into(),
        };
        let first = first(&effect);
        let owner = ContinuationToken::from_wire("merge-1".into());
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            build_group_successor(
                &group,
                &effect,
                saved,
                GroupSuccessorContext {
                    owner: &owner,
                    first: &first,
                    fuel_remaining: 23,
                    position: if called { 0 } else { usize::MAX },
                },
                move |_| {
                    guard.0.borrow_mut().push("call");
                    let _guard = guard;
                    panic!("original builder unwind");
                },
            )
        }));
        if called {
            assert!(result.is_err());
            assert_eq!(*trace.borrow(), ["call", "drop"]);
        } else {
            assert_eq!(result.unwrap().unwrap_err(), invalid());
            assert_eq!(*trace.borrow(), ["drop"]);
        }
    }
}
