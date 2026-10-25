use leselang_command::reference_merge_graph_commit::*;
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::retention_policy::MAX_JOURNAL_RECORDS;
use leselang_command::{ContinuationToken, EffectRequest, MergePlan, SchedulerLimits};
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
fn token() -> ContinuationToken {
    ContinuationToken::from_wire("merge-100".into())
}
fn plan() -> MergePlan {
    MergePlan::new(["left", "right"]).unwrap()
}
#[derive(Default)]
struct Hooks {
    trace: Rc<RefCell<Vec<&'static str>>>,
    fail: Option<usize>,
    unwind: bool,
    error: RefCell<Option<Fault>>,
}
impl Hooks {
    fn enter(&self, name: &'static str) -> Result<(), Fault> {
        let index = self.trace.borrow().len();
        self.trace.borrow_mut().push(name);
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
    conflict: Option<usize>,
    inserted: Vec<EffectRequest>,
    group: Option<(ContinuationToken, MergePlan, Vec<ContinuationToken>)>,
}
impl MemoryMergeGraphJournal for Memory {
    fn contains_group(&self, _: &ContinuationToken) -> bool {
        let n = self.hooks.trace.borrow().len();
        self.hooks.enter("group?").unwrap();
        self.conflict == Some(n)
    }
    fn contains_dispatch(&self, _: &ContinuationToken) -> bool {
        let n = self.hooks.trace.borrow().len();
        self.hooks.enter("dispatch?").unwrap();
        self.conflict == Some(n)
    }
    fn check_capacity(&self, additional: usize, _: SchedulerLimits) -> Result<(), Fault> {
        assert_eq!(additional, 2);
        self.hooks.enter("capacity")
    }
    fn insert_dispatch(&mut self, request: &EffectRequest) {
        self.hooks.enter("dispatch+").unwrap();
        self.inserted.push(request.clone());
    }
    fn insert_group(
        &mut self,
        token: &ContinuationToken,
        plan: &MergePlan,
        branches: Vec<ContinuationToken>,
    ) {
        self.hooks.enter("group+").unwrap();
        self.group = Some((token.clone(), plan.clone(), branches));
    }
}
fn memory_run(host: &mut Memory) -> Result<(), Fault> {
    register_memory(
        host,
        &token(),
        &plan(),
        &[("left", &request(7)), ("right", &request(8))],
        SchedulerLimits::default(),
    )
}
const MEMORY_TRACE: &[&str] = &[
    "group?",
    "dispatch?",
    "dispatch?",
    "group?",
    "dispatch?",
    "group?",
    "capacity",
    "dispatch+",
    "dispatch+",
    "group+",
];
#[test]
fn memory_registration_keeps_conflict_order_and_native_insertion_payloads() {
    let mut host = Memory::default();
    memory_run(&mut host).unwrap();
    assert_eq!(*host.hooks.trace.borrow(), MEMORY_TRACE);
    assert_eq!(host.inserted, [request(7), request(8)]);
    assert_eq!(
        host.group,
        Some((
            token(),
            plan(),
            vec![request(7).continuation.token, request(8).continuation.token]
        ))
    );
}
#[test]
fn memory_every_conflict_short_circuits_before_capacity_and_mutation() {
    for at in 0..6 {
        let mut host = Memory {
            conflict: Some(at),
            ..Memory::default()
        };
        assert_eq!(memory_run(&mut host).unwrap_err().code, "LSV4011");
        assert_eq!(*host.hooks.trace.borrow(), MEMORY_TRACE[..=at]);
        assert!(host.inserted.is_empty());
        assert!(host.group.is_none());
    }
}
#[test]
fn memory_encoding_precedes_conflict_and_capacity_preserves_original_fault() {
    let mut bad = request(8);
    bad.continuation.pending_effect = Effect::UiFocus {
        node_id: "x".repeat(100_000),
    };
    let mut host = Memory {
        conflict: Some(0),
        ..Memory::default()
    };
    assert!(
        register_memory(
            &mut host,
            &token(),
            &plan(),
            &[("left", &request(7)), ("right", &bad)],
            SchedulerLimits::default()
        )
        .is_err()
    );
    assert!(host.hooks.trace.borrow().is_empty());
    host.conflict = None;
    host.hooks.fail = Some(6);
    host.hooks.error.replace(Some(Fault {
        code: "CAPACITY".into(),
        message: "original buffer".into(),
    }));
    let pointer = host.hooks.error.borrow().as_ref().unwrap().message.as_ptr();
    assert_eq!(memory_run(&mut host).unwrap_err().message.as_ptr(), pointer);
    assert!(host.inserted.is_empty());
}
#[test]
fn memory_owner_unwind_keeps_the_original_write_prefix_without_compensation() {
    for at in 7..10 {
        let mut host = Memory::default();
        host.hooks.fail = Some(at);
        host.hooks.unwind = true;
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| memory_run(&mut host)))
                .is_err()
        );
        assert_eq!(*host.hooks.trace.borrow(), MEMORY_TRACE[..=at]);
        assert_eq!(host.inserted.len(), at - 7);
        assert!(host.group.is_none());
    }
}
struct Durable {
    hooks: Hooks,
    count: usize,
    conflict: bool,
}
impl Durable {
    fn new() -> Self {
        Self {
            hooks: Hooks::default(),
            count: 0,
            conflict: false,
        }
    }
}
impl Drop for Durable {
    fn drop(&mut self) {
        self.hooks.trace.borrow_mut().push("drop");
    }
}
impl DurableMergeGraphTransaction for Durable {
    fn record_count(&mut self) -> Result<usize, Fault> {
        self.hooks.enter("count")?;
        Ok(self.count)
    }
    fn check_capacity(&mut self, additional: usize, _: SchedulerLimits) -> Result<(), Fault> {
        assert_eq!(additional, 2);
        self.hooks.enter("capacity")
    }
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault> {
        assert_eq!(additional, 5);
        self.hooks.enter("quota")
    }
    fn group_conflicts(&mut self, t: &ContinuationToken) -> Result<bool, Fault> {
        assert_eq!(t, &token());
        self.hooks.enter("conflict")?;
        Ok(self.conflict)
    }
    fn insert_group(
        &mut self,
        t: &ContinuationToken,
        p: &MergePlan,
        bytes: &[u8],
    ) -> Result<(), Fault> {
        assert_eq!(t, &token());
        assert_eq!(p, &plan());
        assert_eq!(bytes, b"p");
        self.hooks.enter("group")
    }
    fn insert_branch(
        &mut self,
        t: &ContinuationToken,
        r: &EffectRequest,
        position: usize,
        encoded: &EncodedMergeBranch<'_>,
    ) -> Result<(), Fault> {
        assert_eq!(t, &token());
        assert_eq!(r, &request(7 + position as u64));
        assert_eq!(encoded.0, ["left", "right"][position]);
        assert_eq!(encoded.1, b"i");
        assert_eq!(encoded.2, b"r");
        self.hooks
            .enter(if position == 0 { "left" } else { "right" })
    }
    fn advance_watermark(&mut self, seq: u64) -> Result<(), Fault> {
        assert_eq!(seq, 100);
        self.hooks.enter("watermark")
    }
    fn commit(self) -> Result<(), Fault> {
        self.hooks.enter("commit")
    }
}
fn durable_run(host: Durable) -> Result<(), Fault> {
    let encoded = [
        ("left", b"i".to_vec(), b"r".to_vec()),
        ("right", b"i".to_vec(), b"r".to_vec()),
    ];
    register_durable(
        host,
        &token(),
        &plan(),
        &[("left", &request(7)), ("right", &request(8))],
        SchedulerLimits::default(),
        MergeGraphEncoding {
            plan: b"p",
            branches: &encoded,
            additional_bytes: 5,
        },
    )
}
const DURABLE_TRACE: &[&str] = &[
    "count",
    "capacity",
    "quota",
    "conflict",
    "group",
    "left",
    "right",
    "watermark",
    "commit",
    "drop",
];
#[test]
fn durable_registration_orders_limits_group_branches_watermark_and_consuming_commit() {
    let host = Durable::new();
    let trace = host.hooks.trace.clone();
    durable_run(host).unwrap();
    assert_eq!(*trace.borrow(), DURABLE_TRACE);
}
#[test]
fn durable_record_limit_is_inclusive_and_uses_saturating_arithmetic() {
    for count in [MAX_JOURNAL_RECORDS - 3, MAX_JOURNAL_RECORDS - 2, usize::MAX] {
        let mut host = Durable::new();
        host.count = count;
        let trace = host.hooks.trace.clone();
        let result = durable_run(host);
        if count == MAX_JOURNAL_RECORDS - 3 {
            result.unwrap();
        } else {
            assert_eq!(result.unwrap_err().code, "LSV4006");
            assert_eq!(*trace.borrow(), ["count", "drop"]);
        }
    }
}
#[test]
fn durable_conflict_follows_capacity_and_quota_without_writes_or_replay_repair() {
    let mut host = Durable::new();
    host.conflict = true;
    let trace = host.hooks.trace.clone();
    assert_eq!(durable_run(host).unwrap_err().code, "LSV4011");
    assert_eq!(
        *trace.borrow(),
        ["count", "capacity", "quota", "conflict", "drop"]
    );
}
#[test]
fn durable_every_hook_error_or_unwind_consumes_once_without_retry_or_compensation() {
    for at in 0..DURABLE_TRACE.len() - 1 {
        for unwind in [false, true] {
            let mut host = Durable::new();
            host.hooks.fail = Some(at);
            host.hooks.unwind = unwind;
            host.hooks.error.replace(Some(Fault {
                code: "NATIVE".into(),
                message: "original buffer".into(),
            }));
            let pointer = host.hooks.error.borrow().as_ref().unwrap().message.as_ptr();
            let trace = host.hooks.trace.clone();
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| durable_run(host)));
            if unwind {
                assert!(result.is_err());
            } else {
                assert_eq!(result.unwrap().unwrap_err().message.as_ptr(), pointer);
            }
            let mut expected = DURABLE_TRACE[..=at].to_vec();
            expected.push("drop");
            assert_eq!(*trace.borrow(), expected);
        }
    }
}
