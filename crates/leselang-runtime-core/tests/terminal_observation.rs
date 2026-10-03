use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_runtime_core::{
    Admission, AdmissionAdapter, AdmissionAttempt, AdmissionEnd, AdmissionPolicy, AdmissionPoll,
    AdmissionStatus, Fault,
};
use serde_json::json;

const PRIVATE_INPUT: &str = "PRIVATE_TERMINAL_INPUT";
const PRIVATE_ERROR: &str = "PRIVATE_TERMINAL_HOST_ERROR";

// Opaque, thread-local input and output deliberately implement neither Clone nor Debug.
struct Input {
    private: String,
    drops: Rc<Cell<usize>>,
    panic_on_drop: bool,
}

impl Drop for Input {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        assert!(!self.panic_on_drop, "host-owned cleanup failed");
    }
}

struct Ticket(Rc<Cell<usize>>);

impl Drop for Ticket {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

enum Mode {
    Started,
    Rejected,
    Busy,
    Panic,
}

struct Host {
    mode: Mode,
    calls: usize,
    publications: usize,
    output_drops: Rc<Cell<usize>>,
}

impl Host {
    fn new(mode: Mode) -> Self {
        Self {
            mode,
            calls: 0,
            publications: 0,
            output_drops: Rc::new(Cell::new(0)),
        }
    }
}

impl AdmissionAdapter<Input> for Host {
    type Started = Ticket;

    fn try_admit(&mut self, input: &Input, _: u64, _: u64) -> AdmissionAttempt<Ticket> {
        assert_eq!(input.private, PRIVATE_INPUT);
        self.calls += 1;
        match self.mode {
            Mode::Started => {
                self.publications += 1;
                AdmissionAttempt::Started(Ticket(self.output_drops.clone()))
            }
            Mode::Rejected => AdmissionAttempt::Rejected(Fault {
                code: "HOST_DENIED".into(),
                message: PRIVATE_ERROR.into(),
            }),
            Mode::Busy => AdmissionAttempt::Backpressured(Fault {
                code: "HOST_BUSY".into(),
                message: PRIVATE_ERROR.into(),
            }),
            Mode::Panic => {
                self.publications += 1;
                panic!("host publication state is unknown");
            }
        }
    }
}

fn handle(panic_on_drop: bool) -> (Admission<Input>, Rc<Cell<usize>>) {
    let drops = Rc::new(Cell::new(0));
    let handle = Admission::new(
        Input {
            private: PRIVATE_INPUT.into(),
            drops: drops.clone(),
            panic_on_drop,
        },
        100,
        1_000,
        AdmissionPolicy {
            max_attempts: 2,
            base_delay_ms: 10,
            max_delay_ms: 20,
        },
    )
    .unwrap();
    (handle, drops)
}

fn assert_terminal_stays(
    handle: &mut Admission<Input>,
    host: &mut Host,
    reason: AdmissionEnd,
    attempts: u32,
) {
    let calls = host.calls;
    for _ in 0..100 {
        assert_eq!(handle.terminal_reason(), Some(reason));
        assert_eq!(handle.status(), AdmissionStatus::Finished { attempts });
        assert!(!handle.cancel());
    }
    for now_ms in [0, 100, u64::MAX] {
        assert!(matches!(
            handle.poll(host, now_ms).unwrap(),
            AdmissionPoll::Finished
        ));
        assert_eq!(handle.terminal_reason(), Some(reason));
    }
    assert_eq!(host.calls, calls);
}

#[test]
fn pending_wait_and_clock_errors_never_fabricate_a_terminal_reason() {
    let (mut handle, drops) = handle(false);
    let mut host = Host::new(Mode::Busy);
    for _ in 0..100 {
        assert_eq!(handle.terminal_reason(), None);
    }
    assert_eq!(host.calls, 0);
    assert_eq!(drops.get(), 0);
    assert!(matches!(
        handle.poll(&mut host, 100).unwrap(),
        AdmissionPoll::Waiting(_)
    ));
    assert!(matches!(
        handle.poll(&mut host, 105).unwrap(),
        AdmissionPoll::Waiting(_)
    ));
    let status = handle.status();
    for (now_ms, code) in [(104, "LSV2511"), (u64::MAX, "LSV2011")] {
        let error = handle.poll(&mut host, now_ms).err().unwrap();
        assert_eq!(error.code, code);
        assert_eq!(handle.terminal_reason(), None);
        assert_eq!(handle.status(), status);
    }
    assert_eq!(host.calls, 1);
    assert_eq!(drops.get(), 0);
    assert_eq!(
        serde_json::to_value(handle.terminal_reason()).unwrap(),
        json!(null)
    );
}

#[test]
fn accepted_output_belongs_to_the_caller_not_terminal_metadata() {
    let (mut handle, drops) = handle(false);
    let mut host = Host::new(Mode::Started);
    let outcome = handle.poll(&mut host, 100).unwrap();
    assert!(matches!(&outcome, AdmissionPoll::Started(_)));
    assert_eq!(handle.terminal_reason(), Some(AdmissionEnd::Started));
    assert_eq!(drops.get(), 1);
    assert_eq!(host.output_drops.get(), 0);
    drop(outcome);
    assert_eq!(host.output_drops.get(), 1);
    assert_terminal_stays(&mut handle, &mut host, AdmissionEnd::Started, 1);
    assert_eq!(host.publications, 1);
    drop(handle);
    assert_eq!(drops.get(), 1);
    assert_eq!(host.output_drops.get(), 1);
}

#[test]
fn host_rejection_is_distinct_but_its_text_and_original_wire_stay_caller_owned() {
    let (mut handle, drops) = handle(false);
    let mut host = Host::new(Mode::Rejected);
    let AdmissionPoll::Rejected(error) = handle.poll(&mut host, 100).unwrap() else {
        panic!("expected host rejection");
    };
    assert_eq!(error.code, "HOST_DENIED");
    assert_eq!(error.message, PRIVATE_ERROR);
    assert_terminal_stays(&mut handle, &mut host, AdmissionEnd::Rejected, 1);
    assert_eq!(
        serde_json::to_value(handle.status()).unwrap(),
        json!({"kind":"finished","payload":{"attempts":1}})
    );
    for (reason, tag) in [
        (AdmissionEnd::Started, "started"),
        (AdmissionEnd::Rejected, "rejected"),
        (AdmissionEnd::Cancelled, "cancelled"),
        (AdmissionEnd::Expired, "expired"),
        (AdmissionEnd::Exhausted, "exhausted"),
        (AdmissionEnd::HostUncertain, "host_uncertain"),
    ] {
        assert_eq!(serde_json::to_value(reason).unwrap(), json!(tag));
    }
    let debug = format!("{handle:#?}");
    for private in [PRIVATE_INPUT, PRIVATE_ERROR, "HOST_DENIED"] {
        assert!(!debug.contains(private));
    }
    assert_eq!(drops.get(), 1);
    assert_eq!(host.publications, 0);
}

#[test]
fn local_cancellation_is_sticky_before_admission_and_while_waiting() {
    for waiting in [false, true] {
        let (mut handle, drops) = handle(false);
        let mut host = Host::new(Mode::Busy);
        if waiting {
            assert!(matches!(
                handle.poll(&mut host, 100).unwrap(),
                AdmissionPoll::Waiting(_)
            ));
        }
        assert!(handle.cancel());
        assert_terminal_stays(
            &mut handle,
            &mut host,
            AdmissionEnd::Cancelled,
            u32::from(waiting),
        );
        assert_eq!(drops.get(), 1);
        assert_eq!(host.publications, 0);
    }
}

#[test]
fn observed_deadline_has_its_own_reason_without_entering_the_host() {
    for waiting in [false, true] {
        let (mut handle, drops) = handle(false);
        let mut host = Host::new(Mode::Busy);
        if waiting {
            assert!(matches!(
                handle.poll(&mut host, 100).unwrap(),
                AdmissionPoll::Waiting(_)
            ));
        }
        assert_eq!(handle.terminal_reason(), None);
        assert!(
            matches!(handle.poll(&mut host, 1_100).unwrap(), AdmissionPoll::Rejected(error) if error.code == "LSV2513")
        );
        assert_terminal_stays(
            &mut handle,
            &mut host,
            AdmissionEnd::Expired,
            u32::from(waiting),
        );
        assert_eq!(drops.get(), 1);
        assert_eq!(host.calls, usize::from(waiting));
    }
}

#[test]
fn attempt_exhaustion_does_not_become_a_host_rejection_or_retain_private_pressure() {
    let (mut handle, drops) = handle(false);
    let mut host = Host::new(Mode::Busy);
    assert!(matches!(
        handle.poll(&mut host, 100).unwrap(),
        AdmissionPoll::Waiting(_)
    ));
    let AdmissionPoll::Rejected(error) = handle.poll(&mut host, 110).unwrap() else {
        panic!("expected exhausted attempts");
    };
    assert_eq!(error.code, "LSV2512");
    assert_eq!(error.message, "admission exhausted after 2 attempts");
    assert_terminal_stays(&mut handle, &mut host, AdmissionEnd::Exhausted, 2);
    assert!(!format!("{handle:?}").contains(PRIVATE_ERROR));
    assert_eq!(drops.get(), 1);
    assert_eq!(host.publications, 0);
}

#[test]
fn callback_unwind_records_uncertainty_not_false_rejection_or_replay_permission() {
    let (mut handle, drops) = handle(false);
    let mut host = Host::new(Mode::Panic);
    assert!(catch_unwind(AssertUnwindSafe(|| handle.poll(&mut host, 100))).is_err());
    assert_terminal_stays(&mut handle, &mut host, AdmissionEnd::HostUncertain, 1);
    assert_eq!(host.publications, 1);
    assert_eq!(drops.get(), 1);
    assert_eq!(host.output_drops.get(), 0);
}

#[test]
fn known_acceptance_survives_cleanup_unwind_but_does_not_claim_output_delivery() {
    let (mut handle, drops) = handle(true);
    let mut host = Host::new(Mode::Started);
    let mut delivered = false;
    let result = catch_unwind(AssertUnwindSafe(|| {
        let outcome = handle.poll(&mut host, 100).unwrap();
        delivered = true;
        drop(outcome);
    }));
    assert!(result.is_err());
    assert!(!delivered);
    assert_terminal_stays(&mut handle, &mut host, AdmissionEnd::Started, 1);
    assert_eq!(host.publications, 1);
    assert_eq!(drops.get(), 1);
    assert_eq!(host.output_drops.get(), 1);
    drop(handle);
    assert_eq!(drops.get(), 1);
}
