use leselang_command::{
    CaptureReplayError as Error, ProjectedBinding, ProjectedGroup, ProjectedGroupBinding,
    ProjectedResult, ResultBinding, StructuredField, Value, validate_group_capture_correspondence,
    validate_result_binding_extension,
};
use leselang_runtime_core::{CaptureCorrespondenceLimits, GroupCaptureLimits, ScalarValue};
use leserpent_hir::{Effect, HirBranch, computation::Computation, host_call::HostOperation};

fn raw(node: &str) -> Value {
    Value::UiFocus {
        node_id: node.into(),
    }
}

fn effect() -> Effect {
    Effect::UiFocus {
        node_id: "new".into(),
    }
}

fn result(name: &str, node: &str) -> ProjectedBinding {
    ProjectedBinding {
        name: name.into(),
        result: ProjectedResult::capture(HostOperation::UiFocus, &raw(node)).unwrap(),
    }
}

fn frames() -> (ResultBinding, ResultBinding) {
    let prior = ResultBinding {
        name: "current".into(),
        locals: vec![],
        results: vec![result("old", "old")],
        groups: vec![ProjectedGroupBinding {
            name: "group".into(),
            group: ProjectedGroup {
                members: vec![result("member", "old")],
            },
        }],
        body: Computation::Literal {
            value: ScalarValue::None,
        },
    };
    let mut saved = prior.clone();
    saved.name = "next".into();
    saved.results.push(result("current", "new"));
    (prior, saved)
}

fn branches() -> Vec<HirBranch> {
    vec![HirBranch {
        name: "member".into(),
        result_type: HostOperation::UiFocus.result_type(),
        effect: Effect::UiFocus {
            node_id: "old".into(),
        },
    }]
}

fn rows() -> Vec<StructuredField> {
    vec![
        StructuredField {
            name: "member".into(),
            value: Box::new(raw("old")),
        },
        StructuredField {
            name: "tail".into(),
            value: Box::new(raw("new")),
        },
    ]
}

fn limits() -> CaptureCorrespondenceLimits {
    CaptureCorrespondenceLimits {
        max_saved_bindings: 4,
        max_original_rows: 3,
    }
}

fn group_limits() -> GroupCaptureLimits {
    GroupCaptureLimits {
        min_members: 1,
        max_members: 1,
    }
}

#[test]
fn extension_keeps_aliases_named_retention_and_distinct_late_projection_errors() {
    for case in 0..12 {
        let (mut prior, mut saved) = frames();
        let mut pending = effect();
        let expected = match case {
            0 => Ok(()),
            1 => {
                let mut alias = saved.results[0].clone();
                alias.name = "alias".into();
                saved.results.push(alias);
                Ok(())
            }
            2 => {
                let mut alias = saved.groups[0].clone();
                alias.name = "alias".into();
                saved.groups.push(alias);
                Ok(())
            }
            3 => {
                saved.groups[0].name = "replacement".into();
                Err(Error::Mismatch)
            }
            4 => {
                saved.results.remove(0);
                Err(Error::Mismatch)
            }
            5 => {
                saved.results.pop();
                Err(Error::Mismatch)
            }
            6 => {
                saved.results[1].name = "alias".into();
                Err(Error::Mismatch)
            }
            7 => {
                saved.results.push(result("invented", "other"));
                Err(Error::Mismatch)
            }
            8 => {
                saved.results.push(result("late", "new"));
                saved.results[2].result.projection_version = 0;
                Err(Error::InvalidProjection)
            }
            9 => {
                saved.groups[0].group.members[0] = result("member", "other");
                saved.results[1].result.projection_version = 0;
                Err(Error::Mismatch)
            }
            10 => {
                pending = Effect::Sequence { steps: vec![] };
                saved.results[1].result.projection_version = 0;
                Err(Error::Mismatch)
            }
            11 => {
                prior.results[0].result.projection_version = 0;
                saved.results[0] = prior.results[0].clone();
                Err(Error::InvalidProjection)
            }
            _ => unreachable!(),
        };
        let before = serde_json::to_vec(&(&prior, &saved, &pending)).unwrap();
        assert_eq!(
            validate_result_binding_extension(&prior, &saved, &pending, &raw("new"), 4),
            expected,
            "case {case}"
        );
        assert_eq!(
            serde_json::to_vec(&(&prior, &saved, &pending)).unwrap(),
            before
        );
    }
}

#[test]
fn correspondence_keeps_ordered_groups_full_row_scans_and_error_categories() {
    for case in 0..11 {
        let (_, mut saved) = frames();
        let mut fields = rows();
        let mut operations = vec![HostOperation::UiFocus; 2];
        let expected = match case {
            0 => Ok(()),
            1 => {
                fields.clear();
                operations.clear();
                saved.groups[0].group.members[0].result.projection_version = 0;
                Err(Error::Mismatch)
            }
            2 => {
                operations.pop();
                Err(Error::Mismatch)
            }
            3 => {
                saved.groups[0].group.members[0].result.projection_version = 0;
                Err(Error::InvalidProjection)
            }
            4 => {
                saved.groups[0].group.members[0].name = "other".into();
                Err(Error::Mismatch)
            }
            5 => {
                saved.results[1].result.projection_version = 0;
                Err(Error::InvalidProjection)
            }
            6 => {
                saved.results[1] = result("current", "unmatched");
                Err(Error::Mismatch)
            }
            7 => {
                fields.push(StructuredField {
                    name: "late".into(),
                    value: Box::new(Value::Scalar {
                        value: ScalarValue::Boolean(true),
                    }),
                });
                operations.push(HostOperation::UiFocus);
                Err(Error::InvalidProjection)
            }
            8 => {
                fields[0].name = "other".into();
                Err(Error::Mismatch)
            }
            9 => {
                fields[1].name = "irrelevant-row-name".into();
                Ok(())
            }
            10 => {
                fields.swap(0, 1);
                Err(Error::Mismatch)
            }
            _ => unreachable!(),
        };
        let before = serde_json::to_vec(&(&saved, &fields, &operations)).unwrap();
        assert_eq!(
            validate_group_capture_correspondence(
                &saved,
                &branches(),
                &fields,
                &operations,
                limits(),
                group_limits(),
                |_| true
            ),
            expected,
            "case {case}"
        );
        assert_eq!(
            serde_json::to_vec(&(&saved, &fields, &operations)).unwrap(),
            before
        );
    }
}

#[test]
fn caller_limits_and_row_shapes_precede_callbacks_without_implicit_vm_defaults() {
    let (prior, saved) = frames();
    for (max, expected) in [
        (0, Err(Error::Mismatch)),
        (2, Err(Error::Mismatch)),
        (3, Ok(())),
        (64, Ok(())),
        (65, Err(Error::Mismatch)),
    ] {
        assert_eq!(
            validate_result_binding_extension(&prior, &saved, &effect(), &raw("new"), max),
            expected
        );
    }
    for case in 0..8 {
        let mut policy = limits();
        let mut members = group_limits();
        let mut calls = 0;
        let mut fields = rows();
        let mut operations = vec![HostOperation::UiFocus; 2];
        let expected = match case {
            0 => {
                policy.max_saved_bindings = 2;
                Error::Mismatch
            }
            1 => {
                policy.max_original_rows = 1;
                Error::Mismatch
            }
            2 => {
                policy.max_saved_bindings = 65;
                Error::Mismatch
            }
            3 => {
                policy.max_original_rows = 65;
                Error::Mismatch
            }
            4 => {
                fields.clear();
                operations.clear();
                Error::Mismatch
            }
            5 => {
                operations.clear();
                Error::Mismatch
            }
            6 => {
                members.max_members = 0;
                Error::InvalidProjection
            }
            7 => {
                members.min_members = 2;
                Error::InvalidProjection
            }
            _ => unreachable!(),
        };
        assert_eq!(
            validate_group_capture_correspondence(
                &saved,
                &branches(),
                &fields,
                &operations,
                policy,
                members,
                |_| {
                    calls += 1;
                    true
                }
            ),
            Err(expected)
        );
        assert_eq!(calls, 0);
    }
    let mut empty = saved;
    empty.results.clear();
    empty.groups.clear();
    empty.body = Computation::Local {
        name: "unknown-local".into(),
    };
    // Correspondence is not complete frame/type admission, even at zero bounds.
    assert_eq!(
        validate_group_capture_correspondence(
            &empty,
            &[],
            &[],
            &[],
            CaptureCorrespondenceLimits {
                max_saved_bindings: 0,
                max_original_rows: 0
            },
            group_limits(),
            |_| panic!()
        ),
        Ok(())
    );
}

#[test]
fn comparisons_do_not_replace_frame_admission_raw_identity_or_original_arguments() {
    let (mut prior, mut saved) = frames();
    prior.body = Computation::Local {
        name: "unbound".into(),
    };
    saved.body = prior.body.clone();
    saved.locals.push(leselang_runtime_core::ScalarBinding {
        name: "not a local".into(),
        value: ScalarValue::String("x".repeat(4097)),
    });
    // A different raw result can export the same scalar projection.
    let raw = Value::UiActivate {
        node_id: "new".into(),
    };
    assert_eq!(
        validate_result_binding_extension(
            &prior,
            &saved,
            &Effect::UiFocus {
                node_id: "different-request-argument".into()
            },
            &raw,
            3
        ),
        Ok(())
    );
    let mut original = branches();
    original[0].effect = Effect::UiFocus {
        node_id: "different-request-argument".into(),
    };
    original[0].result_type = leserpent_hir::Type::Scalar(leselang_runtime_core::ScalarType::None);
    assert_eq!(
        validate_group_capture_correspondence(
            &saved,
            &original,
            &rows(),
            &[HostOperation::UiFocus; 2],
            limits(),
            group_limits(),
            |_| true
        ),
        Ok(())
    );
    // The full data/type entry still rejects the malformed local before recovery.
    assert!(
        leselang_command::validate_result_binding(
            &saved,
            &effect(),
            leselang_command::ResultBindingLimits {
                captures: leselang_runtime_core::CaptureValidationLimits {
                    max_saved_bindings: 4,
                    max_name_bytes: 64
                },
                groups: group_limits(),
                accesses: leselang_hir_core::projection_access::ProjectionAccessLimits {
                    max_nodes: 100,
                    max_depth: 16
                },
            },
            |_| true
        )
        .is_err()
    );
}

#[test]
fn name_callback_refusal_and_unwind_keep_prefix_order_without_mutating_captures() {
    let (_, mut saved) = frames();
    let mut alias = saved.groups[0].clone();
    alias.name = "alias".into();
    saved.groups.push(alias);
    let before = serde_json::to_vec(&saved).unwrap();
    let mut names = Vec::new();
    assert_eq!(
        validate_group_capture_correspondence(
            &saved,
            &branches(),
            &rows(),
            &[HostOperation::UiFocus; 2],
            limits(),
            group_limits(),
            |name| {
                names.push(name.to_owned());
                names.len() != 2
            }
        ),
        Err(Error::InvalidProjection)
    );
    assert_eq!(names, ["member", "member"]);
    names.clear();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        validate_group_capture_correspondence(
            &saved,
            &branches(),
            &rows(),
            &[HostOperation::UiFocus; 2],
            limits(),
            group_limits(),
            |name| {
                names.push(name.to_owned());
                assert_eq!(names.len(), 1);
                true
            },
        )
    }));
    assert!(result.is_err());
    assert_eq!(names, ["member", "member"]);
    assert_eq!(serde_json::to_vec(&saved).unwrap(), before);
    for (error, text) in [
        (Error::Mismatch, "product capture correspondence mismatch"),
        (
            Error::InvalidProjection,
            "invalid product capture projection",
        ),
    ] {
        assert_eq!(error.to_string(), text);
        assert!(std::error::Error::source(&error).is_none());
        assert!(!format!("{error:?}").contains("member"));
    }
}
