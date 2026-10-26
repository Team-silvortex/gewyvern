//! One bounded helper source instance through explicit native copying/admission.

use std::fmt;

use leselang_runtime_core::{NamedParameter, ScalarType, StructureError};
use leselang_syntax::{Expression, NamedArgument};

use crate::helper_bindings::HelperBindingError;
use crate::helper_body::HelperBody;
use crate::helper_expansion::HelperExpansionError;
use crate::helper_hygiene::HelperHygieneError;
use crate::helper_source::{
    HelperSignature, HelperSourceError, HelperSourceLimits, lower_helper_arguments,
};
use crate::helper_templates::HelperTemplateError;
use crate::ir::Computation;
use crate::source_call::SourceCallLimits;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HelperInstanceLimits {
    pub source: SourceCallLimits,
    pub max_parameters: usize,
    pub max_bindings: usize,
    pub max_reserved_names: usize,
}

/// Explicit selection of an original admitted body, not a name registry. The
/// caller reserves its whole live prefix/global namespace, including unused names.
pub struct SelectedHelper<'body, 'names, Node, ResultType> {
    pub name: &'body str,
    pub body: &'body HelperBody<Node, ResultType>,
    pub reserved_names: &'names [&'names str],
    pub caller_depth: usize,
}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type HelperInstanceOperand<Node, Error> = Result<(Node, Option<ScalarType>), Error>;

/// Native work is explicit, not Clone, dispatch or an implicit materializer.
/// Argument lowering owns the exact caller scope and complete cold scalar typing.
/// Depth observations retain folded/opaque source depth, not just physical IR.
/// Materialization must faithfully copy/remap every native slot. Final admission
/// corroborates the entire cold output against the original result observation,
/// caller prefix, live schema/version/grants, source costs and canonical policy.
pub trait HelperInstanceAdapter<'source, Field, Operation, HostEffect, IrResult, ResultType> {
    type Error;

    fn lower_argument(
        &mut self,
        argument: &'source NamedArgument,
    ) -> HelperInstanceOperand<Node<Field, Operation, HostEffect, IrResult>, Self::Error>;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HelperInstancePhase {
    Argument { index: usize },
    Alias,
    Admit,
}
pub enum HelperInstanceError<Error> {
    InvalidLimits,
    ReservedNames,
    ArgumentCount,
    Argument {
        index: usize,
        error: leselang_runtime_core::ArgumentTypeError,
    },
    Source(HelperSourceError<Error>),
    Output(StructureError),
    Expansion(HelperExpansionError<Error>),
    Template(HelperTemplateError<Error>),
    Hygiene(HelperHygieneError<Error>),
    Binding(HelperBindingError),
    Native {
        phase: HelperInstancePhase,
        error: Error,
    },
}
impl<Error> fmt::Display for HelperInstanceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "helper instance limits exceed safety ceilings",
            Self::ReservedNames => "helper instance reservations are invalid or exceed their limit",
            Self::ArgumentCount => "helper instance operand count disagrees with its signature",
            Self::Argument { .. } => "helper instance operand must match its declared scalar type",
            Self::Source(_) => "helper instance source arguments are invalid",
            Self::Output(_) => "helper instance output exceeds physical limits",
            Self::Expansion(_) => "helper instance source expansion is invalid",
            Self::Template(_) => "helper instance materialization failed",
            Self::Hygiene(_) => "helper instance name isolation failed",
            Self::Binding(_) => "helper instance parameter wrapping failed",
            Self::Native { .. } => "native helper instance preparation failed",
        })
    }
}
impl<Error> fmt::Debug for HelperInstanceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for HelperInstanceError<Error> {}
pub type HelperInstanceResult<'body, Node, ResultType, Error> =
    Result<(Node, &'body ResultType), HelperInstanceError<Error>>;

struct SourceFinisher<'adapter, 'source, Adapter> {
    adapter: &'adapter mut Adapter,
    source: std::marker::PhantomData<&'source ()>,
}
impl<'source, Field, Operation, HostEffect, IrResult, ResultType, Adapter>
    crate::helper_instance_finish::HelperInstanceFinisher<
        Field,
        Operation,
        HostEffect,
        IrResult,
        ResultType,
    > for SourceFinisher<'_, 'source, Adapter>
where
    Adapter: HelperInstanceAdapter<'source, Field, Operation, HostEffect, IrResult, ResultType>,
{
    type Error = Adapter::Error;
    fn admit_argument(
        &mut self,
        _: &Node<Field, Operation, HostEffect, IrResult>,
        _: ScalarType,
        _: usize,
    ) -> Result<(), Self::Error> {
        // Source lowering has already checked each operand through the native adapter.
        Ok(())
    }
    fn argument_depth(
        &mut self,
        value: &Node<Field, Operation, HostEffect, IrResult>,
    ) -> Result<usize, Self::Error> {
        self.adapter.argument_depth(value)
    }
    fn materialize(
        &mut self,
        body: &Node<Field, Operation, HostEffect, IrResult>,
    ) -> Result<Node<Field, Operation, HostEffect, IrResult>, Self::Error> {
        self.adapter.materialize(body)
    }
    fn fresh_name(&mut self) -> Result<String, Self::Error> {
        self.adapter.fresh_name()
    }
    fn admit(
        &mut self,
        value: &Node<Field, Operation, HostEffect, IrResult>,
        result: &ResultType,
    ) -> Result<(), Self::Error> {
        self.adapter.admit(value, result)
    }
}

/// Lower one selected helper call through the existing argument, reservation,
/// template, hygiene and binding contracts. All signature keys precede argument
/// callbacks; operands compile once in declaration order, in the caller's scope.
/// Cached body shape plus wrappers and shifted arguments must fit before source
/// depth queries, materialization or name allocation. Caller/global reservations
/// and every cold operand lexical name protect later operands from alias capture.
/// Fixed ceilings: 16,384 source/output nodes/reserved names, depth 64, eight
/// parameters and 1,024 active body bindings. Zero/exact limits remain explicit.
///
/// The caller-owned source counter already includes the call and all original
/// operands, including folded weights. Reserve cached body cost plus parameter
/// wrappers at caller_depth before the single materialization hook. Depth observers
/// must retain original source weights. A committed reservation is never refunded
/// on subsequent factory/name/hygiene/wrapper/admission failure or unwind. Hidden
/// copy work and native allocation/Drop remain trusted, not metered or preempted.
///
/// Original operand slots/buffers move unchanged; only language bindings and names
/// are added. The cached body/result are borrowed, never mutated or automatically
/// cloned. The result observation returned is the exact original borrow. Mandatory
/// final admission runs once on complete output, never a partial tree. No native
/// Clone/PartialEq/Debug/serde/Send bound, registry, second IR, evaluation, dispatch,
/// receipt, retry, rollback or durable state is installed. Callbacks may mutate;
/// no native side-effect rollback is promised and a second cleanup panic aborts.
///
/// This is one helper instance, not declaration/dependency compilation or complete
/// source recursion. Whole cold source/operator/schema typing, source/span/selection
/// authenticity, enclosing output/active-prefix budgets and live authority remain
/// host gates. Matching physical shape alone is not semantic identity or permission.
pub fn lower_helper_instance<
    'source,
    'body,
    Field,
    Operation,
    HostEffect,
    IrResult,
    ResultType,
    Adapter,
>(
    expression: &'source Expression,
    selected: SelectedHelper<'body, '_, Node<Field, Operation, HostEffect, IrResult>, ResultType>,
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
    Adapter: HelperInstanceAdapter<'source, Field, Operation, HostEffect, IrResult, ResultType>,
{
    let (template_limits, shape) =
        crate::helper_instance_finish::preflight_instance(&selected, limits)?;
    let template = selected.body.template();
    let parameters = template
        .parameters()
        .iter()
        .map(|(name, ty)| NamedParameter::required(name.as_str(), *ty))
        .collect::<Vec<_>>();
    let arguments = lower_helper_arguments(
        expression,
        HelperSignature {
            name: selected.name,
            parameters: &parameters,
        },
        HelperSourceLimits {
            source: limits.source,
            max_parameters: limits.max_parameters,
        },
        |argument| adapter.lower_argument(argument),
    )
    .map_err(HelperInstanceError::Source)?;

    let arguments = arguments
        .into_iter()
        .map(
            |argument| crate::helper_instance_finish::HelperInstanceArgument {
                scalar_type: Some(argument.parameter.domain),
                value: argument.value,
            },
        )
        .collect();
    crate::helper_instance_finish::finish_preflighted_instance(
        selected,
        arguments,
        limits,
        used_source_nodes,
        &mut SourceFinisher {
            adapter,
            source: std::marker::PhantomData,
        },
        template_limits,
        shape,
    )
}
