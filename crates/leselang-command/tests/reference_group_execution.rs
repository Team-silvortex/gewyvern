use leselang_command::execution_outcome::ComputationOutcome as Outcome;
use leselang_command::reference_group_binding::validate_saved_capture;
use leselang_command::reference_group_execution::{
    CaptureProgress, ReferenceGroupExecutionHost, advance_capture, finish, prepare_initial,
};
use leselang_command::reference_group_successor::{GroupSuccessorContext, ReservedEffectInput};
use leselang_command::reference_merge::invalid_group_binding as invalid;
use leselang_command::reference_request::{DEFAULT_MAX_OUTPUT_ITEMS, validate_effect_request};
use leselang_command::{
    ContinuationImage, ContinuationToken, EffectRequest, GroupResultBinding, ProjectedBinding,
    ProjectedGroup, ProjectedGroupBinding, ProjectedResult, ResultBinding, Step, StructuredField,
    Value,
};
use leselang_hir::{Effect, HirBranch, Type, computation::Computation, host_call::HostOperation};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::{Fault, Fuel, ScalarValue};
use leserpent_domain::{CommandId, IdempotencyKey};
use std::{borrow::Cow, rc::Rc};

fn fault(message: &str) -> Fault {
    Fault {
        code: "NATIVE".into(),
        message: message.into(),
    }
}

struct Host {
    events: Vec<String>,
    mode: usize,
    fail: Option<&'static str>,
    panic_at: Option<&'static str>,
    output: EffectRequest,
    effect_pointer: usize,
    binding_pointer: usize,
    local: Rc<()>,
}
impl Host {
    fn new(output: &EffectRequest, mode: usize, fail: Option<&'static str>) -> Self {
        Self {
            events: vec![],
            mode,
            fail,
            panic_at: None,
            output: output.clone(),
            effect_pointer: 0,
            binding_pointer: 0,
            local: Rc::new(()),
        }
    }
    fn hit(&mut self, phase: &str, detail: impl std::fmt::Display) -> Result<(), Fault> {
        assert_eq!(Rc::strong_count(&self.local), 1);
        self.events.push(format!("{phase}:{detail}"));
        assert_ne!(self.panic_at, Some(phase), "native hook unwind");
        if self.fail == Some(phase) {
            Err(fault(phase))
        } else {
            Ok(())
        }
    }
    fn evaluate<'a>(
        &mut self,
        phase: &str,
        binding: &'a ResultBinding,
        value: &'a Value,
        fuel: &mut Fuel,
    ) -> Result<Outcome<'a>, Fault> {
        self.hit(
            phase,
            format!(
                "{}:{}",
                fuel.remaining(),
                serde_json::to_string(value).unwrap()
            ),
        )?;
        fuel.charge(2).map_err(|_| fault("original debit"))?;
        let effect = match &binding.body {
            Computation::Host { effect } => Cow::Borrowed(&**effect),
            _ => Cow::Owned(Effect::UiFocus {
                node_id: "native-next".into(),
            }),
        };
        self.effect_pointer = &*effect as *const Effect as usize;
        match self.mode {
            0 => Ok(Outcome::Scalar(ScalarValue::Integer(7))),
            1 => Ok(Outcome::Result),
            2 => Ok(Outcome::Host(effect)),
            3 => {
                let saved = crate::binding(scalar());
                self.binding_pointer = &*saved as *const ResultBinding as usize;
                Ok(Outcome::BoundHost {
                    effect,
                    binding: saved,
                })
            }
            _ => Err(fault("original evaluation")),
        }
    }
}
impl ReferenceGroupExecutionHost for Host {
    fn resume_group<'a>(
        &mut self,
        binding: &'a ResultBinding,
        value: &'a Value,
        branches: &'a [HirBranch],
        fuel: &mut Fuel,
    ) -> Result<Outcome<'a>, Fault> {
        assert_eq!(branches.len(), 1);
        self.evaluate("group", binding, value, fuel)
    }
    fn resume_capture<'a>(
        &mut self,
        binding: &'a ResultBinding,
        value: &'a Value,
        operation: HostOperation,
        fuel: &mut Fuel,
    ) -> Result<Outcome<'a>, Fault> {
        assert_eq!(operation, HostOperation::UiFocus);
        self.evaluate("capture", binding, value, fuel)
    }
    fn finish_group(
        &mut self,
        _binding: &ResultBinding,
        value: &Value,
        branches: &[HirBranch],
        fuel: &mut Fuel,
    ) -> Result<ScalarValue, Fault> {
        assert_eq!(branches.len(), 1);
        self.hit(
            "finish",
            format!(
                "{}:{}",
                fuel.remaining(),
                serde_json::to_string(value).unwrap()
            ),
        )?;
        fuel.charge(2).map_err(|_| fault("original debit"))?;
        if self.mode == 4 {
            Err(fault("original evaluation"))
        } else {
            Ok(ScalarValue::Integer(7))
        }
    }
    fn build_successor(
        &mut self,
        _group: &GroupResultBinding,
        effect: &Effect,
        binding: Option<Box<ResultBinding>>,
        context: GroupSuccessorContext<'_>,
    ) -> Result<EffectRequest, Fault> {
        self.hit(
            "build",
            format!(
                "{}:{}:{}:{}:{}",
                context.owner.as_str(),
                context.first.effect_id,
                context.fuel_remaining,
                context.position,
                serde_json::to_string(effect).unwrap()
            ),
        )?;
        let mut output = self.output.clone();
        output.continuation.fuel_remaining = context.fuel_remaining;
        output.continuation.result_binding = binding;
        Ok(output)
    }
    fn validate_successor(
        &mut self,
        parent: &EffectRequest,
        result: &Step,
        next: &EffectRequest,
    ) -> Result<(), Fault> {
        self.hit(
            "pair",
            format!(
                "{}:{}:{}",
                parent.effect_id,
                serde_json::to_string(result).unwrap(),
                next.effect_id
            ),
        )
    }
}

fn comparable(result: Result<CaptureProgress, Fault>) -> Result<Step, Fault> {
    result.map(|progress| match progress {
        CaptureProgress::Done(value) => Step::Done(Value::Scalar { value }),
        CaptureProgress::Pending(request) => Step::Effect(request),
    })
}

fn capture_value() -> Value {
    Value::Structured {
        fields: vec![
            StructuredField {
                name: "a".into(),
                value: Box::new(Value::UiFocus {
                    node_id: "a".into(),
                }),
            },
            StructuredField {
                name: "successor".into(),
                value: Box::new(Value::UiFocus {
                    node_id: "next".into(),
                }),
            },
        ],
    }
}

#[test]
fn group_coordination_matches_frozen_outcomes_faults_and_original_hook_order() {
    let effect = Effect::UiFocus {
        node_id: "next".into(),
    };
    let first = first(&effect);
    let owner = ContinuationToken::from_wire("merge-1".into());
    let mut comparisons = 0;
    for parallel in [false, true] {
        for kind in 1..=4 {
            for remaining in [0, 1, 2, 30] {
                for mode in 0..5 {
                    for fail in [None, Some("build"), Some("pair")] {
                        for defect in 0..4 {
                            let (mut group, saved) = fixture(parallel, kind);
                            group.fuel_remaining = remaining;
                            let mut last = first.clone();
                            last.continuation.result_binding = saved;
                            last.continuation.fuel_remaining = remaining;
                            let mut requests = vec![first.clone(), last];
                            let mut value = capture_value();
                            match defect {
                                0 => {}
                                1 => group.pending = effect.clone(),
                                2 => value = Value::Structured { fields: vec![] },
                                3 => requests[1].continuation.result_binding = None,
                                _ => unreachable!(),
                            }
                            let before = serde_json::to_vec(&(&group, &requests, &value)).unwrap();
                            for stage in 0..3 {
                                let mut actual_host = Host::new(&first, mode, fail);
                                let mut expected_host = Host::new(&first, mode, fail);
                                let actual_host_dyn: &mut dyn ReferenceGroupExecutionHost =
                                    &mut actual_host;
                                let actual = match stage {
                                    0 => comparable(prepare_initial(
                                        &group,
                                        &owner,
                                        &first,
                                        &value,
                                        actual_host_dyn,
                                    )),
                                    1 => comparable(advance_capture(
                                        &group,
                                        &owner,
                                        &requests,
                                        &value,
                                        actual_host_dyn,
                                    )),
                                    _ => Ok(finish(&group, &value, actual_host_dyn)),
                                };
                                let expected = match stage {
                                    0 => comparable(frozen_prepare_initial(
                                        &group,
                                        &owner,
                                        &first,
                                        &value,
                                        &mut expected_host,
                                    )),
                                    1 => comparable(frozen_advance_capture(
                                        &group,
                                        &owner,
                                        &requests,
                                        &value,
                                        &mut expected_host,
                                    )),
                                    _ => Ok(frozen_finish(&group, &value, &mut expected_host)),
                                };
                                assert_eq!(
                                    actual, expected,
                                    "kind={kind}, fuel={remaining}, mode={mode}, defect={defect}, stage={stage}"
                                );
                                assert_eq!(actual_host.events, expected_host.events);
                                assert_eq!(
                                    serde_json::to_vec(&(&group, &requests, &value)).unwrap(),
                                    before
                                );
                                comparisons += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 5760);
}

#[test]
fn group_outcome_classification_precedes_zero_remaining_fuel_and_never_refills() {
    let effect = Effect::UiFocus {
        node_id: "next".into(),
    };
    let first = first(&effect);
    let owner = ContinuationToken::from_wire("merge-1".into());
    for kind in 1..=4 {
        for mode in 0..4 {
            let (mut group, _) = fixture(false, kind);
            group.fuel_remaining = 2;
            let mut host = Host::new(&first, mode, None);
            let result = comparable(prepare_initial(
                &group,
                &owner,
                &first,
                &capture_value(),
                &mut host,
            ));
            if mode == 0 && kind == 4 {
                assert_eq!(
                    result,
                    Ok(Step::Done(Value::Scalar {
                        value: ScalarValue::Integer(7)
                    }))
                );
            } else {
                let error = result.unwrap_err();
                let accepted_host = (mode == 2 && kind == 1) || (mode == 3 && kind != 1);
                assert_eq!(
                    error.code,
                    if accepted_host { "LSV1001" } else { "LSV1409" }
                );
            }
            assert_eq!(host.events.len(), 1);
        }
    }
}

#[test]
fn group_coordination_keeps_borrowed_effects_and_moves_original_native_capture() {
    let effect = Effect::UiFocus {
        node_id: "next".into(),
    };
    let first = first(&effect);
    let owner = ContinuationToken::from_wire("merge-1".into());
    for kind in [1, 2] {
        let (mut group, _) = fixture(false, kind);
        group.fuel_remaining = 30;
        let mut host = Host::new(&first, if kind == 1 { 2 } else { 3 }, None);
        let CaptureProgress::Pending(request) =
            prepare_initial(&group, &owner, &first, &capture_value(), &mut host).unwrap()
        else {
            panic!()
        };
        assert_eq!(request.continuation.fuel_remaining, 28);
        if kind == 1 {
            let Computation::Host { effect } = &group.binding.body else {
                panic!()
            };
            assert_eq!(host.effect_pointer, &**effect as *const Effect as usize);
        } else {
            assert_eq!(
                &**request.continuation.result_binding.as_ref().unwrap() as *const ResultBinding
                    as usize,
                host.binding_pointer
            );
        }
        assert_eq!(host.events.len(), 2);
    }
}

#[test]
fn group_coordinator_never_retries_or_hands_off_after_any_native_hook_unwind() {
    let effect = Effect::UiFocus {
        node_id: "next".into(),
    };
    let first = first(&effect);
    let owner = ContinuationToken::from_wire("merge-1".into());
    for phase in ["group", "capture", "finish", "build", "pair"] {
        let (group, saved) = fixture(false, 3);
        let mut last = first.clone();
        last.continuation.result_binding = saved;
        let requests = vec![first.clone(), last];
        let value = capture_value();
        let mut host = Host::new(&first, 3, None);
        host.panic_at = Some(phase);
        let mut handed_off = false;
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            match phase {
                "group" => {
                    let _ = prepare_initial(&group, &owner, &first, &value, &mut host);
                }
                "finish" => {
                    let _ = finish(&group, &value, &mut host);
                }
                _ => {
                    let _ = advance_capture(&group, &owner, &requests, &value, &mut host);
                }
            }
            handed_off = true;
        }));
        assert!(result.is_err(), "{phase}");
        assert!(!handed_off);
        assert_eq!(
            host.events
                .iter()
                .filter(|event| event.starts_with(&format!("{phase}:")))
                .count(),
            1
        );
        assert!(
            host.events
                .last()
                .unwrap()
                .starts_with(&format!("{phase}:"))
        );
    }
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

// Frozen coordination over the same trusted hook boundary; never calls the new entry points.
fn frozen_prepare_initial<Host: ReferenceGroupExecutionHost + ?Sized>(
    group: &GroupResultBinding,
    owner: &ContinuationToken,
    first: &EffectRequest,
    value: &Value,
    host: &mut Host,
) -> Result<CaptureProgress, Fault> {
    let mut fuel = Fuel::new(group.fuel_remaining);
    let (effect, binding) = match host.resume_group(
        &group.binding,
        value,
        group.plan_branches().map_err(|_| invalid())?,
        &mut fuel,
    )? {
        Outcome::Scalar(value) if group.is_conditional() => {
            return Ok(CaptureProgress::Done(value));
        }
        Outcome::Host(effect) if !group.captures_successor() => (effect, None),
        Outcome::BoundHost { effect, binding } if group.captures_successor() => {
            (effect, Some(binding))
        }
        _ => return Err(invalid()),
    };
    require_successor_fuel(&fuel)?;
    host.build_successor(
        group,
        &effect,
        binding,
        GroupSuccessorContext {
            owner,
            first,
            fuel_remaining: fuel.remaining(),
            position: 0,
        },
    )
    .map(|request| CaptureProgress::Pending(Box::new(request)))
}

/// Validate the ordered saved prefix before native evaluation, then validate the
/// original parent/result/next relationship before handing back prepared data.
fn frozen_advance_capture<Host: ReferenceGroupExecutionHost + ?Sized>(
    group: &GroupResultBinding,
    owner: &ContinuationToken,
    requests: &[EffectRequest],
    value: &Value,
    host: &mut Host,
) -> Result<CaptureProgress, Fault> {
    let Value::Structured { fields } = value else {
        return Err(invalid());
    };
    let (tail, prefix) = fields.split_last().ok_or_else(invalid)?;
    let request = requests.last().ok_or_else(invalid)?;
    let operations = requests
        .iter()
        .take(prefix.len())
        .map(|request| {
            HostOperation::for_effect(&request.continuation.pending_effect).ok_or_else(invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_saved_capture(group, request, prefix, &operations)?;
    let image = &request.continuation;
    let saved = image.result_binding.as_ref().ok_or_else(invalid)?;
    let operation = HostOperation::for_effect(&image.pending_effect).ok_or_else(invalid)?;
    let mut fuel = Fuel::new(image.fuel_remaining);
    match host.resume_capture(saved, &tail.value, operation, &mut fuel)? {
        Outcome::Scalar(value) => Ok(CaptureProgress::Done(value)),
        Outcome::BoundHost { effect, binding } if group.is_dataflow() || group.is_conditional() => {
            require_successor_fuel(&fuel)?;
            let next = host.build_successor(
                group,
                &effect,
                Some(binding),
                GroupSuccessorContext {
                    owner,
                    first: requests.first().ok_or_else(invalid)?,
                    fuel_remaining: fuel.remaining(),
                    position: requests.len() - group.plan_branches().map_err(|_| invalid())?.len(),
                },
            )?;
            host.validate_successor(request, &Step::Done((*tail.value).clone()), &next)?;
            Ok(CaptureProgress::Pending(Box::new(next)))
        }
        _ => Err(invalid()),
    }
}

fn frozen_finish<Host: ReferenceGroupExecutionHost + ?Sized>(
    group: &GroupResultBinding,
    value: &Value,
    host: &mut Host,
) -> Step {
    let mut fuel = Fuel::new(group.fuel_remaining);
    match group
        .plan_branches()
        .map_err(|_| invalid())
        .and_then(|branches| host.finish_group(&group.binding, value, branches, &mut fuel))
    {
        Ok(value) => Step::Done(Value::Scalar { value }),
        Err(error) => Step::Fault(error),
    }
}

fn require_successor_fuel(fuel: &Fuel) -> Result<(), Fault> {
    if fuel.remaining() == 0 {
        Err(Fault {
            code: "LSV1001".into(),
            message: "execution fuel exhausted before group successor".into(),
        })
    } else {
        Ok(())
    }
}
