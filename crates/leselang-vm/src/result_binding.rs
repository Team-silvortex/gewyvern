use leselang_hir::computation::{Computation, MAX_SCALAR_STRING_BYTES, ScalarValue};
use leselang_hir::host_call::HostOperation;
use leselang_hir::result_field::ResultField;
use leselang_hir::{Effect, MAX_BRANCH_NAME_BYTES, MAX_EFFECT_NESTING_DEPTH, Type};
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
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub results: Vec<ProjectedBinding>,
    pub body: Computation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedBinding {
    pub name: String,
    pub result: ProjectedResult,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedResult {
    pub operation: HostOperation,
    pub fields: Vec<ProjectedField>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProjectedField {
    pub field: ResultField,
    pub value: ScalarValue,
}

impl ProjectedResult {
    pub(super) fn capture(operation: HostOperation, value: &Value) -> Result<Self, Fault> {
        let fields = ResultField::ALL
            .into_iter()
            .filter(|field| field.result_type(operation.result_type()).is_some())
            .map(|field| {
                Ok(ProjectedField {
                    field,
                    value: project(value, field)?,
                })
            })
            .collect::<Result<Vec<_>, Fault>>()?;
        let result = Self { operation, fields };
        result.validate()?;
        Ok(result)
    }

    fn validate(&self) -> Result<(), Fault> {
        let expected = ResultField::ALL
            .into_iter()
            .filter_map(|field| {
                field
                    .result_type(self.operation.result_type())
                    .map(|ty| (field, ty))
            })
            .collect::<Vec<_>>();
        if self.fields.len() != expected.len() || self.fields.iter().zip(expected).any(|(stored, (field, ty))| {
            stored.field != field || stored.value.scalar_type() != ty
                || matches!(&stored.value, ScalarValue::String(value) if value.len() > MAX_SCALAR_STRING_BYTES)
        }) { return Err(invalid()); }
        Ok(())
    }

    pub(super) fn field(&self, field: ResultField) -> Result<ScalarValue, Fault> {
        self.fields
            .iter()
            .find(|stored| stored.field == field)
            .map(|stored| stored.value.clone())
            .ok_or_else(invalid)
    }
}

pub(super) fn invalid() -> Fault {
    Fault {
        code: "LSV1405".into(),
        message: "invalid or oversized result-binding continuation".into(),
    }
}

impl ResultBinding {
    pub(super) fn schema_version(&self) -> u32 {
        if !self.body.is_pure() && !self.body.is_result_chain() {
            crate::CONDITIONAL_CONTINUATION_SCHEMA_VERSION
        } else if !self.results.is_empty() || (!self.body.is_pure() && !self.body.is_atomic_tail())
        {
            crate::DATAFLOW_CONTINUATION_SCHEMA_VERSION
        } else if self.body.is_pure() {
            crate::RESULT_BINDING_CONTINUATION_SCHEMA_VERSION
        } else {
            crate::SUCCESSOR_CONTINUATION_SCHEMA_VERSION
        }
    }

    pub(super) fn validate_structure(&self) -> Result<(), Fault> {
        if self.locals.len().saturating_add(self.results.len()) > MAX_EFFECT_NESTING_DEPTH
            || self.name.len() > MAX_BRANCH_NAME_BYTES
            || self.locals.iter().any(|local| {
                local.name.len() > MAX_BRANCH_NAME_BYTES
                    || matches!(&local.value, ScalarValue::String(value) if value.len() > MAX_SCALAR_STRING_BYTES)
            })
        {
            return Err(invalid());
        }
        for result in &self.results {
            if result.name.len() > MAX_BRANCH_NAME_BYTES {
                return Err(invalid());
            }
            result.result.validate()?;
        }
        self.body.validate_structure().map_err(|_| invalid())?;
        if !self.body.is_result_flow() {
            return Err(invalid());
        }
        Ok(())
    }

    pub(super) fn validate(&self, pending: &Effect) -> Result<Type, Fault> {
        self.validate_structure()?;
        // Reconstruct the lexical scope for the normal type/canonical validator.
        // The normal type checker also fences groups and effectful operands.
        let mut expression = Computation::Bind {
            name: self.name.clone(),
            value: Box::new(Computation::Host {
                effect: Box::new(pending.clone()),
            }),
            body: Box::new(self.body.clone()),
        };
        if !self.results.is_empty() {
            let scope = self
                .locals
                .iter()
                .map(|local| (local.name.clone(), Type::Scalar(local.value.scalar_type())))
                .chain(
                    self.results
                        .iter()
                        .map(|result| (result.name.clone(), result.result.operation.result_type())),
                )
                .collect::<Vec<_>>();
            return expression.validate_in_scope(&scope).map_err(|_| invalid());
        }
        for local in self.locals.iter().rev() {
            expression = Computation::Bind {
                name: local.name.clone(),
                value: Box::new(Computation::Literal {
                    value: local.value.clone(),
                }),
                body: Box::new(expression),
            };
        }
        expression.validate_in_scope(&[]).map_err(|_| invalid())
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
