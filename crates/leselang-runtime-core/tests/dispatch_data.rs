use std::cell::Cell;
use std::rc::Rc;

use leselang_runtime_core::{
    DispatchLease, DispatchLeaseError, DispatchLeaseObservation, DispatchLeaseStamp, FailedEffect,
    RetrySchedule, RetrySchedulePoint, RetryTiming, next_dispatch_attempt, plan_dispatch_lease,
    plan_retry_schedule, validate_dispatch_lease, validate_retry_schedule,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct CounterRequest {
    register: u16,
    delta: i64,
}

#[test]
fn numeric_host_roundtrips_the_original_delivery_shape_and_zero_retry_default() {
    let wire = r#"{"request":{"register":3,"delta":7},"attempt":2,"retry_count":1,"lease_expires_at_ms":140}"#;
    let lease: DispatchLease<CounterRequest> = serde_json::from_str(wire).unwrap();
    assert_eq!(
        lease.request,
        CounterRequest {
            register: 3,
            delta: 7
        }
    );
    assert_eq!(serde_json::to_string(&lease).unwrap(), wire);
    let legacy = r#"{"request":{"register":3,"delta":7},"attempt":2,"lease_expires_at_ms":140,"extension":true}"#;
    let old: DispatchLease<CounterRequest> = serde_json::from_str(legacy).unwrap();
    assert_eq!(old.retry_count, 0);
    assert_eq!(old.request, lease.request);
    for bad in [
        r#"{"request":{"register":3,"delta":7,"extra":0},"attempt":2,"lease_expires_at_ms":140}"#,
        r#"{"request":{"register":3,"delta":7},"retry_count":1,"lease_expires_at_ms":140}"#,
        r#"{"request":{"register":3,"delta":7},"attempt":2,"retry_count":null,"lease_expires_at_ms":140}"#,
    ] {
        assert!(serde_json::from_str::<DispatchLease<CounterRequest>>(bad).is_err());
    }
}

#[test]
fn unrelated_failure_schemas_preserve_exact_fields_without_classification() {
    let numeric = FailedEffect {
        error: 17u16,
        retry_count: 2,
    };
    let text = RetrySchedule {
        retry_count: 1,
        ready_at_ms: 150,
        error: String::from("offline"),
    };
    assert_eq!(
        serde_json::to_string(&numeric).unwrap(),
        r#"{"error":17,"retry_count":2}"#
    );
    assert_eq!(
        serde_json::to_string(&text).unwrap(),
        r#"{"retry_count":1,"ready_at_ms":150,"error":"offline"}"#
    );
    assert_eq!(
        serde_json::from_str::<FailedEffect<u16>>(r#"{"error":17,"retry_count":2,"extra":false}"#)
            .unwrap(),
        numeric
    );
    assert_eq!(
        serde_json::from_str::<RetrySchedule<String>>(&serde_json::to_string(&text).unwrap())
            .unwrap(),
        text
    );
    assert!(
        serde_json::from_str::<FailedEffect<u16>>(r#"{"error":"17","retry_count":2}"#).is_err()
    );
    assert!(
        serde_json::from_str::<RetrySchedule<String>>(r#"{"retry_count":1,"ready_at_ms":150}"#)
            .is_err()
    );
}

struct GuiRequest<'a> {
    widget: &'a Cell<u32>,
    pixels: Box<[u8]>,
    owner: Rc<()>,
}
struct GuiError<'a>(&'a str);

fn retry<Request, Error>(
    request: Request,
    error: Error,
) -> (DispatchLease<Request>, RetrySchedule<Error>) {
    let point = plan_retry_schedule(
        0,
        100,
        RetryTiming {
            max_retries: 1,
            base_delay_ms: 10,
            max_delay_ms: 20,
        },
    )
    .unwrap()
    .unwrap();
    validate_retry_schedule(0, point, 100, 1).unwrap();
    let lease = DispatchLease {
        request,
        attempt: next_dispatch_attempt(1, 3).unwrap(),
        retry_count: point.retry_count,
        lease_expires_at_ms: plan_dispatch_lease(point.ready_at_ms, 10, 20).unwrap(),
    };
    let submitted = DispatchLeaseStamp {
        attempt: lease.attempt,
        retry_count: lease.retry_count,
        expires_at_ms: lease.lease_expires_at_ms,
    };
    let current = DispatchLeaseObservation {
        same_request: true,
        active: true,
        attempt: lease.attempt,
        retry_count: lease.retry_count,
        ready_at_ms: point.ready_at_ms,
        expires_at_ms: Some(lease.lease_expires_at_ms),
    };
    assert_eq!(
        validate_dispatch_lease(submitted, current, point.ready_at_ms - 1),
        Err(DispatchLeaseError::Superseded)
    );
    validate_dispatch_lease(submitted, current, point.ready_at_ms).unwrap();
    (
        lease,
        RetrySchedule {
            retry_count: point.retry_count,
            ready_at_ms: point.ready_at_ms,
            error,
        },
    )
}

#[test]
fn two_hosts_move_original_requests_and_errors_through_shared_retry_and_lease_rules() {
    let (numeric, error) = retry(
        CounterRequest {
            register: 3,
            delta: 7,
        },
        17u16,
    );
    assert_eq!(numeric.request.delta, 7);
    assert_eq!(error.error, 17);
    let widget = Cell::new(4);
    let error_text = String::from("generation changed");
    let pixels = vec![1, 2, 3].into_boxed_slice();
    let address = pixels.as_ptr();
    let owner = Rc::new(());
    let (gui, error) = retry(
        GuiRequest {
            widget: &widget,
            pixels,
            owner,
        },
        GuiError(&error_text),
    );
    assert_eq!(gui.attempt, numeric.attempt);
    assert_eq!(gui.retry_count, 1);
    assert_eq!(gui.request.pixels.as_ptr(), address);
    assert_eq!(Rc::strong_count(&gui.request.owner), 1);
    gui.request.widget.set(9);
    assert_eq!(widget.get(), 9);
    assert!(std::ptr::eq(error.error.0.as_ptr(), error_text.as_ptr()));
    // A host can move the same opaque refusal into its own terminal result.
    let failed = FailedEffect {
        error: error.error,
        retry_count: error.retry_count,
    };
    assert_eq!(failed.error.0, "generation changed");
}

#[test]
fn borrowed_unsized_request_and_error_views_do_not_require_static_or_owned_codecs() {
    let bytes = [1u8, 2, 3];
    let message = String::from("busy");
    let lease = DispatchLease {
        request: &bytes[..],
        attempt: 1,
        retry_count: 0,
        lease_expires_at_ms: 10,
    };
    let scheduled = RetrySchedule {
        retry_count: 1,
        ready_at_ms: 4,
        error: message.as_str(),
    };
    assert!(std::ptr::eq(lease.request.as_ptr(), bytes.as_ptr()));
    let failed = FailedEffect {
        error: scheduled.error,
        retry_count: scheduled.retry_count,
    };
    assert!(std::ptr::eq(failed.error.as_ptr(), message.as_ptr()));
}

#[test]
fn send_but_not_sync_payloads_move_between_workers_without_cloning() {
    let lease = DispatchLease {
        request: Box::new(Cell::new(3u32)),
        attempt: 2,
        retry_count: 1,
        lease_expires_at_ms: 110,
    };
    let schedule = RetrySchedule {
        error: Cell::new(7),
        retry_count: 1,
        ready_at_ms: 100,
    };
    let address = (&*lease.request as *const Cell<u32>) as usize;
    let (lease, failure) = std::thread::spawn(move || {
        lease.request.set(5);
        (
            lease,
            FailedEffect {
                error: schedule.error,
                retry_count: schedule.retry_count,
            },
        )
    })
    .join()
    .unwrap();
    assert_eq!((&*lease.request as *const Cell<u32>) as usize, address);
    assert_eq!(lease.request.get(), 5);
    assert_eq!(failure.error.get(), 7);
    assert_eq!(failure.retry_count, 1);
}

#[test]
fn decoded_counters_and_time_are_data_not_admitted_runtime_state() {
    let lease: DispatchLease<()> = serde_json::from_str(r#"{"request":null,"attempt":4294967295,"retry_count":4294967295,"lease_expires_at_ms":18446744073709551615}"#).unwrap();
    assert!(next_dispatch_attempt(lease.attempt, 3).is_err());
    let schedule: RetrySchedule<()> = serde_json::from_str(
        r#"{"retry_count":1,"ready_at_ms":18446744073709551615,"error":null}"#,
    )
    .unwrap();
    assert!(
        validate_retry_schedule(
            0,
            RetrySchedulePoint {
                retry_count: schedule.retry_count,
                ready_at_ms: schedule.ready_at_ms
            },
            100,
            3
        )
        .is_err()
    );
    assert_eq!(lease.retry_count, u32::MAX);
    assert_eq!(schedule.ready_at_ms, u64::MAX);
}

#[test]
fn drop_releases_opaque_payloads_once_without_native_trait_requirements() {
    struct Tracked<'a>(&'a Cell<u32>);
    impl Drop for Tracked<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }
    let dropped = Cell::new(0);
    let lease = DispatchLease {
        request: Tracked(&dropped),
        attempt: 1,
        retry_count: 0,
        lease_expires_at_ms: 10,
    };
    let scheduled = RetrySchedule {
        retry_count: 1,
        ready_at_ms: 5,
        error: Tracked(&dropped),
    };
    let failed = FailedEffect {
        retry_count: scheduled.retry_count,
        error: scheduled.error,
    };
    assert_eq!(dropped.get(), 0);
    drop(lease);
    assert_eq!(dropped.get(), 1);
    drop(failed);
    assert_eq!(dropped.get(), 2);
}
