use super::*;
use crate::ProjectedGroupExt;

fn projection_fault(_: crate::InvalidProjectedResult) -> Fault {
    invalid_projection()
}
use leserpent_hir::result_field::ResultField;

fn frozen_field(view: ResultView<'_>, field: ResultField) -> Result<ScalarValue, Fault> {
    match view {
        ResultView::Raw { value, .. } => value.project_scalar(field).ok_or_else(invalid_projection),
        ResultView::Projected(result) => result.field(field).map_err(projection_fault),
        ResultView::Group { .. } | ResultView::ProjectedGroup(_) => Err(invalid()),
    }
}

fn frozen_snapshot(view: ResultView<'_>) -> Result<ProjectedResult, Fault> {
    match view {
        ResultView::Raw { value, operation } => {
            ProjectedResult::capture(operation, value).map_err(projection_fault)
        }
        ResultView::Projected(result) => Ok(result.clone()),
        ResultView::Group { .. } | ResultView::ProjectedGroup(_) => Err(invalid()),
    }
}

fn frozen_group(view: ResultView<'_>) -> Result<ProjectedGroup, Fault> {
    match view {
        ResultView::Group { value, branches } => {
            ProjectedGroup::capture(branches, value, group_capture_limits(), valid_group_name)
                .map_err(projection_fault)
        }
        ResultView::ProjectedGroup(group) => Ok(group.clone()),
        _ => Err(invalid()),
    }
}

fn frozen_member<'a>(
    group: ResultView<'a>,
    name: &str,
    operation: HostOperation,
) -> Result<ResultView<'a>, Fault> {
    match group {
        ResultView::Group {
            value: Value::Structured { fields },
            branches,
        } => {
            if !branches.iter().any(|branch| {
                branch.name == name && HostOperation::for_effect(&branch.effect) == Some(operation)
            }) {
                return Err(invalid());
            }
            let field = fields
                .iter()
                .find(|field| field.name == name)
                .ok_or_else(invalid)?;
            Ok(ResultView::Raw {
                value: &field.value,
                operation,
            })
        }
        ResultView::ProjectedGroup(saved) => {
            let member = saved
                .members
                .iter()
                .find(|member| member.name == name && member.result.operation == operation)
                .ok_or_else(invalid)?;
            Ok(ResultView::Projected(&member.result))
        }
        _ => Err(invalid()),
    }
}

fn identity(view: ResultView<'_>) -> (usize, HostOperation, bool) {
    match view {
        ResultView::Raw { value, operation } => {
            (std::ptr::from_ref(value).addr(), operation, false)
        }
        ResultView::Projected(result) => {
            (std::ptr::from_ref(result).addr(), result.operation, true)
        }
        _ => panic!("member selection returned a group"),
    }
}

#[test]
fn result_views_preserve_frozen_projection_selection_copy_and_fault_boundaries() {
    let raw = Value::UiFocus {
        node_id: "node".into(),
    };
    let scalar = Value::Scalar {
        value: ScalarValue::None,
    };
    let saved = ProjectedResult::capture(HostOperation::UiFocus, &raw).unwrap();
    let mut malformed = saved.clone();
    malformed.projection_version = 0;
    let branches = [HirBranch {
        name: "member".into(),
        result_type: leserpent_hir::Type::UiActivate,
        effect: Effect::UiFocus {
            node_id: "different-argument".into(),
        },
    }];
    let rows = Value::Structured {
        fields: vec![
            crate::StructuredField {
                name: "member".into(),
                value: Box::new(raw.clone()),
            },
            crate::StructuredField {
                name: "member".into(),
                value: Box::new(scalar.clone()),
            },
        ],
    };
    let group = ProjectedGroup {
        members: vec![
            crate::ProjectedBinding {
                name: "member".into(),
                result: malformed.clone(),
            },
            crate::ProjectedBinding {
                name: "member".into(),
                result: saved.clone(),
            },
        ],
    };
    let empty = ProjectedGroup { members: vec![] };
    let environment = result_environment();
    let before =
        serde_json::to_vec(&(&raw, &scalar, &saved, &malformed, &branches, &rows, &group)).unwrap();
    for view in [
        ResultView::Raw {
            value: &raw,
            operation: HostOperation::UiFocus,
        },
        ResultView::Raw {
            value: &raw,
            operation: HostOperation::UiActivate,
        },
        ResultView::Raw {
            value: &scalar,
            operation: HostOperation::UiFocus,
        },
        ResultView::Projected(&saved),
        ResultView::Projected(&malformed),
        ResultView::Group {
            value: &rows,
            branches: &branches,
        },
        ResultView::Group {
            value: &rows,
            branches: &[],
        },
        ResultView::Group {
            value: &scalar,
            branches: &branches,
        },
        ResultView::ProjectedGroup(&group),
        ResultView::ProjectedGroup(&empty),
    ] {
        for field in ResultField::ALL {
            assert_eq!(view.field(field), frozen_field(view, field));
        }
        assert_eq!(view.snapshot(), frozen_snapshot(view));
        assert_eq!(view.snapshot_group(), frozen_group(view));
        for name in ["member", "missing"] {
            for operation in [HostOperation::UiFocus, HostOperation::UiActivate] {
                assert_eq!(
                    environment.member(&view, name, &operation).map(identity),
                    frozen_member(view, name, operation).map(identity),
                );
            }
        }
    }
    assert_eq!(
        serde_json::to_vec(&(&raw, &scalar, &saved, &malformed, &branches, &rows, &group)).unwrap(),
        before
    );
}
