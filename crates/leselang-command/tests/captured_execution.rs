//! Reference-adapter execution proof, not a replacement VM or storage backend.
//! The fixture owns admission and reply policy; production HIR owns all evaluation.

use std::{borrow::Cow, cell::Cell, marker::PhantomData};

use leselang_command::{
    EffectPreparationError, EffectResult, EffectValueContext, PreflightedResultEnvironment,
    PresentationResult, ProjectedGroup, ProjectedResult, ResultBinding, ResultBindingLimits,
    ResultCaptureHost, ResultCaptureLimits, ResultExecutionHost, ResultExecutionLimits,
    ResultValueDomain, ResultValueHost, ResultView, ResultViewError, StructuredField, Value,
    ValueValidationLimits, prepare_preflighted_effect_in_scope,
    resume_preflighted_result_binding_in_scope, validate_group_capture_correspondence,
    validate_result_binding, validate_result_binding_extension,
};
use leselang_hir::{
    Effect, Type,
    call_evaluation::CallEvaluationError,
    computation::Computation,
    effect_evaluation::{
        EffectEvaluationFailure, EffectEvaluationFault, EffectEvaluationLimits,
        EffectEvaluationOutcome, evaluate_effects_in_scope,
    },
    host_call::HostOperation,
    projection_access::ProjectionAccessLimits,
    pure_evaluation::{
        PureEvaluationFailure, PureEvaluationFault, PureEvaluationLimits, PureValue,
        scalar_copy_cost,
    },
    result_field::ResultField,
};
use leselang_runtime_core::{
    CalculationFailure, CaptureCorrespondenceLimits, CaptureScopeError, CaptureSnapshotError,
    CaptureSnapshotLimits, CaptureValidationLimits, Fuel, GroupCaptureLimits, ScalarValue,
    ScopeFrame, validate_host_result,
};

const LIMITS: EffectEvaluationLimits = EffectEvaluationLimits {
    pure: PureEvaluationLimits {
        max_nodes: 256,
        max_depth: 32,
        max_bindings: 16,
    },
    max_arguments: 8,
    max_branches: 4,
};
const CAPTURES: ResultBindingLimits = ResultBindingLimits {
    captures: CaptureValidationLimits {
        max_saved_bindings: 16,
        max_name_bytes: 64,
    },
    groups: GroupCaptureLimits {
        min_members: 1,
        max_members: 4,
    },
    accesses: ProjectionAccessLimits {
        max_nodes: 256,
        max_depth: 32,
    },
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Refusal {
    Contract,
    Projection,
    Fuel,
    Admission,
}
type Failure = CalculationFailure<Refusal>;
type Evaluated<'a> = EffectEvaluationOutcome<ResultView<'a>, Cow<'a, Effect>, Box<ResultBinding>>;

fn debit(fuel: &mut Fuel, amount: u64) -> Result<(), Refusal> {
    fuel.charge(amount).map_err(|_| Refusal::Fuel)
}

fn debit_result(fuel: &mut Fuel, result: &ProjectedResult) -> Result<(), Refusal> {
    debit(
        fuel,
        1 + result
            .fields
            .iter()
            .map(|field| 1 + scalar_copy_cost(&field.value))
            .sum::<u64>(),
    )
}

fn debit_group(fuel: &mut Fuel, group: &ProjectedGroup) -> Result<(), Refusal> {
    debit(fuel, 1)?;
    for member in &group.members {
        debit_result(fuel, &member.result)?;
    }
    Ok(())
}

fn pure_failure(error: PureEvaluationFailure<Refusal>) -> Failure {
    match error {
        CalculationFailure::Scalar(error) => CalculationFailure::Scalar(error),
        CalculationFailure::External(PureEvaluationFault::Native(error)) => error.into(),
        CalculationFailure::External(PureEvaluationFault::FuelExhausted) => Refusal::Fuel.into(),
        _ => Refusal::Contract.into(),
    }
}

fn effect_failure(error: EffectEvaluationFailure<Refusal>) -> Failure {
    match error {
        CalculationFailure::Scalar(error) => CalculationFailure::Scalar(error),
        CalculationFailure::External(EffectEvaluationFault::Pure(error)) => {
            pure_failure(CalculationFailure::External(error))
        }
        CalculationFailure::External(EffectEvaluationFault::Native(error)) => error.into(),
        _ => Refusal::Contract.into(),
    }
}

fn preparation_failure(error: EffectPreparationError<Refusal>) -> Failure {
    match error {
        EffectPreparationError::Evaluation(error) => effect_failure(error),
        EffectPreparationError::Call { failure, .. } => match failure {
            CallEvaluationError::Evaluation { failure, .. } => pure_failure(failure),
            CallEvaluationError::Scope(error) => pure_failure(CalculationFailure::External(error)),
            CallEvaluationError::FuelExhausted => Refusal::Fuel.into(),
            _ => Refusal::Contract.into(),
        },
        _ => Refusal::Contract.into(),
    }
}

struct Adapter<'a, 'p> {
    allowed: &'p Cell<bool>,
    prepared: &'p Cell<usize>,
    fail_projection: bool,
    marker: PhantomData<&'a Value>,
}

impl ResultCaptureHost for Adapter<'_, '_> {
    type Error = Refusal;

    fn charge_scalar(&self, value: &ScalarValue, fuel: &mut Fuel) -> Result<(), Refusal> {
        debit(fuel, 1 + scalar_copy_cost(value))
    }

    fn charge_result(&self, result: &ProjectedResult, fuel: &mut Fuel) -> Result<(), Refusal> {
        debit_result(fuel, result)
    }

    fn charge_group(&self, group: &ProjectedGroup, fuel: &mut Fuel) -> Result<(), Refusal> {
        debit_group(fuel, group)
    }

    fn projection_error(&self, _: ResultViewError) -> Refusal {
        Refusal::Projection
    }

    fn allow_saved_projection(&self) -> Result<(), Refusal> {
        Ok(())
    }
}

impl<'a> Adapter<'a, '_> {
    fn environment(&self) -> PreflightedResultEnvironment<'a, '_, Self> {
        PreflightedResultEnvironment::new(
            self,
            ResultExecutionLimits {
                effects: LIMITS,
                captures: ResultCaptureLimits {
                    snapshot: CaptureSnapshotLimits {
                        max_bindings: CAPTURES.captures.max_saved_bindings,
                        max_name_bytes: CAPTURES.captures.max_name_bytes,
                    },
                    groups: CAPTURES.groups,
                },
            },
        )
    }
}

impl ResultExecutionHost for Adapter<'_, '_> {
    fn preflight_effect(&self, _: &Computation) -> Result<(), Refusal> {
        if self.allowed.get() {
            Ok(())
        } else {
            Err(Refusal::Admission)
        }
    }
    fn before_prepare_effect(&self, expression: &Computation) -> Result<(), Refusal> {
        self.preflight_effect(expression)?;
        if matches!(expression, Computation::Host { effect } if matches!(**effect, Effect::Compute { .. }))
        {
            return Err(Refusal::Contract);
        }
        Ok(())
    }
    fn prepared_effect(&self, _: &Effect) -> Result<(), Refusal> {
        self.prepared.set(self.prepared.get() + 1);
        Ok(())
    }
    fn before_projection(&self, _: ResultField) -> Result<(), Refusal> {
        if self.fail_projection {
            Err(Refusal::Projection)
        } else {
            Ok(())
        }
    }
    fn valid_group_name(&self, name: &str) -> bool {
        valid_name(name)
    }
    fn preparation_failure(&self, error: EffectPreparationError<Refusal>) -> Failure {
        preparation_failure(error)
    }
    fn capture_failure(&self, error: CaptureSnapshotError<Refusal>) -> Refusal {
        match error {
            CaptureSnapshotError::Native { error, .. } => error,
            _ => Refusal::Contract,
        }
    }
}

fn valid_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
}

impl ResultValueHost for Adapter<'_, '_> {
    type Error = Refusal;

    fn valid_field_name(&self, name: &str) -> bool {
        valid_name(name)
    }

    fn validate_encoded_value(&self, value: &Value) -> Result<(), Refusal> {
        // The fixture's already bounded ingress chooses its own codec/byte policy.
        let bytes = serde_json::to_vec(value).map_err(|_| Refusal::Admission)?;
        if bytes.len() > 16 * 1024 {
            return Err(Refusal::Admission);
        }
        Ok(())
    }
}

#[derive(Debug, Eq, PartialEq)]
enum Step {
    Done(ScalarValue),
    Pending {
        effect: Effect,
        binding: Box<ResultBinding>,
    },
}

fn own(outcome: Evaluated<'_>) -> Result<Step, Failure> {
    match outcome {
        EffectEvaluationOutcome::Value(PureValue::Scalar(value)) => Ok(Step::Done(value)),
        EffectEvaluationOutcome::Suspended { request, capture } => Ok(Step::Pending {
            effect: request.into_owned(),
            binding: capture,
        }),
        _ => Err(Refusal::Contract.into()),
    }
}

fn compile(source: &str) -> Computation {
    let program = leselang_hir::lower(&leselang_syntax::parse(source)).unwrap();
    let Effect::Compute { expression } = program.function.effect else {
        panic!("expected control computation")
    };
    expression.validate_in_scope(&[]).unwrap();
    *expression
}

fn begin(
    expression: &Computation,
    adapter: &Adapter<'_, '_>,
    fuel: &mut Fuel,
) -> Result<Step, Failure> {
    evaluate_effects_in_scope(
        expression,
        &mut ScopeFrame::new(&mut Vec::new()),
        &adapter.environment(),
        fuel,
        LIMITS,
    )
    .map_err(effect_failure)
    .and_then(own)
}

fn matches_reply(effect: &Effect, value: &Value) -> bool {
    let result = match value {
        Value::UiFocus { node_id } => PresentationResult::Focus {
            node_id: node_id.clone(),
        },
        Value::UiSetSelection { node_id, state } => PresentationResult::SetSelection {
            node_id: node_id.clone(),
            state: *state,
        },
        _ => return false,
    };
    result.matches_effect(effect, None)
}

fn resume<'a>(
    effect: &'a Effect,
    binding: &'a ResultBinding,
    value: &'a Value,
    adapter: &Adapter<'a, '_>,
    fuel: &mut Fuel,
) -> Result<Step, Failure> {
    // Native ingress owns these gates. The shared walker must not infer authority.
    if !adapter.allowed.get() {
        return Err(Refusal::Admission.into());
    }
    validate_result_binding(binding, effect, CAPTURES, valid_name)
        .map_err(|_| Refusal::Admission)?;
    let result_type = match effect {
        Effect::Sequence { .. } | Effect::All { .. } => Type::Structured,
        _ => HostOperation::for_effect(effect)
            .ok_or(Refusal::Admission)?
            .result_type(),
    };
    validate_host_result(
        &ResultValueDomain {
            result_type,
            value_limits: ValueValidationLimits {
                max_depth: 16,
                max_fields: 4,
                max_query_items: 4,
            },
            max_output_items: 4,
            host: adapter,
        },
        value,
    )
    .map_err(|_| Refusal::Admission)?;
    let pending = match effect {
        Effect::Sequence { steps } | Effect::All { branches: steps } => {
            let Value::Structured { fields } = value else {
                return Err(Refusal::Admission.into());
            };
            if fields.len() != steps.len()
                || fields.iter().zip(steps).any(|(field, branch)| {
                    field.name != branch.name
                        || field.value.result_type() != branch.result_type
                        || !matches_reply(&branch.effect, &field.value)
                })
            {
                return Err(Refusal::Admission.into());
            }
            ResultView::Group {
                value,
                branches: steps,
            }
        }
        _ => {
            let operation = HostOperation::for_effect(effect).ok_or(Refusal::Admission)?;
            if value.result_type() != operation.result_type() || !matches_reply(effect, value) {
                return Err(Refusal::Admission.into());
            }
            ResultView::Raw { value, operation }
        }
    };
    let mut locals = Vec::new();
    let outcome = resume_preflighted_result_binding_in_scope(
        binding,
        &mut ScopeFrame::new(&mut locals),
        &adapter.environment(),
        fuel,
        LIMITS,
        pending,
        adapter,
    )
    .map_err(|error| match error {
        CaptureScopeError::Native { error, .. } => effect_failure(error),
        _ => Refusal::Contract.into(),
    })?;
    assert!(
        locals.is_empty(),
        "restored scope escaped the production walker"
    );
    own(outcome.into_legacy())
}

// Headless fixture replies, not native dispatch or cryptographic receipts.
fn reply(effect: &Effect) -> Value {
    if let Effect::Sequence { steps } | Effect::All { branches: steps } = effect {
        return Value::Structured {
            fields: steps
                .iter()
                .map(|branch| StructuredField {
                    name: branch.name.clone(),
                    value: Box::new(reply(&branch.effect)),
                })
                .collect(),
        };
    }
    let result = match effect {
        Effect::UiFocus { node_id } => PresentationResult::Focus {
            node_id: node_id.clone(),
        },
        Effect::UiSetSelection { node_id, state } => PresentationResult::SetSelection {
            node_id: node_id.clone(),
            state: *state,
        },
        _ => panic!("fixture received unsupported effect: {effect:?}"),
    };
    EffectResult::Presentation(result)
        .into_effect_value(EffectValueContext {
            effect,
            result_type: HostOperation::for_effect(effect).unwrap().result_type(),
            operation: None,
            expected_revision: None,
            max_output_items: 4,
        })
        .unwrap()
}

fn source(group: &str) -> String {
    format!(
        r#"fn main() = bind(prefix: "p-", body:
      bind(batch: {group}(a: ui.set_selection(node_id: "a", state: "selected"), b: ui.focus(node_id: "b")), body:
      bind(alias: batch, body: bind(first: member(value: alias, name: "a"), body:
      bind(next: ui.focus(node_id: concat(left: prefix, right: field(value: first, name: "node_id"))), body:
      bind(last: ui.focus(node_id: field(value: next, name: "node_id")), body:
      concat(left: field(value: first, name: "node_id"), right: field(value: last, name: "node_id"))))))))"#
    )
}

#[derive(Debug)]
struct Trace {
    value: ScalarValue,
    spent: u64,
    frames: Vec<Vec<u8>>,
    effects: Vec<Effect>,
    preparations: usize,
}

fn run(expression: &Computation, grant: u64, wire: bool) -> Result<Trace, Failure> {
    let allowed = Cell::new(true);
    let prepared = Cell::new(0);
    let adapter = Adapter {
        allowed: &allowed,
        prepared: &prepared,
        fail_projection: false,
        marker: PhantomData,
    };
    let mut fuel = Fuel::new(grant);
    let mut step = begin(expression, &adapter, &mut fuel)?;
    let mut frames = Vec::new();
    let mut effects = Vec::new();
    loop {
        let Step::Pending { effect, binding } = step else {
            let Step::Done(value) = step else {
                unreachable!()
            };
            return Ok(Trace {
                value,
                spent: grant - fuel.remaining(),
                frames,
                effects,
                preparations: prepared.get(),
            });
        };
        let bytes = serde_json::to_vec(&binding).unwrap();
        let restored: Box<ResultBinding> = if wire {
            serde_json::from_slice(&bytes).unwrap()
        } else {
            binding
        };
        let value = reply(&effect);
        step = resume(&effect, &restored, &value, &adapter, &mut fuel)?;
        assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
        if let Step::Pending {
            effect: next_effect,
            binding: next,
        } = &step
        {
            validate_result_binding(next, next_effect, CAPTURES, valid_name).unwrap();
            match &effect {
                Effect::Sequence { steps } | Effect::All { branches: steps } => {
                    let Value::Structured { fields } = &value else {
                        panic!()
                    };
                    let operations = steps
                        .iter()
                        .map(|branch| HostOperation::for_effect(&branch.effect).unwrap())
                        .collect::<Vec<_>>();
                    validate_group_capture_correspondence(
                        next,
                        steps,
                        fields,
                        &operations,
                        CaptureCorrespondenceLimits {
                            max_saved_bindings: 16,
                            max_original_rows: 4,
                        },
                        CAPTURES.groups,
                        valid_name,
                    )
                    .unwrap();
                }
                _ => {
                    validate_result_binding_extension(&restored, next, &effect, &value, 16).unwrap()
                }
            }
        }
        frames.push(bytes);
        effects.push(effect);
        assert!(
            effects.len() <= 8,
            "fixture must not hide an unbounded driver loop"
        );
    }
}

#[test]
fn adapter_executes_group_aliases_captured_successors_and_scalar_tail_without_vm() {
    for group in ["seq", "all"] {
        let expression = compile(&source(group));
        let direct = run(&expression, 1000, false).unwrap();
        let restored = run(&expression, 1000, true).unwrap();
        assert_eq!(direct.value, ScalarValue::String("ap-a".into()));
        assert_eq!(direct.value, restored.value);
        assert_eq!(direct.spent, restored.spent);
        assert_eq!(direct.frames, restored.frames);
        assert_eq!(direct.effects, restored.effects);
        assert_eq!(restored.effects.len(), 3);
        assert_eq!(restored.preparations, 3);
        let captured: ResultBinding = serde_json::from_slice(&restored.frames[1]).unwrap();
        assert_eq!(captured.groups.len(), 2);
        assert_eq!(captured.results.len(), 1);
        assert_eq!(captured.locals.len(), 1);
    }
}

#[test]
fn adapter_reentry_and_wire_restoration_use_one_meter_at_every_fuel_boundary() {
    for group in ["seq", "all"] {
        let expression = compile(&source(group));
        let complete = run(&expression, 1000, true).unwrap();
        for grant in 0..complete.spent {
            assert!(
                matches!(
                    run(&expression, grant, true),
                    Err(CalculationFailure::External(Refusal::Fuel))
                ),
                "{group} grant {grant}"
            );
        }
        for grant in [complete.spent, complete.spent + 1] {
            let result = run(&expression, grant, true).unwrap();
            assert_eq!(result.value, complete.value);
            assert_eq!(result.spent, complete.spent);
            assert_eq!(result.frames, complete.frames);
        }
    }
}

fn saved_stage(expression: &Computation) -> (Effect, ResultBinding, Value) {
    let trace = run(expression, 1000, true).unwrap();
    let binding = serde_json::from_slice(&trace.frames[1]).unwrap();
    let effect = trace.effects[1].clone();
    let value = reply(&effect);
    (effect, binding, value)
}

#[test]
fn adapter_restores_all_five_closed_vocabularies_without_upgrading_successor_captures() {
    for mode in ["seq", "all"] {
        let expression = compile(&source(mode));
        let trace = run(&expression, 1000, true).unwrap();
        let (Effect::Sequence { steps } | Effect::All { branches: steps }) = &trace.effects[0]
        else {
            panic!()
        };
        let originals = steps
            .iter()
            .map(|branch| {
                (
                    HostOperation::for_effect(&branch.effect).unwrap(),
                    reply(&branch.effect),
                )
            })
            .collect::<Vec<_>>();
        for version in 1..=5 {
            let (effect, mut saved, value) = saved_stage(&expression);
            for capture in saved.results.iter_mut().chain(
                saved
                    .groups
                    .iter_mut()
                    .flat_map(|group| group.group.members.iter_mut()),
            ) {
                let (_, raw) = originals
                    .iter()
                    .find(|(operation, value)| {
                        capture.result.matches_capture(*operation, value).unwrap()
                    })
                    .unwrap();
                capture.result =
                    ProjectedResult::capture_version(capture.result.operation, raw, version)
                        .unwrap();
            }
            let bytes = serde_json::to_vec(&saved).unwrap();
            let restored: ResultBinding = serde_json::from_slice(&bytes).unwrap();
            let allowed = Cell::new(true);
            let prepared = Cell::new(0);
            let adapter = Adapter {
                allowed: &allowed,
                prepared: &prepared,
                fail_projection: false,
                marker: PhantomData,
            };
            let mut direct_fuel = Fuel::new(1000);
            let direct = resume(&effect, &saved, &value, &adapter, &mut direct_fuel).unwrap();
            let mut fuel = Fuel::new(1000);
            let output = resume(&effect, &restored, &value, &adapter, &mut fuel).unwrap();
            assert_eq!(output, direct);
            assert_eq!(fuel.remaining(), direct_fuel.remaining());
            let Step::Pending {
                effect: next_effect,
                binding: next,
            } = output
            else {
                panic!()
            };
            validate_result_binding_extension(&restored, &next, &effect, &value, 16).unwrap();
            assert_eq!(next.groups, restored.groups);
            for original in &restored.results {
                assert!(next.results.contains(original));
                assert_eq!(original.result.projection_version, version);
            }
            let final_value = reply(&next_effect);
            assert_eq!(
                resume(&next_effect, &next, &final_value, &adapter, &mut fuel).unwrap(),
                Step::Done(ScalarValue::String("ap-a".into()))
            );
            assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
        }
    }
}

#[test]
fn invalid_frames_cold_fields_replies_and_revoked_admission_never_spend_or_prepare() {
    let expression = compile(&source("seq"));
    let (effect, original, value) = saved_stage(&expression);
    let allowed = Cell::new(true);
    let prepared = Cell::new(0);
    let adapter = Adapter {
        allowed: &allowed,
        prepared: &prepared,
        fail_projection: false,
        marker: PhantomData,
    };
    for case in 0..9 {
        let mut saved = original.clone();
        let mut raw = value.clone();
        match case {
            0 => saved.results[0].result.projection_version = 0,
            1 => saved.name = saved.locals[0].name.clone(),
            2 => saved.groups[0].group.members[0].name = "bad name".into(),
            3 => {
                saved.results[0].result = ProjectedResult::capture_version(
                    HostOperation::UiSetSelection,
                    &Value::UiSetSelection {
                        node_id: "a".into(),
                        state: leselang_hir::UiSelectionState::Selected,
                    },
                    1,
                )
                .unwrap();
                saved.body = Computation::Choose {
                    when: Box::new(Computation::Literal {
                        value: ScalarValue::Boolean(true),
                    }),
                    then: Box::new(saved.body),
                    otherwise: Box::new(Computation::Field {
                        value: Box::new(Computation::Local {
                            name: "first".into(),
                        }),
                        field: ResultField::Selected,
                    }),
                };
            }
            4 => {
                saved.body = Computation::Local {
                    name: "missing".into(),
                }
            }
            5 => {
                raw = Value::UiActivate {
                    node_id: "p-a".into(),
                }
            }
            6 => raw = Value::UiFocus { node_id: "".into() },
            7 => {
                raw = Value::UiFocus {
                    node_id: "wrong-node".into(),
                }
            }
            8 => allowed.set(false),
            _ => unreachable!(),
        }
        let bytes = serde_json::to_vec(&(&saved, &raw)).unwrap();
        let mut fuel = Fuel::new(1000);
        assert!(
            matches!(
                resume(&effect, &saved, &raw, &adapter, &mut fuel),
                Err(CalculationFailure::External(Refusal::Admission))
            ),
            "case {case}"
        );
        assert_eq!(fuel.remaining(), 1000);
        assert_eq!(prepared.get(), 0);
        assert_eq!(serde_json::to_vec(&(&saved, &raw)).unwrap(), bytes);
        allowed.set(true);
    }
    assert!(matches!(
        resume(&effect, &original, &value, &adapter, &mut Fuel::new(1000)).unwrap(),
        Step::Pending { .. }
    ));
    assert_eq!(prepared.get(), 1);
}

#[test]
fn scalar_recovery_and_conditional_successors_do_not_catch_native_projection_or_fuel_failures() {
    let expression = compile(
        r#"fn main() = bind(first: ui.focus(node_id: "not-a-number"), body:
      bind(n: recover(value: parse_integer(value: field(value: first, name: "node_id")), fallback: 7), body:
      choose(when: eq(left: n, right: 7), then:
        bind(next: ui.focus(node_id: "recovered"), body: add(left: n, right: 1)),
        otherwise: 0)))"#,
    );
    let trace = run(&expression, 1000, true).unwrap();
    assert_eq!(trace.value, ScalarValue::Integer(8));
    assert_eq!(trace.effects.len(), 2);
    for grant in 0..trace.spent {
        assert!(matches!(
            run(&expression, grant, true),
            Err(CalculationFailure::External(Refusal::Fuel))
        ));
    }
    let allowed = Cell::new(true);
    let prepared = Cell::new(0);
    let adapter = Adapter {
        allowed: &allowed,
        prepared: &prepared,
        fail_projection: true,
        marker: PhantomData,
    };
    let frame: ResultBinding = serde_json::from_slice(&trace.frames[0]).unwrap();
    let value = reply(&trace.effects[0]);
    assert!(matches!(
        resume(
            &trace.effects[0],
            &frame,
            &value,
            &adapter,
            &mut Fuel::new(1000)
        ),
        Err(CalculationFailure::External(Refusal::Projection))
    ));
    assert_eq!(
        prepared.get(),
        0,
        "native refusal must not reach a recovered successor"
    );
}

#[test]
fn malformed_group_receipts_are_rejected_before_scope_restore_with_retryable_original_data() {
    for mode in ["seq", "all"] {
        let expression = compile(&source(mode));
        let trace = run(&expression, 1000, true).unwrap();
        let frame: ResultBinding = serde_json::from_slice(&trace.frames[0]).unwrap();
        let effect = &trace.effects[0];
        let original = reply(effect);
        let allowed = Cell::new(true);
        let prepared = Cell::new(0);
        let adapter = Adapter {
            allowed: &allowed,
            prepared: &prepared,
            fail_projection: false,
            marker: PhantomData,
        };
        for case in 0..5 {
            let mut value = original.clone();
            let Value::Structured { fields } = &mut value else {
                panic!()
            };
            match case {
                0 => {
                    fields.pop();
                }
                1 => fields.swap(0, 1),
                2 => fields[1].name = fields[0].name.clone(),
                3 => {
                    *fields[1].value = Value::UiActivate {
                        node_id: "b".into(),
                    }
                }
                4 => {
                    *fields[1].value = Value::UiFocus {
                        node_id: "wrong-node".into(),
                    }
                }
                _ => unreachable!(),
            }
            let before = serde_json::to_vec(&(&frame, &value)).unwrap();
            let mut fuel = Fuel::new(1000);
            assert!(matches!(
                resume(effect, &frame, &value, &adapter, &mut fuel),
                Err(CalculationFailure::External(Refusal::Admission))
            ));
            assert_eq!(fuel.remaining(), 1000);
            assert_eq!(prepared.get(), 0);
            assert_eq!(serde_json::to_vec(&(&frame, &value)).unwrap(), before);
        }
        assert!(matches!(
            resume(effect, &frame, &original, &adapter, &mut Fuel::new(1000)).unwrap(),
            Step::Pending { .. }
        ));
        assert_eq!(prepared.get(), 1);
    }
}

#[test]
fn computed_groups_share_product_preparation_through_wire_reentry_and_every_fuel_cutoff() {
    for mode in ["seq", "all"] {
        let source = source(mode)
            .replace(
                "node_id: \"a\"",
                "node_id: concat(left: \"\", right: \"a\")",
            )
            .replace(
                "node_id: \"b\"",
                "node_id: concat(left: prefix, right: \"b\")",
            );
        let expression = compile(&source);
        let direct = run(&expression, 1000, false).unwrap();
        let restored = run(&expression, 1000, true).unwrap();
        assert_eq!(direct.value, ScalarValue::String("ap-a".into()));
        assert_eq!(direct.value, restored.value);
        assert_eq!(direct.spent, restored.spent);
        assert_eq!(direct.frames, restored.frames);
        assert_eq!(direct.effects, restored.effects);
        assert_eq!(restored.preparations, 5); // Two members, group, two successors; no dispatch.
        let (Effect::Sequence { steps } | Effect::All { branches: steps }) = &restored.effects[0]
        else {
            panic!()
        };
        assert_eq!(
            steps
                .iter()
                .map(|step| step.name.as_str())
                .collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(
            steps[1].effect,
            Effect::UiFocus {
                node_id: "p-b".into()
            }
        );
        for grant in 0..restored.spent {
            assert!(
                matches!(
                    run(&expression, grant, true),
                    Err(CalculationFailure::External(Refusal::Fuel))
                ),
                "{mode}: grant {grant}"
            );
        }
        let exact = run(&expression, restored.spent, true).unwrap();
        assert_eq!(exact.value, restored.value);
        assert_eq!(exact.frames, restored.frames);
        assert_eq!(exact.spent, restored.spent);
    }
}

#[test]
fn computed_group_failures_keep_cold_preflight_and_late_failure_boundaries() {
    for mode in ["seq", "all"] {
        let expression = compile(&format!(
            "fn main() = {mode}(a: ui.focus(node_id: concat(left: \"\", right: \"a\")), b: ui.focus(node_id: concat(left: \"\", right: \"b\")))",
        ));
        let allowed = Cell::new(true);
        let prepared = Cell::new(0);
        let adapter = Adapter {
            allowed: &allowed,
            prepared: &prepared,
            fail_projection: false,
            marker: PhantomData,
        };
        for case in 0..3 {
            let mut forged = expression.clone();
            let Computation::Group { branches, .. } = &mut forged else {
                panic!()
            };
            match case {
                0 => branches[1].name = branches[0].name.clone(),
                1 => {
                    let original = branches[0].clone();
                    while branches.len() <= LIMITS.max_branches {
                        let mut row = original.clone();
                        row.name = format!("member{}", branches.len());
                        branches.push(row);
                    }
                }
                2 => allowed.set(false),
                _ => unreachable!(),
            }
            let original = serde_json::to_vec(&forged).unwrap();
            let mut fuel = Fuel::new(1000);
            assert!(begin(&forged, &adapter, &mut fuel).is_err());
            assert_eq!(fuel.remaining(), 1000);
            assert_eq!(prepared.get(), 0);
            assert_eq!(serde_json::to_vec(&forged).unwrap(), original);
            allowed.set(true);
        }
        for (tail, recoverable) in [
            (
                "ui.focus(node_id: concat(left: \"bad\", right: \" node\"))",
                false,
            ),
            (
                "bind(n: parse_integer(value: \"bad\"), body: ui.focus(node_id: \"b\"))",
                true,
            ),
        ] {
            let expression = compile(&format!(
                "fn main() = {mode}(a: ui.focus(node_id: concat(left: \"\", right: \"a\")), b: {tail})",
            ));
            prepared.set(0);
            let mut fuel = Fuel::new(1000);
            let failure = begin(&expression, &adapter, &mut fuel).unwrap_err();
            assert_eq!(failure.is_recoverable(), recoverable);
            assert!(fuel.remaining() < 1000);
            assert_eq!(
                prepared.get(),
                1,
                "first member prepared, but no whole group returned"
            );
        }
    }
}

#[test]
fn preparation_borrows_admitted_host_payloads_and_redacts_non_debug_native_errors() {
    let allowed = Cell::new(true);
    let prepared = Cell::new(0);
    let adapter = Adapter {
        allowed: &allowed,
        prepared: &prepared,
        fail_projection: false,
        marker: PhantomData,
    };
    let expression = Computation::Host {
        effect: Box::new(Effect::UiFocus {
            node_id: "node".into(),
        }),
    };
    let mut fuel = Fuel::new(100);
    let effect = prepare_preflighted_effect_in_scope(
        &expression,
        &mut ScopeFrame::new(&mut Vec::new()),
        &adapter.environment(),
        &mut fuel,
        LIMITS,
    )
    .unwrap();
    let (Cow::Borrowed(actual), Computation::Host { effect: original }) = (effect, &expression)
    else {
        panic!()
    };
    assert!(std::ptr::eq(actual, original.as_ref()));
    assert_eq!(
        fuel.remaining(),
        100,
        "shared outer executor owns the root charge"
    );
    assert_eq!(
        prepared.get(),
        0,
        "helper does not dispatch or call the raw root hook"
    );

    struct Opaque(String); // Deliberately no Debug, Clone, Display or serialization.
    let payload = Opaque("private-host-payload".into());
    let pointer = payload.0.as_ptr();
    let error = EffectPreparationError::Evaluation(CalculationFailure::External(
        EffectEvaluationFault::Native(payload),
    ));
    assert_eq!(format!("{error:?}"), "group member preparation failed");
    assert_eq!(error.to_string(), "group member preparation failed");
    let EffectPreparationError::Evaluation(CalculationFailure::External(
        EffectEvaluationFault::Native(payload),
    )) = error
    else {
        panic!()
    };
    assert_eq!(payload.0.as_ptr(), pointer);
    let forged = EffectPreparationError::<Opaque>::Resolution {
        operation: HostOperation::UiFocus,
        diagnostic_code: "private-host-payload".into(),
    };
    assert_eq!(format!("{forged:?}"), "computed call resolution rejected");
}

#[test]
fn computed_argument_projection_refusal_stays_native_inside_scalar_recovery() {
    let expression = compile(
        r#"fn main() = bind(first: ui.focus(node_id: "source"), body:
          bind(next: ui.focus(node_id: recover(value: field(value: first, name: "node_id"), fallback: "fallback")),
            body: 1))"#,
    );
    let allowed = Cell::new(true);
    let prepared = Cell::new(0);
    let adapter = Adapter {
        allowed: &allowed,
        prepared: &prepared,
        fail_projection: false,
        marker: PhantomData,
    };
    let Step::Pending { effect, binding } =
        begin(&expression, &adapter, &mut Fuel::new(1000)).unwrap()
    else {
        panic!()
    };
    let value = reply(&effect);
    let bytes = serde_json::to_vec(&binding).unwrap();
    let adapter = Adapter {
        fail_projection: true,
        ..adapter
    };
    prepared.set(0);
    let mut fuel = Fuel::new(1000);
    assert!(matches!(
        resume(&effect, &binding, &value, &adapter, &mut fuel),
        Err(CalculationFailure::External(Refusal::Projection))
    ));
    assert!(fuel.remaining() < 1000);
    assert_eq!(
        prepared.get(),
        0,
        "native projection refusal must not prepare the fallback"
    );
    assert_eq!(serde_json::to_vec(&binding).unwrap(), bytes);
}
