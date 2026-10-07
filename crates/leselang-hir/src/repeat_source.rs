//! Bounded flat repeat assembly with explicit native instance factories.

use std::fmt;

use leselang_runtime_core::{StructureBudget, StructureError};
use leselang_syntax::{Expression, NamedArgument, Span};

use crate::group_exports::{MAX_GROUP_EXPORT_MEMBERS, atomic_candidate};
use crate::helper_expansion::HelperExpansionCost;
use crate::ir::{Computation, ComputedBranch, GroupKind};
use crate::source_call::{SourceCallError, SourceCallLimits, preflight, span, valid_limits};
use crate::source_cost::{SourceCostError, SourceCostExtra, SourceCostLimits, measure_source_cost};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepeatSourceLimits {
    pub source: SourceCallLimits,
    pub expanded: SourceCostLimits,
    pub max_repetitions: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepeatSourceShape {
    pub nodes: usize,
    pub depth: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RepeatSourcePhase {
    Lower,
    Materialize,
    Admit,
}

pub enum RepeatSourceError<Error> {
    InvalidLimits,
    NotRepeat {
        span: Span,
    },
    Names {
        span: Span,
    },
    Count {
        span: Span,
    },
    NestedBody {
        span: Span,
    },
    Source(SourceCallError<Error>),
    Output {
        iteration: usize,
        error: StructureError,
    },
    Candidate {
        iteration: usize,
        span: Span,
    },
    Cost {
        iteration: usize,
        error: SourceCostError<Error>,
    },
    ShapeChanged {
        iteration: usize,
        expected: RepeatSourceShape,
        actual: RepeatSourceShape,
    },
    CostChanged {
        iteration: usize,
        expected: HelperExpansionCost,
        actual: HelperExpansionCost,
    },
    Native {
        iteration: usize,
        phase: RepeatSourcePhase,
        span: Span,
        error: Error,
    },
}
impl<Error> fmt::Display for RepeatSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "repeat source limits exceed safety ceilings",
            Self::NotRepeat { .. } => "repeat source requires repeat",
            Self::Names { .. } => "repeat requires exactly times and body",
            Self::Count { .. } => "repeat count must be a bounded positive integer literal",
            Self::NestedBody { .. } => "flat repeat source does not expand nested groups or repeat",
            Self::Source(_) => "repeat source shape or limits are invalid",
            Self::Output { .. } => "repeat output exceeds physical or expanded limits",
            Self::Candidate { .. } => {
                "repeat body must be an atomic candidate after pure preparation"
            }
            Self::Cost { .. } => "repeat source cost measurement failed",
            Self::ShapeChanged { .. } => "repeat instance changed its reserved physical shape",
            Self::CostChanged { .. } => "repeat instance changed its reserved source cost",
            Self::Native { .. } => "native repeat source preparation failed",
        })
    }
}
impl<Error> fmt::Debug for RepeatSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for RepeatSourceError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
type Branch<Field, Operation, HostEffect, IrResult> =
    ComputedBranch<Node<Field, Operation, HostEffect, IrResult>, IrResult>;
pub type RepeatSourceOperand<Node, Result, Error> = std::result::Result<(Node, Result), Error>;
pub type RepeatSourceResult<Node, Error> = Result<Node, RepeatSourceError<Error>>;

/// Explicit trusted child compilation, opaque cost, instance and admission policy.
/// No native Clone/PartialEq/Debug/serde/Send bound or accept-all default is added.
pub trait RepeatSourceAdapter<'source, Field, Operation, HostEffect, IrResult> {
    type Error;

    fn lower_body(
        &mut self,
        body: &'source NamedArgument,
    ) -> RepeatSourceOperand<Node<Field, Operation, HostEffect, IrResult>, IrResult, Self::Error>;

    /// Additional native source weights, relative to the existing Host root.
    /// Returning zero is explicit policy, not proof that an opaque graph is empty.
    fn host_cost(&mut self, effect: &HostEffect) -> Result<SourceCostExtra, Self::Error>;

    /// Iterations are one-based; only 2..=count call this factory. The first owned
    /// body/result is retained unchanged. Factories must faithfully reproduce its
    /// semantics, scopes and native payloads, not dispatch effects or make receipts.
    fn materialize(
        &mut self,
        iteration: usize,
        body: &'source NamedArgument,
        original: &Branch<Field, Operation, HostEffect, IrResult>,
    ) -> RepeatSourceOperand<Node<Field, Operation, HostEffect, IrResult>, IrResult, Self::Error>;

    /// Called once for every iteration after all outputs fit. Exact original and
    /// candidate IR/result pairs are supplied, including the original at iteration 1.
    /// Corroborate complete cold typing, faithful literal/native identity, uniform
    /// atomic operation, original result declarations and live version/grant policy.
    /// Matching physical shape/source cost/result tags is not that certificate.
    fn admit(
        &mut self,
        iteration: usize,
        body: &'source NamedArgument,
        original: &Branch<Field, Operation, HostEffect, IrResult>,
        candidate: &Branch<Field, Operation, HostEffect, IrResult>,
    ) -> Result<(), Self::Error>;
}

fn header<Error>(
    expression: &Expression,
    maximum: usize,
) -> Result<(usize, &NamedArgument), RepeatSourceError<Error>> {
    let at = span(expression);
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(RepeatSourceError::NotRepeat { span: at });
    };
    if callee != "repeat" {
        return Err(RepeatSourceError::NotRepeat { span: at });
    }
    if arguments.len() != 2 {
        return Err(RepeatSourceError::Names { span: at });
    }
    let times = arguments.iter().find(|argument| argument.name == "times");
    let body = arguments.iter().find(|argument| argument.name == "body");
    let (Some(times), Some(body)) = (times, body) else {
        return Err(RepeatSourceError::Names { span: at });
    };
    let Expression::Integer { value, .. } = times.value else {
        return Err(RepeatSourceError::Count { span: times.span });
    };
    if !(1..=maximum as u64).contains(&value) {
        return Err(RepeatSourceError::Count { span: times.span });
    }
    Ok((value as usize, body))
}

fn cold_headers<Error>(
    expression: &Expression,
    maximum: usize,
) -> Result<(), RepeatSourceError<Error>> {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        if let Expression::Call {
            callee, arguments, ..
        } = node
        {
            if callee == "repeat" {
                header::<Error>(node, maximum)?;
            }
            pending.extend(arguments.iter().rev().map(|argument| &argument.value));
        }
    }
    Ok(())
}

fn shape<Field, Operation, HostEffect, IrResult>(
    node: &Node<Field, Operation, HostEffect, IrResult>,
    limits: SourceCallLimits,
) -> Result<RepeatSourceShape, StructureError> {
    let mut budget = StructureBudget::new(limits.max_lowered_nodes, limits.max_lowered_depth);
    let mut pending = vec![(node, 0)];
    let mut depth = 0;
    while let Some((node, at)) = pending.pop() {
        budget.visit(at, 0, 0)?;
        depth = depth.max(at);
        for child in node.children() {
            budget.check_pending(pending.len(), 1)?;
            pending.push((child, at + 1));
        }
    }
    Ok(RepeatSourceShape {
        nodes: budget.visited(),
        depth,
    })
}

fn reserve(
    nodes: usize,
    depth: usize,
    count: usize,
    maximum_nodes: usize,
    maximum_depth: usize,
) -> Result<(), StructureError> {
    depth
        .checked_add(1)
        .filter(|&depth| depth <= maximum_depth)
        .ok_or(StructureError::DepthLimit)?;
    nodes
        .checked_mul(count)
        .and_then(|nodes| nodes.checked_add(1))
        .filter(|&nodes| nodes <= maximum_nodes)
        .ok_or(StructureError::NodeLimit)?;
    Ok(())
}

/// Construct one flat Sequence for repeat(times: 1..=64, body: atomic candidate).
/// Whole source and every cold repeat header precede one original body lowering.
/// Before factories, reserve the complete shifted physical and folded/native
/// source-expanded output, including its Group root. No allocation/copy budget
/// is inferred from a small AST. Original body/result moves into iteration_1;
/// explicit native factories produce 2..=count once, in order, borrowing it.
/// Every instance must preserve exact physical nodes/depth and measured source
/// cost. All bounded atomic candidates precede once ordered native admission.
/// Names are generated once; no native slot, vector or Box is copied by the core.
///
/// Physical/source ceilings are 16,384 nodes and depth 64; limits are inclusive.
/// Zero nodes/count or depth zero deny a repeated Group. Direct nested seq/all/
/// repeat body expansion and naming remain separate outer adapter policy.
/// Physical/source costs do not certify semantics, native graph identity, types,
/// generated language data or authority: admission must validate those properties.
/// Native cost/factory/admission/Drop work and side effects are trusted, not fuel
/// bounded, preempted or sandboxed. Bound ingress before allocation and revalidate
/// live policy before use. Failure/unwind drops consumed output once, stops later
/// hooks and yields no partial group, retry, rollback, fuel or reservation refund.
/// Native errors remain matchable with phase/iteration/span, never formatted or
/// exposed through private source chains. No lexical/runtime frames, prior receipts,
/// dispatcher, repeat loop executor, helper registry or suspension are installed.
pub fn lower_flat_repeat_source<'source, Field, Operation, HostEffect, IrResult, Adapter>(
    expression: &'source Expression,
    limits: RepeatSourceLimits,
    adapter: &mut Adapter,
) -> RepeatSourceResult<Node<Field, Operation, HostEffect, IrResult>, Adapter::Error>
where
    Adapter: RepeatSourceAdapter<'source, Field, Operation, HostEffect, IrResult>,
{
    if !valid_limits(limits.source)
        || limits.expanded.max_nodes > crate::pure_typing::MAX_TYPE_INFERENCE_NODES
        || limits.expanded.max_depth > crate::pure_typing::MAX_TYPE_INFERENCE_DEPTH
        || limits.max_repetitions > MAX_GROUP_EXPORT_MEMBERS
    {
        return Err(RepeatSourceError::InvalidLimits);
    }
    let (count, body) = header(expression, limits.max_repetitions)?;
    preflight(expression, limits.source).map_err(RepeatSourceError::Source)?;
    cold_headers(expression, limits.max_repetitions)?;
    if matches!(&body.value, Expression::Call { callee, .. } if matches!(callee.as_str(), "seq" | "all" | "repeat"))
    {
        return Err(RepeatSourceError::NestedBody { span: body.span });
    }
    let output = |iteration, error| RepeatSourceError::Output { iteration, error };
    reserve(
        1,
        0,
        count,
        limits.source.max_lowered_nodes,
        limits.source.max_lowered_depth,
    )
    .map_err(|error| output(0, error))?;
    reserve(
        1,
        0,
        count,
        limits.expanded.max_nodes,
        limits.expanded.max_depth,
    )
    .map_err(|error| output(0, error))?;
    let native = |iteration, phase, error| RepeatSourceError::Native {
        iteration,
        phase,
        span: body.span,
        error,
    };
    let (value, result_type) = adapter
        .lower_body(body)
        .map_err(|error| native(1, RepeatSourcePhase::Lower, error))?;
    let expected = shape(&value, limits.source).map_err(|error| output(1, error))?;
    reserve(
        expected.nodes,
        expected.depth,
        count,
        limits.source.max_lowered_nodes,
        limits.source.max_lowered_depth,
    )
    .map_err(|error| output(1, error))?;
    if !atomic_candidate(&value) {
        return Err(RepeatSourceError::Candidate {
            iteration: 1,
            span: body.span,
        });
    }
    let cost = measure_source_cost(&value, limits.expanded, |effect| adapter.host_cost(effect))
        .map_err(|error| RepeatSourceError::Cost {
            iteration: 1,
            error,
        })?;
    reserve(
        cost.nodes,
        cost.depth,
        count,
        limits.expanded.max_nodes,
        limits.expanded.max_depth,
    )
    .map_err(|error| output(1, error))?;
    let mut branches = Vec::with_capacity(count);
    branches.push(ComputedBranch {
        name: "iteration_1".into(),
        value,
        result_type,
    });
    for iteration in 2..=count {
        let (value, result_type) = adapter
            .materialize(iteration, body, &branches[0])
            .map_err(|error| native(iteration, RepeatSourcePhase::Materialize, error))?;
        let actual = shape(&value, limits.source).map_err(|error| output(iteration, error))?;
        if actual != expected {
            return Err(RepeatSourceError::ShapeChanged {
                iteration,
                expected,
                actual,
            });
        }
        if !atomic_candidate(&value) {
            return Err(RepeatSourceError::Candidate {
                iteration,
                span: body.span,
            });
        }
        let actual =
            measure_source_cost(&value, limits.expanded, |effect| adapter.host_cost(effect))
                .map_err(|error| RepeatSourceError::Cost { iteration, error })?;
        if actual != cost {
            return Err(RepeatSourceError::CostChanged {
                iteration,
                expected: cost,
                actual,
            });
        }
        branches.push(ComputedBranch {
            name: format!("iteration_{iteration}"),
            value,
            result_type,
        });
    }
    for (index, branch) in branches.iter().enumerate() {
        adapter
            .admit(index + 1, body, &branches[0], branch)
            .map_err(|error| native(index + 1, RepeatSourcePhase::Admit, error))?;
    }
    Ok(Computation::Group {
        group_kind: GroupKind::Sequence,
        branches,
    })
}
