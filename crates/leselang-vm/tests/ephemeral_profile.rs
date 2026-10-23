//! Reference-product VM coverage that needs no database or filesystem fixture.
use leselang_command::{CancellationReason, EffectRequest};
use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    DispatchClaim, DispatchLease, EffectError, EffectErrorClass, EffectOperation, EffectResult,
    FailedEffect, PresentationOperation, PresentationResult, RetentionPolicy, RetryDisposition,
    RetryPolicy, RetrySchedule, ScalarValue, SchedulerLimits, Step, Value, Vm, decode_continuation,
    encode_continuation,
};

#[path = "support/reference_dispatch_authority.rs"]
mod reference_dispatch_authority;

#[test]
fn inspected_group_cannot_publish_a_receipt_or_replace_live_member_authority() {
    use leselang_command::reference_group_batch::{
        AdmittedMergeGroup, ReferenceGroupBatchProofs, inspect_group_batch,
    };
    use leselang_command::reference_group_recovery::{ReferenceGroupProofs, inspect_group_record};
    use leselang_runtime_core::Fault;
    struct PermissiveProofs;
    impl ReferenceGroupProofs for PermissiveProofs {
        fn validate_chain(&mut self) -> Result<(), Fault> {
            Ok(())
        }
        fn validate_blocked_dispatches(&mut self, _: usize) -> Result<(), Fault> {
            Ok(())
        }
        fn validate_unbound_receipt(&mut self, _: &Step) -> Result<(), Fault> {
            Ok(())
        }
        fn validate_records(&mut self, _: Option<&Step>) -> Result<(), Fault> {
            Ok(())
        }
    }
    impl<'a> ReferenceGroupBatchProofs<&'a [u8]> for PermissiveProofs {
        fn token_conflicts(
            &mut self,
            _: &leselang_command::ContinuationToken,
        ) -> Result<bool, Fault> {
            Ok(false)
        }
        fn inspect_group(&mut self, _: AdmittedMergeGroup<&'a [u8]>) -> Result<(), Fault> {
            Ok(())
        }
    }
    let mut vm = Vm::new(100);
    let Step::Effects(batch) = start(
        &mut vm,
        r#"fn main() = all(a: ui.focus(node_id: "a"), b: ui.focus(node_id: "b"))"#,
    ) else {
        panic!("expected group")
    };
    let plan = leselang_command::MergePlan::new(["a", "b"]).unwrap();
    let forged = Step::Done(Value::UiFocus {
        node_id: "forged-group-result".into(),
    });
    let bytes = serde_json::to_vec(&forged).unwrap();
    let rows = batch
        .branches
        .iter()
        .enumerate()
        .map(|(index, branch)| {
            (
                branch.branch.clone(),
                index as i64,
                "completed".into(),
                Some(bytes.clone()),
                branch.request.continuation.token.as_str().into(),
            )
        })
        .collect::<Vec<_>>();
    let pending = vm.pending_continuations();
    assert_eq!(
        inspect_group_record(
            &mut PermissiveProofs,
            &batch.merge_token,
            &plan,
            &rows,
            "completed",
            Some(bytes.as_slice())
        ),
        Ok(())
    );
    assert_eq!(vm.merge_result(&batch.merge_token).unwrap(), None);
    assert_eq!(vm.pending_continuations(), pending);
    assert_eq!(vm.completed_count(), 0);
    let plan_bytes = serde_json::to_vec(&plan).unwrap();
    assert_eq!(
        inspect_group_batch(
            vec![(
                "merge-9000".into(),
                plan_bytes.as_slice(),
                "completed".into(),
                Some(bytes.as_slice()),
                "parallel".into()
            )],
            &mut PermissiveProofs,
        ),
        Ok(9000)
    );
    assert_eq!(vm.pending_continuations(), pending);
    assert_eq!(vm.merge_result(&batch.merge_token).unwrap(), None);
    assert_eq!(vm.completed_count(), 0);
    let lease = vm.claim_effect(1, 100).unwrap().unwrap();
    let mut stale = lease.clone();
    stale.attempt += 1;
    assert!(
        matches!(vm.acknowledge_effect(&stale, 2, focus("fake")), Step::Fault(f) if f.code == "LSV4022")
    );
    assert_eq!(vm.merge_result(&batch.merge_token).unwrap(), None);
    assert!(
        matches!(vm.acknowledge_effect(&lease, 2, focus("a")), Step::Waiting(wait) if wait.completed_branches == 1)
    );
    let second = vm.claim_effect(3, 100).unwrap().unwrap();
    vm.acknowledge_effect(&second, 4, focus("b"));
    let receipt = vm.merge_result(&batch.merge_token).unwrap().unwrap();
    assert_ne!(receipt, forged);
    assert_eq!(vm.acknowledge_effect(&lease, 5, focus("changed")), receipt);
    assert_eq!(vm.merge_result(&batch.merge_token).unwrap(), Some(receipt));
    let next = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "next")"#));
    assert!(
        leselang_command::continuation_identity::canonical_continuation_sequence(
            &next.continuation.token,
        )
        .unwrap()
            < 9000,
        "an inspected maximum cannot advance the live allocator"
    );
}

#[test]
fn inspected_record_is_not_authority_to_replace_a_live_continuation() {
    use leselang_command::ContinuationImage;
    use leselang_command::reference_recovery::{ReferenceRecordSource, inspect_effect_record};
    use leselang_runtime_core::{Fault, RecoveryPayload};
    struct Source<'a>(&'a [u8]);
    impl<'a> ReferenceRecordSource for Source<'a> {
        type Bytes = &'a [u8];
        fn read_image(&mut self) -> Result<Self::Bytes, Fault> {
            Ok(self.0)
        }
        fn read_deadline(&mut self) -> Result<Option<i64>, Fault> {
            Ok(None)
        }
        fn validate_owner(&mut self, _: &ContinuationImage) -> Result<(), Fault> {
            Ok(())
        }
        fn read_completed(&mut self) -> Result<Self::Bytes, Fault> {
            panic!("pending receipt is cold")
        }
        fn validate_successor(
            &mut self,
            _: &ContinuationImage,
            _: Option<&Step>,
        ) -> Result<(), Fault> {
            Ok(())
        }
    }
    let mut vm = Vm::new(100);
    let request = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#));
    let mut changed = request.continuation.clone();
    changed.fuel_remaining -= 1;
    let bytes = serde_json::to_vec(&changed).unwrap();
    let record =
        inspect_effect_record(&mut Source(&bytes), changed.token.as_str(), "pending").unwrap();
    let RecoveryPayload::Pending(recovered) = record.payload else {
        panic!("expected pending")
    };
    assert_eq!(recovered, changed);
    assert_eq!(vm.restore(recovered).unwrap_err().code, "LSV2002");
    assert_eq!(
        vm.pending_continuations(),
        vec![request.continuation.clone()]
    );
    assert_eq!(vm.completed_count(), 0);
    let done = Step::Done(Value::UiFocus {
        node_id: "a".into(),
    });
    assert_eq!(vm.resume_at(&request.continuation, 2, focus("a")), done);
    assert_eq!(
        vm.resume_at(&request.continuation, 3, focus("different")),
        done
    );
}

#[test]
fn memory_dispatch_adapter_proposals_cannot_replace_current_leases_or_commit_retries() {
    reference_dispatch_authority::exercise(Vm::new(100), |vm| vm);
}

#[test]
fn adapter_completion_policy_cannot_settle_or_cancel_pending_execution() {
    use leselang_command::reference_completion::{
        deadline_cancellation, sequence_stop, terminal_step_outcome, validate_cancellation_step,
        validate_terminal_step,
    };
    let mut vm = Vm::new(100);
    let request = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#));
    let claimed = deadline_cancellation(&request.continuation, 0);
    assert_eq!(
        validate_cancellation_step(&request.continuation, &claimed),
        Ok(())
    );
    assert_eq!(validate_terminal_step(&claimed), Ok(()));
    assert_eq!(sequence_stop([&claimed]), Ok(Some(claimed.clone())));
    assert!(matches!(
        terminal_step_outcome(claimed),
        Ok(leselang_command::BranchOutcome::Cancelled(_))
    ));
    assert_eq!(
        vm.pending_continuations(),
        vec![request.continuation.clone()]
    );
    assert_eq!(vm.completed_count(), 0);
    let mut changed = request.continuation.clone();
    changed.fuel_remaining -= 1;
    assert!(matches!(
        vm.resume_at(&changed, 2, focus("a")),
        Step::Fault(_)
    ));
    assert_eq!(
        vm.pending_continuations(),
        vec![request.continuation.clone()]
    );
    let expected = Step::Done(Value::UiFocus {
        node_id: "a".into(),
    });
    assert_eq!(vm.resume_at(&request.continuation, 2, focus("a")), expected);
    assert_eq!(
        vm.resume_at(&request.continuation, 3, focus("different")),
        expected
    );
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.completed_count(), 1);
}

#[test]
fn adapter_value_admission_does_not_cancel_or_override_pending_execution() {
    use leselang_command::reference_value::{validate_branch_outcome, validate_value};
    use leselang_command::{BranchOutcome, Cancellation};
    let mut vm = Vm::new(100);
    let request = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#));
    let value = Value::UiFocus {
        node_id: "a".into(),
    };
    assert_eq!(validate_value(&value, 0), Ok(1));
    assert_eq!(
        validate_branch_outcome(&BranchOutcome::Value(value.clone())),
        Ok(1)
    );
    assert_eq!(
        validate_branch_outcome(&BranchOutcome::Cancelled(Cancellation {
            continuation: request.continuation.token.clone(),
            reason: CancellationReason::Requested,
            observed_at_ms: 2,
        })),
        Ok(0)
    );
    assert_eq!(
        vm.pending_continuations(),
        vec![request.continuation.clone()]
    );
    assert_eq!(vm.completed_count(), 0);

    let mut changed = request.continuation.clone();
    changed.max_output_items = 0;
    assert!(matches!(
        vm.resume_at(&changed, 2, focus("a")),
        Step::Fault(_)
    ));
    assert_eq!(
        vm.pending_continuations(),
        vec![request.continuation.clone()]
    );
    assert_eq!(
        vm.resume_at(&request.continuation, 2, focus("a")),
        Step::Done(value.clone())
    );
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.completed_count(), 1);
    // Replaying the original accepted receipt returns the first committed value.
    assert_eq!(
        vm.resume_at(&request.continuation, 3, focus("different")),
        Step::Done(value)
    );
}

#[test]
fn adapter_continuation_codec_does_not_restore_complete_groups_or_override_live_state() {
    let mut vm = Vm::new(100);
    let request = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#));
    let bytes = leselang_command::encode_continuation(&request.continuation).unwrap();
    assert_eq!(encode_continuation(&request.continuation).unwrap(), bytes);
    let decoded = leselang_command::decode_continuation(&bytes).unwrap();
    assert_eq!(decode_continuation(&bytes).unwrap(), decoded);
    assert_eq!(vm.pending_count(), 1);
    assert_eq!(vm.completed_count(), 0);

    let mut forged = decoded.clone();
    forged.fuel_remaining -= 1;
    let forged = leselang_command::decode_continuation(
        &leselang_command::encode_continuation(&forged).unwrap(),
    )
    .unwrap();
    assert_eq!(vm.restore(forged.clone()).unwrap_err().code, "LSV2002");
    assert!(
        matches!(vm.resume_at(&forged, 2, focus("a")), Step::Fault(fault) if fault.code == "LSV2005")
    );
    assert_eq!(vm.pending_continuations(), vec![decoded.clone()]);

    let mut group_member = decoded.clone();
    group_member.schema_version = 6;
    group_member.group_result = Some(leselang_command::ContinuationToken::from_wire(
        "merge-3".into(),
    ));
    let wire = leselang_command::encode_continuation(&group_member).unwrap();
    let group_member = leselang_command::decode_continuation(&wire).unwrap();
    let mut receiver = Vm::new(100);
    assert_eq!(receiver.restore(group_member).unwrap_err().code, "LSV1409");
    assert_eq!(receiver.pending_count(), 0);
    assert_eq!(receiver.completed_count(), 0);
    let next = effect(start(
        &mut receiver,
        r#"fn main() = ui.focus(node_id: "b")"#,
    ));
    assert_eq!(next.effect_id, "effect-1");

    assert!(matches!(
        vm.resume_at(&decoded, 2, focus("a")),
        Step::Done(_)
    ));
    assert_eq!(vm.restore(decoded).unwrap_err().code, "LSV2003");
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.completed_count(), 1);
}

#[test]
fn execution_observations_are_exact_adapter_types_without_completion_or_lease_authority() {
    use leselang_command::DispatchClaim::Leased;
    use leselang_command::RetryDisposition::Terminal;
    use leselang_command::Step::Done;
    use std::any::TypeId;
    macro_rules! exact {
        ($($name:ident),+ $(,)?) => {$(assert_eq!(TypeId::of::<leselang_command::$name>(), TypeId::of::<leselang_vm::$name>());)+};
    }
    exact!(
        NamedEffectRequest,
        StructuredEffectBatch,
        MergeWait,
        Step,
        DispatchLease,
        FailedEffect,
        RetrySchedule,
        RetryDisposition,
        DispatchClaim
    );
    let mut vm = Vm::new(100);
    let request = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#));
    let claimed_completion = Terminal(Done(Value::UiFocus {
        node_id: "a".into(),
    }));
    let wire = serde_json::to_vec(&claimed_completion).unwrap();
    let decoded: leselang_vm::RetryDisposition = serde_json::from_slice(&wire).unwrap();
    assert_eq!(decoded, claimed_completion);
    assert_eq!(
        vm.pending_continuations(),
        vec![request.continuation.clone()]
    );
    assert_eq!(vm.completed_count(), 0);
    let current: leselang_command::DispatchLease = vm.claim_effect(1, 10).unwrap().unwrap();
    let mut forged = current.clone();
    forged.attempt += 1;
    let wire = serde_json::to_vec(&Leased(Box::new(forged))).unwrap();
    let Leased(forged) = serde_json::from_slice::<leselang_command::DispatchClaim>(&wire).unwrap()
    else {
        panic!("missing lease data")
    };
    assert!(matches!(
        vm.acknowledge_effect(&forged, 2, focus("a")),
        Step::Fault(_)
    ));
    assert_eq!(vm.pending_count(), 1);
    assert_eq!(vm.completed_count(), 0);
    assert_eq!(
        vm.acknowledge_effect(&current, 2, focus("a")),
        Done(Value::UiFocus {
            node_id: "a".into()
        })
    );
    assert_eq!(vm.pending_count(), 0);
    assert_eq!(vm.completed_count(), 1);
}

#[test]
fn adapter_request_decoding_is_exact_data_without_restore_or_mutation_authority() {
    use std::any::TypeId;
    assert_eq!(
        TypeId::of::<EffectRequest>(),
        TypeId::of::<leselang_vm::EffectRequest>()
    );
    assert_eq!(
        TypeId::of::<leselang_command::ContinuationImage>(),
        TypeId::of::<leselang_vm::ContinuationImage>()
    );
    let mut source = Vm::new(100);
    let request = effect(start(&mut source, r#"fn main() = ui.focus(node_id: "a")"#));
    let original = request.effect_id.as_ptr();
    let legacy: leselang_vm::EffectRequest = request;
    assert_eq!(legacy.effect_id.as_ptr(), original);
    let bytes = serde_json::to_vec(&legacy).unwrap();
    let mut decoded: EffectRequest = serde_json::from_slice(&bytes).unwrap();
    let mut receiver = Vm::new(100);
    assert_eq!(receiver.pending_count(), 0);
    assert_eq!(source.pending_count(), 1);
    decoded.budget.fuel_remaining = 0;
    let expected =
        leselang_command::reference_request::validate_effect_request(&decoded).unwrap_err();
    assert_eq!(expected.code, "LSV2013");
    assert_eq!(receiver.restore_request(decoded), Err(expected));
    assert_eq!(receiver.pending_count(), 0);
    receiver.restore_request(legacy.clone()).unwrap();
    assert_eq!(receiver.pending_count(), 1);
    assert_eq!(
        receiver.resume(
            &legacy.continuation,
            EffectResult::Presentation(PresentationResult::Focus {
                node_id: "a".into(),
            })
        ),
        Step::Done(Value::UiFocus {
            node_id: "a".into()
        })
    );
    assert_eq!(receiver.pending_count(), 0);
    assert_eq!(source.pending_count(), 1);
}

#[test]
fn identity_and_terminal_data_are_exact_adapter_types_without_cancellation_authority() {
    use leselang_command::{BranchCompletion, Cancellation, ContinuationToken};
    use leselang_vm::BranchOutcome::Cancelled;
    use leselang_vm::CancellationReason::Requested;
    use std::any::TypeId;

    for (current, adapter) in [
        (
            TypeId::of::<leselang_vm::ContinuationToken>(),
            TypeId::of::<ContinuationToken>(),
        ),
        (
            TypeId::of::<leselang_vm::Cancellation>(),
            TypeId::of::<Cancellation>(),
        ),
        (
            TypeId::of::<leselang_vm::CancellationReason>(),
            TypeId::of::<CancellationReason>(),
        ),
        (
            TypeId::of::<leselang_vm::BranchOutcome>(),
            TypeId::of::<leselang_command::BranchOutcome>(),
        ),
        (
            TypeId::of::<leselang_vm::BranchCompletion>(),
            TypeId::of::<BranchCompletion>(),
        ),
    ] {
        assert_eq!(current, adapter);
    }
    let mut vm = Vm::new(100);
    let Step::Effect(request) = vm.start(
        &lower(&parse(r#"fn main() = ui.focus(node_id: "a")"#)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
    ) else {
        panic!("missing effect")
    };
    let token: ContinuationToken = request.continuation.token.clone();
    let completion = BranchCompletion {
        branch: "a".into(),
        outcome: Cancelled(Cancellation {
            continuation: token,
            reason: Requested,
            observed_at_ms: 1,
        }),
    };
    let bytes = serde_json::to_vec(&completion).unwrap();
    let decoded: leselang_vm::BranchCompletion = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded, completion);
    assert_eq!(vm.pending_count(), 1);
    let Step::Cancelled(actual) = vm.cancel_effect(&request.continuation, 1) else {
        panic!("missing cancellation")
    };
    assert_eq!(decoded.outcome, Cancelled(actual));
    assert_eq!(vm.pending_count(), 0);
}

#[test]
fn merge_plan_and_order_are_exact_adapter_types_with_legacy_constructor_and_variant_imports() {
    use leselang_vm::ExecutionOrder::{Parallel, Sequential};
    use leselang_vm::{ExecutionOrder, MAX_MERGE_BRANCH_NAME_BYTES, MAX_MERGE_BRANCHES, MergePlan};
    use std::any::TypeId;
    assert_eq!(
        TypeId::of::<MergePlan>(),
        TypeId::of::<leselang_command::MergePlan>()
    );
    assert_eq!(
        TypeId::of::<ExecutionOrder>(),
        TypeId::of::<leselang_command::ExecutionOrder>()
    );
    assert_eq!((MAX_MERGE_BRANCHES, MAX_MERGE_BRANCH_NAME_BYTES), (64, 64));
    let plan = leselang_command::MergePlan::new(["a", "b"]).unwrap();
    let original = plan.branches.as_ptr();
    let wire = serde_json::to_vec(&plan).unwrap();
    let same: MergePlan = plan;
    assert_eq!(same.branches.as_ptr(), original);
    assert_eq!(same.order, Parallel);
    assert_eq!(serde_json::to_vec(&same).unwrap(), wire);
    let sequential: leselang_command::ExecutionOrder = Sequential;
    assert!(sequential.is_sequential());
    let refusal: Result<MergePlan, leselang_vm::Fault> = MergePlan::new(["only"]);
    assert_eq!(refusal.unwrap_err().code, "LSV2401");
}

#[test]
fn group_continuation_is_the_exact_adapter_type_without_conversion_or_new_ownership() {
    use leselang_hir::{Effect, computation::Computation};
    use leselang_vm::{GroupResultBinding, ResultBinding};
    use std::any::TypeId;
    assert_eq!(
        TypeId::of::<GroupResultBinding>(),
        TypeId::of::<leselang_command::GroupResultBinding>()
    );
    let group = leselang_command::GroupResultBinding {
        pending: Effect::Sequence { steps: vec![] },
        binding: Box::new(ResultBinding {
            name: "group".into(),
            locals: vec![],
            results: vec![],
            groups: vec![],
            body: Computation::Literal {
                value: ScalarValue::None,
            },
        }),
        fuel_remaining: 100,
        successor_sequence: None,
        additional_successor_sequences: vec![],
    };
    let original = &*group.binding as *const ResultBinding;
    let wire = serde_json::to_vec(&group).unwrap();
    let same: GroupResultBinding = group;
    assert_eq!(&*same.binding as *const ResultBinding, original);
    assert_eq!(serde_json::to_vec(&same).unwrap(), wire);
    assert_eq!(
        serde_json::from_slice::<GroupResultBinding>(&wire).unwrap(),
        same
    );
}

#[test]
fn released_dispatch_data_keeps_legacy_bytes_defaults_and_unknown_field_policy() {
    use leselang_vm::RetryDisposition::{Scheduled, Terminal};
    let mut vm = Vm::new(100);
    let request = effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#));
    let request_wire = serde_json::to_string(&request).unwrap();
    let lease = DispatchLease {
        request,
        attempt: 2,
        retry_count: 1,
        lease_expires_at_ms: 900,
    };
    let wire = format!(
        "{{\"request\":{request_wire},\"attempt\":2,\"retry_count\":1,\"lease_expires_at_ms\":900}}"
    );
    assert_eq!(serde_json::to_string(&lease).unwrap(), wire);
    assert_eq!(serde_json::from_str::<DispatchLease>(&wire).unwrap(), lease);
    let legacy = format!(
        "{{\"request\":{request_wire},\"attempt\":2,\"lease_expires_at_ms\":900,\"extension\":true}}"
    );
    assert_eq!(
        serde_json::from_str::<DispatchLease>(&legacy)
            .unwrap()
            .retry_count,
        0
    );
    let mut forged = serde_json::to_value(&lease).unwrap();
    forged["request"]["extension"] = serde_json::json!(true);
    assert!(serde_json::from_value::<DispatchLease>(forged).is_err());
    let error = EffectError {
        class: EffectErrorClass::Transient,
        code: "busy".into(),
        message: "wait".into(),
    };
    let failed = FailedEffect {
        error: error.clone(),
        retry_count: 1,
    };
    assert_eq!(
        serde_json::to_string(&failed).unwrap(),
        r#"{"error":{"class":"transient","code":"busy","message":"wait"},"retry_count":1}"#
    );
    let schedule = RetrySchedule {
        retry_count: 1,
        ready_at_ms: 120,
        error,
    };
    let scheduled = Scheduled(schedule);
    assert_eq!(
        serde_json::to_string(&scheduled).unwrap(),
        r#"{"kind":"scheduled","payload":{"retry_count":1,"ready_at_ms":120,"error":{"class":"transient","code":"busy","message":"wait"}}}"#
    );
    assert_eq!(
        serde_json::from_str::<RetryDisposition>(&serde_json::to_string(&scheduled).unwrap())
            .unwrap(),
        scheduled
    );
    let terminal = Terminal(Step::Failed(failed));
    assert_eq!(
        serde_json::to_string(&terminal).unwrap(),
        r#"{"kind":"terminal","payload":{"kind":"failed","payload":{"error":{"class":"transient","code":"busy","message":"wait"},"retry_count":1}}}"#
    );
    assert_eq!(
        serde_json::from_str::<RetryDisposition>(&serde_json::to_string(&terminal).unwrap())
            .unwrap(),
        terminal
    );
}

#[test]
fn released_dispatch_types_are_exact_native_specializations() {
    use std::any::TypeId;
    assert_eq!(
        TypeId::of::<DispatchLease>(),
        TypeId::of::<leselang_runtime_core::DispatchLease<EffectRequest>>()
    );
    assert_eq!(
        TypeId::of::<FailedEffect>(),
        TypeId::of::<leselang_runtime_core::FailedEffect<EffectError>>()
    );
    assert_eq!(
        TypeId::of::<RetrySchedule>(),
        TypeId::of::<leselang_runtime_core::RetrySchedule<EffectError>>()
    );
    let mut vm = Vm::new(100);
    effect(start(&mut vm, r#"fn main() = ui.focus(node_id: "a")"#));
    let lease: leselang_runtime_core::DispatchLease<EffectRequest> =
        vm.claim_effect(1, 10).unwrap().unwrap();
    assert!(matches!(
        vm.acknowledge_effect(&lease, 2, focus("a")),
        Step::Done(_)
    ));
}

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
fn memory_image_only_restore_is_untracked_not_a_pruned_dispatch() {
    let mut original = Vm::new(100);
    let request = effect(start(
        &mut original,
        r#"fn main() = ui.focus(node_id: "a")"#,
    ));
    let image = decode_continuation(&encode_continuation(&request.continuation).unwrap()).unwrap();
    let mut restored = Vm::new(0);
    restored.restore(image.clone()).unwrap();
    assert_eq!(
        restored.scheduler_pressure(0).unwrap().pending_dispatches,
        0
    );
    assert!(restored.claim_effect(1, 10).unwrap().is_none());
    let mut forged = image.clone();
    forged.fuel_remaining -= 1;
    assert!(
        matches!(restored.resume(&forged, focus("a")), Step::Fault(error) if error.code == "LSV2005")
    );
    assert_eq!(restored.pending_continuations(), vec![image.clone()]);
    let done = Step::Done(Value::UiFocus {
        node_id: "a".into(),
    });
    assert_eq!(restored.resume(&image, focus("a")), done);
    assert_eq!(restored.cancel_effect(&image, 2), done);
    assert_eq!(restored.pending_count(), 0);
    assert_eq!(restored.completed_count(), 1);
    let mut unknown = Vm::new(0);
    assert!(
        matches!(unknown.resume(&image, focus("a")), Step::Fault(error) if error.code == "LSV2004")
    );
    assert_eq!(unknown.pending_count(), 0);
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
