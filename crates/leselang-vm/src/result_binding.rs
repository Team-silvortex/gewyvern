use leselang_hir::computation::{Computation, MAX_SCALAR_STRING_BYTES, ScalarValue};
use leselang_hir::result_field::ResultField;
use leselang_hir::{Effect, MAX_BRANCH_NAME_BYTES, MAX_EFFECT_NESTING_DEPTH, canonical_source};
use serde::{Deserialize, Serialize};

use crate::{Fault, Value};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ScalarBinding {
    pub name: String,
    pub value: ScalarValue,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResultBinding {
    pub name: String,
    pub locals: Vec<ScalarBinding>,
    pub body: Computation,
}

pub(super) fn invalid() -> Fault {
    Fault {
        code: "LSV1405".into(),
        message: "invalid or oversized result-binding continuation".into(),
    }
}

impl ResultBinding {
    pub(super) fn validate_structure(&self) -> Result<(), Fault> {
        if self.locals.len() > MAX_EFFECT_NESTING_DEPTH
            || self.name.len() > MAX_BRANCH_NAME_BYTES
            || self.locals.iter().any(|local| {
                local.name.len() > MAX_BRANCH_NAME_BYTES
                    || matches!(&local.value, ScalarValue::String(value) if value.len() > MAX_SCALAR_STRING_BYTES)
            })
        {
            return Err(invalid());
        }
        self.body.validate_structure().map_err(|_| invalid())?;
        if !self.body.is_pure() {
            return Err(invalid());
        }
        Ok(())
    }

    pub(super) fn validate(&self, pending: &Effect) -> Result<(), Fault> {
        self.validate_structure()?;
        // Reconstruct the lexical scope for the normal type/canonical validator.
        // No saved environment can smuggle a second effect into result handling.
        let mut expression = Computation::Bind {
            name: self.name.clone(),
            value: Box::new(Computation::Host {
                effect: Box::new(pending.clone()),
            }),
            body: Box::new(self.body.clone()),
        };
        for local in self.locals.iter().rev() {
            expression = Computation::Bind {
                name: local.name.clone(),
                value: Box::new(Computation::Literal {
                    value: local.value.clone(),
                }),
                body: Box::new(expression),
            };
        }
        canonical_source(&Effect::Compute {
            expression: Box::new(expression),
        })
        .map_err(|_| invalid())?;
        Ok(())
    }
}

pub(super) fn project(value: &Value, field: ResultField) -> Result<ScalarValue, Fault> {
    use ResultField::*;
    match (field, value) {
        (
            Revision,
            Value::RuntimeList { revision, .. }
            | Value::RuntimeInspect { revision, .. }
            | Value::RuntimeHistory { revision, .. }
            | Value::RuntimeLogs { revision, .. },
        ) => Ok(ScalarValue::Integer(revision.0)),
        (Count, Value::RuntimeList { runtimes, .. }) => count(runtimes.len()),
        (Count, Value::RuntimeHistory { entries, .. }) => count(entries.len()),
        (Count, Value::RuntimeLogs { entries, .. }) => count(entries.len()),
        (
            Count,
            Value::UiAssertChildCount { count: value, .. }
            | Value::UiWaitChildCount { count: value, .. },
        )
        | (
            MaxLength,
            Value::UiAssertFormFieldMaxLength {
                max_length: value, ..
            }
            | Value::UiWaitFormFieldMaxLength {
                max_length: value, ..
            },
        ) => count(*value),
        (
            FocusedNodeId,
            Value::UiNavigateFocus {
                focused_node_id, ..
            },
        ) => Ok(ScalarValue::String(focused_node_id.clone())),
        (
            NodeId,
            Value::UiActivate { node_id }
            | Value::UiFocus { node_id }
            | Value::UiNavigateFocus { node_id, .. }
            | Value::UiScrollIntoView { node_id }
            | Value::UiAssertVisible { node_id }
            | Value::UiAssertHidden { node_id }
            | Value::UiWaitHidden { node_id }
            | Value::UiAssertRealized { node_id }
            | Value::UiWaitRealized { node_id }
            | Value::UiWaitVisible { node_id }
            | Value::UiWaitEnabled { node_id }
            | Value::UiWaitDisabled { node_id }
            | Value::UiOpenWindow { node_id }
            | Value::UiCloseWindow { node_id }
            | Value::UiAssertWindowOpen { node_id }
            | Value::UiWaitWindowOpen { node_id }
            | Value::UiAssertWindowClosed { node_id }
            | Value::UiWaitWindowClosed { node_id }
            | Value::UiWaitFocused { node_id }
            | Value::UiAssertFocused { node_id }
            | Value::UiWaitUnfocused { node_id }
            | Value::UiAssertUnfocused { node_id }
            | Value::UiAssertEnabled { node_id }
            | Value::UiAssertDisabled { node_id }
            | Value::UiAssertChildCount { node_id, .. }
            | Value::UiWaitChildCount { node_id, .. }
            | Value::UiSetSelection { node_id, .. }
            | Value::UiAssertSelection { node_id, .. }
            | Value::UiWaitSelection { node_id, .. }
            | Value::UiAssertText { node_id, .. }
            | Value::UiWaitText { node_id, .. }
            | Value::UiAssertAutomationId { node_id, .. }
            | Value::UiWaitAutomationId { node_id, .. }
            | Value::UiAssertNodeKind { node_id, .. }
            | Value::UiWaitNodeKind { node_id, .. }
            | Value::UiAssertActionKind { node_id, .. }
            | Value::UiWaitActionKind { node_id, .. }
            | Value::UiAssertActionLabel { node_id, .. }
            | Value::UiWaitActionLabel { node_id, .. }
            | Value::UiAssertActionAvailable { node_id }
            | Value::UiWaitActionAvailable { node_id }
            | Value::UiAssertActionUnavailableReason { node_id, .. }
            | Value::UiWaitActionUnavailableReason { node_id, .. }
            | Value::UiSubmitForm { node_id }
            | Value::UiCancelForm { node_id }
            | Value::UiSetFormValue { node_id, .. }
            | Value::UiAssertFormValue { node_id, .. }
            | Value::UiWaitFormValue { node_id, .. }
            | Value::UiAssertFormField { node_id, .. }
            | Value::UiWaitFormField { node_id, .. }
            | Value::UiAssertFormFieldInputKind { node_id, .. }
            | Value::UiWaitFormFieldInputKind { node_id, .. }
            | Value::UiAssertFormFieldRequired { node_id, .. }
            | Value::UiWaitFormFieldRequired { node_id, .. }
            | Value::UiAssertFormFieldMaxLength { node_id, .. }
            | Value::UiWaitFormFieldMaxLength { node_id, .. }
            | Value::UiAssertFormFieldPlaceholder { node_id, .. }
            | Value::UiWaitFormFieldPlaceholder { node_id, .. }
            | Value::UiAssertAccessibleName { node_id, .. }
            | Value::UiWaitAccessibleName { node_id, .. }
            | Value::UiAssertAccessibleDescription { node_id, .. }
            | Value::UiWaitAccessibleDescription { node_id, .. },
        ) => Ok(ScalarValue::String(node_id.clone())),
        _ => Err(invalid()),
    }
}

fn count(value: usize) -> Result<ScalarValue, Fault> {
    u64::try_from(value)
        .map(ScalarValue::Integer)
        .map_err(|_| invalid())
}
