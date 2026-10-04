use leselang_hir::{
    Effect, Type, authorize, host_call::HostOperation, lower, result_field::ResultField,
};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::ScalarValue;
use leselang_syntax::parse;
use leselang_vm::{
    EffectResult, PresentationResult, Step, Value, Vm, decode_continuation, encode_continuation,
};

type SharedReferenceIr = leselang_hir::ir::Computation<ResultField, HostOperation, Effect, Type>;

#[test]
fn shared_ir_residual_body_keeps_saved_budget_wire_and_first_receipt_replay() {
    let program = lower(&parse(
        r#"fn main() = bind(saved: ui.focus(node_id: "target"),
        body: concat(left: "arrived:", right: field(value: saved, name: "node_id")))"#,
    ))
    .unwrap();
    let Step::Effect(request) = Vm::new(200).start(
        &program,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
    ) else {
        panic!("expected effect")
    };
    let binding = request.continuation.result_binding.as_ref().unwrap();
    let shared: &SharedReferenceIr = &binding.body;
    let legacy: &leselang_hir::computation::Computation = shared;
    assert!(std::ptr::eq(shared, legacy));
    assert!(shared.is_pure());
    shared.validate_structure().unwrap();
    assert_eq!(request.continuation.schema_version, 2);
    assert_eq!(
        serde_json::to_string(shared).unwrap(),
        r#"{"kind":"binary","operator":"concat","left":{"kind":"literal","value":{"kind":"string","value":"arrived:"}},"right":{"kind":"field","value":{"kind":"local","name":"saved"},"field":"node_id"}}"#
    );
    let wire = encode_continuation(&request.continuation).unwrap();
    let image = decode_continuation(&wire).unwrap();
    assert_eq!(image, request.continuation);
    assert_eq!(encode_continuation(&image).unwrap(), wire);
    let remaining = image.fuel_remaining;
    let mut restored = Vm::new(0);
    restored.restore_request((*request).clone()).unwrap();
    assert_eq!(
        restored.pending_continuations()[0].fuel_remaining,
        remaining
    );
    let result = EffectResult::Presentation(PresentationResult::Focus {
        node_id: "target".into(),
    });
    let outcome = restored.resume(&image, result);
    assert_eq!(
        outcome,
        Step::Done(Value::Scalar {
            value: ScalarValue::String("arrived:target".into())
        })
    );
    assert_eq!(restored.pending_count(), 0);
    assert_eq!(
        restored.resume(
            &image,
            EffectResult::Presentation(PresentationResult::Focus {
                node_id: "changed".into(),
            })
        ),
        outcome
    );
}

#[test]
fn invalid_cold_field_in_shared_ir_is_rejected_before_any_effect_admission() {
    let mut program = lower(&parse(
        r#"fn main() = bind(saved: ui.focus(node_id: "target"),
        body: choose(when: true, then: "ready", otherwise: field(value: saved, name: "node_id")))"#,
    ))
    .unwrap();
    let Effect::Compute { expression } = &mut program.function.effect else {
        panic!("expected computation")
    };
    let shared: &mut SharedReferenceIr = expression;
    let SharedReferenceIr::Bind { body, .. } = shared else {
        panic!("expected binding")
    };
    let SharedReferenceIr::Choose { otherwise, .. } = body.as_mut() else {
        panic!("expected choice")
    };
    let SharedReferenceIr::Field { field, .. } = otherwise.as_mut() else {
        panic!("expected field")
    };
    *field = ResultField::Revision;
    let capabilities = CapabilitySet::new(["ui.presentation"]);
    assert_eq!(
        authorize(&program, &capabilities).unwrap_err().code,
        "LSH1405"
    );
    let mut vm = Vm::new(200);
    assert!(matches!(
        vm.start(
            &program,
            Principal::new("operator").unwrap(),
            capabilities,
            None
        ),
        Step::Fault(_)
    ));
    assert_eq!(vm.pending_count(), 0);
}
