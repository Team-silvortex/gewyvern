use leselang_command::{
    InvalidProjectedResult, ProjectedBinding, ProjectedGroup, ProjectedGroupBinding,
    ProjectedResult, Value, validate_captured_projection_accesses,
};
use leselang_hir::{
    Effect, computation::Computation, host_call::HostOperation, ir::ComputedArgument,
    projection_access::ProjectionAccessLimits, result_field::ResultField,
};
use leselang_runtime_core::{CapturedBindings, ScalarBinding, ScalarValue};

type Frame = CapturedBindings<Computation, ProjectedResult, ProjectedGroup>;

fn limits() -> ProjectionAccessLimits {
    ProjectionAccessLimits {
        max_nodes: 100,
        max_depth: 16,
    }
}

fn pending() -> Effect {
    Effect::UiAssertText {
        node_id: "node".into(),
        expected: "private text".into(),
    }
}

fn literal() -> Computation {
    Computation::Literal {
        value: ScalarValue::Boolean(true),
    }
}

fn local(name: &str) -> Computation {
    Computation::Local { name: name.into() }
}

fn field(value: Computation, field: ResultField) -> Computation {
    Computation::Field {
        value: Box::new(value),
        field,
    }
}

fn bind(name: &str, value: Computation, body: Computation) -> Computation {
    Computation::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}

fn frame(body: Computation) -> Frame {
    let result = ProjectedResult::capture_version(
        HostOperation::UiAssertText,
        &Value::UiAssertText {
            node_id: "node".into(),
            expected: "private text".into(),
        },
        1,
    )
    .unwrap();
    Frame {
        name: "pending".into(),
        locals: vec![ScalarBinding {
            name: "scalar".into(),
            value: ScalarValue::Integer(1),
        }],
        results: vec![ProjectedBinding {
            name: "saved".into(),
            result: result.clone(),
        }],
        groups: vec![ProjectedGroupBinding {
            name: "group".into(),
            group: ProjectedGroup {
                members: vec![ProjectedBinding {
                    name: "member".into(),
                    result,
                }],
            },
        }],
        body,
    }
}

fn check(frame: &Frame) -> Result<(), InvalidProjectedResult> {
    validate_captured_projection_accesses(frame, &pending(), limits())
}

#[test]
fn closed_fields_are_checked_in_cold_recovery_loops_folds_and_call_arguments() {
    let missing = || field(local("saved"), ResultField::Expected);
    for body in [
        Computation::Recover {
            value: Box::new(literal()),
            fallback: Box::new(missing()),
        },
        Computation::Loop {
            name: "counter".into(),
            initial: Box::new(literal()),
            condition: Box::new(literal()),
            next: Box::new(missing()),
            limit: 0,
        },
        Computation::Fold {
            name: "counter".into(),
            item: "item".into(),
            items: Box::new(Computation::Strings { items: vec![] }),
            initial: Box::new(literal()),
            next: Box::new(missing()),
            limit: 0,
        },
        Computation::Call {
            operation: HostOperation::UiFocus,
            arguments: vec![ComputedArgument {
                name: "node_id".into(),
                value: missing(),
            }],
        },
    ] {
        let captured = frame(body);
        let before = serde_json::to_vec(&captured).unwrap();
        let error = check(&captured).unwrap_err();
        assert_eq!(error.to_string(), "invalid product result projection");
        assert_eq!(format!("{error:?}"), "InvalidProjectedResult");
        assert_eq!(serde_json::to_vec(&captured).unwrap(), before);
    }
}

#[test]
fn capture_names_share_one_namespace_and_lexical_siblings_do_not_leak() {
    for name in ["scalar", "saved", "group", "pending"] {
        assert!(check(&frame(bind(name, literal(), literal()))).is_err());
        let mut captured = frame(literal());
        captured.name = name.into();
        assert_eq!(check(&captured).is_ok(), name == "pending");
    }
    let original = frame(literal());
    let mut duplicate = original.clone();
    duplicate.results.push(original.results[0].clone());
    assert!(check(&duplicate).is_err());
    duplicate = original.clone();
    duplicate.groups[0].name = "scalar".into();
    assert!(check(&duplicate).is_err());
    duplicate = original;
    duplicate.locals[0].name = "saved".into();
    assert!(check(&duplicate).is_err());

    let body = Computation::Choose {
        when: Box::new(literal()),
        then: Box::new(bind(
            "alias",
            local("saved"),
            field(local("alias"), ResultField::NodeId),
        )),
        otherwise: Box::new(bind(
            "alias",
            local("pending"),
            field(local("alias"), ResultField::Expected),
        )),
    };
    assert!(check(&frame(body.clone())).is_ok());
    // An alias from either closed branch cannot be read after that branch.
    assert!(
        check(&frame(bind(
            "after",
            body,
            field(local("alias"), ResultField::NodeId)
        )))
        .is_err()
    );
    for name in ["scalar", "missing", "group"] {
        assert!(check(&frame(field(local(name), ResultField::NodeId))).is_err());
    }
}

#[test]
fn saved_members_require_exact_names_and_operations_but_fallback_is_not_type_admission() {
    for (group, name, operation, allowed) in [
        ("group", "member", HostOperation::UiAssertText, true),
        ("group", "missing", HostOperation::UiAssertText, false),
        ("group", "member", HostOperation::UiFocus, false),
        ("missing", "member", HostOperation::UiAssertText, true),
        ("scalar", "member", HostOperation::UiAssertText, true),
    ] {
        let body = field(
            Computation::Member {
                group: group.into(),
                name: name.into(),
                operation,
            },
            ResultField::NodeId,
        );
        let captured = frame(body);
        assert_eq!(check(&captured).is_ok(), allowed);
        // The separate canonical checker still rejects unbound/non-group names.
        if group != "group" {
            assert!(captured.body.validate_in_scope(&[]).is_err());
        }
    }
    for receiver in [
        local("pending"),
        Computation::Host {
            effect: Box::new(pending()),
        },
    ] {
        assert!(check(&frame(field(receiver, ResultField::Expected))).is_ok());
    }
    let mut malformed = frame(field(local("saved"), ResultField::NodeId));
    malformed.results[0].result.projection_version = 0;
    assert!(malformed.results[0].result.validate().is_err());
    assert!(check(&malformed).is_ok()); // Access observation never replaces schema admission.

    // Joining saved groups exports no closed signature. Catalog fallback alone
    // must not make a conditional group alias admissible to the full checker.
    let conditional_group = frame(bind(
        "alias",
        Computation::Choose {
            when: Box::new(literal()),
            then: Box::new(local("group")),
            otherwise: Box::new(local("group")),
        },
        field(
            Computation::Member {
                group: "alias".into(),
                name: "member".into(),
                operation: HostOperation::UiAssertText,
            },
            ResultField::Expected,
        ),
    ));
    assert!(check(&conditional_group).is_ok());
    assert!(
        conditional_group
            .body
            .validate_in_group_scope(
                &[],
                &[leselang_hir::computation::GroupLocalType {
                    name: "group".into(),
                    members: vec![("member".into(), HostOperation::UiAssertText)],
                },]
            )
            .is_err()
    );
}

#[test]
fn explicit_physical_walk_limits_are_inclusive_and_cover_unselected_branches() {
    let captured = frame(Computation::Choose {
        when: Box::new(literal()),
        then: Box::new(literal()),
        otherwise: Box::new(literal()),
    });
    let before = serde_json::to_vec(&captured).unwrap();
    for (max_nodes, max_depth, allowed) in [
        (4, 1, true),
        (3, 1, false),
        (4, 0, false),
        (0, 1, false),
        (usize::MAX, 1, false),
        (4, usize::MAX, false),
    ] {
        assert_eq!(
            validate_captured_projection_accesses(
                &captured,
                &pending(),
                ProjectionAccessLimits {
                    max_nodes,
                    max_depth
                }
            )
            .is_ok(),
            allowed
        );
    }
    assert_eq!(serde_json::to_vec(&captured).unwrap(), before);
}
