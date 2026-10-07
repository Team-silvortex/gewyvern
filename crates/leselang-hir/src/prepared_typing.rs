//! Shared call-only atomic preparation inference, not capture/group execution.

use std::fmt;

use leselang_runtime_core::{
    OperationCatalogError, OperationSchema, ScalarArgumentDomain, ScalarType, ScopeFrame,
    StructureBudget, StructureError,
};

use crate::call_typing::{
    CallTypeError, CallTypeHost, CallTypeLimits, MAX_CALL_ARGUMENTS, check_in_scope,
    preflight_arguments_with_budget, preflight_signature,
};
use crate::ir::Computation;
use crate::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, infer_in_scope, preflight_scope,
    preflight_with_budget, valid_local_name,
};

/// The original native operation schema, not a second signature or wire model.
pub type PreparedCallSchema<'a, Key, Domain, ResultTag, Capability> =
    OperationSchema<'a, Key, &'a str, Domain, ResultTag, Capability>;

/// Borrowed declaration selected by a native schema profile, not an owned handle.
pub type SelectedPreparedCall<'schema, Operation, Schemas> = &'schema PreparedCallSchema<
    'schema,
    <Schemas as PreparedCallSchemas<'schema, Operation>>::Key,
    <Schemas as PreparedCallSchemas<'schema, Operation>>::Domain,
    <Schemas as PreparedCallSchemas<'schema, Operation>>::Result,
    <Schemas as PreparedCallSchemas<'schema, Operation>>::Capability,
>;

/// Trusted schema selection. Implementations validate their own exact versions
/// and capability policy, returning a stable original row for each operation.
/// Equal operation keys must select the same declaration identity. Different
/// operations may not share one row, even if their result tags happen to agree.
/// Queries may allocate, mutate interior state or unwind; no effects are invoked.
/// Host-owned schema/key/payload/work limits remain separate from physical IR.
pub trait PreparedCallSchemas<'schema, Operation> {
    type Key: 'schema;
    type Domain: ScalarArgumentDomain + 'schema;
    type Result: 'schema;
    type Capability: 'schema;

    fn select(
        &self,
        operation: &Operation,
    ) -> Result<SelectedPreparedCall<'schema, Operation, Self>, OperationCatalogError>;
}

impl<'schema, Operation, Domain, ResultTag, Capability, Environment>
    PreparedCallSchemas<'schema, Operation>
    for CallTypeHost<'_, 'schema, Operation, Domain, ResultTag, Capability, Environment>
where
    Operation: PartialEq + 'schema,
    Domain: ScalarArgumentDomain + 'schema,
    ResultTag: 'schema,
    Capability: PartialEq + 'schema,
{
    type Key = Operation;
    type Domain = Domain;
    type Result = ResultTag;
    type Capability = Capability;

    fn select(
        &self,
        operation: &Operation,
    ) -> Result<
        &'schema PreparedCallSchema<'schema, Operation, Domain, ResultTag, Capability>,
        OperationCatalogError,
    > {
        self.catalog
            .authorize(operation, self.version, self.granted)
    }
}

/// Closed failures: never native keys, source names, payloads or execution handles.
///
/// ```compile_fail
/// use leselang_hir::prepared_typing::PreparedCallTypeError;
/// let wire = serde_json::to_string(&PreparedCallTypeError::InvalidFlow).unwrap();
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PreparedCallTypeError {
    InvalidFlow,
    InvalidLimits,
    Structure(StructureError),
    Preparation(PureTypeError),
    ConditionType,
    InconsistentOperation,
    Call {
        call_index: usize,
        error: CallTypeError,
    },
}

impl fmt::Display for PreparedCallTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidFlow => {
                formatter.write_str("prepared call flow must end in one call on every path")
            }
            Self::InvalidLimits => {
                formatter.write_str("prepared call limits exceed safety ceilings")
            }
            Self::Structure(_) => formatter.write_str("prepared call structure exceeds its limits"),
            Self::Preparation(error) => error.fmt(formatter),
            Self::ConditionType => formatter.write_str("prepared call condition must be boolean"),
            Self::InconsistentOperation => {
                formatter.write_str("prepared call paths must use the same operation declaration")
            }
            Self::Call { error, .. } => error.fmt(formatter),
        }
    }
}

impl std::error::Error for PreparedCallTypeError {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

// Call/Bind/Choose edges only. Reference callers first validate full structure,
// canonical source and the old prepared-atomic classification before using this.
pub(crate) fn call_leaves_only<Field, Operation, HostEffect, IrResult>(
    expression: &Node<Field, Operation, HostEffect, IrResult>,
) -> bool {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        match node {
            Computation::Call { .. } => {}
            Computation::Bind { body, .. } => pending.push(body),
            Computation::Choose {
                then, otherwise, ..
            } => {
                pending.push(otherwise);
                pending.push(then);
            }
            _ => return false,
        }
    }
    true
}

fn preflight<'expression, Field, Operation, HostEffect, IrResult, ResultTag>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    bindings: &[(&str, PureType<ResultTag>)],
    limits: CallTypeLimits,
) -> Result<Vec<&'expression Node<Field, Operation, HostEffect, IrResult>>, PreparedCallTypeError> {
    if limits.max_arguments > MAX_CALL_ARGUMENTS {
        return Err(PreparedCallTypeError::InvalidLimits);
    }
    preflight_scope(bindings, limits.pure).map_err(PreparedCallTypeError::Preparation)?;
    let mut budget = StructureBudget::new(limits.pure.max_nodes, limits.pure.max_depth);
    preflight_call_leaves_with_budget(expression, 0, &mut budget, limits.max_arguments)
}

// Shared physical-only atomic preparation walk. The caller owns scope/limit
// validity and supplies one budget for all enclosing groups and cold branches.
pub(crate) fn preflight_call_leaves_with_budget<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    depth: usize,
    budget: &mut StructureBudget,
    max_arguments: usize,
) -> Result<Vec<&'expression Node<Field, Operation, HostEffect, IrResult>>, PreparedCallTypeError> {
    preflight_atomic_leaves_with_budget(expression, depth, budget, max_arguments, false)
}

pub(crate) fn preflight_atomic_leaves_with_budget<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    depth: usize,
    budget: &mut StructureBudget,
    max_arguments: usize,
    allow_hosts: bool,
) -> Result<Vec<&'expression Node<Field, Operation, HostEffect, IrResult>>, PreparedCallTypeError> {
    let mut pending = vec![(expression, depth)];
    let mut calls = Vec::new();
    let mut call_count = 0;
    while let Some((node, depth)) = pending.pop() {
        budget
            .visit(depth, 0, 0)
            .map_err(PreparedCallTypeError::Structure)?;
        match node {
            Computation::Call { arguments, .. } => {
                preflight_arguments_with_budget(arguments, depth + 1, budget, max_arguments)
                    .map_err(|error| PreparedCallTypeError::Call {
                        call_index: call_count,
                        error,
                    })?;
                calls.push(node);
                call_count += 1;
            }
            Computation::Host { .. } if allow_hosts => calls.push(node),
            Computation::Bind { name, value, body } => {
                if !valid_local_name(name) {
                    return Err(PreparedCallTypeError::Preparation(
                        PureTypeError::InvalidName,
                    ));
                }
                preflight_with_budget(value, depth + 1, budget)
                    .map_err(PreparedCallTypeError::Preparation)?;
                budget
                    .check_pending(pending.len(), 1)
                    .map_err(PreparedCallTypeError::Structure)?;
                pending.push((body, depth + 1));
            }
            Computation::Choose {
                when,
                then,
                otherwise,
            } => {
                preflight_with_budget(when, depth + 1, budget)
                    .map_err(PreparedCallTypeError::Preparation)?;
                budget
                    .check_pending(pending.len(), 2)
                    .map_err(PreparedCallTypeError::Structure)?;
                pending.push((otherwise, depth + 1));
                pending.push((then, depth + 1));
            }
            _ => return Err(PreparedCallTypeError::InvalidFlow),
        }
    }
    Ok(calls)
}

type Selection<'expression, 'schema, Key, Domain, ResultTag, Capability> = (
    Vec<&'expression str>,
    &'schema PreparedCallSchema<'schema, Key, Domain, ResultTag, Capability>,
);

struct Selections<'expression, 'schema, Key, Domain, ResultTag, Capability> {
    calls: Vec<Selection<'expression, 'schema, Key, Domain, ResultTag, Capability>>,
    next: usize,
}

fn infer<
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
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'expression, PureType<Environment::Result>>,
    environment: &Environment,
    selections: &mut Selections<'expression, 'schema, Key, Domain, ResultTag, Capability>,
    limits: CallTypeLimits,
) -> Result<&'schema ResultTag, PreparedCallTypeError>
where
    Domain: ScalarArgumentDomain,
    Environment: PureTypeEnvironment<Field, Operation>,
{
    match expression {
        Computation::Call { arguments, .. } => {
            let call_index = selections.next;
            let Some((names, schema)) = selections.calls.get(call_index) else {
                return Err(PreparedCallTypeError::InvalidFlow);
            };
            selections.next += 1;
            check_in_scope(arguments, names, schema, scope, environment, limits)
                .map_err(|error| PreparedCallTypeError::Call { call_index, error })
        }
        Computation::Bind { name, value, body } => {
            if scope.get(name).is_some() {
                return Err(PreparedCallTypeError::Preparation(
                    PureTypeError::ShadowedBinding,
                ));
            }
            if scope.len() >= limits.pure.max_bindings {
                return Err(PreparedCallTypeError::Preparation(
                    PureTypeError::BindingLimit,
                ));
            }
            let ty = infer_in_scope(value, scope, environment, limits.pure.max_bindings)
                .map_err(PreparedCallTypeError::Preparation)?;
            let mut local = scope.nested();
            local
                .push(name, ty)
                .map_err(|_| PreparedCallTypeError::Preparation(PureTypeError::ShadowedBinding))?;
            infer(body, &mut local, environment, selections, limits)
        }
        Computation::Choose {
            when,
            then,
            otherwise,
        } => {
            let condition = infer_in_scope(when, scope, environment, limits.pure.max_bindings)
                .map_err(PreparedCallTypeError::Preparation)?;
            if condition.scalar_type() != Some(ScalarType::Boolean) {
                return Err(PreparedCallTypeError::ConditionType);
            }
            let result = infer(then, scope, environment, selections, limits)?;
            infer(otherwise, scope, environment, selections, limits)?;
            Ok(result)
        }
        _ => Err(PreparedCallTypeError::InvalidFlow),
    }
}

/// Infer call-only Bind/Choose preparation and every cold terminal signature.
///
/// Phase 1 checks the whole physical tree/prefix under one inclusive budget before
/// any native comparison, metadata clone or query. Phase 2 selects every terminal
/// schema and checks count/named shape, requiring the exact same original row on
/// all paths. Phase 3 infers pure bindings, boolean guards and every call argument
/// in declaration order. Lexical storage is reused; siblings never leak locals.
/// A result alias may carry externally supplied type metadata, not a new capture.
///
/// The schema selector owns version/capability policy and stable row identity.
/// CallTypeHost implements it through exact-version native catalog authorization.
/// Native work/clone/equality/drop are trusted, may unwind and are not rolled back.
/// Only query identifiers may be cloned; IR/text/schema result payloads are not.
/// Arithmetic/guards do not execute and eventual values require domain validation.
///
/// Success returns the first original borrowed result declaration, not a prepared
/// request, execution grant, receipt, continuation or wire handle. Host leaves and
/// Group/capture/multi-effect/scalar-exit flows are rejected, not auto-adapted.
/// Opaque Host validation, generic source lowering, full flow typing, execution
/// and suspension/recovery remain separate. Physical bounds are not source costs,
/// prior allocation/native work/memory quotas or evaluation fuel.
///
/// ```
/// use leselang_hir::{ir::{Computation, ComputedArgument}, call_typing::*, pure_typing::*, prepared_typing::*};
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
/// let expression: Computation<(), u32, (), ()> = Computation::Bind {
///     name: "position".into(), value: Box::new(Computation::Literal { value: ScalarValue::Integer(7) }),
///     body: Box::new(Computation::Call { operation: 17, arguments: vec![ComputedArgument {
///         name: "position".into(), value: Computation::Local { name: "position".into() } }] }) };
/// let host = CallTypeHost { catalog: &catalog, version: 9, granted: &[31], environment: &Host };
/// let result = infer_prepared_call_type(&expression, &[], &Host, &host, CallTypeLimits {
///     pure: TypeInferenceLimits { max_nodes: 4, max_depth: 2, max_bindings: 1 }, max_arguments: 1 }).unwrap();
/// assert!(std::ptr::eq(result, &schemas[0].result));
/// ```
pub fn infer_prepared_call_type<
    'expression,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Schemas,
    Environment,
>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    bindings: &[(&'expression str, PureType<Environment::Result>)],
    environment: &Environment,
    schemas: &Schemas,
    limits: CallTypeLimits,
) -> Result<&'schema Schemas::Result, PreparedCallTypeError>
where
    Schemas: PreparedCallSchemas<'schema, Operation>,
    Environment: PureTypeEnvironment<Field, Operation>,
{
    let calls = preflight(expression, bindings, limits)?;
    let mut selected = Vec::with_capacity(calls.len());
    let mut common = None;
    for (call_index, call) in calls.iter().enumerate() {
        let Computation::Call {
            operation,
            arguments,
        } = call
        else {
            return Err(PreparedCallTypeError::InvalidFlow);
        };
        let schema = schemas
            .select(operation)
            .map_err(|error| PreparedCallTypeError::Call {
                call_index,
                error: CallTypeError::Catalog(error),
            })?;
        let names = arguments
            .iter()
            .map(|argument| argument.name.as_str())
            .collect::<Vec<_>>();
        preflight_signature(&names, schema, limits.max_arguments)
            .map_err(|error| PreparedCallTypeError::Call { call_index, error })?;
        if common.is_some_and(|expected| !std::ptr::eq(expected, schema)) {
            return Err(PreparedCallTypeError::InconsistentOperation);
        }
        common = Some(schema);
        selected.push((names, schema));
    }
    let mut locals = bindings.to_vec();
    let mut scope = ScopeFrame::new(&mut locals);
    let mut selections = Selections {
        calls: selected,
        next: 0,
    };
    let result = infer(expression, &mut scope, environment, &mut selections, limits)?;
    if selections.next != selections.calls.len() {
        return Err(PreparedCallTypeError::InvalidFlow);
    }
    Ok(result)
}
