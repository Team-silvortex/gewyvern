use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::reference_retry_commit::*;
use leselang_command::{
    DispatchLease, EffectError, EffectErrorClass, RetryDisposition, RetrySchedule, Step, Value,
};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::{DispatchLeaseObservation, Fault};
use leserpent_hir::{Effect, Type};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn lease() -> DispatchLease {
    lease_with_sequence(7)
}
fn lease_with_sequence(sequence: u64) -> DispatchLease {
    DispatchLease {
        request: build_effect_request(ReservedEffectInput {
            effect: &Effect::UiFocus {
                node_id: "node".into(),
            },
            result_type: Type::UiFocus,
            sequence,
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
        lease_expires_at_ms: 100,
    }
}
fn error() -> EffectError {
    EffectError {
        class: EffectErrorClass::Transient,
        code: "busy".into(),
        message: "busy now".into(),
    }
}
fn scheduled() -> RetryDisposition {
    RetryDisposition::Scheduled(RetrySchedule {
        retry_count: 1,
        ready_at_ms: 20,
        error: error(),
    })
}
fn terminal() -> RetryDisposition {
    RetryDisposition::Terminal(Step::Fault(Fault {
        code: "HOST".into(),
        message: "stopped".into(),
    }))
}
fn invalid() -> RetryDisposition {
    RetryDisposition::Terminal(Step::Yield(lease().request.continuation))
}
#[derive(Default)]
struct Hooks {
    trace: Rc<RefCell<Vec<&'static str>>>,
    fail: Option<&'static str>,
    unwind: Option<&'static str>,
    error: Option<Fault>,
}
impl Hooks {
    fn enter(&mut self, name: &'static str) -> Result<(), Fault> {
        self.trace.borrow_mut().push(name);
        assert_ne!(self.unwind, Some(name), "native unwind");
        if self.fail == Some(name) {
            return Err(self.error.take().unwrap());
        }
        Ok(())
    }
}
struct Memory {
    hooks: Hooks,
    present: bool,
    same_request: bool,
    active: bool,
    attempt: u32,
    retry: u32,
    ready: u64,
    error: Option<EffectError>,
    expires: Option<u64>,
    acknowledged: bool,
    terminal: Option<Step>,
}
impl Memory {
    fn new() -> Self {
        Self {
            hooks: Hooks::default(),
            present: true,
            same_request: true,
            active: true,
            attempt: 1,
            retry: 0,
            ready: 0,
            error: None,
            expires: Some(100),
            acknowledged: false,
            terminal: None,
        }
    }
}
impl MemoryRetryJournal for Memory {
    fn dispatch(&mut self, _: &DispatchLease) -> Option<MemoryRetryState<'_>> {
        self.hooks.enter("dispatch").unwrap();
        self.present.then_some(MemoryRetryState {
            observation: DispatchLeaseObservation {
                same_request: self.same_request,
                active: self.active,
                attempt: self.attempt,
                retry_count: self.retry,
                ready_at_ms: self.ready,
                expires_at_ms: self.expires,
            },
            retry_count: &mut self.retry,
            ready_at_ms: &mut self.ready,
            last_error: &mut self.error,
            lease_expires_at_ms: &mut self.expires,
            acknowledged: &mut self.acknowledged,
            terminal_step: &mut self.terminal,
        })
    }
    fn finalize_group(&mut self, _: &leselang_command::ContinuationToken) -> Result<(), Fault> {
        assert!(self.acknowledged && self.expires.is_none() && self.terminal.is_some());
        self.hooks.enter("finalize")
    }
}

#[test]
fn memory_lease_fences_precede_proposal_validation_without_partial_mutation() {
    for kind in 0..8 {
        let mut host = Memory::new();
        match kind {
            0 => host.present = false,
            1 => host.same_request = false,
            2 => host.active = false,
            3 => host.attempt = 2,
            4 => host.retry = 1,
            5 => host.ready = 30,
            6 => host.expires = Some(99),
            _ => {}
        }
        let now = if kind == 7 { 101 } else { 10 };
        assert_eq!(
            report_memory(&mut host, &lease(), now, &invalid())
                .unwrap_err()
                .code,
            if kind == 0 {
                "LSV4021"
            } else if kind == 7 {
                "LSV4023"
            } else {
                "LSV4022"
            }
        );
        assert!(!host.acknowledged && host.terminal.is_none() && host.error.is_none());
        assert_eq!(*host.hooks.trace.borrow(), ["dispatch"]);
    }
    let mut host = Memory::new();
    // The released lease boundary is inclusive; proposal validation still runs.
    assert_eq!(
        report_memory(&mut host, &lease(), 100, &invalid())
            .unwrap_err()
            .code,
        "LSV4007"
    );
    assert_eq!(
        report_memory(&mut host, &lease(), 10, &invalid())
            .unwrap_err()
            .code,
        "LSV4007"
    );
    assert_eq!(host.expires, Some(100));
}

#[test]
fn memory_schedule_updates_only_retry_slots_and_never_finalizes() {
    let mut host = Memory::new();
    let local: &mut dyn MemoryRetryJournal = &mut host;
    assert_eq!(
        report_memory(local, &lease(), 10, &scheduled()).unwrap(),
        scheduled()
    );
    assert_eq!((host.retry, host.ready, host.expires), (1, 20, None));
    assert_eq!(host.error, Some(error()));
    assert!(!host.acknowledged && host.terminal.is_none());
    assert_eq!(*host.hooks.trace.borrow(), ["dispatch"]);
    assert_eq!(
        report_memory(&mut host, &lease(), 11, &scheduled())
            .unwrap_err()
            .code,
        "LSV4022"
    );
}

#[test]
fn memory_finalize_fault_and_unwind_preserve_prior_mutation_without_retry() {
    for unwind in [false, true] {
        let mut host = Memory::new();
        host.hooks.error = Some(Fault {
            code: "NATIVE".into(),
            message: "original buffer".into(),
        });
        let pointer = host.hooks.error.as_ref().unwrap().message.as_ptr();
        if unwind {
            host.hooks.unwind = Some("finalize");
        } else {
            host.hooks.fail = Some("finalize");
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            report_memory(&mut host, &lease(), 10, &terminal())
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().unwrap_err().message.as_ptr(), pointer);
        }
        let RetryDisposition::Terminal(step) = terminal() else {
            unreachable!()
        };
        assert_eq!(host.terminal, Some(step));
        assert!(host.acknowledged && host.expires.is_none());
        assert_eq!((host.retry, host.ready, host.error), (0, 0, None));
        assert_eq!(*host.hooks.trace.borrow(), ["dispatch", "finalize"]);
    }
}

struct Durable {
    hooks: Hooks,
    effect: Option<StoredEffect>,
    dispatch: Option<StoredDispatch>,
    encoded_pointer: *const u8,
    committed: Rc<Cell<bool>>,
}
impl Durable {
    fn new() -> Self {
        Self {
            hooks: Hooks::default(),
            effect: Some(StoredEffect {
                image: b"image".to_vec(),
                state: "pending".into(),
                terminal_step: None,
            }),
            dispatch: Some(StoredDispatch {
                request: serde_json::to_vec(&lease().request).unwrap(),
                state: "leased".into(),
                attempt: 1,
                lease_expires_at_ms: Some(100),
                ready_at_ms: 0,
                retry_count: 0,
            }),
            encoded_pointer: std::ptr::null(),
            committed: Rc::default(),
        }
    }
    fn replay(step: &Step) -> Self {
        let mut host = Self::new();
        let record = host.effect.as_mut().unwrap();
        record.state = "completed".into();
        record.terminal_step = Some(serde_json::to_vec(step).unwrap());
        host
    }
    fn encode(&mut self, stage: &'static str) -> Result<Vec<u8>, Fault> {
        self.hooks.enter(stage)?;
        let bytes = b"encoded original".to_vec();
        self.encoded_pointer = bytes.as_ptr();
        Ok(bytes)
    }
}
impl Drop for Durable {
    fn drop(&mut self) {
        self.hooks.trace.borrow_mut().push("drop");
    }
}
impl DurableRetryTransaction for Durable {
    fn effect(&mut self, _: &str) -> Result<Option<StoredEffect>, Fault> {
        self.hooks.enter("effect")?;
        Ok(self.effect.take())
    }
    fn dispatch(&mut self, _: &str) -> Result<Option<StoredDispatch>, Fault> {
        self.hooks.enter("dispatch")?;
        Ok(self.dispatch.take())
    }
    fn encode_error(&mut self, e: &EffectError) -> Result<Vec<u8>, Fault> {
        assert_eq!(*e, error());
        self.encode("error")
    }
    fn encode_terminal(&mut self, _: &Step) -> Result<Vec<u8>, Fault> {
        self.encode("terminal")
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        assert_eq!(additional, 16);
        self.hooks.enter("growth")
    }
    fn schedule(&mut self, _: &str, retry: u32, ready: i64, bytes: &[u8]) -> Result<(), Fault> {
        assert_eq!((retry, ready), (1, 20));
        assert_eq!(bytes.as_ptr(), self.encoded_pointer);
        self.hooks.enter("schedule")
    }
    fn complete(&mut self, _: &str, bytes: &[u8]) -> Result<(), Fault> {
        assert_eq!(bytes.as_ptr(), self.encoded_pointer);
        self.hooks.enter("complete")
    }
    fn finalize_group(&mut self, _: &str) -> Result<(), Fault> {
        self.hooks.enter("finalize")
    }
    fn commit(mut self, kind: RetryCommit) -> Result<(), Fault> {
        self.committed.set(true);
        self.hooks.enter(if kind == RetryCommit::Replay {
            "replay"
        } else {
            "commit"
        })
    }
}
fn run(host: Durable, disposition: &RetryDisposition) -> Result<RetryDisposition, Fault> {
    report_durable(host, &lease(), 10, disposition, b"image")
}

#[test]
fn durable_fresh_paths_keep_order_and_exact_encoded_buffers() {
    for state in ["pending", "unknown"] {
        for proposed in [scheduled(), terminal()] {
            let mut host = Durable::new();
            host.effect.as_mut().unwrap().state = state.into();
            let trace = host.hooks.trace.clone();
            assert_eq!(run(host, &proposed).unwrap(), proposed);
            let expected: &[&str] = if matches!(proposed, RetryDisposition::Scheduled(_)) {
                &[
                    "effect", "dispatch", "error", "growth", "schedule", "commit", "drop",
                ]
            } else {
                &[
                    "effect", "dispatch", "terminal", "growth", "complete", "finalize", "commit",
                    "drop",
                ]
            };
            assert_eq!(*trace.borrow(), expected);
        }
    }
}

#[test]
fn durable_terminal_replay_precedes_lease_and_proposal_but_keeps_image_and_terminal_gates() {
    for winner in [
        Step::Done(Value::UiFocus {
            node_id: "original".into(),
        }),
        Step::Fault(Fault {
            code: "HOST".into(),
            message: "old".into(),
        }),
    ] {
        let mut host = Durable::replay(&winner);
        host.dispatch = None;
        let trace = host.hooks.trace.clone();
        let mut stale = lease();
        stale.attempt = 0;
        assert_eq!(
            report_durable(host, &stale, u64::MAX, &invalid(), b"image").unwrap(),
            RetryDisposition::Terminal(winner)
        );
        assert_eq!(*trace.borrow(), ["effect", "replay", "drop"]);
    }
    for kind in 0..6 {
        let mut host = Durable::replay(&Step::Yield(lease().request.continuation));
        match kind {
            0 => host.effect = None,
            1 => host.effect.as_mut().unwrap().image.clear(),
            2 => host.effect.as_mut().unwrap().terminal_step = None,
            3 => host.effect.as_mut().unwrap().terminal_step = Some(b"{".to_vec()),
            4 => {
                host.effect.as_mut().unwrap().terminal_step = Some(vec![
                    b' ';
                    leselang_command::reference_recovery::MAX_JOURNAL_ENTRY_BYTES
                        + 1
                ])
            }
            _ => {}
        }
        let trace = host.hooks.trace.clone();
        assert_eq!(
            run(host, &invalid()).unwrap_err().code,
            [
                "LSV4021", "LSV4014", "LSV4007", "LSV4007", "LSV4006", "LSV4007"
            ][kind]
        );
        assert_eq!(*trace.borrow(), ["effect", "drop"]);
    }
}

#[test]
fn durable_pending_lease_and_decode_refusals_precede_proposal_codecs() {
    for kind in 0..8 {
        let mut host = Durable::new();
        match kind {
            0 => host.dispatch = None,
            1 => host.dispatch.as_mut().unwrap().request = b"{".to_vec(),
            2 => host.dispatch.as_mut().unwrap().state = "ready".into(),
            3 => host.dispatch.as_mut().unwrap().attempt = 2,
            4 => host.dispatch.as_mut().unwrap().retry_count = 1,
            5 => host.dispatch.as_mut().unwrap().lease_expires_at_ms = None,
            6 => {
                let request = lease_with_sequence(8).request;
                host.dispatch.as_mut().unwrap().request = serde_json::to_vec(&request).unwrap();
            }
            _ => {}
        }
        let trace = host.hooks.trace.clone();
        assert_eq!(
            run(host, &invalid()).unwrap_err().code,
            if kind == 0 {
                "LSV4021"
            } else if kind == 1 || kind == 7 {
                "LSV4007"
            } else {
                "LSV4022"
            },
            "case {kind}"
        );
        assert_eq!(*trace.borrow(), ["effect", "dispatch", "drop"]);
    }
    let mut proposed = scheduled();
    let RetryDisposition::Scheduled(schedule) = &mut proposed else {
        unreachable!()
    };
    schedule.retry_count = u32::MAX;
    let host = Durable::new();
    let trace = host.hooks.trace.clone();
    assert_eq!(run(host, &proposed).unwrap_err().code, "LSV4026");
    assert_eq!(*trace.borrow(), ["effect", "dispatch", "drop"]);
}

#[test]
fn durable_faults_and_unwinds_drop_once_without_compensating_for_uncertain_commit() {
    for route in 0..3 {
        let stages: &[&str] = match route {
            0 => &[
                "effect", "dispatch", "error", "growth", "schedule", "commit",
            ],
            1 => &[
                "effect", "dispatch", "terminal", "growth", "complete", "finalize", "commit",
            ],
            _ => &["effect", "replay"],
        };
        for (index, stage) in stages.iter().enumerate() {
            for unwind in [false, true] {
                let mut host = if route == 2 {
                    Durable::replay(&Step::Done(Value::UiFocus {
                        node_id: "old".into(),
                    }))
                } else {
                    Durable::new()
                };
                let trace = host.hooks.trace.clone();
                let committed = host.committed.clone();
                host.hooks.error = Some(Fault {
                    code: "NATIVE".into(),
                    message: "original error buffer".into(),
                });
                let pointer = host.hooks.error.as_ref().unwrap().message.as_ptr();
                if unwind {
                    host.hooks.unwind = Some(stage);
                } else {
                    host.hooks.fail = Some(stage);
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(host, &if route == 0 { scheduled() } else { terminal() })
                }));
                if unwind {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap().unwrap_err().message.as_ptr(), pointer);
                }
                let mut expected = stages[..=index].to_vec();
                expected.push("drop");
                assert_eq!(*trace.borrow(), expected);
                assert_eq!(committed.get(), index + 1 == stages.len());
            }
        }
    }
}
