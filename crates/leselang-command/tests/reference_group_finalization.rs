use leselang_command::reference_group_finalization::*;
use leselang_command::{
    BranchCompletion, ContinuationToken, ExecutionOrder, MergePlan, Step, Value,
};
use leselang_runtime_core::Fault;
use std::cell::RefCell;

fn token(n: usize) -> ContinuationToken {
    ContinuationToken::from_wire(format!("continuation-{n}"))
}
fn done() -> Step {
    Step::Done(Value::UiFocus {
        node_id: "node".into(),
    })
}
fn stop() -> Step {
    Step::Fault(Fault {
        code: "HOST".into(),
        message: "stopped".into(),
    })
}
fn plan(order: ExecutionOrder) -> MergePlan {
    let mut p = MergePlan::new(["a", "b"]).unwrap();
    p.order = order;
    p
}
#[derive(Default)]
struct Hooks {
    trace: RefCell<Vec<&'static str>>,
    fail: Option<usize>,
    unwind: bool,
}
impl Hooks {
    fn call(&self, name: &'static str) -> Result<(), Fault> {
        let i = self.trace.borrow().len();
        self.trace.borrow_mut().push(name);
        if self.fail == Some(i) {
            assert!(!self.unwind, "owner unwind");
            Err(Fault {
                code: "OWNER".into(),
                message: name.into(),
            })
        } else {
            Ok(())
        }
    }
}
struct Branch {
    token: ContinuationToken,
    step: Option<Step>,
    ack: bool,
    expiry: Option<u64>,
}
struct Memory {
    hooks: Hooks,
    plan: MergePlan,
    tokens: Vec<ContinuationToken>,
    branches: Vec<Branch>,
    terminal: Option<Step>,
    absent: bool,
    missing_group: bool,
    missing_write: bool,
    successor: Option<Step>,
}
impl Memory {
    fn new(order: ExecutionOrder) -> Self {
        Self {
            hooks: Hooks::default(),
            plan: plan(order),
            tokens: vec![token(1), token(2)],
            branches: (1..=2)
                .map(|n| Branch {
                    token: token(n),
                    step: Some(done()),
                    ack: true,
                    expiry: Some(100),
                })
                .collect(),
            terminal: None,
            absent: false,
            missing_group: false,
            missing_write: false,
            successor: None,
        }
    }
}
impl GroupFinalizationOwner for Memory {
    fn merge_prefix(&mut self, _: &MergePlan, _: Vec<BranchCompletion>) -> Result<Step, Fault> {
        self.hooks.call("prefix")?;
        Ok(done())
    }
    fn admit_successor(
        &mut self,
        _: &ContinuationToken,
        _: &MergePlan,
        _: &Step,
    ) -> Result<Option<Step>, Fault> {
        self.hooks.call("successor")?;
        Ok(self.successor.clone())
    }
    fn finish(&mut self, _: &MergePlan, completions: Vec<BranchCompletion>) -> Result<Step, Fault> {
        self.hooks.call("finish")?;
        assert_eq!(completions.len(), 2);
        Ok(done())
    }
}
impl MemoryGroupFinalization for Memory {
    fn locate(&self, _: &ContinuationToken) -> Option<ContinuationToken> {
        self.hooks.call("locate").unwrap();
        (!self.absent).then(|| token(0))
    }
    fn group(&self, _: &ContinuationToken) -> Option<MemoryGroupState<'_>> {
        self.hooks.call("group").unwrap();
        (!self.missing_group).then_some(MemoryGroupState {
            plan: &self.plan,
            branches: &self.tokens,
            completed: self.terminal.is_some(),
        })
    }
    fn branch_terminal(&self, t: &ContinuationToken) -> Option<&Step> {
        self.hooks.call("read").unwrap();
        self.branches
            .iter()
            .find(|b| &b.token == t)
            .and_then(|b| b.step.as_ref())
    }
    fn branch_mut(&mut self, t: &ContinuationToken) -> Option<MemoryStoppedBranch<'_>> {
        self.hooks.call("branch_write").unwrap();
        self.branches
            .iter_mut()
            .find(|b| &b.token == t)
            .map(|b| MemoryStoppedBranch {
                acknowledged: &mut b.ack,
                lease_expires_at_ms: &mut b.expiry,
                terminal_step: &mut b.step,
            })
    }
    fn group_terminal_mut(&mut self, _: &ContinuationToken) -> Option<&mut Option<Step>> {
        self.hooks.call("group_write").unwrap();
        (!self.missing_write).then_some(&mut self.terminal)
    }
}
#[test]
fn memory_absent_and_completed_groups_do_not_read_or_validate_branches() {
    let mut h = Memory::new(ExecutionOrder::Sequential);
    h.absent = true;
    finalize_memory(&mut h, &token(1)).unwrap();
    assert_eq!(*h.hooks.trace.borrow(), ["locate"]);
    let mut h = Memory::new(ExecutionOrder::Sequential);
    h.terminal = Some(Step::Fault(Fault {
        code: String::new(),
        message: String::new(),
    }));
    finalize_memory(&mut h, &token(1)).unwrap();
    assert_eq!(*h.hooks.trace.borrow(), ["locate", "group"]);
    let mut h = Memory::new(ExecutionOrder::Parallel);
    h.missing_group = true;
    assert_eq!(
        finalize_memory(&mut h, &token(1)).unwrap_err().message,
        "located merge group disappeared"
    );
}
#[test]
fn memory_stop_is_lazy_and_preserves_acknowledged_and_missing_members() {
    let mut h = Memory::new(ExecutionOrder::Sequential);
    h.branches[0].step = Some(stop());
    h.branches[1].step = None;
    h.branches[1].ack = false;
    h.tokens.push(token(3));
    finalize_memory(&mut h, &token(1)).unwrap();
    assert_eq!(h.terminal, Some(stop()));
    assert_eq!(h.branches[0].expiry, Some(100));
    assert_eq!(h.branches[1].step, Some(stop()));
    assert!(h.branches[1].ack);
    assert_eq!(h.branches[1].expiry, None);
    assert_eq!(
        *h.hooks.trace.borrow(),
        [
            "locate",
            "group",
            "read",
            "branch_write",
            "branch_write",
            "branch_write",
            "group_write"
        ]
    );
}
#[test]
fn memory_pending_and_successor_paths_never_publish_early_and_missing_write_keeps_prefix() {
    let mut h = Memory::new(ExecutionOrder::Parallel);
    h.branches[0].step = None;
    finalize_memory(&mut h, &token(1)).unwrap();
    assert_eq!(*h.hooks.trace.borrow(), ["locate", "group", "read"]);
    let mut h = Memory::new(ExecutionOrder::GroupCapture);
    finalize_memory(&mut h, &token(1)).unwrap();
    assert!(h.terminal.is_none());
    assert_eq!(&h.hooks.trace.borrow()[6..], ["prefix", "successor"]);
    let mut h = Memory::new(ExecutionOrder::Sequential);
    h.branches[0].step = Some(stop());
    h.branches[1].step = None;
    h.branches[1].ack = false;
    h.missing_write = true;
    assert_eq!(
        finalize_memory(&mut h, &token(1)).unwrap_err().message,
        "sequence disappeared"
    );
    assert!(h.branches[1].ack);
    assert_eq!(h.branches[1].step, Some(stop()));
    let mut h = Memory::new(ExecutionOrder::Parallel);
    h.missing_write = true;
    assert_eq!(
        finalize_memory(&mut h, &token(1)).unwrap_err().message,
        "located merge group disappeared"
    );
}

struct Durable {
    hooks: Hooks,
    group: Option<StoredFinalizingGroup>,
    rows: Vec<StoredFinalizingBranch>,
    pending: Vec<String>,
    merged: Step,
    successor: Option<Step>,
    updated: usize,
    growth: usize,
    writes: Vec<String>,
}
impl Durable {
    fn new(order: ExecutionOrder) -> Self {
        Self {
            hooks: Hooks::default(),
            group: Some(StoredFinalizingGroup {
                token: "merge-10".into(),
                plan: serde_json::to_vec(&plan(order)).unwrap(),
                state: "pending".into(),
                terminal: None,
            }),
            rows: ["a", "b"]
                .map(|name| StoredFinalizingBranch {
                    name: name.into(),
                    state: "completed".into(),
                    terminal: Some(serde_json::to_vec(&done()).unwrap()),
                })
                .into(),
            pending: vec!["continuation-2".into()],
            merged: done(),
            successor: None,
            updated: 1,
            growth: 0,
            writes: vec![],
        }
    }
}
impl GroupFinalizationOwner for Durable {
    fn merge_prefix(&mut self, _: &MergePlan, _: Vec<BranchCompletion>) -> Result<Step, Fault> {
        self.hooks.call("prefix")?;
        Ok(done())
    }
    fn admit_successor(
        &mut self,
        _: &ContinuationToken,
        _: &MergePlan,
        _: &Step,
    ) -> Result<Option<Step>, Fault> {
        self.hooks.call("successor")?;
        Ok(self.successor.clone())
    }
    fn finish(&mut self, _: &MergePlan, _: Vec<BranchCompletion>) -> Result<Step, Fault> {
        self.hooks.call("finish")?;
        Ok(self.merged.clone())
    }
}
impl DurableGroupFinalization for Durable {
    fn locate(&mut self, _: &str) -> Result<Option<StoredFinalizingGroup>, Fault> {
        self.hooks.call("locate")?;
        Ok(self.group.take())
    }
    fn branches(&mut self, _: &str) -> Result<Vec<StoredFinalizingBranch>, Fault> {
        self.hooks.call("branches")?;
        Ok(std::mem::take(&mut self.rows))
    }
    fn encode_terminal(
        &mut self,
        step: &Step,
        kind: GroupTerminalEncoding,
    ) -> Result<Vec<u8>, Fault> {
        self.hooks.call(match kind {
            GroupTerminalEncoding::SequenceStop => "encode_stop",
            GroupTerminalEncoding::MergeResult => "encode_result",
        })?;
        Ok(serde_json::to_vec(step).unwrap())
    }
    fn pending_tokens(&mut self, _: &str) -> Result<Vec<String>, Fault> {
        self.hooks.call("pending")?;
        Ok(self.pending.clone())
    }
    fn ensure_growth(&mut self, n: usize) -> Result<(), Fault> {
        self.hooks.call("growth")?;
        self.growth = n;
        Ok(())
    }
    fn complete_branch(&mut self, t: &str, _: &[u8]) -> Result<(), Fault> {
        self.hooks.call("complete_branch")?;
        self.writes.push(t.into());
        Ok(())
    }
    fn stop_group(&mut self, _: &str, _: &[u8]) -> Result<(), Fault> {
        self.hooks.call("stop_group")?;
        self.writes.push("stop".into());
        Ok(())
    }
    fn complete_group(&mut self, _: &str, _: &[u8]) -> Result<usize, Fault> {
        self.hooks.call("complete_group")?;
        self.writes.push("complete".into());
        Ok(self.updated)
    }
}
#[test]
fn durable_replay_admits_terminal_but_never_decodes_plan_or_scans_branches() {
    for mode in 0..5 {
        let mut h = Durable::new(ExecutionOrder::Parallel);
        let g = h.group.as_mut().unwrap();
        g.plan = b"{".to_vec();
        g.state = "completed".into();
        g.terminal = match mode {
            0 => Some(serde_json::to_vec(&done()).unwrap()),
            1 => None,
            2 => Some(b"{".to_vec()),
            3 => Some(
                serde_json::to_vec(&Step::Fault(Fault {
                    code: "".into(),
                    message: "".into(),
                }))
                .unwrap(),
            ),
            _ => Some(vec![
                b' ';
                leselang_command::reference_recovery::MAX_JOURNAL_ENTRY_BYTES
                    + 1
            ]),
        };
        assert_eq!(finalize_durable(&mut h, "branch").is_ok(), mode == 0);
        assert_eq!(*h.hooks.trace.borrow(), ["locate"]);
    }
}
#[test]
fn durable_read_and_decode_fault_order_retains_pending_mask_and_eager_sequence_bytes() {
    let mut h = Durable::new(ExecutionOrder::Parallel);
    h.group.as_mut().unwrap().plan = b"{".to_vec();
    h.hooks.fail = Some(1);
    assert_eq!(
        finalize_durable(&mut h, "branch").unwrap_err().code,
        "OWNER"
    );
    let mut h = Durable::new(ExecutionOrder::Parallel);
    h.rows[0].state = "invalid".into();
    h.rows[0].terminal = Some(b"{".to_vec());
    h.rows[1].state = "pending".into();
    finalize_durable(&mut h, "branch").unwrap();
    assert_eq!(*h.hooks.trace.borrow(), ["locate", "branches"]);
    let mut h = Durable::new(ExecutionOrder::Sequential);
    h.rows[0].terminal = Some(serde_json::to_vec(&stop()).unwrap());
    h.rows[1].terminal = Some(b"{".to_vec());
    assert!(finalize_durable(&mut h, "branch").is_err());
    assert_eq!(*h.hooks.trace.borrow(), ["locate", "branches"]);
    let mut h = Durable::new(ExecutionOrder::Parallel);
    h.rows[1].terminal = None;
    assert_eq!(
        finalize_durable(&mut h, "branch").unwrap_err().message,
        "merge branch has an invalid durable state"
    );
}
#[test]
fn durable_stop_budgets_before_writes_and_completion_requires_one_pending_owner() {
    let mut h = Durable::new(ExecutionOrder::Sequential);
    h.rows[0].terminal = Some(serde_json::to_vec(&stop()).unwrap());
    finalize_durable(&mut h, "branch").unwrap();
    assert_eq!(
        *h.hooks.trace.borrow(),
        [
            "locate",
            "branches",
            "encode_stop",
            "pending",
            "growth",
            "complete_branch",
            "stop_group"
        ]
    );
    assert_eq!(h.growth, serde_json::to_vec(&stop()).unwrap().len() * 2);
    for count in [0, 1, 2] {
        let mut h = Durable::new(ExecutionOrder::Parallel);
        h.updated = count;
        assert_eq!(finalize_durable(&mut h, "branch").is_ok(), count == 1);
        assert_eq!(
            *h.hooks.trace.borrow(),
            [
                "locate",
                "branches",
                "finish",
                "encode_result",
                "growth",
                "complete_group"
            ]
        );
    }
    let mut h = Durable::new(ExecutionOrder::Parallel);
    h.merged = Step::Fault(Fault {
        code: "".into(),
        message: "".into(),
    });
    assert!(finalize_durable(&mut h, "branch").is_err());
    assert_eq!(*h.hooks.trace.borrow(), ["locate", "branches", "finish"]);
}
#[test]
fn successor_admission_can_defer_or_return_a_terminal_without_ordinary_finish() {
    for terminal in [None, Some(done()), Some(stop())] {
        let mut h = Durable::new(ExecutionOrder::GroupCapture);
        h.successor = terminal.clone();
        finalize_durable(&mut h, "branch").unwrap();
        let trace = h.hooks.trace.borrow();
        assert!(!trace.contains(&"finish"));
        assert_eq!(trace.contains(&"complete_group"), terminal.is_some());
        let mut m = Memory::new(ExecutionOrder::GroupCapture);
        m.successor = terminal.clone();
        finalize_memory(&mut m, &token(1)).unwrap();
        assert_eq!(m.terminal, terminal);
    }
}
#[test]
fn every_durable_hook_error_or_unwind_stops_without_retry_or_later_publication() {
    for route in 0..3 {
        let build = || {
            let mut h = Durable::new(if route == 0 {
                ExecutionOrder::Parallel
            } else if route == 1 {
                ExecutionOrder::Sequential
            } else {
                ExecutionOrder::GroupCapture
            });
            if route == 1 {
                h.rows[0].terminal = Some(serde_json::to_vec(&stop()).unwrap());
            }
            if route == 2 {
                h.successor = Some(done());
            }
            h
        };
        let mut baseline = build();
        finalize_durable(&mut baseline, "branch").unwrap();
        let trace = baseline.hooks.trace.into_inner();
        for fail in 0..trace.len() {
            for unwind in [false, true] {
                let mut h = build();
                h.hooks.fail = Some(fail);
                h.hooks.unwind = unwind;
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    finalize_durable(&mut h, "branch")
                }));
                if unwind {
                    assert!(result.is_err());
                } else {
                    assert_eq!(
                        result.unwrap().unwrap_err(),
                        Fault {
                            code: "OWNER".into(),
                            message: trace[fail].into()
                        }
                    );
                }
                assert_eq!(*h.hooks.trace.borrow(), trace[..=fail]);
            }
        }
    }
}
