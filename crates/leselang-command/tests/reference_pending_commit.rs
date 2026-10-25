use leselang_command::reference_pending_commit::*;
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::retention_policy::MAX_JOURNAL_RECORDS;
use leselang_command::{ContinuationImage, ContinuationToken, EffectRequest, SchedulerLimits};
use leselang_hir::{Effect, Type};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;
use std::{cell::RefCell, rc::Rc};

fn request(sequence: u64) -> EffectRequest {
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
        deadline_at_ms: None,
        fuel_remaining: 100,
    })
    .unwrap()
}
#[derive(Default)]
struct Hooks {
    trace: Rc<RefCell<Vec<&'static str>>>,
    fail: Option<usize>,
    unwind: bool,
    error: RefCell<Option<Fault>>,
}
impl Hooks {
    fn enter(&self, label: &'static str) -> Result<(), Fault> {
        let index = self.trace.borrow().len();
        self.trace.borrow_mut().push(label);
        if self.fail == Some(index) {
            assert!(!self.unwind, "native unwind");
            return Err(self.error.borrow_mut().take().unwrap());
        }
        Ok(())
    }
}
#[derive(Default)]
struct Memory {
    hooks: Hooks,
    existing: Option<EffectRequest>,
    inserted: Option<(ContinuationToken, EffectRequest)>,
}
impl MemoryPendingJournal for Memory {
    fn request(&self, _: &ContinuationToken) -> Option<&EffectRequest> {
        self.hooks.enter("request").unwrap();
        self.existing.as_ref()
    }
    fn check_capacity(&self, _: SchedulerLimits) -> Result<(), Fault> {
        self.hooks.enter("capacity")
    }
    fn insert_pending(&mut self, token: ContinuationToken, request: &EffectRequest) {
        self.hooks.enter("insert").unwrap();
        self.inserted = Some((token, request.clone()));
    }
}
#[test]
fn memory_absent_requests_are_cold_and_invalid_requests_fail_before_lookup() {
    let mut host = Memory::default();
    register_memory(&mut host, None, SchedulerLimits::default()).unwrap();
    assert!(host.hooks.trace.borrow().is_empty());
    let mut invalid = request(7);
    invalid.continuation.schema_version = 0;
    host.existing = Some(invalid.clone());
    assert!(register_memory(&mut host, Some(&invalid), SchedulerLimits::default()).is_err());
    assert!(host.hooks.trace.borrow().is_empty());
}
#[test]
fn memory_exact_replay_borrows_original_request_and_conflicts_before_capacity() {
    for sequence in [7, 8] {
        let mut host = Memory {
            existing: Some(request(sequence)),
            ..Memory::default()
        };
        let pointer = host.existing.as_ref().unwrap().effect_id.as_ptr();
        let result = register_memory(
            &mut host,
            Some(&request(7)),
            SchedulerLimits {
                max_pending_dispatches: 0,
                ..SchedulerLimits::default()
            },
        );
        assert_eq!(result.is_ok(), sequence == 7);
        if sequence == 8 {
            assert_eq!(result.unwrap_err().code, "LSV4011");
        }
        assert_eq!(host.existing.as_ref().unwrap().effect_id.as_ptr(), pointer);
        assert!(host.inserted.is_none());
        assert_eq!(*host.hooks.trace.borrow(), ["request"]);
    }
}
#[test]
fn memory_capacity_failure_keeps_state_untouched_and_fresh_insert_uses_original_payload() {
    for fail in [false, true] {
        let mut host = Memory::default();
        host.hooks.error.replace(Some(Fault {
            code: "NATIVE".into(),
            message: "original error buffer".into(),
        }));
        let pointer = host.hooks.error.borrow().as_ref().unwrap().message.as_ptr();
        if fail {
            host.hooks.fail = Some(1);
        }
        let input = request(7);
        let result = register_memory(&mut host, Some(&input), SchedulerLimits::default());
        if fail {
            assert_eq!(result.unwrap_err().message.as_ptr(), pointer);
            assert!(host.inserted.is_none());
        } else {
            result.unwrap();
            assert_eq!(
                host.inserted,
                Some((input.continuation.token.clone(), input))
            );
        }
    }
}

struct Durable {
    hooks: Hooks,
    effect: Option<StoredEffect>,
    dispatch: Option<StoredDispatch>,
    count: usize,
}
impl Durable {
    fn new(replay: bool) -> Self {
        Self {
            hooks: Hooks::default(),
            count: 0,
            effect: replay.then(|| StoredEffect {
                image: b"image".to_vec(),
                state: "pending".into(),
                terminal_step: None,
            }),
            dispatch: Some(StoredDispatch {
                request: serde_json::to_vec(&request(7)).unwrap(),
                state: "leased".into(),
                attempt: 2,
                retry_count: 0,
                ready_at_ms: 0,
                lease_expires_at_ms: Some(100),
            }),
        }
    }
}
impl Drop for Durable {
    fn drop(&mut self) {
        self.hooks.trace.borrow_mut().push("drop");
    }
}
impl DurablePendingTransaction for Durable {
    fn effect(&mut self, _: &str) -> Result<Option<StoredEffect>, Fault> {
        self.hooks.enter("effect")?;
        Ok(self.effect.take())
    }
    fn dispatch(&mut self, _: &str) -> Result<Option<StoredDispatch>, Fault> {
        self.hooks.enter("dispatch")?;
        Ok(self.dispatch.take())
    }
    fn record_count(&mut self) -> Result<usize, Fault> {
        self.hooks.enter("count")?;
        Ok(self.count)
    }
    fn check_capacity(&mut self, _: SchedulerLimits) -> Result<(), Fault> {
        self.hooks.enter("capacity")
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        assert!(matches!(additional, 5 | 12));
        self.hooks.enter("quota")
    }
    fn insert_effect(&mut self, image: &ContinuationImage, bytes: &[u8]) -> Result<(), Fault> {
        assert_eq!(image, &request(7).continuation);
        assert_eq!(bytes.as_ptr(), b"image".as_ptr());
        self.hooks.enter("insert-effect")
    }
    fn insert_dispatch(&mut self, _: &str, bytes: &[u8]) -> Result<(), Fault> {
        assert_eq!(bytes.as_ptr(), b"request".as_ptr());
        self.hooks.enter("insert-dispatch")
    }
    fn advance_watermark(&mut self, sequence: u64) -> Result<(), Fault> {
        assert_eq!(sequence, 7);
        self.hooks.enter("watermark")
    }
    fn commit(self, kind: PendingCommit) -> Result<(), Fault> {
        self.hooks.enter(match kind {
            PendingCommit::Existing => "replay-commit",
            PendingCommit::New => "new-commit",
        })
    }
}
fn run(host: Durable, full: bool) -> Result<(), Fault> {
    let input = request(7);
    register_durable(
        host,
        &input.continuation,
        full.then_some(&input),
        SchedulerLimits::default(),
        PendingEncoding {
            image: b"image",
            request: full.then_some(b"request"),
            sequence: 7,
        },
    )
}
fn trace(replay: bool, full: bool) -> Vec<&'static str> {
    let mut out = vec!["effect"];
    if replay {
        if full {
            out.push("dispatch");
        }
    } else {
        out.push("count");
        if full {
            out.push("capacity");
        }
        out.extend(["quota", "insert-effect"]);
        if full {
            out.push("insert-dispatch");
        }
    }
    out.extend([
        "watermark",
        if replay {
            "replay-commit"
        } else {
            "new-commit"
        },
        "drop",
    ]);
    out
}
#[test]
fn durable_new_bare_images_skip_only_dispatch_capacity_and_dispatch_insertion() {
    for full in [false, true] {
        let host = Durable::new(false);
        let observed = host.hooks.trace.clone();
        run(host, full).unwrap();
        assert_eq!(*observed.borrow(), trace(false, full));
    }
}
#[test]
fn durable_replay_bypasses_limits_but_always_reconciles_watermark_and_commits() {
    for full in [false, true] {
        let mut host = Durable::new(true);
        host.count = MAX_JOURNAL_RECORDS;
        if !full {
            host.dispatch = None;
        }
        let observed = host.hooks.trace.clone();
        run(host, full).unwrap();
        assert_eq!(*observed.borrow(), trace(true, full));
    }
}
#[test]
fn durable_conflicting_records_or_invalid_dispatches_cannot_repair_or_advance_watermark() {
    for kind in 0..8 {
        let mut host = Durable::new(true);
        match kind {
            0 => host.effect.as_mut().unwrap().state = "completed".into(),
            1 => host.effect.as_mut().unwrap().state = "unknown".into(),
            2 => host.effect.as_mut().unwrap().image = vec![],
            3 => host.dispatch = None,
            4 => host.dispatch.as_mut().unwrap().request = b"{".to_vec(),
            5 => host.dispatch.as_mut().unwrap().request = serde_json::to_vec(&request(8)).unwrap(),
            6 => {
                let mut r = request(7);
                r.budget.fuel_remaining += 1;
                host.dispatch.as_mut().unwrap().request = serde_json::to_vec(&r).unwrap();
            }
            7 => {
                host.dispatch.as_mut().unwrap().request =
                    vec![b' '; leselang_command::reference_recovery::MAX_JOURNAL_ENTRY_BYTES + 1]
            }
            _ => unreachable!(),
        }
        let observed = host.hooks.trace.clone();
        assert!(run(host, true).is_err());
        assert_eq!(
            *observed.borrow(),
            if kind < 3 {
                vec!["effect", "drop"]
            } else {
                vec!["effect", "dispatch", "drop"]
            }
        );
    }
}
#[test]
fn durable_record_limit_precedes_capacity_and_has_the_original_inclusive_boundary() {
    for full in [false, true] {
        for count in [MAX_JOURNAL_RECORDS - 1, MAX_JOURNAL_RECORDS] {
            let mut host = Durable::new(false);
            host.count = count;
            let observed = host.hooks.trace.clone();
            let result = run(host, full);
            if count == MAX_JOURNAL_RECORDS {
                assert_eq!(result.unwrap_err().code, "LSV4006");
                assert_eq!(*observed.borrow(), ["effect", "count", "drop"]);
            } else {
                result.unwrap();
            }
        }
    }
}
#[test]
fn durable_each_hook_failure_or_unwind_drops_once_without_retry_or_compensation() {
    for replay in [false, true] {
        for full in [false, true] {
            let expected = trace(replay, full);
            for index in 0..expected.len() - 1 {
                for unwind in [false, true] {
                    let mut host = Durable::new(replay);
                    host.hooks.fail = Some(index);
                    host.hooks.unwind = unwind;
                    host.hooks.error.replace(Some(Fault {
                        code: "NATIVE".into(),
                        message: "original fault buffer".into(),
                    }));
                    let pointer = host.hooks.error.borrow().as_ref().unwrap().message.as_ptr();
                    let observed = host.hooks.trace.clone();
                    let result =
                        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(host, full)));
                    if unwind {
                        assert!(result.is_err());
                    } else {
                        assert_eq!(result.unwrap().unwrap_err().message.as_ptr(), pointer);
                    }
                    let mut prefix = expected[..=index].to_vec();
                    prefix.push("drop");
                    assert_eq!(*observed.borrow(), prefix);
                }
            }
        }
    }
}
