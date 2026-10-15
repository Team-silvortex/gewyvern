// Shared by product-only integration targets, not part of the native VM module tree.
use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, CommandOrigin, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    DebuggerAuditContext, EffectRequest, EffectResult, PresentationResult, RetentionPolicy, Step,
    Vm,
};
use leserpent_domain::{CommandId, IdempotencyKey};

fn start(vm: &mut Vm, source: &str) -> EffectRequest {
    let Step::Effect(request) = start_step(vm, source) else {
        panic!("expected a pending effect");
    };
    *request
}

fn start_step(vm: &mut Vm, source: &str) -> Step {
    vm.start_timed(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        100,
        1000,
    )
}

pub(super) fn focus(vm: &mut Vm) -> EffectRequest {
    start(vm, r#"fn main() = ui.focus(node_id: "node")"#)
}

pub(super) fn receipt() -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: "node".into(),
    })
}

pub(super) fn audit(suffix: &str) -> DebuggerAuditContext {
    DebuggerAuditContext {
        command_id: CommandId::new(format!("command-{suffix}")).unwrap(),
        idempotency_key: IdempotencyKey::new(format!("key-{suffix}")).unwrap(),
        principal: Principal::new("debugger-operator").unwrap(),
        origin: CommandOrigin::Gui,
        session_id: "session-a".into(),
        expected_revision: Revision(7),
    }
}

pub fn ordinary_cancellation_cannot_be_retroactively_audited(
    mut vm: Vm,
    reopen: &mut dyn FnMut(Vm) -> Vm,
) {
    let request = focus(&mut vm);
    let terminal = vm.cancel_effect(&request.continuation, 102);
    assert!(matches!(terminal, Step::Cancelled(_)));
    vm = reopen(vm);
    let proposed = audit("late");
    let error = vm
        .cancel_effect_audited(&request.continuation, 200, &proposed)
        .unwrap_err();
    assert_eq!(error.code, "LSV4033");
    assert_eq!(
        error.message,
        "completed effect has no matching debugger audit"
    );
    assert_eq!(vm.debugger_audit(&proposed.command_id).unwrap(), None);
    assert_eq!(vm.cancel_effect(&request.continuation, 201), terminal);
    vm = reopen(vm);
    assert_eq!(vm.cancel_effect(&request.continuation, 202), terminal);
    assert_eq!(vm.debugger_audit(&proposed.command_id).unwrap(), None);
}

pub fn fresh_command_cannot_replace_committed_cancellation_attribution(
    mut vm: Vm,
    reopen: &mut dyn FnMut(Vm) -> Vm,
) {
    let request = focus(&mut vm);
    let first = audit("first");
    let record = vm
        .cancel_effect_audited(&request.continuation, 102, &first)
        .unwrap();
    let terminal = vm.cancel_effect(&request.continuation, 103);
    vm = reopen(vm);
    let replacement = audit("replacement");
    let error = vm
        .cancel_effect_audited(&request.continuation, 200, &replacement)
        .unwrap_err();
    assert_eq!(error.code, "LSV4033");
    assert_eq!(vm.debugger_audit(&replacement.command_id).unwrap(), None);
    assert_eq!(
        vm.debugger_audit(&first.command_id).unwrap(),
        Some(record.clone())
    );
    assert_eq!(vm.cancel_effect(&request.continuation, 201), terminal);
    vm = reopen(vm);
    assert_eq!(
        vm.cancel_effect_audited(&request.continuation, 202, &first)
            .unwrap(),
        record
    );
    assert_eq!(vm.cancel_effect(&request.continuation, 203), terminal);
}

pub fn exact_replay_preserves_original_timestamp_and_all_identity_fields(
    mut vm: Vm,
    reopen: &mut dyn FnMut(Vm) -> Vm,
) {
    let request = focus(&mut vm);
    let original = audit("exact");
    let record = vm
        .cancel_effect_audited(&request.continuation, 102, &original)
        .unwrap();
    let terminal = vm.cancel_effect(&request.continuation, 103);
    vm = reopen(vm);
    assert_eq!(
        vm.cancel_effect_audited(&request.continuation, 200, &original)
            .unwrap(),
        record
    );
    for field in 0..6 {
        let mut changed = original.clone();
        match field {
            0 => changed.command_id = CommandId::new("other-command").unwrap(),
            1 => changed.idempotency_key = IdempotencyKey::new("other-key").unwrap(),
            2 => changed.principal = Principal::new("other-operator").unwrap(),
            3 => changed.origin = CommandOrigin::Cli,
            4 => changed.session_id = "other-session".into(),
            5 => changed.expected_revision = Revision(8),
            _ => unreachable!(),
        }
        assert_eq!(
            vm.cancel_effect_audited(&request.continuation, 201, &changed)
                .unwrap_err()
                .code,
            "LSV4033"
        );
        assert_eq!(
            vm.debugger_audit(&original.command_id).unwrap(),
            Some(record.clone())
        );
        assert_eq!(vm.cancel_effect(&request.continuation, 202), terminal);
    }
    vm = reopen(vm);
    assert_eq!(
        vm.cancel_effect_audited(&request.continuation, 203, &original)
            .unwrap(),
        record
    );
    assert_eq!(record.observed_at_ms, 102);
}

pub fn command_and_principal_key_conflicts_do_not_cancel_another_pending_root(
    mut vm: Vm,
    reopen: &mut dyn FnMut(Vm) -> Vm,
) {
    let first = focus(&mut vm);
    let second = focus(&mut vm);
    let original = audit("root-a");
    let record = vm
        .cancel_effect_audited(&first.continuation, 102, &original)
        .unwrap();
    vm = reopen(vm);
    let pending = vm.pending_continuations();
    let conflict = DebuggerAuditContext {
        command_id: CommandId::new("root-b").unwrap(),
        ..original.clone()
    };
    for candidate in [&original, &conflict] {
        assert_eq!(
            vm.cancel_effect_audited(&second.continuation, 200, candidate)
                .unwrap_err()
                .code,
            "LSV4033"
        );
        assert_eq!(vm.pending_continuations(), pending);
    }
    assert_eq!(vm.debugger_audit(&conflict.command_id).unwrap(), None);
    assert_eq!(
        vm.debugger_audit(&original.command_id).unwrap(),
        Some(record)
    );
    // Idempotency keys are principal-scoped, while command IDs are globally unique.
    let independent = DebuggerAuditContext {
        principal: Principal::new("second-operator").unwrap(),
        ..conflict
    };
    let second_record = vm
        .cancel_effect_audited(&second.continuation, 201, &independent)
        .unwrap();
    vm = reopen(vm);
    assert_eq!(
        vm.cancel_effect_audited(&second.continuation, 202, &independent)
            .unwrap(),
        second_record
    );
}

pub fn propagated_sequence_cancellation_cannot_gain_a_sibling_audit(
    mut vm: Vm,
    reopen: &mut dyn FnMut(Vm) -> Vm,
) {
    let first = start(
        &mut vm,
        r#"fn main() = seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#,
    );
    let sibling = vm
        .pending_continuations()
        .into_iter()
        .find(|image| image.token != first.continuation.token)
        .unwrap();
    let original = audit("sequence");
    let record = vm
        .cancel_effect_audited(&first.continuation, 102, &original)
        .unwrap();
    let terminal = vm.cancel_effect(&first.continuation, 103);
    vm = reopen(vm);
    let late = audit("sibling");
    assert_eq!(
        vm.cancel_effect_audited(&sibling, 200, &late)
            .unwrap_err()
            .code,
        "LSV4033"
    );
    assert_eq!(vm.debugger_audit(&late.command_id).unwrap(), None);
    assert_eq!(
        vm.debugger_audit(&original.command_id).unwrap(),
        Some(record)
    );
    assert_eq!(vm.cancel_effect(&sibling, 201), terminal);
    assert_eq!(vm.pending_count(), 0);
    vm = reopen(vm);
    assert_eq!(vm.cancel_effect(&first.continuation, 202), terminal);
    assert!(vm.claim_effect(202, 10).unwrap().is_none());
}

pub fn completion_and_deadline_cannot_be_relabelled_as_debugger_cancellation(
    mut vm: Vm,
    reopen: &mut dyn FnMut(Vm) -> Vm,
) {
    let done = focus(&mut vm);
    let expired = focus(&mut vm);
    let terminal = vm.resume_at(&done.continuation, 101, receipt());
    assert!(matches!(terminal, Step::Done(_)));
    vm = reopen(vm);
    for (request, now) in [(&done, 200), (&expired, 1100)] {
        let proposed = audit(&request.effect_id);
        assert_eq!(
            vm.cancel_effect_audited(&request.continuation, now, &proposed)
                .unwrap_err()
                .code,
            "LSV2014"
        );
        assert_eq!(vm.debugger_audit(&proposed.command_id).unwrap(), None);
    }
    assert_eq!(vm.cancel_effect(&done.continuation, 1101), terminal);
    assert_eq!(vm.pending_count(), 0);
    vm = reopen(vm);
    assert_eq!(vm.cancel_effect(&done.continuation, 1102), terminal);
}

pub fn compaction_releases_only_pruned_audits_and_their_command_keys(
    mut vm: Vm,
    reopen: &mut dyn FnMut(Vm) -> Vm,
) {
    let oldest = focus(&mut vm);
    let retained = focus(&mut vm);
    let pending = focus(&mut vm);
    let old_audit = audit("oldest");
    let retained_audit = audit("retained");
    vm.cancel_effect_audited(&oldest.continuation, 102, &old_audit)
        .unwrap();
    let retained_record = vm
        .cancel_effect_audited(&retained.continuation, 103, &retained_audit)
        .unwrap();
    vm = reopen(vm);
    let report = vm
        .compact_journal(&RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 1,
        })
        .unwrap();
    assert_eq!(report.removed_records, 1);
    assert_eq!(report.remaining_completed, 1);
    assert!(report.reclaimed_logical_bytes > 0);
    assert_eq!(vm.debugger_audit(&old_audit.command_id).unwrap(), None);
    assert_eq!(
        vm.debugger_audit(&retained_audit.command_id).unwrap(),
        Some(retained_record.clone())
    );
    assert!(
        matches!(vm.cancel_effect(&oldest.continuation, 200), Step::Fault(fault) if fault.code == "LSV2004")
    );
    assert_eq!(
        vm.pending_continuations(),
        vec![pending.continuation.clone()]
    );
    vm = reopen(vm);
    let replacement = vm
        .cancel_effect_audited(&pending.continuation, 201, &old_audit)
        .unwrap();
    assert_eq!(replacement.observed_at_ms, 201);
    assert_ne!(oldest.continuation.token, pending.continuation.token);
    let no_op = vm
        .compact_journal(&RetentionPolicy {
            max_completed_records: 2,
            max_delete_per_run: 1,
        })
        .unwrap();
    assert_eq!(no_op.removed_records, 0);
    assert_eq!(no_op.reclaimed_logical_bytes, 0);
    vm = reopen(vm);
    assert_eq!(
        vm.cancel_effect_audited(&pending.continuation, 202, &old_audit)
            .unwrap(),
        replacement
    );
    assert_eq!(
        vm.cancel_effect_audited(&retained.continuation, 202, &retained_audit)
            .unwrap(),
        retained_record
    );
    assert_eq!(
        vm.cancel_effect_audited(&oldest.continuation, 202, &old_audit)
            .unwrap_err()
            .code,
        "LSV2004"
    );
}

pub fn compaction_keeps_live_group_audits_then_removes_the_complete_unit(
    mut vm: Vm,
    reopen: &mut dyn FnMut(Vm) -> Vm,
) {
    let Step::Effects(batch) = start_step(
        &mut vm,
        r#"fn main() = all(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b"))"#,
    ) else {
        panic!("expected a parallel batch");
    };
    let first = &batch.branches[0].request;
    let second = &batch.branches[1].request;
    let standalone = focus(&mut vm);
    let retained = focus(&mut vm);
    let standalone_audit = audit("standalone");
    let retained_audit = audit("retained-standalone");
    vm.cancel_effect_audited(&standalone.continuation, 101, &standalone_audit)
        .unwrap();
    let retained_record = vm
        .cancel_effect_audited(&retained.continuation, 101, &retained_audit)
        .unwrap();
    let first_audit = audit("parallel-first");
    let first_record = vm
        .cancel_effect_audited(&first.continuation, 102, &first_audit)
        .unwrap();
    let policy = RetentionPolicy {
        max_completed_records: 1,
        max_delete_per_run: 1,
    };
    let report = vm.compact_journal(&policy).unwrap();
    assert_eq!(report.removed_records, 1);
    assert_eq!(report.remaining_completed, 1);
    assert_eq!(
        vm.debugger_audit(&standalone_audit.command_id).unwrap(),
        None
    );
    vm = reopen(vm);
    assert_eq!(
        vm.pending_continuations(),
        vec![second.continuation.clone()]
    );
    assert_eq!(
        vm.cancel_effect_audited(&first.continuation, 200, &first_audit)
            .unwrap(),
        first_record
    );
    let no_op = vm.compact_journal(&policy).unwrap();
    assert_eq!(no_op.removed_records, 0);
    assert_eq!(no_op.reclaimed_logical_bytes, 0);
    assert_eq!(
        vm.debugger_audit(&first_audit.command_id).unwrap(),
        Some(first_record)
    );
    let second_audit = audit("parallel-second");
    vm.cancel_effect_audited(&second.continuation, 201, &second_audit)
        .unwrap();
    assert_eq!(vm.pending_count(), 0);
    vm = reopen(vm);
    let report = vm.compact_journal(&policy).unwrap();
    assert_eq!(report.removed_records, 1);
    assert_eq!(report.remaining_completed, 1);
    assert!(report.reclaimed_logical_bytes > 0);
    assert_eq!(vm.merge_result(&batch.merge_token).unwrap(), None);
    vm = reopen(vm);
    assert_eq!(
        vm.debugger_audit(&retained_audit.command_id).unwrap(),
        Some(retained_record)
    );
    for (request, context) in [(first, &first_audit), (second, &second_audit)] {
        assert_eq!(vm.debugger_audit(&context.command_id).unwrap(), None);
        assert_eq!(
            vm.cancel_effect_audited(&request.continuation, 202, context)
                .unwrap_err()
                .code,
            "LSV2004"
        );
    }
    assert!(vm.claim_effect(202, 10).unwrap().is_none());
}

pub fn bounded_compaction_tracks_audits_across_batches_without_touching_pending_work(
    mut vm: Vm,
    reopen: &mut dyn FnMut(Vm) -> Vm,
) {
    let mut completed = Vec::new();
    for index in 0..12 {
        let request = focus(&mut vm);
        let context = audit(&format!("bounded-{index}"));
        let record = (index % 2 == 1).then(|| {
            vm.cancel_effect_audited(&request.continuation, 102, &context)
                .unwrap()
        });
        let terminal = vm.cancel_effect(&request.continuation, 103);
        completed.push((request, context, record, terminal));
    }
    let pending = focus(&mut vm);
    vm = reopen(vm);
    assert_eq!(
        vm.compact_journal(&RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 0,
        })
        .unwrap_err()
        .code,
        "LSV2301"
    );
    for (_, context, record, _) in &completed {
        assert_eq!(vm.debugger_audit(&context.command_id).unwrap(), *record);
    }
    let mut removed = 0;
    for expected_removed in [3, 3, 3, 1] {
        let report = vm
            .compact_journal(&RetentionPolicy {
                max_completed_records: 2,
                max_delete_per_run: 3,
            })
            .unwrap();
        removed += expected_removed;
        assert_eq!(report.removed_records, expected_removed);
        assert_eq!(report.remaining_completed, 12 - removed);
        assert!(report.reclaimed_logical_bytes > 0);
        vm = reopen(vm);
        for (index, (request, context, record, terminal)) in completed.iter().enumerate() {
            if index < removed {
                assert_eq!(vm.debugger_audit(&context.command_id).unwrap(), None);
                assert!(
                    matches!(vm.cancel_effect(&request.continuation, 200), Step::Fault(fault) if fault.code == "LSV2004")
                );
            } else {
                assert_eq!(vm.debugger_audit(&context.command_id).unwrap(), *record);
                assert_eq!(vm.cancel_effect(&request.continuation, 200), *terminal);
            }
        }
        assert_eq!(
            vm.pending_continuations(),
            vec![pending.continuation.clone()]
        );
    }
    let reused_key = DebuggerAuditContext {
        command_id: CommandId::new("fresh-command-pruned-key").unwrap(),
        ..completed[1].1.clone()
    };
    let record = vm
        .cancel_effect_audited(&pending.continuation, 201, &reused_key)
        .unwrap();
    vm = reopen(vm);
    assert_eq!(
        vm.cancel_effect_audited(&pending.continuation, 202, &reused_key)
            .unwrap(),
        record
    );
    assert_eq!(vm.debugger_audit(&completed[1].1.command_id).unwrap(), None);
}
