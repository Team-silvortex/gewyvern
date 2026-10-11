use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_hir::{
    group_reply::{GroupReply, GroupReplyError, assemble_group_reply},
    ir::{Computation, ComputedBranch, GroupKind},
    parallel_evaluation::*,
    sequence_evaluation::{SequenceEvaluation, SequencePoll},
};
use leselang_runtime_core::{Fuel, HostResultDomain, ReplyAuthority};

struct Private;
struct Domain(Cell<u64>);
struct Input {
    value: u64,
    observed: Cell<u64>,
    buffer: Box<[u8]>,
    drops: Rc<Cell<usize>>,
}
impl std::borrow::Borrow<u64> for Input {
    fn borrow(&self) -> &u64 {
        &self.value
    }
}
impl Drop for Input {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
impl HostResultDomain<u64> for Domain {
    type Error = Private;
    fn matches_type(&self, _: &u64) -> bool {
        true
    }
    fn validate_value(&self, value: &u64) -> Result<(), Private> {
        if *value <= self.0.get() {
            Ok(())
        } else {
            Err(Private)
        }
    }
}
struct Authority(Cell<bool>);
impl ReplyAuthority<(u64, usize)> for Authority {
    type Error = Private;
    fn authorize(&self, &(owner, _): &(u64, usize)) -> Result<(), Private> {
        if self.0.get() && owner == 7 {
            Ok(())
        } else {
            Err(Private)
        }
    }
}
type Node = Computation<(), usize, (), Domain>;
type Branch = ComputedBranch<Node, Domain>;
type Accepted<'a> = ParallelAccepted<'a, Node, Domain, (u64, usize), Domain, Input>;
type Session<'a> = ParallelEvaluation<'a, (), usize, (), Domain, (u64, usize), Domain, Input>;
type Reply<'a> = GroupReply<'a, Node, Domain, (u64, usize), Domain, Input>;
const LIMITS: ParallelEvaluationLimits = ParallelEvaluationLimits {
    max_nodes: 256,
    max_depth: 32,
    max_branches: 64,
};
struct Host {
    owner: u64,
    prepared: RefCell<Vec<usize>>,
}
impl<'a> ParallelEvaluationEnvironment<'a, (), usize, (), Domain> for Host {
    type Identity = (u64, usize);
    type Declaration = Domain;
    type Request = usize;
    type Error = Private;
    fn preflight_member(&self, _: usize, _: &'a Branch) -> Result<(), Private> {
        Ok(())
    }
    fn prepare_member(
        &self,
        index: usize,
        row: &'a Branch,
        _: &mut Fuel,
    ) -> ParallelPreparation<'a, (u64, usize), Domain, usize, Private> {
        self.prepared.borrow_mut().push(index);
        Ok(PreparedParallelMember {
            identity: (self.owner, index),
            declaration: &row.result_type,
            request: index,
        })
    }
}
fn group(kind: GroupKind) -> Node {
    Node::Group {
        group_kind: kind,
        branches: (0..2)
            .map(|index| Branch {
                name: if index == 0 { "left" } else { "right" }.into(),
                value: Node::Call {
                    operation: index,
                    arguments: Vec::new(),
                },
                result_type: Domain(Cell::new(100)),
            })
            .collect(),
    }
}
fn host() -> Host {
    Host {
        owner: 7,
        prepared: RefCell::new(Vec::new()),
    }
}
fn input(index: usize, drops: &Rc<Cell<usize>>) -> Input {
    Input {
        value: 40 + index as u64,
        observed: Cell::new(40 + index as u64),
        buffer: vec![index as u8; 19].into_boxed_slice(),
        drops: drops.clone(),
    }
}
fn complete<'a>(node: &'a Node, host: &Host, drops: &Rc<Cell<usize>>) -> (Vec<Accepted<'a>>, Fuel) {
    let authority = Authority(Cell::new(true));
    let mut session = Session::start(node, host, Fuel::new(100), LIMITS).unwrap();
    let ParallelPoll::Requests(batch) = session.poll(host).unwrap() else {
        panic!()
    };
    for request in batch.into_iter().rev() {
        session
            .try_accept::<u64, _>(
                request.index,
                &(host.owner, request.index),
                input(request.index, drops),
                &authority,
            )
            .unwrap();
    }
    let ParallelPoll::Results(rows) = session.poll(host).unwrap() else {
        panic!()
    };
    (rows, session.into_completed_fuel().unwrap())
}
fn admit(reply: &Reply<'_>, host: &Host, authority: &Authority) -> Result<(), Private> {
    let Node::Group { branches, .. } = reply.group() else {
        return Err(Private);
    };
    for (index, row) in reply.members().iter().enumerate() {
        let (_, _, key, declaration, input) = row.as_parts();
        authority.authorize(key)?;
        if *key != (host.owner, index) || !std::ptr::eq(declaration, &branches[index].result_type) {
            return Err(Private);
        }
        declaration.validate_value(&input.observed.get())?;
    }
    Ok(())
}

#[test]
fn complete_assembly_preserves_move_only_input_original_rows_and_borrowed_lookup() {
    let node = group(GroupKind::Parallel);
    let host = host();
    let drops = Rc::new(Cell::new(0));
    let (rows, mut fuel) = complete(&node, &host, &drops);
    let pointers: Vec<_> = rows
        .iter()
        .map(|row| row.as_parts().4.buffer.as_ptr())
        .collect();
    assert_eq!(fuel.remaining(), 97);
    let authority = Authority(Cell::new(true));
    let reply = assemble_group_reply(&node, rows, LIMITS, &mut fuel, |reply, meter| {
        admit(reply, &host, &authority)?;
        meter.charge(2).map_err(|_| Private)
    })
    .unwrap();
    assert_eq!(fuel.remaining(), 92);
    assert!(std::ptr::eq(reply.group(), &node));
    assert_eq!(reply.kind(), GroupKind::Parallel);
    let right = reply
        .lookup(64, "right", |row| {
            authority.authorize(row.as_parts().2)?;
            Ok::<_, Private>(true)
        })
        .unwrap()
        .unwrap();
    assert!(std::ptr::eq(right, &reply.members()[1]));
    assert_eq!(right.as_parts().4.buffer.as_ptr(), pointers[1]);
    assert_eq!(
        format!("{reply:?}"),
        "GroupReply { kind: Parallel, members: 2 }"
    );
    assert_eq!(drops.get(), 0);
    drop(reply);
    assert_eq!(drops.get(), 2);
}

#[test]
fn partial_reordered_foreign_and_duplicate_rows_are_cold_rejections() {
    let node = group(GroupKind::Parallel);
    let foreign = group(GroupKind::Parallel);
    let host = host();
    for case in 0..4 {
        let drops = Rc::new(Cell::new(0));
        let (mut rows, mut fuel) = complete(&node, &host, &drops);
        match case {
            0 => {
                rows.pop();
            }
            1 => rows.swap(0, 1),
            2 => {}
            _ => {
                let (mut other, _) = complete(&node, &host, &drops);
                rows[1] = other.remove(0);
            }
        }
        let before = fuel.remaining();
        let calls = Cell::new(0);
        let result = assemble_group_reply(
            if case == 2 { &foreign } else { &node },
            rows,
            LIMITS,
            &mut fuel,
            |_, _| {
                calls.set(calls.get() + 1);
                Ok::<_, Private>(())
            },
        );
        assert!(matches!(
            result,
            Err(GroupReplyError::Count | GroupReplyError::Row { .. })
        ));
        assert_eq!(fuel.remaining(), before);
        assert_eq!(calls.get(), 0);
        assert_eq!(drops.get(), if case == 3 { 4 } else { 2 });
    }
}

#[test]
fn mixed_generation_changed_values_and_revoked_authority_require_fresh_whole_admission() {
    let node = group(GroupKind::Parallel);
    let host = host();
    for case in 0..4 {
        let drops = Rc::new(Cell::new(0));
        let (mut rows, mut fuel) = complete(&node, &host, &drops);
        let authority = Authority(Cell::new(case != 3));
        if case == 0 {
            let other = Host {
                owner: 8,
                prepared: RefCell::new(Vec::new()),
            };
            // Initial acceptance is allowed by that execution's native authority.
            struct Other;
            impl ReplyAuthority<(u64, usize)> for Other {
                type Error = Private;
                fn authorize(&self, _: &(u64, usize)) -> Result<(), Private> {
                    Ok(())
                }
            }
            let mut session = Session::start(&node, &other, Fuel::new(100), LIMITS).unwrap();
            let ParallelPoll::Requests(batch) = session.poll(&other).unwrap() else {
                panic!()
            };
            for item in batch {
                session
                    .try_accept::<u64, _>(
                        item.index,
                        &(8, item.index),
                        input(item.index, &drops),
                        &Other,
                    )
                    .unwrap();
            }
            let ParallelPoll::Results(mut foreign) = session.poll(&other).unwrap() else {
                panic!()
            };
            rows[1] = foreign.remove(1);
        } else if case == 1 {
            rows[1].as_parts().4.observed.set(101);
        } else if case == 2 {
            let Node::Group { branches, .. } = &node else {
                panic!()
            };
            branches[1].result_type.0.set(0);
        }
        let calls = Cell::new(0);
        let result = assemble_group_reply(&node, rows, LIMITS, &mut fuel, |reply, _| {
            calls.set(calls.get() + 1);
            admit(reply, &host, &authority)
        });
        assert!(matches!(result, Err(GroupReplyError::Native(_))));
        assert_eq!(calls.get(), 1);
        assert_eq!(fuel.remaining(), 94);
        assert_eq!(drops.get(), if case == 0 { 4 } else { 2 });
        let Node::Group { branches, .. } = &node else {
            panic!()
        };
        branches[1].result_type.0.set(100);
    }
}

#[test]
fn lookup_is_closed_and_requires_current_selected_policy_without_copy_or_fallback() {
    let node = group(GroupKind::Parallel);
    let host = host();
    let drops = Rc::new(Cell::new(0));
    let (rows, mut fuel) = complete(&node, &host, &drops);
    let authority = Authority(Cell::new(true));
    let reply = assemble_group_reply(&node, rows, LIMITS, &mut fuel, |r, _| {
        admit(r, &host, &authority)
    })
    .unwrap();
    let calls = Cell::new(0);
    for name in ["missing", "invalid.name"] {
        let result = reply.lookup(64, name, |_| {
            calls.set(calls.get() + 1);
            Ok::<_, Private>(true)
        });
        assert!(matches!(result, Ok(None) | Err(_)));
    }
    assert_eq!(calls.get(), 0);
    assert!(reply.lookup(1, "left", |_| Ok::<_, Private>(true)).is_err());
    assert!(
        reply
            .lookup(64, "left", |_| Ok::<_, Private>(false))
            .unwrap()
            .is_none()
    );
    authority.0.set(false);
    assert!(
        reply
            .lookup(64, "left", |row| {
                authority.authorize(row.as_parts().2)?;
                Ok::<_, Private>(true)
            })
            .is_err()
    );
    assert_eq!(drops.get(), 0);
    drop(reply.into_members());
    assert_eq!(drops.get(), 2);
}

#[test]
fn native_failure_and_unwind_consume_inputs_once_without_refund_or_partial_output() {
    let node = group(GroupKind::Parallel);
    let host = host();
    for unwind in [false, true] {
        let drops = Rc::new(Cell::new(0));
        let (rows, mut fuel) = complete(&node, &host, &drops);
        let result = catch_unwind(AssertUnwindSafe(|| {
            assemble_group_reply(&node, rows, LIMITS, &mut fuel, |_, meter| {
                meter.charge(2).map_err(|_| Private)?;
                assert!(!unwind, "private whole-batch admission unwind");
                Err(Private)
            })
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            let error = result.unwrap().unwrap_err();
            assert_eq!(
                format!("{error:?}"),
                "group reply fresh whole-batch native admission failed"
            );
            assert!(std::error::Error::source(&error).is_none());
        }
        assert_eq!(fuel.remaining(), 92);
        assert_eq!(drops.get(), 2);
    }
}

#[test]
fn assembly_limits_shape_and_fuel_fail_before_native_admission() {
    let node = group(GroupKind::Parallel);
    let host = host();
    for case in 0..4 {
        let drops = Rc::new(Cell::new(0));
        let (rows, original) = complete(&node, &host, &drops);
        let mut fuel = if case == 3 { Fuel::new(2) } else { original };
        let scalar = Node::Literal {
            value: leselang_runtime_core::ScalarValue::Integer(0),
        };
        let limits = match case {
            0 => ParallelEvaluationLimits {
                max_nodes: 16_385,
                ..LIMITS
            },
            1 => ParallelEvaluationLimits {
                max_nodes: 0,
                ..LIMITS
            },
            _ => LIMITS,
        };
        let calls = Cell::new(0);
        let before = fuel.remaining();
        let result = assemble_group_reply(
            if case == 2 { &scalar } else { &node },
            rows,
            limits,
            &mut fuel,
            |_, _| {
                calls.set(calls.get() + 1);
                Ok::<_, Private>(())
            },
        );
        assert!(result.is_err());
        assert_eq!(calls.get(), 0);
        assert_eq!(fuel.remaining(), before);
        assert_eq!(drops.get(), 2);
    }
}

#[test]
fn original_parallel_fuel_moves_only_after_complete_result_handoff() {
    let node = group(GroupKind::Parallel);
    let host = host();
    let drops = Rc::new(Cell::new(0));
    let authority = Authority(Cell::new(true));
    let session = Session::start(&node, &host, Fuel::new(100), LIMITS).unwrap();
    let mut session = session.into_completed_fuel().unwrap_err();
    let ParallelPoll::Requests(batch) = session.poll(&host).unwrap() else {
        panic!()
    };
    let mut session = session.into_completed_fuel().unwrap_err();
    for item in batch {
        session
            .try_accept::<u64, _>(
                item.index,
                &(7, item.index),
                input(item.index, &drops),
                &authority,
            )
            .unwrap();
    }
    assert_eq!(session.status(), ParallelStatus::ReadyResults);
    let mut session = session.into_completed_fuel().unwrap_err();
    let ParallelPoll::Results(rows) = session.poll(&host).unwrap() else {
        panic!()
    };
    assert_eq!(session.into_completed_fuel().unwrap().remaining(), 97);
    drop(rows);
    let mut session = Session::start(&node, &host, Fuel::new(100), LIMITS).unwrap();
    assert!(session.cancel());
    let session = session.into_completed_fuel().unwrap_err();
    assert_eq!(
        session.status(),
        ParallelStatus::Terminal(ParallelEnd::Cancelled)
    );
    assert_eq!(session.fuel_remaining(), 99);
}

#[test]
fn sequential_assembly_uses_the_same_original_acceptance_and_budget_handoff() {
    let node = group(GroupKind::Sequence);
    let host = host();
    let drops = Rc::new(Cell::new(0));
    let authority = Authority(Cell::new(true));
    let session = SequenceEvaluation::start(&node, &host, Fuel::new(100), LIMITS).unwrap();
    let mut session = session.into_completed_fuel().unwrap_err();
    let mut rows = Vec::new();
    loop {
        match session.poll(&host).unwrap() {
            SequencePoll::Request { index, .. } => {
                rows.push(
                    session
                        .try_accept::<_, u64, _>(&(7, index), input(index, &drops), &authority)
                        .unwrap(),
                );
            }
            SequencePoll::Terminal(_) => break,
            _ => panic!(),
        }
    }
    let mut fuel = session.into_completed_fuel().unwrap();
    let reply = assemble_group_reply(&node, rows, LIMITS, &mut fuel, |r, _| {
        admit(r, &host, &authority)
    })
    .unwrap();
    assert_eq!(reply.kind(), GroupKind::Sequence);
    assert_eq!(fuel.remaining(), 94);
    drop(reply);
    assert_eq!(drops.get(), 2);
}

#[test]
fn malformed_or_nonatomic_original_groups_never_enter_native_assembly_hooks() {
    for case in 0..4 {
        let mut node = group(GroupKind::Parallel);
        let Node::Group { branches, .. } = &mut node else {
            panic!()
        };
        match case {
            0 => branches.clear(),
            1 => {
                branches.pop();
            }
            2 => branches[1].name = branches[0].name.clone(),
            _ => {
                branches[1].value = Node::Literal {
                    value: leselang_runtime_core::ScalarValue::Integer(1),
                }
            }
        }
        let calls = Cell::new(0);
        let mut fuel = Fuel::new(100);
        let rows: Vec<Accepted<'_>> = Vec::new();
        let result = assemble_group_reply(&node, rows, LIMITS, &mut fuel, |_, _| {
            calls.set(calls.get() + 1);
            Ok::<_, Private>(())
        });
        assert!(matches!(
            result,
            Err(GroupReplyError::Shape(_) | GroupReplyError::Candidate { .. })
        ));
        assert_eq!(fuel.remaining(), 100);
        assert_eq!(calls.get(), 0);
    }
}

#[test]
fn failed_or_uncertain_sessions_cannot_export_their_retained_meter() {
    struct Failure {
        unwind: bool,
    }
    impl<'a> ParallelEvaluationEnvironment<'a, (), usize, (), Domain> for Failure {
        type Identity = (u64, usize);
        type Declaration = Domain;
        type Request = usize;
        type Error = Private;
        fn preflight_member(&self, _: usize, _: &'a Branch) -> Result<(), Private> {
            Ok(())
        }
        fn prepare_member(
            &self,
            _: usize,
            _: &'a Branch,
            _: &mut Fuel,
        ) -> ParallelPreparation<'a, (u64, usize), Domain, usize, Private> {
            assert!(!self.unwind, "private preparation unwind");
            Err(Private)
        }
    }
    let node = group(GroupKind::Parallel);
    for unwind in [false, true] {
        let host = Failure { unwind };
        let mut session = Session::start(&node, &host, Fuel::new(100), LIMITS).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| session.poll(&host)));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        let session = session.into_completed_fuel().unwrap_err();
        assert_eq!(
            session.status(),
            ParallelStatus::Terminal(if unwind {
                ParallelEnd::HostUncertain
            } else {
                ParallelEnd::Failed
            })
        );
        assert_eq!(session.fuel_remaining(), 98);
        let sequential = group(GroupKind::Sequence);
        let mut sequence =
            SequenceEvaluation::start(&sequential, &host, Fuel::new(100), LIMITS).unwrap();
        let result = catch_unwind(AssertUnwindSafe(|| sequence.poll(&host)));
        if unwind {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        let sequence = sequence.into_completed_fuel().unwrap_err();
        assert_eq!(sequence.fuel_remaining(), 98);
    }
}
