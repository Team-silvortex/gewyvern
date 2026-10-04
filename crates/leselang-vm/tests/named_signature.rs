use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_syntax::parse;
use leselang_vm::{Fault, Step, Vm, decode_continuation, encode_continuation};

fn start(vm: &mut Vm, source: &str) -> Step {
    vm.start(
        &lower(&parse(&format!("fn main() = {source}"))).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
    )
}

#[test]
fn host_values_evaluate_in_signature_order_not_submitted_name_order() {
    for source in [
        r#"ui.assert_text(expected: to_string(value: parse_boolean(value: "expected-secret")), node_id: to_string(value: parse_integer(value: "node-secret")))"#,
        r#"ui.assert_text(node_id: to_string(value: parse_integer(value: "node-secret")), expected: to_string(value: parse_boolean(value: "expected-secret")))"#,
    ] {
        let mut vm = Vm::new(100);
        let step = start(&mut vm, source);
        assert_eq!(
            step,
            Step::Fault(Fault {
                code: "LSV1408".into(),
                message: "invalid integer text: expected ASCII decimal within u64".into(),
            })
        );
        assert!(!serde_json::to_string(&step).unwrap().contains("secret"));
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 0);
        let Step::Effect(request) = start(&mut vm, r#"ui.focus(node_id: "target")"#) else {
            panic!("expected a fresh effect")
        };
        assert_eq!(request.effect_id, "effect-1");
    }
}

#[test]
fn reordered_names_keep_identical_resolved_requests_fuel_and_continuation_bytes() {
    let sources = [
        r#"ui.assert_text(expected: concat(left: "re", right: "ady"), node_id: concat(left: "tar", right: "get"))"#,
        r#"ui.assert_text(node_id: concat(left: "tar", right: "get"), expected: concat(left: "re", right: "ady"))"#,
    ];
    let requests = sources.map(|source| {
        let Step::Effect(request) = start(&mut Vm::new(100), source) else {
            panic!("expected a resolved request")
        };
        *request
    });
    assert_eq!(requests[0], requests[1]);
    let bytes = encode_continuation(&requests[0].continuation).unwrap();
    assert_eq!(
        bytes,
        encode_continuation(&requests[1].continuation).unwrap()
    );
    assert_eq!(
        decode_continuation(&bytes).unwrap(),
        requests[0].continuation
    );
}
