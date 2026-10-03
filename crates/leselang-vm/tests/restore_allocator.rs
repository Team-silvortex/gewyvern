use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectOperation, EffectRequest, EffectResult, RetentionPolicy, SchedulerLimits, Step, Vm,
};
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct JournalPath(PathBuf);
impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-restore-allocator-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root.join("flow.sqlite3"))
    }
    fn vm(&self) -> Vm {
        Vm::open_journal(&self.0, 10000).unwrap()
    }
    fn connection(&self) -> Connection {
        Connection::open(&self.0).unwrap()
    }
    fn next_sequence(&self) -> i64 {
        self.connection()
            .query_row("SELECT next_sequence FROM vm_metadata", [], |row| {
                row.get(0)
            })
            .unwrap()
    }
    fn rewind_metadata(&self) {
        self.set_next_sequence(1);
    }
    fn set_next_sequence(&self, sequence: i64) {
        self.connection()
            .execute("UPDATE vm_metadata SET next_sequence = ?1", [sequence])
            .unwrap();
    }
}
impl Drop for JournalPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}
fn start(vm: &mut Vm, source: &str) -> Step {
    vm.start_timed(
        &lower(&parse(source)).unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        100,
        1000,
    )
}
fn focus(vm: &mut Vm) -> Step {
    start(
        vm,
        r#"fn main() = bind(result: ui.focus(node_id: "target"), body: true)"#,
    )
}
fn effect(step: Step) -> EffectRequest {
    let Step::Effect(request) = step else {
        panic!("{step:?}")
    };
    *request
}
fn request_at(sequence: u64) -> EffectRequest {
    let mut request = effect(focus(&mut Vm::new(10000)));
    request.effect_id = format!("effect-{sequence}");
    request.continuation.token =
        serde_json::from_value(serde_json::json!(format!("continuation-{sequence}"))).unwrap();
    request
}
fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}
fn group_with_cold_tail() -> &'static str {
    r#"fn main() = bind(group: seq(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b")), body: bind(tail: ui.focus(node_id: "c"), body: bind(end: ui.focus(node_id: "d"), body: true)))"#
}

#[test]
fn imported_requests_advance_the_shared_allocator_for_already_open_workers() {
    let path = JournalPath::new();
    let mut importer = path.vm();
    let mut stale = path.vm();
    importer.restore_request(request_at(10)).unwrap();
    assert_eq!(path.next_sequence(), 11);
    assert_eq!(effect(focus(&mut stale)).effect_id, "effect-11");
}

#[test]
fn bare_and_duplicate_imports_preserve_the_durable_high_water_mark() {
    for bare in [false, true] {
        let path = JournalPath::new();
        let mut importer = path.vm();
        let mut stale = path.vm();
        let request = request_at(4);
        if bare {
            importer.restore(request.continuation.clone()).unwrap();
        } else {
            importer.restore_request(request.clone()).unwrap();
        }
        assert_eq!(path.next_sequence(), 5);
        path.rewind_metadata();
        if bare {
            importer.restore(request.continuation).unwrap();
        } else {
            importer.restore_request(request).unwrap();
        }
        assert_eq!(path.next_sequence(), 5);
        assert_eq!(effect(focus(&mut stale)).effect_id, "effect-5");
    }
}

#[test]
fn compaction_cannot_reuse_the_identity_of_a_completed_import() {
    let path = JournalPath::new();
    let mut importer = path.vm();
    let mut stale = path.vm();
    importer.restore_request(request_at(1)).unwrap();
    let lease = importer.claim_effect(101, 50).unwrap().unwrap();
    assert!(matches!(
        importer.acknowledge_effect(&lease, 102, receipt(&lease.request)),
        Step::Done(_)
    ));
    importer.restore_request(request_at(2)).unwrap();
    let lease = importer.claim_effect(103, 50).unwrap().unwrap();
    assert!(matches!(
        importer.acknowledge_effect(&lease, 104, receipt(&lease.request)),
        Step::Done(_)
    ));
    let report = importer
        .compact_journal(&RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 100,
        })
        .unwrap();
    assert_eq!(report.removed_records, 1);
    assert_eq!(effect(focus(&mut stale)).effect_id, "effect-3");
}

#[test]
fn opening_a_legacy_journal_repairs_both_pending_and_completed_identities() {
    for completed in [false, true] {
        let path = JournalPath::new();
        let mut importer = path.vm();
        let mut stale = path.vm();
        let request = request_at(20);
        importer.restore_request(request.clone()).unwrap();
        if completed {
            assert!(matches!(
                importer.cancel_effect(&request.continuation, 101),
                Step::Cancelled(_)
            ));
        }
        path.rewind_metadata();
        let repaired = path.vm();
        assert_eq!(repaired.pending_count(), usize::from(!completed));
        assert_eq!(path.next_sequence(), 21);
        assert_eq!(effect(focus(&mut stale)).effect_id, "effect-21");
    }
}

#[test]
fn startup_repairs_imported_ids_without_changing_cold_group_reentry() {
    let path = JournalPath::new();
    let mut original = path.vm();
    let mut stale = path.vm();
    let first = effect(start(&mut original, group_with_cold_tail()));
    let merge_token = first.continuation.group_result.as_ref().unwrap();
    assert_eq!(path.next_sequence(), 6);
    let imported = request_at(20);
    original.restore_request(imported.clone()).unwrap();
    // Older restore writes left the correctly allocated group watermark at six.
    path.set_next_sequence(6);
    let mut repaired = path.vm();
    assert_eq!(path.next_sequence(), 21);
    assert!(matches!(
        repaired.cancel_effect(&imported.continuation, 101),
        Step::Cancelled(_)
    ));
    let unrelated = effect(focus(&mut stale));
    assert_eq!(unrelated.effect_id, "effect-21");
    stale.cancel_effect(&unrelated.continuation, 101);
    for (position, sequence) in [2, 3, 4, 5].into_iter().enumerate() {
        let now = 102 + position as u64 * 2;
        let lease = repaired.claim_effect(now, 50).unwrap().unwrap();
        assert_eq!(lease.request.effect_id, format!("effect-{sequence}"));
        let step = repaired.acknowledge_effect(&lease, now + 1, receipt(&lease.request));
        assert!(!matches!(step, Step::Fault(_)), "{step:?}");
    }
    assert!(matches!(
        original.merge_result(merge_token).unwrap(),
        Some(Step::Done(_))
    ));
    assert_eq!(path.next_sequence(), 22);
}

#[test]
fn cancelled_groups_still_reserve_their_unused_cold_tail_identities() {
    let path = JournalPath::new();
    let mut original = path.vm();
    let mut stale = path.vm();
    let first = effect(start(&mut original, group_with_cold_tail()));
    assert!(matches!(
        original.cancel_effect(&first.continuation, 101),
        Step::Cancelled(_)
    ));
    let imported = request_at(20);
    original.restore_request(imported.clone()).unwrap();
    assert!(matches!(
        original.cancel_effect(&imported.continuation, 101),
        Step::Cancelled(_)
    ));
    path.set_next_sequence(6);
    assert_eq!(path.vm().pending_count(), 0);
    assert_eq!(path.next_sequence(), 21);
    assert_eq!(effect(focus(&mut stale)).effect_id, "effect-21");
}

#[test]
fn lower_imports_and_restarts_never_reduce_an_existing_allocator_gap() {
    let path = JournalPath::new();
    let mut importer = path.vm();
    path.connection()
        .execute("UPDATE vm_metadata SET next_sequence = 100", [])
        .unwrap();
    importer.restore_request(request_at(4)).unwrap();
    importer.restore_request(request_at(4)).unwrap();
    assert_eq!(path.next_sequence(), 100);
    let mut reopened = path.vm();
    assert_eq!(effect(focus(&mut reopened)).effect_id, "effect-100");
}

#[test]
fn late_validation_failure_leaves_a_lagging_watermark_unchanged() {
    let path = JournalPath::new();
    let mut original = path.vm();
    effect(start(&mut original, group_with_cold_tail()));
    original.restore_request(request_at(20)).unwrap();
    path.set_next_sequence(6);
    path.connection()
        .execute_batch("DROP INDEX vm_debugger_audit_token_idx;")
        .unwrap();
    assert_eq!(
        Vm::open_journal(&path.0, 10000).err().unwrap().code,
        "LSV4007"
    );
    assert_eq!(path.next_sequence(), 6);
    path.connection()
        .execute_batch(
            "CREATE INDEX vm_debugger_audit_token_idx ON vm_debugger_audit(continuation_token);",
        )
        .unwrap();
    assert_eq!(path.vm().pending_count(), 3);
    assert_eq!(path.next_sequence(), 21);
}

#[test]
fn high_imported_ids_cannot_launder_unallocated_cold_reservations() {
    let path = JournalPath::new();
    let mut original = path.vm();
    effect(start(&mut original, group_with_cold_tail()));
    original.restore_request(request_at(20)).unwrap();
    path.set_next_sequence(6);
    let connection = path.connection();
    let bytes: Vec<u8> = connection
        .query_row("SELECT plan FROM vm_merge_groups", [], |row| row.get(0))
        .unwrap();
    let mut plan: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    plan["result_binding"]["successor_sequence"] = 7.into();
    plan["result_binding"]["additional_successor_sequences"][0] = 8.into();
    connection
        .execute(
            "UPDATE vm_merge_groups SET plan = ?1",
            [serde_json::to_vec(&plan).unwrap()],
        )
        .unwrap();
    assert_eq!(
        Vm::open_journal(&path.0, 10000).err().unwrap().code,
        "LSV1409"
    );
    assert_eq!(path.next_sequence(), 6);
}

#[test]
fn concurrent_restarts_observe_whole_groups_during_successor_publication() {
    let path = JournalPath::new();
    let mut writer = path.vm();
    let barrier = std::sync::Barrier::new(3);
    std::thread::scope(|scope| {
        let write_barrier = &barrier;
        let writer_handle = scope.spawn(move || {
            write_barrier.wait();
            for _ in 0..16 {
                effect(start(&mut writer, group_with_cold_tail()));
                for offset in 0..4 {
                    let lease = writer.claim_effect(101 + offset * 2, 50).unwrap().unwrap();
                    let step = writer.acknowledge_effect(
                        &lease,
                        102 + offset * 2,
                        receipt(&lease.request),
                    );
                    assert!(!matches!(step, Step::Fault(_)), "{step:?}");
                }
            }
        });
        let readers = (0..2)
            .map(|_| {
                let path = &path;
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    for _ in 0..24 {
                        path.vm();
                        std::thread::yield_now();
                    }
                })
            })
            .collect::<Vec<_>>();
        writer_handle.join().unwrap();
        for reader in readers {
            reader.join().unwrap();
        }
    });
    assert_eq!(path.vm().pending_count(), 0);
    assert_eq!(path.next_sequence(), 81);
}

#[test]
fn rejected_imports_do_not_advance_the_watermark() {
    let path = JournalPath::new();
    let mut importer = Vm::open_journal_with_limits(
        &path.0,
        10000,
        SchedulerLimits {
            max_pending_dispatches: 1,
            max_active_leases: 1,
        },
    )
    .unwrap();
    importer.restore_request(request_at(1)).unwrap();
    let before = path.next_sequence();
    assert_eq!(
        importer.restore_request(request_at(100)).unwrap_err().code,
        "LSV2501"
    );
    assert_eq!(path.next_sequence(), before);
    let mut conflicting = request_at(1);
    conflicting.continuation.fuel_remaining -= 1;
    conflicting.budget.fuel_remaining -= 1;
    assert_eq!(
        importer.restore_request(conflicting).unwrap_err().code,
        "LSV2002"
    );
    assert_eq!(path.next_sequence(), before);
}

#[test]
fn failed_watermark_write_rolls_back_the_import_and_outbox() {
    let path = JournalPath::new();
    let mut importer = path.vm();
    path.connection()
        .execute_batch(
            "CREATE TRIGGER reject_watermark BEFORE UPDATE ON vm_metadata
         WHEN NEW.next_sequence > OLD.next_sequence
         BEGIN SELECT RAISE(ABORT, 'injected watermark failure'); END;",
        )
        .unwrap();
    assert_eq!(
        importer.restore_request(request_at(3)).unwrap_err().code,
        "LSV4008"
    );
    assert_eq!(importer.pending_count(), 0);
    assert_eq!(path.next_sequence(), 1);
    for table in ["vm_effects", "vm_dispatches"] {
        assert_eq!(
            path.connection()
                .query_row(&format!("SELECT COUNT(*) FROM {table}"), [], |row| row
                    .get::<_, i64>(0))
                .unwrap(),
            0
        );
    }
    path.connection()
        .execute_batch("DROP TRIGGER reject_watermark;")
        .unwrap();
    importer.restore_request(request_at(3)).unwrap();
    assert_eq!(path.next_sequence(), 4);
}

#[test]
fn invalid_journal_does_not_commit_a_partial_watermark_repair() {
    let path = JournalPath::new();
    let mut importer = path.vm();
    importer.restore_request(request_at(10)).unwrap();
    path.rewind_metadata();
    path.connection()
        .execute("UPDATE vm_dispatches SET request = ?1", [b"{}".as_slice()])
        .unwrap();
    assert!(Vm::open_journal(&path.0, 10000).is_err());
    assert_eq!(path.next_sequence(), 1);
}

#[test]
fn imported_maximum_identity_exhausts_instead_of_wrapping_or_reusing() {
    let path = JournalPath::new();
    let mut importer = path.vm();
    let mut stale = path.vm();
    importer
        .restore_request(request_at(i64::MAX as u64 - 1))
        .unwrap();
    assert_eq!(path.next_sequence(), i64::MAX);
    assert!(matches!(focus(&mut stale), Step::Fault(ref fault) if fault.code == "LSV4009"));
    assert_eq!(path.next_sequence(), i64::MAX);
    assert_eq!(path.vm().pending_count(), 1);
}

#[test]
fn concurrent_imports_publish_one_monotonic_watermark_to_stale_workers() {
    let path = JournalPath::new();
    let importers = (0..4).map(|_| path.vm()).collect::<Vec<_>>();
    let workers = (0..4).map(|_| path.vm()).collect::<Vec<_>>();
    let barrier = std::sync::Barrier::new(4);
    std::thread::scope(|scope| {
        let handles = importers
            .into_iter()
            .enumerate()
            .map(|(index, mut vm)| {
                let barrier = &barrier;
                scope.spawn(move || {
                    barrier.wait();
                    vm.restore_request(request_at(100 + index as u64)).unwrap();
                })
            })
            .collect::<Vec<_>>();
        for handle in handles {
            handle.join().unwrap();
        }
    });
    assert_eq!(path.next_sequence(), 104);
    let mut ids = std::thread::scope(|scope| {
        workers
            .into_iter()
            .map(|mut vm| scope.spawn(move || effect(focus(&mut vm)).effect_id))
            .collect::<Vec<_>>()
            .into_iter()
            .map(|handle| handle.join().unwrap())
            .collect::<Vec<_>>()
    });
    ids.sort();
    assert_eq!(
        ids,
        ["effect-104", "effect-105", "effect-106", "effect-107"]
    );
    assert_eq!(path.next_sequence(), 108);
}
