use std::cell::Cell;
use std::error::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};

use leselang_command::{
    ProjectedBinding, ProjectedGroup, ProjectedGroupExt, ProjectedResult, ResultView,
    ResultViewError as ViewError, StructuredField, Value,
};
use leselang_hir::{Effect, HirBranch, host_call::HostOperation, result_field::ResultField};
use leselang_runtime_core::{GroupCaptureLimits, ScalarValue};

fn limits() -> GroupCaptureLimits {
    GroupCaptureLimits {
        min_members: 1,
        max_members: 2,
    }
}

fn fixture() -> (Vec<HirBranch>, Value, ProjectedGroup) {
    let branches = ["a", "b"]
        .into_iter()
        .map(|name| HirBranch {
            name: name.into(),
            effect: Effect::UiFocus {
                node_id: name.into(),
            },
            result_type: HostOperation::UiFocus.result_type(),
        })
        .collect::<Vec<_>>();
    let value = Value::Structured {
        fields: ["a", "b"]
            .into_iter()
            .map(|name| StructuredField {
                name: name.into(),
                value: Box::new(Value::UiFocus {
                    node_id: name.into(),
                }),
            })
            .collect(),
    };
    let group = ProjectedGroup::capture(&branches, &value, limits(), |_| true).unwrap();
    (branches, value, group)
}

#[test]
fn views_borrow_original_members_and_copy_only_at_explicit_projection_boundaries() {
    let (branches, value, group) = fixture();
    let Value::Structured { fields } = &value else {
        panic!()
    };
    let raw = ResultView::Group {
        value: &value,
        branches: &branches,
    };
    let saved = ResultView::ProjectedGroup(&group);
    for (index, name) in ["a", "b"].into_iter().enumerate() {
        let selected = raw.select_member(name, HostOperation::UiFocus).unwrap();
        let ResultView::Raw {
            value: borrowed,
            operation,
        } = selected
        else {
            panic!()
        };
        assert!(std::ptr::eq(borrowed, &*fields[index].value));
        assert_eq!(operation, HostOperation::UiFocus);
        let projected = saved.select_member(name, operation).unwrap();
        let ResultView::Projected(borrowed) = projected else {
            panic!()
        };
        assert!(std::ptr::eq(borrowed, &group.members[index].result));
        assert_eq!(
            selected.project_scalar(ResultField::NodeId),
            projected.project_scalar(ResultField::NodeId)
        );
        let copied = projected.capture_result().unwrap();
        assert_eq!(&copied, borrowed);
        assert_ne!(copied.fields.as_ptr(), borrowed.fields.as_ptr());
        assert_eq!(selected.capture_result().unwrap(), copied);
    }
    let copied = raw.capture_group(limits(), |_| true).unwrap();
    assert_eq!(copied, group);
    assert_ne!(copied.members.as_ptr(), group.members.as_ptr());
    for view in [raw, saved] {
        assert!(matches!(
            view.select_member("a", HostOperation::UiActivate),
            Err(ViewError::InvalidAccess)
        ));
        assert!(matches!(
            view.select_member("missing", HostOperation::UiFocus),
            Err(ViewError::InvalidAccess)
        ));
        assert_eq!(
            view.project_scalar(ResultField::NodeId),
            Err(ViewError::InvalidAccess)
        );
        assert_eq!(view.capture_result(), Err(ViewError::InvalidAccess));
    }
}

#[test]
fn views_preserve_saved_bytes_and_first_match_without_claiming_admission() {
    let (mut branches, mut value, mut group) = fixture();
    group.members[0].result.projection_version = 0;
    group.members.insert(1, group.members[0].clone());
    assert!(group.validate(limits(), |_| true).is_err());
    let before = serde_json::to_vec(&group).unwrap();
    let selected = ResultView::ProjectedGroup(&group)
        .select_member("a", HostOperation::UiFocus)
        .unwrap();
    let ResultView::Projected(borrowed) = selected else {
        panic!()
    };
    assert!(std::ptr::eq(borrowed, &group.members[0].result));
    assert_eq!(
        selected.project_scalar(ResultField::NodeId),
        Ok(ScalarValue::String("a".into()))
    );
    assert_eq!(selected.capture_result().unwrap().projection_version, 0);
    let copied = ResultView::ProjectedGroup(&group)
        .capture_group(
            GroupCaptureLimits {
                min_members: 5,
                max_members: 0,
            },
            |_| panic!("saved copying must not invoke raw name policy"),
        )
        .unwrap();
    assert_eq!(serde_json::to_vec(&copied).unwrap(), before);
    assert_eq!(serde_json::to_vec(&group).unwrap(), before);

    branches[0].result_type = HostOperation::UiActivate.result_type();
    branches[0].effect = Effect::UiFocus {
        node_id: "different-request".into(),
    };
    let Value::Structured { fields } = &mut value else {
        panic!()
    };
    fields.insert(
        1,
        StructuredField {
            name: "a".into(),
            value: Box::new(Value::UiFocus {
                node_id: "second".into(),
            }),
        },
    );
    let view = ResultView::Group {
        value: &value,
        branches: &branches,
    };
    assert_eq!(
        view.select_member("a", HostOperation::UiFocus)
            .unwrap()
            .project_scalar(ResultField::NodeId),
        Ok(ScalarValue::String("a".into()))
    );
    assert_eq!(
        view.capture_group(limits(), |_| true),
        Err(ViewError::InvalidProjection)
    );
    let unrelated = Value::UiActivate {
        node_id: "same-field".into(),
    };
    let raw = ResultView::Raw {
        value: &unrelated,
        operation: HostOperation::UiFocus,
    };
    assert_eq!(
        raw.project_scalar(ResultField::NodeId),
        Ok(ScalarValue::String("same-field".into()))
    );
    assert_eq!(
        raw.capture_result().unwrap().operation,
        HostOperation::UiFocus
    );
}

#[test]
fn raw_capture_policy_keeps_projection_before_names_and_propagates_native_unwind() {
    let (branches, value, _) = fixture();
    let view = ResultView::Group {
        value: &value,
        branches: &branches,
    };
    let mut calls = Vec::new();
    assert_eq!(
        view.capture_group(limits(), |name| {
            calls.push(name.to_owned());
            name != "b"
        }),
        Err(ViewError::InvalidProjection)
    );
    assert_eq!(calls, ["a", "b"]);
    calls.clear();
    assert_eq!(
        view.capture_group(
            GroupCaptureLimits {
                min_members: 1,
                max_members: 1
            },
            |name| {
                calls.push(name.to_owned());
                true
            }
        ),
        Err(ViewError::InvalidProjection)
    );
    assert!(calls.is_empty());
    let mut bad = value.clone();
    let Value::Structured { fields } = &mut bad else {
        panic!()
    };
    *fields[1].value = Value::Scalar {
        value: ScalarValue::None,
    };
    assert_eq!(
        ResultView::Group {
            value: &bad,
            branches: &branches
        }
        .capture_group(limits(), |_| panic!("projection must fail before names")),
        Err(ViewError::InvalidProjection)
    );
    let before = serde_json::to_vec(&(&branches, &value)).unwrap();
    let visits = Cell::new(0);
    assert!(
        catch_unwind(AssertUnwindSafe(|| view.capture_group(limits(), |_| {
            visits.set(visits.get() + 1);
            if visits.get() == 2 {
                panic!("native policy unwind")
            }
            true
        })))
        .is_err()
    );
    assert_eq!(visits.get(), 2);
    assert_eq!(serde_json::to_vec(&(&branches, &value)).unwrap(), before);
}

#[test]
fn diagnostics_are_payload_free_and_legacy_fields_are_never_filled_from_raw_catalogs() {
    let raw = Value::UiSetSelection {
        node_id: "private-node".into(),
        state: leselang_hir::UiSelectionState::Selected,
    };
    let projected =
        ProjectedResult::capture_version(HostOperation::UiSetSelection, &raw, 1).unwrap();
    let group = ProjectedGroup {
        members: vec![ProjectedBinding {
            name: "private-member".into(),
            result: projected.clone(),
        }],
    };
    let view = ResultView::Projected(&projected);
    assert_eq!(
        view.project_scalar(ResultField::Selected),
        Err(ViewError::InvalidProjection)
    );
    assert_eq!(view.capture_result().unwrap(), projected);
    assert!(matches!(
        view.select_member("private-member", HostOperation::UiSetSelection),
        Err(ViewError::InvalidAccess)
    ));
    assert_eq!(
        view.capture_group(limits(), |_| panic!("not a group")),
        Err(ViewError::InvalidAccess)
    );
    assert_eq!(format!("{view:?}"), "ProjectedResultView");
    assert_eq!(
        format!("{:?}", ResultView::ProjectedGroup(&group)),
        "ProjectedGroupView"
    );
    assert_eq!(
        format!(
            "{:?}",
            ResultView::Raw {
                value: &raw,
                operation: HostOperation::UiSetSelection
            }
        ),
        "RawResultView"
    );
    assert_eq!(
        format!(
            "{:?}",
            ResultView::Group {
                value: &raw,
                branches: &[]
            }
        ),
        "RawGroupView"
    );
    for error in [ViewError::InvalidAccess, ViewError::InvalidProjection] {
        assert!(error.source().is_none());
        assert!(!format!("{error} {error:?}").contains("private"));
    }
}
