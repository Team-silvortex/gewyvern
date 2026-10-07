use crate::call_typing::{CallTypeError, CallTypeLimits, check_call_arguments};
use crate::flow_typing::{
    CallFlowEnvironment, CallFlowTypeError, CallFlowTypeLimits, GroupFlowEnvironment,
    GroupFlowTypeLimits, GroupMemberType, HostFlowEnvironment, HostFlowTypeLimits,
    HostGroupFlowEnvironment, HostGroupFlowTypeLimits, infer_call_flow_type, infer_group_flow_type,
    infer_host_flow_type, infer_host_group_flow_type,
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

// Canonical admission has already checked native payload domains. This view
// corroborates only one closed flat graph, never nested graphs or dispatch.
pub(crate) fn flat_native_group(
    effect: &crate::Effect,
) -> Option<(crate::ir::GroupKind, &[crate::HirBranch])> {
    let (kind, branches, minimum) = match effect {
        crate::Effect::Sequence { steps } => (crate::ir::GroupKind::Sequence, steps, 1),
        crate::Effect::All { branches } => (crate::ir::GroupKind::Parallel, branches, 2),
        _ => return None,
    };
    if !(minimum..=crate::MAX_ALL_BRANCHES).contains(&branches.len())
        || branches.iter().enumerate().any(|(index, branch)| {
            !crate::pure_typing::valid_member_name(&branch.name)
                || branches[..index]
                    .iter()
                    .any(|prior| prior.name == branch.name)
                || HostOperation::for_effect(&branch.effect)
                    .is_none_or(|operation| operation.schema().result.ty != branch.result_type)
        })
    {
        return None;
    }
    Some((kind, branches))
}

pub(crate) fn supports_host_flow(effect: &crate::Effect) -> bool {
    HostOperation::for_effect(effect).is_some() || flat_native_group(effect).is_some()
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

impl<'expression, 'a>
    HostFlowEnvironment<
        'expression,
        'static,
        ResultField,
        HostOperation,
        crate::host_call::ReferenceResult,
        crate::Effect,
    > for ReferenceEnvironment<'a>
where
    'expression: 'a,
{
    fn admit_host(&self, effect: &'expression crate::Effect) -> bool {
        // This adapter is called only after bounded canonical product admission.
        // Never reparse here: source admission would reenter the lowering chain.
        supports_host_flow(effect)
    }

    fn host_result_type(
        &self,
        effect: &'expression crate::Effect,
    ) -> Option<PureType<Self::Result>> {
        if let Some(operation) = HostOperation::for_effect(effect) {
            let ty = operation.schema().result.ty;
            return Some(match ty {
                Type::Scalar(ty) => PureType::Scalar(ty),
                _ => PureType::Result(ReferenceResult { ty, members: None }),
            });
        }
        let (kind, branches) = flat_native_group(effect)?;
        let members = branches
            .iter()
            .map(|branch| {
                Some((
                    branch.name.as_str(),
                    &HostOperation::for_effect(&branch.effect)?
                        .schema()
                        .result
                        .operation,
                ))
            })
            .collect::<Option<std::rc::Rc<[_]>>>()?;
        Some(PureType::Result(ReferenceResult {
            ty: Type::Structured,
            members: Some(ReferenceMembers::Computed { kind, members }),
        }))
    }
}

impl<'e>
    HostGroupFlowEnvironment<
        'e,
        'static,
        ResultField,
        HostOperation,
        crate::host_call::ReferenceResult,
        crate::Effect,
        Type,
    > for ReferenceEnvironment<'e>
{
    fn host_group_operation(&self, effect: &'e crate::Effect) -> Option<&'e HostOperation> {
        Some(&HostOperation::for_effect(effect)?.schema().result.operation)
    }
    fn admit_host_group_operation(
        &self,
        effect: &'e crate::Effect,
        operation: &'e HostOperation,
        declaration: &'static crate::host_call::ReferenceResult,
    ) -> bool {
        HostOperation::for_effect(effect) == Some(*operation)
            && std::ptr::eq(operation, &operation.schema().result.operation)
            && std::ptr::eq(declaration, &operation.schema().result)
            && declaration.ty == operation.result_type()
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

// Source admission must not enter canonical round-trip lowering recursively.
// Borrow the exact native prefix and closed member signatures instead of copying
// names or using residual-expression depth limits for the ambient type scope.
pub(crate) fn infer_source_call_in_scope(
    operation: HostOperation,
    arguments: &[crate::computation::ComputedArgument],
    scope: &crate::computation::TypeScope<'_, '_>,
) -> Result<Type, CallTypeError> {
    let bindings = scope
        .bindings()
        .iter()
        .map(|(name, local)| {
            let ty = match local.ty() {
                Type::Scalar(ty) => PureType::Scalar(ty),
                ty => PureType::Result(ReferenceResult {
                    ty,
                    members: local.members().map(ReferenceMembers::External),
                }),
            };
            (*name, ty)
        })
        .collect::<Vec<_>>();
    let result = check_call_arguments(
        arguments,
        operation.schema(),
        &bindings,
        &ReferenceEnvironment { _groups: &[] },
        CallTypeLimits {
            pure: TypeInferenceLimits {
                max_bindings: crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS,
                ..limits()
            },
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

pub(crate) fn infer_host_flow(
    expression: &Computation,
    scope: &[(String, Type)],
    groups: &[GroupLocalType],
) -> Result<Type, CallFlowTypeError> {
    let bindings = bindings(scope, groups);
    let result = infer_host_flow_type(
        expression,
        &bindings,
        &ReferenceEnvironment { _groups: groups },
        &ReferenceSchemas,
        HostFlowTypeLimits {
            flow: CallFlowTypeLimits {
                call: CallTypeLimits {
                    pure: limits(),
                    max_arguments: 3,
                },
                max_calls: MAX_COMPUTATION_NODES,
            },
            max_hosts: MAX_COMPUTATION_NODES,
        },
    )?;
    Ok(match result {
        PureType::Scalar(ty) => Type::Scalar(ty),
        PureType::Result(result) => result.ty,
    })
}

pub(crate) fn infer_host_group_flow<'a>(
    expression: &'a Computation,
    scope: &'a [(String, Type)],
    groups: &'a [GroupLocalType],
) -> Result<Type, CallFlowTypeError> {
    let bindings = bindings(scope, groups);
    let result = infer_host_group_flow_type(
        expression,
        &bindings,
        &ReferenceEnvironment { _groups: groups },
        &ReferenceSchemas,
        HostGroupFlowTypeLimits {
            group: GroupFlowTypeLimits {
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
            max_hosts: MAX_COMPUTATION_NODES,
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

#[cfg(test)]
mod native_group_tests {
    use super::*;
    use crate::{Effect, HirBranch};
    use leselang_runtime_core::ScalarType;

    fn native(node_id: &str) -> Effect {
        Effect::Sequence {
            steps: vec![HirBranch {
                name: "focus".into(),
                effect: Effect::UiFocus {
                    node_id: node_id.into(),
                },
                result_type: Type::UiFocus,
            }],
        }
    }

    #[test]
    fn native_group_type_observations_borrow_names_and_exact_canonical_operation_rows() {
        let effect = native("a");
        let environment = ReferenceEnvironment { _groups: &[] };
        assert!(environment.admit_host(&effect));
        let PureType::Result(result) = environment.host_result_type(&effect).unwrap() else {
            panic!()
        };
        let ReferenceMembers::Computed { kind, members } = result.members.unwrap() else {
            panic!()
        };
        let Effect::Sequence { steps } = &effect else {
            panic!()
        };
        assert_eq!(kind, crate::ir::GroupKind::Sequence);
        assert_eq!(members[0].0.as_ptr(), steps[0].name.as_ptr());
        assert!(std::ptr::eq(
            members[0].1,
            &HostOperation::UiFocus.schema().result.operation
        ));
    }

    #[test]
    fn native_and_computed_group_joins_ignore_payloads_but_never_union_exports() {
        let left = native("a");
        let right = native("different-target");
        let operation = HostOperation::UiFocus;
        let environment = ReferenceEnvironment { _groups: &[] };
        let observe = |effect| match environment.host_result_type(effect).unwrap() {
            PureType::Result(result) => result,
            _ => panic!(),
        };
        let computed = environment
            .group_result_type(
                crate::ir::GroupKind::Sequence,
                &[GroupMemberType {
                    name: "focus",
                    operation: &operation,
                    inferred: PureType::Result(ReferenceResult {
                        ty: Type::UiFocus,
                        members: None,
                    }),
                }],
            )
            .unwrap();
        let joined = environment
            .join_results(observe(&left), observe(&right))
            .unwrap();
        assert!(
            environment
                .join_results(joined, computed)
                .unwrap()
                .members
                .is_some()
        );
        let mut changed = native("a");
        let Effect::Sequence { steps } = &mut changed else {
            panic!()
        };
        steps[0].name = "private".into();
        assert!(
            environment
                .join_results(observe(&left), observe(&changed))
                .unwrap()
                .members
                .is_none()
        );
    }

    #[test]
    fn the_native_group_view_rejects_nested_graphs_compute_and_wrong_declarations() {
        for (effect, result_type) in [
            (native("a"), Type::Structured),
            (
                Effect::Compute {
                    expression: Box::new(Computation::Literal {
                        value: leselang_runtime_core::ScalarValue::Boolean(true),
                    }),
                },
                Type::Scalar(ScalarType::Boolean),
            ),
            (
                Effect::UiFocus {
                    node_id: "a".into(),
                },
                Type::RuntimeList,
            ),
        ] {
            let value = Effect::Sequence {
                steps: vec![HirBranch {
                    name: "entry".into(),
                    effect,
                    result_type,
                }],
            };
            assert!(flat_native_group(&value).is_none());
            assert!(!supports_host_flow(&value));
        }
    }
}
