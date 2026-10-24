use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::reference_restore::{ReferenceRestoreHost, restore, restore_request};
use leselang_command::{ContinuationImage, ContinuationToken, EffectRequest};
use leselang_hir::{Effect, Type};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;
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

#[derive(Default)]
struct Host {
    trace: Rc<RefCell<Vec<&'static str>>>,
    pending: Option<ContinuationImage>,
    completed: bool,
    committed: Option<ContinuationImage>,
    watermark: Option<u64>,
    authority_pointer: Option<*const u8>,
    failure: Option<Fault>,
    unwind_at: Option<&'static str>,
    native_calls: usize,
}

impl Host {
    fn observe(&self, stage: &'static str) {
        self.trace.borrow_mut().push(stage);
        assert_ne!(self.unwind_at, Some(stage), "trusted hook unwind");
    }
    fn commit(&mut self, request: Option<&EffectRequest>) -> Result<(), Fault> {
        self.native_calls += 1;
        if let Some(pointer) = self.authority_pointer {
            assert_eq!(
                request.unwrap().operation.authority().0.id.as_ptr(),
                pointer
            );
        }
        if let Some(error) = self.failure.take() {
            return Err(error);
        }
        Ok(())
    }
}

impl ReferenceRestoreHost for Host {
    fn pending(&self, _: &ContinuationToken) -> Option<&ContinuationImage> {
        self.observe("pending");
        self.pending.as_ref()
    }
    fn is_completed(&self, _: &ContinuationToken) -> bool {
        self.observe("completed");
        self.completed
    }
    fn commit_existing(
        &mut self,
        image: &ContinuationImage,
        request: Option<&EffectRequest>,
    ) -> Result<(), Fault> {
        self.observe("existing");
        assert_eq!(self.pending.as_ref(), Some(image));
        self.commit(request)
    }
    fn commit_new(
        &mut self,
        image: ContinuationImage,
        request: Option<&EffectRequest>,
        next_sequence: u64,
    ) -> Result<(), Fault> {
        self.observe("new");
        self.commit(request)?;
        self.watermark = Some(next_sequence);
        self.committed = Some(image);
        Ok(())
    }
}

#[test]
fn restore_moves_bare_images_borrows_original_authority_and_keeps_duplicate_hooks() {
    for full in [false, true] {
        for duplicate in [false, true] {
            let request = request();
            let image_pointer = request.continuation.token.as_str().as_ptr();
            let mut host = Host {
                completed: duplicate,
                ..Host::default()
            };
            if duplicate {
                host.pending = Some(request.continuation.clone());
            }
            let pending_pointer = host.pending.as_ref().map(|i| i.token.as_str().as_ptr());
            if full {
                host.authority_pointer = Some(request.operation.authority().0.id.as_ptr());
                restore_request(request, &mut host as &mut dyn ReferenceRestoreHost).unwrap();
            } else {
                restore(request.continuation, &mut host).unwrap();
            }
            assert_eq!(host.native_calls, 1);
            if duplicate {
                assert_eq!(*host.trace.borrow(), ["pending", "existing"]);
                assert_eq!(
                    host.pending.as_ref().map(|i| i.token.as_str().as_ptr()),
                    pending_pointer
                );
                assert!(host.committed.is_none());
                assert!(host.watermark.is_none());
            } else {
                assert_eq!(*host.trace.borrow(), ["pending", "completed", "new"]);
                assert_eq!(host.watermark, Some(8));
                if !full {
                    assert_eq!(
                        host.committed.unwrap().token.as_str().as_ptr(),
                        image_pointer
                    );
                }
            }
        }
    }
}

#[test]
fn restore_preflight_and_local_conflicts_preserve_refusal_precedence() {
    for full in [false, true] {
        for defect in 0..6 {
            let mut request = request();
            let mut host = Host {
                completed: true,
                ..Host::default()
            };
            let expected = match defect {
                0 => {
                    request.continuation.schema_version = 0;
                    "LSV2001"
                }
                1 => {
                    request.continuation.token = ContinuationToken::from_wire("bad".into());
                    "LSV2006"
                }
                2 => {
                    request.continuation.schema_version = 6;
                    request.continuation.group_result =
                        Some(ContinuationToken::from_wire("merge-1".into()));
                    "LSV1409"
                }
                3 => {
                    let mut saved = request.continuation.clone();
                    saved.fuel_remaining -= 1;
                    host.pending = Some(saved);
                    "LSV2002"
                }
                4 => "LSV2003",
                _ => {
                    request.effect_id.clear();
                    request.continuation.schema_version = 0;
                    "LSV2001"
                }
            };
            let result = if full {
                restore_request(request, &mut host)
            } else {
                restore(request.continuation, &mut host)
            };
            let error = result.unwrap_err();
            // Full request admission can refuse a malformed identity before the
            // image-only canonical-sequence diagnostic.
            if full && defect == 1 {
                assert_eq!(error.code, "LSV2010");
            } else {
                assert_eq!(error.code, expected, "full={full}, defect={defect}");
            }
            let trace: &[&str] = match defect {
                3 => &["pending"],
                4 => &["pending", "completed"],
                _ => &[],
            };
            assert_eq!(host.trace.borrow().as_slice(), trace);
            assert_eq!(host.native_calls, 0);
            assert!(host.committed.is_none());
        }
    }
}

#[test]
fn restore_never_retries_native_failures_or_unwinds_and_does_not_roll_back_native_state() {
    for duplicate in [false, true] {
        let mut host = Host::default();
        let request = request();
        if duplicate {
            host.pending = Some(request.continuation.clone());
        }
        let error = Fault {
            code: "NATIVE".into(),
            message: "original native refusal".into(),
        };
        let pointer = error.message.as_ptr();
        host.failure = Some(error);
        let error = restore_request(request, &mut host).unwrap_err();
        assert_eq!(error.message.as_ptr(), pointer);
        assert_eq!(host.native_calls, 1);
        assert!(host.committed.is_none());
        assert!(host.watermark.is_none());
    }
    for stage in ["pending", "completed", "existing", "new"] {
        let request = request();
        let mut host = Host {
            unwind_at: Some(stage),
            ..Host::default()
        };
        if stage == "existing" {
            host.pending = Some(request.continuation.clone());
        }
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| restore_request(
                request, &mut host
            )))
            .is_err()
        );
        assert_eq!(
            host.trace.borrow().iter().filter(|s| **s == stage).count(),
            1
        );
        assert!(host.committed.is_none());
        assert!(host.watermark.is_none());
    }
}
