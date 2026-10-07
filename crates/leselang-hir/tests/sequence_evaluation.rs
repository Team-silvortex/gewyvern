use std::borrow::Borrow;
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::ir::{Computation, ComputedBranch, GroupKind};
use leselang_hir::sequence_evaluation::*;
use leselang_runtime_core::{
    Fuel, HostResultDomain, ReplyAcceptanceError, ReplyAuthority, ScalarValue,
};

struct PrivateError(&'static str);
#[derive(Default)]
struct Control {
    events: RefCell<Vec<(&'static str, usize)>>,
    panic_at: Cell<Option<&'static str>>,
    fail_at: Cell<Option<(&'static str, usize)>>,
    identity_drops: Cell<usize>,
    request_drops: Cell<usize>,
    reply_drops: Cell<usize>,
    token_drops: Cell<usize>,
    panic_drop: Cell<bool>,
}
impl Control {
    fn visit(&self, phase: &'static str, index: usize) -> Result<(), PrivateError> {
        self.events.borrow_mut().push((phase, index));
        assert_ne!(self.panic_at.get(), Some(phase), "private native unwind");
        if self.fail_at.get() == Some((phase, index)) {
            Err(PrivateError("private secret"))
        } else {
            Ok(())
        }
    }
}
struct Token {
    index: usize,
    bytes: Vec<u8>,
    control: Rc<Control>,
}
impl Drop for Token {
    fn drop(&mut self) {
        self.control
            .token_drops
            .set(self.control.token_drops.get() + 1);
    }
}
struct Domain {
    index: usize,
    control: Rc<Control>,
}
impl HostResultDomain<ScalarValue> for Domain {
    type Error = PrivateError;
    fn matches_type(&self, value: &ScalarValue) -> bool {
        self.control
            .visit("type", self.index)
            .unwrap_or_else(|_| panic!());
        matches!(value, ScalarValue::Integer(_))
    }
    fn validate_value(&self, value: &ScalarValue) -> Result<(), PrivateError> {
        self.control.visit("value", self.index)?;
        match value {
            ScalarValue::Integer(value) if *value <= 100 => Ok(()),
            _ => Err(PrivateError("private domain")),
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
            self.control
                .identity_drops
                .set(self.control.identity_drops.get() + 1);
            assert!(
                !self.control.panic_drop.replace(false),
                "private identity cleanup"
            );
        }
    }
}
struct Request {
    buffer: Vec<u8>,
    original: *const Token,
    control: Rc<Control>,
}
impl Drop for Request {
    fn drop(&mut self) {
        self.control
            .request_drops
            .set(self.control.request_drops.get() + 1);
    }
}
struct Reply {
    value: ScalarValue,
    buffer: Vec<u8>,
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
        self.control
            .reply_drops
            .set(self.control.reply_drops.get() + 1);
    }
}
struct Environment {
    control: Rc<Control>,
}
struct Authority {
    control: Rc<Control>,
    live: bool,
}
impl ReplyAuthority<Key> for Authority {
    type Error = PrivateError;
    fn authorize(&self, key: &Key) -> Result<(), PrivateError> {
        self.control.visit("policy", key.index)?;
        if self.live && key.owner == 7 {
            Ok(())
        } else {
            Err(PrivateError("revoked"))
        }
    }
}
type Node = Computation<Token, Token, Token, Domain>;
type Branch = ComputedBranch<Node, Domain>;
type Session<'a> = SequenceEvaluation<'a, Token, Token, Token, Domain, Key, Domain>;
const LIMITS: SequenceEvaluationLimits = SequenceEvaluationLimits {
    max_nodes: 256,
    max_depth: 32,
    max_branches: 64,
};
impl<'a> SequenceEvaluationEnvironment<'a, Token, Token, Token, Domain> for Environment {
    type Identity = Key;
    type Declaration = Domain;
    type Request = Request;
    type Error = PrivateError;
    fn preflight_member(&self, index: usize, branch: &'a Branch) -> Result<(), PrivateError> {
        self.control.visit("preflight", index)?;
        if let Node::Host { effect } = &branch.value
            && effect.index == branch.result_type.index
        {
            Ok(())
        } else {
            Err(PrivateError("wrong original domain"))
        }
    }
    fn prepare_member(
        &self,
        index: usize,
        branch: &'a Branch,
        fuel: &mut Fuel,
    ) -> Result<PreparedSequenceMember<'a, Key, Domain, Request>, PrivateError> {
        fuel.charge(2).map_err(|_| PrivateError("native fuel"))?;
        self.control.visit("prepare", index)?;
        let Node::Host { effect } = &branch.value else {
            return Err(PrivateError("not registered atomic"));
        };
        if effect.index != branch.result_type.index {
            return Err(PrivateError("live domain changed"));
        }
        Ok(PreparedSequenceMember {
            identity: Key {
                owner: 7,
                index,
                owned: true,
                control: self.control.clone(),
            },
            declaration: &branch.result_type,
            request: Request {
                buffer: effect.bytes.clone(),
                original: effect.as_ref(),
                control: self.control.clone(),
            },
        })
    }
}
fn group(count: usize, control: &Rc<Control>) -> Node {
    Node::Group {
        group_kind: GroupKind::Sequence,
        branches: (0..count)
            .map(|index| Branch {
                name: format!("step_{index}"),
                value: Node::Host {
                    effect: Box::new(Token {
                        index,
                        bytes: vec![3; 33],
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
fn key(index: usize, control: &Rc<Control>) -> Key {
    Key {
        owner: 7,
        index,
        owned: false,
        control: control.clone(),
    }
}
fn reply(value: ScalarValue, control: &Rc<Control>) -> Reply {
    Reply {
        value,
        buffer: vec![9; 37],
        control: control.clone(),
    }
}
fn fixture() -> (Rc<Control>, Environment, Authority) {
    let control = Rc::new(Control::default());
    (
        control.clone(),
        Environment {
            control: control.clone(),
        },
        Authority {
            control,
            live: true,
        },
    )
}
fn request(session: &mut Session<'_>, env: &Environment, expected: usize) -> Request {
    let SequencePoll::Request { index, request } = session.poll(env).unwrap() else {
        panic!()
    };
    assert_eq!(index, expected);
    request
}
fn accept(session: &mut Session<'_>, control: &Rc<Control>, authority: &Authority, index: usize) {
    let accepted = session
        .try_accept::<_, ScalarValue, _>(
            &key(index, control),
            reply(ScalarValue::Integer(42), control),
            authority,
        )
        .unwrap();
    let (position, branch, identity, domain, result) = accepted.into_parts();
    assert_eq!(position, index);
    assert_eq!(identity.index, index);
    assert!(std::ptr::eq(domain, &branch.result_type));
    assert_eq!(result.value, ScalarValue::Integer(42));
}

#[test]
fn cold_shape_limits_and_candidates_precede_all_native_hooks() {
    let (control, environment, _) = fixture();
    let valid = group(2, &control);
    for limits in [
        SequenceEvaluationLimits {
            max_nodes: 16_385,
            ..LIMITS
        },
        SequenceEvaluationLimits {
            max_depth: 65,
            ..LIMITS
        },
        SequenceEvaluationLimits {
            max_branches: 65,
            ..LIMITS
        },
        SequenceEvaluationLimits {
            max_nodes: 0,
            ..LIMITS
        },
        SequenceEvaluationLimits {
            max_depth: 0,
            ..LIMITS
        },
        SequenceEvaluationLimits {
            max_branches: 0,
            ..LIMITS
        },
        SequenceEvaluationLimits {
            max_nodes: 2,
            ..LIMITS
        },
    ] {
        assert!(Session::start(&valid, &environment, Fuel::new(100), limits).is_err());
    }
    for mode in 0..7 {
        let mut invalid = group(if mode == 0 { 0 } else { 2 }, &control);
        let Node::Group {
            group_kind,
            branches,
        } = &mut invalid
        else {
            panic!()
        };
        match mode {
            1 => *group_kind = GroupKind::Parallel,
            2 => branches[1].name = branches[0].name.clone(),
            3 => branches[1].name = "bad name".into(),
            4 => branches[1].name = "a".repeat(65),
            5 => branches[1].value = group(1, &control),
            6 => {
                branches[1].value = Node::Literal {
                    value: ScalarValue::Integer(1),
                }
            }
            _ => {}
        }
        assert!(Session::start(&invalid, &environment, Fuel::new(100), LIMITS).is_err());
    }
    assert!(control.events.borrow().is_empty());
}

#[test]
fn inclusive_total_nodes_depth_and_sixty_four_members_are_bounded() {
    let (control, env, authority) = fixture();
    let input = group(64, &control);
    let limits = SequenceEvaluationLimits {
        max_nodes: 65,
        max_depth: 1,
        max_branches: 64,
    };
    assert!(
        Session::start(
            &input,
            &env,
            Fuel::new(1000),
            SequenceEvaluationLimits {
                max_nodes: 64,
                ..limits
            }
        )
        .is_err()
    );
    assert!(control.events.borrow().is_empty());
    let mut session = Session::start(&input, &env, Fuel::new(1000), limits).unwrap();
    for index in 0..64 {
        drop(request(&mut session, &env, index));
        accept(&mut session, &control, &authority, index);
    }
    assert_eq!(
        session.status(),
        SequenceStatus::Terminal(SequenceEnd::Completed)
    );
    assert_eq!(session.fuel_remaining(), 807);
    assert_eq!(control.identity_drops.get(), 64);
}

#[test]
fn every_cold_row_is_checked_before_first_preparation_and_native_errors_are_redacted() {
    let (control, env, _) = fixture();
    let input = group(2, &control);
    control.fail_at.set(Some(("preflight", 1)));
    let error = Session::start(&input, &env, Fuel::new(100), LIMITS).unwrap_err();
    assert!(matches!(
        error,
        SequenceEvaluationError::Native {
            index: 1,
            phase: SequencePhase::Preflight,
            error: PrivateError("private secret")
        }
    ));
    assert_eq!(
        *control.events.borrow(),
        [("preflight", 0), ("preflight", 1)]
    );
    assert!(!format!("{error:?} {error}").contains("private secret"));
    assert!(std::error::Error::source(&error).is_none());
}

#[test]
fn move_only_native_rows_replies_and_requests_preserve_pointers_without_eager_work() {
    let (control, env, authority) = fixture();
    let input = group(2, &control);
    let Node::Group { branches, .. } = &input else {
        panic!()
    };
    let mut session = Session::start(&input, &env, Fuel::new(100), LIMITS).unwrap();
    assert_eq!(session.fuel_remaining(), 99);
    let first = request(&mut session, &env, 0);
    let Node::Host { effect } = &branches[0].value else {
        panic!()
    };
    assert_eq!(first.original, effect.as_ref() as *const Token);
    assert_eq!(first.buffer, effect.bytes);
    let events = control.events.borrow().clone();
    for _ in 0..3 {
        assert!(matches!(
            session.poll(&env).unwrap(),
            SequencePoll::Awaiting { index: 0 }
        ));
    }
    assert_eq!(*control.events.borrow(), events);
    assert_eq!(session.fuel_remaining(), 96);
    let value = reply(ScalarValue::Integer(42), &control);
    let buffer = value.buffer.as_ptr();
    let accepted = session
        .try_accept::<_, ScalarValue, _>(&key(0, &control), value, &authority)
        .unwrap();
    let (index, original, identity, domain, value) = accepted.into_parts();
    assert_eq!(index, 0);
    assert!(std::ptr::eq(original, &branches[0]));
    assert!(std::ptr::eq(domain, &branches[0].result_type));
    assert_eq!(value.buffer.as_ptr(), buffer);
    assert_eq!(session.status(), SequenceStatus::Ready { index: 1 });
    assert_eq!(session.fuel_remaining(), 96);
    drop((first, identity, value));
    drop(request(&mut session, &env, 1));
    accept(&mut session, &control, &authority, 1);
    assert_eq!(session.fuel_remaining(), 93);
    assert_eq!(
        session.status(),
        SequenceStatus::Terminal(SequenceEnd::Completed)
    );
    let events = control.events.borrow().clone();
    assert!(matches!(
        session.poll(&env).unwrap(),
        SequencePoll::Terminal(SequenceEnd::Completed)
    ));
    assert!(!session.cancel());
    assert_eq!(*control.events.borrow(), events);
    assert_eq!(control.request_drops.get(), 2);
    assert_eq!(control.identity_drops.get(), 2);
    assert_eq!(control.reply_drops.get(), 2);
    assert_eq!(control.token_drops.get(), 0);
    drop(session);
    drop(input);
    assert_eq!(control.token_drops.get(), 2);
}

#[test]
fn rejected_identity_authority_type_and_value_preserve_input_and_pending_member() {
    let (control, env, authority) = fixture();
    let input = group(2, &control);
    let mut session = Session::start(&input, &env, Fuel::new(100), LIMITS).unwrap();
    drop(request(&mut session, &env, 0));
    for mode in 0..4 {
        control.events.borrow_mut().clear();
        let actual = key(if mode == 0 { 1 } else { 0 }, &control);
        let policy = Authority {
            control: control.clone(),
            live: mode != 1,
        };
        let value = reply(
            if mode == 2 {
                ScalarValue::Boolean(true)
            } else {
                ScalarValue::Integer(if mode == 3 { 101 } else { 42 })
            },
            &control,
        );
        let buffer = value.buffer.as_ptr();
        let rejected = session
            .try_accept::<_, ScalarValue, _>(&actual, value, &policy)
            .unwrap_err();
        assert_eq!(rejected.reply.buffer.as_ptr(), buffer);
        assert_eq!(session.status(), SequenceStatus::Awaiting { index: 0 });
        assert_eq!(session.fuel_remaining(), 96);
        let expected = ["identity", "policy", "borrow", "type", "value"];
        let count = [1, 2, 4, 5][mode];
        assert_eq!(
            control
                .events
                .borrow()
                .iter()
                .map(|event| event.0)
                .collect::<Vec<_>>(),
            &expected[..count]
        );
        assert!(!format!("{rejected:?}").contains("private"));
    }
    assert_eq!(control.identity_drops.get(), 0);
    accept(&mut session, &control, &authority, 0);
}

#[test]
fn old_or_premature_replies_cannot_advance_or_prepare_successors() {
    let (control, env, authority) = fixture();
    let input = group(2, &control);
    let mut session = Session::start(&input, &env, Fuel::new(100), LIMITS).unwrap();
    control.events.borrow_mut().clear();
    assert!(matches!(
        session
            .try_accept::<_, ScalarValue, _>(
                &key(0, &control),
                reply(ScalarValue::Integer(1), &control),
                &authority
            )
            .unwrap_err()
            .error,
        SequenceReplyError::NotAwaiting(SequenceStatus::Ready { index: 0 })
    ));
    assert!(control.events.borrow().is_empty());
    drop(request(&mut session, &env, 0));
    accept(&mut session, &control, &authority, 0);
    control.events.borrow_mut().clear();
    assert!(matches!(
        session
            .try_accept::<_, ScalarValue, _>(
                &key(0, &control),
                reply(ScalarValue::Integer(1), &control),
                &authority
            )
            .unwrap_err()
            .error,
        SequenceReplyError::NotAwaiting(SequenceStatus::Ready { index: 1 })
    ));
    assert!(control.events.borrow().is_empty());
    drop(request(&mut session, &env, 1));
    assert!(matches!(
        session
            .try_accept::<_, ScalarValue, _>(
                &key(0, &control),
                reply(ScalarValue::Integer(1), &control),
                &authority
            )
            .unwrap_err()
            .error,
        SequenceReplyError::Reply(ReplyAcceptanceError::IdentityMismatch)
    ));
    assert_eq!(session.status(), SequenceStatus::Awaiting { index: 1 });
    accept(&mut session, &control, &authority, 1);
    control.events.borrow_mut().clear();
    assert!(
        session
            .try_accept::<_, ScalarValue, _>(
                &key(1, &control),
                reply(ScalarValue::Integer(1), &control),
                &authority
            )
            .is_err()
    );
    assert!(control.events.borrow().is_empty());
}

#[test]
fn cancel_ready_or_waiting_closes_once_without_preparing_or_accepting_late_replies() {
    for waiting in [false, true] {
        let (control, env, authority) = fixture();
        let input = group(2, &control);
        let mut session = Session::start(&input, &env, Fuel::new(100), LIMITS).unwrap();
        let outbound = waiting.then(|| request(&mut session, &env, 0));
        assert!(session.cancel());
        assert!(!session.cancel());
        assert_eq!(control.identity_drops.get(), usize::from(waiting));
        control.events.borrow_mut().clear();
        assert!(matches!(
            session.poll(&env).unwrap(),
            SequencePoll::Terminal(SequenceEnd::Cancelled)
        ));
        assert!(
            session
                .try_accept::<_, ScalarValue, _>(
                    &key(0, &control),
                    reply(ScalarValue::Integer(1), &control),
                    &authority
                )
                .is_err()
        );
        assert!(control.events.borrow().is_empty());
        assert_eq!(control.request_drops.get(), 0);
        drop(outbound);
        assert_eq!(control.request_drops.get(), usize::from(waiting));
    }
}

#[test]
fn preparation_failure_or_unwind_is_terminal_without_fuel_refund_or_retry() {
    for unwind in [false, true] {
        let (control, env, _) = fixture();
        let input = group(2, &control);
        let mut session = Session::start(&input, &env, Fuel::new(100), LIMITS).unwrap();
        if unwind {
            control.panic_at.set(Some("prepare"));
            assert!(catch_unwind(AssertUnwindSafe(|| session.poll(&env))).is_err());
        } else {
            control.fail_at.set(Some(("prepare", 0)));
            assert!(matches!(
                session.poll(&env),
                Err(SequenceEvaluationError::Native {
                    phase: SequencePhase::Prepare,
                    index: 0,
                    ..
                })
            ));
        }
        let end = if unwind {
            SequenceEnd::HostUncertain
        } else {
            SequenceEnd::Failed
        };
        assert_eq!(session.status(), SequenceStatus::Terminal(end));
        assert_eq!(session.fuel_remaining(), 96);
        let events = control.events.borrow().clone();
        assert!(
            matches!(session.poll(&env).unwrap(), SequencePoll::Terminal(reason) if reason == end)
        );
        assert!(!session.cancel());
        assert_eq!(*control.events.borrow(), events);
    }
}

#[test]
fn fuel_is_one_owned_meter_across_waits_replies_and_successors() {
    let (control, env, authority) = fixture();
    let input = group(2, &control);
    assert!(matches!(
        Session::start(&input, &env, Fuel::new(0), LIMITS),
        Err(SequenceEvaluationError::Fuel)
    ));
    let mut session = Session::start(&input, &env, Fuel::new(1), LIMITS).unwrap();
    control.events.borrow_mut().clear();
    assert!(matches!(
        session.poll(&env),
        Err(SequenceEvaluationError::Fuel)
    ));
    assert!(control.events.borrow().is_empty());
    assert_eq!(session.fuel_remaining(), 0);
    let mut session = Session::start(&input, &env, Fuel::new(3), LIMITS).unwrap();
    assert!(matches!(
        session.poll(&env),
        Err(SequenceEvaluationError::Native { .. })
    ));
    assert_eq!(session.fuel_remaining(), 1);
    let mut session = Session::start(&input, &env, Fuel::new(4), LIMITS).unwrap();
    drop(request(&mut session, &env, 0));
    assert_eq!(session.fuel_remaining(), 0);
    accept(&mut session, &control, &authority, 0);
    assert!(matches!(
        session.poll(&env),
        Err(SequenceEvaluationError::Fuel)
    ));
    assert_eq!(
        session.status(),
        SequenceStatus::Terminal(SequenceEnd::Failed)
    );
    assert_eq!(session.fuel_remaining(), 0);
}

#[test]
fn every_reply_callback_unwind_releases_identity_and_closes_without_rearming() {
    for phase in ["identity", "policy", "borrow", "type", "value"] {
        let (control, env, authority) = fixture();
        let input = group(2, &control);
        let mut session = Session::start(&input, &env, Fuel::new(100), LIMITS).unwrap();
        drop(request(&mut session, &env, 0));
        control.panic_at.set(Some(phase));
        assert!(
            catch_unwind(AssertUnwindSafe(|| session
                .try_accept::<_, ScalarValue, _>(
                    &key(0, &control),
                    reply(ScalarValue::Integer(42), &control),
                    &authority
                )))
            .is_err()
        );
        assert_eq!(
            session.status(),
            SequenceStatus::Terminal(SequenceEnd::HostUncertain)
        );
        assert_eq!(control.identity_drops.get(), 1);
        assert_eq!(control.reply_drops.get(), 1);
        assert_eq!(session.fuel_remaining(), 96);
        control.events.borrow_mut().clear();
        assert!(
            session
                .try_accept::<_, ScalarValue, _>(
                    &key(0, &control),
                    reply(ScalarValue::Integer(42), &control),
                    &authority
                )
                .is_err()
        );
        assert!(matches!(
            session.poll(&env).unwrap(),
            SequencePoll::Terminal(SequenceEnd::HostUncertain)
        ));
        assert!(control.events.borrow().is_empty());
        assert!(!session.cancel());
    }
}

#[test]
fn cancellation_destructor_unwind_preserves_closed_reason_and_no_second_cleanup() {
    let (control, env, _) = fixture();
    let input = group(2, &control);
    let mut session = Session::start(&input, &env, Fuel::new(100), LIMITS).unwrap();
    let outbound = request(&mut session, &env, 0);
    control.panic_drop.set(true);
    assert!(catch_unwind(AssertUnwindSafe(|| session.cancel())).is_err());
    assert_eq!(
        session.status(),
        SequenceStatus::Terminal(SequenceEnd::Cancelled)
    );
    assert!(!session.cancel());
    assert_eq!(control.identity_drops.get(), 1);
    assert_eq!(control.request_drops.get(), 0);
    drop(outbound);
    drop(session);
    assert_eq!(control.identity_drops.get(), 1);
    assert_eq!(control.request_drops.get(), 1);
}

#[test]
fn metadata_debug_and_errors_never_format_native_slots_or_payloads() {
    let (control, env, _) = fixture();
    let input = group(2, &control);
    let mut session = Session::start(&input, &env, Fuel::new(100), LIMITS).unwrap();
    assert_eq!(
        format!("{session:?}"),
        "SequenceEvaluation { status: Ready { index: 0 }, members: 2, fuel: 99 }"
    );
    let prepared = session.poll(&env).unwrap();
    assert_eq!(format!("{prepared:?}"), "Request");
    assert_eq!(
        format!("{session:?}"),
        "SequenceEvaluation { status: Awaiting { index: 0 }, members: 2, fuel: 96 }"
    );
    assert!(session.cancel());
    drop(prepared);
}

#[test]
fn unsized_native_domain_and_text_reply_view_need_no_cloning_or_serialization() {
    struct TextDomain;
    impl HostResultDomain<str> for TextDomain {
        type Error = PrivateError;
        fn matches_type(&self, _: &str) -> bool {
            true
        }
        fn validate_value(&self, value: &str) -> Result<(), PrivateError> {
            if value.is_empty() || value.len() > 16 {
                Err(PrivateError("private text"))
            } else {
                Ok(())
            }
        }
    }
    type TextNode = Computation<(), (), u8, TextDomain>;
    struct TextEnvironment;
    impl<'a> SequenceEvaluationEnvironment<'a, (), (), u8, TextDomain> for TextEnvironment {
        type Identity = (u64, usize);
        type Declaration = dyn HostResultDomain<str, Error = PrivateError>;
        type Request = u8;
        type Error = PrivateError;
        fn preflight_member(
            &self,
            _: usize,
            branch: &'a ComputedBranch<TextNode, TextDomain>,
        ) -> Result<(), PrivateError> {
            if matches!(&branch.value, TextNode::Host { effect } if **effect == 9) {
                Ok(())
            } else {
                Err(PrivateError("not registered text host"))
            }
        }
        fn prepare_member(
            &self,
            index: usize,
            branch: &'a ComputedBranch<TextNode, TextDomain>,
            fuel: &mut Fuel,
        ) -> SequencePreparation<'a, Self::Identity, Self::Declaration, Self::Request, Self::Error>
        {
            self.preflight_member(index, branch)?;
            fuel.charge(1).map_err(|_| PrivateError("text fuel"))?;
            Ok(PreparedSequenceMember {
                identity: (316, index),
                declaration: &branch.result_type,
                request: 9,
            })
        }
    }
    struct TextPolicy;
    impl ReplyAuthority<(u64, usize)> for TextPolicy {
        type Error = PrivateError;
        fn authorize(&self, &(owner, index): &(u64, usize)) -> Result<(), PrivateError> {
            if owner == 316 && index == 0 {
                Ok(())
            } else {
                Err(PrivateError("wrong text generation"))
            }
        }
    }
    let input = TextNode::Group {
        group_kind: GroupKind::Sequence,
        branches: vec![ComputedBranch {
            name: "text".into(),
            value: TextNode::Host {
                effect: Box::new(9),
            },
            result_type: TextDomain,
        }],
    };
    let mut session =
        SequenceEvaluation::start(&input, &TextEnvironment, Fuel::new(10), LIMITS).unwrap();
    assert!(matches!(
        session.poll(&TextEnvironment).unwrap(),
        SequencePoll::Request {
            index: 0,
            request: 9
        }
    ));
    assert!(
        session
            .try_accept::<_, str, _>(&(316, 0), String::new(), &TextPolicy)
            .is_err()
    );
    let text = "receipt".to_string();
    let pointer = text.as_ptr();
    let (_, _, _, _, accepted) = session
        .try_accept::<_, str, _>(&(316, 0), text, &TextPolicy)
        .unwrap()
        .into_parts();
    assert_eq!(accepted.as_ptr(), pointer);
    assert_eq!(accepted, "receipt");
    assert_eq!(session.fuel_remaining(), 7);
    assert_eq!(
        session.status(),
        SequenceStatus::Terminal(SequenceEnd::Completed)
    );
}
