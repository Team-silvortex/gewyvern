//! Borrowed correspondence checks for Leserpent presentation replies.

use crate::{EffectOperation, PresentationEnvelope, PresentationOperation, PresentationResult};
use leselang_hir::{
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
    UI_WAIT_WINDOW_OPEN_TIMEOUT_MS, validate_ui_node_id,
};

impl PresentationResult {
    /// Check this reply against the pending effect and, when available, its envelope.
    ///
    /// This is product payload correspondence only, not authority or admission.
    /// The caller must validate the saved effect, result type, identity, capability,
    /// deadline and budget separately. `None` preserves legacy image-only resume;
    /// callers with a saved request must supply its original operation. All checks
    /// borrow their inputs and do not consume a reply, allocate, or dispatch work.
    pub fn matches_effect(&self, effect: &Effect, operation: Option<&EffectOperation>) -> bool {
        match (effect, self) {
            (
                Effect::UiActivate { node_id },
                PresentationResult::Activate {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::Activate {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiFocus { node_id },
                PresentationResult::Focus {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::Focus {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiNavigateFocus { node_id, direction },
                PresentationResult::NavigateFocus {
                    node_id: result_node_id,
                    direction: result_direction,
                    focused_node_id,
                },
            ) => {
                result_node_id == node_id
                    && result_direction == direction
                    && validate_ui_node_id(focused_node_id)
                    && focused_node_id != result_node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::NavigateFocus {
                                    node_id: operation_node_id,
                                    direction: operation_direction,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_direction == direction
                        )
                    })
            }
            (
                Effect::UiScrollIntoView { node_id },
                PresentationResult::ScrollIntoView {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::ScrollIntoView {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiAssertVisible { node_id },
                PresentationResult::AssertVisible {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertVisible {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiAssertHidden { node_id },
                PresentationResult::AssertHidden {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertHidden {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiWaitHidden { node_id },
                PresentationResult::WaitHidden {
                    node_id: result_node_id,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && *timeout_ms == UI_WAIT_VISIBLE_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitHidden {
                                    node_id: operation_node_id,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && *operation_timeout_ms == UI_WAIT_VISIBLE_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertRealized { node_id },
                PresentationResult::AssertRealized {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertRealized {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiWaitRealized { node_id },
                PresentationResult::WaitRealized {
                    node_id: result_node_id,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && *timeout_ms == UI_WAIT_REALIZED_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitRealized {
                                    node_id: operation_node_id,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && *operation_timeout_ms == UI_WAIT_REALIZED_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiWaitVisible { node_id },
                PresentationResult::WaitVisible {
                    node_id: result_node_id,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && *timeout_ms == UI_WAIT_VISIBLE_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitVisible {
                                    node_id: operation_node_id,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && *operation_timeout_ms == UI_WAIT_VISIBLE_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiWaitEnabled { node_id },
                PresentationResult::WaitEnabled {
                    node_id: result_node_id,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && *timeout_ms == UI_WAIT_ENABLED_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitEnabled {
                                    node_id: operation_node_id,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && *operation_timeout_ms == UI_WAIT_ENABLED_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiWaitDisabled { node_id },
                PresentationResult::WaitDisabled {
                    node_id: result_node_id,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && *timeout_ms == UI_WAIT_ENABLED_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitDisabled {
                                    node_id: operation_node_id,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && *operation_timeout_ms == UI_WAIT_ENABLED_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiOpenWindow { node_id },
                PresentationResult::OpenWindow {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::OpenWindow {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiCloseWindow { node_id },
                PresentationResult::CloseWindow {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::CloseWindow {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiAssertWindowOpen { node_id },
                PresentationResult::AssertWindowOpen {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertWindowOpen {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiWaitWindowOpen { node_id },
                PresentationResult::WaitWindowOpen {
                    node_id: result_node_id,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && *timeout_ms == UI_WAIT_WINDOW_OPEN_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitWindowOpen {
                                    node_id: operation_node_id,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && *operation_timeout_ms == UI_WAIT_WINDOW_OPEN_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertWindowClosed { node_id },
                PresentationResult::AssertWindowClosed {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertWindowClosed {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiWaitWindowClosed { node_id },
                PresentationResult::WaitWindowClosed {
                    node_id: result_node_id,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && *timeout_ms == UI_WAIT_WINDOW_CLOSED_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitWindowClosed {
                                    node_id: operation_node_id,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && *operation_timeout_ms == UI_WAIT_WINDOW_CLOSED_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiWaitFocused { node_id },
                PresentationResult::WaitFocused {
                    node_id: result_node_id,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && *timeout_ms == UI_WAIT_FOCUSED_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitFocused {
                                    node_id: operation_node_id,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && *operation_timeout_ms == UI_WAIT_FOCUSED_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertFocused { node_id },
                PresentationResult::AssertFocused {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertFocused {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiWaitUnfocused { node_id },
                PresentationResult::WaitUnfocused {
                    node_id: result_node_id,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && *timeout_ms == UI_WAIT_UNFOCUSED_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitUnfocused {
                                    node_id: operation_node_id,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && *operation_timeout_ms == UI_WAIT_UNFOCUSED_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertUnfocused { node_id },
                PresentationResult::AssertUnfocused {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertUnfocused {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiAssertEnabled { node_id },
                PresentationResult::AssertEnabled {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertEnabled {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiAssertDisabled { node_id },
                PresentationResult::AssertDisabled {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertDisabled {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiAssertChildCount { node_id, count },
                PresentationResult::AssertChildCount {
                    node_id: result_node_id,
                    count: result_count,
                },
            ) => {
                result_node_id == node_id
                    && result_count == count
                    && *result_count <= MAX_UI_CHILD_COUNT
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertChildCount {
                                    node_id: operation_node_id,
                                    count: operation_count,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_count == count
                        )
                    })
            }
            (
                Effect::UiWaitChildCount { node_id, count },
                PresentationResult::WaitChildCount {
                    node_id: result_node_id,
                    count: result_count,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && result_count == count
                    && *result_count <= MAX_UI_CHILD_COUNT
                    && *timeout_ms == UI_WAIT_CHILD_COUNT_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitChildCount {
                                    node_id: operation_node_id,
                                    count: operation_count,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_count == count
                                && *operation_timeout_ms == UI_WAIT_CHILD_COUNT_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiSetSelection { node_id, state },
                PresentationResult::SetSelection {
                    node_id: result_node_id,
                    state: result_state,
                },
            ) => {
                result_node_id == node_id
                    && result_state == state
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::SetSelection {
                                    node_id: operation_node_id,
                                    state: operation_state,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_state == state
                        )
                    })
            }
            (
                Effect::UiAssertSelection { node_id, state },
                PresentationResult::AssertSelection {
                    node_id: result_node_id,
                    state: result_state,
                },
            ) => {
                result_node_id == node_id
                    && result_state == state
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertSelection {
                                    node_id: operation_node_id,
                                    state: operation_state,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_state == state
                        )
                    })
            }
            (
                Effect::UiWaitSelection { node_id, state },
                PresentationResult::WaitSelection {
                    node_id: result_node_id,
                    state: result_state,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && result_state == state
                    && *timeout_ms == UI_WAIT_SELECTION_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitSelection {
                                    node_id: operation_node_id,
                                    state: operation_state,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_state == state
                                && *operation_timeout_ms == UI_WAIT_SELECTION_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertText { node_id, expected },
                PresentationResult::AssertText {
                    node_id: result_node_id,
                    expected: result_expected,
                },
            ) => {
                result_node_id == node_id
                    && result_expected == expected
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertText {
                                    node_id: operation_node_id,
                                    expected: operation_expected,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected == expected
                        )
                    })
            }
            (
                Effect::UiWaitText { node_id, expected },
                PresentationResult::WaitText {
                    node_id: result_node_id,
                    expected: result_expected,
                    timeout_ms: result_timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && result_expected == expected
                    && *result_timeout_ms == UI_WAIT_TEXT_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitText {
                                    node_id: operation_node_id,
                                    expected: operation_expected,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected == expected
                                && *operation_timeout_ms == UI_WAIT_TEXT_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertAutomationId { node_id, expected },
                PresentationResult::AssertAutomationId {
                    node_id: result_node_id,
                    expected: result_expected,
                },
            ) => {
                result_node_id == node_id
                    && result_expected == expected
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertAutomationId {
                                    node_id: operation_node_id,
                                    expected: operation_expected,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected == expected
                        )
                    })
            }
            (
                Effect::UiWaitAutomationId { node_id, expected },
                PresentationResult::WaitAutomationId {
                    node_id: result_node_id,
                    expected: result_expected,
                    timeout_ms: result_timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && result_expected == expected
                    && *result_timeout_ms == UI_WAIT_AUTOMATION_ID_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitAutomationId {
                                    node_id: operation_node_id,
                                    expected: operation_expected,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected == expected
                                && *operation_timeout_ms == UI_WAIT_AUTOMATION_ID_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertNodeKind {
                    node_id,
                    expected_kind,
                },
                PresentationResult::AssertNodeKind {
                    node_id: result_node_id,
                    expected_kind: result_expected_kind,
                },
            ) => {
                result_node_id == node_id
                    && result_expected_kind == expected_kind
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertNodeKind {
                                    node_id: operation_node_id,
                                    expected_kind: operation_expected_kind,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected_kind == expected_kind
                        )
                    })
            }
            (
                Effect::UiWaitNodeKind {
                    node_id,
                    expected_kind,
                },
                PresentationResult::WaitNodeKind {
                    node_id: result_node_id,
                    expected_kind: result_expected_kind,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && result_expected_kind == expected_kind
                    && *timeout_ms == UI_WAIT_NODE_KIND_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitNodeKind {
                                    node_id: operation_node_id,
                                    expected_kind: operation_expected_kind,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected_kind == expected_kind
                                && *operation_timeout_ms == UI_WAIT_NODE_KIND_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertActionKind {
                    node_id,
                    expected_kind,
                },
                PresentationResult::AssertActionKind {
                    node_id: result_node_id,
                    expected_kind: result_expected_kind,
                },
            ) => {
                result_node_id == node_id
                    && result_expected_kind == expected_kind
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertActionKind {
                                    node_id: operation_node_id,
                                    expected_kind: operation_expected_kind,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected_kind == expected_kind
                        )
                    })
            }
            (
                Effect::UiWaitActionKind {
                    node_id,
                    expected_kind,
                },
                PresentationResult::WaitActionKind {
                    node_id: result_node_id,
                    expected_kind: result_expected_kind,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && result_expected_kind == expected_kind
                    && *timeout_ms == UI_WAIT_ACTION_KIND_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitActionKind {
                                    node_id: operation_node_id,
                                    expected_kind: operation_expected_kind,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected_kind == expected_kind
                                && *operation_timeout_ms == UI_WAIT_ACTION_KIND_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertActionLabel { node_id, expected },
                PresentationResult::AssertActionLabel {
                    node_id: result_node_id,
                    expected: result_expected,
                },
            ) => {
                result_node_id == node_id
                    && result_expected == expected
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertActionLabel {
                                    node_id: operation_node_id,
                                    expected: operation_expected,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected == expected
                        )
                    })
            }
            (
                Effect::UiWaitActionLabel { node_id, expected },
                PresentationResult::WaitActionLabel {
                    node_id: result_node_id,
                    expected: result_expected,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && result_expected == expected
                    && *timeout_ms == UI_WAIT_ACTION_LABEL_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitActionLabel {
                                    node_id: operation_node_id,
                                    expected: operation_expected,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected == expected
                                && *operation_timeout_ms == UI_WAIT_ACTION_LABEL_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertActionAvailable { node_id },
                PresentationResult::AssertActionAvailable {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertActionAvailable {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiWaitActionAvailable { node_id },
                PresentationResult::WaitActionAvailable {
                    node_id: result_node_id,
                    timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && *timeout_ms == UI_WAIT_ACTION_AVAILABLE_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitActionAvailable {
                                    node_id: operation_node_id,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && *operation_timeout_ms == UI_WAIT_ACTION_AVAILABLE_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertActionUnavailableReason { node_id, expected },
                PresentationResult::AssertActionUnavailableReason {
                    node_id: result_node_id,
                    expected: result_expected,
                },
            ) => {
                result_node_id == node_id
                    && result_expected == expected
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertActionUnavailableReason {
                                    node_id: operation_node_id,
                                    expected: operation_expected,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected == expected
                        )
                    })
            }
            (
                Effect::UiWaitActionUnavailableReason { node_id, expected },
                PresentationResult::WaitActionUnavailableReason {
                    node_id: result_node_id,
                    expected: result_expected,
                    timeout_ms,
                },
            ) => result_node_id == node_id
                && result_expected == expected
                && *timeout_ms == UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS
                && operation.is_none_or(|operation| {
                    matches!(
                        operation,
                        EffectOperation::Presentation(PresentationEnvelope {
                            operation: PresentationOperation::WaitActionUnavailableReason {
                                node_id: operation_node_id,
                                expected: operation_expected,
                                timeout_ms: operation_timeout_ms,
                            },
                            ..
                        }) if operation_node_id == node_id
                            && operation_expected == expected
                            && *operation_timeout_ms == UI_WAIT_ACTION_UNAVAILABLE_REASON_TIMEOUT_MS
                    )
                }),
            (
                Effect::UiSubmitForm { node_id },
                PresentationResult::SubmitForm {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::SubmitForm {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiCancelForm { node_id },
                PresentationResult::CancelForm {
                    node_id: result_node_id,
                },
            ) => {
                result_node_id == node_id
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::CancelForm {
                                    node_id: operation_node_id,
                                },
                                ..
                            }) if operation_node_id == node_id
                        )
                    })
            }
            (
                Effect::UiSetFormValue {
                    node_id,
                    field,
                    value,
                },
                PresentationResult::SetFormValue {
                    node_id: result_node_id,
                    field: result_field,
                    value: result_value,
                },
            ) => {
                result_node_id == node_id
                    && result_field == field
                    && result_value == value
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::SetFormValue {
                                    node_id: operation_node_id,
                                    field: operation_field,
                                    value: operation_value,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_field == field
                                && operation_value == value
                        )
                    })
            }
            (
                Effect::UiAssertFormValue {
                    node_id,
                    field,
                    expected,
                },
                PresentationResult::AssertFormValue {
                    node_id: result_node_id,
                    field: result_field,
                    expected: result_expected,
                },
            ) => {
                result_node_id == node_id
                    && result_field == field
                    && result_expected == expected
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertFormValue {
                                    node_id: operation_node_id,
                                    field: operation_field,
                                    expected: operation_expected,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_field == field
                                && operation_expected == expected
                        )
                    })
            }
            (
                Effect::UiWaitFormValue {
                    node_id,
                    field,
                    expected,
                },
                PresentationResult::WaitFormValue {
                    node_id: result_node_id,
                    field: result_field,
                    expected: result_expected,
                    timeout_ms: result_timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && result_field == field
                    && result_expected == expected
                    && *result_timeout_ms == UI_WAIT_FORM_VALUE_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitFormValue {
                                    node_id: operation_node_id,
                                    field: operation_field,
                                    expected: operation_expected,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_field == field
                                && operation_expected == expected
                                && *operation_timeout_ms == UI_WAIT_FORM_VALUE_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertFormField {
                    node_id,
                    field,
                    expected,
                },
                PresentationResult::AssertFormField {
                    node_id: result_node_id,
                    field: result_field,
                    expected: result_expected,
                },
            ) => {
                result_node_id == node_id
                    && result_field == field
                    && result_expected == expected
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertFormField {
                                    node_id: operation_node_id,
                                    field: operation_field,
                                    expected: operation_expected,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_field == field
                                && operation_expected == expected
                        )
                    })
            }
            (
                Effect::UiWaitFormField {
                    node_id,
                    field,
                    expected,
                },
                PresentationResult::WaitFormField {
                    node_id: result_node_id,
                    field: result_field,
                    expected: result_expected,
                    timeout_ms: result_timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && result_field == field
                    && result_expected == expected
                    && *result_timeout_ms == UI_WAIT_FORM_FIELD_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitFormField {
                                    node_id: operation_node_id,
                                    field: operation_field,
                                    expected: operation_expected,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_field == field
                                && operation_expected == expected
                                && *operation_timeout_ms == UI_WAIT_FORM_FIELD_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertFormFieldInputKind {
                    node_id,
                    field,
                    input_kind,
                },
                PresentationResult::AssertFormFieldInputKind {
                    node_id: result_node_id,
                    field: result_field,
                    input_kind: result_input_kind,
                },
            ) => {
                result_node_id == node_id
                    && result_field == field
                    && result_input_kind == input_kind
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertFormFieldInputKind {
                                    node_id: operation_node_id,
                                    field: operation_field,
                                    input_kind: operation_input_kind,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_field == field
                                && operation_input_kind == input_kind
                        )
                    })
            }
            (
                Effect::UiWaitFormFieldInputKind {
                    node_id,
                    field,
                    input_kind,
                },
                PresentationResult::WaitFormFieldInputKind {
                    node_id: result_node_id,
                    field: result_field,
                    input_kind: result_input_kind,
                    timeout_ms: result_timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && result_field == field
                    && result_input_kind == input_kind
                    && *result_timeout_ms == UI_WAIT_FORM_FIELD_INPUT_KIND_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitFormFieldInputKind {
                                    node_id: operation_node_id,
                                    field: operation_field,
                                    input_kind: operation_input_kind,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_field == field
                                && operation_input_kind == input_kind
                                && *operation_timeout_ms == UI_WAIT_FORM_FIELD_INPUT_KIND_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertFormFieldRequired {
                    node_id,
                    field,
                    state,
                },
                PresentationResult::AssertFormFieldRequired {
                    node_id: result_node_id,
                    field: result_field,
                    state: result_state,
                },
            ) => {
                result_node_id == node_id
                    && result_field == field
                    && result_state == state
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertFormFieldRequired {
                                    node_id: operation_node_id,
                                    field: operation_field,
                                    state: operation_state,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_field == field
                                && operation_state == state
                        )
                    })
            }
            (
                Effect::UiWaitFormFieldRequired {
                    node_id,
                    field,
                    state,
                },
                PresentationResult::WaitFormFieldRequired {
                    node_id: result_node_id,
                    field: result_field,
                    state: result_state,
                    timeout_ms: result_timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && result_field == field
                    && result_state == state
                    && *result_timeout_ms == UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitFormFieldRequired {
                                    node_id: operation_node_id,
                                    field: operation_field,
                                    state: operation_state,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_field == field
                                && operation_state == state
                                && *operation_timeout_ms == UI_WAIT_FORM_FIELD_REQUIRED_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertFormFieldMaxLength {
                    node_id,
                    field,
                    max_length,
                },
                PresentationResult::AssertFormFieldMaxLength {
                    node_id: result_node_id,
                    field: result_field,
                    max_length: result_max_length,
                },
            ) => {
                result_node_id == node_id
                    && result_field == field
                    && result_max_length == max_length
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertFormFieldMaxLength {
                                    node_id: operation_node_id,
                                    field: operation_field,
                                    max_length: operation_max_length,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_field == field
                                && operation_max_length == max_length
                        )
                    })
            }
            (
                Effect::UiWaitFormFieldMaxLength {
                    node_id,
                    field,
                    max_length,
                },
                PresentationResult::WaitFormFieldMaxLength {
                    node_id: result_node_id,
                    field: result_field,
                    max_length: result_max_length,
                    timeout_ms: result_timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && result_field == field
                    && result_max_length == max_length
                    && *result_timeout_ms == UI_WAIT_FORM_FIELD_MAX_LENGTH_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitFormFieldMaxLength {
                                    node_id: operation_node_id,
                                    field: operation_field,
                                    max_length: operation_max_length,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_field == field
                                && operation_max_length == max_length
                                && *operation_timeout_ms == UI_WAIT_FORM_FIELD_MAX_LENGTH_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertFormFieldPlaceholder {
                    node_id,
                    field,
                    expected,
                },
                PresentationResult::AssertFormFieldPlaceholder {
                    node_id: result_node_id,
                    field: result_field,
                    expected: result_expected,
                },
            ) => {
                result_node_id == node_id
                    && result_field == field
                    && result_expected == expected
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertFormFieldPlaceholder {
                                    node_id: operation_node_id,
                                    field: operation_field,
                                    expected: operation_expected,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_field == field
                                && operation_expected == expected
                        )
                    })
            }
            (
                Effect::UiWaitFormFieldPlaceholder {
                    node_id,
                    field,
                    expected,
                },
                PresentationResult::WaitFormFieldPlaceholder {
                    node_id: result_node_id,
                    field: result_field,
                    expected: result_expected,
                    timeout_ms: result_timeout_ms,
                },
            ) => result_node_id == node_id
                && result_field == field
                && result_expected == expected
                && *result_timeout_ms == UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS
                && operation.is_none_or(|operation| {
                    matches!(
                        operation,
                        EffectOperation::Presentation(PresentationEnvelope {
                            operation: PresentationOperation::WaitFormFieldPlaceholder {
                                node_id: operation_node_id,
                                field: operation_field,
                                expected: operation_expected,
                                timeout_ms: operation_timeout_ms,
                            },
                            ..
                        }) if operation_node_id == node_id
                            && operation_field == field
                            && operation_expected == expected
                            && *operation_timeout_ms == UI_WAIT_FORM_FIELD_PLACEHOLDER_TIMEOUT_MS
                    )
                }),
            (
                Effect::UiAssertAccessibleName { node_id, expected },
                PresentationResult::AssertAccessibleName {
                    node_id: result_node_id,
                    expected: result_expected,
                },
            ) => {
                result_node_id == node_id
                    && result_expected == expected
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertAccessibleName {
                                    node_id: operation_node_id,
                                    expected: operation_expected,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected == expected
                        )
                    })
            }
            (
                Effect::UiWaitAccessibleName { node_id, expected },
                PresentationResult::WaitAccessibleName {
                    node_id: result_node_id,
                    expected: result_expected,
                    timeout_ms: result_timeout_ms,
                },
            ) => {
                result_node_id == node_id
                    && result_expected == expected
                    && *result_timeout_ms == UI_WAIT_ACCESSIBLE_NAME_TIMEOUT_MS
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::WaitAccessibleName {
                                    node_id: operation_node_id,
                                    expected: operation_expected,
                                    timeout_ms: operation_timeout_ms,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected == expected
                                && *operation_timeout_ms == UI_WAIT_ACCESSIBLE_NAME_TIMEOUT_MS
                        )
                    })
            }
            (
                Effect::UiAssertAccessibleDescription { node_id, expected },
                PresentationResult::AssertAccessibleDescription {
                    node_id: result_node_id,
                    expected: result_expected,
                },
            ) => {
                result_node_id == node_id
                    && result_expected == expected
                    && operation.is_none_or(|operation| {
                        matches!(
                            operation,
                            EffectOperation::Presentation(PresentationEnvelope {
                                operation: PresentationOperation::AssertAccessibleDescription {
                                    node_id: operation_node_id,
                                    expected: operation_expected,
                                },
                                ..
                            }) if operation_node_id == node_id
                                && operation_expected == expected
                        )
                    })
            }
            (
                Effect::UiWaitAccessibleDescription { node_id, expected },
                PresentationResult::WaitAccessibleDescription {
                    node_id: result_node_id,
                    expected: result_expected,
                    timeout_ms: result_timeout_ms,
                },
            ) => result_node_id == node_id
                && result_expected == expected
                && *result_timeout_ms == UI_WAIT_ACCESSIBLE_DESCRIPTION_TIMEOUT_MS
                && operation.is_none_or(|operation| {
                    matches!(
                        operation,
                        EffectOperation::Presentation(PresentationEnvelope {
                            operation: PresentationOperation::WaitAccessibleDescription {
                                node_id: operation_node_id,
                                expected: operation_expected,
                                timeout_ms: operation_timeout_ms,
                            },
                            ..
                        }) if operation_node_id == node_id
                            && operation_expected == expected
                            && *operation_timeout_ms == UI_WAIT_ACCESSIBLE_DESCRIPTION_TIMEOUT_MS
                    )
                }),
            _ => false,
        }
    }
}
