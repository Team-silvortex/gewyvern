use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_runtime_core::*;

type Events = Rc<RefCell<Vec<&'static str>>>;
type CleanupEvents = Rc<RefCell<Vec<(&'static str, bool)>>>;
struct CleanupProbe {
    lease: LocalExecutionLease,
    events: CleanupEvents,
    role: &'static str,
}
impl Drop for CleanupProbe {
    fn drop(&mut self) {
        self.events
            .borrow_mut()
            .push((self.role, !self.lease.is_open()));
    }
}
struct Key {
    owner: Rc<()>,
    generation: u32,
    events: Events,
    unwind: bool,
    _cleanup: Option<CleanupProbe>,
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.events.borrow_mut().push("identity");
        assert!(!self.unwind, "native identity unwind");
        Rc::ptr_eq(&self.owner, &other.owner) && self.generation == other.generation
    }
}
struct Frame {
    drops: Rc<Cell<usize>>,
    unwind: bool,
    _cleanup: Option<CleanupProbe>,
}
impl Drop for Frame {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
        assert!(!self.unwind, "native frame destructor unwind");
    }
}
struct PrivateError(String);
struct Authority {
    events: Events,
    deny: Cell<bool>,
    unwind: Cell<bool>,
}
impl ReplyAuthority<Key> for Authority {
    type Error = PrivateError;
    fn authorize(&self, _: &Key) -> Result<(), PrivateError> {
        self.events.borrow_mut().push("authority");
        assert!(!self.unwind.get(), "native policy unwind");
        if self.deny.get() {
            Err(PrivateError("private authority failure".into()))
        } else {
            Ok(())
        }
    }
}
struct Body {
    kind: u8,
    private: String,
}
struct Input {
    body: Body,
    events: Events,
    unwind: bool,
    _cleanup: Option<CleanupProbe>,
}
impl std::borrow::Borrow<Body> for Input {
    fn borrow(&self) -> &Body {
        self.events.borrow_mut().push("borrow");
        assert!(!self.unwind, "native borrow unwind");
        &self.body
    }
}
struct Declaration {
    events: Events,
    type_unwind: Cell<bool>,
    value_unwind: Cell<bool>,
}
impl HostResultDomain<Body> for Declaration {
    type Error = PrivateError;
    fn matches_type(&self, body: &Body) -> bool {
        self.events.borrow_mut().push("type");
        assert!(!self.type_unwind.get(), "native type unwind");
        body.kind == 7
    }
    fn validate_value(&self, body: &Body) -> Result<(), PrivateError> {
        self.events.borrow_mut().push("value");
        assert!(!self.value_unwind.get(), "native value unwind");
        if body.private.len() <= 16 {
            Ok(())
        } else {
            Err(PrivateError("private domain failure".into()))
        }
    }
}
struct Fixture {
    owner: Rc<()>,
    events: Events,
    drops: Rc<Cell<usize>>,
    authority: Authority,
    declaration: Declaration,
}
impl Fixture {
    fn new() -> Self {
        let events = Rc::new(RefCell::new(Vec::new()));
        Self {
            owner: Rc::new(()),
            events: events.clone(),
            drops: Rc::new(Cell::new(0)),
            authority: Authority {
                events: events.clone(),
                deny: Cell::new(false),
                unwind: Cell::new(false),
            },
            declaration: Declaration {
                events,
                type_unwind: Cell::new(false),
                value_unwind: Cell::new(false),
            },
        }
    }
    fn key(&self, generation: u32) -> Key {
        Key {
            owner: self.owner.clone(),
            generation,
            events: self.events.clone(),
            unwind: false,
            _cleanup: None,
        }
    }
    fn frame(&self) -> Frame {
        Frame {
            drops: self.drops.clone(),
            unwind: false,
            _cleanup: None,
        }
    }
    fn input(&self, kind: u8, text: &str) -> Input {
        Input {
            body: Body {
                kind,
                private: text.into(),
            },
            events: self.events.clone(),
            unwind: false,
            _cleanup: None,
        }
    }
}

#[test]
fn successful_acceptance_moves_original_frame_reply_and_declaration_once() {
    let f = Fixture::new();
    let mut pending = PendingReply::new(f.key(4), f.frame(), &f.declaration);
    let reply = f.input(7, "private-value");
    let buffer = reply.body.private.as_ptr();
    assert_eq!(pending.status(), ReplyStatus::Pending);
    let accepted = pending.try_accept(&f.key(4), reply, &f.authority).unwrap();
    assert_eq!(
        *f.events.borrow(),
        ["identity", "authority", "borrow", "type", "value"]
    );
    assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Accepted));
    assert_eq!(f.drops.get(), 0);
    assert_eq!(format!("{accepted:?}"), "AcceptedReply");
    let before = f.events.borrow().clone();
    let (borrowed_identity, borrowed_frame, borrowed_declaration, borrowed_input) =
        accepted.as_parts();
    assert_eq!(borrowed_identity.generation, 4);
    assert!(Rc::ptr_eq(&borrowed_frame.drops, &f.drops));
    assert!(std::ptr::eq(borrowed_declaration, &f.declaration));
    assert_eq!(borrowed_input.body.private.as_ptr(), buffer);
    assert_eq!(*f.events.borrow(), before);
    let frame_pointer = borrowed_frame as *const Frame;
    assert_eq!(accepted.as_parts().1 as *const Frame, frame_pointer);
    let (identity, frame, declaration, reply) = accepted.into_parts();
    assert_eq!(identity.generation, 4);
    assert!(std::ptr::eq(declaration, &f.declaration));
    assert_eq!(reply.body.private.as_ptr(), buffer);
    f.events.borrow_mut().clear();
    let rejected = pending
        .try_accept(&f.key(4), f.input(7, "later"), &f.authority)
        .unwrap_err();
    assert!(matches!(
        rejected.error,
        ReplyAcceptanceError::Closed(ReplyEnd::Accepted)
    ));
    assert!(f.events.borrow().is_empty());
    assert!(!pending.cancel());
    assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Accepted));
    drop(frame);
    assert_eq!(f.drops.get(), 1);
}

#[test]
fn mismatched_owner_or_generation_never_enters_authority_or_reply_queries() {
    let f = Fixture::new();
    let mut pending = PendingReply::new(f.key(4), f.frame(), &f.declaration);
    for actual in [
        f.key(5),
        Key {
            owner: Rc::new(()),
            ..f.key(4)
        },
    ] {
        f.events.borrow_mut().clear();
        let input = f.input(7, "private");
        let buffer = input.body.private.as_ptr();
        let rejected = pending
            .try_accept(&actual, input, &f.authority)
            .unwrap_err();
        assert!(matches!(
            rejected.error,
            ReplyAcceptanceError::IdentityMismatch
        ));
        assert_eq!(rejected.reply.body.private.as_ptr(), buffer);
        assert_eq!(*f.events.borrow(), ["identity"]);
        assert_eq!(pending.status(), ReplyStatus::Pending);
        assert_eq!(f.drops.get(), 0);
        assert!(!format!("{rejected:?}").contains("private"));
    }
    assert!(pending.cancel());
    assert_eq!(f.drops.get(), 1);
}

#[test]
fn live_authority_rejection_retains_frame_and_original_typed_error_without_retry() {
    let f = Fixture::new();
    f.authority.deny.set(true);
    let mut pending = PendingReply::new(f.key(4), f.frame(), &f.declaration);
    let failure = pending
        .try_accept(&f.key(4), f.input(7, "private"), &f.authority)
        .unwrap_err();
    assert_eq!(*f.events.borrow(), ["identity", "authority"]);
    assert!(std::error::Error::source(&failure.error).is_none());
    assert!(!format!("{failure:?}").contains("private"));
    let ReplyAcceptanceError::Authority(PrivateError(message)) = failure.error else {
        panic!()
    };
    assert_eq!(message, "private authority failure");
    assert_eq!(pending.status(), ReplyStatus::Pending);
    f.authority.deny.set(false);
    let accepted = pending
        .try_accept(&f.key(4), failure.reply, &f.authority)
        .unwrap();
    assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Accepted));
    drop(accepted);
    assert_eq!(f.drops.get(), 1);
}

#[test]
fn type_precedes_domain_and_normal_domain_failure_returns_input_unchanged() {
    let f = Fixture::new();
    let mut pending = PendingReply::new(f.key(4), f.frame(), &f.declaration);
    let rejected = pending
        .try_accept(&f.key(4), f.input(8, "private"), &f.authority)
        .unwrap_err();
    assert!(matches!(
        rejected.error,
        ReplyAcceptanceError::Result(HostResultError::TypeMismatch)
    ));
    assert_eq!(
        *f.events.borrow(),
        ["identity", "authority", "borrow", "type"]
    );
    f.events.borrow_mut().clear();
    let input = f.input(7, "private value is too long");
    let buffer = input.body.private.as_ptr();
    let rejected = pending
        .try_accept(&f.key(4), input, &f.authority)
        .unwrap_err();
    assert_eq!(
        *f.events.borrow(),
        ["identity", "authority", "borrow", "type", "value"]
    );
    assert_eq!(rejected.reply.body.private.as_ptr(), buffer);
    assert!(!format!("{rejected:?}").contains("private"));
    let ReplyAcceptanceError::Result(HostResultError::InvalidValue(PrivateError(message))) =
        rejected.error
    else {
        panic!()
    };
    assert_eq!(message, "private domain failure");
    assert_eq!(pending.status(), ReplyStatus::Pending);
    assert_eq!(f.drops.get(), 0);
}

#[test]
fn cancellation_releases_frame_once_and_never_validates_late_replies() {
    let f = Fixture::new();
    let mut pending = PendingReply::new(f.key(4), f.frame(), &f.declaration);
    assert!(pending.cancel());
    assert!(!pending.cancel());
    assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Cancelled));
    assert_eq!(f.drops.get(), 1);
    let rejected = pending
        .try_accept(&f.key(4), f.input(7, "private"), &f.authority)
        .unwrap_err();
    assert!(matches!(
        rejected.error,
        ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)
    ));
    assert!(f.events.borrow().is_empty());
    drop(pending);
    assert_eq!(f.drops.get(), 1);
}

#[test]
fn native_unwind_at_every_acceptance_stage_closes_and_releases_without_rearming() {
    for stage in ["identity", "authority", "borrow", "type", "value"] {
        let f = Fixture::new();
        let mut key = f.key(4);
        key.unwind = stage == "identity";
        f.authority.unwind.set(stage == "authority");
        f.declaration.type_unwind.set(stage == "type");
        f.declaration.value_unwind.set(stage == "value");
        let mut pending = PendingReply::new(key, f.frame(), &f.declaration);
        let mut input = f.input(7, "private");
        input.unwind = stage == "borrow";
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _ = pending.try_accept(&f.key(4), input, &f.authority);
            }))
            .is_err()
        );
        assert_eq!(
            pending.status(),
            ReplyStatus::Terminal(ReplyEnd::HostUncertain)
        );
        assert_eq!(f.drops.get(), 1);
        f.events.borrow_mut().clear();
        let rejected = pending
            .try_accept(&f.key(4), f.input(7, "later"), &f.authority)
            .unwrap_err();
        assert!(matches!(
            rejected.error,
            ReplyAcceptanceError::Closed(ReplyEnd::HostUncertain)
        ));
        assert!(f.events.borrow().is_empty());
        assert!(!pending.cancel());
        assert_eq!(f.drops.get(), 1);
    }
}

#[test]
fn scoped_acceptance_unwind_revokes_before_owned_identity_frame_and_reply_cleanup() {
    for scoped in [false, true] {
        for (index, stage) in ["identity", "authority", "borrow", "type", "value"]
            .into_iter()
            .enumerate()
        {
            let f = Fixture::new();
            let scope = LocalExecutionScope::new();
            let sibling = LocalExecutionScope::new();
            let cleanup = Rc::new(RefCell::new(Vec::new()));
            let probe = |role| {
                Some(CleanupProbe {
                    lease: scope.lease(),
                    events: cleanup.clone(),
                    role,
                })
            };
            let mut key = f.key(4);
            key.unwind = stage == "identity";
            key._cleanup = probe("identity");
            let mut frame = f.frame();
            frame._cleanup = probe("frame");
            let mut input = f.input(7, "private");
            input.unwind = stage == "borrow";
            input._cleanup = probe("reply");
            f.authority.unwind.set(stage == "authority");
            f.declaration.type_unwind.set(stage == "type");
            f.declaration.value_unwind.set(stage == "value");
            let mut pending = PendingReply::new(key, frame, &f.declaration);
            assert!(
                catch_unwind(AssertUnwindSafe(|| {
                    if scoped {
                        let _ = pending.try_accept_in_scope(&f.key(4), input, &f.authority, &scope);
                    } else {
                        let _ = pending.try_accept(&f.key(4), input, &f.authority);
                    }
                }))
                .is_err()
            );
            assert_eq!(scope.is_open(), !scoped);
            assert!(sibling.is_open());
            assert_eq!(
                *cleanup.borrow(),
                [("identity", scoped), ("frame", scoped), ("reply", scoped)]
            );
            assert_eq!(
                *f.events.borrow(),
                ["identity", "authority", "borrow", "type", "value"][..=index]
            );
            assert_eq!(
                pending.status(),
                ReplyStatus::Terminal(ReplyEnd::HostUncertain)
            );
            assert_eq!(f.drops.get(), 1);
            f.events.borrow_mut().clear();
            let late = f.input(7, "late");
            let pointer = late.body.private.as_ptr();
            let rejected = pending
                .try_accept_in_scope(&f.key(4), late, &f.authority, &scope)
                .unwrap_err();
            assert_eq!(rejected.reply.body.private.as_ptr(), pointer);
            assert!(matches!(
                rejected.error,
                ReplyAcceptanceError::Closed(ReplyEnd::HostUncertain)
            ));
            assert!(f.events.borrow().is_empty());
            assert!(!pending.cancel());
            assert_eq!(cleanup.borrow().len(), 3);
        }
    }
}

#[test]
fn scoped_normal_rejection_preserves_original_input_domain_and_live_retry() {
    for stage in ["identity", "authority", "type", "value"] {
        let f = Fixture::new();
        let scope = LocalExecutionScope::new();
        let mut pending = PendingReply::new(f.key(4), f.frame(), &f.declaration);
        f.authority.deny.set(stage == "authority");
        let input = f.input(
            if stage == "type" { 8 } else { 7 },
            if stage == "value" {
                "private value is too long"
            } else {
                "private"
            },
        );
        let pointer = input.body.private.as_ptr();
        let identity = f.key(if stage == "identity" { 5 } else { 4 });
        let mut rejected = pending
            .try_accept_in_scope(&identity, input, &f.authority, &scope)
            .unwrap_err();
        assert!(scope.is_open());
        assert_eq!(pending.status(), ReplyStatus::Pending);
        assert_eq!(f.drops.get(), 0);
        assert_eq!(rejected.reply.body.private.as_ptr(), pointer);
        let expected: &[&str] = match stage {
            "identity" => &["identity"],
            "authority" => &["identity", "authority"],
            "type" => &["identity", "authority", "borrow", "type"],
            _ => &["identity", "authority", "borrow", "type", "value"],
        };
        assert_eq!(*f.events.borrow(), expected);
        f.authority.deny.set(false);
        rejected.reply.body.kind = 7;
        rejected.reply.body.private.truncate(7);
        f.events.borrow_mut().clear();
        let accepted = pending
            .try_accept_in_scope(&f.key(4), rejected.reply, &f.authority, &scope)
            .unwrap();
        assert_eq!(
            *f.events.borrow(),
            ["identity", "authority", "borrow", "type", "value"]
        );
        assert!(scope.is_open());
        assert_eq!(accepted.as_parts().3.body.private.as_ptr(), pointer);
        assert!(std::ptr::eq(accepted.as_parts().2, &f.declaration));
        assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Accepted));
        drop(accepted);
        assert_eq!(f.drops.get(), 1);
    }
}

#[test]
fn preclosed_acceptance_scope_cancels_before_cleanup_without_entering_callbacks() {
    for unwind in [false, true] {
        let f = Fixture::new();
        let scope = LocalExecutionScope::new();
        let cleanup = Rc::new(RefCell::new(Vec::new()));
        let mut frame = f.frame();
        frame.unwind = unwind;
        frame._cleanup = Some(CleanupProbe {
            lease: scope.lease(),
            events: cleanup.clone(),
            role: "frame",
        });
        let mut pending = PendingReply::new(f.key(4), frame, &f.declaration);
        scope.close();
        let input = f.input(7, "private");
        let pointer = input.body.private.as_ptr();
        let result = catch_unwind(AssertUnwindSafe(|| {
            pending.try_accept_in_scope(&f.key(4), input, &f.authority, &scope)
        }));
        if unwind {
            assert!(result.is_err());
        } else {
            let rejected = result.unwrap().unwrap_err();
            assert_eq!(rejected.reply.body.private.as_ptr(), pointer);
            assert!(matches!(
                rejected.error,
                ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)
            ));
        }
        assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Cancelled));
        assert_eq!(*cleanup.borrow(), [("frame", true)]);
        assert!(f.events.borrow().is_empty());
        assert!(!pending.cancel());
        assert_eq!(f.drops.get(), 1);
    }
}

#[test]
fn cancellation_destructor_unwind_keeps_terminal_state_and_releases_no_frame_twice() {
    let f = Fixture::new();
    let mut frame = f.frame();
    frame.unwind = true;
    let mut pending = PendingReply::new(f.key(4), frame, &f.declaration);
    assert!(catch_unwind(AssertUnwindSafe(|| pending.cancel())).is_err());
    assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Cancelled));
    assert!(!pending.cancel());
    drop(pending);
    assert_eq!(f.drops.get(), 1);
}

#[test]
fn abandoning_pending_or_accepted_data_drops_owned_frame_without_any_host_queries() {
    let f = Fixture::new();
    drop(PendingReply::new(f.key(4), f.frame(), &f.declaration));
    assert_eq!(f.drops.get(), 1);
    assert!(f.events.borrow().is_empty());
    let mut pending = PendingReply::new(f.key(4), f.frame(), &f.declaration);
    let accepted = pending
        .try_accept(&f.key(4), f.input(7, "private"), &f.authority)
        .unwrap();
    f.events.borrow_mut().clear();
    drop(accepted);
    drop(pending);
    assert_eq!(f.drops.get(), 2);
    assert!(f.events.borrow().is_empty());
}

#[test]
fn status_and_debug_never_clone_inspect_or_format_private_native_data() {
    let f = Fixture::new();
    let mut pending = PendingReply::new(f.key(4), f.frame(), &f.declaration);
    assert_eq!(format!("{pending:?}"), "PendingReply { status: Pending }");
    assert_eq!(pending.status(), ReplyStatus::Pending);
    assert!(f.events.borrow().is_empty());
    assert!(pending.cancel());
    assert_eq!(
        format!("{pending:?}"),
        "PendingReply { status: Terminal(Cancelled) }"
    );
    assert!(f.events.borrow().is_empty());
}

struct TextDomain;
impl HostResultDomain<str> for TextDomain {
    type Error = ScalarError;
    fn matches_type(&self, _: &str) -> bool {
        true
    }
    fn validate_value(&self, value: &str) -> Result<(), ScalarError> {
        if value == "ready" {
            Ok(())
        } else {
            Err(ScalarError::InvalidIntegerText)
        }
    }
}
mod synchronous_revocation {
    use super::*;

    const PHASES: [&str; 5] = ["identity", "authority", "borrow", "type", "value"];

    #[test]
    fn scoped_and_unscoped_entries_preserve_the_original_native_inequality_hook() {
        struct NativeIdentity<'a>(&'a Cell<usize>, u8);
        #[expect(
            clippy::partialeq_ne_impl,
            reason = "freeze existing native callback selection, including an overridden ne"
        )]
        impl PartialEq for NativeIdentity<'_> {
            fn eq(&self, _: &Self) -> bool {
                panic!("acceptance must retain its original inequality hook");
            }
            fn ne(&self, other: &Self) -> bool {
                self.0.set(self.0.get() + 1);
                self.1 != other.1
            }
        }
        struct Policy;
        impl ReplyAuthority<NativeIdentity<'_>> for Policy {
            type Error = ();
            fn authorize(&self, _: &NativeIdentity<'_>) -> Result<(), ()> {
                Ok(())
            }
        }
        for scoped in [false, true] {
            for mismatch in [false, true] {
                let scope = LocalExecutionScope::new();
                let calls = Cell::new(0);
                let domain = ScalarTypeSet::only(ScalarType::Integer);
                let mut pending = PendingReply::new(NativeIdentity(&calls, 7), (), &domain);
                let actual = NativeIdentity(&calls, if mismatch { 8 } else { 7 });
                let reply = ScalarValue::Integer(3);
                let result = if scoped {
                    pending.try_accept_in_scope(&actual, reply, &Policy, &scope)
                } else {
                    pending.try_accept(&actual, reply, &Policy)
                };
                assert_eq!(calls.get(), 1);
                assert!(scope.is_open());
                if mismatch {
                    assert!(matches!(
                        result.unwrap_err().error,
                        ReplyAcceptanceError::IdentityMismatch
                    ));
                    assert_eq!(pending.status(), ReplyStatus::Pending);
                } else {
                    assert!(result.is_ok());
                    assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Accepted));
                }
            }
        }
    }

    struct Gate<'a> {
        scope: &'a LocalExecutionScope,
        phase: &'static str,
        refuse: bool,
        error_unwind: bool,
        calls: RefCell<Vec<&'static str>>,
        cleanup: CleanupEvents,
    }
    impl Gate<'_> {
        fn observe(&self, phase: &'static str) -> bool {
            self.calls.borrow_mut().push(phase);
            if self.phase == phase {
                self.scope.close();
                !self.refuse
            } else {
                true
            }
        }
        fn probe(&self, role: &'static str) -> CleanupProbe {
            CleanupProbe {
                lease: self.scope.lease(),
                events: self.cleanup.clone(),
                role,
            }
        }
    }
    struct Identity<'a>(&'a Gate<'a>, Option<CleanupProbe>);
    impl PartialEq for Identity<'_> {
        fn eq(&self, other: &Self) -> bool {
            self.0.observe("identity") && std::ptr::eq(self.0, other.0)
        }
    }
    impl Drop for Identity<'_> {
        fn drop(&mut self) {
            drop(self.1.take());
        }
    }
    struct Reply<'a> {
        gate: &'a Gate<'a>,
        body: Body,
        _cleanup: CleanupProbe,
    }
    impl std::borrow::Borrow<Body> for Reply<'_> {
        fn borrow(&self) -> &Body {
            self.gate.observe("borrow");
            &self.body
        }
    }
    struct NativeError<'a>(&'a Gate<'a>);
    impl Drop for NativeError<'_> {
        fn drop(&mut self) {
            self.0
                .cleanup
                .borrow_mut()
                .push(("error", !self.0.scope.is_open()));
            assert!(!self.0.error_unwind, "native error disposal unwind");
        }
    }
    impl<'a> ReplyAuthority<Identity<'a>> for Gate<'a> {
        type Error = NativeError<'a>;
        fn authorize(&self, identity: &Identity<'a>) -> Result<(), Self::Error> {
            if self.observe("authority") {
                Ok(())
            } else {
                Err(NativeError(identity.0))
            }
        }
    }
    struct Domain<'a>(&'a Gate<'a>);
    impl<'a> HostResultDomain<Body> for Domain<'a> {
        type Error = NativeError<'a>;
        fn matches_type(&self, body: &Body) -> bool {
            self.0.observe("type") && body.kind == 7
        }
        fn validate_value(&self, _: &Body) -> Result<(), Self::Error> {
            if self.0.observe("value") {
                Ok(())
            } else {
                Err(NativeError(self.0))
            }
        }
    }

    fn run(
        phase: &'static str,
        refuse: bool,
        scoped: bool,
        frame_unwind: bool,
        error_unwind: bool,
    ) {
        let scope = LocalExecutionScope::new();
        let sibling = LocalExecutionScope::new();
        let cleanup = Rc::new(RefCell::new(Vec::new()));
        let gate = Gate {
            scope: &scope,
            phase,
            refuse,
            error_unwind,
            calls: RefCell::new(Vec::new()),
            cleanup: cleanup.clone(),
        };
        let domain = Domain(&gate);
        let drops = Rc::new(Cell::new(0));
        let mut pending = PendingReply::new(
            Identity(&gate, Some(gate.probe("identity"))),
            Frame {
                drops: drops.clone(),
                unwind: frame_unwind,
                _cleanup: Some(gate.probe("frame")),
            },
            &domain,
        );
        let reply = Reply {
            gate: &gate,
            body: Body {
                kind: 7,
                private: String::from("original-input"),
            },
            _cleanup: gate.probe("reply"),
        };
        let pointer = reply.body.private.as_ptr();
        let actual = Identity(&gate, None);
        let result = catch_unwind(AssertUnwindSafe(|| {
            if scoped {
                pending.try_accept_in_scope(&actual, reply, &gate, &scope)
            } else {
                pending.try_accept(&actual, reply, &gate)
            }
        }));
        assert!(!scope.is_open());
        assert!(sibling.is_open());
        if !scoped {
            let accepted = result.unwrap().unwrap();
            assert_eq!(*gate.calls.borrow(), PHASES);
            assert_eq!(accepted.as_parts().3.body.private.as_ptr(), pointer);
            assert!(std::ptr::eq(accepted.as_parts().2, &domain));
            assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Accepted));
            drop(accepted);
        } else {
            if frame_unwind || error_unwind {
                let unwound = result.is_err();
                // A regression may return an owner with a panicking destructor.
                // Dispose it before asserting, so failure cannot double-panic.
                let _ = catch_unwind(AssertUnwindSafe(|| drop(result)));
                assert!(unwound);
            } else {
                let rejected = result.unwrap().unwrap_err();
                assert!(matches!(
                    rejected.error,
                    ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)
                ));
                assert_eq!(rejected.reply.body.private.as_ptr(), pointer);
                assert!(!cleanup.borrow().iter().any(|(role, _)| *role == "reply"));
                drop(rejected);
            }
            assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Cancelled));
            let index = PHASES.iter().position(|p| *p == phase).unwrap();
            assert_eq!(*gate.calls.borrow(), PHASES[..=index]);
        }
        assert!(!pending.cancel());
        assert_eq!(drops.get(), 1);
        let events = cleanup.borrow();
        for role in ["identity", "frame", "reply"] {
            assert_eq!(
                events
                    .iter()
                    .filter(|event| **event == (role, true))
                    .count(),
                1
            );
        }
        assert!(events.iter().all(|(_, closed)| *closed));
        if refuse && matches!(phase, "authority" | "value") {
            assert_eq!(
                events.iter().filter(|(role, _)| *role == "error").count(),
                1
            );
        }
    }

    #[test]
    fn scoped_native_callback_revocation_stops_before_the_next_phase_and_returns_input() {
        for phase in PHASES {
            for refuse in [false, true] {
                run(phase, refuse, true, false, false);
            }
        }
    }
    #[test]
    fn scoped_revocation_seals_cancelled_before_frame_or_native_error_cleanup_unwind() {
        for phase in PHASES {
            run(phase, false, true, true, false);
        }
        for phase in ["authority", "value"] {
            run(phase, true, true, false, true);
        }
    }
    #[test]
    fn unscoped_acceptance_does_not_infer_an_unrelated_scope_policy() {
        for phase in PHASES {
            run(phase, false, false, false, false);
        }
    }
}

struct TextAuthority;
impl ReplyAuthority<u32> for TextAuthority {
    type Error = ();
    fn authorize(&self, _: &u32) -> Result<(), ()> {
        Ok(())
    }
}

#[test]
fn distinct_unsized_domain_keeps_native_scalar_shaped_errors_external() {
    let mut pending = PendingReply::new(17, (), &TextDomain);
    let rejected = pending
        .try_accept(&17, String::from("invalid"), &TextAuthority)
        .unwrap_err();
    assert!(matches!(
        rejected.error,
        ReplyAcceptanceError::Result(HostResultError::InvalidValue(
            ScalarError::InvalidIntegerText
        ))
    ));
    assert_eq!(pending.status(), ReplyStatus::Pending);
    let value = String::from("ready");
    let buffer = value.as_ptr();
    let accepted = pending.try_accept(&17, value, &TextAuthority).unwrap();
    let (_, _, declaration, reply) = accepted.into_parts();
    assert!(std::ptr::eq(declaration, &TextDomain));
    assert_eq!(reply.as_ptr(), buffer);
}
