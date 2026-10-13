//! Owned presentation reply projection for the released Leserpent value format.

use crate::{PresentationResult, Value};

impl From<PresentationResult> for Value {
    /// Move reply fields into their released value shape, omitting wait metadata.
    ///
    /// This does not check correspondence, authority, types or resource limits.
    /// Callers must validate the original reply before dropping its wait timeout.
    fn from(reply: PresentationResult) -> Self {
        match reply {
            PresentationResult::Activate { node_id } => Self::UiActivate { node_id },
            PresentationResult::Focus { node_id } => Self::UiFocus { node_id },
            PresentationResult::NavigateFocus {
                node_id,
                direction,
                focused_node_id,
            } => Self::UiNavigateFocus {
                node_id,
                direction,
                focused_node_id,
            },
            PresentationResult::ScrollIntoView { node_id } => Self::UiScrollIntoView { node_id },
            PresentationResult::AssertVisible { node_id } => Self::UiAssertVisible { node_id },
            PresentationResult::AssertHidden { node_id } => Self::UiAssertHidden { node_id },
            PresentationResult::WaitHidden {
                node_id,
                timeout_ms: _,
            } => Self::UiWaitHidden { node_id },
            PresentationResult::AssertRealized { node_id } => Self::UiAssertRealized { node_id },
            PresentationResult::WaitRealized {
                node_id,
                timeout_ms: _,
            } => Self::UiWaitRealized { node_id },
            PresentationResult::WaitVisible {
                node_id,
                timeout_ms: _,
            } => Self::UiWaitVisible { node_id },
            PresentationResult::WaitEnabled {
                node_id,
                timeout_ms: _,
            } => Self::UiWaitEnabled { node_id },
            PresentationResult::WaitDisabled {
                node_id,
                timeout_ms: _,
            } => Self::UiWaitDisabled { node_id },
            PresentationResult::OpenWindow { node_id } => Self::UiOpenWindow { node_id },
            PresentationResult::CloseWindow { node_id } => Self::UiCloseWindow { node_id },
            PresentationResult::AssertWindowOpen { node_id } => {
                Self::UiAssertWindowOpen { node_id }
            }
            PresentationResult::WaitWindowOpen {
                node_id,
                timeout_ms: _,
            } => Self::UiWaitWindowOpen { node_id },
            PresentationResult::AssertWindowClosed { node_id } => {
                Self::UiAssertWindowClosed { node_id }
            }
            PresentationResult::WaitWindowClosed {
                node_id,
                timeout_ms: _,
            } => Self::UiWaitWindowClosed { node_id },
            PresentationResult::WaitFocused {
                node_id,
                timeout_ms: _,
            } => Self::UiWaitFocused { node_id },
            PresentationResult::AssertFocused { node_id } => Self::UiAssertFocused { node_id },
            PresentationResult::WaitUnfocused {
                node_id,
                timeout_ms: _,
            } => Self::UiWaitUnfocused { node_id },
            PresentationResult::AssertUnfocused { node_id } => Self::UiAssertUnfocused { node_id },
            PresentationResult::AssertEnabled { node_id } => Self::UiAssertEnabled { node_id },
            PresentationResult::AssertDisabled { node_id } => Self::UiAssertDisabled { node_id },
            PresentationResult::AssertChildCount { node_id, count } => {
                Self::UiAssertChildCount { node_id, count }
            }
            PresentationResult::WaitChildCount {
                node_id,
                count,
                timeout_ms: _,
            } => Self::UiWaitChildCount { node_id, count },
            PresentationResult::SetSelection { node_id, state } => {
                Self::UiSetSelection { node_id, state }
            }
            PresentationResult::AssertSelection { node_id, state } => {
                Self::UiAssertSelection { node_id, state }
            }
            PresentationResult::WaitSelection {
                node_id,
                state,
                timeout_ms: _,
            } => Self::UiWaitSelection { node_id, state },
            PresentationResult::AssertText { node_id, expected } => {
                Self::UiAssertText { node_id, expected }
            }
            PresentationResult::WaitText {
                node_id,
                expected,
                timeout_ms: _,
            } => Self::UiWaitText { node_id, expected },
            PresentationResult::AssertAutomationId { node_id, expected } => {
                Self::UiAssertAutomationId { node_id, expected }
            }
            PresentationResult::WaitAutomationId {
                node_id,
                expected,
                timeout_ms: _,
            } => Self::UiWaitAutomationId { node_id, expected },
            PresentationResult::AssertNodeKind {
                node_id,
                expected_kind,
            } => Self::UiAssertNodeKind {
                node_id,
                expected_kind,
            },
            PresentationResult::WaitNodeKind {
                node_id,
                expected_kind,
                timeout_ms: _,
            } => Self::UiWaitNodeKind {
                node_id,
                expected_kind,
            },
            PresentationResult::AssertActionKind {
                node_id,
                expected_kind,
            } => Self::UiAssertActionKind {
                node_id,
                expected_kind,
            },
            PresentationResult::WaitActionKind {
                node_id,
                expected_kind,
                timeout_ms: _,
            } => Self::UiWaitActionKind {
                node_id,
                expected_kind,
            },
            PresentationResult::AssertActionLabel { node_id, expected } => {
                Self::UiAssertActionLabel { node_id, expected }
            }
            PresentationResult::WaitActionLabel {
                node_id,
                expected,
                timeout_ms: _,
            } => Self::UiWaitActionLabel { node_id, expected },
            PresentationResult::AssertActionAvailable { node_id } => {
                Self::UiAssertActionAvailable { node_id }
            }
            PresentationResult::WaitActionAvailable {
                node_id,
                timeout_ms: _,
            } => Self::UiWaitActionAvailable { node_id },
            PresentationResult::AssertActionUnavailableReason { node_id, expected } => {
                Self::UiAssertActionUnavailableReason { node_id, expected }
            }
            PresentationResult::WaitActionUnavailableReason {
                node_id,
                expected,
                timeout_ms: _,
            } => Self::UiWaitActionUnavailableReason { node_id, expected },
            PresentationResult::SubmitForm { node_id } => Self::UiSubmitForm { node_id },
            PresentationResult::CancelForm { node_id } => Self::UiCancelForm { node_id },
            PresentationResult::SetFormValue {
                node_id,
                field,
                value,
            } => Self::UiSetFormValue {
                node_id,
                field,
                value,
            },
            PresentationResult::AssertFormValue {
                node_id,
                field,
                expected,
            } => Self::UiAssertFormValue {
                node_id,
                field,
                expected,
            },
            PresentationResult::WaitFormValue {
                node_id,
                field,
                expected,
                timeout_ms: _,
            } => Self::UiWaitFormValue {
                node_id,
                field,
                expected,
            },
            PresentationResult::AssertFormField {
                node_id,
                field,
                expected,
            } => Self::UiAssertFormField {
                node_id,
                field,
                expected,
            },
            PresentationResult::WaitFormField {
                node_id,
                field,
                expected,
                timeout_ms: _,
            } => Self::UiWaitFormField {
                node_id,
                field,
                expected,
            },
            PresentationResult::AssertFormFieldInputKind {
                node_id,
                field,
                input_kind,
            } => Self::UiAssertFormFieldInputKind {
                node_id,
                field,
                input_kind,
            },
            PresentationResult::WaitFormFieldInputKind {
                node_id,
                field,
                input_kind,
                timeout_ms: _,
            } => Self::UiWaitFormFieldInputKind {
                node_id,
                field,
                input_kind,
            },
            PresentationResult::AssertFormFieldRequired {
                node_id,
                field,
                state,
            } => Self::UiAssertFormFieldRequired {
                node_id,
                field,
                state,
            },
            PresentationResult::WaitFormFieldRequired {
                node_id,
                field,
                state,
                timeout_ms: _,
            } => Self::UiWaitFormFieldRequired {
                node_id,
                field,
                state,
            },
            PresentationResult::AssertFormFieldMaxLength {
                node_id,
                field,
                max_length,
            } => Self::UiAssertFormFieldMaxLength {
                node_id,
                field,
                max_length,
            },
            PresentationResult::WaitFormFieldMaxLength {
                node_id,
                field,
                max_length,
                timeout_ms: _,
            } => Self::UiWaitFormFieldMaxLength {
                node_id,
                field,
                max_length,
            },
            PresentationResult::AssertFormFieldPlaceholder {
                node_id,
                field,
                expected,
            } => Self::UiAssertFormFieldPlaceholder {
                node_id,
                field,
                expected,
            },
            PresentationResult::WaitFormFieldPlaceholder {
                node_id,
                field,
                expected,
                timeout_ms: _,
            } => Self::UiWaitFormFieldPlaceholder {
                node_id,
                field,
                expected,
            },
            PresentationResult::AssertAccessibleName { node_id, expected } => {
                Self::UiAssertAccessibleName { node_id, expected }
            }
            PresentationResult::WaitAccessibleName {
                node_id,
                expected,
                timeout_ms: _,
            } => Self::UiWaitAccessibleName { node_id, expected },
            PresentationResult::AssertAccessibleDescription { node_id, expected } => {
                Self::UiAssertAccessibleDescription { node_id, expected }
            }
            PresentationResult::WaitAccessibleDescription {
                node_id,
                expected,
                timeout_ms: _,
            } => Self::UiWaitAccessibleDescription { node_id, expected },
        }
    }
}
