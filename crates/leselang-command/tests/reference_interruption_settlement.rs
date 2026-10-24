use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::reference_settlement::interruption::{
    self, ReferenceCancellationSettlementHost, ReferenceRetrySettlementHost,
};
use leselang_command::{
    Cancellation, CancellationReason, ContinuationImage, DebuggerAuditContext, DebuggerAuditRecord,
    DispatchLease, EffectError, EffectErrorClass, RetryDisposition, RetrySchedule, Step, Value,
};
use leselang_hir::{Effect, Type};
use leselang_host_contract::{CapabilitySet, CommandOrigin, Principal, Revision};
use leselang_runtime_core::Fault;
use leserpent_domain::{CommandId, IdempotencyKey};

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
        attempt: 3,
        retry_count: 2,
        lease_expires_at_ms: 90,
    }
}
fn audit() -> DebuggerAuditContext {
    DebuggerAuditContext {
        command_id: CommandId::new("cancel-command").unwrap(),
        idempotency_key: IdempotencyKey::new("cancel-key").unwrap(),
        principal: Principal::new("operator").unwrap(),
        origin: CommandOrigin::Gui,
        session_id: "session-original".into(),
        expected_revision: Revision(7),
    }
}
fn cancelled(image: &ContinuationImage, reason: CancellationReason) -> Step {
    Step::Cancelled(Cancellation {
        continuation: image.token.clone(),
        reason,
        observed_at_ms: 12,
    })
}
fn scheduled() -> RetryDisposition {
    RetryDisposition::Scheduled(RetrySchedule {
        retry_count: 4,
        ready_at_ms: 77,
        error: EffectError {
            class: EffectErrorClass::Transient,
            code: "busy".into(),
            message: "native schedule".into(),
        },
    })
}
fn fault() -> Fault {
    Fault {
        code: "native-fault".into(),
        message: "native-owned-error".into(),
    }
}
fn payload(step: &Step) -> *const u8 {
    match step {
        Step::Cancelled(c) => c.continuation.as_str().as_ptr(),
        Step::Done(Value::UiFocus { node_id }) => node_id.as_ptr(),
        Step::Fault(e) => e.message.as_ptr(),
        _ => panic!(),
    }
}

#[derive(Default)]
struct Host {
    trace: Vec<&'static str>,
    image: Option<*const ContinuationImage>,
    lease: Option<*const DispatchLease>,
    audit: Option<*const DebuggerAuditContext>,
    proposed_step: Option<*const Step>,
    proposed_retry: Option<*const RetryDisposition>,
    reason: Option<CancellationReason>,
    winner: Option<Step>,
    retry: Option<RetryDisposition>,
    record: Option<DebuggerAuditRecord>,
    winner_pointer: Option<*const u8>,
    failure: Option<Fault>,
    fail_at: Option<&'static str>,
    unwind_at: Option<&'static str>,
    committed: bool,
}
impl Host {
    fn commit(&mut self, stage: &'static str) -> Result<(), Fault> {
        self.trace.push(stage);
        if self.fail_at == Some(stage) {
            return Err(self.failure.take().unwrap());
        }
        self.committed = true;
        assert_ne!(
            self.unwind_at,
            Some(stage),
            "native hook unwind after side effect"
        );
        Ok(())
    }
    fn check_image(&self, image: &ContinuationImage) {
        if let Some(expected) = self.image {
            assert_eq!(std::ptr::from_ref(image), expected);
        }
    }
    fn publication(&mut self, image: &ContinuationImage, step: &Step) -> Result<(), Fault> {
        self.trace.push("publish");
        assert!(self.committed);
        self.check_image(image);
        if let Some(pointer) = self.winner_pointer {
            assert_eq!(payload(step), pointer);
        }
        assert_ne!(self.unwind_at, Some("publish"), "native publication unwind");
        if self.fail_at == Some("publish") {
            return Err(self.failure.take().unwrap());
        }
        Ok(())
    }
}
impl ReferenceCancellationSettlementHost for Host {
    fn cancel(&mut self, image: &ContinuationImage, proposed: &Step) -> Result<Step, Fault> {
        self.check_image(image);
        if let Some(reason) = &self.reason {
            assert_eq!(proposed, &cancelled(image, reason.clone()));
        }
        self.commit("cancel")?;
        Ok(self.winner.take().unwrap())
    }
    fn cancel_audited(
        &mut self,
        image: &ContinuationImage,
        proposed: &Step,
        audit: &DebuggerAuditContext,
        now: u64,
    ) -> Result<(Step, DebuggerAuditRecord), Fault> {
        self.check_image(image);
        if let Some(pointer) = self.proposed_step {
            assert_eq!(std::ptr::from_ref(proposed), pointer);
        }
        if let Some(pointer) = self.audit {
            assert_eq!(std::ptr::from_ref(audit), pointer);
        }
        assert_eq!(now, 12);
        self.commit("audit")?;
        Ok((self.winner.take().unwrap(), self.record.take().unwrap()))
    }
    fn publish_cancellation(&mut self, image: &ContinuationImage, step: Step) -> Step {
        match self.publication(image, &step) {
            Ok(()) => step,
            Err(error) => Step::Fault(error),
        }
    }
}
impl ReferenceRetrySettlementHost for Host {
    fn report_error(
        &mut self,
        lease: &DispatchLease,
        now: u64,
        proposed: &RetryDisposition,
    ) -> Result<RetryDisposition, Fault> {
        if let Some(pointer) = self.lease {
            assert_eq!(std::ptr::from_ref(lease), pointer);
        }
        if let Some(pointer) = self.proposed_retry {
            assert_eq!(std::ptr::from_ref(proposed), pointer);
        }
        assert_eq!(
            (
                lease.attempt,
                lease.retry_count,
                lease.lease_expires_at_ms,
                now
            ),
            (3, 2, 90, 12)
        );
        self.commit("retry")?;
        Ok(self.retry.take().unwrap())
    }
    fn publish_retry_terminal(&mut self, image: &ContinuationImage, step: &Step) -> Step {
        match self.publication(image, step) {
            Ok(()) => step.clone(),
            Err(error) => Step::Fault(error),
        }
    }
}

#[test]
fn ordinary_cancellation_keeps_reason_time_and_original_native_winner_payload() {
    let lease = lease();
    let image = &lease.request.continuation;
    let before = serde_json::to_vec(image).unwrap();
    for reason in [
        CancellationReason::Requested,
        CancellationReason::DeadlineExceeded,
    ] {
        for winner in [
            cancelled(image, reason.clone()),
            Step::Done(Value::UiFocus {
                node_id: "peer-winner".into(),
            }),
        ] {
            let pointer = payload(&winner);
            let mut host = Host {
                image: Some(std::ptr::from_ref(image)),
                reason: Some(reason.clone()),
                winner_pointer: Some(pointer),
                winner: Some(winner),
                ..Host::default()
            };
            let actual = interruption::finish_cancellation(
                image,
                reason.clone(),
                12,
                &mut host as &mut dyn ReferenceCancellationSettlementHost,
            );
            assert_eq!(payload(&actual), pointer);
            assert_eq!(host.trace, ["cancel", "publish"]);
        }
    }
    assert_eq!(serde_json::to_vec(image).unwrap(), before);
}

#[test]
fn audited_winner_controls_publication_without_rebuilding_original_record_or_inputs() {
    let lease = lease();
    let image = &lease.request.continuation;
    let audit = audit();
    let proposed = cancelled(image, CancellationReason::Requested);
    let before = serde_json::to_vec(&(image, &proposed, &audit)).unwrap();
    for winner in [
        cancelled(image, CancellationReason::DeadlineExceeded),
        Step::Done(Value::UiFocus {
            node_id: "peer".into(),
        }),
        Step::Fault(fault()),
    ] {
        let publishes = matches!(winner, Step::Cancelled(_));
        let record = leselang_command::debugger_audit::debugger_audit_record(&audit, 2);
        let pointer = record.session_id.as_ptr();
        let mut host = Host {
            image: Some(std::ptr::from_ref(image)),
            audit: Some(std::ptr::from_ref(&audit)),
            proposed_step: Some(std::ptr::from_ref(&proposed)),
            winner_pointer: Some(payload(&winner)),
            winner: Some(winner),
            record: Some(record),
            ..Host::default()
        };
        let record =
            interruption::finish_audited_cancellation(image, &proposed, &audit, 12, &mut host)
                .unwrap();
        assert_eq!(record.session_id.as_ptr(), pointer);
        assert_eq!(record.observed_at_ms, 2);
        assert_eq!(
            host.trace,
            if publishes {
                vec!["audit", "publish"]
            } else {
                vec!["audit"]
            }
        );
    }
    assert_eq!(
        serde_json::to_vec(&(image, &proposed, &audit)).unwrap(),
        before
    );
}

#[test]
fn native_retry_disposition_not_proposal_controls_terminal_publication() {
    let lease = lease();
    for proposal in [
        scheduled(),
        RetryDisposition::Terminal(Step::Fault(fault())),
    ] {
        for winner in [
            scheduled(),
            RetryDisposition::Terminal(Step::Fault(fault())),
        ] {
            let terminal = matches!(winner, RetryDisposition::Terminal(_));
            let expected = winner.clone();
            let pointer = match &winner {
                RetryDisposition::Scheduled(s) => s.error.message.as_ptr(),
                RetryDisposition::Terminal(step) => payload(step),
            };
            let before = serde_json::to_vec(&(&lease, &proposal)).unwrap();
            let mut host = Host {
                image: Some(std::ptr::from_ref(&lease.request.continuation)),
                lease: Some(std::ptr::from_ref(&lease)),
                proposed_retry: Some(std::ptr::from_ref(&proposal)),
                retry: Some(winner),
                winner_pointer: Some(pointer),
                ..Host::default()
            };
            let actual = interruption::settle_effect_error(
                &lease,
                12,
                &proposal,
                &mut host as &mut dyn ReferenceRetrySettlementHost,
            )
            .unwrap();
            assert_eq!(actual, expected);
            if let RetryDisposition::Scheduled(s) = actual {
                assert_eq!(s.error.message.as_ptr(), pointer);
            }
            assert_eq!(
                host.trace,
                if terminal {
                    vec!["retry", "publish"]
                } else {
                    vec!["retry"]
                }
            );
            assert_eq!(serde_json::to_vec(&(&lease, &proposal)).unwrap(), before);
        }
    }
}

fn failure_case(route: usize, stage: &'static str, unwind: bool) {
    let lease = lease();
    let image = &lease.request.continuation;
    let audit = audit();
    let proposed = cancelled(image, CancellationReason::Requested);
    let failure = fault();
    let pointer = failure.message.as_ptr();
    let mut host = Host {
        winner: Some(proposed.clone()),
        retry: Some(RetryDisposition::Terminal(proposed.clone())),
        record: Some(leselang_command::debugger_audit::debugger_audit_record(
            &audit, 2,
        )),
        failure: Some(failure),
        fail_at: (!unwind).then_some(stage),
        unwind_at: unwind.then_some(stage),
        ..Host::default()
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| match route {
        0 => interruption::finish_cancellation(image, CancellationReason::Requested, 12, &mut host),
        1 => Step::Fault(
            interruption::finish_audited_cancellation(image, &proposed, &audit, 12, &mut host)
                .unwrap_err(),
        ),
        _ => match interruption::settle_effect_error(&lease, 12, &scheduled(), &mut host) {
            Err(error) if stage == "retry" => Step::Fault(error),
            Ok(RetryDisposition::Terminal(Step::Fault(error))) if stage == "publish" => {
                Step::Fault(error)
            }
            other => panic!("wrong error layer: {other:?}"),
        },
    }));
    let commit = ["cancel", "audit", "retry"][route];
    assert_eq!(
        host.trace,
        if stage == "publish" {
            vec![commit, "publish"]
        } else {
            vec![commit]
        }
    );
    assert_eq!(host.committed, unwind || stage == "publish");
    if unwind {
        assert!(result.is_err());
    } else {
        assert_eq!(payload(&result.unwrap()), pointer);
    }
}

#[test]
fn commit_failures_keep_original_faults_and_stop_before_publication() {
    for (route, stage) in ["cancel", "audit", "retry"].into_iter().enumerate() {
        failure_case(route, stage, false);
    }
}

#[test]
fn publication_faults_preserve_the_three_distinct_return_layers_after_commit() {
    for route in 0..3 {
        failure_case(route, "publish", false);
    }
}

#[test]
fn hook_unwinds_do_not_retry_or_roll_back_prior_native_effects() {
    for (route, stage) in ["cancel", "audit", "retry"].into_iter().enumerate() {
        failure_case(route, stage, true);
        failure_case(route, "publish", true);
    }
}
