use std::collections::{BTreeMap, BTreeSet};

use leselang_command::{LoweringContext, LoweringError, plan_runtime_refresh};
use leserpent_domain::{CommandPlan, QueryResult, RefreshStatus, Revision, RuntimeId};
use serde::{Deserialize, Serialize};

pub const UI_SCHEMA_VERSION: u32 = 1;
pub const MAX_UI_NODES: usize = 4_096;
pub const MAX_UI_DEPTH: usize = 32;
pub const MAX_UI_TEXT_BYTES: usize = 1_024;
pub const MAX_UI_PATCH_OPERATIONS: usize = 8_192;
pub const MAX_UI_IR_BYTES: usize = 2 * 1024 * 1024;
pub const MAX_RUNTIME_LOG_ENTRIES: usize = 256;
pub const MAX_RUNTIME_LOG_DISPLAY_BYTES: usize = 768;
pub const MAX_DEBUGGER_FRAMES: usize = 64;
pub const MAX_DEBUGGER_DISPLAY_BYTES: usize = 512;
pub const MAX_DEBUGGER_DEADLINE_REMAINING_MS: u64 = 24 * 60 * 60 * 1_000;

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct NodeId(String);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiDocument {
    pub schema_version: u32,
    pub revision: Revision,
    pub root: UiNode,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiNode {
    pub id: NodeId,
    pub kind: UiNodeKind,
    pub runtime_id: Option<RuntimeId>,
    pub text: Option<LocalizedText>,
    pub accessibility: Accessibility,
    pub action: Option<UiAction>,
    pub children: Vec<UiNode>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiNodeKind {
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
pub enum RuntimeLogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeLogEntry {
    pub sequence: u64,
    pub level: RuntimeLogLevel,
    pub display: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RuntimeLogProjection {
    pub revision: Revision,
    pub runtime_id: RuntimeId,
    pub runtime_name: String,
    pub entries: Vec<RuntimeLogEntry>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebuggerState {
    Running,
    WaitingEffect,
    Yielded,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebuggerEffectKind {
    RuntimeList,
    RuntimeInspect,
    RuntimeHistory,
    RuntimeRefresh,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerPendingEffect {
    pub effect_id: String,
    pub kind: DebuggerEffectKind,
    pub runtime_id: Option<RuntimeId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerFrame {
    pub frame_id: String,
    pub instruction: u32,
    pub display: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerFaultSummary {
    pub code: String,
    pub display: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerProjection {
    pub revision: Revision,
    pub session_id: String,
    pub state: DebuggerState,
    pub program_counter: u32,
    pub fuel_remaining: u64,
    pub deadline_remaining_ms: Option<u64>,
    pub pending_effect: Option<DebuggerPendingEffect>,
    pub frames: Vec<DebuggerFrame>,
    pub fault: Option<DebuggerFaultSummary>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct LocalizedText {
    pub key: String,
    pub fallback: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Accessibility {
    pub label: Option<LocalizedText>,
    pub description: Option<LocalizedText>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiAction {
    RuntimeRefresh { runtime_id: RuntimeId },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiEvent {
    pub node_id: NodeId,
    pub kind: UiEventKind,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum UiEventKind {
    Activate,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UiPatch {
    pub schema_version: u32,
    pub from_revision: Revision,
    pub to_revision: Revision,
    pub operations: Vec<UiPatchOperation>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum UiPatchOperation {
    Remove {
        node_id: NodeId,
    },
    Insert {
        parent_id: NodeId,
        index: usize,
        node: UiNode,
    },
    Move {
        node_id: NodeId,
        parent_id: NodeId,
        index: usize,
    },
    Update {
        node: UiNode,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum UiError {
    UnsupportedSchema {
        actual: u32,
        expected: u32,
    },
    InvalidNodeId,
    DuplicateNodeId {
        node_id: String,
    },
    InvalidText,
    MissingActionLabel {
        node_id: String,
    },
    InvalidRuntimeBinding {
        node_id: String,
    },
    NodeLimitExceeded,
    DepthLimitExceeded,
    PatchLimitExceeded,
    RevisionRegression,
    PatchRevisionMismatch {
        document: Revision,
        patch_from: Revision,
    },
    InvalidPatch {
        reason: &'static str,
    },
    PayloadTooLarge {
        size: usize,
        limit: usize,
    },
    InvalidJson(String),
    EventRevisionMismatch {
        document: Revision,
        expected: Option<Revision>,
    },
    StateRevisionMismatch {
        primary: Revision,
        related: Revision,
    },
    InvalidState,
    LogLimitExceeded,
    DebuggerLimitExceeded,
    UnknownEventTarget {
        node_id: String,
    },
    EventTargetHasNoAction {
        node_id: String,
    },
    Lowering(LoweringError),
}

impl NodeId {
    pub fn new(value: impl Into<String>) -> Result<Self, UiError> {
        let value = value.into();
        if value.is_empty()
            || value.len() > 128
            || !value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':')
            })
        {
            return Err(UiError::InvalidNodeId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub fn fleet_document(result: &QueryResult) -> Result<UiDocument, UiError> {
    let QueryResult::RuntimeList { revision, runtimes } = result else {
        return Err(UiError::InvalidText);
    };
    let mut children = vec![
        text_node(
            "fleet-title",
            UiNodeKind::Heading,
            "fleet.title",
            "Runtimes",
        )?,
        text_node(
            "fleet-revision",
            UiNodeKind::Text,
            "fleet.revision",
            &format!("Revision {}", revision.0),
        )?,
    ];
    for runtime in runtimes {
        let prefix = format!("runtime-{}", runtime.id.as_str());
        let status = match runtime.refresh_status {
            RefreshStatus::NeverRequested => "Never requested",
            RefreshStatus::Pending => "Refresh pending",
            RefreshStatus::Ready => "Ready",
            RefreshStatus::Failed => "Refresh failed",
        };
        children.push(UiNode {
            id: NodeId::new(prefix.clone())?,
            kind: UiNodeKind::RuntimeCard,
            runtime_id: Some(runtime.id.clone()),
            text: None,
            accessibility: Accessibility {
                label: Some(localized("fleet.runtime.card", &runtime.name)?),
                description: None,
            },
            action: None,
            children: vec![
                text_node(
                    &format!("{prefix}-name"),
                    UiNodeKind::Heading,
                    "fleet.runtime.name",
                    &runtime.name,
                )?,
                text_node(
                    &format!("{prefix}-status"),
                    UiNodeKind::Text,
                    "fleet.runtime.status",
                    status,
                )?,
                UiNode {
                    id: NodeId::new(format!("{prefix}-refresh"))?,
                    kind: UiNodeKind::Action,
                    runtime_id: None,
                    text: Some(localized("fleet.runtime.refresh", "Refresh")?),
                    accessibility: Accessibility {
                        label: Some(localized("fleet.runtime.refresh", "Refresh runtime")?),
                        description: None,
                    },
                    action: Some(UiAction::RuntimeRefresh {
                        runtime_id: runtime.id.clone(),
                    }),
                    children: Vec::new(),
                },
            ],
        });
    }
    let document = UiDocument {
        schema_version: UI_SCHEMA_VERSION,
        revision: *revision,
        root: UiNode {
            id: NodeId::new("fleet-root")?,
            kind: UiNodeKind::Column,
            runtime_id: None,
            text: None,
            accessibility: Accessibility {
                label: Some(localized("fleet.root", "Runtime fleet")?),
                description: None,
            },
            action: None,
            children,
        },
    };
    validate_document(&document)?;
    Ok(document)
}

pub fn runtime_workspace_document(
    inspect: &QueryResult,
    history: &QueryResult,
) -> Result<UiDocument, UiError> {
    let QueryResult::RuntimeInspect { revision, runtime } = inspect else {
        return Err(UiError::InvalidState);
    };
    let QueryResult::RuntimeHistory {
        revision: history_revision,
        entries,
    } = history
    else {
        return Err(UiError::InvalidState);
    };
    if revision != history_revision {
        return Err(UiError::StateRevisionMismatch {
            primary: *revision,
            related: *history_revision,
        });
    }
    if entries.iter().any(|entry| entry.runtime.id != runtime.id) {
        return Err(UiError::InvalidState);
    }

    let prefix = format!("workspace-{}", runtime.id.as_str());
    let refresh_status = match runtime.refresh_status {
        RefreshStatus::NeverRequested => "Never requested",
        RefreshStatus::Pending => "Refresh pending",
        RefreshStatus::Ready => "Ready",
        RefreshStatus::Failed => "Refresh failed",
    };
    let mut history_children = Vec::with_capacity(entries.len().max(1));
    for entry in entries {
        history_children.push(UiNode {
            id: NodeId::new(format!(
                "{prefix}-history-{}-{}",
                entry.runtime.revision.0,
                entry.command_id.as_str()
            ))?,
            kind: UiNodeKind::HistoryEntry,
            runtime_id: None,
            text: Some(localized(
                "runtime.history.entry",
                &format!(
                    "Revision {}: {}",
                    entry.runtime.revision.0,
                    match entry.status {
                        leserpent_domain::CommandStatus::Planned => "planned",
                        leserpent_domain::CommandStatus::Applied => "applied",
                    }
                ),
            )?),
            accessibility: Accessibility::default(),
            action: None,
            children: Vec::new(),
        });
    }
    if history_children.is_empty() {
        history_children.push(text_node(
            &format!("{prefix}-history-empty"),
            UiNodeKind::Text,
            "runtime.history.empty",
            "No applied commands",
        )?);
    }

    let document = UiDocument {
        schema_version: UI_SCHEMA_VERSION,
        revision: *revision,
        root: UiNode {
            id: NodeId::new(prefix.clone())?,
            kind: UiNodeKind::RuntimeWorkspace,
            runtime_id: Some(runtime.id.clone()),
            text: None,
            accessibility: Accessibility {
                label: Some(localized("runtime.workspace", &runtime.name)?),
                description: None,
            },
            action: None,
            children: vec![
                text_node(
                    &format!("{prefix}-title"),
                    UiNodeKind::Heading,
                    "runtime.workspace.title",
                    &runtime.name,
                )?,
                text_node(
                    &format!("{prefix}-revision"),
                    UiNodeKind::Text,
                    "runtime.workspace.revision",
                    &format!("Revision {}", revision.0),
                )?,
                text_node(
                    &format!("{prefix}-status"),
                    UiNodeKind::Text,
                    "runtime.workspace.status",
                    refresh_status,
                )?,
                text_node(
                    &format!("{prefix}-snapshot"),
                    UiNodeKind::Text,
                    "runtime.workspace.snapshot",
                    if runtime.status.has_latest_snapshot {
                        "Latest snapshot available"
                    } else {
                        "No runtime snapshot"
                    },
                )?,
                UiNode {
                    id: NodeId::new(format!("{prefix}-refresh"))?,
                    kind: UiNodeKind::Action,
                    runtime_id: None,
                    text: Some(localized("runtime.workspace.refresh", "Refresh")?),
                    accessibility: Accessibility {
                        label: Some(localized("runtime.workspace.refresh", "Refresh runtime")?),
                        description: None,
                    },
                    action: Some(UiAction::RuntimeRefresh {
                        runtime_id: runtime.id.clone(),
                    }),
                    children: Vec::new(),
                },
                UiNode {
                    id: NodeId::new(format!("{prefix}-history"))?,
                    kind: UiNodeKind::Section,
                    runtime_id: None,
                    text: Some(localized("runtime.history.title", "History")?),
                    accessibility: Accessibility {
                        label: Some(localized("runtime.history.title", "Runtime history")?),
                        description: None,
                    },
                    action: None,
                    children: history_children,
                },
            ],
        },
    };
    validate_document(&document)?;
    Ok(document)
}

pub fn runtime_log_document(projection: &RuntimeLogProjection) -> Result<UiDocument, UiError> {
    if projection.entries.len() > MAX_RUNTIME_LOG_ENTRIES {
        return Err(UiError::LogLimitExceeded);
    }
    let mut previous_sequence = None;
    let prefix = format!("logs-{}", projection.runtime_id.as_str());
    let mut entries = Vec::with_capacity(projection.entries.len().max(1));
    for entry in &projection.entries {
        if previous_sequence.is_some_and(|previous| entry.sequence <= previous)
            || entry.display.len() > MAX_RUNTIME_LOG_DISPLAY_BYTES
            || entry.display.chars().any(char::is_control)
        {
            return Err(UiError::InvalidState);
        }
        previous_sequence = Some(entry.sequence);
        let level = match entry.level {
            RuntimeLogLevel::Trace => "TRACE",
            RuntimeLogLevel::Debug => "DEBUG",
            RuntimeLogLevel::Info => "INFO",
            RuntimeLogLevel::Warning => "WARN",
            RuntimeLogLevel::Error => "ERROR",
        };
        entries.push(UiNode {
            id: NodeId::new(format!("{prefix}-entry-{}", entry.sequence))?,
            kind: UiNodeKind::LogEntry,
            runtime_id: None,
            text: Some(localized(
                "runtime.logs.entry",
                &format!("[{level}] {}", entry.display),
            )?),
            accessibility: Accessibility::default(),
            action: None,
            children: Vec::new(),
        });
    }
    if entries.is_empty() {
        entries.push(text_node(
            &format!("{prefix}-empty"),
            UiNodeKind::Text,
            "runtime.logs.empty",
            "No log entries",
        )?);
    }

    let document = UiDocument {
        schema_version: UI_SCHEMA_VERSION,
        revision: projection.revision,
        root: UiNode {
            id: NodeId::new(prefix.clone())?,
            kind: UiNodeKind::RuntimeWorkspace,
            runtime_id: Some(projection.runtime_id.clone()),
            text: None,
            accessibility: Accessibility {
                label: Some(localized(
                    "runtime.logs.workspace",
                    &projection.runtime_name,
                )?),
                description: None,
            },
            action: None,
            children: vec![
                text_node(
                    &format!("{prefix}-title"),
                    UiNodeKind::Heading,
                    "runtime.logs.title",
                    &format!("{} logs", projection.runtime_name),
                )?,
                text_node(
                    &format!("{prefix}-revision"),
                    UiNodeKind::Text,
                    "runtime.logs.revision",
                    &format!("Revision {}", projection.revision.0),
                )?,
                UiNode {
                    id: NodeId::new(format!("{prefix}-entries"))?,
                    kind: UiNodeKind::Section,
                    runtime_id: None,
                    text: Some(localized("runtime.logs.entries", "Log entries")?),
                    accessibility: Accessibility {
                        label: Some(localized("runtime.logs.entries", "Runtime log entries")?),
                        description: None,
                    },
                    action: None,
                    children: entries,
                },
            ],
        },
    };
    validate_document(&document)?;
    Ok(document)
}

pub fn debugger_document(projection: &DebuggerProjection) -> Result<UiDocument, UiError> {
    if projection.frames.len() > MAX_DEBUGGER_FRAMES
        || projection
            .deadline_remaining_ms
            .is_some_and(|deadline| deadline > MAX_DEBUGGER_DEADLINE_REMAINING_MS)
    {
        return Err(UiError::DebuggerLimitExceeded);
    }
    NodeId::new(&projection.session_id)?;
    if (projection.state == DebuggerState::WaitingEffect) != projection.pending_effect.is_some()
        || (projection.state == DebuggerState::Failed) != projection.fault.is_some()
    {
        return Err(UiError::InvalidState);
    }
    if let Some(effect) = &projection.pending_effect {
        NodeId::new(&effect.effect_id)?;
        let binding_valid = match effect.kind {
            DebuggerEffectKind::RuntimeList => effect.runtime_id.is_none(),
            DebuggerEffectKind::RuntimeInspect
            | DebuggerEffectKind::RuntimeHistory
            | DebuggerEffectKind::RuntimeRefresh => effect.runtime_id.is_some(),
        };
        if !binding_valid {
            return Err(UiError::InvalidState);
        }
    }
    if let Some(fault) = &projection.fault {
        NodeId::new(&fault.code)?;
        validate_debugger_display(&fault.display)?;
    }

    let prefix = format!("debug-{}", projection.session_id);
    let mut frame_ids = BTreeSet::new();
    let mut frames = Vec::with_capacity(projection.frames.len().max(1));
    for frame in &projection.frames {
        let frame_id = NodeId::new(&frame.frame_id)?;
        if !frame_ids.insert(frame_id) {
            return Err(UiError::InvalidState);
        }
        validate_debugger_display(&frame.display)?;
        frames.push(UiNode {
            id: NodeId::new(format!("{prefix}-frame-{}", frame.frame_id))?,
            kind: UiNodeKind::DebuggerFrame,
            runtime_id: None,
            text: Some(localized(
                "debugger.frame",
                &format!("[pc {}] {}", frame.instruction, frame.display),
            )?),
            accessibility: Accessibility::default(),
            action: None,
            children: Vec::new(),
        });
    }
    if frames.is_empty() {
        frames.push(text_node(
            &format!("{prefix}-frames-empty"),
            UiNodeKind::Text,
            "debugger.frames.empty",
            "No logical frames",
        )?);
    }

    let state = match projection.state {
        DebuggerState::Running => "Running",
        DebuggerState::WaitingEffect => "Waiting for effect",
        DebuggerState::Yielded => "Yielded",
        DebuggerState::Completed => "Completed",
        DebuggerState::Failed => "Failed",
        DebuggerState::Cancelled => "Cancelled",
    };
    let mut children = vec![
        text_node(
            &format!("{prefix}-title"),
            UiNodeKind::Heading,
            "debugger.title",
            "Leselang debugger",
        )?,
        text_node(
            &format!("{prefix}-state"),
            UiNodeKind::Text,
            "debugger.state",
            state,
        )?,
        text_node(
            &format!("{prefix}-program-counter"),
            UiNodeKind::Text,
            "debugger.program_counter",
            &format!("Program counter {}", projection.program_counter),
        )?,
        text_node(
            &format!("{prefix}-budget"),
            UiNodeKind::Text,
            "debugger.budget",
            &format!(
                "Fuel {} / deadline {}",
                projection.fuel_remaining,
                projection
                    .deadline_remaining_ms
                    .map_or_else(|| "none".to_string(), |value| format!("{value} ms"))
            ),
        )?,
    ];
    if let Some(effect) = &projection.pending_effect {
        let kind = match effect.kind {
            DebuggerEffectKind::RuntimeList => "runtime list",
            DebuggerEffectKind::RuntimeInspect => "runtime inspect",
            DebuggerEffectKind::RuntimeHistory => "runtime history",
            DebuggerEffectKind::RuntimeRefresh => "runtime refresh",
        };
        children.push(text_node(
            &format!("{prefix}-pending-effect"),
            UiNodeKind::Text,
            "debugger.pending_effect",
            &format!("Pending {kind} ({})", effect.effect_id),
        )?);
    }
    children.push(UiNode {
        id: NodeId::new(format!("{prefix}-frames"))?,
        kind: UiNodeKind::Section,
        runtime_id: None,
        text: Some(localized("debugger.frames", "Logical frames")?),
        accessibility: Accessibility {
            label: Some(localized("debugger.frames", "Debugger logical frames")?),
            description: None,
        },
        action: None,
        children: frames,
    });
    if let Some(fault) = &projection.fault {
        children.push(text_node(
            &format!("{prefix}-fault"),
            UiNodeKind::Text,
            "debugger.fault",
            &format!("{}: {}", fault.code, fault.display),
        )?);
    }

    let document = UiDocument {
        schema_version: UI_SCHEMA_VERSION,
        revision: projection.revision,
        root: UiNode {
            id: NodeId::new(prefix)?,
            kind: UiNodeKind::DebuggerWorkspace,
            runtime_id: None,
            text: None,
            accessibility: Accessibility {
                label: Some(localized(
                    "debugger.workspace",
                    "Leselang debugger workspace",
                )?),
                description: None,
            },
            action: None,
            children,
        },
    };
    validate_document(&document)?;
    Ok(document)
}

fn validate_debugger_display(value: &str) -> Result<(), UiError> {
    if value.len() > MAX_DEBUGGER_DISPLAY_BYTES || value.chars().any(char::is_control) {
        return Err(UiError::InvalidState);
    }
    Ok(())
}

pub fn plan_event(
    document: &UiDocument,
    event: &UiEvent,
    context: &LoweringContext,
) -> Result<CommandPlan, UiError> {
    validate_document(document)?;
    if context.expected_revision != Some(document.revision) {
        return Err(UiError::EventRevisionMismatch {
            document: document.revision,
            expected: context.expected_revision,
        });
    }
    let node =
        find_node(&document.root, &event.node_id).ok_or_else(|| UiError::UnknownEventTarget {
            node_id: event.node_id.as_str().to_string(),
        })?;
    match (&event.kind, &node.action) {
        (UiEventKind::Activate, Some(UiAction::RuntimeRefresh { runtime_id })) => {
            plan_runtime_refresh(runtime_id, context).map_err(UiError::Lowering)
        }
        _ => Err(UiError::EventTargetHasNoAction {
            node_id: event.node_id.as_str().to_string(),
        }),
    }
}

pub fn diff(previous: &UiDocument, next: &UiDocument) -> Result<UiPatch, UiError> {
    validate_document(previous)?;
    validate_document(next)?;
    if next.revision < previous.revision {
        return Err(UiError::RevisionRegression);
    }
    if previous.root.id != next.root.id {
        return Err(UiError::InvalidPatch {
            reason: "root node identity cannot change",
        });
    }
    let old = index_document(previous);
    let new = index_document(next);
    let mut operations = Vec::new();
    let mut working = previous.clone();

    let removed = old
        .iter()
        .filter(|(id, entry)| {
            !new.contains_key(*id)
                && entry
                    .parent
                    .as_ref()
                    .is_some_and(|parent| new.contains_key(parent))
        })
        .map(|(id, _)| (*id).clone())
        .collect::<Vec<_>>();
    for node_id in removed {
        remove_node(&mut working.root, &node_id).ok_or(UiError::InvalidPatch {
            reason: "diff removal target disappeared",
        })?;
        operations.push(UiPatchOperation::Remove { node_id });
    }

    reconcile_children(&mut working.root, &next.root, &mut operations)?;
    for (node_id, target) in &new {
        let current = find_node(&working.root, node_id).ok_or(UiError::InvalidPatch {
            reason: "diff update target disappeared",
        })?;
        if shallow_node(current) != shallow_node(target.node) {
            let mut node = target.node.clone();
            node.children.clear();
            let current =
                find_node_mut(&mut working.root, node_id).ok_or(UiError::InvalidPatch {
                    reason: "diff update target disappeared",
                })?;
            let children = std::mem::take(&mut current.children);
            *current = node.clone();
            current.children = children;
            operations.push(UiPatchOperation::Update { node });
        }
    }
    working.revision = next.revision;
    if working != *next {
        return Err(UiError::InvalidPatch {
            reason: "diff failed to converge on the target document",
        });
    }
    if operations.len() > MAX_UI_PATCH_OPERATIONS {
        return Err(UiError::PatchLimitExceeded);
    }
    Ok(UiPatch {
        schema_version: UI_SCHEMA_VERSION,
        from_revision: previous.revision,
        to_revision: next.revision,
        operations,
    })
}

fn reconcile_children(
    working_root: &mut UiNode,
    target_parent: &UiNode,
    operations: &mut Vec<UiPatchOperation>,
) -> Result<(), UiError> {
    for (target_index, target_child) in target_parent.children.iter().enumerate() {
        let inserted = match find_location(working_root, &target_child.id) {
            Some((current_parent, current_index)) => {
                if current_parent != target_parent.id || current_index != target_index {
                    let moving = remove_node(working_root, &target_child.id).ok_or(
                        UiError::InvalidPatch {
                            reason: "diff move target disappeared",
                        },
                    )?;
                    let parent = find_node_mut(working_root, &target_parent.id).ok_or(
                        UiError::InvalidPatch {
                            reason: "diff move parent disappeared",
                        },
                    )?;
                    if target_index > parent.children.len() {
                        return Err(UiError::InvalidPatch {
                            reason: "diff produced an invalid move index",
                        });
                    }
                    parent.children.insert(target_index, moving);
                    operations.push(UiPatchOperation::Move {
                        node_id: target_child.id.clone(),
                        parent_id: target_parent.id.clone(),
                        index: target_index,
                    });
                }
                false
            }
            None => {
                let parent = find_node_mut(working_root, &target_parent.id).ok_or(
                    UiError::InvalidPatch {
                        reason: "diff insert parent disappeared",
                    },
                )?;
                if target_index > parent.children.len() {
                    return Err(UiError::InvalidPatch {
                        reason: "diff produced an invalid insert index",
                    });
                }
                parent.children.insert(target_index, target_child.clone());
                operations.push(UiPatchOperation::Insert {
                    parent_id: target_parent.id.clone(),
                    index: target_index,
                    node: target_child.clone(),
                });
                true
            }
        };
        if !inserted {
            reconcile_children(working_root, target_child, operations)?;
        }
    }
    Ok(())
}

fn find_location(node: &UiNode, target: &NodeId) -> Option<(NodeId, usize)> {
    for (index, child) in node.children.iter().enumerate() {
        if &child.id == target {
            return Some((node.id.clone(), index));
        }
        if let Some(location) = find_location(child, target) {
            return Some(location);
        }
    }
    None
}

pub fn apply_patch(document: &UiDocument, patch: &UiPatch) -> Result<UiDocument, UiError> {
    validate_document(document)?;
    if patch.schema_version != UI_SCHEMA_VERSION {
        return Err(UiError::UnsupportedSchema {
            actual: patch.schema_version,
            expected: UI_SCHEMA_VERSION,
        });
    }
    if patch.from_revision != document.revision {
        return Err(UiError::PatchRevisionMismatch {
            document: document.revision,
            patch_from: patch.from_revision,
        });
    }
    if patch.to_revision < patch.from_revision {
        return Err(UiError::RevisionRegression);
    }
    if patch.operations.len() > MAX_UI_PATCH_OPERATIONS {
        return Err(UiError::PatchLimitExceeded);
    }

    let mut result = document.clone();
    for operation in &patch.operations {
        match operation {
            UiPatchOperation::Remove { node_id } => {
                if node_id == &result.root.id {
                    return Err(UiError::InvalidPatch {
                        reason: "root node cannot be removed",
                    });
                }
                remove_node(&mut result.root, node_id).ok_or(UiError::InvalidPatch {
                    reason: "remove target does not exist",
                })?;
            }
            UiPatchOperation::Insert {
                parent_id,
                index,
                node,
            } => {
                let mut inserted_ids = BTreeSet::new();
                collect_ids(node, &mut inserted_ids)?;
                if inserted_ids
                    .iter()
                    .any(|node_id| find_node(&result.root, node_id).is_some())
                {
                    return Err(UiError::InvalidPatch {
                        reason: "inserted subtree conflicts with an existing node",
                    });
                }
                let parent =
                    find_node_mut(&mut result.root, parent_id).ok_or(UiError::InvalidPatch {
                        reason: "insert parent does not exist",
                    })?;
                if *index > parent.children.len() {
                    return Err(UiError::InvalidPatch {
                        reason: "insert index is out of bounds",
                    });
                }
                parent.children.insert(*index, node.clone());
            }
            UiPatchOperation::Move {
                node_id,
                parent_id,
                index,
            } => {
                if node_id == &result.root.id {
                    return Err(UiError::InvalidPatch {
                        reason: "root node cannot be moved",
                    });
                }
                let moving = find_node(&result.root, node_id).ok_or(UiError::InvalidPatch {
                    reason: "move target does not exist",
                })?;
                if find_node(moving, parent_id).is_some() {
                    return Err(UiError::InvalidPatch {
                        reason: "node cannot be moved into its own subtree",
                    });
                }
                let moving =
                    remove_node(&mut result.root, node_id).ok_or(UiError::InvalidPatch {
                        reason: "move target disappeared",
                    })?;
                let parent =
                    find_node_mut(&mut result.root, parent_id).ok_or(UiError::InvalidPatch {
                        reason: "move parent does not exist",
                    })?;
                if *index > parent.children.len() {
                    return Err(UiError::InvalidPatch {
                        reason: "move index is out of bounds",
                    });
                }
                parent.children.insert(*index, moving);
            }
            UiPatchOperation::Update { node } => {
                if !node.children.is_empty() {
                    return Err(UiError::InvalidPatch {
                        reason: "update nodes must be shallow",
                    });
                }
                let target =
                    find_node_mut(&mut result.root, &node.id).ok_or(UiError::InvalidPatch {
                        reason: "update target does not exist",
                    })?;
                let children = std::mem::take(&mut target.children);
                *target = node.clone();
                target.children = children;
            }
        }
    }
    result.revision = patch.to_revision;
    validate_document(&result)?;
    Ok(result)
}

pub fn encode_document(document: &UiDocument) -> Result<Vec<u8>, UiError> {
    validate_document(document)?;
    encode_json(document)
}

pub fn decode_document(bytes: &[u8]) -> Result<UiDocument, UiError> {
    check_payload_size(bytes.len())?;
    let document: UiDocument =
        serde_json::from_slice(bytes).map_err(|error| UiError::InvalidJson(error.to_string()))?;
    validate_document(&document)?;
    Ok(document)
}

pub fn encode_patch(patch: &UiPatch) -> Result<Vec<u8>, UiError> {
    validate_patch(patch)?;
    encode_json(patch)
}

pub fn decode_patch(bytes: &[u8]) -> Result<UiPatch, UiError> {
    check_payload_size(bytes.len())?;
    let patch: UiPatch =
        serde_json::from_slice(bytes).map_err(|error| UiError::InvalidJson(error.to_string()))?;
    validate_patch(&patch)?;
    Ok(patch)
}

fn validate_patch(patch: &UiPatch) -> Result<(), UiError> {
    if patch.schema_version != UI_SCHEMA_VERSION {
        return Err(UiError::UnsupportedSchema {
            actual: patch.schema_version,
            expected: UI_SCHEMA_VERSION,
        });
    }
    if patch.to_revision < patch.from_revision {
        return Err(UiError::RevisionRegression);
    }
    if patch.operations.len() > MAX_UI_PATCH_OPERATIONS {
        return Err(UiError::PatchLimitExceeded);
    }
    Ok(())
}

fn encode_json(value: &impl Serialize) -> Result<Vec<u8>, UiError> {
    let bytes =
        serde_json::to_vec(value).map_err(|error| UiError::InvalidJson(error.to_string()))?;
    check_payload_size(bytes.len())?;
    Ok(bytes)
}

fn check_payload_size(size: usize) -> Result<(), UiError> {
    if size > MAX_UI_IR_BYTES {
        return Err(UiError::PayloadTooLarge {
            size,
            limit: MAX_UI_IR_BYTES,
        });
    }
    Ok(())
}

pub fn validate_document(document: &UiDocument) -> Result<(), UiError> {
    if document.schema_version != UI_SCHEMA_VERSION {
        return Err(UiError::UnsupportedSchema {
            actual: document.schema_version,
            expected: UI_SCHEMA_VERSION,
        });
    }
    let mut ids = BTreeSet::new();
    validate_node(&document.root, 1, None, &mut ids)?;
    if ids.len() > MAX_UI_NODES {
        return Err(UiError::NodeLimitExceeded);
    }
    Ok(())
}

fn validate_node(
    node: &UiNode,
    depth: usize,
    runtime_context: Option<&RuntimeId>,
    ids: &mut BTreeSet<NodeId>,
) -> Result<(), UiError> {
    if depth > MAX_UI_DEPTH {
        return Err(UiError::DepthLimitExceeded);
    }
    NodeId::new(node.id.as_str())?;
    if !ids.insert(node.id.clone()) {
        return Err(UiError::DuplicateNodeId {
            node_id: node.id.as_str().to_string(),
        });
    }
    validate_optional_text(node.text.as_ref())?;
    validate_optional_text(node.accessibility.label.as_ref())?;
    validate_optional_text(node.accessibility.description.as_ref())?;
    if node.action.is_some() && node.accessibility.label.is_none() {
        return Err(UiError::MissingActionLabel {
            node_id: node.id.as_str().to_string(),
        });
    }
    let runtime_context = match (&node.kind, &node.runtime_id) {
        (UiNodeKind::RuntimeCard | UiNodeKind::RuntimeWorkspace, Some(runtime_id)) => {
            Some(runtime_id)
        }
        (UiNodeKind::RuntimeCard | UiNodeKind::RuntimeWorkspace, None) | (_, Some(_)) => {
            return Err(UiError::InvalidRuntimeBinding {
                node_id: node.id.as_str().to_string(),
            });
        }
        _ => runtime_context,
    };
    if let Some(UiAction::RuntimeRefresh { runtime_id }) = &node.action
        && runtime_context != Some(runtime_id)
    {
        return Err(UiError::InvalidRuntimeBinding {
            node_id: node.id.as_str().to_string(),
        });
    }
    if ids.len() > MAX_UI_NODES {
        return Err(UiError::NodeLimitExceeded);
    }
    for child in &node.children {
        validate_node(child, depth + 1, runtime_context, ids)?;
    }
    Ok(())
}

fn validate_optional_text(text: Option<&LocalizedText>) -> Result<(), UiError> {
    let Some(text) = text else {
        return Ok(());
    };
    if text.key.is_empty()
        || text.key.len() > 128
        || !text
            .key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
        || text.fallback.len() > MAX_UI_TEXT_BYTES
        || text.fallback.chars().any(char::is_control)
    {
        return Err(UiError::InvalidText);
    }
    Ok(())
}

fn localized(key: &str, fallback: &str) -> Result<LocalizedText, UiError> {
    let text = LocalizedText {
        key: key.to_string(),
        fallback: fallback.to_string(),
    };
    validate_optional_text(Some(&text))?;
    Ok(text)
}

fn text_node(id: &str, kind: UiNodeKind, key: &str, fallback: &str) -> Result<UiNode, UiError> {
    Ok(UiNode {
        id: NodeId::new(id)?,
        kind,
        runtime_id: None,
        text: Some(localized(key, fallback)?),
        accessibility: Accessibility::default(),
        action: None,
        children: Vec::new(),
    })
}

fn find_node<'a>(node: &'a UiNode, id: &NodeId) -> Option<&'a UiNode> {
    if &node.id == id {
        return Some(node);
    }
    node.children.iter().find_map(|child| find_node(child, id))
}

fn find_node_mut<'a>(node: &'a mut UiNode, id: &NodeId) -> Option<&'a mut UiNode> {
    if &node.id == id {
        return Some(node);
    }
    node.children
        .iter_mut()
        .find_map(|child| find_node_mut(child, id))
}

fn remove_node(node: &mut UiNode, id: &NodeId) -> Option<UiNode> {
    if let Some(index) = node.children.iter().position(|child| &child.id == id) {
        return Some(node.children.remove(index));
    }
    node.children
        .iter_mut()
        .find_map(|child| remove_node(child, id))
}

fn collect_ids(node: &UiNode, ids: &mut BTreeSet<NodeId>) -> Result<(), UiError> {
    if !ids.insert(node.id.clone()) {
        return Err(UiError::InvalidPatch {
            reason: "inserted subtree contains duplicate node identifiers",
        });
    }
    for child in &node.children {
        collect_ids(child, ids)?;
    }
    Ok(())
}

#[derive(Clone)]
struct IndexedNode<'a> {
    node: &'a UiNode,
    parent: Option<NodeId>,
}

fn index_document(document: &UiDocument) -> BTreeMap<NodeId, IndexedNode<'_>> {
    fn visit<'a>(
        node: &'a UiNode,
        parent: Option<NodeId>,
        output: &mut BTreeMap<NodeId, IndexedNode<'a>>,
    ) {
        output.insert(
            node.id.clone(),
            IndexedNode {
                node,
                parent: parent.clone(),
            },
        );
        for child in &node.children {
            visit(child, Some(node.id.clone()), output);
        }
    }
    let mut output = BTreeMap::new();
    visit(&document.root, None, &mut output);
    output
}

fn shallow_node(
    node: &UiNode,
) -> (
    &NodeId,
    UiNodeKind,
    &Option<RuntimeId>,
    &Option<LocalizedText>,
    &Accessibility,
    &Option<UiAction>,
) {
    (
        &node.id,
        node.kind,
        &node.runtime_id,
        &node.text,
        &node.accessibility,
        &node.action,
    )
}

#[cfg(test)]
mod tests {
    use leserpent_domain::{
        CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, CapabilitySet, Command,
        CommandEnvelope, CommandId, CommandOrigin, Confirmation, DOMAIN_SCHEMA_VERSION,
        IdempotencyKey, Principal, Query, QueryEnvelope, RuntimeListFilter,
    };

    use super::*;

    fn context() -> LoweringContext {
        LoweringContext {
            principal: Principal {
                id: "operator".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
            expected_revision: Some(Revision(1)),
            command_id: CommandId::new("command-a").unwrap(),
            idempotency_key: IdempotencyKey::new("effect-a").unwrap(),
            origin: CommandOrigin::Gui,
            confirmation: Confirmation::Confirmed,
            dry_run: true,
        }
    }

    fn fleet(revision: u64, names: &[(&str, &str)]) -> QueryResult {
        let mut control = leserpent_domain::InMemoryControlPlane::default();
        for (id, name) in names {
            control.register_runtime(RuntimeId::new(*id).unwrap(), *name, "hidden-endpoint");
        }
        let QueryResult::RuntimeList { runtimes, .. } = control
            .query(leserpent_domain::QueryEnvelope {
                schema_version: leserpent_domain::DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".into(),
                },
                capabilities: CapabilitySet::new([leserpent_domain::CAPABILITY_RUNTIME_READ]),
                query: leserpent_domain::Query::RuntimeList {
                    filter: RuntimeListFilter::default(),
                },
            })
            .unwrap()
        else {
            unreachable!()
        };
        QueryResult::RuntimeList {
            revision: Revision(revision),
            runtimes,
        }
    }

    fn workspace(applied: bool) -> (QueryResult, QueryResult) {
        let mut control = leserpent_domain::InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(runtime_id.clone(), "Runtime A", "hidden-workspace-endpoint");
        if applied {
            control
                .execute(CommandEnvelope {
                    schema_version: DOMAIN_SCHEMA_VERSION,
                    command_id: CommandId::new("command-refresh").unwrap(),
                    idempotency_key: IdempotencyKey::new("effect-refresh").unwrap(),
                    expected_revision: Some(Revision(1)),
                    principal: Principal {
                        id: "operator".into(),
                    },
                    capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
                    origin: CommandOrigin::Gui,
                    confirmation: Confirmation::Confirmed,
                    dry_run: false,
                    command: Command::RuntimeRefresh {
                        runtime_id: runtime_id.clone(),
                    },
                })
                .unwrap();
        }
        let query = |query| {
            control
                .query(QueryEnvelope {
                    schema_version: DOMAIN_SCHEMA_VERSION,
                    principal: Principal {
                        id: "operator".into(),
                    },
                    capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                    query,
                })
                .unwrap()
        };
        (
            query(Query::RuntimeInspect {
                runtime_id: runtime_id.clone(),
            }),
            query(Query::RuntimeHistory { runtime_id }),
        )
    }

    fn workspace_with_history_entries(count: u64) -> UiDocument {
        let mut control = leserpent_domain::InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-history").unwrap();
        control.register_runtime(
            runtime_id.clone(),
            "History Runtime",
            "hidden-history-endpoint",
        );
        for index in 0..count {
            control
                .execute(CommandEnvelope {
                    schema_version: DOMAIN_SCHEMA_VERSION,
                    command_id: CommandId::new(format!("history-command-{index}")).unwrap(),
                    idempotency_key: IdempotencyKey::new(format!("history-effect-{index}"))
                        .unwrap(),
                    expected_revision: Some(Revision(index + 1)),
                    principal: Principal {
                        id: "operator".into(),
                    },
                    capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
                    origin: CommandOrigin::Gui,
                    confirmation: Confirmation::Confirmed,
                    dry_run: false,
                    command: Command::RuntimeRefresh {
                        runtime_id: runtime_id.clone(),
                    },
                })
                .unwrap();
        }
        let query = |query| {
            control
                .query(QueryEnvelope {
                    schema_version: DOMAIN_SCHEMA_VERSION,
                    principal: Principal {
                        id: "operator".into(),
                    },
                    capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                    query,
                })
                .unwrap()
        };
        runtime_workspace_document(
            &query(Query::RuntimeInspect {
                runtime_id: runtime_id.clone(),
            }),
            &query(Query::RuntimeHistory { runtime_id }),
        )
        .unwrap()
    }

    fn logs(revision: u64, start: u64, count: u64) -> RuntimeLogProjection {
        RuntimeLogProjection {
            revision: Revision(revision),
            runtime_id: RuntimeId::new("runtime-logs").unwrap(),
            runtime_name: "Log Runtime".into(),
            entries: (start..start + count)
                .map(|sequence| RuntimeLogEntry {
                    sequence,
                    level: match sequence % 5 {
                        0 => RuntimeLogLevel::Trace,
                        1 => RuntimeLogLevel::Debug,
                        2 => RuntimeLogLevel::Info,
                        3 => RuntimeLogLevel::Warning,
                        _ => RuntimeLogLevel::Error,
                    },
                    display: format!("sanitized event {sequence}"),
                })
                .collect(),
        }
    }

    fn debugger(
        revision: u64,
        state: DebuggerState,
        frame_start: u32,
        frame_count: u32,
    ) -> DebuggerProjection {
        DebuggerProjection {
            revision: Revision(revision),
            session_id: "session-a".into(),
            state,
            program_counter: if state == DebuggerState::WaitingEffect {
                7
            } else {
                8
            },
            fuel_remaining: 900 - revision,
            deadline_remaining_ms: Some(5_000),
            pending_effect: (state == DebuggerState::WaitingEffect).then(|| {
                DebuggerPendingEffect {
                    effect_id: "effect-7".into(),
                    kind: DebuggerEffectKind::RuntimeInspect,
                    runtime_id: Some(RuntimeId::new("runtime-a").unwrap()),
                }
            }),
            frames: (frame_start..frame_start + frame_count)
                .map(|instruction| DebuggerFrame {
                    frame_id: format!("frame-{instruction}"),
                    instruction,
                    display: format!("logical frame {instruction}"),
                })
                .collect(),
            fault: None,
        }
    }

    #[test]
    fn fleet_ir_has_stable_ids_and_omits_endpoints() {
        let document = fleet_document(&fleet(7, &[("runtime-a", "Runtime A")])).unwrap();
        assert_eq!(document.revision, Revision(7));
        assert!(find_node(&document.root, &NodeId::new("runtime-runtime-a").unwrap()).is_some());
        let json = serde_json::to_string(&document).unwrap();
        assert!(!json.contains("hidden-endpoint"));
        validate_document(&document).unwrap();
    }

    #[test]
    fn declared_action_lowers_to_shared_command_plan() {
        let document = fleet_document(&fleet(1, &[("runtime-a", "Runtime A")])).unwrap();
        let plan = plan_event(
            &document,
            &UiEvent {
                node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
                kind: UiEventKind::Activate,
            },
            &context(),
        )
        .unwrap();
        let leserpent_domain::PlannedOperation::Command(command) = plan.operation else {
            panic!("refresh action must lower to a command");
        };
        assert!(matches!(
            command.command,
            Command::RuntimeRefresh { runtime_id } if runtime_id.as_str() == "runtime-a"
        ));
        assert_eq!(command.origin, CommandOrigin::Gui);
    }

    #[test]
    fn runtime_workspace_combines_consistent_typed_projections() {
        let (inspect, history) = workspace(false);
        let document = runtime_workspace_document(&inspect, &history).unwrap();
        assert_eq!(document.revision, Revision(1));
        assert_eq!(document.root.kind, UiNodeKind::RuntimeWorkspace);
        assert_eq!(
            document.root.runtime_id.as_ref().map(RuntimeId::as_str),
            Some("runtime-a")
        );
        assert!(
            find_node(
                &document.root,
                &NodeId::new("workspace-runtime-a-history-empty").unwrap()
            )
            .is_some()
        );
        let json = serde_json::to_string(&document).unwrap();
        assert!(!json.contains("hidden-workspace-endpoint"));

        let mut workspace_context = context();
        workspace_context.expected_revision = Some(document.revision);
        assert!(
            plan_event(
                &document,
                &UiEvent {
                    node_id: NodeId::new("workspace-runtime-a-refresh").unwrap(),
                    kind: UiEventKind::Activate,
                },
                &workspace_context,
            )
            .is_ok()
        );
    }

    #[test]
    fn runtime_workspace_patch_adds_history_and_rejects_torn_state() {
        let (before_inspect, before_history) = workspace(false);
        let before = runtime_workspace_document(&before_inspect, &before_history).unwrap();
        let (after_inspect, after_history) = workspace(true);
        let after = runtime_workspace_document(&after_inspect, &after_history).unwrap();
        let patch = diff(&before, &after).unwrap();
        assert_eq!(apply_patch(&before, &patch).unwrap(), after);
        assert!(patch.operations.iter().any(|operation| matches!(
            operation,
            UiPatchOperation::Insert { node, .. }
                if node.kind == UiNodeKind::HistoryEntry
        )));

        assert!(matches!(
            runtime_workspace_document(&before_inspect, &after_history),
            Err(UiError::StateRevisionMismatch { .. })
        ));
    }

    #[test]
    fn diff_remains_executable_when_bounded_history_window_slides() {
        let previous = workspace_with_history_entries(32);
        let next = workspace_with_history_entries(33);
        let patch = diff(&previous, &next).unwrap();
        assert_eq!(apply_patch(&previous, &patch).unwrap(), next);
        for expected in ["remove", "insert", "update"] {
            assert!(patch.operations.iter().any(|operation| matches!(
                (expected, operation),
                ("remove", UiPatchOperation::Remove { .. })
                    | ("move", UiPatchOperation::Move { .. })
                    | ("insert", UiPatchOperation::Insert { .. })
                    | ("update", UiPatchOperation::Update { .. })
            )));
        }
    }

    #[test]
    fn runtime_logs_are_bounded_stable_and_incremental() {
        let previous = runtime_log_document(&logs(1, 0, 48)).unwrap();
        let next = runtime_log_document(&logs(2, 1, 48)).unwrap();
        assert_eq!(previous.root.kind, UiNodeKind::RuntimeWorkspace);
        assert!(
            find_node(
                &previous.root,
                &NodeId::new("logs-runtime-logs-entry-0").unwrap()
            )
            .is_some()
        );
        let patch = diff(&previous, &next).unwrap();
        assert_eq!(apply_patch(&previous, &patch).unwrap(), next);
        assert!(
            patch
                .operations
                .iter()
                .any(|operation| matches!(operation, UiPatchOperation::Remove { .. }))
        );
        assert!(
            patch
                .operations
                .iter()
                .any(|operation| matches!(operation, UiPatchOperation::Insert { .. }))
        );
    }

    #[test]
    fn runtime_logs_reject_oversized_or_unsafe_batches() {
        let mut oversized = logs(1, 0, MAX_RUNTIME_LOG_ENTRIES as u64 + 1);
        assert_eq!(
            runtime_log_document(&oversized),
            Err(UiError::LogLimitExceeded)
        );
        oversized.entries.truncate(2);
        oversized.entries[1].sequence = oversized.entries[0].sequence;
        assert_eq!(runtime_log_document(&oversized), Err(UiError::InvalidState));

        let mut unsafe_text = logs(1, 0, 1);
        unsafe_text.entries[0].display = "line\nbreak".into();
        assert_eq!(
            runtime_log_document(&unsafe_text),
            Err(UiError::InvalidState)
        );
        unsafe_text.entries[0].display = "x".repeat(MAX_RUNTIME_LOG_DISPLAY_BYTES + 1);
        assert_eq!(
            runtime_log_document(&unsafe_text),
            Err(UiError::InvalidState)
        );
    }

    #[test]
    fn debugger_waiting_effect_reenters_through_incremental_document() {
        let previous =
            debugger_document(&debugger(1, DebuggerState::WaitingEffect, 0, 40)).unwrap();
        let next = debugger_document(&debugger(2, DebuggerState::Yielded, 1, 40)).unwrap();
        assert_eq!(previous.root.kind, UiNodeKind::DebuggerWorkspace);
        assert!(
            find_node(
                &previous.root,
                &NodeId::new("debug-session-a-pending-effect").unwrap()
            )
            .is_some()
        );
        let patch = diff(&previous, &next).unwrap();
        assert_eq!(apply_patch(&previous, &patch).unwrap(), next);
        assert!(
            patch
                .operations
                .iter()
                .any(|operation| matches!(operation, UiPatchOperation::Remove { .. }))
        );
        assert!(
            patch
                .operations
                .iter()
                .any(|operation| matches!(operation, UiPatchOperation::Insert { .. }))
        );
    }

    #[test]
    fn debugger_rejects_inconsistent_or_unsafe_projection() {
        let mut invalid = debugger(1, DebuggerState::WaitingEffect, 0, 1);
        invalid.pending_effect = None;
        assert_eq!(debugger_document(&invalid), Err(UiError::InvalidState));

        invalid = debugger(1, DebuggerState::Yielded, 0, 1);
        invalid.pending_effect = Some(DebuggerPendingEffect {
            effect_id: "effect-7".into(),
            kind: DebuggerEffectKind::RuntimeList,
            runtime_id: Some(RuntimeId::new("runtime-a").unwrap()),
        });
        assert_eq!(debugger_document(&invalid), Err(UiError::InvalidState));

        invalid = debugger(1, DebuggerState::Yielded, 0, 2);
        invalid.frames[1].frame_id = invalid.frames[0].frame_id.clone();
        assert_eq!(debugger_document(&invalid), Err(UiError::InvalidState));
        invalid = debugger(1, DebuggerState::Yielded, 0, MAX_DEBUGGER_FRAMES as u32 + 1);
        assert_eq!(
            debugger_document(&invalid),
            Err(UiError::DebuggerLimitExceeded)
        );
        invalid = debugger(1, DebuggerState::Yielded, 0, 1);
        invalid.deadline_remaining_ms = Some(MAX_DEBUGGER_DEADLINE_REMAINING_MS + 1);
        assert_eq!(
            debugger_document(&invalid),
            Err(UiError::DebuggerLimitExceeded)
        );
        invalid = debugger(1, DebuggerState::Yielded, 0, 1);
        invalid.frames[0].display = "unsafe\nframe".into();
        assert_eq!(debugger_document(&invalid), Err(UiError::InvalidState));
    }

    #[test]
    fn unknown_or_non_action_event_fails_closed() {
        let document = fleet_document(&fleet(1, &[("runtime-a", "Runtime A")])).unwrap();
        for node_id in ["missing", "runtime-runtime-a-name"] {
            assert!(
                plan_event(
                    &document,
                    &UiEvent {
                        node_id: NodeId::new(node_id).unwrap(),
                        kind: UiEventKind::Activate
                    },
                    &context(),
                )
                .is_err()
            );
        }
        let mut stale = context();
        stale.expected_revision = Some(Revision(2));
        assert!(matches!(
            plan_event(
                &document,
                &UiEvent {
                    node_id: NodeId::new("runtime-runtime-a-refresh").unwrap(),
                    kind: UiEventKind::Activate,
                },
                &stale,
            ),
            Err(UiError::EventRevisionMismatch { .. })
        ));
    }

    #[test]
    fn diff_is_incremental_and_revision_fenced() {
        let previous = fleet_document(&fleet(1, &[("runtime-a", "Runtime A")])).unwrap();
        let next = fleet_document(&fleet(
            2,
            &[("runtime-a", "Runtime A"), ("runtime-b", "Runtime B")],
        ))
        .unwrap();
        let patch = diff(&previous, &next).unwrap();
        assert_eq!(patch.from_revision, Revision(1));
        assert_eq!(patch.to_revision, Revision(2));
        assert!(patch.operations.iter().any(|operation| matches!(
            operation,
            UiPatchOperation::Insert { node, .. }
                if node.id.as_str() == "runtime-runtime-b"
        )));
        assert_eq!(apply_patch(&previous, &patch).unwrap(), next);
        assert_eq!(diff(&next, &previous), Err(UiError::RevisionRegression));
    }

    #[test]
    fn patch_application_handles_removal_and_rejects_invalid_graph_edits() {
        let previous = fleet_document(&fleet(
            1,
            &[("runtime-a", "Runtime A"), ("runtime-b", "Runtime B")],
        ))
        .unwrap();
        let next = fleet_document(&fleet(2, &[("runtime-b", "Runtime B")])).unwrap();
        let patch = diff(&previous, &next).unwrap();
        assert_eq!(apply_patch(&previous, &patch).unwrap(), next);

        let mut stale = patch.clone();
        stale.from_revision = Revision(0);
        assert!(matches!(
            apply_patch(&previous, &stale),
            Err(UiError::PatchRevisionMismatch { .. })
        ));

        let invalid = UiPatch {
            schema_version: UI_SCHEMA_VERSION,
            from_revision: previous.revision,
            to_revision: Revision(2),
            operations: vec![UiPatchOperation::Move {
                node_id: NodeId::new("runtime-runtime-a").unwrap(),
                parent_id: NodeId::new("runtime-runtime-a-name").unwrap(),
                index: 0,
            }],
        };
        assert!(matches!(
            apply_patch(&previous, &invalid),
            Err(UiError::InvalidPatch { .. })
        ));
    }

    #[test]
    fn validation_rejects_duplicate_ids_and_unlabelled_actions() {
        let mut document = fleet_document(&fleet(1, &[("runtime-a", "Runtime A")])).unwrap();
        document.root.children[1].id = document.root.children[0].id.clone();
        assert!(matches!(
            validate_document(&document),
            Err(UiError::DuplicateNodeId { .. })
        ));

        let mut document = fleet_document(&fleet(1, &[("runtime-a", "Runtime A")])).unwrap();
        document.root.children[2].children[2].accessibility.label = None;
        assert!(matches!(
            validate_document(&document),
            Err(UiError::MissingActionLabel { .. })
        ));

        let mut document = fleet_document(&fleet(1, &[("runtime-a", "Runtime A")])).unwrap();
        document.root.children[2].children[2].action = Some(UiAction::RuntimeRefresh {
            runtime_id: RuntimeId::new("runtime-b").unwrap(),
        });
        assert!(matches!(
            validate_document(&document),
            Err(UiError::InvalidRuntimeBinding { .. })
        ));

        let mut encoded =
            serde_json::to_value(fleet_document(&fleet(1, &[("runtime-a", "Runtime A")])).unwrap())
                .unwrap();
        encoded["unexpected"] = serde_json::json!(true);
        assert!(serde_json::from_value::<UiDocument>(encoded).is_err());
    }

    #[test]
    fn bounded_json_codec_round_trips_documents_and_patches() {
        let previous = fleet_document(&fleet(1, &[("runtime-a", "Runtime A")])).unwrap();
        let next = fleet_document(&fleet(
            2,
            &[("runtime-a", "Runtime A"), ("runtime-b", "Runtime B")],
        ))
        .unwrap();
        let patch = diff(&previous, &next).unwrap();
        let document_bytes = encode_document(&previous).unwrap();
        let patch_bytes = encode_patch(&patch).unwrap();
        assert_eq!(decode_document(&document_bytes).unwrap(), previous);
        assert_eq!(decode_patch(&patch_bytes).unwrap(), patch);
        assert!(matches!(
            decode_document(&vec![b' '; MAX_UI_IR_BYTES + 1]),
            Err(UiError::PayloadTooLarge { .. })
        ));
    }
}
