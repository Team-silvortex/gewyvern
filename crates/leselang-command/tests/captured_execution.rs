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
use leselang_command::{
    ExecutionOrder, GroupBindingLimits, GroupPlanLimits, GroupResultBinding, MergePlan,
    MergePlanShapeLimits,
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

fn frame_limits() -> leselang_command::ContinuationAdmissionLimits {
    leselang_command::ContinuationAdmissionLimits {
        binding: CAPTURES,
        budget: leselang_runtime_core::ResourceBudgetLimits {
            max_fuel: 10_000,
            max_deadline_ms: 2000,
            max_deadline_at_ms: i64::MAX as u64,
            max_output_items: 4,
        },
        max_group_sequence: 101,
    }
}

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

impl Adapter<'_, '_> {
    fn admit_successor_value(
        &self,
        result_type: Type,
        max_output_items: usize,
        value: &Value,
    ) -> Result<(), Refusal> {
        validate_host_result(
            &ResultValueDomain {
                result_type,
                value_limits: ValueValidationLimits {
                    max_depth: 16,
                    max_fields: 4,
                    max_query_items: 4,
                },
                max_output_items,
                host: self,
            },
            value,
        )
        .map_err(|_| Refusal::Admission)
    }
}

impl leselang_command::SuccessorAdmissionHost<u64> for Adapter<'_, '_> {
    type Error = Refusal;

    fn group_owner_text<'a>(&self, _: &'a u64) -> &'a str {
        panic!("atomic source fixture has no group owner")
    }

    fn valid_token(&self, token: &u64) -> bool {
        0 < *token && *token <= 64
    }

    fn canonical_sequence(&self, token: &u64) -> Option<u64> {
        self.valid_token(token).then_some(*token)
    }

    fn valid_group_name(&self, name: &str) -> bool {
        valid_name(name)
    }

    fn validate_completed_value(
        &self,
        parent: leselang_command::EffectRequestView<'_, u64>,
        value: &Value,
    ) -> Result<(), Refusal> {
        self.admit_successor_value(
            parent.continuation.result_type,
            parent.continuation.max_output_items,
            value,
        )
    }
}

fn canonical_group_sequence(token: &str) -> Option<u64> {
    let sequence = token.strip_prefix("continuation-")?.parse::<u64>().ok()?;
    (0 < sequence && sequence <= 101 && token == format!("continuation-{sequence}"))
        .then_some(sequence)
}

impl leselang_command::SuccessorAdmissionHost<String> for Adapter<'_, '_> {
    type Error = Refusal;
    fn group_owner_text<'a>(&self, token: &'a String) -> &'a str {
        token.as_str()
    }
    fn valid_token(&self, token: &String) -> bool {
        canonical_group_sequence(token).is_some()
    }
    fn canonical_sequence(&self, token: &String) -> Option<u64> {
        canonical_group_sequence(token)
    }
    fn valid_group_name(&self, name: &str) -> bool {
        valid_name(name)
    }
    fn validate_completed_value(
        &self,
        parent: leselang_command::EffectRequestView<'_, String>,
        value: &Value,
    ) -> Result<(), Refusal> {
        self.admit_successor_value(
            parent.continuation.result_type,
            parent.continuation.max_output_items,
            value,
        )
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
    run_with_request_admission(expression, grant, wire, false)
}

fn run_with_request_admission(
    expression: &Computation,
    grant: u64,
    wire: bool,
    admit_requests: bool,
) -> Result<Trace, Failure> {
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
    let mut previous: Option<(AtomicRequest, Value)> = None;
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
        let request = if admit_requests {
            let request = admit_atomic_request(
                &effect,
                &restored,
                fuel.remaining(),
                effects.len() as u64 + 1,
            )?;
            if let Some((parent, value)) = &previous {
                let before = (fuel.remaining(), prepared.get());
                leselang_command::validate_successor_pair(
                    parent.view(),
                    request.view(),
                    Some(value),
                    frame_limits(),
                    &adapter,
                )
                .map_err(|_| Refusal::Admission)?;
                assert_eq!((fuel.remaining(), prepared.get()), before);
            }
            Some(request)
        } else {
            None
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
        previous = request.map(|request| (request, value));
        frames.push(bytes);
        effects.push(effect);
        assert!(
            effects.len() <= 8,
            "fixture must not hide an unbounded driver loop"
        );
    }
}

#[derive(Clone, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct AtomicRequest {
    effect_id: String,
    required: String,
    operation: leselang_command::EffectOperation,
    frame: leselang_runtime_core::ContinuationFrame<
        u64,
        leselang_host_contract::Revision,
        Type,
        Effect,
        ResultBinding,
    >,
    budget: leselang_runtime_core::ResourceBudget,
}

impl AtomicRequest {
    fn view(&self) -> leselang_command::EffectRequestView<'_, u64> {
        leselang_command::EffectRequestView {
            effect_id: &self.effect_id,
            required_capability: &self.required,
            operation: &self.operation,
            continuation: &self.frame,
            budget: &self.budget,
        }
    }
}

fn admit_atomic_request(
    effect: &Effect,
    binding: &ResultBinding,
    remaining: u64,
    sequence: u64,
) -> Result<AtomicRequest, Failure> {
    use leselang_command::{EffectRequestView, validate_effect_request};
    use leselang_host_contract::{CapabilitySet, Principal};
    use leselang_runtime_core::ContinuationFrame;
    use leserpent_domain::{CommandId, IdempotencyKey};

    // This bounded source fixture owns frame construction, not durable admission.
    validate_result_binding(binding, effect, CAPTURES, valid_name)
        .map_err(|_| Refusal::Admission)?;
    let result_type = HostOperation::for_effect(effect)
        .ok_or(Refusal::Admission)?
        .result_type();
    let frame = ContinuationFrame {
        schema_version:
            leselang_command::continuation_schema::preflighted_result_binding_schema_version(
                binding,
            ),
        token: sequence,
        program_counter: 1,
        expected_revision: None,
        result_type,
        pending_effect: effect.clone(),
        fuel_remaining: remaining,
        deadline_ms: 2_000,
        deadline_at_ms: None,
        max_output_items: 4,
        result_binding: Some(Box::new(binding.clone())),
        group_result: None,
    };
    let (required, operation) = leselang_command::lower_effect_operation(
        effect,
        Principal::new("fixture").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
        || {
            Some((
                CommandId::new(format!("leselang-command-{sequence}")).unwrap(),
                IdempotencyKey::new(format!("leselang-effect-{sequence}")).unwrap(),
            ))
        },
    )
    .map_err(|_| Refusal::Admission)?;
    let bytes = serde_json::to_vec(&frame).unwrap();
    assert!(bytes.len() < 16 * 1024);
    let restored = serde_json::from_slice(&bytes).unwrap();
    let budget = frame.resource_budget();
    for candidate in [&frame, &restored] {
        validate_effect_request(
            EffectRequestView {
                effect_id: &format!("effect-{sequence}"),
                required_capability: &required,
                operation: &operation,
                continuation: candidate,
                budget: &budget,
            },
            frame_limits(),
            |_| "",
            |token| 0 < *token && *token <= 64,
            valid_name,
            |token| (0 < *token && *token <= 64).then_some(*token),
        )
        .map_err(|_| Refusal::Admission)?;
    }
    assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
    let request = AtomicRequest {
        effect_id: format!("effect-{sequence}"),
        required,
        operation,
        frame: restored,
        budget,
    };
    let bytes = serde_json::to_vec(&request).unwrap();
    assert!(bytes.len() < 16 * 1024);
    let restored: AtomicRequest = serde_json::from_slice(&bytes).unwrap();
    validate_effect_request(
        restored.view(),
        frame_limits(),
        |_| "",
        |token| 0 < *token && *token <= 64,
        valid_name,
        |token| (0 < *token && *token <= 64).then_some(*token),
    )
    .map_err(|_| Refusal::Admission)?;
    Ok(restored)
}

#[test]
fn source_restored_frames_refuse_torn_admission_without_repair() {
    use leselang_command::ContinuationAdmissionError as Error;
    use leselang_runtime_core::ResourceBudgetError;

    let (original, _, _) = source_pair_fixture();
    for defect in 0..12 {
        let mut request = original.clone();
        let expected = match defect {
            0 => {
                request.frame.schema_version = 0;
                None
            }
            1 => {
                request.frame.program_counter = 0;
                Some(Error::ProgramCounter)
            }
            2 => {
                request.frame.result_type = Type::Structured;
                Some(Error::ResultType)
            }
            3 => {
                request.frame.fuel_remaining = frame_limits().budget.max_fuel + 1;
                Some(Error::Budget(ResourceBudgetError::Fuel))
            }
            4 => {
                request.frame.deadline_ms = 0;
                Some(Error::Budget(ResourceBudgetError::Deadline))
            }
            5 => {
                request.frame.deadline_at_ms = Some(u64::MAX);
                Some(Error::Budget(ResourceBudgetError::AbsoluteDeadline))
            }
            6 => {
                request.frame.max_output_items = frame_limits().budget.max_output_items + 1;
                Some(Error::Budget(ResourceBudgetError::Output))
            }
            7 => {
                request.frame.group_result = Some(0);
                None
            }
            8 => {
                request.frame.token = 0;
                Some(Error::Token)
            }
            9 => {
                request.frame.result_binding = None;
                None
            }
            10 => {
                request.frame.result_binding.as_mut().unwrap().body = Computation::Local {
                    name: "missing".into(),
                };
                // Changing flow shape can refuse its schema before lexical typing.
                None
            }
            11 => {
                request.frame.result_binding.as_mut().unwrap().name =
                    "x".repeat(CAPTURES.captures.max_name_bytes + 1);
                Some(Error::Binding)
            }
            _ => unreachable!(),
        };
        let bytes = serde_json::to_vec(&request).unwrap();
        assert!(bytes.len() < 16 * 1024);
        let restored: AtomicRequest = serde_json::from_slice(&bytes).unwrap();
        let error = leselang_command::validate_continuation_frame(
            &restored.frame,
            frame_limits(),
            |_| "",
            |token| 0 < *token && *token <= 64,
            valid_name,
        )
        .unwrap_err();
        assert_eq!(
            leselang_command::validate_effect_request(
                restored.view(),
                frame_limits(),
                |_| "",
                |token| 0 < *token && *token <= 64,
                valid_name,
                |_| panic!("frame refusal must precede request identity"),
            ),
            Err(leselang_command::RequestAdmissionError::Frame(error)),
        );
        if let Some(expected) = expected {
            assert_eq!(error, expected, "defect {defect}");
        } else {
            assert!(
                matches!(error, Error::Schema(_)),
                "defect {defect}: {error:?}"
            );
        }
        assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
        assert_eq!(restored.budget, original.budget);
    }
}

#[test]
fn source_successor_pairs_share_wire_reentry_without_extra_fuel_or_preparation() {
    for conditional in [false, true] {
        let tail = r#"bind(next: ui.set_selection(node_id: field(value: first, name: "node_id"), state: "selected"), body: bind(last: ui.focus(node_id: field(value: next, name: "node_id")), body: concat(left: field(value: first, name: "node_id"), right: field(value: last, name: "node_id"))))"#;
        let body = if conditional {
            format!(
                r#"choose(when: eq(left: field(value: first, name: "node_id"), right: "a"), then: {tail}, otherwise: "skip")"#
            )
        } else {
            tail.into()
        };
        let expression = compile(&format!(
            r#"fn main() = bind(first: ui.focus(node_id: "a"), body: {body})"#
        ));
        let original = run(&expression, 1000, false).unwrap();
        assert_eq!(original.effects.len(), 3);
        for grant in 0..original.spent {
            assert!(
                matches!(
                    run_with_request_admission(&expression, grant, true, true),
                    Err(CalculationFailure::External(Refusal::Fuel))
                ),
                "conditional {conditional}, grant {grant}"
            );
        }
        for grant in [original.spent, original.spent + 1, 1000] {
            let admitted = run_with_request_admission(&expression, grant, true, true).unwrap();
            assert_eq!(admitted.value, original.value);
            assert_eq!(admitted.spent, original.spent);
            assert_eq!(admitted.preparations, original.preparations);
            assert_eq!(admitted.effects, original.effects);
            assert_eq!(admitted.frames, original.frames);
        }
    }
}

fn source_pair_fixture() -> (AtomicRequest, AtomicRequest, Value) {
    let expression = compile(
        r#"fn main() = bind(first: ui.focus(node_id: "a"), body: bind(next: ui.focus(node_id: field(value: first, name: "node_id")), body: field(value: next, name: "node_id")))"#,
    );
    let trace = run(&expression, 1000, true).unwrap();
    let parent = admit_atomic_request(
        &trace.effects[0],
        &serde_json::from_slice::<ResultBinding>(&trace.frames[0]).unwrap(),
        100,
        1,
    )
    .unwrap();
    let next = admit_atomic_request(
        &trace.effects[1],
        &serde_json::from_slice::<ResultBinding>(&trace.frames[1]).unwrap(),
        80,
        2,
    )
    .unwrap();
    let value = reply(&trace.effects[0]);
    (parent, next, value)
}

#[test]
fn complete_successor_admission_rechecks_requests_before_completion_and_value() {
    use leselang_command::{SuccessorAdmissionError as Error, validate_successor_pair};

    let (mut parent, mut next, value) = source_pair_fixture();
    let allowed = Cell::new(true);
    let prepared = Cell::new(0);
    let host = Adapter {
        allowed: &allowed,
        prepared: &prepared,
        fail_projection: false,
        marker: PhantomData,
    };
    let check = |parent: &AtomicRequest, next: &AtomicRequest, value: Option<&Value>| {
        validate_successor_pair(parent.view(), next.view(), value, frame_limits(), &host)
    };
    assert_eq!(check(&parent, &next, Some(&value)), Ok(()));
    parent.frame.program_counter = 0;
    next.frame.schema_version = 0;
    assert!(matches!(check(&parent, &next, None), Err(Error::Parent(_))));
    parent.frame.program_counter = 1;
    assert!(matches!(check(&parent, &next, None), Err(Error::Next(_))));
    next.frame.schema_version =
        leselang_command::continuation_schema::preflighted_result_binding_schema_version(
            next.frame.result_binding.as_ref().unwrap(),
        );
    assert_eq!(check(&parent, &next, None), Err(Error::Completion));
    assert_eq!(
        check(
            &parent,
            &next,
            Some(&Value::Scalar {
                value: ScalarValue::None
            })
        ),
        Err(Error::Completion)
    );
    assert_eq!(
        check(
            &parent,
            &next,
            Some(&Value::UiActivate {
                node_id: "a".into()
            })
        ),
        Err(Error::Value(Refusal::Admission))
    );
    assert!(matches!(
        check(
            &parent,
            &next,
            Some(&Value::UiFocus {
                node_id: "other".into()
            })
        ),
        Err(Error::Correspondence(_))
    ));
    for limit in [0, 4] {
        parent.frame.max_output_items = limit;
        parent.budget = parent.frame.resource_budget();
        next.frame.max_output_items = limit;
        next.budget = next.frame.resource_budget();
        let bytes = serde_json::to_vec(&(&parent, &next, &value)).unwrap();
        let (first, second, raw): (AtomicRequest, AtomicRequest, Value) =
            serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            check(&first, &second, Some(&raw)),
            if limit == 0 {
                Err(Error::Value(Refusal::Admission))
            } else {
                Ok(())
            }
        );
        assert_eq!(
            serde_json::to_vec(&(&parent, &next, &value)).unwrap(),
            bytes
        );
        assert_eq!(serde_json::to_vec(&(&first, &second, &raw)).unwrap(), bytes);
    }
    // Consistency does not consume or refresh the caller's execution grant.
    allowed.set(false);
    assert_eq!(check(&parent, &next, Some(&value)), Ok(()));
    assert!(!allowed.get());
    assert_eq!(prepared.get(), 0);
    assert_eq!(
        (parent.frame.fuel_remaining, next.frame.fuel_remaining),
        (100, 80)
    );
}

#[test]
fn complete_successor_admission_borrows_local_owners_and_preserves_each_native_unwind() {
    use leselang_command::{
        EffectRequestView, SuccessorAdmissionError, SuccessorAdmissionHost, validate_successor_pair,
    };
    use leselang_host_contract::Revision;
    use leselang_runtime_core::ContinuationFrame;
    use std::{
        cell::RefCell,
        error::Error,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    struct Token {
        sequence: u64,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for Token {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    struct NativeFailure(Rc<Cell<usize>>);
    impl Drop for NativeFailure {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    type Frame = ContinuationFrame<Token, Revision, Type, Effect, ResultBinding>;
    fn local(request: &AtomicRequest, drops: &Rc<Cell<usize>>) -> Frame {
        let frame = request.frame.clone();
        Frame {
            token: Token {
                sequence: frame.token,
                drops: drops.clone(),
            },
            schema_version: frame.schema_version,
            program_counter: frame.program_counter,
            expected_revision: frame.expected_revision,
            result_type: frame.result_type,
            pending_effect: frame.pending_effect,
            fuel_remaining: frame.fuel_remaining,
            deadline_ms: frame.deadline_ms,
            deadline_at_ms: frame.deadline_at_ms,
            max_output_items: frame.max_output_items,
            result_binding: frame.result_binding,
            group_result: None,
        }
    }
    fn view<'a>(request: &'a AtomicRequest, frame: &'a Frame) -> EffectRequestView<'a, Token> {
        EffectRequestView {
            effect_id: &request.effect_id,
            required_capability: &request.required,
            operation: &request.operation,
            continuation: frame,
            budget: &request.budget,
        }
    }
    struct Host<'a> {
        first: &'a Frame,
        second: &'a Frame,
        value: &'a Value,
        calls: RefCell<Vec<String>>,
        panic_at: Cell<Option<usize>>,
        fail_value: Cell<bool>,
        failure_drops: Rc<Cell<usize>>,
    }
    impl Host<'_> {
        fn record(&self, event: String) {
            let mut calls = self.calls.borrow_mut();
            calls.push(event);
            assert_ne!(self.panic_at.get(), Some(calls.len()), "native hook unwind");
        }
    }
    impl SuccessorAdmissionHost<Token> for Host<'_> {
        type Error = NativeFailure;
        fn group_owner_text<'a>(&self, _: &'a Token) -> &'a str {
            panic!("no group owner")
        }
        fn valid_token(&self, token: &Token) -> bool {
            assert!(
                std::ptr::eq(token, &self.first.token) || std::ptr::eq(token, &self.second.token)
            );
            self.record(format!("token:{}", token.sequence));
            token.sequence > 0
        }
        fn canonical_sequence(&self, token: &Token) -> Option<u64> {
            self.record(format!("sequence:{}", token.sequence));
            Some(token.sequence)
        }
        fn valid_group_name(&self, name: &str) -> bool {
            self.record(format!("name:{name}"));
            valid_name(name)
        }
        fn validate_completed_value(
            &self,
            parent: EffectRequestView<'_, Token>,
            value: &Value,
        ) -> Result<(), NativeFailure> {
            assert!(std::ptr::eq(parent.continuation, self.first));
            assert!(std::ptr::eq(value, self.value));
            self.record("value".into());
            if self.fail_value.get() {
                Err(NativeFailure(self.failure_drops.clone()))
            } else {
                Ok(())
            }
        }
    }
    let (parent, next, value) = source_pair_fixture();
    let drops = Rc::new(Cell::new(0));
    let failure_drops = Rc::new(Cell::new(0));
    let first = local(&parent, &drops);
    let second = local(&next, &drops);
    let before = serde_json::to_vec(&(&parent, &next, &value)).unwrap();
    let host = Host {
        first: &first,
        second: &second,
        value: &value,
        calls: RefCell::new(vec![]),
        panic_at: Cell::new(None),
        fail_value: Cell::new(false),
        failure_drops: failure_drops.clone(),
    };
    let check = || {
        let policy: &dyn SuccessorAdmissionHost<Token, Error = NativeFailure> = &host;
        validate_successor_pair(
            view(&parent, &first),
            view(&next, &second),
            Some(&value),
            frame_limits(),
            policy,
        )
    };
    assert!(check().is_ok());
    let events = host.calls.borrow().clone();
    assert_eq!(
        events
            .iter()
            .filter(|event| !event.starts_with("name:"))
            .map(String::as_str)
            .collect::<Vec<_>>(),
        [
            "token:1",
            "sequence:1",
            "token:2",
            "sequence:2",
            "value",
            "sequence:1",
            "sequence:2"
        ]
    );
    for cutoff in 1..=events.len() {
        host.calls.borrow_mut().clear();
        host.panic_at.set(Some(cutoff));
        assert!(catch_unwind(AssertUnwindSafe(check)).is_err());
        assert_eq!(*host.calls.borrow(), events[..cutoff]);
        assert_eq!(drops.get(), 0);
        assert_eq!(failure_drops.get(), 0);
        assert_eq!(
            serde_json::to_vec(&(&parent, &next, &value)).unwrap(),
            before
        );
    }
    host.panic_at.set(None);
    host.fail_value.set(true);
    host.calls.borrow_mut().clear();
    let error = check().unwrap_err();
    assert_eq!(error.to_string(), "invalid successor completed value");
    assert_eq!(format!("{error:?}"), error.to_string());
    assert!(error.source().is_none());
    assert_eq!(host.calls.borrow().last().unwrap(), "value");
    let SuccessorAdmissionError::Value(original) = error else {
        panic!("native failure lost")
    };
    assert!(Rc::ptr_eq(&original.0, &failure_drops));
    assert_eq!(failure_drops.get(), 0);
    drop(original);
    assert_eq!(failure_drops.get(), 1);
    drop(host);
    drop((first, second));
    assert_eq!(drops.get(), 2);
}

#[test]
fn successor_pair_refuses_torn_rows_without_mutating_original_frames() {
    let (parent, next, value) = source_pair_fixture();
    let check = |parent: &AtomicRequest, next: &AtomicRequest, value: &Value| {
        leselang_command::validate_preflighted_successor_pair(
            parent.view(),
            next.view(),
            value,
            CAPTURES,
            |_| "group",
            |token| Some(*token),
            valid_name,
        )
    };
    assert_eq!(check(&parent, &next, &value), Ok(()));
    for defect in 0..12 {
        let mut torn = next.clone();
        let mut raw = value.clone();
        match defect {
            0 => torn.frame.schema_version = 0,
            1 => torn.frame.token = 1,
            2 => torn.frame.fuel_remaining = 100,
            3 => torn.frame.deadline_ms += 1,
            4 => torn.frame.expected_revision = Some(leselang_host_contract::Revision(99)),
            5 => torn.frame.max_output_items += 1,
            6 => {
                let leselang_command::EffectOperation::Presentation(e) = &mut torn.operation else {
                    panic!()
                };
                e.principal = leselang_host_contract::Principal::new("other").unwrap();
            }
            7 => torn.frame.result_binding.as_mut().unwrap().name = "bad name".into(),
            8 => torn.frame.result_binding.as_mut().unwrap().results.clear(),
            9 => torn.frame.result_binding.as_mut().unwrap().results[0].name = "renamed".into(),
            10 => {
                torn.frame.result_binding.as_mut().unwrap().results[0]
                    .result
                    .fields[0]
                    .value = ScalarValue::String("forged".into())
            }
            11 => {
                raw = Value::UiFocus {
                    node_id: "different".into(),
                }
            }
            _ => unreachable!(),
        }
        let before = serde_json::to_vec(&(&parent, &torn, &raw)).unwrap();
        assert!(check(&parent, &torn, &raw).is_err(), "defect {defect}");
        assert_eq!(serde_json::to_vec(&(&parent, &torn, &raw)).unwrap(), before);
    }
}

#[test]
fn successor_pair_borrows_opaque_local_tokens_and_preserves_hook_order_and_unwind() {
    use leselang_command::{
        EffectRequestView, SuccessorCorrespondenceError, validate_preflighted_successor_pair,
    };
    use leselang_host_contract::Revision;
    use leselang_runtime_core::ContinuationFrame;
    use std::{
        cell::RefCell,
        error::Error,
        panic::{AssertUnwindSafe, catch_unwind},
        rc::Rc,
    };

    struct Token {
        sequence: u64,
        drops: Rc<Cell<usize>>,
    }
    impl Drop for Token {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    type Frame<T> = ContinuationFrame<T, Revision, Type, Effect, ResultBinding>;
    fn local(frame: Frame<u64>, drops: Rc<Cell<usize>>) -> Frame<Token> {
        assert!(frame.group_result.is_none());
        Frame {
            schema_version: frame.schema_version,
            token: Token {
                sequence: frame.token,
                drops,
            },
            program_counter: frame.program_counter,
            expected_revision: frame.expected_revision,
            result_type: frame.result_type,
            pending_effect: frame.pending_effect,
            fuel_remaining: frame.fuel_remaining,
            deadline_ms: frame.deadline_ms,
            deadline_at_ms: frame.deadline_at_ms,
            max_output_items: frame.max_output_items,
            result_binding: frame.result_binding,
            group_result: None,
        }
    }
    fn view<'a, T>(request: &'a AtomicRequest, frame: &'a Frame<T>) -> EffectRequestView<'a, T> {
        EffectRequestView {
            effect_id: &request.effect_id,
            required_capability: &request.required,
            operation: &request.operation,
            continuation: frame,
            budget: &request.budget,
        }
    }
    let (parent, next, value) = source_pair_fixture();
    let drops = Rc::new(Cell::new(0));
    let first = local(parent.frame.clone(), drops.clone());
    let mut second = local(next.frame.clone(), drops.clone());
    let calls = RefCell::new(Vec::new());
    let sequence = |token: &Token| {
        assert!(std::ptr::eq(token, &first.token) || std::ptr::eq(token, &second.token));
        calls.borrow_mut().push(token.sequence);
        Some(token.sequence)
    };
    assert_eq!(
        validate_preflighted_successor_pair(
            view(&parent, &first),
            view(&next, &second),
            &value,
            CAPTURES,
            |_| panic!("no group owner"),
            sequence,
            valid_name
        ),
        Ok(())
    );
    assert_eq!(*calls.borrow(), [1, 2]);
    assert_eq!(drops.get(), 0);
    let count = Cell::new(0);
    let panic = catch_unwind(AssertUnwindSafe(|| {
        let _ = validate_preflighted_successor_pair(
            view(&parent, &first),
            view(&next, &second),
            &value,
            CAPTURES,
            |_| panic!("no group owner"),
            |token| {
                count.set(count.get() + 1);
                if token.sequence == 2 {
                    panic!("native projection failed");
                }
                Some(token.sequence)
            },
            valid_name,
        );
    }));
    assert!(panic.is_err());
    assert_eq!(count.get(), 2);
    assert_eq!(drops.get(), 0);
    assert_eq!((first.fuel_remaining, second.fuel_remaining), (100, 80));
    second.deadline_ms += 1;
    let error = validate_preflighted_successor_pair(
        view(&parent, &first),
        view(&next, &second),
        &value,
        CAPTURES,
        |_| panic!("no group owner"),
        |_| panic!("context refuses before identity"),
        |_| panic!("context refuses before typing"),
    )
    .unwrap_err();
    assert_eq!(error, SuccessorCorrespondenceError::Mismatch);
    assert_eq!(error.to_string(), "invalid successor correspondence");
    assert!(error.source().is_none());
    drop((first, second));
    assert_eq!(drops.get(), 2);
}

#[test]
fn source_atomic_requests_share_frame_correspondence_before_wire_restored_reentry() {
    let expression = compile(
        r#"fn main() = bind(first: ui.focus(node_id: "first"), body:
        bind(next: ui.focus(node_id: field(value: first, name: "node_id")), body:
        concat(left: field(value: first, name: "node_id"), right: field(value: next, name: "node_id"))))"#,
    );
    let original = run(&expression, 10_000, false).unwrap();
    let admitted = run_with_request_admission(&expression, 10_000, true, true).unwrap();
    assert_eq!(admitted.value, ScalarValue::String("firstfirst".into()));
    assert_eq!(admitted.value, original.value);
    assert_eq!(admitted.effects.len(), 2);
    assert_eq!(admitted.effects, original.effects);
    assert_eq!(admitted.frames, original.frames);
    assert_eq!(admitted.preparations, original.preparations);
    assert_eq!(admitted.spent, original.spent);
}

#[test]
fn adapter_saved_group_plan_gates_source_reentry_without_vm_or_resource_changes() {
    use leselang_command::{EffectOperation, EffectRequestView};
    use leselang_host_contract::{CapabilitySet, Principal, Revision};
    use leselang_runtime_core::{ContinuationFrame, ResourceBudget};

    #[derive(Clone, serde::Serialize, serde::Deserialize)]
    struct SavedRequest {
        operation: EffectOperation,
        required: String,
        effect_id: String,
        frame: ContinuationFrame<String, Revision, Type, Effect, ResultBinding>,
        budget: ResourceBudget,
    }
    impl SavedRequest {
        fn view(&self) -> EffectRequestView<'_, String> {
            EffectRequestView {
                effect_id: &self.effect_id,
                required_capability: &self.required,
                operation: &self.operation,
                continuation: &self.frame,
                budget: &self.budget,
            }
        }
    }
    struct GraphHost;
    impl leselang_command::MergeGraphAdmissionHost<String> for GraphHost {
        type Error = Refusal;
        fn token_text<'a>(&self, token: &'a String) -> &'a str {
            token.as_str()
        }
        fn valid_token(&self, token: &String) -> bool {
            token == "merge-1" || canonical(token).is_some()
        }
        fn canonical_sequence(&self, token: &String) -> Option<u64> {
            canonical(token)
        }
        fn valid_group_name(&self, name: &str) -> bool {
            valid_name(name)
        }
        fn validate_plan_encoding(&self, plan: &MergePlan) -> Result<(), Refusal> {
            let bytes = serde_json::to_vec(plan).map_err(|_| Refusal::Admission)?;
            if bytes.len() > 16 * 1024 {
                Err(Refusal::Admission)
            } else {
                Ok(())
            }
        }
    }
    fn plan_policy() -> leselang_command::MergePlanAdmissionLimits {
        leselang_command::MergePlanAdmissionLimits {
            shape: MergePlanShapeLimits {
                max_branches: 4,
                max_name_bytes: 64,
            },
            group: GroupBindingLimits {
                plan: GroupPlanLimits {
                    max_branches: 4,
                    max_effect_sequence: 101,
                    max_fuel: 1000,
                },
                binding: CAPTURES,
            },
        }
    }
    fn saved_request(
        group: &GroupResultBinding,
        effect: &Effect,
        binding: Option<Box<ResultBinding>>,
        sequence: u64,
        fuel: u64,
    ) -> SavedRequest {
        let (required, operation) = leselang_command::lower_effect_operation(
            effect,
            Principal::new("fixture").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            None,
            || panic!("source fixture only prepares presentation operations"),
        )
        .unwrap();
        let frame = ContinuationFrame {
            schema_version: group.continuation_schema(),
            token: format!("continuation-{sequence}"),
            program_counter: 1,
            expected_revision: None,
            result_type: HostOperation::for_effect(effect).unwrap().result_type(),
            pending_effect: effect.clone(),
            fuel_remaining: fuel,
            deadline_ms: 2_000,
            deadline_at_ms: None,
            max_output_items: 4,
            result_binding: binding,
            group_result: Some("merge-1".into()),
        };
        let budget = frame.resource_budget();
        SavedRequest {
            operation,
            required,
            effect_id: format!("effect-{sequence}"),
            frame,
            budget,
        }
    }
    fn canonical(token: &str) -> Option<u64> {
        canonical_group_sequence(token)
    }
    fn check_requests(
        plan: &MergePlan,
        requests: &[(String, SavedRequest)],
    ) -> Result<(), Refusal> {
        let names = requests
            .iter()
            .map(|(name, request)| (name.as_str(), request))
            .collect::<Vec<_>>();
        leselang_command::validate_merge_graph(
            &"merge-1".into(),
            plan,
            &names,
            leselang_command::MergeGraphAdmissionLimits {
                plan: plan_policy(),
                frame: frame_limits(),
            },
            |request| request.view(),
            &GraphHost,
        )
        .map_err(|_| Refusal::Admission)
    }
    fn verify_requests(
        plan: &MergePlan,
        requests: &[(String, SavedRequest)],
        fuel: &Fuel,
        prepared: &Cell<usize>,
    ) {
        let remaining = fuel.remaining();
        let preparations = prepared.get();
        assert_eq!(check_requests(plan, requests), Ok(()));
        let bytes = serde_json::to_vec(requests).unwrap();
        assert!(bytes.len() < 32 * 1024);
        let restored: Vec<(String, SavedRequest)> = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(check_requests(plan, &restored), Ok(()));
        assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
        for defect in 0..8 {
            let mut broken = restored.clone();
            let (name, request) = broken.last_mut().unwrap();
            match defect {
                0 => *name = "wrong-name".into(),
                1 | 2 => {
                    let EffectOperation::Presentation(envelope) = &mut request.operation else {
                        panic!()
                    };
                    if defect == 1 {
                        envelope.principal.id = "other".into();
                    } else {
                        envelope.capabilities =
                            CapabilitySet::new(["ui.presentation", "runtime.read"]);
                    }
                }
                3 => request.frame.expected_revision = Some(Revision(1)),
                4 => request.frame.deadline_ms += 1,
                5 => request.frame.group_result = Some("merge-999".into()),
                6 => request.frame.token = "continuation-0100".into(),
                7 => request.frame.fuel_remaining = u64::MAX,
                _ => unreachable!(),
            }
            let before = serde_json::to_vec(&broken).unwrap();
            assert_eq!(
                check_requests(plan, &broken),
                Err(Refusal::Admission),
                "defect {defect}"
            );
            assert_eq!(serde_json::to_vec(&broken).unwrap(), before);
        }
        assert_eq!(fuel.remaining(), remaining);
        assert_eq!(prepared.get(), preparations);
    }

    // Fixture-owned ingress/identity policy, not a production recovery authority.
    fn admit(plan: &MergePlan) -> Result<&GroupResultBinding, Refusal> {
        leselang_command::validate_merge_plan(plan, plan_policy(), valid_name, |plan| {
            leselang_command::MergeGraphAdmissionHost::validate_plan_encoding(&GraphHost, plan)
        })
        .map_err(|_| Refusal::Admission)?;
        plan.result_binding.as_deref().ok_or(Refusal::Admission)
    }

    let mut admitted_pairs = 0;
    for mode in ["seq", "all"] {
        let expression = compile(&source(mode));
        let expected = run(&expression, 1000, true).unwrap();
        let allowed = Cell::new(true);
        let prepared = Cell::new(0);
        let adapter = Adapter {
            allowed: &allowed,
            prepared: &prepared,
            fail_projection: false,
            marker: PhantomData,
        };
        let mut fuel = Fuel::new(1000);
        let Step::Pending { effect, binding } = begin(&expression, &adapter, &mut fuel).unwrap()
        else {
            panic!("expected source group");
        };
        let plan = MergePlan {
            branches: vec!["a".into(), "b".into()],
            order: if mode == "all" {
                ExecutionOrder::ParallelGroupDataflow
            } else {
                ExecutionOrder::GroupDataflow
            },
            result_binding: Some(Box::new(GroupResultBinding {
                pending: effect,
                binding,
                fuel_remaining: fuel.remaining(),
                successor_sequence: Some(100),
                additional_successor_sequences: vec![101],
            })),
        };
        let bytes = serde_json::to_vec(&plan).unwrap();
        drop(plan);
        let restored: MergePlan = serde_json::from_slice(&bytes).unwrap();
        let remaining = fuel.remaining();
        let preparations = prepared.get();
        for defect in 0..5 {
            let mut broken = restored.clone();
            match defect {
                0 => broken.branches.reverse(),
                1 => broken.order = ExecutionOrder::GroupTail,
                2 => {
                    broken
                        .result_binding
                        .as_mut()
                        .unwrap()
                        .additional_successor_sequences[0] = 100
                }
                3 => broken.result_binding.as_mut().unwrap().fuel_remaining = 1001,
                4 => {
                    let group = broken.result_binding.as_mut().unwrap();
                    let (Effect::Sequence { steps } | Effect::All { branches: steps }) =
                        &mut group.pending
                    else {
                        unreachable!()
                    };
                    steps[0].result_type = Type::UiFocus;
                }
                _ => unreachable!(),
            }
            let original = serde_json::to_vec(&broken).unwrap();
            assert_eq!(admit(&broken), Err(Refusal::Admission));
            assert_eq!(serde_json::to_vec(&broken).unwrap(), original);
            assert_eq!(fuel.remaining(), remaining);
            assert_eq!(prepared.get(), preparations);
        }
        let group = admit(&restored).unwrap();
        assert_eq!(group.fuel_remaining, fuel.remaining());
        let mut active_plan = restored.clone();
        let mut requests = group
            .plan_branches()
            .unwrap()
            .iter()
            .enumerate()
            .map(|(index, branch)| {
                let fuel = group.fuel_remaining + 1 - if mode == "seq" { index as u64 } else { 0 };
                (
                    branch.name.clone(),
                    saved_request(group, &branch.effect, None, index as u64 + 2, fuel),
                )
            })
            .collect::<Vec<_>>();
        verify_requests(&active_plan, &requests, &fuel, &prepared);
        let mut step = resume(
            &group.pending,
            &group.binding,
            &reply(&group.pending),
            &adapter,
            &mut fuel,
        )
        .unwrap();
        for position in 0..2 {
            let Step::Pending { effect, binding } = step else {
                panic!("expected captured successor")
            };
            let name = group.plan_successor_name_at(position, 4).unwrap();
            active_plan.branches.push(name.clone());
            requests.push((
                name,
                saved_request(
                    group,
                    &effect,
                    Some(binding.clone()),
                    100 + position as u64,
                    fuel.remaining(),
                ),
            ));
            verify_requests(&active_plan, &requests, &fuel, &prepared);
            if position > 0 {
                let parent = &requests[requests.len() - 2].1;
                let next = &requests.last().unwrap().1;
                let value = reply(&parent.frame.pending_effect);
                let bytes = serde_json::to_vec(&(parent, next, &value)).unwrap();
                let (first, second, raw): (SavedRequest, SavedRequest, Value) =
                    serde_json::from_slice(&bytes).unwrap();
                let before = (fuel.remaining(), prepared.get());
                for (first, second, value) in [(parent, next, &value), (&first, &second, &raw)] {
                    leselang_command::validate_successor_pair(
                        first.view(),
                        second.view(),
                        Some(value),
                        frame_limits(),
                        &adapter,
                    )
                    .unwrap();
                }
                assert_eq!((fuel.remaining(), prepared.get()), before);
                assert_eq!(serde_json::to_vec(&(&first, &second, &raw)).unwrap(), bytes);
                assert_eq!(serde_json::to_vec(&(parent, next, &value)).unwrap(), bytes);
                admitted_pairs += 1;
            }
            step = resume(&effect, &binding, &reply(&effect), &adapter, &mut fuel).unwrap();
        }
        assert_eq!(step, Step::Done(expected.value));
        assert_eq!(1000 - fuel.remaining(), expected.spent);
        assert_eq!(prepared.get(), expected.preparations);
        assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
    }
    assert_eq!(admitted_pairs, 2);
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
