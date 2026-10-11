//! Original-schema atomic call preparation through the shared synchronous walker.

use std::{convert::Infallible, fmt};

use leselang_runtime_core::{
    CalculationFailure, Fuel, OperationSchema, ScalarArgumentDomain, ScalarValue, ScopeFrame,
    StructureBudget,
};

use crate::call_evaluation::{
    CallEvaluationError, CallEvaluationHost, CallEvaluationLimits, PreparedCall, evaluate_arguments,
};
use crate::call_typing::{CallTypeError, MAX_CALL_ARGUMENTS, preflight_signature};
use crate::effect_evaluation::{EffectEvaluationEnvironment, EffectEvaluationFailure, evaluate};
use crate::effect_reentry::ResumableEffectOutcome;
use crate::ir::Computation;
use crate::prepared_typing::{PreparedCallTypeError, preflight_call_leaves_with_budget};
use crate::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationFault, PureValue, preflight_value_scope,
};

/// Original move-only native errors. Formatting exposes neither values nor keys.
pub enum PreparedCallNativeFault<Native> {
    Projection(Native),
    Arguments(CallEvaluationError<Native>),
    InvalidSite,
}

impl<Native> fmt::Display for PreparedCallNativeFault<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Projection(_) => "prepared call native projection failed",
            Self::Arguments(_) => "prepared call actual arguments rejected",
            Self::InvalidSite => "prepared call original site rejected",
        })
    }
}
impl<Native> fmt::Debug for PreparedCallNativeFault<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Native> std::error::Error for PreparedCallNativeFault<Native> {}

/// Cold failures are distinct from charged execution failures; neither is retried.
///
/// ```compile_fail
/// use leselang_hir::prepared_call_evaluation::PreparedCallEvaluationError;
/// let error = PreparedCallEvaluationError::<()>::InvalidOutcome;
/// let duplicate = error.clone();
/// ```
///
/// ```compile_fail
/// use leselang_hir::prepared_call_evaluation::PreparedCallEvaluationError;
/// let error = PreparedCallEvaluationError::<()>::InvalidOutcome;
/// let wire = serde_json::to_string(&error);
/// ```
pub enum PreparedCallEvaluationError<Native> {
    Preflight(PreparedCallTypeError),
    Scope(PureEvaluationFault<Native>),
    Execution(EffectEvaluationFailure<PreparedCallNativeFault<Native>>),
    InvalidOutcome,
}

impl<Native> fmt::Display for PreparedCallEvaluationError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Preflight(_) => "prepared call cold admission rejected",
            Self::Scope(_) => "prepared call borrowed scope rejected",
            Self::Execution(_) => "prepared call selected execution failed",
            Self::InvalidOutcome => "prepared call did not produce one atomic request",
        })
    }
}
impl<Native> fmt::Debug for PreparedCallEvaluationError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Native> std::error::Error for PreparedCallEvaluationError<Native> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
type Site<'expression, Field, Operation, HostEffect, IrResult> = (
    &'expression Node<Field, Operation, HostEffect, IrResult>,
    Vec<&'expression str>,
);

struct Atomic<
    'a,
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
> {
    sites: Vec<Site<'expression, Field, Operation, HostEffect, IrResult>>,
    schema:
        &'schema OperationSchema<'schema, Operation, &'schema str, Domain, ResultTag, Capability>,
    environment: &'a Environment,
    limits: CallEvaluationLimits,
}

impl<Field, Operation, HostEffect, IrResult, Domain, ResultTag, Capability, Environment>
    PureEvaluationEnvironment<Field, Operation>
    for Atomic<
        '_,
        '_,
        '_,
        Field,
        Operation,
        HostEffect,
        IrResult,
        Domain,
        ResultTag,
        Capability,
        Environment,
    >
where
    Environment: PureEvaluationEnvironment<Field, Operation>,
{
    type Result = Environment::Result;
    type Error = PreparedCallNativeFault<Environment::Error>;
    fn field(&self, result: &Self::Result, field: &Field) -> Result<ScalarValue, Self::Error> {
        self.environment
            .field(result, field)
            .map_err(Self::Error::Projection)
    }
    fn member(
        &self,
        group: &Self::Result,
        name: &str,
        operation: &Operation,
    ) -> Result<Self::Result, Self::Error> {
        self.environment
            .member(group, name, operation)
            .map_err(Self::Error::Projection)
    }
}

impl<
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
> EffectEvaluationEnvironment<'expression, Field, Operation, HostEffect, IrResult>
    for Atomic<
        '_,
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
    >
where
    Domain: ScalarArgumentDomain,
    Environment: PureEvaluationEnvironment<Field, Operation>,
{
    type Request = PreparedCall<'schema, Operation, Domain, ResultTag, Capability>;
    type Capture = Infallible;
    fn preflight_effect(
        &self,
        _: &'expression Node<Field, Operation, HostEffect, IrResult>,
    ) -> Result<(), Self::Error> {
        Err(Self::Error::InvalidSite)
    }
    fn prepare_effect(
        &self,
        expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
        scope: &mut ScopeFrame<'_, 'expression, PureValue<Self::Result>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Request, CalculationFailure<Self::Error>> {
        let Some((_, names)) = self
            .sites
            .iter()
            .find(|(original, _)| std::ptr::eq(*original, expression))
        else {
            return Err(Self::Error::InvalidSite.into());
        };
        let Node::Call { arguments, .. } = expression else {
            return Err(Self::Error::InvalidSite.into());
        };
        // The shared control walker has already charged this selected call root.
        evaluate_arguments(
            arguments,
            names,
            self.schema,
            scope,
            self.environment,
            fuel,
            self.limits,
        )
        .map_err(|error| Self::Error::Arguments(error).into())
    }
    fn capture(
        &self,
        _: &'expression str,
        _: &'expression Node<Field, Operation, HostEffect, IrResult>,
        _: &ScopeFrame<'_, 'expression, PureValue<Self::Result>>,
        _: &mut Fuel,
    ) -> Result<Self::Capture, Self::Error> {
        Err(Self::Error::InvalidSite)
    }
}

/// Prepare pure Bind/Choose ending in exactly one uniform original catalog Call.
///
/// The entire borrowed prefix and cold physical IR pass before native catalog
/// lookup. All cold calls then pass version/grants/named shape and select the
/// same original schema by pointer identity, before fuel, projection or value
/// cloning. Host/Group, effect-valued bindings, scalar tails and mixed-operation
/// branches are rejected, including unselected paths. Pure preparation can use
/// the existing bounded scalar control forms, but may not perform effects.
///
/// Selected execution reuses the guarded synchronous control walker and original
/// argument evaluator with the caller's one Fuel and still-live ScopeFrame.
/// There is no added wrapper/call-root charge, prefix snapshot or capture. The
/// original schema and declaration-ordered owned arguments are returned without
/// dispatch, identity allocation, persistence or retry. Temporary locals unwind
/// on errors and native unwind; fuel is never refunded. This dynamic entry does
/// not replace `infer_prepared_call_type` for cold type checking or live dispatch
/// authority/result validation. Native catalog/projection/domain work and Drop
/// remain trusted, non-preemptible, non-transactional host responsibilities.
pub fn prepare_atomic_call_in_scope<
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
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'expression, PureValue<Environment::Result>>,
    host: &CallEvaluationHost<'_, 'schema, Operation, Domain, ResultTag, Capability, Environment>,
    fuel: &mut Fuel,
    limits: CallEvaluationLimits,
) -> Result<
    PreparedCall<'schema, Operation, Domain, ResultTag, Capability>,
    PreparedCallEvaluationError<Environment::Error>,
>
where
    Operation: PartialEq,
    Domain: ScalarArgumentDomain,
    Capability: PartialEq,
    Environment: PureEvaluationEnvironment<Field, Operation>,
{
    use PreparedCallEvaluationError as Error;
    if limits.max_arguments > MAX_CALL_ARGUMENTS {
        return Err(Error::Preflight(PreparedCallTypeError::InvalidLimits));
    }
    preflight_value_scope(scope, limits.pure).map_err(Error::Scope)?;
    let mut budget = StructureBudget::new(limits.pure.max_nodes, limits.pure.max_depth);
    let leaves =
        preflight_call_leaves_with_budget(expression, 0, &mut budget, limits.max_arguments)
            .map_err(Error::Preflight)?;
    let mut common: Option<
        &OperationSchema<'schema, Operation, &'schema str, Domain, ResultTag, Capability>,
    > = None;
    let mut sites = Vec::with_capacity(leaves.len());
    for (call_index, original) in leaves.into_iter().enumerate() {
        let Node::Call {
            operation,
            arguments,
        } = original
        else {
            return Err(Error::Preflight(PreparedCallTypeError::InvalidFlow));
        };
        let schema = host
            .catalog
            .authorize(operation, host.version, host.granted)
            .map_err(|error| {
                Error::Preflight(PreparedCallTypeError::Call {
                    call_index,
                    error: CallTypeError::Catalog(error),
                })
            })?;
        let names: Vec<_> = arguments
            .iter()
            .map(|argument| argument.name.as_str())
            .collect();
        preflight_signature(&names, schema, limits.max_arguments)
            .map_err(|error| Error::Preflight(PreparedCallTypeError::Call { call_index, error }))?;
        if common.is_some_and(|row| !std::ptr::eq(row, schema)) {
            return Err(Error::Preflight(
                PreparedCallTypeError::InconsistentOperation,
            ));
        }
        common = Some(schema);
        sites.push((original, names));
    }
    let schema = common.ok_or(Error::Preflight(PreparedCallTypeError::InvalidFlow))?;
    let adapter = Atomic {
        sites,
        schema,
        environment: host.environment,
        limits,
    };
    match evaluate(expression, scope, &adapter, fuel, limits.pure.max_bindings)
        .map_err(Error::Execution)?
    {
        ResumableEffectOutcome::Request(request) => Ok(request),
        _ => Err(Error::InvalidOutcome),
    }
}
