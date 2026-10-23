use leselang_hir::{Effect, Type};
use leselang_runtime_core::GroupCaptureLimits;

use crate::Fault;

pub use leselang_command::{
    ProjectedBinding, ProjectedField, ProjectedGroup, ProjectedGroupBinding, ProjectedResult,
    ResultBinding,
};
pub use leselang_runtime_core::ScalarBinding;

#[cfg(test)]
pub(super) use leselang_command::ProjectedGroupExt as ReferenceProjectedGroup;

pub(super) fn group_capture_limits() -> GroupCaptureLimits {
    result_binding_limits().groups
}

pub(super) fn projection_fault(_: leselang_command::InvalidProjectedResult) -> Fault {
    invalid()
}

pub(super) fn invalid() -> Fault {
    leselang_command::reference_request::continuation_admission_fault(
        leselang_command::ContinuationAdmissionError::Binding,
    )
}

pub(super) fn result_binding_limits() -> leselang_command::ResultBindingLimits {
    leselang_command::reference_request::result_binding_limits()
}

pub(super) trait ReferenceResultBinding {
    fn schema_version(&self) -> u32;
    #[cfg(test)]
    fn validate_structure(&self) -> Result<(), Fault>;
    fn validate(&self, pending: &Effect) -> Result<Type, Fault>;
}

impl ReferenceResultBinding for ResultBinding {
    fn schema_version(&self) -> u32 {
        leselang_command::continuation_schema::preflighted_result_binding_schema_version(self)
    }

    #[cfg(test)]
    fn validate_structure(&self) -> Result<(), Fault> {
        let limits = result_binding_limits();
        leselang_command::validate_result_binding_structure(
            self,
            limits.captures,
            limits.groups,
            crate::valid_merge_branch_name,
        )
        .map_err(projection_fault)
    }

    fn validate(&self, pending: &Effect) -> Result<Type, Fault> {
        leselang_command::validate_result_binding(
            self,
            pending,
            result_binding_limits(),
            crate::valid_merge_branch_name,
        )
        .map_err(projection_fault)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Value;
    use leselang_hir::computation::{Computation, ScalarValue};
    use leselang_hir::host_call::HostOperation;
    use leselang_hir::result_field::ResultField;
    use leselang_hir::{UiFormInputKind, UiSelectionState};
    use serde_json::json;

    #[test]
    fn restored_binding_keeps_scalar_result_group_types_and_canonical_refusals() {
        let pending = Effect::UiFocus {
            node_id: "pending".into(),
        };
        let result = ProjectedResult::capture(
            HostOperation::UiFocus,
            &Value::UiFocus {
                node_id: "saved".into(),
            },
        )
        .unwrap();
        let mut binding = ResultBinding {
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
                    members: vec![ProjectedBinding {
                        name: "member".into(),
                        result,
                    }],
                },
            }],
            body: Computation::Literal {
                value: ScalarValue::None,
            },
        };
        for (body, expected) in [
            (
                Computation::Local {
                    name: "scalar".into(),
                },
                Type::Scalar(leselang_hir::computation::ScalarType::Integer),
            ),
            (
                Computation::Field {
                    value: Box::new(Computation::Local {
                        name: "reply".into(),
                    }),
                    field: ResultField::NodeId,
                },
                Type::Scalar(leselang_hir::computation::ScalarType::String),
            ),
            (
                Computation::Field {
                    value: Box::new(Computation::Local {
                        name: "saved".into(),
                    }),
                    field: ResultField::NodeId,
                },
                Type::Scalar(leselang_hir::computation::ScalarType::String),
            ),
            (
                Computation::Field {
                    value: Box::new(Computation::Member {
                        group: "group".into(),
                        name: "member".into(),
                        operation: HostOperation::UiFocus,
                    }),
                    field: ResultField::NodeId,
                },
                Type::Scalar(leselang_hir::computation::ScalarType::String),
            ),
        ] {
            binding.body = body;
            let bytes = serde_json::to_vec(&binding).unwrap();
            assert_eq!(binding.validate(&pending), Ok(expected));
            assert_eq!(serde_json::to_vec(&binding).unwrap(), bytes);
        }
        for corruption in 0..7 {
            let mut changed = binding.clone();
            match corruption {
                0 => changed.results[0].result.projection_version = 0,
                1 => changed.locals[0].name = "saved".into(),
                2 => changed.name = "saved".into(),
                3 => changed.groups[0].group.members[0].name = "bad name".into(),
                4 => {
                    changed.body = Computation::Local {
                        name: "unknown".into(),
                    }
                }
                5 => {
                    changed.body = Computation::Field {
                        value: Box::new(Computation::Member {
                            group: "group".into(),
                            name: "member".into(),
                            operation: HostOperation::UiActivate,
                        }),
                        field: ResultField::NodeId,
                    }
                }
                6 => {
                    changed.body = Computation::Field {
                        value: Box::new(Computation::Local {
                            name: "saved".into(),
                        }),
                        field: ResultField::Revision,
                    }
                }
                _ => unreachable!(),
            }
            let bytes = serde_json::to_vec(&changed).unwrap();
            assert_eq!(changed.validate(&pending), Err(invalid()));
            assert_eq!(serde_json::to_vec(&changed).unwrap(), bytes);
        }
        // Without saved results/groups, scalar values retain their lexical order.
        binding.results.clear();
        binding.groups.clear();
        binding.body = Computation::Local {
            name: "scalar".into(),
        };
        assert_eq!(
            binding.validate(&pending),
            Ok(Type::Scalar(leselang_hir::computation::ScalarType::Integer))
        );
        binding.locals.push(binding.locals[0].clone());
        assert_eq!(binding.validate(&pending), Err(invalid()));
    }

    fn capture_group(
        branches: &[leselang_hir::HirBranch],
        value: &Value,
    ) -> Result<ProjectedGroup, Fault> {
        ProjectedGroup::capture(
            branches,
            value,
            group_capture_limits(),
            crate::valid_merge_branch_name,
        )
        .map_err(projection_fault)
    }

    fn validate_group(group: &ProjectedGroup) -> Result<(), Fault> {
        group
            .validate(group_capture_limits(), crate::valid_merge_branch_name)
            .map_err(projection_fault)
    }

    fn match_group(
        group: &ProjectedGroup,
        branches: &[leselang_hir::HirBranch],
        value: &Value,
    ) -> Result<bool, Fault> {
        let Value::Structured { fields } = value else {
            panic!()
        };
        group
            .matches_members(
                branches,
                fields,
                group_capture_limits(),
                crate::valid_merge_branch_name,
            )
            .map_err(projection_fault)
    }

    fn group_input(count: usize) -> (Vec<leselang_hir::HirBranch>, Value) {
        let branches = (0..count)
            .map(|index| leselang_hir::HirBranch {
                name: format!("member_{index}"),
                effect: Effect::UiFocus {
                    node_id: format!("node-{index}"),
                },
                result_type: Type::UiFocus,
            })
            .collect();
        let fields = (0..count)
            .map(|index| crate::StructuredField {
                name: format!("member_{index}"),
                value: Box::new(Value::UiFocus {
                    node_id: format!("node-{index}"),
                }),
            })
            .collect();
        (branches, Value::Structured { fields })
    }

    #[test]
    fn group_capture_preserves_original_limits_names_and_refusal_order() {
        for count in [0, 1, 2, 63, 64, 65] {
            let (branches, value) = group_input(count);
            let result = capture_group(&branches, &value);
            if (1..64).contains(&count) {
                let group = result.unwrap();
                assert_eq!(group.members.len(), count);
                assert_eq!(validate_group(&group), Ok(()));
                assert_eq!(match_group(&group, &branches, &value), Ok(true));
            } else {
                assert_eq!(result, Err(invalid()));
            }
        }
        for name in [
            "".into(),
            "x".repeat(65),
            "bad name".into(),
            "non-ascii-\u{e9}".into(),
        ] {
            let (mut branches, mut value) = group_input(1);
            branches[0].name = name.clone();
            let Value::Structured { fields } = &mut value else {
                panic!()
            };
            fields[0].name = name;
            assert_eq!(capture_group(&branches, &value), Err(invalid()));
        }
        for name in ["x".repeat(64), "0-A_b".into()] {
            let (mut branches, mut value) = group_input(1);
            branches[0].name = name.clone();
            let Value::Structured { fields } = &mut value else {
                panic!()
            };
            fields[0].name = name;
            assert!(capture_group(&branches, &value).is_ok());
        }
        let (branches, value) = group_input(2);
        let group = capture_group(&branches, &value).unwrap();
        for mutation in 0..5 {
            let mut changed = group.clone();
            match mutation {
                0 => changed.members.clear(),
                1 => changed.members[1].name = changed.members[0].name.clone(),
                2 => changed.members[0].name.clear(),
                3 => changed.members[0].result.projection_version = 0,
                4 => changed.members[0].result.fields.clear(),
                _ => unreachable!(),
            }
            let before = changed.clone();
            assert_eq!(validate_group(&changed), Err(invalid()));
            assert_eq!(match_group(&changed, &[], &value), Err(invalid()));
            assert_eq!(changed, before);
        }
        let raw = Value::UiFocus {
            node_id: "a".into(),
        };
        assert_eq!(capture_group(&branches, &raw), Err(invalid()));
    }

    #[test]
    fn group_capture_keeps_legacy_versions_order_and_projection_only_correspondence() {
        let (mut branches, value) = group_input(2);
        let group = capture_group(&branches, &value).unwrap();
        assert_eq!(
            serde_json::to_string(&group).unwrap(),
            r#"{"members":[{"name":"member_0","result":{"operation":"ui.focus","fields":[{"field":"node_id","value":{"kind":"string","value":"node-0"}}]}},{"name":"member_1","result":{"operation":"ui.focus","fields":[{"field":"node_id","value":{"kind":"string","value":"node-1"}}]}}]}"#
        );
        for version in 1..=5 {
            let mut legacy = group.clone();
            for member in &mut legacy.members {
                member.result.projection_version = version;
            }
            let bytes = serde_json::to_vec(&legacy).unwrap();
            let restored: ProjectedGroup = serde_json::from_slice(&bytes).unwrap();
            assert_eq!(serde_json::to_vec(&restored).unwrap(), bytes);
            assert_eq!(match_group(&restored, &branches, &value), Ok(true));
        }
        let mut reversed = group.clone();
        reversed.members.reverse();
        assert_eq!(validate_group(&reversed), Ok(()));
        assert!(!reversed.matches_signature(&branches));
        assert_eq!(match_group(&reversed, &branches, &value), Ok(false));
        branches[0].effect = Effect::UiActivate {
            node_id: "node-0".into(),
        };
        assert!(!group.matches_signature(&branches));
        assert_eq!(match_group(&group, &branches, &value), Ok(false));
        branches[0].effect = Effect::UiFocus {
            node_id: "different-request".into(),
        };
        assert!(group.matches_signature(&branches));
        // Signature/capture comparisons are not original argument or raw-type admission.
        let mut compatible = value.clone();
        let Value::Structured { fields } = &mut compatible else {
            panic!()
        };
        *fields[0].value = Value::UiActivate {
            node_id: "node-0".into(),
        };
        assert_eq!(match_group(&group, &branches, &compatible), Ok(true));
        let Value::Structured { fields } = &mut compatible else {
            panic!()
        };
        *fields[0].value = Value::Scalar {
            value: ScalarValue::None,
        };
        assert_eq!(match_group(&group, &branches, &compatible), Err(invalid()));
        branches[0].effect = Effect::Sequence { steps: vec![] };
        assert_eq!(capture_group(&branches, &value), Err(invalid()));
    }

    fn frames() -> Vec<(HostOperation, Value, u32, serde_json::Value)> {
        vec![
            (
                HostOperation::UiFocus,
                Value::UiFocus {
                    node_id: "a".into(),
                },
                1,
                json!([{"field":"node_id","value":{"kind":"string","value":"a"}}]),
            ),
            (
                HostOperation::UiSetSelection,
                Value::UiSetSelection {
                    node_id: "a".into(),
                    state: UiSelectionState::Unselected,
                },
                2,
                json!([
                    {"field":"node_id","value":{"kind":"string","value":"a"}},
                    {"field":"selected","value":{"kind":"boolean","value":false}}
                ]),
            ),
            (
                HostOperation::UiSetFormValue,
                Value::UiSetFormValue {
                    node_id: "a".into(),
                    field: "name".into(),
                    value: "ready".into(),
                },
                3,
                json!([
                    {"field":"node_id","value":{"kind":"string","value":"a"}},
                    {"field":"field","value":{"kind":"string","value":"name"}},
                    {"field":"value","value":{"kind":"string","value":"ready"}}
                ]),
            ),
            (
                HostOperation::UiAssertFormFieldInputKind,
                Value::UiAssertFormFieldInputKind {
                    node_id: "a".into(),
                    field: "name".into(),
                    input_kind: UiFormInputKind::TrimmedText,
                },
                4,
                json!([
                    {"field":"node_id","value":{"kind":"string","value":"a"}},
                    {"field":"field","value":{"kind":"string","value":"name"}},
                    {"field":"kind","value":{"kind":"string","value":"trimmed_text"}}
                ]),
            ),
            (
                HostOperation::UiAssertFormFieldPlaceholder,
                Value::UiAssertFormFieldPlaceholder {
                    node_id: "a".into(),
                    field: "name".into(),
                    expected: None,
                },
                5,
                json!([
                    {"field":"node_id","value":{"kind":"string","value":"a"}},
                    {"field":"field","value":{"kind":"string","value":"name"}},
                    {"field":"optional_expected","value":{"kind":"optional_string","value":null}}
                ]),
            ),
        ]
    }

    #[test]
    fn scalar_projection_keeps_five_capture_versions_and_frozen_field_wire() {
        for (operation, value, version, fields) in frames() {
            let current = ProjectedResult::capture(operation, &value).unwrap();
            assert_eq!(current.projection_version, version);
            assert_eq!(serde_json::to_value(&current.fields).unwrap(), fields);
            for saved_version in 1..=5 {
                let frame =
                    ProjectedResult::capture_version(operation, &value, saved_version).unwrap();
                let wire = serde_json::to_value(&frame).unwrap();
                assert_eq!(
                    wire.get("projection_version"),
                    (saved_version != 1).then_some(&json!(saved_version))
                );
                let restored: ProjectedResult = serde_json::from_value(wire.clone()).unwrap();
                assert_eq!(serde_json::to_value(&restored).unwrap(), wire);
                assert!(restored.matches_capture(operation, &value).unwrap());
                assert!(
                    !restored
                        .matches_capture(HostOperation::RuntimeList, &value)
                        .unwrap()
                );
                for field in ResultField::ALL {
                    let vocabularies: [&[ResultField]; 5] = [
                        &ResultField::V1,
                        &ResultField::V2,
                        &ResultField::V3,
                        &ResultField::V4,
                        &ResultField::V5,
                    ];
                    let expected = vocabularies[saved_version as usize - 1].contains(&field)
                        && field.result_type(operation.result_type()).is_some();
                    assert_eq!(restored.field(field).is_ok(), expected);
                }
            }
        }
    }

    #[test]
    fn scalar_projection_keeps_invalid_frame_fault_and_capture_correspondence() {
        for (operation, value, _, _) in frames() {
            let frame = ProjectedResult::capture(operation, &value).unwrap();
            for mutation in 0..4 {
                let mut changed = frame.clone();
                match mutation {
                    0 => changed.projection_version = 6,
                    1 => {
                        changed.fields.remove(0);
                    }
                    2 => changed.fields.push(changed.fields[0].clone()),
                    3 => changed.fields[0].value = ScalarValue::Integer(7),
                    _ => unreachable!(),
                }
                assert_eq!(changed.validate().map_err(projection_fault), Err(invalid()));
                assert_eq!(
                    changed
                        .matches_capture(operation, &value)
                        .map_err(projection_fault),
                    Err(invalid())
                );
            }
            let mut changed = frame.clone();
            changed.fields[0].value = ScalarValue::String("another-node".into());
            assert!(changed.validate().is_ok());
            assert!(!changed.matches_capture(operation, &value).unwrap());
            assert_eq!(
                leselang_command::ResultView::Raw {
                    value: &value,
                    operation
                }
                .project_scalar(ResultField::Revision),
                Err(leselang_command::ResultViewError::InvalidProjection)
            );
            assert_eq!(
                ProjectedResult::capture_version(operation, &value, 0).map_err(projection_fault),
                Err(invalid())
            );
        }
    }

    #[test]
    fn capture_validation_precedes_correspondence_and_rejects_semantic_forgery() {
        for (operation, value, version, _) in frames() {
            let original = ProjectedResult::capture(operation, &value).unwrap();
            for mutation in 0..6 {
                let mut changed = original.clone();
                match mutation {
                    0 => changed.projection_version = 0,
                    1 => changed.fields[0].field = ResultField::Revision,
                    2 => changed.fields[0].value = ScalarValue::String("x".repeat(4097)),
                    3 => changed.fields.insert(0, changed.fields[0].clone()),
                    4 => changed.fields.clear(),
                    5 if version == 4 => {
                        changed.fields.last_mut().unwrap().value =
                            ScalarValue::String("not_a_kind".into())
                    }
                    5 if version == 5 => {
                        changed.fields.last_mut().unwrap().value = ScalarValue::OptionalString(
                            leselang_hir::computation::OptionalStringValue(Some(
                                "bad\0text".into(),
                            )),
                        )
                    }
                    _ => continue,
                }
                let before = changed.clone();
                assert_eq!(changed.validate().map_err(|_| invalid()), Err(invalid()));
                // A different operation must not hide an invalid saved frame.
                assert_eq!(
                    changed
                        .matches_capture(HostOperation::RuntimeList, &value)
                        .map_err(|_| invalid()),
                    Err(invalid())
                );
                assert_eq!(changed, before);
            }
            let mut wire = serde_json::to_value(&original).unwrap();
            wire["unexpected"] = json!(true);
            assert!(serde_json::from_value::<ProjectedResult>(wire).is_err());
            let mut wire = serde_json::to_value(&original).unwrap();
            wire["fields"][0]["unexpected"] = json!(true);
            assert!(serde_json::from_value::<ProjectedResult>(wire).is_err());
        }
    }
}
