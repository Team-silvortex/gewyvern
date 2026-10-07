//! Closed scalar helper-body source preparation with explicit native lowering.

use std::fmt;

use leselang_runtime_core::ScalarType;
use leselang_syntax::{Function, MAX_FUNCTION_PARAMETERS, Span};

use crate::helper_body::{HelperBody, HelperBodyError, HelperBodyLimits, LoweredHelperBody};
use crate::helper_source::{HelperParameter, HelperParameterError, helper_parameters};
use crate::ir::Computation;
use crate::pure_typing::{
    MAX_TYPE_INFERENCE_BINDINGS, MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES,
    valid_local_name,
};
use crate::source_call::{
    SourceCallError, SourceCallLimits, SourceShapeError, preflight, valid_limits,
};
use crate::source_cost::SourceCostExtra;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperBodySourceLimits {
    pub max_source_nodes: usize,
    pub max_source_depth: usize,
    pub max_arguments: usize,
    pub body: HelperBodyLimits,
}

/// Unchecked native output, without a replaceable parameter signature. Native
/// result/category agreement and complete cold source fidelity remain host policy.
pub struct HelperBodyObservation<Node, ResultType> {
    pub expression: Node,
    pub result_type: ResultType,
    pub scalar_result: Option<ScalarType>,
}

pub enum HelperBodySourceError<Lowering, Validation, Cost> {
    InvalidLimits,
    InvalidName,
    Parameters(HelperParameterError),
    ParameterScopeLimit,
    Source { span: Span, error: SourceShapeError },
    SourceCounterLimit,
    Lowering(Lowering),
    Admission(HelperBodyError<Validation, Cost>),
}
impl<Lowering, Validation, Cost> fmt::Display
    for HelperBodySourceError<Lowering, Validation, Cost>
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper body source limits exceed safety ceilings",
            Self::InvalidName => "helper body declaration name is invalid",
            Self::Parameters(_) => "helper body scalar signature is invalid",
            Self::ParameterScopeLimit => "helper scalar prefix exceeds its binding limit",
            Self::Source { .. } => "helper body physical source is invalid",
            Self::SourceCounterLimit => "helper body source counter exceeds its limit",
            Self::Lowering(_) => "native helper body source lowering failed",
            Self::Admission(_) => "lowered helper body admission failed",
        })
    }
}
impl<Lowering, Validation, Cost> fmt::Debug for HelperBodySourceError<Lowering, Validation, Cost> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Lowering, Validation, Cost> std::error::Error
    for HelperBodySourceError<Lowering, Validation, Cost>
{
}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type HelperBodySourceResult<Node, ResultType, Lowering, Validation, Cost> =
    Result<HelperBody<Node, ResultType>, HelperBodySourceError<Lowering, Validation, Cost>>;

struct PreparedSource<'source> {
    function: &'source Function,
    parameters: Vec<HelperParameter<'source>>,
}

fn prepare_header<Lowering, Validation, Cost>(
    function: &Function,
    limits: HelperBodySourceLimits,
    used_source_nodes: usize,
) -> Result<PreparedSource<'_>, HelperBodySourceError<Lowering, Validation, Cost>> {
    if !valid_limits(source_limits(limits))
        || limits.body.template.max_bindings > MAX_TYPE_INFERENCE_BINDINGS
        || limits.body.template.max_parameters > MAX_FUNCTION_PARAMETERS
        || limits.body.source_cost.max_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.body.source_cost.max_depth > MAX_TYPE_INFERENCE_DEPTH
    {
        return Err(HelperBodySourceError::InvalidLimits);
    }
    if !valid_local_name(&function.name) {
        return Err(HelperBodySourceError::InvalidName);
    }
    let parameters = helper_parameters(function, limits.body.template.max_parameters)
        .map_err(HelperBodySourceError::Parameters)?;
    if parameters.len() > limits.body.template.max_bindings {
        return Err(HelperBodySourceError::ParameterScopeLimit);
    }
    if used_source_nodes > limits.max_source_nodes {
        return Err(HelperBodySourceError::SourceCounterLimit);
    }
    Ok(PreparedSource {
        function,
        parameters,
    })
}

fn source_limits(limits: HelperBodySourceLimits) -> SourceCallLimits {
    SourceCallLimits {
        max_source_nodes: limits.max_source_nodes,
        max_source_depth: limits.max_source_depth,
        max_lowered_nodes: limits.body.template.max_nodes,
        max_lowered_depth: limits.body.template.max_depth,
        max_arguments: limits.max_arguments,
    }
}

/// Prepare one original borrowed declaration, not an entire compiler or registry.
/// Check all source/output/cost ceilings, exact six-token scalar parameters and
/// prefix capacity before callbacks. Whole cold source names/text/arity/physical
/// bounds then pass before native lowering, including unused branches and loops.
/// Entry/builtin naming, dependencies, source/span authenticity and aggregate
/// ingress bytes are caller policy; an unknown call is not authorized here.
///
/// The lowerer runs once with the original Function, declaration-order borrowed
/// parameters and the original mutable source counter. It must seed exactly that
/// scalar prefix, reject captures, type/lower every cold path and recursively
/// account on that same counter. No copied meter, precharge, rollback or retry is
/// installed. Native accounting is trusted; counter bounds do not prove fidelity.
/// An over-limit successful callback fails before body admission without refund.
///
/// The core alone materializes the declared signature, then existing HelperBody
/// admission bounds the complete output, infers pure returns, invokes native
/// validation once and measures original host costs before returning body+cost.
/// All native slots/result buffers move unchanged, without Clone/Debug/serde/Send
/// bounds. No parameter substitution, native copy, execution, fuel, cache insert
/// or partial handoff occurs. Errors remain matchable, never native formatting or
/// source chains. Failed/unwinding hooks drop owned output; earlier work and the
/// caller's source counter stay committed. Bound owned ingress first; native work,
/// allocation, interior mutation, Drop/unwind and live authority remain trusted
/// host responsibilities. This is source preparation around explicit recursion,
/// not a generic recursive compiler, durable restart or execution certificate.
pub fn lower_helper_body<
    'source,
    Field,
    Operation,
    HostEffect,
    IrResult,
    ResultType,
    Lowering,
    Validation,
    Cost,
>(
    function: &'source Function,
    limits: HelperBodySourceLimits,
    used_source_nodes: &mut usize,
    lower: impl FnOnce(
        &'source Function,
        &[HelperParameter<'source>],
        &mut usize,
    ) -> Result<
        HelperBodyObservation<Node<Field, Operation, HostEffect, IrResult>, ResultType>,
        Lowering,
    >,
    validate: impl FnOnce(
        &Node<Field, Operation, HostEffect, IrResult>,
        &[(String, ScalarType)],
        &ResultType,
        Option<ScalarType>,
    ) -> Result<(), Validation>,
    host_cost: impl FnMut(&HostEffect) -> Result<SourceCostExtra, Cost>,
) -> HelperBodySourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    ResultType,
    Lowering,
    Validation,
    Cost,
> {
    let source = prepare_header(function, limits, *used_source_nodes)?;
    preflight::<Lowering>(&function.body, source_limits(limits)).map_err(|error| match error {
        SourceCallError::Shape { span, error } => HelperBodySourceError::Source { span, error },
        _ => HelperBodySourceError::InvalidLimits,
    })?;
    finish(
        source,
        limits,
        used_source_nodes,
        lower,
        validate,
        host_cost,
    )
}

// The reference declaration/parser boundary already bounds source. Preserve its
// recursive diagnostic priority while sharing exact headers, meter and admission.
pub(crate) fn lower_preflighted_helper_body<
    'source,
    Field,
    Operation,
    HostEffect,
    IrResult,
    ResultType,
    Lowering,
    Validation,
    Cost,
>(
    function: &'source Function,
    limits: HelperBodySourceLimits,
    used_source_nodes: &mut usize,
    lower: impl FnOnce(
        &'source Function,
        &[HelperParameter<'source>],
        &mut usize,
    ) -> Result<
        HelperBodyObservation<Node<Field, Operation, HostEffect, IrResult>, ResultType>,
        Lowering,
    >,
    validate: impl FnOnce(
        &Node<Field, Operation, HostEffect, IrResult>,
        &[(String, ScalarType)],
        &ResultType,
        Option<ScalarType>,
    ) -> Result<(), Validation>,
    host_cost: impl FnMut(&HostEffect) -> Result<SourceCostExtra, Cost>,
) -> HelperBodySourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    ResultType,
    Lowering,
    Validation,
    Cost,
> {
    let source = prepare_header(function, limits, *used_source_nodes)?;
    finish(
        source,
        limits,
        used_source_nodes,
        lower,
        validate,
        host_cost,
    )
}

fn finish<
    'source,
    Field,
    Operation,
    HostEffect,
    IrResult,
    ResultType,
    Lowering,
    Validation,
    Cost,
>(
    source: PreparedSource<'source>,
    limits: HelperBodySourceLimits,
    used_source_nodes: &mut usize,
    lower: impl FnOnce(
        &'source Function,
        &[HelperParameter<'source>],
        &mut usize,
    ) -> Result<
        HelperBodyObservation<Node<Field, Operation, HostEffect, IrResult>, ResultType>,
        Lowering,
    >,
    validate: impl FnOnce(
        &Node<Field, Operation, HostEffect, IrResult>,
        &[(String, ScalarType)],
        &ResultType,
        Option<ScalarType>,
    ) -> Result<(), Validation>,
    host_cost: impl FnMut(&HostEffect) -> Result<SourceCostExtra, Cost>,
) -> HelperBodySourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    ResultType,
    Lowering,
    Validation,
    Cost,
> {
    let observed = lower(source.function, &source.parameters, used_source_nodes)
        .map_err(HelperBodySourceError::Lowering)?;
    if *used_source_nodes > limits.max_source_nodes {
        return Err(HelperBodySourceError::SourceCounterLimit);
    }
    LoweredHelperBody {
        parameters: source
            .parameters
            .into_iter()
            .map(|parameter| (parameter.name.to_owned(), parameter.domain))
            .collect(),
        expression: observed.expression,
        result_type: observed.result_type,
        scalar_result: observed.scalar_result,
    }
    .prepare(limits.body, validate, host_cost)
    .map_err(HelperBodySourceError::Admission)
}
