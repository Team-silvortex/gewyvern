//! Owned sequential source composition without an opaque-effect round trip.

use std::fmt;

use leselang_runtime_core::{StructureBudget, StructureError};
use leselang_syntax::{Expression, NamedArgument, Span};

use crate::binding_source::physical;
use crate::call_typing::valid_argument_name;
use crate::group_exports::{MAX_GROUP_EXPORT_MEMBERS, atomic_candidate};
use crate::group_source::{
    GroupSourceError, GroupSourceLimits, GroupSourcePhase, cold_headers, header,
};
use crate::ir::{Computation, ComputedBranch, GroupKind};
use crate::pure_typing::MAX_LOCAL_NAME_BYTES;
use crate::source_call::{preflight, span, valid_limits};

/// A child compiler explicitly distinguishes one atomic result from an already
/// flattened sequence. No synthetic result declaration is inferred for a group.
pub enum SequenceSourceMember<Node, Result> {
    Atomic {
        value: Node,
        result_type: Result,
    },
    Sequence {
        branches: Vec<ComputedBranch<Node, Result>>,
    },
}

pub enum SequenceSourceError<Error> {
    Header(GroupSourceError<Error>),
    NotSequence {
        span: Span,
    },
    ParallelMember {
        index: usize,
        span: Span,
    },
    MemberKind {
        index: usize,
        span: Span,
    },
    Width {
        index: usize,
        span: Span,
    },
    Name {
        index: usize,
        member: usize,
        span: Span,
    },
    Collision {
        index: usize,
        member: usize,
        span: Span,
    },
    Candidate {
        index: usize,
        member: usize,
        span: Span,
    },
    Output {
        index: Option<usize>,
        error: StructureError,
    },
    Native {
        index: usize,
        member: Option<usize>,
        phase: GroupSourcePhase,
        span: Span,
        error: Error,
    },
}
impl<Error> fmt::Display for SequenceSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Header(_) => "sequence source header or cold shape is invalid",
            Self::NotSequence { .. } => "sequence source requires seq",
            Self::ParallelMember { .. } => "parallel groups cannot be sequential members",
            Self::MemberKind { .. } => "native sequence member kind disagrees with source",
            Self::Width { .. } => "expanded sequence member count is invalid",
            Self::Name { .. } => "expanded sequence name is not a bounded identifier",
            Self::Collision { .. } => "expanded sequence names must be unique",
            Self::Candidate { .. } => "sequence member is not a prepared atomic candidate",
            Self::Output { .. } => "sequence output exceeds physical limits",
            Self::Native { .. } => "native sequence source preparation failed",
        })
    }
}
impl<Error> fmt::Debug for SequenceSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for SequenceSourceError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
type Branch<Field, Operation, HostEffect, IrResult> =
    ComputedBranch<Node<Field, Operation, HostEffect, IrResult>, IrResult>;
pub type SequenceSourceResult<Node, Error> = Result<Node, SequenceSourceError<Error>>;
pub type SequenceSourceOperand<Node, Result, Error> =
    std::result::Result<SequenceSourceMember<Node, Result>, Error>;

/// Compose Seq from owned atomic members and explicitly lowered direct Seq/Repeat
/// children. The child compiler owns recursion and repeat materialization; nested
/// Parallel is not flattened or silently serialized. Root and all cold Seq/All
/// headers plus complete AST/name/literal bounds precede every native callback.
/// Repeat headers, lexical scopes and helper expansion remain child-compiler policy.
/// Inclusive ceilings: 16,384 source/output nodes, depth 64, 64 operands/branches.
/// Zero nodes/branches denies entry; zero output depth cannot contain members.
///
/// Lower visits original NamedArguments once in declaration order. Direct Seq or
/// Repeat must return a nonempty, already-flat Sequence; other members must return
/// Atomic. Child result declarations and all four native IR slots move unchanged,
/// without Clone/PartialEq/Debug/serde/Send bounds. Only language labels are joined
/// as `parent__child`, after checking their final length (64 bytes) before allocation.
/// A shared shifted output meter reserves future member roots; expanded branch
/// count, names, collisions and every atomic candidate are checked before admission.
/// Consumed child vector storage is not retained; child Boxes/operand buffers are.
///
/// Mandatory Admit observes final branches once, in flattened declaration order,
/// with the original source index/member index/NamedArgument. It must corroborate
/// exact operations, result declarations, generated payloads, types, closed opaque
/// graphs, version/grant and cost policy. Candidate shape is not authority; opaque
/// Host internals are not inspected. Earlier result rows and locals are not installed.
/// Native callbacks must not dispatch; allocation/work/Drop/unwind are trusted and
/// unmetered. Bound ingress before constructing AST/IR, guard native mutable scopes,
/// and revalidate live policy before use. No source-weight/fuel refund, retries,
/// rollback or partial output on failure/unwind. Native errors remain matchable by
/// index/phase/span/payload but are never formatted or exposed in source chains.
/// This is owned composition, not a complete child compiler, scheduler, interpreter,
/// suspension lifecycle or received-result acceptance protocol.
pub fn lower_sequence_source<'source, Field, Operation, HostEffect, IrResult, Error>(
    expression: &'source Expression,
    limits: GroupSourceLimits,
    mut lower: impl FnMut(
        usize,
        &'source NamedArgument,
    ) -> SequenceSourceOperand<
        Node<Field, Operation, HostEffect, IrResult>,
        IrResult,
        Error,
    >,
    mut admit: impl FnMut(
        usize,
        usize,
        &'source NamedArgument,
        &Branch<Field, Operation, HostEffect, IrResult>,
    ) -> Result<(), Error>,
) -> SequenceSourceResult<Node<Field, Operation, HostEffect, IrResult>, Error> {
    let cold = SequenceSourceError::Header;
    if !valid_limits(limits.source) || limits.max_branches > MAX_GROUP_EXPORT_MEMBERS {
        return Err(cold(GroupSourceError::InvalidLimits));
    }
    let (kind, arguments) = header(expression, limits.max_branches).map_err(cold)?;
    if kind != GroupKind::Sequence {
        return Err(SequenceSourceError::NotSequence {
            span: span(expression),
        });
    }
    preflight(expression, limits.source)
        .map_err(GroupSourceError::Source)
        .map_err(cold)?;
    cold_headers(expression, limits.max_branches).map_err(cold)?;
    for (index, argument) in arguments.iter().enumerate() {
        if matches!(&argument.value, Expression::Call { callee, .. } if callee == "all") {
            return Err(SequenceSourceError::ParallelMember {
                index,
                span: argument.span,
            });
        }
    }
    let mut budget = StructureBudget::new(
        limits.source.max_lowered_nodes,
        limits.source.max_lowered_depth,
    );
    let root_error = |error| SequenceSourceError::Output { index: None, error };
    budget.visit(0, 0, 0).map_err(root_error)?;
    if limits.source.max_lowered_depth == 0 {
        return Err(root_error(StructureError::DepthLimit));
    }
    budget
        .check_pending(0, arguments.len())
        .map_err(root_error)?;
    let mut branches = Vec::new();
    let mut origins = Vec::new();
    for (index, argument) in arguments.iter().enumerate() {
        let nested = matches!(&argument.value, Expression::Call { callee, .. }
            if matches!(callee.as_str(), "seq" | "repeat"));
        let member = lower(index, argument).map_err(|error| SequenceSourceError::Native {
            index,
            member: None,
            phase: GroupSourcePhase::Lower,
            span: argument.span,
            error,
        })?;
        let members = match member {
            SequenceSourceMember::Atomic { value, result_type } if !nested => {
                vec![ComputedBranch {
                    name: argument.name.to_owned(),
                    value,
                    result_type,
                }]
            }
            SequenceSourceMember::Sequence { branches } if nested => branches,
            _ => {
                return Err(SequenceSourceError::MemberKind {
                    index,
                    span: argument.span,
                });
            }
        };
        if members.is_empty()
            || members.len() > limits.max_branches - branches.len()
            || members.len() + branches.len() + arguments.len() - index - 1 > limits.max_branches
        {
            return Err(SequenceSourceError::Width {
                index,
                span: argument.span,
            });
        }
        for (member, mut branch) in members.into_iter().enumerate() {
            let at = argument.span;
            if !valid_argument_name(&branch.name)
                || (nested && argument.name.len() + 2 + branch.name.len() > MAX_LOCAL_NAME_BYTES)
            {
                return Err(SequenceSourceError::Name {
                    index,
                    member,
                    span: at,
                });
            }
            if nested {
                branch.name = format!("{}__{}", argument.name, branch.name);
            }
            if branches.iter().any(
                |previous: &Branch<Field, Operation, HostEffect, IrResult>| {
                    previous.name == branch.name
                },
            ) {
                return Err(SequenceSourceError::Collision {
                    index,
                    member,
                    span: at,
                });
            }
            physical(&branch.value, 1, &mut budget).map_err(|error| {
                SequenceSourceError::Output {
                    index: Some(index),
                    error,
                }
            })?;
            if !atomic_candidate(&branch.value) {
                return Err(SequenceSourceError::Candidate {
                    index,
                    member,
                    span: at,
                });
            }
            branches.push(branch);
            origins.push((index, member));
        }
        budget
            .check_pending(0, arguments.len() - index - 1)
            .map_err(|error| SequenceSourceError::Output {
                index: Some(index),
                error,
            })?;
    }
    for (branch, &(index, member)) in branches.iter().zip(&origins) {
        let argument = &arguments[index];
        admit(index, member, argument, branch).map_err(|error| SequenceSourceError::Native {
            index,
            member: Some(member),
            phase: GroupSourcePhase::Admit,
            span: argument.span,
            error,
        })?;
    }
    Ok(Computation::Group {
        group_kind: GroupKind::Sequence,
        branches,
    })
}
