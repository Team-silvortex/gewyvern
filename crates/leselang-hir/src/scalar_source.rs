//! Shared literal/operator source lowering with explicit native extension boundaries.

use std::fmt;

use leselang_runtime_core::{
    BinaryOperator, OptionalStringValue, ScalarType, ScalarValue, StructureBudget, StructureError,
    UnaryOperator,
};
use leselang_syntax::{Expression, NamedArgument, Span};

use crate::ir::Computation;
use crate::pure_typing::{PureTypeError, preflight_with_budget};
use crate::source_call::{SourceCallError, SourceCallLimits, preflight, span, valid_limits};

pub enum ScalarSourceError<Error> {
    Source(SourceCallError<Error>),
    Names {
        span: Span,
        expected: &'static [&'static str],
    },
    LiteralLimit {
        span: Span,
    },
    NonScalar {
        span: Span,
    },
    Binary {
        span: Span,
        operator: BinaryOperator,
        left: ScalarType,
        right: ScalarType,
    },
    Unary {
        span: Span,
        operator: UnaryOperator,
    },
    InconsistentLiteral {
        span: Span,
    },
    Native {
        span: Span,
        error: Error,
    },
    Generation {
        span: Span,
        error: StructureError,
    },
    Produced {
        span: Span,
        error: PureTypeError,
    },
}

impl<Error> fmt::Display for ScalarSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Source(_) => "scalar source shape is invalid",
            Self::Names { .. } => "scalar source named operands are invalid",
            Self::LiteralLimit { .. } => "scalar source literal exceeds language bounds",
            Self::NonScalar { .. } => "scalar source operand must be pure and scalar",
            Self::Binary { .. } => "binary source operand types are incompatible",
            Self::Unary { .. } => "unary source operand type is incompatible",
            Self::InconsistentLiteral { .. } => "native source facts disagree with a literal",
            Self::Native { .. } => "native source extension lowering failed",
            Self::Generation { .. } => "scalar source generation exceeds its limits",
            Self::Produced { .. } => "native source extension IR is invalid",
        })
    }
}
impl<Error> fmt::Debug for ScalarSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for ScalarSourceError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type ScalarSourceResult<Node, Error> = Result<(Node, ScalarType), ScalarSourceError<Error>>;

/// Unchecked native type/purity observations for the reference adapter only.
/// The public bounded entry establishes structural purity itself. No native
/// Clone/Debug/serde/Send requirement is imposed on IR slots or errors.
pub(crate) struct Operand<Node> {
    pub value: Node,
    pub scalar_type: Option<ScalarType>,
    pub pure: bool,
}

type OperandResult<Node, Error> = Result<Operand<Node>, ScalarSourceError<Error>>;

fn check_named<Error>(
    arguments: &[NamedArgument],
    expected: &'static [&'static str],
    at: Span,
) -> Result<(), ScalarSourceError<Error>> {
    if arguments.len() != expected.len()
        || expected.iter().any(|name| {
            arguments
                .iter()
                .filter(|argument| argument.name == *name)
                .count()
                != 1
        })
    {
        return Err(ScalarSourceError::Names { span: at, expected });
    }
    Ok(())
}

fn named<'source, Error>(
    arguments: &'source [NamedArgument],
    expected: &'static [&'static str],
    at: Span,
) -> Result<Vec<&'source Expression>, ScalarSourceError<Error>> {
    check_named(arguments, expected, at)?;
    Ok(expected
        .iter()
        .filter_map(|name| {
            arguments
                .iter()
                .find(|argument| argument.name == *name)
                .map(|argument| &argument.value)
        })
        .collect())
}

pub(crate) fn primitive(expression: &Expression) -> bool {
    match expression {
        Expression::Call { callee, .. } => {
            BinaryOperator::parse(callee).is_some() || UnaryOperator::parse(callee).is_some()
        }
        Expression::Reference { .. } => false,
        _ => true,
    }
}

// Shared single-form construction. Legacy callers retain source accounting,
// lexical/type gates and exact diagnostics; neither child is evaluated here.
pub(crate) fn lower_form<'source, Field, Operation, HostEffect, IrResult, Error>(
    expression: &'source Expression,
    mut child: impl FnMut(
        &'source Expression,
    ) -> Result<
        Operand<Node<Field, Operation, HostEffect, IrResult>>,
        ScalarSourceError<Error>,
    >,
) -> ScalarSourceResult<Node<Field, Operation, HostEffect, IrResult>, Error> {
    let at = span(expression);
    let literal = match expression {
        Expression::Integer { value, .. } => Some(ScalarValue::Integer(*value)),
        Expression::Boolean { value, .. } => Some(ScalarValue::Boolean(*value)),
        Expression::None { .. } => Some(ScalarValue::None),
        Expression::String { value, .. } => {
            if value.len() > leselang_runtime_core::MAX_SCALAR_STRING_BYTES {
                return Err(ScalarSourceError::LiteralLimit { span: at });
            }
            Some(ScalarValue::String(value.clone()))
        }
        _ => None,
    };
    if let Some(value) = literal {
        let ty = value.scalar_type();
        return Ok((Computation::Literal { value }, ty));
    }
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(ScalarSourceError::NonScalar { span: at });
    };
    if let Some(operator) = BinaryOperator::parse(callee) {
        let arguments = named(arguments, &["left", "right"], at)?;
        let left = child(arguments[0])?;
        let right = child(arguments[1])?;
        let scalar = |operand: &Operand<_>, expression| {
            operand
                .scalar_type
                .filter(|_| operand.pure)
                .ok_or(ScalarSourceError::NonScalar {
                    span: span(expression),
                })
        };
        let left_type = scalar(&left, arguments[0])?;
        let right_type = scalar(&right, arguments[1])?;
        let ty = operator
            .result_type(left_type, right_type)
            .ok_or(ScalarSourceError::Binary {
                span: at,
                operator,
                left: left_type,
                right: right_type,
            })?;
        return Ok((
            Computation::Binary {
                operator,
                left: Box::new(left.value),
                right: Box::new(right.value),
            },
            ty,
        ));
    }
    let operator = UnaryOperator::parse(callee).ok_or(ScalarSourceError::NonScalar { span: at })?;
    let arguments = named(arguments, &["value"], at)?;
    let input = child(arguments[0])?;
    let ty = input
        .scalar_type
        .filter(|_| input.pure)
        .and_then(|ty| operator.result_type(ty))
        .ok_or(ScalarSourceError::Unary { span: at, operator })?;
    if operator == UnaryOperator::OptionalString
        && let Computation::Literal { value } = input.value
    {
        let text = match value {
            ScalarValue::String(text) => Some(text),
            ScalarValue::None => None,
            _ => return Err(ScalarSourceError::Unary { span: at, operator }),
        };
        return Ok((
            Computation::Literal {
                value: ScalarValue::OptionalString(OptionalStringValue(text)),
            },
            ty,
        ));
    }
    Ok((
        Computation::Unary {
            operator,
            value: Box::new(input.value),
        },
        ty,
    ))
}

fn preflight_names<Error>(expression: &Expression) -> Result<(), ScalarSourceError<Error>> {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        if let Expression::Call {
            callee, arguments, ..
        } = node
        {
            if BinaryOperator::parse(callee).is_some() {
                check_named::<Error>(arguments, &["left", "right"], span(node))?;
            } else if UnaryOperator::parse(callee).is_some() {
                check_named::<Error>(arguments, &["value"], span(node))?;
            }
            pending.extend(arguments.iter().rev().map(|argument| &argument.value));
        }
    }
    Ok(())
}

fn build<'source, Field, Operation, HostEffect, IrResult, Error>(
    expression: &'source Expression,
    depth: usize,
    max_depth: usize,
    budget: &mut StructureBudget,
    extension: &mut impl FnMut(
        &'source Expression,
    ) -> Result<
        (
            Node<Field, Operation, HostEffect, IrResult>,
            Option<ScalarType>,
        ),
        Error,
    >,
) -> OperandResult<Node<Field, Operation, HostEffect, IrResult>, Error> {
    let at = span(expression);
    if depth > max_depth {
        return Err(ScalarSourceError::Generation {
            span: at,
            error: StructureError::DepthLimit,
        });
    }
    if primitive(expression) {
        budget
            .visit(depth, 0, 0)
            .map_err(|error| ScalarSourceError::Generation { span: at, error })?;
        let (value, ty) = lower_form(expression, |child| {
            build(child, depth + 1, max_depth, budget, extension)
        })?;
        return Ok(Operand {
            value,
            scalar_type: Some(ty),
            pure: true,
        });
    }
    budget
        .check_pending(0, 1)
        .map_err(|error| ScalarSourceError::Generation { span: at, error })?;
    let (value, scalar_type) =
        extension(expression).map_err(|error| ScalarSourceError::Native { span: at, error })?;
    preflight_with_budget(&value, depth, budget)
        .map_err(|error| ScalarSourceError::Produced { span: at, error })?;
    if let Computation::Literal { value } = &value
        && scalar_type != Some(value.scalar_type())
    {
        return Err(ScalarSourceError::InconsistentLiteral { span: at });
    }
    Ok(Operand {
        value,
        scalar_type,
        pure: true,
    })
}

/// Lower literals and every closed scalar operator into the original generic IR.
/// Whole cold AST bounds and all operator named signatures precede any extension.
/// Other forms, including locals/fields/control/helpers, require an explicit
/// trusted callback returning bounded pure IR and inferred type observations.
/// Extensions are not automatically parsed, typed, dispatched or authorized.
///
/// Generation shares one aggregate budget, counting constructors before folding;
/// optional literal text moves unchanged, with no refund or second buffer copy.
/// Both short-circuit operands are lowered/typed; no arithmetic or parsing executes.
/// Extension errors/unwind release partial IR without retries or rollback. Native
/// callbacks/metadata/drop cannot be preempted. Hosts own ingress, source/span
/// authenticity, lexical/complete cold typing, native expansion, work and redaction.
/// Output is an observed scalar expression, not a program, receipt, authority,
/// continuation, helper/control lowerer or security sandbox. All native IR slots
/// and errors can be non-Clone, non-Debug, non-serde and GUI-local.
pub fn lower_scalar_source<'source, Field, Operation, HostEffect, IrResult, Error>(
    expression: &'source Expression,
    limits: SourceCallLimits,
    mut extension: impl FnMut(
        &'source Expression,
    ) -> Result<
        (
            Node<Field, Operation, HostEffect, IrResult>,
            Option<ScalarType>,
        ),
        Error,
    >,
) -> ScalarSourceResult<Node<Field, Operation, HostEffect, IrResult>, Error> {
    if !valid_limits(limits) {
        return Err(ScalarSourceError::Source(SourceCallError::InvalidLimits));
    }
    preflight(expression, limits).map_err(ScalarSourceError::Source)?;
    preflight_names(expression)?;
    let mut budget = StructureBudget::new(limits.max_lowered_nodes, limits.max_lowered_depth);
    let output = build(
        expression,
        0,
        limits.max_lowered_depth,
        &mut budget,
        &mut extension,
    )?;
    let ty = output.scalar_type.ok_or(ScalarSourceError::NonScalar {
        span: span(expression),
    })?;
    Ok((output.value, ty))
}
