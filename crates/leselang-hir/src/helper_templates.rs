//! Owned bounded helper templates with explicit native materialization policy.

use std::{collections::HashSet, convert::Infallible, fmt};

use leselang_runtime_core::{ScalarType, ScopeFrame};
use leselang_syntax::MAX_FUNCTION_PARAMETERS;

use crate::helper_hygiene::{HelperHygieneError, HelperHygieneLimits, check_scope, collect_names};
use crate::ir::Computation;
use crate::pure_typing::{
    MAX_TYPE_INFERENCE_BINDINGS, MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES,
    valid_local_name,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperTemplateLimits {
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_bindings: usize,
    pub max_parameters: usize,
}

/// Physical language nodes/depth only. No folded source, opaque native graph,
/// parameter wrapper, active caller prefix, execution fuel or authority is counted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperTemplateShape {
    pub nodes: usize,
    pub depth: usize,
}

pub enum HelperTemplateError<Error> {
    InvalidLimits,
    ParameterLimit,
    InvalidParameter {
        index: usize,
    },
    Body(HelperHygieneError<Infallible>),
    Factory(Error),
    ShapeChanged {
        expected: HelperTemplateShape,
        actual: HelperTemplateShape,
    },
}
impl<Error> fmt::Display for HelperTemplateError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper template limits exceed safety ceilings",
            Self::ParameterLimit => "helper template parameter count exceeds its limit",
            Self::InvalidParameter { .. } => "helper template parameter names are invalid",
            Self::Body(_) => "helper template body has invalid physical or lexical shape",
            Self::Factory(_) => "native helper materialization failed",
            Self::ShapeChanged { .. } => "materialized helper changed its reserved physical shape",
        })
    }
}
impl<Error> fmt::Debug for HelperTemplateError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for HelperTemplateError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

fn preflight<Field, Operation, HostEffect, IrResult, Error>(
    parameters: &[(String, ScalarType)],
    body: &Node<Field, Operation, HostEffect, IrResult>,
    limits: HelperTemplateLimits,
) -> Result<HelperTemplateShape, HelperTemplateError<Error>> {
    if limits.max_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH
        || limits.max_bindings > MAX_TYPE_INFERENCE_BINDINGS
        || limits.max_parameters > MAX_FUNCTION_PARAMETERS
    {
        return Err(HelperTemplateError::InvalidLimits);
    }
    if parameters.len() > limits.max_parameters {
        return Err(HelperTemplateError::ParameterLimit);
    }
    if parameters.len() > limits.max_bindings {
        return Err(HelperTemplateError::Body(HelperHygieneError::BindingLimit));
    }
    let mut names = HashSet::new();
    for (index, (name, _)) in parameters.iter().enumerate() {
        if !valid_local_name(name) || !names.insert(name.to_owned()) {
            return Err(HelperTemplateError::InvalidParameter { index });
        }
    }
    let (nodes, depth) = collect_names(
        body,
        HelperHygieneLimits {
            max_nodes: limits.max_nodes,
            max_depth: limits.max_depth,
            max_bindings: limits.max_bindings,
            max_reserved_names: 0,
        },
        &mut names,
    )
    .map_err(HelperTemplateError::Body)?;
    let mut bindings = parameters
        .iter()
        .map(|(name, _)| (name.as_str(), ()))
        .collect();
    check_scope(
        body,
        &mut ScopeFrame::new(&mut bindings),
        limits.max_bindings,
    )
    .map_err(HelperTemplateError::Body)?;
    Ok(HelperTemplateShape { nodes, depth })
}

/// An owned reusable body/signature/result observation, not an executable program.
/// No global registry, name selection, locks, implicit Clone/default/serde/Send,
/// journal or product host is installed. Language structure is private/read-only;
/// native slots/result observations may still have interior mutability. Debug
/// reveals counts/shape only, never names, scalar literals or native metadata.
#[must_use = "retain or explicitly consume the owned helper template"]
pub struct HelperTemplate<Node, ResultType> {
    parameters: Vec<(String, ScalarType)>,
    body: Node,
    result_type: ResultType,
    shape: HelperTemplateShape,
}
impl<Node, ResultType> fmt::Debug for HelperTemplate<Node, ResultType> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HelperTemplate")
            .field("parameters", &self.parameters.len())
            .field("shape", &self.shape)
            .finish()
    }
}
impl<Node, ResultType> HelperTemplate<Node, ResultType> {
    pub fn parameters(&self) -> &[(String, ScalarType)] {
        &self.parameters
    }
    pub fn body(&self) -> &Node {
        &self.body
    }
    pub fn result_type(&self) -> &ResultType {
        &self.result_type
    }
    pub const fn shape(&self) -> HelperTemplateShape {
        self.shape
    }

    /// Move the exact original signature/body/result out without cloning. This
    /// destroys the cache entry; returned mutable data is not a certificate.
    pub fn into_parts(self) -> (Vec<(String, ScalarType)>, Node, ResultType) {
        (self.parameters, self.body, self.result_type)
    }
}
impl<Field, Operation, HostEffect, IrResult, ResultType>
    HelperTemplate<Node<Field, Operation, HostEffect, IrResult>, ResultType>
{
    /// Take ownership after complete physical and cold lexical preflight against
    /// the exact scalar parameter names. Inclusive safety ceilings are 16,384
    /// nodes, depth 64, 1,024 active bindings and eight parameters. Zero nodes denies
    /// bodies; zero depth allows leaves; zero bindings forbids parameters/locals.
    /// No name normalization, declaration lookup, builtin policy or return typing
    /// is introduced. Body/result/native buffers move unchanged, without queries.
    /// Free locals/groups and active shadowing fail even in unused/cold paths.
    ///
    /// The shape is physical only, not source-expanded cost or a type certificate.
    /// Body literals/operators/group schemas, native versions/grants, closed Host
    /// graphs and complete cold typing remain caller checks. Native ingress, work,
    /// interior mutability and Drop are trusted adapter responsibilities.
    pub fn new(
        parameters: Vec<(String, ScalarType)>,
        body: Node<Field, Operation, HostEffect, IrResult>,
        result_type: ResultType,
        limits: HelperTemplateLimits,
    ) -> Result<Self, HelperTemplateError<Infallible>> {
        let shape = preflight(&parameters, &body, limits)?;
        Ok(Self {
            parameters,
            body,
            result_type,
            shape,
        })
    }

    /// Materialize once through an explicit trusted host factory. Recheck complete
    /// cached physical/lexical shape under this call's limits before invoking it;
    /// check the returned body again before returning it. The physical node count
    /// and maximum depth must match exactly, so no reserved shape is widened or
    /// silently shrunk. The callback receives the exact original borrowed body.
    ///
    /// Matching shape does not prove semantic/literal/native identity: same-shape
    /// output still needs complete cold typing, source-cost/canonical checks and
    /// live schema/version/authority validation. The factory owns faithful template
    /// copying, native remapping, allocation and unwind; no Clone bound is imposed.
    /// It cannot use shape checks as an in-process sandbox or fuel for native work.
    ///
    /// Failure/unwind drops partial output without returning a partial body, retry
    /// or source/native reservation refund. The cached entry remains owned, but
    /// native callback/Drop side effects are not rolled back. A later explicit call
    /// needs fresh caller-owned admission and expansion accounting, not replay
    /// authority from this reusable template. No operands, aliases, result receipt,
    /// execution, dispatch, suspension or durable cache are created here.
    pub fn materialize<Error>(
        &self,
        limits: HelperTemplateLimits,
        factory: impl FnOnce(
            &Node<Field, Operation, HostEffect, IrResult>,
        ) -> Result<Node<Field, Operation, HostEffect, IrResult>, Error>,
    ) -> Result<Node<Field, Operation, HostEffect, IrResult>, HelperTemplateError<Error>> {
        let expected = preflight(&self.parameters, &self.body, limits)?;
        let body = factory(&self.body).map_err(HelperTemplateError::Factory)?;
        let actual = preflight(&self.parameters, &body, limits)?;
        if expected != actual {
            return Err(HelperTemplateError::ShapeChanged { expected, actual });
        }
        Ok(body)
    }
}
