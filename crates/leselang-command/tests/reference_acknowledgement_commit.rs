use leselang_command::reference_acknowledgement_commit::*;
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::{ContinuationToken, DispatchLease, EffectRequest, Step, Value};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;
use leserpent_hir::{Effect, Type};
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

fn lease(sequence: u64) -> DispatchLease {
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
fn done(node: &str) -> Step {
    Step::Done(Value::UiFocus {
        node_id: node.into(),
    })
}
fn invalid() -> Step {
    Step::Yield(lease(7).request.continuation)
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
    request: EffectRequest,
    attempt: u32,
    retry: u32,
    ready: u64,
    acknowledged: bool,
    expires: Option<u64>,
    terminal: Option<Step>,
}
impl Memory {
    fn new() -> Self {
        Self {
            hooks: Hooks::default(),
            present: true,
            request: lease(7).request,
            attempt: 1,
            retry: 0,
            ready: 0,
            acknowledged: false,
            expires: Some(100),
            terminal: None,
        }
    }
}
impl MemoryAcknowledgementJournal for Memory {
    fn dispatch(&mut self, _: &ContinuationToken) -> Option<MemoryAcknowledgementState<'_>> {
        self.hooks.enter("dispatch").unwrap();
        self.present.then_some(MemoryAcknowledgementState {
            request: &self.request,
            attempt: self.attempt,
            retry_count: self.retry,
            ready_at_ms: self.ready,
            acknowledged: &mut self.acknowledged,
            lease_expires_at_ms: &mut self.expires,
            terminal_step: &mut self.terminal,
        })
    }
    fn finalize_group(&mut self, _: &ContinuationToken) -> Result<(), Fault> {
        assert!(self.acknowledged && self.expires.is_none() && self.terminal.is_some());
        self.hooks.enter("finalize")
    }
}

#[test]
fn memory_checks_proposal_before_lookup_and_fences_lease_before_mutation() {
    for present in [false, true] {
        let mut host = Memory::new();
        host.present = present;
        host.acknowledged = true;
        assert_eq!(
            acknowledge_memory(&mut host, &lease(7), 10, &invalid())
                .unwrap_err()
                .code,
            "LSV4007"
        );
        assert!(host.hooks.trace.borrow().is_empty());
    }
    for kind in 0..8 {
        let mut host = Memory::new();
        match kind {
            0 => host.present = false,
            1 => host.request = lease(8).request,
            2 => host.attempt = 2,
            3 => host.retry = 1,
            4 => host.ready = 11,
            5 => host.expires = None,
            6 => host.expires = Some(99),
            _ => {}
        }
        let now = if kind == 7 { 101 } else { 10 };
        assert_eq!(
            acknowledge_memory(&mut host, &lease(7), now, &done("new"))
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
        assert!(!host.acknowledged && host.terminal.is_none());
        assert_eq!(*host.hooks.trace.borrow(), ["dispatch"]);
    }
}

#[test]
fn memory_completed_returns_proposal_without_lease_validation_or_terminal_replacement() {
    for saved in [None, Some(done("old")), Some(invalid())] {
        let mut host = Memory::new();
        host.acknowledged = true;
        host.terminal = saved.clone();
        host.request = lease(8).request;
        host.expires = None;
        let local: &mut dyn MemoryAcknowledgementJournal = &mut host;
        assert_eq!(
            acknowledge_memory(local, &lease(7), u64::MAX, &done("new")).unwrap(),
            done("new")
        );
        assert_eq!(host.terminal, saved);
        assert!(host.expires.is_none());
        assert_eq!(*host.hooks.trace.borrow(), ["dispatch"]);
    }
}

#[test]
fn memory_completion_keeps_inclusive_expiry_and_partial_finalize_fault_or_unwind() {
    for mode in 0..3 {
        let mut host = Memory::new();
        host.hooks.error = Some(Fault {
            code: "NATIVE".into(),
            message: "original fault buffer".into(),
        });
        let pointer = host.hooks.error.as_ref().unwrap().message.as_ptr();
        if mode == 1 {
            host.hooks.fail = Some("finalize");
        } else if mode == 2 {
            host.hooks.unwind = Some("finalize");
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            acknowledge_memory(&mut host, &lease(7), 100, &done("new"))
        }));
        match mode {
            0 => assert_eq!(result.unwrap().unwrap(), done("new")),
            1 => assert_eq!(result.unwrap().unwrap_err().message.as_ptr(), pointer),
            _ => assert!(result.is_err()),
        }
        assert!(host.acknowledged && host.expires.is_none());
        assert_eq!(host.terminal, Some(done("new")));
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
                request: serde_json::to_vec(&lease(7).request).unwrap(),
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
}
impl Drop for Durable {
    fn drop(&mut self) {
        self.hooks.trace.borrow_mut().push("drop");
    }
}
impl DurableAcknowledgementTransaction for Durable {
    fn effect(&mut self, _: &str) -> Result<Option<StoredEffect>, Fault> {
        self.hooks.enter("effect")?;
        Ok(self.effect.take())
    }
    fn dispatch(&mut self, _: &str) -> Result<Option<StoredDispatch>, Fault> {
        self.hooks.enter("dispatch")?;
        Ok(self.dispatch.take())
    }
    fn ensure_growth(&mut self, size: usize) -> Result<(), Fault> {
        assert_eq!(size, 4);
        self.hooks.enter("growth")
    }
    fn complete_effect(&mut self, _: &str, bytes: &[u8]) -> Result<(), Fault> {
        assert_eq!(bytes.as_ptr(), self.encoded_pointer);
        self.hooks.enter("complete")
    }
    fn acknowledge_dispatch(&mut self, _: &str) -> Result<(), Fault> {
        self.hooks.enter("acknowledge")
    }
    fn finalize_group(&mut self, _: &str) -> Result<(), Fault> {
        self.hooks.enter("finalize")
    }
    fn commit(mut self, kind: AcknowledgementCommit) -> Result<(), Fault> {
        self.committed.set(true);
        self.hooks.enter(if kind == AcknowledgementCommit::Replay {
            "replay"
        } else {
            "commit"
        })
    }
}
fn run(mut host: Durable, now: u64) -> Result<Step, Fault> {
    let encoded = b"step";
    host.encoded_pointer = encoded.as_ptr();
    acknowledge_durable(host, &lease(7), now, &done("new"), b"image", encoded)
}

#[test]
fn durable_fresh_and_replay_routes_keep_original_winner_buffers_and_hook_order() {
    for state in ["pending", "unknown"] {
        let mut host = Durable::new();
        host.effect.as_mut().unwrap().state = state.into();
        let trace = host.hooks.trace.clone();
        assert_eq!(run(host, 100).unwrap(), done("new"));
        assert_eq!(
            *trace.borrow(),
            [
                "effect",
                "dispatch",
                "growth",
                "complete",
                "acknowledge",
                "finalize",
                "commit",
                "drop"
            ]
        );
    }
    for winner in [
        done("old"),
        Step::Fault(Fault {
            code: "HOST".into(),
            message: "old".into(),
        }),
    ] {
        let mut host = Durable::replay(&winner);
        host.dispatch = None;
        let trace = host.hooks.trace.clone();
        assert_eq!(run(host, u64::MAX).unwrap(), winner);
        assert_eq!(*trace.borrow(), ["effect", "replay", "drop"]);
    }
}

#[test]
fn durable_image_missing_record_and_terminal_decode_keep_original_refusal_precedence() {
    for kind in 0..6 {
        let mut host = Durable::replay(&invalid());
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
            run(host, 10).unwrap_err().code,
            [
                "LSV4021", "LSV4014", "LSV4007", "LSV4007", "LSV4006", "LSV4007"
            ][kind]
        );
        assert_eq!(*trace.borrow(), ["effect", "drop"]);
    }
}

#[test]
fn durable_dispatch_decoding_and_lease_fences_precede_growth_and_writes() {
    for kind in 0..9 {
        let mut host = Durable::new();
        match kind {
            0 => host.dispatch = None,
            1 => host.dispatch.as_mut().unwrap().request = b"{".to_vec(),
            2 => {
                host.dispatch.as_mut().unwrap().request =
                    serde_json::to_vec(&lease(8).request).unwrap()
            }
            3 => host.dispatch.as_mut().unwrap().state = "acknowledged".into(),
            4 => host.dispatch.as_mut().unwrap().attempt = 2,
            5 => host.dispatch.as_mut().unwrap().retry_count = 1,
            6 => host.dispatch.as_mut().unwrap().lease_expires_at_ms = None,
            7 => host.dispatch.as_mut().unwrap().ready_at_ms = 11,
            _ => {}
        }
        let trace = host.hooks.trace.clone();
        let now = if kind == 8 { 101 } else { 10 };
        assert_eq!(
            run(host, now).unwrap_err().code,
            if kind == 0 {
                "LSV4021"
            } else if kind == 1 {
                "LSV4007"
            } else if kind == 8 {
                "LSV4023"
            } else {
                "LSV4022"
            }
        );
        assert_eq!(*trace.borrow(), ["effect", "dispatch", "drop"]);
    }
}

#[test]
fn every_native_hook_fault_or_unwind_consumes_owner_once_without_retry_or_compensation() {
    for replay in [false, true] {
        let stages: &[&str] = if replay {
            &["effect", "replay"]
        } else {
            &[
                "effect",
                "dispatch",
                "growth",
                "complete",
                "acknowledge",
                "finalize",
                "commit",
            ]
        };
        for (index, stage) in stages.iter().enumerate() {
            for unwind in [false, true] {
                let mut host = if replay {
                    Durable::replay(&done("old"))
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
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(host, 10)));
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
