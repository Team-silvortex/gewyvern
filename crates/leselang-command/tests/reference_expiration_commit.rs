use leselang_command::reference_completion::deadline_cancellation;
use leselang_command::reference_expiration_commit::*;
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::{ContinuationImage, ContinuationToken, Step};
use leselang_hir::{Effect, Type};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;
use std::{cell::RefCell, rc::Rc};

fn image(sequence: u64, deadline: Option<u64>) -> ContinuationImage {
    build_effect_request(ReservedEffectInput {
        effect: &Effect::UiFocus {
            node_id: "node".into(),
        },
        result_type: Type::UiFocus,
        sequence,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: None,
        deadline_ms: 1000,
        deadline_at_ms: deadline,
        fuel_remaining: 100,
    })
    .unwrap()
    .continuation
}
#[derive(Default)]
struct Hooks {
    trace: Rc<RefCell<Vec<&'static str>>>,
    fail_at: Option<usize>,
    unwind: bool,
    error: Option<Fault>,
}
impl Hooks {
    fn enter(&mut self, label: &'static str) -> Result<(), Fault> {
        let position = self.trace.borrow().len();
        self.trace.borrow_mut().push(label);
        if self.fail_at == Some(position) {
            assert!(!self.unwind, "native unwind");
            return Err(self.error.take().unwrap());
        }
        Ok(())
    }
}
struct Entry {
    image: ContinuationImage,
    ack: bool,
    expires: Option<u64>,
    terminal: Option<Step>,
}
struct Memory {
    hooks: Hooks,
    entries: Vec<Entry>,
    now: u64,
}
impl Memory {
    fn new() -> Self {
        Self {
            hooks: Hooks::default(),
            now: 10,
            entries: [Some(10), None, Some(9), Some(11), Some(8)]
                .into_iter()
                .enumerate()
                .map(|(i, d)| Entry {
                    image: image(i as u64 + 1, d),
                    ack: i == 4,
                    expires: Some(100),
                    terminal: None,
                })
                .collect(),
        }
    }
}
impl MemoryExpirationJournal for Memory {
    fn dispatches(&mut self) -> impl Iterator<Item = MemoryExpirationState<'_>> {
        self.hooks.enter("scan").unwrap();
        self.entries.iter_mut().map(|e| MemoryExpirationState {
            image: &e.image,
            acknowledged: &mut e.ack,
            lease_expires_at_ms: &mut e.expires,
            terminal_step: &mut e.terminal,
        })
    }
    fn finalize_group(&mut self, _: &ContinuationToken) -> Result<(), Fault> {
        for index in [0, 2] {
            let e = &self.entries[index];
            assert!(e.ack && e.expires.is_none());
            assert_eq!(e.terminal, Some(deadline_cancellation(&e.image, self.now)));
        }
        self.hooks.enter("finalize")
    }
}
#[test]
fn memory_marks_whole_due_batch_before_finalization_and_preserves_skipped_entries() {
    let mut host = Memory::new();
    let expected = [0, 2].map(|i| {
        (
            host.entries[i].image.clone(),
            deadline_cancellation(&host.entries[i].image, 10),
        )
    });
    assert_eq!(expire_memory(&mut host, 10).unwrap(), expected);
    for i in [1, 3, 4] {
        assert_eq!(host.entries[i].expires, Some(100));
        assert!(host.entries[i].terminal.is_none());
    }
    assert_eq!(*host.hooks.trace.borrow(), ["scan", "finalize", "finalize"]);
    assert!(expire_memory(&mut host, 10).unwrap().is_empty());
}
#[test]
fn memory_failure_or_unwind_keeps_all_due_mutations_without_publishing_partial_batch() {
    for index in 1..=2 {
        for unwind in [false, true] {
            let mut host = Memory::new();
            host.hooks.fail_at = Some(index);
            host.hooks.unwind = unwind;
            host.hooks.error = Some(Fault {
                code: "NATIVE".into(),
                message: "original buffer".into(),
            });
            let pointer = host.hooks.error.as_ref().unwrap().message.as_ptr();
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                expire_memory(&mut host, 10)
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap().unwrap_err().message.as_ptr(), pointer);
            }
            for i in [0, 2] {
                assert!(host.entries[i].ack);
                assert!(host.entries[i].expires.is_none());
                assert!(host.entries[i].terminal.is_some());
            }
            assert_eq!(host.hooks.trace.borrow().len(), index + 1);
        }
    }
}
#[test]
fn memory_primitive_does_not_add_image_or_clock_validation() {
    let mut host = Memory::new();
    host.entries.truncate(3);
    host.now = u64::MAX;
    host.entries[0].image.schema_version = 0;
    let result = expire_memory(&mut host, u64::MAX).unwrap();
    assert_eq!(result.len(), 2);
    assert_eq!(result[0].0.schema_version, 0);
}

struct Durable {
    hooks: Hooks,
    rows: Option<Vec<TimedEffectRecord>>,
    bytes: Option<(*const u8, usize)>,
}
impl Durable {
    fn new(count: usize) -> Self {
        Self {
            hooks: Hooks::default(),
            bytes: None,
            rows: Some(
                (0..count)
                    .map(|i| {
                        let image = image(10 - i as u64, Some(10));
                        TimedEffectRecord {
                            token: image.token.as_str().into(),
                            image: serde_json::to_vec(&image).unwrap(),
                        }
                    })
                    .collect(),
            ),
        }
    }
}
impl Drop for Durable {
    fn drop(&mut self) {
        self.hooks.trace.borrow_mut().push("drop");
    }
}
impl DurableExpirationTransaction for Durable {
    fn due_records(&mut self) -> Result<Vec<TimedEffectRecord>, Fault> {
        self.hooks.enter("scan")?;
        Ok(self.rows.take().unwrap())
    }
    fn admit_image(&mut self, bytes: &[u8]) -> Result<ContinuationImage, Fault> {
        self.hooks.enter("admit")?;
        Ok(serde_json::from_slice(bytes).unwrap())
    }
    fn encode_terminal(&mut self, step: &Step) -> Result<Vec<u8>, Fault> {
        self.hooks.enter("encode")?;
        let bytes = serde_json::to_vec(step).unwrap();
        self.bytes = Some((bytes.as_ptr(), bytes.len()));
        Ok(bytes)
    }
    fn ensure_growth(&mut self, size: usize) -> Result<(), Fault> {
        assert_eq!(size, self.bytes.unwrap().1);
        self.hooks.enter("quota")
    }
    fn complete(&mut self, _: &str, bytes: &[u8]) -> Result<(), Fault> {
        assert_eq!((bytes.as_ptr(), bytes.len()), self.bytes.unwrap());
        self.hooks.enter("complete")
    }
    fn finalize_group(&mut self, _: &str) -> Result<(), Fault> {
        assert!(self.rows.is_none());
        self.hooks.enter("finalize")
    }
    fn commit(mut self) -> Result<(), Fault> {
        self.hooks.enter("commit")
    }
}
fn expected_trace(count: usize) -> Vec<&'static str> {
    let mut trace = vec!["scan"];
    for _ in 0..count {
        trace.extend(["admit", "encode", "quota", "complete", "finalize"]);
    }
    trace.extend(["commit", "drop"]);
    trace
}
#[test]
fn durable_empty_batch_still_consumes_and_commits_transaction() {
    let host = Durable::new(0);
    let trace = host.hooks.trace.clone();
    assert!(expire_durable(host, 10).unwrap().is_empty());
    assert_eq!(*trace.borrow(), expected_trace(0));
}
#[test]
fn durable_uses_one_snapshot_in_owner_order_without_reselecting_group_siblings() {
    let host = Durable::new(2);
    let trace = host.hooks.trace.clone();
    let expected = (0..2)
        .map(|i| {
            let image = image(10 - i, Some(10));
            let step = deadline_cancellation(&image, 10);
            (image, step)
        })
        .collect::<Vec<_>>();
    assert_eq!(expire_durable(host, 10).unwrap(), expected);
    assert_eq!(*trace.borrow(), expected_trace(2));
}
#[test]
fn durable_checks_identity_and_inclusive_deadline_after_admission_before_encoding() {
    for kind in 0..4 {
        let mut host = Durable::new(2);
        let row = &mut host.rows.as_mut().unwrap()[1];
        match kind {
            0 => row.token = "other".into(),
            1 => row.image = serde_json::to_vec(&image(9, None)).unwrap(),
            2 => row.image = serde_json::to_vec(&image(9, Some(11))).unwrap(),
            _ => {}
        }
        let trace = host.hooks.trace.clone();
        let result = expire_durable(host, 10);
        if kind == 3 {
            assert!(result.is_ok());
        } else {
            assert_eq!(result.unwrap_err().code, "LSV4007");
            assert_eq!(
                *trace.borrow(),
                [
                    "scan", "admit", "encode", "quota", "complete", "finalize", "admit", "drop"
                ]
            );
        }
    }
}
#[test]
fn durable_failures_and_unwinds_forward_faults_drop_once_and_never_retry_any_stage() {
    for count in [0, 2] {
        let trace = expected_trace(count);
        for index in 0..trace.len() - 1 {
            for unwind in [false, true] {
                let mut host = Durable::new(count);
                host.hooks.fail_at = Some(index);
                host.hooks.unwind = unwind;
                host.hooks.error = Some(Fault {
                    code: "NATIVE".into(),
                    message: "original fault buffer".into(),
                });
                let pointer = host.hooks.error.as_ref().unwrap().message.as_ptr();
                let observed = host.hooks.trace.clone();
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    expire_durable(host, 10)
                }));
                if unwind {
                    assert!(result.is_err());
                } else {
                    assert_eq!(result.unwrap().unwrap_err().message.as_ptr(), pointer);
                }
                let mut expected = trace[..=index].to_vec();
                expected.push("drop");
                assert_eq!(*observed.borrow(), expected);
            }
        }
    }
}
