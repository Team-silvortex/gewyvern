//! Flat native group source assembly, not a scheduler or complete source compiler.

use std::fmt;

use leselang_runtime_core::{StructureBudget, StructureError};
use leselang_syntax::{Expression, NamedArgument, Span};

use crate::binding_source::physical;
use crate::call_typing::valid_argument_name;
use crate::group_exports::{MAX_GROUP_EXPORT_MEMBERS, atomic_candidate};
use crate::ir::{Computation, ComputedBranch, GroupKind};
use crate::source_call::{SourceCallError, SourceCallLimits, preflight, span, valid_limits};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupSourceLimits {
    pub source: SourceCallLimits,
    pub max_branches: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupSourcePhase {
    Lower,
    Admit,
}

pub enum GroupSourceError<Error> {
    InvalidLimits,
    NotGroup {
        span: Span,
    },
    Arity {
        kind: GroupKind,
        span: Span,
    },
    MemberName {
        kind: GroupKind,
        index: usize,
        span: Span,
    },
    DuplicateMember {
        kind: GroupKind,
        index: usize,
        span: Span,
    },
    NestedMember {
        index: usize,
        span: Span,
    },
    Source(SourceCallError<Error>),
    Output {
        index: Option<usize>,
        error: StructureError,
    },
    Candidate {
        index: usize,
        span: Span,
    },
    Native {
        index: usize,
        phase: GroupSourcePhase,
        span: Span,
        error: Error,
    },
}
impl<Error> fmt::Display for GroupSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "group source limits exceed safety ceilings",
            Self::NotGroup { .. } => "group source requires seq or all",
            Self::Arity { .. } => "group source member count is invalid",
            Self::MemberName { .. } => "group source member name is invalid",
            Self::DuplicateMember { .. } => "group source member names must be unique",
            Self::NestedMember { .. } => {
                "flat group source does not expand nested groups or repeat"
            }
            Self::Source(_) => "group source shape or limits are invalid",
            Self::Output { .. } => "group source output exceeds physical limits",
            Self::Candidate { .. } => {
                "group member must be an atomic candidate after pure preparation"
            }
            Self::Native { .. } => "native group source preparation failed",
        })
    }
}
impl<Error> fmt::Debug for GroupSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for GroupSourceError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
type Branch<Field, Operation, HostEffect, IrResult> =
    ComputedBranch<Node<Field, Operation, HostEffect, IrResult>, IrResult>;
pub type GroupSourceOperand<Node, Result, Error> = std::result::Result<(Node, Result), Error>;
pub type GroupSourceResult<Node, Error> = Result<Node, GroupSourceError<Error>>;

pub(crate) fn header<Error>(
    expression: &Expression,
    maximum: usize,
) -> Result<(GroupKind, &[NamedArgument]), GroupSourceError<Error>> {
    let at = span(expression);
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(GroupSourceError::NotGroup { span: at });
    };
    let kind = match callee.as_str() {
        "seq" => GroupKind::Sequence,
        "all" => GroupKind::Parallel,
        _ => return Err(GroupSourceError::NotGroup { span: at }),
    };
    let minimum = if kind == GroupKind::Sequence { 1 } else { 2 };
    if !(minimum..=maximum).contains(&arguments.len()) {
        return Err(GroupSourceError::Arity { kind, span: at });
    }
    for (index, argument) in arguments.iter().enumerate() {
        if !valid_argument_name(&argument.name) {
            return Err(GroupSourceError::MemberName {
                kind,
                index,
                span: argument.span,
            });
        }
        if arguments[..index]
            .iter()
            .any(|previous| previous.name == argument.name)
        {
            return Err(GroupSourceError::DuplicateMember {
                kind,
                index,
                span: argument.span,
            });
        }
    }
    Ok((kind, arguments))
}

pub(crate) fn cold_headers<Error>(
    expression: &Expression,
    maximum: usize,
) -> Result<(), GroupSourceError<Error>> {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        if let Expression::Call {
            callee, arguments, ..
        } = node
        {
            if matches!(callee.as_str(), "seq" | "all") {
                header::<Error>(node, maximum)?;
            }
            pending.extend(arguments.iter().rev().map(|argument| &argument.value));
        }
    }
    Ok(())
}

/// Construct one flat Seq/All on the original generic IR without dispatching.
/// Explicit ceilings: 16,384 source/output nodes, depth 64, 64 operands/branches.
/// Sequence needs one member; parallel needs two. Zero nodes/branches deny entry;
/// depth zero cannot contain members. All root labels, whole cold AST bounds and
/// nested Seq/All header shapes precede lowering. Direct nested seq/all/repeat
/// members are rejected; flattening, repeat factories and helper expansion remain
/// separate adapter policy. No native result Clone/PartialEq/Debug/serde/Send bound.
///
/// Lower visits original borrowed NamedArguments once in declaration order and
/// returns owned IR plus its original native result observation. One physical
/// output meter charges the group root and every shifted child, reserving future
/// member roots before further lowering. Complete bounded candidate-shape checks
/// precede every admission hook: Host, Call with pure operands, or Bind/Choose
/// routes with pure preparation and atomic tails. Opaque Host graphs are not
/// inspected; boolean/lexical typing, generated language data, exact uniform atomic
/// operation identity, result declarations, versions/grants and native graph costs
/// must be corroborated by the mandatory fallible admission hook or child compiler.
/// Matching result tags alone never establish these properties or union exports.
///
/// Admission visits original source/IR/result pairs once in declaration order only
/// after every cold output fits. Observers must not dispatch effects. All branches
/// retain their own result observation; no group result type or receipt is inferred.
/// Names are materialized once; native slots, nested Boxes and child vector buffers
/// move unchanged into one Group. Pure-prefix values and earlier sequence results
/// are not implicitly installed: adapters guard their own lexical/type frames.
///
/// Failure/unwind stops later hooks, drops consumed parts once and yields no partial
/// group, retry, rollback or source-cost/fuel refund. Errors retain phase/index/span
/// and native payload for matching, never formatting or private source chains.
/// Native work, mutation, allocation, Drop and unwind are trusted, not sandboxed or
/// preempted. Bound ingress before AST/IR allocation and revalidate live policy
/// before use. This is flat source assembly, not complete generic child compilation,
/// source-cost measurement, scheduling, reply acceptance or suspension ownership.
pub fn lower_flat_group_source<'source, Field, Operation, HostEffect, IrResult, Error>(
    expression: &'source Expression,
    limits: GroupSourceLimits,
    mut lower: impl FnMut(
        usize,
        &'source NamedArgument,
    ) -> GroupSourceOperand<
        Node<Field, Operation, HostEffect, IrResult>,
        IrResult,
        Error,
    >,
    mut admit: impl FnMut(
        usize,
        &'source NamedArgument,
        &Branch<Field, Operation, HostEffect, IrResult>,
    ) -> Result<(), Error>,
) -> GroupSourceResult<Node<Field, Operation, HostEffect, IrResult>, Error> {
    if !valid_limits(limits.source) || limits.max_branches > MAX_GROUP_EXPORT_MEMBERS {
        return Err(GroupSourceError::InvalidLimits);
    }
    let (kind, arguments) = header(expression, limits.max_branches)?;
    preflight(expression, limits.source).map_err(GroupSourceError::Source)?;
    cold_headers(expression, limits.max_branches)?;
    for (index, argument) in arguments.iter().enumerate() {
        if matches!(&argument.value, Expression::Call { callee, .. } if matches!(callee.as_str(), "seq" | "all" | "repeat"))
        {
            return Err(GroupSourceError::NestedMember {
                index,
                span: argument.span,
            });
        }
    }
    let mut budget = StructureBudget::new(
        limits.source.max_lowered_nodes,
        limits.source.max_lowered_depth,
    );
    let root_error = |error| GroupSourceError::Output { index: None, error };
    budget.visit(0, 0, 0).map_err(root_error)?;
    if limits.source.max_lowered_depth == 0 {
        return Err(root_error(StructureError::DepthLimit));
    }
    budget
        .check_pending(0, arguments.len())
        .map_err(root_error)?;
    let mut branches = Vec::with_capacity(arguments.len());
    for (index, argument) in arguments.iter().enumerate() {
        let (value, result_type) =
            lower(index, argument).map_err(|error| GroupSourceError::Native {
                index,
                phase: GroupSourcePhase::Lower,
                span: argument.span,
                error,
            })?;
        let output_error = |error| GroupSourceError::Output {
            index: Some(index),
            error,
        };
        physical(&value, 1, &mut budget).map_err(output_error)?;
        budget
            .check_pending(0, arguments.len() - index - 1)
            .map_err(output_error)?;
        if !atomic_candidate(&value) {
            return Err(GroupSourceError::Candidate {
                index,
                span: argument.span,
            });
        }
        branches.push(ComputedBranch {
            name: argument.name.to_owned(),
            value,
            result_type,
        });
    }
    for (index, (argument, branch)) in arguments.iter().zip(&branches).enumerate() {
        admit(index, argument, branch).map_err(|error| GroupSourceError::Native {
            index,
            phase: GroupSourcePhase::Admit,
            span: argument.span,
            error,
        })?;
    }
    Ok(Computation::Group {
        group_kind: kind,
        branches,
    })
}
