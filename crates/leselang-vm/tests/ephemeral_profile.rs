//! Reference-product VM coverage that needs no database or filesystem fixture.
use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    CancellationReason, DispatchClaim, EffectError, EffectErrorClass, EffectOperation,
    EffectRequest, EffectResult, PresentationOperation, PresentationResult, RetentionPolicy,
    RetryDisposition, RetryPolicy, ScalarValue, SchedulerLimits, Step, Value, Vm,
    decode_continuation, encode_continuation,
};

#[test]
fn released_host_protocol_paths_are_exact_product_adapter_types() {
    use std::any::TypeId;

    macro_rules! same_type {
        ($($name:ident),+ $(,)?) => {
            $(assert_eq!(TypeId::of::<leselang_vm::$name>(), TypeId::of::<leselang_command::$name>());)+
        };
    }
    same_type!(
        EffectOperation,
        EffectResult,
        PresentationEnvelope,
        PresentationOperation,
        PresentationResult,
        DebuggerCancelResult,
        Value,
        StructuredField,
    );

    let mut vm = Vm::new(1000);
    let first = effect(start(
        &mut vm,
        r#"fn main() = bind(r: ui.focus(node_id: "a"),
        body: ui.focus(node_id: field(value: r, name: "node_id")))"#,
    ));
    let wire = serde_json::to_vec(&first.operation).unwrap();
    let from_adapter: leselang_command::EffectOperation = serde_json::from_slice(&wire).unwrap();
    assert_eq!(from_adapter, first.operation);
    assert_eq!(serde_json::to_vec(&from_adapter).unwrap(), wire);
    let reply = || {
        leselang_command::EffectResult::Presentation(leselang_command::PresentationResult::Focus {
            node_id: "a".into(),
        })
    };
    let second = effect(vm.resume(&first.continuation, reply()));
    assert_eq!(second.operation, first.operation);
    assert!(second.budget.fuel_remaining < first.budget.fuel_remaining);
    let done = vm.resume(&second.continuation, reply());
    assert_eq!(
        done,
        Step::Done(Value::UiFocus {
            node_id: "a".into()
        })
    );
    assert_eq!(vm.resume(&first.continuation, reply()), done);
    assert_eq!(vm.pending_count(), 0);
}

fn start(vm: &mut Vm, source: &str) -> Step {
    vm.start(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
    )
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
fn node(request: &EffectRequest) -> &str {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    let PresentationOperation::Focus { node_id } = &envelope.operation else {
        panic!()
    };
    node_id
}
fn scalar(value: ScalarValue) -> Step {
    Step::Done(Value::Scalar { value })
}

#[test]
fn memory_profile_executes_typed_helpers_and_preserves_legacy_scalar_wire() {
    let mut vm = Vm::new(100);
    let output = start(
        &mut vm,
        "fn main() = describe(n: 41)\nfn describe(n: integer) = concat(left: \"answer=\", right: to_string(value: add(left: n, right: 1)))",
    );
    assert_eq!(output, scalar(ScalarValue::String("answer=42".into())));
    assert_eq!(
        serde_json::to_string(&output).unwrap(),
        r#"{"kind":"done","payload":{"kind":"scalar","value":{"kind":"string","value":"answer=42"}}}"#
    );
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.scheduler_pressure(0).unwrap().pending_dispatches, 0);
}

#[test]
fn memory_profile_seq_keeps_reply_gates_one_budget_and_once_only_completion() {
    let mut vm = Vm::new(100);
    let first = effect(start(
        &mut vm,
        r#"fn main() = seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#,
    ));
    assert_eq!(node(&first), "a");
    let late = vm
        .pending_continuations()
        .into_iter()
        .find(|image| image.token != first.continuation.token)
        .unwrap();
    assert!(matches!(vm.resume(&late, focus("b")), Step::Fault(fault) if fault.code == "LSV2410"));
    let lease = vm.claim_effect(1, 20).unwrap().unwrap();
    assert_eq!(lease.request, first);
    assert!(vm.claim_effect(2, 20).unwrap().is_none());
    let second = effect(vm.acknowledge_effect(&lease, 2, focus("a")));
    assert_eq!(node(&second), "b");
    assert_eq!(
        second.budget.fuel_remaining + 1,
        first.budget.fuel_remaining
    );
    assert_eq!(
        effect(vm.resume(&first.continuation, focus("ignored"))),
        second
    );
    let done = vm.resume(&second.continuation, focus("b"));
    let Step::Done(Value::Structured { fields }) = &done else {
        panic!("{done:?}")
    };
    assert_eq!(
        fields.iter().map(|f| f.name.as_str()).collect::<Vec<_>>(),
        ["first", "second"]
    );
    assert_eq!(vm.resume(&first.continuation, focus("ignored")), done);
    assert_eq!(vm.resume(&second.continuation, focus("ignored")), done);
    assert_eq!(vm.pending_count(), 0);
    assert!(vm.claim_effect(3, 20).unwrap().is_none());
}

#[test]
fn memory_profile_all_reenters_captured_successor_only_after_complete_original_barrier() {
    let source = r#"fn main() = bind(g: all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b")),
      body: bind(r: ui.focus(node_id: field(value: member(value: g, name: "b"), name: "node_id")),
      body: eq(left: field(value: member(value: g, name: "a"), name: "node_id"), right: field(value: r, name: "node_id"))))"#;
    for reverse in [false, true] {
        let mut vm = Vm::new(500);
        let Step::Effects(batch) = start(&mut vm, source) else {
            panic!()
        };
        let requests: Vec<_> = batch.branches.into_iter().map(|b| b.request).collect();
        assert_eq!(requests.len(), 2);
        let owner = requests[0].continuation.group_result.clone().unwrap();
        let first = usize::from(reverse);
        let last = 1 - first;
        assert!(
            matches!(vm.resume(&requests[first].continuation, focus(node(&requests[first]))), Step::Waiting(wait) if wait.completed_branches == 1)
        );
        assert_eq!(vm.merge_result(&owner).unwrap(), None);
        let tail = effect(vm.resume(&requests[last].continuation, focus(node(&requests[last]))));
        assert_eq!(node(&tail), "b");
        assert!(tail.budget.fuel_remaining < requests[0].budget.fuel_remaining);
        assert_eq!(tail.continuation.expected_revision, Some(Revision(7)));
        assert_eq!(tail.continuation.group_result.as_ref(), Some(&owner));
        assert!(tail.continuation.result_binding.is_some());
        let expected = scalar(ScalarValue::Boolean(false));
        assert_eq!(vm.resume(&tail.continuation, focus("b")), expected);
        for request in requests.iter().chain(std::iter::once(&tail)) {
            assert_eq!(vm.resume(&request.continuation, focus("ignored")), expected);
        }
        assert_eq!(vm.pending_count(), 0);
        assert!(vm.claim_effect(1, 20).unwrap().is_none());
    }
}

#[test]
fn memory_profile_fences_forged_images_wrong_reply_types_and_independent_local_roots() {
    let mut left = Vm::new(100);
    let mut right = Vm::new(100);
    let a = effect(start(&mut left, r#"fn main() = ui.focus(node_id: "a")"#));
    let b = effect(start(&mut right, r#"fn main() = ui.focus(node_id: "b")"#));
    // Local namespaces can reuse token text; hosts still own cross-VM routing identity.
    assert_eq!(a.continuation.token, b.continuation.token);
    assert!(
        matches!(left.resume(&b.continuation, focus("b")), Step::Fault(fault) if fault.code == "LSV2005")
    );
    let mut forged = a.continuation.clone();
    forged.fuel_remaining += 1;
    assert!(
        matches!(left.resume(&forged, focus("a")), Step::Fault(fault) if fault.code == "LSV2005")
    );
    let malformed = effect(start(
        &mut left,
        r#"fn main() = ui.focus(node_id: "malformed")"#,
    ));
    let terminal = left.resume(
        &malformed.continuation,
        EffectResult::Presentation(PresentationResult::AssertVisible {
            node_id: "malformed".into(),
        }),
    );
    assert!(matches!(&terminal, Step::Fault(_)));
    assert_eq!(
        left.resume(&malformed.continuation, focus("malformed")),
        terminal
    );
    assert_eq!(left.pending_count(), 1);
    let decoded = decode_continuation(&encode_continuation(&a.continuation).unwrap()).unwrap();
    assert_eq!(decoded, a.continuation);
    assert!(matches!(left.resume(&decoded, focus("a")), Step::Done(_)));
    assert_eq!(right.pending_count(), 1);
    assert!(matches!(
        right.resume(&b.continuation, focus("b")),
        Step::Done(_)
    ));
    let mut fresh = Vm::new(100);
    assert!(
        matches!(fresh.resume(&decoded, focus("a")), Step::Fault(fault) if fault.code == "LSV2004")
    );
}

#[test]
fn memory_profile_cancel_or_deadline_seals_captured_tail_and_replays_terminal_receipt() {
    for deadline in [false, true] {
        let mut vm = Vm::new(100);
        let program = lower(&parse(
            r#"fn main() = bind(r: ui.focus(node_id: "a"), body: ui.focus(node_id: "b"))"#,
        ))
        .unwrap();
        let first = effect(vm.start_timed(
            &program,
            Principal::new("operator").unwrap(),
            CapabilitySet::new(["ui.presentation"]),
            None,
            100,
            10,
        ));
        let lease = vm.claim_effect(101, 5).unwrap().unwrap();
        let terminal = if deadline {
            assert!(vm.claim_effect(110, 5).unwrap().is_none());
            vm.resume_at(&first.continuation, 110, focus("a"))
        } else {
            vm.cancel_effect(&first.continuation, 102)
        };
        assert!(
            matches!(&terminal, Step::Cancelled(c) if c.reason == if deadline { CancellationReason::DeadlineExceeded } else { CancellationReason::Requested })
        );
        assert_eq!(vm.resume_at(&first.continuation, 111, focus("a")), terminal);
        assert_eq!(vm.acknowledge_effect(&lease, 111, focus("a")), terminal);
        assert_eq!(vm.pending_count(), 0);
        assert!(vm.claim_effect(111, 5).unwrap().is_none());
    }
}

#[test]
fn memory_profile_retry_backpressure_and_compaction_keep_original_dispatch_authority() {
    let mut vm = Vm::new_with_limits(
        100,
        SchedulerLimits {
            max_pending_dispatches: 2,
            max_active_leases: 1,
        },
    )
    .unwrap();
    let first = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#));
    let second = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "b")"#));
    assert!(
        matches!(start(&mut vm, r#"fn main() = ui.focus(node_id: "c")"#), Step::Fault(fault) if fault.code == "LSV2501")
    );
    let lease = vm.claim_effect(1, 5).unwrap().unwrap();
    assert_eq!(lease.request, first);
    assert!(matches!(
        vm.try_claim_effect(2, 5).unwrap(),
        DispatchClaim::Backpressured(_)
    ));
    let policy = RetryPolicy {
        max_retries: 1,
        base_delay_ms: 5,
        max_delay_ms: 5,
    };
    let error = EffectError {
        class: EffectErrorClass::Transient,
        code: "host_busy".into(),
        message: "temporary".into(),
    };
    let RetryDisposition::Scheduled(schedule) =
        vm.report_effect_error(&lease, 2, error, &policy).unwrap()
    else {
        panic!()
    };
    assert_eq!(schedule.ready_at_ms, 7);
    let other = vm.claim_effect(3, 5).unwrap().unwrap();
    assert_eq!(other.request, second);
    assert!(matches!(
        vm.acknowledge_effect(&other, 4, focus("b")),
        Step::Done(_)
    ));
    assert!(vm.claim_effect(6, 5).unwrap().is_none());
    let redelivery = vm.claim_effect(7, 5).unwrap().unwrap();
    assert_eq!(redelivery.request, first);
    assert_eq!(redelivery.retry_count, 1);
    assert_eq!(redelivery.attempt, 2);
    assert!(matches!(
        vm.acknowledge_effect(&lease, 8, focus("a")),
        Step::Fault(_)
    ));
    assert!(matches!(
        vm.acknowledge_effect(&redelivery, 8, focus("a")),
        Step::Done(_)
    ));
    assert_eq!(vm.pending_count(), 0);
    let report = vm
        .compact_journal(&RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 1,
        })
        .unwrap();
    assert_eq!(report.removed_records, 1);
    assert_eq!(vm.completed_count(), 1);
}
