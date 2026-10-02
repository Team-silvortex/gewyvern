use super::*;

pub(super) type CompletionLease<'a> = Option<(&'a DispatchLease, u64)>;

pub(super) fn invalid() -> Fault {
    Fault {
        code: "LSV1407".into(),
        message: "invalid result-driven successor or missing original effect authority".into(),
    }
}

pub(super) fn authority(operation: &EffectOperation) -> (&Principal, &CapabilitySet) {
    match operation {
        EffectOperation::Query(envelope) => (&envelope.principal, &envelope.capabilities),
        EffectOperation::Command(envelope) => (&envelope.principal, &envelope.capabilities),
        EffectOperation::Presentation(envelope) => (&envelope.principal, &envelope.capabilities),
    }
}

pub(super) fn validate_pair(
    parent: &EffectRequest,
    result: &Step,
    next: &EffectRequest,
) -> Result<(), Fault> {
    validate_effect_request(parent)?;
    validate_effect_request(next)?;
    let first = &parent.continuation;
    let second = &next.continuation;
    let Step::Done(value) = result else {
        return Err(invalid());
    };
    if matches!(value, Value::Scalar { .. }) {
        return Err(invalid());
    }
    validate_bound_value(first, value)?;
    let schema_valid = match first.schema_version {
        SUCCESSOR_CONTINUATION_SCHEMA_VERSION => {
            second.schema_version == CONTINUATION_SCHEMA_VERSION
        }
        DATAFLOW_CONTINUATION_SCHEMA_VERSION => matches!(
            second.schema_version,
            CONTINUATION_SCHEMA_VERSION | DATAFLOW_CONTINUATION_SCHEMA_VERSION
        ),
        CONDITIONAL_CONTINUATION_SCHEMA_VERSION => matches!(
            second.schema_version,
            CONTINUATION_SCHEMA_VERSION
                | DATAFLOW_CONTINUATION_SCHEMA_VERSION
                | CONDITIONAL_CONTINUATION_SCHEMA_VERSION
        ),
        _ => false,
    };
    if !schema_valid
        || first
            .result_binding
            .as_ref()
            .is_none_or(|binding| binding.body.is_pure())
        || authority(&parent.operation) != authority(&next.operation)
        || first.expected_revision != second.expected_revision
        || first.deadline_ms != second.deadline_ms
        || first.deadline_at_ms != second.deadline_at_ms
        || first.max_output_items != second.max_output_items
        || first.fuel_remaining <= second.fuel_remaining
        || canonical_continuation_sequence(&first.token)
            >= canonical_continuation_sequence(&second.token)
    {
        return Err(invalid());
    }
    let prior = first.result_binding.as_ref().ok_or_else(invalid)?;
    let next_type = match &second.result_binding {
        Some(binding) => binding.validate(&second.pending_effect)?,
        None => second.result_type,
    };
    if prior.validate(&first.pending_effect)? != next_type {
        return Err(invalid());
    }
    if let Some(binding) = &second.result_binding {
        let operation = leselang_hir::host_call::HostOperation::for_effect(&first.pending_effect)
            .ok_or_else(invalid)?;
        let current = result_binding::ProjectedResult::capture(operation, value)?;
        if !binding
            .results
            .iter()
            .any(|saved| saved.name == prior.name && saved.result == current)
            || prior
                .results
                .iter()
                .any(|saved| !binding.results.contains(saved))
            || binding.results.iter().any(|saved| {
                saved.result != current
                    && !prior.results.iter().any(|old| old.result == saved.result)
            })
        {
            return Err(invalid());
        }
    }
    Ok(())
}

enum PreparedResult {
    Scalar(ScalarValue),
    Host {
        effect: Effect,
        binding: Option<Box<ResultBinding>>,
        fuel: u64,
    },
}

fn materialize(image: &ContinuationImage, value: &Value) -> Result<PreparedResult, Fault> {
    let binding = image.result_binding.as_ref().ok_or_else(invalid)?;
    let mut fuel = image.fuel_remaining;
    let operation = leselang_hir::host_call::HostOperation::for_effect(&image.pending_effect)
        .ok_or_else(invalid)?;
    let (effect, binding) = match computation::resume_outcome(binding, value, operation, &mut fuel)?
    {
        computation::Outcome::Scalar(value) => return Ok(PreparedResult::Scalar(value)),
        computation::Outcome::Host(effect) => (effect, None),
        computation::Outcome::BoundHost { effect, binding } => (effect, Some(binding)),
        computation::Outcome::Result(_) => return Err(invalid()),
    };
    if fuel == 0 {
        return Err(Fault {
            code: "LSV1001".into(),
            message: "execution fuel exhausted before successor".into(),
        });
    }
    if leselang_hir::host_call::HostOperation::for_effect(&effect).is_none() {
        return Err(invalid());
    }
    Ok(PreparedResult::Host {
        effect: effect.into_owned(),
        binding,
        fuel,
    })
}

impl Vm {
    pub(super) fn complete_result(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        result: EffectResult,
    ) -> Step {
        let operation = lease.map(|(lease, _)| &lease.request.operation);
        let step = if matches!(
            image.schema_version,
            SUCCESSOR_CONTINUATION_SCHEMA_VERSION
                | DATAFLOW_CONTINUATION_SCHEMA_VERSION
                | CONDITIONAL_CONTINUATION_SCHEMA_VERSION
        ) {
            let raw = atomic_step_from_effect_result(image, operation, result);
            if let Step::Done(value) = raw {
                match self
                    .journal
                    .validate_result_output(image, &value)
                    .and_then(|()| materialize(image, &value))
                {
                    Ok(PreparedResult::Host {
                        effect,
                        binding,
                        fuel,
                    }) => {
                        return self.complete_successor(image, lease, value, effect, binding, fuel);
                    }
                    Ok(PreparedResult::Scalar(value)) => Step::Done(Value::Scalar { value }),
                    Err(error) => Step::Fault(error),
                }
            } else {
                raw
            }
        } else {
            step_from_effect_result(image, operation, result)
        };
        self.record_result(image, lease, step)
    }

    fn complete_successor(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        value: Value,
        effect: Effect,
        binding: Option<Box<ResultBinding>>,
        fuel: u64,
    ) -> Step {
        let parent = match self.journal.effect_request(&image.token) {
            Ok(request) if request.continuation == *image => request,
            Ok(_) => return Step::Fault(invalid()),
            Err(error) => return Step::Fault(error),
        };
        let sequence = match self.allocate_sequence() {
            Ok(sequence) => sequence,
            Err(error) => return Step::Fault(error),
        };
        let (principal, capabilities) = authority(&parent.operation);
        let Some(operation) = leselang_hir::host_call::HostOperation::for_effect(&effect) else {
            return self.record_result(image, lease, Step::Fault(invalid()));
        };
        let mut next = match Self::build_effect_request(
            &effect,
            operation.result_type(),
            sequence,
            principal.clone(),
            capabilities.clone(),
            image.expected_revision,
            image.deadline_ms,
            image.deadline_at_ms,
            fuel,
        ) {
            Ok(request) => request,
            Err(error) => return self.record_result(image, lease, Step::Fault(error)),
        };
        next.continuation.max_output_items = image.max_output_items;
        next.budget.max_output_items = image.max_output_items;
        if let Some(binding) = binding {
            next.continuation.schema_version = binding.schema_version();
            next.continuation.result_binding = Some(binding);
        }
        if let Err(error) = validate_effect_request(&next)
            .and_then(|()| validate_continuation_size(&next.continuation))
        {
            return self.record_result(image, lease, Step::Fault(error));
        }
        let step = Step::Done(value);
        let authoritative = match self.journal.record_successor(&parent, &step, &next, lease) {
            Ok(step) => step,
            Err(error) => return Step::Fault(error),
        };
        self.pending.remove(&image.token);
        self.completed
            .insert(image.token.clone(), authoritative.clone());
        // The journal may have accepted a competing acknowledgement, so discover its winner.
        self.visible_step(&image.token, authoritative)
    }

    fn record_result(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        step: Step,
    ) -> Step {
        let recorded = match lease {
            Some((lease, now_ms)) => self.journal.acknowledge_dispatch(lease, now_ms, &step),
            None => self.journal.record_completed(image, &step),
        };
        let authoritative = match recorded {
            Ok(step) => step,
            Err(error) => return Step::Fault(error),
        };
        self.pending.remove(&image.token);
        self.completed
            .insert(image.token.clone(), authoritative.clone());
        self.visible_step(&image.token, authoritative)
    }
}
