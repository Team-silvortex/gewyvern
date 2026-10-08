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

#[derive(Clone)]
enum ReferenceMembers<'a> {
    External(&'a [(String, HostOperation)]),
    Computed {
        kind: crate::ir::GroupKind,
        members: std::rc::Rc<[(&'a str, &'a HostOperation)]>,
    },
}

impl PartialEq for ReferenceMembers<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::External(left), Self::External(right)) => left == right,
            (
                Self::Computed {
                    kind: left_kind,
                    members: left,
                },
                Self::Computed {
                    kind: right_kind,
                    members: right,
                },
            ) => crate::native_group_signature::compare_native_group_signatures(
                (*left_kind, left.as_ref()),
                (*right_kind, right.as_ref()),
                crate::MAX_ALL_BRANCHES,
                |member| -> Result<_, ()> { Ok(member.0) },
                |member| Ok(member.0),
                |left, right| Ok(left.1 == right.1),
            )
            .unwrap_or(false),
            // External legacy metadata has no mode; never invent one to bridge variants.
            _ => false,
        }
    }
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
    let (kind, branches) = match effect {
        crate::Effect::Sequence { steps } => (crate::ir::GroupKind::Sequence, steps),
        crate::Effect::All { branches } => (crate::ir::GroupKind::Parallel, branches),
        _ => return None,
    };
    crate::native_group::admit_native_group_members(
        kind,
        branches,
        crate::MAX_ALL_BRANCHES,
        |branch| Ok(branch.name.as_str()),
        |branch| {
            if HostOperation::for_effect(&branch.effect)
                .is_none_or(|operation| operation.schema().result.ty != branch.result_type)
            {
                Err(())
            } else {
                Ok(())
            }
        },
    )
    .ok()
    .map(|admitted| (admitted.kind(), admitted.members()))
}

pub(crate) fn supports_host_flow(effect: &crate::Effect) -> bool {
    HostOperation::for_effect(effect).is_some() || flat_native_group(effect).is_some()
}

// Fresh flat export observations still require the caller's canonical/domain gate.
pub(crate) fn native_group_exports(
    effect: &crate::Effect,
) -> Option<crate::group_exports::GroupExports<'_, HostOperation>> {
    let (kind, branches) = match effect {
        crate::Effect::Sequence { steps } => (crate::ir::GroupKind::Sequence, steps),
        crate::Effect::All { branches } => (crate::ir::GroupKind::Parallel, branches),
        _ => return None,
    };
    crate::native_group_exports::observe_native_group_exports(
        kind,
        branches,
        crate::MAX_ALL_BRANCHES,
        |branch| Ok(branch.name.as_str()),
        |branch| {
            let operation = HostOperation::for_effect(&branch.effect).ok_or(())?;
            (operation.schema().result.ty == branch.result_type)
                .then_some(operation)
                .ok_or(())
        },
        |candidate_kind, original, exports| {
            if candidate_kind == kind
                && original.len() == exports.len()
                && original.iter().zip(exports).all(|(branch, export)| {
                    std::ptr::eq(branch.name.as_str(), export.name)
                        && HostOperation::for_effect(&branch.effect) == Some(export.operation)
                        && export.operation.schema().result.ty == branch.result_type
                })
            {
                Ok(())
            } else {
                Err(())
            }
        },
    )
    .ok()
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
        let exports = native_group_exports(effect)?;
        let kind = exports.kind;
        let members = exports
            .members
            .into_iter()
            .map(|member| (member.name, &member.operation.schema().result.operation))
            .collect();
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
            ReferenceMembers::Computed { kind, members } => {
                crate::native_group_lookup::lookup_native_group_member(
                    *kind,
                    members.as_ref(),
                    crate::MAX_ALL_BRANCHES,
                    name,
                    |member| -> Result<_, ()> { Ok(member.0) },
                    |member| Ok(member.1 == operation),
                )
                .ok()
                .flatten()
                .is_some()
            }
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

    #[test]
    fn shared_native_member_gate_keeps_original_label_limits_and_requires_prior_payload_admission()
    {
        let mut effect = Effect::Sequence {
            steps: (0..crate::MAX_ALL_BRANCHES)
                .map(|index| HirBranch {
                    name: format!("member_{index}"),
                    effect: Effect::UiFocus {
                        node_id: "target".into(),
                    },
                    result_type: Type::UiFocus,
                })
                .collect(),
        };
        let (kind, members) = flat_native_group(&effect).unwrap();
        assert_eq!(kind, crate::ir::GroupKind::Sequence);
        assert_eq!(members.len(), crate::MAX_ALL_BRANCHES);
        let Effect::Sequence { steps } = &effect else {
            panic!()
        };
        assert!(std::ptr::eq(members, &steps[..]));
        crate::canonical_source(&effect).unwrap();
        let Effect::Sequence { steps } = &mut effect else {
            panic!()
        };
        steps[0].effect = Effect::UiFocus {
            node_id: String::new(),
        };
        // Closed declaration matching is deliberately not native payload admission.
        assert!(flat_native_group(&effect).is_some());
        assert!(crate::canonical_source(&effect).is_err());
        let Effect::Sequence { steps } = &mut effect else {
            panic!()
        };
        steps[63].name = steps[0].name.clone();
        assert!(flat_native_group(&effect).is_none());
    }

    #[test]
    fn shared_native_exports_keep_original_order_canonical_rows_and_closed_flat_profile() {
        let mut effect = Effect::Sequence {
            steps: vec![
                HirBranch {
                    name: "focus".into(),
                    effect: Effect::UiFocus {
                        node_id: "a".into(),
                    },
                    result_type: Type::UiFocus,
                },
                HirBranch {
                    name: "activate".into(),
                    effect: Effect::UiActivate {
                        node_id: "b".into(),
                    },
                    result_type: Type::UiActivate,
                },
            ],
        };
        let exports = native_group_exports(&effect).unwrap();
        assert_eq!(exports.kind, crate::ir::GroupKind::Sequence);
        assert_eq!(
            exports
                .members
                .iter()
                .map(|member| member.operation)
                .collect::<Vec<_>>(),
            [HostOperation::UiFocus, HostOperation::UiActivate]
        );
        let Effect::Sequence { steps } = &effect else {
            panic!()
        };
        for (branch, export) in steps.iter().zip(&exports.members) {
            assert!(std::ptr::eq(branch.name.as_str(), export.name));
        }
        drop(exports);
        let Effect::Sequence { steps } = &mut effect else {
            panic!()
        };
        steps[1].result_type = Type::RuntimeList;
        assert!(native_group_exports(&effect).is_none());
        let Effect::Sequence { steps } = &mut effect else {
            panic!()
        };
        steps[1].effect = native("b");
        steps[1].result_type = Type::Structured;
        assert!(native_group_exports(&effect).is_none());
    }

    #[test]
    fn shared_signature_comparison_preserves_product_tags_and_legacy_external_variant_identity() {
        let focus = HostOperation::UiFocus;
        let activate = HostOperation::UiActivate;
        let left = ReferenceMembers::Computed {
            kind: crate::ir::GroupKind::Sequence,
            members: vec![("focus", &focus), ("activate", &activate)].into(),
        };
        let names = [String::from("focus"), String::from("activate")];
        let same = ReferenceMembers::Computed {
            kind: crate::ir::GroupKind::Sequence,
            members: vec![(names[0].as_str(), &focus), (names[1].as_str(), &activate)].into(),
        };
        assert!(left == same);
        let changed = ReferenceMembers::Computed {
            kind: crate::ir::GroupKind::Parallel,
            members: vec![("focus", &focus), ("activate", &activate)].into(),
        };
        assert!(left != changed);
        let changed = ReferenceMembers::Computed {
            kind: crate::ir::GroupKind::Sequence,
            members: vec![("activate", &activate), ("focus", &focus)].into(),
        };
        assert!(left != changed);
        let changed = ReferenceMembers::Computed {
            kind: crate::ir::GroupKind::Sequence,
            members: vec![("focus", &focus), ("activate", &focus)].into(),
        };
        assert!(left != changed);
        let external = vec![("focus".into(), focus), ("activate".into(), activate)];
        let borrowed = ReferenceMembers::External(&external);
        assert!(borrowed == borrowed.clone());
        assert!(left != borrowed);
        let invalid = ReferenceMembers::Computed {
            kind: crate::ir::GroupKind::Parallel,
            members: vec![("focus", &focus)].into(),
        };
        assert!(invalid != invalid.clone());
    }

    #[test]
    fn shared_member_lookup_keeps_closed_operation_tags_and_legacy_external_queries() {
        let focus = HostOperation::UiFocus;
        let activate = HostOperation::UiActivate;
        let environment = ReferenceEnvironment { _groups: &[] };
        let group = ReferenceResult {
            ty: Type::Structured,
            members: Some(ReferenceMembers::Computed {
                kind: crate::ir::GroupKind::Sequence,
                members: vec![("focus", &focus), ("activate", &activate)].into(),
            }),
        };
        assert_eq!(
            environment
                .member_result(&group, "activate", &activate)
                .unwrap()
                .ty,
            Type::UiActivate
        );
        assert!(
            environment
                .member_result(&group, "focus", &activate)
                .is_none()
        );
        assert!(
            environment
                .member_result(&group, "missing", &focus)
                .is_none()
        );
        let malformed = ReferenceResult {
            ty: Type::Structured,
            members: Some(ReferenceMembers::Computed {
                kind: crate::ir::GroupKind::Sequence,
                members: vec![("focus", &focus), ("bad tail", &activate)].into(),
            }),
        };
        assert!(
            environment
                .member_result(&malformed, "focus", &focus)
                .is_none()
        );
        let external = vec![("focus".into(), focus)];
        let external = ReferenceResult {
            ty: Type::Structured,
            members: Some(ReferenceMembers::External(&external)),
        };
        assert_eq!(
            environment
                .member_result(&external, "focus", &focus)
                .unwrap()
                .ty,
            Type::UiFocus
        );
        assert!(
            environment
                .member_result(&external, "focus", &activate)
                .is_none()
        );
    }
}
