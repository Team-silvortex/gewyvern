use leselang_command::reference_group_start::{
    GroupStartContext, ReferenceGroupStartHost, prepare_group_start,
};
use leselang_command::reference_group_successor_commit::*;
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::{
    ContinuationImage, ContinuationToken, EffectRequest, ExecutionOrder, MergePlan, Step, Value,
};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::{Fault, ScalarValue};
use leserpent_hir::{Effect, HirBranch, Type};
use std::cell::RefCell;
#[path = "support/group_start_fixture.rs"]
mod fixture;

struct Alloc(u64);
impl ReferenceGroupStartHost for Alloc {
    fn check_admission(&mut self, _: usize) -> Result<(), Fault> {
        Ok(())
    }
    fn allocate_sequence(&mut self) -> Result<u64, Fault> {
        let n = self.0;
        self.0 += 1;
        Ok(n)
    }
}
fn plan() -> MergePlan {
    let branches = ["left", "right"].map(|name| HirBranch {
        name: name.into(),
        effect: Effect::UiFocus {
            node_id: name.into(),
        },
        result_type: Type::UiFocus,
    });
    prepare_group_start(
        &branches,
        ExecutionOrder::Sequential,
        GroupStartContext {
            principal: Principal::new("operator").unwrap(),
            capabilities: CapabilitySet::new(["ui.presentation"]),
            expected_revision: None,
            deadline_ms: 1000,
            deadline_at_ms: Some(1000),
            fuel_remaining: 100,
        },
        fixture::binding(2),
        &mut Alloc(1),
    )
    .unwrap()
    .plan
}
fn owner() -> ContinuationToken {
    ContinuationToken::from_wire("merge-1".into())
}
fn request() -> EffectRequest {
    build_effect_request(ReservedEffectInput {
        effect: &Effect::UiFocus {
            node_id: "next".into(),
        },
        result_type: Type::UiFocus,
        sequence: 4,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: None,
        deadline_ms: 1000,
        deadline_at_ms: Some(1000),
        fuel_remaining: 90,
    })
    .unwrap()
}
fn prefix() -> Step {
    Step::Done(Value::UiFocus {
        node_id: "node".into(),
    })
}
fn fault() -> Fault {
    Fault {
        code: "OWNER".into(),
        message: "original buffer".into(),
    }
}
struct Host {
    trace: RefCell<Vec<&'static str>>,
    fail: Option<usize>,
    unwind: bool,
    error: RefCell<Option<Fault>>,
    mode: usize,
    pending: Option<Box<EffectRequest>>,
    conflict: usize,
    missing_group: bool,
    plan: MergePlan,
    branches: Vec<ContinuationToken>,
    inserted: Vec<EffectRequest>,
    writes: Vec<&'static str>,
    records: i64,
    updated: usize,
    growth: usize,
}
impl Host {
    fn new() -> Self {
        Self {
            trace: RefCell::default(),
            fail: None,
            unwind: false,
            error: RefCell::new(Some(fault())),
            mode: 0,
            pending: Some(Box::new(request())),
            conflict: 0,
            missing_group: false,
            plan: plan(),
            branches: vec![],
            inserted: vec![],
            writes: vec![],
            records: 3,
            updated: 1,
            growth: 0,
        }
    }
    fn call(&self, name: &'static str) -> Result<(), Fault> {
        let n = self.trace.borrow().len();
        self.trace.borrow_mut().push(name);
        if self.fail == Some(n) {
            assert!(!self.unwind, "owner unwind");
            Err(self.error.borrow_mut().take().unwrap())
        } else {
            Ok(())
        }
    }
}
impl GroupSuccessorOwner for Host {
    fn requests(&mut self, _: &ContinuationToken) -> Result<Vec<EffectRequest>, Fault> {
        self.call("requests")?;
        Ok(vec![request(), request()])
    }
    fn prepare(
        &mut self,
        _: &ContinuationToken,
        _: &MergePlan,
        requests: &[EffectRequest],
        _: &Value,
    ) -> Result<CaptureProgress, Fault> {
        self.call("prepare")?;
        assert_eq!(requests.len(), 2);
        match self.mode {
            0 => Ok(CaptureProgress::Pending(self.pending.take().unwrap())),
            1 => Ok(CaptureProgress::Done(ScalarValue::None)),
            _ => Err(fault()),
        }
    }
    fn validate_graph(
        &mut self,
        _: &ContinuationToken,
        p: &MergePlan,
        g: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault> {
        self.call("graph")?;
        assert_eq!(p.branches, vec!["left", "right", "successor"]);
        assert_eq!(g.len(), 3);
        assert_eq!(g[2].1.continuation.token, request().continuation.token);
        Ok(())
    }
    fn encode_request(&mut self, _: &EffectRequest) -> Result<Vec<u8>, Fault> {
        self.call("request_bytes")?;
        Ok(vec![2; 7])
    }
}
impl MemoryGroupSuccessor for Host {
    fn contains_dispatch(&self, _: &ContinuationToken) -> bool {
        self.call("dispatch?").unwrap();
        self.conflict == 1
    }
    fn contains_group(&self, _: &ContinuationToken) -> bool {
        self.call("group?").unwrap();
        self.conflict == 2
    }
    fn insert_dispatch(&mut self, t: ContinuationToken, r: EffectRequest) {
        self.call("dispatch+").unwrap();
        assert_eq!(t, r.continuation.token);
        self.inserted.push(r);
    }
    fn group_mut(&mut self, _: &ContinuationToken) -> Option<MemorySuccessorGroup<'_>> {
        self.call("owner_mut").unwrap();
        (!self.missing_group).then_some(MemorySuccessorGroup {
            plan: &mut self.plan,
            branches: &mut self.branches,
        })
    }
}
impl DurableGroupSuccessor for Host {
    fn record_count(&mut self) -> Result<i64, Fault> {
        self.call("count")?;
        Ok(self.records)
    }
    fn encode_image(&mut self, _: &ContinuationImage) -> Result<Vec<u8>, Fault> {
        self.call("image_bytes")?;
        Ok(vec![1; 3])
    }
    fn encode_plan(&mut self, _: &MergePlan, k: SuccessorPlanEncoding) -> Result<Vec<u8>, Fault> {
        self.call(match k {
            SuccessorPlanEncoding::Next => "next_bytes",
            SuccessorPlanEncoding::Prefix => "prefix_bytes",
        })?;
        Ok(vec![
            3;
            if k == SuccessorPlanEncoding::Next {
                13
            } else {
                11
            }
        ])
    }
    fn ensure_growth(&mut self, n: usize) -> Result<(), Fault> {
        self.call("growth")?;
        self.growth = n;
        Ok(())
    }
    fn insert_effect(&mut self, _: &str, b: &[u8], deadline: Option<i64>) -> Result<(), Fault> {
        self.call("effect+")?;
        assert_eq!(b, &[1; 3]);
        assert_eq!(deadline, Some(1000));
        self.writes.push("effect");
        Ok(())
    }
    fn insert_dispatch(&mut self, _: &str, b: &[u8]) -> Result<(), Fault> {
        self.call("dispatch+")?;
        assert_eq!(b, &[2; 7]);
        self.writes.push("dispatch");
        Ok(())
    }
    fn update_plan(&mut self, _: &str, b: &[u8]) -> Result<usize, Fault> {
        self.call("plan+")?;
        assert_eq!(b, &[3; 13]);
        self.writes.push("plan");
        Ok(self.updated)
    }
    fn link_branch(&mut self, _: &str, _: &str, name: &str, pos: usize) -> Result<(), Fault> {
        self.call("link+")?;
        assert_eq!(name, "successor");
        assert_eq!(pos, 2);
        self.writes.push("link");
        Ok(())
    }
}
const MEMORY: &[&str] = &[
    "requests",
    "prepare",
    "graph",
    "dispatch?",
    "group?",
    "request_bytes",
    "dispatch+",
    "owner_mut",
];
const DURABLE: &[&str] = &[
    "requests",
    "prepare",
    "graph",
    "count",
    "image_bytes",
    "request_bytes",
    "next_bytes",
    "prefix_bytes",
    "growth",
    "effect+",
    "dispatch+",
    "plan+",
    "link+",
];
#[test]
fn non_done_prefix_bypasses_all_reads_even_with_missing_owner_and_invalid_plan() {
    for step in [Step::Fault(fault()), Step::Yield(request().continuation)] {
        let mut p = plan();
        p.result_binding = None;
        let mut h = Host::new();
        h.fail = Some(0);
        assert_eq!(
            admit_memory(&mut h, &owner(), &p, &step).unwrap(),
            Some(step.clone())
        );
        assert_eq!(
            admit_durable(&mut h, &owner(), &p, &step).unwrap(),
            Some(step)
        );
        assert!(h.trace.borrow().is_empty());
    }
}
#[test]
fn only_prepare_errors_are_terminal_and_done_does_not_validate_the_next_plan() {
    for durable in [false, true] {
        for mode in 0..3 {
            let mut h = Host::new();
            let mut p = plan();
            p.result_binding = None;
            if mode == 0 {
                h.fail = Some(0)
            } else if mode == 1 {
                h.fail = Some(1)
            } else {
                h.mode = 1
            }
            let r = if durable {
                admit_durable(&mut h, &owner(), &p, &prefix())
            } else {
                admit_memory(&mut h, &owner(), &p, &prefix())
            };
            match mode {
                0 => assert_eq!(r.unwrap_err(), fault()),
                1 => assert_eq!(r.unwrap(), Some(Step::Fault(fault()))),
                _ => assert_eq!(
                    r.unwrap(),
                    Some(Step::Done(Value::Scalar {
                        value: ScalarValue::None
                    }))
                ),
            }
            assert_eq!(h.trace.borrow().len(), if mode == 0 { 1 } else { 2 });
        }
    }
}
#[test]
fn next_plan_admission_precedes_native_graph_proof_and_graph_failure_precedes_writes() {
    let mut p = plan();
    p.result_binding = None;
    let mut h = Host::new();
    assert!(admit_memory(&mut h, &owner(), &p, &prefix()).is_err());
    assert_eq!(*h.trace.borrow(), ["requests", "prepare"]);
    for durable in [false, true] {
        let mut h = Host::new();
        h.fail = Some(2);
        let ptr = h.error.borrow().as_ref().unwrap().message.as_ptr();
        let r = if durable {
            admit_durable(&mut h, &owner(), &plan(), &prefix())
        } else {
            admit_memory(&mut h, &owner(), &plan(), &prefix())
        };
        assert_eq!(r.unwrap_err().message.as_ptr(), ptr);
        assert_eq!(*h.trace.borrow(), ["requests", "prepare", "graph"]);
        assert!(h.inserted.is_empty());
        assert!(h.writes.is_empty());
    }
}
#[test]
fn memory_conflicts_short_circuit_and_late_owner_loss_keeps_original_inserted_payload() {
    for conflict in 1..=2 {
        let mut h = Host::new();
        h.conflict = conflict;
        assert!(admit_memory(&mut h, &owner(), &plan(), &prefix()).is_err());
        assert_eq!(*h.trace.borrow(), MEMORY[..3 + conflict]);
        assert!(h.inserted.is_empty());
    }
    for missing in [false, true] {
        let mut h = Host::new();
        h.missing_group = missing;
        let ptr = h.pending.as_ref().unwrap().effect_id.as_ptr();
        let r = admit_memory(&mut h, &owner(), &plan(), &prefix());
        assert_eq!(r.is_err(), missing);
        assert_eq!(*h.trace.borrow(), MEMORY);
        assert_eq!(h.inserted[0].effect_id.as_ptr(), ptr);
        assert_eq!(h.inserted[0], request());
        assert_eq!(h.branches.len(), usize::from(!missing));
        assert_eq!(h.plan.branches.len(), if missing { 2 } else { 3 });
    }
}
#[test]
fn durable_order_and_incremental_quota_require_one_pending_owner_before_linking() {
    for updated in 0..=2 {
        let mut h = Host::new();
        h.updated = updated;
        let r = admit_durable(&mut h, &owner(), &plan(), &prefix());
        assert_eq!(r.is_ok(), updated == 1);
        assert_eq!(h.growth, 12);
        assert_eq!(
            *h.trace.borrow(),
            DURABLE[..if updated == 1 { 13 } else { 12 }]
        );
        assert_eq!(h.writes.len(), if updated == 1 { 4 } else { 3 });
        if updated != 1 {
            assert_eq!(
                r.unwrap_err().message,
                "group successor lost its pending owner"
            );
        }
    }
}
#[test]
fn durable_record_limit_precedes_encodings_and_signed_deadline_follows_all_four_buffers() {
    let mut h = Host::new();
    h.records = leselang_command::retention_policy::MAX_JOURNAL_RECORDS as i64;
    assert_eq!(
        admit_durable(&mut h, &owner(), &plan(), &prefix())
            .unwrap_err()
            .code,
        "LSV4008"
    );
    assert_eq!(*h.trace.borrow(), DURABLE[..4]);
    let mut h = Host::new();
    h.pending.as_mut().unwrap().continuation.deadline_at_ms = Some(u64::MAX);
    assert!(admit_durable(&mut h, &owner(), &plan(), &prefix()).is_err());
    assert_eq!(*h.trace.borrow(), DURABLE[..8]);
    assert!(h.writes.is_empty());
}
#[test]
fn durable_hook_errors_and_unwinds_preserve_stop_order_without_retry_or_compensation() {
    for at in 0..DURABLE.len() {
        for unwind in [false, true] {
            let mut h = Host::new();
            h.fail = Some(at);
            h.unwind = unwind;
            let ptr = h.error.borrow().as_ref().unwrap().message.as_ptr();
            let r = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                admit_durable(&mut h, &owner(), &plan(), &prefix())
            }));
            if unwind {
                assert!(r.is_err());
            } else {
                let error = if at == 1 {
                    let Some(Step::Fault(e)) = r.unwrap().unwrap() else {
                        panic!()
                    };
                    e
                } else {
                    r.unwrap().unwrap_err()
                };
                assert_eq!(error.message.as_ptr(), ptr);
            }
            assert_eq!(*h.trace.borrow(), DURABLE[..=at]);
            assert_eq!(h.writes.len(), at.saturating_sub(9));
        }
    }
}
#[test]
fn memory_hook_unwind_retains_only_writes_before_failure() {
    for at in 0..MEMORY.len() {
        let mut h = Host::new();
        h.fail = Some(at);
        h.unwind = true;
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| admit_memory(
                &mut h,
                &owner(),
                &plan(),
                &prefix()
            )))
            .is_err()
        );
        assert_eq!(*h.trace.borrow(), MEMORY[..=at]);
        assert_eq!(h.inserted.len(), usize::from(at == 7));
        assert!(h.branches.is_empty());
    }
}
