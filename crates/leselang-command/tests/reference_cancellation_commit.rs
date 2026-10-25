use leselang_command::reference_cancellation_commit::*;
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::{
    Cancellation, CancellationReason, ContinuationImage, ContinuationToken, Step, Value,
};
use leselang_hir::{Effect, Type};
use leselang_host_contract::{CapabilitySet, Principal, Revision};
use leselang_runtime_core::Fault;
use std::{
    cell::{Cell, RefCell},
    rc::Rc,
};

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
fn step(now: u64) -> Step {
    Step::Cancelled(Cancellation {
        continuation: image().token,
        reason: CancellationReason::Requested,
        observed_at_ms: now,
    })
}
#[derive(Default)]
struct Hooks {
    trace: Rc<RefCell<Vec<&'static str>>>,
    fail_at: Option<&'static str>,
    unwind_at: Option<&'static str>,
    error: Option<Fault>,
}
impl Hooks {
    fn observe(&self, name: &'static str) {
        self.trace.borrow_mut().push(name);
        assert_ne!(self.unwind_at, Some(name), "native unwind");
    }
    fn enter(&mut self, name: &'static str) -> Result<(), Fault> {
        self.observe(name);
        if self.fail_at == Some(name) {
            return Err(self.error.take().unwrap());
        }
        Ok(())
    }
}
struct Memory {
    hooks: Hooks,
    present: bool,
    acknowledged: bool,
    lease: Option<u64>,
    terminal: Option<Step>,
}
impl Memory {
    fn new() -> Self {
        Self {
            hooks: Hooks::default(),
            present: true,
            acknowledged: false,
            lease: Some(1000),
            terminal: None,
        }
    }
}
impl MemoryCancellationJournal for Memory {
    fn dispatch(&mut self, token: &ContinuationToken) -> Option<MemoryCancellationState<'_>> {
        self.hooks.observe("dispatch");
        assert_eq!(token, &image().token);
        self.present.then_some(MemoryCancellationState {
            acknowledged: &mut self.acknowledged,
            lease_expires_at_ms: &mut self.lease,
            terminal_step: &mut self.terminal,
        })
    }
    fn finalize_group(&mut self, token: &ContinuationToken) -> Result<(), Fault> {
        assert_eq!(token, &image().token);
        if self.present {
            assert!(self.acknowledged);
            assert!(self.lease.is_none());
            assert!(self.terminal.is_some());
        }
        self.hooks.enter("finalize")
    }
}

#[test]
fn memory_validation_precedes_borrowed_mutation_and_keeps_terminal_before_token_errors() {
    for proposed in [
        Step::Yield(image()),
        Step::Done(Value::UiFocus {
            node_id: "node".into(),
        }),
        step(u64::MAX),
        Step::Cancelled(Cancellation {
            continuation: ContinuationToken::from_wire("other".into()),
            reason: CancellationReason::Requested,
            observed_at_ms: 30,
        }),
    ] {
        let mut host = Memory::new();
        assert_eq!(
            cancel_memory(&mut host, &image(), &proposed)
                .unwrap_err()
                .code,
            "LSV4007"
        );
        assert!(host.hooks.trace.borrow().is_empty());
        assert!(!host.acknowledged);
        assert_eq!(host.lease, Some(1000));
        assert!(host.terminal.is_none());
    }
    let mut host = Memory::new();
    let owner: &mut dyn MemoryCancellationJournal = &mut host;
    assert_eq!(cancel_memory(owner, &image(), &step(30)).unwrap(), step(30));
    assert_eq!(*host.hooks.trace.borrow(), ["dispatch", "finalize"]);
    assert_eq!(host.terminal, Some(step(30)));
}

#[test]
fn missing_memory_dispatch_still_finalizes_and_low_level_replacement_keeps_original_semantics() {
    for present in [false, true] {
        let mut host = Memory::new();
        host.present = present;
        host.acknowledged = true;
        host.terminal = Some(step(10));
        assert_eq!(
            cancel_memory(&mut host, &image(), &step(30)).unwrap(),
            step(30)
        );
        assert_eq!(host.terminal, Some(step(if present { 30 } else { 10 })));
        assert_eq!(host.lease, if present { None } else { Some(1000) });
        assert_eq!(*host.hooks.trace.borrow(), ["dispatch", "finalize"]);
    }
}

#[test]
fn memory_errors_and_unwinds_never_retry_or_undo_prior_native_slots() {
    for unwind in [false, true] {
        let mut host = Memory::new();
        let error = Fault {
            code: "NATIVE".into(),
            message: "original native error".into(),
        };
        let pointer = error.message.as_ptr();
        host.hooks.error = Some(error);
        if unwind {
            host.hooks.unwind_at = Some("finalize");
        } else {
            host.hooks.fail_at = Some("finalize");
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cancel_memory(&mut host, &image(), &step(30))
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().unwrap_err().message.as_ptr(), pointer);
        }
        assert!(host.acknowledged);
        assert!(host.lease.is_none());
        assert_eq!(host.terminal, Some(step(30)));
        assert_eq!(*host.hooks.trace.borrow(), ["dispatch", "finalize"]);
    }
    let mut host = Memory::new();
    host.hooks.unwind_at = Some("dispatch");
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| cancel_memory(
            &mut host,
            &image(),
            &step(30)
        )))
        .is_err()
    );
    assert!(!host.acknowledged);
    assert_eq!(host.lease, Some(1000));
    assert!(host.terminal.is_none());
    assert_eq!(*host.hooks.trace.borrow(), ["dispatch"]);
}

struct Durable {
    hooks: Hooks,
    effect: Option<StoredEffect>,
    encoded_pointer: *const u8,
    commit_observed: Rc<Cell<bool>>,
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
            encoded_pointer: std::ptr::null(),
            commit_observed: Rc::default(),
        }
    }
    fn replay(bytes: Vec<u8>) -> Self {
        let mut host = Self::new();
        let effect = host.effect.as_mut().unwrap();
        effect.state = "completed".into();
        effect.terminal_step = Some(bytes);
        host
    }
}
impl Drop for Durable {
    fn drop(&mut self) {
        self.hooks.trace.borrow_mut().push("drop");
    }
}
impl DurableCancellationTransaction for Durable {
    fn effect(&mut self, _: &str) -> Result<Option<StoredEffect>, Fault> {
        self.hooks.enter("effect")?;
        Ok(self.effect.take())
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        assert_eq!(additional, 4);
        self.hooks.enter("growth")
    }
    fn complete(&mut self, _: &str, bytes: &[u8]) -> Result<(), Fault> {
        assert_eq!(bytes.as_ptr(), self.encoded_pointer);
        self.hooks.enter("complete")
    }
    fn finalize_group(&mut self, _: &str) -> Result<(), Fault> {
        self.hooks.enter("finalize")
    }
    fn commit(mut self, kind: CancellationCommit) -> Result<(), Fault> {
        // Model a native error/unwind after commit has already been observed.
        self.commit_observed.set(true);
        self.hooks.enter(match kind {
            CancellationCommit::Replay => "replay",
            CancellationCommit::Cancellation => "commit",
        })
    }
}
fn run(mut host: Durable) -> Result<Step, Fault> {
    let encoded = b"step";
    host.encoded_pointer = encoded.as_ptr();
    cancel_durable(host, &image(), &step(200), b"image", encoded)
}

#[test]
fn durable_fresh_path_keeps_exact_bytes_growth_finalization_and_single_commit_order() {
    for state in ["pending", "unknown-saved-state"] {
        let mut host = Durable::new();
        host.effect.as_mut().unwrap().state = state.into();
        let trace = host.hooks.trace.clone();
        assert_eq!(run(host).unwrap(), step(200));
        assert_eq!(
            *trace.borrow(),
            ["effect", "growth", "complete", "finalize", "commit", "drop"]
        );
    }
}

#[test]
fn durable_replay_admits_original_terminal_and_never_overwrites_it_with_cancellation() {
    for original in [
        step(30),
        Step::Done(Value::UiFocus {
            node_id: "original".into(),
        }),
        Step::Fault(Fault {
            code: "HOST".into(),
            message: "original fault".into(),
        }),
    ] {
        let host = Durable::replay(serde_json::to_vec(&original).unwrap());
        let trace = host.hooks.trace.clone();
        assert_eq!(run(host).unwrap(), original);
        assert_eq!(*trace.borrow(), ["effect", "replay", "drop"]);
    }
    for original in [
        Step::Yield(image()),
        step(u64::MAX),
        Step::Fault(Fault {
            code: "".into(),
            message: "invalid".into(),
        }),
    ] {
        let host = Durable::replay(serde_json::to_vec(&original).unwrap());
        let trace = host.hooks.trace.clone();
        assert_eq!(run(host).unwrap_err().code, "LSV4007");
        assert_eq!(*trace.borrow(), ["effect", "drop"]);
    }
}

#[test]
fn durable_missing_image_payload_and_bounded_decode_faults_keep_precedence() {
    for kind in 0..5 {
        let mut host = Durable::replay(b"{".to_vec());
        match kind {
            0 => host.effect = None,
            1 => host.effect.as_mut().unwrap().image.clear(),
            2 => host.effect.as_mut().unwrap().terminal_step = None,
            3 => {}
            4 => {
                host.effect.as_mut().unwrap().terminal_step = Some(vec![
                    b' ';
                    leselang_command::reference_recovery::MAX_JOURNAL_ENTRY_BYTES
                        + 1
                ])
            }
            _ => unreachable!(),
        }
        let trace = host.hooks.trace.clone();
        let error = run(host).unwrap_err();
        assert_eq!(
            error.code,
            ["LSV4013", "LSV4014", "LSV4007", "LSV4007", "LSV4006"][kind]
        );
        let prefix = [
            "pending continuation is missing",
            "continuation image conflicts with durable journal state",
            "completed journal record has no terminal step",
            "invalid journal JSON:",
            "journal entry exceeds",
        ][kind];
        assert!(error.message.starts_with(prefix), "{error:?}");
        assert_eq!(*trace.borrow(), ["effect", "drop"]);
    }
}

#[test]
fn every_durable_hook_fault_or_unwind_consumes_owner_without_retry_or_compensation() {
    for replay in [false, true] {
        let stages: &[&str] = if replay {
            &["effect", "replay"]
        } else {
            &["effect", "growth", "complete", "finalize", "commit"]
        };
        for (index, stage) in stages.iter().enumerate() {
            for unwind in [false, true] {
                let mut host = if replay {
                    Durable::replay(serde_json::to_vec(&step(30)).unwrap())
                } else {
                    Durable::new()
                };
                let trace = host.hooks.trace.clone();
                let committed = host.commit_observed.clone();
                let error = Fault {
                    code: "NATIVE".into(),
                    message: "original error buffer".into(),
                };
                let pointer = error.message.as_ptr();
                host.hooks.error = Some(error);
                if unwind {
                    host.hooks.unwind_at = Some(stage);
                } else {
                    host.hooks.fail_at = Some(stage);
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(host)));
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
