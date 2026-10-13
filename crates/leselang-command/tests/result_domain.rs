use std::{
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_command::{
    ResultValueDomain, ResultValueError, ResultValueHost, StructuredField, Value, ValueBoundsError,
    ValueValidationLimits,
};
use leselang_hir::Type;
use leselang_host_contract::Revision;
use leselang_runtime_core::{
    HostResultError, LocalExecutionScope, PendingReply, ReplyAcceptanceError, ReplyAuthority,
    ReplyEnd, ReplyStatus, validate_host_result,
};

// Deliberately neither Clone nor Debug; native payloads must stay opaque.
struct PrivateError(Box<str>);

struct Host<'a> {
    allowed: Cell<bool>,
    visits: RefCell<Vec<&'static str>>,
    encoding_error: RefCell<Option<PrivateError>>,
    panic_at: Cell<Option<&'static str>>,
    close_at: Cell<Option<&'static str>>,
    scope: &'a LocalExecutionScope,
}

impl<'a> Host<'a> {
    fn new(scope: &'a LocalExecutionScope) -> Self {
        Self {
            allowed: Cell::new(true),
            visits: RefCell::new(vec![]),
            encoding_error: RefCell::new(None),
            panic_at: Cell::new(None),
            close_at: Cell::new(None),
            scope,
        }
    }

    fn visit(&self, stage: &'static str) {
        self.visits.borrow_mut().push(stage);
        assert_ne!(self.panic_at.get(), Some(stage), "native hook unwind");
        if self.close_at.get() == Some(stage) {
            self.scope.close();
        }
    }

    fn domain(&self, result_type: Type, max_output_items: usize) -> ResultValueDomain<'_, Self> {
        ResultValueDomain {
            result_type,
            value_limits: ValueValidationLimits {
                max_depth: 2,
                max_fields: 2,
                max_query_items: 2,
            },
            max_output_items,
            host: self,
        }
    }

    fn take_visits(&self) -> Vec<&'static str> {
        self.visits.take()
    }
}

impl ResultValueHost for Host<'_> {
    type Error = PrivateError;

    fn valid_field_name(&self, name: &str) -> bool {
        self.visit("name");
        !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_alphanumeric())
    }

    fn validate_encoded_value(&self, _: &Value) -> Result<(), PrivateError> {
        self.visit("encoding");
        match self.encoding_error.take() {
            Some(error) => Err(error),
            None => Ok(()),
        }
    }
}

impl ReplyAuthority<&str> for Host<'_> {
    type Error = PrivateError;

    fn authorize(&self, _: &&str) -> Result<(), PrivateError> {
        self.visit("authority");
        if self.allowed.get() {
            Ok(())
        } else {
            Err(PrivateError("private authority refusal".into()))
        }
    }
}

fn focus(node: &str) -> Value {
    Value::UiFocus {
        node_id: node.into(),
    }
}

fn group(name: &str, value: Value) -> Value {
    Value::Structured {
        fields: vec![StructuredField {
            name: name.into(),
            value: Box::new(value),
        }],
    }
}

struct Frame {
    payload: Box<str>,
    drops: Rc<Cell<usize>>,
}

impl Drop for Frame {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn result_domain_preserves_type_tree_output_encoding_order_and_zero_limit() {
    let scope = LocalExecutionScope::new();
    let host = Host::new(&scope);
    let wrong_kind = group("invalid name", focus("bad id"));
    assert!(matches!(
        validate_host_result(&host.domain(Type::UiFocus, 0), &wrong_kind),
        Err(HostResultError::TypeMismatch)
    ));
    assert!(host.take_visits().is_empty());
    assert!(matches!(
        validate_host_result(&host.domain(Type::Structured, 0), &wrong_kind),
        Err(HostResultError::InvalidValue(ResultValueError::Bounds(
            ValueBoundsError::Fields
        )))
    ));
    assert_eq!(host.take_visits(), ["name"]);
    assert!(matches!(
        validate_host_result(&host.domain(Type::UiFocus, 0), &focus("bad id")),
        Err(HostResultError::InvalidValue(ResultValueError::Bounds(
            ValueBoundsError::Value
        )))
    ));
    assert!(host.take_visits().is_empty());
    for value in [
        Value::RuntimeList {
            revision: Revision(7),
            runtimes: vec![],
        },
        focus("a"),
        group("field", focus("a")),
    ] {
        let before = serde_json::to_vec(&value).unwrap();
        let count = if matches!(value, Value::RuntimeList { .. }) {
            0
        } else {
            1
        };
        let domain = host.domain(value.result_type(), 0);
        assert!(matches!(
            validate_host_result(&domain, &value),
            Err(HostResultError::InvalidValue(ResultValueError::OutputLimit { count: actual, limit: 0 })) if actual == count
        ));
        assert!(!host.take_visits().contains(&"encoding"));
        validate_host_result(&host.domain(value.result_type(), 1), &value).unwrap();
        assert_eq!(host.take_visits().last(), Some(&"encoding"));
        assert_eq!(serde_json::to_vec(&value).unwrap(), before);
    }
    let value = group("a", group("b", group("c", focus("a"))));
    assert!(matches!(
        validate_host_result(&host.domain(Type::Structured, 100), &value),
        Err(HostResultError::InvalidValue(ResultValueError::Bounds(
            ValueBoundsError::Nesting
        )))
    ));
    assert_eq!(host.take_visits(), ["name", "name", "name"]);
}

#[test]
fn dynamic_host_keeps_native_error_ownership_without_debug_or_clone_bounds() {
    let scope = LocalExecutionScope::new();
    let host = Host::new(&scope);
    let private: Box<str> = "private encoding refusal".into();
    let pointer = private.as_ptr();
    *host.encoding_error.borrow_mut() = Some(PrivateError(private));
    let native: &dyn ResultValueHost<Error = PrivateError> = &host;
    let original = host.domain(Type::UiFocus, 1);
    let domain = ResultValueDomain {
        result_type: original.result_type,
        value_limits: original.value_limits,
        max_output_items: original.max_output_items,
        host: native,
    };
    assert_eq!(format!("{domain:?}"), "ResultValueDomain");
    let error = validate_host_result(&domain, &focus("a")).unwrap_err();
    let HostResultError::InvalidValue(error) = error else {
        panic!()
    };
    assert_eq!(format!("{error:?}"), "ResultValueError::Encoding");
    assert_eq!(
        error.to_string(),
        "result value failed native encoding validation"
    );
    let ResultValueError::Encoding(PrivateError(private)) = error else {
        panic!()
    };
    assert_eq!(private.as_ptr(), pointer);
    assert_eq!(host.take_visits(), ["encoding"]);
}

#[test]
fn pending_rejection_keeps_original_reply_and_frame_until_explicit_once_only_handoff() {
    let scope = LocalExecutionScope::new();
    let host = Host::new(&scope);
    let domain = host.domain(Type::UiFocus, 1);
    let drops = Rc::new(Cell::new(0));
    let frame = Frame {
        payload: "private saved frame".into(),
        drops: Rc::clone(&drops),
    };
    let frame_pointer = frame.payload.as_ptr();
    let mut pending = PendingReply::new("generation-7", frame, &domain);
    let value = focus("a");
    let Value::UiFocus { node_id } = &value else {
        panic!()
    };
    let pointer = node_id.as_ptr();
    let wrong_identity = pending
        .try_accept(&"generation-8", value, &host)
        .unwrap_err();
    assert!(matches!(
        wrong_identity.error,
        ReplyAcceptanceError::IdentityMismatch
    ));
    assert!(host.take_visits().is_empty());
    host.allowed.set(false);
    let denied = pending
        .try_accept(&"generation-7", wrong_identity.reply, &host)
        .unwrap_err();
    assert!(matches!(denied.error, ReplyAcceptanceError::Authority(_)));
    assert_eq!(host.take_visits(), ["authority"]);
    host.allowed.set(true);
    *host.encoding_error.borrow_mut() = Some(PrivateError("private encoding refusal".into()));
    let rejected = pending
        .try_accept(&"generation-7", denied.reply, &host)
        .unwrap_err();
    assert!(matches!(
        rejected.error,
        ReplyAcceptanceError::Result(HostResultError::InvalidValue(ResultValueError::Encoding(_)))
    ));
    assert_eq!(host.take_visits(), ["authority", "encoding"]);
    assert_eq!(pending.status(), ReplyStatus::Pending);
    assert_eq!(drops.get(), 0);
    let Value::UiFocus { node_id } = &rejected.reply else {
        panic!()
    };
    assert_eq!(node_id.as_ptr(), pointer);
    let accepted = pending
        .try_accept(&"generation-7", rejected.reply, &host)
        .unwrap();
    let (identity, frame, declaration, reply) = accepted.into_parts();
    assert_eq!(identity, "generation-7");
    assert!(std::ptr::eq(declaration, &domain));
    assert_eq!(frame.payload.as_ptr(), frame_pointer);
    let Value::UiFocus { node_id } = reply else {
        panic!()
    };
    assert_eq!(node_id.as_ptr(), pointer);
    assert_eq!(host.take_visits(), ["authority", "encoding"]);
    assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Accepted));
    assert_eq!(drops.get(), 0);
    let again = pending
        .try_accept(&"generation-7", focus("a"), &host)
        .unwrap_err();
    assert!(matches!(
        again.error,
        ReplyAcceptanceError::Closed(ReplyEnd::Accepted)
    ));
    assert!(host.take_visits().is_empty());
    drop(frame);
    assert_eq!(drops.get(), 1);
}

#[test]
fn native_unwind_seals_pending_state_and_scope_before_frame_cleanup() {
    for stage in ["name", "encoding"] {
        let scope = LocalExecutionScope::new();
        let host = Host::new(&scope);
        host.panic_at.set(Some(stage));
        let domain = host.domain(Type::Structured, 1);
        let drops = Rc::new(Cell::new(0));
        struct ScopedFrame<'a>(&'a LocalExecutionScope, Rc<Cell<usize>>);
        impl Drop for ScopedFrame<'_> {
            fn drop(&mut self) {
                assert!(!self.0.is_open());
                self.1.set(self.1.get() + 1);
            }
        }
        let mut pending = PendingReply::new("a", ScopedFrame(&scope, Rc::clone(&drops)), &domain);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                pending.try_accept_in_scope(&"a", group("field", focus("a")), &host, &scope)
            }))
            .is_err()
        );
        assert_eq!(
            pending.status(),
            ReplyStatus::Terminal(ReplyEnd::HostUncertain)
        );
        assert!(!scope.is_open());
        assert_eq!(drops.get(), 1);
        host.take_visits();
        assert!(
            pending
                .try_accept_in_scope(&"a", group("field", focus("a")), &host, &scope)
                .is_err()
        );
        assert!(host.take_visits().is_empty());
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn scope_cancellation_wins_over_native_refusal_and_never_rearms() {
    for already_closed in [false, true] {
        let scope = LocalExecutionScope::new();
        let host = Host::new(&scope);
        host.close_at.set(Some("encoding"));
        *host.encoding_error.borrow_mut() = Some(PrivateError("private encoding refusal".into()));
        let domain = host.domain(Type::UiFocus, 1);
        let drops = Rc::new(Cell::new(0));
        let frame = Frame {
            payload: "private frame".into(),
            drops: Rc::clone(&drops),
        };
        let mut pending = PendingReply::new("a", frame, &domain);
        if already_closed {
            scope.close();
        }
        let value = focus("a");
        let Value::UiFocus { node_id } = &value else {
            panic!()
        };
        let pointer = node_id.as_ptr();
        let rejected = pending
            .try_accept_in_scope(&"a", value, &host, &scope)
            .unwrap_err();
        assert!(matches!(
            rejected.error,
            ReplyAcceptanceError::Closed(ReplyEnd::Cancelled)
        ));
        assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Cancelled));
        let Value::UiFocus { node_id } = rejected.reply else {
            panic!()
        };
        assert_eq!(node_id.as_ptr(), pointer);
        assert_eq!(drops.get(), 1);
        assert_eq!(
            host.take_visits(),
            if already_closed {
                vec![]
            } else {
                vec!["authority", "encoding"]
            }
        );
        assert!(!pending.cancel());
    }
}
