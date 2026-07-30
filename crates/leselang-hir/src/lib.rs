use std::collections::HashSet;

use leselang_syntax::{Expression, Span, SyntaxTree, format as format_syntax, parse};
use leserpent_domain::{
    CAPABILITY_DEBUGGER_CONTROL, CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ,
    CAPABILITY_RUNTIME_REFRESH, CapabilitySet, DomainError, RuntimeId, RuntimeListFilter,
    validate_debugger_session_id, validate_deployment_intent,
};
use serde::{Deserialize, Serialize};

pub const MAX_ALL_BRANCHES: usize = 64;
pub const MAX_BRANCH_NAME_BYTES: usize = 64;
pub const MAX_UI_FORM_FIELD_KEY_BYTES: usize = 128;
pub const MAX_UI_FORM_FIELD_MAX_LENGTH: usize = 256;
pub const MAX_UI_NODE_ID_BYTES: usize = 128;
pub const MAX_UI_EXPECTED_TEXT_BYTES: usize = 1_024;
pub const CAPABILITY_UI_PRESENTATION: &str = "ui.presentation";
pub const UI_WAIT_ENABLED_TIMEOUT_MS: u64 = 2_000;
pub const UI_WAIT_FOCUSED_TIMEOUT_MS: u64 = 2_000;
pub const UI_WAIT_REALIZED_TIMEOUT_MS: u64 = 2_000;
pub const UI_WAIT_SELECTION_TIMEOUT_MS: u64 = 2_000;
pub const UI_WAIT_VISIBLE_TIMEOUT_MS: u64 = 2_000;
pub const UI_WAIT_WINDOW_OPEN_TIMEOUT_MS: u64 = 2_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFocusNavigationDirection {
    Next,
    Previous,
    First,
    Last,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSelectionState {
    Selected,
    Unselected,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSemanticNodeKind {
    Column,
    Heading,
    Text,
    RuntimeCard,
    RuntimeWorkspace,
    Section,
    HistoryEntry,
    LogEntry,
    DebuggerWorkspace,
    DebuggerFrame,
    Action,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiSemanticActionKind {
    RuntimeInspect,
    RuntimeRefresh,
    RuntimeCapabilitiesRefresh,
    RuntimeDeploy,
    DebuggerCancel,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFormInputKind {
    PathToken,
    TrimmedText,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiFormRequirementState {
    Required,
    Optional,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HirProgram {
    pub function: HirFunction,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HirFunction {
    pub name: String,
    pub effect: Effect,
    pub result_type: Type,
    pub required_capabilities: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct HirBranch {
    pub name: String,
    pub effect: Effect,
    pub result_type: Type,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum Effect {
    RuntimeList {
        filter: RuntimeListFilter,
    },
    RuntimeInspect {
        runtime_id: RuntimeId,
    },
    RuntimeHistory {
        runtime_id: RuntimeId,
    },
    RuntimeLogs {
        runtime_id: RuntimeId,
    },
    RuntimeRefresh {
        runtime_id: RuntimeId,
    },
    RuntimeCapabilitiesRefresh {
        runtime_id: RuntimeId,
    },
    RuntimeDeploy {
        runtime_id: RuntimeId,
        pipeline_kind: String,
        target: Option<String>,
    },
    DebuggerCancel {
        session_id: String,
    },
    UiFocus {
        node_id: String,
    },
    UiNavigateFocus {
        node_id: String,
        direction: UiFocusNavigationDirection,
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
    UiAssertWindowOpen {
        node_id: String,
    },
    UiWaitWindowOpen {
        node_id: String,
    },
    UiWaitFocused {
        node_id: String,
    },
    UiAssertFocused {
        node_id: String,
    },
    UiAssertEnabled {
        node_id: String,
    },
    UiAssertDisabled {
        node_id: String,
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
    UiAssertAutomationId {
        node_id: String,
        expected: String,
    },
    UiAssertNodeKind {
        node_id: String,
        expected_kind: UiSemanticNodeKind,
    },
    UiAssertActionKind {
        node_id: String,
        expected_kind: UiSemanticActionKind,
    },
    UiAssertActionUnavailableReason {
        node_id: String,
        expected: Option<String>,
    },
    UiAssertFormField {
        node_id: String,
        field: String,
        expected: String,
    },
    UiAssertFormFieldInputKind {
        node_id: String,
        field: String,
        input_kind: UiFormInputKind,
    },
    UiAssertFormFieldRequired {
        node_id: String,
        field: String,
        state: UiFormRequirementState,
    },
    UiAssertFormFieldMaxLength {
        node_id: String,
        field: String,
        max_length: usize,
    },
    UiAssertFormFieldPlaceholder {
        node_id: String,
        field: String,
        expected: Option<String>,
    },
    UiAssertAccessibleName {
        node_id: String,
        expected: String,
    },
    UiAssertAccessibleDescription {
        node_id: String,
        expected: String,
    },
    All {
        branches: Vec<HirBranch>,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Type {
    RuntimeList,
    RuntimeInspect,
    RuntimeHistory,
    RuntimeLogs,
    RuntimeRefresh,
    RuntimeCapabilitiesRefresh,
    RuntimeDeploy,
    DebuggerCancel,
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
    UiAssertWindowOpen,
    UiWaitWindowOpen,
    UiWaitFocused,
    UiAssertFocused,
    UiAssertEnabled,
    UiAssertDisabled,
    UiAssertSelection,
    UiWaitSelection,
    UiAssertText,
    UiAssertAutomationId,
    UiAssertNodeKind,
    UiAssertActionKind,
    UiAssertActionUnavailableReason,
    UiAssertFormField,
    UiAssertFormFieldInputKind,
    UiAssertFormFieldRequired,
    UiAssertFormFieldMaxLength,
    UiAssertFormFieldPlaceholder,
    UiAssertAccessibleName,
    UiAssertAccessibleDescription,
    Structured,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Diagnostic {
    pub code: String,
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CanonicalSourceError {
    Syntax(Vec<leselang_syntax::Diagnostic>),
    InvalidEffect(Vec<Diagnostic>),
    RoundTripMismatch,
}

pub fn lower(tree: &SyntaxTree) -> Result<HirProgram, Vec<Diagnostic>> {
    if !tree.diagnostics.is_empty() {
        return Err(tree
            .diagnostics
            .iter()
            .map(|item| Diagnostic {
                code: item.code.clone(),
                message: item.message.clone(),
                span: Some(item.span),
            })
            .collect());
    }
    let Some(function) = &tree.function else {
        return Err(vec![Diagnostic {
            code: "LSH1001".to_string(),
            message: "program has no function".to_string(),
            span: None,
        }]);
    };
    let lowered = lower_effect(&function.body)?;
    Ok(HirProgram {
        function: HirFunction {
            name: function.name.clone(),
            effect: lowered.effect,
            result_type: lowered.result_type,
            required_capabilities: lowered.required_capabilities,
        },
    })
}

struct LoweredEffect {
    effect: Effect,
    result_type: Type,
    required_capabilities: Vec<String>,
}

fn lower_effect(expression: &Expression) -> Result<LoweredEffect, Vec<Diagnostic>> {
    let Expression::Call {
        callee,
        arguments,
        span,
    } = expression
    else {
        return Err(vec![Diagnostic {
            code: "LSH1002".to_string(),
            message: "structured branches must contain effect calls".to_string(),
            span: Some(expression_span(expression)),
        }]);
    };
    match callee.as_str() {
        "runtime.list"
        | "runtime.inspect"
        | "runtime.history"
        | "runtime.logs"
        | "runtime.refresh"
        | "runtime.refresh_capabilities"
        | "runtime.deploy"
        | "debugger.cancel"
        | "ui.focus"
        | "ui.navigate_focus"
        | "ui.scroll_into_view"
        | "ui.assert_visible"
        | "ui.assert_hidden"
        | "ui.wait_hidden"
        | "ui.assert_realized"
        | "ui.wait_realized"
        | "ui.wait_visible"
        | "ui.wait_enabled"
        | "ui.wait_disabled"
        | "ui.assert_window_open"
        | "ui.wait_window_open"
        | "ui.wait_focused"
        | "ui.assert_focused"
        | "ui.assert_enabled"
        | "ui.assert_disabled"
        | "ui.assert_selection"
        | "ui.wait_selection"
        | "ui.assert_text"
        | "ui.assert_automation_id"
        | "ui.assert_node_kind"
        | "ui.assert_action_kind"
        | "ui.assert_action_unavailable_reason"
        | "ui.assert_form_field"
        | "ui.assert_form_field_input_kind"
        | "ui.assert_form_field_required"
        | "ui.assert_form_field_max_length"
        | "ui.assert_form_field_placeholder"
        | "ui.assert_accessible_name"
        | "ui.assert_accessible_description" => lower_atomic_effect(callee, arguments, *span),
        "all" => lower_all(arguments, *span),
        _ => Err(vec![Diagnostic {
            code: "LSH1003".to_string(),
            message: format!("unknown effect or structured form '{callee}'"),
            span: Some(*span),
        }]),
    }
}

fn lower_atomic_effect(
    callee: &str,
    arguments: &[leselang_syntax::NamedArgument],
    span: Span,
) -> Result<LoweredEffect, Vec<Diagnostic>> {
    let mut seen = HashSet::with_capacity(arguments.len());
    let mut filter = RuntimeListFilter::default();
    let mut runtime_id = None;
    let mut pipeline_kind = None;
    let mut target = None;
    let mut session_id = None;
    let mut node_id = None;
    let mut focus_navigation_direction = None;
    let mut selection_state = None;
    let mut semantic_node_kind = None;
    let mut semantic_action_kind = None;
    let mut action_unavailable_reason_expected = None;
    let mut form_field_key = None;
    let mut form_input_kind = None;
    let mut form_requirement_state = None;
    let mut form_max_length = None;
    let mut form_placeholder_expected = None;
    let mut expected_text = None;
    let mut diagnostics = Vec::new();
    for argument in arguments {
        if !seen.insert(argument.name.as_str()) {
            diagnostics.push(Diagnostic {
                code: "LSH1101".to_string(),
                message: format!("duplicate argument '{}'", argument.name),
                span: Some(argument.span),
            });
            continue;
        }
        let value = match &argument.value {
            Expression::String { value, .. } => Some(value.clone()),
            Expression::None { .. } => None,
            Expression::Call { .. } => {
                diagnostics.push(Diagnostic {
                    code: "LSH1102".to_string(),
                    message: "filter arguments require a string or 'none'".to_string(),
                    span: Some(argument.span),
                });
                continue;
            }
        };
        match (callee, argument.name.as_str()) {
            ("runtime.list", "environment") => filter.environment = value,
            ("runtime.list", "cluster") => filter.cluster = value,
            ("runtime.list", "role") => filter.role = value,
            (
                "runtime.inspect"
                | "runtime.history"
                | "runtime.logs"
                | "runtime.refresh"
                | "runtime.refresh_capabilities"
                | "runtime.deploy",
                "runtime_id",
            ) => match value.and_then(|value| RuntimeId::new(value).ok()) {
                Some(value) => runtime_id = Some(value),
                None => diagnostics.push(Diagnostic {
                    code: "LSH1104".to_string(),
                    message: format!("{callee} runtime_id must be a valid identifier string"),
                    span: Some(argument.span),
                }),
            },
            ("runtime.deploy", "pipeline_kind") => match value {
                Some(value) => pipeline_kind = Some(value),
                None => diagnostics.push(Diagnostic {
                    code: "LSH1106".to_string(),
                    message: "runtime.deploy pipeline_kind must be a valid token string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("runtime.deploy", "target") => match value {
                Some(value) => target = Some(value),
                None => target = None,
            },
            ("debugger.cancel", "session_id") => match value {
                Some(value) if validate_debugger_session_id(&value).is_ok() => {
                    session_id = Some(value);
                }
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1110".to_string(),
                    message: "debugger.cancel session_id must be a valid identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.focus", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1112".to_string(),
                    message: "ui.focus node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.navigate_focus", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1144".to_string(),
                    message:
                        "ui.navigate_focus node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.navigate_focus", "direction") => match value.as_deref() {
                Some("next") => {
                    focus_navigation_direction = Some(UiFocusNavigationDirection::Next);
                }
                Some("previous") => {
                    focus_navigation_direction = Some(UiFocusNavigationDirection::Previous);
                }
                Some("first") => {
                    focus_navigation_direction = Some(UiFocusNavigationDirection::First);
                }
                Some("last") => {
                    focus_navigation_direction = Some(UiFocusNavigationDirection::Last);
                }
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1145".to_string(),
                    message:
                        "ui.navigate_focus direction must be \"next\", \"previous\", \"first\", or \"last\""
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.scroll_into_view", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1114".to_string(),
                    message:
                        "ui.scroll_into_view node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_visible", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1116".to_string(),
                    message: "ui.assert_visible node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_hidden", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1170".to_string(),
                    message: "ui.assert_hidden node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.wait_hidden", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1172".to_string(),
                    message: "ui.wait_hidden node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_realized", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1134".to_string(),
                    message:
                        "ui.assert_realized node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.wait_realized", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1136".to_string(),
                    message:
                        "ui.wait_realized node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.wait_visible", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1138".to_string(),
                    message: "ui.wait_visible node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.wait_enabled", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1140".to_string(),
                    message: "ui.wait_enabled node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.wait_disabled", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1174".to_string(),
                    message: "ui.wait_disabled node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_window_open", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1176".to_string(),
                    message:
                        "ui.assert_window_open node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.wait_window_open", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1178".to_string(),
                    message:
                        "ui.wait_window_open node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.wait_focused", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1142".to_string(),
                    message: "ui.wait_focused node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_focused", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1118".to_string(),
                    message: "ui.assert_focused node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_enabled", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1120".to_string(),
                    message: "ui.assert_enabled node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_disabled", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1168".to_string(),
                    message: "ui.assert_disabled node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_selection", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1148".to_string(),
                    message:
                        "ui.assert_selection node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_selection", "state") => match value.as_deref() {
                Some("selected") => selection_state = Some(UiSelectionState::Selected),
                Some("unselected") => selection_state = Some(UiSelectionState::Unselected),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1149".to_string(),
                    message: "ui.assert_selection state must be \"selected\" or \"unselected\""
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.wait_selection", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1152".to_string(),
                    message:
                        "ui.wait_selection node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.wait_selection", "state") => match value.as_deref() {
                Some("selected") => selection_state = Some(UiSelectionState::Selected),
                Some("unselected") => selection_state = Some(UiSelectionState::Unselected),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1153".to_string(),
                    message: "ui.wait_selection state must be \"selected\" or \"unselected\""
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_text", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1122".to_string(),
                    message: "ui.assert_text node_id must be a valid UI node identifier string"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_text", "expected") => match value {
                Some(value) if validate_ui_expected_text(&value) => expected_text = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1123".to_string(),
                    message: "ui.assert_text expected must be bounded display text".to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_automation_id", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1156".to_string(),
                    message:
                        "ui.assert_automation_id node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_automation_id", "expected") => match value {
                Some(value) if validate_ui_node_id(&value) => expected_text = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1157".to_string(),
                    message:
                        "ui.assert_automation_id expected must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_node_kind", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1160".to_string(),
                    message:
                        "ui.assert_node_kind node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_node_kind", "kind") => match value
                .as_deref()
                .and_then(parse_semantic_node_kind)
            {
                Some(value) => semantic_node_kind = Some(value),
                None => diagnostics.push(Diagnostic {
                    code: "LSH1161".to_string(),
                    message: "ui.assert_node_kind kind must be a known semantic UI node kind"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_action_kind", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1164".to_string(),
                    message:
                        "ui.assert_action_kind node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_action_kind", "kind") => match value
                .as_deref()
                .and_then(parse_semantic_action_kind)
            {
                Some(value) => semantic_action_kind = Some(value),
                None => diagnostics.push(Diagnostic {
                    code: "LSH1165".to_string(),
                    message: "ui.assert_action_kind kind must be a known semantic UI action kind"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_action_unavailable_reason", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1168".to_string(),
                    message:
                        "ui.assert_action_unavailable_reason node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_action_unavailable_reason", "expected") => match &argument.value {
                Expression::String { value, .. } if validate_ui_expected_text(value) => {
                    action_unavailable_reason_expected = Some(Some(value.clone()));
                }
                Expression::None { .. } => action_unavailable_reason_expected = Some(None),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1169".to_string(),
                    message:
                        "ui.assert_action_unavailable_reason expected must be bounded display text or none"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1180".to_string(),
                    message:
                        "ui.assert_form_field node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field", "field") => match value {
                Some(value) if validate_ui_form_field_key(&value) => form_field_key = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1181".to_string(),
                    message: "ui.assert_form_field field must be a valid UI form field key"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field", "expected") => match value {
                Some(value) if validate_ui_expected_text(&value) => expected_text = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1182".to_string(),
                    message: "ui.assert_form_field expected must be bounded display text"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field_input_kind", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1186".to_string(),
                    message:
                        "ui.assert_form_field_input_kind node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field_input_kind", "field") => match value {
                Some(value) if validate_ui_form_field_key(&value) => form_field_key = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1187".to_string(),
                    message:
                        "ui.assert_form_field_input_kind field must be a valid UI form field key"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field_input_kind", "kind") => match value
                .as_deref()
                .and_then(parse_form_input_kind)
            {
                Some(value) => form_input_kind = Some(value),
                None => diagnostics.push(Diagnostic {
                    code: "LSH1188".to_string(),
                    message:
                        "ui.assert_form_field_input_kind kind must be a known UI form input kind"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field_required", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1192".to_string(),
                    message:
                        "ui.assert_form_field_required node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field_required", "field") => match value {
                Some(value) if validate_ui_form_field_key(&value) => form_field_key = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1193".to_string(),
                    message:
                        "ui.assert_form_field_required field must be a valid UI form field key"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field_required", "state") => match value
                .as_deref()
                .and_then(parse_form_requirement_state)
            {
                Some(value) => form_requirement_state = Some(value),
                None => diagnostics.push(Diagnostic {
                    code: "LSH1194".to_string(),
                    message: "ui.assert_form_field_required state must be required or optional"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field_max_length", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1198".to_string(),
                    message:
                        "ui.assert_form_field_max_length node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field_max_length", "field") => match value {
                Some(value) if validate_ui_form_field_key(&value) => form_field_key = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1199".to_string(),
                    message:
                        "ui.assert_form_field_max_length field must be a valid UI form field key"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field_max_length", "max_length") => match value
                .as_deref()
                .and_then(parse_form_max_length)
            {
                Some(value) => form_max_length = Some(value),
                None => diagnostics.push(Diagnostic {
                    code: "LSH1200".to_string(),
                    message:
                        "ui.assert_form_field_max_length max_length must be a decimal string from 1 to 256"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field_placeholder", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1213".to_string(),
                    message:
                        "ui.assert_form_field_placeholder node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field_placeholder", "field") => match value {
                Some(value) if validate_ui_form_field_key(&value) => form_field_key = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1214".to_string(),
                    message:
                        "ui.assert_form_field_placeholder field must be a valid UI form field key"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_form_field_placeholder", "expected") => match &argument.value {
                Expression::String { value, .. } if validate_ui_expected_text(value) => {
                    form_placeholder_expected = Some(Some(value.clone()));
                }
                Expression::None { .. } => form_placeholder_expected = Some(None),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1215".to_string(),
                    message:
                        "ui.assert_form_field_placeholder expected must be bounded display text or none"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_accessible_name", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1126".to_string(),
                    message:
                        "ui.assert_accessible_name node_id must be a valid UI node identifier string"
                            .to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_accessible_name", "expected") => match value {
                Some(value) if validate_ui_expected_text(&value) => expected_text = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1127".to_string(),
                    message:
                        "ui.assert_accessible_name expected must be bounded display text".to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_accessible_description", "node_id") => match value {
                Some(value) if validate_ui_node_id(&value) => node_id = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1130".to_string(),
                    message: "ui.assert_accessible_description node_id must be a valid UI node identifier string".to_string(),
                    span: Some(argument.span),
                }),
            },
            ("ui.assert_accessible_description", "expected") => match value {
                Some(value) if validate_ui_expected_text(&value) => expected_text = Some(value),
                _ => diagnostics.push(Diagnostic {
                    code: "LSH1131".to_string(),
                    message: "ui.assert_accessible_description expected must be bounded display text"
                        .to_string(),
                    span: Some(argument.span),
                }),
            },
            _ => diagnostics.push(Diagnostic {
                code: "LSH1103".to_string(),
                message: format!("unknown {callee} argument '{}'", argument.name),
                span: Some(argument.span),
            }),
        }
    }
    if matches!(
        callee,
        "runtime.inspect"
            | "runtime.history"
            | "runtime.logs"
            | "runtime.refresh"
            | "runtime.refresh_capabilities"
            | "runtime.deploy"
    ) && runtime_id.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1105".to_string(),
            message: format!("{callee} requires runtime_id"),
            span: Some(span),
        });
    }
    if callee == "runtime.deploy" && pipeline_kind.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1108".to_string(),
            message: "runtime.deploy requires pipeline_kind".to_string(),
            span: Some(span),
        });
    }
    if callee == "debugger.cancel" && session_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1111".to_string(),
            message: "debugger.cancel requires session_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.focus" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1113".to_string(),
            message: "ui.focus requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.navigate_focus" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1146".to_string(),
            message: "ui.navigate_focus requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.navigate_focus"
        && focus_navigation_direction.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1147".to_string(),
            message: "ui.navigate_focus requires direction".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.scroll_into_view" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1115".to_string(),
            message: "ui.scroll_into_view requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_visible" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1117".to_string(),
            message: "ui.assert_visible requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_hidden" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1171".to_string(),
            message: "ui.assert_hidden requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.wait_hidden" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1173".to_string(),
            message: "ui.wait_hidden requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_realized" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1135".to_string(),
            message: "ui.assert_realized requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.wait_realized" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1137".to_string(),
            message: "ui.wait_realized requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.wait_visible" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1139".to_string(),
            message: "ui.wait_visible requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.wait_enabled" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1141".to_string(),
            message: "ui.wait_enabled requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.wait_disabled" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1175".to_string(),
            message: "ui.wait_disabled requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_window_open" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1177".to_string(),
            message: "ui.assert_window_open requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.wait_window_open" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1179".to_string(),
            message: "ui.wait_window_open requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.wait_focused" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1143".to_string(),
            message: "ui.wait_focused requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_focused" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1119".to_string(),
            message: "ui.assert_focused requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_enabled" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1121".to_string(),
            message: "ui.assert_enabled requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_disabled" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1169".to_string(),
            message: "ui.assert_disabled requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_selection" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1150".to_string(),
            message: "ui.assert_selection requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_selection" && selection_state.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1151".to_string(),
            message: "ui.assert_selection requires state".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.wait_selection" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1154".to_string(),
            message: "ui.wait_selection requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.wait_selection" && selection_state.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1155".to_string(),
            message: "ui.wait_selection requires state".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_text" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1124".to_string(),
            message: "ui.assert_text requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_text" && expected_text.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1125".to_string(),
            message: "ui.assert_text requires expected".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_automation_id" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1158".to_string(),
            message: "ui.assert_automation_id requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_automation_id" && expected_text.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1159".to_string(),
            message: "ui.assert_automation_id requires expected".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_node_kind" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1162".to_string(),
            message: "ui.assert_node_kind requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_node_kind" && semantic_node_kind.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1163".to_string(),
            message: "ui.assert_node_kind requires kind".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_action_kind" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1166".to_string(),
            message: "ui.assert_action_kind requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_action_kind" && semantic_action_kind.is_none() && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1167".to_string(),
            message: "ui.assert_action_kind requires kind".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_action_unavailable_reason"
        && node_id.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1170".to_string(),
            message: "ui.assert_action_unavailable_reason requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_action_unavailable_reason"
        && action_unavailable_reason_expected.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1171".to_string(),
            message: "ui.assert_action_unavailable_reason requires expected".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1183".to_string(),
            message: "ui.assert_form_field requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field" && form_field_key.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1184".to_string(),
            message: "ui.assert_form_field requires field".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field" && expected_text.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1185".to_string(),
            message: "ui.assert_form_field requires expected".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field_input_kind" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1189".to_string(),
            message: "ui.assert_form_field_input_kind requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field_input_kind"
        && form_field_key.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1190".to_string(),
            message: "ui.assert_form_field_input_kind requires field".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field_input_kind"
        && form_input_kind.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1191".to_string(),
            message: "ui.assert_form_field_input_kind requires kind".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field_required" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1195".to_string(),
            message: "ui.assert_form_field_required requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field_required"
        && form_field_key.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1196".to_string(),
            message: "ui.assert_form_field_required requires field".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field_required"
        && form_requirement_state.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1197".to_string(),
            message: "ui.assert_form_field_required requires state".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field_max_length" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1210".to_string(),
            message: "ui.assert_form_field_max_length requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field_max_length"
        && form_field_key.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1211".to_string(),
            message: "ui.assert_form_field_max_length requires field".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field_max_length"
        && form_max_length.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1212".to_string(),
            message: "ui.assert_form_field_max_length requires max_length".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field_placeholder" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1216".to_string(),
            message: "ui.assert_form_field_placeholder requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field_placeholder"
        && form_field_key.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1217".to_string(),
            message: "ui.assert_form_field_placeholder requires field".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_form_field_placeholder"
        && form_placeholder_expected.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1218".to_string(),
            message: "ui.assert_form_field_placeholder requires expected".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_accessible_name" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1128".to_string(),
            message: "ui.assert_accessible_name requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_accessible_name" && expected_text.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1129".to_string(),
            message: "ui.assert_accessible_name requires expected".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_accessible_description" && node_id.is_none() && diagnostics.is_empty() {
        diagnostics.push(Diagnostic {
            code: "LSH1132".to_string(),
            message: "ui.assert_accessible_description requires node_id".to_string(),
            span: Some(span),
        });
    }
    if callee == "ui.assert_accessible_description"
        && expected_text.is_none()
        && diagnostics.is_empty()
    {
        diagnostics.push(Diagnostic {
            code: "LSH1133".to_string(),
            message: "ui.assert_accessible_description requires expected".to_string(),
            span: Some(span),
        });
    }
    if callee == "runtime.deploy"
        && diagnostics.is_empty()
        && let Some(pipeline_kind) = pipeline_kind.as_deref()
        && let Err(DomainError::InvalidIdentifier { field }) =
            validate_deployment_intent(pipeline_kind, target.as_deref())
    {
        diagnostics.push(Diagnostic {
            code: if field == "target" {
                "LSH1107"
            } else {
                "LSH1106"
            }
            .to_string(),
            message: if field == "target" {
                "runtime.deploy target must be a bounded text string or none"
            } else {
                "runtime.deploy pipeline_kind must be a valid token string"
            }
            .to_string(),
            span: Some(span),
        });
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }

    let (effect, result_type, required_capability) = match callee {
        "runtime.list" => (
            Effect::RuntimeList {
                filter: filter.normalized(),
            },
            Type::RuntimeList,
            CAPABILITY_RUNTIME_READ,
        ),
        "runtime.inspect" => (
            Effect::RuntimeInspect {
                runtime_id: runtime_id.expect("validated runtime.inspect identifier"),
            },
            Type::RuntimeInspect,
            CAPABILITY_RUNTIME_READ,
        ),
        "runtime.history" => (
            Effect::RuntimeHistory {
                runtime_id: runtime_id.expect("validated runtime.history identifier"),
            },
            Type::RuntimeHistory,
            CAPABILITY_RUNTIME_READ,
        ),
        "runtime.logs" => (
            Effect::RuntimeLogs {
                runtime_id: runtime_id.expect("validated runtime.logs identifier"),
            },
            Type::RuntimeLogs,
            CAPABILITY_RUNTIME_READ,
        ),
        "runtime.refresh" => (
            Effect::RuntimeRefresh {
                runtime_id: runtime_id.expect("validated runtime.refresh identifier"),
            },
            Type::RuntimeRefresh,
            CAPABILITY_RUNTIME_REFRESH,
        ),
        "runtime.refresh_capabilities" => (
            Effect::RuntimeCapabilitiesRefresh {
                runtime_id: runtime_id.expect("validated runtime.refresh_capabilities identifier"),
            },
            Type::RuntimeCapabilitiesRefresh,
            CAPABILITY_RUNTIME_REFRESH,
        ),
        "runtime.deploy" => (
            Effect::RuntimeDeploy {
                runtime_id: runtime_id.expect("validated runtime.deploy identifier"),
                pipeline_kind: pipeline_kind.expect("validated runtime.deploy pipeline kind"),
                target,
            },
            Type::RuntimeDeploy,
            CAPABILITY_RUNTIME_DEPLOY,
        ),
        "debugger.cancel" => (
            Effect::DebuggerCancel {
                session_id: session_id.expect("validated debugger session identifier"),
            },
            Type::DebuggerCancel,
            CAPABILITY_DEBUGGER_CONTROL,
        ),
        "ui.focus" => (
            Effect::UiFocus {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiFocus,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.navigate_focus" => (
            Effect::UiNavigateFocus {
                node_id: node_id.expect("validated UI node identifier"),
                direction: focus_navigation_direction
                    .expect("validated UI focus navigation direction"),
            },
            Type::UiNavigateFocus,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.scroll_into_view" => (
            Effect::UiScrollIntoView {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiScrollIntoView,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_visible" => (
            Effect::UiAssertVisible {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiAssertVisible,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_hidden" => (
            Effect::UiAssertHidden {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiAssertHidden,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.wait_hidden" => (
            Effect::UiWaitHidden {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiWaitHidden,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_realized" => (
            Effect::UiAssertRealized {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiAssertRealized,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.wait_realized" => (
            Effect::UiWaitRealized {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiWaitRealized,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.wait_visible" => (
            Effect::UiWaitVisible {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiWaitVisible,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.wait_enabled" => (
            Effect::UiWaitEnabled {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiWaitEnabled,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.wait_disabled" => (
            Effect::UiWaitDisabled {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiWaitDisabled,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_window_open" => (
            Effect::UiAssertWindowOpen {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiAssertWindowOpen,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.wait_window_open" => (
            Effect::UiWaitWindowOpen {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiWaitWindowOpen,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.wait_focused" => (
            Effect::UiWaitFocused {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiWaitFocused,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_focused" => (
            Effect::UiAssertFocused {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiAssertFocused,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_enabled" => (
            Effect::UiAssertEnabled {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiAssertEnabled,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_disabled" => (
            Effect::UiAssertDisabled {
                node_id: node_id.expect("validated UI node identifier"),
            },
            Type::UiAssertDisabled,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_selection" => (
            Effect::UiAssertSelection {
                node_id: node_id.expect("validated UI node identifier"),
                state: selection_state.expect("validated UI selection state"),
            },
            Type::UiAssertSelection,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.wait_selection" => (
            Effect::UiWaitSelection {
                node_id: node_id.expect("validated UI node identifier"),
                state: selection_state.expect("validated UI selection state"),
            },
            Type::UiWaitSelection,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_text" => (
            Effect::UiAssertText {
                node_id: node_id.expect("validated UI node identifier"),
                expected: expected_text.expect("validated UI expected text"),
            },
            Type::UiAssertText,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_automation_id" => (
            Effect::UiAssertAutomationId {
                node_id: node_id.expect("validated UI node identifier"),
                expected: expected_text.expect("validated UI expected automation identifier"),
            },
            Type::UiAssertAutomationId,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_node_kind" => (
            Effect::UiAssertNodeKind {
                node_id: node_id.expect("validated UI node identifier"),
                expected_kind: semantic_node_kind.expect("validated UI semantic node kind"),
            },
            Type::UiAssertNodeKind,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_action_kind" => (
            Effect::UiAssertActionKind {
                node_id: node_id.expect("validated UI node identifier"),
                expected_kind: semantic_action_kind.expect("validated UI semantic action kind"),
            },
            Type::UiAssertActionKind,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_action_unavailable_reason" => (
            Effect::UiAssertActionUnavailableReason {
                node_id: node_id.expect("validated UI node identifier"),
                expected: action_unavailable_reason_expected
                    .expect("validated UI action unavailable reason"),
            },
            Type::UiAssertActionUnavailableReason,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_form_field" => (
            Effect::UiAssertFormField {
                node_id: node_id.expect("validated UI node identifier"),
                field: form_field_key.expect("validated UI form field key"),
                expected: expected_text.expect("validated UI expected form field label"),
            },
            Type::UiAssertFormField,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_form_field_input_kind" => (
            Effect::UiAssertFormFieldInputKind {
                node_id: node_id.expect("validated UI node identifier"),
                field: form_field_key.expect("validated UI form field key"),
                input_kind: form_input_kind.expect("validated UI form input kind"),
            },
            Type::UiAssertFormFieldInputKind,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_form_field_required" => (
            Effect::UiAssertFormFieldRequired {
                node_id: node_id.expect("validated UI node identifier"),
                field: form_field_key.expect("validated UI form field key"),
                state: form_requirement_state.expect("validated UI form requirement state"),
            },
            Type::UiAssertFormFieldRequired,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_form_field_max_length" => (
            Effect::UiAssertFormFieldMaxLength {
                node_id: node_id.expect("validated UI node identifier"),
                field: form_field_key.expect("validated UI form field key"),
                max_length: form_max_length.expect("validated UI form max length"),
            },
            Type::UiAssertFormFieldMaxLength,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_form_field_placeholder" => (
            Effect::UiAssertFormFieldPlaceholder {
                node_id: node_id.expect("validated UI node identifier"),
                field: form_field_key.expect("validated UI form field key"),
                expected: form_placeholder_expected.expect("validated UI form placeholder"),
            },
            Type::UiAssertFormFieldPlaceholder,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_accessible_name" => (
            Effect::UiAssertAccessibleName {
                node_id: node_id.expect("validated UI node identifier"),
                expected: expected_text.expect("validated UI expected accessible name"),
            },
            Type::UiAssertAccessibleName,
            CAPABILITY_UI_PRESENTATION,
        ),
        "ui.assert_accessible_description" => (
            Effect::UiAssertAccessibleDescription {
                node_id: node_id.expect("validated UI node identifier"),
                expected: expected_text.expect("validated UI expected accessible description"),
            },
            Type::UiAssertAccessibleDescription,
            CAPABILITY_UI_PRESENTATION,
        ),
        _ => unreachable!("unknown effects returned above"),
    };
    Ok(LoweredEffect {
        effect,
        result_type,
        required_capabilities: vec![required_capability.to_string()],
    })
}

pub fn canonical_source(effect: &Effect) -> Result<String, CanonicalSourceError> {
    let source = format!("fn main() = {}", canonical_effect_source(effect, 0));
    let formatted = format_syntax(&parse(&source)).map_err(CanonicalSourceError::Syntax)?;
    let round_trip = lower(&parse(&formatted)).map_err(CanonicalSourceError::InvalidEffect)?;
    if round_trip.function.effect != *effect {
        return Err(CanonicalSourceError::RoundTripMismatch);
    }
    Ok(formatted)
}

fn canonical_effect_source(effect: &Effect, depth: usize) -> String {
    match effect {
        Effect::RuntimeList { filter } => format!(
            "runtime.list(\n{}environment: {},\n{}cluster: {},\n{}role: {},\n{})",
            indent(depth + 1),
            optional_string(filter.environment.as_deref()),
            indent(depth + 1),
            optional_string(filter.cluster.as_deref()),
            indent(depth + 1),
            optional_string(filter.role.as_deref()),
            indent(depth),
        ),
        Effect::RuntimeInspect { runtime_id } => {
            atomic_identifier_source("runtime.inspect", "runtime_id", runtime_id.as_str(), depth)
        }
        Effect::RuntimeHistory { runtime_id } => {
            atomic_identifier_source("runtime.history", "runtime_id", runtime_id.as_str(), depth)
        }
        Effect::RuntimeLogs { runtime_id } => {
            atomic_identifier_source("runtime.logs", "runtime_id", runtime_id.as_str(), depth)
        }
        Effect::RuntimeRefresh { runtime_id } => {
            atomic_identifier_source("runtime.refresh", "runtime_id", runtime_id.as_str(), depth)
        }
        Effect::RuntimeCapabilitiesRefresh { runtime_id } => atomic_identifier_source(
            "runtime.refresh_capabilities",
            "runtime_id",
            runtime_id.as_str(),
            depth,
        ),
        Effect::RuntimeDeploy {
            runtime_id,
            pipeline_kind,
            target,
        } => format!(
            "runtime.deploy(\n{}runtime_id: {},\n{}pipeline_kind: {},\n{}target: {},\n{})",
            indent(depth + 1),
            quote(runtime_id.as_str()),
            indent(depth + 1),
            quote(pipeline_kind),
            indent(depth + 1),
            optional_string(target.as_deref()),
            indent(depth),
        ),
        Effect::DebuggerCancel { session_id } => {
            atomic_identifier_source("debugger.cancel", "session_id", session_id, depth)
        }
        Effect::UiFocus { node_id } => {
            atomic_identifier_source("ui.focus", "node_id", node_id, depth)
        }
        Effect::UiNavigateFocus { node_id, direction } => format!(
            "ui.navigate_focus(\n{}node_id: {},\n{}direction: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(match direction {
                UiFocusNavigationDirection::Next => "next",
                UiFocusNavigationDirection::Previous => "previous",
                UiFocusNavigationDirection::First => "first",
                UiFocusNavigationDirection::Last => "last",
            }),
            indent(depth),
        ),
        Effect::UiScrollIntoView { node_id } => {
            atomic_identifier_source("ui.scroll_into_view", "node_id", node_id, depth)
        }
        Effect::UiAssertVisible { node_id } => {
            atomic_identifier_source("ui.assert_visible", "node_id", node_id, depth)
        }
        Effect::UiAssertHidden { node_id } => {
            atomic_identifier_source("ui.assert_hidden", "node_id", node_id, depth)
        }
        Effect::UiWaitHidden { node_id } => {
            atomic_identifier_source("ui.wait_hidden", "node_id", node_id, depth)
        }
        Effect::UiAssertRealized { node_id } => {
            atomic_identifier_source("ui.assert_realized", "node_id", node_id, depth)
        }
        Effect::UiWaitRealized { node_id } => {
            atomic_identifier_source("ui.wait_realized", "node_id", node_id, depth)
        }
        Effect::UiWaitVisible { node_id } => {
            atomic_identifier_source("ui.wait_visible", "node_id", node_id, depth)
        }
        Effect::UiWaitEnabled { node_id } => {
            atomic_identifier_source("ui.wait_enabled", "node_id", node_id, depth)
        }
        Effect::UiWaitDisabled { node_id } => {
            atomic_identifier_source("ui.wait_disabled", "node_id", node_id, depth)
        }
        Effect::UiAssertWindowOpen { node_id } => {
            atomic_identifier_source("ui.assert_window_open", "node_id", node_id, depth)
        }
        Effect::UiWaitWindowOpen { node_id } => {
            atomic_identifier_source("ui.wait_window_open", "node_id", node_id, depth)
        }
        Effect::UiWaitFocused { node_id } => {
            atomic_identifier_source("ui.wait_focused", "node_id", node_id, depth)
        }
        Effect::UiAssertFocused { node_id } => {
            atomic_identifier_source("ui.assert_focused", "node_id", node_id, depth)
        }
        Effect::UiAssertEnabled { node_id } => {
            atomic_identifier_source("ui.assert_enabled", "node_id", node_id, depth)
        }
        Effect::UiAssertDisabled { node_id } => {
            atomic_identifier_source("ui.assert_disabled", "node_id", node_id, depth)
        }
        Effect::UiAssertSelection { node_id, state } => format!(
            "ui.assert_selection(\n{}node_id: {},\n{}state: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(selection_state_source(*state)),
            indent(depth),
        ),
        Effect::UiWaitSelection { node_id, state } => format!(
            "ui.wait_selection(\n{}node_id: {},\n{}state: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(selection_state_source(*state)),
            indent(depth),
        ),
        Effect::UiAssertText { node_id, expected } => format!(
            "ui.assert_text(\n{}node_id: {},\n{}expected: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(expected),
            indent(depth),
        ),
        Effect::UiAssertAutomationId { node_id, expected } => format!(
            "ui.assert_automation_id(\n{}node_id: {},\n{}expected: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(expected),
            indent(depth),
        ),
        Effect::UiAssertNodeKind {
            node_id,
            expected_kind,
        } => format!(
            "ui.assert_node_kind(\n{}node_id: {},\n{}kind: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(semantic_node_kind_source(*expected_kind)),
            indent(depth),
        ),
        Effect::UiAssertActionKind {
            node_id,
            expected_kind,
        } => format!(
            "ui.assert_action_kind(\n{}node_id: {},\n{}kind: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(semantic_action_kind_source(*expected_kind)),
            indent(depth),
        ),
        Effect::UiAssertActionUnavailableReason { node_id, expected } => format!(
            "ui.assert_action_unavailable_reason(\n{}node_id: {},\n{}expected: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            optional_string(expected.as_deref()),
            indent(depth),
        ),
        Effect::UiAssertFormField {
            node_id,
            field,
            expected,
        } => format!(
            "ui.assert_form_field(\n{}node_id: {},\n{}field: {},\n{}expected: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(field),
            indent(depth + 1),
            quote(expected),
            indent(depth),
        ),
        Effect::UiAssertFormFieldInputKind {
            node_id,
            field,
            input_kind,
        } => format!(
            "ui.assert_form_field_input_kind(\n{}node_id: {},\n{}field: {},\n{}kind: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(field),
            indent(depth + 1),
            quote(form_input_kind_source(*input_kind)),
            indent(depth),
        ),
        Effect::UiAssertFormFieldRequired {
            node_id,
            field,
            state,
        } => format!(
            "ui.assert_form_field_required(\n{}node_id: {},\n{}field: {},\n{}state: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(field),
            indent(depth + 1),
            quote(form_requirement_state_source(*state)),
            indent(depth),
        ),
        Effect::UiAssertFormFieldMaxLength {
            node_id,
            field,
            max_length,
        } => format!(
            "ui.assert_form_field_max_length(\n{}node_id: {},\n{}field: {},\n{}max_length: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(field),
            indent(depth + 1),
            quote(&max_length.to_string()),
            indent(depth),
        ),
        Effect::UiAssertFormFieldPlaceholder {
            node_id,
            field,
            expected,
        } => format!(
            "ui.assert_form_field_placeholder(\n{}node_id: {},\n{}field: {},\n{}expected: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(field),
            indent(depth + 1),
            optional_string(expected.as_deref()),
            indent(depth),
        ),
        Effect::UiAssertAccessibleName { node_id, expected } => format!(
            "ui.assert_accessible_name(\n{}node_id: {},\n{}expected: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(expected),
            indent(depth),
        ),
        Effect::UiAssertAccessibleDescription { node_id, expected } => format!(
            "ui.assert_accessible_description(\n{}node_id: {},\n{}expected: {},\n{})",
            indent(depth + 1),
            quote(node_id),
            indent(depth + 1),
            quote(expected),
            indent(depth),
        ),
        Effect::All { branches } => {
            let mut source = String::from("all(\n");
            for branch in branches {
                source.push_str(&indent(depth + 1));
                source.push_str(&branch.name);
                source.push_str(": ");
                source.push_str(&canonical_effect_source(&branch.effect, depth + 1));
                source.push_str(",\n");
            }
            source.push_str(&indent(depth));
            source.push(')');
            source
        }
    }
}

fn selection_state_source(state: UiSelectionState) -> &'static str {
    match state {
        UiSelectionState::Selected => "selected",
        UiSelectionState::Unselected => "unselected",
    }
}

fn parse_semantic_node_kind(value: &str) -> Option<UiSemanticNodeKind> {
    Some(match value {
        "column" => UiSemanticNodeKind::Column,
        "heading" => UiSemanticNodeKind::Heading,
        "text" => UiSemanticNodeKind::Text,
        "runtime_card" => UiSemanticNodeKind::RuntimeCard,
        "runtime_workspace" => UiSemanticNodeKind::RuntimeWorkspace,
        "section" => UiSemanticNodeKind::Section,
        "history_entry" => UiSemanticNodeKind::HistoryEntry,
        "log_entry" => UiSemanticNodeKind::LogEntry,
        "debugger_workspace" => UiSemanticNodeKind::DebuggerWorkspace,
        "debugger_frame" => UiSemanticNodeKind::DebuggerFrame,
        "action" => UiSemanticNodeKind::Action,
        _ => return None,
    })
}

fn semantic_node_kind_source(kind: UiSemanticNodeKind) -> &'static str {
    match kind {
        UiSemanticNodeKind::Column => "column",
        UiSemanticNodeKind::Heading => "heading",
        UiSemanticNodeKind::Text => "text",
        UiSemanticNodeKind::RuntimeCard => "runtime_card",
        UiSemanticNodeKind::RuntimeWorkspace => "runtime_workspace",
        UiSemanticNodeKind::Section => "section",
        UiSemanticNodeKind::HistoryEntry => "history_entry",
        UiSemanticNodeKind::LogEntry => "log_entry",
        UiSemanticNodeKind::DebuggerWorkspace => "debugger_workspace",
        UiSemanticNodeKind::DebuggerFrame => "debugger_frame",
        UiSemanticNodeKind::Action => "action",
    }
}

fn parse_semantic_action_kind(value: &str) -> Option<UiSemanticActionKind> {
    Some(match value {
        "runtime_inspect" => UiSemanticActionKind::RuntimeInspect,
        "runtime_refresh" => UiSemanticActionKind::RuntimeRefresh,
        "runtime_capabilities_refresh" => UiSemanticActionKind::RuntimeCapabilitiesRefresh,
        "runtime_deploy" => UiSemanticActionKind::RuntimeDeploy,
        "debugger_cancel" => UiSemanticActionKind::DebuggerCancel,
        _ => return None,
    })
}

fn semantic_action_kind_source(kind: UiSemanticActionKind) -> &'static str {
    match kind {
        UiSemanticActionKind::RuntimeInspect => "runtime_inspect",
        UiSemanticActionKind::RuntimeRefresh => "runtime_refresh",
        UiSemanticActionKind::RuntimeCapabilitiesRefresh => "runtime_capabilities_refresh",
        UiSemanticActionKind::RuntimeDeploy => "runtime_deploy",
        UiSemanticActionKind::DebuggerCancel => "debugger_cancel",
    }
}

fn parse_form_input_kind(value: &str) -> Option<UiFormInputKind> {
    Some(match value {
        "path_token" => UiFormInputKind::PathToken,
        "trimmed_text" => UiFormInputKind::TrimmedText,
        _ => return None,
    })
}

fn form_input_kind_source(kind: UiFormInputKind) -> &'static str {
    match kind {
        UiFormInputKind::PathToken => "path_token",
        UiFormInputKind::TrimmedText => "trimmed_text",
    }
}

fn parse_form_requirement_state(value: &str) -> Option<UiFormRequirementState> {
    Some(match value {
        "required" => UiFormRequirementState::Required,
        "optional" => UiFormRequirementState::Optional,
        _ => return None,
    })
}

fn form_requirement_state_source(state: UiFormRequirementState) -> &'static str {
    match state {
        UiFormRequirementState::Required => "required",
        UiFormRequirementState::Optional => "optional",
    }
}

fn parse_form_max_length(value: &str) -> Option<usize> {
    if value.is_empty()
        || value.len() > 3
        || value.starts_with('0')
        || !value.bytes().all(|byte| byte.is_ascii_digit())
    {
        return None;
    }
    let parsed = value.parse::<usize>().ok()?;
    (1..=MAX_UI_FORM_FIELD_MAX_LENGTH)
        .contains(&parsed)
        .then_some(parsed)
}

pub fn validate_ui_node_id(node_id: &str) -> bool {
    !node_id.is_empty()
        && node_id.len() <= MAX_UI_NODE_ID_BYTES
        && node_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

pub fn validate_ui_form_field_key(field: &str) -> bool {
    !field.is_empty()
        && field.len() <= MAX_UI_FORM_FIELD_KEY_BYTES
        && field
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
}

pub fn validate_ui_expected_text(expected: &str) -> bool {
    expected.len() <= MAX_UI_EXPECTED_TEXT_BYTES && !expected.chars().any(char::is_control)
}

fn atomic_identifier_source(callee: &str, argument: &str, value: &str, depth: usize) -> String {
    format!(
        "{callee}(\n{}{argument}: {},\n{})",
        indent(depth + 1),
        quote(value),
        indent(depth),
    )
}

fn optional_string(value: Option<&str>) -> String {
    value.map_or_else(|| "none".to_string(), quote)
}

fn quote(value: &str) -> String {
    let mut output = String::with_capacity(value.len() + 2);
    output.push('"');
    for character in value.chars() {
        match character {
            '"' => output.push_str("\\\""),
            '\\' => output.push_str("\\\\"),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn indent(depth: usize) -> String {
    "  ".repeat(depth)
}

fn lower_all(
    arguments: &[leselang_syntax::NamedArgument],
    span: Span,
) -> Result<LoweredEffect, Vec<Diagnostic>> {
    if !(2..=MAX_ALL_BRANCHES).contains(&arguments.len()) {
        return Err(vec![Diagnostic {
            code: "LSH1201".to_string(),
            message: "all requires between 2 and 64 named branches".to_string(),
            span: Some(span),
        }]);
    }
    let mut names = HashSet::with_capacity(arguments.len());
    let mut branches = Vec::with_capacity(arguments.len());
    let mut capabilities = Vec::new();
    let mut diagnostics = Vec::new();
    for argument in arguments {
        if argument.name.len() > MAX_BRANCH_NAME_BYTES {
            diagnostics.push(Diagnostic {
                code: "LSH1203".to_string(),
                message: "all branch name exceeds 64 bytes".to_string(),
                span: Some(argument.span),
            });
            continue;
        }
        if !names.insert(argument.name.as_str()) {
            diagnostics.push(Diagnostic {
                code: "LSH1202".to_string(),
                message: format!("duplicate all branch '{}'", argument.name),
                span: Some(argument.span),
            });
            continue;
        }
        match lower_effect(&argument.value) {
            Ok(lowered) => {
                for capability in lowered.required_capabilities {
                    if !capabilities.contains(&capability) {
                        capabilities.push(capability);
                    }
                }
                branches.push(HirBranch {
                    name: argument.name.clone(),
                    effect: lowered.effect,
                    result_type: lowered.result_type,
                });
            }
            Err(mut errors) => diagnostics.append(&mut errors),
        }
    }
    if !diagnostics.is_empty() {
        return Err(diagnostics);
    }
    Ok(LoweredEffect {
        effect: Effect::All { branches },
        result_type: Type::Structured,
        required_capabilities: capabilities,
    })
}

fn expression_span(expression: &Expression) -> Span {
    match expression {
        Expression::Call { span, .. }
        | Expression::String { span, .. }
        | Expression::None { span } => *span,
    }
}

pub fn authorize(program: &HirProgram, capabilities: &CapabilitySet) -> Result<(), Diagnostic> {
    program
        .function
        .required_capabilities
        .iter()
        .find(|required| !capabilities.contains(required))
        .map_or(Ok(()), |required| {
            Err(Diagnostic {
                code: "LSH2001".to_string(),
                message: format!("missing capability '{required}'"),
                span: None,
            })
        })
}

#[cfg(test)]
mod tests {
    use leselang_syntax::parse;

    use super::*;

    #[test]
    fn runtime_list_lowers_to_typed_effect_and_normalized_filter() {
        let program = lower(&parse(
            "fn main() = runtime.list(environment: \" production \", cluster: none)",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::RuntimeList);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_READ]
        );
        let Effect::RuntimeList { filter } = program.function.effect else {
            panic!("expected runtime.list effect");
        };
        assert_eq!(filter.environment.as_deref(), Some("production"));
        assert_eq!(filter.cluster, None);
    }

    #[test]
    fn lowering_rejects_unknown_and_duplicate_arguments() {
        let errors = lower(&parse(
            "fn main() = runtime.list(role: \"edge\", role: none, mystery: \"x\")",
        ))
        .unwrap_err();
        assert!(errors.iter().any(|item| item.code == "LSH1101"));
        assert!(errors.iter().any(|item| item.code == "LSH1103"));
    }

    #[test]
    fn capability_check_is_explicit_and_origin_independent() {
        let program = lower(&parse("fn main() = runtime.list()")).unwrap();
        assert_eq!(
            authorize(&program, &CapabilitySet::default())
                .unwrap_err()
                .code,
            "LSH2001"
        );
        authorize(&program, &CapabilitySet::new([CAPABILITY_RUNTIME_READ])).unwrap();
    }

    #[test]
    fn runtime_refresh_lowers_to_typed_mutating_effect() {
        let program = lower(&parse(
            "fn main() = runtime.refresh(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::RuntimeRefresh);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_REFRESH]
        );
        assert!(matches!(
            program.function.effect,
            Effect::RuntimeRefresh { ref runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_capability_refresh_has_a_typed_shared_capability_contract() {
        let program = lower(&parse(
            "fn main() = runtime.refresh_capabilities(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        assert_eq!(
            program.function.result_type,
            Type::RuntimeCapabilitiesRefresh
        );
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_REFRESH]
        );
        assert!(matches!(
            program.function.effect,
            Effect::RuntimeCapabilitiesRefresh { ref runtime_id }
                if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_inspect_lowers_to_typed_read_effect() {
        let program = lower(&parse(
            "fn main() = runtime.inspect(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::RuntimeInspect);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_READ]
        );
        assert!(matches!(
            program.function.effect,
            Effect::RuntimeInspect { ref runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_history_lowers_to_typed_read_effect() {
        let program = lower(&parse(
            "fn main() = runtime.history(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::RuntimeHistory);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_READ]
        );
        assert!(matches!(
            program.function.effect,
            Effect::RuntimeHistory { ref runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_logs_lowers_to_typed_read_effect() {
        let program = lower(&parse(
            "fn main() = runtime.logs(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::RuntimeLogs);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_READ]
        );
        assert!(matches!(
            program.function.effect,
            Effect::RuntimeLogs { ref runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
    }

    #[test]
    fn runtime_refresh_requires_one_valid_identifier() {
        for source in [
            "fn main() = runtime.refresh()",
            "fn main() = runtime.refresh(runtime_id: none)",
            "fn main() = runtime.refresh(runtime_id: \"bad/id\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn runtime_logs_requires_one_valid_identifier() {
        for source in [
            "fn main() = runtime.logs()",
            "fn main() = runtime.logs(runtime_id: none)",
            "fn main() = runtime.logs(runtime_id: \"bad/id\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn runtime_deploy_lowers_a_bounded_explicit_intent() {
        let program = lower(&parse(
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\", pipeline_kind: \"http/request\", target: \"pid:42\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::RuntimeDeploy);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_DEPLOY]
        );
        assert!(matches!(
            program.function.effect,
            Effect::RuntimeDeploy {
                ref runtime_id,
                ref pipeline_kind,
                target: Some(ref target),
            } if runtime_id.as_str() == "runtime-a"
                && pipeline_kind == "http/request"
                && target == "pid:42"
        ));
        for source in [
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\")",
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\", pipeline_kind: none)",
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\", pipeline_kind: \"bad kind\")",
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\", pipeline_kind: \"http/request\", target: \" bad\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn debugger_cancel_lowers_and_canonical_source_round_trips() {
        let program = lower(&parse(
            "fn main() = debugger.cancel(session_id: \"session-a\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::DebuggerCancel);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_DEBUGGER_CONTROL]
        );
        assert!(matches!(
            program.function.effect,
            Effect::DebuggerCancel { ref session_id } if session_id == "session-a"
        ));

        let source = canonical_source(&program.function.effect).unwrap();
        assert_eq!(
            source,
            "fn main() = debugger.cancel(session_id: \"session-a\")\n"
        );
        assert_eq!(
            lower(&parse(&source)).unwrap().function.effect,
            program.function.effect
        );

        for source in [
            "fn main() = debugger.cancel()",
            "fn main() = debugger.cancel(session_id: none)",
            "fn main() = debugger.cancel(session_id: \"bad/session\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
        assert!(matches!(
            canonical_source(&Effect::DebuggerCancel {
                session_id: "bad/session".into(),
            }),
            Err(CanonicalSourceError::InvalidEffect(errors))
                if errors.iter().any(|error| error.code == "LSH1110")
        ));
    }

    #[test]
    fn ui_focus_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.focus(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiFocus);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiFocus { ref node_id } if node_id == "runtime-a:refresh"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.focus(node_id: \"runtime-a:refresh\")\n"
        );

        for source in [
            "fn main() = ui.focus()",
            "fn main() = ui.focus(node_id: none)",
            "fn main() = ui.focus(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_navigate_focus_is_a_typed_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.navigate_focus(node_id: \"runtime-a:inspect\", direction: \"next\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiNavigateFocus);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiNavigateFocus {
                ref node_id,
                direction: UiFocusNavigationDirection::Next,
            } if node_id == "runtime-a:inspect"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.navigate_focus(\n  node_id: \"runtime-a:inspect\",\n  direction: \"next\",\n)\n"
        );
        let first_program = lower(&parse(
            "fn main() = ui.navigate_focus(node_id: \"runtime-a:inspect\", direction: \"first\")",
        ))
        .unwrap();
        assert!(matches!(
            first_program.function.effect,
            Effect::UiNavigateFocus {
                direction: UiFocusNavigationDirection::First,
                ..
            }
        ));
        assert_eq!(
            canonical_source(&first_program.function.effect).unwrap(),
            "fn main() = ui.navigate_focus(\n  node_id: \"runtime-a:inspect\",\n  direction: \"first\",\n)\n"
        );
        let last_program = lower(&parse(
            "fn main() = ui.navigate_focus(node_id: \"runtime-a:inspect\", direction: \"last\")",
        ))
        .unwrap();
        assert!(matches!(
            last_program.function.effect,
            Effect::UiNavigateFocus {
                direction: UiFocusNavigationDirection::Last,
                ..
            }
        ));

        for source in [
            "fn main() = ui.navigate_focus(direction: \"next\")",
            "fn main() = ui.navigate_focus(node_id: \"runtime-a:inspect\")",
            "fn main() = ui.navigate_focus(node_id: none, direction: \"next\")",
            "fn main() = ui.navigate_focus(node_id: \"bad/node\", direction: \"next\")",
            "fn main() = ui.navigate_focus(node_id: \"runtime-a:inspect\", direction: none)",
            "fn main() = ui.navigate_focus(node_id: \"runtime-a:inspect\", direction: \"left\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_scroll_into_view_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.scroll_into_view(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiScrollIntoView);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiScrollIntoView { ref node_id } if node_id == "runtime-a:card"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.scroll_into_view(node_id: \"runtime-a:card\")\n"
        );

        for source in [
            "fn main() = ui.scroll_into_view()",
            "fn main() = ui.scroll_into_view(node_id: none)",
            "fn main() = ui.scroll_into_view(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_assert_visible_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.assert_visible(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertVisible);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertVisible { ref node_id } if node_id == "runtime-a:card"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_visible(node_id: \"runtime-a:card\")\n"
        );

        for source in [
            "fn main() = ui.assert_visible()",
            "fn main() = ui.assert_visible(node_id: none)",
            "fn main() = ui.assert_visible(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_assert_hidden_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.assert_hidden(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertHidden);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertHidden { ref node_id } if node_id == "runtime-a:card"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_hidden(node_id: \"runtime-a:card\")\n"
        );

        for source in [
            "fn main() = ui.assert_hidden()",
            "fn main() = ui.assert_hidden(node_id: none)",
            "fn main() = ui.assert_hidden(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_wait_hidden_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.wait_hidden(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiWaitHidden);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiWaitHidden { ref node_id } if node_id == "runtime-a:card"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.wait_hidden(node_id: \"runtime-a:card\")\n"
        );

        for source in [
            "fn main() = ui.wait_hidden()",
            "fn main() = ui.wait_hidden(node_id: none)",
            "fn main() = ui.wait_hidden(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_assert_realized_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.assert_realized(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertRealized);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertRealized { ref node_id } if node_id == "runtime-a:card"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_realized(node_id: \"runtime-a:card\")\n"
        );

        for source in [
            "fn main() = ui.assert_realized()",
            "fn main() = ui.assert_realized(node_id: none)",
            "fn main() = ui.assert_realized(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_wait_realized_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.wait_realized(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiWaitRealized);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiWaitRealized { ref node_id } if node_id == "runtime-a:card"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.wait_realized(node_id: \"runtime-a:card\")\n"
        );

        for source in [
            "fn main() = ui.wait_realized()",
            "fn main() = ui.wait_realized(node_id: none)",
            "fn main() = ui.wait_realized(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_wait_visible_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.wait_visible(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiWaitVisible);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiWaitVisible { ref node_id } if node_id == "runtime-a:card"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.wait_visible(node_id: \"runtime-a:card\")\n"
        );

        for source in [
            "fn main() = ui.wait_visible()",
            "fn main() = ui.wait_visible(node_id: none)",
            "fn main() = ui.wait_visible(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_wait_enabled_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.wait_enabled(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiWaitEnabled);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiWaitEnabled { ref node_id } if node_id == "runtime-a:refresh"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.wait_enabled(node_id: \"runtime-a:refresh\")\n"
        );

        for source in [
            "fn main() = ui.wait_enabled()",
            "fn main() = ui.wait_enabled(node_id: none)",
            "fn main() = ui.wait_enabled(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_wait_disabled_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.wait_disabled(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiWaitDisabled);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiWaitDisabled { ref node_id } if node_id == "runtime-a:refresh"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.wait_disabled(node_id: \"runtime-a:refresh\")\n"
        );

        for source in [
            "fn main() = ui.wait_disabled()",
            "fn main() = ui.wait_disabled(node_id: none)",
            "fn main() = ui.wait_disabled(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_assert_window_open_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.assert_window_open(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertWindowOpen);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertWindowOpen { ref node_id } if node_id == "runtime-a:card"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_window_open(node_id: \"runtime-a:card\")\n"
        );

        for source in [
            "fn main() = ui.assert_window_open()",
            "fn main() = ui.assert_window_open(node_id: none)",
            "fn main() = ui.assert_window_open(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_wait_window_open_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.wait_window_open(node_id: \"runtime-a:card\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiWaitWindowOpen);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiWaitWindowOpen { ref node_id } if node_id == "runtime-a:card"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.wait_window_open(node_id: \"runtime-a:card\")\n"
        );

        for source in [
            "fn main() = ui.wait_window_open()",
            "fn main() = ui.wait_window_open(node_id: none)",
            "fn main() = ui.wait_window_open(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_wait_focused_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.wait_focused(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiWaitFocused);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiWaitFocused { ref node_id } if node_id == "runtime-a:refresh"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.wait_focused(node_id: \"runtime-a:refresh\")\n"
        );

        for source in [
            "fn main() = ui.wait_focused()",
            "fn main() = ui.wait_focused(node_id: none)",
            "fn main() = ui.wait_focused(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "{source} should fail lowering"
            );
        }
    }

    #[test]
    fn ui_assert_focused_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.assert_focused(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertFocused);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertFocused { ref node_id } if node_id == "runtime-a:refresh"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_focused(node_id: \"runtime-a:refresh\")\n"
        );

        for source in [
            "fn main() = ui.assert_focused()",
            "fn main() = ui.assert_focused(node_id: none)",
            "fn main() = ui.assert_focused(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_assert_enabled_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.assert_enabled(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertEnabled);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertEnabled { ref node_id } if node_id == "runtime-a:refresh"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_enabled(node_id: \"runtime-a:refresh\")\n"
        );

        for source in [
            "fn main() = ui.assert_enabled()",
            "fn main() = ui.assert_enabled(node_id: none)",
            "fn main() = ui.assert_enabled(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_assert_disabled_is_a_capability_gated_canonical_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.assert_disabled(node_id: \"runtime-a:refresh\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertDisabled);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertDisabled { ref node_id } if node_id == "runtime-a:refresh"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_disabled(node_id: \"runtime-a:refresh\")\n"
        );

        for source in [
            "fn main() = ui.assert_disabled()",
            "fn main() = ui.assert_disabled(node_id: none)",
            "fn main() = ui.assert_disabled(node_id: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_selection_assert_and_wait_are_typed_canonical_presentation_effects() {
        let assert_program = lower(&parse(
            "fn main() = ui.assert_selection(node_id: \"runtime-a:card\", state: \"selected\")",
        ))
        .unwrap();
        assert_eq!(assert_program.function.result_type, Type::UiAssertSelection);
        assert_eq!(
            assert_program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            assert_program.function.effect,
            Effect::UiAssertSelection {
                ref node_id,
                state: UiSelectionState::Selected,
            } if node_id == "runtime-a:card"
        ));
        assert_eq!(
            canonical_source(&assert_program.function.effect).unwrap(),
            "fn main() = ui.assert_selection(\n  node_id: \"runtime-a:card\",\n  state: \"selected\",\n)\n"
        );

        let wait_program = lower(&parse(
            "fn main() = ui.wait_selection(node_id: \"runtime-b:card\", state: \"unselected\")",
        ))
        .unwrap();
        assert_eq!(wait_program.function.result_type, Type::UiWaitSelection);
        assert_eq!(
            wait_program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            wait_program.function.effect,
            Effect::UiWaitSelection {
                ref node_id,
                state: UiSelectionState::Unselected,
            } if node_id == "runtime-b:card"
        ));
        assert_eq!(
            canonical_source(&wait_program.function.effect).unwrap(),
            "fn main() = ui.wait_selection(\n  node_id: \"runtime-b:card\",\n  state: \"unselected\",\n)\n"
        );

        for source in [
            "fn main() = ui.assert_selection()",
            "fn main() = ui.assert_selection(node_id: \"runtime-a:card\")",
            "fn main() = ui.assert_selection(state: \"selected\")",
            "fn main() = ui.assert_selection(node_id: none, state: \"selected\")",
            "fn main() = ui.assert_selection(node_id: \"bad/node\", state: \"selected\")",
            "fn main() = ui.assert_selection(node_id: \"runtime-a:card\", state: none)",
            "fn main() = ui.assert_selection(node_id: \"runtime-a:card\", state: \"maybe\")",
            "fn main() = ui.wait_selection()",
            "fn main() = ui.wait_selection(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_selection(state: \"selected\")",
            "fn main() = ui.wait_selection(node_id: none, state: \"selected\")",
            "fn main() = ui.wait_selection(node_id: \"bad/node\", state: \"selected\")",
            "fn main() = ui.wait_selection(node_id: \"runtime-a:card\", state: none)",
            "fn main() = ui.wait_selection(node_id: \"runtime-a:card\", state: \"maybe\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_assert_text_is_a_capability_gated_bounded_presentation_effect() {
        let program = lower(&parse(
            "fn main() = ui.assert_text(node_id: \"fleet-title\", expected: \"Runtime fleet\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertText);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertText {
                ref node_id,
                ref expected,
            } if node_id == "fleet-title" && expected == "Runtime fleet"
        ));
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(
            lower(&parse(&canonical)).unwrap().function.effect,
            program.function.effect
        );

        for source in [
            "fn main() = ui.assert_text()",
            "fn main() = ui.assert_text(node_id: \"fleet-title\")",
            "fn main() = ui.assert_text(expected: \"Runtime fleet\")",
            "fn main() = ui.assert_text(node_id: \"bad/node\", expected: \"Runtime fleet\")",
            "fn main() = ui.assert_text(node_id: \"fleet-title\", expected: none)",
            "fn main() = ui.assert_text(node_id: \"fleet-title\", expected: \"bad\\ntext\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
        let oversized = format!(
            "fn main() = ui.assert_text(node_id: \"fleet-title\", expected: \"{}\")",
            "x".repeat(MAX_UI_EXPECTED_TEXT_BYTES + 1)
        );
        assert!(lower(&parse(&oversized)).is_err());
    }

    #[test]
    fn ui_assert_automation_id_is_capability_gated_and_identifier_bounded() {
        let program = lower(&parse(
            "fn main() = ui.assert_automation_id(node_id: \"fleet-title\", expected: \"fleet-title\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertAutomationId);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertAutomationId {
                ref node_id,
                ref expected,
            } if node_id == "fleet-title" && expected == "fleet-title"
        ));
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(
            lower(&parse(&canonical)).unwrap().function.effect,
            program.function.effect
        );

        for source in [
            "fn main() = ui.assert_automation_id()",
            "fn main() = ui.assert_automation_id(node_id: \"fleet-title\")",
            "fn main() = ui.assert_automation_id(expected: \"fleet-title\")",
            "fn main() = ui.assert_automation_id(node_id: \"bad/node\", expected: \"fleet-title\")",
            "fn main() = ui.assert_automation_id(node_id: \"fleet-title\", expected: none)",
            "fn main() = ui.assert_automation_id(node_id: \"fleet-title\", expected: \"bad/node\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
        let oversized = format!(
            "fn main() = ui.assert_automation_id(node_id: \"fleet-title\", expected: \"{}\")",
            "x".repeat(MAX_UI_NODE_ID_BYTES + 1)
        );
        assert!(lower(&parse(&oversized)).is_err());
    }

    #[test]
    fn ui_assert_node_kind_is_capability_gated_and_enum_typed() {
        let program = lower(&parse(
            "fn main() = ui.assert_node_kind(node_id: \"fleet-title\", kind: \"heading\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertNodeKind);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertNodeKind {
                ref node_id,
                expected_kind: UiSemanticNodeKind::Heading,
            } if node_id == "fleet-title"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_node_kind(\n  node_id: \"fleet-title\",\n  kind: \"heading\",\n)\n"
        );

        for source in [
            "fn main() = ui.assert_node_kind()",
            "fn main() = ui.assert_node_kind(node_id: \"fleet-title\")",
            "fn main() = ui.assert_node_kind(kind: \"heading\")",
            "fn main() = ui.assert_node_kind(node_id: \"bad/node\", kind: \"heading\")",
            "fn main() = ui.assert_node_kind(node_id: \"fleet-title\", kind: none)",
            "fn main() = ui.assert_node_kind(node_id: \"fleet-title\", kind: \"button\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_assert_action_kind_is_capability_gated_and_enum_typed() {
        let program = lower(&parse(
            "fn main() = ui.assert_action_kind(node_id: \"runtime-a:refresh\", kind: \"runtime_refresh\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertActionKind);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertActionKind {
                ref node_id,
                expected_kind: UiSemanticActionKind::RuntimeRefresh,
            } if node_id == "runtime-a:refresh"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_action_kind(\n  node_id: \"runtime-a:refresh\",\n  kind: \"runtime_refresh\",\n)\n"
        );

        for source in [
            "fn main() = ui.assert_action_kind()",
            "fn main() = ui.assert_action_kind(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.assert_action_kind(kind: \"runtime_refresh\")",
            "fn main() = ui.assert_action_kind(node_id: \"bad/node\", kind: \"runtime_refresh\")",
            "fn main() = ui.assert_action_kind(node_id: \"runtime-a:refresh\", kind: none)",
            "fn main() = ui.assert_action_kind(node_id: \"runtime-a:refresh\", kind: \"runtime.delete\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_assert_action_unavailable_reason_is_capability_gated_and_optional() {
        let program = lower(&parse(
            "fn main() = ui.assert_action_unavailable_reason(node_id: \"runtime-a:refresh\", expected: \"Verification action is temporarily unavailable\")",
        ))
        .unwrap();
        assert_eq!(
            program.function.result_type,
            Type::UiAssertActionUnavailableReason
        );
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert_eq!(
            program.function.effect,
            Effect::UiAssertActionUnavailableReason {
                node_id: "runtime-a:refresh".into(),
                expected: Some("Verification action is temporarily unavailable".into()),
            }
        );
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_action_unavailable_reason(\n  node_id: \"runtime-a:refresh\",\n  expected: \"Verification action is temporarily unavailable\",\n)\n"
        );
        let absent = lower(&parse(
            "fn main() = ui.assert_action_unavailable_reason(node_id: \"runtime-a:refresh\", expected: none)",
        ))
        .unwrap()
        .function
        .effect;
        assert_eq!(
            absent,
            Effect::UiAssertActionUnavailableReason {
                node_id: "runtime-a:refresh".into(),
                expected: None,
            }
        );
        assert_eq!(
            canonical_source(&absent).unwrap(),
            "fn main() = ui.assert_action_unavailable_reason(\n  node_id: \"runtime-a:refresh\",\n  expected: none,\n)\n"
        );

        for source in [
            "fn main() = ui.assert_action_unavailable_reason()",
            "fn main() = ui.assert_action_unavailable_reason(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.assert_action_unavailable_reason(expected: \"Verification action is temporarily unavailable\")",
            "fn main() = ui.assert_action_unavailable_reason(node_id: \"bad/node\", expected: \"Verification action is temporarily unavailable\")",
            "fn main() = ui.assert_action_unavailable_reason(node_id: \"runtime-a:refresh\", expected: \"bad\nreason\")",
            "fn main() = ui.assert_action_unavailable_reason(node_id: \"runtime-a:refresh\", expected: runtime.list())",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
        let oversized_expected = format!(
            "fn main() = ui.assert_action_unavailable_reason(node_id: \"runtime-a:refresh\", expected: \"{}\")",
            "x".repeat(MAX_UI_EXPECTED_TEXT_BYTES + 1)
        );
        assert!(lower(&parse(&oversized_expected)).is_err());
    }

    #[test]
    fn ui_assert_form_field_is_capability_gated_and_bounded() {
        let program = lower(&parse(
            "fn main() = ui.assert_form_field(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"Pipeline kind\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertFormField);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertFormField {
                ref node_id,
                ref field,
                ref expected,
            } if node_id == "workspace-runtime-a-deploy"
                && field == "pipeline_kind"
                && expected == "Pipeline kind"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_form_field(\n  node_id: \"workspace-runtime-a-deploy\",\n  field: \"pipeline_kind\",\n  expected: \"Pipeline kind\",\n)\n"
        );

        for source in [
            "fn main() = ui.assert_form_field()",
            "fn main() = ui.assert_form_field(node_id: \"workspace-runtime-a-deploy\")",
            "fn main() = ui.assert_form_field(field: \"pipeline_kind\", expected: \"Pipeline kind\")",
            "fn main() = ui.assert_form_field(node_id: \"bad/node\", field: \"pipeline_kind\", expected: \"Pipeline kind\")",
            "fn main() = ui.assert_form_field(node_id: \"workspace-runtime-a-deploy\", field: none, expected: \"Pipeline kind\")",
            "fn main() = ui.assert_form_field(node_id: \"workspace-runtime-a-deploy\", field: \"bad/field\", expected: \"Pipeline kind\")",
            "fn main() = ui.assert_form_field(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: none)",
            "fn main() = ui.assert_form_field(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"bad\\nfield\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
        let oversized_field = format!(
            "fn main() = ui.assert_form_field(node_id: \"workspace-runtime-a-deploy\", field: \"{}\", expected: \"Pipeline kind\")",
            "x".repeat(MAX_UI_FORM_FIELD_KEY_BYTES + 1)
        );
        assert!(lower(&parse(&oversized_field)).is_err());
        let oversized_expected = format!(
            "fn main() = ui.assert_form_field(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"{}\")",
            "x".repeat(MAX_UI_EXPECTED_TEXT_BYTES + 1)
        );
        assert!(lower(&parse(&oversized_expected)).is_err());
    }

    #[test]
    fn ui_assert_form_field_input_kind_is_capability_gated_and_enum_typed() {
        let program = lower(&parse(
            "fn main() = ui.assert_form_field_input_kind(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", kind: \"path_token\")",
        ))
        .unwrap();
        assert_eq!(
            program.function.result_type,
            Type::UiAssertFormFieldInputKind
        );
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertFormFieldInputKind {
                ref node_id,
                ref field,
                input_kind: UiFormInputKind::PathToken,
            } if node_id == "workspace-runtime-a-deploy" && field == "pipeline_kind"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_form_field_input_kind(\n  node_id: \"workspace-runtime-a-deploy\",\n  field: \"pipeline_kind\",\n  kind: \"path_token\",\n)\n"
        );

        for source in [
            "fn main() = ui.assert_form_field_input_kind()",
            "fn main() = ui.assert_form_field_input_kind(node_id: \"workspace-runtime-a-deploy\")",
            "fn main() = ui.assert_form_field_input_kind(field: \"pipeline_kind\", kind: \"path_token\")",
            "fn main() = ui.assert_form_field_input_kind(node_id: \"bad/node\", field: \"pipeline_kind\", kind: \"path_token\")",
            "fn main() = ui.assert_form_field_input_kind(node_id: \"workspace-runtime-a-deploy\", field: none, kind: \"path_token\")",
            "fn main() = ui.assert_form_field_input_kind(node_id: \"workspace-runtime-a-deploy\", field: \"bad/field\", kind: \"path_token\")",
            "fn main() = ui.assert_form_field_input_kind(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", kind: none)",
            "fn main() = ui.assert_form_field_input_kind(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", kind: \"free_text\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
        let oversized_field = format!(
            "fn main() = ui.assert_form_field_input_kind(node_id: \"workspace-runtime-a-deploy\", field: \"{}\", kind: \"trimmed_text\")",
            "x".repeat(MAX_UI_FORM_FIELD_KEY_BYTES + 1)
        );
        assert!(lower(&parse(&oversized_field)).is_err());
    }

    #[test]
    fn ui_assert_form_field_required_is_capability_gated_and_enum_typed() {
        let program = lower(&parse(
            "fn main() = ui.assert_form_field_required(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", state: \"required\")",
        ))
        .unwrap();
        assert_eq!(
            program.function.result_type,
            Type::UiAssertFormFieldRequired
        );
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertFormFieldRequired {
                ref node_id,
                ref field,
                state: UiFormRequirementState::Required,
            } if node_id == "workspace-runtime-a-deploy" && field == "pipeline_kind"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_form_field_required(\n  node_id: \"workspace-runtime-a-deploy\",\n  field: \"pipeline_kind\",\n  state: \"required\",\n)\n"
        );

        for source in [
            "fn main() = ui.assert_form_field_required()",
            "fn main() = ui.assert_form_field_required(node_id: \"workspace-runtime-a-deploy\")",
            "fn main() = ui.assert_form_field_required(field: \"pipeline_kind\", state: \"required\")",
            "fn main() = ui.assert_form_field_required(node_id: \"bad/node\", field: \"pipeline_kind\", state: \"required\")",
            "fn main() = ui.assert_form_field_required(node_id: \"workspace-runtime-a-deploy\", field: none, state: \"required\")",
            "fn main() = ui.assert_form_field_required(node_id: \"workspace-runtime-a-deploy\", field: \"bad/field\", state: \"required\")",
            "fn main() = ui.assert_form_field_required(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", state: none)",
            "fn main() = ui.assert_form_field_required(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", state: \"maybe\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
        let optional = lower(&parse(
            "fn main() = ui.assert_form_field_required(node_id: \"workspace-runtime-a-deploy\", field: \"target\", state: \"optional\")",
        ))
        .unwrap();
        assert!(matches!(
            optional.function.effect,
            Effect::UiAssertFormFieldRequired {
                state: UiFormRequirementState::Optional,
                ..
            }
        ));
        let oversized_field = format!(
            "fn main() = ui.assert_form_field_required(node_id: \"workspace-runtime-a-deploy\", field: \"{}\", state: \"optional\")",
            "x".repeat(MAX_UI_FORM_FIELD_KEY_BYTES + 1)
        );
        assert!(lower(&parse(&oversized_field)).is_err());
    }

    #[test]
    fn ui_assert_form_field_max_length_is_capability_gated_and_bounded() {
        let program = lower(&parse(
            "fn main() = ui.assert_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", max_length: \"128\")",
        ))
        .unwrap();
        assert_eq!(
            program.function.result_type,
            Type::UiAssertFormFieldMaxLength
        );
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertFormFieldMaxLength {
                ref node_id,
                ref field,
                max_length: 128,
            } if node_id == "workspace-runtime-a-deploy" && field == "pipeline_kind"
        ));
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_form_field_max_length(\n  node_id: \"workspace-runtime-a-deploy\",\n  field: \"pipeline_kind\",\n  max_length: \"128\",\n)\n"
        );

        for source in [
            "fn main() = ui.assert_form_field_max_length()",
            "fn main() = ui.assert_form_field_max_length(node_id: \"workspace-runtime-a-deploy\")",
            "fn main() = ui.assert_form_field_max_length(field: \"pipeline_kind\", max_length: \"128\")",
            "fn main() = ui.assert_form_field_max_length(node_id: \"bad/node\", field: \"pipeline_kind\", max_length: \"128\")",
            "fn main() = ui.assert_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: none, max_length: \"128\")",
            "fn main() = ui.assert_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: \"bad/field\", max_length: \"128\")",
            "fn main() = ui.assert_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", max_length: none)",
            "fn main() = ui.assert_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", max_length: \"0\")",
            "fn main() = ui.assert_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", max_length: \"001\")",
            "fn main() = ui.assert_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", max_length: \"257\")",
            "fn main() = ui.assert_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", max_length: \"12x\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
        let oversized_field = format!(
            "fn main() = ui.assert_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: \"{}\", max_length: \"128\")",
            "x".repeat(MAX_UI_FORM_FIELD_KEY_BYTES + 1)
        );
        assert!(lower(&parse(&oversized_field)).is_err());
    }

    #[test]
    fn ui_assert_form_field_placeholder_is_capability_gated_and_optional() {
        let program = lower(&parse(
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"http/request\")",
        ))
        .unwrap();
        assert_eq!(
            program.function.result_type,
            Type::UiAssertFormFieldPlaceholder
        );
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert_eq!(
            program.function.effect,
            Effect::UiAssertFormFieldPlaceholder {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                expected: Some("http/request".into()),
            }
        );
        assert_eq!(
            canonical_source(&program.function.effect).unwrap(),
            "fn main() = ui.assert_form_field_placeholder(\n  node_id: \"workspace-runtime-a-deploy\",\n  field: \"pipeline_kind\",\n  expected: \"http/request\",\n)\n"
        );
        let absent = lower(&parse(
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: none)",
        ))
        .unwrap()
        .function
        .effect;
        assert_eq!(
            absent,
            Effect::UiAssertFormFieldPlaceholder {
                node_id: "workspace-runtime-a-deploy".into(),
                field: "pipeline_kind".into(),
                expected: None,
            }
        );
        assert_eq!(
            canonical_source(&absent).unwrap(),
            "fn main() = ui.assert_form_field_placeholder(\n  node_id: \"workspace-runtime-a-deploy\",\n  field: \"pipeline_kind\",\n  expected: none,\n)\n"
        );

        for source in [
            "fn main() = ui.assert_form_field_placeholder()",
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\")",
            "fn main() = ui.assert_form_field_placeholder(field: \"pipeline_kind\", expected: \"http/request\")",
            "fn main() = ui.assert_form_field_placeholder(node_id: \"bad/node\", field: \"pipeline_kind\", expected: \"http/request\")",
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: none, expected: \"http/request\")",
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"bad/field\", expected: \"http/request\")",
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\")",
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"bad\nplaceholder\")",
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: runtime.list())",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
        let oversized_field = format!(
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"{}\", expected: \"http/request\")",
            "x".repeat(MAX_UI_FORM_FIELD_KEY_BYTES + 1)
        );
        assert!(lower(&parse(&oversized_field)).is_err());
        let oversized_expected = format!(
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"{}\")",
            "x".repeat(MAX_UI_EXPECTED_TEXT_BYTES + 1)
        );
        assert!(lower(&parse(&oversized_expected)).is_err());
    }

    #[test]
    fn ui_assert_accessible_name_is_capability_gated_and_bounded() {
        let program = lower(&parse(
            "fn main() = ui.assert_accessible_name(node_id: \"fleet-title\", expected: \"Runtime fleet\")",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::UiAssertAccessibleName);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertAccessibleName {
                ref node_id,
                ref expected,
            } if node_id == "fleet-title" && expected == "Runtime fleet"
        ));
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(
            lower(&parse(&canonical)).unwrap().function.effect,
            program.function.effect
        );

        for source in [
            "fn main() = ui.assert_accessible_name()",
            "fn main() = ui.assert_accessible_name(node_id: \"fleet-title\")",
            "fn main() = ui.assert_accessible_name(expected: \"Runtime fleet\")",
            "fn main() = ui.assert_accessible_name(node_id: \"bad/node\", expected: \"Runtime fleet\")",
            "fn main() = ui.assert_accessible_name(node_id: \"fleet-title\", expected: none)",
            "fn main() = ui.assert_accessible_name(node_id: \"fleet-title\", expected: \"bad\\nname\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
    }

    #[test]
    fn ui_assert_accessible_description_is_capability_gated_and_bounded() {
        let program = lower(&parse(
            "fn main() = ui.assert_accessible_description(node_id: \"runtime-runtime-a-inspect\", expected: \"Open the read-only runtime workspace\")",
        ))
        .unwrap();
        assert_eq!(
            program.function.result_type,
            Type::UiAssertAccessibleDescription
        );
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_UI_PRESENTATION]
        );
        assert!(matches!(
            program.function.effect,
            Effect::UiAssertAccessibleDescription {
                ref node_id,
                ref expected,
            } if node_id == "runtime-runtime-a-inspect"
                && expected == "Open the read-only runtime workspace"
        ));
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(
            lower(&parse(&canonical)).unwrap().function.effect,
            program.function.effect
        );

        for source in [
            "fn main() = ui.assert_accessible_description()",
            "fn main() = ui.assert_accessible_description(node_id: \"runtime-runtime-a-inspect\")",
            "fn main() = ui.assert_accessible_description(expected: \"description\")",
            "fn main() = ui.assert_accessible_description(node_id: \"bad/node\", expected: \"description\")",
            "fn main() = ui.assert_accessible_description(node_id: \"runtime-runtime-a-inspect\", expected: none)",
            "fn main() = ui.assert_accessible_description(node_id: \"runtime-runtime-a-inspect\", expected: \"bad\\ndescription\")",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
        let oversized = format!(
            "fn main() = ui.assert_accessible_description(node_id: \"runtime-runtime-a-inspect\", expected: \"{}\")",
            "x".repeat(MAX_UI_EXPECTED_TEXT_BYTES + 1)
        );
        assert!(lower(&parse(&oversized)).is_err());
    }

    #[test]
    fn every_atomic_effect_has_one_semantically_stable_canonical_source() {
        for source in [
            "fn main() = runtime.list(environment: \"prod\", cluster: none, role: \"edge\")",
            "fn main() = runtime.inspect(runtime_id: \"runtime-a\")",
            "fn main() = runtime.history(runtime_id: \"runtime-a\")",
            "fn main() = runtime.logs(runtime_id: \"runtime-a\")",
            "fn main() = runtime.refresh(runtime_id: \"runtime-a\")",
            "fn main() = runtime.refresh_capabilities(runtime_id: \"runtime-a\")",
            "fn main() = runtime.deploy(runtime_id: \"runtime-a\", pipeline_kind: \"http/request\", target: none)",
            "fn main() = debugger.cancel(session_id: \"session-a\")",
            "fn main() = ui.focus(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.navigate_focus(node_id: \"runtime-a:refresh\", direction: \"previous\")",
            "fn main() = ui.scroll_into_view(node_id: \"runtime-a:card\")",
            "fn main() = ui.assert_visible(node_id: \"runtime-a:card\")",
            "fn main() = ui.assert_hidden(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_hidden(node_id: \"runtime-a:card\")",
            "fn main() = ui.assert_realized(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_realized(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_visible(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_enabled(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.wait_disabled(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.assert_window_open(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_window_open(node_id: \"runtime-a:card\")",
            "fn main() = ui.wait_focused(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.assert_focused(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.assert_enabled(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.assert_disabled(node_id: \"runtime-a:refresh\")",
            "fn main() = ui.assert_selection(node_id: \"runtime-a:card\", state: \"selected\")",
            "fn main() = ui.wait_selection(node_id: \"runtime-a:card\", state: \"unselected\")",
            "fn main() = ui.assert_text(node_id: \"fleet-title\", expected: \"Runtime fleet\")",
            "fn main() = ui.assert_automation_id(node_id: \"fleet-title\", expected: \"fleet-title\")",
            "fn main() = ui.assert_node_kind(node_id: \"fleet-title\", kind: \"heading\")",
            "fn main() = ui.assert_action_kind(node_id: \"runtime-a:refresh\", kind: \"runtime_refresh\")",
            "fn main() = ui.assert_action_unavailable_reason(node_id: \"runtime-a:refresh\", expected: \"Verification action is temporarily unavailable\")",
            "fn main() = ui.assert_form_field(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"Pipeline kind\")",
            "fn main() = ui.assert_form_field_input_kind(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", kind: \"path_token\")",
            "fn main() = ui.assert_form_field_required(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", state: \"required\")",
            "fn main() = ui.assert_form_field_max_length(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", max_length: \"128\")",
            "fn main() = ui.assert_form_field_placeholder(node_id: \"workspace-runtime-a-deploy\", field: \"pipeline_kind\", expected: \"http/request\")",
            "fn main() = ui.assert_accessible_name(node_id: \"fleet-title\", expected: \"Runtime fleet\")",
            "fn main() = ui.assert_accessible_description(node_id: \"runtime-runtime-a-inspect\", expected: \"Open the read-only runtime workspace\")",
        ] {
            let effect = lower(&parse(source)).unwrap().function.effect;
            let canonical = canonical_source(&effect).unwrap();
            assert_eq!(
                lower(&parse(&canonical)).unwrap().function.effect,
                effect,
                "canonical source changed effect `{source}`"
            );
        }
    }

    #[test]
    fn all_lowers_branches_in_declared_order_and_unions_capabilities() {
        let program = lower(&parse(
            "fn main() = all(inventory: runtime.list(role: \"edge\"), refresh: runtime.refresh(runtime_id: \"runtime-a\"))",
        ))
        .unwrap();
        assert_eq!(program.function.result_type, Type::Structured);
        assert_eq!(
            program.function.required_capabilities,
            [CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH]
        );
        let Effect::All { branches } = program.function.effect else {
            panic!("expected all effect");
        };
        assert_eq!(
            branches
                .iter()
                .map(|branch| branch.name.as_str())
                .collect::<Vec<_>>(),
            ["inventory", "refresh"]
        );
        assert_eq!(branches[0].result_type, Type::RuntimeList);
        assert_eq!(branches[1].result_type, Type::RuntimeRefresh);
    }

    #[test]
    fn all_rejects_invalid_shape_and_authorizes_every_branch() {
        for source in [
            "fn main() = all(only: runtime.list())",
            "fn main() = all(left: runtime.list(), left: runtime.list())",
            "fn main() = all(left: \"not-an-effect\", right: runtime.list())",
        ] {
            assert!(
                lower(&parse(source)).is_err(),
                "source should fail: {source}"
            );
        }
        let long_name = "x".repeat(MAX_BRANCH_NAME_BYTES + 1);
        assert!(
            lower(&parse(&format!(
                "fn main() = all({long_name}: runtime.list(), right: runtime.list())"
            )))
            .unwrap_err()
            .iter()
            .any(|item| item.code == "LSH1203")
        );
        let program = lower(&parse(
            "fn main() = all(read: runtime.list(), write: runtime.refresh(runtime_id: \"runtime-a\"))",
        ))
        .unwrap();
        let error =
            authorize(&program, &CapabilitySet::new([CAPABILITY_RUNTIME_READ])).unwrap_err();
        assert_eq!(error.code, "LSH2001");
        assert!(error.message.contains(CAPABILITY_RUNTIME_REFRESH));
    }
}
