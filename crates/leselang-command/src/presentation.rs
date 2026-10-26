//! Pure mapping of Leserpent presentation effects into host protocol payloads.

use leserpent_hir::{
    Effect, UI_WAIT_ACCESSIBLE_DESCRIPTION_TIMEOUT_MS, UI_WAIT_ACCESSIBLE_NAME_TIMEOUT_MS,
    UI_WAIT_ACTION_AVAILABLE_TIMEOUT_MS, UI_WAIT_ACTION_KIND_TIMEOUT_MS,
    UI_WAIT_ACTION_LABEL_TIMEOUT_MS, UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS,
    UI_WAIT_AUTOMATION_ID_TIMEOUT_MS, UI_WAIT_CHILD_COUNT_TIMEOUT_MS, UI_WAIT_ENABLED_TIMEOUT_MS,
    UI_WAIT_FOCUSED_TIMEOUT_MS, UI_WAIT_FORM_FIELD_INPUT_KIND_TIMEOUT_MS,
    UI_WAIT_FORM_FIELD_MAX_LENGTH_TIMEOUT_MS, UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS,
    UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS, UI_WAIT_FORM_FIELD_TIMEOUT_MS,
    UI_WAIT_FORM_VALUE_TIMEOUT_MS, UI_WAIT_NODE_KIND_TIMEOUT_MS, UI_WAIT_REALIZED_TIMEOUT_MS,
    UI_WAIT_SELECTION_TIMEOUT_MS, UI_WAIT_TEXT_TIMEOUT_MS, UI_WAIT_UNFOCUSED_TIMEOUT_MS,
    UI_WAIT_VISIBLE_TIMEOUT_MS, UI_WAIT_WINDOW_CLOSED_TIMEOUT_MS, UI_WAIT_WINDOW_OPEN_TIMEOUT_MS,
};

use crate::PresentationOperation;

impl PresentationOperation {
    /// Copy a presentation effect into its host payload with the fixed wait policy.
    ///
    /// This is data conversion, not validation or dispatch. The caller must bound
    /// and validate the effect, authenticate the principal and grants, and retain
    /// the pending-operation/revision/budget checks before sending or accepting
    /// a reply. No identity, continuation, fuel meter or journal is created here.
    /// Non-presentation effects return None and must use their own execution path.
    pub fn from_effect(effect: &Effect) -> Option<Self> {
        match effect {
            Effect::UiActivate { node_id } => Some(Self::Activate {
                node_id: node_id.clone(),
            }),
            Effect::UiFocus { node_id } => Some(Self::Focus {
                node_id: node_id.clone(),
            }),
            Effect::UiNavigateFocus { node_id, direction } => Some(Self::NavigateFocus {
                node_id: node_id.clone(),
                direction: *direction,
            }),
            Effect::UiScrollIntoView { node_id } => Some(Self::ScrollIntoView {
                node_id: node_id.clone(),
            }),
            Effect::UiAssertVisible { node_id } => Some(Self::AssertVisible {
                node_id: node_id.clone(),
            }),
            Effect::UiAssertHidden { node_id } => Some(Self::AssertHidden {
                node_id: node_id.clone(),
            }),
            Effect::UiWaitHidden { node_id } => Some(Self::WaitHidden {
                node_id: node_id.clone(),
                timeout_ms: UI_WAIT_VISIBLE_TIMEOUT_MS,
            }),
            Effect::UiAssertRealized { node_id } => Some(Self::AssertRealized {
                node_id: node_id.clone(),
            }),
            Effect::UiWaitRealized { node_id } => Some(Self::WaitRealized {
                node_id: node_id.clone(),
                timeout_ms: UI_WAIT_REALIZED_TIMEOUT_MS,
            }),
            Effect::UiWaitVisible { node_id } => Some(Self::WaitVisible {
                node_id: node_id.clone(),
                timeout_ms: UI_WAIT_VISIBLE_TIMEOUT_MS,
            }),
            Effect::UiWaitEnabled { node_id } => Some(Self::WaitEnabled {
                node_id: node_id.clone(),
                timeout_ms: UI_WAIT_ENABLED_TIMEOUT_MS,
            }),
            Effect::UiWaitDisabled { node_id } => Some(Self::WaitDisabled {
                node_id: node_id.clone(),
                timeout_ms: UI_WAIT_ENABLED_TIMEOUT_MS,
            }),
            Effect::UiOpenWindow { node_id } => Some(Self::OpenWindow {
                node_id: node_id.clone(),
            }),
            Effect::UiCloseWindow { node_id } => Some(Self::CloseWindow {
                node_id: node_id.clone(),
            }),
            Effect::UiAssertWindowOpen { node_id } => Some(Self::AssertWindowOpen {
                node_id: node_id.clone(),
            }),
            Effect::UiWaitWindowOpen { node_id } => Some(Self::WaitWindowOpen {
                node_id: node_id.clone(),
                timeout_ms: UI_WAIT_WINDOW_OPEN_TIMEOUT_MS,
            }),
            Effect::UiAssertWindowClosed { node_id } => Some(Self::AssertWindowClosed {
                node_id: node_id.clone(),
            }),
            Effect::UiWaitWindowClosed { node_id } => Some(Self::WaitWindowClosed {
                node_id: node_id.clone(),
                timeout_ms: UI_WAIT_WINDOW_CLOSED_TIMEOUT_MS,
            }),
            Effect::UiWaitFocused { node_id } => Some(Self::WaitFocused {
                node_id: node_id.clone(),
                timeout_ms: UI_WAIT_FOCUSED_TIMEOUT_MS,
            }),
            Effect::UiAssertFocused { node_id } => Some(Self::AssertFocused {
                node_id: node_id.clone(),
            }),
            Effect::UiWaitUnfocused { node_id } => Some(Self::WaitUnfocused {
                node_id: node_id.clone(),
                timeout_ms: UI_WAIT_UNFOCUSED_TIMEOUT_MS,
            }),
            Effect::UiAssertUnfocused { node_id } => Some(Self::AssertUnfocused {
                node_id: node_id.clone(),
            }),
            Effect::UiAssertEnabled { node_id } => Some(Self::AssertEnabled {
                node_id: node_id.clone(),
            }),
            Effect::UiAssertDisabled { node_id } => Some(Self::AssertDisabled {
                node_id: node_id.clone(),
            }),
            Effect::UiAssertChildCount { node_id, count } => Some(Self::AssertChildCount {
                node_id: node_id.clone(),
                count: *count,
            }),
            Effect::UiWaitChildCount { node_id, count } => Some(Self::WaitChildCount {
                node_id: node_id.clone(),
                count: *count,
                timeout_ms: UI_WAIT_CHILD_COUNT_TIMEOUT_MS,
            }),
            Effect::UiSetSelection { node_id, state } => Some(Self::SetSelection {
                node_id: node_id.clone(),
                state: *state,
            }),
            Effect::UiAssertSelection { node_id, state } => Some(Self::AssertSelection {
                node_id: node_id.clone(),
                state: *state,
            }),
            Effect::UiWaitSelection { node_id, state } => Some(Self::WaitSelection {
                node_id: node_id.clone(),
                state: *state,
                timeout_ms: UI_WAIT_SELECTION_TIMEOUT_MS,
            }),
            Effect::UiAssertText { node_id, expected } => Some(Self::AssertText {
                node_id: node_id.clone(),
                expected: expected.clone(),
            }),
            Effect::UiWaitText { node_id, expected } => Some(Self::WaitText {
                node_id: node_id.clone(),
                expected: expected.clone(),
                timeout_ms: UI_WAIT_TEXT_TIMEOUT_MS,
            }),
            Effect::UiAssertAutomationId { node_id, expected } => Some(Self::AssertAutomationId {
                node_id: node_id.clone(),
                expected: expected.clone(),
            }),
            Effect::UiWaitAutomationId { node_id, expected } => Some(Self::WaitAutomationId {
                node_id: node_id.clone(),
                expected: expected.clone(),
                timeout_ms: UI_WAIT_AUTOMATION_ID_TIMEOUT_MS,
            }),
            Effect::UiAssertNodeKind {
                node_id,
                expected_kind,
            } => Some(Self::AssertNodeKind {
                node_id: node_id.clone(),
                expected_kind: *expected_kind,
            }),
            Effect::UiWaitNodeKind {
                node_id,
                expected_kind,
            } => Some(Self::WaitNodeKind {
                node_id: node_id.clone(),
                expected_kind: *expected_kind,
                timeout_ms: UI_WAIT_NODE_KIND_TIMEOUT_MS,
            }),
            Effect::UiAssertActionKind {
                node_id,
                expected_kind,
            } => Some(Self::AssertActionKind {
                node_id: node_id.clone(),
                expected_kind: *expected_kind,
            }),
            Effect::UiWaitActionKind {
                node_id,
                expected_kind,
            } => Some(Self::WaitActionKind {
                node_id: node_id.clone(),
                expected_kind: *expected_kind,
                timeout_ms: UI_WAIT_ACTION_KIND_TIMEOUT_MS,
            }),
            Effect::UiAssertActionLabel { node_id, expected } => Some(Self::AssertActionLabel {
                node_id: node_id.clone(),
                expected: expected.clone(),
            }),
            Effect::UiWaitActionLabel { node_id, expected } => Some(Self::WaitActionLabel {
                node_id: node_id.clone(),
                expected: expected.clone(),
                timeout_ms: UI_WAIT_ACTION_LABEL_TIMEOUT_MS,
            }),
            Effect::UiAssertActionAvailable { node_id } => Some(Self::AssertActionAvailable {
                node_id: node_id.clone(),
            }),
            Effect::UiWaitActionAvailable { node_id } => Some(Self::WaitActionAvailable {
                node_id: node_id.clone(),
                timeout_ms: UI_WAIT_ACTION_AVAILABLE_TIMEOUT_MS,
            }),
            Effect::UiAssertActionUnavailableReason { node_id, expected } => {
                Some(Self::AssertActionUnavailableReason {
                    node_id: node_id.clone(),
                    expected: expected.clone(),
                })
            }
            Effect::UiWaitActionUnavailableReason { node_id, expected } => {
                Some(Self::WaitActionUnavailableReason {
                    node_id: node_id.clone(),
                    expected: expected.clone(),
                    timeout_ms: UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS,
                })
            }
            Effect::UiSubmitForm { node_id } => Some(Self::SubmitForm {
                node_id: node_id.clone(),
            }),
            Effect::UiCancelForm { node_id } => Some(Self::CancelForm {
                node_id: node_id.clone(),
            }),
            Effect::UiSetFormValue {
                node_id,
                field,
                value,
            } => Some(Self::SetFormValue {
                node_id: node_id.clone(),
                field: field.clone(),
                value: value.clone(),
            }),
            Effect::UiAssertFormValue {
                node_id,
                field,
                expected,
            } => Some(Self::AssertFormValue {
                node_id: node_id.clone(),
                field: field.clone(),
                expected: expected.clone(),
            }),
            Effect::UiWaitFormValue {
                node_id,
                field,
                expected,
            } => Some(Self::WaitFormValue {
                node_id: node_id.clone(),
                field: field.clone(),
                expected: expected.clone(),
                timeout_ms: UI_WAIT_FORM_VALUE_TIMEOUT_MS,
            }),
            Effect::UiAssertFormField {
                node_id,
                field,
                expected,
            } => Some(Self::AssertFormField {
                node_id: node_id.clone(),
                field: field.clone(),
                expected: expected.clone(),
            }),
            Effect::UiWaitFormField {
                node_id,
                field,
                expected,
            } => Some(Self::WaitFormField {
                node_id: node_id.clone(),
                field: field.clone(),
                expected: expected.clone(),
                timeout_ms: UI_WAIT_FORM_FIELD_TIMEOUT_MS,
            }),
            Effect::UiAssertFormFieldInputKind {
                node_id,
                field,
                input_kind,
            } => Some(Self::AssertFormFieldInputKind {
                node_id: node_id.clone(),
                field: field.clone(),
                input_kind: *input_kind,
            }),
            Effect::UiWaitFormFieldInputKind {
                node_id,
                field,
                input_kind,
            } => Some(Self::WaitFormFieldInputKind {
                node_id: node_id.clone(),
                field: field.clone(),
                input_kind: *input_kind,
                timeout_ms: UI_WAIT_FORM_FIELD_INPUT_KIND_TIMEOUT_MS,
            }),
            Effect::UiAssertFormFieldRequired {
                node_id,
                field,
                state,
            } => Some(Self::AssertFormFieldRequired {
                node_id: node_id.clone(),
                field: field.clone(),
                state: *state,
            }),
            Effect::UiWaitFormFieldRequired {
                node_id,
                field,
                state,
            } => Some(Self::WaitFormFieldRequired {
                node_id: node_id.clone(),
                field: field.clone(),
                state: *state,
                timeout_ms: UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS,
            }),
            Effect::UiAssertFormFieldMaxLength {
                node_id,
                field,
                max_length,
            } => Some(Self::AssertFormFieldMaxLength {
                node_id: node_id.clone(),
                field: field.clone(),
                max_length: *max_length,
            }),
            Effect::UiWaitFormFieldMaxLength {
                node_id,
                field,
                max_length,
            } => Some(Self::WaitFormFieldMaxLength {
                node_id: node_id.clone(),
                field: field.clone(),
                max_length: *max_length,
                timeout_ms: UI_WAIT_FORM_FIELD_MAX_LENGTH_TIMEOUT_MS,
            }),
            Effect::UiAssertFormFieldPlaceholder {
                node_id,
                field,
                expected,
            } => Some(Self::AssertFormFieldPlaceholder {
                node_id: node_id.clone(),
                field: field.clone(),
                expected: expected.clone(),
            }),
            Effect::UiWaitFormFieldPlaceholder {
                node_id,
                field,
                expected,
            } => Some(Self::WaitFormFieldPlaceholder {
                node_id: node_id.clone(),
                field: field.clone(),
                expected: expected.clone(),
                timeout_ms: UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS,
            }),
            Effect::UiAssertAccessibleName { node_id, expected } => {
                Some(Self::AssertAccessibleName {
                    node_id: node_id.clone(),
                    expected: expected.clone(),
                })
            }
            Effect::UiWaitAccessibleName { node_id, expected } => Some(Self::WaitAccessibleName {
                node_id: node_id.clone(),
                expected: expected.clone(),
                timeout_ms: UI_WAIT_ACCESSIBLE_NAME_TIMEOUT_MS,
            }),
            Effect::UiAssertAccessibleDescription { node_id, expected } => {
                Some(Self::AssertAccessibleDescription {
                    node_id: node_id.clone(),
                    expected: expected.clone(),
                })
            }
            Effect::UiWaitAccessibleDescription { node_id, expected } => {
                Some(Self::WaitAccessibleDescription {
                    node_id: node_id.clone(),
                    expected: expected.clone(),
                    timeout_ms: UI_WAIT_ACCESSIBLE_DESCRIPTION_TIMEOUT_MS,
                })
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
            | Effect::Compute { .. } => None,
        }
    }
}
