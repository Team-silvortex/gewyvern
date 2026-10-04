use leselang_hir::computation::Computation;
use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_syntax::parse;
use leselang_vm::{
    EffectResult, PresentationResult, Step, Vm, decode_continuation, encode_continuation,
};

fn start(vm: &mut Vm, source: &str) -> Step {
    vm.start(
        &lower(&parse(&format!("fn main() = {source}"))).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation", "runtime.read"]),
        None,
    )
}

#[test]
fn optional_filters_keep_identical_request_budget_and_wire_when_source_names_move() {
    for sources in [
        [
            r#"runtime.list(role: concat(left: "worker", right: "-a"), environment: none)"#,
            r#"runtime.list(environment: none, role: concat(left: "worker", right: "-a"))"#,
        ],
        [
            r#"runtime.list(role: concat(left: "worker", right: "-a"), cluster: none)"#,
            r#"runtime.list(cluster: none, role: concat(left: "worker", right: "-a"))"#,
        ],
    ] {
        let requests = sources.map(|source| {
            let Step::Effect(request) = start(&mut Vm::new(100), source) else {
                panic!("expected effect")
            };
            *request
        });
        assert_eq!(requests[0], requests[1]);
        let wire = encode_continuation(&requests[0].continuation).unwrap();
        assert_eq!(
            wire,
            encode_continuation(&requests[1].continuation).unwrap()
        );
        assert_eq!(
            decode_continuation(&wire).unwrap(),
            requests[0].continuation
        );
    }
}

#[test]
fn restored_computed_arguments_keep_saved_fuel_and_reject_reordered_residuals() {
    let Step::Effect(request) = start(
        &mut Vm::new(200),
        r#"bind(saved: ui.focus(node_id: "target"),
        body: ui.assert_text(expected: field(value: saved, name: "node_id"),
            node_id: concat(left: "tar", right: "get")))"#,
    ) else {
        panic!("expected effect")
    };
    let mut request = *request;
    let wire = encode_continuation(&request.continuation).unwrap();
    let image = decode_continuation(&wire).unwrap();
    let mut restored = Vm::new(0);
    restored.restore_request(request.clone()).unwrap();
    let result = EffectResult::Presentation(PresentationResult::Focus {
        node_id: "target".into(),
    });
    let Step::Effect(next) = restored.resume(&image, result.clone()) else {
        panic!("expected next effect")
    };
    assert!(next.budget.fuel_remaining < image.fuel_remaining);
    assert_eq!(restored.resume(&image, result), Step::Effect(next));

    let binding = request.continuation.result_binding.as_mut().unwrap();
    let Computation::Call { arguments, .. } = &mut binding.body else {
        panic!("expected residual call")
    };
    arguments.reverse();
    let wire = serde_json::to_vec(&request.continuation).unwrap();
    assert!(decode_continuation(&wire).is_err());
    assert!(Vm::new(0).restore_request(request).is_err());
}
