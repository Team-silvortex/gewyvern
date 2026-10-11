use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_hir::ir::{Computation, ComputedBranch, GroupKind};
use leselang_hir::parallel_evaluation::*;
use leselang_runtime_core::{
    Fuel, HostResultDomain, HostResultError, ReplyAcceptanceError, ReplyAuthority, ScalarValue,
};

struct Private(&'static str);
#[derive(Default)]
struct Control {
    events: RefCell<Vec<(&'static str, usize)>>,
    fail: Cell<Option<(&'static str, usize)>>,
    unwind: Cell<Option<(&'static str, usize)>>,
    duplicate: Cell<bool>,
    panic_key_drop: Cell<bool>,
    keys: Cell<usize>,
    requests: Cell<usize>,
    replies: Cell<usize>,
    tokens: Cell<usize>,
}
impl Control {
    fn visit(&self, phase: &'static str, index: usize) -> Result<(), Private> {
        self.events.borrow_mut().push((phase, index));
        assert_ne!(
            self.unwind.get(),
            Some((phase, index)),
            "private native unwind"
        );
        if self.fail.get() == Some((phase, index)) {
            Err(Private("private native failure"))
        } else {
            Ok(())
        }
    }
}
struct Token {
    index: usize,
    control: Rc<Control>,
}
impl Drop for Token {
    fn drop(&mut self) {
        self.control.tokens.set(self.control.tokens.get() + 1);
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
            Err(Private("private value domain"))
        }
    }
}
struct Key {
    owner: u64,
    index: usize,
    owned: bool,
    control: Rc<Control>,
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.control
            .visit("identity", self.index)
            .unwrap_or_else(|_| panic!());
        self.owner == other.owner && self.index == other.index
    }
}
impl Drop for Key {
    fn drop(&mut self) {
        if self.owned {
            self.control.keys.set(self.control.keys.get() + 1);
            assert!(
                !self.control.panic_key_drop.replace(false),
                "private cleanup unwind"
            );
        }
    }
}
struct Request {
    original: *const Token,
    buffer: Vec<u8>,
    control: Rc<Control>,
}
impl Drop for Request {
    fn drop(&mut self) {
        self.control.requests.set(self.control.requests.get() + 1);
    }
}
struct Reply {
    index: usize,
    value: ScalarValue,
    buffer: Vec<u8>,
    control: Rc<Control>,
}
impl Borrow<ScalarValue> for Reply {
    fn borrow(&self) -> &ScalarValue {
        self.control
            .visit("borrow", self.index)
            .unwrap_or_else(|_| panic!());
        &self.value
    }
}
impl Drop for Reply {
    fn drop(&mut self) {
        self.control.replies.set(self.control.replies.get() + 1);
    }
}
struct Environment(Rc<Control>);
struct Authority {
    control: Rc<Control>,
    live: bool,
}
impl ReplyAuthority<Key> for Authority {
    type Error = Private;
    fn authorize(&self, key: &Key) -> Result<(), Private> {
        self.control.visit("policy", key.index)?;
        if self.live && key.owner == 7 {
            Ok(())
        } else {
            Err(Private("private revoked grant"))
        }
    }
}
type Node = Computation<Token, Token, Token, Domain>;
type Branch = ComputedBranch<Node, Domain>;
type Session<'a> = ParallelEvaluation<'a, Token, Token, Token, Domain, Key, Domain, Reply>;
const LIMITS: ParallelEvaluationLimits = ParallelEvaluationLimits {
    max_nodes: 256,
    max_depth: 32,
    max_branches: 64,
};
impl<'a> ParallelEvaluationEnvironment<'a, Token, Token, Token, Domain> for Environment {
    type Identity = Key;
    type Declaration = Domain;
    type Request = Request;
    type Error = Private;
    fn preflight_member(&self, index: usize, branch: &'a Branch) -> Result<(), Private> {
        self.0.visit("preflight", index)?;
        if matches!(&branch.value, Node::Host { effect } if effect.index == branch.result_type.index)
        {
            Ok(())
        } else {
            Err(Private("private wrong declaration"))
        }
    }
    fn prepare_member(
        &self,
        index: usize,
        branch: &'a Branch,
        fuel: &mut Fuel,
    ) -> ParallelPreparation<'a, Key, Domain, Request, Private> {
        fuel.charge(2).map_err(|_| Private("private native fuel"))?;
        self.0.visit("prepare", index)?;
        let Node::Host { effect } = &branch.value else {
            return Err(Private("private not atomic"));
        };
        Ok(PreparedParallelMember {
            identity: Key {
                owner: 7,
                index: if self.0.duplicate.get() { 0 } else { index },
                owned: true,
                control: self.0.clone(),
            },
            declaration: &branch.result_type,
            request: Request {
                original: effect.as_ref(),
                buffer: vec![index as u8; 17],
                control: self.0.clone(),
            },
        })
    }
}
fn group(count: usize, control: &Rc<Control>) -> Node {
    Node::Group {
        group_kind: GroupKind::Parallel,
        branches: (0..count)
            .map(|index| Branch {
                name: format!("task_{index}"),
                value: Node::Host {
                    effect: Box::new(Token {
                        index,
                        control: control.clone(),
                    }),
                },
                result_type: Domain {
                    index,
                    control: control.clone(),
                },
            })
            .collect(),
    }
}
fn fixture() -> (Rc<Control>, Environment, Authority) {
    let control = Rc::new(Control::default());
    (
        control.clone(),
        Environment(control.clone()),
        Authority {
            control,
            live: true,
        },
    )
}
fn key(index: usize, control: &Rc<Control>) -> Key {
    Key {
        owner: 7,
        index,
        owned: false,
        control: control.clone(),
    }
}
fn reply(index: usize, value: ScalarValue, control: &Rc<Control>) -> Reply {
    Reply {
        index,
        value,
        buffer: vec![9; 31],
        control: control.clone(),
    }
}
fn requests(session: &mut Session<'_>, env: &Environment) -> Vec<ParallelRequest<Request>> {
    let ParallelPoll::Requests(requests) = session.poll(env).unwrap() else {
        panic!()
    };
    requests
}
fn accept(session: &mut Session<'_>, control: &Rc<Control>, authority: &Authority, index: usize) {
    session
        .try_accept::<ScalarValue, _>(
            index,
            &key(index, control),
            reply(index, ScalarValue::Integer(42), control),
            authority,
        )
        .unwrap();
}

#[test]
fn whole_cold_shape_candidates_and_policy_precede_preparation_and_fuel() {
    for count in [0, 1, 65] {
        let (c, env, _) = fixture();
        assert!(Session::start(&group(count, &c), &env, Fuel::new(100), LIMITS).is_err());
        assert!(c.events.borrow().is_empty());
    }
    for limits in [
        ParallelEvaluationLimits {
            max_nodes: 0,
            ..LIMITS
        },
        ParallelEvaluationLimits {
            max_nodes: 2,
            ..LIMITS
        },
        ParallelEvaluationLimits {
            max_depth: 0,
            ..LIMITS
        },
        ParallelEvaluationLimits {
            max_branches: 0,
            ..LIMITS
        },
        ParallelEvaluationLimits {
            max_branches: 1,
            ..LIMITS
        },
        ParallelEvaluationLimits {
            max_nodes: 16_385,
            ..LIMITS
        },
        ParallelEvaluationLimits {
            max_depth: 65,
            ..LIMITS
        },
        ParallelEvaluationLimits {
            max_branches: 65,
            ..LIMITS
        },
    ] {
        let (c, env, _) = fixture();
        assert!(Session::start(&group(2, &c), &env, Fuel::new(100), limits).is_err());
        assert!(c.events.borrow().is_empty());
    }
    for malformed in 0..4 {
        let (c, env, _) = fixture();
        let mut node = group(3, &c);
        let Node::Group {
            group_kind,
            branches,
        } = &mut node
        else {
            panic!()
        };
        match malformed {
            0 => *group_kind = GroupKind::Sequence,
            1 => branches[2].name = branches[0].name.clone(),
            2 => {
                branches[2].value = Node::Local {
                    name: "unbound".into(),
                }
            }
            3 => branches[2].value = group(2, &c),
            _ => unreachable!(),
        }
        assert!(Session::start(&node, &env, Fuel::new(100), LIMITS).is_err());
        assert!(c.events.borrow().is_empty());
    }
    let (c, env, _) = fixture();
    let node = group(3, &c);
    c.fail.set(Some(("preflight", 2)));
    assert!(matches!(
        Session::start(&node, &env, Fuel::new(0), LIMITS),
        Err(ParallelEvaluationError::Native {
            index: 2,
            phase: ParallelPhase::Preflight,
            ..
        })
    ));
    assert_eq!(
        *c.events.borrow(),
        [("preflight", 0), ("preflight", 1), ("preflight", 2)]
    );
}

#[test]
fn complete_move_only_batch_is_prepared_once_and_preserves_original_rows() {
    let (c, env, _) = fixture();
    let node = group(3, &c);
    let mut session = Session::start(&node, &env, Fuel::new(100), LIMITS).unwrap();
    assert_eq!(session.fuel_remaining(), 99);
    assert_eq!(session.status(), ParallelStatus::Ready);
    let batch = requests(&mut session, &env);
    assert_eq!(session.fuel_remaining(), 90);
    let Node::Group { branches, .. } = &node else {
        panic!()
    };
    for (index, item) in batch.iter().enumerate() {
        assert_eq!(item.index, index);
        assert_eq!(item.request.buffer, vec![index as u8; 17]);
        let Node::Host { effect } = &branches[index].value else {
            panic!()
        };
        assert_eq!(item.request.original, effect.as_ref() as *const Token);
    }
    let events = c.events.borrow().clone();
    for _ in 0..5 {
        assert!(matches!(
            session.poll(&env).unwrap(),
            ParallelPoll::Awaiting { remaining: 3 }
        ));
    }
    assert_eq!(*c.events.borrow(), events);
    assert_eq!(session.fuel_remaining(), 90);
    assert_eq!(c.requests.get(), 0);
    drop(batch);
    assert_eq!(c.requests.get(), 3);
    assert_eq!(c.keys.get(), 0);
    drop(session);
    assert_eq!(c.keys.get(), 3);
    drop(node);
    assert_eq!(c.tokens.get(), 3);
}

#[test]
fn out_of_order_replies_publish_only_one_complete_ordered_original_result_batch() {
    for order in [[2, 0, 1], [1, 2, 0], [0, 1, 2]] {
        let (c, env, authority) = fixture();
        let node = group(3, &c);
        let mut session = Session::start(&node, &env, Fuel::new(100), LIMITS).unwrap();
        drop(requests(&mut session, &env));
        let mut pointers = [std::ptr::null(); 3];
        for (done, index) in order.into_iter().enumerate() {
            let received = reply(index, ScalarValue::Integer(index as u64), &c);
            pointers[index] = received.buffer.as_ptr();
            session
                .try_accept::<ScalarValue, _>(index, &key(index, &c), received, &authority)
                .unwrap();
            let events = c.events.borrow().clone();
            if done < 2 {
                assert!(
                    matches!(session.poll(&env).unwrap(), ParallelPoll::Awaiting { remaining } if remaining == 2 - done)
                );
            } else {
                assert_eq!(session.status(), ParallelStatus::ReadyResults);
            }
            assert_eq!(*c.events.borrow(), events);
            assert_eq!(c.replies.get(), 0);
            assert_eq!(session.fuel_remaining(), 90);
        }
        let events = c.events.borrow().clone();
        let ParallelPoll::Results(results) = session.poll(&env).unwrap() else {
            panic!()
        };
        let Node::Group { branches, .. } = &node else {
            panic!()
        };
        for (expected, item) in results.into_iter().enumerate() {
            let (index, branch, identity, declaration, received) = item.into_parts();
            assert_eq!(index, expected);
            assert!(std::ptr::eq(branch, &branches[index]));
            assert!(std::ptr::eq(declaration, &branches[index].result_type));
            assert_eq!(identity.owner, 7);
            assert_eq!(identity.index, index);
            assert_eq!(received.value, ScalarValue::Integer(index as u64));
            assert_eq!(received.buffer.as_ptr(), pointers[index]);
        }
        assert_eq!(c.replies.get(), 3);
        assert_eq!(c.keys.get(), 3);
        assert_eq!(
            session.status(),
            ParallelStatus::Terminal(ParallelEnd::Completed)
        );
        assert!(!session.cancel());
        assert!(matches!(
            session.poll(&env).unwrap(),
            ParallelPoll::Terminal(ParallelEnd::Completed)
        ));
        assert_eq!(*c.events.borrow(), events);
    }
}

#[test]
fn identity_live_authority_type_and_value_rejections_preserve_the_input_and_barrier() {
    let (c, env, authority) = fixture();
    let node = group(3, &c);
    let mut session = Session::start(&node, &env, Fuel::new(100), LIMITS).unwrap();
    drop(requests(&mut session, &env));
    for phase in 0..4 {
        c.events.borrow_mut().clear();
        let mut incoming_key = key(1, &c);
        if phase == 0 {
            incoming_key.owner = 8;
        }
        let policy = Authority {
            control: c.clone(),
            live: phase != 1,
        };
        let received = reply(
            1,
            if phase == 2 {
                ScalarValue::Boolean(true)
            } else {
                ScalarValue::Integer(if phase == 3 { 101 } else { 42 })
            },
            &c,
        );
        let pointer = received.buffer.as_ptr();
        let rejected = session
            .try_accept::<ScalarValue, _>(1, &incoming_key, received, &policy)
            .unwrap_err();
        assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
        match phase {
            0 => assert!(matches!(
                rejected.error,
                ParallelReplyError::Reply(ReplyAcceptanceError::IdentityMismatch)
            )),
            1 => assert!(matches!(
                rejected.error,
                ParallelReplyError::Reply(ReplyAcceptanceError::Authority(Private(
                    "private revoked grant"
                )))
            )),
            2 => assert!(matches!(
                rejected.error,
                ParallelReplyError::Reply(ReplyAcceptanceError::Result(
                    HostResultError::TypeMismatch
                ))
            )),
            3 => assert!(matches!(
                rejected.error,
                ParallelReplyError::Reply(ReplyAcceptanceError::Result(
                    HostResultError::InvalidValue(Private("private value domain"))
                ))
            )),
            _ => unreachable!(),
        }
        let expected = [
            ("identity", 1),
            ("policy", 1),
            ("borrow", 1),
            ("type", 1),
            ("value", 1),
        ];
        assert_eq!(
            *c.events.borrow(),
            expected[..if phase == 0 {
                1
            } else if phase == 1 {
                2
            } else if phase == 2 {
                4
            } else {
                5
            }]
        );
        assert_eq!(session.status(), ParallelStatus::Awaiting { remaining: 3 });
        assert_eq!(session.fuel_remaining(), 90);
    }
    for index in [2, 0, 1] {
        accept(&mut session, &c, &authority, index);
    }
    assert!(matches!(
        session.poll(&env).unwrap(),
        ParallelPoll::Results(_)
    ));
}

#[test]
fn early_unknown_duplicate_and_terminal_replies_run_no_native_hooks() {
    let (c, env, authority) = fixture();
    let node = group(3, &c);
    let mut session = Session::start(&node, &env, Fuel::new(100), LIMITS).unwrap();
    let events = c.events.borrow().clone();
    assert!(matches!(
        session
            .try_accept::<ScalarValue, _>(
                0,
                &key(0, &c),
                reply(0, ScalarValue::Integer(1), &c),
                &authority
            )
            .unwrap_err()
            .error,
        ParallelReplyError::NotAwaiting(ParallelStatus::Ready)
    ));
    assert_eq!(*c.events.borrow(), events);
    drop(requests(&mut session, &env));
    accept(&mut session, &c, &authority, 1);
    let events = c.events.borrow().clone();
    for index in [1, 3, usize::MAX] {
        let received = reply(index, ScalarValue::Integer(1), &c);
        let pointer = received.buffer.as_ptr();
        let rejected = session
            .try_accept::<ScalarValue, _>(index, &key(index, &c), received, &authority)
            .unwrap_err();
        assert!(
            matches!(rejected.error, ParallelReplyError::MemberNotAwaiting { index: at } if at == index)
        );
        assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
    }
    assert_eq!(*c.events.borrow(), events);
    assert_eq!(session.status(), ParallelStatus::Awaiting { remaining: 2 });
    accept(&mut session, &c, &authority, 0);
    accept(&mut session, &c, &authority, 2);
    for completed in [false, true] {
        if completed {
            drop(session.poll(&env).unwrap());
        }
        let events = c.events.borrow().clone();
        assert!(matches!(
            session
                .try_accept::<ScalarValue, _>(
                    0,
                    &key(0, &c),
                    reply(0, ScalarValue::Integer(1), &c),
                    &authority
                )
                .unwrap_err()
                .error,
            ParallelReplyError::NotAwaiting(_)
        ));
        assert_eq!(*c.events.borrow(), events);
    }
}

#[test]
fn cancellation_discards_pending_and_accepted_results_before_any_handoff() {
    for accepted in 0..=3 {
        let (c, env, authority) = fixture();
        let node = group(3, &c);
        let mut session = Session::start(&node, &env, Fuel::new(100), LIMITS).unwrap();
        drop(requests(&mut session, &env));
        for index in 0..accepted {
            accept(&mut session, &c, &authority, index);
        }
        let events = c.events.borrow().clone();
        assert_eq!(c.replies.get(), 0);
        assert!(session.cancel());
        assert_eq!(c.keys.get(), 3);
        assert_eq!(c.replies.get(), accepted);
        assert!(!session.cancel());
        assert!(matches!(
            session.poll(&env).unwrap(),
            ParallelPoll::Terminal(ParallelEnd::Cancelled)
        ));
        assert!(
            session
                .try_accept::<ScalarValue, _>(
                    2,
                    &key(2, &c),
                    reply(2, ScalarValue::Integer(1), &c),
                    &authority
                )
                .is_err()
        );
        assert_eq!(*c.events.borrow(), events);
    }
    let (c, env, _) = fixture();
    let node = group(3, &c);
    let mut session = Session::start(&node, &env, Fuel::new(100), LIMITS).unwrap();
    assert!(session.cancel());
    assert!(matches!(
        session.poll(&env).unwrap(),
        ParallelPoll::Terminal(ParallelEnd::Cancelled)
    ));
    assert_eq!(session.fuel_remaining(), 99);
    assert_eq!(c.requests.get(), 0);
}

#[test]
fn preparation_failure_or_fuel_exhaustion_never_publish_a_partial_batch_or_retry() {
    for (fuel, fail, remaining) in [(100, Some(("prepare", 1)), 93), (4, None, 0)] {
        let (c, env, _) = fixture();
        let node = group(3, &c);
        let mut session = Session::start(&node, &env, Fuel::new(fuel), LIMITS).unwrap();
        c.fail.set(fail);
        assert!(session.poll(&env).is_err());
        assert_eq!(c.requests.get(), 1);
        assert_eq!(c.keys.get(), 1);
        assert_eq!(
            session.status(),
            ParallelStatus::Terminal(ParallelEnd::Failed)
        );
        assert_eq!(session.fuel_remaining(), remaining);
        let events = c.events.borrow().clone();
        assert!(matches!(
            session.poll(&env).unwrap(),
            ParallelPoll::Terminal(ParallelEnd::Failed)
        ));
        assert!(!session.cancel());
        assert_eq!(*c.events.borrow(), events);
    }
}

#[test]
fn duplicate_identity_refuses_the_complete_batch_before_publication() {
    let (c, env, _) = fixture();
    let node = group(3, &c);
    c.duplicate.set(true);
    let mut session = Session::start(&node, &env, Fuel::new(100), LIMITS).unwrap();
    assert!(matches!(
        session.poll(&env),
        Err(ParallelEvaluationError::DuplicateIdentity {
            index: 1,
            previous: 0
        })
    ));
    assert_eq!(c.requests.get(), 3);
    assert_eq!(c.keys.get(), 3);
    assert_eq!(session.fuel_remaining(), 90);
    assert_eq!(
        session.status(),
        ParallelStatus::Terminal(ParallelEnd::Failed)
    );
}

#[test]
fn preparation_identity_and_prefix_cleanup_unwind_close_without_rearming() {
    for failure in 0..3 {
        let (c, env, _) = fixture();
        let node = group(3, &c);
        let mut session = Session::start(&node, &env, Fuel::new(100), LIMITS).unwrap();
        match failure {
            0 => c.unwind.set(Some(("prepare", 1))),
            1 => c.unwind.set(Some(("identity", 1))),
            2 => {
                c.fail.set(Some(("prepare", 1)));
                c.panic_key_drop.set(true);
            }
            _ => unreachable!(),
        }
        assert!(catch_unwind(AssertUnwindSafe(|| session.poll(&env))).is_err());
        assert_eq!(
            session.status(),
            ParallelStatus::Terminal(ParallelEnd::HostUncertain)
        );
        assert_eq!(c.keys.get(), if failure == 1 { 3 } else { 1 });
        assert_eq!(c.requests.get(), if failure == 1 { 3 } else { 1 });
        let events = c.events.borrow().clone();
        assert!(matches!(
            session.poll(&env).unwrap(),
            ParallelPoll::Terminal(ParallelEnd::HostUncertain)
        ));
        assert!(!session.cancel());
        assert_eq!(*c.events.borrow(), events);
    }
}

#[test]
fn every_reply_callback_unwind_discards_sibling_barrier_and_never_rearms() {
    for phase in ["identity", "policy", "borrow", "type", "value"] {
        let (c, env, authority) = fixture();
        let node = group(3, &c);
        let mut session = Session::start(&node, &env, Fuel::new(100), LIMITS).unwrap();
        drop(requests(&mut session, &env));
        accept(&mut session, &c, &authority, 0);
        c.unwind.set(Some((phase, 1)));
        assert!(
            catch_unwind(AssertUnwindSafe(|| session.try_accept::<ScalarValue, _>(
                1,
                &key(1, &c),
                reply(1, ScalarValue::Integer(1), &c),
                &authority
            )))
            .is_err()
        );
        assert_eq!(
            session.status(),
            ParallelStatus::Terminal(ParallelEnd::HostUncertain)
        );
        assert_eq!(c.keys.get(), 3);
        assert_eq!(c.replies.get(), 2);
        let events = c.events.borrow().clone();
        assert!(!session.cancel());
        assert!(matches!(
            session.poll(&env).unwrap(),
            ParallelPoll::Terminal(ParallelEnd::HostUncertain)
        ));
        assert_eq!(*c.events.borrow(), events);
        assert_eq!(session.fuel_remaining(), 90);
    }
}

#[test]
fn cancellation_cleanup_unwind_preserves_cancelled_reason_and_releases_siblings() {
    let (c, env, authority) = fixture();
    let node = group(3, &c);
    let mut session = Session::start(&node, &env, Fuel::new(100), LIMITS).unwrap();
    drop(requests(&mut session, &env));
    accept(&mut session, &c, &authority, 2);
    c.panic_key_drop.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| session.cancel())).is_err());
    assert_eq!(
        session.status(),
        ParallelStatus::Terminal(ParallelEnd::Cancelled)
    );
    assert_eq!(c.keys.get(), 3);
    assert_eq!(c.replies.get(), 1);
    assert!(!session.cancel());
}

#[test]
fn dropping_an_incomplete_session_releases_owned_barrier_without_consuming_ir() {
    let (c, env, authority) = fixture();
    let node = group(3, &c);
    let mut session = Session::start(&node, &env, Fuel::new(100), LIMITS).unwrap();
    drop(requests(&mut session, &env));
    accept(&mut session, &c, &authority, 1);
    drop(session);
    assert_eq!(c.keys.get(), 3);
    assert_eq!(c.replies.get(), 1);
    assert_eq!(c.tokens.get(), 0);
    drop(node);
    assert_eq!(c.tokens.get(), 3);
}

#[test]
fn maximum_width_and_inclusive_physical_limits_keep_one_owned_fuel_meter() {
    let (c, env, authority) = fixture();
    let node = group(64, &c);
    let mut session = Session::start(
        &node,
        &env,
        Fuel::new(200),
        ParallelEvaluationLimits {
            max_nodes: 65,
            max_depth: 1,
            max_branches: 64,
        },
    )
    .unwrap();
    assert_eq!(requests(&mut session, &env).len(), 64);
    assert_eq!(session.fuel_remaining(), 7);
    for index in (0..64).rev() {
        accept(&mut session, &c, &authority, index);
    }
    let ParallelPoll::Results(results) = session.poll(&env).unwrap() else {
        panic!()
    };
    assert_eq!(results.len(), 64);
    drop(results);
    assert_eq!(c.keys.get(), 64);
    assert_eq!(c.replies.get(), 64);
    assert_eq!(session.fuel_remaining(), 7);
}

#[test]
fn debug_and_native_errors_never_format_private_payloads_or_invoke_hooks() {
    let (c, env, _) = fixture();
    let node = group(3, &c);
    let mut session = Session::start(&node, &env, Fuel::new(100), LIMITS).unwrap();
    let batch = requests(&mut session, &env);
    let events = c.events.borrow().clone();
    let error: ParallelEvaluationError<Private> = ParallelEvaluationError::Native {
        index: 1,
        phase: ParallelPhase::Prepare,
        error: Private("private secret"),
    };
    assert!(!format!("{error:?} {error} {session:?} {:?}", batch[0]).contains("private"));
    assert!(std::error::Error::source(&error).is_none());
    assert_eq!(*c.events.borrow(), events);
}

#[test]
fn unsized_native_result_domains_and_borrowed_text_views_need_no_native_clones() {
    struct TextDomain(usize);
    impl HostResultDomain<str> for TextDomain {
        type Error = ();
        fn matches_type(&self, _: &str) -> bool {
            true
        }
        fn validate_value(&self, value: &str) -> Result<(), ()> {
            if value.len() <= self.0 {
                Ok(())
            } else {
                Err(())
            }
        }
    }
    struct TextEnvironment;
    type TextNode = Computation<(), (), (), TextDomain>;
    impl<'a> ParallelEvaluationEnvironment<'a, (), (), (), TextDomain> for TextEnvironment {
        type Identity = usize;
        type Declaration = dyn HostResultDomain<str, Error = ()>;
        type Request = usize;
        type Error = ();
        fn preflight_member(
            &self,
            _: usize,
            _: &'a ComputedBranch<TextNode, TextDomain>,
        ) -> Result<(), ()> {
            Ok(())
        }
        fn prepare_member(
            &self,
            index: usize,
            branch: &'a ComputedBranch<TextNode, TextDomain>,
            _: &mut Fuel,
        ) -> ParallelPreparation<'a, usize, Self::Declaration, usize, ()> {
            Ok(PreparedParallelMember {
                identity: index,
                declaration: &branch.result_type,
                request: index,
            })
        }
    }
    struct TextAuthority;
    impl ReplyAuthority<usize> for TextAuthority {
        type Error = ();
        fn authorize(&self, _: &usize) -> Result<(), ()> {
            Ok(())
        }
    }
    let node: TextNode = Computation::Group {
        group_kind: GroupKind::Parallel,
        branches: (0..2)
            .map(|i| ComputedBranch {
                name: format!("text_{i}"),
                value: Computation::Host {
                    effect: Box::new(()),
                },
                result_type: TextDomain(4),
            })
            .collect(),
    };
    let mut session: ParallelEvaluation<
        '_,
        (),
        (),
        (),
        TextDomain,
        usize,
        dyn HostResultDomain<str, Error = ()>,
        String,
    > = ParallelEvaluation::start(&node, &TextEnvironment, Fuel::new(10), LIMITS).unwrap();
    assert!(matches!(
        session.poll(&TextEnvironment).unwrap(),
        ParallelPoll::Requests(_)
    ));
    let text = "long value".to_string();
    let pointer = text.as_ptr();
    let rejected = session
        .try_accept::<str, _>(1, &1, text, &TextAuthority)
        .unwrap_err();
    assert_eq!(rejected.reply.as_ptr(), pointer);
    session
        .try_accept::<str, _>(1, &1, "yes".into(), &TextAuthority)
        .unwrap();
    session
        .try_accept::<str, _>(0, &0, "no".into(), &TextAuthority)
        .unwrap();
    let ParallelPoll::Results(results) = session.poll(&TextEnvironment).unwrap() else {
        panic!()
    };
    let mut fuel = session.into_completed_fuel().unwrap();
    let group = leselang_hir::group_reply::assemble_group_reply(
        &node,
        results,
        LIMITS,
        &mut fuel,
        |reply, _| {
            for member in reply.members() {
                let (_, _, key, declaration, value) = member.as_parts();
                TextAuthority.authorize(key)?;
                declaration.validate_value(value.as_str())?;
            }
            Ok::<_, ()>(())
        },
    )
    .unwrap();
    let selected = group
        .lookup(64, "text_1", |member| {
            let (_, _, key, declaration, value) = member.as_parts();
            TextAuthority.authorize(key)?;
            declaration.validate_value(value.as_str())?;
            Ok::<_, ()>(true)
        })
        .unwrap()
        .unwrap();
    assert_eq!(selected.as_parts().4, "yes");
    assert_eq!(fuel.remaining(), 4);
    assert_eq!(
        group
            .into_members()
            .into_iter()
            .map(|r| r.into_parts().4)
            .collect::<Vec<_>>(),
        ["no", "yes"]
    );
}
