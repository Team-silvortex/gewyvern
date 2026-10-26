use super::*;
#[cfg(test)]
use crate::result_binding::ReferenceResultBinding;

#[cfg(test)]
#[path = "successor_pair_contract_tests.rs"]
mod pair_contract_tests;

#[cfg(test)]
#[path = "completion_contract_tests.rs"]
mod completion_contract_tests;

#[cfg(test)]
#[path = "successor_assembly_contract_tests.rs"]
mod assembly_contract_tests;

#[cfg(test)]
#[path = "settlement_contract_tests.rs"]
mod settlement_contract_tests;

#[cfg(test)]
fn validate_extension(
    prior: &ResultBinding,
    binding: &ResultBinding,
    effect: &Effect,
    value: &Value,
) -> Result<(), Fault> {
    leselang_command::validate_result_binding_extension(
        prior,
        binding,
        effect,
        value,
        leserpent_hir::MAX_EFFECT_NESTING_DEPTH,
    )
    .map_err(|error| match error {
        leselang_command::CaptureReplayError::InvalidProjection => crate::result_binding::invalid(),
        leselang_command::CaptureReplayError::Mismatch => invalid(),
    })
}

pub(super) use leselang_command::reference_reply::CompletionLease;

pub(super) fn invalid() -> Fault {
    Fault {
        code: "LSV1407".into(),
        message: "invalid result-driven successor or missing original effect authority".into(),
    }
}

#[cfg(test)]
pub(super) fn authority(operation: &EffectOperation) -> (&Principal, &CapabilitySet) {
    operation.authority()
}

#[cfg(test)]
pub(super) fn same_inherited_context(original: &EffectRequest, saved: &EffectRequest) -> bool {
    original
        .as_view()
        .has_same_inherited_context(&saved.as_view())
}

struct SuccessorHost;

impl leselang_command::SuccessorAdmissionHost<ContinuationToken> for SuccessorHost {
    type Error = Fault;

    fn group_owner_text<'a>(&self, token: &'a ContinuationToken) -> &'a str {
        token.as_str()
    }

    fn valid_token(&self, token: &ContinuationToken) -> bool {
        valid_continuation_token(token)
    }

    fn canonical_sequence(&self, token: &ContinuationToken) -> Option<u64> {
        canonical_continuation_sequence(token)
    }

    fn valid_group_name(&self, name: &str) -> bool {
        crate::valid_merge_branch_name(name)
    }

    fn validate_completed_value(
        &self,
        parent: leselang_command::EffectRequestView<'_, ContinuationToken>,
        value: &Value,
    ) -> Result<(), Fault> {
        validate_bound_value(parent.continuation, value)
    }
}

pub(super) fn validate_pair(
    parent: &EffectRequest,
    result: &Step,
    next: &EffectRequest,
) -> Result<(), Fault> {
    leselang_command::validate_successor_pair(
        parent.as_view(),
        next.as_view(),
        match result {
            Step::Done(value) => Some(value),
            _ => None,
        },
        continuation_admission_limits(),
        &SuccessorHost,
    )
    .map_err(|error| match error {
        leselang_command::SuccessorAdmissionError::Parent(error)
        | leselang_command::SuccessorAdmissionError::Next(error) => request_admission_fault(error),
        leselang_command::SuccessorAdmissionError::Completion => invalid(),
        leselang_command::SuccessorAdmissionError::Value(error) => error,
        leselang_command::SuccessorAdmissionError::Correspondence(error) => match error {
            leselang_command::SuccessorCorrespondenceError::Mismatch => invalid(),
            leselang_command::SuccessorCorrespondenceError::InvalidBinding => {
                crate::result_binding::invalid()
            }
        },
    })
}

struct ReferenceCompletionOwner<'a>(&'a mut Vm);

impl leselang_command::reference_result_completion::ReferenceCompletionHost
    for ReferenceCompletionOwner<'_>
{
    fn atomic_step_from_effect_result(
        &mut self,
        image: &ContinuationImage,
        operation: Option<&EffectOperation>,
        result: EffectResult,
    ) -> Step {
        atomic_step_from_effect_result(image, operation, result)
    }
    fn step_from_effect_result(
        &mut self,
        image: &ContinuationImage,
        operation: Option<&EffectOperation>,
        result: EffectResult,
    ) -> Step {
        step_from_effect_result(image, operation, result)
    }
    fn validate_result_output(
        &mut self,
        image: &ContinuationImage,
        value: &Value,
    ) -> Result<(), Fault> {
        self.0.journal.validate_result_output(image, value)
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
        self.0
            .complete_successor(image, lease, value, effect, binding, fuel)
    }
    fn record_result(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        step: Step,
    ) -> Step {
        self.0.record_result(image, lease, step)
    }
}

struct ReferenceSuccessorOwner<'a>(&'a mut Vm);

impl leselang_command::reference_successor::ReferenceSuccessorHost for ReferenceSuccessorOwner<'_> {
    fn effect_request(&mut self, token: &ContinuationToken) -> Result<EffectRequest, Fault> {
        self.0.journal.effect_request(token)
    }
    fn allocate_sequence(&mut self) -> Result<u64, Fault> {
        self.0.allocate_sequence()
    }
    fn build_effect_request(
        &mut self,
        input: leselang_command::reference_successor::ReservedEffectInput<'_>,
    ) -> Result<EffectRequest, Fault> {
        Vm::build_effect_request(
            input.effect,
            input.result_type,
            input.sequence,
            input.principal,
            input.capabilities,
            input.expected_revision,
            input.deadline_ms,
            input.deadline_at_ms,
            input.fuel_remaining,
        )
    }
    fn record_result(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        step: Step,
    ) -> Step {
        self.0.record_result(image, lease, step)
    }
    fn commit_successor(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        parent: &EffectRequest,
        step: &Step,
        next: &EffectRequest,
    ) -> Step {
        self.0.commit_successor(image, lease, parent, step, next)
    }
}

struct ReferenceSettlementOwner<'a>(&'a mut Vm);

impl leselang_command::reference_settlement::ReferenceSettlementHost
    for ReferenceSettlementOwner<'_>
{
    fn record_completed(
        &mut self,
        image: &ContinuationImage,
        proposed: &Step,
    ) -> Result<Step, Fault> {
        self.0.journal.record_completed(image, proposed)
    }
    fn acknowledge_dispatch(
        &mut self,
        lease: &DispatchLease,
        now_ms: u64,
        proposed: &Step,
    ) -> Result<Step, Fault> {
        self.0.journal.acknowledge_dispatch(lease, now_ms, proposed)
    }
    fn record_successor(
        &mut self,
        parent: &EffectRequest,
        proposed: &Step,
        next: &EffectRequest,
        lease: CompletionLease<'_>,
    ) -> Result<Step, Fault> {
        self.0
            .journal
            .record_successor(parent, proposed, next, lease)
    }
    fn publish_completed(&mut self, image: &ContinuationImage, authoritative: Step) -> Step {
        self.0.publish_completed(image, authoritative)
    }
}

impl Vm {
    pub(super) fn complete_result(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        result: EffectResult,
    ) -> Step {
        leselang_command::reference_result_completion::complete_result(
            image,
            lease,
            result,
            &mut ReferenceCompletionOwner(self),
        )
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
        leselang_command::reference_successor::complete_successor(
            image,
            lease,
            value,
            effect,
            binding,
            fuel,
            &mut ReferenceSuccessorOwner(self),
        )
    }

    fn commit_successor(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        parent: &EffectRequest,
        step: &Step,
        next: &EffectRequest,
    ) -> Step {
        leselang_command::reference_settlement::commit_successor(
            image,
            lease,
            parent,
            step,
            next,
            &mut ReferenceSettlementOwner(self),
        )
    }

    fn record_result(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        step: Step,
    ) -> Step {
        leselang_command::reference_settlement::record_result(
            image,
            lease,
            step,
            &mut ReferenceSettlementOwner(self),
        )
    }

    pub(super) fn publish_completed(
        &mut self,
        image: &ContinuationImage,
        authoritative: Step,
    ) -> Step {
        self.pending.remove(&image.token);
        self.completed
            .insert(image.token.clone(), authoritative.clone());
        // The journal may have accepted a competing acknowledgement, so discover its winner.
        self.visible_step(&image.token, authoritative)
    }
}

#[cfg(test)]
mod extension_tests {
    use super::*;
    use crate::result_binding::projection_fault;
    use leserpent_hir::UiFocusNavigationDirection;
    use leserpent_hir::host_call::HostOperation;

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
                &leserpent_hir::lower(&leselang_syntax::parse(source)).unwrap(),
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
            let current = saved
                .result
                .matches_capture(operation, value)
                .map_err(projection_fault)?;
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
            &leserpent_hir::lower(&leselang_syntax::parse(source)).unwrap(),
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
