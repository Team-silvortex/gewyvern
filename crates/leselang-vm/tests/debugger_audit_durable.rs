use leselang_vm::{CancellationReason, RetentionPolicy, Step, Vm};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

#[path = "../src/debugger_audit_contract_tests.rs"]
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

#[test]
fn shared_audit_replay_discards_pending_cache_when_completion_was_already_pruned() {
    let root = JournalPath::new();
    let path = root.0.join("audit.sqlite3");
    let mut owner = Vm::open_journal(&path, 500).unwrap();
    let request = contract::focus(&mut owner);
    let mut stale = Vm::open_journal(&path, 0).unwrap();
    let context = contract::audit("unobserved");
    owner
        .cancel_effect_audited(&request.continuation, 101, &context)
        .unwrap();
    let retained = contract::focus(&mut owner);
    assert!(matches!(
        owner.resume_at(&retained.continuation, 102, contract::receipt()),
        Step::Done(_)
    ));
    assert_eq!(
        owner
            .compact_journal(&RetentionPolicy {
                max_completed_records: 1,
                max_delete_per_run: 1,
            })
            .unwrap()
            .removed_records,
        1
    );
    assert_eq!(stale.pending_count(), 1);
    assert_eq!(stale.completed_count(), 0);
    let error = stale
        .cancel_effect_audited(&request.continuation, 103, &context)
        .unwrap_err();
    assert_eq!(stale.pending_count(), 0, "{error:?}");
    assert_eq!(error.code, "LSV2004");
    assert_eq!(stale.completed_count(), 0);
    assert_eq!(stale.debugger_audit(&context.command_id).unwrap(), None);
    let next = contract::focus(&mut owner);
    assert_ne!(next.continuation.token, request.continuation.token);
    let new_record = owner
        .cancel_effect_audited(&next.continuation, 104, &context)
        .unwrap();
    assert_eq!(
        stale
            .cancel_effect_audited(&request.continuation, 105, &context)
            .unwrap_err()
            .code,
        "LSV2004"
    );
    assert_eq!(
        stale.debugger_audit(&context.command_id).unwrap(),
        Some(new_record)
    );
}

#[test]
fn shared_audit_replay_observes_later_completion_without_adopting_unclaimed_pending_work() {
    let root = JournalPath::new();
    let path = root.0.join("audit.sqlite3");
    let mut owner = Vm::open_journal(&path, 500).unwrap();
    let mut observer = Vm::open_journal(&path, 0).unwrap();
    let request = contract::focus(&mut owner);
    let context = contract::audit("later-admission");
    assert_eq!(observer.pending_count(), 0);
    assert_eq!(
        observer
            .cancel_effect_audited(&request.continuation, 101, &context)
            .unwrap_err()
            .code,
        "LSV2004"
    );
    assert_eq!(observer.debugger_audit(&context.command_id).unwrap(), None);
    assert_eq!(owner.pending_count(), 1);
    let mut known = Vm::open_journal(&path, 0).unwrap();
    let mut forged = request.continuation.clone();
    forged.fuel_remaining -= 1;
    assert_eq!(
        known
            .cancel_effect_audited(&forged, 101, &context)
            .unwrap_err()
            .code,
        "LSV2005"
    );
    assert_eq!(
        known.pending_continuations(),
        vec![request.continuation.clone()]
    );
    assert_eq!(known.debugger_audit(&context.command_id).unwrap(), None);
    let original = owner
        .cancel_effect_audited(&request.continuation, 102, &context)
        .unwrap();

    let mut invalid = context.clone();
    invalid.session_id.clear();
    assert_eq!(
        observer
            .cancel_effect_audited(&request.continuation, 200, &invalid)
            .unwrap_err()
            .code,
        "LSV2015"
    );
    assert_eq!(observer.completed_count(), 0);
    assert!(
        observer
            .cancel_effect_audited(&request.continuation, u64::MAX, &context)
            .is_err()
    );
    assert_eq!(observer.completed_count(), 0);
    assert_eq!(
        observer
            .cancel_effect_audited(&request.continuation, 200, &context)
            .unwrap(),
        original
    );
    assert_eq!(observer.pending_count(), 0);
    assert_eq!(observer.completed_count(), 1);
    assert_eq!(
        observer
            .cancel_effect_audited(&request.continuation, 201, &context)
            .unwrap(),
        original
    );
    let fresh = contract::audit("no-new-attribution");
    assert_eq!(
        observer
            .cancel_effect_audited(&request.continuation, 202, &fresh)
            .unwrap_err()
            .code,
        "LSV4033"
    );
    assert_eq!(observer.debugger_audit(&fresh.command_id).unwrap(), None);
    assert_eq!(
        observer.debugger_audit(&context.command_id).unwrap(),
        Some(original)
    );
    assert_eq!(
        observer.cancel_effect(&request.continuation, 203),
        owner.cancel_effect(&request.continuation, 203)
    );
}

#[test]
fn shared_audit_refusal_reconciles_peer_success_deadline_and_ordinary_cancellation() {
    for outcome in ["success", "deadline", "ordinary"] {
        let root = JournalPath::new();
        let path = root.0.join("audit.sqlite3");
        let mut owner = Vm::open_journal(&path, 500).unwrap();
        let request = contract::focus(&mut owner);
        let mut observer = Vm::open_journal(&path, 0).unwrap();
        let (terminal, now, code) = match outcome {
            "success" => (
                owner.resume_at(&request.continuation, 101, contract::receipt()),
                200,
                "LSV2014",
            ),
            "deadline" => (
                owner.cancel_effect(&request.continuation, 1100),
                1101,
                "LSV2014",
            ),
            "ordinary" => (
                owner.cancel_effect(&request.continuation, 102),
                200,
                "LSV4033",
            ),
            _ => unreachable!(),
        };
        match (&terminal, outcome) {
            (Step::Done(_), "success") => {}
            (Step::Cancelled(cancelled), "deadline") => {
                assert_eq!(cancelled.reason, CancellationReason::DeadlineExceeded)
            }
            (Step::Cancelled(cancelled), "ordinary") => {
                assert_eq!(cancelled.reason, CancellationReason::Requested)
            }
            _ => panic!("unexpected peer terminal: {terminal:?}"),
        }
        assert_eq!(observer.pending_count(), 1);
        let context = contract::audit(outcome);
        assert_eq!(
            observer
                .cancel_effect_audited(&request.continuation, now, &context)
                .unwrap_err()
                .code,
            code
        );
        assert_eq!(observer.pending_count(), 0);
        assert_eq!(observer.completed_count(), 1);
        assert_eq!(observer.debugger_audit(&context.command_id).unwrap(), None);
        assert_eq!(
            observer.cancel_effect(&request.continuation, now + 1),
            terminal
        );
        let mut reopened = Vm::open_journal(&path, 0).unwrap();
        assert_eq!(
            reopened.cancel_effect(&request.continuation, now + 2),
            terminal
        );
    }
}

#[test]
fn shared_audit_replay_drops_externally_pruned_cache_before_command_key_reuse() {
    let root = JournalPath::new();
    let path = root.0.join("audit.sqlite3");
    let mut owner = Vm::open_journal(&path, 500).unwrap();
    let oldest = contract::focus(&mut owner);
    let retained = contract::focus(&mut owner);
    let pending = contract::focus(&mut owner);
    let mut observer = Vm::open_journal(&path, 0).unwrap();
    let context = contract::audit("shared-retention");
    let original = owner
        .cancel_effect_audited(&oldest.continuation, 102, &context)
        .unwrap();
    assert_eq!(
        observer
            .cancel_effect_audited(&oldest.continuation, 103, &context)
            .unwrap(),
        original
    );
    owner.cancel_effect(&retained.continuation, 104);
    let report = owner
        .compact_journal(&RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 1,
        })
        .unwrap();
    assert_eq!(report.removed_records, 1);
    assert_eq!(observer.completed_count(), 1);
    assert_eq!(observer.debugger_audit(&context.command_id).unwrap(), None);
    assert_eq!(
        observer
            .cancel_effect_audited(&oldest.continuation, 200, &context)
            .unwrap_err()
            .code,
        "LSV2004"
    );
    assert_eq!(observer.completed_count(), 0);
    let replacement = owner
        .cancel_effect_audited(&pending.continuation, 201, &context)
        .unwrap();
    assert_eq!(replacement.observed_at_ms, 201);
    assert_eq!(
        observer
            .cancel_effect_audited(&oldest.continuation, 202, &context)
            .unwrap_err()
            .code,
        "LSV2004"
    );
    assert_eq!(
        observer
            .cancel_effect_audited(&pending.continuation, 203, &context)
            .unwrap(),
        replacement
    );
    assert_eq!(
        observer.debugger_audit(&context.command_id).unwrap(),
        Some(replacement)
    );
    let mut reopened = Vm::open_journal(&path, 0).unwrap();
    assert_eq!(
        reopened
            .cancel_effect_audited(&oldest.continuation, 204, &context)
            .unwrap_err()
            .code,
        "LSV2004"
    );
}

#[test]
fn shared_audit_races_keep_one_committed_timestamp_and_attribution() {
    for same_command in [true, false] {
        for _ in 0..4 {
            let root = JournalPath::new();
            let path = root.0.join("audit.sqlite3");
            let mut owner = Vm::open_journal(&path, 500).unwrap();
            let request = contract::focus(&mut owner);
            let mut left = Vm::open_journal(&path, 0).unwrap();
            let mut right = Vm::open_journal(&path, 0).unwrap();
            let contexts = [
                contract::audit("race-a"),
                contract::audit(if same_command { "race-a" } else { "race-b" }),
            ];
            let barrier = std::sync::Barrier::new(3);
            let image = &request.continuation;
            let results = std::thread::scope(|scope| {
                let left = scope.spawn(|| {
                    barrier.wait();
                    left.cancel_effect_audited(image, 200, &contexts[0])
                });
                let right = scope.spawn(|| {
                    barrier.wait();
                    right.cancel_effect_audited(image, 201, &contexts[1])
                });
                barrier.wait();
                [left.join().unwrap(), right.join().unwrap()]
            });
            let records = results
                .iter()
                .filter_map(|result| result.as_ref().ok())
                .collect::<Vec<_>>();
            assert_eq!(records.len(), if same_command { 2 } else { 1 });
            let winner = records[0];
            assert!(matches!(winner.observed_at_ms, 200 | 201));
            assert!(records.iter().all(|record| *record == winner));
            for (index, result) in results.iter().enumerate() {
                if let Err(error) = result {
                    assert_eq!(error.code, "LSV4033");
                    assert_eq!(
                        owner.debugger_audit(&contexts[index].command_id).unwrap(),
                        None
                    );
                }
            }
            assert_eq!(
                owner.debugger_audit(&winner.command_id).unwrap().as_ref(),
                Some(winner)
            );
            let Step::Cancelled(terminal) = owner.cancel_effect(image, 202) else {
                panic!("expected the committed cancellation");
            };
            assert_eq!(terminal.observed_at_ms, winner.observed_at_ms);
            let count: i64 = rusqlite::Connection::open(&path)
                .unwrap()
                .query_row("SELECT COUNT(*) FROM vm_debugger_audit", [], |row| {
                    row.get(0)
                })
                .unwrap();
            assert_eq!(count, 1);
            let mut reopened = Vm::open_journal(&path, 0).unwrap();
            assert_eq!(
                reopened
                    .debugger_audit(&winner.command_id)
                    .unwrap()
                    .as_ref(),
                Some(winner)
            );
            assert!(reopened.claim_effect(203, 10).unwrap().is_none());
        }
    }
}
