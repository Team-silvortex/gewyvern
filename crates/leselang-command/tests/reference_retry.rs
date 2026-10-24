use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::reference_retry::{self, ReferenceRetryHost};
use leselang_command::{
    ContinuationToken, DispatchLease, EffectError, EffectErrorClass, RetryDisposition, RetryPolicy,
    Step, Value,
};
use leselang_hir::{Effect, Type};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;
use std::{cell::RefCell, rc::Rc};

fn lease() -> DispatchLease {
    DispatchLease {
        request: build_effect_request(ReservedEffectInput {
            effect: &Effect::UiFocus {
                node_id: "node".into(),
            },
            result_type: Type::UiFocus,
            sequence: 7,
            principal: Principal::new("operator").unwrap(),
            capabilities: CapabilitySet::new(["ui.presentation"]),
            expected_revision: None,
            deadline_ms: 1000,
            deadline_at_ms: None,
            fuel_remaining: 100,
        })
        .unwrap(),
        attempt: 1,
        retry_count: 0,
        lease_expires_at_ms: 101,
    }
}
fn error() -> EffectError {
    EffectError {
        class: EffectErrorClass::Transient,
        code: "host_busy".into(),
        message: "original host error".into(),
    }
}

#[derive(Default)]
struct Host {
    trace: Rc<RefCell<Vec<&'static str>>>,
    replay: Option<Step>,
    winner: Option<RetryDisposition>,
    proposal: Option<RetryDisposition>,
    lease_pointer: Option<*const DispatchLease>,
    message_pointer: Option<*const u8>,
    failure: Option<Fault>,
    fail_at: Option<&'static str>,
    unwind_at: Option<&'static str>,
}
impl Host {
    fn enter(&mut self, stage: &'static str) -> Result<(), Fault> {
        self.trace.borrow_mut().push(stage);
        assert_ne!(self.unwind_at, Some(stage), "trusted owner unwind");
        if self.fail_at == Some(stage) {
            return Err(self.failure.take().unwrap());
        }
        Ok(())
    }
}
impl ReferenceRetryHost for Host {
    fn expire_due(&mut self, _: u64) -> Result<(), Fault> {
        self.enter("expire")
    }
    fn replay_completed(&mut self, _: &ContinuationToken) -> Result<Option<Step>, Fault> {
        self.enter("replay")?;
        Ok(self.replay.take())
    }
    fn visible_step(&mut self, _: &ContinuationToken, completed: Step) -> Step {
        if let Err(error) = self.enter("visible") {
            return Step::Fault(error);
        }
        completed
    }
    fn settle_error(
        &mut self,
        lease: &DispatchLease,
        now: u64,
        proposal: &RetryDisposition,
    ) -> Result<RetryDisposition, Fault> {
        self.enter("settle")?;
        assert_eq!(now, 12);
        if let Some(pointer) = self.lease_pointer {
            assert_eq!(std::ptr::from_ref(lease), pointer);
        }
        let error = match proposal {
            RetryDisposition::Scheduled(schedule) => &schedule.error,
            RetryDisposition::Terminal(Step::Failed(failure)) => &failure.error,
            _ => panic!("unexpected proposal"),
        };
        if let Some(pointer) = self.message_pointer {
            assert_eq!(error.message.as_ptr(), pointer);
        }
        self.proposal = Some(proposal.clone());
        Ok(self.winner.take().unwrap_or_else(|| proposal.clone()))
    }
}

#[test]
fn pending_retry_keeps_original_lease_and_error_buffers_and_native_winner() {
    for route in 0..3 {
        let mut lease = lease();
        let mut error = error();
        if route == 1 {
            error.class = EffectErrorClass::Permanent;
        }
        if route == 2 {
            lease.retry_count = 3;
        }
        let winner = "authoritative winner".to_owned();
        let winner_pointer = winner.as_ptr();
        let mut host = Host {
            lease_pointer: Some(&lease),
            message_pointer: Some(error.message.as_ptr()),
            winner: Some(RetryDisposition::Terminal(Step::Done(Value::UiFocus {
                node_id: winner,
            }))),
            ..Host::default()
        };
        let before = serde_json::to_vec(&lease).unwrap();
        let got = reference_retry::report_effect_error(
            &lease,
            12,
            error,
            &RetryPolicy::default(),
            &mut host as &mut dyn ReferenceRetryHost,
        )
        .unwrap();
        let RetryDisposition::Terminal(Step::Done(Value::UiFocus { node_id })) = got else {
            panic!()
        };
        assert_eq!(node_id.as_ptr(), winner_pointer);
        assert_eq!(serde_json::to_vec(&lease).unwrap(), before);
        match host.proposal.unwrap() {
            RetryDisposition::Scheduled(schedule) => {
                assert_eq!(route, 0);
                assert_eq!(schedule.retry_count, 1);
                assert_eq!(schedule.ready_at_ms, 262);
            }
            RetryDisposition::Terminal(Step::Failed(failure)) => {
                assert_eq!(failure.retry_count, lease.retry_count)
            }
            _ => panic!(),
        }
        assert_eq!(*host.trace.borrow(), ["expire", "replay", "settle"]);
    }
}

#[test]
fn replay_bypasses_late_metadata_but_clock_and_expiration_keep_precedence() {
    let mut lease = lease();
    lease.request.continuation.schema_version = 0;
    let mut error = error();
    error.code.clear();
    let policy = RetryPolicy {
        base_delay_ms: 0,
        ..RetryPolicy::default()
    };
    let payload = "original replay allocation".to_owned();
    let pointer = payload.as_ptr();
    let mut host = Host {
        replay: Some(Step::Done(Value::UiFocus { node_id: payload })),
        ..Host::default()
    };
    let got =
        reference_retry::report_effect_error(&lease, u64::MAX, error.clone(), &policy, &mut host);
    assert_eq!(
        got,
        Err(leselang_runtime_core::validate_clock(u64::MAX).unwrap_err())
    );
    assert!(host.trace.borrow().is_empty());
    let got = reference_retry::report_effect_error(&lease, 12, error, &policy, &mut host).unwrap();
    let RetryDisposition::Terminal(Step::Done(Value::UiFocus { node_id })) = got else {
        panic!()
    };
    assert_eq!(node_id.as_ptr(), pointer);
    assert_eq!(*host.trace.borrow(), ["expire", "replay", "visible"]);
}

#[test]
fn planner_refusals_precede_settlement_and_permanent_errors_bypass_policy() {
    let lease = lease();
    for case in 0..5 {
        let mut error = error();
        let mut policy = RetryPolicy::default();
        let mut now = 12;
        match case {
            0 => {
                error.code.clear();
                policy.base_delay_ms = 0;
            }
            1 => policy.base_delay_ms = 0,
            2 => now = leselang_runtime_core::MAX_CLOCK_MS,
            3 => {
                error.class = EffectErrorClass::Permanent;
                policy.base_delay_ms = 0;
            }
            _ => policy.max_retries = 0,
        }
        let mut host = Host::default();
        let got = reference_retry::report_effect_error(&lease, now, error, &policy, &mut host);
        if case < 3 {
            assert_eq!(
                got.unwrap_err().code,
                ["LSV2202", "LSV2201", "LSV2203"][case]
            );
            assert_eq!(*host.trace.borrow(), ["expire", "replay"]);
        } else {
            assert!(matches!(
                got,
                Ok(RetryDisposition::Terminal(Step::Failed(_)))
            ));
            assert_eq!(*host.trace.borrow(), ["expire", "replay", "settle"]);
        }
    }
}

#[test]
fn owner_failures_and_unwinds_are_once_only_and_visible_faults_stay_terminal() {
    let lease = lease();
    for stage in ["expire", "replay", "visible", "settle"] {
        for unwind in [false, true] {
            let failure = Fault {
                code: "owner-error".into(),
                message: "original owner failure".into(),
            };
            let pointer = failure.message.as_ptr();
            let mut host = Host {
                failure: Some(failure),
                fail_at: (!unwind).then_some(stage),
                unwind_at: unwind.then_some(stage),
                ..Host::default()
            };
            if stage == "visible" {
                host.replay = Some(Step::Done(Value::UiFocus {
                    node_id: "node".into(),
                }));
            }
            let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                reference_retry::report_effect_error(
                    &lease,
                    12,
                    error(),
                    &RetryPolicy::default(),
                    &mut host,
                )
            }));
            if unwind {
                assert!(got.is_err());
            } else {
                let got = got.unwrap();
                let failure = if stage == "visible" {
                    let Ok(RetryDisposition::Terminal(Step::Fault(error))) = got else {
                        panic!()
                    };
                    error
                } else {
                    got.unwrap_err()
                };
                assert_eq!(failure.message.as_ptr(), pointer);
            }
            let trace = host.trace.borrow();
            assert_eq!(trace.iter().filter(|s| **s == stage).count(), 1);
            assert_eq!(trace.last(), Some(&stage));
        }
    }
}

#[test]
fn diagnostic_mapping_keeps_exact_product_codes_and_messages() {
    use leselang_command::retry_policy::EffectRetryError;
    for (error, code, message) in [
        (
            EffectRetryError::Metadata,
            "LSV2202",
            "effect error metadata is invalid",
        ),
        (
            EffectRetryError::Policy,
            "LSV2201",
            "semantic retry policy exceeds runtime bounds",
        ),
        (
            EffectRetryError::Clock,
            "LSV2203",
            "semantic retry clock overflow",
        ),
    ] {
        assert_eq!(
            reference_retry::effect_retry_fault(error),
            Fault {
                code: code.into(),
                message: message.into()
            }
        );
    }
}
