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
fn group_continuation_data_preserves_closed_wire_defaults_and_unchecked_observations() {
    use leselang_command::GroupResultBinding;
    let group = GroupResultBinding {
        pending: Effect::Sequence { steps: vec![] },
        binding: Box::new(frame()),
        fuel_remaining: u64::MAX,
        successor_sequence: None,
        additional_successor_sequences: vec![],
    };
    let bytes = serde_json::to_vec(&group).unwrap();
    let mut wire = serde_json::to_value(&group).unwrap();
    assert!(wire.get("successor_sequence").is_none());
    assert!(wire.get("additional_successor_sequences").is_none());
    assert_eq!(
        serde_json::from_slice::<GroupResultBinding>(&bytes).unwrap(),
        group
    );
    wire["successor_sequence"] = 0.into();
    wire["additional_successor_sequences"] = serde_json::json!([u64::MAX, 0, 0]);
    let unchecked: GroupResultBinding = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(
        unchecked.reserved_sequences().collect::<Vec<_>>(),
        [0, u64::MAX, 0, 0]
    );
    assert_eq!(unchecked.fuel_remaining, u64::MAX);
    wire["unknown"] = true.into();
    assert!(serde_json::from_value::<GroupResultBinding>(wire).is_err());
    let mut nested = serde_json::to_value(&group).unwrap();
    nested["binding"]["unknown"] = true.into();
    assert!(serde_json::from_value::<GroupResultBinding>(nested).is_err());
}

#[test]
fn group_continuation_classification_keeps_conditional_dataflow_and_parallel_precedence() {
    use leselang_command::GroupResultBinding;
    let pure = Computation::Literal {
        value: ScalarValue::None,
    };
    let host = Computation::Host {
        effect: Box::new(pending()),
    };
    let capture = Computation::Bind {
        name: "next".into(),
        value: Box::new(host.clone()),
        body: Box::new(pure.clone()),
    };
    let chain = Computation::Bind {
        name: "first".into(),
        value: Box::new(host.clone()),
        body: Box::new(capture.clone()),
    };
    let conditional = Computation::Choose {
        when: Box::new(Computation::Literal {
            value: ScalarValue::Boolean(false),
        }),
        then: Box::new(capture.clone()),
        otherwise: Box::new(pure.clone()),
    };
    for (kind, body) in [pure, host, capture, chain, conditional]
        .into_iter()
        .enumerate()
    {
        body.validate_structure().unwrap();
        for parallel in [false, true] {
            for tail in [false, true] {
                let mut binding = frame();
                binding.body = body.clone();
                let group = GroupResultBinding {
                    pending: if parallel {
                        Effect::All { branches: vec![] }
                    } else {
                        Effect::Sequence { steps: vec![] }
                    },
                    binding: Box::new(binding),
                    fuel_remaining: 100,
                    successor_sequence: tail.then_some(2),
                    additional_successor_sequences: vec![],
                };
                let before = serde_json::to_vec(&group).unwrap();
                assert_eq!(group.is_conditional(), kind == 4);
                assert_eq!(group.is_dataflow(), kind == 3);
                assert_eq!(group.captures_successor(), kind >= 2);
                let expected = if kind == 4 {
                    11
                } else if kind == 3 {
                    10
                } else if parallel && tail {
                    9
                } else if kind == 2 {
                    8
                } else if tail {
                    7
                } else {
                    6
                };
                assert_eq!(group.continuation_schema(), expected);
                assert_eq!(serde_json::to_vec(&group).unwrap(), before);
            }
        }
    }
}

#[test]
fn successor_schema_matrix_preserves_version_edges_without_authenticating_owners() {
    use leselang_command::continuation_schema::successor_schema_matches;
    let owners = [
        None,
        Some("merge-1"),
        Some("merge-2"),
        Some("merge-01"),
        Some(""),
    ];
    let mut comparisons = 0;
    for prior in (0..=12).chain([u32::MAX]) {
        for next in (0..=12).chain([u32::MAX]) {
            for prior_owner in owners {
                for next_owner in owners {
                    let expected = match prior {
                        3 => next == 1,
                        4 => matches!(next, 1 | 4),
                        5 => matches!(next, 1 | 4 | 5),
                        10 | 11 => {
                            prior == next && prior_owner.is_some() && prior_owner == next_owner
                        }
                        _ => false,
                    };
                    assert_eq!(
                        successor_schema_matches(prior, prior_owner, next, next_owner),
                        expected
                    );
                    comparisons += 1;
                }
            }
        }
    }
    assert_eq!(comparisons, 4_900);
    // Frame admission, not this compatibility predicate, authenticates owner spelling.
    assert!(successor_schema_matches(10, Some(""), 10, Some("")));
    assert!(!successor_schema_matches(10, None, 10, None));
}

#[test]
fn continuation_schema_selection_keeps_all_released_tags_and_binding_shapes() {
    use leselang_command::continuation_schema::*;
    assert_eq!(
        [
            CONTINUATION_SCHEMA_VERSION,
            RESULT_BINDING_CONTINUATION_SCHEMA_VERSION,
            SUCCESSOR_CONTINUATION_SCHEMA_VERSION,
            DATAFLOW_CONTINUATION_SCHEMA_VERSION,
            CONDITIONAL_CONTINUATION_SCHEMA_VERSION,
            GROUP_RESULT_CONTINUATION_SCHEMA_VERSION,
            GROUP_TAIL_CONTINUATION_SCHEMA_VERSION,
            GROUP_CAPTURE_CONTINUATION_SCHEMA_VERSION,
            PARALLEL_GROUP_CONTINUATION_SCHEMA_VERSION,
            GROUP_DATAFLOW_CONTINUATION_SCHEMA_VERSION,
            GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION
        ],
        [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11],
    );
    let pure = Computation::Literal {
        value: ScalarValue::None,
    };
    let host = Computation::Host {
        effect: Box::new(pending()),
    };
    let chain = Computation::Bind {
        name: "next".into(),
        value: Box::new(host.clone()),
        body: Box::new(pure.clone()),
    };
    let flow = Computation::Choose {
        when: Box::new(Computation::Literal {
            value: ScalarValue::Boolean(false),
        }),
        then: Box::new(chain.clone()),
        otherwise: Box::new(pure.clone()),
    };
    for (body, schemas, grouped_versions) in [
        (pure, [2, 4, 8, 8], vec![8, 9, 10, 11]),
        (host, [3, 4, 10, 10], vec![10, 11]),
        (chain, [4, 4, 10, 10], vec![10, 11]),
        (flow, [5, 5, 11, 11], vec![11]),
    ] {
        for (index, (results, groups)) in
            [(false, false), (true, false), (false, true), (true, true)]
                .into_iter()
                .enumerate()
        {
            let mut binding = frame();
            binding.body = body.clone();
            if !results {
                binding.results.clear();
            }
            if !groups {
                binding.groups.clear();
            }
            let policy = limits();
            validate_result_binding_structure(&binding, policy.captures, policy.groups, |_| true)
                .unwrap();
            let before = serde_json::to_vec(&binding).unwrap();
            assert_eq!(
                preflighted_result_binding_schema_version(&binding),
                schemas[index]
            );
            for version in (0..=12).chain([u32::MAX]) {
                let actual = validate_preflighted_continuation_schema(
                    Some(&binding),
                    groups.then_some("merge-1"),
                    version,
                    1,
                );
                let expected = if groups {
                    if grouped_versions.contains(&version) {
                        Ok(())
                    } else {
                        Err(ContinuationSchemaError::Group)
                    }
                } else if version == schemas[index] {
                    Ok(())
                } else {
                    Err(ContinuationSchemaError::Version {
                        actual: version,
                        expected: schemas[index],
                    })
                };
                assert_eq!(actual, expected);
            }
            assert_eq!(serde_json::to_vec(&binding).unwrap(), before);
        }
    }
}

#[test]
fn continuation_schema_preserves_legacy_group_only_version_matrix() {
    use leselang_command::continuation_schema::*;
    for version in (0..=12).chain([u32::MAX]) {
        let group = validate_preflighted_continuation_schema(None, Some("merge-1"), version, 1);
        assert_eq!(
            group,
            if (6..=11).contains(&version) {
                Ok(())
            } else {
                Err(ContinuationSchemaError::Version {
                    actual: version,
                    expected: 6,
                })
            }
        );
        let plain = validate_preflighted_continuation_schema(None, None, version, 0);
        assert_eq!(
            plain,
            if version == 1 {
                Ok(())
            } else {
                Err(ContinuationSchemaError::Version {
                    actual: version,
                    expected: 1,
                })
            }
        );
    }
}

#[test]
fn continuation_schema_group_identity_uses_explicit_full_width_limits_and_no_repairs() {
    use leselang_command::continuation_schema::*;
    for limit in [0, 1, i64::MAX as u64 - 1, u64::MAX] {
        for sequence in [0, 1, i64::MAX as u64 - 1, i64::MAX as u64, u64::MAX] {
            assert_eq!(
                validate_preflighted_continuation_schema(
                    None,
                    Some(&format!("merge-{sequence}")),
                    6,
                    limit
                ),
                if sequence > 0 && sequence <= limit {
                    Ok(())
                } else {
                    Err(ContinuationSchemaError::Group)
                },
            );
        }
    }
    for owner in [
        "",
        "merge-",
        "merge-01",
        "merge-+1",
        "merge--1",
        "merge- 1",
        "merge-1 ",
        "foreign-1",
        "merge-18446744073709551616",
    ] {
        assert_eq!(
            validate_preflighted_continuation_schema(None, Some(owner), u32::MAX, u64::MAX),
            Err(ContinuationSchemaError::Group)
        );
    }
}

#[test]
fn continuation_schema_group_refusals_precede_versions_without_exposing_payloads() {
    use leselang_command::continuation_schema::*;
    let mut binding = frame();
    assert_eq!(
        validate_preflighted_continuation_schema(Some(&binding), None, u32::MAX, 1),
        Err(ContinuationSchemaError::Group)
    );
    binding.groups.clear();
    assert_eq!(
        validate_preflighted_continuation_schema(Some(&binding), Some("merge-1"), 8, 1),
        Err(ContinuationSchemaError::Group)
    );
    let group = ContinuationSchemaError::Group;
    assert_eq!(group.to_string(), "incompatible continuation group");
    assert!(std::error::Error::source(&group).is_none());
    let version = ContinuationSchemaError::Version {
        actual: 12,
        expected: 1,
    };
    assert_eq!(
        version.to_string(),
        "unsupported continuation version 12, expected 1"
    );
    assert!(std::error::Error::source(&version).is_none());
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
