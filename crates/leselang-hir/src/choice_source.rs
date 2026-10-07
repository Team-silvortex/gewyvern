//! Native-typed source choice construction, not full compilation or authority.

use std::fmt;

use leselang_runtime_core::{ScalarValue, StructureBudget, StructureError};
use leselang_syntax::{Expression, Span};

use crate::binding_source::physical;
use crate::ir::Computation;
use crate::pure_typing::{PureTypeError, preflight_with_budget};
use crate::source_call::{SourceCallError, SourceCallLimits, preflight, span, valid_limits};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChoiceSourcePhase {
    When,
    Condition,
    Then,
    Otherwise,
    Compare,
}

pub enum ChoiceSourceError<Error> {
    Source(SourceCallError<Error>),
    Names {
        span: Span,
    },
    Condition {
        span: Span,
    },
    BranchTypes {
        span: Span,
    },
    ProducedWhen {
        span: Span,
        error: PureTypeError,
    },
    Output {
        phase: ChoiceSourcePhase,
        error: StructureError,
    },
    Native {
        phase: ChoiceSourcePhase,
        span: Span,
        error: Error,
    },
}
impl<Error> fmt::Display for ChoiceSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Source(_) => "choice source shape or limits are invalid",
            Self::Names { .. } => "source choice requires exactly when, then and otherwise",
            Self::Condition { .. } => "source choice condition must be pure and boolean",
            Self::BranchTypes { .. } => "source choice branches have different native types",
            Self::ProducedWhen { .. } => "lowered choice condition is not bounded pure IR",
            Self::Output { .. } => "source choice output exceeds physical limits",
            Self::Native { .. } => "native source choice preparation failed",
        })
    }
}
impl<Error> fmt::Debug for ChoiceSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for ChoiceSourceError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type ChoiceOperand<Node, Type, Error> = Result<(Node, Type), Error>;
pub type ChoiceSourceResult<Node, Type, Error> = Result<(Node, Type), ChoiceSourceError<Error>>;

fn operands<Error>(expression: &Expression) -> Result<[&Expression; 3], ChoiceSourceError<Error>> {
    let at = span(expression);
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(ChoiceSourceError::Names { span: at });
    };
    let expected = ["when", "then", "otherwise"];
    if callee != "choose"
        || arguments.len() != expected.len()
        || expected.iter().any(|name| {
            arguments
                .iter()
                .filter(|argument| argument.name == *name)
                .count()
                != 1
        })
    {
        return Err(ChoiceSourceError::Names { span: at });
    }
    let get = |name| {
        arguments
            .iter()
            .find(|argument| argument.name == name)
            .map(|argument| &argument.value)
            .ok_or(ChoiceSourceError::Names { span: at })
    };
    Ok([get("when")?, get("then")?, get("otherwise")?])
}

fn cold_names<Error>(expression: &Expression) -> Result<(), ChoiceSourceError<Error>> {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        if let Expression::Call {
            callee, arguments, ..
        } = node
        {
            if callee == "choose" {
                operands::<Error>(node)?;
            }
            pending.extend(arguments.iter().rev().map(|argument| &argument.value));
        }
    }
    Ok(())
}

/// Lower one native-typed Choose without evaluating or selecting either branch.
/// Whole cold AST names/text/physical bounds and every nested Choose's exact
/// named signature precede all hooks. Ceilings: 16,384 source/output nodes, depth
/// 64 and 64 call operands. A Choose needs at least four output nodes and depth
/// one, checked before child lowering. Unknown extensions, nested bind/loop/fold
/// lexical/type policy, spans and hidden source expansion remain adapter-owned.
///
/// Children run once in semantic When/Then/Otherwise order, irrespective of named
/// submission order or literal truth. One output meter charges the new root and
/// each original child at shifted depth one; future child roots are reserved.
/// When's complete cold IR must pass shared bounded pure preflight before the
/// boolean hook. A literal When must independently have boolean kind. The explicit
/// fallible boolean hook corroborates full lexical/type observations for that
/// original IR; structural purity alone does not prove boolean typing. Branch IR
/// is physically bounded, not independently type/schema/opaque-graph accepted.
///
/// Only after both cold branches fit does the explicit fallible comparator run
/// once on their original borrowed type observations. It must enforce the host's
/// same declared type/interface policy, not union exports, cast or grant authority.
/// Exact operation identity, closed group exports, schema versions/grants, actual
/// reply domains and complete cold inference remain separate native policy gates.
/// A matching result tag is not an accepted native schema or receipt.
///
/// No native PartialEq/Clone/Debug/serde/Send bound. Original nested Boxes/vector buffers
/// and native slots move unchanged into Choose; the exact Then observation moves
/// out, When/Otherwise observations are released. Construction adds only the root
/// operand Boxes; preflight uses bounded scratch frontiers. Failure/unwind drops
/// consumed parts once, stops later hooks and
/// yields no partial output, retry, dispatch or native side-effect rollback. Errors
/// keep original native phase/span/payload for matching only, not formatting or
/// private source chains. Native work, allocation, interior mutation, Drop and
/// unwind are trusted, not fuel-bounded or sandboxed. Bound ingress before AST/IR
/// construction, guard native lexical frames and revalidate live policy before
/// use. Physical costs do not refund folded/native source weights or reservations.
/// This is a choice constructor, not a complete independent source compiler,
/// interpreter, captured reply, registry or suspension/dispatch lifecycle.
pub fn lower_choice_source<'source, Field, Operation, HostEffect, IrResult, Type, Error>(
    expression: &'source Expression,
    limits: SourceCallLimits,
    mut child: impl FnMut(
        ChoiceSourcePhase,
        &'source Expression,
    )
        -> ChoiceOperand<Node<Field, Operation, HostEffect, IrResult>, Type, Error>,
    boolean: impl FnOnce(&Node<Field, Operation, HostEffect, IrResult>, &Type) -> Result<bool, Error>,
    same: impl FnOnce(&Type, &Type) -> Result<bool, Error>,
) -> ChoiceSourceResult<Node<Field, Operation, HostEffect, IrResult>, Type, Error> {
    if !valid_limits(limits) {
        return Err(ChoiceSourceError::Source(SourceCallError::InvalidLimits));
    }
    preflight(expression, limits).map_err(ChoiceSourceError::Source)?;
    cold_names(expression)?;
    let [when_source, then_source, otherwise_source] = operands(expression)?;
    let mut budget = StructureBudget::new(limits.max_lowered_nodes, limits.max_lowered_depth);
    let output_error = |phase, error| ChoiceSourceError::Output { phase, error };
    let native_error = |phase, source: &Expression, error| ChoiceSourceError::Native {
        phase,
        span: span(source),
        error,
    };
    budget
        .visit(0, 0, 0)
        .map_err(|error| output_error(ChoiceSourcePhase::When, error))?;
    if limits.max_lowered_depth == 0 {
        return Err(output_error(
            ChoiceSourcePhase::When,
            StructureError::DepthLimit,
        ));
    }
    budget
        .check_pending(0, 3)
        .map_err(|error| output_error(ChoiceSourcePhase::When, error))?;
    let (when, when_type) = child(ChoiceSourcePhase::When, when_source)
        .map_err(|error| native_error(ChoiceSourcePhase::When, when_source, error))?;
    preflight_with_budget(&when, 1, &mut budget).map_err(|error| {
        ChoiceSourceError::ProducedWhen {
            span: span(when_source),
            error,
        }
    })?;
    budget
        .check_pending(0, 2)
        .map_err(|error| output_error(ChoiceSourcePhase::When, error))?;
    if matches!(&when, Computation::Literal { value } if !matches!(value, ScalarValue::Boolean(_)))
    {
        return Err(ChoiceSourceError::Condition {
            span: span(when_source),
        });
    }
    if !boolean(&when, &when_type)
        .map_err(|error| native_error(ChoiceSourcePhase::Condition, when_source, error))?
    {
        return Err(ChoiceSourceError::Condition {
            span: span(when_source),
        });
    }
    drop(when_type);
    let (then, then_type) = child(ChoiceSourcePhase::Then, then_source)
        .map_err(|error| native_error(ChoiceSourcePhase::Then, then_source, error))?;
    physical(&then, 1, &mut budget)
        .map_err(|error| output_error(ChoiceSourcePhase::Then, error))?;
    budget
        .check_pending(0, 1)
        .map_err(|error| output_error(ChoiceSourcePhase::Then, error))?;
    let (otherwise, otherwise_type) = child(ChoiceSourcePhase::Otherwise, otherwise_source)
        .map_err(|error| native_error(ChoiceSourcePhase::Otherwise, otherwise_source, error))?;
    physical(&otherwise, 1, &mut budget)
        .map_err(|error| output_error(ChoiceSourcePhase::Otherwise, error))?;
    if !same(&then_type, &otherwise_type)
        .map_err(|error| native_error(ChoiceSourcePhase::Compare, expression, error))?
    {
        return Err(ChoiceSourceError::BranchTypes {
            span: span(expression),
        });
    }
    Ok((
        Computation::Choose {
            when: Box::new(when),
            then: Box::new(then),
            otherwise: Box::new(otherwise),
        },
        then_type,
    ))
}
