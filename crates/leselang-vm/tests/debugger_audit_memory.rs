// Exercise the released HIR path with the same product-owned audit oracle.
extern crate leselang_hir as leserpent_hir;

#[path = "../../leserpent-vm/src/debugger_audit_contract_tests.rs"]
mod contract;

macro_rules! memory_case {
    ($($name:ident),+ $(,)?) => { $(
        #[test]
        fn $name() {
            contract::$name(leselang_vm::Vm::new(500), &mut |vm| vm);
        }
    )+ };
}

memory_case!(
    ordinary_cancellation_cannot_be_retroactively_audited,
    fresh_command_cannot_replace_committed_cancellation_attribution,
    exact_replay_preserves_original_timestamp_and_all_identity_fields,
    command_and_principal_key_conflicts_do_not_cancel_another_pending_root,
    propagated_sequence_cancellation_cannot_gain_a_sibling_audit,
    completion_and_deadline_cannot_be_relabelled_as_debugger_cancellation,
    compaction_releases_only_pruned_audits_and_their_command_keys,
    compaction_keeps_live_group_audits_then_removes_the_complete_unit,
    bounded_compaction_tracks_audits_across_batches_without_touching_pending_work,
);
