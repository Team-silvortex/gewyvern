//! Source-bounded normal-return joining with explicit native copy and admission.

use std::{collections::HashMap, convert::Infallible, fmt};

use leselang_runtime_core::StructureError;

use crate::helper_expansion::HelperExpansionCost;
use crate::helper_returns::{
    HelperReturnError, HelperReturnLimits, HelperReturnShape, HelperReturns,
};
use crate::ir::Computation;
use crate::pure_typing::{MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES};
use crate::source_cost::{SourceCostError, SourceCostExtra, SourceCostLimits, measure_source_cost};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperJoinLimits {
    pub output: HelperReturnLimits,
    pub source: SourceCostLimits,
}

pub enum HelperJoinError<Error> {
    InvalidLimits,
    Plan(HelperReturnError<Infallible>),
    Language(SourceCostError<Infallible>),
    Bounds(StructureError),
    NativeCost(Error),
    MissingObservation,
    Copy(HelperReturnError<Error>),
    ChangedSource,
    Admission(Error),
}
impl<Error> fmt::Display for HelperJoinError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper join limits exceed safety ceilings",
            Self::Plan(_) => "helper join return plan was rejected",
            Self::Language(_) => "helper join language source exceeds bounds",
            Self::Bounds(_) => "helper join combined source exceeds bounds",
            Self::NativeCost(_) => "helper join native source observation failed",
            Self::MissingObservation => "helper join original native observation is missing",
            Self::Copy(_) => "helper join continuation materialization failed",
            Self::ChangedSource => "helper join materialization changed language source weights",
            Self::Admission(_) => "helper join complete native admission failed",
        })
    }
}
impl<Error> fmt::Debug for HelperJoinError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for HelperJoinError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

/// Single-use owned return plan, not a global reservation, type or authority token.
/// Debug exposes only counts. Native slots require no Clone/Debug/serde/Send bound.
#[must_use = "retain or explicitly consume the bounded helper join"]
pub struct HelperJoin<Node> {
    returns: HelperReturns<Node>,
    source_cost: HelperExpansionCost,
    language_cost: HelperExpansionCost,
    limits: HelperJoinLimits,
}
impl<Node> fmt::Debug for HelperJoin<Node> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HelperJoin")
            .field("shape", &self.returns.shape())
            .field("source_cost", &self.source_cost)
            .finish()
    }
}
impl<Node> HelperJoin<Node> {
    pub fn name(&self) -> &str {
        self.returns.name()
    }
    pub fn value(&self) -> &Node {
        self.returns.value()
    }
    pub fn continuation(&self) -> &Node {
        self.returns.continuation()
    }
    pub fn shape(&self) -> HelperReturnShape {
        self.returns.shape()
    }
    pub const fn source_cost(&self) -> HelperExpansionCost {
        self.source_cost
    }
}

impl<Field, Operation, HostEffect, IrResult>
    HelperJoin<Node<Field, Operation, HostEffect, IrResult>>
{
    /// Bound both complete cold inputs, names, return routes, physical output and
    /// joined folded language weights before any native observer. Inclusive source
    /// and physical ceilings are 16,384 nodes and depth 64, independently enforced.
    /// Each original Host is then observed once: value followed by continuation,
    /// each in rightmost-first DFS order. Only explicit extra-cost metadata is
    /// cached locally by original language-node identity, never native payloads or
    /// grants. Zero-sized native slots are distinct occurrences even at one address.
    /// Return subtrees reuse these costs; every cold continuation copy is included.
    /// No native materialization occurs. Enclosing expansion counters, canonical
    /// bytes, native graph closure, lexical typing and live authority remain caller
    /// gates before connect. Native work/mutation/allocation cannot be preempted.
    pub fn new<Error>(
        name: String,
        value: Node<Field, Operation, HostEffect, IrResult>,
        continuation: Node<Field, Operation, HostEffect, IrResult>,
        limits: HelperJoinLimits,
        mut host_cost: impl FnMut(&HostEffect) -> Result<SourceCostExtra, Error>,
    ) -> Result<Self, HelperJoinError<Error>> {
        if limits.source.max_nodes > MAX_TYPE_INFERENCE_NODES
            || limits.source.max_depth > MAX_TYPE_INFERENCE_DEPTH
        {
            return Err(HelperJoinError::InvalidLimits);
        }
        let returns = HelperReturns::new(name, value, continuation, limits.output)
            .map_err(HelperJoinError::Plan)?;
        let language_cost = language_join_cost(&returns, limits.source)?;
        let mut observations = HashMap::new();
        let mut observe = |node, effect: &HostEffect| {
            let extra = host_cost(effect).map_err(HelperJoinError::NativeCost)?;
            observations.insert(node, extra);
            Ok(extra)
        };
        let value_cost = measure_observed(returns.value(), limits.source, &mut observe)?;
        let continuation_cost =
            measure_observed(returns.continuation(), limits.source, &mut observe)?;
        let source_cost = joined_cost(
            &returns,
            limits.source,
            value_cost,
            continuation_cost,
            |node| {
                measure_observed(node, limits.source, |node, _| {
                    observations
                        .get(&node)
                        .copied()
                        .ok_or(HelperJoinError::MissingObservation)
                })
            },
        )?;
        Ok(Self {
            returns,
            source_cost,
            language_cost,
            limits,
        })
    }

    /// Consume once: explicit left-to-right continuation factories, complete
    /// physical/name/capture checks, exact joined language/folded source weights,
    /// then mandatory whole-output admission once with the planned native-inclusive
    /// source cost. Admission must corroborate faithful copies, bounded native
    /// graphs/costs, typing, canonical bytes and live schemas/grants. Equal physical
    /// shape or source weight does not certify semantic identity or authority.
    /// No evaluation, dispatch, implicit retry, reservation refund or rollback.
    /// Error/unwind publishes no partial IR and drops consumed inputs; host Drop
    /// and a second cleanup panic can still abort. Errors retain native payloads
    /// for matching, not formatting or source chains.
    pub fn connect<Error>(
        self,
        factory: impl FnMut(
            &Node<Field, Operation, HostEffect, IrResult>,
            usize,
        ) -> Result<Node<Field, Operation, HostEffect, IrResult>, Error>,
        admit: impl FnOnce(
            &Node<Field, Operation, HostEffect, IrResult>,
            HelperExpansionCost,
        ) -> Result<(), Error>,
    ) -> Result<Node<Field, Operation, HostEffect, IrResult>, HelperJoinError<Error>> {
        let expression = self
            .returns
            .connect(factory)
            .map_err(HelperJoinError::Copy)?;
        let actual =
            language_cost(&expression, self.limits.source).map_err(HelperJoinError::Language)?;
        if actual != self.language_cost {
            return Err(HelperJoinError::ChangedSource);
        }
        admit(&expression, self.source_cost).map_err(HelperJoinError::Admission)?;
        Ok(expression)
    }
}

// The language node, unlike a boxed zero-sized native payload, has stable distinct
// identity during this borrowed measurement. The complete cold plan is bounded.
fn measure_observed<Field, Operation, HostEffect, IrResult, Error>(
    node: &Node<Field, Operation, HostEffect, IrResult>,
    limits: SourceCostLimits,
    mut observe: impl FnMut(
        *const Node<Field, Operation, HostEffect, IrResult>,
        &HostEffect,
    ) -> Result<SourceCostExtra, HelperJoinError<Error>>,
) -> Result<HelperExpansionCost, HelperJoinError<Error>> {
    let mut pending = vec![node];
    let mut hosts = Vec::new();
    while let Some(node) = pending.pop() {
        if let Node::Host { effect } = node {
            hosts.push((std::ptr::from_ref(node), effect.as_ref()));
        }
        pending.extend(node.children());
    }
    let mut hosts = hosts.into_iter();
    measure_source_cost(node, limits, |effect| {
        let (node, original) = hosts.next().ok_or(HelperJoinError::MissingObservation)?;
        if !std::ptr::eq(effect, original) {
            return Err(HelperJoinError::MissingObservation);
        }
        observe(node, effect)
    })
    .map_err(|error| match error {
        SourceCostError::InvalidLimits => HelperJoinError::InvalidLimits,
        SourceCostError::Structure(error) => HelperJoinError::Bounds(error),
        SourceCostError::Observation { error, .. } => error,
    })
}

fn language_cost<Field, Operation, HostEffect, IrResult>(
    node: &Node<Field, Operation, HostEffect, IrResult>,
    limits: SourceCostLimits,
) -> Result<HelperExpansionCost, SourceCostError<Infallible>> {
    measure_source_cost(node, limits, |_| Ok(SourceCostExtra { nodes: 0, depth: 0 }))
}

fn language_join_cost<Field, Operation, HostEffect, IrResult, Error>(
    plan: &HelperReturns<Node<Field, Operation, HostEffect, IrResult>>,
    limits: SourceCostLimits,
) -> Result<HelperExpansionCost, HelperJoinError<Error>> {
    let value = language_cost(plan.value(), limits).map_err(HelperJoinError::Language)?;
    let continuation =
        language_cost(plan.continuation(), limits).map_err(HelperJoinError::Language)?;
    joined_cost(plan, limits, value, continuation, |node| {
        language_cost(node, limits).map_err(HelperJoinError::Language)
    })
}

fn joined_cost<Field, Operation, HostEffect, IrResult, Error>(
    plan: &HelperReturns<Node<Field, Operation, HostEffect, IrResult>>,
    limits: SourceCostLimits,
    value: HelperExpansionCost,
    continuation: HelperExpansionCost,
    mut return_cost: impl FnMut(
        &Node<Field, Operation, HostEffect, IrResult>,
    ) -> Result<HelperExpansionCost, HelperJoinError<Error>>,
) -> Result<HelperExpansionCost, HelperJoinError<Error>> {
    let mut depth = value.depth;
    for (node, level) in plan.return_sites() {
        let cost = return_cost(node)?;
        let shifted = level
            .checked_add(1)
            .and_then(|level| level.checked_add(cost.depth.max(continuation.depth)))
            .filter(|depth| *depth <= limits.max_depth)
            .ok_or(HelperJoinError::Bounds(StructureError::DepthLimit))?;
        depth = depth.max(shifted);
    }
    let nodes = continuation
        .nodes
        .checked_add(1)
        .and_then(|nodes| nodes.checked_mul(plan.shape().returns))
        .and_then(|nodes| value.nodes.checked_add(nodes))
        .filter(|nodes| *nodes <= limits.max_nodes)
        .ok_or(HelperJoinError::Bounds(StructureError::NodeLimit))?;
    Ok(HelperExpansionCost { nodes, depth })
}
