use std::cell::Cell;
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use leselang_runtime_core::{
    Admission, AdmissionAdapter, AdmissionAttempt, AdmissionPolicy, AdmissionPoll, AdmissionStatus,
    Fault,
};

fn busy() -> Fault {
    Fault {
        code: "HOST_BUSY".into(),
        message: "temporary host pressure".into(),
    }
}

fn policy() -> AdmissionPolicy {
    AdmissionPolicy {
        max_attempts: 3,
        base_delay_ms: 10,
        max_delay_ms: 20,
    }
}

// Cell makes this input Send but not Sync; no Clone or Debug implementation is needed.
struct TransferInput {
    value: Cell<u64>,
    drops: Arc<AtomicUsize>,
}

impl Drop for TransferInput {
    fn drop(&mut self) {
        self.drops.fetch_add(1, Ordering::SeqCst);
    }
}

fn handle(value: u64, drops: &Arc<AtomicUsize>) -> Admission<TransferInput> {
    Admission::new(
        TransferInput {
            value: Cell::new(value),
            drops: drops.clone(),
        },
        100,
        1_000,
        policy(),
    )
    .unwrap()
}

struct ValueHost {
    pressure: bool,
    calls: usize,
}

impl AdmissionAdapter<TransferInput> for ValueHost {
    type Started = u64;

    fn try_admit(
        &mut self,
        input: &TransferInput,
        _: u64,
        _: u64,
    ) -> AdmissionAttempt<Self::Started> {
        self.calls += 1;
        if self.pressure {
            AdmissionAttempt::Backpressured(busy())
        } else {
            AdmissionAttempt::Started(input.value.get())
        }
    }
}

#[test]
fn auto_traits_follow_input_and_output_instead_of_imposing_sync_bounds() {
    fn assert_send<T: Send>() {}
    fn assert_sync<T: Sync>() {}
    assert_send::<Admission<TransferInput>>();
    assert_send::<AdmissionPoll<u64>>();
    assert_sync::<Admission<u64>>();
    assert_send::<AdmissionStatus>();
    assert_sync::<AdmissionStatus>();
}

#[test]
fn gui_local_input_adapter_and_success_output_need_not_be_send() {
    struct LocalInput(Rc<Cell<u64>>);
    struct LocalOutput(Rc<Cell<u64>>);
    struct LocalHost {
        owner: thread::ThreadId,
        calls: Rc<Cell<usize>>,
    }
    impl AdmissionAdapter<LocalInput> for LocalHost {
        type Started = LocalOutput;

        fn try_admit(
            &mut self,
            input: &LocalInput,
            _: u64,
            _: u64,
        ) -> AdmissionAttempt<Self::Started> {
            assert_eq!(thread::current().id(), self.owner);
            let calls = self.calls.get() + 1;
            self.calls.set(calls);
            if calls == 1 {
                AdmissionAttempt::Backpressured(busy())
            } else {
                AdmissionAttempt::Started(LocalOutput(input.0.clone()))
            }
        }
    }

    let value = Rc::new(Cell::new(7));
    let calls = Rc::new(Cell::new(0));
    let mut admission = Admission::new(LocalInput(value.clone()), 100, 1_000, policy()).unwrap();
    let mut local = LocalHost {
        owner: thread::current().id(),
        calls: calls.clone(),
    };
    let host: &mut dyn AdmissionAdapter<LocalInput, Started = LocalOutput> = &mut local;
    let AdmissionPoll::Waiting(wait) = admission.poll(host, 100).unwrap() else {
        panic!("first local attempt should wait");
    };
    let AdmissionPoll::Started(output) = admission.poll(host, wait.retry_at_ms).unwrap() else {
        panic!("second local attempt should start");
    };
    assert!(Rc::ptr_eq(&value, &output.0));
    assert_eq!(output.0.get(), 7);
    assert_eq!(calls.get(), 2);
    assert!(!admission.cancel());
    assert!(matches!(
        admission.poll(host, 120).unwrap(),
        AdmissionPoll::Finished
    ));

    let mut cancelled = Admission::new(LocalInput(value.clone()), 100, 1_000, policy()).unwrap();
    assert!(cancelled.cancel());
    assert!(matches!(
        cancelled.poll(host, 100).unwrap(),
        AdmissionPoll::Finished
    ));
    assert_eq!(calls.get(), 2);
}

#[test]
fn waiting_handle_can_move_workers_without_resetting_attempt_clock_or_deadline() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut admission = handle(42, &drops);
    let mut first_host = ValueHost {
        pressure: true,
        calls: 0,
    };
    let AdmissionPoll::Waiting(wait) = admission.poll(&mut first_host, 100).unwrap() else {
        panic!("expected pressure before handoff");
    };
    assert_eq!(
        admission.poll(&mut first_host, 109).unwrap(),
        AdmissionPoll::Waiting(wait)
    );
    let owner = thread::current().id();
    let worker = thread::spawn(move || {
        assert_ne!(thread::current().id(), owner);
        assert_eq!(admission.status(), AdmissionStatus::Pending(wait));
        let mut host = ValueHost {
            pressure: false,
            calls: 0,
        };
        assert_eq!(admission.poll(&mut host, 108).unwrap_err().code, "LSV2511");
        assert_eq!(
            admission.poll(&mut host, 109).unwrap(),
            AdmissionPoll::Waiting(wait)
        );
        assert_eq!(host.calls, 0);
        assert_eq!(
            admission.poll(&mut host, 110).unwrap(),
            AdmissionPoll::Started(42)
        );
        assert_eq!(host.calls, 1);
        admission
    });
    let mut admission = worker.join().unwrap();
    assert_eq!(
        admission.status(),
        AdmissionStatus::Finished { attempts: 2 }
    );
    assert_eq!(drops.load(Ordering::SeqCst), 1);
    assert_eq!(first_host.calls, 1);
    first_host.pressure = false;
    assert_eq!(
        admission.poll(&mut first_host, u64::MAX).unwrap(),
        AdmissionPoll::Finished
    );
    assert_eq!(first_host.calls, 1);
    assert!(!admission.cancel());
}

struct GatedHost {
    entered: SyncSender<u64>,
    permit: Receiver<()>,
    calls: usize,
}

impl AdmissionAdapter<TransferInput> for GatedHost {
    type Started = u64;

    fn try_admit(
        &mut self,
        input: &TransferInput,
        _: u64,
        _: u64,
    ) -> AdmissionAttempt<Self::Started> {
        self.calls += 1;
        self.entered.send(input.value.get()).unwrap();
        // Test-only rendezvous, bounded so a lock regression cannot hang the suite.
        self.permit.recv_timeout(Duration::from_secs(10)).unwrap();
        AdmissionAttempt::Started(input.value.get())
    }
}

type WorkerResult = (Admission<TransferInput>, AdmissionPoll<u64>, usize);

fn gated_worker(
    admission: Admission<TransferInput>,
    entered: SyncSender<u64>,
) -> (SyncSender<()>, JoinHandle<WorkerResult>) {
    let (release, permit) = mpsc::sync_channel(1);
    let worker = thread::spawn(move || {
        let mut admission = admission;
        let mut host = GatedHost {
            entered,
            permit,
            calls: 0,
        };
        let result = admission.poll(&mut host, 100).unwrap();
        (admission, result, host.calls)
    });
    (release, worker)
}

#[test]
fn independent_callbacks_overlap_without_a_process_global_admission_lock() {
    let drops = Arc::new(AtomicUsize::new(0));
    let (entered, observations) = mpsc::sync_channel(2);
    let (release_a, worker_a) = gated_worker(handle(1, &drops), entered.clone());
    let (release_b, worker_b) = gated_worker(handle(2, &drops), entered);
    let first = observations.recv_timeout(Duration::from_secs(5));
    let second = observations.recv_timeout(Duration::from_secs(5));
    // Release both before asserting, including when either rendezvous failed.
    let _ = release_a.send(());
    let _ = release_b.send(());
    let result_a = worker_a.join();
    let result_b = worker_b.join();
    let mut ids = [first.unwrap(), second.unwrap()];
    ids.sort();
    assert_eq!(ids, [1, 2]);
    for (expected, result) in [(1, result_a), (2, result_b)] {
        let (admission, outcome, calls) = result.unwrap();
        assert_eq!(outcome, AdmissionPoll::Started(expected));
        assert_eq!(calls, 1);
        assert_eq!(
            admission.status(),
            AdmissionStatus::Finished { attempts: 1 }
        );
    }
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

#[test]
fn cancelling_one_handle_does_not_wait_for_or_cancel_another_hosts_callback() {
    let drops = Arc::new(AtomicUsize::new(0));
    let mut cancelled = handle(1, &drops);
    let (entered, observations) = mpsc::sync_channel(1);
    let (release, worker) = gated_worker(handle(2, &drops), entered);
    let observed = observations.recv_timeout(Duration::from_secs(5));
    let did_cancel = cancelled.cancel();
    let cancelled_status = cancelled.status();
    let _ = release.send(());
    let (mut accepted, outcome, calls) = worker.join().unwrap();
    assert_eq!(observed.unwrap(), 2);
    assert!(did_cancel);
    assert_eq!(cancelled_status, AdmissionStatus::Finished { attempts: 0 });
    assert_eq!(outcome, AdmissionPoll::Started(2));
    assert_eq!(calls, 1);
    assert!(!accepted.cancel());
    assert!(!cancelled.cancel());
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}

#[test]
fn callback_unwind_is_local_and_does_not_poison_sibling_admission() {
    struct PanickingHost(usize);
    impl AdmissionAdapter<TransferInput> for PanickingHost {
        type Started = u64;
        fn try_admit(&mut self, _: &TransferInput, _: u64, _: u64) -> AdmissionAttempt<u64> {
            self.0 += 1;
            panic!("host callback failed");
        }
    }
    let drops = Arc::new(AtomicUsize::new(0));
    let mut failed = handle(1, &drops);
    let worker = thread::spawn(move || {
        let mut host = PanickingHost(0);
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                failed.poll(&mut host, 100)
            }))
            .is_err()
        );
        assert_eq!(
            failed.poll(&mut host, 110).unwrap(),
            AdmissionPoll::Finished
        );
        assert_eq!(host.0, 1);
        failed
    });
    let mut sibling = handle(2, &drops);
    let mut host = ValueHost {
        pressure: false,
        calls: 0,
    };
    assert_eq!(
        sibling.poll(&mut host, 100).unwrap(),
        AdmissionPoll::Started(2)
    );
    let failed = worker.join().unwrap();
    assert_eq!(failed.status(), AdmissionStatus::Finished { attempts: 1 });
    assert_eq!(sibling.status(), AdmissionStatus::Finished { attempts: 1 });
    assert_eq!(host.calls, 1);
    assert_eq!(drops.load(Ordering::SeqCst), 2);
}
