// Exercise the released HIR path with the same product-owned audit oracle.
extern crate leselang_hir as leserpent_hir;

use leselang_vm::Vm;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "../../leserpent-vm/src/debugger_audit_contract_tests.rs"]
mod contract;

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "leselang-debugger-audit-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for JournalPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

macro_rules! durable_case {
    ($($name:ident),+ $(,)?) => { $(
        #[test]
        fn $name() {
            let root = JournalPath::new();
            let path = root.0.join("audit.sqlite3");
            contract::$name(Vm::open_journal(&path, 500).unwrap(), &mut |vm| {
                drop(vm);
                Vm::open_journal(&path, 0).unwrap()
            });
        }
    )+ };
}

durable_case!(
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
