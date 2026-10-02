use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, HirBranch, Type};
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
}

pub(super) fn invalid() -> Fault {
    Fault {
        code: "LSV1409".into(),
        message: "invalid group-result binding or missing complete group journal".into(),
    }
}

impl GroupResultBinding {
    fn branches(&self) -> Result<&[HirBranch], Fault> {
        match &self.pending {
            Effect::All { branches } | Effect::Sequence { steps: branches } => Ok(branches),
            _ => Err(invalid()),
        }
    }

    pub(super) fn validate(&self, plan: &MergePlan) -> Result<Type, Fault> {
        self.binding.validate_structure()?;
        let branches = self.branches()?;
        if branches.len() != plan.branches.len()
            || !self.binding.results.is_empty()
            || !self.binding.body.is_pure()
            || self.fuel_remaining > crate::MAX_EXECUTION_FUEL
            || !matches!(
                (&self.pending, plan.order),
                (Effect::All { .. }, ExecutionOrder::BoundParallel)
                    | (Effect::Sequence { .. }, ExecutionOrder::BoundSequential)
            )
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
        if !matches!(ty, Type::Scalar(_)) {
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
        self.validate(plan)?;
        let first = requests.first().ok_or_else(invalid)?.1;
        if requests.len() != plan.branches.len()
            || first
                .continuation
                .fuel_remaining
                .checked_sub((requests.len() - 1) as u64)
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
        Ok(())
    }

    pub(super) fn validate_member(&self, name: &str, value: &Value) -> Result<(), Fault> {
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

    pub(super) fn finish(&self, value: &Value) -> Step {
        let mut fuel = self.fuel_remaining;
        match crate::computation::resume_group(&self.binding, value, &mut fuel) {
            Ok(value) => Step::Done(Value::Scalar { value }),
            Err(error) => Step::Fault(error),
        }
    }
}

fn value_type(value: &Value) -> Type {
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
