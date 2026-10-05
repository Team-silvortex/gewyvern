//! Native projection source construction, not result acceptance or effect authority.

use std::fmt;

use leselang_runtime_core::{ScalarType, StructureBudget, StructureError};
use leselang_syntax::{Expression, NamedArgument, Span};

use crate::ir::Computation;
use crate::pure_typing::{
    PureType, PureTypeError, preflight_with_budget, valid_local_name, valid_member_name,
};
use crate::source_call::{SourceCallError, SourceCallLimits, preflight, span, valid_limits};

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

pub type ProjectionSourceResult<Node, Result, Error> =
    std::result::Result<(Node, PureType<Result>), ProjectionSourceError<Error>>;

pub type NativeProjectionInputResult<Node, Result, Error> =
    std::result::Result<(Node, PureType<Result>), Error>;

/// Trusted native construction/type observations, not actual result values.
///
/// Callbacks receive original borrowed AST/name metadata. Lowering must bound
/// hidden expansion/work and use the exact caller lexical prefix. Field/member
/// queries must reject exports absent from that exact result/group, not a union
/// of unrelated schemas. They must not dispatch effects or create receipts.
/// No native slot, result observation or error needs Clone/Debug/serde/Send.
/// Complete cold IR type inference is still mandatory before execution.
pub trait ProjectionSourceEnvironment<'source, Field, Operation, HostEffect, IrResult> {
    type Result;
    type Error;

    fn lower_value(
        &mut self,
        expression: &'source Expression,
    ) -> NativeProjectionInputResult<
        Node<Field, Operation, HostEffect, IrResult>,
        Self::Result,
        Self::Error,
    >;

    fn field(
        &mut self,
        result: &Self::Result,
        name: &'source str,
    ) -> Result<Option<(Field, ScalarType)>, Self::Error>;

    fn member(
        &mut self,
        group: &'source str,
        name: &'source str,
    ) -> Result<Option<(Operation, Self::Result)>, Self::Error>;
}

pub enum ProjectionSourceError<Error> {
    Source(SourceCallError<Error>),
    NotProjection { span: Span },
    Names { span: Span },
    FieldName { span: Span },
    MemberShape { span: Span },
    MemberNames { span: Span },
    NonResult { span: Span },
    FieldNotExported { span: Span },
    MemberNotExported { span: Span },
    Native { span: Span, error: Error },
    Generation { span: Span, error: StructureError },
    Produced { span: Span, error: PureTypeError },
}

impl<Error> fmt::Display for ProjectionSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Source(_) => "projection source shape is invalid",
            Self::NotProjection { .. } => "projection source requires field or member",
            Self::Names { .. } => "projection requires exactly value and name operands",
            Self::FieldName { .. } => "field name must be a string literal",
            Self::MemberShape { .. } => "member requires a group reference and literal name",
            Self::MemberNames { .. } => "member source names are invalid",
            Self::NonResult { .. } => "field input must observe a native result type",
            Self::FieldNotExported { .. } => "field is not exported by this native result",
            Self::MemberNotExported { .. } => "member is not exported by this bound group",
            Self::Native { .. } => "native projection source preparation failed",
            Self::Generation { .. } => "projection source generation exceeds its limits",
            Self::Produced { .. } => "native projection input IR is invalid",
        })
    }
}
impl<Error> fmt::Debug for ProjectionSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for ProjectionSourceError<Error> {}

pub(crate) struct FieldSource<'source> {
    pub value: &'source Expression,
    name: &'source Expression,
}
impl<'source> FieldSource<'source> {
    pub fn literal_name<Error>(&self) -> Result<&'source str, ProjectionSourceError<Error>> {
        let Expression::String { value, .. } = self.name else {
            return Err(ProjectionSourceError::FieldName {
                span: span(self.name),
            });
        };
        Ok(value)
    }

    pub fn construct<Field, Operation, HostEffect, IrResult>(
        &self,
        value: Node<Field, Operation, HostEffect, IrResult>,
        field: Field,
    ) -> Node<Field, Operation, HostEffect, IrResult> {
        Computation::Field {
            value: Box::new(value),
            field,
        }
    }
}

pub(crate) fn field_source<Error>(
    arguments: &[NamedArgument],
    at: Span,
) -> Result<FieldSource<'_>, ProjectionSourceError<Error>> {
    if arguments.len() != 2
        || ["value", "name"].iter().any(|name| {
            arguments
                .iter()
                .filter(|argument| argument.name == *name)
                .count()
                != 1
        })
    {
        return Err(ProjectionSourceError::Names { span: at });
    }
    let get = |name: &str| {
        arguments
            .iter()
            .find(|argument| argument.name == name)
            .map(|argument| &argument.value)
            .ok_or(ProjectionSourceError::Names { span: at })
    };
    Ok(FieldSource {
        value: get("value")?,
        name: get("name")?,
    })
}

pub(crate) struct MemberSource<'source> {
    pub group: &'source str,
    pub name: &'source str,
}
impl MemberSource<'_> {
    fn check_names<Error>(&self, at: Span) -> Result<(), ProjectionSourceError<Error>> {
        if !valid_local_name(self.group) || !valid_member_name(self.name) {
            return Err(ProjectionSourceError::MemberNames { span: at });
        }
        Ok(())
    }

    pub fn construct<Field, Operation, HostEffect, IrResult>(
        &self,
        operation: Operation,
    ) -> Node<Field, Operation, HostEffect, IrResult> {
        Computation::Member {
            group: self.group.to_owned(),
            name: self.name.to_owned(),
            operation,
        }
    }
}

pub(crate) fn member_source<Error>(
    arguments: &[NamedArgument],
    at: Span,
) -> Result<MemberSource<'_>, ProjectionSourceError<Error>> {
    let source = field_source(arguments, at)?;
    let (Expression::Reference { name: group, .. }, Expression::String { value: name, .. }) =
        (source.value, source.name)
    else {
        return Err(ProjectionSourceError::MemberShape { span: at });
    };
    Ok(MemberSource { group, name })
}

fn preflight_names<Error>(expression: &Expression) -> Result<(), ProjectionSourceError<Error>> {
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        if let Expression::Call {
            callee, arguments, ..
        } = expression
        {
            if callee == "field" {
                field_source(arguments, span(expression))?.literal_name()?;
            } else if callee == "member" {
                member_source(arguments, span(expression))?.check_names(span(expression))?;
            }
            pending.extend(arguments.iter().rev().map(|argument| &argument.value));
        }
    }
    Ok(())
}

/// Construct one native field/member projection on the original generic IR.
///
/// Physical source and minimum generated capacity precede native work. All
/// cold projection signatures/literal metadata inside the input source precede
/// every callback. Field value lowering receives the original AST, then its
/// entire pure generated IR is checked before the native field export query.
/// Member metadata is a bound group reference plus a literal step name: it does
/// not evaluate either operand, synthesize a Local node or grant a group export.
/// Name literals are source metadata, not generated IR nodes. Field roots and
/// all native input nodes share one generated budget; no folding refund occurs.
///
/// Native observations are not type certificates. Complete cold inference,
/// actual result acceptance, live authority and value-domain validation remain
/// adapter-owned. Hidden native expansion/allocation/Drop cannot be preempted.
/// Errors/unwind drop partial native IR without rollback, retries or dispatch.
pub fn lower_projection_source<'source, Field, Operation, HostEffect, IrResult, Environment>(
    expression: &'source Expression,
    limits: SourceCallLimits,
    environment: &mut Environment,
) -> ProjectionSourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    Environment::Result,
    Environment::Error,
>
where
    Environment: ProjectionSourceEnvironment<'source, Field, Operation, HostEffect, IrResult>,
{
    if !valid_limits(limits) {
        return Err(ProjectionSourceError::Source(
            SourceCallError::InvalidLimits,
        ));
    }
    preflight(expression, limits).map_err(ProjectionSourceError::Source)?;
    preflight_names(expression)?;
    let at = span(expression);
    let Expression::Call {
        callee, arguments, ..
    } = expression
    else {
        return Err(ProjectionSourceError::NotProjection { span: at });
    };
    if !matches!(callee.as_str(), "field" | "member") {
        return Err(ProjectionSourceError::NotProjection { span: at });
    }
    let mut budget = StructureBudget::new(limits.max_lowered_nodes, limits.max_lowered_depth);
    budget
        .visit(0, 0, usize::from(callee == "field"))
        .map_err(|error| ProjectionSourceError::Generation { span: at, error })?;
    if callee == "member" {
        let source = member_source(arguments, at)?;
        let (operation, result) = environment
            .member(source.group, source.name)
            .map_err(|error| ProjectionSourceError::Native { span: at, error })?
            .ok_or(ProjectionSourceError::MemberNotExported { span: at })?;
        return Ok((source.construct(operation), PureType::Result(result)));
    }
    budget
        .check_pending(0, 1)
        .map_err(|error| ProjectionSourceError::Generation { span: at, error })?;
    let source = field_source(arguments, at)?;
    let name = source.literal_name()?;
    let input_at = span(source.value);
    let (value, input_type) =
        environment
            .lower_value(source.value)
            .map_err(|error| ProjectionSourceError::Native {
                span: input_at,
                error,
            })?;
    preflight_with_budget(&value, 1, &mut budget).map_err(|error| {
        ProjectionSourceError::Produced {
            span: input_at,
            error,
        }
    })?;
    let PureType::Result(result) = input_type else {
        return Err(ProjectionSourceError::NonResult { span: input_at });
    };
    let (field, ty) = environment
        .field(&result, name)
        .map_err(|error| ProjectionSourceError::Native { span: at, error })?
        .ok_or(ProjectionSourceError::FieldNotExported { span: at })?;
    Ok((source.construct(value, field), PureType::Scalar(ty)))
}
