use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_hir::{
    group_reply::assemble_group_reply,
    ir::{Computation, ComputedBranch, GroupKind},
    native_group_preparation::{PreparedNativeGroup, prepare_native_group_members},
    prepared_sequence::*,
    sequence_evaluation::*,
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
}
impl Control {
    fn visit(&self, phase: &'static str, index: usize) -> Result<(), Private> {
        self.events.borrow_mut().push((phase, index));
        assert_ne!(
            self.unwind.get(),
            Some((phase, index)),
            "native sequence unwind"
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
    buffer: Box<usize>,
    control: Rc<Control>,
}
impl Drop for Operand {
    fn drop(&mut self) {
        self.control.operands.set(self.control.operands.get() + 1);
    }
}
struct Identity {
    index: usize,
    owned: bool,
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
        if self.owned {
            self.control
                .identities
                .set(self.control.identities.get() + 1);
            assert!(
                !self.control.cleanup_unwind.replace(false),
                "native cleanup unwind"
            );
        }
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
        if matches!(value, ScalarValue::Integer(n) if *n <= 100) {
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
struct Live {
    control: Rc<Control>,
    live: bool,
}
impl ReplyAuthority<Identity> for Live {
    type Error = Private;
    fn authorize(&self, identity: &Identity) -> Result<(), Private> {
        self.control.visit("authority", identity.index)?;
        if self.live && Rc::ptr_eq(&self.control, &identity.control) {
            Ok(())
        } else {
            Err(Private)
        }
    }
}
type Session<'a> = PreparedSequenceSession<'a, Node, usize, Identity, Domain, Operand>;
const LIMITS: SequenceEvaluationLimits = SequenceEvaluationLimits {
    max_nodes: 256,
    max_depth: 8,
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
) -> PreparedNativeGroup<'a, Branch, Operand> {
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
            control.visit("operand", index)?;
            Ok::<_, Private>(Operand {
                buffer: Box::new(index),
                control: control.clone(),
            })
        },
    )
    .unwrap()
}
fn start<'a>(
    node: &'a Node,
    prepared: PreparedNativeGroup<'a, Branch, Operand>,
    domains: &'a [Domain],
    control: &Rc<Control>,
    fuel: &mut Fuel,
) -> Result<Session<'a>, PreparedSequenceError<Private>> {
    Session::start(
        node,
        prepared,
        fuel,
        LIMITS,
        |index, original, operand| {
            control.visit("admit", index)?;
            if original.result_type != index || *operand.buffer != index {
                return Err(Private);
            }
            Ok(())
        },
        |index, original, request, fuel| {
            control.visit("correlate", index)?;
            fuel.charge(1).map_err(|_| Private)?;
            Ok(PreparedSequenceMember {
                identity: Identity {
                    index: if control.duplicate.get() { 0 } else { index },
                    owned: true,
                    control: control.clone(),
                },
                declaration: &domains[original.result_type],
                request,
            })
        },
    )
}
fn publish(
    session: &mut Session<'_>,
    control: &Rc<Control>,
) -> Result<SequencePoll<Operand>, PreparedSequenceError<Private>> {
    session.poll(|index, original, identity, domain, request| {
        control.visit("publish", index)?;
        if identity.index == index
            && domain.index == original.result_type
            && *request.buffer == index
        {
            Ok(())
        } else {
            Err(Private)
        }
    })
}
fn key(control: &Rc<Control>, index: usize) -> Identity {
    Identity {
        index,
        owned: false,
        control: control.clone(),
    }
}
fn reply(control: &Rc<Control>, value: ScalarValue) -> Reply {
    Reply {
        value,
        buffer: Box::new(9),
        control: control.clone(),
    }
}

#[test]
fn complete_scoped_correlation_then_one_request_per_accepted_reply_uses_original_rows_and_one_meter()
 {
    let control = Rc::new(Control::default());
    let node = group(GroupKind::Sequence, 3);
    let domains = domains(3, &control);
    let mut fuel = Fuel::new(40);
    let prepared = prepare(&node, &control, &mut fuel);
    let pointers: Vec<_> = prepared
        .members()
        .iter()
        .map(|r| (&*r.prepared().buffer as *const usize) as usize)
        .collect();
    assert_eq!(fuel.remaining(), 33);
    let mut session = start(&node, prepared, &domains, &control, &mut fuel).unwrap();
    assert_eq!(fuel.remaining(), 26);
    let events = control.events.borrow();
    assert_eq!(
        &events[3..9],
        [
            ("admit", 0),
            ("admit", 1),
            ("admit", 2),
            ("correlate", 0),
            ("correlate", 1),
            ("correlate", 2)
        ]
    );
    assert!(!events.iter().any(|e| e.0 == "publish"));
    drop(events);
    let live = Live {
        control: control.clone(),
        live: true,
    };
    let mut rows = Vec::new();
    for (index, pointer) in pointers.into_iter().enumerate() {
        assert_eq!(session.status(), SequenceStatus::Ready { index });
        let SequencePoll::Request {
            index: position,
            request,
        } = publish(&mut session, &control).unwrap()
        else {
            panic!()
        };
        assert_eq!(position, index);
        assert_eq!((&*request.buffer as *const usize) as usize, pointer);
        drop(request);
        let events = control.events.borrow().len();
        assert!(
            matches!(publish(&mut session, &control).unwrap(), SequencePoll::Awaiting { index: waiting } if waiting == index)
        );
        assert_eq!(control.events.borrow().len(), events);
        let input = reply(&control, ScalarValue::Integer(41 + index as u64));
        let pointer = (&*input.buffer as *const u8) as usize;
        let rejected = session
            .try_accept(&key(&control, index + 9), input, &live)
            .unwrap_err();
        assert_eq!((&*rejected.reply.buffer as *const u8) as usize, pointer);
        let before = control
            .events
            .borrow()
            .iter()
            .filter(|e| e.0 == "publish")
            .count();
        rows.push(
            session
                .try_accept(&key(&control, index), rejected.reply, &live)
                .unwrap(),
        );
        assert_eq!(
            control
                .events
                .borrow()
                .iter()
                .filter(|e| e.0 == "publish")
                .count(),
            before
        );
        assert_eq!(fuel.remaining(), 26);
    }
    assert_eq!(
        session.status(),
        SequenceStatus::Terminal(SequenceEnd::Completed)
    );
    assert!(!session.cancel());
    assert!(matches!(
        publish(&mut session, &control).unwrap(),
        SequencePoll::Terminal(SequenceEnd::Completed)
    ));
    let whole = assemble_group_reply(&node, rows, LIMITS, &mut fuel, |group, _| {
        for (i, row) in group.members().iter().enumerate() {
            let (index, original, identity, domain, input) = row.as_parts();
            assert_eq!(index, i);
            assert_eq!(identity.index, i);
            assert_eq!(original.result_type, domain.index);
            domain.validate_value(&input.value)?;
        }
        Ok::<_, Private>(())
    })
    .unwrap();
    assert_eq!(whole.members().len(), 3);
    assert_eq!(fuel.remaining(), 22);
    assert!(!format!("{session:?}").contains("fuel"));
}

#[test]
fn shape_foreign_original_rows_and_all_prepared_admission_precede_correlation_and_additional_fuel()
{
    for phase in 0..5 {
        let control = Rc::new(Control::default());
        let node = group(GroupKind::Sequence, 3);
        let foreign = group(GroupKind::Sequence, 3);
        let domains = domains(3, &control);
        let mut fuel = Fuel::new(40);
        let prepared = prepare(&node, &control, &mut fuel);
        control.events.borrow_mut().clear();
        if phase == 4 {
            control.fail.set(Some(("admit", 2)));
        }
        let mut limits = LIMITS;
        if phase == 1 {
            limits.max_nodes = 0;
        }
        if phase == 2 {
            limits.max_branches = 65;
        }
        if phase == 3 {
            limits.max_depth = 0;
        }
        let result: Result<Session<'_>, _> = Session::start(
            if phase == 0 { &foreign } else { &node },
            prepared,
            &mut fuel,
            limits,
            |index, _, _| control.visit("admit", index),
            |index, _, request, _| {
                control.visit("correlate", index)?;
                Ok(PreparedSequenceMember {
                    identity: key(&control, index),
                    declaration: &domains[index],
                    request,
                })
            },
        );
        assert!(result.is_err());
        assert_eq!(fuel.remaining(), 33);
        assert!(!control.events.borrow().iter().any(|e| e.0 == "correlate"));
        assert_eq!(control.operands.get(), 3);
        if phase < 4 {
            assert!(control.events.borrow().is_empty());
        }
    }
    let control = Rc::new(Control::default());
    let node = group(GroupKind::Parallel, 2);
    let domains = domains(2, &control);
    let mut fuel = Fuel::new(40);
    let prepared = prepare(&node, &control, &mut fuel);
    assert!(matches!(
        start(&node, prepared, &domains, &control, &mut fuel),
        Err(PreparedSequenceError::Sequence(
            SequenceEvaluationError::NotSequence
        ))
    ));
    assert_eq!(fuel.remaining(), 35);
}

#[test]
fn late_correlation_duplicate_identity_and_exhaustion_return_no_session_and_drop_every_operand() {
    for phase in 0..3 {
        let control = Rc::new(Control::default());
        let node = group(GroupKind::Sequence, 3);
        let domains = domains(3, &control);
        let mut fuel = Fuel::new(if phase == 2 { 7 } else { 40 });
        let prepared = prepare(&node, &control, &mut fuel);
        if phase == 0 {
            control.fail.set(Some(("correlate", 1)));
        }
        if phase == 1 {
            control.duplicate.set(true);
        }
        let outcome = start(&node, prepared, &domains, &control, &mut fuel);
        assert!(outcome.is_err());
        assert_eq!(control.operands.get(), 3);
        assert_eq!(fuel.remaining(), [29, 26, 0][phase]);
        assert!(!control.events.borrow().iter().any(|e| e.0 == "publish"));
        if phase == 1 {
            assert!(matches!(
                outcome,
                Err(PreparedSequenceError::DuplicateIdentity {
                    index: 1,
                    previous: 0
                })
            ));
        }
    }
}

#[test]
fn admission_correlation_identity_or_prefix_cleanup_unwind_never_publishes_a_request() {
    for phase in 0..4 {
        let control = Rc::new(Control::default());
        let node = group(GroupKind::Sequence, 3);
        let domains = domains(3, &control);
        let mut fuel = Fuel::new(40);
        let prepared = prepare(&node, &control, &mut fuel);
        match phase {
            0 => control.unwind.set(Some(("admit", 2))),
            1 => control.unwind.set(Some(("correlate", 1))),
            2 => control.unwind.set(Some(("identity", 1))),
            _ => {
                control.fail.set(Some(("correlate", 1)));
                control.cleanup_unwind.set(true);
            }
        }
        assert!(
            catch_unwind(AssertUnwindSafe(|| start(
                &node, prepared, &domains, &control, &mut fuel
            )))
            .is_err()
        );
        assert_eq!(control.operands.get(), 3);
        assert_eq!(fuel.remaining(), [33, 29, 26, 29][phase]);
        assert!(!control.events.borrow().iter().any(|e| e.0 == "publish"));
    }
}

#[test]
fn publication_refusal_revocation_or_unwind_closes_before_queued_suffix_cleanup() {
    for phase in 0..3 {
        let control = Rc::new(Control::default());
        let node = group(GroupKind::Sequence, 3);
        let domains = domains(3, &control);
        let mut fuel = Fuel::new(40);
        let prepared = prepare(&node, &control, &mut fuel);
        let mut session = start(&node, prepared, &domains, &control, &mut fuel).unwrap();
        let SequencePoll::Request { request, .. } = publish(&mut session, &control).unwrap() else {
            panic!()
        };
        drop(request);
        let accepted = session
            .try_accept(
                &key(&control, 0),
                reply(&control, ScalarValue::Integer(41)),
                &Live {
                    control: control.clone(),
                    live: true,
                },
            )
            .unwrap();
        drop(accepted);
        if phase == 0 {
            control.fail.set(Some(("publish", 1)));
        }
        if phase == 1 {
            control.unwind.set(Some(("publish", 1)));
        }
        if phase == 2 {
            control.fail.set(Some(("publish", 1)));
            control.cleanup_unwind.set(true);
        }
        let outcome = catch_unwind(AssertUnwindSafe(|| publish(&mut session, &control)));
        if phase == 0 {
            assert!(outcome.unwrap().is_err());
        } else {
            assert!(outcome.is_err());
        }
        assert_eq!(
            session.status(),
            SequenceStatus::Terminal(if phase == 1 {
                SequenceEnd::HostUncertain
            } else {
                SequenceEnd::Failed
            })
        );
        assert_eq!(control.operands.get(), 3);
        assert_eq!(control.identities.get(), 3);
        let events = control.events.borrow().len();
        assert!(matches!(
            publish(&mut session, &control).unwrap(),
            SequencePoll::Terminal(_)
        ));
        assert_eq!(events, control.events.borrow().len());
        assert_eq!(fuel.remaining(), 26);
    }
}

#[test]
fn rejected_identity_authority_type_and_value_preserve_owned_input_and_all_queued_work() {
    let control = Rc::new(Control::default());
    let node = group(GroupKind::Sequence, 3);
    let domains = domains(3, &control);
    let mut fuel = Fuel::new(40);
    let prepared = prepare(&node, &control, &mut fuel);
    let mut session = start(&node, prepared, &domains, &control, &mut fuel).unwrap();
    let input = reply(&control, ScalarValue::Integer(1));
    assert!(
        session
            .try_accept(
                &key(&control, 0),
                input,
                &Live {
                    control: control.clone(),
                    live: true
                }
            )
            .is_err()
    );
    let SequencePoll::Request { request, .. } = publish(&mut session, &control).unwrap() else {
        panic!()
    };
    drop(request);
    for phase in 0..4 {
        let value = match phase {
            2 => ScalarValue::Boolean(true),
            3 => ScalarValue::Integer(101),
            _ => ScalarValue::Integer(41),
        };
        let input = reply(&control, value);
        let pointer = (&*input.buffer as *const u8) as usize;
        let rejected = session
            .try_accept(
                &key(&control, usize::from(phase == 0)),
                input,
                &Live {
                    control: control.clone(),
                    live: phase != 1,
                },
            )
            .unwrap_err();
        assert_eq!((&*rejected.reply.buffer as *const u8) as usize, pointer);
        assert_eq!(session.status(), SequenceStatus::Awaiting { index: 0 });
        assert_eq!(control.operands.get(), 1);
    }
    assert!(session.cancel());
    assert_eq!(control.operands.get(), 3);
    assert_eq!(control.identities.get(), 3);
    assert_eq!(fuel.remaining(), 26);
}

#[test]
fn acceptance_hook_unwind_or_cancel_identity_cleanup_seals_and_releases_all_future_requests() {
    for phase in ["identity", "authority", "borrow", "type", "value", "cancel"] {
        let control = Rc::new(Control::default());
        let node = group(GroupKind::Sequence, 3);
        let domains = domains(3, &control);
        let mut fuel = Fuel::new(40);
        let prepared = prepare(&node, &control, &mut fuel);
        let mut session = start(&node, prepared, &domains, &control, &mut fuel).unwrap();
        let SequencePoll::Request { request, .. } = publish(&mut session, &control).unwrap() else {
            panic!()
        };
        drop(request);
        if phase == "cancel" {
            control.cleanup_unwind.set(true);
            assert!(catch_unwind(AssertUnwindSafe(|| session.cancel())).is_err());
        } else {
            control.unwind.set(Some((phase, 0)));
            assert!(
                catch_unwind(AssertUnwindSafe(|| session.try_accept(
                    &key(&control, 0),
                    reply(&control, ScalarValue::Integer(41)),
                    &Live {
                        control: control.clone(),
                        live: true
                    }
                )))
                .is_err()
            );
        }
        assert_eq!(
            session.status(),
            SequenceStatus::Terminal(if phase == "cancel" {
                SequenceEnd::Cancelled
            } else {
                SequenceEnd::HostUncertain
            })
        );
        assert_eq!(control.operands.get(), 3);
        assert_eq!(control.identities.get(), 3);
        assert!(!session.cancel());
        assert!(matches!(
            publish(&mut session, &control).unwrap(),
            SequencePoll::Terminal(_)
        ));
        assert_eq!(fuel.remaining(), 26);
    }
}

#[test]
fn inclusive_single_and_maximum_width_need_no_poll_meter_and_cancel_unpublished_work() {
    for count in [1, 64] {
        let control = Rc::new(Control::default());
        let node = group(GroupKind::Sequence, count);
        let domains = domains(count, &control);
        let mut fuel = Fuel::new(2 + 4 * count as u64);
        let prepared = prepare(&node, &control, &mut fuel);
        let mut session = start(&node, prepared, &domains, &control, &mut fuel).unwrap();
        assert_eq!(fuel.remaining(), 0);
        assert!(session.cancel());
        assert!(!session.cancel());
        assert_eq!(control.operands.get(), count);
        assert_eq!(control.identities.get(), count);
        assert!(matches!(
            publish(&mut session, &control).unwrap(),
            SequencePoll::Terminal(SequenceEnd::Cancelled)
        ));
        assert!(!control.events.borrow().iter().any(|e| e.0 == "publish"));
    }
}

#[test]
fn unsized_original_domain_and_borrowed_text_view_preserve_rejection_buffer() {
    type TextSession<'a> = PreparedSequenceSession<
        'a,
        Node,
        usize,
        usize,
        dyn HostResultDomain<str, Error = ()>,
        Box<usize>,
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
    struct Authority;
    impl ReplyAuthority<usize> for Authority {
        type Error = ();
        fn authorize(&self, _: &usize) -> Result<(), ()> {
            Ok(())
        }
    }
    let node = group(GroupKind::Sequence, 1);
    let Node::Group { branches, .. } = &node else {
        panic!()
    };
    let mut fuel = Fuel::new(4);
    let prepared = prepare_native_group_members(
        GroupKind::Sequence,
        branches,
        1,
        &mut fuel,
        |b| Ok(b.name.as_str()),
        |_| Ok(()),
        |i, _, _| Ok::<_, ()>(Box::new(i)),
    )
    .unwrap();
    let domain = Text;
    let mut session: TextSession<'_> = TextSession::start(
        &node,
        prepared,
        &mut fuel,
        LIMITS,
        |_, _, _| Ok::<_, ()>(()),
        |i, _, request, _| {
            Ok(PreparedSequenceMember {
                identity: i,
                declaration: &domain as &dyn HostResultDomain<str, Error = ()>,
                request,
            })
        },
    )
    .unwrap();
    assert_eq!(fuel.remaining(), 0);
    assert!(matches!(
        session.poll(|_, _, _, _, _| Ok::<_, ()>(())).unwrap(),
        SequencePoll::Request { index: 0, .. }
    ));
    let input = "too long".to_string();
    let pointer = input.as_ptr();
    let rejected = session
        .try_accept::<String, str, _>(&0, input, &Authority)
        .unwrap_err();
    assert_eq!(rejected.reply.as_ptr(), pointer);
    assert_eq!(
        session
            .try_accept::<String, str, _>(&0, "yes".into(), &Authority)
            .unwrap()
            .into_parts()
            .4,
        "yes"
    );
    assert_eq!(
        session.status(),
        SequenceStatus::Terminal(SequenceEnd::Completed)
    );
}
