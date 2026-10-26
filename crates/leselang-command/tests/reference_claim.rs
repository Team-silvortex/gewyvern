use leselang_command::reference_claim::{self, ReferenceClaimHost};
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::{
    ContinuationImage, DispatchClaim, DispatchLease, SchedulerLimits, SchedulerPressure,
};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;
use leserpent_hir::{Effect, Type};
use std::{cell::RefCell, rc::Rc};

fn lease() -> Box<DispatchLease> {
    Box::new(DispatchLease {
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
        attempt: 2,
        retry_count: 1,
        lease_expires_at_ms: 62,
    })
}

#[test]
fn cache_adoption_clones_only_vacancies_and_preserves_equal_or_conflicting_entries() {
    use std::collections::BTreeMap;
    for state in 0..4 {
        let lease = lease();
        let image = &lease.request.continuation;
        let bytes = serde_json::to_vec(&lease).unwrap();
        let mut pending = BTreeMap::new();
        if state != 0 {
            let mut stored = image.clone();
            if state == 2 {
                stored.fuel_remaining -= 1;
            }
            if state == 3 {
                stored.token =
                    leselang_command::ContinuationToken::from_wire("continuation-8".into());
            }
            pending.insert(image.token.clone(), stored);
        }
        let pointer = pending.get(&image.token).map(|i| i.token.as_str().as_ptr());
        let before = pending.clone();
        let got = reference_claim::adopt_claimed_image(&mut pending, image);
        if state < 2 {
            got.unwrap();
            assert_eq!(&pending[&image.token], image);
        } else {
            assert_eq!(got.unwrap_err().code, "LSV2005");
            assert_eq!(pending, before);
        }
        if let Some(pointer) = pointer {
            assert_eq!(pending[&image.token].token.as_str().as_ptr(), pointer);
        } else {
            assert_ne!(
                pending[&image.token].token.as_str().as_ptr(),
                image.token.as_str().as_ptr()
            );
        }
        assert_eq!(serde_json::to_vec(&lease).unwrap(), bytes);
    }
}
#[derive(Default)]
struct Host {
    trace: Rc<RefCell<Vec<&'static str>>>,
    claim: Option<DispatchClaim>,
    image_pointer: Option<*const ContinuationImage>,
    existing: Option<ContinuationImage>,
    committed: bool,
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
impl ReferenceClaimHost for Host {
    fn expire_due(&mut self, _: u64) -> Result<(), Fault> {
        self.enter("expire")
    }
    fn claim_dispatch(&mut self, _: u64, _: u64) -> Result<DispatchClaim, Fault> {
        self.enter("claim")?;
        let claim = self.claim.take().unwrap_or(DispatchClaim::Idle);
        self.committed = matches!(claim, DispatchClaim::Leased(_));
        Ok(claim)
    }
    fn adopt_claimed_image(&mut self, image: &ContinuationImage) -> Result<(), Fault> {
        self.enter("adopt")?;
        assert!(self.committed);
        if let Some(pointer) = self.image_pointer {
            assert_eq!(std::ptr::from_ref(image), pointer);
        }
        if let Some(existing) = &self.existing {
            reference_claim::validate_claimed_image(existing, image)?;
        }
        Ok(())
    }
}

#[test]
fn both_claim_apis_keep_the_original_lease_payload_and_borrowed_image() {
    for structured in [false, true] {
        let lease = lease();
        let box_pointer = std::ptr::from_ref(lease.as_ref());
        let payload_pointer = lease.request.effect_id.as_ptr();
        let image_pointer = std::ptr::from_ref(&lease.request.continuation);
        let before = serde_json::to_vec(&lease).unwrap();
        let mut host = Host {
            claim: Some(DispatchClaim::Leased(lease)),
            image_pointer: Some(image_pointer),
            ..Host::default()
        };
        let got = if structured {
            let DispatchClaim::Leased(lease) =
                reference_claim::try_claim_effect(12, 50, &mut host as &mut dyn ReferenceClaimHost)
                    .unwrap()
            else {
                panic!()
            };
            assert_eq!(std::ptr::from_ref(lease.as_ref()), box_pointer);
            *lease
        } else {
            reference_claim::claim_effect(12, 50, &mut host as &mut dyn ReferenceClaimHost)
                .unwrap()
                .unwrap()
        };
        assert_eq!(got.request.effect_id.as_ptr(), payload_pointer);
        assert_eq!(serde_json::to_vec(&got).unwrap(), before);
        assert_eq!(*host.trace.borrow(), ["expire", "claim", "adopt"]);
    }
}

#[test]
fn idle_and_backpressure_never_adopt_and_keep_the_legacy_fault_mapping() {
    let pressure = SchedulerPressure {
        pending_dispatches: 3,
        active_leases: 2,
        limits: SchedulerLimits {
            max_pending_dispatches: 10,
            max_active_leases: 1,
        },
    };
    let fault = Fault {
        code: "LSV2502".into(),
        message: "dispatch leasing is backpressured: 2 active leases, limit 1".into(),
    };
    assert_eq!(reference_claim::lease_capacity_fault(pressure), fault);
    for structured in [false, true] {
        for pressured in [false, true] {
            let claim = if pressured {
                DispatchClaim::Backpressured(pressure)
            } else {
                DispatchClaim::Idle
            };
            let mut host = Host {
                claim: Some(claim.clone()),
                ..Host::default()
            };
            if structured {
                assert_eq!(
                    reference_claim::try_claim_effect(12, 50, &mut host).unwrap(),
                    claim
                );
            } else {
                assert_eq!(
                    reference_claim::claim_effect(12, 50, &mut host),
                    if pressured {
                        Err(fault.clone())
                    } else {
                        Ok(None)
                    }
                );
            }
            assert_eq!(*host.trace.borrow(), ["expire", "claim"]);
            assert!(!host.committed);
        }
    }
}

#[test]
fn clock_duration_and_overflow_refuse_before_expiration_or_native_issuance() {
    use leselang_runtime_core::MAX_CLOCK_MS;
    for structured in [false, true] {
        for (now, duration) in [
            (u64::MAX, 0),
            (12, 0),
            (12, 300001),
            (MAX_CLOCK_MS, 1),
            (MAX_CLOCK_MS - 1, 2),
        ] {
            let expected = leselang_runtime_core::validate_clock(now)
                .and_then(|()| {
                    leselang_command::reference_dispatch::validate_lease_clock(now, duration)
                })
                .unwrap_err();
            let mut host = Host::default();
            let got = if structured {
                reference_claim::try_claim_effect(now, duration, &mut host).unwrap_err()
            } else {
                reference_claim::claim_effect(now, duration, &mut host).unwrap_err()
            };
            assert_eq!(got, expected);
            assert!(host.trace.borrow().is_empty());
        }
        let mut host = Host::default();
        assert_eq!(
            reference_claim::try_claim_effect(MAX_CLOCK_MS - 1, 1, &mut host).unwrap(),
            DispatchClaim::Idle
        );
        assert_eq!(*host.trace.borrow(), ["expire", "claim"]);
    }
}

#[test]
fn local_correspondence_failure_keeps_committed_claim_and_original_cache() {
    for conflict in [false, true] {
        let lease = lease();
        let mut existing = lease.request.continuation.clone();
        if conflict {
            existing.fuel_remaining -= 1;
        }
        let before = serde_json::to_vec(&existing).unwrap();
        let mut host = Host {
            claim: Some(DispatchClaim::Leased(lease)),
            existing: Some(existing),
            ..Host::default()
        };
        let got = reference_claim::try_claim_effect(12, 50, &mut host);
        if conflict {
            assert_eq!(
                got.unwrap_err(),
                Fault {
                    code: "LSV2005".into(),
                    message: "claimed continuation conflicts with local pending state".into()
                }
            );
        } else {
            assert!(matches!(got, Ok(DispatchClaim::Leased(_))));
        }
        assert!(host.committed);
        assert_eq!(
            serde_json::to_vec(host.existing.as_ref().unwrap()).unwrap(),
            before
        );
        assert_eq!(*host.trace.borrow(), ["expire", "claim", "adopt"]);
    }
}

#[test]
fn owner_errors_and_unwinds_never_repeat_cleanup_claim_or_adoption() {
    for stage in ["expire", "claim", "adopt"] {
        for unwind in [false, true] {
            let failure = Fault {
                code: "owner-error".into(),
                message: "original owner failure".into(),
            };
            let pointer = failure.message.as_ptr();
            let mut host = Host {
                claim: Some(DispatchClaim::Leased(lease())),
                failure: Some(failure),
                fail_at: (!unwind).then_some(stage),
                unwind_at: unwind.then_some(stage),
                ..Host::default()
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                reference_claim::try_claim_effect(12, 50, &mut host)
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap().unwrap_err().message.as_ptr(), pointer);
            }
            assert_eq!(host.committed, stage == "adopt");
            let trace = host.trace.borrow();
            assert_eq!(trace.iter().filter(|s| **s == stage).count(), 1);
            assert_eq!(trace.last(), Some(&stage));
        }
    }
}
