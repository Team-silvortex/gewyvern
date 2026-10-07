//! Projection of explicit bound native type observations, never name-only exports.

use std::fmt;

use leselang_runtime_core::{ScalarType, StructureBudget};
use leselang_syntax::{Expression, Span};

use crate::ir::Computation;
use crate::projection_source::{
    ProjectionSourceError, field_source, member_source, preflight_names,
};
use crate::pure_typing::{MAX_TYPE_INFERENCE_BINDINGS, PureType, PureTypeError, valid_local_name};
use crate::source_call::{SourceCallError, SourceCallLimits, preflight, span, valid_limits};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BoundProjectionSourceLimits {
    pub source: SourceCallLimits,
    pub max_bindings: usize,
}

/// Explicit export queries on the exact borrowed native binding observation.
/// Result metadata is not a received value, schema grant or group certificate.
/// Queries must corroborate original native schema/owner/version and exact closed
/// field/member exports; member must reject non-groups and foreign/look-alike
/// observations. Native returned slots/observations move, with no Clone/PartialEq/
/// Debug/serde/Send bounds. No lowering, result copying or dispatch occurs here.
pub trait BoundProjectionSourceEnvironment<'source, Field, Operation> {
    type Result;
    type Error;

    fn field(
        &mut self,
        result: &Self::Result,
        name: &'source str,
    ) -> Result<Option<(Field, ScalarType)>, Self::Error>;
    fn member(
        &mut self,
        result: &Self::Result,
        group: &'source str,
        name: &'source str,
    ) -> Result<Option<(Operation, Self::Result)>, Self::Error>;
}

pub enum BoundProjectionSourceError<Error> {
    Projection(ProjectionSourceError<Error>),
    Scope { span: Span, error: PureTypeError },
    BoundReference { span: Span },
    Unbound { span: Span },
}
impl<Error> fmt::Display for BoundProjectionSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Projection(error) => fmt::Display::fmt(error, formatter),
            Self::Scope { .. } => formatter.write_str("bound projection type scope is invalid"),
            Self::BoundReference { .. } => {
                formatter.write_str("bound projection requires a direct bound reference")
            }
            Self::Unbound { .. } => {
                formatter.write_str("bound projection input is absent from the type scope")
            }
        }
    }
}
impl<Error> fmt::Debug for BoundProjectionSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for BoundProjectionSourceError<Error> {}

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type BoundProjectionSourceResult<Node, Type, Error> =
    Result<(Node, PureType<Type>), BoundProjectionSourceError<Error>>;

/// Construct field/member IR directly from an explicit read-only native type
/// prefix, including `ControlSourceScope::bindings()`. The prefix is borrowed,
/// bounded and unique: no type clone, mutable host frame or lower-value callback.
/// `field` accepts a direct result reference and builds one Local plus Field;
/// `member` accepts a bound group reference and builds only Member, not a fake
/// Local or implicit export union. Other pure inputs use the existing projection
/// entry with an explicit child compiler. Bound scalar inputs never query native
/// exports. Missing names remain missing rather than consulting a global registry.
///
/// Whole cold source/projection metadata, prefix names/counts and minimum output
/// depth/nodes precede native queries. Fixed ceilings: 16,384 nodes, depth 64,
/// 64 operands, 1,024 bindings. Name literals are source metadata, not IR nodes.
/// Original native observations/names are borrowed, returned fields/operations
/// and member observations move unchanged. Queries run once; errors/unwind stop
/// without partial output, retry, dispatch or native side-effect rollback.
///
/// This is type-directed construction, not full lexical/type inference, native
/// value projection, actual result acceptance or an execution grant. Whole cold
/// native IR admission, source/span authenticity, hidden expansion/work, ingress,
/// live version/grants, mutation and resource policy remain adapter-owned. Native
/// work/allocation/Drop cannot be preempted; a second cleanup panic can abort.
pub fn lower_bound_projection_source<'source, Field, Operation, HostEffect, IrResult, Environment>(
    expression: &'source Expression,
    prefix: &[(&'source str, &PureType<Environment::Result>)],
    limits: BoundProjectionSourceLimits,
    environment: &mut Environment,
) -> BoundProjectionSourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    Environment::Result,
    Environment::Error,
>
where
    Environment: BoundProjectionSourceEnvironment<'source, Field, Operation>,
{
    let projection_error = BoundProjectionSourceError::Projection;
    if !valid_limits(limits.source) || limits.max_bindings > MAX_TYPE_INFERENCE_BINDINGS {
        return Err(projection_error(ProjectionSourceError::Source(
            SourceCallError::InvalidLimits,
        )));
    }
    preflight(expression, limits.source)
        .map_err(ProjectionSourceError::Source)
        .map_err(projection_error)?;
    preflight_names(expression).map_err(projection_error)?;
    let at = span(expression);
    if prefix.len() > limits.max_bindings
        || prefix.iter().enumerate().any(|(index, (name, _))| {
            !valid_local_name(name) || prefix[..index].iter().any(|(old, _)| old == name)
        })
    {
        return Err(BoundProjectionSourceError::Scope {
            span: at,
            error: PureTypeError::InvalidScope,
        });
    }
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(projection_error(ProjectionSourceError::NotProjection {
            span: at,
        }));
    };
    if !matches!(callee.as_str(), "field" | "member") {
        return Err(projection_error(ProjectionSourceError::NotProjection {
            span: at,
        }));
    }
    let mut budget = StructureBudget::new(
        limits.source.max_lowered_nodes,
        limits.source.max_lowered_depth,
    );
    budget
        .visit(0, 0, usize::from(callee == "field"))
        .map_err(|error| projection_error(ProjectionSourceError::Generation { span: at, error }))?;
    let form = field_source(arguments, at).map_err(projection_error)?;
    let name = form.literal_name().map_err(projection_error)?;
    let input_at = span(form.value);
    let Expression::Reference { name: bound, .. } = form.value else {
        return Err(BoundProjectionSourceError::BoundReference { span: input_at });
    };
    let observed = prefix
        .iter()
        .find(|(old, _)| *old == bound)
        .map(|(_, ty)| *ty)
        .ok_or(BoundProjectionSourceError::Unbound { span: input_at })?;
    let PureType::Result(result) = observed else {
        return Err(projection_error(ProjectionSourceError::NonResult {
            span: input_at,
        }));
    };
    let native_error = |error| projection_error(ProjectionSourceError::Native { span: at, error });
    if callee == "member" {
        let member = member_source(arguments, at).map_err(projection_error)?;
        let (operation, result) = environment
            .member(result, member.group, member.name)
            .map_err(native_error)?
            .ok_or(projection_error(ProjectionSourceError::MemberNotExported {
                span: at,
            }))?;
        return Ok((member.construct(operation), PureType::Result(result)));
    }
    budget
        .visit(1, 0, 0)
        .map_err(|error| projection_error(ProjectionSourceError::Generation { span: at, error }))?;
    let (field, ty) = environment
        .field(result, name)
        .map_err(native_error)?
        .ok_or(projection_error(ProjectionSourceError::FieldNotExported {
            span: at,
        }))?;
    Ok((
        form.construct(
            Node::Local {
                name: bound.to_owned(),
            },
            field,
        ),
        PureType::Scalar(ty),
    ))
}
