use leselang_command::debugger_audit::debugger_audit_record;
use leselang_command::reference_audit_commit::*;
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::{
    Cancellation, CancellationReason, ContinuationImage, ContinuationToken, DebuggerAuditContext,
    DebuggerAuditRecord, Step,
};
use leselang_host_contract::{CapabilitySet, CommandOrigin, Principal, Revision};
use leselang_runtime_core::Fault;
use leserpent_domain::{CommandId, IdempotencyKey};
use leserpent_hir::{Effect, Type};
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
        expected_revision: Some(Revision(7)),
        deadline_ms: 1000,
        deadline_at_ms: None,
        fuel_remaining: 100,
    })
    .unwrap()
    .continuation
}
fn audit() -> DebuggerAuditContext {
    DebuggerAuditContext {
        command_id: CommandId::new("command").unwrap(),
        idempotency_key: IdempotencyKey::new("key").unwrap(),
        principal: Principal::new("operator").unwrap(),
        origin: CommandOrigin::Gui,
        session_id: "session".into(),
        expected_revision: Revision(7),
    }
}
fn step(image: &ContinuationImage, now: u64) -> Step {
    Step::Cancelled(Cancellation {
        continuation: image.token.clone(),
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
    image: ContinuationImage,
    terminal: Option<Step>,
    acknowledged: bool,
    audit: Option<(ContinuationToken, String, String, DebuggerAuditRecord)>,
}
impl Memory {
    fn new() -> Self {
        Self {
            hooks: Hooks::default(),
            image: image(),
            terminal: None,
            acknowledged: false,
            audit: None,
        }
    }
    fn view(&self) -> Option<MemoryAudit<'_>> {
        self.audit
            .as_ref()
            .map(|(token, key, principal, record)| MemoryAudit {
                token,
                idempotency_key: key,
                principal_id: principal,
                record,
            })
    }
}
impl MemoryAuditJournal for Memory {
    fn audit(&self, command: &str) -> Option<MemoryAudit<'_>> {
        self.hooks.observe("audit");
        self.view()
            .filter(|a| a.record.command_id.as_str() == command)
    }
    fn audits(&self) -> impl Iterator<Item = MemoryAudit<'_>> {
        self.hooks.observe("keys");
        self.view().into_iter()
    }
    fn effect(&self, _: &ContinuationToken) -> Option<MemoryEffect<'_>> {
        self.hooks.observe("effect");
        Some(MemoryEffect {
            image: &self.image,
            acknowledged: self.acknowledged,
            terminal: self.terminal.as_ref(),
        })
    }
    fn cancel(&mut self, _: &ContinuationImage, proposed: &Step) -> Result<Step, Fault> {
        self.acknowledged = true;
        self.terminal = Some(proposed.clone());
        self.hooks.enter("cancel")?;
        Ok(proposed.clone())
    }
    fn insert(
        &mut self,
        image: &ContinuationImage,
        audit: &DebuggerAuditContext,
        record: &DebuggerAuditRecord,
    ) {
        self.hooks.observe("insert");
        self.audit = Some((
            image.token.clone(),
            audit.idempotency_key.as_str().into(),
            audit.principal.id.clone(),
            record.clone(),
        ));
    }
}

#[test]
fn memory_replay_keeps_original_receipt_and_skips_new_image_or_step_admission() {
    let mut host = Memory::new();
    let mut image = image();
    let audit = audit();
    let original = cancel_memory(&mut host, &image, &step(&image, 30), &audit, 30).unwrap();
    assert_eq!(
        *host.hooks.trace.borrow(),
        ["audit", "keys", "effect", "cancel", "insert"]
    );
    host.hooks.trace.borrow_mut().clear();
    image.fuel_remaining += 1;
    let replay = cancel_memory(
        &mut host,
        &image,
        &Step::Yield(image.clone()),
        &audit,
        u64::MAX,
    )
    .unwrap();
    assert_eq!(replay, original);
    assert_eq!(*host.hooks.trace.borrow(), ["audit", "effect"]);
}

#[test]
fn memory_key_and_completion_refusals_precede_mutation_without_inventing_rollback() {
    let image = image();
    let original_audit = audit();
    let mut host = Memory::new();
    cancel_memory(&mut host, &image, &step(&image, 30), &original_audit, 30).unwrap();
    let mut changed = original_audit.clone();
    changed.command_id = CommandId::new("new-command").unwrap();
    assert_eq!(
        cancel_memory(&mut host, &image, &step(&image, 40), &changed, 40)
            .unwrap_err()
            .message,
        "debugger idempotency key was reused"
    );
    changed.principal = Principal::new("other").unwrap();
    assert_eq!(
        cancel_memory(&mut host, &image, &step(&image, 40), &changed, 40)
            .unwrap_err()
            .message,
        "completed effect has no matching debugger audit"
    );
    for unwind in [false, true] {
        let mut host = Memory::new();
        if unwind {
            host.hooks.unwind_at = Some("insert");
        } else {
            host.hooks.fail_at = Some("cancel");
            host.hooks.error = Some(Fault {
                code: "NATIVE".into(),
                message: "partial mutation".into(),
            });
        }
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            cancel_memory(&mut host, &image, &step(&image, 30), &original_audit, 30)
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            assert_eq!(result.unwrap().unwrap_err().code, "NATIVE");
        }
        assert!(host.acknowledged);
        assert!(host.audit.is_none());
        assert_eq!(host.terminal, Some(step(&image, 30)));
        assert_eq!(
            host.hooks
                .trace
                .borrow()
                .iter()
                .filter(|s| **s == "cancel")
                .count(),
            1
        );
    }
}

struct Durable {
    hooks: Hooks,
    audit: Option<StoredAudit>,
    effect: Option<StoredEffect>,
    original_bytes: *const u8,
}
impl Durable {
    fn new(bytes: &[u8]) -> Self {
        Self {
            hooks: Hooks::default(),
            audit: None,
            effect: Some(StoredEffect {
                image: b"image".to_vec(),
                state: "pending".into(),
                terminal_step: None,
            }),
            original_bytes: bytes.as_ptr(),
        }
    }
    fn replay(bytes: &[u8], terminal: Vec<u8>) -> Self {
        let mut host = Self::new(bytes);
        let audit = audit();
        host.audit = Some((
            audit.command_id.as_str().into(),
            image().token.as_str().into(),
            audit.idempotency_key.as_str().into(),
            debugger_audit_record(&audit, 30),
        ));
        host.effect.as_mut().unwrap().terminal_step = Some(terminal);
        host
    }
}
impl Drop for Durable {
    fn drop(&mut self) {
        self.hooks.trace.borrow_mut().push("drop");
    }
}
impl DurableAuditTransaction for Durable {
    fn audit(&mut self, _: &DebuggerAuditContext) -> Result<Option<StoredAudit>, Fault> {
        self.hooks.enter("audit")?;
        Ok(self.audit.take())
    }
    fn effect(&mut self, _: &str) -> Result<Option<StoredEffect>, Fault> {
        self.hooks.enter("effect")?;
        Ok(self.effect.take())
    }
    fn insert(
        &mut self,
        _: &ContinuationImage,
        _: &DebuggerAuditContext,
        revision: i64,
        now: i64,
    ) -> Result<(), Fault> {
        assert_eq!((revision, now), (7, 200));
        self.hooks.enter("insert")
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        assert_eq!(additional, b"step".len());
        self.hooks.enter("growth")
    }
    fn complete(&mut self, _: &str, bytes: &[u8]) -> Result<(), Fault> {
        assert_eq!(bytes.as_ptr(), self.original_bytes);
        self.hooks.enter("complete")
    }
    fn finalize_group(&mut self, _: &str) -> Result<(), Fault> {
        self.hooks.enter("finalize")
    }
    fn commit(mut self, kind: AuditCommit) -> Result<(), Fault> {
        self.hooks.enter(match kind {
            AuditCommit::Replay => "replay",
            AuditCommit::Cancellation => "commit",
        })
    }
}
fn run(
    host: Durable,
    audit: &DebuggerAuditContext,
    now: u64,
) -> Result<(Step, DebuggerAuditRecord), Fault> {
    let image = image();
    cancel_durable(
        host,
        &image,
        &step(&image, now),
        audit,
        now,
        b"image",
        b"step",
    )
}

#[test]
fn durable_fresh_commit_preserves_native_order_and_exact_encoding_buffers() {
    let host = Durable::new(b"step");
    let trace = host.hooks.trace.clone();
    let record = run(host, &audit(), 200).unwrap();
    assert_eq!(record.0, step(&image(), 200));
    assert_eq!(record.1.observed_at_ms, 200);
    assert_eq!(
        *trace.borrow(),
        [
            "audit", "effect", "insert", "growth", "complete", "finalize", "commit", "drop"
        ]
    );
}

#[test]
fn durable_replay_moves_original_record_and_preserves_decode_only_behavior() {
    for terminal in [step(&image(), 30), Step::Yield(image())] {
        let host = Durable::replay(b"step", serde_json::to_vec(&terminal).unwrap());
        let pointer = host.audit.as_ref().unwrap().3.session_id.as_ptr();
        let trace = host.hooks.trace.clone();
        let (actual, record) = run(host, &audit(), u64::MAX).unwrap();
        assert_eq!(actual, terminal);
        assert_eq!(record.session_id.as_ptr(), pointer);
        assert_eq!(record.observed_at_ms, 30);
        assert_eq!(*trace.borrow(), ["audit", "effect", "replay", "drop"]);
    }
}

#[test]
fn durable_conflict_image_completion_and_clock_faults_keep_original_precedence() {
    for variant in 0..8 {
        let mut host = if variant < 3 {
            Durable::replay(b"step", serde_json::to_vec(&step(&image(), 30)).unwrap())
        } else {
            Durable::new(b"step")
        };
        let mut audit = audit();
        match variant {
            0 => host.audit.as_mut().unwrap().0 = "other".into(),
            1 | 3 => host.effect = None,
            2 => host.effect.as_mut().unwrap().terminal_step = None,
            4 => {
                let effect = host.effect.as_mut().unwrap();
                effect.image.clear();
                effect.state = "completed".into();
            }
            5 => host.effect.as_mut().unwrap().state = "completed".into(),
            6 => audit.expected_revision = Revision(u64::MAX),
            7 => {}
            _ => unreachable!(),
        }
        let trace = host.hooks.trace.clone();
        let error = run(host, &audit, u64::MAX).unwrap_err();
        assert_eq!(
            error.message,
            [
                "debugger command audit conflicts",
                "debugger audit effect is missing",
                "debugger audit has no terminal step",
                "pending continuation is missing",
                "continuation image conflicts with durable journal state",
                "completed effect has no matching debugger audit",
                "debugger audit revision is out of range",
                "debugger audit clock is out of range"
            ][variant]
        );
        assert!(!trace.borrow().contains(&"insert"));
    }
}

#[test]
fn durable_faults_and_unwinds_drop_the_single_transaction_without_retry_or_compensation() {
    for replay in [false, true] {
        let stages: &[&str] = if replay {
            &["audit", "effect", "replay"]
        } else {
            &[
                "audit", "effect", "insert", "growth", "complete", "finalize", "commit",
            ]
        };
        for (index, stage) in stages.iter().enumerate() {
            for unwind in [false, true] {
                let mut host = if replay {
                    Durable::replay(b"step", serde_json::to_vec(&step(&image(), 30)).unwrap())
                } else {
                    Durable::new(b"step")
                };
                let error = Fault {
                    code: "NATIVE".into(),
                    message: "original fault buffer".into(),
                };
                let pointer = error.message.as_ptr();
                host.hooks.error = Some(error);
                if unwind {
                    host.hooks.unwind_at = Some(stage);
                } else {
                    host.hooks.fail_at = Some(stage);
                }
                let trace = host.hooks.trace.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    run(host, &audit(), 200)
                }));
                if unwind {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap().unwrap_err().message.as_ptr(), pointer);
                }
                let mut expected = stages[..=index].to_vec();
                expected.push("drop");
                assert_eq!(*trace.borrow(), expected);
            }
        }
    }
}
