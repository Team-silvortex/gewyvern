//! Leserpent-owned host operation and reply wire types, independent of VM state.
//!
//! These are product protocol payloads, not execution authority. Hosts must still
//! validate grants, revisions, pending operations and reply provenance before use.

use leselang_hir::{
    UiFocusNavigationDirection, UiFormInputKind, UiFormRequirementState, UiSelectionState,
    UiSemanticActionKind, UiSemanticNodeKind,
};
use leselang_host_contract::{CapabilitySet, Principal};
use leserpent_domain::{CommandEnvelope, CommandId, CommandResult, QueryEnvelope, QueryResult};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum EffectOperation {
    Query(QueryEnvelope),
    Command(CommandEnvelope),
    Presentation(PresentationEnvelope),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PresentationEnvelope {
    pub schema_version: u32,
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub operation: PresentationOperation,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PresentationOperation {
    Activate {
        node_id: String,
    },
    Focus {
        node_id: String,
    },
    NavigateFocus {
        node_id: String,
        direction: UiFocusNavigationDirection,
    },
    ScrollIntoView {
        node_id: String,
    },
    AssertVisible {
        node_id: String,
    },
    AssertHidden {
        node_id: String,
    },
    WaitHidden {
        node_id: String,
        timeout_ms: u64,
    },
    AssertRealized {
        node_id: String,
    },
    WaitRealized {
        node_id: String,
        timeout_ms: u64,
    },
    WaitVisible {
        node_id: String,
        timeout_ms: u64,
    },
    WaitEnabled {
        node_id: String,
        timeout_ms: u64,
    },
    WaitDisabled {
        node_id: String,
        timeout_ms: u64,
    },
    OpenWindow {
        node_id: String,
    },
    CloseWindow {
        node_id: String,
    },
    AssertWindowOpen {
        node_id: String,
    },
    WaitWindowOpen {
        node_id: String,
        timeout_ms: u64,
    },
    AssertWindowClosed {
        node_id: String,
    },
    WaitWindowClosed {
        node_id: String,
        timeout_ms: u64,
    },
    WaitFocused {
        node_id: String,
        timeout_ms: u64,
    },
    AssertFocused {
        node_id: String,
    },
    WaitUnfocused {
        node_id: String,
        timeout_ms: u64,
    },
    AssertUnfocused {
        node_id: String,
    },
    AssertEnabled {
        node_id: String,
    },
    AssertDisabled {
        node_id: String,
    },
    AssertChildCount {
        node_id: String,
        count: usize,
    },
    WaitChildCount {
        node_id: String,
        count: usize,
        timeout_ms: u64,
    },
    SetSelection {
        node_id: String,
        state: UiSelectionState,
    },
    AssertSelection {
        node_id: String,
        state: UiSelectionState,
    },
    WaitSelection {
        node_id: String,
        state: UiSelectionState,
        timeout_ms: u64,
    },
    AssertText {
        node_id: String,
        expected: String,
    },
    WaitText {
        node_id: String,
        expected: String,
        timeout_ms: u64,
    },
    AssertAutomationId {
        node_id: String,
        expected: String,
    },
    WaitAutomationId {
        node_id: String,
        expected: String,
        timeout_ms: u64,
    },
    AssertNodeKind {
        node_id: String,
        expected_kind: UiSemanticNodeKind,
    },
    WaitNodeKind {
        node_id: String,
        expected_kind: UiSemanticNodeKind,
        timeout_ms: u64,
    },
    AssertActionKind {
        node_id: String,
        expected_kind: UiSemanticActionKind,
    },
    WaitActionKind {
        node_id: String,
        expected_kind: UiSemanticActionKind,
        timeout_ms: u64,
    },
    AssertActionLabel {
        node_id: String,
        expected: String,
    },
    WaitActionLabel {
        node_id: String,
        expected: String,
        timeout_ms: u64,
    },
    AssertActionAvailable {
        node_id: String,
    },
    WaitActionAvailable {
        node_id: String,
        timeout_ms: u64,
    },
    AssertActionUnavailableReason {
        node_id: String,
        expected: Option<String>,
    },
    WaitActionUnavailableReason {
        node_id: String,
        expected: Option<String>,
        timeout_ms: u64,
    },
    SubmitForm {
        node_id: String,
    },
    CancelForm {
        node_id: String,
    },
    SetFormValue {
        node_id: String,
        field: String,
        value: String,
    },
    AssertFormValue {
        node_id: String,
        field: String,
        expected: String,
    },
    WaitFormValue {
        node_id: String,
        field: String,
        expected: String,
        timeout_ms: u64,
    },
    AssertFormField {
        node_id: String,
        field: String,
        expected: String,
    },
    WaitFormField {
        node_id: String,
        field: String,
        expected: String,
        timeout_ms: u64,
    },
    AssertFormFieldInputKind {
        node_id: String,
        field: String,
        input_kind: UiFormInputKind,
    },
    WaitFormFieldInputKind {
        node_id: String,
        field: String,
        input_kind: UiFormInputKind,
        timeout_ms: u64,
    },
    AssertFormFieldRequired {
        node_id: String,
        field: String,
        state: UiFormRequirementState,
    },
    WaitFormFieldRequired {
        node_id: String,
        field: String,
        state: UiFormRequirementState,
        timeout_ms: u64,
    },
    AssertFormFieldMaxLength {
        node_id: String,
        field: String,
        max_length: usize,
    },
    WaitFormFieldMaxLength {
        node_id: String,
        field: String,
        max_length: usize,
        timeout_ms: u64,
    },
    AssertFormFieldPlaceholder {
        node_id: String,
        field: String,
        expected: Option<String>,
    },
    WaitFormFieldPlaceholder {
        node_id: String,
        field: String,
        expected: Option<String>,
        timeout_ms: u64,
    },
    AssertAccessibleName {
        node_id: String,
        expected: String,
    },
    WaitAccessibleName {
        node_id: String,
        expected: String,
        timeout_ms: u64,
    },
    AssertAccessibleDescription {
        node_id: String,
        expected: String,
    },
    WaitAccessibleDescription {
        node_id: String,
        expected: String,
        timeout_ms: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
// Preserve the established VM result API until the v2 schema seal.
#[allow(clippy::large_enum_variant)]
pub enum EffectResult {
    Query(QueryResult),
    Command(Box<CommandResult>),
    DebuggerCancel(DebuggerCancelResult),
    Presentation(PresentationResult),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum PresentationResult {
    Activate {
        node_id: String,
    },
    Focus {
        node_id: String,
    },
    NavigateFocus {
        node_id: String,
        direction: UiFocusNavigationDirection,
        focused_node_id: String,
    },
    ScrollIntoView {
        node_id: String,
    },
    AssertVisible {
        node_id: String,
    },
    AssertHidden {
        node_id: String,
    },
    WaitHidden {
        node_id: String,
        timeout_ms: u64,
    },
    AssertRealized {
        node_id: String,
    },
    WaitRealized {
        node_id: String,
        timeout_ms: u64,
    },
    WaitVisible {
        node_id: String,
        timeout_ms: u64,
    },
    WaitEnabled {
        node_id: String,
        timeout_ms: u64,
    },
    WaitDisabled {
        node_id: String,
        timeout_ms: u64,
    },
    OpenWindow {
        node_id: String,
    },
    CloseWindow {
        node_id: String,
    },
    AssertWindowOpen {
        node_id: String,
    },
    WaitWindowOpen {
        node_id: String,
        timeout_ms: u64,
    },
    AssertWindowClosed {
        node_id: String,
    },
    WaitWindowClosed {
        node_id: String,
        timeout_ms: u64,
    },
    WaitFocused {
        node_id: String,
        timeout_ms: u64,
    },
    AssertFocused {
        node_id: String,
    },
    WaitUnfocused {
        node_id: String,
        timeout_ms: u64,
    },
    AssertUnfocused {
        node_id: String,
    },
    AssertEnabled {
        node_id: String,
    },
    AssertDisabled {
        node_id: String,
    },
    AssertChildCount {
        node_id: String,
        count: usize,
    },
    WaitChildCount {
        node_id: String,
        count: usize,
        timeout_ms: u64,
    },
    SetSelection {
        node_id: String,
        state: UiSelectionState,
    },
    AssertSelection {
        node_id: String,
        state: UiSelectionState,
    },
    WaitSelection {
        node_id: String,
        state: UiSelectionState,
        timeout_ms: u64,
    },
    AssertText {
        node_id: String,
        expected: String,
    },
    WaitText {
        node_id: String,
        expected: String,
        timeout_ms: u64,
    },
    AssertAutomationId {
        node_id: String,
        expected: String,
    },
    WaitAutomationId {
        node_id: String,
        expected: String,
        timeout_ms: u64,
    },
    AssertNodeKind {
        node_id: String,
        expected_kind: UiSemanticNodeKind,
    },
    WaitNodeKind {
        node_id: String,
        expected_kind: UiSemanticNodeKind,
        timeout_ms: u64,
    },
    AssertActionKind {
        node_id: String,
        expected_kind: UiSemanticActionKind,
    },
    WaitActionKind {
        node_id: String,
        expected_kind: UiSemanticActionKind,
        timeout_ms: u64,
    },
    AssertActionLabel {
        node_id: String,
        expected: String,
    },
    WaitActionLabel {
        node_id: String,
        expected: String,
        timeout_ms: u64,
    },
    AssertActionAvailable {
        node_id: String,
    },
    WaitActionAvailable {
        node_id: String,
        timeout_ms: u64,
    },
    AssertActionUnavailableReason {
        node_id: String,
        expected: Option<String>,
    },
    WaitActionUnavailableReason {
        node_id: String,
        expected: Option<String>,
        timeout_ms: u64,
    },
    SubmitForm {
        node_id: String,
    },
    CancelForm {
        node_id: String,
    },
    SetFormValue {
        node_id: String,
        field: String,
        value: String,
    },
    AssertFormValue {
        node_id: String,
        field: String,
        expected: String,
    },
    WaitFormValue {
        node_id: String,
        field: String,
        expected: String,
        timeout_ms: u64,
    },
    AssertFormField {
        node_id: String,
        field: String,
        expected: String,
    },
    WaitFormField {
        node_id: String,
        field: String,
        expected: String,
        timeout_ms: u64,
    },
    AssertFormFieldInputKind {
        node_id: String,
        field: String,
        input_kind: UiFormInputKind,
    },
    WaitFormFieldInputKind {
        node_id: String,
        field: String,
        input_kind: UiFormInputKind,
        timeout_ms: u64,
    },
    AssertFormFieldRequired {
        node_id: String,
        field: String,
        state: UiFormRequirementState,
    },
    WaitFormFieldRequired {
        node_id: String,
        field: String,
        state: UiFormRequirementState,
        timeout_ms: u64,
    },
    AssertFormFieldMaxLength {
        node_id: String,
        field: String,
        max_length: usize,
    },
    WaitFormFieldMaxLength {
        node_id: String,
        field: String,
        max_length: usize,
        timeout_ms: u64,
    },
    AssertFormFieldPlaceholder {
        node_id: String,
        field: String,
        expected: Option<String>,
    },
    WaitFormFieldPlaceholder {
        node_id: String,
        field: String,
        expected: Option<String>,
        timeout_ms: u64,
    },
    AssertAccessibleName {
        node_id: String,
        expected: String,
    },
    WaitAccessibleName {
        node_id: String,
        expected: String,
        timeout_ms: u64,
    },
    AssertAccessibleDescription {
        node_id: String,
        expected: String,
    },
    WaitAccessibleDescription {
        node_id: String,
        expected: String,
        timeout_ms: u64,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerCancelResult {
    pub command_id: CommandId,
    pub session_id: String,
    pub observed_at_ms: u64,
}
