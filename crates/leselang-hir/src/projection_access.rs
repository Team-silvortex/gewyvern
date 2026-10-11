//! Bounded, cold-branch projection observations with explicit host policy.

use std::fmt;

use leselang_runtime_core::{ScopeFrame, StructureBudget, StructureError};

use crate::ir::Computation;
use crate::pure_typing::{MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProjectionAccessLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProjectionAccessPhase {
    Local,
    Field,
    Host,
    Call,
    Member,
    Join,
}

pub enum ProjectionAccessError<Error> {
    InvalidLimits,
    Structure(StructureError),
    DuplicateBinding,
    Native {
        phase: ProjectionAccessPhase,
        error: Error,
    },
}

impl<Error> fmt::Display for ProjectionAccessError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "projection access limits exceed safety ceilings",
            Self::Structure(_) => "projection access IR exceeds physical limits",
            Self::DuplicateBinding => "projection access binding collides with a visible name",
            Self::Native { .. } => "native projection access policy refused the observation",
        })
    }
}

impl<Error> fmt::Debug for ProjectionAccessError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl<Error> std::error::Error for ProjectionAccessError<Error> {}

/// Native observations are not inferred schemas, field grants or execution authority.
///
/// Every method is explicit. In particular, local observations need not be Clone:
/// the host may borrow native data or produce a new move-only observation. A missing
/// local and a scalar-only slot both supply None; complete name/type validation
/// belongs to the adapter. Member fallback and conditional joins are host policy.
/// Call arguments and both conditional branches are observed before their hooks.
/// Native work/interior state/destructors are trusted, not preempted or rolled back.
pub trait ProjectionAccessPolicy<'expression, Field, Operation, HostEffect> {
    type Observation;
    type Error;

    fn local(
        &mut self,
        value: Option<&Self::Observation>,
    ) -> Result<Option<Self::Observation>, Self::Error>;
    fn field(
        &mut self,
        value: Option<&Self::Observation>,
        field: &'expression Field,
    ) -> Result<(), Self::Error>;
    fn host(
        &mut self,
        effect: &'expression HostEffect,
    ) -> Result<Option<Self::Observation>, Self::Error>;
    fn call(
        &mut self,
        operation: &'expression Operation,
    ) -> Result<Option<Self::Observation>, Self::Error>;
    fn member(
        &mut self,
        value: Option<&Self::Observation>,
        group: &'expression str,
        name: &'expression str,
        operation: &'expression Operation,
    ) -> Result<Option<Self::Observation>, Self::Error>;
    fn join(
        &mut self,
        left: Option<Self::Observation>,
        right: Option<Self::Observation>,
    ) -> Result<Option<Self::Observation>, Self::Error>;
}

/// Observe all physical control nodes without evaluating or dispatching host work.
///
/// An iterative node/depth preflight, including cold branches, completes before any
/// policy hook. Inclusive limits are capped by the HIR safety ceilings. No scalar
/// bounds, name grammar, native payloads, canonical types or receipts are validated.
/// Borrowed original names share the supplied scope; Bind exports its body result,
/// while loop/fold names are scalar-only and export no projection. Recover and
/// groups observe all children but export none. Only Choose invokes the native join.
/// Temporary suffixes are restored on success, refusal and unwind, not native
/// interior mutations. Outputs/errors stay local until suffix cleanup completes.
/// Success is an instant observation, not a cached certificate or execution grant.
pub fn observe_projection_accesses<'expression, Field, Operation, HostEffect, IrResult, Policy>(
    expression: &'expression Computation<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'expression, Option<Policy::Observation>>,
    limits: ProjectionAccessLimits,
    policy: &mut Policy,
) -> Result<Option<Policy::Observation>, ProjectionAccessError<Policy::Error>>
where
    Policy: ProjectionAccessPolicy<'expression, Field, Operation, HostEffect>,
{
    if limits.max_nodes > MAX_TYPE_INFERENCE_NODES || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH {
        return Err(ProjectionAccessError::InvalidLimits);
    }
    let mut budget = StructureBudget::new(limits.max_nodes, limits.max_depth);
    let mut pending = vec![(expression, 0)];
    while let Some((node, depth)) = pending.pop() {
        budget
            .visit(depth, 0, 0)
            .map_err(ProjectionAccessError::Structure)?;
        for child in node.children() {
            budget
                .check_pending(pending.len(), 1)
                .map_err(ProjectionAccessError::Structure)?;
            pending.push((child, depth + 1));
        }
    }
    walk(expression, scope, policy)
}

fn native<Value, Error>(
    phase: ProjectionAccessPhase,
    value: Result<Value, Error>,
) -> Result<Value, ProjectionAccessError<Error>> {
    value.map_err(|error| ProjectionAccessError::Native { phase, error })
}

fn walk<'expression, Field, Operation, HostEffect, IrResult, Policy>(
    expression: &'expression Computation<Field, Operation, HostEffect, IrResult>,
    scope: &mut ScopeFrame<'_, 'expression, Option<Policy::Observation>>,
    policy: &mut Policy,
) -> Result<Option<Policy::Observation>, ProjectionAccessError<Policy::Error>>
where
    Policy: ProjectionAccessPolicy<'expression, Field, Operation, HostEffect>,
{
    use Computation::*;
    use ProjectionAccessPhase as Phase;
    match expression {
        Local { name } => native(
            Phase::Local,
            policy.local(scope.get(name).and_then(Option::as_ref)),
        ),
        Field { value, field } => {
            let value = walk(value, scope, policy)?;
            let output = native(Phase::Field, policy.field(value.as_ref(), field)).map(|_| None);
            drop(value);
            output
        }
        Bind { name, value, body } => {
            let value = walk(value, scope, policy)?;
            let mut local = scope.nested();
            let output = (|| {
                local
                    .push(name, value)
                    .map_err(|_| ProjectionAccessError::DuplicateBinding)?;
                walk(body, &mut local, policy)
            })();
            drop(local);
            output
        }
        Choose {
            when,
            then,
            otherwise,
        } => {
            walk(when, scope, policy)?;
            let left = walk(then, scope, policy)?;
            let right = match walk(otherwise, scope, policy) {
                Ok(right) => right,
                Err(error) => {
                    drop(left);
                    return Err(error);
                }
            };
            native(Phase::Join, policy.join(left, right))
        }
        Fold {
            name,
            item,
            items,
            initial,
            next,
            ..
        } => {
            walk(items, scope, policy)?;
            walk(initial, scope, policy)?;
            let mut local = scope.nested();
            let output = (|| {
                local
                    .push(name, None)
                    .map_err(|_| ProjectionAccessError::DuplicateBinding)?;
                local
                    .push(item, None)
                    .map_err(|_| ProjectionAccessError::DuplicateBinding)?;
                walk(next, &mut local, policy).map(|_| None)
            })();
            drop(local);
            output
        }
        Loop {
            name,
            initial,
            condition,
            next,
            ..
        } => {
            walk(initial, scope, policy)?;
            let mut local = scope.nested();
            let output = (|| {
                local
                    .push(name, None)
                    .map_err(|_| ProjectionAccessError::DuplicateBinding)?;
                walk(condition, &mut local, policy)?;
                walk(next, &mut local, policy).map(|_| None)
            })();
            drop(local);
            output
        }
        Host { effect } => native(Phase::Host, policy.host(effect)),
        Call {
            operation,
            arguments,
        } => {
            for argument in arguments {
                walk(&argument.value, scope, policy)?;
            }
            native(Phase::Call, policy.call(operation))
        }
        Member {
            group,
            name,
            operation,
        } => native(
            Phase::Member,
            policy.member(
                scope.get(group).and_then(Option::as_ref),
                group,
                name,
                operation,
            ),
        ),
        Literal { .. } => Ok(None),
        Strings { .. } | Binary { .. } | Recover { .. } | Unary { .. } | Group { .. } => {
            for child in expression.children() {
                walk(child, scope, policy)?;
            }
            Ok(None)
        }
    }
}
