use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, HirBranch, Type};
use leselang_runtime_core::Fuel;
use serde::{Deserialize, Serialize};

use crate::{
    ContinuationToken, EffectRequest, ExecutionOrder, Fault, MergePlan, ResultBinding, Step, Value,
};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupResultBinding {
    pub pending: Effect,
    pub binding: Box<ResultBinding>,
    pub fuel_remaining: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub successor_sequence: Option<u64>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional_successor_sequences: Vec<u64>,
}

pub(super) enum CaptureProgress {
    Done(crate::ScalarValue),
    Pending(Box<EffectRequest>),
}

pub(super) fn invalid() -> Fault {
    Fault {
        code: "LSV1409".into(),
        message: "invalid group-result binding or missing complete group journal".into(),
    }
}

impl GroupResultBinding {
    pub(super) fn is_conditional(&self) -> bool {
        !self.binding.body.is_pure()
            && !self.binding.body.is_result_chain()
            && self.binding.body.is_result_flow()
    }

    pub(super) fn is_dataflow(&self) -> bool {
        self.binding.body.is_result_chain()
            && self
                .binding
                .body
                .atomic_chain_bound()
                .is_some_and(|bound| bound > 1)
    }

    pub(super) fn captures_successor(&self) -> bool {
        self.binding.body.is_atomic_capture() || self.is_dataflow() || self.is_conditional()
    }

    pub(super) fn reserved_sequences(&self) -> impl Iterator<Item = u64> + '_ {
        self.successor_sequence
            .into_iter()
            .chain(self.additional_successor_sequences.iter().copied())
    }

    fn continuation_schema(&self) -> u32 {
        if self.is_conditional() {
            crate::GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION
        } else if self.is_dataflow() {
            crate::GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION
        } else if self.successor_sequence.is_some() && matches!(self.pending, Effect::All { .. }) {
            crate::PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION
        } else if self.captures_successor() {
            crate::GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION
        } else if self.successor_sequence.is_some() {
            crate::GROUP_TAIL_CONTINUATION_SCHEMA_VERSION
        } else {
            crate::GROUP_RESULT_CONTINUATION_SCHEMA_VERSION
        }
    }

    pub(super) fn branches(&self) -> Result<&[HirBranch], Fault> {
        match &self.pending {
            Effect::All { branches } | Effect::Sequence { steps: branches } => Ok(branches),
            _ => Err(invalid()),
        }
    }

    pub(super) fn successor_name_at(&self, position: usize) -> Result<String, Fault> {
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

    pub(super) fn has_successor(&self, plan: &MergePlan) -> bool {
        self.successor_sequence.is_some()
            && self
                .branches()
                .is_ok_and(|branches| plan.branches.len() > branches.len())
    }

    pub(super) fn validate(&self, plan: &MergePlan) -> Result<Type, Fault> {
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

    pub(super) fn validate_requests(
        &self,
        owner: &ContinuationToken,
        plan: &MergePlan,
        requests: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault> {
        let final_type = self.validate(plan)?;
        let branches = self.branches()?;
        let first = requests.first().ok_or_else(invalid)?.1;
        if requests.len() != plan.branches.len()
            || first
                .continuation
                .fuel_remaining
                .checked_sub((self.branches()?.len() - 1) as u64)
                != Some(self.fuel_remaining)
        {
            return Err(invalid());
        }
        for (index, (branch, (name, request))) in self.branches()?.iter().zip(requests).enumerate()
        {
            let image = &request.continuation;
            let expected_fuel =
                first
                    .continuation
                    .fuel_remaining
                    .checked_sub(if plan.order.is_sequential() {
                        index as u64
                    } else {
                        0
                    });
            if *name != branch.name
                || image.pending_effect != branch.effect
                || image.result_type != branch.result_type
                || image.group_result.as_ref() != Some(owner)
                || image.result_binding.is_some()
                || image.schema_version != self.continuation_schema()
                || Some(image.fuel_remaining) != expected_fuel
                || crate::successor::authority(&request.operation)
                    != crate::successor::authority(&first.operation)
                || image.expected_revision != first.continuation.expected_revision
                || image.deadline_ms != first.continuation.deadline_ms
                || image.deadline_at_ms != first.continuation.deadline_at_ms
                || image.max_output_items != first.continuation.max_output_items
            {
                return Err(invalid());
            }
        }
        let mut previous_fuel = self.fuel_remaining;
        for (index, (name, request)) in requests.iter().skip(branches.len()).enumerate() {
            let image = &request.continuation;
            let sequence = self.reserved_sequences().nth(index).ok_or_else(invalid)?;
            if *name != self.successor_name_at(index)?
                || image.token.as_str() != format!("continuation-{sequence}")
                || image.schema_version != self.continuation_schema()
                || image.group_result.as_ref() != Some(owner)
                || HostOperation::for_effect(&image.pending_effect).is_none()
                || if self.captures_successor() {
                    image.result_binding.as_ref().is_none_or(|binding| {
                        !(binding.body.is_pure()
                            || (self.is_dataflow() && binding.body.is_result_chain())
                            || (self.is_conditional() && binding.body.is_result_flow()))
                            || binding.groups.is_empty()
                            || binding.validate(&image.pending_effect).ok() != Some(final_type)
                            || !capture_accepts_type_at(
                                &self.binding.body,
                                index,
                                image.result_type,
                            )
                            || !binding
                                .groups
                                .iter()
                                .any(|group| group.name == self.binding.name)
                            || binding
                                .groups
                                .iter()
                                .any(|group| !group.group.matches_signature(branches))
                    })
                } else {
                    image.result_binding.is_some() || image.result_type != final_type
                }
                || image.fuel_remaining >= previous_fuel
                || crate::successor::authority(&request.operation)
                    != crate::successor::authority(&first.operation)
                || image.expected_revision != first.continuation.expected_revision
                || image.deadline_ms != first.continuation.deadline_ms
                || image.deadline_at_ms != first.continuation.deadline_at_ms
                || image.max_output_items != first.continuation.max_output_items
            {
                return Err(invalid());
            }
            previous_fuel = image.fuel_remaining;
        }
        if let Some(sequence) = self.successor_sequence
            && requests
                .iter()
                .take(self.branches()?.len())
                .any(|(_, request)| {
                    crate::canonical_continuation_sequence(&request.continuation.token)
                        .is_none_or(|prefix| prefix >= sequence)
                })
        {
            return Err(invalid());
        }
        Ok(())
    }

    pub(super) fn validate_member(&self, name: &str, value: &Value) -> Result<(), Fault> {
        if let Some(index) = (0..self.reserved_sequences().count()).find(|index| {
            self.successor_name_at(*index)
                .is_ok_and(|candidate| candidate == name)
        }) {
            let accepted = if self.captures_successor() {
                capture_accepts_type_at(&self.binding.body, index, value_type(value))
            } else {
                value_type(value) == self.binding.validate(&self.pending)?
            };
            return if accepted { Ok(()) } else { Err(invalid()) };
        }
        let branch = self
            .branches()?
            .iter()
            .find(|branch| branch.name == name)
            .ok_or_else(invalid)?;
        if value_type(value) != branch.result_type {
            return Err(invalid());
        }
        Ok(())
    }

    pub(super) fn prepare_initial(
        &self,
        owner: &ContinuationToken,
        first: &EffectRequest,
        value: &Value,
    ) -> Result<CaptureProgress, Fault> {
        let mut fuel = Fuel::new(self.fuel_remaining);
        let (effect, binding) = match crate::computation::resume_group_outcome(
            &self.binding,
            value,
            self.branches()?,
            &mut fuel,
        )? {
            crate::computation::Outcome::Scalar(value) if self.is_conditional() => {
                return Ok(CaptureProgress::Done(value));
            }
            crate::computation::Outcome::Host(effect) if !self.captures_successor() => {
                (effect, None)
            }
            crate::computation::Outcome::BoundHost { effect, binding }
                if self.captures_successor() =>
            {
                (effect, Some(binding))
            }
            _ => return Err(invalid()),
        };
        if fuel.remaining() == 0 {
            return Err(Fault {
                code: "LSV1001".into(),
                message: "execution fuel exhausted before group successor".into(),
            });
        }
        self.build_successor(owner, first, &effect, binding, fuel.remaining(), 0)
            .map(|request| CaptureProgress::Pending(Box::new(request)))
    }

    fn build_successor(
        &self,
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
        let mut request = crate::Vm::build_effect_request(
            effect,
            operation.result_type(),
            self.reserved_sequences()
                .nth(position)
                .ok_or_else(invalid)?,
            principal.clone(),
            capabilities.clone(),
            image.expected_revision,
            image.deadline_ms,
            image.deadline_at_ms,
            fuel,
        )?;
        request.continuation.schema_version = self.continuation_schema();
        request.continuation.group_result = Some(owner.clone());
        request.continuation.result_binding = binding;
        request.continuation.max_output_items = image.max_output_items;
        request.budget.max_output_items = image.max_output_items;
        crate::validate_effect_request(&request)?;
        crate::validate_continuation_size(&request.continuation)?;
        Ok(request)
    }

    pub(super) fn advance_capture(
        &self,
        owner: &ContinuationToken,
        requests: &[EffectRequest],
        value: &Value,
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
        self.validate_saved_capture(request, prefix, &operations)?;
        let image = &request.continuation;
        let saved = image.result_binding.as_ref().ok_or_else(invalid)?;
        let operation = HostOperation::for_effect(&image.pending_effect).ok_or_else(invalid)?;
        let mut fuel = Fuel::new(image.fuel_remaining);
        match crate::computation::resume_outcome(saved, &tail.value, operation, &mut fuel)? {
            crate::computation::Outcome::Scalar(value) => Ok(CaptureProgress::Done(value)),
            crate::computation::Outcome::BoundHost { effect, binding }
                if self.is_dataflow() || self.is_conditional() =>
            {
                if fuel.remaining() == 0 {
                    return Err(Fault {
                        code: "LSV1001".into(),
                        message: "execution fuel exhausted before group successor".into(),
                    });
                }
                let next = self.build_successor(
                    owner,
                    requests.first().ok_or_else(invalid)?,
                    &effect,
                    Some(binding),
                    fuel.remaining(),
                    requests.len() - self.branches()?.len(),
                )?;
                crate::successor::validate_pair(
                    request,
                    &Step::Done((*tail.value).clone()),
                    &next,
                )?;
                Ok(CaptureProgress::Pending(Box::new(next)))
            }
            _ => Err(invalid()),
        }
    }

    pub(super) fn finish(&self, value: &Value) -> Step {
        let mut fuel = Fuel::new(self.fuel_remaining);
        match self.branches().and_then(|branches| {
            crate::computation::resume_group(&self.binding, value, branches, &mut fuel)
        }) {
            Ok(value) => Step::Done(Value::Scalar { value }),
            Err(error) => Step::Fault(error),
        }
    }

    pub(super) fn validate_saved_capture(
        &self,
        request: &EffectRequest,
        fields: &[crate::StructuredField],
        operations: &[HostOperation],
    ) -> Result<(), Fault> {
        let branches = self.branches()?;
        if fields.len() < branches.len() || fields.len() != operations.len() {
            return Err(invalid());
        }
        let binding = request
            .continuation
            .result_binding
            .as_ref()
            .ok_or_else(invalid)?;
        for group in &binding.groups {
            if !group
                .group
                .matches_members(branches, &fields[..branches.len()])?
            {
                return Err(invalid());
            }
        }
        for result in &binding.results {
            let mut matched = false;
            for (operation, field) in operations.iter().zip(fields) {
                matched |= result.result.matches_capture(*operation, &field.value)?;
            }
            if !matched {
                return Err(invalid());
            }
        }
        Ok(())
    }
}

fn capture_accepts_type_at(
    expression: &leselang_hir::computation::Computation,
    position: usize,
    ty: Type,
) -> bool {
    use leselang_hir::computation::Computation;
    match expression {
        Computation::Bind { value, body, .. } if value.is_pure() => {
            capture_accepts_type_at(body, position, ty)
        }
        Computation::Bind { body, .. } if position > 0 => {
            capture_accepts_type_at(body, position - 1, ty)
        }
        Computation::Bind { value, .. } => value
            .prepared_atomic_operation()
            .is_some_and(|operation| operation.result_type() == ty),
        Computation::Choose {
            then, otherwise, ..
        } => {
            capture_accepts_type_at(then, position, ty)
                || capture_accepts_type_at(otherwise, position, ty)
        }
        _ => false,
    }
}

pub(super) fn value_type(value: &Value) -> Type {
    macro_rules! atomic_types {
        ($($variant:ident),+ $(,)?) => {
            match value {
                $(Value::$variant { .. } => Type::$variant,)+
                Value::Scalar { value } => Type::Scalar(value.scalar_type()),
                Value::Structured { .. } => Type::Structured,
            }
        };
    }
    atomic_types!(
        RuntimeList,
        RuntimeInspect,
        RuntimeHistory,
        RuntimeLogs,
        RuntimeRefresh,
        RuntimeCapabilitiesRefresh,
        RuntimeDeploy,
        DebuggerCancel,
        UiActivate,
        UiFocus,
        UiNavigateFocus,
        UiScrollIntoView,
        UiAssertVisible,
        UiAssertHidden,
        UiWaitHidden,
        UiAssertRealized,
        UiWaitRealized,
        UiWaitVisible,
        UiWaitEnabled,
        UiWaitDisabled,
        UiOpenWindow,
        UiCloseWindow,
        UiAssertWindowOpen,
        UiWaitWindowOpen,
        UiAssertWindowClosed,
        UiWaitWindowClosed,
        UiWaitFocused,
        UiAssertFocused,
        UiWaitUnfocused,
        UiAssertUnfocused,
        UiAssertEnabled,
        UiAssertDisabled,
        UiAssertChildCount,
        UiWaitChildCount,
        UiSetSelection,
        UiAssertSelection,
        UiWaitSelection,
        UiAssertText,
        UiWaitText,
        UiAssertAutomationId,
        UiWaitAutomationId,
        UiAssertNodeKind,
        UiWaitNodeKind,
        UiAssertActionKind,
        UiWaitActionKind,
        UiAssertActionLabel,
        UiWaitActionLabel,
        UiAssertActionAvailable,
        UiWaitActionAvailable,
        UiAssertActionUnavailableReason,
        UiWaitActionUnavailableReason,
        UiSubmitForm,
        UiCancelForm,
        UiSetFormValue,
        UiAssertFormValue,
        UiWaitFormValue,
        UiAssertFormField,
        UiWaitFormField,
        UiAssertFormFieldInputKind,
        UiWaitFormFieldInputKind,
        UiAssertFormFieldRequired,
        UiWaitFormFieldRequired,
        UiAssertFormFieldMaxLength,
        UiWaitFormFieldMaxLength,
        UiAssertFormFieldPlaceholder,
        UiWaitFormFieldPlaceholder,
        UiAssertAccessibleName,
        UiWaitAccessibleName,
        UiAssertAccessibleDescription,
        UiWaitAccessibleDescription,
    )
}
