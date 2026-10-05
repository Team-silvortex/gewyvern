use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::call_evaluation::{
    CallEvaluationError, CallEvaluationLimits, PreparedCall, evaluate_call_arguments_in_scope,
};
use leselang_hir::effect_evaluation::*;
use leselang_hir::ir::{Computation, ComputedArgument};
use leselang_hir::pure_evaluation::*;
use leselang_runtime_core::*;

struct Opaque;
type Ir = Computation<u8, u32, Opaque, ()>;
const LIMITS: EffectEvaluationLimits = EffectEvaluationLimits {
    pure: PureEvaluationLimits {
        max_nodes: 128,
        max_depth: 16,
        max_bindings: 8,
    },
    max_arguments: 4,
    max_branches: 4,
};
struct NativeError(&'static str);
struct Reply {
    kind: u8,
    value: u64,
}
struct Declaration(u8);
impl HostResultDomain<Reply> for Declaration {
    type Error = NativeError;
    fn matches_type(&self, reply: &Reply) -> bool {
        self.0 == reply.kind
    }
    fn validate_value(&self, reply: &Reply) -> Result<(), NativeError> {
        (reply.value <= 100)
            .then_some(())
            .ok_or(NativeError("private reply"))
    }
}
type Schema<'a> = OperationSchema<'a, u32, &'a str, ScalarTypeSet, Declaration, u8>;
struct Request<'schema> {
    prepared: PreparedCall<'schema, u32, ScalarTypeSet, Declaration, u8>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Request<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Capture<'expression> {
    name: &'expression str,
    body: &'expression Ir,
    bindings: Vec<(&'expression str, PureValue<Rc<Reply>>)>,
}
#[derive(Clone, Copy)]
enum CaptureMode {
    Accept,
    Reject,
    Unwind,
}
struct Counter<'schema> {
    catalog: OperationCatalog<'schema, u32, &'schema str, ScalarTypeSet, Declaration, u8>,
    grants: &'schema [u8],
    events: RefCell<Vec<&'static str>>,
    selected: RefCell<Vec<u32>>,
    request_drops: Rc<Cell<usize>>,
    capture_mode: Cell<CaptureMode>,
}
impl<'schema> Counter<'schema> {
    fn new(schemas: &'schema [Schema<'schema>], grants: &'schema [u8]) -> Self {
        Self {
            catalog: OperationCatalog::new(
                7,
                schemas,
                OperationCatalogLimits {
                    max_operations: 4,
                    max_parameters_per_operation: 4,
                },
            )
            .unwrap(),
            grants,
            events: RefCell::new(Vec::new()),
            selected: RefCell::new(Vec::new()),
            request_drops: Rc::new(Cell::new(0)),
            capture_mode: Cell::new(CaptureMode::Accept),
        }
    }
}
struct ReplyPolicy<'host, 'schema> {
    host: &'host Counter<'schema>,
    live: Cell<bool>,
}
impl ReplyAuthority<(u64, u32)> for ReplyPolicy<'_, '_> {
    type Error = NativeError;
    fn authorize(&self, identity: &(u64, u32)) -> Result<(), NativeError> {
        self.host.events.borrow_mut().push("reply-policy");
        if !self.live.get() {
            return Err(NativeError("private revoked reply authority"));
        }
        self.host
            .catalog
            .authorize(&identity.1, 7, self.host.grants)
            .map(|_| ())
            .map_err(|_| NativeError("private denied reply schema"))
    }
}
impl PureEvaluationEnvironment<u8, u32> for Counter<'_> {
    type Result = Rc<Reply>;
    type Error = NativeError;
    fn field(&self, reply: &Rc<Reply>, field: &u8) -> Result<ScalarValue, NativeError> {
        self.events.borrow_mut().push("field");
        if *field == 0 {
            Ok(ScalarValue::Integer(reply.value))
        } else {
            Err(NativeError("private projection failure"))
        }
    }
    fn member(&self, _: &Rc<Reply>, _: &str, _: &u32) -> Result<Rc<Reply>, NativeError> {
        Err(NativeError("not a group"))
    }
}
impl<'expression, 'schema> EffectEvaluationEnvironment<'expression, u8, u32, Opaque, ()>
    for Counter<'schema>
{
    type Request = Request<'schema>;
    type Capture = Capture<'expression>;
    fn preflight_effect(&self, expression: &'expression Ir) -> Result<(), NativeError> {
        self.events.borrow_mut().push("preflight");
        let Ir::Call {
            operation,
            arguments,
        } = expression
        else {
            return Err(NativeError("unsupported native effect"));
        };
        let schema = self
            .catalog
            .authorize(operation, 7, self.grants)
            .map_err(|_| NativeError("private denied schema"))?;
        let names = arguments
            .iter()
            .map(|value| value.name.as_str())
            .collect::<Vec<_>>();
        schema
            .bind_arguments(&names)
            .map_err(|_| NativeError("private call shape"))?;
        Ok(())
    }
    fn prepare_effect(
        &self,
        expression: &'expression Ir,
        scope: &mut ScopeFrame<'_, 'expression, PureValue<Rc<Reply>>>,
        fuel: &mut Fuel,
    ) -> Result<Request<'schema>, CalculationFailure<NativeError>> {
        self.events.borrow_mut().push("prepare");
        let Ir::Call {
            operation,
            arguments,
        } = expression
        else {
            panic!()
        };
        self.selected.borrow_mut().push(*operation);
        let schema = self
            .catalog
            .authorize(operation, 7, self.grants)
            .map_err(|_| NativeError("private denied schema"))?;
        let prepared = evaluate_call_arguments_in_scope(
            arguments,
            schema,
            scope,
            self,
            fuel,
            CallEvaluationLimits {
                pure: LIMITS.pure,
                max_arguments: LIMITS.max_arguments,
            },
        )
        .map_err(|failure| match failure {
            CallEvaluationError::Evaluation {
                failure: CalculationFailure::Scalar(error),
                ..
            } => CalculationFailure::Scalar(error),
            CallEvaluationError::Evaluation {
                failure: CalculationFailure::External(PureEvaluationFault::Native(error)),
                ..
            } => CalculationFailure::External(error),
            _ => CalculationFailure::External(NativeError("private argument failure")),
        })?;
        Ok(Request {
            prepared,
            drops: self.request_drops.clone(),
        })
    }
    fn capture(
        &self,
        name: &'expression str,
        body: &'expression Ir,
        scope: &ScopeFrame<'_, 'expression, PureValue<Rc<Reply>>>,
        fuel: &mut Fuel,
    ) -> Result<Capture<'expression>, NativeError> {
        self.events.borrow_mut().push("capture");
        match self.capture_mode.get() {
            CaptureMode::Reject => return Err(NativeError("private capture failure")),
            CaptureMode::Unwind => panic!("native capture unwind"),
            CaptureMode::Accept => {}
        }
        let mut bindings = Vec::new();
        for (name, value) in scope.bindings() {
            let cost = match value {
                PureValue::Scalar(value) => scalar_copy_cost(value),
                _ => 0,
            };
            fuel.charge(1 + cost)
                .map_err(|_| NativeError("capture fuel exhausted"))?;
            bindings.push((*name, value.clone()));
        }
        Ok(Capture {
            name,
            body,
            bindings,
        })
    }
}
fn integer(value: u64) -> Ir {
    Ir::Literal {
        value: ScalarValue::Integer(value),
    }
}
fn local(name: &str) -> Ir {
    Ir::Local { name: name.into() }
}
fn field(name: &str, field: u8) -> Ir {
    Ir::Field {
        value: Box::new(local(name)),
        field,
    }
}
fn add(left: Ir, right: Ir) -> Ir {
    Ir::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(left),
        right: Box::new(right),
    }
}
fn bind(name: &str, value: Ir, body: Ir) -> Ir {
    Ir::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn call(operation: u32, value: Ir) -> Ir {
    Ir::Call {
        operation,
        arguments: vec![ComputedArgument {
            name: "value".into(),
            value,
        }],
    }
}
fn schemas<'a>(parameters: &'a [NamedParameter<&'a str, ScalarTypeSet>]) -> [Schema<'a>; 2] {
    [
        OperationSchema {
            key: 1,
            parameters,
            result: Declaration(1),
            required_capability: 3,
        },
        OperationSchema {
            key: 2,
            parameters,
            result: Declaration(2),
            required_capability: 3,
        },
    ]
}
fn parameters() -> [NamedParameter<&'static str, ScalarTypeSet>; 1] {
    [NamedParameter::required(
        "value",
        ScalarTypeSet::only(ScalarType::Integer),
    )]
}
type CounterOutcome<'expression, 'schema> =
    EffectEvaluationOutcome<Rc<Reply>, Request<'schema>, Capture<'expression>>;
fn suspended<'expression, 'schema>(
    outcome: CounterOutcome<'expression, 'schema>,
) -> (Request<'schema>, Capture<'expression>) {
    let EffectEvaluationOutcome::Suspended { request, capture } = outcome else {
        panic!()
    };
    (request, capture)
}

#[test]
fn two_native_calls_capture_aliases_accept_actual_replies_and_resume_to_a_pure_tail() {
    let tail = add(field("second", 0), local("prefix"));
    let second = bind(
        "second",
        call(2, add(field("alias", 0), local("prefix"))),
        tail,
    );
    let alias = bind("alias", local("first"), second);
    let expression = bind(
        "prefix",
        integer(5),
        bind("first", call(1, integer(5)), alias),
    );
    let parameters = parameters();
    let schemas = schemas(&parameters);
    let host = Counter::new(&schemas, &[3]);
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    let (first, capture) = suspended(
        evaluate_effects_in_scope(&expression, &mut scope, &host, &mut fuel, LIMITS).unwrap(),
    );
    assert_eq!(scope.len(), 0);
    assert_eq!(fuel.remaining(), 94);
    assert!(std::ptr::eq(first.prepared.schema(), &schemas[0]));
    assert_eq!(
        capture
            .bindings
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>(),
        ["prefix"]
    );
    assert_eq!(first.prepared.arguments()[0].value, ScalarValue::Integer(5));
    // Invocation stays caller-owned; the shared handle owns one capture handoff.
    let policy = ReplyPolicy {
        host: &host,
        live: Cell::new(true),
    };
    let mut pending = PendingReply::new((7, 1), capture, &first.prepared.schema().result);
    let reply = Rc::new(Reply { kind: 1, value: 10 });
    let accepted = pending.try_accept(&(7, 1), reply, &policy).unwrap();
    assert_eq!(pending.status(), ReplyStatus::Terminal(ReplyEnd::Accepted));
    let (_, mut capture, declaration, reply) = accepted.into_parts();
    assert!(std::ptr::eq(declaration, &schemas[0].result));
    drop(first);
    fuel.charge(1).unwrap();
    capture
        .bindings
        .push((capture.name, PureValue::Result(reply)));
    let mut restored = ScopeFrame::new(&mut capture.bindings);
    let (second, capture2) = suspended(
        evaluate_effects_in_scope(capture.body, &mut restored, &host, &mut fuel, LIMITS).unwrap(),
    );
    assert_eq!(fuel.remaining(), 82);
    assert!(std::ptr::eq(second.prepared.schema(), &schemas[1]));
    assert_eq!(
        second.prepared.arguments()[0].value,
        ScalarValue::Integer(15)
    );
    assert_eq!(
        capture2
            .bindings
            .iter()
            .map(|(name, _)| *name)
            .collect::<Vec<_>>(),
        ["prefix", "first", "alias"]
    );
    let (PureValue::Result(first), PureValue::Result(alias)) =
        (&capture2.bindings[1].1, &capture2.bindings[2].1)
    else {
        panic!()
    };
    assert!(Rc::ptr_eq(first, alias));
    let mut pending2 = PendingReply::new((7, 2), capture2, &second.prepared.schema().result);
    let reply = Rc::new(Reply { kind: 2, value: 30 });
    let accepted = pending2.try_accept(&(7, 2), reply, &policy).unwrap();
    assert_eq!(pending2.status(), ReplyStatus::Terminal(ReplyEnd::Accepted));
    let (_, mut capture2, declaration, reply) = accepted.into_parts();
    assert!(std::ptr::eq(declaration, &schemas[1].result));
    drop(second);
    fuel.charge(3).unwrap();
    capture2
        .bindings
        .push((capture2.name, PureValue::Result(reply)));
    let mut restored2 = ScopeFrame::new(&mut capture2.bindings);
    assert!(matches!(
        evaluate_effects_in_scope(capture2.body, &mut restored2, &host, &mut fuel, LIMITS).unwrap(),
        EffectEvaluationOutcome::Value(PureValue::Scalar(ScalarValue::Integer(35)))
    ));
    assert_eq!(fuel.remaining(), 75);
    assert_eq!(*host.selected.borrow(), [1, 2]);
    assert_eq!(host.request_drops.get(), 2);
}

#[test]
fn wrong_actual_reply_is_rejected_before_the_host_restores_or_runs_the_body() {
    let expression = bind("reply", call(1, integer(5)), field("reply", 0));
    let parameters = parameters();
    let schemas = schemas(&parameters);
    let host = Counter::new(&schemas, &[3]);
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    let (request, capture) = suspended(
        evaluate_effects_in_scope(&expression, &mut scope, &host, &mut fuel, LIMITS).unwrap(),
    );
    let before = fuel.remaining();
    let policy = ReplyPolicy {
        host: &host,
        live: Cell::new(false),
    };
    let mut pending = PendingReply::new((7, 1), capture, &request.prepared.schema().result);
    host.events.borrow_mut().clear();
    let reply = Rc::new(Reply { kind: 1, value: 10 });
    let pointer = Rc::as_ptr(&reply);
    let rejected = pending.try_accept(&(8, 1), reply, &policy).unwrap_err();
    assert!(matches!(
        rejected.error,
        ReplyAcceptanceError::IdentityMismatch
    ));
    assert_eq!(Rc::as_ptr(&rejected.reply), pointer);
    assert!(host.events.borrow().is_empty());
    let rejected = pending
        .try_accept(&(7, 1), rejected.reply, &policy)
        .unwrap_err();
    assert!(matches!(rejected.error, ReplyAcceptanceError::Authority(_)));
    assert_eq!(Rc::as_ptr(&rejected.reply), pointer);
    assert_eq!(*host.events.borrow(), ["reply-policy"]);
    policy.live.set(true);
    assert!(matches!(
        pending.try_accept(&(7, 1), Rc::new(Reply { kind: 2, value: 1 }), &policy),
        Err(RejectedReply {
            error: ReplyAcceptanceError::Result(HostResultError::TypeMismatch),
            ..
        })
    ));
    assert!(matches!(
        pending.try_accept(
            &(7, 1),
            Rc::new(Reply {
                kind: 1,
                value: 101
            }),
            &policy
        ),
        Err(RejectedReply {
            error: ReplyAcceptanceError::Result(HostResultError::InvalidValue(_)),
            ..
        })
    ));
    assert_eq!(pending.status(), ReplyStatus::Pending);
    let events = host.events.borrow().len();
    assert!(pending.cancel());
    assert!(matches!(
        pending.try_accept(&(7, 1), rejected.reply, &policy),
        Err(RejectedReply {
            error: ReplyAcceptanceError::Closed(ReplyEnd::Cancelled),
            ..
        })
    ));
    assert_eq!(host.events.borrow().len(), events);
    assert_eq!(fuel.remaining(), before);
    assert!(!host.events.borrow().contains(&"field"));
}

#[test]
fn all_cold_schemas_are_checked_before_guard_values_and_fuel() {
    let expression = Ir::Choose {
        when: Box::new(Ir::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(call(1, integer(1))),
        otherwise: Box::new(call(99, integer(2))),
    };
    let parameters = parameters();
    let schemas = schemas(&parameters);
    for grants in [&[3][..], &[][..]] {
        let host = Counter::new(&schemas, grants);
        let mut bindings = Vec::new();
        let mut scope = ScopeFrame::new(&mut bindings);
        let mut fuel = Fuel::new(100);
        let failure = evaluate_effects_in_scope(&expression, &mut scope, &host, &mut fuel, LIMITS)
            .unwrap_err();
        assert!(matches!(
            failure,
            CalculationFailure::External(EffectEvaluationFault::Native(_))
        ));
        assert!(!failure.is_recoverable());
        assert!(!format!("{failure:?}").contains("private"));
        assert!(host.selected.borrow().is_empty());
        assert_eq!(fuel.remaining(), 100);
    }
}

#[test]
fn late_cold_structural_failure_precedes_every_native_hook() {
    let expression = Ir::Choose {
        when: Box::new(Ir::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(call(1, integer(1))),
        otherwise: Box::new(Ir::Literal {
            value: ScalarValue::String("private".repeat(600)),
        }),
    };
    let parameters = parameters();
    let schemas = schemas(&parameters);
    let host = Counter::new(&schemas, &[3]);
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    assert!(evaluate_effects_in_scope(&expression, &mut scope, &host, &mut fuel, LIMITS).is_err());
    assert!(host.events.borrow().is_empty());
    assert_eq!(fuel.remaining(), 100);
}

#[test]
fn selection_is_lazy_after_complete_cold_preflight_and_counts_one_root() {
    let expression = Ir::Choose {
        when: Box::new(Ir::Literal {
            value: ScalarValue::Boolean(false),
        }),
        then: Box::new(call(1, integer(1))),
        otherwise: Box::new(call(2, integer(2))),
    };
    let parameters = parameters();
    let schemas = schemas(&parameters);
    let host = Counter::new(&schemas, &[3]);
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    let outcome =
        evaluate_effects_in_scope(&expression, &mut scope, &host, &mut fuel, LIMITS).unwrap();
    assert_eq!(format!("{outcome:?}"), "Request");
    assert_eq!(*host.events.borrow(), ["preflight", "preflight", "prepare"]);
    assert_eq!(*host.selected.borrow(), [2]);
    assert_eq!(fuel.remaining(), 96);
}

#[test]
fn pure_recovery_composes_into_a_request_but_native_failure_never_enters_fallback() {
    let expression = bind(
        "amount",
        Ir::Recover {
            value: Box::new(Ir::Unary {
                operator: UnaryOperator::ParseInteger,
                value: Box::new(Ir::Literal {
                    value: ScalarValue::String("invalid".into()),
                }),
            }),
            fallback: Box::new(integer(7)),
        },
        call(1, local("amount")),
    );
    let native = Ir::Recover {
        value: Box::new(field("reply", 9)),
        fallback: Box::new(field("reply", 0)),
    };
    let parameters = parameters();
    let schemas = schemas(&parameters);
    let host = Counter::new(&schemas, &[3]);
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let EffectEvaluationOutcome::Request(request) =
        evaluate_effects_in_scope(&expression, &mut scope, &host, &mut Fuel::new(100), LIMITS)
            .unwrap()
    else {
        panic!()
    };
    assert_eq!(
        request.prepared.arguments()[0].value,
        ScalarValue::Integer(7)
    );
    assert_eq!(scope.len(), 0);
    drop(scope);
    let mut bindings = vec![(
        "reply",
        PureValue::Result(Rc::new(Reply { kind: 1, value: 10 })),
    )];
    let mut scope = ScopeFrame::new(&mut bindings);
    host.events.borrow_mut().clear();
    let failure =
        evaluate_effects_in_scope(&native, &mut scope, &host, &mut Fuel::new(100), LIMITS)
            .unwrap_err();
    assert!(!failure.is_recoverable());
    let CalculationFailure::External(EffectEvaluationFault::Pure(PureEvaluationFault::Native(
        NativeError(message),
    ))) = failure
    else {
        panic!()
    };
    assert_eq!(message, "private projection failure");
    assert_eq!(*host.events.borrow(), ["field"]);
}

#[test]
fn capture_failure_or_unwind_releases_prepared_request_and_temporary_scope() {
    let expression = bind(
        "temporary",
        integer(3),
        bind("reply", call(1, integer(5)), field("reply", 0)),
    );
    let parameters = parameters();
    let schemas = schemas(&parameters);
    for mode in [CaptureMode::Reject, CaptureMode::Unwind] {
        let host = Counter::new(&schemas, &[3]);
        host.capture_mode.set(mode);
        let mut bindings = vec![("prefix", PureValue::Scalar(ScalarValue::Integer(11)))];
        let mut scope = ScopeFrame::new(&mut bindings);
        let mut fuel = Fuel::new(100);
        let outcome = catch_unwind(AssertUnwindSafe(|| {
            evaluate_effects_in_scope(&expression, &mut scope, &host, &mut fuel, LIMITS)
        }));
        match mode {
            CaptureMode::Reject => {
                let failure = outcome.unwrap().unwrap_err();
                assert!(!failure.is_recoverable());
                assert!(!format!("{failure:?}").contains("private"));
            }
            CaptureMode::Unwind => assert!(outcome.is_err()),
            CaptureMode::Accept => unreachable!(),
        }
        assert_eq!(scope.len(), 1);
        assert!(scope.get("temporary").is_none());
        assert_eq!(host.request_drops.get(), 1);
        assert_eq!(*host.events.borrow(), ["preflight", "prepare", "capture"]);
    }
}

#[test]
fn nested_suspension_is_not_flattened_or_replayed() {
    let expression = bind(
        "outer",
        bind("inner", call(1, integer(1)), field("inner", 0)),
        local("outer"),
    );
    let parameters = parameters();
    let schemas = schemas(&parameters);
    let host = Counter::new(&schemas, &[3]);
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    assert!(matches!(
        evaluate_effects_in_scope(&expression, &mut scope, &host, &mut Fuel::new(100), LIMITS),
        Err(CalculationFailure::External(
            EffectEvaluationFault::NestedSuspension
        ))
    ));
    assert_eq!(host.request_drops.get(), 1);
    assert_eq!(*host.selected.borrow(), [1]);
    assert!(scope.is_empty());
}

#[test]
fn scope_limits_and_physical_budgets_reject_before_callbacks_and_value_clones() {
    let expression = call(1, integer(1));
    let parameters = parameters();
    let schemas = schemas(&parameters);
    let host = Counter::new(&schemas, &[3]);
    for limits in [
        EffectEvaluationLimits {
            pure: PureEvaluationLimits {
                max_nodes: 1,
                ..LIMITS.pure
            },
            ..LIMITS
        },
        EffectEvaluationLimits {
            pure: PureEvaluationLimits {
                max_depth: 0,
                ..LIMITS.pure
            },
            ..LIMITS
        },
        EffectEvaluationLimits {
            max_arguments: 65,
            ..LIMITS
        },
        EffectEvaluationLimits {
            max_branches: 65,
            ..LIMITS
        },
        EffectEvaluationLimits {
            pure: PureEvaluationLimits {
                max_bindings: 0,
                ..LIMITS.pure
            },
            ..LIMITS
        },
    ] {
        let mut bindings = vec![(
            "reply",
            PureValue::Result(Rc::new(Reply { kind: 1, value: 10 })),
        )];
        let mut scope = ScopeFrame::new(&mut bindings);
        let mut fuel = Fuel::new(100);
        assert!(
            evaluate_effects_in_scope(&expression, &mut scope, &host, &mut fuel, limits).is_err()
        );
        assert!(host.events.borrow().is_empty());
        assert_eq!(fuel.remaining(), 100);
    }
}

#[test]
fn malformed_group_names_and_impure_conditions_do_not_reach_host_hooks() {
    use leselang_hir::ir::{ComputedBranch, GroupKind};
    let group = Ir::Group {
        group_kind: GroupKind::Parallel,
        branches: vec![
            ComputedBranch {
                name: "same".into(),
                value: call(1, integer(1)),
                result_type: (),
            },
            ComputedBranch {
                name: "same".into(),
                value: call(2, integer(2)),
                result_type: (),
            },
        ],
    };
    let condition = Ir::Choose {
        when: Box::new(call(1, integer(1))),
        then: Box::new(integer(1)),
        otherwise: Box::new(integer(2)),
    };
    let parameters = parameters();
    let schemas = schemas(&parameters);
    let host = Counter::new(&schemas, &[3]);
    for expression in [&group, &condition] {
        let mut bindings = Vec::new();
        let mut scope = ScopeFrame::new(&mut bindings);
        let mut fuel = Fuel::new(100);
        assert!(
            evaluate_effects_in_scope(expression, &mut scope, &host, &mut fuel, LIMITS).is_err()
        );
        assert!(host.events.borrow().is_empty());
        assert_eq!(fuel.remaining(), 100);
    }
}

#[test]
fn zero_fuel_preflights_schema_but_never_prepares_or_captures() {
    let expression = bind("reply", call(1, integer(1)), field("reply", 0));
    let parameters = parameters();
    let schemas = schemas(&parameters);
    let host = Counter::new(&schemas, &[3]);
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let failure =
        evaluate_effects_in_scope(&expression, &mut scope, &host, &mut Fuel::new(0), LIMITS)
            .unwrap_err();
    assert!(matches!(
        failure,
        CalculationFailure::External(EffectEvaluationFault::Pure(
            PureEvaluationFault::FuelExhausted
        ))
    ));
    assert_eq!(*host.events.borrow(), ["preflight"]);
    assert_eq!(host.request_drops.get(), 0);
}

struct WidgetField;
struct WidgetOperation;
struct WidgetResultTag;
struct WidgetAction(Rc<Cell<bool>>);
impl ReplyAuthority<u64> for WidgetAction {
    type Error = NativeError;
    fn authorize(&self, _: &u64) -> Result<(), NativeError> {
        self.0
            .get()
            .then_some(())
            .ok_or(NativeError("widget reply is not live"))
    }
}
type WidgetIr = Computation<WidgetField, WidgetOperation, WidgetAction, WidgetResultTag>;
struct Widget;
struct WidgetCapture<'expression> {
    name: &'expression str,
    body: &'expression WidgetIr,
    bindings: Vec<(&'expression str, PureValue<Rc<String>>)>,
}
impl PureEvaluationEnvironment<WidgetField, WidgetOperation> for Widget {
    type Result = Rc<String>;
    type Error = NativeError;
    fn field(&self, value: &Rc<String>, _: &WidgetField) -> Result<ScalarValue, NativeError> {
        Ok(ScalarValue::String(value.as_ref().clone()))
    }
    fn member(
        &self,
        _: &Rc<String>,
        _: &str,
        _: &WidgetOperation,
    ) -> Result<Rc<String>, NativeError> {
        Err(NativeError("not a widget group"))
    }
}
impl<'expression>
    EffectEvaluationEnvironment<
        'expression,
        WidgetField,
        WidgetOperation,
        WidgetAction,
        WidgetResultTag,
    > for Widget
{
    type Request = &'expression WidgetAction;
    type Capture = WidgetCapture<'expression>;
    fn preflight_effect(&self, expression: &'expression WidgetIr) -> Result<(), NativeError> {
        match expression {
            WidgetIr::Host { effect } if !effect.0.get() => Ok(()),
            _ => Err(NativeError("private widget policy")),
        }
    }
    fn prepare_effect(
        &self,
        expression: &'expression WidgetIr,
        _: &mut ScopeFrame<'_, 'expression, PureValue<Rc<String>>>,
        _: &mut Fuel,
    ) -> Result<Self::Request, CalculationFailure<NativeError>> {
        let WidgetIr::Host { effect } = expression else {
            panic!()
        };
        Ok(effect)
    }
    fn capture(
        &self,
        name: &'expression str,
        body: &'expression WidgetIr,
        scope: &ScopeFrame<'_, 'expression, PureValue<Rc<String>>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Capture, NativeError> {
        let mut bindings = Vec::new();
        for (name, value) in scope.bindings() {
            fuel.charge(1)
                .map_err(|_| NativeError("widget capture limit"))?;
            bindings.push((*name, value.clone()));
        }
        Ok(WidgetCapture {
            name,
            body,
            bindings,
        })
    }
}
struct WidgetReply;
impl HostResultDomain<str> for WidgetReply {
    type Error = NativeError;
    fn matches_type(&self, _: &str) -> bool {
        true
    }
    fn validate_value(&self, value: &str) -> Result<(), NativeError> {
        (!value.is_empty() && value.len() <= 64)
            .then_some(())
            .ok_or(NativeError("private widget reply"))
    }
}

#[test]
fn unrelated_gui_local_host_borrows_nonclone_native_slots_and_owns_invocation() {
    let state = Rc::new(Cell::new(false));
    let expression = WidgetIr::Bind {
        name: "reply".into(),
        value: Box::new(WidgetIr::Host {
            effect: Box::new(WidgetAction(state.clone())),
        }),
        body: Box::new(WidgetIr::Field {
            value: Box::new(WidgetIr::Local {
                name: "reply".into(),
            }),
            field: WidgetField,
        }),
    };
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    let outcome =
        evaluate_effects_in_scope(&expression, &mut scope, &Widget, &mut fuel, LIMITS).unwrap();
    assert_eq!(format!("{outcome:?}"), "Suspended");
    assert!(!state.get());
    assert_eq!(fuel.remaining(), 98);
    let EffectEvaluationOutcome::Suspended { request, capture } = outcome else {
        panic!()
    };
    assert!(Rc::ptr_eq(&request.0, &state));
    // Only this caller-owned invocation changes native widget state.
    request.0.set(true);
    let declaration = WidgetReply;
    let mut pending = PendingReply::new(1u64, capture, &declaration);
    let reply = String::from("ready");
    let pointer = reply.as_ptr();
    let accepted = pending.try_accept(&1, reply, request).unwrap();
    let (_, mut capture, original, reply) = accepted.into_parts();
    assert!(std::ptr::eq(original, &declaration));
    assert_eq!(reply.as_ptr(), pointer);
    capture
        .bindings
        .push((capture.name, PureValue::Result(Rc::new(reply))));
    let mut restored = ScopeFrame::new(&mut capture.bindings);
    assert!(
        matches!(evaluate_effects_in_scope(capture.body, &mut restored, &Widget, &mut fuel, LIMITS).unwrap(),
        EffectEvaluationOutcome::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "ready")
    );
    assert_eq!(fuel.remaining(), 95);
    assert!(state.get());
}
