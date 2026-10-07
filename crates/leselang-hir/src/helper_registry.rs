//! Owned helper registry assembly around explicit native storage policy.

use std::{convert::Infallible, fmt};

use leselang_runtime_core::StructureError;
use leselang_syntax::{Function, MAX_FUNCTION_PARAMETERS, Span};

use crate::helper_body::HelperBody;
use crate::helper_declarations::HelperDeclarations;
use crate::helper_dependencies::{
    HelperDependencyError, HelperDependencyLimits, helper_dependency_order,
};
use crate::helper_source::{HelperParameterError, helper_parameters};
use crate::helper_templates::{HelperTemplateError, HelperTemplateLimits, preflight};
use crate::ir::Computation;
use crate::pure_typing::{
    MAX_TYPE_INFERENCE_BINDINGS, MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES,
};
use crate::source_cost::SourceCostLimits;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperRegistryLimits {
    pub dependencies: HelperDependencyLimits,
    pub template: HelperTemplateLimits,
    pub source_cost: SourceCostLimits,
}

pub enum HelperRegistryError<Lowering, Registration> {
    InvalidLimits,
    Dependency(HelperDependencyError),
    Parameters {
        span: Span,
        error: HelperParameterError,
    },
    ParameterScopeLimit {
        span: Span,
    },
    Signature {
        span: Span,
    },
    Template {
        span: Span,
        error: HelperTemplateError<Infallible>,
    },
    SourceCost {
        span: Span,
        error: StructureError,
    },
    Lowering {
        span: Span,
        error: Lowering,
    },
    Registration {
        span: Span,
        error: Registration,
    },
}
impl<Lowering, Registration> fmt::Display for HelperRegistryError<Lowering, Registration> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper registry limits exceed safety ceilings",
            Self::Dependency(_) => "helper dependency planning failed",
            Self::Parameters { .. } => "helper registry scalar signature is invalid",
            Self::ParameterScopeLimit { .. } => {
                "helper registry parameter prefix exceeds its limit"
            }
            Self::Signature { .. } => "prepared helper signature differs from its declaration",
            Self::Template { .. } => "prepared helper exceeds current physical or lexical bounds",
            Self::SourceCost { .. } => "prepared helper exceeds current source cost bounds",
            Self::Lowering { .. } => "native helper preparation failed",
            Self::Registration { .. } => "native helper registration failed",
        })
    }
}
impl<Lowering, Registration> fmt::Debug for HelperRegistryError<Lowering, Registration> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Lowering, Registration> std::error::Error for HelperRegistryError<Lowering, Registration> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type HelperRegistryResult<State, Lowering, Registration> =
    Result<State, HelperRegistryError<Lowering, Registration>>;

/// Assemble all admitted helpers, including unused declarations, into an owned
/// caller-supplied state. The designated entry comes from the original declaration
/// observation; it is neither lowered nor registered here. State/storage, builtin
/// policy, source/body fidelity, native canonical/type/grant checks and aggregate
/// ingress/work quotas stay with the host. No global registry or default is made.
///
/// Check every dependency/template/cost ceiling before callbacks, then bound the
/// complete helper source forest and plan the borrowed dependency graph. Advance
/// lazily: choose the lexically smallest ready declaration, validate its current
/// exact scalar prefix, prepare once, check its returned declaration-order
/// signature, current physical/lexical shape and cached source cost, then register
/// once with the original Function and original mutable state. Only after success
/// ask for the next declaration. A ready native error precedes a later cycle;
/// unknown calls are never authorized by dependency planning. There is no eager
/// order collection, skip/resume, key normalization or implicit native copy.
///
/// Registration consumes the original HelperBody with body+cost still paired.
/// Native metadata/cost observations are not live certificates: prepare must
/// corroborate all cold native policy and faithful source accounting; copied
/// instances and dispatch need fresh validation. Existing state contents carry
/// no implicit admission, and callbacks must not dispatch or publish execution
/// authority. Hosts own lookup/storage identity and generated-name reservations.
///
/// Return the same owned state only after every helper has prepared/registered.
/// Failure/unwind drops state and unregistered owned output, without a partial
/// state handoff, retry, native formatting/source chains or reservation refund.
/// This is not transactional rollback of native/interior-mutability side effects,
/// external storage or state copies made by the caller. Native allocation, work,
/// mutation, Drop/unwind (including a second cleanup panic) remain trusted. No
/// Clone/Debug/serde/Send bound, locks, callbacks on Drop, fuel, dispatch, durable
/// journal or interpreter is installed. This is shared registry orchestration,
/// not complete generic recursive source compilation or execution authority.
pub fn assemble_helper_registry<
    'source,
    Field,
    Operation,
    HostEffect,
    IrResult,
    ResultType,
    State,
    Lowering,
    Registration,
>(
    declarations: &HelperDeclarations<'source>,
    limits: HelperRegistryLimits,
    mut state: State,
    mut prepare: impl FnMut(
        &'source Function,
        &mut State,
    ) -> Result<
        HelperBody<Node<Field, Operation, HostEffect, IrResult>, ResultType>,
        Lowering,
    >,
    mut register: impl FnMut(
        &'source Function,
        HelperBody<Node<Field, Operation, HostEffect, IrResult>, ResultType>,
        &mut State,
    ) -> Result<(), Registration>,
) -> HelperRegistryResult<State, Lowering, Registration> {
    if limits.dependencies.max_helpers > crate::helper_dependencies::MAX_DEPENDENCY_HELPERS
        || limits.dependencies.max_source_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.dependencies.max_source_depth > MAX_TYPE_INFERENCE_DEPTH
        || limits.template.max_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.template.max_depth > MAX_TYPE_INFERENCE_DEPTH
        || limits.template.max_bindings > MAX_TYPE_INFERENCE_BINDINGS
        || limits.template.max_parameters > MAX_FUNCTION_PARAMETERS
        || limits.source_cost.max_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.source_cost.max_depth > MAX_TYPE_INFERENCE_DEPTH
    {
        return Err(HelperRegistryError::InvalidLimits);
    }
    let order = helper_dependency_order(
        declarations.helpers(),
        &declarations.entry().name,
        limits.dependencies,
    )
    .map_err(HelperRegistryError::Dependency)?;
    for function in order {
        let function = function.map_err(HelperRegistryError::Dependency)?;
        let span = function.span;
        let parameters = helper_parameters(function, limits.template.max_parameters)
            .map_err(|error| HelperRegistryError::Parameters { span, error })?;
        if parameters.len() > limits.template.max_bindings {
            return Err(HelperRegistryError::ParameterScopeLimit { span });
        }
        let body = prepare(function, &mut state)
            .map_err(|error| HelperRegistryError::Lowering { span, error })?;
        let template = body.template();
        if parameters.len() != template.parameters().len()
            || parameters
                .iter()
                .zip(template.parameters())
                .any(|(declared, (name, ty))| declared.name != name || declared.domain != *ty)
        {
            return Err(HelperRegistryError::Signature { span });
        }
        preflight(template.parameters(), template.body(), limits.template)
            .map_err(|error| HelperRegistryError::Template { span, error })?;
        let cost = body.source_cost();
        if cost.depth > limits.source_cost.max_depth {
            return Err(HelperRegistryError::SourceCost {
                span,
                error: StructureError::DepthLimit,
            });
        }
        if cost.nodes > limits.source_cost.max_nodes {
            return Err(HelperRegistryError::SourceCost {
                span,
                error: StructureError::NodeLimit,
            });
        }
        register(function, body, &mut state)
            .map_err(|error| HelperRegistryError::Registration { span, error })?;
    }
    Ok(state)
}
