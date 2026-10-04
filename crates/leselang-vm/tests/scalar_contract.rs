use leselang_hir::{Effect, lower};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_syntax::parse;
use leselang_vm::{
    EffectResult, PresentationResult, Step, Vm, decode_continuation, encode_continuation,
};

fn start(vm: &mut Vm, source: &str) -> Step {
    vm.start(
        &lower(&parse(&format!("fn main() = {source}"))).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
    )
}

#[test]
fn computed_strings_still_need_the_receiving_host_domain_before_effect_admission() {
    for source in [
        r#"ui.focus(node_id: concat(left: "bad", right: " node"))"#.to_owned(),
        format!(
            r#"ui.focus(node_id: concat(left: "{}", right: "x"))"#,
            "a".repeat(128)
        ),
        format!(
            r#"ui.assert_text(node_id: "target", expected: concat(left: "{}", right: "x"))"#,
            "a".repeat(1024)
        ),
    ] {
        let mut vm = Vm::new(10_000);
        let Step::Fault(fault) = start(&mut vm, &source) else {
            panic!("expected domain rejection")
        };
        assert_eq!(fault.code, "LSV1404");
        assert!(!fault.message.contains("bad node"));
        assert_eq!(vm.pending_count(), 0);
        let Step::Effect(request) = start(&mut vm, r#"ui.focus(node_id: "target")"#) else {
            panic!("expected a fresh effect")
        };
        assert_eq!(request.effect_id, "effect-1");
    }
}

#[test]
fn typed_optional_preparation_keeps_absence_empty_wire_and_saved_fuel_on_restore() {
    for (value, expected) in [("none", None), (r#""""#, Some(String::new()))] {
        let source = format!(
            r#"ui.assert_form_field_placeholder(node_id: "target", field: "caption", expected: optional_string(value: {value}))"#
        );
        let Step::Effect(request) = start(&mut Vm::new(100), &source) else {
            panic!("expected an optional-text request")
        };
        assert_eq!(
            request.continuation.pending_effect,
            Effect::UiAssertFormFieldPlaceholder {
                node_id: "target".into(),
                field: "caption".into(),
                expected: expected.clone(),
            }
        );
        let bytes = encode_continuation(&request.continuation).unwrap();
        let restored = decode_continuation(&bytes).unwrap();
        assert_eq!(encode_continuation(&restored).unwrap(), bytes);
        assert_eq!(restored.fuel_remaining, request.budget.fuel_remaining);
        let mut vm = Vm::new(0);
        vm.restore_request(*request).unwrap();
        let receipt = EffectResult::Presentation(PresentationResult::AssertFormFieldPlaceholder {
            node_id: "target".into(),
            field: "caption".into(),
            expected,
        });
        let step = vm.resume(&restored, receipt.clone());
        assert!(matches!(step, Step::Done(_)));
        assert_eq!(vm.resume(&restored, receipt), step);
    }
}
