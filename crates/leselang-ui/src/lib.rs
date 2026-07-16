use std::collections::{BTreeMap, BTreeSet};

use leselang_command::{LoweringContext, LoweringError, plan_runtime_refresh};
use leserpent_domain::{CommandPlan, QueryResult, RefreshStatus, Revision, RuntimeId};
use serde::{Deserialize, Serialize};

pub const UI_SCHEMA_VERSION: u32 = 1;
pub const MAX_UI_NODES: usize = 4_096;
pub const MAX_UI_DEPTH: usize = 32;
pub const MAX_UI_TEXT_BYTES: usize = 1_024;
pub const MAX_UI_PATCH_OPERATIONS: usize = 8_192;

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
    Action,
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
    EventRevisionMismatch {
        document: Revision,
        expected: Option<Revision>,
    },
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
    let old = index_document(previous);
    let new = index_document(next);
    let mut operations = Vec::new();

    let mut removed = old
        .iter()
        .filter(|(id, _)| !new.contains_key(*id))
        .map(|(id, entry)| (entry.depth, (*id).clone()))
        .collect::<Vec<_>>();
    removed.sort_by(|left, right| right.cmp(left));
    operations.extend(
        removed
            .into_iter()
            .map(|(_, node_id)| UiPatchOperation::Remove { node_id }),
    );

    for (id, entry) in &new {
        let Some(old_entry) = old.get(id) else {
            if entry
                .parent
                .as_ref()
                .is_some_and(|parent| old.contains_key(parent))
            {
                operations.push(UiPatchOperation::Insert {
                    parent_id: entry.parent.clone().expect("checked parent"),
                    index: entry.index,
                    node: entry.node.clone(),
                });
            }
            continue;
        };
        if old_entry.parent != entry.parent || old_entry.index != entry.index {
            if let Some(parent_id) = &entry.parent {
                operations.push(UiPatchOperation::Move {
                    node_id: id.clone(),
                    parent_id: parent_id.clone(),
                    index: entry.index,
                });
            }
        }
        if shallow_node(old_entry.node) != shallow_node(entry.node) {
            let mut node = entry.node.clone();
            node.children.clear();
            operations.push(UiPatchOperation::Update { node });
        }
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
        (UiNodeKind::RuntimeCard, Some(runtime_id)) => Some(runtime_id),
        (UiNodeKind::RuntimeCard, None) | (_, Some(_)) => {
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
    index: usize,
    depth: usize,
}

fn index_document(document: &UiDocument) -> BTreeMap<NodeId, IndexedNode<'_>> {
    fn visit<'a>(
        node: &'a UiNode,
        parent: Option<NodeId>,
        index: usize,
        depth: usize,
        output: &mut BTreeMap<NodeId, IndexedNode<'a>>,
    ) {
        output.insert(
            node.id.clone(),
            IndexedNode {
                node,
                parent: parent.clone(),
                index,
                depth,
            },
        );
        for (index, child) in node.children.iter().enumerate() {
            visit(child, Some(node.id.clone()), index, depth + 1, output);
        }
    }
    let mut output = BTreeMap::new();
    visit(&document.root, None, 0, 1, &mut output);
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
        CAPABILITY_RUNTIME_REFRESH, CapabilitySet, Command, CommandId, CommandOrigin, Confirmation,
        IdempotencyKey, Principal, RuntimeListFilter,
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
}
