use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    DispatchLease, EffectError, EffectErrorClass, EffectRequest, EffectResult, PresentationResult,
    RetryDisposition, RetryPolicy, RetrySchedule, Step, Value, Vm,
};

pub fn exercise(mut vm: Vm, reopen: impl Fn(Vm) -> Vm) {
    use leselang_command::reference_dispatch::{
        lease_expiration, next_attempt, validate_observed_lease, validate_proposed_retry,
    };
    use leselang_runtime_core::DispatchLeaseObservation;

    let request = effect(vm.start_timed(
        &lower(&parse(r#"fn main() = ui.focus(node_id: "a")"#)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        1,
        30_000,
    ));
    let original = vm.claim_effect(1, 10).unwrap().unwrap();
    let mut forged = original.clone();
    forged.attempt = next_attempt(original.attempt).unwrap();
    forged.lease_expires_at_ms = lease_expiration(11, 10).unwrap();
    let observation = |lease: &DispatchLease, ready_at_ms| DispatchLeaseObservation {
        same_request: true,
        active: true,
        attempt: lease.attempt,
        retry_count: lease.retry_count,
        ready_at_ms,
        expires_at_ms: Some(lease.lease_expires_at_ms),
    };
    let error = EffectError {
        class: EffectErrorClass::Transient,
        code: "host_busy".into(),
        message: "temporary".into(),
    };
    let policy = RetryPolicy {
        max_retries: 1,
        base_delay_ms: 5,
        max_delay_ms: 5,
    };
    assert_eq!(
        validate_observed_lease(&forged, 2, observation(&forged, 1)),
        Ok(())
    );
    assert!(
        matches!(vm.acknowledge_effect(&forged, 2, focus("a")), Step::Fault(f) if f.code == "LSV4022")
    );
    assert_eq!(
        vm.report_effect_error(&forged, 2, error.clone(), &policy)
            .unwrap_err()
            .code,
        "LSV4022"
    );
    assert_eq!(
        vm.pending_continuations(),
        vec![request.continuation.clone()]
    );
    assert_eq!(vm.completed_count(), 0);
    vm = reopen(vm);

    // Equal-expiry acknowledgement is valid only until a replacement commits.
    let current = vm.claim_effect(11, 10).unwrap().unwrap();
    assert_eq!(current.attempt, 2);
    assert_eq!(
        validate_observed_lease(&original, 11, observation(&original, 1)),
        Ok(())
    );
    assert!(
        matches!(vm.acknowledge_effect(&original, 11, focus("a")), Step::Fault(f) if f.code == "LSV4022")
    );
    assert_eq!(
        vm.report_effect_error(&original, 11, error.clone(), &policy)
            .unwrap_err()
            .code,
        "LSV4022"
    );
    let proposal = RetrySchedule {
        retry_count: 1,
        ready_at_ms: 17,
        error: error.clone(),
    };
    assert_eq!(validate_proposed_retry(0, &proposal, 12), Ok(()));
    let scheduled = vm
        .report_effect_error(&current, 12, error.clone(), &policy)
        .unwrap();
    assert_eq!(scheduled, RetryDisposition::Scheduled(proposal));
    vm = reopen(vm);
    assert!(vm.claim_effect(16, 10).unwrap().is_none());
    assert!(
        matches!(vm.acknowledge_effect(&current, 16, focus("a")), Step::Fault(f) if f.code == "LSV4022")
    );
    assert_eq!(
        vm.report_effect_error(&current, 16, error.clone(), &policy)
            .unwrap_err()
            .code,
        "LSV4022"
    );
    let retry = vm.claim_effect(17, 10).unwrap().unwrap();
    assert_eq!((retry.attempt, retry.retry_count), (3, 1));
    assert_eq!(retry.request, request);

    // Invalid issuance fails before deadline cleanup, despite the supplied late clock.
    let late = request.continuation.deadline_at_ms.unwrap() + 1;
    assert_eq!(vm.claim_effect(late, 0).unwrap_err().code, "LSV4015");
    assert_eq!(
        vm.pending_continuations(),
        vec![request.continuation.clone()]
    );
    let done = Step::Done(Value::UiFocus {
        node_id: "a".into(),
    });
    assert_eq!(vm.acknowledge_effect(&retry, 18, focus("a")), done);
    vm = reopen(vm);
    assert_eq!(
        vm.acknowledge_effect(&original, 19, focus("different")),
        done
    );
    assert_eq!(
        vm.report_effect_error(&original, 19, error, &policy),
        Ok(RetryDisposition::Terminal(done))
    );
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.completed_count(), 1);
}

fn effect(step: Step) -> EffectRequest {
    let Step::Effect(request) = step else {
        panic!("expected effect: {step:?}")
    };
    *request
}

fn focus(node: &str) -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: node.into(),
    })
}
