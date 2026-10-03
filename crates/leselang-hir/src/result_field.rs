//! Explicit scalar projections, not dynamic properties on host-owned objects.

use crate::Type;
use crate::computation::ScalarType;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ResultField {
    Revision,
    Count,
    NodeId,
    FocusedNodeId,
    MaxLength,
    Selected,
    Required,
    Expected,
    Field,
    Value,
    Kind,
    OptionalExpected,
}

impl ResultField {
    /// Frozen vocabularies for durable projection frames; extend through a new version.
    pub const V1: [Self; 5] = [
        Self::Revision,
        Self::Count,
        Self::NodeId,
        Self::FocusedNodeId,
        Self::MaxLength,
    ];

    pub const V2: [Self; 7] = [
        Self::Revision,
        Self::Count,
        Self::NodeId,
        Self::FocusedNodeId,
        Self::MaxLength,
        Self::Selected,
        Self::Required,
    ];

    pub const V3: [Self; 10] = [
        Self::Revision,
        Self::Count,
        Self::NodeId,
        Self::FocusedNodeId,
        Self::MaxLength,
        Self::Selected,
        Self::Required,
        Self::Expected,
        Self::Field,
        Self::Value,
    ];

    pub const V4: [Self; 11] = [
        Self::Revision,
        Self::Count,
        Self::NodeId,
        Self::FocusedNodeId,
        Self::MaxLength,
        Self::Selected,
        Self::Required,
        Self::Expected,
        Self::Field,
        Self::Value,
        Self::Kind,
    ];

    pub const V5: [Self; 12] = [
        Self::Revision,
        Self::Count,
        Self::NodeId,
        Self::FocusedNodeId,
        Self::MaxLength,
        Self::Selected,
        Self::Required,
        Self::Expected,
        Self::Field,
        Self::Value,
        Self::Kind,
        Self::OptionalExpected,
    ];

    pub const ALL: [Self; 12] = Self::V5;

    pub fn name(self) -> &'static str {
        match self {
            Self::Revision => "revision",
            Self::Count => "count",
            Self::NodeId => "node_id",
            Self::FocusedNodeId => "focused_node_id",
            Self::MaxLength => "max_length",
            Self::Selected => "selected",
            Self::Required => "required",
            Self::Expected => "expected",
            Self::Field => "field",
            Self::Value => "value",
            Self::Kind => "kind",
            Self::OptionalExpected => "optional_expected",
        }
    }

    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            "revision" => Some(Self::Revision),
            "count" => Some(Self::Count),
            "node_id" => Some(Self::NodeId),
            "focused_node_id" => Some(Self::FocusedNodeId),
            "max_length" => Some(Self::MaxLength),
            "selected" => Some(Self::Selected),
            "required" => Some(Self::Required),
            "expected" => Some(Self::Expected),
            "field" => Some(Self::Field),
            "value" => Some(Self::Value),
            "kind" => Some(Self::Kind),
            "optional_expected" => Some(Self::OptionalExpected),
            _ => None,
        }
    }

    pub fn result_type(self, input: Type) -> Option<ScalarType> {
        use ScalarType::{Boolean, Integer, String};
        use Type::*;
        match (self, input) {
            (
                Self::OptionalExpected,
                UiAssertFormFieldPlaceholder
                | UiWaitFormFieldPlaceholder
                | UiAssertActionUnavailableReason
                | UiWaitActionUnavailableReason,
            ) => Some(ScalarType::OptionalString),
            (Self::Revision, RuntimeList | RuntimeInspect | RuntimeHistory | RuntimeLogs)
            | (
                Self::Count,
                RuntimeList | RuntimeHistory | RuntimeLogs | UiAssertChildCount | UiWaitChildCount,
            )
            | (Self::MaxLength, UiAssertFormFieldMaxLength | UiWaitFormFieldMaxLength) => {
                Some(Integer)
            }
            (Self::FocusedNodeId, UiNavigateFocus) => Some(String),
            (
                Self::Kind,
                UiAssertNodeKind
                | UiWaitNodeKind
                | UiAssertActionKind
                | UiWaitActionKind
                | UiAssertFormFieldInputKind
                | UiWaitFormFieldInputKind,
            ) => Some(String),
            (
                Self::Expected,
                UiAssertText
                | UiWaitText
                | UiAssertAutomationId
                | UiWaitAutomationId
                | UiAssertActionLabel
                | UiWaitActionLabel
                | UiAssertFormValue
                | UiWaitFormValue
                | UiAssertFormField
                | UiWaitFormField
                | UiAssertAccessibleName
                | UiWaitAccessibleName
                | UiAssertAccessibleDescription
                | UiWaitAccessibleDescription,
            )
            | (
                Self::Field,
                UiSetFormValue
                | UiAssertFormValue
                | UiWaitFormValue
                | UiAssertFormField
                | UiWaitFormField
                | UiAssertFormFieldInputKind
                | UiWaitFormFieldInputKind
                | UiAssertFormFieldRequired
                | UiWaitFormFieldRequired
                | UiAssertFormFieldMaxLength
                | UiWaitFormFieldMaxLength
                | UiAssertFormFieldPlaceholder
                | UiWaitFormFieldPlaceholder,
            )
            | (Self::Value, UiSetFormValue) => Some(String),
            (Self::Selected, UiSetSelection | UiAssertSelection | UiWaitSelection)
            | (Self::Required, UiAssertFormFieldRequired | UiWaitFormFieldRequired) => {
                Some(Boolean)
            }
            (
                Self::NodeId,
                UiActivate
                | UiFocus
                | UiNavigateFocus
                | UiScrollIntoView
                | UiAssertVisible
                | UiAssertHidden
                | UiWaitHidden
                | UiAssertRealized
                | UiWaitRealized
                | UiWaitVisible
                | UiWaitEnabled
                | UiWaitDisabled
                | UiOpenWindow
                | UiCloseWindow
                | UiAssertWindowOpen
                | UiWaitWindowOpen
                | UiAssertWindowClosed
                | UiWaitWindowClosed
                | UiWaitFocused
                | UiAssertFocused
                | UiWaitUnfocused
                | UiAssertUnfocused
                | UiAssertEnabled
                | UiAssertDisabled
                | UiAssertChildCount
                | UiWaitChildCount
                | UiSetSelection
                | UiAssertSelection
                | UiWaitSelection
                | UiAssertText
                | UiWaitText
                | UiAssertAutomationId
                | UiWaitAutomationId
                | UiAssertNodeKind
                | UiWaitNodeKind
                | UiAssertActionKind
                | UiWaitActionKind
                | UiAssertActionLabel
                | UiWaitActionLabel
                | UiAssertActionAvailable
                | UiWaitActionAvailable
                | UiAssertActionUnavailableReason
                | UiWaitActionUnavailableReason
                | UiSubmitForm
                | UiCancelForm
                | UiSetFormValue
                | UiAssertFormValue
                | UiWaitFormValue
                | UiAssertFormField
                | UiWaitFormField
                | UiAssertFormFieldInputKind
                | UiWaitFormFieldInputKind
                | UiAssertFormFieldRequired
                | UiWaitFormFieldRequired
                | UiAssertFormFieldMaxLength
                | UiWaitFormFieldMaxLength
                | UiAssertFormFieldPlaceholder
                | UiWaitFormFieldPlaceholder
                | UiAssertAccessibleName
                | UiWaitAccessibleName
                | UiAssertAccessibleDescription
                | UiWaitAccessibleDescription,
            ) => Some(String),
            _ => None,
        }
    }
}
