use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_hir::{Effect, Type, lower};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_syntax::parse;
use leselang_vm::{DispatchLease, EffectResult, PresentationResult, Step, Vm};

pub fn exercise(mut vm: Vm, reopen: impl Fn(Vm) -> Vm) {
    let effect = Effect::UiFocus {
        node_id: "a".into(),
    };
    let input = |sequence, fuel_remaining| ReservedEffectInput {
        effect: &effect,
        result_type: Type::UiFocus,
        sequence,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: None,
        deadline_ms: 30_000,
        deadline_at_ms: Some(30_001),
        fuel_remaining,
    };
    let constructed = build_effect_request(input(9000, 21)).unwrap();
    let fake = DispatchLease {
        request: constructed,
        attempt: 1,
        retry_count: 0,
        lease_expires_at_ms: 101,
    };
    let reply = || {
        EffectResult::Presentation(PresentationResult::Focus {
            node_id: "a".into(),
        })
    };
    assert!(
        matches!(vm.acknowledge_effect(&fake, 1, reply()), Step::Fault(f) if f.code == "LSV2004")
    );
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.completed_count(), 0);
    assert!(vm.claim_effect(1, 100).unwrap().is_none());
    vm = reopen(vm);
    let Step::Effect(request) = vm.start_timed(
        &lower(&parse(r#"fn main() = ui.focus(node_id: "a")"#)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
        1,
        30_000,
    ) else {
        panic!("expected admitted request")
    };
    assert_eq!(request.continuation.token.as_str(), "continuation-1");
    let rebuilt = build_effect_request(input(1, request.continuation.fuel_remaining + 1)).unwrap();
    assert_eq!(rebuilt, *request);
    let lease = vm.claim_effect(1, 100).unwrap().unwrap();
    let mut forged = lease.clone();
    forged.request = rebuilt;
    forged.attempt += 1;
    vm = reopen(vm);
    assert!(
        matches!(vm.acknowledge_effect(&forged, 2, reply()), Step::Fault(f) if f.code == "LSV4022")
    );
    assert_eq!(vm.pending_count(), 1);
    assert_eq!(vm.completed_count(), 0);
    assert!(matches!(
        vm.acknowledge_effect(&lease, 2, reply()),
        Step::Done(_)
    ));
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.completed_count(), 1);
}
