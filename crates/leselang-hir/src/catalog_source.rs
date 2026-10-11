//! Catalog-driven compilation of scalar/call Bind/Choose expressions.

use std::{borrow::Borrow, fmt, marker::PhantomData};

use leselang_runtime_core::{ScalarArgumentDomain, ScalarType};
use leselang_syntax::{Expression, Span};

use crate::bound_projection_source::{
    BoundProjectionSourceEnvironment, BoundProjectionSourceError, BoundProjectionSourceLimits,
    lower_bound_projection_source,
};
use crate::call_typing::{CallTypeHost, CallTypeLimits};
use crate::catalog_program::{CatalogProgramContext, CatalogProgramError};
use crate::control_source::{
    ControlSourceAdapter, ControlSourceError, ControlSourceLimits, ControlSourceScope,
    lower_preflighted_control_source, preflight_control_source,
};
use crate::flow_typing::{
    CallFlowEnvironment, CallFlowTypeError, CallFlowTypeLimits, infer_call_flow_type,
};
use crate::ir::Computation;
use crate::pure_typing::{MAX_TYPE_INFERENCE_NODES, PureType, TypeInferenceLimits};
use crate::scalar_source::{
    ScalarSourceError, ScalarSourceLimits, lower_scalar_source_with_scope, primitive,
};
use crate::source_call::{SourceCallError, SourceCallHost, lower_source_call, span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogSourceLimits {
    pub control: ControlSourceLimits,
    /// Counts all cold source call sites, not only the selected path.
    pub max_calls: usize,
}

/// Host metadata and slot construction, never a recursive child compiler.
/// Result query identifiers retain the existing Clone/PartialEq contract; they
/// may borrow nonclone native declarations. IR slots need no Clone/serde/Send.
/// Mapping must preserve the exact source/schema operation identity. Export
/// queries must retain the same closed native owner/schema as type inference.
/// Native work, interior mutation, allocation, equality and unwind remain trusted
/// adapter responsibilities, not preempted or rolled back by this compiler.
pub trait CatalogSourceEnvironment<'source, 'schema, Field, Operation, Declaration>:
    CallFlowEnvironment<'schema, Field, Operation, Declaration>
{
    type Error;

    fn source_operation(&self, original: &Operation) -> Result<Operation, Self::Error>;
    fn source_field(
        &self,
        result: &Self::Result,
        name: &'source str,
    ) -> Result<Option<(Field, ScalarType)>, Self::Error>;
    fn source_member(
        &self,
        result: &Self::Result,
        group: &'source str,
        name: &'source str,
    ) -> Result<Option<(Operation, Self::Result)>, Self::Error>;
}

pub enum CatalogSourceError<Error> {
    Control(Box<ControlSourceError<Self>>),
    Scalar {
        span: Span,
        error: Box<ScalarSourceError<Self>>,
    },
    Call {
        span: Span,
        error: Box<SourceCallError<Self>>,
    },
    Projection(Box<BoundProjectionSourceError<Error>>),
    Mapping {
        span: Span,
        error: Error,
    },
    OperationIdentity {
        span: Span,
    },
    UnknownLocal {
        span: Span,
    },
    NonScalar {
        span: Span,
    },
    ResultType {
        span: Span,
    },
    UnsupportedSource {
        span: Span,
    },
    Type(CallFlowTypeError),
    InconsistentType,
    Program(Box<CatalogProgramError<Self>>),
    Group(Box<crate::catalog_group_source::CatalogGroupSourceError<Error>>),
}
impl<Error> fmt::Display for CatalogSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Control(_) => "catalog expression control source rejected",
            Self::Scalar { .. } => "catalog expression scalar source rejected",
            Self::Call { .. } => "catalog expression operation signature rejected",
            Self::Projection(_) => "catalog expression bound projection rejected",
            Self::Mapping { .. } => "catalog expression native operation mapping failed",
            Self::OperationIdentity { .. } => {
                "catalog expression mapped operation differs from source"
            }
            Self::UnknownLocal { .. } => "catalog expression local is not bound",
            Self::NonScalar { .. } => "catalog expression operand must be pure and scalar",
            Self::ResultType { .. } => "catalog expression operation has no declared query type",
            Self::UnsupportedSource { .. } => "catalog expression source profile is unsupported",
            Self::Type(_) => "catalog expression whole cold type admission rejected",
            Self::InconsistentType => "catalog expression inferred type differs from construction",
            Self::Program(_) => "catalog program declaration or expansion rejected",
            Self::Group(_) => "catalog program group entry rejected",
        })
    }
}
impl<Error> fmt::Debug for CatalogSourceError<Error> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}
impl<Error> std::error::Error for CatalogSourceError<Error> {}
pub type CatalogSourceResult<Node, ResultType, Error> =
    Result<(Node, PureType<ResultType>), CatalogSourceError<Error>>;
type ScalarResult<Node, Error> = Result<(Node, ScalarType), CatalogSourceError<Error>>;
type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

fn intrinsic(expression: &Expression) -> bool {
    primitive(expression)
        || matches!(expression, Expression::Call { callee, .. }
            if matches!(callee.as_str(), "bind" | "choose" | "loop" | "fold" | "field" | "member"))
}

// Physical/lexical preflight has already bounded this whole cold source.
pub(crate) fn cold_catalog<Operation, Domain, Declaration, Capability, Error>(
    source: &Expression,
    host: &SourceCallHost<'_, '_, Operation, Domain, Declaration, Capability>,
    limits: CatalogSourceLimits,
    local_call: impl FnMut(&Expression) -> Result<bool, CatalogSourceError<Error>>,
) -> Result<usize, CatalogSourceError<Error>>
where
    Operation: Borrow<str> + PartialEq,
    Capability: PartialEq,
{
    cold_catalog_profile(source, host, limits, false, local_call)
}

pub(crate) fn cold_catalog_profile<Operation, Domain, Declaration, Capability, Error>(
    source: &Expression,
    host: &SourceCallHost<'_, '_, Operation, Domain, Declaration, Capability>,
    limits: CatalogSourceLimits,
    groups: bool,
    mut local_call: impl FnMut(&Expression) -> Result<bool, CatalogSourceError<Error>>,
) -> Result<usize, CatalogSourceError<Error>>
where
    Operation: Borrow<str> + PartialEq,
    Capability: PartialEq,
{
    let mut pending = vec![source];
    let mut calls = 0usize;
    while let Some(node) = pending.pop() {
        if let Expression::Call {
            callee, arguments, ..
        } = node
        {
            let at = span(node);
            if callee == "repeat" || (!groups && matches!(callee.as_str(), "seq" | "all")) {
                return Err(CatalogSourceError::UnsupportedSource { span: at });
            }
            if !matches!(callee.as_str(), "seq" | "all") && !intrinsic(node) && !local_call(node)? {
                if calls >= limits.max_calls {
                    return Err(CatalogSourceError::Type(CallFlowTypeError::CallLimit));
                }
                calls += 1;
                let error = |error| CatalogSourceError::Call {
                    span: at,
                    error: Box::new(error),
                };
                let schema = host
                    .catalog
                    .authorize(callee.as_str(), host.version, host.granted)
                    .map_err(|e| error(SourceCallError::Catalog(e)))?;
                if schema.parameters.len() > limits.control.source.max_arguments {
                    return Err(error(SourceCallError::ParameterLimit));
                }
                let names: Vec<_> = arguments
                    .iter()
                    .map(|argument| argument.name.as_str())
                    .collect();
                schema
                    .bind_arguments(&names)
                    .map_err(|e| error(SourceCallError::Names(e)))?;
            }
            pending.extend(arguments.iter().rev().map(|argument| &argument.value));
        }
    }
    Ok(calls)
}

pub(crate) struct Compiler<
    'a,
    'source,
    'host,
    'schema,
    Operation,
    Domain,
    Declaration,
    Capability,
    Environment,
    Locals = (),
> {
    host: &'a SourceCallHost<'host, 'schema, Operation, Domain, Declaration, Capability>,
    environment: &'a Environment,
    limits: CatalogSourceLimits,
    program: Option<&'a CatalogProgramContext<'source>>,
    locals: Locals,
}

// Internal language profile selection, never a developer-owned recursive hook.
type LocalSourceResult<Node, ResultType, Error> =
    Option<CatalogSourceResult<Node, ResultType, Error>>;
pub(crate) trait CatalogLocalSource<
    'source,
    Field,
    Operation,
    HostEffect,
    IrResult,
    ResultType,
    Error,
>
{
    fn contains(&self, source: &Expression) -> bool;
    fn lower(
        &self,
        source: &'source Expression,
        prefix: &[(&'source str, &PureType<ResultType>)],
    ) -> LocalSourceResult<Node<Field, Operation, HostEffect, IrResult>, ResultType, Error>;
}
impl<'source, Field, Operation, HostEffect, IrResult, ResultType, Error>
    CatalogLocalSource<'source, Field, Operation, HostEffect, IrResult, ResultType, Error> for ()
{
    fn contains(&self, _: &Expression) -> bool {
        false
    }
    fn lower(
        &self,
        _: &'source Expression,
        _: &[(&'source str, &PureType<ResultType>)],
    ) -> LocalSourceResult<Node<Field, Operation, HostEffect, IrResult>, ResultType, Error> {
        None
    }
}
struct Projection<'a, 'schema, Declaration, Environment>(
    &'a Environment,
    PhantomData<fn(&'schema ()) -> Declaration>,
);
impl<'source, 'schema, Field, Operation, Declaration, Environment>
    BoundProjectionSourceEnvironment<'source, Field, Operation>
    for Projection<'_, 'schema, Declaration, Environment>
where
    Environment: CatalogSourceEnvironment<'source, 'schema, Field, Operation, Declaration>,
{
    type Result = Environment::Result;
    type Error = Environment::Error;
    fn field(
        &mut self,
        result: &Self::Result,
        name: &'source str,
    ) -> Result<Option<(Field, ScalarType)>, Self::Error> {
        self.0.source_field(result, name)
    }
    fn member(
        &mut self,
        result: &Self::Result,
        group: &'source str,
        name: &'source str,
    ) -> Result<Option<(Operation, Self::Result)>, Self::Error> {
        self.0.source_member(result, group, name)
    }
}

impl<'source, 'schema, Operation, Domain, Declaration, Capability, Environment, Locals>
    Compiler<
        '_,
        'source,
        '_,
        'schema,
        Operation,
        Domain,
        Declaration,
        Capability,
        Environment,
        Locals,
    >
{
    fn scalar<Field, HostEffect, IrResult>(
        &self,
        source: &'source Expression,
        prefix: &[(&'source str, &PureType<Environment::Result>)],
    ) -> ScalarResult<Node<Field, Operation, HostEffect, IrResult>, Environment::Error>
    where
        Operation: Borrow<str> + PartialEq,
        Domain: ScalarArgumentDomain,
        Capability: PartialEq,
        Environment: CatalogSourceEnvironment<'source, 'schema, Field, Operation, Declaration>,
        Locals: CatalogLocalSource<
                'source,
                Field,
                Operation,
                HostEffect,
                IrResult,
                Environment::Result,
                Environment::Error,
            >,
    {
        let scalar_prefix: Vec<_> = prefix
            .iter()
            .filter_map(|(name, ty)| ty.scalar_type().map(|ty| (*name, ty)))
            .collect();
        lower_scalar_source_with_scope(
            source,
            ScalarSourceLimits {
                source: self.limits.control.source,
                max_bindings: self.limits.control.max_bindings,
            },
            &scalar_prefix,
            |child, local| {
                if self.program.is_some() || self.locals.contains(child) {
                    let scalar_types: Vec<_> = local
                        .bindings()
                        .iter()
                        .map(|(name, ty)| (*name, PureType::Scalar(*ty)))
                        .collect();
                    let mut extended: Vec<_> = prefix
                        .iter()
                        .copied()
                        .filter(|(_, ty)| ty.scalar_type().is_none())
                        .collect();
                    extended.extend(scalar_types.iter().map(|(name, ty)| (*name, ty)));
                    if let Some(lowered) = self.locals.lower(child, &extended).or_else(|| {
                        self.program.and_then(|program| {
                            program.lower_helper(child, &extended, self.host, self.environment)
                        })
                    }) {
                        let (node, ty) = lowered?;
                        let PureType::Scalar(ty) = ty else {
                            return Err(CatalogSourceError::NonScalar { span: span(child) });
                        };
                        return Ok((node, Some(ty)));
                    }
                }
                if matches!(child, Expression::Reference { .. }) {
                    return Err(CatalogSourceError::NonScalar { span: span(child) });
                }
                let (node, ty) = lower_bound_projection_source(
                    child,
                    prefix,
                    BoundProjectionSourceLimits {
                        source: self.limits.control.source,
                        max_bindings: self.limits.control.max_bindings,
                    },
                    &mut Projection(
                        self.environment,
                        PhantomData::<fn(&'schema ()) -> Declaration>,
                    ),
                )
                .map_err(|e| CatalogSourceError::Projection(Box::new(e)))?;
                let PureType::Scalar(ty) = ty else {
                    return Err(CatalogSourceError::NonScalar { span: span(child) });
                };
                Ok((node, Some(ty)))
            },
        )
        .map_err(|error| CatalogSourceError::Scalar {
            span: span(source),
            error: Box::new(error),
        })
    }
}

impl<'a, 'source, 'host, 'schema, Operation, Domain, Declaration, Capability, Environment>
    Compiler<'a, 'source, 'host, 'schema, Operation, Domain, Declaration, Capability, Environment>
{
    pub(crate) fn new(
        host: &'a SourceCallHost<'host, 'schema, Operation, Domain, Declaration, Capability>,
        environment: &'a Environment,
        limits: CatalogSourceLimits,
    ) -> Self {
        Self {
            host,
            environment,
            limits,
            program: None,
            locals: (),
        }
    }

    pub(crate) fn with_program(
        mut self,
        program: Option<&'a CatalogProgramContext<'source>>,
    ) -> Self {
        self.program = program;
        self
    }
}

impl<'a, 'source, 'host, 'schema, Operation, Domain, Declaration, Capability, Environment, Locals>
    Compiler<
        'a,
        'source,
        'host,
        'schema,
        Operation,
        Domain,
        Declaration,
        Capability,
        Environment,
        Locals,
    >
{
    pub(crate) fn with_locals<Next>(
        self,
        locals: Next,
    ) -> Compiler<
        'a,
        'source,
        'host,
        'schema,
        Operation,
        Domain,
        Declaration,
        Capability,
        Environment,
        Next,
    > {
        Compiler {
            host: self.host,
            environment: self.environment,
            limits: self.limits,
            program: self.program,
            locals,
        }
    }
}

impl<
    'source,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Domain,
    Declaration,
    Capability,
    Environment,
    Locals,
> ControlSourceAdapter<'source, Field, Operation, HostEffect, IrResult>
    for Compiler<
        '_,
        'source,
        '_,
        'schema,
        Operation,
        Domain,
        Declaration,
        Capability,
        Environment,
        Locals,
    >
where
    Operation: Borrow<str> + PartialEq,
    Domain: ScalarArgumentDomain,
    Capability: PartialEq,
    Environment: CatalogSourceEnvironment<'source, 'schema, Field, Operation, Declaration>,
    Locals: CatalogLocalSource<
            'source,
            Field,
            Operation,
            HostEffect,
            IrResult,
            Environment::Result,
            Environment::Error,
        >,
{
    type Type = PureType<Environment::Result>;
    type Error = CatalogSourceError<Environment::Error>;
    fn lower_leaf(
        &mut self,
        source: &'source Expression,
        scope: ControlSourceScope<'_, 'source, Self::Type>,
    ) -> Result<(Node<Field, Operation, HostEffect, IrResult>, Self::Type), Self::Error> {
        if let Expression::Reference { name, .. } = source {
            let ty = scope
                .get(name)
                .ok_or(CatalogSourceError::UnknownLocal { span: span(source) })?;
            return Ok((Node::Local { name: name.clone() }, ty.clone()));
        }
        if matches!(source, Expression::Call { callee, .. } if matches!(callee.as_str(), "field" | "member"))
        {
            return lower_bound_projection_source(
                source,
                scope.bindings(),
                BoundProjectionSourceLimits {
                    source: self.limits.control.source,
                    max_bindings: self.limits.control.max_bindings,
                },
                &mut Projection(
                    self.environment,
                    PhantomData::<fn(&'schema ()) -> Declaration>,
                ),
            )
            .map_err(|e| CatalogSourceError::Projection(Box::new(e)));
        }
        if !intrinsic(source) {
            if let Some(lowered) = self.locals.lower(source, scope.bindings()) {
                return lowered;
            }
            if let Some(lowered) = self.program.and_then(|program| {
                program.lower_helper(source, scope.bindings(), self.host, self.environment)
            }) {
                return lowered;
            }
            let prepared =
                lower_source_call(source, self.host, self.limits.control.source, |arg| {
                    self.scalar(&arg.value, scope.bindings())
                        .map(|(node, ty)| (node, Some(ty)))
                })
                .map_err(|error| CatalogSourceError::Call {
                    span: span(source),
                    error: Box::new(error),
                })?;
            let schema = prepared.schema();
            let operation = self
                .environment
                .source_operation(&schema.key)
                .map_err(|error| CatalogSourceError::Mapping {
                    span: span(source),
                    error,
                })?;
            if operation != schema.key || operation.borrow() != schema.key.borrow() {
                return Err(CatalogSourceError::OperationIdentity { span: span(source) });
            }
            let ty = self
                .environment
                .call_result_type(&schema.key, &schema.result)
                .ok_or(CatalogSourceError::ResultType { span: span(source) })?;
            return Ok((
                Node::Call {
                    operation,
                    arguments: prepared.into_arguments(),
                },
                ty,
            ));
        }
        self.scalar(source, scope.bindings())
            .map(|(node, ty)| (node, PureType::Scalar(ty)))
    }
    fn boolean(
        &mut self,
        _: &Node<Field, Operation, HostEffect, IrResult>,
        ty: &Self::Type,
    ) -> Result<bool, Self::Error> {
        Ok(matches!(ty, PureType::Scalar(ScalarType::Boolean)))
    }
    fn same(&mut self, left: &Self::Type, right: &Self::Type) -> Result<bool, Self::Error> {
        Ok(left == right)
    }
    fn admit(
        &mut self,
        _: &'source Expression,
        node: &Node<Field, Operation, HostEffect, IrResult>,
        ty: &Self::Type,
        scope: ControlSourceScope<'_, 'source, Self::Type>,
    ) -> Result<(), Self::Error> {
        let prefix: Vec<_> = scope
            .bindings()
            .iter()
            .map(|(name, ty)| (*name, (*ty).clone()))
            .collect();
        let host = CallTypeHost {
            catalog: self.host.catalog,
            version: self.host.version,
            granted: self.host.granted,
            environment: self.environment,
        };
        let limits = self.limits.control;
        let actual = infer_call_flow_type(
            node,
            &prefix,
            self.environment,
            &host,
            CallFlowTypeLimits {
                call: CallTypeLimits {
                    pure: TypeInferenceLimits {
                        max_nodes: limits.source.max_lowered_nodes,
                        max_depth: limits.source.max_lowered_depth,
                        max_bindings: limits.max_bindings,
                    },
                    max_arguments: limits.source.max_arguments,
                },
                max_calls: self.limits.max_calls,
            },
        )
        .map_err(CatalogSourceError::Type)?;
        if &actual != ty {
            return Err(CatalogSourceError::InconsistentType);
        }
        Ok(())
    }
}

/// Compile this closed expression profile through shared orchestration, not host
/// recursive callbacks. Complete physical/lexical/signature checks precede a
/// declaration-order cold catalog pass; every native call's version/grants/names
/// pass before slot mapping, export or result-type queries. All arguments use the
/// schema's declaration order. One control-output budget and final whole cold
/// flow inference prevent per-leaf budgets or supplied observations granting IR.
///
/// Supports scalar forms/loop/fold, Bind/Choose, catalog calls and direct bound
/// field/member projection. Groups/repeats, helper registration, opaque Host
/// payloads and arbitrary result-expression field inputs are not this profile.
/// Unknown names fail closed; host catalogs cannot replace reserved intrinsics.
/// Output is still mutable data, not execution authority or a schema snapshot.
/// Execution must revalidate live policy and actual replies through its protocol.
/// This does not parse text, run effects, own fuel, install storage or implement
/// complete independent compilation/durable VM recovery. Ingress bytes/AST
/// allocation and native work remain explicitly host-bounded.
pub fn compile_catalog_source<
    'source,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Domain,
    Declaration,
    Capability,
    Environment,
>(
    source: &'source Expression,
    prefix: &[(&'source str, &PureType<Environment::Result>)],
    limits: CatalogSourceLimits,
    host: &SourceCallHost<'_, 'schema, Operation, Domain, Declaration, Capability>,
    environment: &Environment,
) -> CatalogSourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    Environment::Result,
    Environment::Error,
>
where
    Operation: Borrow<str> + PartialEq,
    Domain: ScalarArgumentDomain,
    Capability: PartialEq,
    Environment: CatalogSourceEnvironment<'source, 'schema, Field, Operation, Declaration>,
{
    if limits.max_calls > MAX_TYPE_INFERENCE_NODES {
        return Err(CatalogSourceError::Type(CallFlowTypeError::InvalidLimits));
    }
    preflight_control_source(source, prefix, limits.control)
        .map_err(|e| CatalogSourceError::Control(Box::new(e)))?;
    cold_catalog(source, host, limits, |_| Ok(false))?;
    lower_preflighted_control_source(
        source,
        prefix,
        limits.control,
        &mut Compiler {
            host,
            environment,
            limits,
            program: None,
            locals: (),
        },
    )
    .map_err(|e| CatalogSourceError::Control(Box::new(e)))
}

pub(crate) fn lower_catalog_expression<
    'source,
    'schema,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Domain,
    Declaration,
    Capability,
    Environment,
>(
    source: &'source Expression,
    prefix: &[(&'source str, &PureType<Environment::Result>)],
    host: &SourceCallHost<'_, 'schema, Operation, Domain, Declaration, Capability>,
    environment: &Environment,
    program: &CatalogProgramContext<'source>,
) -> CatalogSourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    Environment::Result,
    Environment::Error,
>
where
    Operation: Borrow<str> + PartialEq,
    Domain: ScalarArgumentDomain,
    Capability: PartialEq,
    Environment: CatalogSourceEnvironment<'source, 'schema, Field, Operation, Declaration>,
{
    lower_preflighted_control_source(
        source,
        prefix,
        program.limits.expression.control,
        &mut Compiler {
            host,
            environment,
            limits: program.limits.expression,
            program: Some(program),
            locals: (),
        },
    )
    .map_err(|e| CatalogSourceError::Control(Box::new(e)))
}
