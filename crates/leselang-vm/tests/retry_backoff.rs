use leselang_hir::lower;
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_syntax::parse;
use leselang_vm::{
    EffectError, EffectErrorClass, EffectRequest, EffectResult, MAX_EFFECT_DEADLINE_MS,
    MAX_RETRY_DELAY_MS, MAX_SEMANTIC_RETRIES, PresentationResult, RetryDisposition, RetryPolicy,
    ScalarValue, Step, Value, Vm,
};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

struct JournalPath(PathBuf);

impl JournalPath {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        let root = std::env::temp_dir().join(format!(
            "leselang-retry-backoff-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        Self(root.join("flow.sqlite3"))
    }

    fn vm(&self, durable: bool, fuel: u64) -> Vm {
        if durable {
            Vm::open_journal(&self.0, fuel).unwrap()
        } else {
            Vm::new(fuel)
        }
    }
}

impl Drop for JournalPath {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(self.0.parent().unwrap());
    }
}

fn admit(vm: &mut Vm) -> EffectRequest {
    let Step::Effect(request) = vm.start_timed(
        &lower(&parse(
            r#"fn main() = bind(result: ui.focus(node_id: "node"), body: true)"#,
        ))
        .unwrap(),
        Principal::new("operator").unwrap(),
        CapabilitySet::new(["ui.presentation"]),
        Some(Revision(7)),
        100,
        MAX_EFFECT_DEADLINE_MS,
    ) else {
        panic!("expected a pending effect");
    };
    *request
}

fn host_error() -> EffectError {
    EffectError {
        class: EffectErrorClass::Transient,
        code: "host_busy".into(),
        message: "temporary host failure".into(),
    }
}

fn receipt() -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: "node".into(),
    })
}

#[test]
fn every_retry_preserves_exact_delays_saved_fuel_and_deadline_across_recovery() {
    for durable in [false, true] {
        for policy in [
            RetryPolicy::default(),
            RetryPolicy {
                max_retries: MAX_SEMANTIC_RETRIES,
                base_delay_ms: 1,
                max_delay_ms: 1,
            },
            RetryPolicy {
                max_retries: MAX_SEMANTIC_RETRIES,
                base_delay_ms: 7,
                max_delay_ms: 25,
            },
            RetryPolicy {
                max_retries: MAX_SEMANTIC_RETRIES,
                base_delay_ms: MAX_RETRY_DELAY_MS,
                max_delay_ms: MAX_RETRY_DELAY_MS,
            },
        ] {
            let path = JournalPath::new();
            let mut vm = path.vm(durable, 100);
            let request = admit(&mut vm);
            let mut now_ms = 101;
            for retry_count in 1..=policy.max_retries {
                let lease = vm.claim_effect(now_ms, 10).unwrap().unwrap();
                assert_eq!(lease.request, request);
                assert_eq!(lease.attempt, retry_count);
                assert_eq!(lease.retry_count, retry_count - 1);
                let reported_at_ms = now_ms + 1;
                let RetryDisposition::Scheduled(schedule) = vm
                    .report_effect_error(&lease, reported_at_ms, host_error(), &policy)
                    .unwrap()
                else {
                    panic!("expected a scheduled retry");
                };
                let expected_delay =
                    (policy.base_delay_ms * (1_u64 << (retry_count - 1))).min(policy.max_delay_ms);
                assert_eq!(schedule.retry_count, retry_count);
                assert_eq!(schedule.ready_at_ms, reported_at_ms + expected_delay);
                assert_eq!(schedule.error, host_error());
                assert_eq!(vm.completed_count(), 0);
                assert_eq!(
                    vm.pending_continuations(),
                    std::slice::from_ref(&request.continuation)
                );
                assert_eq!(
                    vm.scheduler_pressure(reported_at_ms).unwrap().active_leases,
                    0
                );
                assert!(
                    vm.claim_effect(schedule.ready_at_ms - 1, 1)
                        .unwrap()
                        .is_none()
                );
                let stale = vm.acknowledge_effect(&lease, reported_at_ms, receipt());
                assert!(
                    matches!(stale, Step::Fault(ref error) if error.code == "LSV4022"),
                    "{stale:?}"
                );
                if durable {
                    drop(vm);
                    // A worker with no default fuel still resumes the original saved grant.
                    vm = path.vm(true, 0);
                    assert_eq!(
                        vm.pending_continuations(),
                        std::slice::from_ref(&request.continuation)
                    );
                    assert_eq!(vm.completed_count(), 0);
                }
                now_ms = schedule.ready_at_ms;
            }
            let lease = vm.claim_effect(now_ms, 10).unwrap().unwrap();
            assert_eq!(lease.request, request);
            assert_eq!(lease.retry_count, policy.max_retries);
            assert_eq!(lease.attempt, policy.max_retries + 1);
            let done = Step::Done(Value::Scalar {
                value: ScalarValue::Boolean(true),
            });
            assert_eq!(vm.acknowledge_effect(&lease, now_ms + 1, receipt()), done);
            if durable {
                drop(vm);
                vm = path.vm(true, 0);
            }
            assert_eq!(vm.acknowledge_effect(&lease, now_ms + 2, receipt()), done);
            assert_eq!(vm.pending_count(), 0);
        }
    }
}

#[test]
fn semantic_retry_limit_does_not_count_lease_redelivery_or_admission_attempts() {
    let policy = RetryPolicy {
        max_retries: 0,
        ..RetryPolicy::default()
    };
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 100);
        let request = admit(&mut vm);
        for now_ms in [101, 102] {
            let lease = vm.claim_effect(now_ms, 1).unwrap().unwrap();
            assert_eq!(lease.retry_count, 0);
            assert_eq!(lease.request, request);
        }
        if durable {
            drop(vm);
            vm = path.vm(true, 0);
        }
        let lease = vm.claim_effect(103, 10).unwrap().unwrap();
        assert_eq!(lease.attempt, 3);
        assert_eq!(lease.retry_count, 0);
        let terminal = vm
            .report_effect_error(&lease, 104, host_error(), &policy)
            .unwrap();
        assert!(matches!(
            &terminal,
            RetryDisposition::Terminal(Step::Failed(failure))
                if failure.retry_count == 0 && failure.error == host_error()
        ));
        assert_eq!(vm.pending_count(), 0);
        if durable {
            drop(vm);
            vm = path.vm(true, 0);
        }
        assert_eq!(
            vm.report_effect_error(&lease, 105, host_error(), &policy)
                .unwrap(),
            terminal
        );
        assert!(vm.claim_effect(105, 10).unwrap().is_none());
    }
}

#[test]
fn shared_arithmetic_does_not_bypass_retry_policy_or_classification_checks() {
    for durable in [false, true] {
        let path = JournalPath::new();
        let mut vm = path.vm(durable, 100);
        let request = admit(&mut vm);
        let lease = vm.claim_effect(101, 100).unwrap().unwrap();
        for invalid in [
            RetryPolicy {
                base_delay_ms: 0,
                ..RetryPolicy::default()
            },
            RetryPolicy {
                base_delay_ms: 10,
                max_delay_ms: 1,
                ..RetryPolicy::default()
            },
            RetryPolicy {
                max_retries: u32::MAX,
                ..RetryPolicy::default()
            },
            RetryPolicy {
                max_delay_ms: u64::MAX,
                ..RetryPolicy::default()
            },
        ] {
            assert_eq!(
                vm.report_effect_error(&lease, 102, host_error(), &invalid)
                    .unwrap_err()
                    .code,
                "LSV2201"
            );
            assert_eq!(
                vm.pending_continuations(),
                std::slice::from_ref(&request.continuation)
            );
            assert_eq!(vm.completed_count(), 0);
        }
        let permanent = EffectError {
            class: EffectErrorClass::Permanent,
            ..host_error()
        };
        let invalid = RetryPolicy {
            max_retries: u32::MAX,
            base_delay_ms: 0,
            max_delay_ms: 0,
        };
        let terminal = vm
            .report_effect_error(&lease, 102, permanent.clone(), &invalid)
            .unwrap();
        assert!(
            matches!(terminal, RetryDisposition::Terminal(Step::Failed(failure)) if failure.error == permanent && failure.retry_count == 0)
        );
        assert_eq!(vm.pending_count(), 0);
    }
}
