use leselang_command::reference_group_progress::*;
use leselang_command::reference_group_start::{
    GroupStartContext, ReferenceGroupStartHost, prepare_group_start,
};
use leselang_command::reference_request_build::{ReservedEffectInput, build_effect_request};
use leselang_command::{ContinuationToken, EffectRequest, ExecutionOrder, MergePlan, Step, Value};
use leselang_host_contract::{CapabilitySet, Principal};
use leselang_runtime_core::Fault;
use leserpent_hir::{Effect, HirBranch, Type};
use std::cell::RefCell;
#[path = "support/group_start_fixture.rs"]
mod fixture;

fn token(n: usize) -> ContinuationToken {
    ContinuationToken::from_wire(format!("continuation-{n}"))
}
fn done() -> Step {
    Step::Done(Value::UiFocus {
        node_id: "node".into(),
    })
}
fn request() -> EffectRequest {
    build_effect_request(ReservedEffectInput {
        effect: &Effect::UiFocus {
            node_id: "node".into(),
        },
        result_type: Type::UiFocus,
        sequence: 1,
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new(["ui.presentation"]),
        expected_revision: None,
        deadline_ms: 1000,
        deadline_at_ms: None,
        fuel_remaining: 100,
    })
    .unwrap()
}
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
fn successor_plan() -> MergePlan {
    let branches = ["a", "b"].map(|name| HirBranch {
        name: name.into(),
        effect: Effect::UiFocus {
            node_id: name.into(),
        },
        result_type: Type::UiFocus,
    });
    let mut p = prepare_group_start(
        &branches,
        ExecutionOrder::Parallel,
        GroupStartContext {
            principal: Principal::new("operator").unwrap(),
            capabilities: CapabilitySet::new(["ui.presentation"]),
            expected_revision: None,
            deadline_ms: 1000,
            deadline_at_ms: None,
            fuel_remaining: 100,
        },
        fixture::binding(2),
        &mut Alloc(1),
    )
    .unwrap()
    .plan;
    p.branches.push("successor".into());
    p
}
struct Memory {
    owner: ContinuationToken,
    plan: MergePlan,
    branches: Vec<ContinuationToken>,
    request: EffectRequest,
    states: [Option<(bool, bool)>; 2],
    terminal: Option<Step>,
    absent: bool,
    reads: RefCell<Vec<ContinuationToken>>,
}
impl Memory {
    fn new() -> Self {
        Self {
            owner: token(9),
            plan: MergePlan::new(["a", "b"]).unwrap(),
            branches: vec![token(1), token(2)],
            request: request(),
            states: [Some((false, false)); 2],
            terminal: None,
            absent: false,
            reads: RefCell::default(),
        }
    }
}
impl MemoryGroupProgress for Memory {
    fn locate(&self, _: &ContinuationToken) -> Option<MemoryProgressGroup<'_>> {
        (!self.absent).then_some(MemoryProgressGroup {
            token: &self.owner,
            plan: &self.plan,
            branches: &self.branches,
            terminal: self.terminal.as_ref(),
        })
    }
    fn dispatch(&self, t: &ContinuationToken) -> Option<MemoryProgressDispatch<'_>> {
        self.reads.borrow_mut().push(t.clone());
        self.states[self.branches.iter().position(|b| b == t).unwrap()].map(
            |(acknowledged, terminal)| MemoryProgressDispatch {
                request: &self.request,
                acknowledged,
                terminal,
            },
        )
    }
}
#[test]
fn memory_absence_and_terminal_bypass_dispatch_reads_and_admission() {
    let mut h = Memory::new();
    h.absent = true;
    assert!(matches!(
        read_memory(&h, &token(1)),
        MergeProgress::Standalone
    ));
    h.absent = false;
    h.terminal = Some(Step::Yield(h.request.continuation.clone()));
    let MergeProgress::Completed {
        step,
        branch_tokens,
        ..
    } = read_memory(&h, &token(1))
    else {
        panic!()
    };
    assert_eq!(*step, h.terminal.clone().unwrap());
    assert_eq!(branch_tokens, h.branches);
    assert!(h.reads.borrow().is_empty());
}
#[test]
fn memory_next_skips_missing_uses_ack_and_retains_duplicate_member_order() {
    let mut h = Memory::new();
    h.plan.order = ExecutionOrder::Sequential;
    h.states = [None, Some((false, true))];
    assert!(matches!(
        read_memory(&h, &token(1)),
        MergeProgress::Next { .. }
    ));
    h.reads.borrow_mut().clear();
    h.states = [Some((true, false)), Some((false, true))];
    h.branches.push(token(1));
    let MergeProgress::Next {
        request,
        completed_tokens,
    } = read_memory(&h, &token(1))
    else {
        panic!()
    };
    assert_eq!(*request, h.request);
    assert_eq!(completed_tokens, vec![token(1), token(1)]);
    assert_eq!(
        *h.reads.borrow(),
        vec![token(1), token(2), token(1), token(2), token(1)]
    );
}
#[test]
fn memory_pending_counts_terminals_not_ack_and_successor_does_not_require_order_marker() {
    let mut h = Memory::new();
    h.states = [Some((true, false)), Some((false, true))];
    assert!(matches!(
        read_memory(&h, &token(1)),
        MergeProgress::Pending {
            completed_branches: 1,
            total_branches: 2,
            ..
        }
    ));
    h.plan = successor_plan();
    h.plan.order = ExecutionOrder::Parallel;
    assert!(matches!(
        read_memory(&h, &token(1)),
        MergeProgress::Next { .. }
    ));
    h.states = [Some((true, false)); 2];
    assert!(matches!(
        read_memory(&h, &token(1)),
        MergeProgress::Pending {
            completed_branches: 0,
            ..
        }
    ));
}

struct Durable {
    row: Option<StoredGroupProgress>,
    plan: Vec<u8>,
    request: Vec<u8>,
    trace: Vec<&'static str>,
    fail: Option<usize>,
    unwind: bool,
}
impl Durable {
    fn new() -> Self {
        Self {
            row: Some(StoredGroupProgress {
                token: "merge-9".into(),
                state: "pending".into(),
                terminal: None,
                completed: 0,
                total: 2,
                order: "sequential".into(),
            }),
            plan: serde_json::to_vec(&successor_plan()).unwrap(),
            request: serde_json::to_vec(&request()).unwrap(),
            trace: vec![],
            fail: None,
            unwind: false,
        }
    }
    fn call(&mut self, name: &'static str) -> Result<(), Fault> {
        let n = self.trace.len();
        self.trace.push(name);
        if self.fail == Some(n) {
            assert!(!self.unwind, "original owner unwind");
            Err(Fault {
                code: "OWNER".into(),
                message: name.into(),
            })
        } else {
            Ok(())
        }
    }
}
impl DurableGroupProgress for Durable {
    fn locate(&mut self, _: &str) -> Result<Option<StoredGroupProgress>, Fault> {
        self.call("locate")?;
        Ok(self.row.take())
    }
    fn plan(&mut self, _: &str) -> Result<Vec<u8>, Fault> {
        self.call("plan")?;
        Ok(self.plan.clone())
    }
    fn next_request(&mut self, _: &str) -> Result<Vec<u8>, Fault> {
        self.call("request")?;
        Ok(self.request.clone())
    }
    fn completed_tokens(&mut self, _: &str) -> Result<Vec<ContinuationToken>, Fault> {
        self.call("prefix")?;
        Ok(vec![token(2), token(1), token(2)])
    }
    fn branch_tokens(&mut self, _: &str) -> Result<Vec<ContinuationToken>, Fault> {
        self.call("branches")?;
        Ok(vec![token(2), token(1), token(2)])
    }
}
fn completed_host() -> Durable {
    let mut h = Durable::new();
    let r = h.row.as_mut().unwrap();
    r.state = "completed".into();
    r.completed = 2;
    r.terminal = Some(serde_json::to_vec(&done()).unwrap());
    h
}
#[test]
fn durable_count_conversion_precedes_state_checks_and_absence_skips_everything() {
    let mut h = Durable::new();
    h.row = None;
    assert!(matches!(
        read_durable(&mut h, &token(1)),
        Ok(MergeProgress::Standalone)
    ));
    assert_eq!(h.trace, ["locate"]);
    for (complete, total, message) in [
        (-1, -1, "merge completion count is invalid"),
        (0, -1, "merge branch count is invalid"),
        (2, 2, "merge progress has an invalid durable state"),
    ] {
        let mut h = Durable::new();
        let r = h.row.as_mut().unwrap();
        r.completed = complete;
        r.total = total;
        r.state = "bad".into();
        let e = read_durable(&mut h, &token(1)).err().unwrap();
        assert_eq!(e.code, "LSV4007");
        assert_eq!(e.message, message);
        assert_eq!(h.trace, ["locate"]);
    }
}
#[test]
fn durable_sequential_and_completed_keep_ordered_tokens_and_skip_plan() {
    let mut h = Durable::new();
    h.plan = b"{".to_vec();
    let MergeProgress::Next {
        request: r,
        completed_tokens,
    } = read_durable(&mut h, &token(1)).unwrap()
    else {
        panic!()
    };
    assert_eq!(*r, request());
    assert_eq!(completed_tokens, vec![token(2), token(1), token(2)]);
    assert_eq!(h.trace, ["locate", "request", "prefix"]);
    let mut h = completed_host();
    h.plan = b"{".to_vec();
    let MergeProgress::Completed {
        step,
        branch_tokens,
        ..
    } = read_durable(&mut h, &token(1)).unwrap()
    else {
        panic!()
    };
    assert_eq!(*step, done());
    assert_eq!(branch_tokens, vec![token(2), token(1), token(2)]);
    assert_eq!(h.trace, ["locate", "branches"]);
}
#[test]
fn durable_parallel_probes_only_last_pending_member_and_requires_successor_marker() {
    for mode in 0..5 {
        let mut h = Durable::new();
        let r = h.row.as_mut().unwrap();
        r.order = if mode == 4 { "unknown" } else { "parallel" }.into();
        r.completed = if mode == 0 { 0 } else { 1 };
        if mode == 2 {
            let mut p = successor_plan();
            p.order = ExecutionOrder::Parallel;
            h.plan = serde_json::to_vec(&p).unwrap();
        }
        if mode == 3 {
            let mut p = successor_plan();
            p.branches.pop();
            h.plan = serde_json::to_vec(&p).unwrap();
        }
        let result = read_durable(&mut h, &token(1)).unwrap();
        assert_eq!(matches!(result, MergeProgress::Next { .. }), mode == 1);
        assert_eq!(
            h.trace,
            match mode {
                0 | 4 => vec!["locate"],
                1 => vec!["locate", "plan", "request", "prefix"],
                _ => vec!["locate", "plan"],
            }
        );
    }
}
#[test]
fn durable_bounded_decode_and_admission_fail_before_dependent_reads() {
    use leselang_command::reference_recovery::MAX_JOURNAL_ENTRY_BYTES;
    for mode in 0..7 {
        let mut h = if mode >= 3 {
            completed_host()
        } else {
            Durable::new()
        };
        match mode {
            0 => h.request = b"{".to_vec(),
            1 => {
                let mut r = request();
                r.continuation.token = token(0);
                h.request = serde_json::to_vec(&r).unwrap();
            }
            2 => h.request = vec![b' '; MAX_JOURNAL_ENTRY_BYTES + 1],
            3 => h.row.as_mut().unwrap().terminal = Some(b"{".to_vec()),
            4 => {
                h.row.as_mut().unwrap().terminal =
                    Some(serde_json::to_vec(&Step::Yield(request().continuation)).unwrap())
            }
            5 => h.row.as_mut().unwrap().terminal = Some(vec![b' '; MAX_JOURNAL_ENTRY_BYTES + 1]),
            6 => {
                h = Durable::new();
                let r = h.row.as_mut().unwrap();
                r.order = "parallel".into();
                r.completed = 1;
                h.plan = b"{".to_vec();
            }
            _ => unreachable!(),
        }
        assert!(read_durable(&mut h, &token(1)).is_err(), "{mode}");
        assert_eq!(
            h.trace,
            if mode < 3 {
                vec!["locate", "request"]
            } else if mode == 6 {
                vec!["locate", "plan"]
            } else {
                vec!["locate"]
            }
        );
    }
}
#[test]
fn durable_hook_failures_and_unwinds_escape_once_without_fallback_reads() {
    for complete in [false, true] {
        let expected = if complete {
            vec!["locate", "branches"]
        } else {
            vec!["locate", "plan", "request", "prefix"]
        };
        for fail in 0..expected.len() {
            for unwind in [false, true] {
                let mut h = if complete {
                    completed_host()
                } else {
                    Durable::new()
                };
                if !complete {
                    let r = h.row.as_mut().unwrap();
                    r.order = "parallel".into();
                    r.completed = 1;
                }
                h.fail = Some(fail);
                h.unwind = unwind;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    read_durable(&mut h, &token(1))
                }));
                if unwind {
                    assert!(result.is_err());
                } else {
                    assert_eq!(
                        result.unwrap().err().unwrap(),
                        Fault {
                            code: "OWNER".into(),
                            message: expected[fail].into()
                        }
                    );
                }
                assert_eq!(h.trace, expected[..=fail]);
            }
        }
    }
}
