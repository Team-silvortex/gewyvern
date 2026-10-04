//! Bounded atomic call inference, without evaluation, dispatch or effect-flow typing.

use std::fmt;

use leselang_runtime_core::{
    ArgumentTypeError, NamedArgumentError, OperationCatalog, OperationCatalogError,
    OperationSchema, ScalarArgumentDomain, ScalarArgumentType, ScopeFrame, StructureBudget,
    StructureError, check_argument_type,
};

use crate::ir::{Computation, ComputedArgument};
use crate::pure_typing::{
    MAX_LOCAL_NAME_BYTES, PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits,
    infer_in_scope, preflight_scope, preflight_with_budget,
};

pub const MAX_CALL_ARGUMENTS: usize = 64;

/// Inclusive explicit policy. The pure limits cover one call root plus all
/// argument trees, not a fresh node/depth budget per argument. max_arguments
/// bounds both submitted arguments and the selected schema's parameter count.
/// Zero arguments allows a parameterless call, not an implicit deny-all.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallTypeLimits {
    pub pure: TypeInferenceLimits,
    pub max_arguments: usize,
}

/// Trusted borrowed host policy, not script-controlled grants or an execution host.
/// Catalog operation keys match the native IR operation slot; parameter names are
/// exact language strings. No keys/domains/result declarations are cloned.
/// This has no default, wire codec, global registry or persistence dependency.
pub struct CallTypeHost<'a, 'schema, Operation, Domain, ResultTag, Capability, Environment> {
    pub catalog:
        &'a OperationCatalog<'schema, Operation, &'schema str, Domain, ResultTag, Capability>,
    pub version: u32,
    pub granted: &'a [Capability],
    pub environment: &'a Environment,
}

impl<Operation, Domain, ResultTag, Capability, Environment> fmt::Debug
    for CallTypeHost<'_, '_, Operation, Domain, ResultTag, Capability, Environment>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CallTypeHost")
            .field("version", &self.version)
            .field("grants", &self.granted.len())
            .finish()
    }
}

/// Closed tags and source/declaration positions, never private names or payloads.
///
/// ```compile_fail
/// use leselang_hir::call_typing::CallTypeError;
/// let wire = serde_json::to_string(&CallTypeError::NotCall).unwrap();
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallTypeError {
    NotCall,
    InvalidLimits,
    ArgumentLimit,
    ParameterLimit,
    InvalidArgumentName {
        argument_index: usize,
    },
    Scope(PureTypeError),
    Structure(StructureError),
    Catalog(OperationCatalogError),
    Names(NamedArgumentError),
    Inference {
        argument_index: usize,
        error: PureTypeError,
    },
    Argument {
        parameter_index: usize,
        argument_index: usize,
        error: ArgumentTypeError,
    },
}

impl fmt::Display for CallTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotCall => formatter.write_str("atomic call inference requires a call node"),
            Self::InvalidLimits => formatter.write_str("atomic call limits exceed safety ceilings"),
            Self::ArgumentLimit => {
                formatter.write_str("atomic call argument count exceeds its limit")
            }
            Self::ParameterLimit => {
                formatter.write_str("atomic call parameter count exceeds its limit")
            }
            Self::InvalidArgumentName { .. } => {
                formatter.write_str("atomic call argument name is invalid")
            }
            Self::Scope(error) | Self::Inference { error, .. } => error.fmt(formatter),
            Self::Structure(_) => formatter.write_str("atomic call structure exceeds its limits"),
            Self::Catalog(error) => error.fmt(formatter),
            Self::Names(error) => error.fmt(formatter),
            Self::Argument { error, .. } => error.fmt(formatter),
        }
    }
}

impl std::error::Error for CallTypeError {}

type Arguments<Field, Operation, HostEffect, IrResult> =
    [ComputedArgument<Computation<Field, Operation, HostEffect, IrResult>>];

fn preflight_arguments<'expression, Field, Operation, HostEffect, IrResult, ResultTag>(
    arguments: &'expression Arguments<Field, Operation, HostEffect, IrResult>,
    bindings: &[(&str, PureType<ResultTag>)],
    limits: CallTypeLimits,
) -> Result<Vec<&'expression str>, CallTypeError> {
    if limits.max_arguments > MAX_CALL_ARGUMENTS {
        return Err(CallTypeError::InvalidLimits);
    }
    preflight_scope(bindings, limits.pure).map_err(CallTypeError::Scope)?;
    if arguments.len() > limits.max_arguments {
        return Err(CallTypeError::ArgumentLimit);
    }
    let mut budget = StructureBudget::new(limits.pure.max_nodes, limits.pure.max_depth);
    budget.visit(0, 0, 0).map_err(CallTypeError::Structure)?;
    preflight_arguments_with_budget(arguments, 1, &mut budget, limits.max_arguments)?;
    Ok(arguments
        .iter()
        .map(|argument| argument.name.as_str())
        .collect())
}

pub(crate) fn preflight_arguments_with_budget<Field, Operation, HostEffect, IrResult>(
    arguments: &Arguments<Field, Operation, HostEffect, IrResult>,
    depth: usize,
    budget: &mut StructureBudget,
    max_arguments: usize,
) -> Result<(), CallTypeError> {
    if arguments.len() > max_arguments {
        return Err(CallTypeError::ArgumentLimit);
    }
    for (argument_index, argument) in arguments.iter().enumerate() {
        let mut bytes = argument.name.bytes();
        if argument.name.len() > MAX_LOCAL_NAME_BYTES
            || !bytes
                .next()
                .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
            || !bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        {
            return Err(CallTypeError::InvalidArgumentName { argument_index });
        }
        preflight_with_budget(&argument.value, depth, budget).map_err(|error| {
            CallTypeError::Inference {
                argument_index,
                error,
            }
        })?;
    }
    Ok(())
}

pub(crate) fn preflight_signature<Key, Domain, ResultTag, Capability>(
    names: &[&str],
    schema: &OperationSchema<'_, Key, &str, Domain, ResultTag, Capability>,
    max_parameters: usize,
) -> Result<(), CallTypeError> {
    if schema.parameters.len() > max_parameters {
        return Err(CallTypeError::ParameterLimit);
    }
    schema.bind_arguments(names).map_err(CallTypeError::Names)?;
    Ok(())
}

fn check_preflighted<
    'expression,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Key,
    Domain,
    ResultTag,
    Capability,
    Environment,
>(
    arguments: &'expression Arguments<Field, Operation, HostEffect, IrResult>,
    names: &[&str],
    schema: &'schema OperationSchema<'_, Key, &str, Domain, ResultTag, Capability>,
    bindings: &[(&'expression str, PureType<Environment::Result>)],
    environment: &Environment,
    limits: CallTypeLimits,
) -> Result<&'schema ResultTag, CallTypeError>
where
    Domain: ScalarArgumentDomain,
    Environment: PureTypeEnvironment<Field, Operation>,
{
    preflight_signature(names, schema, limits.max_arguments)?;
    if arguments.is_empty() {
        return Ok(&schema.result);
    }
    let mut locals = bindings.to_vec();
    let mut scope = ScopeFrame::new(&mut locals);
    check_in_scope(arguments, names, schema, &mut scope, environment, limits)
}

// All argument trees and the selected named signature must already be preflighted.
pub(crate) fn check_in_scope<
    'expression,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Key,
    Domain,
    ResultTag,
    Capability,
    Environment,
>(
    arguments: &'expression Arguments<Field, Operation, HostEffect, IrResult>,
    names: &[&str],
    schema: &'schema OperationSchema<'_, Key, &str, Domain, ResultTag, Capability>,
    scope: &mut ScopeFrame<'_, 'expression, PureType<Environment::Result>>,
    environment: &Environment,
    limits: CallTypeLimits,
) -> Result<&'schema ResultTag, CallTypeError>
where
    Domain: ScalarArgumentDomain,
    Environment: PureTypeEnvironment<Field, Operation>,
{
    for (parameter_index, parameter) in schema.parameters.iter().enumerate() {
        let Some(argument_index) = names.iter().position(|name| *name == parameter.name) else {
            continue;
        };
        let expression = &arguments[argument_index].value;
        let ty = infer_in_scope(expression, scope, environment, limits.pure.max_bindings).map_err(
            |error| CallTypeError::Inference {
                argument_index,
                error,
            },
        )?;
        let facts = ScalarArgumentType {
            scalar_type: ty.scalar_type(),
            is_pure: true,
            literal: match expression {
                Computation::Literal { value } => Some(value),
                _ => None,
            },
        };
        check_argument_type(&parameter.domain, facts).map_err(|error| CallTypeError::Argument {
            parameter_index,
            argument_index,
            error,
        })?;
    }
    Ok(&schema.result)
}

/// Infer arguments from real shared IR against an already selected host signature.
///
/// Limits/prefix and the entire physical argument forest (including every cold
/// child) are preflighted before cloning metadata or invoking field/member/domain
/// queries. Shape checks then precede declaration-order inference/domain checks.
/// Optional omissions stay absent; reported indices preserve original submission
/// and declaration positions. No IR, literal text, operation key or schema result
/// payload is cloned; query identifiers follow the environment's Clone contract.
/// No guard/arithmetic/helper/effect executes.
///
/// Success returns only the borrowed unchecked result declaration. Selecting the
/// operation/version/capability is the caller's responsibility for this entry.
/// Native queries/comparisons/Clone/drop remain trusted code and may unwind or
/// mutate interior state, without preemption or rollback. Dynamic domains require
/// revalidation after evaluation. This is not full source/effect-flow typing,
/// dispatch permission, result acceptance or a continuation handle.
pub fn check_call_arguments<
    'expression,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Key,
    Domain,
    ResultTag,
    Capability,
    Environment,
>(
    arguments: &'expression Arguments<Field, Operation, HostEffect, IrResult>,
    schema: &'schema OperationSchema<'_, Key, &str, Domain, ResultTag, Capability>,
    bindings: &[(&'expression str, PureType<Environment::Result>)],
    environment: &Environment,
    limits: CallTypeLimits,
) -> Result<&'schema ResultTag, CallTypeError>
where
    Domain: ScalarArgumentDomain,
    Environment: PureTypeEnvironment<Field, Operation>,
{
    let names = preflight_arguments(arguments, bindings, limits)?;
    check_preflighted(arguments, &names, schema, bindings, environment, limits)
}

/// Infer one native Call through an exact-version catalog and trusted grants.
///
/// Structural/pure preflight of all parameters precedes native catalog/capability
/// comparisons; catalog selection precedes signature/type/domain queries. Host,
/// Group, Bind and Choose roots are not silently treated as atomic calls. Nested
/// effects in any argument remain rejected. No schema result is cloned/formatted.
/// The returned borrow pins direct schema mutation, not interior state, authority,
/// execution, receipts or evaluated values. There is no dispatch or saved frame.
/// Hosts separately bound catalog/grant counts, native work and metadata payloads.
///
/// ```
/// use leselang_hir::{ir::{Computation, ComputedArgument}, call_typing::*, pure_typing::*};
/// use leselang_runtime_core::*;
/// struct Host;
/// impl PureTypeEnvironment<(), u32> for Host {
///     type Result = ();
///     fn field_type(&self, _: &(), _: &()) -> Option<ScalarType> { None }
///     fn member_result(&self, _: &(), _: &str, _: &u32) -> Option<()> { None }
/// }
/// let parameters = [NamedParameter::required("position", ScalarTypeSet::only(ScalarType::Integer))];
/// let schemas = [OperationSchema { key: 17u32, parameters: &parameters,
///     result: "position-receipt", required_capability: 31u8 }];
/// let catalog = OperationCatalog::new(9, &schemas, OperationCatalogLimits {
///     max_operations: 1, max_parameters_per_operation: 1 }).unwrap();
/// let call: Computation<(), u32, (), ()> = Computation::Call { operation: 17,
///     arguments: vec![ComputedArgument { name: "position".into(),
///         value: Computation::Literal { value: ScalarValue::Integer(7) } }] };
/// let host = CallTypeHost { catalog: &catalog, version: 9, granted: &[31], environment: &Host };
/// let result = infer_call_type(&call, &[], &host, CallTypeLimits {
///     pure: TypeInferenceLimits { max_nodes: 2, max_depth: 1, max_bindings: 0 }, max_arguments: 1 }).unwrap();
/// assert!(std::ptr::eq(result, &schemas[0].result));
/// ```
pub fn infer_call_type<
    'expression,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Domain,
    ResultTag,
    Capability,
    Environment,
>(
    expression: &'expression Computation<Field, Operation, HostEffect, IrResult>,
    bindings: &[(&'expression str, PureType<Environment::Result>)],
    host: &CallTypeHost<'_, 'schema, Operation, Domain, ResultTag, Capability, Environment>,
    limits: CallTypeLimits,
) -> Result<&'schema ResultTag, CallTypeError>
where
    Operation: PartialEq,
    Domain: ScalarArgumentDomain,
    Capability: PartialEq,
    Environment: PureTypeEnvironment<Field, Operation>,
{
    let Computation::Call {
        operation,
        arguments,
    } = expression
    else {
        return Err(CallTypeError::NotCall);
    };
    let names = preflight_arguments(arguments, bindings, limits)?;
    let schema = host
        .catalog
        .authorize(operation, host.version, host.granted)
        .map_err(CallTypeError::Catalog)?;
    check_preflighted(
        arguments,
        &names,
        schema,
        bindings,
        host.environment,
        limits,
    )
}
