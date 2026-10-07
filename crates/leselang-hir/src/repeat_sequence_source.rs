//! Repeated owned sequence templates, without opaque-effect conversion or cloning.

use std::fmt;

use leselang_syntax::{Expression, NamedArgument, Span};

use crate::call_typing::valid_argument_name;
use crate::group_exports::{MAX_GROUP_EXPORT_MEMBERS, atomic_candidate};
use crate::group_source::{GroupSourceError, cold_headers as group_headers};
use crate::ir::{Computation, ComputedBranch, GroupKind};
use crate::pure_typing::{
    MAX_LOCAL_NAME_BYTES, MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES,
};
use crate::repeat_source::{
    RepeatSourceError, RepeatSourceLimits, RepeatSourcePhase, cold_headers, header, reserve, shape,
};
use crate::source_call::{preflight, valid_limits};
use crate::source_cost::{SourceCostExtra, measure_source_cost};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RepeatSequenceSourceLimits {
    pub repeat: RepeatSourceLimits,
    pub max_branches: usize,
}

pub enum RepeatSequenceSourceError<Error> {
    Repeat(RepeatSourceError<Error>),
    Group(GroupSourceError<Error>),
    Body {
        span: Span,
    },
    Parallel {
        span: Span,
    },
    Width {
        iteration: usize,
        span: Span,
    },
    Name {
        iteration: usize,
        member: usize,
        span: Span,
    },
    NamesChanged {
        iteration: usize,
        member: usize,
        span: Span,
    },
    Candidate {
        iteration: usize,
        member: usize,
        span: Span,
    },
    Admission {
        iteration: usize,
        member: usize,
        span: Span,
        error: Error,
    },
}
impl<Error> fmt::Display for RepeatSequenceSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Repeat(_) => "repeated sequence source or output bounds are invalid",
            Self::Group(_) => "repeated sequence cold group headers are invalid",
            Self::Body { .. } => "repeated sequence body requires seq or repeat",
            Self::Parallel { .. } => "parallel groups cannot be repeated sequentially",
            Self::Width { .. } => "repeated sequence width is empty, changed or exceeds limits",
            Self::Name { .. } => "repeated sequence labels must be unique bounded identifiers",
            Self::NamesChanged { .. } => "repeated sequence instance changed template labels",
            Self::Candidate { .. } => "repeated sequence member is not an atomic candidate",
            Self::Admission { .. } => "native repeated sequence admission failed",
        })
    }
}
impl<Error> fmt::Debug for RepeatSequenceSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for RepeatSequenceSourceError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
type Branch<Field, Operation, HostEffect, IrResult> =
    ComputedBranch<Node<Field, Operation, HostEffect, IrResult>, IrResult>;
pub type RepeatSequenceOperand<Node, Result, Error> =
    std::result::Result<Vec<ComputedBranch<Node, Result>>, Error>;
pub type RepeatSequenceResult<Node, Error> = Result<Node, RepeatSequenceSourceError<Error>>;

/// Trusted child compiler, native cost and faithful instance factories. No native
/// Clone/PartialEq/Debug/serde/Send bound or accept-all default is imposed.
pub trait RepeatSequenceSourceAdapter<'source, Field, Operation, HostEffect, IrResult> {
    type Error;

    fn lower_sequence(
        &mut self,
        body: &'source NamedArgument,
    ) -> RepeatSequenceOperand<Node<Field, Operation, HostEffect, IrResult>, IrResult, Self::Error>;
    fn host_cost(&mut self, effect: &HostEffect) -> Result<SourceCostExtra, Self::Error>;
    /// Called for 2..=count with the original template's unprefixed labels, IR and
    /// declarations, never a previous copy or a synthetic group result tag.
    fn materialize_sequence(
        &mut self,
        iteration: usize,
        body: &'source NamedArgument,
        original: &[Branch<Field, Operation, HostEffect, IrResult>],
    ) -> RepeatSequenceOperand<Node<Field, Operation, HostEffect, IrResult>, IrResult, Self::Error>;
    /// One-based iteration, zero-based template member. Called after all instances
    /// fit, with original/candidate rows at the same position, including iteration 1.
    fn admit_member(
        &mut self,
        iteration: usize,
        member: usize,
        body: &'source NamedArgument,
        original: &Branch<Field, Operation, HostEffect, IrResult>,
        candidate: &Branch<Field, Operation, HostEffect, IrResult>,
    ) -> Result<(), Self::Error>;
}

/// Construct one flat Group for repeat whose direct body is Seq/Repeat. Recursion
/// and template expansion belong to the child compiler; direct All is refused.
/// Whole cold AST, repeat counts and Seq/All headers precede one template lowering.
/// Limits are inclusive: 16,384 source/output nodes, depth 64, 64 repetitions and
/// total expanded branches. Zero capacity/depth denies entry. The template must be
/// nonempty and already flat, with unique bounded labels and atomic candidates.
///
/// Before any factory, reserve the whole flattened output: one Group root plus
/// count times the template member forest, for both physical and folded/native
/// source costs. Removed intermediate Group roots are not repeatedly charged.
/// Check every final `iteration_N__child` label against 64 bytes before allocation.
/// The original template becomes iteration 1 without materialization; factories
/// run once in iteration order, borrowing its exact original rows and buffers.
/// Generated width, ordered labels, aggregate physical shape and source cost must
/// match the template. All instances/candidates precede once-ordered admission.
///
/// Admit must corroborate faithful literal/native identity, exact declarations,
/// complete cold typing, closed opaque graphs, cost accuracy and live grants/version.
/// Shape/cost/labels or equal result tags are not these certificates. No native
/// slot/Box/operand buffer is copied by the core; child branch vectors are consumed
/// into one bounded result vector. Previous receipts/type/runtime locals are not
/// installed. Callback work/allocation/Drop/unwind is trusted, unmetered and not
/// sandboxed. Bound ingress first, guard native scopes and revalidate policy before
/// use. Failure/unwind stops later hooks, drops consumed parts once and returns no
/// partial output, retry, rollback or source/fuel refund. Error payloads remain
/// matchable with iteration/member/phase/span, never formatted or exposed as private
/// source chains. This is assembly, not complete child compilation, dispatch,
/// reply acceptance, scheduling or suspension ownership.
pub fn lower_repeat_sequence_source<'source, Field, Operation, HostEffect, IrResult, Adapter>(
    expression: &'source Expression,
    limits: RepeatSequenceSourceLimits,
    adapter: &mut Adapter,
) -> RepeatSequenceResult<Node<Field, Operation, HostEffect, IrResult>, Adapter::Error>
where
    Adapter: RepeatSequenceSourceAdapter<'source, Field, Operation, HostEffect, IrResult>,
{
    let repeat = RepeatSequenceSourceError::Repeat;
    let bounds = limits.repeat;
    if !valid_limits(bounds.source)
        || bounds.expanded.max_nodes > MAX_TYPE_INFERENCE_NODES
        || bounds.expanded.max_depth > MAX_TYPE_INFERENCE_DEPTH
        || bounds.max_repetitions > MAX_GROUP_EXPORT_MEMBERS
        || limits.max_branches > MAX_GROUP_EXPORT_MEMBERS
    {
        return Err(repeat(RepeatSourceError::InvalidLimits));
    }
    let (count, body) = header(expression, bounds.max_repetitions).map_err(repeat)?;
    preflight(expression, bounds.source)
        .map_err(RepeatSourceError::Source)
        .map_err(repeat)?;
    cold_headers(expression, bounds.max_repetitions).map_err(repeat)?;
    group_headers(expression, limits.max_branches).map_err(RepeatSequenceSourceError::Group)?;
    match &body.value {
        Expression::Call { callee, .. } if matches!(callee.as_str(), "seq" | "repeat") => {}
        Expression::Call { callee, .. } if callee == "all" => {
            return Err(RepeatSequenceSourceError::Parallel { span: body.span });
        }
        _ => return Err(RepeatSequenceSourceError::Body { span: body.span }),
    }
    let width_error = |iteration| RepeatSequenceSourceError::Width {
        iteration,
        span: body.span,
    };
    if count > limits.max_branches {
        return Err(width_error(0));
    }
    let output = |iteration, error| repeat(RepeatSourceError::Output { iteration, error });
    reserve(
        1,
        0,
        count,
        bounds.source.max_lowered_nodes,
        bounds.source.max_lowered_depth,
    )
    .map_err(|error| output(0, error))?;
    reserve(
        1,
        0,
        count,
        bounds.expanded.max_nodes,
        bounds.expanded.max_depth,
    )
    .map_err(|error| output(0, error))?;
    let native = |iteration, phase, error| {
        repeat(RepeatSourceError::Native {
            iteration,
            phase,
            span: body.span,
            error,
        })
    };
    let branches = adapter
        .lower_sequence(body)
        .map_err(|error| native(1, RepeatSourcePhase::Lower, error))?;
    let width = branches.len();
    let expanded_width = width
        .checked_mul(count)
        .filter(|&width| width <= limits.max_branches)
        .ok_or_else(|| width_error(1))?;
    if width == 0 {
        return Err(width_error(1));
    }
    let original = Node::Group {
        group_kind: GroupKind::Sequence,
        branches,
    };
    let expected = shape(&original, bounds.source).map_err(|error| output(1, error))?;
    reserve(
        expected.nodes - 1,
        expected.depth - 1,
        count,
        bounds.source.max_lowered_nodes,
        bounds.source.max_lowered_depth,
    )
    .map_err(|error| output(1, error))?;
    let Node::Group { branches, .. } = &original else {
        return Err(width_error(1));
    };
    let prefix = format!("iteration_{count}");
    for (member, branch) in branches.iter().enumerate() {
        if !valid_argument_name(&branch.name)
            || prefix.len() + 2 + branch.name.len() > MAX_LOCAL_NAME_BYTES
            || branches[..member]
                .iter()
                .any(|previous| previous.name == branch.name)
        {
            return Err(RepeatSequenceSourceError::Name {
                iteration: 1,
                member,
                span: body.span,
            });
        }
        if !atomic_candidate(&branch.value) {
            return Err(RepeatSequenceSourceError::Candidate {
                iteration: 1,
                member,
                span: body.span,
            });
        }
    }
    let cost = measure_source_cost(&original, bounds.expanded, |effect| {
        adapter.host_cost(effect)
    })
    .map_err(|error| {
        repeat(RepeatSourceError::Cost {
            iteration: 1,
            error,
        })
    })?;
    reserve(
        cost.nodes - 1,
        cost.depth - 1,
        count,
        bounds.expanded.max_nodes,
        bounds.expanded.max_depth,
    )
    .map_err(|error| output(1, error))?;
    let mut instances = Vec::with_capacity(count);
    instances.push(original);
    for iteration in 2..=count {
        let Node::Group {
            branches: original, ..
        } = &instances[0]
        else {
            return Err(width_error(1));
        };
        let branches = adapter
            .materialize_sequence(iteration, body, original)
            .map_err(|error| native(iteration, RepeatSourcePhase::Materialize, error))?;
        if branches.len() != width {
            return Err(width_error(iteration));
        }
        let value = Node::Group {
            group_kind: GroupKind::Sequence,
            branches,
        };
        let actual = shape(&value, bounds.source).map_err(|error| output(iteration, error))?;
        if actual != expected {
            return Err(repeat(RepeatSourceError::ShapeChanged {
                iteration,
                expected,
                actual,
            }));
        }
        let Node::Group { branches, .. } = &value else {
            return Err(width_error(iteration));
        };
        for (member, (branch, original)) in branches.iter().zip(original).enumerate() {
            if branch.name != original.name {
                return Err(RepeatSequenceSourceError::NamesChanged {
                    iteration,
                    member,
                    span: body.span,
                });
            }
            if !atomic_candidate(&branch.value) {
                return Err(RepeatSequenceSourceError::Candidate {
                    iteration,
                    member,
                    span: body.span,
                });
            }
        }
        let actual =
            measure_source_cost(&value, bounds.expanded, |effect| adapter.host_cost(effect))
                .map_err(|error| repeat(RepeatSourceError::Cost { iteration, error }))?;
        if actual != cost {
            return Err(repeat(RepeatSourceError::CostChanged {
                iteration,
                expected: cost,
                actual,
            }));
        }
        instances.push(value);
    }
    let Node::Group {
        branches: original, ..
    } = &instances[0]
    else {
        return Err(width_error(1));
    };
    for (index, instance) in instances.iter().enumerate() {
        let Node::Group { branches, .. } = instance else {
            return Err(width_error(index + 1));
        };
        for (member, (candidate, original)) in branches.iter().zip(original).enumerate() {
            adapter
                .admit_member(index + 1, member, body, original, candidate)
                .map_err(|error| RepeatSequenceSourceError::Admission {
                    iteration: index + 1,
                    member,
                    span: body.span,
                    error,
                })?;
        }
    }
    let mut output = Vec::with_capacity(expanded_width);
    for (index, instance) in instances.into_iter().enumerate() {
        let Node::Group { branches, .. } = instance else {
            return Err(width_error(index + 1));
        };
        for mut branch in branches {
            branch.name = format!("iteration_{}__{}", index + 1, branch.name);
            output.push(branch);
        }
    }
    Ok(Node::Group {
        group_kind: GroupKind::Sequence,
        branches: output,
    })
}
