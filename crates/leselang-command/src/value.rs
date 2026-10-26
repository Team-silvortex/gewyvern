//! Released Leserpent result data; not the host-neutral runtime value model.

use leselang_host_contract::{Revision, RuntimeId};
use leserpent_domain::{CommandResult, RuntimeLogRecord, RuntimeProjection};
use leserpent_hir::{
    Type, UiFocusNavigationDirection, UiFormInputKind, UiFormRequirementState, UiSelectionState,
    UiSemanticActionKind, UiSemanticNodeKind, computation::ScalarValue,
};
use serde::{Deserialize, Serialize};

use crate::DebuggerCancelResult;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct StructuredField {
    pub name: String,
    pub value: Box<Value>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Value {
    Scalar {
        value: ScalarValue,
    },
    RuntimeList {
        revision: Revision,
        runtimes: Vec<RuntimeProjection>,
    },
    RuntimeInspect {
        revision: Revision,
        runtime: Box<RuntimeProjection>,
    },
    RuntimeHistory {
        revision: Revision,
        entries: Vec<CommandResult>,
    },
    RuntimeLogs {
        revision: Revision,
        runtime_id: RuntimeId,
        runtime_name: String,
        entries: Vec<RuntimeLogRecord>,
    },
    RuntimeRefresh {
        result: Box<CommandResult>,
    },
    RuntimeCapabilitiesRefresh {
        result: Box<CommandResult>,
    },
    RuntimeDeploy {
        result: Box<CommandResult>,
    },
    DebuggerCancel {
        result: DebuggerCancelResult,
    },
    UiActivate {
        node_id: String,
    },
    UiFocus {
        node_id: String,
    },
    UiNavigateFocus {
        node_id: String,
        direction: UiFocusNavigationDirection,
        focused_node_id: String,
    },
    UiScrollIntoView {
        node_id: String,
    },
    UiAssertVisible {
        node_id: String,
    },
    UiAssertHidden {
        node_id: String,
    },
    UiWaitHidden {
        node_id: String,
    },
    UiAssertRealized {
        node_id: String,
    },
    UiWaitRealized {
        node_id: String,
    },
    UiWaitVisible {
        node_id: String,
    },
    UiWaitEnabled {
        node_id: String,
    },
    UiWaitDisabled {
        node_id: String,
    },
    UiOpenWindow {
        node_id: String,
    },
    UiCloseWindow {
        node_id: String,
    },
    UiAssertWindowOpen {
        node_id: String,
    },
    UiWaitWindowOpen {
        node_id: String,
    },
    UiAssertWindowClosed {
        node_id: String,
    },
    UiWaitWindowClosed {
        node_id: String,
    },
    UiWaitFocused {
        node_id: String,
    },
    UiAssertFocused {
        node_id: String,
    },
    UiWaitUnfocused {
        node_id: String,
    },
    UiAssertUnfocused {
        node_id: String,
    },
    UiAssertEnabled {
        node_id: String,
    },
    UiAssertDisabled {
        node_id: String,
    },
    UiAssertChildCount {
        node_id: String,
        count: usize,
    },
    UiWaitChildCount {
        node_id: String,
        count: usize,
    },
    UiSetSelection {
        node_id: String,
        state: UiSelectionState,
    },
    UiAssertSelection {
        node_id: String,
        state: UiSelectionState,
    },
    UiWaitSelection {
        node_id: String,
        state: UiSelectionState,
    },
    UiAssertText {
        node_id: String,
        expected: String,
    },
    UiWaitText {
        node_id: String,
        expected: String,
    },
    UiAssertAutomationId {
        node_id: String,
        expected: String,
    },
    UiWaitAutomationId {
        node_id: String,
        expected: String,
    },
    UiAssertNodeKind {
        node_id: String,
        expected_kind: UiSemanticNodeKind,
    },
    UiWaitNodeKind {
        node_id: String,
        expected_kind: UiSemanticNodeKind,
    },
    UiAssertActionKind {
        node_id: String,
        expected_kind: UiSemanticActionKind,
    },
    UiWaitActionKind {
        node_id: String,
        expected_kind: UiSemanticActionKind,
    },
    UiAssertActionLabel {
        node_id: String,
        expected: String,
    },
    UiWaitActionLabel {
        node_id: String,
        expected: String,
    },
    UiAssertActionAvailable {
        node_id: String,
    },
    UiWaitActionAvailable {
        node_id: String,
    },
    UiAssertActionUnavailableReason {
        node_id: String,
        expected: Option<String>,
    },
    UiWaitActionUnavailableReason {
        node_id: String,
        expected: Option<String>,
    },
    UiSubmitForm {
        node_id: String,
    },
    UiCancelForm {
        node_id: String,
    },
    UiSetFormValue {
        node_id: String,
        field: String,
        value: String,
    },
    UiAssertFormValue {
        node_id: String,
        field: String,
        expected: String,
    },
    UiWaitFormValue {
        node_id: String,
        field: String,
        expected: String,
    },
    UiAssertFormField {
        node_id: String,
        field: String,
        expected: String,
    },
    UiWaitFormField {
        node_id: String,
        field: String,
        expected: String,
    },
    UiAssertFormFieldInputKind {
        node_id: String,
        field: String,
        input_kind: UiFormInputKind,
    },
    UiWaitFormFieldInputKind {
        node_id: String,
        field: String,
        input_kind: UiFormInputKind,
    },
    UiAssertFormFieldRequired {
        node_id: String,
        field: String,
        state: UiFormRequirementState,
    },
    UiWaitFormFieldRequired {
        node_id: String,
        field: String,
        state: UiFormRequirementState,
    },
    UiAssertFormFieldMaxLength {
        node_id: String,
        field: String,
        max_length: usize,
    },
    UiWaitFormFieldMaxLength {
        node_id: String,
        field: String,
        max_length: usize,
    },
    UiAssertFormFieldPlaceholder {
        node_id: String,
        field: String,
        expected: Option<String>,
    },
    UiWaitFormFieldPlaceholder {
        node_id: String,
        field: String,
        expected: Option<String>,
    },
    UiAssertAccessibleName {
        node_id: String,
        expected: String,
    },
    UiWaitAccessibleName {
        node_id: String,
        expected: String,
    },
    UiAssertAccessibleDescription {
        node_id: String,
        expected: String,
    },
    UiWaitAccessibleDescription {
        node_id: String,
        expected: String,
    },
    Structured {
        fields: Vec<StructuredField>,
    },
}

impl Value {
    /// Read the released product type tag without validating its payload or bounds.
    pub fn result_type(&self) -> Type {
        macro_rules! atomic_types {
        ($($variant:ident),+ $(,)?) => {
            match self {
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
}
