//! Scalar helper signatures and argument construction, not helper expansion.

use std::fmt;

use leselang_runtime_core::{
    ArgumentTypeError, NamedArgumentError, NamedParameter, ScalarArgumentType, ScalarType,
    ScalarTypeSet, StructureBudget, bind_named_arguments, check_argument_type,
};
use leselang_syntax::{Expression, Function, MAX_FUNCTION_PARAMETERS, NamedArgument, Span};

use crate::ir::Computation;
use crate::pure_typing::{PureTypeError, preflight_with_budget, valid_local_name};
use crate::source_call::{
    SourceCallError, SourceCallLimits, SourceShapeError, preflight, valid_limits,
};

pub type HelperParameter<'source> = NamedParameter<&'source str, ScalarType>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelperParameterError {
    InvalidLimits,
    ParameterLimit,
    InvalidName { index: usize },
    DuplicateName { index: usize },
    UnknownType { index: usize },
}
impl fmt::Display for HelperParameterError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper parameter limit exceeds the safety ceiling",
            Self::ParameterLimit => "helper parameter count exceeds its limit",
            Self::InvalidName { .. } => "helper parameter name is invalid",
            Self::DuplicateName { .. } => "helper parameter name is duplicated",
            Self::UnknownType { .. } => "helper parameter type is not a closed scalar type",
        })
    }
}
impl std::error::Error for HelperParameterError {}

/// Borrow declaration names and parse the six exact scalar tokens in source order.
/// The explicit inclusive parameter ceiling cannot exceed the syntax ceiling (8).
/// Zero permits an empty signature. Function names/bodies, main/builtin policy,
/// recursion, return typing and template ownership are deliberately not checked.
pub fn helper_parameters(
    function: &Function,
    max_parameters: usize,
) -> Result<Vec<HelperParameter<'_>>, HelperParameterError> {
    if max_parameters > MAX_FUNCTION_PARAMETERS {
        return Err(HelperParameterError::InvalidLimits);
    }
    if function.parameters.len() > max_parameters {
        return Err(HelperParameterError::ParameterLimit);
    }
    let mut parameters = Vec::with_capacity(function.parameters.len());
    for (index, parameter) in function.parameters.iter().enumerate() {
        if !valid_local_name(&parameter.name) {
            return Err(HelperParameterError::InvalidName { index });
        }
        if parameters
            .iter()
            .any(|previous: &HelperParameter<'_>| previous.name == parameter.name)
        {
            return Err(HelperParameterError::DuplicateName { index });
        }
        let ty = match parameter.type_name.as_str() {
            "integer" => ScalarType::Integer,
            "boolean" => ScalarType::Boolean,
            "string" => ScalarType::String,
            "none" => ScalarType::None,
            "optional_string" => ScalarType::OptionalString,
            "string_list" => ScalarType::StringList,
            _ => return Err(HelperParameterError::UnknownType { index }),
        };
        parameters.push(NamedParameter::required(parameter.name.as_str(), ty));
    }
    Ok(parameters)
}

/// Borrowed selected signature, not a catalog registration or type certificate.
pub struct HelperSignature<'signature> {
    pub name: &'signature str,
    pub parameters: &'signature [HelperParameter<'signature>],
}

/// Physical source and produced argument forest budgets are separate. The source
/// call root is charged; output roots start at depth zero and share one meter.
/// No helper body, Bind wrapper, pre-fold expansion or execution fuel is included.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperSourceLimits {
    pub source: SourceCallLimits,
    pub max_parameters: usize,
}

pub enum HelperSourceError<Error> {
    InvalidLimits,
    NotCall,
    InvalidSignature,
    WrongCallee,
    ParameterLimit,
    ArgumentLimit,
    InvalidParameter {
        index: usize,
    },
    Shape {
        span: Span,
        error: SourceShapeError,
    },
    Names(NamedArgumentError),
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
impl<Error> fmt::Display for HelperSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper source limits exceed safety ceilings",
            Self::NotCall => "helper argument lowering requires a call",
            Self::InvalidSignature => "helper signature name is invalid",
            Self::WrongCallee => "source callee does not match the selected helper",
            Self::ParameterLimit => "helper signature has too many parameters",
            Self::ArgumentLimit => "helper call has too many arguments",
            Self::InvalidParameter { .. } => "helper parameters must be bounded required scalars",
            Self::Shape { .. } => "helper call physical source is invalid",
            Self::Names(_) => "helper named arguments are invalid",
            Self::Lowering { .. } => "native helper argument lowering failed",
            Self::Output { .. } => "produced helper argument shape is invalid",
            Self::Argument { .. } => "helper argument must have the exact pure scalar type",
        })
    }
}
impl<Error> fmt::Debug for HelperSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for HelperSourceError<Error> {}

/// Declaration order, original submitted index and borrowed metadata/AST identity.
/// Owned native IR slots move without Clone/Debug/serde/Send requirements. This
/// ephemeral observation is not a validated program, receipt or execution grant.
pub struct LoweredHelperArgument<'source, 'signature, Node> {
    pub parameter: &'signature HelperParameter<'signature>,
    pub argument: &'source NamedArgument,
    pub parameter_index: usize,
    pub argument_index: usize,
    pub value: Node,
}
type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type HelperSourceResult<'source, 'signature, Node, Error> =
    Result<Vec<LoweredHelperArgument<'source, 'signature, Node>>, HelperSourceError<Error>>;

/// Bound the entire cold source before lowering any argument. All required scalar
/// signature keys and submitted keys are checked before callbacks; values lower
/// once in declaration order, not submission order, in the caller-owned scope.
/// Native observations are not type certificates: complete cold inference against
/// the exact native prefix remains mandatory. Literal facts are checked exactly.
/// Nested source calls use the separate source argument bound, not the helper's
/// eight-parameter ceiling. Invalid/failed callbacks never retry; partial outputs
/// drop once, without reservation/fuel rollback. Native work/Drop remain trusted
/// adapter policy. No template cloning, parameter wrapper, body hygiene, dispatch,
/// implicit coercion/default, capability grant or suspension is performed here.
pub fn lower_helper_arguments<
    'source,
    'signature,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Error,
>(
    expression: &'source Expression,
    signature: HelperSignature<'signature>,
    limits: HelperSourceLimits,
    lower: impl FnMut(
        &'source NamedArgument,
    ) -> Result<
        (
            Node<Field, Operation, HostEffect, IrResult>,
            Option<ScalarType>,
        ),
        Error,
    >,
) -> HelperSourceResult<'source, 'signature, Node<Field, Operation, HostEffect, IrResult>, Error> {
    if !valid_limits(limits.source) || limits.max_parameters > MAX_FUNCTION_PARAMETERS {
        return Err(HelperSourceError::InvalidLimits);
    }
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(HelperSourceError::NotCall);
    };
    preflight::<Error>(expression, limits.source).map_err(|error| match error {
        SourceCallError::Shape { span, error } => HelperSourceError::Shape { span, error },
        _ => HelperSourceError::InvalidLimits,
    })?;
    if !valid_local_name(signature.name) {
        return Err(HelperSourceError::InvalidSignature);
    }
    if callee != signature.name {
        return Err(HelperSourceError::WrongCallee);
    }
    lower_preflighted_helper_arguments(arguments, signature.parameters, limits, lower)
}

// The reference adapter already bounds source and retains legacy error precedence.
pub(crate) fn lower_preflighted_helper_arguments<
    'source,
    'signature,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Error,
>(
    arguments: &'source [NamedArgument],
    parameters: &'signature [HelperParameter<'signature>],
    limits: HelperSourceLimits,
    mut lower: impl FnMut(
        &'source NamedArgument,
    ) -> Result<
        (
            Node<Field, Operation, HostEffect, IrResult>,
            Option<ScalarType>,
        ),
        Error,
    >,
) -> HelperSourceResult<'source, 'signature, Node<Field, Operation, HostEffect, IrResult>, Error> {
    if !valid_limits(limits.source) || limits.max_parameters > MAX_FUNCTION_PARAMETERS {
        return Err(HelperSourceError::InvalidLimits);
    }
    if parameters.len() > limits.max_parameters {
        return Err(HelperSourceError::ParameterLimit);
    }
    if arguments.len() > limits.max_parameters || arguments.len() > limits.source.max_arguments {
        return Err(HelperSourceError::ArgumentLimit);
    }
    for (index, parameter) in parameters.iter().enumerate() {
        if !parameter.required || !valid_local_name(parameter.name) {
            return Err(HelperSourceError::InvalidParameter { index });
        }
    }
    let names = arguments
        .iter()
        .map(|argument| argument.name.as_str())
        .collect::<Vec<_>>();
    let bound = bind_named_arguments(&names, parameters).map_err(HelperSourceError::Names)?;
    let mut budget = StructureBudget::new(
        limits.source.max_lowered_nodes,
        limits.source.max_lowered_depth,
    );
    let mut output = Vec::with_capacity(arguments.len());
    for (parameter_index, (_, argument_index)) in bound.iter().enumerate() {
        // Required-only binding visits every parameter; reborrow the original
        // signature rather than returning the short-lived alignment view's borrow.
        let parameter = &parameters[parameter_index];
        let argument = &arguments[argument_index];
        budget
            .check_pending(0, arguments.len() - parameter_index)
            .map_err(|error| HelperSourceError::Output {
                argument_index,
                error: PureTypeError::Structure(error),
            })?;
        let (value, scalar_type) =
            lower(argument).map_err(|error| HelperSourceError::Lowering {
                argument_index,
                error,
            })?;
        preflight_with_budget(&value, 0, &mut budget).map_err(|error| match error {
            PureTypeError::Impure => HelperSourceError::Argument {
                parameter_index,
                argument_index,
                error: ArgumentTypeError::Impure,
            },
            _ => HelperSourceError::Output {
                argument_index,
                error,
            },
        })?;
        let facts = ScalarArgumentType {
            scalar_type,
            is_pure: true,
            literal: match &value {
                Node::Literal { value } => Some(value),
                _ => None,
            },
        };
        check_argument_type(&ScalarTypeSet::only(parameter.domain), facts).map_err(|error| {
            HelperSourceError::Argument {
                parameter_index,
                argument_index,
                error,
            }
        })?;
        output.push(LoweredHelperArgument {
            parameter,
            argument,
            parameter_index,
            argument_index,
            value,
        });
    }
    Ok(output)
}
