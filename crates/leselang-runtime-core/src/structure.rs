use std::fmt;

/// Checked, host-granted node/depth accounting for a caller-owned structural walk.
///
/// The caller chooses inclusive limits, starting depth, traversal order and source
/// expansion weights. This meter holds no graph, payload, callback or pending queue.
/// Every physical node costs one, plus caller-supplied folded/expanded source nodes.
/// Repeated edges must be charged each time; there is no implicit deduplication.
///
/// Failed checks leave the successful visit count unchanged. That is not permission
/// to skip a rejected branch: the adapter must reject the graph. Depth is checked
/// before node count, including arithmetic overflow. Neither counter wraps.
/// This is not type/authority validation, a graph certificate, evaluator fuel,
/// allocation quota, cycle detector, callback preemption or an in-process sandbox.
///
/// ```
/// use leselang_runtime_core::{StructureBudget, StructureError};
/// let mut budget = StructureBudget::new(4, 2);
/// budget.visit(0, 0, 0).unwrap();
/// budget.check_pending(0, 1).unwrap(); // observation, not a reservation
/// budget.visit(1, 2, 1).unwrap(); // one folded node plus two source nodes
/// assert_eq!(budget.visited(), 4);
/// assert_eq!(budget.visit(1, 0, 0), Err(StructureError::NodeLimit));
/// assert_eq!(budget.visited(), 4);
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::StructureBudget;
/// let duplicate = StructureBudget::new(4, 2).clone();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::StructureBudget;
/// let budget = StructureBudget::default();
/// ```
///
/// ```compile_fail
/// use leselang_runtime_core::StructureBudget;
/// let wire = serde_json::to_string(&StructureBudget::new(4, 2)).unwrap();
/// ```
#[must_use = "retain the budget and reject a graph when any structural check fails"]
#[derive(Debug)]
pub struct StructureBudget {
    max_nodes: usize,
    max_depth: usize,
    visited: usize,
}

impl StructureBudget {
    /// Limits are explicit host policy, including zero or `usize::MAX`.
    /// Starting a fresh walk is not authorization to re-enter an execution.
    pub const fn new(max_nodes: usize, max_depth: usize) -> Self {
        Self {
            max_nodes,
            max_depth,
            visited: 0,
        }
    }

    /// Successful physical and source-expanded node count, not a graph identity.
    pub const fn visited(&self) -> usize {
        self.visited
    }

    /// Charge one physical node and its source expansion, at an inclusive depth.
    /// `extra_depth` preserves folded source nesting; it does not raise the limit.
    /// A zero expansion still costs one node. No field changes on rejection.
    pub fn visit(
        &mut self,
        depth: usize,
        extra_nodes: usize,
        extra_depth: usize,
    ) -> Result<(), StructureError> {
        depth
            .checked_add(extra_depth)
            .filter(|&depth| depth <= self.max_depth)
            .ok_or(StructureError::DepthLimit)?;
        let visited = self
            .visited
            .checked_add(1)
            .and_then(|count| count.checked_add(extra_nodes))
            .filter(|&count| count <= self.max_nodes)
            .ok_or(StructureError::NodeLimit)?;
        self.visited = visited;
        Ok(())
    }

    /// Check queued physical nodes before extending the caller's pending frontier.
    /// Counts are supplied by the caller; this neither enqueues nor reserves them.
    /// Source expansion and depth must still be checked when each node is visited.
    /// Additional zero nodes is a check, not a charge. No allocation or iteration.
    pub fn check_pending(
        &self,
        pending_nodes: usize,
        additional_nodes: usize,
    ) -> Result<(), StructureError> {
        self.visited
            .checked_add(pending_nodes)
            .and_then(|count| count.checked_add(additional_nodes))
            .filter(|&count| count <= self.max_nodes)
            .map(|_| ())
            .ok_or(StructureError::NodeLimit)
    }
}

/// Payload-free failure; codes, diagnostics and graph rejection belong to the adapter.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StructureError {
    NodeLimit,
    DepthLimit,
}

impl fmt::Display for StructureError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NodeLimit => "structure node limit exceeded",
            Self::DepthLimit => "structure depth limit exceeded",
        })
    }
}

impl std::error::Error for StructureError {}
