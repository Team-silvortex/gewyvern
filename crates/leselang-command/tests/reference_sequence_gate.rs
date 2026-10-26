use leselang_command::reference_sequence_gate::*;
use leselang_command::{ContinuationToken, ExecutionOrder, Step, Value};
use leselang_runtime_core::Fault;
use std::{cell::RefCell, collections::BTreeMap};

fn token(n: usize) -> ContinuationToken {
    ContinuationToken::from_wire(format!("continuation-{n}"))
}
fn done() -> Step {
    Step::Done(Value::UiFocus {
        node_id: String::new(),
    })
}
struct Group {
    order: ExecutionOrder,
    branches: Vec<ContinuationToken>,
    terminal: bool,
}
fn group(order: ExecutionOrder, members: &[usize], terminal: bool) -> Group {
    Group {
        order,
        branches: members.iter().map(|n| token(*n)).collect(),
        terminal,
    }
}
#[derive(Default)]
struct Memory {
    groups: Vec<Group>,
    terminals: BTreeMap<ContinuationToken, Step>,
    trace: RefCell<Vec<String>>,
    panic_at: Option<usize>,
}
impl Memory {
    fn call(&self, name: String) {
        let n = self.trace.borrow().len();
        self.trace.borrow_mut().push(name);
        assert_ne!(self.panic_at, Some(n), "original owner unwind");
    }
}
impl MemorySequenceJournal for Memory {
    fn groups(&self) -> impl Iterator<Item = MemorySequenceGroup<'_>> {
        self.call("groups".into());
        self.groups.iter().enumerate().map(|(n, g)| {
            self.call(format!("group-{n}"));
            MemorySequenceGroup {
                order: g.order,
                branches: &g.branches,
                terminal: g.terminal,
            }
        })
    }
    fn terminal(&self, t: &ContinuationToken) -> Option<&Step> {
        self.call(t.as_str().into());
        self.terminals.get(t)
    }
}
#[test]
fn empty_parallel_and_unrelated_groups_do_not_read_dispatches() {
    let mut h = Memory::default();
    assert!(ready_memory(&h, &token(2)));
    h.groups = vec![
        group(ExecutionOrder::Parallel, &[1, 2], true),
        group(ExecutionOrder::Sequential, &[3, 4], true),
    ];
    h.trace.borrow_mut().clear();
    assert!(ready_memory(&h, &token(2)));
    assert_eq!(*h.trace.borrow(), ["groups", "group-0", "group-1"]);
}
#[test]
fn first_occurrence_and_first_member_have_an_empty_prefix_but_closed_owner_refuses() {
    let mut h = Memory {
        groups: vec![group(ExecutionOrder::Sequential, &[2, 1, 2], false)],
        ..Memory::default()
    };
    assert!(ready_memory(&h, &token(2)));
    assert_eq!(*h.trace.borrow(), ["groups", "group-0"]);
    h.groups[0].terminal = true;
    h.trace.borrow_mut().clear();
    assert!(!ready_memory(&h, &token(2)));
    assert_eq!(*h.trace.borrow(), ["groups", "group-0"]);
}
#[test]
fn predecessor_and_group_scans_stop_at_first_refusal_without_late_reads() {
    let mut h = Memory {
        groups: vec![
            group(ExecutionOrder::Sequential, &[1, 2, 3], false),
            group(ExecutionOrder::Sequential, &[4, 3], false),
        ],
        ..Memory::default()
    };
    assert!(!ready_memory(&h, &token(3)));
    assert_eq!(*h.trace.borrow(), ["groups", "group-0", "continuation-1"]);
    h.terminals.insert(token(1), done());
    h.trace.borrow_mut().clear();
    assert!(!ready_memory(&h, &token(3)));
    assert_eq!(
        *h.trace.borrow(),
        ["groups", "group-0", "continuation-1", "continuation-2"]
    );
    h.terminals.insert(token(2), done());
    h.trace.borrow_mut().clear();
    assert!(!ready_memory(&h, &token(3)));
    assert_eq!(
        *h.trace.borrow(),
        [
            "groups",
            "group-0",
            "continuation-1",
            "continuation-2",
            "group-1",
            "continuation-4"
        ]
    );
}
#[test]
fn done_classification_does_not_validate_payload_or_read_current_and_later_members() {
    let mut h = Memory {
        groups: vec![group(ExecutionOrder::Sequential, &[1, 2, 3], false)],
        ..Memory::default()
    };
    h.terminals.insert(token(1), done());
    assert!(ready_memory(&h, &token(2)));
    assert_eq!(*h.trace.borrow(), ["groups", "group-0", "continuation-1"]);
    h.terminals.insert(
        token(1),
        Step::Fault(Fault {
            code: "HOST".into(),
            message: "failed".into(),
        }),
    );
    assert!(!ready_memory(&h, &token(2)));
}
#[test]
fn native_memory_unwinds_escape_without_retry_or_continuing_the_scan() {
    for stop in 0..3 {
        let h = Memory {
            groups: vec![group(ExecutionOrder::Sequential, &[1, 2], false)],
            panic_at: Some(stop),
            ..Memory::default()
        };
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ready_memory(&h, &token(2))))
                .is_err()
        );
        assert_eq!(h.trace.borrow().len(), stop + 1);
    }
}
struct Durable {
    blocked: bool,
    calls: usize,
    error: Option<Fault>,
    unwind: bool,
}
impl DurableSequenceJournal for Durable {
    fn has_unfinished_predecessor(&mut self, token: &str) -> Result<bool, Fault> {
        assert_eq!(token, "unchanged-native-token");
        self.calls += 1;
        assert!(!self.unwind, "native query unwind");
        match self.error.take() {
            Some(e) => Err(e),
            None => Ok(self.blocked),
        }
    }
}
#[test]
fn durable_observation_is_read_once_with_the_original_refusal_code() {
    for blocked in [false, true] {
        let mut h = Durable {
            blocked,
            calls: 0,
            error: None,
            unwind: false,
        };
        let result = ensure_durable(&mut h, "unchanged-native-token");
        assert_eq!(
            result,
            if blocked {
                Err(Fault {
                    code: "LSV2410".into(),
                    message: "sequential predecessor has not completed".into(),
                })
            } else {
                Ok(())
            }
        );
        assert_eq!(h.calls, 1);
    }
}
#[test]
fn durable_errors_move_unchanged_and_unwinds_do_not_retry() {
    let error = Fault {
        code: "NATIVE".into(),
        message: "native query failure".into(),
    };
    let ptr = error.message.as_ptr();
    let mut h = Durable {
        blocked: true,
        calls: 0,
        error: Some(error),
        unwind: false,
    };
    let e = ensure_durable(&mut h, "unchanged-native-token").unwrap_err();
    assert_eq!(e.code, "NATIVE");
    assert_eq!(e.message.as_ptr(), ptr);
    assert_eq!(h.calls, 1);
    h.unwind = true;
    h.calls = 0;
    assert!(
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ensure_durable(
            &mut h,
            "unchanged-native-token"
        )))
        .is_err()
    );
    assert_eq!(h.calls, 1);
}
