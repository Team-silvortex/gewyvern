//! Bounded inference over the shared pure control IR, not an effect evaluator.

use std::fmt;

use leselang_runtime_core::{
    MAX_LOOP_ITERATIONS, MAX_STRING_LIST_ITEMS, ScalarType, ScopeFrame, StructureBudget,
    StructureError,
};

use crate::ir::Computation;

pub const MAX_TYPE_INFERENCE_NODES: usize = 16 * 1024;
pub const MAX_TYPE_INFERENCE_DEPTH: usize = 64;
pub const MAX_TYPE_INFERENCE_BINDINGS: usize = 1_024;
pub const MAX_LOCAL_NAME_BYTES: usize = 64;

/// Analysis output: a closed scalar type or host-owned result/signature metadata.
///
/// This is not a value, second expression tree, authority or saved frame. Native
/// result tags may be borrowed references, without cloning their payloads. There
/// is no wire codec or implicit default. Native Debug may expose private metadata.
///
/// ```compile_fail
/// use leselang_hir::pure_typing::PureType;
/// use leselang_runtime_core::ScalarType;
/// let ty = PureType::<()>::Scalar(ScalarType::Integer);
/// let wire = serde_json::to_string(&ty).unwrap();
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PureType<Result> {
    Scalar(ScalarType),
    Result(Result),
}

impl<Result> PureType<Result> {
    pub const fn scalar_type(&self) -> Option<ScalarType> {
        match self {
            Self::Scalar(ty) => Some(*ty),
            Self::Result(_) => None,
        }
    }
}

/// Trusted developer-owned field and closed group-member type queries.
///
/// Result metadata must retain stable type equality and interchangeable type-query
/// behavior for equal identifiers. Explicit joins may conservatively remove exports.
/// Borrowed result identifiers satisfy Clone without copying native payloads;
/// Field/Operation/effect/IR-result
/// slots need no Clone/Debug/serde/Send bounds. A member query must reject names or
/// operation tags not exported by this exact bound group, not a union of groups.
/// Queries/metadata cloning/equality may run code, allocate, mutate interior state
/// or unwind; the adapter bounds native work and owns redaction. They do not invoke
/// effects or confer version/capability/revision/receipt/dispatch authority.
pub trait PureTypeEnvironment<Field, Operation> {
    type Result: Clone + PartialEq;

    fn field_type(&self, result: &Self::Result, field: &Field) -> Option<ScalarType>;

    fn member_result(
        &self,
        group: &Self::Result,
        name: &str,
        operation: &Operation,
    ) -> Option<Self::Result>;

    /// Default exact type identity; hosts may explicitly join compatible result
    /// metadata. A join must retain only fields/members valid for both branches,
    /// never a union of exports. This is a native type query, not guard evaluation.
    fn join_results(&self, left: Self::Result, right: Self::Result) -> Option<Self::Result> {
        (left == right).then_some(left)
    }
}

/// Inclusive host policy, with fixed safety ceilings for the recursive type walk.
///
/// Zero nodes denies all; zero depth allows a leaf; zero bindings forbids locals.
/// No default expands limits. These bound physical IR, not source expansion,
/// evaluated value sizes, fuel, prior allocation or native query work.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TypeInferenceLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_bindings: usize,
}

/// Closed, payload-free static failures, not recoverable calculation faults.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PureTypeError {
    InvalidLimits,
    InvalidScope,
    Structure(StructureError),
    InvalidName,
    Impure,
    UnboundedLiteral,
    UnknownLocal,
    BindingLimit,
    ShadowedBinding,
    NonScalar,
    FieldNotExported,
    MemberNotExported,
    BinaryOperands,
    UnaryOperand,
    StringItem,
    StringListLimit,
    ConditionType,
    BranchTypes,
    RecoveryTypes,
    LoopLimit,
    LoopState,
    FoldLimit,
    FoldItems,
    FoldState,
}

impl fmt::Display for PureTypeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "type inference limits exceed safety ceilings",
            Self::InvalidScope => "type inference scope is invalid",
            Self::Structure(_) => "type inference structure exceeds its limits",
            Self::InvalidName => "type inference name is invalid",
            Self::Impure => "pure type inference does not accept host effects",
            Self::UnboundedLiteral => "type inference literal exceeds language bounds",
            Self::UnknownLocal => "type inference local is not bound",
            Self::BindingLimit => "type inference active binding limit exceeded",
            Self::ShadowedBinding => "type inference cannot shadow an active binding",
            Self::NonScalar => "type inference requires a scalar operand",
            Self::FieldNotExported => "field is not exported by this result type",
            Self::MemberNotExported => "member is not exported by this bound group",
            Self::BinaryOperands => "binary operation does not accept these scalar types",
            Self::UnaryOperand => "unary operation does not accept this scalar type",
            Self::StringItem => "string list entries must have string type",
            Self::StringListLimit => "string list entry count exceeds the language bound",
            Self::ConditionType => "condition must have boolean type",
            Self::BranchTypes => "conditional branches must have the same type",
            Self::RecoveryTypes => "recovery operands must have the same scalar type",
            Self::LoopLimit => "loop limit exceeds the language bound",
            Self::LoopState => "loop next must preserve its scalar state type",
            Self::FoldLimit => "fold limit exceeds the language bound",
            Self::FoldItems => "fold items must have string-list type",
            Self::FoldState => "fold next must preserve its scalar state type",
        })
    }
}

impl std::error::Error for PureTypeError {}

pub(crate) fn valid_local_name(name: &str) -> bool {
    let mut bytes = name.bytes();
    name.len() <= MAX_LOCAL_NAME_BYTES
        && bytes
            .next()
            .is_some_and(|byte| byte.is_ascii_alphabetic() || byte == b'_')
        && bytes.all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
        && !matches!(name, "body" | "fn" | "true" | "false" | "none")
}

pub(crate) fn valid_member_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= MAX_LOCAL_NAME_BYTES
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
}

pub(crate) fn preflight_with_budget<Field, Operation, HostEffect, IrResult>(
    expression: &Computation<Field, Operation, HostEffect, IrResult>,
    depth: usize,
    budget: &mut StructureBudget,
) -> Result<(), PureTypeError> {
    let mut pending = vec![(expression, depth)];
    while let Some((node, depth)) = pending.pop() {
        budget
            .visit(depth, 0, 0)
            .map_err(PureTypeError::Structure)?;
        match node {
            Computation::Literal { value } if !value.is_bounded() => {
                return Err(PureTypeError::UnboundedLiteral);
            }
            Computation::Strings { items } if items.len() > MAX_STRING_LIST_ITEMS => {
                return Err(PureTypeError::StringListLimit);
            }
            Computation::Local { name } | Computation::Bind { name, .. }
                if !valid_local_name(name) =>
            {
                return Err(PureTypeError::InvalidName);
            }
            Computation::Member { group, name, .. }
                if !valid_local_name(group) || !valid_member_name(name) =>
            {
                return Err(PureTypeError::InvalidName);
            }
            Computation::Loop { name, limit, .. } => {
                if !valid_local_name(name) || matches!(name.as_str(), "while" | "next" | "limit") {
                    return Err(PureTypeError::InvalidName);
                }
                if *limit > MAX_LOOP_ITERATIONS {
                    return Err(PureTypeError::LoopLimit);
                }
            }
            Computation::Fold {
                name, item, limit, ..
            } => {
                if !valid_local_name(name)
                    || !valid_local_name(item)
                    || name == item
                    || matches!(name.as_str(), "items" | "item" | "next" | "limit")
                {
                    return Err(PureTypeError::InvalidName);
                }
                if *limit > MAX_STRING_LIST_ITEMS as u64 {
                    return Err(PureTypeError::FoldLimit);
                }
            }
            Computation::Host { .. } | Computation::Call { .. } | Computation::Group { .. } => {
                return Err(PureTypeError::Impure);
            }
            _ => {}
        }
        for child in node.children().rev() {
            budget
                .check_pending(pending.len(), 1)
                .map_err(PureTypeError::Structure)?;
            pending.push((child, depth + 1));
        }
    }
    Ok(())
}

/// Infer all cold pure children using host-owned result metadata, without evaluation.
///
/// Limit validity, prefix count/names/uniqueness and the entire pure physical IR
/// are checked before any result metadata clone/equality or environment query.
/// Prefix/temporary metadata are owned by a local lexical guard; names are borrowed
/// and the caller's scope/IR are not mutated. All branches, recovery fallback,
/// short-circuit operands and zero-limit loop/fold bodies are type checked.
/// Neither arithmetic/parsing nor loop iterations execute; well-typed division by
/// zero remains a runtime failure. Host/Call/Group nodes are rejected, even cold.
///
/// This allocates bounded scope/frontier storage and clones only result identifiers.
/// Native callbacks/equality/Clone/drop are trusted, not preempted or rolled back.
/// Success is a type observation, not a lasting certificate, a full source lowerer,
/// execution permission, an effect/result validator or a saved continuation. Source
/// expansion, schema versions/authority and host effect/result acceptance are separate.
///
/// ```
/// use leselang_hir::{ir::Computation, pure_typing::*};
/// use leselang_runtime_core::{ScalarType, ScalarValue};
/// struct NoHost;
/// impl PureTypeEnvironment<(), ()> for NoHost {
///     type Result = ();
///     fn field_type(&self, _: &(), _: &()) -> Option<ScalarType> { None }
///     fn member_result(&self, _: &(), _: &str, _: &()) -> Option<()> { None }
/// }
/// let value: Computation<(), (), (), ()> = Computation::Literal { value: ScalarValue::Integer(7) };
/// let ty = infer_pure_type(&value, &[], &NoHost, TypeInferenceLimits {
///     max_nodes: 1, max_depth: 0, max_bindings: 0 }).unwrap();
/// assert_eq!(ty, PureType::Scalar(ScalarType::Integer));
/// ```
pub fn infer_pure_type<'names, Field, Operation, HostEffect, IrResult, Environment>(
    expression: &'names Computation<Field, Operation, HostEffect, IrResult>,
    bindings: &[(&'names str, PureType<Environment::Result>)],
    environment: &Environment,
    limits: TypeInferenceLimits,
) -> Result<PureType<Environment::Result>, PureTypeError>
where
    Environment: PureTypeEnvironment<Field, Operation>,
{
    preflight_scope(bindings, limits)?;
    let mut budget = StructureBudget::new(limits.max_nodes, limits.max_depth);
    preflight_with_budget(expression, 0, &mut budget)?;
    infer_preflighted(expression, bindings, environment, limits.max_bindings)
}

pub(crate) fn preflight_scope<ResultTag>(
    bindings: &[(&str, PureType<ResultTag>)],
    limits: TypeInferenceLimits,
) -> Result<(), PureTypeError> {
    if limits.max_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH
        || limits.max_bindings > MAX_TYPE_INFERENCE_BINDINGS
    {
        return Err(PureTypeError::InvalidLimits);
    }
    if bindings.len() > limits.max_bindings
        || bindings.iter().enumerate().any(|(index, (name, _))| {
            !valid_local_name(name)
                || bindings[..index]
                    .iter()
                    .any(|(previous, _)| previous == name)
        })
    {
        return Err(PureTypeError::InvalidScope);
    }
    Ok(())
}

// Only internal callers that preflighted the entire tree and prefix under fixed
// safety ceilings may skip that walk. No mutable IR escapes between these phases.
pub(crate) fn infer_preflighted<'names, Field, Operation, HostEffect, IrResult, Environment>(
    expression: &'names Computation<Field, Operation, HostEffect, IrResult>,
    bindings: &[(&'names str, PureType<Environment::Result>)],
    environment: &Environment,
    max_bindings: usize,
) -> Result<PureType<Environment::Result>, PureTypeError>
where
    Environment: PureTypeEnvironment<Field, Operation>,
{
    let mut locals = bindings.to_vec();
    let mut scope = ScopeFrame::new(&mut locals);
    infer(expression, &mut scope, environment, max_bindings)
}

// The caller has preflighted the complete tree and scope. Pure bindings are
// guarded inside infer, so an enclosing preparation frame can be reused safely.
pub(crate) fn infer_in_scope<'names, Field, Operation, HostEffect, IrResult, Environment>(
    expression: &'names Computation<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'names, PureType<Environment::Result>>,
    environment: &Environment,
    max_bindings: usize,
) -> Result<PureType<Environment::Result>, PureTypeError>
where
    Environment: PureTypeEnvironment<Field, Operation>,
{
    infer(expression, scope, environment, max_bindings)
}

fn scalar<ResultTag>(ty: PureType<ResultTag>) -> Result<ScalarType, PureTypeError> {
    ty.scalar_type().ok_or(PureTypeError::NonScalar)
}

fn bind<'names, ResultTag>(
    scope: &mut ScopeFrame<'_, 'names, PureType<ResultTag>>,
    name: &'names str,
    ty: PureType<ResultTag>,
    max_bindings: usize,
) -> Result<(), PureTypeError> {
    if scope.len() >= max_bindings {
        return Err(PureTypeError::BindingLimit);
    }
    scope
        .push(name, ty)
        .map(|_| ())
        .map_err(|_| PureTypeError::ShadowedBinding)
}

fn infer<'names, Field, Operation, HostEffect, IrResult, Environment>(
    expression: &'names Computation<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'names, PureType<Environment::Result>>,
    environment: &Environment,
    max_bindings: usize,
) -> Result<PureType<Environment::Result>, PureTypeError>
where
    Environment: PureTypeEnvironment<Field, Operation>,
{
    use Computation::*;
    let ty = match expression {
        Literal { value } => PureType::Scalar(value.scalar_type()),
        Strings { items } => {
            for item in items {
                if scalar(infer(item, scope, environment, max_bindings)?)? != ScalarType::String {
                    return Err(PureTypeError::StringItem);
                }
            }
            PureType::Scalar(ScalarType::StringList)
        }
        Local { name } => scope
            .get(name)
            .cloned()
            .ok_or(PureTypeError::UnknownLocal)?,
        Field { value, field } => {
            let PureType::Result(result) = infer(value, scope, environment, max_bindings)? else {
                return Err(PureTypeError::FieldNotExported);
            };
            PureType::Scalar(
                environment
                    .field_type(&result, field)
                    .ok_or(PureTypeError::FieldNotExported)?,
            )
        }
        Member {
            group,
            name,
            operation,
        } => {
            let Some(PureType::Result(group)) = scope.get(group) else {
                return Err(PureTypeError::MemberNotExported);
            };
            PureType::Result(
                environment
                    .member_result(group, name, operation)
                    .ok_or(PureTypeError::MemberNotExported)?,
            )
        }
        Binary {
            operator,
            left,
            right,
        } => {
            let left = scalar(infer(left, scope, environment, max_bindings)?)?;
            let right = scalar(infer(right, scope, environment, max_bindings)?)?;
            PureType::Scalar(
                operator
                    .result_type(left, right)
                    .ok_or(PureTypeError::BinaryOperands)?,
            )
        }
        Unary { operator, value } => {
            let input = scalar(infer(value, scope, environment, max_bindings)?)?;
            PureType::Scalar(
                operator
                    .result_type(input)
                    .ok_or(PureTypeError::UnaryOperand)?,
            )
        }
        Bind { name, value, body } => {
            if scope.get(name).is_some() {
                return Err(PureTypeError::ShadowedBinding);
            }
            let value = infer(value, scope, environment, max_bindings)?;
            let mut local = scope.nested();
            bind(&mut local, name, value, max_bindings)?;
            infer(body, &mut local, environment, max_bindings)?
        }
        Choose {
            when,
            then,
            otherwise,
        } => {
            if scalar(infer(when, scope, environment, max_bindings)?)? != ScalarType::Boolean {
                return Err(PureTypeError::ConditionType);
            }
            let then = infer(then, scope, environment, max_bindings)?;
            let otherwise = infer(otherwise, scope, environment, max_bindings)?;
            match (then, otherwise) {
                (PureType::Scalar(left), PureType::Scalar(right)) if left == right => {
                    PureType::Scalar(left)
                }
                (PureType::Result(left), PureType::Result(right)) => PureType::Result(
                    environment
                        .join_results(left, right)
                        .ok_or(PureTypeError::BranchTypes)?,
                ),
                _ => return Err(PureTypeError::BranchTypes),
            }
        }
        Recover { value, fallback } => {
            let value = infer(value, scope, environment, max_bindings)?;
            let fallback = infer(fallback, scope, environment, max_bindings)?;
            if value.scalar_type().is_none() || value != fallback {
                return Err(PureTypeError::RecoveryTypes);
            }
            value
        }
        Loop {
            name,
            initial,
            condition,
            next,
            ..
        } => {
            if scope.get(name).is_some() {
                return Err(PureTypeError::ShadowedBinding);
            }
            let initial = scalar(infer(initial, scope, environment, max_bindings)?)?;
            let mut local = scope.nested();
            bind(&mut local, name, PureType::Scalar(initial), max_bindings)?;
            if scalar(infer(condition, &mut local, environment, max_bindings)?)?
                != ScalarType::Boolean
            {
                return Err(PureTypeError::ConditionType);
            }
            if scalar(infer(next, &mut local, environment, max_bindings)?)? != initial {
                return Err(PureTypeError::LoopState);
            }
            PureType::Scalar(initial)
        }
        Fold {
            name,
            item,
            items,
            initial,
            next,
            ..
        } => {
            if scope.get(name).is_some() || scope.get(item).is_some() {
                return Err(PureTypeError::ShadowedBinding);
            }
            if scalar(infer(items, scope, environment, max_bindings)?)? != ScalarType::StringList {
                return Err(PureTypeError::FoldItems);
            }
            let initial = scalar(infer(initial, scope, environment, max_bindings)?)?;
            let mut local = scope.nested();
            bind(&mut local, name, PureType::Scalar(initial), max_bindings)?;
            bind(
                &mut local,
                item,
                PureType::Scalar(ScalarType::String),
                max_bindings,
            )?;
            if scalar(infer(next, &mut local, environment, max_bindings)?)? != initial {
                return Err(PureTypeError::FoldState);
            }
            PureType::Scalar(initial)
        }
        Host { .. } | Call { .. } | Group { .. } => return Err(PureTypeError::Impure),
    };
    Ok(ty)
}
