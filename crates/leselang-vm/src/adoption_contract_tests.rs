use super::*;
use leselang_command::reference_restore::ReferenceRestoreCache;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let path = std::env::temp_dir().join(format!(
            "leselang-adoption-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn database(&self) -> PathBuf {
        self.0.join("journal.sqlite3")
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn request(sequence: u64) -> EffectRequest {
    Vm::build_effect_request(
        &Effect::UiFocus {
            node_id: "node".into(),
        },
        Type::UiFocus,
        sequence,
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        None,
        30_000,
        None,
        100,
    )
    .unwrap()
}
fn reply(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!()
    };
    EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}

#[test]
fn adoption_import_commit_failure_keeps_local_state_and_native_rows_unpublished() {
    let mut cases = 0;
    for full in [false, true] {
        for local_next in [1, 1000] {
            let scratch = Scratch::new();
            let path = scratch.database();
            let mut vm = Vm::open_journal(&path, 1).unwrap();
            vm.next_effect_id = local_next;
            let db = Connection::open(&path).unwrap();
            let native_next: i64 = db
                .query_row("SELECT next_sequence FROM vm_metadata", [], |r| r.get(0))
                .unwrap();
            db.execute_batch("CREATE TABLE failure_parent(id INTEGER PRIMARY KEY); CREATE TABLE failure_child(id INTEGER REFERENCES failure_parent(id) DEFERRABLE INITIALLY DEFERRED); CREATE TRIGGER reject_import AFTER INSERT ON vm_effects BEGIN INSERT INTO failure_child VALUES(1); END;").unwrap();
            let input = request(7);
            let result = if full {
                vm.restore_request(input.clone())
            } else {
                vm.restore(input.continuation.clone())
            };
            assert!(
                result.is_err(),
                "deferred foreign-key failure must reach COMMIT"
            );
            assert!(vm.pending.is_empty());
            assert!(vm.completed.is_empty());
            assert_eq!(vm.next_effect_id, local_next);
            for table in ["vm_effects", "vm_dispatches", "failure_child"] {
                assert_eq!(
                    db.query_row(&format!("SELECT count(*) FROM {table}"), [], |r| r
                        .get::<_, i64>(0))
                        .unwrap(),
                    0
                );
            }
            assert_eq!(
                db.query_row("SELECT next_sequence FROM vm_metadata", [], |r| r
                    .get::<_, i64>(0))
                    .unwrap(),
                native_next
            );
            db.execute_batch("DROP TRIGGER reject_import").unwrap();
            drop(db);
            drop(vm);
            let mut vm = Vm::open_journal(&path, 1).unwrap();
            assert!(vm.pending.is_empty());
            assert_eq!(vm.next_effect_id, u64::try_from(native_next).unwrap());
            vm.restore_request(input.clone()).unwrap();
            assert_eq!(vm.next_effect_id, 8);
            let lease = vm.claim_effect(1, 10).unwrap().unwrap();
            assert_eq!(lease.request, input);
            assert!(matches!(
                vm.acknowledge_effect(&lease, 2, reply(&input)),
                Step::Done(_)
            ));
            cases += 1;
        }
    }
    assert_eq!(cases, 4);
}

#[test]
fn adoption_import_rechecks_native_capacity_conflicts_and_terminal_peer_winners() {
    for race in 0..3 {
        let scratch = Scratch::new();
        let path = scratch.database();
        let limits = SchedulerLimits {
            max_pending_dispatches: 1,
            ..SchedulerLimits::default()
        };
        let mut vm = Vm::open_journal_with_limits(&path, 1, limits).unwrap();
        let mut peer = Vm::open_journal_with_limits(&path, 1, limits).unwrap();
        let input = request(7);
        let mut winner = if race == 0 {
            request(20)
        } else {
            input.clone()
        };
        if race == 2 {
            winner.continuation.fuel_remaining -= 1;
            winner.budget = winner.continuation.resource_budget();
        }
        let mut terminal = None;
        let mut calls = 0;
        let result = ReferenceRestoreCache {
            pending: &mut vm.pending,
            completed: &vm.completed,
            next_sequence: &mut vm.next_effect_id,
        }
        .restore_request(input.clone(), |image, authority| {
            calls += 1;
            // A real second owner commits after local preflight but before native import.
            peer.restore_request(winner.clone()).unwrap();
            if race == 1 {
                let lease = peer.claim_effect(1, 10).unwrap().unwrap();
                let step = peer.acknowledge_effect(&lease, 2, reply(&winner));
                assert!(matches!(step, Step::Done(_)));
                terminal = Some(step);
            }
            vm.journal.record_pending(image, authority, limits)
        });
        assert!(result.is_err(), "race={race}: {result:?}");
        assert_eq!(calls, 1);
        assert!(vm.pending.is_empty());
        assert!(vm.completed.is_empty());
        assert_eq!(vm.next_effect_id, 1);
        drop(vm);
        drop(peer);
        let mut reopened = Vm::open_journal_with_limits(&path, 1, limits).unwrap();
        if let Some(step) = terminal {
            assert!(reopened.pending.is_empty());
            assert_eq!(reopened.resume(&input.continuation, reply(&input)), step);
            assert!(reopened.claim_effect(3, 10).unwrap().is_none());
        } else {
            assert_eq!(reopened.pending.len(), 1);
            assert_eq!(
                reopened.pending.get(&winner.continuation.token),
                Some(&winner.continuation)
            );
            let lease = reopened.claim_effect(3, 10).unwrap().unwrap();
            assert_eq!(lease.request, winner);
            assert!(matches!(
                reopened.acknowledge_effect(&lease, 4, reply(&winner)),
                Step::Done(_)
            ));
        }
    }
}

#[test]
fn adoption_conflict_keeps_committed_lease_until_expiry_then_recovers_original_image() {
    let mut cases = 0;
    for structured in [false, true] {
        for corrupt_token in [false, true] {
            let scratch = Scratch::new();
            let path = scratch.database();
            let mut stale = Vm::open_journal(&path, 1).unwrap();
            let mut peer = Vm::open_journal(&path, 1).unwrap();
            let input = request(7);
            peer.restore_request(input.clone()).unwrap();
            let mut conflicting = input.continuation.clone();
            if corrupt_token {
                conflicting.token = ContinuationToken::from_wire("continuation-8".into());
            } else {
                conflicting.fuel_remaining -= 1;
            }
            stale
                .pending
                .insert(input.continuation.token.clone(), conflicting.clone());
            let pointer = stale.pending[&input.continuation.token]
                .token
                .as_str()
                .as_ptr();
            let error = if structured {
                stale.try_claim_effect(1, 10).unwrap_err()
            } else {
                stale.claim_effect(1, 10).unwrap_err()
            };
            assert_eq!(error.code, "LSV2005");
            assert_eq!(stale.next_effect_id, 1);
            assert!(stale.completed.is_empty());
            assert_eq!(stale.pending[&input.continuation.token], conflicting);
            assert_eq!(
                stale.pending[&input.continuation.token]
                    .token
                    .as_str()
                    .as_ptr(),
                pointer
            );
            assert_eq!(peer.scheduler_pressure(2).unwrap().active_leases, 1);
            assert!(peer.claim_effect(2, 10).unwrap().is_none());
            drop(stale);
            drop(peer);
            let mut reopened = Vm::open_journal(&path, 1).unwrap();
            assert_eq!(
                reopened.pending[&input.continuation.token],
                input.continuation
            );
            assert!(reopened.claim_effect(10, 10).unwrap().is_none());
            let lease = reopened.claim_effect(12, 10).unwrap().unwrap();
            assert_eq!(lease.attempt, 2);
            assert_eq!(lease.request, input);
            let step = reopened.acknowledge_effect(&lease, 13, reply(&input));
            assert!(matches!(step, Step::Done(_)));
            drop(reopened);
            let mut reopened = Vm::open_journal(&path, 1).unwrap();
            assert_eq!(reopened.resume(&input.continuation, reply(&input)), step);
            assert!(reopened.claim_effect(14, 10).unwrap().is_none());
            cases += 1;
        }
    }
    assert_eq!(cases, 4);
}
