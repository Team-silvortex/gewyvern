use leselang_command::reference_compaction_commit::*;
use leselang_command::reference_settlement::maintenance::JournalCompaction;
use leselang_command::{
    ContinuationToken, ExecutionOrder, MergePlan, RetentionPolicy, Step, Value,
};
use leselang_runtime_core::Fault;
use std::{cell::RefCell, collections::BTreeMap, rc::Rc};

fn token(n: u64) -> ContinuationToken {
    ContinuationToken::from_wire(format!("continuation-{n}"))
}
fn done() -> Step {
    Step::Done(Value::UiFocus {
        node_id: "node".into(),
    })
}
fn policy(keep: usize, batch: usize) -> RetentionPolicy {
    RetentionPolicy {
        max_completed_records: keep,
        max_delete_per_run: batch,
    }
}
fn error() -> Fault {
    Fault {
        code: "NATIVE".into(),
        message: "original native error".into(),
    }
}
fn report(r: JournalCompaction) -> (usize, usize, usize, Vec<ContinuationToken>) {
    (
        r.removed_records,
        r.remaining_completed,
        r.reclaimed_logical_bytes,
        r.pruned_tokens,
    )
}
struct Group {
    plan: MergePlan,
    members: Vec<ContinuationToken>,
    terminal: Option<Step>,
}
fn view(g: &Group) -> MemoryCompactionGroup<'_> {
    MemoryCompactionGroup {
        plan: &g.plan,
        branches: &g.members,
        terminal: g.terminal.as_ref(),
    }
}
#[derive(Default)]
struct Memory {
    groups: BTreeMap<ContinuationToken, Group>,
    trace: RefCell<Vec<String>>,
    measurements: RefCell<usize>,
    fail_measure: Option<usize>,
    panic_remove: bool,
}
impl MemoryCompactionJournal for Memory {
    fn groups(&self) -> impl Iterator<Item = (&ContinuationToken, MemoryCompactionGroup<'_>)> {
        self.groups.iter().map(|(t, g)| (t, view(g)))
    }
    fn group(&self, t: &ContinuationToken) -> Option<MemoryCompactionGroup<'_>> {
        self.groups.get(t).map(view)
    }
    fn dispatch(&self, _: &ContinuationToken) -> Option<MemoryCompactionDispatch<'_>> {
        None
    }
    fn measure<T: serde::Serialize>(&self, _: &T, _: usize, label: &str) -> Result<usize, Fault> {
        let mut count = self.measurements.borrow_mut();
        *count += 1;
        self.trace.borrow_mut().push(label.into());
        if self.fail_measure == Some(*count) {
            Err(error())
        } else {
            Ok(usize::MAX)
        }
    }
    fn remove_group(&mut self, t: &ContinuationToken) {
        self.trace
            .borrow_mut()
            .push(format!("group:{}", t.as_str()));
        self.groups.remove(t);
    }
    fn remove_dispatch(&mut self, t: &ContinuationToken) {
        self.trace
            .borrow_mut()
            .push(format!("effect:{}", t.as_str()));
        assert!(!self.panic_remove, "native mutation unwind");
    }
    fn retain_live_audits(&mut self) {
        self.trace.borrow_mut().push("audits".into());
    }
}
fn memory() -> Memory {
    let mut h = Memory::default();
    for (id, members, closed) in [(50, vec![3, 4], true), (60, vec![5, 6], false)] {
        h.groups.insert(
            token(id),
            Group {
                plan: MergePlan {
                    order: ExecutionOrder::Parallel,
                    branches: vec!["a".into(), "b".into()],
                    result_binding: None,
                },
                members: members.into_iter().map(token).collect(),
                terminal: closed.then(done),
            },
        );
    }
    h
}
fn local() -> Vec<(ContinuationToken, Step)> {
    [10, 2, 3, 4, 5].map(|n| (token(n), done())).to_vec()
}

#[test]
fn memory_protects_live_members_sorts_numeric_age_and_preflights_before_batch_cascade() {
    let mut h = memory();
    let r = compact_memory(&mut h, &local(), &policy(0, 3)).unwrap();
    assert_eq!(
        report(r),
        (
            3,
            0,
            usize::MAX,
            vec![token(2), token(3), token(4), token(10)]
        )
    );
    assert_eq!(
        *h.trace.borrow(),
        [
            "completed step",
            "merge plan",
            "merge result",
            "completed step",
            "completed step",
            "completed step",
            "group:continuation-50",
            "effect:continuation-2",
            "effect:continuation-3",
            "effect:continuation-4",
            "effect:continuation-10",
            "audits"
        ]
    );
    assert!(h.groups.contains_key(&token(60)));
}
#[test]
fn every_memory_measurement_failure_precedes_all_deletions() {
    for stop in 1..=6 {
        let mut h = memory();
        h.fail_measure = Some(stop);
        assert_eq!(
            compact_memory(&mut h, &local(), &policy(0, 3)).err(),
            Some(error())
        );
        assert_eq!(h.trace.borrow().len(), stop);
        assert!(
            !h.trace
                .borrow()
                .iter()
                .any(|s| s.starts_with("group:") || s.starts_with("effect:") || s == "audits")
        );
        assert_eq!(h.groups.len(), 2);
    }
}
#[test]
fn memory_zero_batch_skips_measurement_but_keeps_empty_completed_group_refusal() {
    let mut h = memory();
    assert_eq!(
        report(compact_memory(&mut h, &local(), &policy(0, 0)).unwrap()),
        (0, 3, 0, vec![])
    );
    assert!(h.trace.borrow().is_empty());
    h.groups.get_mut(&token(50)).unwrap().members.clear();
    let e = compact_memory(&mut h, &[], &policy(100, 0)).err().unwrap();
    assert_eq!(e.code, "LSV4007");
    assert_eq!(e.message, "completed merge group has no branches");
}
#[test]
fn memory_duplicate_local_units_are_not_deduplicated_and_unwinds_do_not_retry() {
    let entries = vec![(token(1), done()), (token(1), done())];
    let mut h = Memory::default();
    assert_eq!(
        report(compact_memory(&mut h, &entries, &policy(0, 2)).unwrap()),
        (2, 0, usize::MAX, vec![token(1), token(1)])
    );
    assert_eq!(
        *h.trace.borrow(),
        [
            "completed step",
            "completed step",
            "effect:continuation-1",
            "effect:continuation-1",
            "audits"
        ]
    );
    let mut h = Memory {
        panic_remove: true,
        ..Memory::default()
    };
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| compact_memory(
            &mut h,
            &entries,
            &policy(0, 2)
        )))
        .is_err()
    );
    assert_eq!(
        *h.trace.borrow(),
        ["completed step", "completed step", "effect:continuation-1"]
    );
}

struct Durable {
    trace: Rc<RefCell<Vec<String>>>,
    count: i64,
    rows: Vec<(i64, String)>,
    deleted: usize,
    fail: Option<usize>,
    panic_at: Option<usize>,
    original: Option<Fault>,
}
impl Durable {
    fn new(trace: Rc<RefCell<Vec<String>>>) -> Self {
        Self {
            trace,
            count: 3,
            rows: vec![(0, "a".into()), (1, "g".into())],
            deleted: 1,
            fail: None,
            panic_at: None,
            original: Some(error()),
        }
    }
    fn call(&mut self, label: String) -> Result<(), Fault> {
        let n = self.trace.borrow().len();
        self.trace.borrow_mut().push(label);
        assert_ne!(self.panic_at, Some(n), "native transaction unwind");
        if self.fail == Some(n) {
            Err(self.original.take().unwrap())
        } else {
            Ok(())
        }
    }
}
impl Drop for Durable {
    fn drop(&mut self) {
        self.trace.borrow_mut().push("drop".into());
    }
}
impl DurableCompactionTransaction for Durable {
    fn completed_count(&mut self) -> Result<i64, Fault> {
        self.call("count".into())?;
        Ok(self.count)
    }
    fn candidates(&mut self, n: i64) -> Result<Vec<(i64, String)>, Fault> {
        self.call(format!("scan:{n}"))?;
        Ok(std::mem::take(&mut self.rows))
    }
    fn effect_bytes(&mut self, t: &str) -> Result<usize, Fault> {
        self.call(format!("bytes:{t}"))?;
        Ok(usize::MAX)
    }
    fn delete_effect(&mut self, t: &str) -> Result<usize, Fault> {
        self.call(format!("effect:{t}"))?;
        Ok(self.deleted)
    }
    fn delete_group(&mut self, t: &str) -> Result<usize, Fault> {
        self.call(format!("group:{t}"))?;
        Ok(1)
    }
    fn stale_local(
        &mut self,
        local: &[(ContinuationToken, Step)],
    ) -> Result<Vec<ContinuationToken>, Fault> {
        self.call(format!("stale:{}", local.len()))?;
        Ok(local.iter().map(|(t, _)| t.clone()).collect())
    }
    fn commit(mut self, no_removal: bool) -> Result<(), Fault> {
        self.call(format!("commit:{no_removal}"))
    }
}
#[test]
fn durable_batch_preserves_eager_scan_order_saturation_and_commit_before_result() {
    let trace = Rc::default();
    let r = compact_durable(Durable::new(Rc::clone(&trace)), &local(), &policy(1, 5)).unwrap();
    assert_eq!(
        report(r),
        (
            2,
            1,
            usize::MAX,
            local().into_iter().map(|(t, _)| t).collect()
        )
    );
    assert_eq!(
        *trace.borrow(),
        [
            "count",
            "scan:2",
            "bytes:a",
            "effect:a",
            "group:g",
            "stale:5",
            "commit:false",
            "drop"
        ]
    );
}
#[test]
fn durable_no_removal_reconciles_stale_cache_and_uses_close_diagnostic_path() {
    let trace = Rc::default();
    let r = compact_durable(Durable::new(Rc::clone(&trace)), &local(), &policy(3, 5)).unwrap();
    assert_eq!(r.removed_records, 0);
    assert_eq!(r.pruned_tokens.len(), 5);
    assert_eq!(*trace.borrow(), ["count", "stale:5", "commit:true", "drop"]);
}
#[test]
fn durable_native_failures_move_unchanged_and_unwinds_drop_without_retry_or_commit() {
    for no_op in [false, true] {
        let calls = if no_op { 3 } else { 7 };
        for stop in 0..calls {
            let trace = Rc::default();
            let mut h = Durable::new(Rc::clone(&trace));
            let pointer = h.original.as_ref().unwrap().message.as_ptr();
            h.fail = Some(stop);
            let e = compact_durable(h, &[], &policy(if no_op { 3 } else { 1 }, 5))
                .err()
                .unwrap();
            assert_eq!(e, error());
            assert_eq!(e.message.as_ptr(), pointer);
            assert_eq!(trace.borrow().len(), stop + 2);
            assert_eq!(trace.borrow().last().unwrap(), "drop");
            let trace = Rc::default();
            let mut h = Durable::new(Rc::clone(&trace));
            h.panic_at = Some(stop);
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| compact_durable(
                    h,
                    &[],
                    &policy(if no_op { 3 } else { 1 }, 5)
                )))
                .is_err()
            );
            assert_eq!(trace.borrow().len(), stop + 2);
            assert_eq!(trace.borrow().last().unwrap(), "drop");
        }
    }
}
#[test]
fn durable_invalid_count_kind_and_delete_cardinality_refuse_before_later_hooks() {
    for kind in 0..3 {
        let trace = Rc::default();
        let mut h = Durable::new(Rc::clone(&trace));
        match kind {
            0 => h.count = -1,
            1 => h.rows = vec![(9, "bad".into())],
            _ => h.deleted = 0,
        }
        let e = compact_durable(h, &[], &policy(0, 5)).err().unwrap();
        assert_eq!(e.code, if kind == 2 { "LSV4030" } else { "LSV4007" });
        assert!(
            !trace
                .borrow()
                .iter()
                .any(|s| s.starts_with("stale:") || s.starts_with("commit:"))
        );
    }
}
#[test]
fn durable_empty_selected_batch_still_uses_mutating_commit_and_actual_removed_count() {
    let trace = Rc::default();
    let mut h = Durable::new(Rc::clone(&trace));
    h.rows.clear();
    assert_eq!(
        report(compact_durable(h, &[], &policy(0, 1)).unwrap()),
        (0, 3, 0, vec![])
    );
    assert_eq!(
        *trace.borrow(),
        ["count", "scan:1", "stale:0", "commit:false", "drop"]
    );
}
