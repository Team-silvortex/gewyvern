//! Bounded borrowed native graph inspection, not typing or execution authority.

use std::fmt;

use leselang_runtime_core::{StructureBudget, StructureError};

use crate::ir::GroupKind;

pub const MAX_NATIVE_GRAPH_NODES: usize = 16 * 1024;
pub const MAX_NATIVE_GRAPH_DEPTH: usize = 64;
pub const MAX_NATIVE_GRAPH_MEMBERS: usize = 64;

/// Explicit inclusive physical limits. Root depth is zero; repeated edges count
/// again. These do not bound leaf payload bytes, callback work or language IR.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeGraphLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_members: usize,
}

/// A trusted original native view, never an owned conversion or inferred schema.
/// Members must borrow the original graph. A Leaf may still hide opaque payloads;
/// its adapter must validate those separately. No native traits are required.
pub enum NativeGraphShape<'graph, Branch> {
    Leaf,
    Group {
        kind: GroupKind,
        members: &'graph [Branch],
    },
}
impl<Branch> fmt::Debug for NativeGraphShape<'_, Branch> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Leaf => formatter.write_str("NativeGraphShape::Leaf"),
            Self::Group { kind, members } => formatter
                .debug_struct("NativeGraphShape::Group")
                .field("kind", kind)
                .field("members", &members.len())
                .finish(),
        }
    }
}

/// Ephemeral counts, not graph identity, a type observation or a reusable grant.
#[must_use = "counts do not replace current native type and authority admission"]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NativeGraphSummary {
    pub nodes: usize,
    pub depth: usize,
    pub groups: usize,
    pub leaves: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NativeGraphPhase {
    View,
    Child { member_index: usize },
    Leaf,
}

/// Positions/phases remain matchable. Native payloads are never
/// formatted or exposed through Error::source. No Debug/Display bound is needed.
///
/// ```compile_fail
/// use leselang_hir::native_graph::NativeGraphError;
/// let wire = serde_json::to_string(&NativeGraphError::<()>::InvalidLimits).unwrap();
/// ```
pub enum NativeGraphError<Error> {
    InvalidLimits,
    Structure {
        node_index: usize,
        error: StructureError,
    },
    Arity {
        node_index: usize,
    },
    Native {
        node_index: usize,
        phase: NativeGraphPhase,
        error: Error,
    },
}
impl<Error> fmt::Display for NativeGraphError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "native graph limits exceed safety ceilings",
            Self::Structure { .. } => "native graph exceeds node or depth bounds",
            Self::Arity { .. } => "native graph group has invalid member count",
            Self::Native { .. } => "native graph structural observation failed",
        })
    }
}
impl<Error> fmt::Debug for NativeGraphError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for NativeGraphError<Error> {}

struct Frame<'graph, Branch> {
    members: &'graph [Branch],
    next: usize,
    parent_index: usize,
    depth: usize,
}

/// Inspect one complete borrowed native graph in declaration-order DFS. Each
/// occurrence visits once, even with shared edges; there is no deduplication or
/// separate cycle detection. Node/depth limits bound cyclic/repeated traversal,
/// provided trusted callbacks return. Sequence needs one member and Parallel two.
/// Zero nodes denies the root before hooks; zero depth permits only leaves. Zero
/// members denies groups, not leaves. No quota silently expands.
///
/// Charge each node BEFORE View. Check group arity before any child mapping; map
/// each original branch once immediately before visiting that child. Leaf runs
/// once at that node, preserving inline native structural diagnostic order.
/// All hooks borrow original native slots; Node may be unsized and GUI-local.
/// There is no Clone/Debug/serde/Send/PartialEq requirement or second graph tree.
/// The core retains one borrowed sibling cursor per open group, not a frontier
/// containing every future sibling. Limits bound cursor depth before growth.
///
/// Views/edge mapping are explicit trusted structural protocols, NOT validation
/// of their own authenticity. Adapters must expose the original graph without
/// hidden expansion and bound native ingress, payloads, allocation and callback
/// work. Leaf is a physical native check, not dispatch, result acceptance or a
/// policy grant. Whole-cold typing/authority must follow complete inspection;
/// this entry does not promise that all physical errors precede inline leaf work.
/// A successful mutable count summary cannot certify later graph/policy changes.
///
/// Failure/unwind stops further hooks without retries or partial summary. The
/// borrowed graph is not consumed; native external work, interior mutation and
/// Drop cannot be rolled back or preempted. This is neither nested export/type
/// inference, source flattening/repeat expansion, a scheduler, fuel reservation,
/// canonical admission, durable continuation nor an execution certificate.
pub fn inspect_native_graph<'graph, Node: ?Sized + 'graph, Branch: 'graph, Error>(
    root: &'graph Node,
    limits: NativeGraphLimits,
    mut view: impl FnMut(&'graph Node) -> Result<NativeGraphShape<'graph, Branch>, Error>,
    mut child: impl FnMut(&'graph Branch) -> Result<&'graph Node, Error>,
    mut leaf: impl FnMut(&'graph Node) -> Result<(), Error>,
) -> Result<NativeGraphSummary, NativeGraphError<Error>> {
    if limits.max_nodes > MAX_NATIVE_GRAPH_NODES
        || limits.max_depth > MAX_NATIVE_GRAPH_DEPTH
        || limits.max_members > MAX_NATIVE_GRAPH_MEMBERS
    {
        return Err(NativeGraphError::InvalidLimits);
    }
    let mut budget = StructureBudget::new(limits.max_nodes, limits.max_depth);
    let mut frames = Vec::<Frame<'graph, Branch>>::new();
    let mut current = Some((root, 0));
    let mut summary = NativeGraphSummary {
        nodes: 0,
        depth: 0,
        groups: 0,
        leaves: 0,
    };
    loop {
        if let Some((node, depth)) = current.take() {
            let node_index = budget.visited();
            budget
                .visit(depth, 0, 0)
                .map_err(|error| NativeGraphError::Structure { node_index, error })?;
            summary.nodes = budget.visited();
            summary.depth = summary.depth.max(depth);
            match view(node).map_err(|error| NativeGraphError::Native {
                node_index,
                phase: NativeGraphPhase::View,
                error,
            })? {
                NativeGraphShape::Leaf => {
                    leaf(node).map_err(|error| NativeGraphError::Native {
                        node_index,
                        phase: NativeGraphPhase::Leaf,
                        error,
                    })?;
                    summary.leaves += 1;
                }
                NativeGraphShape::Group { kind, members } => {
                    let minimum = if kind == GroupKind::Sequence { 1 } else { 2 };
                    if !(minimum..=limits.max_members).contains(&members.len()) {
                        return Err(NativeGraphError::Arity { node_index });
                    }
                    summary.groups += 1;
                    frames.push(Frame {
                        members,
                        next: 0,
                        parent_index: node_index,
                        depth: depth + 1,
                    });
                }
            }
        }
        while let Some(frame) = frames.last_mut() {
            if let Some(member) = frame.members.get(frame.next) {
                let node = child(member).map_err(|error| NativeGraphError::Native {
                    node_index: frame.parent_index,
                    phase: NativeGraphPhase::Child {
                        member_index: frame.next,
                    },
                    error,
                })?;
                frame.next += 1;
                current = Some((node, frame.depth));
                break;
            }
            frames.pop();
        }
        if current.is_none() {
            return Ok(summary);
        }
    }
}
