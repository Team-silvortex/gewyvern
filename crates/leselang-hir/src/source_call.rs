//! Bounded source-call binding, not a parser, complete lowerer or execution authority.

use std::{borrow::Borrow, fmt};

use leselang_runtime_core::{
    ArgumentTypeError, MAX_SCALAR_STRING_BYTES, NamedArgumentError, OperationCatalog,
    OperationCatalogError, OperationSchema, ScalarArgumentDomain, ScalarArgumentType, ScalarType,
    StructureBudget, StructureError, check_argument_type,
};
use leselang_syntax::{Expression, NamedArgument, Span};

use crate::ir::{Computation, ComputedArgument};
use crate::pure_typing::{
    MAX_LOCAL_NAME_BYTES, MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES, PureTypeError,
    preflight_with_budget,
};

pub const MAX_SOURCE_CALL_ARGUMENTS: usize = 64;
pub const MAX_SOURCE_CALL_NAME_BYTES: usize = 128;

/// Separate inclusive budgets for the physical AST and produced IR. No source
/// expansion, lexical type inference, helper expansion or evaluation is implied.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct SourceCallLimits {
    pub max_source_nodes: usize,
    pub max_source_depth: usize,
    pub max_lowered_nodes: usize,
    pub max_lowered_depth: usize,
    pub max_arguments: usize,
}

pub type SourceSchema<'schema, Key, Domain, Result, Capability> =
    OperationSchema<'schema, Key, &'schema str, Domain, Result, Capability>;

/// Trusted version/grants, not values accepted from source. Native key Borrow,
/// equality and domain methods may run code or unwind; hosts bound that work.
pub struct SourceCallHost<'host, 'schema, Key, Domain, Result, Capability> {
    pub catalog: &'host OperationCatalog<'schema, Key, &'schema str, Domain, Result, Capability>,
    pub version: u32,
    pub granted: &'host [Capability],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SourceShapeError {
    Structure(StructureError),
    ArgumentLimit,
    InvalidName,
    UnboundedText,
}

/// Native errors remain available by matching, never by formatting/source chains.
pub enum SourceCallError<Error> {
    NotCall,
    InvalidLimits,
    ArgumentLimit,
    ParameterLimit,
    Shape {
        span: Span,
        error: SourceShapeError,
    },
    Catalog(OperationCatalogError),
    Names(NamedArgumentError),
    OutputRoot(StructureError),
    Lowering {
        argument_index: usize,
        error: Error,
    },
    Output {
        argument_index: usize,
        error: PureTypeError,
    },
    Argument {
        parameter_index: usize,
        argument_index: usize,
        error: ArgumentTypeError,
    },
}

impl<Error> fmt::Display for SourceCallError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NotCall => "source call lowering requires a call",
            Self::InvalidLimits => "source call limits exceed safety ceilings",
            Self::ArgumentLimit => "source call has too many arguments",
            Self::ParameterLimit => "source schema has too many parameters",
            Self::Shape { .. } => "source call shape is invalid",
            Self::Catalog(_) => "source call schema selection failed",
            Self::Names(_) => "source call named signature is invalid",
            Self::OutputRoot(_) => "lowered source call root exceeds its limits",
            Self::Lowering { .. } => "native source argument lowering failed",
            Self::Output { .. } => "lowered source argument shape is invalid",
            Self::Argument { .. } => "lowered source argument type or domain is invalid",
        })
    }
}
impl<Error> fmt::Debug for SourceCallError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for SourceCallError<Error> {}

/// Declaration order plus original submitted positions. Names borrow the AST;
/// values move without a second IR tree or native Clone/serde/Debug requirements.
pub struct LoweredSourceArgument<'source, Node> {
    pub name: &'source str,
    pub parameter_index: usize,
    pub argument_index: usize,
    pub value: Node,
}

/// Ephemeral source preparation, not a typed program or authentic receipt.
/// The original schema is borrowed. Native opcode mapping, complete cold typing,
/// canonicalization, live authority and actual value checks remain host-owned.
#[must_use = "consume the lowered arguments or deliberately discard them"]
pub struct LoweredSourceCall<'source, 'schema, Key, Domain, Result, Capability, Node> {
    schema: &'schema SourceSchema<'schema, Key, Domain, Result, Capability>,
    arguments: Vec<LoweredSourceArgument<'source, Node>>,
}
impl<'source, 'schema, Key, Domain, Result, Capability, Node>
    LoweredSourceCall<'source, 'schema, Key, Domain, Result, Capability, Node>
{
    pub fn schema(&self) -> &'schema SourceSchema<'schema, Key, Domain, Result, Capability> {
        self.schema
    }

    pub fn arguments(&self) -> &[LoweredSourceArgument<'source, Node>] {
        &self.arguments
    }

    /// Materialize bounded language names once, moving the exact native nodes.
    pub fn into_arguments(self) -> Vec<ComputedArgument<Node>> {
        self.arguments
            .into_iter()
            .map(|argument| ComputedArgument {
                name: argument.name.to_owned(),
                value: argument.value,
            })
            .collect()
    }
}
impl<Key, Domain, Result, Capability, Node> fmt::Debug
    for LoweredSourceCall<'_, '_, Key, Domain, Result, Capability, Node>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoweredSourceCall")
            .field("arguments", &self.arguments.len())
            .finish()
    }
}

pub type SourceCallResult<'source, 'schema, Key, Domain, Result, Capability, Node, Error> =
    std::result::Result<
        LoweredSourceCall<'source, 'schema, Key, Domain, Result, Capability, Node>,
        SourceCallError<Error>,
    >;

fn valid_identifier(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= MAX_LOCAL_NAME_BYTES
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
}
pub(crate) fn span(expression: &Expression) -> Span {
    match expression {
        Expression::Call { span, .. }
        | Expression::String { span, .. }
        | Expression::Integer { span, .. }
        | Expression::Boolean { span, .. }
        | Expression::Reference { span, .. }
        | Expression::None { span } => *span,
    }
}
pub(crate) fn valid_limits(limits: SourceCallLimits) -> bool {
    limits.max_source_nodes <= MAX_TYPE_INFERENCE_NODES
        && limits.max_lowered_nodes <= MAX_TYPE_INFERENCE_NODES
        && limits.max_source_depth <= MAX_TYPE_INFERENCE_DEPTH
        && limits.max_lowered_depth <= MAX_TYPE_INFERENCE_DEPTH
        && limits.max_arguments <= MAX_SOURCE_CALL_ARGUMENTS
}

// Whole physical source, including cold operands, before native metadata queries.
pub(crate) fn preflight<Error>(
    expression: &Expression,
    limits: SourceCallLimits,
) -> Result<(), SourceCallError<Error>> {
    let mut budget = StructureBudget::new(limits.max_source_nodes, limits.max_source_depth);
    let mut pending = vec![(expression, 0)];
    while let Some((node, depth)) = pending.pop() {
        let fail = |error| SourceCallError::Shape {
            span: span(node),
            error,
        };
        budget
            .visit(depth, 0, 0)
            .map_err(|error| fail(SourceShapeError::Structure(error)))?;
        match node {
            Expression::Call {
                callee, arguments, ..
            } => {
                if callee.len() > MAX_SOURCE_CALL_NAME_BYTES
                    || !callee.split('.').all(valid_identifier)
                {
                    return Err(fail(SourceShapeError::InvalidName));
                }
                if arguments.len() > limits.max_arguments {
                    return Err(fail(SourceShapeError::ArgumentLimit));
                }
                for argument in arguments {
                    if !valid_identifier(&argument.name) {
                        return Err(SourceCallError::Shape {
                            span: argument.span,
                            error: SourceShapeError::InvalidName,
                        });
                    }
                }
                budget
                    .check_pending(pending.len(), arguments.len())
                    .map_err(|error| fail(SourceShapeError::Structure(error)))?;
                pending.extend(
                    arguments
                        .iter()
                        .rev()
                        .map(|argument| (&argument.value, depth + 1)),
                );
            }
            Expression::Reference { name, .. } if !valid_identifier(name) => {
                return Err(fail(SourceShapeError::InvalidName));
            }
            Expression::String { value, .. } if value.len() > MAX_SCALAR_STRING_BYTES => {
                return Err(fail(SourceShapeError::UnboundedText));
            }
            _ => {}
        }
    }
    Ok(())
}

/// Bind an atomic source call against the exact original native catalog row.
/// All physical source checks precede native lookup; exact version and grants,
/// selected signature and all names precede operand lowering/domain callbacks.
/// Physical AST bounds do not prove source/span authenticity; retain the parser
/// and validated SyntaxTree decoding boundary. Native generated-IR preflight
/// establishes physical purity once, not lexical binding or complete type safety.
/// The callback receives the original borrowed AST argument, in declaration order,
/// and returns a native IR node plus trusted inferred scalar type (not inference
/// performed here). Produced pure IR shares one physical budget across arguments.
/// No evaluation, helper expansion, dispatch, fuel, persistence or recovery occurs.
/// Native callback/equality/drop unwind propagates, releasing partial output; native
/// side effects and caller-owned type-scope/accounting are not rolled back.
pub fn lower_source_call<
    'source,
    'schema,
    Key,
    Domain,
    Result,
    Capability,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Error,
>(
    expression: &'source Expression,
    host: &SourceCallHost<'_, 'schema, Key, Domain, Result, Capability>,
    limits: SourceCallLimits,
    lower: impl FnMut(
        &'source NamedArgument,
    ) -> std::result::Result<
        (
            Computation<Field, Operation, HostEffect, IrResult>,
            Option<ScalarType>,
        ),
        Error,
    >,
) -> SourceCallResult<
    'source,
    'schema,
    Key,
    Domain,
    Result,
    Capability,
    Computation<Field, Operation, HostEffect, IrResult>,
    Error,
>
where
    Key: Borrow<str>,
    Capability: PartialEq,
    Domain: ScalarArgumentDomain,
{
    if !valid_limits(limits) {
        return Err(SourceCallError::InvalidLimits);
    }
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(SourceCallError::NotCall);
    };
    preflight(expression, limits)?;
    let schema = host
        .catalog
        .authorize(callee.as_str(), host.version, host.granted)
        .map_err(SourceCallError::Catalog)?;
    lower_preflighted_arguments(arguments, schema, limits, lower)
}

// Reference adapter only: it preserves legacy source diagnostics/expansion and
// has already bounded the source in its parser/lowering path. It shares binding,
// output-shape checks and native domains without changing source-error precedence.
pub(crate) fn lower_preflighted_arguments<
    'source,
    'schema,
    Key,
    Domain,
    Result,
    Capability,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Error,
>(
    arguments: &'source [NamedArgument],
    schema: &'schema SourceSchema<'schema, Key, Domain, Result, Capability>,
    limits: SourceCallLimits,
    mut lower: impl FnMut(
        &'source NamedArgument,
    ) -> std::result::Result<
        (
            Computation<Field, Operation, HostEffect, IrResult>,
            Option<ScalarType>,
        ),
        Error,
    >,
) -> SourceCallResult<
    'source,
    'schema,
    Key,
    Domain,
    Result,
    Capability,
    Computation<Field, Operation, HostEffect, IrResult>,
    Error,
>
where
    Domain: ScalarArgumentDomain,
{
    if !valid_limits(limits) {
        return Err(SourceCallError::InvalidLimits);
    }
    if schema.parameters.len() > limits.max_arguments {
        return Err(SourceCallError::ParameterLimit);
    }
    if arguments.len() > limits.max_arguments || arguments.len() > schema.parameters.len() {
        return Err(SourceCallError::ArgumentLimit);
    }
    let names = arguments
        .iter()
        .map(|argument| argument.name.as_str())
        .collect::<Vec<_>>();
    let bound = schema
        .bind_arguments(&names)
        .map_err(SourceCallError::Names)?;
    let mut budget = StructureBudget::new(limits.max_lowered_nodes, limits.max_lowered_depth);
    budget.visit(0, 0, 0).map_err(SourceCallError::OutputRoot)?;
    let mut lowered = Vec::with_capacity(arguments.len());
    for (parameter, argument_index) in bound.iter() {
        let argument = &arguments[argument_index];
        let parameter_index = schema
            .parameters
            .iter()
            .position(|candidate| std::ptr::eq(candidate, parameter))
            .ok_or(SourceCallError::Names(
                NamedArgumentError::UnknownArgument {
                    index: argument_index,
                },
            ))?;
        budget
            .check_pending(0, 1)
            .map_err(|error| SourceCallError::Output {
                argument_index,
                error: PureTypeError::Structure(error),
            })?;
        let (value, scalar_type) = lower(argument).map_err(|error| SourceCallError::Lowering {
            argument_index,
            error,
        })?;
        preflight_with_budget(&value, 1, &mut budget).map_err(|error| match error {
            PureTypeError::Impure => SourceCallError::Argument {
                parameter_index,
                argument_index,
                error: ArgumentTypeError::Impure,
            },
            _ => SourceCallError::Output {
                argument_index,
                error,
            },
        })?;
        // Generated-IR preflight already proved physical purity; do not walk it again.
        let facts = ScalarArgumentType {
            scalar_type,
            is_pure: true,
            literal: match &value {
                Computation::Literal { value } => Some(value),
                _ => None,
            },
        };
        check_argument_type(&parameter.domain, facts).map_err(|error| {
            SourceCallError::Argument {
                parameter_index,
                argument_index,
                error,
            }
        })?;
        lowered.push(LoweredSourceArgument {
            name: argument.name.as_str(),
            parameter_index,
            argument_index,
            value,
        });
    }
    Ok(LoweredSourceCall {
        schema,
        arguments: lowered,
    })
}
