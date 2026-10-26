use leselang_command::reference_reply::{self, CompletionLease, ReferenceReplyHost};
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::{
    ContinuationImage, ContinuationToken, DispatchLease, EffectRequest, EffectResult,
    PresentationResult, Step, Value,
};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;
use leserpent_hir::{Effect, Type};
use std::{cell::RefCell, rc::Rc};

fn request() -> EffectRequest {
    build_effect_request(ReservedEffectInput {
        effect: &Effect::UiFocus {
            node_id: "target".into(),
        },
        result_type: Type::UiFocus,
        sequence: 7,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: None,
        deadline_ms: 30_000,
        deadline_at_ms: None,
        fuel_remaining: 100,
    })
    .unwrap()
}
fn lease() -> DispatchLease {
    DispatchLease {
        request: request(),
        attempt: 2,
        retry_count: 1,
        lease_expires_at_ms: 100,
    }
}
fn reply() -> EffectResult {
    EffectResult::Presentation(PresentationResult::Focus {
        node_id: "target".into(),
    })
}

#[derive(Default)]
struct Host {
    trace: Rc<RefCell<Vec<&'static str>>>,
    pending: Option<ContinuationImage>,
    replay: Option<Step>,
    fail_at: Option<&'static str>,
    failure: Option<Fault>,
    unwind_at: Option<&'static str>,
    image_pointer: Option<*const ContinuationImage>,
    lease_pointer: Option<*const DispatchLease>,
}
impl Host {
    fn observe(&self, stage: &'static str) {
        self.trace.borrow_mut().push(stage);
        assert_ne!(self.unwind_at, Some(stage), "trusted hook unwind");
    }
    fn enter(&mut self, stage: &'static str) -> Result<(), Fault> {
        self.observe(stage);
        if self.fail_at == Some(stage) {
            return Err(self.failure.take().unwrap());
        }
        Ok(())
    }
}
impl ReferenceReplyHost for Host {
    fn expire_due(&mut self, now_ms: u64) -> Result<(), Fault> {
        self.enter("expire")?;
        leselang_runtime_core::validate_clock(now_ms)
    }
    fn replay_completed(&mut self, _: &ContinuationToken) -> Result<Option<Step>, Fault> {
        self.enter("replay")?;
        Ok(self.replay.take())
    }
    fn visible_step(&mut self, _: &ContinuationToken, step: Step) -> Step {
        if let Err(error) = self.enter("visible") {
            return Step::Fault(error);
        }
        step
    }
    fn pending(&self, _: &ContinuationToken) -> Option<&ContinuationImage> {
        self.observe("pending");
        self.pending.as_ref()
    }
    fn complete_result(
        &mut self,
        image: &ContinuationImage,
        lease: CompletionLease<'_>,
        result: EffectResult,
    ) -> Step {
        if let Err(error) = self.enter("complete") {
            return Step::Fault(error);
        }
        if let Some(pointer) = self.image_pointer {
            assert_eq!(std::ptr::from_ref(image), pointer);
        }
        if let Some(pointer) = self.lease_pointer {
            let (lease, now) = lease.unwrap();
            assert_eq!(std::ptr::from_ref(lease), pointer);
            assert_eq!(now, 2);
            assert_eq!(
                (lease.attempt, lease.retry_count, lease.lease_expires_at_ms),
                (2, 1, 100)
            );
        } else {
            assert!(lease.is_none());
        }
        let EffectResult::Presentation(PresentationResult::Focus { node_id }) = result else {
            panic!()
        };
        Step::Done(Value::UiFocus { node_id })
    }
}
fn invoke(
    mode: usize,
    lease: &DispatchLease,
    result: EffectResult,
    host: &mut dyn ReferenceReplyHost,
) -> Step {
    match mode {
        0 => reference_reply::resume(&lease.request.continuation, result, host),
        1 => reference_reply::resume_at(&lease.request.continuation, 2, result, host),
        _ => reference_reply::acknowledge_effect(lease, 2, result, host),
    }
}

#[test]
fn reply_entries_keep_original_reply_buffers_images_and_lease_borrows() {
    for mode in 0..3 {
        let lease = lease();
        let mut host = Host {
            pending: Some(lease.request.continuation.clone()),
            image_pointer: Some(&lease.request.continuation),
            lease_pointer: (mode == 2).then_some(&lease),
            ..Host::default()
        };
        let result = reply();
        let EffectResult::Presentation(PresentationResult::Focus { node_id }) = &result else {
            panic!()
        };
        let pointer = node_id.as_ptr();
        let Step::Done(Value::UiFocus { node_id }) = invoke(mode, &lease, result, &mut host) else {
            panic!()
        };
        assert_eq!(node_id.as_ptr(), pointer);
        let trace: &[&str] = if mode == 0 {
            &["replay", "pending", "complete"]
        } else {
            &["expire", "replay", "pending", "complete"]
        };
        assert_eq!(host.trace.borrow().as_slice(), trace);
    }
}

#[test]
fn reply_replay_and_request_validation_keep_distinct_entry_precedence() {
    for mode in 0..3 {
        let mut lease = lease();
        lease.request.continuation.schema_version = 0;
        let saved = "authoritative saved result".to_string();
        let pointer = saved.as_ptr();
        let mut host = Host {
            replay: Some(Step::Done(Value::UiFocus { node_id: saved })),
            ..Host::default()
        };
        let step = invoke(mode, &lease, reply(), &mut host);
        if mode == 2 {
            assert!(matches!(step, Step::Fault(_)));
            assert_eq!(*host.trace.borrow(), ["expire"]);
            assert!(host.replay.is_some());
        } else {
            let Step::Done(Value::UiFocus { node_id }) = step else {
                panic!()
            };
            assert_eq!(node_id.as_ptr(), pointer);
            assert_eq!(host.trace.borrow().last(), Some(&"visible"));
            assert!(!host.trace.borrow().contains(&"pending"));
        }
    }
    let mut lease = lease();
    lease.request.continuation.deadline_at_ms = Some(100);
    let mut host = Host::default();
    assert!(
        matches!(reference_reply::resume(&lease.request.continuation, reply(), &mut host), Step::Fault(ref e) if e.code == "LSV2111")
    );
    assert!(host.trace.borrow().is_empty());
    assert!(matches!(
        reference_reply::resume_at(&lease.request.continuation, u64::MAX, reply(), &mut host),
        Step::Fault(_)
    ));
    assert!(host.trace.borrow().is_empty());
    assert!(matches!(
        reference_reply::acknowledge_effect(&lease, u64::MAX, reply(), &mut host),
        Step::Fault(_)
    ));
    assert_eq!(*host.trace.borrow(), ["expire"]);
}

#[test]
fn reply_pending_refusals_never_invoke_completion() {
    for mode in 0..3 {
        for mismatch in [false, true] {
            let lease = lease();
            let mut host = Host::default();
            if mismatch {
                let mut image = lease.request.continuation.clone();
                image.fuel_remaining -= 1;
                host.pending = Some(image);
            }
            let Step::Fault(error) = invoke(mode, &lease, reply(), &mut host) else {
                panic!()
            };
            assert_eq!(error.code, if mismatch { "LSV2005" } else { "LSV2004" });
            assert_eq!(host.trace.borrow().last(), Some(&"pending"));
        }
    }
}

#[test]
fn reply_hook_errors_and_unwind_are_once_only_without_native_rollback() {
    for stage in ["expire", "replay", "pending", "visible", "complete"] {
        for unwind in [false, true] {
            if stage == "pending" && !unwind {
                continue;
            }
            let lease = lease();
            let error = Fault {
                code: "NATIVE".into(),
                message: "original native failure".into(),
            };
            let pointer = error.message.as_ptr();
            let mut host = Host {
                pending: Some(lease.request.continuation.clone()),
                failure: Some(error),
                fail_at: (!unwind).then_some(stage),
                unwind_at: unwind.then_some(stage),
                ..Host::default()
            };
            if stage == "visible" {
                host.replay = Some(Step::Done(Value::UiFocus {
                    node_id: "saved".into(),
                }));
            }
            let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                invoke(2, &lease, reply(), &mut host)
            }));
            if unwind {
                assert!(outcome.is_err());
            } else {
                let Step::Fault(error) = outcome.unwrap() else {
                    panic!()
                };
                assert_eq!(error.message.as_ptr(), pointer);
            }
            let trace = host.trace.borrow();
            assert_eq!(trace.last(), Some(&stage));
            assert_eq!(trace.iter().filter(|s| **s == stage).count(), 1);
        }
    }
}
