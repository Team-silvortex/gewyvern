//! Borrowed payload correspondence for Leserpent presentation requests.

use crate::PresentationOperation;
use leserpent_hir::{
    Effect, MAX_UI_CHILD_COUNT, UI_WAIT_ACCESSIBLE_DESCRIPTION_TIMEOUT_MS,
    UI_WAIT_ACCESSIBLE_NAME_TIMEOUT_MS, UI_WAIT_ACTION_AVAILABLE_TIMEOUT_MS,
    UI_WAIT_ACTION_KIND_TIMEOUT_MS, UI_WAIT_ACTION_LABEL_TIMEOUT_MS,
    UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS, UI_WAIT_AUTOMATION_ID_TIMEOUT_MS,
    UI_WAIT_CHILD_COUNT_TIMEOUT_MS, UI_WAIT_ENABLED_TIMEOUT_MS, UI_WAIT_FOCUSED_TIMEOUT_MS,
    UI_WAIT_FORM_FIELD_INPUT_KIND_TIMEOUT_MS, UI_WAIT_FORM_FIELD_MAX_LENGTH_TIMEOUT_MS,
    UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS, UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS,
    UI_WAIT_FORM_FIELD_TIMEOUT_MS, UI_WAIT_FORM_VALUE_TIMEOUT_MS, UI_WAIT_NODE_KIND_TIMEOUT_MS,
    UI_WAIT_REALIZED_TIMEOUT_MS, UI_WAIT_SELECTION_TIMEOUT_MS, UI_WAIT_TEXT_TIMEOUT_MS,
    UI_WAIT_UNFOCUSED_TIMEOUT_MS, UI_WAIT_VISIBLE_TIMEOUT_MS, UI_WAIT_WINDOW_CLOSED_TIMEOUT_MS,
    UI_WAIT_WINDOW_OPEN_TIMEOUT_MS, validate_ui_expected_text, validate_ui_form_field_key,
    validate_ui_form_value, validate_ui_node_id,
};

impl PresentationOperation {
    /// Check the pending GUI effect's original payload and fixed wait policy.
    ///
    /// This borrows both inputs without allocation, expansion or fuel consumption.
    /// Non-GUI effects return false. The caller must separately check the image,
    /// operation category, identity, capabilities and budget in its required order.
    /// Matching payloads neither authenticate an envelope nor admit a continuation.
    /// Existing request rules are preserved, including equality-only max-length
    /// checks; canonical source validation remains a separate restoration gate.
    pub fn matches_effect(&self, effect: &Effect) -> bool {
        match effect {
            Effect::UiActivate { node_id } => {
                matches!(
                    self,
                    PresentationOperation::Activate {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiFocus { node_id } => {
                matches!(
                    self,
                    PresentationOperation::Focus {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiNavigateFocus { node_id, direction } => {
                matches!(
                    self,
                    PresentationOperation::NavigateFocus {
                        node_id: operation_node_id,
                        direction: operation_direction,
                    } if operation_node_id == node_id
                        && operation_direction == direction
                        && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiScrollIntoView { node_id } => {
                matches!(
                    self,
                    PresentationOperation::ScrollIntoView {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiAssertVisible { node_id } => {
                matches!(
                    self,
                    PresentationOperation::AssertVisible {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiAssertHidden { node_id } => {
                matches!(
                    self,
                    PresentationOperation::AssertHidden {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiWaitHidden { node_id } => {
                matches!(
                    self,
                    PresentationOperation::WaitHidden {
                        node_id: operation_node_id,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_VISIBLE_TIMEOUT_MS
                )
            }
            Effect::UiAssertRealized { node_id } => {
                matches!(
                    self,
                    PresentationOperation::AssertRealized {
                        node_id: operation_node_id
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiWaitRealized { node_id } => {
                matches!(
                    self,
                    PresentationOperation::WaitRealized {
                        node_id: operation_node_id,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_REALIZED_TIMEOUT_MS
                )
            }
            Effect::UiWaitVisible { node_id } => {
                matches!(
                    self,
                    PresentationOperation::WaitVisible {
                        node_id: operation_node_id,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_VISIBLE_TIMEOUT_MS
                )
            }
            Effect::UiWaitEnabled { node_id } => {
                matches!(
                    self,
                    PresentationOperation::WaitEnabled {
                        node_id: operation_node_id,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_ENABLED_TIMEOUT_MS
                )
            }
            Effect::UiWaitDisabled { node_id } => {
                matches!(
                    self,
                    PresentationOperation::WaitDisabled {
                        node_id: operation_node_id,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_ENABLED_TIMEOUT_MS
                )
            }
            Effect::UiOpenWindow { node_id } => {
                matches!(
                    self,
                    PresentationOperation::OpenWindow {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiCloseWindow { node_id } => {
                matches!(
                    self,
                    PresentationOperation::CloseWindow {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiAssertWindowOpen { node_id } => {
                matches!(
                    self,
                    PresentationOperation::AssertWindowOpen {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id
                        && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiWaitWindowOpen { node_id } => {
                matches!(
                    self,
                    PresentationOperation::WaitWindowOpen {
                        node_id: operation_node_id,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_WINDOW_OPEN_TIMEOUT_MS
                )
            }
            Effect::UiAssertWindowClosed { node_id } => {
                matches!(
                    self,
                    PresentationOperation::AssertWindowClosed {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiWaitWindowClosed { node_id } => {
                matches!(
                    self,
                    PresentationOperation::WaitWindowClosed {
                        node_id: operation_node_id,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_WINDOW_CLOSED_TIMEOUT_MS
                )
            }
            Effect::UiWaitFocused { node_id } => {
                matches!(
                    self,
                    PresentationOperation::WaitFocused {
                        node_id: operation_node_id,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_FOCUSED_TIMEOUT_MS
                )
            }
            Effect::UiAssertFocused { node_id } => {
                matches!(
                    self,
                    PresentationOperation::AssertFocused {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiWaitUnfocused { node_id } => {
                matches!(
                    self,
                    PresentationOperation::WaitUnfocused {
                        node_id: operation_node_id,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_UNFOCUSED_TIMEOUT_MS
                )
            }
            Effect::UiAssertUnfocused { node_id } => {
                matches!(
                    self,
                    PresentationOperation::AssertUnfocused {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiAssertEnabled { node_id } => {
                matches!(
                    self,
                    PresentationOperation::AssertEnabled {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiAssertDisabled { node_id } => {
                matches!(
                    self,
                    PresentationOperation::AssertDisabled {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiAssertChildCount { node_id, count } => {
                matches!(
                    self,
                    PresentationOperation::AssertChildCount {
                        node_id: operation_node_id,
                        count: operation_count,
                    } if operation_node_id == node_id
                        && operation_count == count
                        && *operation_count <= MAX_UI_CHILD_COUNT
                        && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiWaitChildCount { node_id, count } => {
                matches!(
                    self,
                    PresentationOperation::WaitChildCount {
                        node_id: operation_node_id,
                        count: operation_count,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_count == count
                        && *operation_count <= MAX_UI_CHILD_COUNT
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_CHILD_COUNT_TIMEOUT_MS
                )
            }
            Effect::UiSetSelection { node_id, state } => {
                matches!(
                    self,
                    PresentationOperation::SetSelection {
                        node_id: operation_node_id,
                        state: operation_state,
                    } if operation_node_id == node_id
                        && operation_state == state
                        && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiAssertSelection { node_id, state } => {
                matches!(
                    self,
                    PresentationOperation::AssertSelection {
                        node_id: operation_node_id,
                        state: operation_state,
                    } if operation_node_id == node_id
                        && operation_state == state
                        && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiWaitSelection { node_id, state } => {
                matches!(
                    self,
                    PresentationOperation::WaitSelection {
                        node_id: operation_node_id,
                        state: operation_state,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_state == state
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_SELECTION_TIMEOUT_MS
                )
            }
            Effect::UiAssertText { node_id, expected } => {
                matches!(
                    self,
                    PresentationOperation::AssertText {
                        node_id: operation_node_id,
                        expected: operation_expected,
                    } if operation_node_id == node_id
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_expected_text(operation_expected)
                )
            }
            Effect::UiWaitText { node_id, expected } => {
                matches!(
                    self,
                    PresentationOperation::WaitText {
                        node_id: operation_node_id,
                        expected: operation_expected,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_expected_text(operation_expected)
                        && *timeout_ms == UI_WAIT_TEXT_TIMEOUT_MS
                )
            }
            Effect::UiAssertAutomationId { node_id, expected } => {
                matches!(
                    self,
                    PresentationOperation::AssertAutomationId {
                        node_id: operation_node_id,
                        expected: operation_expected,
                    } if operation_node_id == node_id
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_node_id(operation_expected)
                )
            }
            Effect::UiWaitAutomationId { node_id, expected } => {
                matches!(
                    self,
                    PresentationOperation::WaitAutomationId {
                        node_id: operation_node_id,
                        expected: operation_expected,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_node_id(operation_expected)
                        && *timeout_ms == UI_WAIT_AUTOMATION_ID_TIMEOUT_MS
                )
            }
            Effect::UiAssertNodeKind {
                node_id,
                expected_kind,
            } => {
                matches!(
                    self,
                    PresentationOperation::AssertNodeKind {
                        node_id: operation_node_id,
                        expected_kind: operation_expected_kind,
                    } if operation_node_id == node_id
                        && operation_expected_kind == expected_kind
                        && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiWaitNodeKind {
                node_id,
                expected_kind,
            } => {
                matches!(
                    self,
                    PresentationOperation::WaitNodeKind {
                        node_id: operation_node_id,
                        expected_kind: operation_expected_kind,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_expected_kind == expected_kind
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_NODE_KIND_TIMEOUT_MS
                )
            }
            Effect::UiAssertActionKind {
                node_id,
                expected_kind,
            } => {
                matches!(
                    self,
                    PresentationOperation::AssertActionKind {
                        node_id: operation_node_id,
                        expected_kind: operation_expected_kind,
                    } if operation_node_id == node_id
                        && operation_expected_kind == expected_kind
                        && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiWaitActionKind {
                node_id,
                expected_kind,
            } => {
                matches!(
                    self,
                    PresentationOperation::WaitActionKind {
                        node_id: operation_node_id,
                        expected_kind: operation_expected_kind,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_expected_kind == expected_kind
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_ACTION_KIND_TIMEOUT_MS
                )
            }
            Effect::UiAssertActionLabel { node_id, expected } => {
                matches!(
                    self,
                    PresentationOperation::AssertActionLabel {
                        node_id: operation_node_id,
                        expected: operation_expected,
                    } if operation_node_id == node_id
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_expected_text(operation_expected)
                )
            }
            Effect::UiWaitActionLabel { node_id, expected } => {
                matches!(
                    self,
                    PresentationOperation::WaitActionLabel {
                        node_id: operation_node_id,
                        expected: operation_expected,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_expected_text(operation_expected)
                        && *timeout_ms == UI_WAIT_ACTION_LABEL_TIMEOUT_MS
                )
            }
            Effect::UiAssertActionAvailable { node_id } => {
                matches!(
                    self,
                    PresentationOperation::AssertActionAvailable {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiWaitActionAvailable { node_id } => {
                matches!(
                    self,
                    PresentationOperation::WaitActionAvailable {
                        node_id: operation_node_id,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && validate_ui_node_id(operation_node_id)
                        && *timeout_ms == UI_WAIT_ACTION_AVAILABLE_TIMEOUT_MS
                )
            }
            Effect::UiAssertActionUnavailableReason { node_id, expected } => {
                matches!(
                    self,
                    PresentationOperation::AssertActionUnavailableReason {
                        node_id: operation_node_id,
                        expected: operation_expected,
                    } if operation_node_id == node_id
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && operation_expected
                            .as_deref()
                            .is_none_or(validate_ui_expected_text)
                )
            }
            Effect::UiWaitActionUnavailableReason { node_id, expected } => {
                matches!(
                    self,
                    PresentationOperation::WaitActionUnavailableReason {
                        node_id: operation_node_id,
                        expected: operation_expected,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_expected == expected
                        && *timeout_ms == UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS
                        && validate_ui_node_id(operation_node_id)
                        && operation_expected
                            .as_deref()
                            .is_none_or(validate_ui_expected_text)
                )
            }
            Effect::UiSubmitForm { node_id } => {
                matches!(
                    self,
                    PresentationOperation::SubmitForm {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiCancelForm { node_id } => {
                matches!(
                    self,
                    PresentationOperation::CancelForm {
                        node_id: operation_node_id,
                    } if operation_node_id == node_id && validate_ui_node_id(operation_node_id)
                )
            }
            Effect::UiSetFormValue {
                node_id,
                field,
                value,
            } => {
                matches!(
                    self,
                    PresentationOperation::SetFormValue {
                        node_id: operation_node_id,
                        field: operation_field,
                        value: operation_value,
                    } if operation_node_id == node_id
                        && operation_field == field
                        && operation_value == value
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_form_field_key(operation_field)
                        && validate_ui_form_value(operation_value)
                )
            }
            Effect::UiAssertFormValue {
                node_id,
                field,
                expected,
            } => {
                matches!(
                    self,
                    PresentationOperation::AssertFormValue {
                        node_id: operation_node_id,
                        field: operation_field,
                        expected: operation_expected,
                    } if operation_node_id == node_id
                        && operation_field == field
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_form_field_key(operation_field)
                        && validate_ui_form_value(operation_expected)
                )
            }
            Effect::UiWaitFormValue {
                node_id,
                field,
                expected,
            } => {
                matches!(
                    self,
                    PresentationOperation::WaitFormValue {
                        node_id: operation_node_id,
                        field: operation_field,
                        expected: operation_expected,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_field == field
                        && operation_expected == expected
                        && *timeout_ms == UI_WAIT_FORM_VALUE_TIMEOUT_MS
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_form_field_key(operation_field)
                        && validate_ui_form_value(operation_expected)
                )
            }
            Effect::UiAssertFormField {
                node_id,
                field,
                expected,
            } => {
                matches!(
                    self,
                    PresentationOperation::AssertFormField {
                        node_id: operation_node_id,
                        field: operation_field,
                        expected: operation_expected,
                    } if operation_node_id == node_id
                        && operation_field == field
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_form_field_key(operation_field)
                        && validate_ui_expected_text(operation_expected)
                )
            }
            Effect::UiWaitFormField {
                node_id,
                field,
                expected,
            } => {
                matches!(
                    self,
                    PresentationOperation::WaitFormField {
                        node_id: operation_node_id,
                        field: operation_field,
                        expected: operation_expected,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_field == field
                        && operation_expected == expected
                        && *timeout_ms == UI_WAIT_FORM_FIELD_TIMEOUT_MS
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_form_field_key(operation_field)
                        && validate_ui_expected_text(operation_expected)
                )
            }
            Effect::UiAssertFormFieldInputKind {
                node_id,
                field,
                input_kind,
            } => {
                matches!(
                    self,
                    PresentationOperation::AssertFormFieldInputKind {
                        node_id: operation_node_id,
                        field: operation_field,
                        input_kind: operation_input_kind,
                    } if operation_node_id == node_id
                        && operation_field == field
                        && operation_input_kind == input_kind
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_form_field_key(operation_field)
                )
            }
            Effect::UiWaitFormFieldInputKind {
                node_id,
                field,
                input_kind,
            } => {
                matches!(
                    self,
                    PresentationOperation::WaitFormFieldInputKind {
                        node_id: operation_node_id,
                        field: operation_field,
                        input_kind: operation_input_kind,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_field == field
                        && operation_input_kind == input_kind
                        && *timeout_ms == UI_WAIT_FORM_FIELD_INPUT_KIND_TIMEOUT_MS
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_form_field_key(operation_field)
                )
            }
            Effect::UiAssertFormFieldRequired {
                node_id,
                field,
                state,
            } => {
                matches!(
                    self,
                    PresentationOperation::AssertFormFieldRequired {
                        node_id: operation_node_id,
                        field: operation_field,
                        state: operation_state,
                    } if operation_node_id == node_id
                        && operation_field == field
                        && operation_state == state
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_form_field_key(operation_field)
                )
            }
            Effect::UiWaitFormFieldRequired {
                node_id,
                field,
                state,
            } => {
                matches!(
                    self,
                    PresentationOperation::WaitFormFieldRequired {
                        node_id: operation_node_id,
                        field: operation_field,
                        state: operation_state,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_field == field
                        && operation_state == state
                        && *timeout_ms == UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_form_field_key(operation_field)
                )
            }
            Effect::UiAssertFormFieldMaxLength {
                node_id,
                field,
                max_length,
            } => {
                matches!(
                    self,
                    PresentationOperation::AssertFormFieldMaxLength {
                        node_id: operation_node_id,
                        field: operation_field,
                        max_length: operation_max_length,
                    } if operation_node_id == node_id
                        && operation_field == field
                        && operation_max_length == max_length
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_form_field_key(operation_field)
                )
            }
            Effect::UiWaitFormFieldMaxLength {
                node_id,
                field,
                max_length,
            } => {
                matches!(
                    self,
                    PresentationOperation::WaitFormFieldMaxLength {
                        node_id: operation_node_id,
                        field: operation_field,
                        max_length: operation_max_length,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_field == field
                        && operation_max_length == max_length
                        && *timeout_ms == UI_WAIT_FORM_FIELD_MAX_LENGTH_TIMEOUT_MS
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_form_field_key(operation_field)
                )
            }
            Effect::UiAssertFormFieldPlaceholder {
                node_id,
                field,
                expected,
            } => {
                matches!(
                    self,
                    PresentationOperation::AssertFormFieldPlaceholder {
                        node_id: operation_node_id,
                        field: operation_field,
                        expected: operation_expected,
                    } if operation_node_id == node_id
                        && operation_field == field
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_form_field_key(operation_field)
                        && operation_expected
                            .as_deref()
                            .is_none_or(validate_ui_expected_text)
                )
            }
            Effect::UiWaitFormFieldPlaceholder {
                node_id,
                field,
                expected,
            } => {
                matches!(
                    self,
                    PresentationOperation::WaitFormFieldPlaceholder {
                        node_id: operation_node_id,
                        field: operation_field,
                        expected: operation_expected,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_field == field
                        && operation_expected == expected
                        && *timeout_ms == UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_form_field_key(operation_field)
                        && operation_expected
                            .as_deref()
                            .is_none_or(validate_ui_expected_text)
                )
            }
            Effect::UiAssertAccessibleName { node_id, expected } => {
                matches!(
                    self,
                    PresentationOperation::AssertAccessibleName {
                        node_id: operation_node_id,
                        expected: operation_expected,
                    } if operation_node_id == node_id
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_expected_text(operation_expected)
                )
            }
            Effect::UiWaitAccessibleName { node_id, expected } => {
                matches!(
                    self,
                    PresentationOperation::WaitAccessibleName {
                        node_id: operation_node_id,
                        expected: operation_expected,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_expected_text(operation_expected)
                        && *timeout_ms == UI_WAIT_ACCESSIBLE_NAME_TIMEOUT_MS
                )
            }
            Effect::UiAssertAccessibleDescription { node_id, expected } => {
                matches!(
                    self,
                    PresentationOperation::AssertAccessibleDescription {
                        node_id: operation_node_id,
                        expected: operation_expected,
                    } if operation_node_id == node_id
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_expected_text(operation_expected)
                )
            }
            Effect::UiWaitAccessibleDescription { node_id, expected } => {
                matches!(
                    self,
                    PresentationOperation::WaitAccessibleDescription {
                        node_id: operation_node_id,
                        expected: operation_expected,
                        timeout_ms,
                    } if operation_node_id == node_id
                        && operation_expected == expected
                        && validate_ui_node_id(operation_node_id)
                        && validate_ui_expected_text(operation_expected)
                        && *timeout_ms == UI_WAIT_ACCESSIBLE_DESCRIPTION_TIMEOUT_MS
                )
            }
            Effect::RuntimeList { .. }
            | Effect::RuntimeInspect { .. }
            | Effect::RuntimeHistory { .. }
            | Effect::RuntimeLogs { .. }
            | Effect::RuntimeRefresh { .. }
            | Effect::RuntimeCapabilitiesRefresh { .. }
            | Effect::RuntimeDeploy { .. }
            | Effect::DebuggerCancel { .. }
            | Effect::All { .. }
            | Effect::Sequence { .. }
            | Effect::Compute { .. } => false,
        }
    }
}
