use leselang_command::{
    InvalidProjectedResult, ProjectedBinding, ProjectedGroup, ProjectedGroupBinding,
    ProjectedResult, ResultBinding, ResultBindingLimits, Value, validate_result_binding,
    validate_result_binding_structure,
};
use leselang_hir::{
    Effect, Type, computation::Computation, host_call::HostOperation,
    projection_access::ProjectionAccessLimits, result_field::ResultField,
};
use leselang_runtime_core::{
    CaptureValidationLimits, GroupCaptureLimits, ScalarBinding, ScalarType, ScalarValue,
};

fn limits() -> ResultBindingLimits {
    ResultBindingLimits {
        captures: CaptureValidationLimits {
            max_saved_bindings: 3,
            max_name_bytes: 16,
        },
        groups: GroupCaptureLimits {
            min_members: 1,
            max_members: 2,
        },
        accesses: ProjectionAccessLimits {
            max_nodes: 100,
            max_depth: 16,
        },
    }
}

fn pending() -> Effect {
    Effect::UiFocus {
        node_id: "pending".into(),
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

fn frame() -> ResultBinding {
    let result = ProjectedResult::capture_version(
        HostOperation::UiAssertText,
        &Value::UiAssertText {
            node_id: "node".into(),
            expected: "private payload".into(),
        },
        1,
    )
    .unwrap();
    ResultBinding {
        name: "reply".into(),
        locals: vec![ScalarBinding {
            name: "scalar".into(),
            value: ScalarValue::Integer(7),
        }],
        results: vec![ProjectedBinding {
            name: "saved".into(),
            result: result.clone(),
        }],
        groups: vec![ProjectedGroupBinding {
            name: "group".into(),
            group: ProjectedGroup {
                members: vec![
                    ProjectedBinding {
                        name: "first".into(),
                        result: result.clone(),
                    },
                    ProjectedBinding {
                        name: "second".into(),
                        result,
                    },
                ],
            },
        }],
        body: field(local("saved"), ResultField::NodeId),
    }
}

fn check(frame: &ResultBinding) -> Result<Type, InvalidProjectedResult> {
    validate_result_binding(frame, &pending(), limits(), |_| true)
}

#[test]
fn complete_validation_keeps_scalar_literal_scope_and_typed_saved_scope_paths() {
    let mut captured = frame();
    assert_eq!(check(&captured), Ok(Type::Scalar(ScalarType::String)));
    for (body, expected) in [
        (local("scalar"), ScalarType::Integer),
        (
            field(local("reply"), ResultField::NodeId),
            ScalarType::String,
        ),
        (
            field(
                Computation::Member {
                    group: "group".into(),
                    name: "first".into(),
                    operation: HostOperation::UiAssertText,
                },
                ResultField::NodeId,
            ),
            ScalarType::String,
        ),
    ] {
        captured.body = body;
        let before = serde_json::to_vec(&captured).unwrap();
        assert_eq!(check(&captured), Ok(Type::Scalar(expected)));
        assert_eq!(serde_json::to_vec(&captured).unwrap(), before);
    }
    captured.results.clear();
    captured.groups.clear();
    captured.body = local("scalar");
    let mut policy = limits();
    policy.accesses = ProjectionAccessLimits {
        max_nodes: 0,
        max_depth: 0,
    };
    // Scalar-only frames still use full literal reconstruction, not the observer.
    assert_eq!(
        validate_result_binding(&captured, &pending(), policy, |_| panic!()),
        Ok(Type::Scalar(ScalarType::Integer))
    );
    captured.locals.push(captured.locals[0].clone());
    assert!(
        validate_result_binding_structure(&captured, policy.captures, policy.groups, |_| true)
            .is_ok()
    );
    assert!(validate_result_binding(&captured, &pending(), policy, |_| true).is_err());
    captured.locals.clear();
    captured.body = field(local("reply"), ResultField::NodeId);
    policy.captures.max_saved_bindings = 0;
    assert_eq!(
        validate_result_binding(&captured, &pending(), policy, |_| true),
        Ok(Type::Scalar(ScalarType::String))
    );
}

#[test]
fn complete_validation_rejects_legacy_widening_names_and_forged_group_types() {
    for corruption in 0..9 {
        let mut captured = frame();
        match corruption {
            0 => captured.body = field(local("saved"), ResultField::Expected),
            1 => {
                captured.body = field(
                    Computation::Member {
                        group: "group".into(),
                        name: "first".into(),
                        operation: HostOperation::UiAssertText,
                    },
                    ResultField::Expected,
                )
            }
            2 => {
                captured.body = field(
                    Computation::Member {
                        group: "missing".into(),
                        name: "first".into(),
                        operation: HostOperation::UiFocus,
                    },
                    ResultField::NodeId,
                )
            }
            3 => captured.body = local("unknown"),
            4 => captured.results[0].name = "scalar".into(),
            5 => captured.name = "saved".into(),
            6 => captured.locals[0].name = "not a local".into(),
            7 => {
                captured.body = field(
                    Computation::Member {
                        group: "group".into(),
                        name: "first".into(),
                        operation: HostOperation::UiFocus,
                    },
                    ResultField::NodeId,
                )
            }
            8 => captured.groups[0].group.members[0].name = "dot.name".into(),
            _ => unreachable!(),
        }
        let before = serde_json::to_vec(&captured).unwrap();
        // Length/shape admission is deliberately weaker than lexical/type validity.
        assert!(
            validate_result_binding_structure(
                &captured,
                limits().captures,
                limits().groups,
                |_| true
            )
            .is_ok()
        );
        let error = check(&captured).unwrap_err();
        assert_eq!(format!("{error:?}"), "InvalidProjectedResult");
        assert_eq!(error.to_string(), "invalid product result projection");
        assert_eq!(serde_json::to_vec(&captured).unwrap(), before);
    }
}

#[test]
fn explicit_limits_and_ordered_name_hooks_precede_access_and_canonical_work() {
    for case in 0..10 {
        let mut captured = frame();
        let mut policy = limits();
        match case {
            0 => policy.captures.max_saved_bindings = 2,
            1 => captured.name = "too-long-pending-name".into(),
            2 => captured.locals[0].value = ScalarValue::String("x".repeat(4097)),
            3 => captured.results[0].result.projection_version = 0,
            4 => policy.groups.max_members = 1,
            5 => {
                captured.groups[0].group.members[0]
                    .result
                    .projection_version = 0
            }
            6 => captured.groups[0].group.members[1].name = "first".into(),
            7 => policy.accesses.max_nodes = 0,
            8 => captured.body = local("unknown"),
            9 => {
                policy.groups = GroupCaptureLimits {
                    min_members: 3,
                    max_members: 2,
                }
            }
            _ => unreachable!(),
        }
        let before = serde_json::to_vec(&captured).unwrap();
        let mut names = Vec::new();
        assert!(
            validate_result_binding(&captured, &pending(), policy, |name| {
                names.push(name.to_owned());
                true
            })
            .is_err()
        );
        let expected: &[&str] = match case {
            0..=4 | 9 => &[],
            6 => &["first", "first"],
            _ => &["first", "second"],
        };
        assert_eq!(names, expected, "case {case}");
        assert_eq!(serde_json::to_vec(&captured).unwrap(), before);
    }
    let captured = frame();
    let before = serde_json::to_vec(&captured).unwrap();
    let mut names = Vec::new();
    assert!(
        validate_result_binding(&captured, &pending(), limits(), |name| {
            names.push(name.to_owned());
            false
        })
        .is_err()
    );
    assert_eq!(names, ["first"]);
    names.clear();
    let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        validate_result_binding(&captured, &pending(), limits(), |name| {
            names.push(name.to_owned());
            assert_ne!(name, "second", "native callback unwind");
            true
        })
    }));
    assert!(outcome.is_err());
    assert_eq!(names, ["first", "second"]);
    assert_eq!(serde_json::to_vec(&captured).unwrap(), before);
}
