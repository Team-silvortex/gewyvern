use leselang_hir::{
    effect_evaluation::{
        EffectEvaluationEnvironment, EffectEvaluationLimits, evaluate_resumable_effects_in_scope,
    },
    effect_reentry::{EffectReentryEnvironment, RestoredEffectBindings},
    effect_stack::*,
    ir::{Computation, ComputedArgument},
    pure_evaluation::{
        PureEvaluationEnvironment, PureEvaluationLimits, PureValue, evaluate_pure_in_scope,
    },
};
use leselang_runtime_core::*;
use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

struct Field;
struct Operation(u8);
struct Opaque;
struct Declaration;
type Node = Computation<Field, Operation, Opaque, Declaration>;
const EFFECT: EffectEvaluationLimits = EffectEvaluationLimits {
    pure: PureEvaluationLimits {
        max_nodes: 128,
        max_depth: 32,
        max_bindings: 8,
    },
    max_arguments: 4,
    max_branches: 4,
};
const LIMITS: EffectStackLimits = EffectStackLimits {
    effects: EFFECT,
    max_frames: 8,
};
struct Private;
#[derive(Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    CaptureError,
    CaptureUnwind,
    RestoreError,
    RestoreUnwind,
    Duplicate,
    Shadow,
    Unbounded,
    ProjectionError,
    ProjectionUnwind,
}
struct Trace {
    live: Cell<bool>,
    denied: Cell<u8>,
    mode: Cell<Mode>,
    target: Cell<usize>,
    events: RefCell<Vec<&'static str>>,
    prepared: Cell<usize>,
    captures: Cell<usize>,
    restores: Cell<usize>,
    projections: Cell<usize>,
    request_drops: Cell<usize>,
    capture_drops: Cell<usize>,
    reply_drops: Cell<usize>,
    capture_drop_panic: Cell<bool>,
    calls: Cell<usize>,
    actual: Cell<u64>,
}
impl Trace {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            live: Cell::new(true),
            denied: Cell::new(0),
            mode: Cell::new(Mode::Normal),
            target: Cell::new(2),
            events: RefCell::new(Vec::new()),
            prepared: Cell::new(0),
            captures: Cell::new(0),
            restores: Cell::new(0),
            projections: Cell::new(0),
            request_drops: Cell::new(0),
            capture_drops: Cell::new(0),
            reply_drops: Cell::new(0),
            capture_drop_panic: Cell::new(false),
            calls: Cell::new(0),
            actual: Cell::new(41),
        })
    }
}
struct Domain {
    index: u8,
    trace: Rc<Trace>,
}
impl HostResultDomain<ScalarValue> for Domain {
    type Error = Private;
    fn matches_type(&self, value: &ScalarValue) -> bool {
        matches!(value, ScalarValue::Integer(_))
    }
    fn validate_value(&self, value: &ScalarValue) -> Result<(), Private> {
        self.trace.events.borrow_mut().push("domain");
        if matches!(value, ScalarValue::Integer(0..=200)) {
            Ok(())
        } else {
            Err(Private)
        }
    }
}
type NativeDomain<'s> = dyn HostResultDomain<ScalarValue, Error = Private> + 's;
struct Request<'e> {
    operation: &'e Operation,
    amount: u64,
    owner: Rc<Trace>,
}
impl Drop for Request<'_> {
    fn drop(&mut self) {
        self.owner
            .request_drops
            .set(self.owner.request_drops.get() + 1);
    }
}
struct Reply {
    value: ScalarValue,
    buffer: Box<u8>,
    owner: Rc<Trace>,
}
impl Borrow<ScalarValue> for Reply {
    fn borrow(&self) -> &ScalarValue {
        &self.value
    }
}
impl Drop for Reply {
    fn drop(&mut self) {
        self.owner.reply_drops.set(self.owner.reply_drops.get() + 1);
    }
}
struct Capture<'e> {
    values: RestoredEffectBindings<'e, ()>,
    owner: Rc<Trace>,
}
impl Drop for Capture<'_> {
    fn drop(&mut self) {
        self.owner
            .capture_drops
            .set(self.owner.capture_drops.get() + 1);
        assert!(
            !self.owner.capture_drop_panic.replace(false),
            "native capture cleanup unwind"
        );
    }
}
struct Host<'s> {
    domains: &'s [Domain; 2],
    trace: Rc<Trace>,
}
impl Host<'_> {
    fn reply(&self, value: u64) -> Reply {
        Reply {
            value: ScalarValue::Integer(value),
            buffer: Box::new(17),
            owner: self.trace.clone(),
        }
    }
    fn invoke(&self, request: Request<'_>) -> Reply {
        assert!(self.trace.live.get());
        self.trace.calls.set(self.trace.calls.get() + 1);
        if request.operation.0 == 2 {
            self.trace.actual.set(request.amount);
        }
        self.reply(self.trace.actual.get())
    }
}
impl ReplyAuthority<(u64, u8)> for Host<'_> {
    type Error = Private;
    fn authorize(&self, &(owner, index): &(u64, u8)) -> Result<(), Private> {
        if self.trace.live.get()
            && owner == 73
            && (1..=2).contains(&index)
            && index != self.trace.denied.get()
        {
            Ok(())
        } else {
            Err(Private)
        }
    }
}
impl PureEvaluationEnvironment<Field, Operation> for Host<'_> {
    type Result = ();
    type Error = Private;
    fn field(&self, _: &(), _: &Field) -> Result<ScalarValue, Private> {
        Err(Private)
    }
    fn member(&self, _: &(), _: &str, _: &Operation) -> Result<(), Private> {
        Err(Private)
    }
}
impl<'e> EffectEvaluationEnvironment<'e, Field, Operation, Opaque, Declaration> for Host<'_> {
    type Request = Request<'e>;
    type Capture = Capture<'e>;
    fn preflight_effect(&self, node: &Node) -> Result<(), Private> {
        self.trace.events.borrow_mut().push("cold");
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err(Private);
        };
        self.authorize(&(73, operation.0))?;
        if arguments.len() == 1 && arguments[0].name == "amount" {
            Ok(())
        } else {
            Err(Private)
        }
    }
    fn prepare_effect(
        &self,
        node: &'e Node,
        scope: &mut ScopeFrame<'_, 'e, PureValue<()>>,
        fuel: &mut Fuel,
    ) -> Result<Request<'e>, CalculationFailure<Private>> {
        self.trace.events.borrow_mut().push("prepare");
        self.trace.prepared.set(self.trace.prepared.get() + 1);
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err(Private.into());
        };
        self.authorize(&(73, operation.0))?;
        let PureValue::Scalar(ScalarValue::Integer(amount)) =
            evaluate_pure_in_scope(&arguments[0].value, scope, self, fuel, EFFECT.pure)
                .map_err(|_| Private)?
        else {
            return Err(Private.into());
        };
        Ok(Request {
            operation,
            amount,
            owner: self.trace.clone(),
        })
    }
    fn capture(
        &self,
        _: &'e str,
        _: &'e Node,
        scope: &ScopeFrame<'_, 'e, PureValue<()>>,
        fuel: &mut Fuel,
    ) -> Result<Capture<'e>, Private> {
        self.trace.events.borrow_mut().push("capture");
        let index = self.trace.captures.get() + 1;
        self.trace.captures.set(index);
        if index == self.trace.target.get() {
            if self.trace.mode.get() == Mode::CaptureError {
                return Err(Private);
            }
            assert!(
                self.trace.mode.get() != Mode::CaptureUnwind,
                "native capture unwind"
            );
        }
        fuel.charge(scope.len() as u64).map_err(|_| Private)?;
        Ok(Capture {
            values: scope.bindings().to_vec(),
            owner: self.trace.clone(),
        })
    }
}
impl<'e, 's>
    EffectReentryEnvironment<
        'e,
        Field,
        Operation,
        Opaque,
        Declaration,
        (u64, u8),
        NativeDomain<'s>,
        Reply,
    > for Host<'s>
{
    fn restore_capture(
        &self,
        identity: &(u64, u8),
        domain: &NativeDomain<'s>,
        mut capture: Capture<'e>,
        _: &mut Fuel,
        _: EffectEvaluationLimits,
    ) -> Result<RestoredEffectBindings<'e, ()>, CalculationFailure<Private>> {
        self.authorize(identity)?;
        if !std::ptr::eq(
            domain,
            &self.domains[usize::from(identity.1 - 1)] as &NativeDomain<'s>,
        ) {
            return Err(Private.into());
        }
        self.trace.events.borrow_mut().push("restore");
        let index = self.trace.restores.get() + 1;
        self.trace.restores.set(index);
        if index == self.trace.target.get() {
            match self.trace.mode.get() {
                Mode::RestoreError => return Err(Private.into()),
                Mode::RestoreUnwind => panic!("native restoration unwind"),
                Mode::Duplicate => {
                    return Ok(vec![
                        ("same", PureValue::Scalar(ScalarValue::Integer(1))),
                        ("same", PureValue::Scalar(ScalarValue::Integer(2))),
                    ]);
                }
                Mode::Shadow => {
                    return Ok(vec![("answer", PureValue::Scalar(ScalarValue::Integer(9)))]);
                }
                Mode::Unbounded => {
                    return Ok(vec![(
                        "bad",
                        PureValue::Scalar(ScalarValue::String(
                            "x".repeat(MAX_SCALAR_STRING_BYTES + 1),
                        )),
                    )]);
                }
                _ => {}
            }
        }
        Ok(std::mem::take(&mut capture.values))
    }
    fn bind_reply(
        &self,
        identity: &(u64, u8),
        domain: &NativeDomain<'s>,
        mut reply: Reply,
        _: &mut Fuel,
    ) -> Result<PureValue<()>, CalculationFailure<Private>> {
        self.authorize(identity)?;
        assert!(std::ptr::eq(
            domain,
            &self.domains[usize::from(identity.1 - 1)] as &NativeDomain<'s>
        ));
        self.trace.events.borrow_mut().push("project");
        self.trace.projections.set(self.trace.projections.get() + 1);
        if self.trace.mode.get() == Mode::ProjectionError {
            return Err(Private.into());
        }
        assert!(
            self.trace.mode.get() != Mode::ProjectionUnwind,
            "native projection unwind"
        );
        Ok(PureValue::Scalar(std::mem::replace(
            &mut reply.value,
            ScalarValue::None,
        )))
    }
}
fn fixture(test: impl FnOnce(&Host<'_>)) {
    let trace = Trace::new();
    let domains = [
        Domain {
            index: 1,
            trace: trace.clone(),
        },
        Domain {
            index: 2,
            trace: trace.clone(),
        },
    ];
    assert_eq!(domains[0].index, 1);
    test(&Host {
        domains: &domains,
        trace,
    });
}
fn integer(value: u64) -> Node {
    Node::Literal {
        value: ScalarValue::Integer(value),
    }
}
fn variable(name: &str) -> Node {
    Node::Local { name: name.into() }
}
fn add(left: Node, right: Node) -> Node {
    Node::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(left),
        right: Box::new(right),
    }
}
fn bind(name: &str, value: Node, body: Node) -> Node {
    Node::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn call(index: u8, amount: Node) -> Node {
    Node::Call {
        operation: Operation(index),
        arguments: vec![ComputedArgument {
            name: "amount".into(),
            value: amount,
        }],
    }
}
fn program() -> Node {
    bind(
        "caller",
        integer(10),
        bind(
            "answer",
            bind(
                "helper",
                integer(1),
                bind(
                    "read",
                    call(1, integer(0)),
                    bind(
                        "write",
                        call(2, add(variable("read"), variable("helper"))),
                        add(variable("write"), variable("helper")),
                    ),
                ),
            ),
            add(variable("answer"), variable("caller")),
        ),
    )
}
type Stack<'e> = EffectReturnStack<'e, Node, Capture<'e>>;
fn start<'e>(
    node: &'e Node,
    host: &Host<'_>,
    fuel: &mut Fuel,
    limits: EffectStackLimits,
) -> (Request<'e>, Stack<'e>) {
    let mut values = Vec::new();
    let mut scope = ScopeFrame::new(&mut values);
    let StackedEffectOutcome::Suspended {
        request,
        continuation,
    } = evaluate_stacked_effects_in_scope(node, &mut scope, host, fuel, limits).unwrap()
    else {
        panic!()
    };
    assert!(scope.bindings().is_empty());
    (request, continuation)
}
fn resume<'e, 's>(
    request: Request<'e>,
    stack: Stack<'e>,
    host: &Host<'s>,
    fuel: &mut Fuel,
    limits: EffectStackLimits,
) -> StackedControlResult<'e, Field, Operation, Opaque, Declaration, Host<'s>> {
    let key = (73, request.operation.0);
    let domain: &NativeDomain<'s> = &host.domains[usize::from(key.1 - 1)];
    let reply = host.invoke(request);
    let mut pending = PendingReply::new(key, stack, domain);
    let accepted = pending
        .try_accept::<_, ScalarValue, _>(&key, reply, host)
        .unwrap();
    resume_accepted_stacked_effects(accepted, host, fuel, limits)
}

#[test]
fn nested_helpers_return_through_original_scopes_and_repeated_requests_without_fuel_reset() {
    fixture(|host| {
        let node = program();
        let mut fuel = Fuel::new(100);
        let (request, stack) = start(&node, host, &mut fuel, LIMITS);
        assert_eq!(stack.frame_count(), 2);
        assert_eq!(format!("{stack:?}"), "EffectReturnStack { frames: 2 }");
        assert_eq!(stack.frames()[0].name(), "read");
        assert_eq!(stack.frames()[1].name(), "answer");
        assert_eq!(request.amount, 0);
        assert_eq!(host.trace.calls.get(), 0);
        assert_eq!(fuel.remaining(), 89);
        let StackedEffectOutcome::Suspended {
            request,
            continuation,
        } = resume(request, stack, host, &mut fuel, LIMITS).unwrap()
        else {
            panic!()
        };
        assert_eq!(request.amount, 42);
        assert_eq!(continuation.frame_count(), 2);
        assert_eq!(continuation.frames()[0].name(), "write");
        assert_eq!(continuation.frames()[1].name(), "answer");
        assert_eq!(fuel.remaining(), 79);
        let StackedEffectOutcome::Value(PureValue::Scalar(value)) =
            resume(request, continuation, host, &mut fuel, LIMITS).unwrap()
        else {
            panic!()
        };
        assert_eq!(value, ScalarValue::Integer(53));
        assert_eq!(host.trace.actual.get(), 42);
        assert_eq!(host.trace.calls.get(), 2);
        assert_eq!(host.trace.projections.get(), 2);
        assert_eq!(host.trace.restores.get(), 3);
        assert_eq!(host.trace.captures.get(), 3);
        assert_eq!(host.trace.capture_drops.get(), 3);
        assert_eq!(host.trace.request_drops.get(), 2);
        assert_eq!(host.trace.reply_drops.get(), 2);
        assert_eq!(fuel.remaining(), 69);
    });
}

#[test]
fn request_tail_adopts_the_next_original_return_site_without_replaying_inner_effects() {
    fixture(|host| {
        let node = bind(
            "answer",
            bind(
                "first",
                call(1, integer(0)),
                call(2, add(variable("first"), integer(1))),
            ),
            add(variable("answer"), integer(1)),
        );
        let mut fuel = Fuel::new(100);
        let (request, stack) = start(&node, host, &mut fuel, LIMITS);
        let StackedEffectOutcome::Suspended {
            request,
            continuation,
        } = resume(request, stack, host, &mut fuel, LIMITS).unwrap()
        else {
            panic!()
        };
        assert_eq!(continuation.frame_count(), 1);
        assert_eq!(continuation.frames()[0].name(), "answer");
        assert_eq!(request.amount, 42);
        let StackedEffectOutcome::Value(PureValue::Scalar(ScalarValue::Integer(43))) =
            resume(request, continuation, host, &mut fuel, LIMITS).unwrap()
        else {
            panic!()
        };
        assert_eq!(host.trace.calls.get(), 2);
        assert_eq!(host.trace.projections.get(), 2);
    });
}

#[test]
fn legacy_single_frame_entry_still_rejects_nested_suspension_before_outer_capture() {
    fixture(|host| {
        let node = program();
        let mut fuel = Fuel::new(100);
        let mut values = Vec::new();
        assert!(
            evaluate_resumable_effects_in_scope(
                &node,
                &mut ScopeFrame::new(&mut values),
                host,
                &mut fuel,
                EFFECT
            )
            .is_err()
        );
        assert_eq!(host.trace.captures.get(), 1);
        assert_eq!(host.trace.capture_drops.get(), 1);
        assert_eq!(host.trace.request_drops.get(), 1);
        assert_eq!(host.trace.calls.get(), 0);
        assert!(values.is_empty());
    });
}

#[test]
fn return_quota_is_inclusive_shared_across_new_and_pending_frames_and_never_refunds() {
    fixture(|host| {
        let node = program();
        let mut fuel = Fuel::new(100);
        let limits = EffectStackLimits {
            max_frames: 1,
            ..LIMITS
        };
        let mut values = Vec::new();
        let error = evaluate_stacked_effects_in_scope(
            &node,
            &mut ScopeFrame::new(&mut values),
            host,
            &mut fuel,
            limits,
        )
        .err()
        .unwrap();
        assert!(matches!(
            error,
            CalculationFailure::External(EffectStackFault::ReturnLimit)
        ));
        assert_eq!(host.trace.captures.get(), 1);
        assert_eq!(host.trace.capture_drops.get(), 1);
        assert_eq!(host.trace.calls.get(), 0);
        assert!(fuel.remaining() < 100);
    });
    fixture(|host| {
        let node = program();
        let mut fuel = Fuel::new(100);
        let limits = EffectStackLimits {
            max_frames: 2,
            ..LIMITS
        };
        let (request, stack) = start(&node, host, &mut fuel, limits);
        let StackedEffectOutcome::Suspended {
            request,
            continuation,
        } = resume(request, stack, host, &mut fuel, limits).unwrap()
        else {
            panic!()
        };
        assert_eq!(continuation.frame_count(), 2);
        assert!(resume(request, continuation, host, &mut fuel, limits).is_ok());
    });
    fixture(|host| {
        let node = program();
        let mut fuel = Fuel::new(100);
        let (request, stack) = start(&node, host, &mut fuel, LIMITS);
        let before = fuel.remaining();
        assert!(
            resume(
                request,
                stack,
                host,
                &mut fuel,
                EffectStackLimits {
                    max_frames: 1,
                    ..LIMITS
                }
            )
            .is_err()
        );
        assert_eq!(fuel.remaining(), before);
        assert_eq!(host.trace.restores.get(), 0);
        assert_eq!(host.trace.projections.get(), 0);
    });
}

#[test]
fn invalid_limits_and_cold_whole_return_forest_denial_precede_restore_projection_and_fuel() {
    fixture(|host| {
        let mut values = Vec::new();
        let mut fuel = Fuel::new(100);
        assert!(
            evaluate_stacked_effects_in_scope(
                &integer(1),
                &mut ScopeFrame::new(&mut values),
                host,
                &mut fuel,
                EffectStackLimits {
                    max_frames: MAX_EFFECT_RETURN_FRAMES + 1,
                    ..LIMITS
                }
            )
            .is_err()
        );
        assert!(host.trace.events.borrow().is_empty());
        assert_eq!(fuel.remaining(), 100);
    });
    fixture(|host| {
        let node = bind(
            "answer",
            bind("read", call(1, integer(0)), variable("read")),
            variable("answer"),
        );
        let mut fuel = Fuel::new(100);
        let (request, stack) = start(&node, host, &mut fuel, LIMITS);
        let before = fuel.remaining();
        let mut limits = LIMITS;
        limits.effects.pure.max_nodes = 1; // Each body fits alone; their complete forest does not.
        assert!(resume(request, stack, host, &mut fuel, limits).is_err());
        assert_eq!(fuel.remaining(), before);
        assert_eq!(host.trace.restores.get(), 0);
        assert_eq!(host.trace.projections.get(), 0);
    });
}

#[test]
fn newly_suspended_helper_frames_cannot_reset_the_pending_outer_return_quota() {
    fixture(|host| {
        let node = bind(
            "answer",
            bind(
                "read",
                call(1, integer(0)),
                bind(
                    "middle",
                    bind("second", call(2, variable("read")), variable("second")),
                    variable("middle"),
                ),
            ),
            variable("answer"),
        );
        let limits = EffectStackLimits {
            max_frames: 2,
            ..LIMITS
        };
        let mut fuel = Fuel::new(100);
        let (request, stack) = start(&node, host, &mut fuel, limits);
        let error = resume(request, stack, host, &mut fuel, limits)
            .err()
            .unwrap();
        assert!(matches!(
            error,
            CalculationFailure::External(EffectStackFault::ReturnLimit)
        ));
        assert_eq!(host.trace.calls.get(), 1);
        assert_eq!(host.trace.prepared.get(), 2);
        assert_eq!(host.trace.captures.get(), 3);
        assert_eq!(host.trace.capture_drops.get(), 3);
        assert_eq!(host.trace.request_drops.get(), 2);
    });
}

#[test]
fn cold_denial_in_a_later_outer_body_precedes_every_inner_restore_and_projection() {
    fixture(|host| {
        let node = bind(
            "answer",
            bind("read", call(1, integer(0)), variable("read")),
            call(2, variable("answer")),
        );
        let mut fuel = Fuel::new(100);
        let (request, stack) = start(&node, host, &mut fuel, LIMITS);
        host.trace.denied.set(2);
        let before = fuel.remaining();
        assert!(resume(request, stack, host, &mut fuel, LIMITS).is_err());
        assert_eq!(host.trace.calls.get(), 1);
        assert_eq!(host.trace.prepared.get(), 1);
        assert_eq!(host.trace.restores.get(), 0);
        assert_eq!(host.trace.projections.get(), 0);
        assert_eq!(host.trace.capture_drops.get(), 2);
        assert_eq!(fuel.remaining(), before);
    });
}

#[test]
fn wrong_identity_preserves_original_input_and_cancellation_discards_all_return_frames() {
    fixture(|host| {
        let node = program();
        let mut fuel = Fuel::new(100);
        let (request, stack) = start(&node, host, &mut fuel, LIMITS);
        drop(request);
        let reply = host.reply(41);
        let pointer = &*reply.buffer as *const u8;
        let mut pending = PendingReply::new((73, 1), stack, &host.domains[0] as &NativeDomain<'_>);
        let rejected = pending
            .try_accept::<_, ScalarValue, _>(&(74, 1), reply, host)
            .unwrap_err();
        assert_eq!(&*rejected.reply.buffer as *const u8, pointer);
        assert_eq!(host.trace.capture_drops.get(), 0);
        assert!(pending.cancel());
        assert_eq!(host.trace.capture_drops.get(), 2);
        assert!(
            pending
                .try_accept::<_, ScalarValue, _>(&(73, 1), rejected.reply, host)
                .is_err()
        );
        assert_eq!(host.trace.restores.get(), 0);
        assert_eq!(host.trace.projections.get(), 0);
        assert_eq!(host.trace.calls.get(), 0);
    });
}

#[test]
fn live_revocation_after_acceptance_and_cancel_cleanup_unwind_cannot_rearm_frames() {
    fixture(|host| {
        let node = program();
        let mut fuel = Fuel::new(100);
        let (request, stack) = start(&node, host, &mut fuel, LIMITS);
        drop(request);
        let mut pending = PendingReply::new((73, 1), stack, &host.domains[0] as &NativeDomain<'_>);
        let accepted = pending
            .try_accept::<_, ScalarValue, _>(&(73, 1), host.reply(41), host)
            .unwrap();
        host.trace.live.set(false);
        let before = fuel.remaining();
        assert!(resume_accepted_stacked_effects(accepted, host, &mut fuel, LIMITS).is_err());
        assert_eq!(fuel.remaining(), before);
        assert_eq!(host.trace.restores.get(), 0);
        assert_eq!(host.trace.capture_drops.get(), 2);
    });
    fixture(|host| {
        let node = program();
        let mut fuel = Fuel::new(100);
        let (request, stack) = start(&node, host, &mut fuel, LIMITS);
        drop(request);
        let mut pending = PendingReply::new((73, 1), stack, &host.domains[0] as &NativeDomain<'_>);
        host.trace.capture_drop_panic.set(true);
        assert!(catch_unwind(AssertUnwindSafe(|| pending.cancel())).is_err());
        assert!(!pending.cancel());
        assert_eq!(host.trace.capture_drops.get(), 2);
        assert!(
            pending
                .try_accept::<_, ScalarValue, _>(&(73, 1), host.reply(41), host)
                .is_err()
        );
    });
}

#[test]
fn outer_capture_failure_or_unwind_releases_inner_frames_and_prepared_request_once() {
    for mode in [Mode::CaptureError, Mode::CaptureUnwind] {
        fixture(|host| {
            host.trace.mode.set(mode);
            let node = program();
            let mut fuel = Fuel::new(100);
            let mut values = Vec::new();
            let result = catch_unwind(AssertUnwindSafe(|| {
                evaluate_stacked_effects_in_scope(
                    &node,
                    &mut ScopeFrame::new(&mut values),
                    host,
                    &mut fuel,
                    LIMITS,
                )
            }));
            if mode == Mode::CaptureUnwind {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(host.trace.captures.get(), 2);
            assert_eq!(host.trace.capture_drops.get(), 1);
            assert_eq!(host.trace.request_drops.get(), 1);
            assert_eq!(host.trace.calls.get(), 0);
            assert!(values.is_empty());
        });
    }
}

#[test]
fn restoration_and_projection_failures_unwind_or_bad_outer_prefix_never_replay_dispatch() {
    for mode in [
        Mode::RestoreError,
        Mode::RestoreUnwind,
        Mode::Duplicate,
        Mode::Shadow,
        Mode::Unbounded,
        Mode::ProjectionError,
        Mode::ProjectionUnwind,
    ] {
        fixture(|host| {
            let node = bind(
                "answer",
                bind("read", call(1, integer(0)), variable("read")),
                variable("answer"),
            );
            let mut fuel = Fuel::new(100);
            let (request, stack) = start(&node, host, &mut fuel, LIMITS);
            host.trace.mode.set(mode);
            let result = catch_unwind(AssertUnwindSafe(|| {
                resume(request, stack, host, &mut fuel, LIMITS)
            }));
            if matches!(mode, Mode::RestoreUnwind | Mode::ProjectionUnwind) {
                assert!(result.is_err());
            } else {
                let error = result.unwrap().err().unwrap();
                assert!(!format!("{error:?}").contains("answer"));
            }
            assert_eq!(host.trace.capture_drops.get(), 2);
            assert_eq!(host.trace.request_drops.get(), 1);
            assert_eq!(host.trace.reply_drops.get(), 1);
            assert_eq!(host.trace.calls.get(), 1);
            assert_eq!(host.trace.prepared.get(), 1);
        });
    }
}

#[test]
fn zero_frame_pure_and_final_request_paths_remain_available_without_native_debug_or_clone_bounds() {
    fixture(|host| {
        let limits = EffectStackLimits {
            max_frames: 0,
            ..LIMITS
        };
        let mut fuel = Fuel::new(100);
        let mut values = Vec::new();
        let pure = integer(1);
        assert!(matches!(
            evaluate_stacked_effects_in_scope(
                &pure,
                &mut ScopeFrame::new(&mut values),
                host,
                &mut fuel,
                limits
            )
            .unwrap(),
            StackedEffectOutcome::Value(_)
        ));
        let node = call(1, integer(0));
        assert!(matches!(
            evaluate_stacked_effects_in_scope(
                &node,
                &mut ScopeFrame::new(&mut values),
                host,
                &mut fuel,
                limits
            )
            .unwrap(),
            StackedEffectOutcome::Request(_)
        ));
        assert_eq!(host.trace.captures.get(), 0);
        assert_eq!(host.trace.calls.get(), 0);
    });
}
