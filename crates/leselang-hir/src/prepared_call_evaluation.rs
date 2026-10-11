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
use crate::ir::{Computation, ComputedBranch};
use crate::native_group::{NativeGroupMemberError, preflight_native_group_members};
use crate::native_group_preparation::{
    NativeGroupPreparationError, PreparedNativeGroup, prepare_native_group_members,
};
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

/// One physical IR forest budget includes the group root and all shifted members.
/// The borrowed value prefix has its own bounded admission under `call.pure`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PreparedGroupEvaluationLimits {
    pub call: CallEvaluationLimits,
    pub max_branches: usize,
}

pub enum PreparedGroupMemberError<Native> {
    Admission(Native),
    Execution(PreparedCallEvaluationError<Native>),
}
impl<Native> fmt::Display for PreparedGroupMemberError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Admission(_) => "prepared group native member admission failed",
            Self::Execution(_) => "prepared group selected member execution failed",
        })
    }
}
impl<Native> fmt::Debug for PreparedGroupMemberError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for PreparedGroupMemberError<Native> {}

/// Matchable positions and original move-only errors, never formatted payloads.
/// Cold denial does not spend fuel; preparation errors retain prior debits.
///
/// ```compile_fail
/// use leselang_hir::prepared_call_evaluation::PreparedGroupEvaluationError;
/// let duplicate = PreparedGroupEvaluationError::<()>::NotGroup.clone();
/// ```
///
/// ```compile_fail
/// use leselang_hir::prepared_call_evaluation::PreparedGroupEvaluationError;
/// let wire = serde_json::to_string(&PreparedGroupEvaluationError::<()>::NotGroup);
/// ```
pub enum PreparedGroupEvaluationError<Native> {
    NotGroup,
    Scope(PureEvaluationFault<Native>),
    Shape(NativeGroupMemberError<Infallible>),
    Physical(PreparedCallTypeError),
    Member {
        index: usize,
        error: PreparedCallEvaluationError<Native>,
    },
    Preparation(NativeGroupPreparationError<PreparedGroupMemberError<Native>>),
}
impl<Native> fmt::Display for PreparedGroupEvaluationError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::NotGroup => "prepared group requires original flat Seq or All IR",
            Self::Scope(_) => "prepared group borrowed scope rejected",
            Self::Shape(_) => "prepared group closed member shape rejected",
            Self::Physical(_) => "prepared group whole physical IR rejected",
            Self::Member { .. } => "prepared group cold member catalog admission rejected",
            Self::Preparation(_) => "prepared group complete preparation failed",
        })
    }
}
impl<Native> fmt::Debug for PreparedGroupEvaluationError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for PreparedGroupEvaluationError<Native> {}

pub type PreparedAtomicGroup<
    'expression,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Domain,
    ResultTag,
    Capability,
> = PreparedNativeGroup<
    'expression,
    ComputedBranch<Node<Field, Operation, HostEffect, IrResult>, IrResult>,
    PreparedCall<'schema, Operation, Domain, ResultTag, Capability>,
>;

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
    let adapter = select_atomic(leaves, host, limits)?;
    execute_atomic(expression, scope, &adapter, fuel)
}

// Private cold observations never escape as reusable admission or authority.
#[expect(
    clippy::type_complexity,
    reason = "preserve original native IR/schema borrows"
)]
fn select_atomic<
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
>(
    leaves: Vec<&'expression Node<Field, Operation, HostEffect, IrResult>>,
    host: &CallEvaluationHost<'a, 'schema, Operation, Domain, ResultTag, Capability, Environment>,
    limits: CallEvaluationLimits,
) -> Result<
    Atomic<
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
    >,
    PreparedCallEvaluationError<Environment::Error>,
>
where
    Operation: PartialEq,
    Domain: ScalarArgumentDomain,
    Capability: PartialEq,
    Environment: PureEvaluationEnvironment<Field, Operation>,
{
    use PreparedCallEvaluationError as Error;
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
    Ok(Atomic {
        sites,
        schema,
        environment: host.environment,
        limits,
    })
}

fn execute_atomic<
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
    adapter: &Atomic<
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
    >,
    fuel: &mut Fuel,
) -> Result<
    PreparedCall<'schema, Operation, Domain, ResultTag, Capability>,
    PreparedCallEvaluationError<Environment::Error>,
>
where
    Domain: ScalarArgumentDomain,
    Environment: PureEvaluationEnvironment<Field, Operation>,
{
    match evaluate(
        expression,
        scope,
        adapter,
        fuel,
        adapter.limits.pure.max_bindings,
    )
    .map_err(PreparedCallEvaluationError::Execution)?
    {
        ResumableEffectOutcome::Request(request) => Ok(request),
        _ => Err(PreparedCallEvaluationError::InvalidOutcome),
    }
}

/// Prepare every atomic member of an original flat Seq/All in one live scope.
/// Whole shifted physical IR, all cold call catalog/signature/uniform-schema
/// gates and every once-ordered native admission precede fuel/value execution.
/// Each member must end in one original Call on every path; different members
/// may select different rows. Names/arity reuse the shared closed member gate.
/// Native admission corroborates each original branch/declaration and current
/// policy against the borrowed original schema; it does not grant dispatch.
///
/// Uses shared atomic selection/execution and native group preparation, not a
/// second evaluator, argument replay, prefix snapshot or continuation capture.
/// One group reservation and one member reservation precede selected execution,
/// whose call root is charged once. Only a complete move-only preparation escapes.
/// Failure/unwind drops earlier prepared values, restores temporary scope locals
/// and preserves debits; there is no partial batch, retry, refund or rollback.
///
/// Hosts still perform whole cold type inference, native ingress/work/memory
/// bounds, fresh publication/invocation authority, reply validation and scheduling.
/// All preparation admissions are observations, not promises about later live
/// policy. No result-dependent Seq arguments or durable execution authority is
/// added. Native callbacks, result-view cloning and Drop are trusted, host-bounded
/// and non-preemptible; a second panic during cleanup may abort.
#[expect(
    clippy::type_complexity,
    reason = "keep original IR/schema and native errors without erasure"
)]
pub fn prepare_atomic_group_in_scope<
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
    group: &'expression Node<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'expression, PureValue<Environment::Result>>,
    host: &CallEvaluationHost<'_, 'schema, Operation, Domain, ResultTag, Capability, Environment>,
    fuel: &mut Fuel,
    limits: PreparedGroupEvaluationLimits,
    mut admit: impl FnMut(
        usize,
        &'expression ComputedBranch<Node<Field, Operation, HostEffect, IrResult>, IrResult>,
        &'schema OperationSchema<'schema, Operation, &'schema str, Domain, ResultTag, Capability>,
    ) -> Result<(), Environment::Error>,
) -> Result<
    PreparedAtomicGroup<
        'expression,
        'schema,
        Field,
        Operation,
        HostEffect,
        IrResult,
        Domain,
        ResultTag,
        Capability,
    >,
    PreparedGroupEvaluationError<Environment::Error>,
>
where
    Operation: PartialEq,
    Domain: ScalarArgumentDomain,
    Capability: PartialEq,
    Environment: PureEvaluationEnvironment<Field, Operation>,
{
    use PreparedGroupEvaluationError as Error;
    if limits.call.max_arguments > MAX_CALL_ARGUMENTS {
        return Err(Error::Physical(PreparedCallTypeError::InvalidLimits));
    }
    preflight_value_scope(scope, limits.call.pure).map_err(Error::Scope)?;
    let Node::Group {
        group_kind,
        branches,
    } = group
    else {
        return Err(Error::NotGroup);
    };
    let _ = preflight_native_group_members(*group_kind, branches, limits.max_branches, |branch| {
        Ok(branch.name.as_str())
    })
    .map_err(Error::Shape)?;
    let mut budget = StructureBudget::new(limits.call.pure.max_nodes, limits.call.pure.max_depth);
    budget
        .visit(0, 0, 0)
        .map_err(|error| Error::Physical(PreparedCallTypeError::Structure(error)))?;
    let mut leaves = Vec::with_capacity(branches.len());
    for (index, branch) in branches.iter().enumerate() {
        leaves.push(
            preflight_call_leaves_with_budget(
                &branch.value,
                1,
                &mut budget,
                limits.call.max_arguments,
            )
            .map_err(|error| Error::Member {
                index,
                error: PreparedCallEvaluationError::Preflight(error),
            })?,
        );
    }
    let mut adapters = Vec::with_capacity(branches.len());
    for (index, leaves) in leaves.into_iter().enumerate() {
        adapters.push(
            select_atomic(leaves, host, limits.call)
                .map_err(|error| Error::Member { index, error })?,
        );
    }
    let mut next_admission = 0;
    prepare_native_group_members(
        *group_kind,
        branches,
        limits.max_branches,
        fuel,
        |branch| Ok(branch.name.as_str()),
        |branch| {
            let index = next_admission;
            next_admission += 1;
            let adapter = adapters
                .get(index)
                .ok_or(PreparedGroupMemberError::Execution(
                    PreparedCallEvaluationError::InvalidOutcome,
                ))?;
            admit(index, branch, adapter.schema).map_err(PreparedGroupMemberError::Admission)
        },
        |index, branch, fuel| {
            let adapter = adapters
                .get(index)
                .ok_or(PreparedGroupMemberError::Execution(
                    PreparedCallEvaluationError::InvalidOutcome,
                ))?;
            execute_atomic(&branch.value, scope, adapter, fuel)
                .map_err(PreparedGroupMemberError::Execution)
        },
    )
    .map_err(Error::Preparation)
}
