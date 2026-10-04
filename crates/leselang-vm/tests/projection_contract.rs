use leselang_hir::computation::OptionalStringValue;
use leselang_hir::lower;
use leselang_hir::result_field::ResultField;
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_syntax::parse;
use leselang_vm::{
    EffectOperation, EffectRequest, EffectResult, ProjectedField, ProjectedResult, ScalarValue,
    Step, Value, Vm, decode_continuation, encode_continuation,
};

fn effect(step: Step) -> EffectRequest {
    let Step::Effect(request) = step else {
        panic!("expected effect: {step:?}")
    };
    *request
}

fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}

fn captured(operation: &str) -> EffectRequest {
    let program = lower(&parse(&format!(
        r#"fn main() = bind(seed: {operation}, body:
            bind(current: ui.focus(node_id: "b"), body: "safe"))"#
    )))
    .unwrap();
    let mut vm = Vm::new(1000);
    let first = effect(vm.start(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
    ));
    effect(vm.resume(&first.continuation, receipt(&first)))
}

fn projection(request: &mut EffectRequest) -> &mut ProjectedResult {
    &mut request
        .continuation
        .result_binding
        .as_mut()
        .unwrap()
        .results[0]
        .result
}

fn rejected(request: EffectRequest) {
    assert_eq!(
        encode_continuation(&request.continuation).unwrap_err().code,
        "LSV1405"
    );
    let bytes = serde_json::to_vec(&request.continuation).unwrap();
    assert_eq!(decode_continuation(&bytes).unwrap_err().code, "LSV1405");
    let mut vm = Vm::new(1000);
    assert_eq!(vm.restore_request(request).unwrap_err().code, "LSV1405");
    assert_eq!(vm.pending_count(), 0);
}

const FORM: &str = r#"ui.set_form_value(node_id: "form", field: "name", value: "ready")"#;

#[test]
fn closed_projection_versions_keep_exact_field_order_wire_and_saved_fuel() {
    for operation in [
        FORM,
        r#"ui.assert_form_field_required(node_id: "form", field: "name", state: "required")"#,
        r#"ui.assert_node_kind(node_id: "a", kind: "heading")"#,
        r#"ui.assert_form_field_placeholder(node_id: "form", field: "name", expected: none)"#,
    ] {
        let original = captured(operation);
        for (version, vocabulary) in [
            (1, ResultField::V1.as_slice()),
            (2, ResultField::V2.as_slice()),
            (3, ResultField::V3.as_slice()),
            (4, ResultField::V4.as_slice()),
            (5, ResultField::V5.as_slice()),
        ] {
            let mut request = original.clone();
            let frame = projection(&mut request);
            frame.projection_version = version;
            frame
                .fields
                .retain(|field| vocabulary.contains(&field.field));
            let expected: Vec<_> = vocabulary
                .iter()
                .copied()
                .filter(|field| field.result_type(frame.operation.result_type()).is_some())
                .collect();
            assert_eq!(
                frame
                    .fields
                    .iter()
                    .map(|field| field.field)
                    .collect::<Vec<_>>(),
                expected
            );
            let frame_wire = serde_json::to_value(&*frame).unwrap();
            assert_eq!(frame_wire.get("projection_version").is_some(), version != 1);
            let bytes = encode_continuation(&request.continuation).unwrap();
            let restored = decode_continuation(&bytes).unwrap();
            assert_eq!(encode_continuation(&restored).unwrap(), bytes);
            assert_eq!(
                restored.fuel_remaining,
                original.continuation.fuel_remaining
            );
            request.continuation = restored.clone();
            let result = receipt(&request);
            let mut vm = Vm::new(1);
            vm.restore_request(request).unwrap();
            assert_eq!(
                vm.resume(&restored, result),
                Step::Done(Value::Scalar {
                    value: ScalarValue::String("safe".into())
                })
            );
        }
    }
}

#[test]
fn missing_extra_reordered_duplicate_and_wrong_type_fields_fail_before_admission() {
    for corruption in [
        "missing",
        "extra",
        "reordered",
        "duplicate",
        "type",
        "bytes",
    ] {
        let mut request = captured(FORM);
        let frame = projection(&mut request);
        match corruption {
            "missing" => {
                frame.fields.pop();
            }
            "extra" => frame.fields.push(ProjectedField {
                field: ResultField::Count,
                value: ScalarValue::Integer(1),
            }),
            "reordered" => frame.fields.swap(0, 1),
            "duplicate" => frame.fields[1].field = frame.fields[0].field,
            "type" => frame.fields[0].value = ScalarValue::Integer(1),
            "bytes" => frame.fields[0].value = ScalarValue::String("x".repeat(4097)),
            _ => unreachable!(),
        }
        rejected(request);
    }
    for version in [0, 6, u32::MAX] {
        let mut request = captured(FORM);
        projection(&mut request).projection_version = version;
        rejected(request);
    }
}

#[test]
fn bounded_scalar_shape_does_not_bypass_operation_specific_kind_or_text_domains() {
    let mut request = captured(r#"ui.assert_node_kind(node_id: "a", kind: "heading")"#);
    projection(&mut request)
        .fields
        .iter_mut()
        .find(|field| field.field == ResultField::Kind)
        .unwrap()
        .value = ScalarValue::String("runtime_refresh".into());
    rejected(request);

    let mut request = captured(
        r#"ui.assert_form_field_placeholder(node_id: "form", field: "name", expected: none)"#,
    );
    projection(&mut request)
        .fields
        .iter_mut()
        .find(|field| field.field == ResultField::OptionalExpected)
        .unwrap()
        .value = ScalarValue::OptionalString(OptionalStringValue(Some(
        "x".repeat(leselang_hir::MAX_UI_EXPECTED_TEXT_BYTES + 1),
    )));
    rejected(request);
}

#[test]
fn projected_field_legacy_wire_has_no_extra_keys_or_value_normalization() {
    let field = ProjectedField {
        field: ResultField::NodeId,
        value: ScalarValue::String("a".into()),
    };
    let wire = r#"{"field":"node_id","value":{"kind":"string","value":"a"}}"#;
    assert_eq!(serde_json::to_string(&field).unwrap(), wire);
    assert_eq!(serde_json::from_str::<ProjectedField>(wire).unwrap(), field);
    for invalid in [
        r#"{"field":"node_id","value":{"kind":"string","value":"a"},"extra":true}"#,
        r#"{"field":"node_id","field":"count","value":{"kind":"string","value":"a"}}"#,
        r#"{"field":"node_id"}"#,
        r#"{"field":"unknown","value":{"kind":"string","value":"a"}}"#,
    ] {
        assert!(serde_json::from_str::<ProjectedField>(invalid).is_err());
    }
}

#[test]
fn legacy_projected_field_is_the_core_specialization_not_a_parallel_dto() {
    let text = String::from("owned buffer");
    let pointer = text.as_ptr();
    let shared = leselang_runtime_core::ScalarProjectionField {
        field: ResultField::NodeId,
        value: ScalarValue::String(text),
    };
    let legacy: ProjectedField = shared;
    let returned: leselang_runtime_core::ScalarProjectionField<ResultField> = legacy;
    assert_eq!(returned.value.text().unwrap().as_ptr(), pointer);
}
