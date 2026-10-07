//! Shared recursive Bind/Choose source composition with explicit native leaves.

use std::fmt;

use leselang_runtime_core::{ScalarType, ScalarValue, ScopeFrame, StructureBudget, StructureError};
use leselang_syntax::{Expression, Span};

use crate::binding_source::physical;
use crate::choice_source::{ChoiceSourceError, cold_names, operands};
use crate::ir::Computation;
use crate::pure_typing::{
    MAX_TYPE_INFERENCE_BINDINGS, PureTypeError, preflight_with_budget, valid_local_name,
};
use crate::scalar_source::{
    ScalarSourceError, binding_source, preflight_bindings, preflight_names,
};
use crate::source_call::{SourceCallError, SourceCallLimits, preflight, span, valid_limits};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ControlSourceLimits {
    pub source: SourceCallLimits,
    pub max_bindings: usize,
}

/// Borrowed native type observations, not runtime locals or grants. No native
/// type is copied, formatted, installed into a host frame or allowed to escape.
pub struct ControlSourceScope<'scope, 'names, Type> {
    bindings: &'scope [(&'names str, &'scope Type)],
}
impl<'scope, 'names, Type> ControlSourceScope<'scope, 'names, Type> {
    pub fn get(&self, name: &str) -> Option<&'scope Type> {
        self.bindings
            .iter()
            .find(|(bound, _)| *bound == name)
            .map(|(_, ty)| *ty)
    }
    pub fn bindings(&self) -> &'scope [(&'names str, &'scope Type)] {
        self.bindings
    }
    pub fn len(&self) -> usize {
        self.bindings.len()
    }
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}
impl<Type> fmt::Debug for ControlSourceScope<'_, '_, Type> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ControlSourceScope")
            .field("bindings", &self.len())
            .finish()
    }
}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type ControlSourceOperand<Node, Type, Error> = Result<(Node, Type), Error>;

/// Native leaves and mandatory whole-output type/schema/capture admission.
/// Leaves include scalar forms, fields, calls, groups and helpers; this entry
/// only recursively owns source Bind/Choose at its own control positions.
/// Native leaf compilers must enforce their cold schemas, lexical/type policy,
/// hidden expansions and work/allocation bounds. Admission must corroborate the
/// entire original output and observations, including unfinished nested captures,
/// versions/grants, opaque graphs, group exports and result-view compatibility.
/// None of these callbacks may execute effects or imply reply acceptance.
pub trait ControlSourceAdapter<'source, Field, Operation, HostEffect, IrResult> {
    type Type;
    type Error;

    fn lower_leaf(
        &mut self,
        source: &'source Expression,
        scope: ControlSourceScope<'_, 'source, Self::Type>,
    ) -> ControlSourceOperand<Node<Field, Operation, HostEffect, IrResult>, Self::Type, Self::Error>;
    fn boolean(
        &mut self,
        value: &Node<Field, Operation, HostEffect, IrResult>,
        observed: &Self::Type,
    ) -> Result<bool, Self::Error>;
    fn same(&mut self, left: &Self::Type, right: &Self::Type) -> Result<bool, Self::Error>;
    fn admit(
        &mut self,
        source: &'source Expression,
        value: &Node<Field, Operation, HostEffect, IrResult>,
        observed: &Self::Type,
        scope: ControlSourceScope<'_, 'source, Self::Type>,
    ) -> Result<(), Self::Error>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlSourcePhase {
    Leaf,
    Condition,
    Compare,
    Admit,
}
pub enum ControlSourceError<Error> {
    Source(SourceCallError<Error>),
    Lexical(ScalarSourceError<Error>),
    Choice(ChoiceSourceError<Error>),
    Projection(crate::projection_source::ProjectionSourceError<Error>),
    Output {
        span: Span,
        error: StructureError,
    },
    Condition {
        span: Span,
    },
    ProducedCondition {
        span: Span,
        error: PureTypeError,
    },
    BranchTypes {
        span: Span,
    },
    Native {
        span: Span,
        phase: ControlSourcePhase,
        error: Error,
    },
}
impl<Error> fmt::Display for ControlSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Source(_) => "control source bounds are invalid",
            Self::Lexical(_) => "control source lexical or scalar shape is invalid",
            Self::Choice(_) => "control source choice signature is invalid",
            Self::Projection(_) => "control source projection signature is invalid",
            Self::Output { .. } => "control source output exceeds physical limits",
            Self::Condition { .. } => "control source condition must be pure and boolean",
            Self::ProducedCondition { .. } => "control source condition is not bounded pure IR",
            Self::BranchTypes { .. } => "control source branches have different native types",
            Self::Native { .. } => "control source native adapter failed",
        })
    }
}
impl<Error> fmt::Debug for ControlSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for ControlSourceError<Error> {}
pub type ControlSourceResult<Node, Type, Error> = Result<(Node, Type), ControlSourceError<Error>>;

struct Builder<'adapter, Adapter> {
    adapter: &'adapter mut Adapter,
    limits: ControlSourceLimits,
    budget: StructureBudget,
}
impl<Adapter> Builder<'_, Adapter> {
    fn build<'source, Field, Operation, HostEffect, IrResult>(
        &mut self,
        source: &'source Expression,
        prefix: &[(&'source str, &Adapter::Type)],
        depth: usize,
        future: usize,
    ) -> ControlSourceResult<
        Node<Field, Operation, HostEffect, IrResult>,
        Adapter::Type,
        Adapter::Error,
    >
    where
        Adapter: ControlSourceAdapter<'source, Field, Operation, HostEffect, IrResult>,
    {
        let at = span(source);
        let output_error = |error| ControlSourceError::Output { span: at, error };
        let native_error = |phase, error| ControlSourceError::Native {
            span: at,
            phase,
            error,
        };
        if depth > self.limits.source.max_lowered_depth {
            return Err(output_error(StructureError::DepthLimit));
        }
        self.budget.check_pending(future, 1).map_err(output_error)?;
        match source {
            Expression::Call { callee, .. } if callee == "bind" => {
                let form = binding_source(source).map_err(ControlSourceError::Lexical)?;
                self.budget.visit(depth, 0, 0).map_err(output_error)?;
                self.budget.check_pending(future, 2).map_err(output_error)?;
                let (value, ty) = self.build(&form.binding.value, prefix, depth + 1, future + 1)?;
                // Copy bounded references only; the owned native observation stays here.
                let mut local = prefix.to_vec();
                local.push((form.binding.name.as_str(), &ty));
                let body = self.build(form.body, &local, depth + 1, future)?;
                drop(local);
                drop(ty);
                Ok((form.construct(value, body.0), body.1))
            }
            Expression::Call { callee, .. } if callee == "choose" => {
                let [when_source, then_source, otherwise_source] =
                    operands(source).map_err(ControlSourceError::Choice)?;
                self.budget.visit(depth, 0, 0).map_err(output_error)?;
                self.budget.check_pending(future, 3).map_err(output_error)?;
                let (when, when_type) = self.build(when_source, prefix, depth + 1, future + 2)?;
                let mut condition = StructureBudget::new(
                    self.limits.source.max_lowered_nodes,
                    self.limits.source.max_lowered_depth,
                );
                preflight_with_budget(&when, depth + 1, &mut condition).map_err(|error| {
                    ControlSourceError::ProducedCondition {
                        span: span(when_source),
                        error,
                    }
                })?;
                if matches!(&when, Node::Literal { value } if !matches!(value, ScalarValue::Boolean(_)))
                    || !self.adapter.boolean(&when, &when_type).map_err(|error| {
                        ControlSourceError::Native {
                            span: span(when_source),
                            phase: ControlSourcePhase::Condition,
                            error,
                        }
                    })?
                {
                    return Err(ControlSourceError::Condition {
                        span: span(when_source),
                    });
                }
                drop(when_type);
                let (then, then_type) = self.build(then_source, prefix, depth + 1, future + 1)?;
                let (otherwise, otherwise_type) =
                    self.build(otherwise_source, prefix, depth + 1, future)?;
                if !self
                    .adapter
                    .same(&then_type, &otherwise_type)
                    .map_err(|error| native_error(ControlSourcePhase::Compare, error))?
                {
                    return Err(ControlSourceError::BranchTypes { span: at });
                }
                drop(otherwise_type);
                Ok((
                    Node::Choose {
                        when: Box::new(when),
                        then: Box::new(then),
                        otherwise: Box::new(otherwise),
                    },
                    then_type,
                ))
            }
            _ => {
                let (value, ty) = self
                    .adapter
                    .lower_leaf(source, ControlSourceScope { bindings: prefix })
                    .map_err(|error| native_error(ControlSourcePhase::Leaf, error))?;
                physical(&value, depth, &mut self.budget).map_err(output_error)?;
                self.budget.check_pending(0, future).map_err(output_error)?;
                Ok((value, ty))
            }
        }
    }
}

/// Compose recursive Bind/Choose around explicitly compiled native leaves.
/// Whole cold physical/literal/reserved-scalar/choice signatures and lexical
/// bind/loop/fold policy precede every native callback. Initializers see parents;
/// bodies borrow the exact moved value-type observation; branches share only
/// parents. Active names include the bounded unique prefix, never sibling totals.
///
/// One aggregate output meter charges original leaf IR at shifted depths, plus
/// every shared constructor, retaining all future sibling roots before more
/// native work. Children lower once in Value/Body or When/Then/Otherwise order.
/// Conditions independently pass whole pure IR preflight before native boolean
/// corroboration; literal kind cannot be overridden by a host observation.
/// Native same-type checks never union exports or require PartialEq/Clone.
/// Final mandatory admission runs once after every cold output fits, never on a
/// partial tree. It may reject otherwise physically valid nested suspension.
///
/// Fixed source/output ceilings: 16,384 nodes, depth 64, 64 operands, 1,024 active
/// names. Only reference scratch vectors and language constructor Boxes/names
/// are allocated here; native IR buffers and type observations move unchanged.
/// Errors/unwind release owned parts without retries, dispatch or side-effect
/// rollback. Native work, mutation, allocation and Drop remain trusted; ingress
/// must be bounded before AST construction and a second cleanup panic can abort.
/// Physical output accounting does not bound hidden source weights or callbacks.
/// This is a source composition entry, not complete recursive leaf/helper/group
/// compilation, type inference, a VM, receipt authority or durable recovery.
pub fn lower_control_source<'source, Field, Operation, HostEffect, IrResult, Adapter>(
    expression: &'source Expression,
    prefix: &[(&'source str, &Adapter::Type)],
    limits: ControlSourceLimits,
    adapter: &mut Adapter,
) -> ControlSourceResult<Node<Field, Operation, HostEffect, IrResult>, Adapter::Type, Adapter::Error>
where
    Adapter: ControlSourceAdapter<'source, Field, Operation, HostEffect, IrResult>,
{
    if !valid_limits(limits.source) || limits.max_bindings > MAX_TYPE_INFERENCE_BINDINGS {
        return Err(ControlSourceError::Source(SourceCallError::InvalidLimits));
    }
    preflight(expression, limits.source).map_err(ControlSourceError::Source)?;
    preflight_names(expression).map_err(ControlSourceError::Lexical)?;
    cold_names(expression).map_err(ControlSourceError::Choice)?;
    crate::projection_source::preflight_names(expression)
        .map_err(ControlSourceError::Projection)?;
    if prefix.len() > limits.max_bindings
        || prefix.iter().enumerate().any(|(index, (name, _))| {
            !valid_local_name(name) || prefix[..index].iter().any(|(old, _)| old == name)
        })
    {
        return Err(ControlSourceError::Lexical(ScalarSourceError::Bindings {
            span: span(expression),
            error: PureTypeError::InvalidScope,
        }));
    }
    let mut names = prefix
        .iter()
        .map(|(name, _)| (*name, ScalarType::None))
        .collect();
    preflight_bindings(
        expression,
        &mut ScopeFrame::new(&mut names),
        limits.max_bindings,
    )
    .map_err(ControlSourceError::Lexical)?;
    let (value, ty) = Builder {
        adapter,
        limits,
        budget: StructureBudget::new(
            limits.source.max_lowered_nodes,
            limits.source.max_lowered_depth,
        ),
    }
    .build(expression, prefix, 0, 0)?;
    adapter
        .admit(
            expression,
            &value,
            &ty,
            ControlSourceScope { bindings: prefix },
        )
        .map_err(|error| ControlSourceError::Native {
            span: span(expression),
            phase: ControlSourcePhase::Admit,
            error,
        })?;
    Ok((value, ty))
}
