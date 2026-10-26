//! Method-level oracles share unchanged lowering, type and opaque-effect helpers.
use super::*;

// Frozen from the pre-profile working tree on 2026-10-26. Recursive policy
// calls stay within these functions, not the migrated Computation methods.
fn frozen_validate_structure(expression: &Computation) -> Result<(), CanonicalSourceError> {
    validate_shape(expression)
}

fn frozen_is_atomic_tail(expression: &Computation) -> bool {
    match expression {
        Computation::Host { effect } => HostOperation::for_effect(effect).is_some(),
        Computation::Call { arguments, .. } => {
            arguments.iter().all(|argument| argument.value.is_pure())
        }
        Computation::Bind { value, body, .. } => value.is_pure() && frozen_is_atomic_tail(body),
        Computation::Choose {
            when,
            then,
            otherwise,
        } => when.is_pure() && frozen_is_atomic_tail(then) && frozen_is_atomic_tail(otherwise),
        _ => false,
    }
}

fn frozen_prepared_atomic_operation(expression: &Computation) -> Option<HostOperation> {
    let mut pending = vec![(expression, 0usize, true)];
    let mut budget = StructureBudget::new(MAX_COMPUTATION_NODES, MAX_EFFECT_NESTING_DEPTH);
    let mut signature = None;
    while let Some((value, depth, atomic)) = pending.pop() {
        let (extra_nodes, extra_depth) = match value {
            Computation::Literal { value } => source_shape_extra(value),
            _ => (0, 0),
        };
        budget.visit(depth, extra_nodes, extra_depth).ok()?;
        if !atomic {
            if matches!(
                value,
                Computation::Host { .. } | Computation::Call { .. } | Computation::Group { .. }
            ) {
                return None;
            }
            for child in value.children() {
                budget.check_pending(pending.len(), 1).ok()?;
                pending.push((child, depth + 1, false));
            }
            continue;
        }
        let operation = match value {
            Computation::Host { effect } => {
                budget.visit(depth, 0, 1).ok()?;
                HostOperation::for_effect(effect)?
            }
            Computation::Call {
                operation,
                arguments,
            } => {
                for argument in arguments {
                    budget.check_pending(pending.len(), 1).ok()?;
                    pending.push((&argument.value, depth + 1, false));
                }
                *operation
            }
            Computation::Bind { value, body, .. } => {
                budget.check_pending(pending.len(), 2).ok()?;
                pending.push((value, depth + 1, false));
                pending.push((body, depth + 1, true));
                continue;
            }
            Computation::Choose {
                when,
                then,
                otherwise,
            } => {
                budget.check_pending(pending.len(), 3).ok()?;
                pending.push((when, depth + 1, false));
                pending.push((then, depth + 1, true));
                pending.push((otherwise, depth + 1, true));
                continue;
            }
            _ => return None,
        };
        if signature.is_some_and(|expected| expected != operation) {
            return None;
        }
        signature = Some(operation);
    }
    signature
}

fn frozen_is_atomic_capture(expression: &Computation) -> bool {
    match expression {
        Computation::Bind { value, body, .. } if value.is_pure() => frozen_is_atomic_capture(body),
        Computation::Bind { value, body, .. } => {
            frozen_prepared_atomic_operation(value).is_some() && body.is_pure()
        }
        Computation::Choose {
            when,
            then,
            otherwise,
        } => {
            when.is_pure() && frozen_is_atomic_capture(then) && frozen_is_atomic_capture(otherwise)
        }
        _ => false,
    }
}

fn frozen_is_result_chain(expression: &Computation) -> bool {
    match expression {
        Computation::Host { .. } | Computation::Call { .. } => frozen_is_atomic_tail(expression),
        Computation::Bind { value, body, .. } => {
            if value.is_pure() {
                frozen_is_result_chain(body)
            } else {
                frozen_prepared_atomic_operation(value).is_some()
                    && (body.is_pure() || frozen_is_result_chain(body))
            }
        }
        Computation::Choose {
            when,
            then,
            otherwise,
        } => when.is_pure() && frozen_is_result_chain(then) && frozen_is_result_chain(otherwise),
        _ => false,
    }
}

fn frozen_atomic_chain_bound(expression: &Computation) -> Option<usize> {
    if !expression.is_pure() && !frozen_is_result_chain(expression) {
        return None;
    }
    frozen_atomic_flow_bound(expression)
}

fn frozen_atomic_flow_bound(expression: &Computation) -> Option<usize> {
    if !frozen_is_result_flow(expression) {
        return None;
    }
    if expression.is_pure() {
        return Some(0);
    }
    match expression {
        Computation::Host { .. } | Computation::Call { .. }
            if frozen_is_atomic_tail(expression) =>
        {
            Some(1)
        }
        Computation::Bind { value, body, .. } if value.is_pure() => frozen_atomic_flow_bound(body),
        Computation::Bind { value, body, .. }
            if frozen_prepared_atomic_operation(value).is_some() =>
        {
            frozen_atomic_flow_bound(body)?.checked_add(1)
        }
        Computation::Choose {
            when,
            then,
            otherwise,
        } if when.is_pure() => {
            Some(frozen_atomic_flow_bound(then)?.max(frozen_atomic_flow_bound(otherwise)?))
        }
        _ => None,
    }
}

fn frozen_is_result_flow(expression: &Computation) -> bool {
    if expression.is_pure() {
        return true;
    }
    match expression {
        Computation::Host { .. } | Computation::Call { .. } => frozen_is_atomic_tail(expression),
        Computation::Bind { value, body, .. } => {
            (value.is_pure() || frozen_prepared_atomic_operation(value).is_some())
                && frozen_is_result_flow(body)
        }
        Computation::Choose {
            when,
            then,
            otherwise,
        } => when.is_pure() && frozen_is_result_flow(then) && frozen_is_result_flow(otherwise),
        _ => false,
    }
}

fn frozen_can_return_without_suspending(expression: &Computation) -> bool {
    if expression.is_pure() {
        return true;
    }
    match expression {
        Computation::Bind { value, body, .. } => {
            value.is_pure() && frozen_can_return_without_suspending(body)
        }
        Computation::Choose {
            when,
            then,
            otherwise,
        } => {
            when.is_pure()
                && (frozen_can_return_without_suspending(then)
                    || frozen_can_return_without_suspending(otherwise))
        }
        _ => false,
    }
}

fn frozen_validate_in_scope(
    expression: &Computation,
    scope: &[(String, Type)],
) -> Result<Type, CanonicalSourceError> {
    frozen_validate_in_group_scope(expression, scope, &[])
}

fn frozen_validate_in_type_scope(
    expression: &Computation,
    scope: &TypeScope<'_, '_>,
) -> Result<Type, CanonicalSourceError> {
    if scope.len() > crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS {
        return Err(CanonicalSourceError::RoundTripMismatch);
    }
    let mut values = Vec::new();
    let mut groups = Vec::new();
    for (name, local) in scope.bindings() {
        if let Some(members) = &local.members {
            if members.is_empty() || members.len() > MAX_SEQUENCE_STEPS {
                return Err(CanonicalSourceError::RoundTripMismatch);
            }
            groups.push(GroupLocalType {
                name: (*name).to_owned(),
                members: members.clone(),
            });
        } else {
            values.push(((*name).to_owned(), local.ty));
        }
    }
    frozen_validate_scoped(
        expression,
        &values,
        &groups,
        crate::pure_typing::MAX_TYPE_INFERENCE_BINDINGS,
        0,
        MAX_SEQUENCE_STEPS,
    )
}

fn frozen_validate_in_group_scope(
    expression: &Computation,
    scope: &[(String, Type)],
    groups: &[GroupLocalType],
) -> Result<Type, CanonicalSourceError> {
    frozen_validate_scoped(
        expression,
        scope,
        groups,
        MAX_EFFECT_NESTING_DEPTH,
        scope.len().saturating_add(groups.len()),
        MAX_SEQUENCE_STEPS - 1,
    )
}

fn frozen_validate_scoped(
    expression: &Computation,
    scope: &[(String, Type)],
    groups: &[GroupLocalType],
    max_bindings: usize,
    root_depth: usize,
    max_group_members: usize,
) -> Result<Type, CanonicalSourceError> {
    validate_shape(expression)?;
    let mut names = HashSet::new();
    let scope_len = scope.len().saturating_add(groups.len());
    if scope_len > max_bindings
        || scope
            .iter()
            .any(|(name, _)| !valid_local(name) || !names.insert(name))
        || groups.iter().any(|group| {
            let mut members = HashSet::new();
            !valid_local(&group.name)
                || !names.insert(&group.name)
                || group.members.is_empty()
                || group.members.len() > max_group_members
                || group.members.iter().any(|(name, _)| {
                    name.is_empty()
                        || name.len() > MAX_BRANCH_NAME_BYTES
                        || !name
                            .bytes()
                            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-'))
                        || !members.insert(name)
                })
        })
    {
        return Err(CanonicalSourceError::RoundTripMismatch);
    }
    let source = format!("fn main() = {}", source(expression));
    let formatted = format_syntax(&parse(&source)).map_err(CanonicalSourceError::Syntax)?;
    let tree = parse(&formatted);
    let function = tree
        .function
        .ok_or(CanonicalSourceError::RoundTripMismatch)?;
    let mut visited = scope_len;
    let mut bindings = scope
        .iter()
        .map(|(name, ty)| (name.as_str(), LocalType::from(*ty)))
        .chain(groups.iter().map(|group| {
            (
                group.name.as_str(),
                LocalType {
                    ty: Type::Structured,
                    members: Some(group.members.clone()),
                },
            )
        }))
        .collect();
    let mut lexical = ScopeFrame::new(&mut bindings);
    let (roundtrip, ty) = lower_expression(&function.body, &mut lexical, &mut visited, root_depth)
        .map_err(CanonicalSourceError::InvalidEffect)?;
    if roundtrip != *expression {
        return Err(CanonicalSourceError::RoundTripMismatch);
    }
    if expression.is_pure()
        && crate::pure_reference::infer(expression, scope, groups)
            .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
            != ty
    {
        return Err(CanonicalSourceError::RoundTripMismatch);
    }
    if let Computation::Call {
        operation,
        arguments,
    } = expression
        && crate::pure_reference::infer_call(*operation, arguments, scope, groups)
            .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
            != ty
    {
        return Err(CanonicalSourceError::RoundTripMismatch);
    }
    let prepared_call = !matches!(expression, Computation::Call { .. })
        && frozen_prepared_atomic_operation(expression).is_some()
        && crate::pure_reference::call_leaves_only(expression);
    if prepared_call
        && crate::pure_reference::infer_prepared(expression, scope, groups)
            .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
            != ty
    {
        return Err(CanonicalSourceError::RoundTripMismatch);
    }
    if !prepared_call
        && !matches!(expression, Computation::Call { .. })
        && !expression.is_pure()
        && crate::pure_reference::call_flow_nodes_only(expression)
        && crate::pure_reference::infer_flow(expression, scope, groups)
            .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
            != ty
    {
        return Err(CanonicalSourceError::RoundTripMismatch);
    }
    if crate::pure_reference::group_flow_nodes_only(expression)
        && crate::pure_reference::infer_group_flow(expression, scope, groups)
            .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
            != ty
    {
        return Err(CanonicalSourceError::RoundTripMismatch);
    }
    if crate::pure_reference::host_flow_nodes_only(
        expression,
        crate::pure_reference::supports_host_flow,
    ) && crate::pure_reference::infer_host_flow(expression, scope, groups)
        .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
        != ty
    {
        return Err(CanonicalSourceError::RoundTripMismatch);
    }
    if crate::pure_reference::host_group_flow_nodes_only(
        expression,
        crate::pure_reference::supports_host_flow,
    ) && crate::pure_reference::infer_host_group_flow(expression, scope, groups)
        .map_err(|_| CanonicalSourceError::RoundTripMismatch)?
        != ty
    {
        return Err(CanonicalSourceError::RoundTripMismatch);
    }
    Ok(ty)
}

fn frozen_required_capabilities(expression: &Computation) -> BTreeSet<&'static str> {
    let mut required = BTreeSet::new();
    let mut pending = vec![expression];
    while let Some(expression) = pending.pop() {
        match expression {
            Computation::Host { effect } => {
                required.extend(required_capabilities_for_effect(effect))
            }
            Computation::Call { operation, .. } => {
                required.insert(operation.required_capability());
            }
            _ => {}
        }
        pending.extend(expression.children());
    }
    required
}

fn scalar() -> Computation {
    Computation::Literal {
        value: ScalarValue::Integer(1),
    }
}

fn corpus() -> Vec<Computation> {
    let mut trees = Vec::new();
    for source in [
        "7",
        "none",
        r#""text""#,
        "runtime.list()",
        r#"runtime.refresh(runtime_id: "a")"#,
        r#"ui.focus(node_id: "a")"#,
        r#"bind(result: ui.focus(node_id: "a"), body: field(value: result, name: "node_id"))"#,
        r#"choose(when: true, then: ui.focus(node_id: "a"), otherwise: ui.focus(node_id: "b"))"#,
        "bind(n: 1, body: add(left: n, right: 2))",
        r#"all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
        "seq(a: runtime.list(), b: runtime.list())",
        "recover(value: div(left: 1, right: 0), fallback: 7)",
    ] {
        let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
        let node = match program.function.effect {
            Effect::Compute { expression } => *expression,
            effect => Computation::Host {
                effect: Box::new(effect),
            },
        };
        trees.extend([
            node.clone(),
            Computation::Bind {
                name: "prelude".into(),
                value: Box::new(scalar()),
                body: Box::new(node.clone()),
            },
            Computation::Bind {
                name: "capture".into(),
                value: Box::new(node.clone()),
                body: Box::new(scalar()),
            },
            Computation::Choose {
                when: Box::new(Computation::Literal {
                    value: ScalarValue::Boolean(false),
                }),
                then: Box::new(node.clone()),
                otherwise: Box::new(scalar()),
            },
        ]);
    }
    trees.extend([
        Computation::Local {
            name: "value".into(),
        },
        Computation::Local {
            name: "not valid".into(),
        },
        Computation::Member {
            group: "group".into(),
            name: "a".into(),
            operation: HostOperation::UiFocus,
        },
        Computation::Call {
            operation: HostOperation::UiFocus,
            arguments: Vec::new(),
        },
        Computation::Literal {
            value: ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1)),
        },
        Computation::Strings {
            items: vec![scalar(); MAX_COMPUTATION_NODES + 1],
        },
    ]);
    let mut deep = scalar();
    for _ in 0..=MAX_EFFECT_NESTING_DEPTH {
        deep = Computation::Bind {
            name: "deep".into(),
            value: Box::new(scalar()),
            body: Box::new(deep),
        };
    }
    trees.push(deep);
    trees
}

#[test]
fn product_profile_preserves_frozen_method_results_faults_and_cold_branches() {
    let mut cases = 0;
    let scopes = [
        vec![],
        vec![("value".into(), Type::Scalar(ScalarType::Integer))],
        vec![
            ("value".into(), Type::Scalar(ScalarType::Integer)),
            ("value".into(), Type::Structured),
        ],
        vec![("invalid local".into(), Type::Structured)],
    ];
    let groups = [
        vec![],
        vec![GroupLocalType {
            name: "group".into(),
            members: vec![("a".into(), HostOperation::UiFocus)],
        }],
        vec![GroupLocalType {
            name: "value".into(),
            members: vec![],
        }],
        vec![GroupLocalType {
            name: "group".into(),
            members: vec![
                ("a".into(), HostOperation::UiFocus),
                ("a".into(), HostOperation::RuntimeList),
            ],
        }],
    ];
    let trees = corpus();
    assert_eq!(trees.len(), 55);
    for tree in &trees {
        assert_eq!(tree.validate_structure(), frozen_validate_structure(tree));
        assert_eq!(tree.is_atomic_tail(), frozen_is_atomic_tail(tree));
        assert_eq!(
            tree.prepared_atomic_operation(),
            frozen_prepared_atomic_operation(tree)
        );
        assert_eq!(tree.is_atomic_capture(), frozen_is_atomic_capture(tree));
        assert_eq!(tree.is_result_chain(), frozen_is_result_chain(tree));
        assert_eq!(tree.atomic_chain_bound(), frozen_atomic_chain_bound(tree));
        assert_eq!(tree.atomic_flow_bound(), frozen_atomic_flow_bound(tree));
        assert_eq!(tree.is_result_flow(), frozen_is_result_flow(tree));
        assert_eq!(
            tree.can_return_without_suspending(),
            frozen_can_return_without_suspending(tree)
        );
        assert_eq!(
            tree.required_capabilities(),
            frozen_required_capabilities(tree)
        );
        for scope in &scopes {
            assert_eq!(
                tree.validate_in_scope(scope),
                frozen_validate_in_scope(tree, scope)
            );
            for group in &groups {
                assert_eq!(
                    tree.validate_in_group_scope(scope, group),
                    frozen_validate_in_group_scope(tree, scope, group)
                );
                cases += 1;
            }
        }
    }
    assert_eq!(cases, 880);
}

#[test]
fn private_lexical_validation_retains_original_scalar_and_group_views() {
    let mut cases = 0;
    for tree in corpus() {
        for members in [
            None,
            Some(vec![]),
            Some(vec![("a".into(), HostOperation::UiFocus)]),
        ] {
            let mut values = vec![
                (
                    "value",
                    LocalType {
                        ty: Type::Scalar(ScalarType::Integer),
                        members: None,
                    },
                ),
                (
                    "group",
                    LocalType {
                        ty: Type::Structured,
                        members,
                    },
                ),
            ];
            let scope = ScopeFrame::new(&mut values);
            assert_eq!(
                validate_in_type_scope(&tree, &scope),
                frozen_validate_in_type_scope(&tree, &scope)
            );
            cases += 1;
        }
    }
    assert_eq!(cases, 165);
}
