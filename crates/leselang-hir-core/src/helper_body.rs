//! Lowered helper-body admission, not source lowering or execution authority.

use std::{convert::Infallible, fmt};

use leselang_runtime_core::ScalarType;

use crate::helper_expansion::HelperExpansionCost;
use crate::helper_templates::{HelperTemplate, HelperTemplateError, HelperTemplateLimits};
use crate::ir::Computation;
use crate::pure_typing::{
    MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES, PureType, PureTypeEnvironment,
    PureTypeError, TypeInferenceLimits, infer_pure_type,
};
use crate::source_cost::{SourceCostError, SourceCostExtra, SourceCostLimits, measure_source_cost};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperBodyLimits {
    pub template: HelperTemplateLimits,
    pub source_cost: SourceCostLimits,
}

pub enum HelperBodyError<Validation, Cost> {
    InvalidLimits,
    Template(HelperTemplateError<Infallible>),
    PureResultRequired,
    PureTyping(PureTypeError),
    PureResultMismatch {
        declared: ScalarType,
        inferred: ScalarType,
    },
    Validation(Validation),
    Cost(SourceCostError<Cost>),
}
impl<Validation, Cost> fmt::Display for HelperBodyError<Validation, Cost> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper body limits exceed safety ceilings",
            Self::Template(_) => "helper body has invalid physical or lexical shape",
            Self::PureResultRequired => "a pure helper must return bounded scalar data",
            Self::PureTyping(_) => "pure helper body fails complete cold scalar typing",
            Self::PureResultMismatch { .. } => {
                "pure helper return observation disagrees with inference"
            }
            Self::Validation(_) => "native helper body validation failed",
            Self::Cost(_) => "helper body source cost measurement failed",
        })
    }
}
impl<Validation, Cost> fmt::Debug for HelperBodyError<Validation, Cost> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Validation, Cost> std::error::Error for HelperBodyError<Validation, Cost> {}

/// Unchecked output of a host's source lowerer. The observation is not a type
/// certificate; scalar_result must be corroborated with result_type by the host.
/// No native Clone/Debug/serde/Send bound, implicit conversion or registry.
pub struct LoweredHelperBody<Node, ResultType> {
    pub parameters: Vec<(String, ScalarType)>,
    pub expression: Node,
    pub result_type: ResultType,
    pub scalar_result: Option<ScalarType>,
}
impl<Node, ResultType> fmt::Debug for LoweredHelperBody<Node, ResultType> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoweredHelperBody")
            .field("parameters", &self.parameters.len())
            .finish()
    }
}

/// Owned template and source-cost observation after explicit admission. Not an
/// executable/canonical/authority certificate, persistent cache or source compiler.
/// Native metadata may still change; revalidate live native policy before use.
#[must_use = "retain or explicitly consume the admitted helper body"]
pub struct HelperBody<Node, ResultType> {
    template: HelperTemplate<Node, ResultType>,
    source_cost: HelperExpansionCost,
}
impl<Node, ResultType> fmt::Debug for HelperBody<Node, ResultType> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("HelperBody")
            .field("template", &self.template)
            .field("source_cost", &self.source_cost)
            .finish()
    }
}
impl<Node, ResultType> HelperBody<Node, ResultType> {
    pub fn template(&self) -> &HelperTemplate<Node, ResultType> {
        &self.template
    }
    pub const fn source_cost(&self) -> HelperExpansionCost {
        self.source_cost
    }
    /// Move original signature/body/result and cost out for caller-owned storage.
    /// Registry insertion happens only after this method, outside the core.
    pub fn into_parts(self) -> (HelperTemplate<Node, ResultType>, HelperExpansionCost) {
        (self.template, self.source_cost)
    }
}

type Admission<Node, ResultType, Validation, Cost> =
    Result<HelperBody<Node, ResultType>, HelperBodyError<Validation, Cost>>;

struct ClosedScalarParameters;
impl<Field, Operation> PureTypeEnvironment<Field, Operation> for ClosedScalarParameters {
    type Result = ();
    fn field_type(&self, _: &(), _: &Field) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &Operation) -> Option<()> {
        None
    }
}

impl<Field, Operation, HostEffect, IrResult, ResultType>
    LoweredHelperBody<Computation<Field, Operation, HostEffect, IrResult>, ResultType>
{
    /// Admit already-lowered IR once, without recompiling source or native copying.
    /// Template limits enforce complete cold physical/name/lexical checks against
    /// the exact scalar parameters (ceilings 16,384 nodes, depth 64, 1,024 bindings,
    /// eight parameters). Source-cost ceilings are separate 16,384/64 limits;
    /// zero policies remain explicit. Invalid ceilings fail before any callbacks.
    ///
    /// A physically pure body must declare a scalar result. Complete cold shared
    /// inference checks all branches, zero-limit controls and literals against
    /// that declaration. Only scalar parameter types seed the scope: no external
    /// native result, group, export query or caller capture is invented. Thus pure
    /// field/member references cannot forge a native result through metadata.
    /// No inferred type is trusted solely because the source lowerer reported it.
    ///
    /// Next, validation runs exactly once with the original borrowed body,
    /// signature and result observation. The host corroborates result_type and
    /// scalar_result, checks all cold effects, schemas, canonical bytes and closed
    /// opaque graphs. A successful callback is trusted policy, not sandboxing or
    /// effect authority. Then shared source-weight measurement runs, with explicit
    /// native extras. Cost limits/failure do not undo earlier validation work.
    ///
    /// Original native slots, Boxes, vectors, parameter/result buffers move
    /// unchanged into the template. Failure/unwind drops consumed inputs once,
    /// with no partial entry, retry, registry mutation or prior-reservation refund.
    /// Native errors remain matchable, not formatting or source chains. The host
    /// bounds ingress before owned construction/decoding and owns native work,
    /// allocation, interior mutation, Drop and unwind. Fresh complete live checks
    /// remain mandatory for materialized copies, dispatch and reply acceptance.
    pub fn prepare<Validation, Cost>(
        self,
        limits: HelperBodyLimits,
        validate: impl FnOnce(
            &Computation<Field, Operation, HostEffect, IrResult>,
            &[(String, ScalarType)],
            &ResultType,
            Option<ScalarType>,
        ) -> Result<(), Validation>,
        host_cost: impl FnMut(&HostEffect) -> Result<SourceCostExtra, Cost>,
    ) -> Admission<Computation<Field, Operation, HostEffect, IrResult>, ResultType, Validation, Cost>
    {
        if limits.source_cost.max_nodes > MAX_TYPE_INFERENCE_NODES
            || limits.source_cost.max_depth > MAX_TYPE_INFERENCE_DEPTH
        {
            return Err(HelperBodyError::InvalidLimits);
        }
        let template = HelperTemplate::new(
            self.parameters,
            self.expression,
            self.result_type,
            limits.template,
        )
        .map_err(HelperBodyError::Template)?;
        if template.body().is_pure() {
            let declared = self
                .scalar_result
                .ok_or(HelperBodyError::PureResultRequired)?;
            let bindings = template
                .parameters()
                .iter()
                .map(|(name, ty)| (name.as_str(), PureType::Scalar(*ty)))
                .collect::<Vec<_>>();
            let inferred = infer_pure_type(
                template.body(),
                &bindings,
                &ClosedScalarParameters,
                TypeInferenceLimits {
                    max_nodes: limits.template.max_nodes,
                    max_depth: limits.template.max_depth,
                    max_bindings: limits.template.max_bindings,
                },
            )
            .map_err(HelperBodyError::PureTyping)?;
            let PureType::Scalar(inferred) = inferred else {
                return Err(HelperBodyError::PureResultRequired);
            };
            if inferred != declared {
                return Err(HelperBodyError::PureResultMismatch { declared, inferred });
            }
        }
        validate(
            template.body(),
            template.parameters(),
            template.result_type(),
            self.scalar_result,
        )
        .map_err(HelperBodyError::Validation)?;
        let source_cost = measure_source_cost(template.body(), limits.source_cost, host_cost)
            .map_err(HelperBodyError::Cost)?;
        Ok(HelperBody {
            template,
            source_cost,
        })
    }
}
