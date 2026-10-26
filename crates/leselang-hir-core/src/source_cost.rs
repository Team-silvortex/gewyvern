//! Bounded language source weights with explicit opaque-host cost observations.

use std::fmt;

use leselang_runtime_core::{ScalarValue, StructureBudget, StructureError};

use crate::helper_expansion::HelperExpansionCost;
use crate::ir::Computation;
use crate::pure_typing::{MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCostLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
}

/// Additional source nodes and maximum depth offset from an existing language
/// root. Neither includes that root again. Zero is valid; native observations may
/// undercount, so this is not physical graph identity or a semantic certificate.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCostExtra {
    pub nodes: usize,
    pub depth: usize,
}

pub enum SourceCostError<Error> {
    InvalidLimits,
    Structure(StructureError),
    Observation { host_index: usize, error: Error },
}
impl<Error> fmt::Display for SourceCostError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "source cost limits exceed safety ceilings",
            Self::Structure(_) => "source cost exceeds node or depth bounds",
            Self::Observation { .. } => "native source cost observation failed",
        })
    }
}
impl<Error> fmt::Debug for SourceCostError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for SourceCostError<Error> {}

/// Folded constructor weights, not literal byte/type validation. Every optional
/// constructor retains one child, including none; an empty list retains no child.
/// The language root itself is charged separately. No buffers are inspected/copied.
pub fn literal_source_extra(value: &ScalarValue) -> SourceCostExtra {
    let (nodes, depth) = match value {
        ScalarValue::OptionalString(_) => (1, 1),
        ScalarValue::StringList(value) => (value.0.len(), usize::from(!value.0.is_empty())),
        _ => (0, 0),
    };
    SourceCostExtra { nodes, depth }
}

/// Measure a borrowed complete language tree with explicit inclusive ceilings of
/// 16,384 source nodes and depth 64. Zero nodes denies any root; zero depth permits
/// leaves without folded/native depth. Each language node costs one, preserving
/// folded constructor weights, cold children and repeated occurrences. No purity,
/// literal bounds, lexical/type, schema or authority certificate is produced.
///
/// One iterative language walk bounds pending frontiers before every push and
/// collects only borrowed Host sites. All language/folded costs pass before the
/// first native observer. Sites run once in rightmost-child-first DFS order, like
/// the reference source walk; errors/unwind stop without retry or partial output.
/// Native side effects cannot be rolled back. Extra depth is relative to the Host
/// language root, not the opaque graph root; a direct native child has offset one.
/// Zero native extra cost is explicit policy, never inferred from opaque payloads.
/// Checked additions preserve depth-before-node error priority for each visit or
/// native append. Opaque graphs must be closed and separately bounded by adapters.
///
/// Original Boxes/vectors/native slots move nowhere and need no Clone/Debug/serde/
/// Send bound. Native work/allocation/interior mutation/unwind remains trusted,
/// not fuel-limited, preempted or sandboxed. This is measurement, not expansion
/// reservation, execution fuel, canonical text size, dispatch or replay authority.
/// Reusing this observation requires fresh complete validation of mutable IR and
/// native metadata. No global registry, lock, default, journal or cache is installed.
/// Native errors are retained for matching only, not formatting or source chains.
pub fn measure_source_cost<Field, Operation, HostEffect, IrResult, Error>(
    expression: &Computation<Field, Operation, HostEffect, IrResult>,
    limits: SourceCostLimits,
    mut host_cost: impl FnMut(&HostEffect) -> Result<SourceCostExtra, Error>,
) -> Result<HelperExpansionCost, SourceCostError<Error>> {
    if limits.max_nodes > MAX_TYPE_INFERENCE_NODES || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH {
        return Err(SourceCostError::InvalidLimits);
    }
    let mut budget = StructureBudget::new(limits.max_nodes, limits.max_depth);
    let mut pending = vec![(expression, 0)];
    let mut hosts = Vec::new();
    let mut maximum_depth = 0;
    while let Some((node, depth)) = pending.pop() {
        let extra = match node {
            Computation::Literal { value } => literal_source_extra(value),
            _ => SourceCostExtra { nodes: 0, depth: 0 },
        };
        budget
            .visit(depth, extra.nodes, extra.depth)
            .map_err(SourceCostError::Structure)?;
        maximum_depth = maximum_depth.max(depth + extra.depth);
        if let Computation::Host { effect } = node {
            hosts.push((effect.as_ref(), depth));
        }
        for child in node.children() {
            budget
                .check_pending(pending.len(), 1)
                .map_err(SourceCostError::Structure)?;
            pending.push((child, depth + 1));
        }
    }
    let mut cost = HelperExpansionCost {
        nodes: budget.visited(),
        depth: maximum_depth,
    };
    for (host_index, (effect, depth)) in hosts.into_iter().enumerate() {
        let extra = host_cost(effect)
            .map_err(|error| SourceCostError::Observation { host_index, error })?;
        let maximum = depth
            .checked_add(extra.depth)
            .filter(|&depth| depth <= limits.max_depth)
            .ok_or(SourceCostError::Structure(StructureError::DepthLimit))?;
        let nodes = cost
            .nodes
            .checked_add(extra.nodes)
            .filter(|&nodes| nodes <= limits.max_nodes)
            .ok_or(SourceCostError::Structure(StructureError::NodeLimit))?;
        cost = HelperExpansionCost {
            nodes,
            depth: cost.depth.max(maximum),
        };
    }
    Ok(cost)
}
