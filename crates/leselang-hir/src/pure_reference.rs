use crate::call_typing::{CallTypeError, CallTypeLimits, check_call_arguments};
use crate::flow_typing::{
    CallFlowEnvironment, CallFlowTypeError, CallFlowTypeLimits, GroupFlowEnvironment,
    GroupFlowTypeLimits, GroupMemberType, infer_call_flow_type, infer_group_flow_type,
};
use crate::prepared_typing::{
    PreparedCallSchema, PreparedCallSchemas, PreparedCallTypeError, infer_prepared_call_type,
};
use crate::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits, infer_pure_type,
};
use crate::{
    Type,
    computation::{Computation, GroupLocalType, MAX_COMPUTATION_NODES},
    host_call::{ArgumentDomain, HostOperation},
    result_field::ResultField,
};

#[derive(Clone, PartialEq)]
enum ReferenceMembers<'a> {
    External(&'a [(String, HostOperation)]),
    Computed {
        kind: crate::ir::GroupKind,
        members: std::rc::Rc<[(&'a str, &'a HostOperation)]>,
    },
}

#[derive(Clone, PartialEq)]
struct ReferenceResult<'a> {
    ty: Type,
    members: Option<ReferenceMembers<'a>>,
}

struct ReferenceSchemas;

impl PreparedCallSchemas<'static, HostOperation> for ReferenceSchemas {
    type Key = &'static str;
    type Domain = ArgumentDomain;
    type Result = crate::host_call::ReferenceResult;
    type Capability = &'static str;

    fn select(
        &self,
        operation: &HostOperation,
    ) -> Result<
        &'static PreparedCallSchema<
            'static,
            Self::Key,
            Self::Domain,
            Self::Result,
            Self::Capability,
        >,
        leselang_runtime_core::OperationCatalogError,
    > {
        Ok(operation.schema())
    }
}

struct ReferenceEnvironment<'a> {
    _groups: &'a [GroupLocalType],
}

impl<'schema>
    CallFlowEnvironment<'schema, ResultField, HostOperation, crate::host_call::ReferenceResult>
    for ReferenceEnvironment<'_>
{
    fn call_result_type(
        &self,
        _: &HostOperation,
        declaration: &'schema crate::host_call::ReferenceResult,
    ) -> Option<PureType<Self::Result>> {
        Some(PureType::Result(ReferenceResult {
            ty: declaration.ty,
            members: None,
        }))
    }
}

impl<'a> PureTypeEnvironment<ResultField, HostOperation> for ReferenceEnvironment<'a> {
    type Result = ReferenceResult<'a>;

    fn field_type(
        &self,
        result: &Self::Result,
        field: &ResultField,
    ) -> Option<leselang_runtime_core::ScalarType> {
        field.result_type(result.ty)
    }

    fn member_result(
        &self,
        group: &Self::Result,
        name: &str,
        operation: &HostOperation,
    ) -> Option<Self::Result> {
        let exported = match group.members.as_ref()? {
            ReferenceMembers::External(members) => members
                .iter()
                .any(|(member, declared)| member == name && declared == operation),
            ReferenceMembers::Computed { members, .. } => members
                .iter()
                .any(|(member, declared)| *member == name && *declared == operation),
        };
        if !exported {
            return None;
        }
        Some(ReferenceResult {
            ty: operation.result_type(),
            members: None,
        })
    }

    fn join_results(&self, left: Self::Result, right: Self::Result) -> Option<Self::Result> {
        (left.ty == right.ty).then_some(ReferenceResult {
            ty: left.ty,
            // Preserve legacy type identity without inventing a union of group exports.
            members: if left.members == right.members {
                left.members
            } else {
                None
            },
        })
    }
}

impl<'expression, 'schema>
    GroupFlowEnvironment<
        'expression,
        'schema,
        ResultField,
        HostOperation,
        crate::host_call::ReferenceResult,
        Type,
    > for ReferenceEnvironment<'expression>
{
    fn group_branch_type_matches(
        &self,
        declaration: &Type,
        inferred: &PureType<Self::Result>,
    ) -> bool {
        let ty = match inferred {
            PureType::Scalar(ty) => Type::Scalar(*ty),
            PureType::Result(result) => result.ty,
        };
        *declaration == ty
    }

    fn group_result_type(
        &self,
        kind: crate::ir::GroupKind,
        members: &[GroupMemberType<'expression, HostOperation, Self::Result>],
    ) -> Option<Self::Result> {
        Some(ReferenceResult {
            ty: Type::Structured,
            members: Some(ReferenceMembers::Computed {
                kind,
                members: members
                    .iter()
                    .map(|member| (member.name, member.operation))
                    .collect(),
            }),
        })
    }
}

pub(crate) fn infer(
    expression: &Computation,
    scope: &[(String, Type)],
    groups: &[GroupLocalType],
) -> Result<Type, PureTypeError> {
    let bindings = bindings(scope, groups);
    let inferred = infer_pure_type(
        expression,
        &bindings,
        &ReferenceEnvironment { _groups: groups },
        limits(),
    )?;
    Ok(match inferred {
        PureType::Scalar(ty) => Type::Scalar(ty),
        PureType::Result(result) => result.ty,
    })
}

pub(crate) fn infer_call(
    operation: HostOperation,
    arguments: &[crate::computation::ComputedArgument],
    scope: &[(String, Type)],
    groups: &[GroupLocalType],
) -> Result<Type, CallTypeError> {
    let bindings = bindings(scope, groups);
    let result = check_call_arguments(
        arguments,
        operation.schema(),
        &bindings,
        &ReferenceEnvironment { _groups: groups },
        CallTypeLimits {
            pure: limits(),
            max_arguments: 3,
        },
    )?;
    Ok(result.ty)
}

fn limits() -> TypeInferenceLimits {
    TypeInferenceLimits {
        max_nodes: MAX_COMPUTATION_NODES,
        max_depth: crate::MAX_EFFECT_NESTING_DEPTH,
        max_bindings: crate::MAX_EFFECT_NESTING_DEPTH * 3,
    }
}

pub(crate) fn infer_prepared(
    expression: &Computation,
    scope: &[(String, Type)],
    groups: &[GroupLocalType],
) -> Result<Type, PreparedCallTypeError> {
    let bindings = bindings(scope, groups);
    let result = infer_prepared_call_type(
        expression,
        &bindings,
        &ReferenceEnvironment { _groups: groups },
        &ReferenceSchemas,
        CallTypeLimits {
            pure: limits(),
            max_arguments: 3,
        },
    )?;
    Ok(result.ty)
}

pub(crate) fn infer_flow(
    expression: &Computation,
    scope: &[(String, Type)],
    groups: &[GroupLocalType],
) -> Result<Type, CallFlowTypeError> {
    let bindings = bindings(scope, groups);
    let result = infer_call_flow_type(
        expression,
        &bindings,
        &ReferenceEnvironment { _groups: groups },
        &ReferenceSchemas,
        CallFlowTypeLimits {
            call: CallTypeLimits {
                pure: limits(),
                max_arguments: 3,
            },
            max_calls: MAX_COMPUTATION_NODES,
        },
    )?;
    Ok(match result {
        PureType::Scalar(ty) => Type::Scalar(ty),
        PureType::Result(result) => result.ty,
    })
}

pub(crate) fn infer_group_flow<'a>(
    expression: &'a Computation,
    scope: &'a [(String, Type)],
    groups: &'a [GroupLocalType],
) -> Result<Type, CallFlowTypeError> {
    let bindings = bindings(scope, groups);
    let result = infer_group_flow_type(
        expression,
        &bindings,
        &ReferenceEnvironment { _groups: groups },
        &ReferenceSchemas,
        GroupFlowTypeLimits {
            flow: CallFlowTypeLimits {
                call: CallTypeLimits {
                    pure: limits(),
                    max_arguments: 3,
                },
                max_calls: MAX_COMPUTATION_NODES,
            },
            max_groups: MAX_COMPUTATION_NODES,
            max_branches: crate::MAX_ALL_BRANCHES,
        },
    )?;
    Ok(match result {
        PureType::Scalar(ty) => Type::Scalar(ty),
        PureType::Result(result) => result.ty,
    })
}

fn bindings<'a>(
    scope: &'a [(String, Type)],
    groups: &'a [GroupLocalType],
) -> Vec<(&'a str, PureType<ReferenceResult<'a>>)> {
    scope
        .iter()
        .map(|(name, ty)| {
            (
                name.as_str(),
                match ty {
                    Type::Scalar(ty) => PureType::Scalar(*ty),
                    _ => PureType::Result(ReferenceResult {
                        ty: *ty,
                        members: None,
                    }),
                },
            )
        })
        .chain(groups.iter().map(|group| {
            (
                group.name.as_str(),
                PureType::Result(ReferenceResult {
                    ty: Type::Structured,
                    members: Some(ReferenceMembers::External(&group.members)),
                }),
            )
        }))
        .collect()
}
