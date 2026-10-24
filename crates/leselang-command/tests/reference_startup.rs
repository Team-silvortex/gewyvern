use std::cell::{Cell, RefCell};
use std::collections::BTreeMap;
use std::panic::{AssertUnwindSafe, catch_unwind};

use leselang_command::reference_request::MAX_EXECUTION_FUEL;
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::reference_startup::{self, RecoveredJournal};
use leselang_command::{ContinuationToken, SchedulerLimits, Step};
use leselang_hir::{Effect, Type};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;

struct Journal<'a>(&'a RefCell<Vec<&'static str>>);
impl Drop for Journal<'_> {
    fn drop(&mut self) {
        self.0.borrow_mut().push("drop-native");
    }
}
fn recovered<J>(journal: J) -> RecoveredJournal<J> {
    RecoveredJournal {
        next_sequence: 41,
        pending: BTreeMap::new(),
        completed: BTreeMap::new(),
        journal,
    }
}

#[test]
fn defaults_and_fuel_boundaries_construct_one_borrowed_native_owner() {
    for fuel in [
        0,
        1,
        999,
        MAX_EXECUTION_FUEL,
        MAX_EXECUTION_FUEL + 1,
        u64::MAX,
    ] {
        let trace = RefCell::new(Vec::new());
        let state = reference_startup::in_memory(fuel, || {
            trace.borrow_mut().push("create");
            Journal(&trace)
        });
        assert_eq!(state.next_sequence, 1);
        assert_eq!(state.fuel_limit, fuel.min(MAX_EXECUTION_FUEL));
        assert_eq!(state.scheduler_limits, SchedulerLimits::default());
        assert!(state.pending.is_empty() && state.completed.is_empty());
        assert_eq!(*trace.borrow(), ["create"]);
        drop(state);
        assert_eq!(*trace.borrow(), ["create", "drop-native"]);
    }
}

#[test]
fn policy_matrix_validates_before_either_native_hook() {
    let mut comparisons = 0;
    for pending in [0, 1, 2, 9_999, 10_000, 10_001, usize::MAX] {
        for active in [0, 1, 2, 9_999, 10_000, 10_001, usize::MAX] {
            let limits = SchedulerLimits {
                max_pending_dispatches: pending,
                max_active_leases: active,
            };
            let valid = (1..=10_000).contains(&pending) && (1..=10_000).contains(&active);
            for durable in [false, true] {
                let calls = Cell::new(0);
                let state = if durable {
                    reference_startup::recover(u64::MAX, limits, || {
                        calls.set(calls.get() + 1);
                        Ok(recovered(()))
                    })
                } else {
                    reference_startup::in_memory_with_limits(u64::MAX, limits, || {
                        calls.set(calls.get() + 1);
                    })
                };
                assert_eq!(calls.get(), usize::from(valid));
                match state {
                    Ok(state) => {
                        assert!(valid);
                        assert_eq!(state.scheduler_limits, limits);
                        assert_eq!(state.fuel_limit, MAX_EXECUTION_FUEL);
                    }
                    Err(error) => {
                        assert!(!valid);
                        assert_eq!(error, Fault {
                            code: "LSV2500".into(),
                            message: "scheduler limits must be between 1 and the journal record limit".into(),
                        });
                    }
                }
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 98);
}

#[test]
fn recovery_moves_original_buffers_and_keeps_saved_budgets_and_watermark() {
    let request = build_effect_request(ReservedEffectInput {
        effect: &Effect::UiFocus {
            node_id: "target".into(),
        },
        result_type: Type::UiFocus,
        sequence: 7,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: None,
        deadline_ms: 30_000,
        deadline_at_ms: Some(30_100),
        fuel_remaining: 73,
    })
    .unwrap();
    let trace = RefCell::new(Vec::new());
    let mut saved = recovered(Journal(&trace));
    // A trusted carrier is not a second watermark validator or recovery proof.
    saved.next_sequence = u64::MAX;
    saved
        .pending
        .insert(request.continuation.token.clone(), request.continuation);
    for i in 1..=3 {
        saved.completed.insert(
            ContinuationToken::from_wire(format!("continuation-{i}")),
            Step::Fault(Fault {
                code: "stored".into(),
                message: "owned stored result".into(),
            }),
        );
    }
    let before = serde_json::to_vec(&(&saved.pending, &saved.completed)).unwrap();
    let pending_ptr = std::ptr::from_ref(saved.pending.first_key_value().unwrap().1);
    let completed_ptr = std::ptr::from_ref(saved.completed.first_key_value().unwrap().1);
    let state = reference_startup::recover(
        0,
        SchedulerLimits {
            max_pending_dispatches: 1,
            max_active_leases: 1,
        },
        || {
            trace.borrow_mut().push("recover-committed");
            Ok(saved)
        },
    )
    .unwrap();
    assert_eq!(state.next_sequence, u64::MAX);
    assert_eq!(state.fuel_limit, 0);
    assert_eq!(
        serde_json::to_vec(&(&state.pending, &state.completed)).unwrap(),
        before
    );
    assert_eq!(
        std::ptr::from_ref(state.pending.first_key_value().unwrap().1),
        pending_ptr
    );
    assert_eq!(
        std::ptr::from_ref(state.completed.first_key_value().unwrap().1),
        completed_ptr
    );
    assert_eq!(*trace.borrow(), ["recover-committed"]);
    drop(state);
    assert_eq!(*trace.borrow(), ["recover-committed", "drop-native"]);
}

#[test]
fn native_fault_is_returned_without_replacement_retry_or_rollback() {
    let calls = Cell::new(0);
    let error = Fault {
        code: "native".into(),
        message: "original native open failure".into(),
    };
    let pointer = error.message.as_ptr();
    let result = reference_startup::recover::<()>(100, SchedulerLimits::default(), || {
        calls.set(calls.get() + 1);
        Err(error)
    });
    let Err(error) = result else {
        panic!("unexpected startup")
    };
    assert_eq!(error.message.as_ptr(), pointer);
    assert_eq!(error.code, "native");
    assert_eq!(calls.get(), 1);
}

#[test]
fn native_unwinds_are_not_caught_or_retried() {
    for durable in [false, true] {
        let calls = Cell::new(0);
        let trace = RefCell::new(Vec::new());
        let result = catch_unwind(AssertUnwindSafe(|| {
            if durable {
                let _ = reference_startup::recover::<()>(100, SchedulerLimits::default(), || {
                    calls.set(calls.get() + 1);
                    let _native = Journal(&trace);
                    panic!("native unwind");
                });
            } else {
                reference_startup::in_memory::<()>(100, || {
                    calls.set(calls.get() + 1);
                    let _native = Journal(&trace);
                    panic!("native unwind");
                });
            }
        }));
        assert!(result.is_err());
        assert_eq!(calls.get(), 1);
        assert_eq!(*trace.borrow(), ["drop-native"]);
    }
}

#[test]
fn rejected_policy_drops_owned_hook_captures_without_invoking_them() {
    let trace = RefCell::new(Vec::new());
    let native = Journal(&trace);
    let result = reference_startup::recover(
        0,
        SchedulerLimits {
            max_pending_dispatches: 0,
            max_active_leases: 1,
        },
        move || {
            native.0.borrow_mut().push("must-not-open");
            Ok(recovered(native))
        },
    );
    assert!(result.is_err());
    assert_eq!(*trace.borrow(), ["drop-native"]);
}
