//! Opt-in flat call-group compilation through the existing source/type machinery.

use std::{borrow::Borrow, fmt};

use leselang_runtime_core::{OperationCatalogError, ScalarArgumentDomain, ScalarType};
use leselang_syntax::{Expression, Function, NamedArgument, Span};

use crate::call_typing::{CallTypeHost, CallTypeLimits};
use crate::catalog_program::{
    CatalogProgramContext, CatalogProgramGroupProfile, CatalogProgramLimits,
    compile_catalog_program_with_entry,
};
use crate::catalog_source::{
    CatalogLocalSource, CatalogSourceEnvironment, CatalogSourceError, CatalogSourceLimits,
    CatalogSourceResult, Compiler, cold_catalog_profile,
};
use crate::control_source::{
    ControlSourceAdapter, ControlSourceError, ControlSourceScope, lower_preflighted_control_source,
    preflight_closed_control_source,
};
use crate::flow_typing::{
    CallFlowTypeError, CallFlowTypeLimits, GroupFlowEnvironment, GroupFlowTypeLimits,
    infer_call_flow_type, infer_group_flow_type,
};
use crate::group_exports::MAX_GROUP_EXPORT_MEMBERS;
use crate::group_source::{GroupSourceError, GroupSourceLimits, header, lower_flat_group_source};
use crate::ir::Computation;
use crate::prepared_typing::{PreparedCallTypeError, infer_prepared_call_observation};
use crate::pure_typing::{MAX_TYPE_INFERENCE_NODES, PureType, TypeInferenceLimits};
use crate::scalar_source::primitive_name;
use crate::source_call::{SourceCallHost, span};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogGroupSourceLimits {
    pub expression: CatalogSourceLimits,
    pub max_groups: usize,
    pub max_branches: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogGroupProgramLimits {
    pub program: CatalogProgramLimits,
    /// Group-site ceiling across the cold declarations and expanded output.
    /// The explicit entry selects whether helpers may contain groups.
    pub max_groups: usize,
    pub max_branches: usize,
}

/// Native branch declaration mapping, not child compilation or group execution.
/// The declaration is the original selected catalog result. Map it to the host's
/// move-only IR slot; all group members are later corroborated by group flow typing.
/// Group metadata must precisely preserve mode, ordered names, original operation
/// identity and member types. It must not borrow the generated temporary IR: the
/// higher-ranked group environment permits moving IR without copying native slots.
/// Schema/owner references may still borrow the original catalog. Metadata alone
/// may Clone/PartialEq; branch declarations, fields/effects and payloads need not.
pub trait CatalogGroupSourceEnvironment<'source, 'schema, Field, Operation, Declaration, IrResult>:
    CatalogSourceEnvironment<'source, 'schema, Field, Operation, Declaration>
    + for<'expression> GroupFlowEnvironment<
        'expression,
        'schema,
        Field,
        Operation,
        Declaration,
        IrResult,
    >
{
    fn source_group_branch(
        &self,
        original: &'source NamedArgument,
        operation: &Operation,
        declaration: &'schema Declaration,
        inferred: &PureType<Self::Result>,
    ) -> Result<IrResult, Self::Error>;
}

pub enum CatalogGroupSourceError<Native> {
    InvalidLimits,
    Source(Box<CatalogSourceError<Native>>),
    Control(Box<ControlSourceError<Self>>),
    Group(Box<GroupSourceError<Self>>),
    GroupLimit {
        span: Span,
    },
    NestedGroup {
        span: Span,
    },
    NonAtomic {
        index: usize,
        span: Span,
    },
    PreparedMember {
        index: usize,
        span: Span,
        error: PreparedCallTypeError,
    },
    BranchMapping {
        span: Span,
        error: Native,
    },
    Catalog {
        span: Span,
        error: OperationCatalogError,
    },
    BranchType {
        index: usize,
        span: Span,
    },
    Type(CallFlowTypeError),
    InconsistentType,
}
impl<Native> fmt::Display for CatalogGroupSourceError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLimits => "catalog group limits exceed safety ceilings",
            Self::Source(_) => "catalog group cold source or leaf compilation rejected",
            Self::Control(_) => "catalog group control source rejected",
            Self::Group(_) => "catalog group shape or native branch admission rejected",
            Self::GroupLimit { .. } => "catalog group site limit exceeded",
            Self::NestedGroup { .. } => {
                "catalog group profile does not permit nested group members"
            }
            Self::NonAtomic { .. } => "catalog group member must be a direct registered call",
            Self::PreparedMember { .. } => {
                "catalog group member must prepare one uniform original call"
            }
            Self::BranchMapping { .. } => "catalog group native branch declaration mapping failed",
            Self::Catalog { .. } => "catalog group current original catalog admission failed",
            Self::BranchType { .. } => {
                "catalog group original branch declaration differs from inferred type"
            }
            Self::Type(_) => "catalog group whole cold type admission rejected",
            Self::InconsistentType => "catalog group observed type differs from complete inference",
        })
    }
}
impl<Native> fmt::Debug for CatalogGroupSourceError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for CatalogGroupSourceError<Native> {}
pub type CatalogGroupSourceResult<Node, ResultType, Native> =
    Result<(Node, PureType<ResultType>), CatalogGroupSourceError<Native>>;
type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;

// Complete physical/lexical preflight has bounded this forest before any catalog/native query.
fn cold_groups<Native>(
    source: &Expression,
    limits: CatalogGroupSourceLimits,
) -> Result<(), CatalogGroupSourceError<Native>> {
    cold_groups_with(source, limits, false, |_| false).map(|_| ())
}

pub(crate) fn cold_groups_with<Native>(
    source: &Expression,
    limits: CatalogGroupSourceLimits,
    prepared_members: bool,
    is_helper: impl Fn(&Expression) -> bool,
) -> Result<usize, CatalogGroupSourceError<Native>> {
    let mut pending = vec![(source, false)];
    let mut groups = 0;
    while let Some((node, inside_group)) = pending.pop() {
        let Expression::Call {
            callee, arguments, ..
        } = node
        else {
            continue;
        };
        let group = matches!(callee.as_str(), "seq" | "all");
        if group {
            if inside_group {
                return Err(CatalogGroupSourceError::NestedGroup { span: span(node) });
            }
            if groups >= limits.max_groups {
                return Err(CatalogGroupSourceError::GroupLimit { span: span(node) });
            }
            groups += 1;
            let (_, members) = header(node, limits.max_branches)
                .map_err(|e| CatalogGroupSourceError::Group(Box::new(e)))?;
            for (index, member) in members.iter().enumerate() {
                if !prepared_members
                    && (is_helper(&member.value)
                        || !matches!(&member.value, Expression::Call { callee, .. }
                    if !primitive_name(callee) && !matches!(callee.as_str(), "bind" | "choose" | "field" | "member" | "seq" | "all" | "repeat" | "loop" | "fold")))
                {
                    return Err(CatalogGroupSourceError::NonAtomic {
                        index,
                        span: member.span,
                    });
                }
            }
        }
        pending.extend(
            arguments
                .iter()
                .rev()
                .map(|argument| (&argument.value, inside_group || group)),
        );
    }
    Ok(groups)
}

struct Groups<'a, Inner, Host, Environment> {
    inner: Inner,
    host: &'a Host,
    environment: &'a Environment,
    limits: CatalogGroupSourceLimits,
    prepared_members: bool,
}
fn flow_limits(limits: CatalogSourceLimits) -> CallFlowTypeLimits {
    CallFlowTypeLimits {
        call: CallTypeLimits {
            pure: TypeInferenceLimits {
                max_nodes: limits.control.source.max_lowered_nodes,
                max_depth: limits.control.source.max_lowered_depth,
                max_bindings: limits.control.max_bindings,
            },
            max_arguments: limits.control.source.max_arguments,
        },
        max_calls: limits.max_calls,
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
    Inner,
> ControlSourceAdapter<'source, Field, Operation, HostEffect, IrResult>
    for Groups<
        '_,
        Inner,
        SourceCallHost<'_, 'schema, Operation, Domain, Declaration, Capability>,
        Environment,
    >
where
    Operation: Borrow<str> + PartialEq,
    Domain: ScalarArgumentDomain,
    Capability: PartialEq,
    Environment:
        CatalogGroupSourceEnvironment<'source, 'schema, Field, Operation, Declaration, IrResult>,
    Inner: ControlSourceAdapter<
            'source,
            Field,
            Operation,
            HostEffect,
            IrResult,
            Type = PureType<Environment::Result>,
            Error = CatalogSourceError<Environment::Error>,
        >,
{
    type Type = PureType<Environment::Result>;
    type Error = CatalogGroupSourceError<Environment::Error>;
    fn lower_leaf(
        &mut self,
        source: &'source Expression,
        scope: ControlSourceScope<'_, 'source, Self::Type>,
    ) -> Result<(Node<Field, Operation, HostEffect, IrResult>, Self::Type), Self::Error> {
        if !matches!(source, Expression::Call { callee, .. } if matches!(callee.as_str(), "seq" | "all"))
        {
            return self
                .inner
                .lower_leaf(source, scope)
                .map_err(|e| Self::Error::Source(Box::new(e)));
        }
        let prefix: Vec<_> = scope
            .bindings()
            .iter()
            .map(|(name, ty)| (*name, (*ty).clone()))
            .collect();
        let host = self.host;
        let environment = self.environment;
        let type_host = CallTypeHost {
            catalog: host.catalog,
            version: host.version,
            granted: host.granted,
            environment,
        };
        let flow = flow_limits(self.limits.expression);
        let group = lower_flat_group_source(
            source,
            GroupSourceLimits {
                source: self.limits.expression.control.source,
                max_branches: self.limits.max_branches,
            },
            |index, original| {
                let (node, inferred) = if self.prepared_members {
                    lower_preflighted_control_source(
                        &original.value,
                        scope.bindings(),
                        self.limits.expression.control,
                        &mut self.inner,
                    )
                    .map_err(|error| {
                        Self::Error::Source(Box::new(CatalogSourceError::Control(Box::new(error))))
                    })?
                } else {
                    self.inner
                        .lower_leaf(&original.value, ControlSourceScope::new(scope.bindings()))
                        .map_err(|e| Self::Error::Source(Box::new(e)))?
                };
                let (operation, original_result) = if self.prepared_members {
                    let (operation, result) = infer_prepared_call_observation(
                        &node,
                        &prefix,
                        environment,
                        &type_host,
                        flow.call,
                    )
                    .map_err(|error| Self::Error::PreparedMember {
                        index,
                        span: original.span,
                        error,
                    })?;
                    (operation, Some(result))
                } else {
                    let Node::Call { operation, .. } = &node else {
                        return Err(Self::Error::NonAtomic {
                            index,
                            span: original.span,
                        });
                    };
                    (operation, None)
                };
                let schema = host
                    .catalog
                    .authorize(operation.borrow(), host.version, host.granted)
                    .map_err(|error| Self::Error::Catalog {
                        span: original.span,
                        error,
                    })?;
                if original_result.is_some_and(|result| !std::ptr::eq(result, &schema.result)) {
                    return Err(Self::Error::PreparedMember {
                        index,
                        span: original.span,
                        error: PreparedCallTypeError::InconsistentOperation,
                    });
                }
                let declaration = environment
                    .source_group_branch(original, operation, &schema.result, &inferred)
                    .map_err(|error| Self::Error::BranchMapping {
                        span: original.span,
                        error,
                    })?;
                Ok((node, declaration))
            },
            |index, original, branch| {
                let inferred =
                    infer_call_flow_type(&branch.value, &prefix, environment, &type_host, flow)
                        .map_err(Self::Error::Type)?;
                if !environment.group_branch_type_matches(&branch.result_type, &inferred) {
                    return Err(Self::Error::BranchType {
                        index,
                        span: original.span,
                    });
                }
                Ok(())
            },
        )
        .map_err(|e| Self::Error::Group(Box::new(e)))?;
        let ty = infer_group_flow_type(
            &group,
            &prefix,
            environment,
            &type_host,
            GroupFlowTypeLimits {
                flow,
                max_groups: self.limits.max_groups,
                max_branches: self.limits.max_branches,
            },
        )
        .map_err(Self::Error::Type)?;
        Ok((group, ty))
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
        let actual = infer_group_flow_type(
            node,
            &prefix,
            self.environment,
            &host,
            GroupFlowTypeLimits {
                flow: flow_limits(self.limits.expression),
                max_groups: self.limits.max_groups,
                max_branches: self.limits.max_branches,
            },
        )
        .map_err(Self::Error::Type)?;
        if &actual != ty {
            return Err(Self::Error::InconsistentType);
        }
        Ok(())
    }
}

/// Compile a complete expression with flat Seq/All, Bind/Choose and closed member/field reads.
/// Entire cold physical/lexical/group shape and all catalog versions/grants/named
/// signatures pass before operation/branch mapping or metadata queries. Members
/// are direct registered calls with pure operands sharing only the outer scope;
/// nested groups, repeat, helper calls and opaque Host payloads are not this profile.
/// Existing scalar/call lowering, group assembly, shared lexical/output meters and
/// whole group-flow inference are reused, not a second parser or evaluator.
/// Complete output inference corroborates observed group metadata and member rows;
/// native type queries/mapping/work/unwind remain trusted and cannot be rolled back.
/// Output is mutable data, not a receipt, authority or executable continuation.
/// Dispatch, live result validation, cancellation and persistence remain host-owned.
/// The existing call-only/compiler-program entries retain their group rejection.
pub fn compile_catalog_group_source<
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
    limits: CatalogGroupSourceLimits,
    host: &SourceCallHost<'_, 'schema, Operation, Domain, Declaration, Capability>,
    environment: &Environment,
) -> CatalogGroupSourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    Environment::Result,
    Environment::Error,
>
where
    Operation: Borrow<str> + PartialEq,
    Domain: ScalarArgumentDomain,
    Capability: PartialEq,
    Environment:
        CatalogGroupSourceEnvironment<'source, 'schema, Field, Operation, Declaration, IrResult>,
{
    if limits.max_groups > MAX_TYPE_INFERENCE_NODES
        || limits.max_branches > MAX_GROUP_EXPORT_MEMBERS
        || limits.expression.max_calls > MAX_TYPE_INFERENCE_NODES
    {
        return Err(CatalogGroupSourceError::InvalidLimits);
    }
    preflight_closed_control_source(source, prefix, limits.expression.control)
        .map_err(|e| CatalogGroupSourceError::Control(Box::new(e)))?;
    cold_groups(source, limits)?;
    cold_catalog_profile(source, host, limits.expression, true, |_| Ok(false))
        .map_err(|e| CatalogGroupSourceError::Source(Box::new(e)))?;
    lower_group_catalog_expression(source, prefix, limits, host, environment, None, ())
}

fn lower_group_catalog_expression<
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
>(
    source: &'source Expression,
    prefix: &[(&'source str, &PureType<Environment::Result>)],
    limits: CatalogGroupSourceLimits,
    host: &SourceCallHost<'_, 'schema, Operation, Domain, Declaration, Capability>,
    environment: &Environment,
    program: Option<&CatalogProgramContext<'source>>,
    locals: Locals,
) -> CatalogGroupSourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    Environment::Result,
    Environment::Error,
>
where
    Operation: Borrow<str> + PartialEq,
    Domain: ScalarArgumentDomain,
    Capability: PartialEq,
    Environment:
        CatalogGroupSourceEnvironment<'source, 'schema, Field, Operation, Declaration, IrResult>,
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
    let prepared_members = locals.prepared_members();
    lower_preflighted_control_source(
        source,
        prefix,
        limits.expression.control,
        &mut Groups {
            inner: Compiler::new(host, environment, limits.expression)
                .with_program(program)
                .with_locals(locals),
            host,
            environment,
            limits,
            prepared_members,
        },
    )
    .map_err(|e| CatalogGroupSourceError::Control(Box::new(e)))
}

/// Compile a complete multi-function program with flat groups in its explicit entry.
/// Existing acyclic helpers retain scalar parameters and scalar/call-only bodies;
/// they can supply pure group operands or ordinary entry leaves, never direct group
/// members or group-returning bodies. Every declaration, including unused helpers,
/// passes closed lexical, dependency and catalog policy before any slot/type hook.
/// Shared body recompilation, hygiene, work/expansion guards and original whole
/// group-flow/output admission are reused. No template copy, AST substitution,
/// evaluator, dispatch authority or durable continuation is introduced.
/// The call-only program entry preserves its original profile and diagnostics.
pub fn compile_catalog_group_program<
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
    declarations: &[&'source Function],
    entry: &str,
    limits: CatalogGroupProgramLimits,
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
    Environment:
        CatalogGroupSourceEnvironment<'source, 'schema, Field, Operation, Declaration, IrResult>,
{
    if limits.max_groups > MAX_TYPE_INFERENCE_NODES
        || limits.max_branches > MAX_GROUP_EXPORT_MEMBERS
    {
        return Err(CatalogSourceError::Group(Box::new(
            CatalogGroupSourceError::InvalidLimits,
        )));
    }
    let groups = CatalogGroupSourceLimits {
        expression: limits.program.expression,
        max_groups: limits.max_groups,
        max_branches: limits.max_branches,
    };
    compile_catalog_program_with_entry(
        declarations,
        entry,
        limits.program,
        host,
        environment,
        Some(CatalogProgramGroupProfile {
            limits: groups,
            helper_groups: false,
            prepared_members: false,
        }),
        |program, source, prefix| {
            program.with_expansion(|| {
                lower_group_catalog_expression(
                    source,
                    prefix,
                    groups,
                    host,
                    environment,
                    Some(program),
                    (),
                )
                .map_err(|e| CatalogSourceError::Group(Box::new(e)))
            })
        },
    )
}

struct HelperGroups<'a, 'source, Host, Environment> {
    program: &'a CatalogProgramContext<'source>,
    host: &'a Host,
    environment: &'a Environment,
    limits: CatalogGroupSourceLimits,
    prepared_members: bool,
}
impl<'source, 'schema, Operation, Domain, Declaration, Capability, Environment>
    HelperGroups<
        '_,
        'source,
        SourceCallHost<'_, 'schema, Operation, Domain, Declaration, Capability>,
        Environment,
    >
{
    fn compile<Field, HostEffect, IrResult>(
        &self,
        source: &'source Expression,
        prefix: &[(&'source str, &PureType<Environment::Result>)],
    ) -> CatalogSourceResult<
        Node<Field, Operation, HostEffect, IrResult>,
        Environment::Result,
        Environment::Error,
    >
    where
        Operation: Borrow<str> + PartialEq,
        Domain: ScalarArgumentDomain,
        Capability: PartialEq,
        Environment: CatalogGroupSourceEnvironment<'source, 'schema, Field, Operation, Declaration, IrResult>,
    {
        lower_group_catalog_expression(
            source,
            prefix,
            self.limits,
            self.host,
            self.environment,
            None,
            self,
        )
        .map_err(|e| CatalogSourceError::Group(Box::new(e)))
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
>
    CatalogLocalSource<
        'source,
        Field,
        Operation,
        HostEffect,
        IrResult,
        Environment::Result,
        Environment::Error,
    >
    for &HelperGroups<
        '_,
        'source,
        SourceCallHost<'_, 'schema, Operation, Domain, Declaration, Capability>,
        Environment,
    >
where
    Operation: Borrow<str> + PartialEq,
    Domain: ScalarArgumentDomain,
    Capability: PartialEq,
    Environment:
        CatalogGroupSourceEnvironment<'source, 'schema, Field, Operation, Declaration, IrResult>,
{
    fn prepared_members(&self) -> bool {
        self.prepared_members
    }
    fn contains(&self, source: &Expression) -> bool {
        self.program.is_helper(source)
    }
    fn lower(
        &self,
        source: &'source Expression,
        prefix: &[(&'source str, &PureType<Environment::Result>)],
    ) -> Option<
        CatalogSourceResult<
            Node<Field, Operation, HostEffect, IrResult>,
            Environment::Result,
            Environment::Error,
        >,
    > {
        self.program
            .lower_helper_with(source, prefix, |source, prefix| {
                self.compile(source, prefix)
            })
    }
}

/// Opt in to flat groups in acyclic helper bodies, including group return values.
/// Original scalar parameter scopes, recompilation, native-slot hygiene and work/
/// depth guards are shared with the call-only profile. Complete closed declarations
/// pass before mapping; group quotas cover cold helper sites and expanded output.
/// Group members remain direct catalog calls, never helper aliases/nested groups.
/// Native metadata cannot borrow temporary IR; whole group-flow inference must
/// corroborate the complete moved output. No evaluator/scheduler/capture policy is
/// added: hosts still prepare operands while their scope is live and explicitly
/// reject unsupported nested suspension. Existing entry-only APIs are unchanged.
pub fn compile_catalog_group_helper_program<
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
    declarations: &[&'source Function],
    entry: &str,
    limits: CatalogGroupProgramLimits,
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
    Environment:
        CatalogGroupSourceEnvironment<'source, 'schema, Field, Operation, Declaration, IrResult>,
{
    compile_group_helpers(declarations, entry, limits, host, environment, false)
}

/// Opt in to pure Bind/Choose and acyclic atomic helper calls as flat group members.
/// All closed declarations, helper edges, named/catalog policy and native-free
/// member shape facts pass before compilation hooks, including unused helpers.
/// Expanded members then require whole uniform original-schema inference before
/// branch mapping; equal result tags never admit different operations. Shared
/// helper recompilation, hygiene, work/depth quotas and whole group/output gates
/// remain mandatory. No host payload copying, new grammar, evaluator, scheduling,
/// dispatch, native-work bound or durable execution authority is introduced.
/// Existing source/entry/group-helper APIs remain direct-member-only. Hosts use
/// prepare_atomic_call_in_scope with live scope/fuel, then explicitly own fresh
/// publication authority, actual invocation, reply validation and cancellation.
pub fn compile_catalog_prepared_group_helper_program<
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
    declarations: &[&'source Function],
    entry: &str,
    limits: CatalogGroupProgramLimits,
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
    Environment:
        CatalogGroupSourceEnvironment<'source, 'schema, Field, Operation, Declaration, IrResult>,
{
    compile_group_helpers(declarations, entry, limits, host, environment, true)
}

fn compile_group_helpers<
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
    declarations: &[&'source Function],
    entry: &str,
    limits: CatalogGroupProgramLimits,
    host: &SourceCallHost<'_, 'schema, Operation, Domain, Declaration, Capability>,
    environment: &Environment,
    prepared_members: bool,
) -> CatalogSourceResult<
    Node<Field, Operation, HostEffect, IrResult>,
    Environment::Result,
    Environment::Error,
>
where
    Operation: Borrow<str> + PartialEq,
    Domain: ScalarArgumentDomain,
    Capability: PartialEq,
    Environment:
        CatalogGroupSourceEnvironment<'source, 'schema, Field, Operation, Declaration, IrResult>,
{
    if limits.max_groups > MAX_TYPE_INFERENCE_NODES
        || limits.max_branches > MAX_GROUP_EXPORT_MEMBERS
    {
        return Err(CatalogSourceError::Group(Box::new(
            CatalogGroupSourceError::InvalidLimits,
        )));
    }
    let groups = CatalogGroupSourceLimits {
        expression: limits.program.expression,
        max_groups: limits.max_groups,
        max_branches: limits.max_branches,
    };
    compile_catalog_program_with_entry(
        declarations,
        entry,
        limits.program,
        host,
        environment,
        Some(CatalogProgramGroupProfile {
            limits: groups,
            helper_groups: true,
            prepared_members,
        }),
        |program, source, prefix| {
            HelperGroups {
                program,
                host,
                environment,
                limits: groups,
                prepared_members,
            }
            .compile(source, prefix)
        },
    )
}
