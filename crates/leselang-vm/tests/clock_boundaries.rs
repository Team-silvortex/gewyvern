use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::MAX_CLOCK_MS;
use leselang_syntax::parse;
use leselang_vm::{
    CancellationReason, EffectError, EffectErrorClass, EffectOperation, EffectRequest,
    EffectResult, RetryPolicy, ScalarValue, Step, Value, Vm,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct JournalPath(PathBuf);

impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-clock-boundaries-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root.join("flow.sqlite3"))
    }

    fn vm(&self, durable: bool) -> Vm {
        if durable {
            Vm::open_journal(&self.0, 10_000).unwrap()
        } else {
            Vm::new(10_000)
        }
    }
}

impl Drop for JournalPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}

fn admit(vm: &mut Vm, timed: bool) -> EffectRequest {
    let program = lower(&parse(
        r#"fn main() = bind(result: ui.focus(node_id: "node"), body: true)"#,
    ))
    .unwrap();
    let principal = Principal::new("operator").unwrap();
    let capabilities = CapabilitySet::new(["ui.presentation"]);
    let step = if timed {
        vm.start_timed(
            &program,
            principal,
            capabilities,
            Some(Revision(7)),
            MAX_CLOCK_MS - 20,
            20,
        )
    } else {
        vm.start(&program, principal, capabilities, Some(Revision(7)))
    };
    let Step::Effect(request) = step else {
        panic!("expected a pending effect, got {step:?}");
    };
    *request
}

fn receipt(request: &EffectRequest) -> EffectResult {
    let EffectOperation::Presentation(envelope) = &request.operation else {
        panic!("expected a presentation");
    };
    EffectResult::Presentation(
        serde_json::from_value(serde_json::to_value(&envelope.operation).unwrap()).unwrap(),
    )
}

fn done() -> Step {
    Step::Done(Value::Scalar {
        value: ScalarValue::Boolean(true),
    })
}

fn transient_error() -> EffectError {
    EffectError {
        class: EffectErrorClass::Transient,
        code: "temporarily_unavailable".into(),
        message: "temporary host failure".into(),
    }
}

#[test]
fn readonly_pressure_accepts_the_full_clock_range_without_expiring_work() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable);
        let untimed = admit(&mut vm, false);
        admit(&mut vm, true);
        let lease = vm.claim_effect(MAX_CLOCK_MS - 10, 10).unwrap().unwrap();
        assert_eq!(lease.request, untimed);
        assert_eq!(lease.lease_expires_at_ms, MAX_CLOCK_MS);
        let before = vm.pending_continuations();

        for (now_ms, active_leases) in [(0, 1), (MAX_CLOCK_MS - 1, 1), (MAX_CLOCK_MS, 0)] {
            let pressure = vm.scheduler_pressure(now_ms).unwrap();
            assert_eq!(pressure.pending_dispatches, 2);
            assert_eq!(pressure.active_leases, active_leases);
            assert_eq!(vm.pending_continuations(), before);
            assert_eq!(vm.completed_count(), 0);
        }
        let error = vm.claim_effect(MAX_CLOCK_MS, 1).unwrap_err();
        assert_eq!(error.code, "LSV4015");
        assert_eq!(error.message, "dispatch lease expiration is out of range");
        assert_eq!(vm.pending_continuations(), before);
        assert_eq!(vm.completed_count(), 0);
        if durable {
            drop(vm);
            vm = path.vm(true);
            assert_eq!(vm.pending_continuations(), before);
            assert_eq!(
                vm.scheduler_pressure(MAX_CLOCK_MS)
                    .unwrap()
                    .pending_dispatches,
                2
            );
            assert_eq!(vm.completed_count(), 0);
        }
    }
}

#[test]
fn existing_lease_can_complete_at_the_max_clock_and_replay_after_recovery() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable);
        admit(&mut vm, false);
        let lease = vm.claim_effect(MAX_CLOCK_MS - 1, 1).unwrap().unwrap();
        if durable {
            drop(vm);
            vm = path.vm(true);
        }
        assert_eq!(
            vm.acknowledge_effect(&lease, MAX_CLOCK_MS, receipt(&lease.request)),
            done()
        );
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 1);
        if durable {
            drop(vm);
            vm = path.vm(true);
        }
        assert_eq!(
            vm.acknowledge_effect(&lease, MAX_CLOCK_MS, receipt(&lease.request)),
            done()
        );
        let pressure = vm.scheduler_pressure(MAX_CLOCK_MS).unwrap();
        assert_eq!(pressure.pending_dispatches, 0);
        assert_eq!(pressure.active_leases, 0);
    }
}

#[test]
fn lease_boundary_completion_requires_the_current_attempt() {
    for durable in [false, true] {
        for superseded in [false, true] {
            let path = JournalPath::new();
            let mut vm = path.vm(durable);
            let request = admit(&mut vm, false);
            let first = vm.claim_effect(100, 10).unwrap().unwrap();
            assert_eq!(vm.scheduler_pressure(110).unwrap().active_leases, 0);
            if superseded {
                let second = if durable {
                    let mut worker = path.vm(true);
                    worker.claim_effect(110, 10).unwrap().unwrap()
                } else {
                    vm.claim_effect(110, 10).unwrap().unwrap()
                };
                assert_eq!(second.attempt, first.attempt + 1);
                let rejected = vm.acknowledge_effect(&first, 110, receipt(&request));
                assert!(
                    matches!(
                        rejected,
                        Step::Fault(ref error) if error.code == "LSV4022"
                    ),
                    "{rejected:?}"
                );
                assert_eq!(vm.pending_count(), 1);
                assert_eq!(vm.completed_count(), 0);
                assert_eq!(
                    vm.acknowledge_effect(&second, 110, receipt(&request)),
                    done()
                );
            } else {
                assert_eq!(
                    vm.acknowledge_effect(&first, 110, receipt(&request)),
                    done()
                );
                assert!(vm.claim_effect(110, 10).unwrap().is_none());
            }
            if durable {
                drop(vm);
                vm = path.vm(true);
            }
            assert_eq!(vm.pending_count(), 0);
            assert_eq!(vm.completed_count(), 1);
        }
    }
}

#[test]
fn expired_lease_at_the_max_clock_stays_fenced_and_can_be_cancelled() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable);
        let request = admit(&mut vm, false);
        let lease = vm.claim_effect(MAX_CLOCK_MS - 2, 1).unwrap().unwrap();
        let before = vm.pending_continuations();
        let step = vm.acknowledge_effect(&lease, MAX_CLOCK_MS, receipt(&request));
        assert!(
            matches!(step, Step::Fault(ref error) if error.code == "LSV4023"),
            "{step:?}"
        );
        assert_eq!(vm.pending_continuations(), before);
        assert_eq!(vm.completed_count(), 0);
        let cancelled = vm.cancel_effect(&request.continuation, MAX_CLOCK_MS);
        assert!(matches!(
            &cancelled,
            Step::Cancelled(cancellation)
                if cancellation.reason == CancellationReason::Requested
                    && cancellation.observed_at_ms == MAX_CLOCK_MS
        ));
        if durable {
            drop(vm);
            vm = path.vm(true);
        }
        assert_eq!(
            vm.acknowledge_effect(&lease, MAX_CLOCK_MS, receipt(&request)),
            cancelled
        );
    }
}

#[test]
fn max_clock_execution_deadline_precedes_completion_and_requested_cancellation() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable);
        let request = admit(&mut vm, true);
        let lease = vm.claim_effect(MAX_CLOCK_MS - 1, 1).unwrap().unwrap();
        let expired = vm.acknowledge_effect(&lease, MAX_CLOCK_MS, receipt(&request));
        assert!(matches!(
            &expired,
            Step::Cancelled(cancellation)
                if cancellation.reason == CancellationReason::DeadlineExceeded
                    && cancellation.observed_at_ms == MAX_CLOCK_MS
        ));
        if durable {
            drop(vm);
            vm = path.vm(true);
        }
        assert_eq!(
            vm.cancel_effect(&request.continuation, MAX_CLOCK_MS),
            expired
        );
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 1);
    }
}

#[test]
fn max_clock_retry_overflow_preserves_the_existing_lease_for_completion() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable);
        let request = admit(&mut vm, false);
        let lease = vm.claim_effect(MAX_CLOCK_MS - 1, 1).unwrap().unwrap();
        let before = vm.pending_continuations();
        let error = vm
            .report_effect_error(
                &lease,
                MAX_CLOCK_MS,
                transient_error(),
                &RetryPolicy::default(),
            )
            .unwrap_err();
        assert_eq!(error.code, "LSV2203");
        assert_eq!(error.message, "semantic retry clock overflow");
        assert_eq!(vm.pending_continuations(), before);
        assert_eq!(vm.completed_count(), 0);
        if durable {
            drop(vm);
            vm = path.vm(true);
            assert_eq!(vm.pending_continuations(), before);
            let stored = rusqlite::Connection::open(&path.0)
                .unwrap()
                .query_row(
                    "SELECT state, attempt, retry_count, lease_expires_at_ms FROM vm_dispatches WHERE token = ?1",
                    [request.continuation.token.as_str()],
                    |row| Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)?, row.get::<_, u32>(2)?, row.get::<_, i64>(3)?)),
                )
                .unwrap();
            assert_eq!(stored, ("leased".into(), 1, 0, MAX_CLOCK_MS as i64));
        }
        assert_eq!(
            vm.acknowledge_effect(&lease, MAX_CLOCK_MS, receipt(&request)),
            done()
        );
    }
}

#[test]
fn invalid_observation_and_completion_clocks_leave_pending_state_unchanged() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable);
        let request = admit(&mut vm, false);
        let lease = vm.claim_effect(MAX_CLOCK_MS - 1, 1).unwrap().unwrap();
        let before = vm.pending_continuations();
        for now_ms in [MAX_CLOCK_MS + 1, u64::MAX] {
            let error = vm.scheduler_pressure(now_ms).unwrap_err();
            assert_eq!(error.code, "LSV4015");
            assert_eq!(error.message, "dispatch clock is out of range");
            for step in [
                vm.acknowledge_effect(&lease, now_ms, receipt(&request)),
                vm.cancel_effect(&request.continuation, now_ms),
            ] {
                assert!(
                    matches!(step, Step::Fault(ref error) if error.code == "LSV2011"),
                    "{step:?}"
                );
            }
            assert_eq!(
                vm.report_effect_error(&lease, now_ms, transient_error(), &RetryPolicy::default())
                    .unwrap_err()
                    .code,
                "LSV2011"
            );
            assert_eq!(vm.pending_continuations(), before);
            assert_eq!(vm.completed_count(), 0);
        }
        if durable {
            drop(vm);
            vm = path.vm(true);
            assert_eq!(vm.pending_continuations(), before);
        }
        assert_eq!(
            vm.acknowledge_effect(&lease, MAX_CLOCK_MS, receipt(&request)),
            done()
        );
    }
}
