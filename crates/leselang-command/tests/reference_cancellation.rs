use leselang_command::reference_cancellation::{self, ReferenceCancellationHost};
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::{
    Cancellation, CancellationReason, ContinuationImage, ContinuationToken, DebuggerAuditContext,
    DebuggerAuditRecord, Step, Value,
};
use leselang_host_contract::{CapabilitySet, CommandOrigin, Principal, Revision};
use leselang_runtime_core::Fault;
use leserpent_domain::{CommandId, IdempotencyKey};
use leserpent_hir::{Effect, Type};
use std::{cell::RefCell, rc::Rc};

fn image() -> ContinuationImage {
    build_effect_request(ReservedEffectInput {
        effect: &Effect::UiFocus {
            node_id: "node".into(),
        },
        result_type: Type::UiFocus,
        sequence: 7,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: Some(Revision(7)),
        deadline_ms: 1000,
        deadline_at_ms: None,
        fuel_remaining: 100,
    })
    .unwrap()
    .continuation
}
fn audit() -> DebuggerAuditContext {
    DebuggerAuditContext {
        command_id: CommandId::new("cancel-command").unwrap(),
        idempotency_key: IdempotencyKey::new("cancel-key").unwrap(),
        principal: Principal::new("debugger-operator").unwrap(),
        origin: CommandOrigin::Gui,
        session_id: "session-a".into(),
        expected_revision: Revision(7),
    }
}
fn cancelled(image: &ContinuationImage, reason: CancellationReason) -> Step {
    Step::Cancelled(Cancellation {
        continuation: image.token.clone(),
        reason,
        observed_at_ms: 1,
    })
}

#[derive(Default)]
struct Host {
    trace: Rc<RefCell<Vec<&'static str>>>,
    pending: Option<ContinuationImage>,
    replay: Option<Step>,
    reconciled: Option<Step>,
    fail_at: Option<&'static str>,
    error: Option<Fault>,
    unwind_at: Option<&'static str>,
    image_pointer: Option<*const ContinuationImage>,
    audit_pointer: Option<*const DebuggerAuditContext>,
    record: Option<DebuggerAuditRecord>,
}
impl Host {
    fn observe(&self, stage: &'static str) {
        self.trace.borrow_mut().push(stage);
        assert_ne!(self.unwind_at, Some(stage), "trusted hook unwind");
    }
    fn enter(&mut self, stage: &'static str) -> Result<(), Fault> {
        self.observe(stage);
        if self.fail_at == Some(stage) {
            return Err(self.error.take().unwrap());
        }
        Ok(())
    }
    fn original_image(&self, image: &ContinuationImage) {
        if let Some(pointer) = self.image_pointer {
            assert_eq!(std::ptr::from_ref(image), pointer);
        }
    }
}
impl ReferenceCancellationHost for Host {
    fn expire_due(&mut self, _: u64) -> Result<(), Fault> {
        self.enter("expire")
    }
    fn replay_completed(&mut self, _: &ContinuationToken) -> Result<Option<Step>, Fault> {
        self.enter("replay")?;
        Ok(self.replay.take())
    }
    fn reconcile_completed(&mut self, _: &ContinuationToken) -> Result<Option<&Step>, Fault> {
        self.enter("reconcile")?;
        Ok(self.reconciled.as_ref())
    }
    fn visible_step(&mut self, _: &ContinuationToken, completed: Step) -> Step {
        if let Err(e) = self.enter("visible") {
            return Step::Fault(e);
        }
        completed
    }
    fn pending(&self, _: &ContinuationToken) -> Option<&ContinuationImage> {
        self.observe("pending");
        self.pending.as_ref()
    }
    fn finish_cancellation(
        &mut self,
        image: &ContinuationImage,
        reason: CancellationReason,
        now: u64,
    ) -> Step {
        if let Err(e) = self.enter("finish") {
            return Step::Fault(e);
        }
        self.original_image(image);
        assert_eq!(reason, CancellationReason::Requested);
        Step::Cancelled(Cancellation {
            continuation: image.token.clone(),
            reason,
            observed_at_ms: now,
        })
    }
    fn finish_audited_cancellation(
        &mut self,
        image: &ContinuationImage,
        step: &Step,
        audit: &DebuggerAuditContext,
        now: u64,
    ) -> Result<DebuggerAuditRecord, Fault> {
        self.enter("audit")?;
        self.original_image(image);
        if let Some(pointer) = self.audit_pointer {
            assert_eq!(std::ptr::from_ref(audit), pointer);
        }
        assert_eq!(
            step,
            &Step::Cancelled(Cancellation {
                continuation: image.token.clone(),
                reason: CancellationReason::Requested,
                observed_at_ms: now
            })
        );
        Ok(self
            .record
            .take()
            .unwrap_or_else(|| leselang_command::debugger_audit::debugger_audit_record(audit, now)))
    }
}

#[test]
fn pending_cancellation_keeps_original_borrows_and_native_record_owner() {
    let image = image();
    let audit = audit();
    let mut host = Host {
        pending: Some(image.clone()),
        image_pointer: Some(&image),
        ..Host::default()
    };
    let step = reference_cancellation::cancel_effect(
        &image,
        12,
        &mut host as &mut dyn ReferenceCancellationHost,
    );
    assert!(matches!(
        step,
        Step::Cancelled(Cancellation {
            observed_at_ms: 12,
            ..
        })
    ));
    assert_eq!(
        *host.trace.borrow(),
        ["expire", "replay", "pending", "finish"]
    );
    let record = leselang_command::debugger_audit::debugger_audit_record(&audit, 1);
    let pointer = record.principal_id.as_ptr();
    let mut host = Host {
        pending: Some(image.clone()),
        image_pointer: Some(&image),
        audit_pointer: Some(&audit),
        record: Some(record),
        ..Host::default()
    };
    let record = reference_cancellation::cancel_effect_audited(
        &image,
        12,
        &audit,
        &mut host as &mut dyn ReferenceCancellationHost,
    )
    .unwrap();
    assert_eq!(record.principal_id.as_ptr(), pointer);
    assert_eq!(record.observed_at_ms, 1);
    assert_eq!(
        *host.trace.borrow(),
        ["expire", "reconcile", "pending", "audit"]
    );
}

#[test]
fn audit_metadata_precedes_clock_and_replay_while_ordinary_entry_checks_clock_first() {
    let image = image();
    let mut audit = audit();
    audit.session_id = "invalid session".into();
    let mut host = Host::default();
    let error = reference_cancellation::cancel_effect_audited(&image, u64::MAX, &audit, &mut host)
        .unwrap_err();
    assert_eq!(error.code, "LSV2015");
    assert!(host.trace.borrow().is_empty());
    let expected = leselang_runtime_core::validate_clock(u64::MAX).unwrap_err();
    assert_eq!(
        reference_cancellation::cancel_effect(&image, u64::MAX, &mut host),
        Step::Fault(expected.clone())
    );
    audit.session_id = "valid".into();
    assert_eq!(
        reference_cancellation::cancel_effect_audited(&image, u64::MAX, &audit, &mut host),
        Err(expected)
    );
    assert!(host.trace.borrow().is_empty());
}

#[test]
fn replay_precedes_local_equality_but_audited_replay_requires_requested_cancellation() {
    let mut image = image();
    image.schema_version = 0;
    let text = "original completed payload".to_owned();
    let pointer = text.as_ptr();
    let mut host = Host {
        replay: Some(Step::Done(Value::UiFocus { node_id: text })),
        ..Host::default()
    };
    let Step::Done(Value::UiFocus { node_id }) =
        reference_cancellation::cancel_effect(&image, 12, &mut host)
    else {
        panic!()
    };
    assert_eq!(node_id.as_ptr(), pointer);
    assert_eq!(*host.trace.borrow(), ["expire", "replay", "visible"]);
    let audit = audit();
    for requested in [false, true] {
        let step = cancelled(
            &image,
            if requested {
                CancellationReason::Requested
            } else {
                CancellationReason::DeadlineExceeded
            },
        );
        let before = serde_json::to_vec(&step).unwrap();
        let mut host = Host {
            reconciled: Some(step),
            ..Host::default()
        };
        let result = reference_cancellation::cancel_effect_audited(&image, 12, &audit, &mut host);
        assert_eq!(
            serde_json::to_vec(host.reconciled.as_ref().unwrap()).unwrap(),
            before
        );
        if requested {
            assert!(result.is_ok());
            assert_eq!(*host.trace.borrow(), ["expire", "reconcile", "audit"]);
        } else {
            assert_eq!(result.unwrap_err().code, "LSV2014");
            assert_eq!(*host.trace.borrow(), ["expire", "reconcile"]);
        }
    }
}

#[test]
fn unknown_and_mismatched_pending_images_do_not_enter_native_commit() {
    let image = image();
    let audit = audit();
    for mismatch in [false, true] {
        let mut pending = image.clone();
        pending.fuel_remaining -= 1;
        for audited in [false, true] {
            let mut host = Host {
                pending: mismatch.then_some(pending.clone()),
                ..Host::default()
            };
            let error = if audited {
                reference_cancellation::cancel_effect_audited(&image, 12, &audit, &mut host)
                    .unwrap_err()
            } else {
                let Step::Fault(e) = reference_cancellation::cancel_effect(&image, 12, &mut host)
                else {
                    panic!()
                };
                e
            };
            assert_eq!(error.code, if mismatch { "LSV2005" } else { "LSV2004" });
            assert_eq!(host.trace.borrow().last(), Some(&"pending"));
        }
    }
}

#[test]
fn trusted_hook_faults_and_unwinds_keep_order_ownership_and_prior_side_effects() {
    let image = image();
    let audit = audit();
    for stage in [
        "expire",
        "replay",
        "reconcile",
        "visible",
        "pending",
        "finish",
        "audit",
    ] {
        for unwind in [false, true] {
            if stage == "pending" && !unwind {
                continue;
            }
            let error = Fault {
                code: "owner-error".into(),
                message: "original owner failure".into(),
            };
            let pointer = error.message.as_ptr();
            let mut host = Host {
                pending: Some(image.clone()),
                error: Some(error),
                fail_at: (!unwind).then_some(stage),
                unwind_at: unwind.then_some(stage),
                ..Host::default()
            };
            if stage == "visible" {
                host.replay = Some(cancelled(&image, CancellationReason::Requested));
            }
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                if matches!(stage, "reconcile" | "audit") {
                    reference_cancellation::cancel_effect_audited(&image, 12, &audit, &mut host)
                        .unwrap_err()
                } else {
                    let Step::Fault(error) =
                        reference_cancellation::cancel_effect(&image, 12, &mut host)
                    else {
                        panic!()
                    };
                    error
                }
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap().message.as_ptr(), pointer);
            }
            let trace = host.trace.borrow();
            assert_eq!(trace.iter().filter(|s| **s == stage).count(), 1);
            assert_eq!(trace.last(), Some(&stage));
        }
    }
}
