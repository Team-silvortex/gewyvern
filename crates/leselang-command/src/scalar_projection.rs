use leserpent_hir::computation::ScalarValue;
use leserpent_hir::result_field::ResultField;

use crate::Value;

impl Value {
    /// Copy one explicitly exported product field into an owned script scalar.
    ///
    /// This is a shape projection, not validation, admission, capture-version
    /// selection or metering. The caller owns all bounds and execution policy.
    /// Unsupported fields (including scalar/structured values) return None.
    pub fn project_scalar(&self, field: ResultField) -> Option<ScalarValue> {
        use ResultField::{
            Count, Expected, Field, FocusedNodeId, Kind, MaxLength, NodeId, Required, Revision,
            Selected,
        };
        match (field, self) {
            (
                ResultField::OptionalExpected,
                Value::UiAssertFormFieldPlaceholder { expected, .. }
                | Value::UiWaitFormFieldPlaceholder { expected, .. }
                | Value::UiAssertActionUnavailableReason { expected, .. }
                | Value::UiWaitActionUnavailableReason { expected, .. },
            ) => Some(ScalarValue::OptionalString(
                leserpent_hir::computation::OptionalStringValue(expected.clone()),
            )),
            (
                Kind,
                Value::UiAssertNodeKind { expected_kind, .. }
                | Value::UiWaitNodeKind { expected_kind, .. },
            ) => Some(ScalarValue::String(expected_kind.as_str().into())),
            (
                Kind,
                Value::UiAssertActionKind { expected_kind, .. }
                | Value::UiWaitActionKind { expected_kind, .. },
            ) => Some(ScalarValue::String(expected_kind.as_str().into())),
            (
                Kind,
                Value::UiAssertFormFieldInputKind { input_kind, .. }
                | Value::UiWaitFormFieldInputKind { input_kind, .. },
            ) => Some(ScalarValue::String(input_kind.as_str().into())),
            (
                Expected,
                Value::UiAssertText { expected, .. }
                | Value::UiWaitText { expected, .. }
                | Value::UiAssertAutomationId { expected, .. }
                | Value::UiWaitAutomationId { expected, .. }
                | Value::UiAssertActionLabel { expected, .. }
                | Value::UiWaitActionLabel { expected, .. }
                | Value::UiAssertFormValue { expected, .. }
                | Value::UiWaitFormValue { expected, .. }
                | Value::UiAssertFormField { expected, .. }
                | Value::UiWaitFormField { expected, .. }
                | Value::UiAssertAccessibleName { expected, .. }
                | Value::UiWaitAccessibleName { expected, .. }
                | Value::UiAssertAccessibleDescription { expected, .. }
                | Value::UiWaitAccessibleDescription { expected, .. },
            ) => Some(ScalarValue::String(expected.clone())),
            (
                Field,
                Value::UiSetFormValue { field, .. }
                | Value::UiAssertFormValue { field, .. }
                | Value::UiWaitFormValue { field, .. }
                | Value::UiAssertFormField { field, .. }
                | Value::UiWaitFormField { field, .. }
                | Value::UiAssertFormFieldInputKind { field, .. }
                | Value::UiWaitFormFieldInputKind { field, .. }
                | Value::UiAssertFormFieldRequired { field, .. }
                | Value::UiWaitFormFieldRequired { field, .. }
                | Value::UiAssertFormFieldMaxLength { field, .. }
                | Value::UiWaitFormFieldMaxLength { field, .. }
                | Value::UiAssertFormFieldPlaceholder { field, .. }
                | Value::UiWaitFormFieldPlaceholder { field, .. },
            ) => Some(ScalarValue::String(field.clone())),
            (ResultField::Value, Value::UiSetFormValue { value, .. }) => {
                Some(ScalarValue::String(value.clone()))
            }
            (
                Selected,
                Value::UiSetSelection { state, .. }
                | Value::UiAssertSelection { state, .. }
                | Value::UiWaitSelection { state, .. },
            ) => Some(ScalarValue::Boolean(
                *state == leserpent_hir::UiSelectionState::Selected,
            )),
            (
                Required,
                Value::UiAssertFormFieldRequired { state, .. }
                | Value::UiWaitFormFieldRequired { state, .. },
            ) => Some(ScalarValue::Boolean(
                *state == leserpent_hir::UiFormRequirementState::Required,
            )),
            (
                Revision,
                Value::RuntimeList { revision, .. }
                | Value::RuntimeInspect { revision, .. }
                | Value::RuntimeHistory { revision, .. }
                | Value::RuntimeLogs { revision, .. },
            ) => Some(ScalarValue::Integer(revision.0)),
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
            ) => Some(ScalarValue::String(focused_node_id.clone())),
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
            ) => Some(ScalarValue::String(node_id.clone())),
            _ => None,
        }
    }
}

fn count(value: usize) -> Option<ScalarValue> {
    u64::try_from(value).map(ScalarValue::Integer).ok()
}
