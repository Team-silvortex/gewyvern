use std::{cell::Cell, rc::Rc};

use leselang_runtime_core::{
    CapturedBindings, MAX_SCALAR_STRING_BYTES, OptionalStringValue, ProjectedBinding,
    ProjectedGroup, ProjectedGroupBinding, ScalarBinding, ScalarValue, StringListValue,
};
use serde::{Deserialize, Serialize};
use serde_json::json;

// None of these schema types implements Default.
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Body {
    next: u32,
}
#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct ResultData {
    position: u64,
}
type Frame = CapturedBindings<Body, ResultData, ProjectedGroup<ResultData>>;

fn empty() -> Frame {
    CapturedBindings {
        name: "reply".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body: Body { next: 7 },
    }
}

#[test]
fn empty_optional_arrays_keep_legacy_wire_without_native_default_bounds() {
    let frame = empty();
    let wire = r#"{"name":"reply","locals":[],"body":{"next":7}}"#;
    assert_eq!(serde_json::to_string(&frame).unwrap(), wire);
    assert_eq!(serde_json::from_str::<Frame>(wire).unwrap(), frame);
    let explicit = r#"{"name":"reply","locals":[],"results":[],"groups":[],"body":{"next":7}}"#;
    assert_eq!(serde_json::from_str::<Frame>(explicit).unwrap(), frame);
}

#[test]
fn populated_arrays_preserve_names_member_order_duplicates_and_original_schema_data() {
    let mut frame = empty();
    frame.results = vec![ProjectedBinding {
        name: "first".into(),
        result: ResultData { position: 42 },
    }];
    frame.groups = vec![ProjectedGroupBinding {
        name: "batch".into(),
        group: ProjectedGroup {
            members: vec![
                ProjectedBinding {
                    name: "same".into(),
                    result: ResultData { position: 9 },
                },
                ProjectedBinding {
                    name: "same".into(),
                    result: ResultData { position: 3 },
                },
            ],
        },
    }];
    let wire = r#"{"name":"reply","locals":[],"results":[{"name":"first","result":{"position":42}}],"groups":[{"name":"batch","group":{"members":[{"name":"same","result":{"position":9}},{"name":"same","result":{"position":3}}]}}],"body":{"next":7}}"#;
    assert_eq!(serde_json::to_string(&frame).unwrap(), wire);
    assert_eq!(serde_json::from_str::<Frame>(wire).unwrap(), frame);
}

#[test]
fn second_unrelated_schema_uses_opaque_body_result_and_group_without_core_interpretation() {
    #[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
    struct TextBody(String);
    #[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
    struct Visibility(bool);
    type GuiFrame = CapturedBindings<TextBody, Visibility, Vec<String>>;
    let frame = GuiFrame {
        name: "visible".into(),
        locals: vec![],
        results: vec![ProjectedBinding {
            name: "target".into(),
            result: Visibility(true),
        }],
        groups: vec![ProjectedGroupBinding {
            name: "windows".into(),
            group: vec!["editor".into(), "preview".into()],
        }],
        body: TextBody("return original visibility".into()),
    };
    let wire = serde_json::to_string(&frame).unwrap();
    assert_eq!(serde_json::from_str::<GuiFrame>(&wire).unwrap(), frame);
    assert!(serde_json::from_str::<Frame>(&wire).is_err());
}

#[test]
fn captured_scalar_data_retains_all_six_tagged_payloads_and_absence_distinctions() {
    for value in [
        ScalarValue::Integer(u64::MAX),
        ScalarValue::Boolean(false),
        ScalarValue::String("".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::OptionalString(OptionalStringValue(Some("".into()))),
        ScalarValue::StringList(StringListValue(vec!["z".into(), "".into(), "z".into()])),
    ] {
        let mut frame = empty();
        frame.locals.push(ScalarBinding {
            name: "original".into(),
            value,
        });
        let wire = serde_json::to_vec(&frame).unwrap();
        assert_eq!(serde_json::from_slice::<Frame>(&wire).unwrap(), frame);
    }
}

#[test]
fn closed_containers_reject_unknown_duplicate_missing_and_null_required_fields() {
    for wire in [
        r#"{"name":"reply","locals":[],"body":{"next":7},"extra":true}"#,
        r#"{"name":"reply","name":"other","locals":[],"body":{"next":7}}"#,
        r#"{"name":"reply","body":{"next":7}}"#,
        r#"{"name":"reply","locals":[]}"#,
        r#"{"name":"reply","locals":null,"body":{"next":7}}"#,
        r#"{"name":"reply","locals":[],"results":null,"body":{"next":7}}"#,
        r#"{"name":"reply","locals":[],"groups":null,"body":{"next":7}}"#,
    ] {
        assert!(serde_json::from_str::<Frame>(wire).is_err(), "{wire}");
    }
    assert!(
        serde_json::from_value::<ScalarBinding>(json!({
            "name": "x", "value": {"kind": "none"}, "extra": true
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<ProjectedBinding<ResultData>>(json!({
            "name": "x", "result": {"position": 1}, "extra": true
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<ProjectedGroupBinding<ProjectedGroup<ResultData>>>(json!({
            "name": "x", "group": {"members": []}, "extra": true
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<ProjectedGroup<ResultData>>(json!({
            "members": [], "extra": true
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<ProjectedGroup<ResultData>>(json!({
            "members": [{"name": "x", "result": {"position": 1}, "extra": true}]
        }))
        .is_err()
    );
    assert!(
        serde_json::from_value::<ProjectedBinding<ResultData>>(json!({
            "name": "x"
        }))
        .is_err()
    );
    assert!(serde_json::from_value::<ProjectedGroup<ResultData>>(json!({})).is_err());
}

#[test]
fn direct_and_plain_text_decoded_data_are_not_validation_or_authority_certificates() {
    let mut frame = empty();
    frame.name = "not a validated name".repeat(100);
    frame.locals = vec![ScalarBinding {
        name: frame.name.clone(),
        value: ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1)),
    }];
    let wire = serde_json::to_vec(&frame).unwrap();
    let decoded: Frame = serde_json::from_slice(&wire).unwrap();
    assert_eq!(decoded, frame);
    assert!(!decoded.locals[0].value.is_bounded());
    let oversized_optional = json!({
        "name": "x", "value": {"kind": "optional_string", "value": "x".repeat(MAX_SCALAR_STRING_BYTES + 1)}
    });
    assert!(serde_json::from_value::<ScalarBinding>(oversized_optional).is_err());
}

#[test]
fn move_only_thread_local_native_payloads_and_text_buffers_keep_original_ownership() {
    struct Native {
        drops: Rc<Cell<usize>>,
    }
    impl Drop for Native {
        fn drop(&mut self) {
            self.drops.set(self.drops.get() + 1);
        }
    }
    let drops = Rc::new(Cell::new(0));
    let native = || Native {
        drops: Rc::clone(&drops),
    };
    let name = String::from("original name buffer");
    let name_pointer = name.as_ptr();
    let text = String::from("original value buffer");
    let text_pointer = text.as_ptr();
    let frame = CapturedBindings {
        name,
        locals: vec![ScalarBinding {
            name: "text".into(),
            value: ScalarValue::String(text),
        }],
        results: vec![ProjectedBinding {
            name: "result".into(),
            result: native(),
        }],
        groups: vec![ProjectedGroupBinding {
            name: "group".into(),
            group: ProjectedGroup {
                members: vec![ProjectedBinding {
                    name: "member".into(),
                    result: native(),
                }],
            },
        }],
        body: native(),
    };
    let moved = frame;
    assert_eq!(moved.name.as_ptr(), name_pointer);
    assert_eq!(moved.locals[0].value.text().unwrap().as_ptr(), text_pointer);
    assert_eq!(drops.get(), 0);
    drop(moved);
    assert_eq!(drops.get(), 3);
    assert_eq!(Rc::strong_count(&drops), 1);
}

#[test]
fn send_ownership_is_conditional_and_moving_a_frame_does_not_reexecute_its_body() {
    struct NativeBody(Box<u64>);
    let body = NativeBody(Box::new(42));
    let address = std::ptr::from_ref(body.0.as_ref()) as usize;
    let frame: CapturedBindings<NativeBody, (), ()> = CapturedBindings {
        name: "reply".into(),
        locals: vec![],
        results: vec![],
        groups: vec![],
        body,
    };
    std::thread::spawn(move || {
        assert_eq!(std::ptr::from_ref(frame.body.0.as_ref()) as usize, address);
        assert_eq!(*frame.body.0, 42);
    })
    .join()
    .unwrap();
}
