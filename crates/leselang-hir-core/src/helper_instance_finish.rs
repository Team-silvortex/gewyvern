//! Complete a helper instance after caller-owned recursive operand compilation.

use std::collections::HashSet;

use leselang_runtime_core::{
    ScalarType, ScalarTypeSet, StructureBudget, StructureError, check_argument_type,
};
use leselang_syntax::MAX_FUNCTION_PARAMETERS;

use crate::binding_source::physical;
use crate::helper_bindings::{HelperBinding, HelperBindingLimits, bind_helper_arguments};
use crate::helper_expansion::{HelperExpansionLimits, reserve_helper_expansion};
use crate::helper_hygiene::{HelperHygieneLimits, collect_names, hygienic_helper_body};
use crate::helper_instance::{
    HelperInstanceError, HelperInstanceLimits, HelperInstancePhase, HelperInstanceResult,
    SelectedHelper,
};
use crate::helper_templates::{
    HelperTemplateLimits, HelperTemplateShape, preflight as preflight_template,
};
use crate::ir::Computation;
use crate::pure_typing::{MAX_TYPE_INFERENCE_BINDINGS, MAX_TYPE_INFERENCE_NODES, valid_local_name};
use crate::source_call::valid_limits;

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

/// Declaration-ordered owned operand plus a trusted lowerer's scalar observation.
/// Mutable unchecked data, not a type/source certificate. Bound ingress first.
pub struct HelperInstanceArgument<Node> {
    pub value: Node,
    pub scalar_type: Option<ScalarType>,
}

/// Native gates for completion, without recursive source lowering or a second meter.
/// Every operand, including unused ones, needs exact cold caller-scope typing in
/// admit_argument. Final admit corroborates the whole renamed/wrapped output, the
/// original return observation, native costs, schemas/grants and canonical policy.
pub trait HelperInstanceFinisher<Field, Operation, HostEffect, IrResult, ResultType> {
    type Error;
    fn admit_argument(
        &mut self,
        value: &Node<Field, Operation, HostEffect, IrResult>,
        expected: ScalarType,
        index: usize,
    ) -> Result<(), Self::Error>;
    fn argument_depth(
        &mut self,
        value: &Node<Field, Operation, HostEffect, IrResult>,
    ) -> Result<usize, Self::Error>;
    fn materialize(
        &mut self,
        body: &Node<Field, Operation, HostEffect, IrResult>,
    ) -> Result<Node<Field, Operation, HostEffect, IrResult>, Self::Error>;
    fn fresh_name(&mut self) -> Result<String, Self::Error>;
    fn admit(
        &mut self,
        value: &Node<Field, Operation, HostEffect, IrResult>,
        result: &ResultType,
    ) -> Result<(), Self::Error>;
}

/// Finish already-compiled operands using the same caller-owned source counter.
/// The counter includes the call and operands, including recursively expanded
/// nested helpers; there is no copied meter, precharge estimate or hidden refund.
/// Current cached shape/limits, operand count, complete cold physical forest and
/// lexical reservations precede type hooks. All closed scalar/literal facts pass
/// before every once declaration-order native operand admission. Unused operands
/// are not exempt. Host lowering owns original source/signature/order authenticity.
/// Then shared expansion commits before one copy, aliases, hygiene and wrapping.
/// Mandatory whole-output native admission retains the original borrowed result.
/// No native Clone/Debug/serde/Send bound, dispatch, retry or partial IR handoff.
/// Native work/mutation/Drop is trusted; bound ingress and a second cleanup panic
/// may abort. This is staged completion, not a complete independent compiler.
pub fn finish_helper_instance<'body, Field, Operation, HostEffect, IrResult, ResultType, Adapter>(
    selected: SelectedHelper<'body, '_, Node<Field, Operation, HostEffect, IrResult>, ResultType>,
    arguments: Vec<HelperInstanceArgument<Node<Field, Operation, HostEffect, IrResult>>>,
    limits: HelperInstanceLimits,
    used_source_nodes: &mut usize,
    adapter: &mut Adapter,
) -> HelperInstanceResult<
    'body,
    Node<Field, Operation, HostEffect, IrResult>,
    ResultType,
    Adapter::Error,
>
where
    Adapter: HelperInstanceFinisher<Field, Operation, HostEffect, IrResult, ResultType>,
{
    let (template_limits, shape) = preflight_instance(&selected, limits)?;
    finish_preflighted_instance(
        selected,
        arguments,
        limits,
        used_source_nodes,
        adapter,
        template_limits,
        shape,
    )
}

pub(crate) fn preflight_instance<Field, Operation, HostEffect, IrResult, ResultType, Error>(
    selected: &SelectedHelper<'_, '_, Node<Field, Operation, HostEffect, IrResult>, ResultType>,
    limits: HelperInstanceLimits,
) -> Result<(HelperTemplateLimits, HelperTemplateShape), HelperInstanceError<Error>> {
    if !valid_limits(limits.source)
        || limits.max_parameters > MAX_FUNCTION_PARAMETERS
        || limits.max_bindings > MAX_TYPE_INFERENCE_BINDINGS
        || limits.max_reserved_names > MAX_TYPE_INFERENCE_NODES
    {
        return Err(HelperInstanceError::InvalidLimits);
    }
    if selected.reserved_names.len() > limits.max_reserved_names
        || selected
            .reserved_names
            .iter()
            .any(|name| !valid_local_name(name))
    {
        return Err(HelperInstanceError::ReservedNames);
    }
    let template = selected.body.template();
    let template_limits = HelperTemplateLimits {
        max_nodes: limits.source.max_lowered_nodes,
        max_depth: limits.source.max_lowered_depth,
        max_bindings: limits.max_bindings,
        max_parameters: limits.max_parameters,
    };
    let shape = preflight_template(template.parameters(), template.body(), template_limits)
        .map_err(HelperInstanceError::Template)?;
    Ok((template_limits, shape))
}

pub(crate) fn finish_preflighted_instance<
    'body,
    Field,
    Operation,
    HostEffect,
    IrResult,
    ResultType,
    Adapter,
>(
    selected: SelectedHelper<'body, '_, Node<Field, Operation, HostEffect, IrResult>, ResultType>,
    arguments: Vec<HelperInstanceArgument<Node<Field, Operation, HostEffect, IrResult>>>,
    limits: HelperInstanceLimits,
    used_source_nodes: &mut usize,
    adapter: &mut Adapter,
    template_limits: HelperTemplateLimits,
    shape: HelperTemplateShape,
) -> HelperInstanceResult<
    'body,
    Node<Field, Operation, HostEffect, IrResult>,
    ResultType,
    Adapter::Error,
>
where
    Adapter: HelperInstanceFinisher<Field, Operation, HostEffect, IrResult, ResultType>,
{
    let template = selected.body.template();
    let parameters = template.parameters();
    if arguments.len() != parameters.len() {
        return Err(HelperInstanceError::ArgumentCount);
    }
    let hygiene = HelperHygieneLimits {
        max_nodes: limits.source.max_lowered_nodes,
        max_depth: limits.source.max_lowered_depth,
        max_bindings: limits.max_bindings,
        max_reserved_names: limits.max_reserved_names,
    };
    let mut reserved = selected
        .reserved_names
        .iter()
        .map(|name| (*name).to_owned())
        .collect::<HashSet<_>>();
    let mut budget = StructureBudget::new(
        limits.source.max_lowered_nodes,
        limits.source.max_lowered_depth,
    );
    for index in 0..arguments.len() {
        budget
            .visit(index, 0, 0)
            .map_err(HelperInstanceError::Output)?;
    }
    if shape.depth + arguments.len() > limits.source.max_lowered_depth {
        return Err(HelperInstanceError::Output(StructureError::DepthLimit));
    }
    for (index, argument) in arguments.iter().enumerate() {
        budget
            .check_pending(0, arguments.len() - index + shape.nodes)
            .map_err(HelperInstanceError::Output)?;
        physical(&argument.value, index + 1, &mut budget).map_err(HelperInstanceError::Output)?;
        collect_names(&argument.value, hygiene, &mut reserved)
            .map_err(HelperInstanceError::Hygiene)?;
        if reserved.len() > limits.max_reserved_names {
            return Err(HelperInstanceError::ReservedNames);
        }
    }
    budget
        .check_pending(0, shape.nodes)
        .map_err(HelperInstanceError::Output)?;

    for (index, (argument, (_, expected))) in arguments.iter().zip(parameters).enumerate() {
        check_argument_type(
            &ScalarTypeSet::only(*expected),
            argument.value.scalar_argument_type(argument.scalar_type),
        )
        .map_err(|error| HelperInstanceError::Argument { index, error })?;
    }
    for (index, (argument, (_, expected))) in arguments.iter().zip(parameters).enumerate() {
        adapter
            .admit_argument(&argument.value, *expected, index)
            .map_err(|error| HelperInstanceError::Native {
                phase: HelperInstancePhase::Argument { index },
                error,
            })?;
    }
    reserve_helper_expansion(
        used_source_nodes,
        selected.caller_depth,
        selected.body.source_cost(),
        arguments.len(),
        HelperExpansionLimits {
            max_nodes: limits.source.max_source_nodes,
            max_depth: limits.source.max_source_depth,
            max_parameters: limits.max_parameters,
        },
        |index| adapter.argument_depth(&arguments[index].value),
    )
    .map_err(HelperInstanceError::Expansion)?;
    let body = template
        .materialize(template_limits, |body| adapter.materialize(body))
        .map_err(HelperInstanceError::Template)?;
    let mut aliases = Vec::with_capacity(parameters.len());
    for _ in parameters {
        aliases.push(
            adapter
                .fresh_name()
                .map_err(|error| HelperInstanceError::Native {
                    phase: HelperInstancePhase::Alias,
                    error,
                })?,
        );
    }
    let pairs = parameters
        .iter()
        .zip(&aliases)
        .map(|(parameter, alias)| (parameter.0.as_str(), alias.as_str()))
        .collect::<Vec<_>>();
    let reserved = reserved.iter().map(String::as_str).collect::<Vec<_>>();
    let body = hygienic_helper_body(body, &pairs, &reserved, hygiene, || adapter.fresh_name())
        .map_err(HelperInstanceError::Hygiene)?;
    let bindings = aliases
        .into_iter()
        .zip(arguments)
        .map(|(name, argument)| HelperBinding {
            name,
            value: argument.value,
        })
        .collect();
    let value = bind_helper_arguments(
        body,
        bindings,
        HelperBindingLimits {
            max_nodes: limits.source.max_lowered_nodes,
            max_depth: limits.source.max_lowered_depth,
            max_parameters: limits.max_parameters,
        },
    )
    .map_err(HelperInstanceError::Binding)?;
    adapter
        .admit(&value, template.result_type())
        .map_err(|error| HelperInstanceError::Native {
            phase: HelperInstancePhase::Admit,
            error,
        })?;
    Ok((value, template.result_type()))
}
