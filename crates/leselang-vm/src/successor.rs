use super::*;
use leselang_hir::host_call::HostOperation;
use leselang_runtime_core::{
    CaptureExtensionBindings, CaptureExtensionDomain, CaptureExtensionError, CurrentCapture,
    validate_capture_extension,
};

struct NativeCaptureExtension<'a> {
    prior: &'a ResultBinding,
    effect: &'a Effect,
    value: &'a Value,
    operation: Option<HostOperation>,
}

impl CaptureExtensionDomain<ProjectedGroupBinding, ProjectedBinding>
    for NativeCaptureExtension<'_>
{
    type Error = Fault;

    fn same_group_data(
        &mut self,
        old: &ProjectedGroupBinding,
        saved: &ProjectedGroupBinding,
    ) -> bool {
        old.group == saved.group
    }
    fn same_group_binding(
        &mut self,
        old: &ProjectedGroupBinding,
        saved: &ProjectedGroupBinding,
    ) -> bool {
        old == saved
    }
    fn begin_results(&mut self) -> Result<(), Fault> {
        self.operation = Some(HostOperation::for_effect(self.effect).ok_or_else(invalid)?);
        Ok(())
    }
    fn current_result(&mut self, saved: &ProjectedBinding) -> Result<CurrentCapture, Fault> {
        // Keep exact legacy projections: replay must not synthesize absent fields.
        let matches_result = saved
            .result
            .matches_capture(self.operation.ok_or_else(invalid)?, self.value)?;
        Ok(CurrentCapture {
            matches_result,
            matches_binding: saved.name == self.prior.name,
        })
    }
    fn same_result_data(&mut self, old: &ProjectedBinding, saved: &ProjectedBinding) -> bool {
        old.result == saved.result
    }
    fn same_result_binding(&mut self, old: &ProjectedBinding, saved: &ProjectedBinding) -> bool {
        old == saved
    }
}

fn validate_extension(
    prior: &ResultBinding,
    binding: &ResultBinding,
    effect: &Effect,
    value: &Value,
) -> Result<(), Fault> {
    validate_capture_extension(
        CaptureExtensionBindings {
            groups: &prior.groups,
            results: &prior.results,
        },
        CaptureExtensionBindings {
            groups: &binding.groups,
            results: &binding.results,
        },
        leselang_hir::MAX_EFFECT_NESTING_DEPTH,
        NativeCaptureExtension {
            prior,
            effect,
            value,
            operation: None,
        },
    )
    .map_err(|error| match error {
        CaptureExtensionError::Native { error, .. } => error,
        _ => invalid(),
    })
}

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

pub(super) fn same_inherited_context(original: &EffectRequest, saved: &EffectRequest) -> bool {
    leselang_runtime_core::validate_inherited_context(
        &original.continuation.resource_budget(),
        &saved.continuation.resource_budget(),
        || {
            Ok::<_, std::convert::Infallible>(
                authority(&original.operation) == authority(&saved.operation),
            )
        },
        || Ok(original.continuation.expected_revision == saved.continuation.expected_revision),
    )
    .is_ok()
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
        GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
        | GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION => {
            second.schema_version == first.schema_version
                && first.group_result.is_some()
                && first.group_result == second.group_result
        }
        _ => false,
    };
    if !schema_valid
        || first
            .result_binding
            .as_ref()
            .is_none_or(|binding| binding.body.is_pure())
        || !same_inherited_context(parent, next)
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
        validate_extension(prior, binding, &first.pending_effect, value)?;
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
    let mut fuel = Fuel::new(image.fuel_remaining);
    let operation = leselang_hir::host_call::HostOperation::for_effect(&image.pending_effect)
        .ok_or_else(invalid)?;
    let (effect, binding) = match computation::resume_outcome(binding, value, operation, &mut fuel)?
    {
        computation::Outcome::Scalar(value) => return Ok(PreparedResult::Scalar(value)),
        computation::Outcome::Host(effect) => (effect, None),
        computation::Outcome::BoundHost { effect, binding } => (effect, Some(binding)),
        computation::Outcome::Result => return Err(invalid()),
    };
    if fuel.remaining() == 0 {
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
        fuel: fuel.remaining(),
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
        } else if matches!(
            image.schema_version,
            GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION
                | PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION
                | GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
                | GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION
        ) {
            atomic_step_from_effect_result(image, operation, result)
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

#[cfg(test)]
mod extension_tests {
    use super::*;

    #[test]
    fn inherited_context_keeps_frozen_native_agreement_across_operation_domains() {
        fn frozen(original: &EffectRequest, saved: &EffectRequest) -> bool {
            authority(&original.operation) == authority(&saved.operation)
                && original.continuation.expected_revision == saved.continuation.expected_revision
                && original.continuation.deadline_ms == saved.continuation.deadline_ms
                && original.continuation.deadline_at_ms == saved.continuation.deadline_at_ms
                && original.continuation.max_output_items == saved.continuation.max_output_items
        }
        let mut vm = Vm::new(500);
        let requests = [
            r#"fn main() = ui.focus(node_id: "a")"#,
            "fn main() = runtime.list()",
        ]
        .map(|source| {
            let Step::Effect(request) = vm.start(
                &leselang_hir::lower(&leselang_syntax::parse(source)).unwrap(),
                Principal::new("operator").unwrap(),
                CapabilitySet::new(["ui.presentation", "runtime.read"]),
                Some(Revision(7)),
            ) else {
                panic!("effect expected")
            };
            *request
        });
        for original in &requests {
            for template in &requests {
                for mutation in 0..9 {
                    let mut saved = template.clone();
                    match mutation {
                        0 => {}
                        1 | 2 => {
                            let (principal, capabilities) = match &mut saved.operation {
                                EffectOperation::Presentation(e) => {
                                    (&mut e.principal, &mut e.capabilities)
                                }
                                EffectOperation::Query(e) => {
                                    (&mut e.principal, &mut e.capabilities)
                                }
                                EffectOperation::Command(e) => {
                                    (&mut e.principal, &mut e.capabilities)
                                }
                            };
                            if mutation == 1 {
                                *principal = Principal::new("other").unwrap();
                            } else {
                                *capabilities = CapabilitySet::new([
                                    "ui.presentation",
                                    "runtime.read",
                                    "extra.read",
                                ]);
                            }
                        }
                        3 => saved.continuation.expected_revision = None,
                        4 => saved.continuation.deadline_ms = 1,
                        5 => saved.continuation.deadline_at_ms = Some(1),
                        6 => saved.continuation.max_output_items = 0,
                        7 => saved.continuation.fuel_remaining = 0,
                        8 => saved.continuation.schema_version = u32::MAX,
                        _ => unreachable!(),
                    }
                    saved.budget = saved.continuation.resource_budget();
                    assert_eq!(
                        same_inherited_context(original, &saved),
                        frozen(original, &saved),
                        "mutation {mutation}"
                    );
                }
            }
        }
    }

    // Frozen pre-extraction algorithm, including its native error precedence.
    fn frozen(
        prior: &ResultBinding,
        binding: &ResultBinding,
        effect: &Effect,
        value: &Value,
    ) -> Result<(), Fault> {
        if binding
            .groups
            .iter()
            .any(|group| !prior.groups.iter().any(|old| old.group == group.group))
            || prior.groups.iter().any(|old| !binding.groups.contains(old))
        {
            return Err(invalid());
        }
        let operation = HostOperation::for_effect(effect).ok_or_else(invalid)?;
        let mut contains_current = false;
        for saved in &binding.results {
            let current = saved.result.matches_capture(operation, value)?;
            contains_current |= saved.name == prior.name && current;
            if !current && !prior.results.iter().any(|old| old.result == saved.result) {
                return Err(invalid());
            }
        }
        if !contains_current
            || prior
                .results
                .iter()
                .any(|saved| !binding.results.contains(saved))
        {
            return Err(invalid());
        }
        Ok(())
    }

    #[test]
    fn capture_extension_preserves_frozen_native_alias_retention_and_late_fault_order() {
        fn effect(step: Step) -> EffectRequest {
            let Step::Effect(request) = step else {
                panic!("effect expected")
            };
            *request
        }
        let source = r#"fn main() = bind(first: ui.navigate_focus(node_id: "a", direction: "next"), body:
            bind(alias: first, body: bind(second: ui.focus(node_id: field(value: first, name: "focused_node_id")), body:
                bind(third: ui.focus(node_id: field(value: second, name: "node_id")), body: field(value: third, name: "node_id")))))"#;
        let mut vm = Vm::new(500);
        let first = effect(vm.start(
            &leselang_hir::lower(&leselang_syntax::parse(source)).unwrap(),
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            Some(Revision(7)),
        ));
        let second = effect(vm.resume(
            &first.continuation,
            EffectResult::Presentation(PresentationResult::NavigateFocus {
                node_id: "a".into(),
                direction: UiFocusNavigationDirection::Next,
                focused_node_id: "b".into(),
            }),
        ));
        let third = effect(vm.resume(
            &second.continuation,
            EffectResult::Presentation(PresentationResult::Focus {
                node_id: "b".into(),
            }),
        ));
        let value = Value::UiFocus {
            node_id: "b".into(),
        };
        validate_pair(&second, &Step::Done(value.clone()), &third).unwrap();
        let prior = second.continuation.result_binding.as_ref().unwrap();
        let saved = third.continuation.result_binding.as_ref().unwrap();
        for mutation in 0..10 {
            let mut original = (**prior).clone();
            let mut next = (**saved).clone();
            let group = ProjectedGroupBinding {
                name: "g".into(),
                group: ProjectedGroup {
                    members: vec![original.results[0].clone()],
                },
            };
            original.groups.push(group.clone());
            next.groups.push(group);
            match mutation {
                0 => {}
                1 => {
                    let mut alias = next.results[0].clone();
                    alias.name = "extra".into();
                    next.results.push(alias);
                }
                2 => {
                    next.results[1].name = "replacement".into();
                }
                3 => {
                    next.results.remove(1);
                }
                4 => {
                    next.results[2].name = "not_second".into();
                }
                5 => {
                    next.results[0].result.fields[0].value = ScalarValue::String("invented".into());
                }
                6 => {
                    let mut bad = next.results[2].clone();
                    bad.name = "bad".into();
                    bad.result.projection_version = u32::MAX;
                    next.results.push(bad);
                }
                7 => {
                    next.groups[0].name = "group_alias".into();
                }
                8 => {
                    next.groups.clear();
                }
                9 => {
                    next.groups[0].group.members[0].result.fields[0].value =
                        ScalarValue::String("invented".into());
                }
                _ => unreachable!(),
            }
            let expected = frozen(
                &original,
                &next,
                &second.continuation.pending_effect,
                &value,
            );
            assert_eq!(
                validate_extension(
                    &original,
                    &next,
                    &second.continuation.pending_effect,
                    &value
                ),
                expected,
                "mutation {mutation}"
            );
            if mutation == 6 {
                assert_eq!(expected.unwrap_err().code, "LSV1405");
            }
        }
    }
}
