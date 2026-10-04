use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_syntax::parse;
use leselang_vm::{
    EffectResult, Fault, PresentationResult, ScalarValue, Step, Value, Vm, decode_continuation,
    encode_continuation,
};

fn start(vm: &mut Vm, source: &str) -> Step {
    vm.start(
        &lower(&parse(&format!("fn main() = {source}"))).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
    )
}

fn done(value: ScalarValue) -> Step {
    Step::Done(Value::Scalar { value })
}

#[test]
fn arithmetic_and_both_parse_errors_keep_exact_fault_json_and_fallback_fuel() {
    for (expression, fallback, expected, code, message, required_fuel) in [
        (
            "div(left: 1, right: 0)",
            "7",
            ScalarValue::Integer(7),
            "LSV1401",
            "integer overflow, underflow, or division by zero",
            5,
        ),
        (
            r#"parse_integer(value: "secret")"#,
            "8",
            ScalarValue::Integer(8),
            "LSV1408",
            "invalid integer text: expected ASCII decimal within u64",
            6,
        ),
        (
            r#"parse_boolean(value: "secret")"#,
            "true",
            ScalarValue::Boolean(true),
            "LSV1408",
            "invalid boolean text: expected true or false",
            6,
        ),
    ] {
        let failure = start(&mut Vm::new(100), expression);
        assert_eq!(
            failure,
            Step::Fault(Fault {
                code: code.into(),
                message: message.into()
            })
        );
        assert_eq!(
            serde_json::to_value(&failure).unwrap(),
            serde_json::json!({
                "kind": "fault", "payload": { "code": code, "message": message }
            })
        );
        assert!(!serde_json::to_string(&failure).unwrap().contains("secret"));
        let recovered = format!("recover(value: {expression}, fallback: {fallback})");
        for fuel in 0..required_fuel {
            assert!(matches!(start(&mut Vm::new(fuel), &recovered),
                Step::Fault(fault) if fault.code == "LSV1001"));
        }
        let mut vm = Vm::new(required_fuel);
        assert_eq!(start(&mut vm, &recovered), done(expected));
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn string_list_loop_and_fold_resource_faults_never_select_successful_fallbacks() {
    let text_overflow = format!(
        r#"recover(value: concat(left: "{}", right: "x"), fallback: "safe")"#,
        "x".repeat(4096)
    );
    let list_overflow = format!(
        r#"recover(value: split(left: "{}", right: ","), fallback: strings())"#,
        ",".repeat(64)
    );
    for (source, code) in [
        (text_overflow.as_str(), "LSV1403"),
        (list_overflow.as_str(), "LSV1403"),
        (
            "recover(value: loop(n: 0, while: true, next: n, limit: 0), fallback: 7)",
            "LSV1406",
        ),
        (
            r#"recover(value: fold(n: 0, items: strings(a: "x"), item: "entry", next: n, limit: 0), fallback: 7)"#,
            "LSV1406",
        ),
    ] {
        let mut vm = Vm::new(10_000);
        assert!(matches!(start(&mut vm, source), Step::Fault(fault) if fault.code == code));
        assert_eq!(vm.pending_count(), 0);
    }
}

#[test]
fn failed_fallback_is_caught_only_by_an_explicit_outer_recovery_after_scope_cleanup() {
    let body = r#"bind(base: 2, body:
        recover(value:
            recover(value: bind(tmp: 1, body: div(left: tmp, right: 0)),
                fallback: bind(tmp: "bad", body: parse_integer(value: tmp))),
            fallback: bind(tmp: 5, body: add(left: base, right: tmp))))"#;
    assert_eq!(
        start(&mut Vm::new(100), body),
        done(ScalarValue::Integer(7))
    );
    let lazy = r#"recover(value: strings(a: "safe"),
        fallback: strings(a: to_string(value: parse_integer(value: "secret"))))"#;
    assert!(matches!(start(&mut Vm::new(100), lazy), Step::Done(_)));
}

#[test]
fn restored_calculation_recovery_preserves_wire_and_original_remaining_fuel() {
    let source = r#"bind(r: ui.focus(node_id: "bad"), body:
        recover(value: parse_integer(value: field(value: r, name: "node_id")), fallback: 7))"#;
    let Step::Effect(request) = start(&mut Vm::new(100), source) else {
        panic!()
    };
    let bytes = encode_continuation(&request.continuation).unwrap();
    let restored = decode_continuation(&bytes).unwrap();
    assert_eq!(encode_continuation(&restored).unwrap(), bytes);
    assert_eq!(restored.schema_version, 2);
    let receipt = EffectResult::Presentation(PresentationResult::Focus {
        node_id: "bad".into(),
    });
    let mut request = *request;
    request.continuation = restored.clone();
    let mut vm = Vm::new(1);
    vm.restore_request(request.clone()).unwrap();
    let expected = done(ScalarValue::Integer(7));
    assert_eq!(vm.resume(&restored, receipt.clone()), expected);
    assert_eq!(vm.resume(&restored, receipt.clone()), expected);

    request.continuation.fuel_remaining = 1;
    request.budget.fuel_remaining = 1;
    let image = request.continuation.clone();
    let mut vm = Vm::new(1_000_000);
    vm.restore_request(request).unwrap();
    assert!(matches!(vm.resume(&image, receipt), Step::Fault(fault) if fault.code == "LSV1001"));
}
