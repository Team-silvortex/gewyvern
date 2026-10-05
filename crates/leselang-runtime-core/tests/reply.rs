use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_runtime_core::*;

type Events = Rc<RefCell<Vec<&'static str>>>;
struct Key {
    owner: Rc<()>,
    generation: u32,
    events: Events,
    unwind: bool,
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
        }
    }
    fn frame(&self) -> Frame {
        Frame {
            drops: self.drops.clone(),
            unwind: false,
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
