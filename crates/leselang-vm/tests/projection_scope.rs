use leselang_hir::computation::Computation;
use leselang_hir::result_field::ResultField;
use leselang_hir::{Effect, lower};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_syntax::parse;
use leselang_vm::{
    EffectRequest, EffectResult, PresentationResult, ScalarValue, Step, Value, Vm,
    decode_continuation, encode_continuation,
};

fn program(body: &str) -> leselang_hir::HirProgram {
    lower(&parse(&format!(
        r#"fn main() = bind(seed: ui.assert_text(node_id: "a", expected: "ready"), body:
            bind(current: ui.focus(node_id: "b"), body: {body}))"#
    )))
    .unwrap()
}

fn body(source: &str) -> Computation {
    let Effect::Compute { expression } = program(source).function.effect else {
        panic!()
    };
    let Computation::Bind { body, .. } = *expression else {
        panic!()
    };
    let Computation::Bind { body, .. } = *body else {
        panic!()
    };
    *body
}

fn legacy_request() -> EffectRequest {
    let mut vm = Vm::new(1000);
    let Step::Effect(first) = vm.start(
        &program(r#""safe""#),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
    ) else {
        panic!()
    };
    let Step::Effect(second) = vm.resume(
        &first.continuation,
        EffectResult::Presentation(PresentationResult::AssertText {
            node_id: "a".into(),
            expected: "ready".into(),
        }),
    ) else {
        panic!()
    };
    let mut request = *second;
    let frame = &mut request
        .continuation
        .result_binding
        .as_mut()
        .unwrap()
        .results[0]
        .result;
    frame.projection_version = 1;
    frame
        .fields
        .retain(|field| ResultField::V1.contains(&field.field));
    request
}

fn rejected(request: &EffectRequest) {
    let image = &request.continuation;
    assert_eq!(encode_continuation(image).unwrap_err().code, "LSV1405");
    let bytes = serde_json::to_vec(image).unwrap();
    assert_eq!(decode_continuation(&bytes).unwrap_err().code, "LSV1405");
    let mut vm = Vm::new(1000);
    assert_eq!(
        vm.restore_request(request.clone()).unwrap_err().code,
        "LSV1405"
    );
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn closed_alias_scopes_keep_legacy_wire_and_reentry_without_field_widening() {
    let mut request = legacy_request();
    request.continuation.result_binding.as_mut().unwrap().body = body(
        r#"choose(when: true,
        then: bind(alias: seed, body: field(value: alias, name: "node_id")),
        otherwise: bind(alias: current, body: field(value: alias, name: "node_id")))"#,
    );
    let bytes = encode_continuation(&request.continuation).unwrap();
    let restored = decode_continuation(&bytes).unwrap();
    assert_eq!(encode_continuation(&restored).unwrap(), bytes);
    assert_eq!(
        restored.result_binding.as_ref().unwrap().results[0]
            .result
            .projection_version,
        1
    );
    let mut vm = Vm::new(1);
    request.continuation = restored.clone();
    vm.restore_request(request).unwrap();
    assert_eq!(
        vm.resume(
            &restored,
            EffectResult::Presentation(PresentationResult::Focus {
                node_id: "b".into(),
            })
        ),
        Step::Done(Value::Scalar {
            value: ScalarValue::String("a".into())
        })
    );
}

#[test]
fn missing_legacy_fields_remain_rejected_in_cold_bind_loop_and_fold_scopes() {
    for source in [
        r#"choose(when: true, then: bind(alias: seed, body: field(value: alias, name: "node_id")), otherwise: bind(alias: seed, body: field(value: alias, name: "expected")))"#,
        r#"loop(tmp: "", while: false, next: bind(alias: seed, body: field(value: alias, name: "expected")), limit: 0)"#,
        r#"fold(tmp: "", items: strings(), item: "entry", next: concat(left: tmp, right: field(value: seed, name: "expected")), limit: 0)"#,
    ] {
        let mut request = legacy_request();
        request.continuation.result_binding.as_mut().unwrap().body = body(source);
        rejected(&request);
    }
}

#[test]
fn restored_scalar_result_and_pending_names_cannot_collide() {
    for corruption in ["result", "scalar", "pending"] {
        let mut request = legacy_request();
        let binding = request.continuation.result_binding.as_mut().unwrap();
        match corruption {
            "result" => binding.results.push(binding.results[0].clone()),
            "scalar" => binding.locals.push(leselang_vm::ScalarBinding {
                name: "seed".into(),
                value: ScalarValue::String("private".into()),
            }),
            "pending" => binding.name = "seed".into(),
            _ => unreachable!(),
        }
        rejected(&request);
    }
}
