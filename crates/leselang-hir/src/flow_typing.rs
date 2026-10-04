//! Bounded call-result dataflow typing, not an evaluator or atomic-flow classifier.

use std::fmt;

use leselang_runtime_core::{ScalarType, ScopeFrame, StructureBudget, StructureError};

use crate::call_typing::{
    CallTypeError, CallTypeLimits, MAX_CALL_ARGUMENTS, check_in_scope,
    preflight_arguments_with_budget, preflight_signature,
};
use crate::ir::Computation;
use crate::ir::GroupKind;
use crate::prepared_typing::{
    PreparedCallSchemas, PreparedCallTypeError, SelectedPreparedCall,
    preflight_call_leaves_with_budget,
};
use crate::pure_typing::{
    MAX_TYPE_INFERENCE_NODES, PureType, PureTypeEnvironment, PureTypeError, infer_in_scope,
    preflight_scope, preflight_with_budget, valid_local_name,
};

/// Explicit host mapping from an original result declaration to query metadata.
///
/// This describes a declared type, not an actual reply or receipt. Implementations
/// must retain stable field/member/compatible-join behavior for equal identifiers.
/// Borrowed identifiers can reference nonclone native declarations. No effects or
/// result acceptance occur; native queries, clone/equality/drop may run code or
/// unwind, and their work/redaction remain the adapter's responsibility.
pub trait CallFlowEnvironment<'schema, Field, Operation, ResultDeclaration>:
    PureTypeEnvironment<Field, Operation>
{
    fn call_result_type(
        &self,
        operation: &Operation,
        declaration: &'schema ResultDeclaration,
    ) -> Option<PureType<Self::Result>>;
}

/// Inclusive physical limits. max_calls includes all cold call sites, not the
/// longest selected execution path, evaluation fuel or a dispatch reservation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CallFlowTypeLimits {
    pub call: CallTypeLimits,
    pub max_calls: usize,
}

pub const MAX_TYPED_GROUP_BRANCHES: usize = 64;

/// Ephemeral type-query inputs, not result values, receipts or another IR tree.
/// Names/operations borrow the original IR; inferred query metadata is moved in.
/// The host must retain only these declared exports, with exact operation identity.
///
/// ```compile_fail
/// use leselang_hir::{flow_typing::GroupMemberType, pure_typing::PureType};
/// use leselang_runtime_core::ScalarType;
/// let member = GroupMemberType { name: "private", operation: &17u32,
///     inferred: PureType::<u8>::Scalar(ScalarType::Integer) };
/// let wire = serde_json::to_string(&member).unwrap();
/// ```
pub struct GroupMemberType<'expression, Operation, ResultType> {
    pub name: &'expression str,
    pub operation: &'expression Operation,
    pub inferred: PureType<ResultType>,
}

/// Explicit native group typing. The declaration comparison does not inspect a
/// received reply. Construction must describe precisely the supplied ordered
/// members/mode, never union unrelated groups or publish partially checked members.
/// Native callbacks and query metadata remain trusted, not execution authority.
pub trait GroupFlowEnvironment<'expression, 'schema, Field, Operation, ResultDeclaration, IrResult>:
    CallFlowEnvironment<'schema, Field, Operation, ResultDeclaration>
{
    fn group_branch_type_matches(
        &self,
        declaration: &IrResult,
        inferred: &PureType<Self::Result>,
    ) -> bool;

    fn group_result_type(
        &self,
        kind: GroupKind,
        members: &[GroupMemberType<'expression, Operation, Self::Result>],
    ) -> Option<Self::Result>;
}

/// Explicit cold group-site and per-group member limits. These are physical
/// typing limits, not graph scheduling capacity, fuel or concurrency policy.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupFlowTypeLimits {
    pub flow: CallFlowTypeLimits,
    pub max_groups: usize,
    pub max_branches: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupTypeError {
    GroupLimit,
    Arity,
    InvalidName {
        branch_index: usize,
    },
    DuplicateName {
        branch_index: usize,
    },
    MemberPreparation {
        branch_index: usize,
        error: PreparedCallTypeError,
    },
    InconsistentOperation {
        branch_index: usize,
    },
    BranchType {
        branch_index: usize,
    },
    ResultType,
}

/// Closed source/declaration positions and tags, never private native payloads.
///
/// ```compile_fail
/// use leselang_hir::flow_typing::CallFlowTypeError;
/// let wire = serde_json::to_string(&CallFlowTypeError::UnsupportedFlow).unwrap();
/// ```
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CallFlowTypeError {
    InvalidLimits,
    UnsupportedFlow,
    CallLimit,
    Structure(StructureError),
    Pure(PureTypeError),
    ConditionType,
    BranchTypes,
    ResultType {
        call_index: usize,
    },
    Call {
        call_index: usize,
        error: CallTypeError,
    },
    Group {
        group_index: usize,
        error: GroupTypeError,
    },
}

impl fmt::Display for CallFlowTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidLimits => formatter.write_str("call flow limits exceed safety ceilings"),
            Self::UnsupportedFlow => {
                formatter.write_str("call flow does not adapt opaque effects or groups")
            }
            Self::CallLimit => formatter.write_str("call flow cold call count exceeds its limit"),
            Self::Structure(_) => formatter.write_str("call flow structure exceeds its limits"),
            Self::Pure(error) => error.fmt(formatter),
            Self::ConditionType => formatter.write_str("call flow condition must be boolean"),
            Self::BranchTypes => {
                formatter.write_str("call flow branches have incompatible result types")
            }
            Self::ResultType { .. } => {
                formatter.write_str("call result declaration has no query type")
            }
            Self::Call { error, .. } => error.fmt(formatter),
            Self::Group { .. } => {
                formatter.write_str("group type declaration or preparation is invalid")
            }
        }
    }
}

impl std::error::Error for CallFlowTypeError {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

// Only reference adapters with already bounded/canonical trees may use this
// routing predicate. It is not a shape/type/authority certificate.
pub(crate) fn call_flow_nodes_only<Field, Operation, HostEffect, IrResult>(
    expression: &Node<Field, Operation, HostEffect, IrResult>,
) -> bool {
    let mut pending = vec![expression];
    while let Some(node) = pending.pop() {
        if matches!(node, Computation::Host { .. } | Computation::Group { .. }) {
            return false;
        }
        pending.extend(node.children().rev());
    }
    true
}

pub(crate) fn group_flow_nodes_only<Field, Operation, HostEffect, IrResult>(
    expression: &Node<Field, Operation, HostEffect, IrResult>,
) -> bool {
    let mut pending = vec![expression];
    let mut group = false;
    while let Some(node) = pending.pop() {
        if matches!(node, Computation::Host { .. }) {
            return false;
        }
        group |= matches!(node, Computation::Group { .. });
        pending.extend(node.children().rev());
    }
    group
}

struct GroupShape {
    calls: Vec<std::ops::Range<usize>>,
}

struct Preflight<'expression, Field, Operation, HostEffect, IrResult> {
    calls: Vec<&'expression Node<Field, Operation, HostEffect, IrResult>>,
    groups: Vec<GroupShape>,
}

fn preflight<'expression, Field, Operation, HostEffect, IrResult, ResultTag>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    bindings: &[(&str, PureType<ResultTag>)],
    limits: CallFlowTypeLimits,
    group_limits: Option<GroupFlowTypeLimits>,
) -> Result<Preflight<'expression, Field, Operation, HostEffect, IrResult>, CallFlowTypeError> {
    if limits.call.max_arguments > MAX_CALL_ARGUMENTS || limits.max_calls > MAX_TYPE_INFERENCE_NODES
    {
        return Err(CallFlowTypeError::InvalidLimits);
    }
    preflight_scope(bindings, limits.call.pure).map_err(CallFlowTypeError::Pure)?;
    if group_limits.is_some_and(|limits| {
        limits.max_groups > MAX_TYPE_INFERENCE_NODES
            || limits.max_branches > MAX_TYPED_GROUP_BRANCHES
    }) {
        return Err(CallFlowTypeError::InvalidLimits);
    }
    let mut budget = StructureBudget::new(limits.call.pure.max_nodes, limits.call.pure.max_depth);
    let mut pending = vec![(expression, 0)];
    let mut calls = Vec::new();
    let mut groups = Vec::new();
    while let Some((node, depth)) = pending.pop() {
        match node {
            Computation::Call { arguments, .. } => {
                budget
                    .visit(depth, 0, 0)
                    .map_err(CallFlowTypeError::Structure)?;
                if calls.len() >= limits.max_calls {
                    return Err(CallFlowTypeError::CallLimit);
                }
                preflight_arguments_with_budget(
                    arguments,
                    depth + 1,
                    &mut budget,
                    limits.call.max_arguments,
                )
                .map_err(|error| CallFlowTypeError::Call {
                    call_index: calls.len(),
                    error,
                })?;
                calls.push(node);
            }
            Computation::Bind { name, value, body } => {
                budget
                    .visit(depth, 0, 0)
                    .map_err(CallFlowTypeError::Structure)?;
                if !valid_local_name(name) {
                    return Err(CallFlowTypeError::Pure(PureTypeError::InvalidName));
                }
                budget
                    .check_pending(pending.len(), 2)
                    .map_err(CallFlowTypeError::Structure)?;
                pending.push((body, depth + 1));
                pending.push((value, depth + 1));
            }
            Computation::Choose {
                when,
                then,
                otherwise,
            } => {
                budget
                    .visit(depth, 0, 0)
                    .map_err(CallFlowTypeError::Structure)?;
                preflight_with_budget(when, depth + 1, &mut budget)
                    .map_err(CallFlowTypeError::Pure)?;
                budget
                    .check_pending(pending.len(), 2)
                    .map_err(CallFlowTypeError::Structure)?;
                pending.push((otherwise, depth + 1));
                pending.push((then, depth + 1));
            }
            Computation::Group {
                group_kind,
                branches,
            } => {
                let Some(group_limits) = group_limits else {
                    return Err(CallFlowTypeError::UnsupportedFlow);
                };
                budget
                    .visit(depth, 0, 0)
                    .map_err(CallFlowTypeError::Structure)?;
                let group_index = groups.len();
                let failure = |error| CallFlowTypeError::Group { group_index, error };
                if group_index >= group_limits.max_groups {
                    return Err(failure(GroupTypeError::GroupLimit));
                }
                let minimum = if *group_kind == GroupKind::Sequence {
                    1
                } else {
                    2
                };
                if branches.len() < minimum || branches.len() > group_limits.max_branches {
                    return Err(failure(GroupTypeError::Arity));
                }
                for (branch_index, branch) in branches.iter().enumerate() {
                    if branch.name.is_empty()
                        || branch.name.len() > crate::pure_typing::MAX_LOCAL_NAME_BYTES
                        || !branch
                            .name
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                    {
                        return Err(failure(GroupTypeError::InvalidName { branch_index }));
                    }
                    if branches[..branch_index]
                        .iter()
                        .any(|previous| previous.name == branch.name)
                    {
                        return Err(failure(GroupTypeError::DuplicateName { branch_index }));
                    }
                }
                let mut ranges = Vec::with_capacity(branches.len());
                for (branch_index, branch) in branches.iter().enumerate() {
                    let start = calls.len();
                    let prepared = preflight_call_leaves_with_budget(
                        &branch.value,
                        depth + 1,
                        &mut budget,
                        limits.call.max_arguments,
                    )
                    .map_err(|error| match error {
                        PreparedCallTypeError::Call { call_index, error } => {
                            CallFlowTypeError::Call {
                                call_index: start + call_index,
                                error,
                            }
                        }
                        error => failure(GroupTypeError::MemberPreparation {
                            branch_index,
                            error,
                        }),
                    })?;
                    if calls
                        .len()
                        .checked_add(prepared.len())
                        .is_none_or(|count| count > limits.max_calls)
                    {
                        return Err(CallFlowTypeError::CallLimit);
                    }
                    calls.extend(prepared);
                    ranges.push(start..calls.len());
                }
                groups.push(GroupShape { calls: ranges });
            }
            Computation::Host { .. } => {
                return Err(CallFlowTypeError::UnsupportedFlow);
            }
            _ => {
                preflight_with_budget(node, depth, &mut budget).map_err(CallFlowTypeError::Pure)?
            }
        }
    }
    Ok(Preflight { calls, groups })
}

struct SelectedCall<
    'expression,
    'schema,
    Operation,
    Schemas: PreparedCallSchemas<'schema, Operation>,
> {
    names: Vec<&'expression str>,
    operation: &'expression Operation,
    schema: SelectedPreparedCall<'schema, Operation, Schemas>,
}

struct Selections<'expression, 'schema, Operation, Schemas: PreparedCallSchemas<'schema, Operation>>
{
    calls: Vec<SelectedCall<'expression, 'schema, Operation, Schemas>>,
    next: usize,
    groups: Vec<GroupShape>,
    next_group: usize,
}

trait GroupPolicy<'expression, Operation, IrResult, ResultType> {
    fn matches(&self, declaration: &IrResult, inferred: &PureType<ResultType>) -> bool;
    fn construct(
        &self,
        kind: GroupKind,
        members: &[GroupMemberType<'expression, Operation, ResultType>],
    ) -> Option<ResultType>;
}

struct NoGroups;
impl<'expression, Operation, IrResult, ResultType>
    GroupPolicy<'expression, Operation, IrResult, ResultType> for NoGroups
{
    fn matches(&self, _: &IrResult, _: &PureType<ResultType>) -> bool {
        false
    }
    fn construct(
        &self,
        _: GroupKind,
        _: &[GroupMemberType<'expression, Operation, ResultType>],
    ) -> Option<ResultType> {
        None
    }
}

struct GroupCallbacks<Matches, Construct> {
    matches: Matches,
    construct: Construct,
}
impl<'expression, Operation, IrResult, ResultType, Matches, Construct>
    GroupPolicy<'expression, Operation, IrResult, ResultType> for GroupCallbacks<Matches, Construct>
where
    Matches: Fn(&IrResult, &PureType<ResultType>) -> bool,
    Construct:
        Fn(GroupKind, &[GroupMemberType<'expression, Operation, ResultType>]) -> Option<ResultType>,
{
    fn matches(&self, declaration: &IrResult, inferred: &PureType<ResultType>) -> bool {
        (self.matches)(declaration, inferred)
    }
    fn construct(
        &self,
        kind: GroupKind,
        members: &[GroupMemberType<'expression, Operation, ResultType>],
    ) -> Option<ResultType> {
        (self.construct)(kind, members)
    }
}

fn infer<
    'expression,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Schemas,
    Environment,
    Groups,
>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'expression, PureType<Environment::Result>>,
    environment: &Environment,
    selections: &mut Selections<'expression, 'schema, Operation, Schemas>,
    limits: CallFlowTypeLimits,
    groups: &Groups,
) -> Result<PureType<Environment::Result>, CallFlowTypeError>
where
    Schemas: PreparedCallSchemas<'schema, Operation>,
    Environment: CallFlowEnvironment<'schema, Field, Operation, Schemas::Result>,
    Groups: GroupPolicy<'expression, Operation, IrResult, Environment::Result>,
{
    match expression {
        Computation::Call {
            operation,
            arguments,
        } => {
            let call_index = selections.next;
            let Some(selected) = selections.calls.get(call_index) else {
                return Err(CallFlowTypeError::UnsupportedFlow);
            };
            selections.next += 1;
            let result = check_in_scope(
                arguments,
                &selected.names,
                selected.schema,
                scope,
                environment,
                limits.call,
            )
            .map_err(|error| CallFlowTypeError::Call { call_index, error })?;
            environment
                .call_result_type(operation, result)
                .ok_or(CallFlowTypeError::ResultType { call_index })
        }
        Computation::Bind { name, value, body } => {
            if scope.get(name).is_some() {
                return Err(CallFlowTypeError::Pure(PureTypeError::ShadowedBinding));
            }
            if scope.len() >= limits.call.pure.max_bindings {
                return Err(CallFlowTypeError::Pure(PureTypeError::BindingLimit));
            }
            let ty = infer(value, scope, environment, selections, limits, groups)?;
            let mut local = scope.nested();
            local
                .push(name, ty)
                .map_err(|_| CallFlowTypeError::Pure(PureTypeError::ShadowedBinding))?;
            infer(body, &mut local, environment, selections, limits, groups)
        }
        Computation::Choose {
            when,
            then,
            otherwise,
        } => {
            let when = infer_in_scope(when, scope, environment, limits.call.pure.max_bindings)
                .map_err(CallFlowTypeError::Pure)?;
            if when.scalar_type() != Some(ScalarType::Boolean) {
                return Err(CallFlowTypeError::ConditionType);
            }
            let then = infer(then, scope, environment, selections, limits, groups)?;
            let otherwise = infer(otherwise, scope, environment, selections, limits, groups)?;
            match (then, otherwise) {
                (PureType::Scalar(left), PureType::Scalar(right)) if left == right => {
                    Ok(PureType::Scalar(left))
                }
                (PureType::Result(left), PureType::Result(right)) => environment
                    .join_results(left, right)
                    .map(PureType::Result)
                    .ok_or(CallFlowTypeError::BranchTypes),
                _ => Err(CallFlowTypeError::BranchTypes),
            }
        }
        Computation::Group {
            group_kind,
            branches,
        } => {
            let group_index = selections.next_group;
            selections.next_group += 1;
            let mut members = Vec::with_capacity(branches.len());
            for (branch_index, branch) in branches.iter().enumerate() {
                let Some(range) = selections
                    .groups
                    .get(group_index)
                    .and_then(|shape| shape.calls.get(branch_index))
                else {
                    return Err(CallFlowTypeError::UnsupportedFlow);
                };
                let start = range.start;
                if selections.next != start {
                    return Err(CallFlowTypeError::UnsupportedFlow);
                }
                let Some(selected) = selections.calls.get(start) else {
                    return Err(CallFlowTypeError::UnsupportedFlow);
                };
                let operation = selected.operation;
                let inferred = infer(
                    &branch.value,
                    scope,
                    environment,
                    selections,
                    limits,
                    groups,
                )?;
                if !groups.matches(&branch.result_type, &inferred) {
                    return Err(CallFlowTypeError::Group {
                        group_index,
                        error: GroupTypeError::BranchType { branch_index },
                    });
                }
                members.push(GroupMemberType {
                    name: &branch.name,
                    operation,
                    inferred,
                });
            }
            groups
                .construct(*group_kind, &members)
                .map(PureType::Result)
                .ok_or(CallFlowTypeError::Group {
                    group_index,
                    error: GroupTypeError::ResultType,
                })
        }
        Computation::Host { .. } => Err(CallFlowTypeError::UnsupportedFlow),
        _ => infer_in_scope(
            expression,
            scope,
            environment,
            limits.call.pure.max_bindings,
        )
        .map_err(CallFlowTypeError::Pure),
    }
}

/// Infer pure values and call-result Bind/Choose chains over the original IR.
///
/// The whole physical tree/prefix is preflighted before native callbacks. Every
/// cold call schema/version/capability/named shape is then checked before prefix
/// cloning, argument queries or result-type mapping. A single guarded scope is
/// reused for declared results, pure aliases and sibling branches. Guards and
/// argument/field/operator/loop/fold/recovery operands remain pure, even cold.
///
/// Unlike atomic preparation, this general type observation can contain multiple
/// calls and compatible distinct operation results. It does not classify which
/// flows a source profile, evaluator or atomic/group scheduler may execute. Native
/// result joins must retain only common exports. Newly constructed groups and
/// opaque Host leaves are not adapted; externally declared group metadata can
/// still be queried by the existing closed PureTypeEnvironment member protocol.
///
/// Success is neither an evaluated value nor host-reply validation, an execution
/// grant, prepared request, saved continuation or durable journal. IR/payloads are
/// not copied and native work/unwind is trusted, not sandboxed or rolled back.
///
/// ```
/// use leselang_hir::{ir::Computation, call_typing::*, pure_typing::*, flow_typing::*};
/// use leselang_runtime_core::*;
/// struct Host;
/// impl PureTypeEnvironment<(), u32> for Host {
///     type Result = ();
///     fn field_type(&self, _: &(), _: &()) -> Option<ScalarType> { None }
///     fn member_result(&self, _: &(), _: &str, _: &u32) -> Option<()> { None }
/// }
/// impl<'s> CallFlowEnvironment<'s, (), u32, u8> for Host {
///     fn call_result_type(&self, _: &u32, _: &'s u8) -> Option<PureType<()>> { Some(PureType::Result(())) }
/// }
/// let rows = [OperationSchema { key: 17u32,
///     parameters: &[] as &[NamedParameter<&str, ScalarTypeSet>], result: 1u8, required_capability: () }];
/// let catalog = OperationCatalog::new(9, &rows, OperationCatalogLimits {
///     max_operations: 1, max_parameters_per_operation: 0 }).unwrap();
/// let host = CallTypeHost { catalog: &catalog, version: 9, granted: &[()], environment: &Host };
/// let expression: Computation<(), u32, (), ()> = Computation::Bind {
///     name: "receipt".into(), value: Box::new(Computation::Call { operation: 17, arguments: vec![] }),
///     body: Box::new(Computation::Literal { value: ScalarValue::Boolean(true) }) };
/// let ty = infer_call_flow_type(&expression, &[], &Host, &host, CallFlowTypeLimits {
///     call: CallTypeLimits { pure: TypeInferenceLimits { max_nodes: 3, max_depth: 1, max_bindings: 1 },
///         max_arguments: 0 }, max_calls: 1 }).unwrap();
/// assert_eq!(ty, PureType::Scalar(ScalarType::Boolean));
/// ```
pub fn infer_call_flow_type<
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
    limits: CallFlowTypeLimits,
) -> Result<PureType<Environment::Result>, CallFlowTypeError>
where
    Schemas: PreparedCallSchemas<'schema, Operation>,
    Environment: CallFlowEnvironment<'schema, Field, Operation, Schemas::Result>,
{
    infer_with_groups(
        expression,
        bindings,
        environment,
        schemas,
        limits,
        None,
        &NoGroups,
    )
}

fn infer_with_groups<
    'expression,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Schemas,
    Environment,
    Groups,
>(
    expression: &'expression Node<Field, Operation, HostEffect, IrResult>,
    bindings: &[(&'expression str, PureType<Environment::Result>)],
    environment: &Environment,
    schemas: &Schemas,
    limits: CallFlowTypeLimits,
    group_limits: Option<GroupFlowTypeLimits>,
    groups: &Groups,
) -> Result<PureType<Environment::Result>, CallFlowTypeError>
where
    Schemas: PreparedCallSchemas<'schema, Operation>,
    Environment: CallFlowEnvironment<'schema, Field, Operation, Schemas::Result>,
    Groups: GroupPolicy<'expression, Operation, IrResult, Environment::Result>,
{
    let physical = preflight(expression, bindings, limits, group_limits)?;
    let mut selected = Vec::with_capacity(physical.calls.len());
    for (call_index, call) in physical.calls.iter().enumerate() {
        let Computation::Call {
            operation,
            arguments,
        } = call
        else {
            return Err(CallFlowTypeError::UnsupportedFlow);
        };
        let schema = schemas
            .select(operation)
            .map_err(|error| CallFlowTypeError::Call {
                call_index,
                error: CallTypeError::Catalog(error),
            })?;
        let names = arguments
            .iter()
            .map(|argument| argument.name.as_str())
            .collect::<Vec<_>>();
        preflight_signature(&names, schema, limits.call.max_arguments)
            .map_err(|error| CallFlowTypeError::Call { call_index, error })?;
        selected.push(SelectedCall::<Operation, Schemas> {
            names,
            operation,
            schema,
        });
    }
    for (group_index, group) in physical.groups.iter().enumerate() {
        for (branch_index, range) in group.calls.iter().enumerate() {
            let Some(calls) = selected.get(range.clone()) else {
                return Err(CallFlowTypeError::UnsupportedFlow);
            };
            let Some(first) = calls.first() else {
                return Err(CallFlowTypeError::UnsupportedFlow);
            };
            if calls
                .iter()
                .any(|call| !std::ptr::eq(first.schema, call.schema))
            {
                return Err(CallFlowTypeError::Group {
                    group_index,
                    error: GroupTypeError::InconsistentOperation { branch_index },
                });
            }
        }
    }
    let mut locals = bindings.to_vec();
    let mut scope = ScopeFrame::new(&mut locals);
    let mut selections = Selections::<Operation, Schemas> {
        calls: selected,
        next: 0,
        groups: physical.groups,
        next_group: 0,
    };
    let result = infer(
        expression,
        &mut scope,
        environment,
        &mut selections,
        limits,
        groups,
    )?;
    if selections.next != selections.calls.len() || selections.next_group != selections.groups.len()
    {
        return Err(CallFlowTypeError::UnsupportedFlow);
    }
    Ok(result)
}

/// Infer call-result dataflow plus flat named groups, with explicit host exports.
///
/// Sequence requires at least one member, Parallel at least two; both use explicit
/// bounded unique ASCII member names and caller limits. Each member has pure
/// Bind/Choose preparation ending in exactly one uniform original call declaration.
/// Nested groups, captured/multiple member effects and scalar exits are rejected.
/// All cold schemas/named shapes and per-member row identity finish before type
/// cloning or any field/result/declaration/group-construction query.
///
/// Members share only the enclosing prefix: neither siblings' locals nor earlier
/// sequence receipts are bound. The host checks every original branch result
/// declaration against its inferred type before constructing one ordered closed
/// group query identifier. This observes all potential member types, not partial
/// completion, actual replies, execution order or parallel scheduling. Native
/// joins must retain only common exports; profile-specific stricter source and
/// graph rules, authority, result acceptance, evaluation and persistence are separate.
///
/// The call-only entry remains group-rejecting. IR/result declarations are borrowed
/// without Clone/Debug/serde/Send bounds; only native query identifiers may clone.
///
/// ```
/// use leselang_hir::{ir::{Computation, ComputedBranch, GroupKind}, call_typing::*, pure_typing::*, flow_typing::*};
/// use leselang_runtime_core::*;
/// struct Host;
/// impl PureTypeEnvironment<(), u32> for Host {
///     type Result = u8;
///     fn field_type(&self, _: &u8, _: &()) -> Option<ScalarType> { None }
///     fn member_result(&self, group: &u8, name: &str, operation: &u32) -> Option<u8> {
///         (*group == 2 && name == "window" && *operation == 17).then_some(1)
///     }
/// }
/// impl<'s> CallFlowEnvironment<'s, (), u32, u8> for Host {
///     fn call_result_type(&self, _: &u32, result: &'s u8) -> Option<PureType<u8>> { Some(PureType::Result(*result)) }
/// }
/// impl<'e, 's> GroupFlowEnvironment<'e, 's, (), u32, u8, u8> for Host {
///     fn group_branch_type_matches(&self, declared: &u8, ty: &PureType<u8>) -> bool { matches!(ty, PureType::Result(result) if result == declared) }
///     fn group_result_type(&self, _: GroupKind, members: &[GroupMemberType<'e, u32, u8>]) -> Option<u8> {
///         (members.len() == 1 && members[0].name == "window" && *members[0].operation == 17).then_some(2)
///     }
/// }
/// let rows = [OperationSchema { key: 17u32, parameters: &[] as &[NamedParameter<&str, ScalarTypeSet>], result: 1u8, required_capability: () }];
/// let catalog = OperationCatalog::new(9, &rows, OperationCatalogLimits { max_operations: 1, max_parameters_per_operation: 0 }).unwrap();
/// let host = CallTypeHost { catalog: &catalog, version: 9, granted: &[()], environment: &Host };
/// let expression: Computation<(), u32, (), u8> = Computation::Group { group_kind: GroupKind::Sequence,
///     branches: vec![ComputedBranch { name: "window".into(), value: Computation::Call { operation: 17, arguments: vec![] }, result_type: 1 }] };
/// let ty = infer_group_flow_type(&expression, &[], &Host, &host, GroupFlowTypeLimits {
///     flow: CallFlowTypeLimits { call: CallTypeLimits { pure: TypeInferenceLimits { max_nodes: 2, max_depth: 1, max_bindings: 0 }, max_arguments: 0 }, max_calls: 1 },
///     max_groups: 1, max_branches: 1 }).unwrap();
/// assert_eq!(ty, PureType::Result(2));
/// ```
pub fn infer_group_flow_type<
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
    limits: GroupFlowTypeLimits,
) -> Result<PureType<Environment::Result>, CallFlowTypeError>
where
    Schemas: PreparedCallSchemas<'schema, Operation>,
    Environment:
        GroupFlowEnvironment<'expression, 'schema, Field, Operation, Schemas::Result, IrResult>,
{
    let policy = GroupCallbacks {
        matches: |declaration: &IrResult, inferred: &PureType<Environment::Result>| {
            environment.group_branch_type_matches(declaration, inferred)
        },
        construct:
            |kind: GroupKind,
             members: &[GroupMemberType<'expression, Operation, Environment::Result>]| {
                environment.group_result_type(kind, members)
            },
    };
    infer_with_groups(
        expression,
        bindings,
        environment,
        schemas,
        limits.flow,
        Some(limits),
        &policy,
    )
}
