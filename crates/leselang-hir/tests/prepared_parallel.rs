use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_hir::{
    ir::{Computation, ComputedBranch, GroupKind},
    native_group_preparation::prepare_native_group_members,
    parallel_evaluation::*,
    prepared_parallel::*,
};
use leselang_runtime_core::{Fuel, HostResultDomain, ReplyAuthority, ScalarValue};

struct Private;
#[derive(Default)]
struct Control {
    events: RefCell<Vec<(&'static str, usize)>>,
    fail: Cell<Option<(&'static str, usize)>>,
    unwind: Cell<Option<(&'static str, usize)>>,
    duplicate: Cell<bool>,
    cleanup_unwind: Cell<bool>,
    operands: Cell<usize>,
    identities: Cell<usize>,
    replies: Cell<usize>,
}
impl Control {
    fn visit(&self, phase: &'static str, index: usize) -> Result<(), Private> {
        self.events.borrow_mut().push((phase, index));
        assert_ne!(
            self.unwind.get(),
            Some((phase, index)),
            "native prepared barrier unwind"
        );
        if self.fail.get() == Some((phase, index)) {
            Err(Private)
        } else {
            Ok(())
        }
    }
}
struct Native;
type Node = Computation<(), (), Native, usize>;
type Branch = ComputedBranch<Node, usize>;
struct Operand {
    buffer: Box<u8>,
    control: Rc<Control>,
}
impl Drop for Operand {
    fn drop(&mut self) {
        self.control.operands.set(self.control.operands.get() + 1);
    }
}
struct Identity {
    index: usize,
    control: Rc<Control>,
}
impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        self.control
            .visit("identity", self.index)
            .unwrap_or_else(|_| panic!());
        self.index == other.index && Rc::ptr_eq(&self.control, &other.control)
    }
}
impl Drop for Identity {
    fn drop(&mut self) {
        self.control
            .identities
            .set(self.control.identities.get() + 1);
        assert!(
            !self.control.cleanup_unwind.replace(false),
            "native identity cleanup unwind"
        );
    }
}
struct Domain {
    index: usize,
    control: Rc<Control>,
}
impl HostResultDomain<ScalarValue> for Domain {
    type Error = Private;
    fn matches_type(&self, value: &ScalarValue) -> bool {
        self.control
            .visit("type", self.index)
            .unwrap_or_else(|_| panic!());
        matches!(value, ScalarValue::Integer(_))
    }
    fn validate_value(&self, value: &ScalarValue) -> Result<(), Private> {
        self.control.visit("value", self.index)?;
        if matches!(value, ScalarValue::Integer(n) if *n < 100) {
            Ok(())
        } else {
            Err(Private)
        }
    }
}
struct Reply {
    value: ScalarValue,
    buffer: Box<u8>,
    control: Rc<Control>,
}
impl Borrow<ScalarValue> for Reply {
    fn borrow(&self) -> &ScalarValue {
        self.control.visit("borrow", 0).unwrap_or_else(|_| panic!());
        &self.value
    }
}
impl Drop for Reply {
    fn drop(&mut self) {
        self.control.replies.set(self.control.replies.get() + 1);
    }
}
struct Authority {
    control: Rc<Control>,
    live: bool,
}
impl ReplyAuthority<Identity> for Authority {
    type Error = Private;
    fn authorize(&self, key: &Identity) -> Result<(), Private> {
        self.control.visit("authority", key.index)?;
        if self.live && Rc::ptr_eq(&self.control, &key.control) {
            Ok(())
        } else {
            Err(Private)
        }
    }
}
type Session<'a> = PreparedParallelSession<'a, Node, usize, Identity, Domain, Reply>;
const LIMITS: ParallelEvaluationLimits = ParallelEvaluationLimits {
    max_nodes: 128,
    max_depth: 4,
    max_branches: 64,
};
fn group(kind: GroupKind, count: usize) -> Node {
    Node::Group {
        group_kind: kind,
        branches: (0..count)
            .map(|index| Branch {
                name: format!("m{index}"),
                value: Node::Host {
                    effect: Box::new(Native),
                },
                result_type: index,
            })
            .collect(),
    }
}
fn domains(count: usize, control: &Rc<Control>) -> Vec<Domain> {
    (0..count)
        .map(|index| Domain {
            index,
            control: control.clone(),
        })
        .collect()
}
fn prepare<'a>(
    node: &'a Node,
    control: &Rc<Control>,
    fuel: &mut Fuel,
) -> leselang_hir::native_group_preparation::PreparedNativeGroup<'a, Branch, Operand> {
    let Node::Group {
        group_kind,
        branches,
    } = node
    else {
        panic!()
    };
    prepare_native_group_members(
        *group_kind,
        branches,
        64,
        fuel,
        |b| Ok(b.name.as_str()),
        |_| Ok(()),
        |index, _, fuel| {
            fuel.charge(1).map_err(|_| Private)?;
            Ok::<_, Private>(Operand {
                buffer: Box::new(index as u8),
                control: control.clone(),
            })
        },
    )
    .unwrap()
}
fn correlate<'a>(
    index: usize,
    original: &Branch,
    operand: Operand,
    fuel: &mut Fuel,
    domains: &'a [Domain],
    control: &Rc<Control>,
) -> Result<PreparedParallelMember<'a, Identity, Domain, Operand>, Private> {
    control.visit("correlate", index)?;
    fuel.charge(1).map_err(|_| Private)?;
    assert_eq!(original.result_type, index);
    Ok(PreparedParallelMember {
        identity: Identity {
            index: if control.duplicate.get() { 0 } else { index },
            control: control.clone(),
        },
        declaration: &domains[index],
        request: operand,
    })
}
fn reply(control: &Rc<Control>, n: u64) -> Reply {
    Reply {
        value: ScalarValue::Integer(n),
        buffer: Box::new(8),
        control: control.clone(),
    }
}
fn key(control: &Rc<Control>, index: usize) -> Identity {
    Identity {
        index,
        control: control.clone(),
    }
}

#[test]
fn complete_scoped_operands_move_once_to_the_original_out_of_order_reply_barrier_without_a_new_meter()
 {
    let control = Rc::new(Control::default());
    let node = group(GroupKind::Parallel, 3);
    let domains = domains(3, &control);
    let mut fuel = Fuel::new(40);
    let prepared = prepare(&node, &control, &mut fuel);
    assert_eq!(fuel.remaining(), 33);
    let pointers: Vec<_> = prepared
        .members()
        .iter()
        .map(|r| (&*r.prepared().buffer as *const u8) as usize)
        .collect();
    let (mut session, requests) = Session::start(
        &node,
        prepared,
        &mut fuel,
        LIMITS,
        |i, _, _| control.visit("admit", i),
        |i, b, p, f| correlate(i, b, p, f, &domains, &control),
    )
    .unwrap();
    assert_eq!(fuel.remaining(), 26);
    assert_eq!(
        &control.events.borrow()[..6],
        [
            ("admit", 0),
            ("admit", 1),
            ("admit", 2),
            ("correlate", 0),
            ("correlate", 1),
            ("correlate", 2)
        ]
    );
    for (i, request) in requests.into_iter().enumerate() {
        assert_eq!(request.index, i);
        assert_eq!(
            (&*request.request.buffer as *const u8) as usize,
            pointers[i]
        );
    }
    let authority = Authority {
        control: control.clone(),
        live: true,
    };
    let keys: Vec<_> = (0..3).map(|i| key(&control, i)).collect();
    for (step, index) in [2, 0, 1].into_iter().enumerate() {
        assert!(
            matches!(session.poll().unwrap(), ParallelPoll::Awaiting { remaining } if remaining == 3 - step)
        );
        session
            .try_accept(
                index,
                &keys[index],
                reply(&control, index as u64),
                &authority,
            )
            .unwrap();
        assert_eq!(fuel.remaining(), 26);
    }
    assert_eq!(session.status(), ParallelStatus::ReadyResults);
    let ParallelPoll::Results(rows) = session.poll().unwrap() else {
        panic!()
    };
    let Node::Group { branches, .. } = &node else {
        panic!()
    };
    for (i, row) in rows.into_iter().enumerate() {
        let (index, original, identity, domain, input) = row.into_parts();
        assert_eq!(index, i);
        assert!(std::ptr::eq(original, &branches[i]));
        assert!(std::ptr::eq(domain, &domains[i]));
        assert_eq!(identity.index, i);
        assert_eq!(input.value, ScalarValue::Integer(i as u64));
    }
    assert!(matches!(
        session.poll().unwrap(),
        ParallelPoll::Terminal(ParallelEnd::Completed)
    ));
    assert_eq!(fuel.remaining(), 26);
    assert_eq!(control.operands.get(), 3);
    assert_eq!(control.replies.get(), 3);
    assert!(!format!("{session:?}").contains("fuel"));
}

#[test]
fn foreign_prepared_rows_shape_and_all_native_admission_fail_before_correlation_or_additional_fuel()
{
    for case in 0..5 {
        let control = Rc::new(Control::default());
        let node = group(
            if case == 1 {
                GroupKind::Sequence
            } else {
                GroupKind::Parallel
            },
            3,
        );
        let other = group(GroupKind::Parallel, 3);
        let domains = domains(3, &control);
        let mut fuel = Fuel::new(40);
        let prepared = prepare(&node, &control, &mut fuel);
        let mut limits = LIMITS;
        if case == 2 {
            limits.max_nodes = 0;
        }
        if case == 3 {
            limits.max_branches = 65;
        }
        if case == 4 {
            control.fail.set(Some(("admit", 2)));
        }
        let result = Session::start(
            if case == 0 { &other } else { &node },
            prepared,
            &mut fuel,
            limits,
            |i, _, _| control.visit("admit", i),
            |i, b, p, f| correlate(i, b, p, f, &domains, &control),
        );
        assert!(result.is_err());
        if case == 0 {
            assert!(matches!(result, Err(PreparedParallelError::OriginalGroup)));
        }
        assert_eq!(fuel.remaining(), 33);
        assert!(
            !control
                .events
                .borrow()
                .iter()
                .any(|(p, _)| *p == "correlate")
        );
        assert_eq!(control.operands.get(), 3);
    }
}

#[test]
fn correlation_refusal_duplicate_identity_and_exhaustion_consume_all_preparations_without_partial_handoff()
 {
    for case in 0..3 {
        let control = Rc::new(Control::default());
        let node = group(GroupKind::Parallel, 3);
        let domains = domains(3, &control);
        let mut fuel = Fuel::new(if case == 2 { 7 } else { 40 });
        let prepared = prepare(&node, &control, &mut fuel);
        if case == 0 {
            control.fail.set(Some(("correlate", 1)));
        }
        if case == 1 {
            control.duplicate.set(true);
        }
        let result = Session::start(
            &node,
            prepared,
            &mut fuel,
            LIMITS,
            |i, _, _| control.visit("admit", i),
            |i, b, p, f| correlate(i, b, p, f, &domains, &control),
        );
        assert!(result.is_err());
        if case == 1 {
            assert!(matches!(
                result,
                Err(PreparedParallelError::Parallel(
                    ParallelEvaluationError::DuplicateIdentity {
                        index: 1,
                        previous: 0
                    }
                ))
            ));
        }
        assert_eq!(fuel.remaining(), [29, 26, 0][case]);
        assert_eq!(control.operands.get(), 3);
    }
}

#[test]
fn native_admission_correlation_identity_and_prefix_cleanup_unwind_publish_nothing_and_drop_suffixes()
 {
    for phase in ["admit", "correlate", "identity", "cleanup"] {
        let control = Rc::new(Control::default());
        let node = group(GroupKind::Parallel, 3);
        let domains = domains(3, &control);
        let mut fuel = Fuel::new(40);
        let prepared = prepare(&node, &control, &mut fuel);
        if phase == "cleanup" {
            control.fail.set(Some(("correlate", 1)));
            control.cleanup_unwind.set(true);
        } else {
            control.unwind.set(Some((phase, 1)));
        }
        let result = catch_unwind(AssertUnwindSafe(|| {
            Session::start(
                &node,
                prepared,
                &mut fuel,
                LIMITS,
                |i, _, _| control.visit("admit", i),
                |i, b, p, f| correlate(i, b, p, f, &domains, &control),
            )
        }));
        assert!(result.is_err());
        assert_eq!(control.operands.get(), 3);
        assert_eq!(
            fuel.remaining(),
            if phase == "admit" {
                33
            } else if phase == "identity" {
                26
            } else {
                29
            }
        );
    }
}

#[test]
fn wrong_member_identity_domain_or_authority_keeps_original_input_and_never_exposes_partial_results()
 {
    let control = Rc::new(Control::default());
    let node = group(GroupKind::Parallel, 3);
    let domains = domains(3, &control);
    let mut fuel = Fuel::new(40);
    let prepared = prepare(&node, &control, &mut fuel);
    let (mut session, requests) = Session::start(
        &node,
        prepared,
        &mut fuel,
        LIMITS,
        |i, _, _| control.visit("admit", i),
        |i, b, p, f| correlate(i, b, p, f, &domains, &control),
    )
    .unwrap();
    drop(requests);
    let keys: Vec<_> = (0..3).map(|i| key(&control, i)).collect();
    let mut authority = Authority {
        control: control.clone(),
        live: true,
    };
    for case in 0..4 {
        let input = reply(&control, if case == 2 { 100 } else { 7 });
        let pointer = (&*input.buffer as *const u8) as usize;
        authority.live = case != 3;
        let rejected = session
            .try_accept(
                if case == 0 { 99 } else { 0 },
                &keys[usize::from(case == 1)],
                input,
                &authority,
            )
            .unwrap_err();
        assert_eq!((&*rejected.reply.buffer as *const u8) as usize, pointer);
        assert_eq!(session.status(), ParallelStatus::Awaiting { remaining: 3 });
        assert_eq!(fuel.remaining(), 26);
    }
    authority.live = true;
    session
        .try_accept(2, &keys[2], reply(&control, 7), &authority)
        .unwrap();
    assert!(matches!(
        session.poll().unwrap(),
        ParallelPoll::Awaiting { remaining: 2 }
    ));
    let before = control.events.borrow().len();
    assert!(
        session
            .try_accept(2, &keys[2], reply(&control, 7), &authority)
            .is_err()
    );
    assert_eq!(control.events.borrow().len(), before);
    assert!(session.cancel());
    assert!(matches!(
        session.poll().unwrap(),
        ParallelPoll::Terminal(ParallelEnd::Cancelled)
    ));
    assert_eq!(fuel.remaining(), 26);
}

#[test]
fn acceptance_unwind_and_cancel_cleanup_use_the_same_terminal_fencing_as_the_legacy_barrier() {
    for phase in ["identity", "authority", "borrow", "type", "value", "cancel"] {
        let control = Rc::new(Control::default());
        let node = group(GroupKind::Parallel, 3);
        let domains = domains(3, &control);
        let mut fuel = Fuel::new(40);
        let prepared = prepare(&node, &control, &mut fuel);
        let (mut session, requests) = Session::start(
            &node,
            prepared,
            &mut fuel,
            LIMITS,
            |i, _, _| control.visit("admit", i),
            |i, b, p, f| correlate(i, b, p, f, &domains, &control),
        )
        .unwrap();
        drop(requests);
        let keys: Vec<_> = (0..3).map(|i| key(&control, i)).collect();
        let authority = Authority {
            control: control.clone(),
            live: true,
        };
        session
            .try_accept(2, &keys[2], reply(&control, 7), &authority)
            .unwrap();
        let result = if phase == "cancel" {
            control.cleanup_unwind.set(true);
            catch_unwind(AssertUnwindSafe(|| {
                session.cancel();
            }))
        } else {
            control.unwind.set(Some((phase, 0)));
            catch_unwind(AssertUnwindSafe(|| {
                let _ = session.try_accept(0, &keys[0], reply(&control, 7), &authority);
            }))
        };
        assert!(result.is_err());
        let end = if phase == "cancel" {
            ParallelEnd::Cancelled
        } else {
            ParallelEnd::HostUncertain
        };
        assert_eq!(session.status(), ParallelStatus::Terminal(end));
        assert!(matches!(session.poll().unwrap(), ParallelPoll::Terminal(_)));
        assert_eq!(control.identities.get(), 3);
        assert_eq!(control.replies.get(), if phase == "cancel" { 1 } else { 2 });
        assert_eq!(fuel.remaining(), 26);
    }
}

#[test]
fn inclusive_maximum_width_preserves_the_original_meter_and_ready_results_cancel_without_handoff() {
    for count in [2, 64] {
        let control = Rc::new(Control::default());
        let node = group(GroupKind::Parallel, count);
        let domains = domains(count, &control);
        let mut fuel = Fuel::new(2 + 4 * count as u64);
        let prepared = prepare(&node, &control, &mut fuel);
        let (mut session, requests) = Session::start(
            &node,
            prepared,
            &mut fuel,
            LIMITS,
            |i, _, _| control.visit("admit", i),
            |i, b, p, f| correlate(i, b, p, f, &domains, &control),
        )
        .unwrap();
        assert_eq!(fuel.remaining(), 0);
        drop(requests);
        let authority = Authority {
            control: control.clone(),
            live: true,
        };
        for index in 0..count {
            let identity = key(&control, index);
            session
                .try_accept(index, &identity, reply(&control, 7), &authority)
                .unwrap();
        }
        assert_eq!(session.status(), ParallelStatus::ReadyResults);
        assert!(session.cancel());
        assert!(matches!(
            session.poll().unwrap(),
            ParallelPoll::Terminal(ParallelEnd::Cancelled)
        ));
        assert_eq!(control.replies.get(), count);
        assert_eq!(fuel.remaining(), 0);
    }
}

#[test]
fn unsized_original_domains_and_borrowed_reply_views_work_without_payload_or_ir_clone_bounds() {
    type TextSession<'a> = PreparedParallelSession<
        'a,
        Node,
        usize,
        usize,
        dyn HostResultDomain<str, Error = ()>,
        String,
    >;
    struct Text;
    impl HostResultDomain<str> for Text {
        type Error = ();
        fn matches_type(&self, _: &str) -> bool {
            true
        }
        fn validate_value(&self, value: &str) -> Result<(), ()> {
            if value.len() <= 4 { Ok(()) } else { Err(()) }
        }
    }
    struct Live;
    impl ReplyAuthority<usize> for Live {
        type Error = ();
        fn authorize(&self, _: &usize) -> Result<(), ()> {
            Ok(())
        }
    }
    let node = group(GroupKind::Parallel, 2);
    let Node::Group { branches, .. } = &node else {
        panic!()
    };
    let mut fuel = Fuel::new(9);
    let prepared = prepare_native_group_members(
        GroupKind::Parallel,
        branches,
        2,
        &mut fuel,
        |b| Ok(b.name.as_str()),
        |_| Ok(()),
        |i, _, _| Ok::<_, ()>(Box::new(i)),
    )
    .unwrap();
    let domain = Text;
    let (mut session, requests): (TextSession<'_>, _) = PreparedParallelSession::start(
        &node,
        prepared,
        &mut fuel,
        LIMITS,
        |_, _, _| Ok::<_, ()>(()),
        |index, _, operand, _| {
            Ok(PreparedParallelMember {
                identity: index,
                declaration: &domain as &dyn HostResultDomain<str, Error = ()>,
                request: operand,
            })
        },
    )
    .unwrap();
    assert_eq!(fuel.remaining(), 3);
    assert_eq!(*requests[1].request, 1);
    let input = "too long".to_string();
    let pointer = input.as_ptr();
    let rejected = session
        .try_accept::<str, _>(1, &1, input, &Live)
        .unwrap_err();
    assert_eq!(rejected.reply.as_ptr(), pointer);
    session
        .try_accept::<str, _>(1, &1, "yes".into(), &Live)
        .unwrap();
    session
        .try_accept::<str, _>(0, &0, "no".into(), &Live)
        .unwrap();
    let ParallelPoll::Results(rows) = session.poll().unwrap() else {
        panic!()
    };
    assert_eq!(
        rows.into_iter()
            .map(|r| r.into_parts().4)
            .collect::<Vec<_>>(),
        ["no", "yes"]
    );
    assert_eq!(fuel.remaining(), 3);
}
