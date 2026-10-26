//! Borrowed leaf bounds and output-item accounting for released Leserpent values.

use leselang_host_contract::validate_debugger_session_id;
use leserpent_domain::MAX_RUNTIME_LOG_QUERY_ENTRIES;
use leserpent_hir::{
    MAX_UI_CHILD_COUNT, computation::ScalarValue, validate_ui_expected_text,
    validate_ui_form_field_key, validate_ui_form_value, validate_ui_node_id,
};

use crate::Value;

impl Value {
    /// Check the released leaf bounds and return its output-item count.
    ///
    /// None means an invalid leaf or a structured value requiring caller traversal,
    /// never an empty result. Empty queries count as zero; empty scalar lists count
    /// as one. The caller supplies the RuntimeList/RuntimeHistory ceiling; runtime
    /// logs retain their fixed product ceiling. No recursive traversal, allocation,
    /// authority check or receipt acceptance occurs here. Callers must still fence
    /// depth, field names, aggregate/per-execution items, encoded bytes and identity.
    /// This preserves existing leaf rules, not full domain-record validation or
    /// pending-operation correspondence (for example, saved navigation values may
    /// retain the same source and focused node).
    pub fn bounded_leaf_items(&self, max_query_items: usize) -> Option<usize> {
        match self {
            Value::Scalar { value } if value.is_bounded() => match value {
                ScalarValue::StringList(value) => Some(value.0.len().max(1)),
                _ => Some(1),
            },
            Value::RuntimeList { runtimes, .. } if runtimes.len() <= max_query_items => {
                Some(runtimes.len())
            }
            Value::RuntimeInspect { .. } => Some(1),
            Value::RuntimeHistory { entries, .. } if entries.len() <= max_query_items => {
                Some(entries.len())
            }
            Value::RuntimeLogs { entries, .. }
                if entries.len() <= usize::from(MAX_RUNTIME_LOG_QUERY_ENTRIES) =>
            {
                Some(entries.len())
            }
            Value::RuntimeRefresh { .. }
            | Value::RuntimeCapabilitiesRefresh { .. }
            | Value::RuntimeDeploy { .. } => Some(1),
            Value::DebuggerCancel { result }
                if validate_debugger_session_id(&result.session_id).is_ok()
                    && result.observed_at_ms <= i64::MAX as u64 =>
            {
                Some(1)
            }
            Value::UiActivate { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiFocus { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiNavigateFocus {
                node_id,
                focused_node_id,
                ..
            } if validate_ui_node_id(node_id) && validate_ui_node_id(focused_node_id) => Some(1),
            Value::UiScrollIntoView { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertVisible { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertHidden { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiWaitHidden { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertRealized { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiWaitRealized { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiWaitVisible { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiWaitEnabled { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiWaitDisabled { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiOpenWindow { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiCloseWindow { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertWindowOpen { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiWaitWindowOpen { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertWindowClosed { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiWaitWindowClosed { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiWaitFocused { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertFocused { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiWaitUnfocused { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertUnfocused { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertEnabled { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertDisabled { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertChildCount { node_id, count }
                if validate_ui_node_id(node_id) && *count <= MAX_UI_CHILD_COUNT =>
            {
                Some(1)
            }
            Value::UiWaitChildCount { node_id, count }
                if validate_ui_node_id(node_id) && *count <= MAX_UI_CHILD_COUNT =>
            {
                Some(1)
            }
            Value::UiSetSelection { node_id, .. } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertSelection { node_id, .. } if validate_ui_node_id(node_id) => Some(1),
            Value::UiWaitSelection { node_id, .. } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertText { node_id, expected }
                if validate_ui_node_id(node_id) && validate_ui_expected_text(expected) =>
            {
                Some(1)
            }
            Value::UiWaitText { node_id, expected }
                if validate_ui_node_id(node_id) && validate_ui_expected_text(expected) =>
            {
                Some(1)
            }
            Value::UiAssertAutomationId { node_id, expected }
                if validate_ui_node_id(node_id) && validate_ui_node_id(expected) =>
            {
                Some(1)
            }
            Value::UiWaitAutomationId { node_id, expected }
                if validate_ui_node_id(node_id) && validate_ui_node_id(expected) =>
            {
                Some(1)
            }
            Value::UiAssertNodeKind { node_id, .. } if validate_ui_node_id(node_id) => Some(1),
            Value::UiWaitNodeKind { node_id, .. } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertActionKind { node_id, .. } if validate_ui_node_id(node_id) => Some(1),
            Value::UiWaitActionKind { node_id, .. } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertActionLabel { node_id, expected }
                if validate_ui_node_id(node_id) && validate_ui_expected_text(expected) =>
            {
                Some(1)
            }
            Value::UiWaitActionLabel { node_id, expected }
                if validate_ui_node_id(node_id) && validate_ui_expected_text(expected) =>
            {
                Some(1)
            }
            Value::UiAssertActionAvailable { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiWaitActionAvailable { node_id } if validate_ui_node_id(node_id) => Some(1),
            Value::UiAssertActionUnavailableReason { node_id, expected }
                if validate_ui_node_id(node_id)
                    && expected.as_deref().is_none_or(validate_ui_expected_text) =>
            {
                Some(1)
            }
            Value::UiWaitActionUnavailableReason { node_id, expected }
                if validate_ui_node_id(node_id)
                    && expected.as_deref().is_none_or(validate_ui_expected_text) =>
            {
                Some(1)
            }
            Value::UiSubmitForm { node_id } | Value::UiCancelForm { node_id }
                if validate_ui_node_id(node_id) =>
            {
                Some(1)
            }
            Value::UiSetFormValue {
                node_id,
                field,
                value,
            } if validate_ui_node_id(node_id)
                && validate_ui_form_field_key(field)
                && validate_ui_form_value(value) =>
            {
                Some(1)
            }
            Value::UiAssertFormValue {
                node_id,
                field,
                expected,
            }
            | Value::UiWaitFormValue {
                node_id,
                field,
                expected,
            } if validate_ui_node_id(node_id)
                && validate_ui_form_field_key(field)
                && validate_ui_form_value(expected) =>
            {
                Some(1)
            }
            Value::UiAssertFormField {
                node_id,
                field,
                expected,
            } if validate_ui_node_id(node_id)
                && validate_ui_form_field_key(field)
                && validate_ui_expected_text(expected) =>
            {
                Some(1)
            }
            Value::UiWaitFormField {
                node_id,
                field,
                expected,
            } if validate_ui_node_id(node_id)
                && validate_ui_form_field_key(field)
                && validate_ui_expected_text(expected) =>
            {
                Some(1)
            }
            Value::UiAssertFormFieldInputKind { node_id, field, .. }
                if validate_ui_node_id(node_id) && validate_ui_form_field_key(field) =>
            {
                Some(1)
            }
            Value::UiWaitFormFieldInputKind { node_id, field, .. }
                if validate_ui_node_id(node_id) && validate_ui_form_field_key(field) =>
            {
                Some(1)
            }
            Value::UiAssertFormFieldRequired { node_id, field, .. }
                if validate_ui_node_id(node_id) && validate_ui_form_field_key(field) =>
            {
                Some(1)
            }
            Value::UiWaitFormFieldRequired { node_id, field, .. }
                if validate_ui_node_id(node_id) && validate_ui_form_field_key(field) =>
            {
                Some(1)
            }
            Value::UiAssertFormFieldMaxLength { node_id, field, .. }
                if validate_ui_node_id(node_id) && validate_ui_form_field_key(field) =>
            {
                Some(1)
            }
            Value::UiWaitFormFieldMaxLength { node_id, field, .. }
                if validate_ui_node_id(node_id) && validate_ui_form_field_key(field) =>
            {
                Some(1)
            }
            Value::UiAssertFormFieldPlaceholder {
                node_id,
                field,
                expected,
            } if validate_ui_node_id(node_id)
                && validate_ui_form_field_key(field)
                && expected.as_deref().is_none_or(validate_ui_expected_text) =>
            {
                Some(1)
            }
            Value::UiWaitFormFieldPlaceholder {
                node_id,
                field,
                expected,
            } if validate_ui_node_id(node_id)
                && validate_ui_form_field_key(field)
                && expected.as_deref().is_none_or(validate_ui_expected_text) =>
            {
                Some(1)
            }
            Value::UiAssertAccessibleName { node_id, expected }
                if validate_ui_node_id(node_id) && validate_ui_expected_text(expected) =>
            {
                Some(1)
            }
            Value::UiWaitAccessibleName { node_id, expected }
                if validate_ui_node_id(node_id) && validate_ui_expected_text(expected) =>
            {
                Some(1)
            }
            Value::UiAssertAccessibleDescription { node_id, expected }
                if validate_ui_node_id(node_id) && validate_ui_expected_text(expected) =>
            {
                Some(1)
            }
            Value::UiWaitAccessibleDescription { node_id, expected }
                if validate_ui_node_id(node_id) && validate_ui_expected_text(expected) =>
            {
                Some(1)
            }
            _ => None,
        }
    }
}
