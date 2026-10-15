use leselang_runtime_core::{
    DispatchLeaseError, DispatchLeaseObservation, DispatchLeaseStamp, validate_dispatch_lease,
};

fn submitted() -> DispatchLeaseStamp {
    DispatchLeaseStamp {
        attempt: 2,
        retry_count: 1,
        expires_at_ms: 110,
    }
}

fn current() -> DispatchLeaseObservation {
    DispatchLeaseObservation {
        same_request: true,
        active: true,
        attempt: 2,
        retry_count: 1,
        ready_at_ms: 100,
        expires_at_ms: Some(110),
    }
}

#[test]
fn exact_expiry_remains_valid_only_for_the_current_attempt() {
    assert_eq!(validate_dispatch_lease(submitted(), current(), 100), Ok(()));
    assert_eq!(validate_dispatch_lease(submitted(), current(), 110), Ok(()));
    assert_eq!(
        validate_dispatch_lease(submitted(), current(), 111),
        Err(DispatchLeaseError::Expired)
    );
    let mut next = current();
    next.attempt += 1;
    for now in [110, 111, u64::MAX] {
        assert_eq!(
            validate_dispatch_lease(submitted(), next, now),
            Err(DispatchLeaseError::Superseded)
        );
    }
}

#[test]
fn every_current_field_precedes_expiration_without_normalization() {
    let mut checked = 0;
    for same_request in [false, true] {
        for active in [false, true] {
            for attempt in [0, 2, u32::MAX] {
                for retry_count in [0, 1, u32::MAX] {
                    for ready_at_ms in [0, 110, u64::MAX] {
                        for expires_at_ms in [None, Some(0), Some(110), Some(u64::MAX)] {
                            for now in [109, 110, 111] {
                                let observation = DispatchLeaseObservation {
                                    same_request,
                                    active,
                                    attempt,
                                    retry_count,
                                    ready_at_ms,
                                    expires_at_ms,
                                };
                                let expected = if same_request
                                    && active
                                    && attempt == 2
                                    && retry_count == 1
                                    && ready_at_ms <= now
                                    && expires_at_ms == Some(110)
                                {
                                    if now <= 110 {
                                        Ok(())
                                    } else {
                                        Err(DispatchLeaseError::Expired)
                                    }
                                } else {
                                    Err(DispatchLeaseError::Superseded)
                                };
                                assert_eq!(
                                    validate_dispatch_lease(submitted(), observation, now),
                                    expected
                                );
                                checked += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(checked, 1296);
}

#[test]
fn full_width_comparison_does_not_add_time_or_repair_host_bounds() {
    // Counter/clock policy is explicitly native, including validity of zero.
    for value in [0, u64::MAX] {
        let stamp = DispatchLeaseStamp {
            attempt: 0,
            retry_count: u32::MAX,
            expires_at_ms: value,
        };
        let observation = DispatchLeaseObservation {
            same_request: true,
            active: true,
            attempt: stamp.attempt,
            retry_count: stamp.retry_count,
            ready_at_ms: value,
            expires_at_ms: Some(value),
        };
        assert_eq!(validate_dispatch_lease(stamp, observation, value), Ok(()));
    }
}

#[test]
fn observations_are_not_reservations_and_must_be_read_again_after_mutation() {
    let old = current();
    assert_eq!(validate_dispatch_lease(submitted(), old, 110), Ok(()));
    let cancelled = DispatchLeaseObservation {
        active: false,
        expires_at_ms: None,
        ..old
    };
    assert_eq!(
        validate_dispatch_lease(submitted(), cancelled, 110),
        Err(DispatchLeaseError::Superseded)
    );
    // A pure function cannot detect a stale observation or supply storage atomicity.
    assert_eq!(validate_dispatch_lease(submitted(), old, 110), Ok(()));
}

#[test]
fn opaque_gui_identity_and_authority_remain_outside_numeric_lease_checks() {
    use std::{cell::Cell, rc::Rc};
    struct NativeRequest {
        node: Rc<Cell<bool>>,
    }
    let original = NativeRequest {
        node: Rc::new(Cell::new(true)),
    };
    let unrelated = NativeRequest {
        node: Rc::new(Cell::new(true)),
    };
    let observe = |request: &NativeRequest| DispatchLeaseObservation {
        same_request: Rc::ptr_eq(&request.node, &original.node),
        active: original.node.get(),
        ..current()
    };
    assert_eq!(
        validate_dispatch_lease(submitted(), observe(&unrelated), 110),
        Err(DispatchLeaseError::Superseded)
    );
    assert_eq!(
        validate_dispatch_lease(submitted(), observe(&original), 110),
        Ok(())
    );
    original.node.set(false);
    assert_eq!(
        validate_dispatch_lease(submitted(), observe(&original), 110),
        Err(DispatchLeaseError::Superseded)
    );
}
