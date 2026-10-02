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
}

impl ResultField {
    pub fn name(self) -> &'static str {
        match self {
            Self::Revision => "revision",
            Self::Count => "count",
            Self::NodeId => "node_id",
            Self::FocusedNodeId => "focused_node_id",
            Self::MaxLength => "max_length",
        }
    }

    pub(crate) fn parse(name: &str) -> Option<Self> {
        match name {
            "revision" => Some(Self::Revision),
            "count" => Some(Self::Count),
            "node_id" => Some(Self::NodeId),
            "focused_node_id" => Some(Self::FocusedNodeId),
            "max_length" => Some(Self::MaxLength),
            _ => None,
        }
    }

    pub fn result_type(self, input: Type) -> Option<ScalarType> {
        use ScalarType::{Integer, String};
        use Type::*;
        match (self, input) {
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
