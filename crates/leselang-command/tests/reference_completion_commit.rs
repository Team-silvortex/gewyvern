use leselang_command::reference_completion_commit::*;
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::{ContinuationImage, ContinuationToken, Step, Value};
use leselang_hir::{Effect, Type};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;
use std::{cell::RefCell, rc::Rc};

fn image() -> ContinuationImage {
    build_effect_request(ReservedEffectInput {
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
    .unwrap()
    .continuation
}
fn done(node: &str) -> Step {
    Step::Done(Value::UiFocus {
        node_id: node.into(),
    })
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
    ready: bool,
    present: bool,
    acknowledged: bool,
    expires: Option<u64>,
    terminal: Option<Step>,
}
impl Memory {
    fn new() -> Self {
        Self {
            hooks: Hooks::default(),
            ready: true,
            present: true,
            acknowledged: false,
            expires: None,
            terminal: None,
        }
    }
}
impl MemoryCompletionJournal for Memory {
    fn sequence_ready(&self, _: &ContinuationToken) -> bool {
        self.hooks.trace.borrow_mut().push("sequence");
        self.ready
    }
    fn dispatch(&mut self, _: &ContinuationToken) -> Option<MemoryCompletionState<'_>> {
        self.hooks.enter("dispatch").unwrap();
        self.present.then_some(MemoryCompletionState {
            acknowledged: &mut self.acknowledged,
            lease_expires_at_ms: self.expires,
            terminal_step: &mut self.terminal,
        })
    }
    fn finalize_group(&mut self, _: &ContinuationToken) -> Result<(), Fault> {
        self.hooks.enter("finalize")
    }
}
#[test]
fn memory_sequence_precedes_lease_and_missing_dispatch_still_finalizes() {
    for ready in [false, true] {
        let mut host = Memory::new();
        host.ready = ready;
        host.expires = Some(100);
        assert_eq!(
            complete_memory(&mut host, &image(), &done("new"))
                .unwrap_err()
                .code,
            if ready { "LSV4024" } else { "LSV2410" }
        );
        assert_eq!(
            *host.hooks.trace.borrow(),
            if ready {
                vec!["sequence", "dispatch"]
            } else {
                vec!["sequence"]
            }
        );
        assert!(!host.acknowledged && host.terminal.is_none());
    }
    let mut host = Memory::new();
    host.present = false;
    let local: &mut dyn MemoryCompletionJournal = &mut host;
    assert_eq!(
        complete_memory(local, &image(), &done("new")).unwrap(),
        done("new")
    );
    assert_eq!(
        *host.hooks.trace.borrow(),
        ["sequence", "dispatch", "finalize"]
    );
}
#[test]
fn memory_primitive_retains_lease_and_replaces_terminal_without_adding_admission() {
    for expires in [None, Some(100)] {
        let mut host = Memory::new();
        host.acknowledged = true;
        host.expires = expires;
        host.terminal = Some(done("old"));
        let mut image = image();
        image.schema_version = 0;
        let step = Step::Yield(image.clone());
        assert_eq!(complete_memory(&mut host, &image, &step).unwrap(), step);
        assert_eq!(host.terminal, Some(step));
        assert_eq!(host.expires, expires);
    }
}
#[test]
fn memory_finalize_failure_or_unwind_retains_mutation_without_retry() {
    for unwind in [false, true] {
        let mut host = Memory::new();
        host.hooks.error = Some(Fault {
            code: "NATIVE".into(),
            message: "original fault".into(),
        });
        let pointer = host.hooks.error.as_ref().unwrap().message.as_ptr();
        if unwind {
            host.hooks.unwind = Some("finalize");
        } else {
            host.hooks.fail = Some("finalize");
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            complete_memory(&mut host, &image(), &done("new"))
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().unwrap_err().message.as_ptr(), pointer);
        }
        assert!(host.acknowledged);
        assert_eq!(host.terminal, Some(done("new")));
        assert_eq!(
            *host.hooks.trace.borrow(),
            ["sequence", "dispatch", "finalize"]
        );
    }
}

struct Durable {
    hooks: Hooks,
    effect: Option<StoredEffect>,
    dispatch: Option<StoredDispatch>,
    bytes: *const u8,
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
                request: b"not decoded".to_vec(),
                state: "pending".into(),
                attempt: 0,
                lease_expires_at_ms: None,
                ready_at_ms: 0,
                retry_count: 0,
            }),
            bytes: b"step".as_ptr(),
        }
    }
}
impl Drop for Durable {
    fn drop(&mut self) {
        self.hooks.trace.borrow_mut().push("drop");
    }
}
impl DurableCompletionTransaction for Durable {
    fn effect(&mut self, _: &str) -> Result<Option<StoredEffect>, Fault> {
        self.hooks.enter("effect")?;
        Ok(self.effect.take())
    }
    fn dispatch(&mut self, _: &str) -> Result<Option<StoredDispatch>, Fault> {
        self.hooks.enter("dispatch")?;
        Ok(self.dispatch.take())
    }
    fn ensure_sequence_ready(&mut self, _: &str) -> Result<(), Fault> {
        self.hooks.enter("sequence")
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        assert_eq!(additional, 4);
        self.hooks.enter("quota")
    }
    fn complete_effect(&mut self, _: &str, bytes: &[u8]) -> Result<(), Fault> {
        assert_eq!(bytes.as_ptr(), self.bytes);
        self.hooks.enter("effect-write")
    }
    fn acknowledge_dispatch(&mut self, _: &str) -> Result<(), Fault> {
        self.hooks.enter("dispatch-write")
    }
    fn finalize_group(&mut self, _: &str) -> Result<(), Fault> {
        self.hooks.enter("finalize")
    }
    fn commit(mut self, kind: CompletionCommit) -> Result<(), Fault> {
        self.hooks.enter(match kind {
            CompletionCommit::Replay => "replay",
            CompletionCommit::Completion => "commit",
        })
    }
}
fn complete(host: Durable) -> Result<Step, Fault> {
    complete_durable(host, &image(), &done("new"), b"image", b"step")
}
const FRESH: [&str; 9] = [
    "effect",
    "dispatch",
    "sequence",
    "quota",
    "effect-write",
    "dispatch-write",
    "finalize",
    "commit",
    "drop",
];
#[test]
fn durable_completion_preserves_missing_dispatch_and_raw_noncompleted_state_branches() {
    for kind in 0..3 {
        let mut host = Durable::new();
        if kind == 1 {
            host.dispatch = None;
        }
        if kind == 2 {
            host.effect.as_mut().unwrap().state = "unknown".into();
        }
        let trace = host.hooks.trace.clone();
        assert_eq!(complete(host).unwrap(), done("new"));
        assert_eq!(*trace.borrow(), FRESH);
    }
}
#[test]
fn durable_replay_checks_image_then_saved_terminal_and_skips_dispatch_and_sequence() {
    for kind in 0..7 {
        let mut host = Durable::new();
        let record = host.effect.as_mut().unwrap();
        record.state = "completed".into();
        record.terminal_step = Some(serde_json::to_vec(&done("old")).unwrap());
        match kind {
            0 => {}
            1 => record.image = vec![],
            2 => record.terminal_step = None,
            3 => record.terminal_step = Some(b"{".to_vec()),
            4 => record.terminal_step = Some(serde_json::to_vec(&Step::Yield(image())).unwrap()),
            5 => {
                record.terminal_step = Some(vec![
                    b' ';
                    leselang_command::reference_recovery::MAX_JOURNAL_ENTRY_BYTES
                        + 1
                ])
            }
            6 => host.effect = None,
            _ => unreachable!(),
        }
        let trace = host.hooks.trace.clone();
        let result = complete(host);
        if kind == 0 {
            assert_eq!(result.unwrap(), done("old"));
            assert_eq!(*trace.borrow(), ["effect", "replay", "drop"]);
        } else {
            let fault = result.unwrap_err();
            if kind == 1 {
                assert_eq!(fault.code, "LSV4014");
            }
            if kind == 6 {
                assert_eq!(fault.code, "LSV4013");
            }
            assert_eq!(*trace.borrow(), ["effect", "drop"]);
        }
    }
}
#[test]
fn durable_lease_refusal_precedes_sequence_read() {
    let mut host = Durable::new();
    host.dispatch.as_mut().unwrap().state = "leased".into();
    let trace = host.hooks.trace.clone();
    assert_eq!(complete(host).unwrap_err().code, "LSV4024");
    assert_eq!(*trace.borrow(), ["effect", "dispatch", "drop"]);
}
#[test]
fn durable_callbacks_forward_exact_faults_and_consume_owner_on_failure_or_unwind() {
    for replay in [false, true] {
        let order: &[&str] = if replay {
            &["effect", "replay", "drop"]
        } else {
            &FRESH
        };
        for (index, hook) in order[..order.len() - 1].iter().enumerate() {
            for unwind in [false, true] {
                let mut host = Durable::new();
                if replay {
                    let r = host.effect.as_mut().unwrap();
                    r.state = "completed".into();
                    r.terminal_step = Some(serde_json::to_vec(&done("old")).unwrap());
                }
                host.hooks.error = Some(Fault {
                    code: "NATIVE".into(),
                    message: "original fault buffer".into(),
                });
                let pointer = host.hooks.error.as_ref().unwrap().message.as_ptr();
                if unwind {
                    host.hooks.unwind = Some(hook);
                } else {
                    host.hooks.fail = Some(hook);
                }
                let trace = host.hooks.trace.clone();
                let result =
                    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| complete(host)));
                if unwind {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap().unwrap_err().message.as_ptr(), pointer);
                }
                let mut expected = order[..=index].to_vec();
                expected.push("drop");
                assert_eq!(*trace.borrow(), expected);
            }
        }
    }
}
