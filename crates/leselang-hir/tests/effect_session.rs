use std::borrow::Borrow;
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::effect_evaluation::*;
use leselang_hir::effect_reentry::*;
use leselang_hir::effect_session::*;
use leselang_hir::effect_stack::{
    EffectStackFault, EffectStackLimits, MAX_EFFECT_RETURN_FRAMES, SavedEffectReturnFrame,
    StackedEffectOutcome, evaluate_stacked_effects_in_scope,
};
use leselang_hir::effect_stack_session::*;
use leselang_hir::ir::{Computation, ComputedArgument};
use leselang_hir::local_effect_session::LocalStackedEffectSession;
use leselang_hir::pure_evaluation::*;
use leselang_runtime_core::*;

struct Field;
struct Operation(u8);
struct Opaque;
struct Tag;
type Node = Computation<Field, Operation, Opaque, Tag>;
const LIMITS: EffectEvaluationLimits = EffectEvaluationLimits {
    pure: PureEvaluationLimits {
        max_nodes: 128,
        max_depth: 16,
        max_bindings: 8,
    },
    max_arguments: 4,
    max_branches: 4,
};
struct PrivateError(&'static str);
thread_local! {
    static PRIVATE_ERROR_DROPS: Cell<usize> = const { Cell::new(0) };
}
impl Drop for PrivateError {
    fn drop(&mut self) {
        PRIVATE_ERROR_DROPS.with(|drops| drops.set(drops.get() + 1));
    }
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Stage {
    Preflight,
    Prepare,
    Capture,
    Correlate,
    Restore,
    Project,
    Equality,
    Authority,
    Borrow,
    Type,
    Domain,
}
#[derive(Clone, Copy, Eq, PartialEq)]
enum Mode {
    Normal,
    Reject,
    Unwind,
    Unbounded,
    Owned,
}
struct Hooks {
    fault: Cell<(Stage, Mode)>,
    events: RefCell<Vec<Stage>>,
    request_drops: Cell<usize>,
    capture_drops: Cell<usize>,
    reply_drops: Cell<usize>,
    key_drops: Cell<usize>,
    capture_drop_panic: Cell<bool>,
    key_drop_panic: Cell<bool>,
    lease: RefCell<Option<LocalExecutionLease>>,
    cleanup_liveness: RefCell<Vec<(&'static str, bool)>>,
    correlation_cleanup_panic: Cell<bool>,
}
impl Hooks {
    fn new() -> Rc<Self> {
        Rc::new(Self {
            fault: Cell::new((Stage::Prepare, Mode::Normal)),
            events: RefCell::new(Vec::new()),
            request_drops: Cell::new(0),
            capture_drops: Cell::new(0),
            reply_drops: Cell::new(0),
            key_drops: Cell::new(0),
            capture_drop_panic: Cell::new(false),
            key_drop_panic: Cell::new(false),
            lease: RefCell::new(None),
            cleanup_liveness: RefCell::new(Vec::new()),
            correlation_cleanup_panic: Cell::new(false),
        })
    }
    fn hit(&self, stage: Stage) -> Result<(), PrivateError> {
        self.events.borrow_mut().push(stage);
        if self.fault.get().0 == stage {
            match self.fault.get().1 {
                Mode::Reject => return Err(PrivateError("private native rejection")),
                Mode::Unwind => panic!("native hook unwind"),
                _ => {}
            }
        }
        Ok(())
    }
    fn observe_cleanup(&self, role: &'static str) {
        if let Some(lease) = self.lease.borrow().as_ref() {
            self.cleanup_liveness
                .borrow_mut()
                .push((role, !lease.is_open()));
        }
    }
}
struct Identity {
    serial: Box<usize>,
    op: u8,
    hooks: Rc<Hooks>,
    tracked: bool,
}
impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        self.hooks
            .hit(Stage::Equality)
            .unwrap_or_else(|_| panic!("identity equality rejected"));
        self.serial == other.serial && self.op == other.op
    }
}
impl Drop for Identity {
    fn drop(&mut self) {
        if self.tracked {
            self.hooks.observe_cleanup("identity");
            self.hooks.key_drops.set(self.hooks.key_drops.get() + 1);
            assert!(!self.hooks.key_drop_panic.get(), "native key drop unwind");
        }
    }
}
struct Reply {
    value: ScalarValue,
    buffer: Box<u8>,
    hooks: Rc<Hooks>,
}
impl Borrow<ScalarValue> for Reply {
    fn borrow(&self) -> &ScalarValue {
        self.hooks
            .hit(Stage::Borrow)
            .unwrap_or_else(|_| panic!("borrow rejected"));
        &self.value
    }
}
impl Drop for Reply {
    fn drop(&mut self) {
        self.hooks.observe_cleanup("reply");
        self.hooks.reply_drops.set(self.hooks.reply_drops.get() + 1);
    }
}
struct Domain {
    maximum: u64,
    hooks: Rc<Hooks>,
}
impl HostResultDomain<ScalarValue> for Domain {
    type Error = PrivateError;
    fn matches_type(&self, value: &ScalarValue) -> bool {
        self.hooks.hit(Stage::Type).is_ok() && matches!(value, ScalarValue::Integer(_))
    }
    fn validate_value(&self, value: &ScalarValue) -> Result<(), PrivateError> {
        self.hooks.hit(Stage::Domain)?;
        match value {
            ScalarValue::Integer(value) if *value <= self.maximum => Ok(()),
            _ => Err(PrivateError("private domain bounds")),
        }
    }
}
type NativeDomain<'schema> = dyn HostResultDomain<ScalarValue, Error = PrivateError> + 'schema;
struct View {
    panic: Rc<Cell<bool>>,
    drops: Rc<Cell<usize>>,
    hooks: Option<Rc<Hooks>>,
}
impl Drop for View {
    fn drop(&mut self) {
        if let Some(hooks) = &self.hooks {
            hooks.observe_cleanup("value");
        }
        self.drops.set(self.drops.get() + 1);
        assert!(!self.panic.get(), "native initial prefix drop unwind");
    }
}
struct Capture<'expression> {
    values: RestoredEffectBindings<'expression, Rc<View>>,
    hooks: Rc<Hooks>,
}
impl Drop for Capture<'_> {
    fn drop(&mut self) {
        self.hooks.observe_cleanup("capture");
        self.hooks
            .capture_drops
            .set(self.hooks.capture_drops.get() + 1);
        assert!(
            !self.hooks.capture_drop_panic.replace(false),
            "native capture drop unwind"
        );
    }
}
struct Request<'expression> {
    operation: &'expression Operation,
    amount: u64,
    buffer: Box<u8>,
    hooks: Rc<Hooks>,
}
impl Drop for Request<'_> {
    fn drop(&mut self) {
        self.hooks.observe_cleanup("request");
        self.hooks
            .request_drops
            .set(self.hooks.request_drops.get() + 1);
    }
}
struct Host<'schema> {
    domains: [&'schema NativeDomain<'schema>; 2],
    hooks: Rc<Hooks>,
    serial: Cell<usize>,
    version: Cell<u32>,
    grants: Cell<u8>,
    prepared_pointer: Cell<usize>,
    actual: Cell<u64>,
    invocations: Cell<usize>,
}
impl<'schema> Host<'schema> {
    fn new(domains: &'schema [Domain; 2], hooks: Rc<Hooks>) -> Self {
        Self {
            domains: [&domains[0], &domains[1]],
            hooks,
            serial: Cell::new(0),
            version: Cell::new(7),
            grants: Cell::new(3),
            prepared_pointer: Cell::new(0),
            actual: Cell::new(41),
            invocations: Cell::new(0),
        }
    }
    fn policy(&self, op: u8) -> Result<&'schema NativeDomain<'schema>, PrivateError> {
        if self.version.get() != 7
            || !(1..=2).contains(&op)
            || self.grants.get() & (1 << (op - 1)) == 0
        {
            return Err(PrivateError("private live native policy"));
        }
        Ok(self.domains[usize::from(op - 1)])
    }
    fn identity(&self, serial: usize, op: u8) -> Identity {
        Identity {
            serial: Box::new(serial),
            op,
            hooks: self.hooks.clone(),
            tracked: false,
        }
    }
    fn reply(&self, value: ScalarValue) -> Reply {
        Reply {
            value,
            buffer: Box::new(9),
            hooks: self.hooks.clone(),
        }
    }
    fn invoke(&self, request: Request<'_>) -> Reply {
        self.policy(request.operation.0)
            .unwrap_or_else(|_| panic!("native invocation denied"));
        self.invocations.set(self.invocations.get() + 1);
        if request.operation.0 == 2 {
            self.actual.set(request.amount);
        }
        self.reply(ScalarValue::Integer(self.actual.get()))
    }
}
impl PureEvaluationEnvironment<Field, Operation> for Host<'_> {
    type Result = Rc<View>;
    type Error = PrivateError;
    fn field(&self, _: &Rc<View>, _: &Field) -> Result<ScalarValue, PrivateError> {
        Ok(ScalarValue::Integer(7))
    }
    fn member(&self, _: &Rc<View>, _: &str, _: &Operation) -> Result<Rc<View>, PrivateError> {
        Err(PrivateError("not a group"))
    }
}
impl<'expression> EffectEvaluationEnvironment<'expression, Field, Operation, Opaque, Tag>
    for Host<'_>
{
    type Request = Request<'expression>;
    type Capture = Capture<'expression>;
    fn preflight_effect(&self, node: &Node) -> Result<(), PrivateError> {
        self.hooks.hit(Stage::Preflight)?;
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err(PrivateError("unsupported native effect"));
        };
        self.policy(operation.0)?;
        if arguments.len() != 1 || arguments[0].name != "amount" {
            return Err(PrivateError("native argument shape"));
        }
        Ok(())
    }
    fn prepare_effect(
        &self,
        node: &'expression Node,
        scope: &mut ScopeFrame<'_, 'expression, PureValue<Rc<View>>>,
        fuel: &mut Fuel,
    ) -> Result<Request<'expression>, CalculationFailure<PrivateError>> {
        self.hooks.hit(Stage::Prepare)?;
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err(PrivateError("not a call").into());
        };
        self.policy(operation.0)?;
        let value = evaluate_pure_in_scope(&arguments[0].value, scope, self, fuel, LIMITS.pure)
            .map_err(|_| PrivateError("native argument evaluation"))?;
        let PureValue::Scalar(ScalarValue::Integer(amount)) = value else {
            return Err(PrivateError("native amount type").into());
        };
        if amount > 200 {
            return Err(PrivateError("native amount bounds").into());
        }
        let buffer = Box::new(3);
        self.prepared_pointer.set((&*buffer as *const u8) as usize);
        Ok(Request {
            operation,
            amount,
            buffer,
            hooks: self.hooks.clone(),
        })
    }
    fn capture(
        &self,
        _: &'expression str,
        _: &'expression Node,
        scope: &ScopeFrame<'_, 'expression, PureValue<Rc<View>>>,
        fuel: &mut Fuel,
    ) -> Result<Capture<'expression>, PrivateError> {
        self.hooks.hit(Stage::Capture)?;
        let mut values = Vec::new();
        for (name, value) in scope.bindings() {
            let extra = match value {
                PureValue::Scalar(value) => scalar_copy_cost(value),
                _ => 0,
            };
            fuel.charge(1 + extra)
                .map_err(|_| PrivateError("capture fuel"))?;
            values.push((*name, value.clone()));
        }
        Ok(Capture {
            values,
            hooks: self.hooks.clone(),
        })
    }
}
impl<'expression, 'schema>
    EffectReentryEnvironment<
        'expression,
        Field,
        Operation,
        Opaque,
        Tag,
        Identity,
        NativeDomain<'schema>,
        Reply,
    > for Host<'schema>
{
    fn restore_capture(
        &self,
        _: &Identity,
        _: &NativeDomain<'schema>,
        mut capture: Capture<'expression>,
        _: &mut Fuel,
        _: EffectEvaluationLimits,
    ) -> Result<RestoredEffectBindings<'expression, Rc<View>>, CalculationFailure<PrivateError>>
    {
        self.hooks.hit(Stage::Restore)?;
        if self.hooks.fault.get() == (Stage::Restore, Mode::Unbounded) {
            return Ok(vec![(
                "invalid name",
                PureValue::Result(Rc::new(View {
                    panic: Rc::new(Cell::new(false)),
                    drops: Rc::new(Cell::new(0)),
                    hooks: Some(self.hooks.clone()),
                })),
            )]);
        }
        Ok(std::mem::take(&mut capture.values))
    }
    fn bind_reply(
        &self,
        identity: &Identity,
        declaration: &NativeDomain<'schema>,
        mut reply: Reply,
        _: &mut Fuel,
    ) -> Result<PureValue<Rc<View>>, CalculationFailure<PrivateError>> {
        self.hooks.hit(Stage::Project)?;
        if !std::ptr::eq(self.policy(identity.op)?, declaration) {
            return Err(PrivateError("changed native declaration").into());
        }
        if self.hooks.fault.get() == (Stage::Project, Mode::Unbounded) {
            return Ok(PureValue::Scalar(ScalarValue::String(
                "x".repeat(MAX_SCALAR_STRING_BYTES + 1),
            )));
        }
        if self.hooks.fault.get() == (Stage::Project, Mode::Owned) {
            return Ok(PureValue::Result(Rc::new(View {
                panic: Rc::new(Cell::new(false)),
                drops: Rc::new(Cell::new(0)),
                hooks: Some(self.hooks.clone()),
            })));
        }
        Ok(PureValue::Scalar(std::mem::replace(
            &mut reply.value,
            ScalarValue::None,
        )))
    }
}
impl<'expression, 'schema>
    EffectSessionEnvironment<'expression, 'schema, Field, Operation, Opaque, Tag>
    for Host<'schema>
{
    type Identity = Identity;
    type Declaration = NativeDomain<'schema>;
    type Reply = Reply;
    type Dispatch = Request<'expression>;
    fn correlate_request(
        &self,
        request: Request<'expression>,
        _: &mut Fuel,
    ) -> EffectCorrelation<'schema, Identity, Self::Declaration, Self::Dispatch, PrivateError> {
        if self.hooks.correlation_cleanup_panic.replace(false) {
            self.hooks.capture_drop_panic.set(true);
        }
        self.hooks.hit(Stage::Correlate)?;
        let declaration = self.policy(request.operation.0)?;
        let serial = self.serial.get() + 1;
        self.serial.set(serial);
        Ok(CorrelatedEffectRequest {
            identity: Identity {
                serial: Box::new(serial),
                op: request.operation.0,
                hooks: self.hooks.clone(),
                tracked: true,
            },
            declaration,
            dispatch: request,
        })
    }
}
impl ReplyAuthority<Identity> for Host<'_> {
    type Error = PrivateError;
    fn authorize(&self, identity: &Identity) -> Result<(), PrivateError> {
        self.hooks.hit(Stage::Authority)?;
        self.policy(identity.op).map(|_| ())
    }
}
fn integer(value: u64) -> Node {
    Node::Literal {
        value: ScalarValue::Integer(value),
    }
}
fn local(name: &str) -> Node {
    Node::Local { name: name.into() }
}
fn add(left: Node, right: Node) -> Node {
    Node::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(left),
        right: Box::new(right),
    }
}
fn call(op: u8, amount: Node) -> Node {
    Node::Call {
        operation: Operation(op),
        arguments: vec![ComputedArgument {
            name: "amount".into(),
            value: amount,
        }],
    }
}
fn bind(name: &str, value: Node, body: Node) -> Node {
    Node::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn request(outcome: EffectSessionPoll<Rc<View>, Request<'_>>) -> Request<'_> {
    let EffectSessionPoll::Request(request) = outcome else {
        panic!()
    };
    request
}
macro_rules! host {
    ($hooks:ident, $domains:ident, $host:ident) => {
        let $hooks = Hooks::new();
        let $domains = [
            Domain {
                maximum: 100,
                hooks: $hooks.clone(),
            },
            Domain {
                maximum: 200,
                hooks: $hooks.clone(),
            },
        ];
        let $host = Host::new(&$domains, $hooks.clone());
    };
}

#[test]
fn accepted_binding_waits_for_explicit_poll_and_returns_requests_and_pure_tail_once() {
    host!(hooks, domains, host);
    let expression = bind(
        "first",
        call(1, integer(5)),
        bind(
            "second",
            call(2, add(local("first"), integer(1))),
            add(local("second"), integer(1)),
        ),
    );
    let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(100), LIMITS);
    assert!(hooks.events.borrow().is_empty());
    let first = request(session.poll(&host).unwrap());
    assert_eq!(
        (&*first.buffer as *const u8) as usize,
        host.prepared_pointer.get()
    );
    assert_eq!(first.amount, 5);
    assert_eq!(session.fuel_remaining(), 97);
    let before = hooks.events.borrow().len();
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Awaiting
    ));
    assert_eq!(hooks.events.borrow().len(), before);
    let reply = host.invoke(first);
    session
        .try_accept(&host.identity(1, 1), reply, &host)
        .unwrap();
    assert_eq!(session.status(), EffectSessionStatus::Ready);
    assert_eq!(session.fuel_remaining(), 97);
    assert!(!hooks.events.borrow().contains(&Stage::Restore));
    let second = request(session.poll(&host).unwrap());
    assert_eq!(second.amount, 42);
    assert_eq!(session.fuel_remaining(), 91);
    let reply = host.invoke(second);
    session
        .try_accept(&host.identity(2, 2), reply, &host)
        .unwrap();
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(43)))
    ));
    assert_eq!(session.fuel_remaining(), 87);
    assert_eq!(host.actual.get(), 42);
    assert_eq!(host.invocations.get(), 2);
    let before = hooks.events.borrow().len();
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
    ));
    assert_eq!(hooks.events.borrow().len(), before);
    assert!(!session.cancel());
    assert_eq!(hooks.key_drops.get(), 2);
    assert_eq!(hooks.capture_drops.get(), 2);
    assert_eq!(hooks.reply_drops.get(), 2);
}

#[test]
fn final_native_reply_is_projected_only_on_later_poll_and_handed_off_once() {
    host!(hooks, domains, host);
    let expression = call(1, integer(1));
    let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(100), LIMITS);
    let native = request(session.poll(&host).unwrap());
    let reply = host.invoke(native);
    session
        .try_accept(&host.identity(1, 1), reply, &host)
        .unwrap();
    assert!(!hooks.events.borrow().contains(&Stage::Project));
    assert_eq!(session.fuel_remaining(), 98);
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(41)))
    ));
    assert_eq!(session.fuel_remaining(), 98);
    assert_eq!(
        session.status(),
        EffectSessionStatus::Terminal(EffectSessionEnd::Completed)
    );
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
    ));
    assert_eq!(
        hooks
            .events
            .borrow()
            .iter()
            .filter(|s| **s == Stage::Project)
            .count(),
        1
    );
}

#[test]
fn pure_result_and_early_or_duplicate_replies_have_no_extra_native_work() {
    host!(hooks, domains, host);
    let expression = integer(42);
    let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(5), LIMITS);
    let early = host.reply(ScalarValue::Integer(1));
    let pointer = &*early.buffer as *const u8;
    let rejected = session
        .try_accept(&host.identity(1, 1), early, &host)
        .unwrap_err();
    assert_eq!(&*rejected.reply.buffer as *const u8, pointer);
    assert!(matches!(
        rejected.error,
        EffectSessionReplyError::NotAwaiting(EffectSessionStatus::Ready)
    ));
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(42)))
    ));
    assert_eq!(session.fuel_remaining(), 4);
    assert!(
        session
            .try_accept(
                &host.identity(1, 1),
                host.reply(ScalarValue::Integer(1)),
                &host
            )
            .is_err()
    );
    assert!(hooks.events.borrow().is_empty());
}

#[test]
fn wrong_identity_policy_type_and_domain_return_exact_input_and_keep_waiting() {
    host!(hooks, domains, host);
    let expression = bind("result", call(1, integer(1)), local("result"));
    let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(100), LIMITS);
    drop(request(session.poll(&host).unwrap()));
    for (serial, version, value) in [
        (9, 7, ScalarValue::Integer(1)),
        (1, 8, ScalarValue::Integer(1)),
        (1, 7, ScalarValue::Boolean(true)),
        (1, 7, ScalarValue::Integer(101)),
    ] {
        host.version.set(version);
        let input = host.reply(value);
        let pointer = &*input.buffer as *const u8;
        let before = session.fuel_remaining();
        let rejected = session
            .try_accept(&host.identity(serial, 1), input, &host)
            .unwrap_err();
        assert_eq!(&*rejected.reply.buffer as *const u8, pointer);
        assert_eq!(session.status(), EffectSessionStatus::Awaiting);
        assert_eq!(session.fuel_remaining(), before);
        assert!(!hooks.events.borrow().contains(&Stage::Restore));
    }
    host.version.set(7);
    session
        .try_accept(
            &host.identity(1, 1),
            host.reply(ScalarValue::Integer(42)),
            &host,
        )
        .unwrap();
    let before = hooks.events.borrow().len();
    assert!(
        session
            .try_accept(
                &host.identity(1, 1),
                host.reply(ScalarValue::Integer(42)),
                &host
            )
            .is_err()
    );
    assert_eq!(hooks.events.borrow().len(), before);
}

#[test]
fn cancel_initial_waiting_or_accepted_binding_and_final_reply_prevents_all_later_work() {
    for binding in [false, true] {
        for stage in 0..3 {
            host!(hooks, domains, host);
            let expression = if binding {
                bind("result", call(1, integer(1)), call(2, local("result")))
            } else {
                call(1, integer(1))
            };
            let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(100), LIMITS);
            if stage > 0 {
                drop(request(session.poll(&host).unwrap()));
            }
            if stage > 1 {
                session
                    .try_accept(
                        &host.identity(1, 1),
                        host.reply(ScalarValue::Integer(42)),
                        &host,
                    )
                    .unwrap();
            }
            let before = session.fuel_remaining();
            hooks.events.borrow_mut().clear();
            assert!(session.cancel());
            assert!(!session.cancel());
            assert!(matches!(
                session.poll(&host).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
            ));
            assert!(
                session
                    .try_accept(
                        &host.identity(1, 1),
                        host.reply(ScalarValue::Integer(42)),
                        &host
                    )
                    .is_err()
            );
            assert!(hooks.events.borrow().is_empty());
            assert_eq!(session.fuel_remaining(), before);
            assert_eq!(host.invocations.get(), 0);
        }
    }
}

#[test]
fn physical_prefix_limits_and_cold_policy_precede_fuel_and_preparation() {
    for kind in 0..4 {
        host!(hooks, domains, host);
        let expression = bind("result", call(1, integer(1)), call(2, local("result")));
        let mut limits = LIMITS;
        let bindings = match kind {
            0 => {
                limits.pure.max_nodes = 0;
                Vec::new()
            }
            1 => vec![("bad name", PureValue::Scalar(ScalarValue::None))],
            2 => {
                limits.max_arguments = 0;
                Vec::new()
            }
            _ => {
                host.grants.set(1);
                Vec::new()
            }
        };
        let mut session = EffectSession::new(&expression, bindings, Fuel::new(100), limits);
        assert!(session.poll(&host).is_err());
        assert_eq!(
            session.status(),
            EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
        );
        assert_eq!(session.fuel_remaining(), 100);
        assert!(!hooks.events.borrow().contains(&Stage::Prepare));
        assert!(!hooks.events.borrow().contains(&Stage::Correlate));
        assert!(!session.cancel());
    }
}

#[test]
fn native_preparation_capture_and_correlation_failures_or_unwind_are_terminal() {
    for point in [
        Stage::Preflight,
        Stage::Prepare,
        Stage::Capture,
        Stage::Correlate,
    ] {
        for mode in [Mode::Reject, Mode::Unwind] {
            host!(hooks, domains, host);
            let expression = bind("result", call(1, integer(1)), local("result"));
            hooks.fault.set((point, mode));
            let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(100), LIMITS);
            let outcome = catch_unwind(AssertUnwindSafe(|| session.poll(&host)));
            let reason = if mode == Mode::Unwind {
                assert!(outcome.is_err());
                EffectSessionEnd::HostUncertain
            } else {
                let error = outcome.unwrap().unwrap_err();
                assert!(matches!(&error,
                        CalculationFailure::External(EffectEvaluationFault::Native(PrivateError(value)))
                        if *value == "private native rejection"));
                assert!(!format!("{error:?}").contains("private"));
                EffectSessionEnd::Failed
            };
            assert_eq!(session.status(), EffectSessionStatus::Terminal(reason));
            let count = hooks.events.borrow().len();
            assert!(matches!(
                session.poll(&host).unwrap(),
                EffectSessionPoll::Terminal(_)
            ));
            assert_eq!(hooks.events.borrow().len(), count);
            assert!(!session.cancel());
            assert_eq!(host.invocations.get(), 0);
        }
    }
}

#[test]
fn every_native_acceptance_callback_unwind_seals_the_session_without_rearming() {
    for binding in [false, true] {
        for point in [
            Stage::Equality,
            Stage::Authority,
            Stage::Borrow,
            Stage::Type,
            Stage::Domain,
        ] {
            host!(hooks, domains, host);
            let expression = if binding {
                bind("result", call(1, integer(1)), local("result"))
            } else {
                call(1, integer(1))
            };
            let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(100), LIMITS);
            drop(request(session.poll(&host).unwrap()));
            hooks.fault.set((point, Mode::Unwind));
            assert!(
                catch_unwind(AssertUnwindSafe(|| session.try_accept(
                    &host.identity(1, 1),
                    host.reply(ScalarValue::Integer(42)),
                    &host
                )))
                .is_err()
            );
            assert_eq!(
                session.status(),
                EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
            );
            assert!(!session.cancel());
            assert_eq!(hooks.key_drops.get(), 1);
            assert_eq!(hooks.reply_drops.get(), 1);
            assert_eq!(hooks.capture_drops.get(), usize::from(binding));
            assert!(!hooks.events.borrow().contains(&Stage::Restore));
        }
    }
}

#[test]
fn restored_binding_or_final_projection_failure_unwind_and_bounds_never_replay() {
    for binding in [false, true] {
        for (point, mode) in [
            (Stage::Project, Mode::Reject),
            (Stage::Project, Mode::Unwind),
            (Stage::Project, Mode::Unbounded),
            (Stage::Restore, Mode::Reject),
            (Stage::Restore, Mode::Unwind),
        ] {
            if !binding && point == Stage::Restore {
                continue;
            }
            host!(hooks, domains, host);
            let expression = if binding {
                bind("result", call(1, integer(1)), local("result"))
            } else {
                call(1, integer(1))
            };
            let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(100), LIMITS);
            drop(request(session.poll(&host).unwrap()));
            session
                .try_accept(
                    &host.identity(1, 1),
                    host.reply(ScalarValue::Integer(42)),
                    &host,
                )
                .unwrap();
            hooks.fault.set((point, mode));
            let outcome = catch_unwind(AssertUnwindSafe(|| session.poll(&host)));
            let reason = if mode == Mode::Unwind {
                assert!(outcome.is_err());
                EffectSessionEnd::HostUncertain
            } else {
                assert!(outcome.unwrap().is_err());
                EffectSessionEnd::Failed
            };
            assert_eq!(session.status(), EffectSessionStatus::Terminal(reason));
            assert_eq!(hooks.reply_drops.get(), 1);
            assert_eq!(hooks.key_drops.get(), 1);
            assert!(!session.cancel());
        }
    }
}

#[test]
fn accepted_successor_and_final_projection_recheck_revoked_live_policy() {
    for binding in [false, true] {
        host!(hooks, domains, host);
        let expression = if binding {
            bind("result", call(1, integer(1)), call(2, local("result")))
        } else {
            call(1, integer(1))
        };
        let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(100), LIMITS);
        drop(request(session.poll(&host).unwrap()));
        session
            .try_accept(
                &host.identity(1, 1),
                host.reply(ScalarValue::Integer(42)),
                &host,
            )
            .unwrap();
        host.version.set(8);
        assert!(session.poll(&host).is_err());
        assert_eq!(
            session.status(),
            EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
        );
        assert_eq!(host.invocations.get(), 0);
    }
}

#[test]
fn owned_initial_prefix_cleanup_unwind_cannot_install_a_waiting_request() {
    host!(hooks, domains, host);
    let expression = call(1, integer(1));
    let drops = Rc::new(Cell::new(0));
    let value = Rc::new(View {
        panic: Rc::new(Cell::new(true)),
        drops: drops.clone(),
        hooks: None,
    });
    let weak = Rc::downgrade(&value);
    let mut session = EffectSession::new(
        &expression,
        vec![("unused", PureValue::Result(value))],
        Fuel::new(100),
        LIMITS,
    );
    assert!(catch_unwind(AssertUnwindSafe(|| session.poll(&host))).is_err());
    assert_eq!(
        session.status(),
        EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
    );
    assert_eq!(drops.get(), 1);
    assert!(weak.upgrade().is_none());
    assert_eq!(hooks.request_drops.get(), 1);
    assert!(!hooks.events.borrow().contains(&Stage::Correlate));
    assert!(!session.cancel());
}

#[test]
fn owned_reentry_identity_cleanup_releases_prepared_successor_before_handoff() {
    for stacked in [false, true] {
        host!(hooks, domains, host);
        let expression = bind("result", call(1, integer(1)), call(2, local("result")));
        macro_rules! check {
            ($session:expr) => {{
                let mut session = $session;
                drop(request(session.poll(&host).unwrap()));
                session
                    .try_accept(
                        &host.identity(1, 1),
                        host.reply(ScalarValue::Integer(42)),
                        &host,
                    )
                    .unwrap();
                hooks.key_drop_panic.set(true);
                let before = session.fuel_remaining();
                assert!(catch_unwind(AssertUnwindSafe(|| session.poll(&host))).is_err());
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
                );
                assert!(session.fuel_remaining() < before);
                assert_eq!(hooks.request_drops.get(), 2, "stacked {stacked}");
                assert_eq!(host.serial.get(), 1);
                assert!(!session.cancel());
            }};
        }
        if stacked {
            check!(StackedEffectSession::new(
                &expression,
                Vec::new(),
                Fuel::new(100),
                EffectStackLimits {
                    effects: LIMITS,
                    max_frames: 4
                }
            ));
        } else {
            check!(EffectSession::new(
                &expression,
                Vec::new(),
                Fuel::new(100),
                LIMITS
            ));
        }
    }
}

#[test]
fn owned_reentry_prefix_cleanup_releases_prepared_successor_and_preserves_meter() {
    for stacked in [false, true] {
        host!(hooks, domains, host);
        let expression = bind("result", call(1, integer(1)), call(2, local("result")));
        let panic = Rc::new(Cell::new(false));
        let drops = Rc::new(Cell::new(0));
        let value = Rc::new(View {
            panic: panic.clone(),
            drops: drops.clone(),
            hooks: None,
        });
        let weak = Rc::downgrade(&value);
        macro_rules! check {
            ($session:expr) => {{
                let mut session = $session;
                drop(request(session.poll(&host).unwrap()));
                session
                    .try_accept(
                        &host.identity(1, 1),
                        host.reply(ScalarValue::Integer(42)),
                        &host,
                    )
                    .unwrap();
                panic.set(true);
                let before = session.fuel_remaining();
                assert!(catch_unwind(AssertUnwindSafe(|| session.poll(&host))).is_err());
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
                );
                assert!(session.fuel_remaining() < before);
                assert_eq!(hooks.request_drops.get(), 2, "stacked {stacked}");
                assert_eq!(drops.get(), 1);
                assert!(weak.upgrade().is_none());
                assert_eq!(host.serial.get(), 1);
            }};
        }
        if stacked {
            check!(StackedEffectSession::new(
                &expression,
                vec![("unused", PureValue::Result(value))],
                Fuel::new(100),
                EffectStackLimits {
                    effects: LIMITS,
                    max_frames: 4
                }
            ));
        } else {
            check!(EffectSession::new(
                &expression,
                vec![("unused", PureValue::Result(value))],
                Fuel::new(100),
                LIMITS
            ));
        }
    }
}

#[test]
fn owned_reentry_prefix_cleanup_releases_opaque_projection_error_without_refund() {
    for stacked in [false, true] {
        host!(hooks, domains, host);
        let expression = bind("result", call(1, integer(1)), local("result"));
        let panic = Rc::new(Cell::new(false));
        let drops = Rc::new(Cell::new(0));
        let value = Rc::new(View {
            panic: panic.clone(),
            drops: drops.clone(),
            hooks: None,
        });
        macro_rules! check {
            ($session:expr) => {{
                let mut session = $session;
                drop(request(session.poll(&host).unwrap()));
                session
                    .try_accept(
                        &host.identity(1, 1),
                        host.reply(ScalarValue::Integer(42)),
                        &host,
                    )
                    .unwrap();
                panic.set(true);
                hooks.fault.set((Stage::Project, Mode::Reject));
                let before = session.fuel_remaining();
                let before_drops = PRIVATE_ERROR_DROPS.with(Cell::get);
                assert!(catch_unwind(AssertUnwindSafe(|| session.poll(&host))).is_err());
                assert_eq!(
                    PRIVATE_ERROR_DROPS.with(Cell::get),
                    before_drops + 1,
                    "stacked {stacked}"
                );
                assert_eq!(session.fuel_remaining(), before - 1);
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
                );
                assert_eq!(drops.get(), 1);
                assert_eq!(hooks.request_drops.get(), 1);
            }};
        }
        if stacked {
            check!(StackedEffectSession::new(
                &expression,
                vec![("unused", PureValue::Result(value))],
                Fuel::new(100),
                EffectStackLimits {
                    effects: LIMITS,
                    max_frames: 4
                }
            ));
        } else {
            check!(EffectSession::new(
                &expression,
                vec![("unused", PureValue::Result(value))],
                Fuel::new(100),
                LIMITS
            ));
        }
    }
}

#[test]
fn owned_reentry_cold_refusal_cleanup_releases_opaque_error_and_uncalled_capture() {
    for stacked in [false, true] {
        host!(hooks, domains, host);
        let expression = bind("result", call(1, integer(1)), call(2, local("result")));
        macro_rules! check {
            ($session:expr) => {{
                let mut session = $session;
                drop(request(session.poll(&host).unwrap()));
                session
                    .try_accept(
                        &host.identity(1, 1),
                        host.reply(ScalarValue::Integer(42)),
                        &host,
                    )
                    .unwrap();
                hooks.fault.set((Stage::Preflight, Mode::Reject));
                hooks.capture_drop_panic.set(true);
                let before = session.fuel_remaining();
                let before_drops = PRIVATE_ERROR_DROPS.with(Cell::get);
                assert!(catch_unwind(AssertUnwindSafe(|| session.poll(&host))).is_err());
                assert_eq!(
                    PRIVATE_ERROR_DROPS.with(Cell::get),
                    before_drops + 1,
                    "stacked {stacked}"
                );
                assert_eq!(session.fuel_remaining(), before);
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
                );
                assert_eq!(hooks.capture_drops.get(), 1);
                assert_eq!(hooks.reply_drops.get(), 1);
                assert!(!hooks.events.borrow().contains(&Stage::Restore));
            }};
        }
        if stacked {
            check!(StackedEffectSession::new(
                &expression,
                Vec::new(),
                Fuel::new(100),
                EffectStackLimits {
                    effects: LIMITS,
                    max_frames: 4
                }
            ));
        } else {
            check!(EffectSession::new(
                &expression,
                Vec::new(),
                Fuel::new(100),
                LIMITS
            ));
        }
    }
}

#[test]
fn owned_reentry_nested_unused_frames_cleanup_releases_opaque_cold_error_once() {
    host!(hooks, domains, host);
    let expression = bind(
        "outer",
        bind("inner", call(1, integer(1)), local("inner")),
        call(2, local("outer")),
    );
    let mut session = StackedEffectSession::new(
        &expression,
        Vec::new(),
        Fuel::new(100),
        EffectStackLimits {
            effects: LIMITS,
            max_frames: 4,
        },
    );
    drop(request(session.poll(&host).unwrap()));
    session
        .try_accept(
            &host.identity(1, 1),
            host.reply(ScalarValue::Integer(42)),
            &host,
        )
        .unwrap();
    hooks.fault.set((Stage::Preflight, Mode::Reject));
    hooks.capture_drop_panic.set(true);
    let before = session.fuel_remaining();
    let before_drops = PRIVATE_ERROR_DROPS.with(Cell::get);
    assert!(catch_unwind(AssertUnwindSafe(|| session.poll(&host))).is_err());
    assert_eq!(PRIVATE_ERROR_DROPS.with(Cell::get), before_drops + 1);
    assert_eq!(hooks.capture_drops.get(), 2);
    assert_eq!(hooks.reply_drops.get(), 1);
    assert_eq!(session.fuel_remaining(), before);
    assert_eq!(
        session.status(),
        EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
    );
    assert!(!session.cancel());
}

#[test]
fn cancellation_cleanup_unwind_retains_cancelled_reason_and_drops_once() {
    for after_acceptance in [false, true] {
        for key in [false, true] {
            host!(hooks, domains, host);
            let expression = bind("result", call(1, integer(1)), local("result"));
            let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(100), LIMITS);
            drop(request(session.poll(&host).unwrap()));
            if after_acceptance {
                session
                    .try_accept(
                        &host.identity(1, 1),
                        host.reply(ScalarValue::Integer(42)),
                        &host,
                    )
                    .unwrap();
            }
            if key {
                hooks.key_drop_panic.set(true);
            } else {
                hooks.capture_drop_panic.set(true);
            }
            assert!(catch_unwind(AssertUnwindSafe(|| session.cancel())).is_err());
            assert_eq!(
                session.status(),
                EffectSessionStatus::Terminal(EffectSessionEnd::Cancelled)
            );
            assert!(!session.cancel());
            assert_eq!(hooks.key_drops.get(), 1);
            assert_eq!(hooks.capture_drops.get(), 1);
            assert_eq!(hooks.reply_drops.get(), usize::from(after_acceptance));
        }
    }
}

#[test]
fn exhaustion_is_terminal_and_never_refills_while_waiting_or_after_acceptance() {
    host!(hooks, domains, host);
    let expression = bind("result", call(1, integer(1)), local("result"));
    let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(3), LIMITS);
    drop(request(session.poll(&host).unwrap()));
    assert_eq!(session.fuel_remaining(), 0);
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Awaiting
    ));
    session
        .try_accept(
            &host.identity(1, 1),
            host.reply(ScalarValue::Integer(42)),
            &host,
        )
        .unwrap();
    assert!(session.poll(&host).is_err());
    assert_eq!(session.fuel_remaining(), 0);
    assert_eq!(
        session.status(),
        EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
    );
    assert!(!session.cancel());
}

#[test]
fn accepted_identity_or_capture_cleanup_unwind_cannot_publish_completion_or_successor() {
    for binding in [false, true] {
        for key in [false, true] {
            if !binding && !key {
                continue;
            }
            host!(hooks, domains, host);
            let expression = if binding {
                bind("result", call(1, integer(1)), call(2, local("result")))
            } else {
                call(1, integer(1))
            };
            let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(100), LIMITS);
            drop(request(session.poll(&host).unwrap()));
            session
                .try_accept(
                    &host.identity(1, 1),
                    host.reply(ScalarValue::Integer(42)),
                    &host,
                )
                .unwrap();
            if key {
                hooks.key_drop_panic.set(true);
            } else {
                hooks.capture_drop_panic.set(true);
            }
            assert!(catch_unwind(AssertUnwindSafe(|| session.poll(&host))).is_err());
            assert_eq!(
                session.status(),
                EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
            );
            assert_eq!(host.serial.get(), 1);
            assert_eq!(hooks.key_drops.get(), 1);
            assert_eq!(hooks.capture_drops.get(), usize::from(binding));
            assert_eq!(hooks.reply_drops.get(), 1);
            assert!(!session.cancel());
        }
    }
}

#[test]
fn correlation_error_followed_by_capture_cleanup_unwind_stays_host_uncertain() {
    host!(hooks, domains, host);
    let expression = bind("result", call(1, integer(1)), local("result"));
    let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(100), LIMITS);
    hooks.fault.set((Stage::Correlate, Mode::Reject));
    hooks.capture_drop_panic.set(true);
    let before_drops = PRIVATE_ERROR_DROPS.with(Cell::get);
    assert!(catch_unwind(AssertUnwindSafe(|| session.poll(&host))).is_err());
    assert_eq!(PRIVATE_ERROR_DROPS.with(Cell::get), before_drops + 1);
    assert_eq!(
        session.status(),
        EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
    );
    assert_eq!(hooks.request_drops.get(), 1);
    assert_eq!(hooks.capture_drops.get(), 1);
    assert_eq!(host.serial.get(), 0);
    assert!(!session.cancel());
}

#[test]
fn session_poll_and_rejection_debug_never_format_native_slots_or_private_payloads() {
    host!(hooks, domains, host);
    let expression = bind("secret", call(1, integer(1)), local("secret"));
    let mut session = EffectSession::new(&expression, Vec::new(), Fuel::new(100), LIMITS);
    let output = session.poll(&host).unwrap();
    assert_eq!(format!("{output:?}"), "Request");
    drop(output);
    let debug = format!("{session:?}");
    assert!(debug.contains("Awaiting"));
    assert!(!debug.contains("secret"));
    let rejected = session
        .try_accept(
            &host.identity(9, 1),
            host.reply(ScalarValue::String("private payload".into())),
            &host,
        )
        .unwrap_err();
    assert!(!format!("{rejected:?}").contains("private"));
    assert!(
        hooks
            .events
            .borrow()
            .iter()
            .all(|s| !matches!(s, Stage::Borrow | Stage::Type | Stage::Domain))
    );
}

const STACK_LIMITS: EffectStackLimits = EffectStackLimits {
    effects: LIMITS,
    max_frames: 4,
};

type SavedFrames<'a> = Vec<SavedEffectReturnFrame<'a, Node, Capture<'a>>>;
type RestoredSession<'a, 's> = StackedEffectSession<
    'a,
    's,
    Field,
    Operation,
    Opaque,
    Tag,
    Rc<View>,
    Capture<'a>,
    Identity,
    NativeDomain<'s>,
    Reply,
>;
type LocalRestoredSession<'a, 's> = LocalStackedEffectSession<
    'a,
    's,
    Field,
    Operation,
    Opaque,
    Tag,
    Rc<View>,
    Capture<'a>,
    Identity,
    NativeDomain<'s>,
    Reply,
>;

struct SavedSession<'a, 's> {
    frames: SavedFrames<'a>,
    identity: Identity,
    declaration: &'s NativeDomain<'s>,
    fuel: Fuel,
    dispatch: Request<'a>,
}

// This fixture moves the real suspension; it is not a durable codec or journal.
fn saved_session<'a, 's>(expression: &'a Node, host: &Host<'s>) -> SavedSession<'a, 's> {
    let mut fuel = Fuel::new(100);
    let mut values = Vec::new();
    let StackedEffectOutcome::Suspended {
        request,
        continuation,
    } = evaluate_stacked_effects_in_scope(
        expression,
        &mut ScopeFrame::new(&mut values),
        host,
        &mut fuel,
        STACK_LIMITS,
    )
    .unwrap()
    else {
        panic!("fixture must suspend")
    };
    let request = host
        .correlate_request(request, &mut fuel)
        .unwrap_or_else(|_| panic!("fixture correlation failed"));
    SavedSession {
        frames: continuation
            .into_frames()
            .into_iter()
            .map(|frame| {
                let (name, body, capture) = frame.into_parts();
                SavedEffectReturnFrame {
                    name,
                    body,
                    capture,
                }
            })
            .collect(),
        identity: request.identity,
        declaration: request.declaration,
        fuel,
        dispatch: request.dispatch,
    }
}

#[test]
fn restored_session_preserves_original_frames_budget_and_reply_gated_successor() {
    host!(hooks, domains, host);
    let expression = nested_program();
    let saved = saved_session(&expression, &host);
    let key = (&*saved.identity.serial as *const usize) as usize;
    let bodies: Vec<_> = saved
        .frames
        .iter()
        .map(|frame| frame.body as *const Node)
        .collect();
    let events = hooks.events.borrow().len();
    let admitted = Cell::new(0);
    let mut session = RestoredSession::restore_pending(
        saved.identity,
        saved.declaration,
        saved.frames,
        saved.fuel,
        STACK_LIMITS,
        |identity, domain, frames, fuel| {
            admitted.set(admitted.get() + 1);
            assert_eq!((&*identity.serial as *const usize) as usize, key);
            assert!(std::ptr::eq(domain, saved.declaration));
            assert_eq!(
                frames
                    .iter()
                    .map(|frame| frame.body as *const Node)
                    .collect::<Vec<_>>(),
                bodies
            );
            assert_eq!(fuel.remaining(), 89);
            host.policy(identity.op).map(|_| ())
        },
    )
    .unwrap();
    assert_eq!(admitted.get(), 1);
    assert_eq!(session.status(), EffectSessionStatus::Awaiting);
    for _ in 0..3 {
        assert!(matches!(
            session.poll(&host).unwrap(),
            EffectSessionPoll::Awaiting
        ));
    }
    assert_eq!(hooks.events.borrow().len(), events);
    assert_eq!(session.fuel_remaining(), 89);
    let input = host.invoke(saved.dispatch);
    let pointer = (&*input.buffer as *const u8) as usize;
    let rejected = session
        .try_accept(&host.identity(99, 1), input, &host)
        .unwrap_err();
    assert_eq!((&*rejected.reply.buffer as *const u8) as usize, pointer);
    assert_eq!(session.status(), EffectSessionStatus::Awaiting);
    session
        .try_accept(&host.identity(1, 1), rejected.reply, &host)
        .unwrap();
    assert_eq!(session.status(), EffectSessionStatus::Ready);
    assert_eq!(session.fuel_remaining(), 89);
    assert!(!hooks.events.borrow().contains(&Stage::Restore));
    let successor = request(session.poll(&host).unwrap());
    assert_eq!(successor.amount, 42);
    assert_eq!(session.fuel_remaining(), 79);
    session
        .try_accept(&host.identity(2, 2), host.invoke(successor), &host)
        .unwrap();
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(53)))
    ));
    assert_eq!(session.fuel_remaining(), 69);
    assert_eq!(host.invocations.get(), 2);
    assert_eq!(hooks.capture_drops.get(), 3);
    assert_eq!(hooks.key_drops.get(), 2);
    assert_eq!(hooks.reply_drops.get(), 2);
    let events = hooks.events.borrow().len();
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
    ));
    assert!(!session.cancel());
    assert_eq!(hooks.events.borrow().len(), events);
}

#[test]
fn restored_session_structure_precedes_admission_and_revokes_before_unused_hook_cleanup() {
    struct UnusedHook(Rc<Hooks>);
    impl Drop for UnusedHook {
        fn drop(&mut self) {
            self.0.observe_cleanup("unused admission");
        }
    }
    for phase in 0..7 {
        host!(hooks, domains, host);
        let expression = nested_identity_program();
        let cold = Node::Choose {
            when: Box::new(Node::Literal {
                value: ScalarValue::Boolean(false),
            }),
            then: Box::new(local("invalid name")),
            otherwise: Box::new(integer(1)),
        };
        let mut saved = saved_session(&expression, &host);
        drop(saved.dispatch);
        let owner = LocalExecutionScope::new();
        let lease = owner.lease();
        hooks.lease.replace(Some(lease.clone()));
        let mut limits = STACK_LIMITS;
        match phase {
            0 => saved.frames.clear(),
            1 => limits.max_frames = 1,
            2 => saved.frames[0].name = "invalid name",
            3 => limits.effects.pure.max_nodes = 1,
            4 => limits.effects.pure.max_bindings = 0,
            5 => limits.max_frames = MAX_EFFECT_RETURN_FRAMES + 1,
            _ => saved.frames[1].body = &cold,
        }
        hooks.cleanup_liveness.borrow_mut().clear();
        hooks.events.borrow_mut().clear();
        let guard = UnusedHook(hooks.clone());
        let result = LocalRestoredSession::restore_pending(
            owner,
            saved.identity,
            saved.declaration,
            saved.frames,
            saved.fuel,
            limits,
            move |_, _, _, _| -> Result<(), PrivateError> {
                drop(guard);
                panic!("structural rejection must precede admission")
            },
        );
        assert!(matches!(
            result,
            Err(StackedSessionRestoreError::Structure(_))
        ));
        assert!(!lease.is_open());
        assert!(hooks.events.borrow().is_empty());
        assert!(
            hooks
                .cleanup_liveness
                .borrow()
                .contains(&("unused admission", true))
        );
        assert!(
            hooks
                .cleanup_liveness
                .borrow()
                .iter()
                .all(|(_, closed)| *closed)
        );
        assert_eq!(hooks.capture_drops.get(), 2);
        assert_eq!(hooks.key_drops.get(), 1);
    }
}

#[test]
fn restored_session_closed_scope_native_refusal_and_unwind_never_publish_owner() {
    for phase in 0..3 {
        host!(hooks, domains, host);
        let expression = nested_identity_program();
        let saved = saved_session(&expression, &host);
        drop(saved.dispatch);
        let owner = LocalExecutionScope::new();
        let lease = owner.lease();
        hooks.lease.replace(Some(lease.clone()));
        if phase == 0 {
            owner.close();
        }
        hooks.events.borrow_mut().clear();
        let calls = Cell::new(0);
        let result = catch_unwind(AssertUnwindSafe(|| {
            LocalRestoredSession::restore_pending(
                owner,
                saved.identity,
                saved.declaration,
                saved.frames,
                saved.fuel,
                STACK_LIMITS,
                |_, _, _, _| -> Result<(), PrivateError> {
                    calls.set(calls.get() + 1);
                    match phase {
                        0 => panic!("closed scope must not admit"),
                        1 => Err(PrivateError("private saved credentials")),
                        _ => panic!("native admission unwind"),
                    }
                },
            )
        }));
        if phase == 2 {
            assert!(result.is_err());
        } else {
            let error = result.unwrap().unwrap_err();
            assert!(!format!("{error:?} {error}").contains("private"));
            if phase == 1 {
                assert!(matches!(
                    error,
                    StackedSessionRestoreError::Native(PrivateError("private saved credentials"))
                ));
            } else {
                assert!(matches!(error, StackedSessionRestoreError::ScopeClosed));
            }
        }
        assert_eq!(calls.get(), usize::from(phase != 0));
        assert!(!lease.is_open());
        assert_eq!(hooks.capture_drops.get(), 2);
        assert_eq!(hooks.key_drops.get(), 1);
        assert!(
            hooks
                .cleanup_liveness
                .borrow()
                .iter()
                .all(|(_, closed)| *closed)
        );
        assert!(hooks.events.borrow().is_empty());
    }
}

#[test]
fn restored_session_cancel_or_drop_before_or_after_receipt_revokes_without_reentry() {
    for accepted in [false, true] {
        for disposal in 0..3 {
            host!(hooks, domains, host);
            let expression = nested_identity_program();
            let saved = saved_session(&expression, &host);
            drop(saved.dispatch);
            let owner = LocalExecutionScope::new();
            let lease = owner.lease();
            hooks.lease.replace(Some(lease.clone()));
            let mut session = LocalRestoredSession::restore_pending(
                owner,
                saved.identity,
                saved.declaration,
                saved.frames,
                saved.fuel,
                STACK_LIMITS,
                |_, _, _, _| Ok::<_, PrivateError>(()),
            )
            .unwrap();
            assert!(lease.is_open());
            if accepted {
                session
                    .try_accept(
                        &host.identity(1, 1),
                        host.reply(ScalarValue::Integer(42)),
                        &host,
                    )
                    .unwrap();
            }
            hooks.events.borrow_mut().clear();
            let fuel = session.fuel_remaining();
            if disposal == 0 {
                drop(session);
            } else {
                hooks.capture_drop_panic.set(disposal == 2);
                let outcome = catch_unwind(AssertUnwindSafe(|| session.cancel()));
                if disposal == 2 {
                    assert!(outcome.is_err());
                } else {
                    assert!(outcome.unwrap());
                }
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::Cancelled)
                );
                assert!(!session.cancel());
                assert!(matches!(
                    session.poll(&host).unwrap(),
                    EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
                ));
                let reply = host.reply(ScalarValue::Integer(42));
                let pointer = (&*reply.buffer as *const u8) as usize;
                let rejected = session
                    .try_accept(&host.identity(1, 1), reply, &host)
                    .unwrap_err();
                assert_eq!((&*rejected.reply.buffer as *const u8) as usize, pointer);
                assert_eq!(session.fuel_remaining(), fuel);
            }
            assert!(!lease.is_open());
            assert_eq!(hooks.capture_drops.get(), 2);
            assert_eq!(hooks.key_drops.get(), 1);
            assert_eq!(
                hooks.reply_drops.get(),
                usize::from(accepted) + usize::from(disposal != 0)
            );
            assert!(
                hooks
                    .cleanup_liveness
                    .borrow()
                    .iter()
                    .all(|(_, closed)| *closed)
            );
            assert!(hooks.events.borrow().is_empty());
            assert_eq!(host.invocations.get(), 0);
        }
    }
}

#[test]
fn restored_session_acceptance_unwind_keeps_original_terminal_fence() {
    for point in [
        Stage::Equality,
        Stage::Authority,
        Stage::Borrow,
        Stage::Type,
        Stage::Domain,
    ] {
        host!(hooks, domains, host);
        let expression = nested_identity_program();
        let saved = saved_session(&expression, &host);
        drop(saved.dispatch);
        let owner = LocalExecutionScope::new();
        let lease = owner.lease();
        hooks.lease.replace(Some(lease.clone()));
        let mut session = LocalRestoredSession::restore_pending(
            owner,
            saved.identity,
            saved.declaration,
            saved.frames,
            saved.fuel,
            STACK_LIMITS,
            |_, _, _, _| Ok::<_, PrivateError>(()),
        )
        .unwrap();
        hooks.fault.set((point, Mode::Unwind));
        assert!(
            catch_unwind(AssertUnwindSafe(|| session.try_accept(
                &host.identity(1, 1),
                host.reply(ScalarValue::Integer(42)),
                &host,
            )))
            .is_err()
        );
        assert!(!lease.is_open());
        assert_eq!(
            session.status(),
            EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
        );
        assert_eq!(hooks.capture_drops.get(), 2);
        assert_eq!(hooks.key_drops.get(), 1);
        assert_eq!(hooks.reply_drops.get(), 1);
        assert!(
            hooks
                .cleanup_liveness
                .borrow()
                .iter()
                .all(|(_, closed)| *closed)
        );
        assert!(!session.cancel());
    }
}

#[test]
fn restored_session_local_success_keeps_reply_rejection_recoverable_and_closes_on_completion() {
    host!(hooks, domains, host);
    let expression = nested_identity_program();
    let saved = saved_session(&expression, &host);
    drop(saved.dispatch);
    let owner = LocalExecutionScope::new();
    let lease = owner.lease();
    hooks.lease.replace(Some(lease.clone()));
    let mut session = LocalRestoredSession::restore_pending(
        owner,
        saved.identity,
        saved.declaration,
        saved.frames,
        saved.fuel,
        STACK_LIMITS,
        |_, _, _, _| Ok::<_, PrivateError>(()),
    )
    .unwrap();
    let remaining = session.fuel_remaining();
    let mut input = host.reply(ScalarValue::Integer(42));
    let pointer = (&*input.buffer as *const u8) as usize;
    for phase in 0..3 {
        if phase == 0 {
            host.version.set(8);
        }
        input.value = match phase {
            1 => ScalarValue::String("private wrong type".into()),
            2 => ScalarValue::Integer(101),
            _ => ScalarValue::Integer(42),
        };
        let rejected = session
            .try_accept(&host.identity(1, 1), input, &host)
            .unwrap_err();
        assert_eq!((&*rejected.reply.buffer as *const u8) as usize, pointer);
        assert!(lease.is_open());
        assert_eq!(session.status(), EffectSessionStatus::Awaiting);
        assert_eq!(session.fuel_remaining(), remaining);
        input = rejected.reply;
        host.version.set(7);
    }
    input.value = ScalarValue::Integer(42);
    session
        .try_accept(&host.identity(1, 1), input, &host)
        .unwrap();
    assert!(lease.is_open());
    assert!(!hooks.events.borrow().contains(&Stage::Restore));
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(42)))
    ));
    assert!(!lease.is_open());
    assert_eq!(
        session.status(),
        EffectSessionStatus::Terminal(EffectSessionEnd::Completed)
    );
    assert!(!session.cancel());
    assert_eq!(hooks.capture_drops.get(), 2);
    assert_eq!(hooks.key_drops.get(), 1);
    assert_eq!(hooks.reply_drops.get(), 1);
    assert_eq!(host.invocations.get(), 0);
}

#[test]
fn restored_session_refusal_cleanup_unwind_keeps_scope_closed_and_drops_native_error_once() {
    for native in [false, true] {
        host!(hooks, domains, host);
        let expression = nested_identity_program();
        let mut saved = saved_session(&expression, &host);
        drop(saved.dispatch);
        let owner = LocalExecutionScope::new();
        let lease = owner.lease();
        hooks.lease.replace(Some(lease.clone()));
        if !native {
            saved.frames[0].name = "invalid name";
        }
        hooks.capture_drop_panic.set(true);
        let before = PRIVATE_ERROR_DROPS.with(Cell::get);
        let calls = Cell::new(0);
        assert!(
            catch_unwind(AssertUnwindSafe(|| LocalRestoredSession::restore_pending(
                owner,
                saved.identity,
                saved.declaration,
                saved.frames,
                saved.fuel,
                STACK_LIMITS,
                |_, _, _, _| {
                    calls.set(calls.get() + 1);
                    Err(PrivateError("private rejected payload"))
                },
            )))
            .is_err()
        );
        assert_eq!(calls.get(), usize::from(native));
        assert_eq!(
            PRIVATE_ERROR_DROPS.with(Cell::get) - before,
            usize::from(native)
        );
        assert!(!lease.is_open());
        assert_eq!(hooks.capture_drops.get(), 2);
        assert_eq!(hooks.key_drops.get(), 1);
        assert!(
            hooks
                .cleanup_liveness
                .borrow()
                .iter()
                .all(|(_, closed)| *closed)
        );
    }
}

#[test]
fn restored_session_rechecks_cold_authority_and_exact_remaining_fuel_without_refill() {
    for phase in 0..3 {
        host!(hooks, domains, host);
        let expression = nested_program();
        let saved = saved_session(&expression, &host);
        drop(saved.dispatch);
        let owner = LocalExecutionScope::new();
        let lease = owner.lease();
        hooks.lease.replace(Some(lease.clone()));
        // Host admission is deliberately permissive here to exercise zero-fuel execution.
        let fuel = if phase == 0 { Fuel::new(0) } else { saved.fuel };
        let remaining = fuel.remaining();
        let mut session = LocalRestoredSession::restore_pending(
            owner,
            saved.identity,
            saved.declaration,
            saved.frames,
            fuel,
            STACK_LIMITS,
            |_, _, _, fuel| {
                assert_eq!(fuel.remaining(), remaining);
                Ok::<_, PrivateError>(())
            },
        )
        .unwrap();
        session
            .try_accept(
                &host.identity(1, 1),
                host.reply(ScalarValue::Integer(42)),
                &host,
            )
            .unwrap();
        if phase == 1 {
            host.version.set(8);
        }
        if phase == 2 {
            hooks.fault.set((Stage::Restore, Mode::Unwind));
        }
        hooks.events.borrow_mut().clear();
        let result = catch_unwind(AssertUnwindSafe(|| session.poll(&host)));
        let end = if phase == 2 {
            assert!(result.is_err());
            EffectSessionEnd::HostUncertain
        } else {
            assert!(result.unwrap().is_err());
            EffectSessionEnd::Failed
        };
        assert!(!lease.is_open());
        assert_eq!(session.status(), EffectSessionStatus::Terminal(end));
        assert_eq!(session.fuel_remaining(), remaining);
        assert!(!hooks.events.borrow().contains(&Stage::Prepare));
        assert!(!hooks.events.borrow().contains(&Stage::Correlate));
        if phase == 1 {
            assert!(!hooks.events.borrow().contains(&Stage::Restore));
        }
        let events = hooks.events.borrow().len();
        assert!(matches!(
            session.poll(&host).unwrap(),
            EffectSessionPoll::Terminal(_)
        ));
        assert!(!session.cancel());
        assert_eq!(hooks.events.borrow().len(), events);
        assert_eq!(session.fuel_remaining(), remaining);
    }
}

fn nested_program() -> Node {
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
                        call(2, add(local("read"), local("helper"))),
                        add(local("write"), integer(1)),
                    ),
                ),
            ),
            add(local("answer"), local("caller")),
        ),
    )
}
fn nested_identity_program() -> Node {
    bind(
        "outer",
        bind("inner", call(1, integer(0)), local("inner")),
        local("outer"),
    )
}

#[test]
fn local_reentry_revokes_retained_owners_before_failure_cleanup() {
    for stage in [Stage::Preflight, Stage::Restore, Stage::Project] {
        for mode in [Mode::Reject, Mode::Unwind] {
            host!(hooks, domains, host);
            let expression = nested_program();
            let owner = LocalExecutionScope::new();
            let lease = owner.lease();
            let sibling = LocalExecutionScope::new();
            hooks.lease.replace(Some(lease.clone()));
            let mut session = LocalStackedEffectSession::new(
                owner,
                &expression,
                Vec::new(),
                Fuel::new(100),
                STACK_LIMITS,
            );
            drop(request(session.poll(&host).unwrap()));
            session
                .try_accept(
                    &host.identity(1, 1),
                    host.reply(ScalarValue::Integer(42)),
                    &host,
                )
                .unwrap();
            hooks.cleanup_liveness.borrow_mut().clear();
            hooks.fault.set((stage, mode));
            let result = catch_unwind(AssertUnwindSafe(|| session.poll(&host)));
            let end = if mode == Mode::Unwind {
                assert!(result.is_err());
                EffectSessionEnd::HostUncertain
            } else {
                assert!(result.unwrap().is_err());
                EffectSessionEnd::Failed
            };
            assert_eq!(session.status(), EffectSessionStatus::Terminal(end));
            assert!(!lease.is_open());
            assert!(sibling.is_open());
            let events = hooks.cleanup_liveness.borrow();
            assert!(events.contains(&("identity", true)));
            assert_eq!(
                events.iter().filter(|(role, _)| *role == "capture").count(),
                2
            );
            assert_eq!(
                events.iter().rev().find(|(role, _)| *role == "capture"),
                Some(&("capture", true))
            );
            // A reply already consumed by the native projection callback is outside the fence.
            assert!(events.contains(&("reply", stage != Stage::Project)));
            drop(events);
            assert!(!session.cancel());
            let calls = hooks.events.borrow().len();
            let remaining = session.fuel_remaining();
            assert!(matches!(
                session.poll(&host).unwrap(),
                EffectSessionPoll::Terminal(_)
            ));
            assert_eq!(hooks.events.borrow().len(), calls);
            assert_eq!(session.fuel_remaining(), remaining);
            assert_eq!(host.serial.get(), 1);
        }
    }
}

#[test]
fn local_correlation_failure_closes_retained_successor_frames_before_cleanup() {
    for reentry in [false, true] {
        for mode in [Mode::Reject, Mode::Unwind] {
            for cleanup_panics in [false, true] {
                if cleanup_panics && mode == Mode::Unwind {
                    continue;
                }
                host!(hooks, domains, host);
                let expression = nested_program();
                let owner = LocalExecutionScope::new();
                let lease = owner.lease();
                hooks.lease.replace(Some(lease.clone()));
                let mut session = LocalStackedEffectSession::new(
                    owner,
                    &expression,
                    Vec::new(),
                    Fuel::new(100),
                    STACK_LIMITS,
                );
                if reentry {
                    drop(request(session.poll(&host).unwrap()));
                    session
                        .try_accept(
                            &host.identity(1, 1),
                            host.reply(ScalarValue::Integer(42)),
                            &host,
                        )
                        .unwrap();
                }
                hooks.cleanup_liveness.borrow_mut().clear();
                hooks.fault.set((Stage::Correlate, mode));
                // Only arm cleanup panic once correlation has actually failed.
                hooks.correlation_cleanup_panic.set(cleanup_panics);
                let result = catch_unwind(AssertUnwindSafe(|| session.poll(&host)));
                let uncertain = mode == Mode::Unwind || cleanup_panics;
                if uncertain {
                    assert!(result.is_err());
                } else {
                    assert!(result.unwrap().is_err());
                }
                assert!(!lease.is_open());
                let events = hooks.cleanup_liveness.borrow();
                let captures: Vec<_> = events
                    .iter()
                    .filter(|(role, _)| *role == "capture")
                    .collect();
                assert!(captures.len() >= 2);
                assert!(captures.iter().rev().take(2).all(|(_, closed)| *closed));
                assert!(events.contains(&("request", false)));
                drop(events);
                assert_eq!(host.serial.get(), usize::from(reentry));
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(if uncertain {
                        EffectSessionEnd::HostUncertain
                    } else {
                        EffectSessionEnd::Failed
                    })
                );
                assert!(!session.cancel());
            }
        }
    }
}

#[test]
fn local_identity_cleanup_unwind_revokes_before_computed_output_disposal() {
    for binding in [false, true] {
        host!(hooks, domains, host);
        let expression = if binding {
            bind("result", call(1, integer(1)), local("result"))
        } else {
            call(1, integer(1))
        };
        let owner = LocalExecutionScope::new();
        let lease = owner.lease();
        hooks.lease.replace(Some(lease.clone()));
        let mut session = LocalStackedEffectSession::new(
            owner,
            &expression,
            Vec::new(),
            Fuel::new(100),
            STACK_LIMITS,
        );
        drop(request(session.poll(&host).unwrap()));
        session
            .try_accept(
                &host.identity(1, 1),
                host.reply(ScalarValue::Integer(42)),
                &host,
            )
            .unwrap();
        hooks.cleanup_liveness.borrow_mut().clear();
        hooks.fault.set((Stage::Project, Mode::Owned));
        hooks.key_drop_panic.set(true);
        assert!(catch_unwind(AssertUnwindSafe(|| session.poll(&host))).is_err());
        assert!(!lease.is_open());
        assert_eq!(
            session.status(),
            EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
        );
        let events = hooks.cleanup_liveness.borrow();
        assert!(events.contains(&("identity", false)));
        assert!(events.contains(&("value", true)));
        assert_eq!(
            events.iter().filter(|(role, _)| *role == "value").count(),
            1
        );
        drop(events);
        assert!(!session.cancel());
    }
}

#[test]
fn local_owned_prefix_cleanup_follows_terminal_slice_liveness() {
    for reentry in [false, true] {
        for mode in [Mode::Reject, Mode::Unwind] {
            for cleanup_panics in [false, true] {
                if cleanup_panics && mode == Mode::Unwind {
                    continue;
                }
                host!(hooks, domains, host);
                let expression = if reentry {
                    bind("result", call(1, integer(1)), local("result"))
                } else {
                    call(1, integer(1))
                };
                let owner = LocalExecutionScope::new();
                let lease = owner.lease();
                hooks.lease.replace(Some(lease.clone()));
                let panic = Rc::new(Cell::new(false));
                let drops = Rc::new(Cell::new(0));
                let value = Rc::new(View {
                    panic: panic.clone(),
                    drops: drops.clone(),
                    hooks: Some(hooks.clone()),
                });
                let mut session = LocalStackedEffectSession::new(
                    owner,
                    &expression,
                    vec![("native", PureValue::Result(value))],
                    Fuel::new(100),
                    STACK_LIMITS,
                );
                if reentry {
                    drop(request(session.poll(&host).unwrap()));
                    session
                        .try_accept(
                            &host.identity(1, 1),
                            host.reply(ScalarValue::Integer(42)),
                            &host,
                        )
                        .unwrap();
                }
                panic.set(cleanup_panics);
                hooks.fault.set((
                    if reentry {
                        Stage::Project
                    } else {
                        Stage::Preflight
                    },
                    mode,
                ));
                let result = catch_unwind(AssertUnwindSafe(|| session.poll(&host)));
                let uncertain = cleanup_panics || mode == Mode::Unwind;
                if uncertain {
                    assert!(result.is_err());
                } else {
                    assert!(result.unwrap().is_err());
                }
                assert_eq!(drops.get(), 1);
                assert!(hooks.cleanup_liveness.borrow().contains(&("value", true)));
                assert!(!lease.is_open());
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(if uncertain {
                        EffectSessionEnd::HostUncertain
                    } else {
                        EffectSessionEnd::Failed
                    })
                );
                assert!(!session.cancel());
            }
        }
    }
}

#[test]
fn local_invalid_restored_prefix_revokes_before_owned_value_disposal() {
    host!(hooks, domains, host);
    let expression = nested_identity_program();
    let owner = LocalExecutionScope::new();
    let lease = owner.lease();
    hooks.lease.replace(Some(lease.clone()));
    let mut session = LocalStackedEffectSession::new(
        owner,
        &expression,
        Vec::new(),
        Fuel::new(100),
        STACK_LIMITS,
    );
    drop(request(session.poll(&host).unwrap()));
    session
        .try_accept(
            &host.identity(1, 1),
            host.reply(ScalarValue::Integer(42)),
            &host,
        )
        .unwrap();
    hooks.cleanup_liveness.borrow_mut().clear();
    hooks.fault.set((Stage::Restore, Mode::Unbounded));
    assert!(session.poll(&host).is_err());
    assert_eq!(
        session.status(),
        EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
    );
    assert!(hooks.cleanup_liveness.borrow().contains(&("value", true)));
    assert!(hooks.cleanup_liveness.borrow().contains(&("reply", true)));
    assert!(
        hooks
            .cleanup_liveness
            .borrow()
            .contains(&("identity", true))
    );
    assert!(!hooks.events.borrow().contains(&Stage::Project));
}

#[test]
fn stacked_session_owns_repeated_request_returns_original_scopes_and_one_meter() {
    host!(hooks, domains, host);
    let expression = nested_program();
    let mut session =
        StackedEffectSession::new(&expression, Vec::new(), Fuel::new(100), STACK_LIMITS);
    assert_eq!(session.status(), EffectSessionStatus::Ready);
    assert!(hooks.events.borrow().is_empty());
    let first = request(session.poll(&host).unwrap());
    assert_eq!(first.amount, 0);
    assert_eq!(
        (&*first.buffer as *const u8) as usize,
        host.prepared_pointer.get()
    );
    assert_eq!(session.fuel_remaining(), 89);
    let events = hooks.events.borrow().len();
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Awaiting
    ));
    assert_eq!(hooks.events.borrow().len(), events);
    let reply = host.invoke(first);
    session
        .try_accept(&host.identity(1, 1), reply, &host)
        .unwrap();
    assert_eq!(session.status(), EffectSessionStatus::Ready);
    assert_eq!(session.fuel_remaining(), 89);
    assert!(!hooks.events.borrow().contains(&Stage::Restore));
    let second = request(session.poll(&host).unwrap());
    assert_eq!(second.amount, 42);
    assert_eq!(session.fuel_remaining(), 79);
    let reply = host.invoke(second);
    session
        .try_accept(&host.identity(2, 2), reply, &host)
        .unwrap();
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(53)))
    ));
    assert_eq!(session.fuel_remaining(), 69);
    assert_eq!(host.invocations.get(), 2);
    assert_eq!(host.actual.get(), 42);
    assert_eq!(
        hooks
            .events
            .borrow()
            .iter()
            .filter(|&&s| s == Stage::Project)
            .count(),
        2
    );
    assert_eq!(
        hooks
            .events
            .borrow()
            .iter()
            .filter(|&&s| s == Stage::Restore)
            .count(),
        3
    );
    assert_eq!(hooks.capture_drops.get(), 3);
    assert_eq!(hooks.reply_drops.get(), 2);
    assert_eq!(hooks.key_drops.get(), 2);
    let events = hooks.events.borrow().len();
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
    ));
    assert_eq!(hooks.events.borrow().len(), events);
    assert!(!session.cancel());
}

#[test]
fn stacked_session_preserves_original_rejected_input_and_acceptance_is_not_execution() {
    host!(hooks, domains, host);
    let expression = nested_identity_program();
    let mut session =
        StackedEffectSession::new(&expression, Vec::new(), Fuel::new(100), STACK_LIMITS);
    let early = host.reply(ScalarValue::Integer(42));
    let pointer = (&*early.buffer as *const u8) as usize;
    let rejected = session
        .try_accept(&host.identity(1, 1), early, &host)
        .unwrap_err();
    assert_eq!((&*rejected.reply.buffer as *const u8) as usize, pointer);
    assert!(hooks.events.borrow().is_empty());
    let native = request(session.poll(&host).unwrap());
    drop(rejected);
    let reply = host.invoke(native);
    let pointer = (&*reply.buffer as *const u8) as usize;
    let mut input = reply;
    for phase in 0..4 {
        let (serial, value) = match phase {
            0 => (2, ScalarValue::Integer(42)),
            1 => {
                host.version.set(8);
                (1, ScalarValue::Integer(42))
            }
            2 => (1, ScalarValue::String("private input".into())),
            _ => (1, ScalarValue::Integer(101)),
        };
        input.value = value;
        let rejected = session
            .try_accept(&host.identity(serial, 1), input, &host)
            .unwrap_err();
        assert_eq!((&*rejected.reply.buffer as *const u8) as usize, pointer);
        assert!(!format!("{rejected:?}").contains("private"));
        assert_eq!(session.status(), EffectSessionStatus::Awaiting);
        input = rejected.reply;
        host.version.set(7);
    }
    input.value = ScalarValue::Integer(42);
    let fuel = session.fuel_remaining();
    session
        .try_accept(&host.identity(1, 1), input, &host)
        .unwrap();
    assert_eq!(session.fuel_remaining(), fuel);
    assert!(!hooks.events.borrow().contains(&Stage::Restore));
    let duplicate = host.reply(ScalarValue::Integer(42));
    let events = hooks.events.borrow().len();
    let rejected = session
        .try_accept(&host.identity(1, 1), duplicate, &host)
        .unwrap_err();
    assert!(matches!(
        rejected.error,
        EffectSessionReplyError::NotAwaiting(EffectSessionStatus::Ready)
    ));
    assert_eq!(hooks.events.borrow().len(), events);
    assert!(matches!(
        session.poll(&host).unwrap(),
        EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(42)))
    ));
}

#[test]
fn stacked_session_cancel_initial_pending_accepted_or_final_keeps_all_later_polls_inert() {
    for nested in [false, true] {
        for stage in 0..3 {
            host!(hooks, domains, host);
            let expression = if nested {
                nested_identity_program()
            } else {
                call(1, integer(0))
            };
            let mut session =
                StackedEffectSession::new(&expression, Vec::new(), Fuel::new(100), STACK_LIMITS);
            if stage > 0 {
                drop(request(session.poll(&host).unwrap()));
            }
            if stage > 1 {
                session
                    .try_accept(
                        &host.identity(1, 1),
                        host.reply(ScalarValue::Integer(42)),
                        &host,
                    )
                    .unwrap();
            }
            let fuel = session.fuel_remaining();
            hooks.events.borrow_mut().clear();
            assert!(session.cancel());
            assert!(!session.cancel());
            assert!(matches!(
                session.poll(&host).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
            ));
            let input = host.reply(ScalarValue::Integer(42));
            let pointer = (&*input.buffer as *const u8) as usize;
            let rejected = session
                .try_accept(&host.identity(1, 1), input, &host)
                .unwrap_err();
            assert_eq!((&*rejected.reply.buffer as *const u8) as usize, pointer);
            assert!(hooks.events.borrow().is_empty());
            assert_eq!(session.fuel_remaining(), fuel);
            assert_eq!(
                hooks.capture_drops.get(),
                if nested && stage > 0 { 2 } else { 0 }
            );
            assert_eq!(host.invocations.get(), 0);
        }
    }
}

#[test]
fn stacked_session_drop_releases_every_pending_or_accepted_frame_without_reentry() {
    for accepted in [false, true] {
        host!(hooks, domains, host);
        let expression = nested_identity_program();
        let mut session =
            StackedEffectSession::new(&expression, Vec::new(), Fuel::new(100), STACK_LIMITS);
        drop(request(session.poll(&host).unwrap()));
        if accepted {
            session
                .try_accept(
                    &host.identity(1, 1),
                    host.reply(ScalarValue::Integer(42)),
                    &host,
                )
                .unwrap();
        }
        drop(session);
        assert_eq!(hooks.capture_drops.get(), 2);
        assert_eq!(hooks.key_drops.get(), 1);
        assert_eq!(hooks.reply_drops.get(), usize::from(accepted));
        assert!(!hooks.events.borrow().contains(&Stage::Restore));
        assert_eq!(host.invocations.get(), 0);
    }
}

#[test]
fn stacked_session_limits_are_terminal_and_legacy_session_still_rejects_nested_returns() {
    for max_frames in [MAX_EFFECT_RETURN_FRAMES + 1, 0, 1] {
        host!(hooks, domains, host);
        let expression = nested_identity_program();
        let mut session = StackedEffectSession::new(
            &expression,
            Vec::new(),
            Fuel::new(100),
            EffectStackLimits {
                max_frames,
                ..STACK_LIMITS
            },
        );
        let error = session.poll(&host).unwrap_err();
        assert!(matches!(
            error,
            CalculationFailure::External(
                EffectStackFault::InvalidLimits | EffectStackFault::ReturnLimit
            )
        ));
        assert_eq!(
            session.status(),
            EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
        );
        assert!(!hooks.events.borrow().contains(&Stage::Correlate));
        assert_eq!(host.invocations.get(), 0);
        let fuel = session.fuel_remaining();
        assert!(matches!(
            session.poll(&host).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
        ));
        assert_eq!(session.fuel_remaining(), fuel);
        assert_eq!(hooks.capture_drops.get(), usize::from(max_frames == 1));
        if max_frames > MAX_EFFECT_RETURN_FRAMES {
            assert_eq!(fuel, 100);
        }
    }
    host!(hooks, domains, host);
    let expression = nested_identity_program();
    let mut legacy = EffectSession::new(&expression, Vec::new(), Fuel::new(100), LIMITS);
    assert!(matches!(
        legacy.poll(&host).unwrap_err(),
        CalculationFailure::External(EffectEvaluationFault::NestedSuspension)
    ));
    assert_eq!(hooks.capture_drops.get(), 1);
    assert!(!hooks.events.borrow().contains(&Stage::Correlate));
}

#[test]
fn stacked_session_every_acceptance_unwind_seals_before_all_frame_cleanup() {
    for point in [
        Stage::Equality,
        Stage::Authority,
        Stage::Borrow,
        Stage::Type,
        Stage::Domain,
    ] {
        host!(hooks, domains, host);
        let expression = nested_identity_program();
        let mut session =
            StackedEffectSession::new(&expression, Vec::new(), Fuel::new(100), STACK_LIMITS);
        drop(request(session.poll(&host).unwrap()));
        hooks.fault.set((point, Mode::Unwind));
        assert!(
            catch_unwind(AssertUnwindSafe(|| session.try_accept(
                &host.identity(1, 1),
                host.reply(ScalarValue::Integer(42)),
                &host
            )))
            .is_err()
        );
        assert_eq!(
            session.status(),
            EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
        );
        assert_eq!(hooks.capture_drops.get(), 2);
        assert_eq!(hooks.key_drops.get(), 1);
        assert_eq!(hooks.reply_drops.get(), 1);
        assert!(!session.cancel());
        let count = hooks.events.borrow().len();
        assert!(matches!(
            session.poll(&host).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::HostUncertain)
        ));
        assert_eq!(hooks.events.borrow().len(), count);
    }
}

#[test]
fn stacked_session_preparation_correlation_restore_and_projection_faults_never_rearm() {
    for point in [
        Stage::Preflight,
        Stage::Prepare,
        Stage::Capture,
        Stage::Correlate,
        Stage::Restore,
        Stage::Project,
    ] {
        for mode in [Mode::Reject, Mode::Unwind] {
            host!(hooks, domains, host);
            let expression = nested_identity_program();
            let mut session =
                StackedEffectSession::new(&expression, Vec::new(), Fuel::new(100), STACK_LIMITS);
            if matches!(point, Stage::Restore | Stage::Project) {
                drop(request(session.poll(&host).unwrap()));
                session
                    .try_accept(
                        &host.identity(1, 1),
                        host.reply(ScalarValue::Integer(42)),
                        &host,
                    )
                    .unwrap();
            }
            hooks.fault.set((point, mode));
            let outcome = catch_unwind(AssertUnwindSafe(|| session.poll(&host)));
            let reason = if mode == Mode::Unwind {
                assert!(outcome.is_err());
                EffectSessionEnd::HostUncertain
            } else {
                let error = outcome.unwrap().unwrap_err();
                assert!(!format!("{error:?}").contains("private"));
                EffectSessionEnd::Failed
            };
            assert_eq!(session.status(), EffectSessionStatus::Terminal(reason));
            assert!(!session.cancel());
            let count = hooks.events.borrow().len();
            let fuel = session.fuel_remaining();
            assert!(matches!(
                session.poll(&host).unwrap(),
                EffectSessionPoll::Terminal(_)
            ));
            assert_eq!(hooks.events.borrow().len(), count);
            assert_eq!(session.fuel_remaining(), fuel);
            if matches!(point, Stage::Correlate | Stage::Restore | Stage::Project) {
                assert_eq!(hooks.capture_drops.get(), 2);
            }
            assert_eq!(host.invocations.get(), 0);
        }
    }
}

#[test]
fn stacked_session_cleanup_unwind_keeps_cancelled_or_host_uncertain_without_partial_handoff() {
    for accepted in [false, true] {
        for cancellation in [false, true] {
            host!(hooks, domains, host);
            let expression = nested_identity_program();
            let mut session =
                StackedEffectSession::new(&expression, Vec::new(), Fuel::new(100), STACK_LIMITS);
            drop(request(session.poll(&host).unwrap()));
            if accepted {
                session
                    .try_accept(
                        &host.identity(1, 1),
                        host.reply(ScalarValue::Integer(42)),
                        &host,
                    )
                    .unwrap();
            }
            hooks.capture_drop_panic.set(true);
            if cancellation {
                assert!(catch_unwind(AssertUnwindSafe(|| session.cancel())).is_err());
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::Cancelled)
                );
            } else if accepted {
                assert!(catch_unwind(AssertUnwindSafe(|| session.poll(&host))).is_err());
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::HostUncertain)
                );
            } else {
                // Idle polling does not even drop a pending frame.
                assert!(matches!(
                    session.poll(&host).unwrap(),
                    EffectSessionPoll::Awaiting
                ));
                assert!(catch_unwind(AssertUnwindSafe(|| session.cancel())).is_err());
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::Cancelled)
                );
            }
            assert_eq!(hooks.capture_drops.get(), 2);
            assert_eq!(hooks.key_drops.get(), 1);
            assert_eq!(hooks.reply_drops.get(), usize::from(accepted));
            assert!(!session.cancel());
        }
    }
}

#[test]
fn stacked_session_revocation_and_exhaustion_after_acceptance_do_not_refill_or_replay() {
    for revoke in [false, true] {
        host!(hooks, domains, host);
        let expression = nested_identity_program();
        let mut session = StackedEffectSession::new(
            &expression,
            Vec::new(),
            Fuel::new(if revoke { 100 } else { 4 }),
            STACK_LIMITS,
        );
        drop(request(session.poll(&host).unwrap()));
        let fuel = session.fuel_remaining();
        session
            .try_accept(
                &host.identity(1, 1),
                host.reply(ScalarValue::Integer(42)),
                &host,
            )
            .unwrap();
        if revoke {
            host.version.set(8);
        }
        assert!(session.poll(&host).is_err());
        assert_eq!(
            session.status(),
            EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
        );
        assert_eq!(session.fuel_remaining(), fuel);
        assert_eq!(hooks.capture_drops.get(), 2);
        assert_eq!(host.invocations.get(), 0);
        assert!(matches!(
            session.poll(&host).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
        ));
        assert_eq!(session.fuel_remaining(), fuel);
    }
}

#[test]
fn stacked_session_zero_frames_pure_final_reply_bounds_and_debug_remain_native_free() {
    for phase in 0..3 {
        let pure = phase == 0;
        host!(hooks, domains, host);
        let expression = if pure {
            integer(42)
        } else {
            call(1, integer(0))
        };
        let mut session = StackedEffectSession::new(
            &expression,
            Vec::new(),
            Fuel::new(100),
            EffectStackLimits {
                max_frames: 0,
                ..STACK_LIMITS
            },
        );
        if pure {
            assert!(matches!(
                session.poll(&host).unwrap(),
                EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(42)))
            ));
        } else {
            let outcome = session.poll(&host).unwrap();
            assert_eq!(format!("{outcome:?}"), "Request");
            drop(request(outcome));
            assert_eq!(
                format!("{session:?}"),
                "StackedControlSession { status: Awaiting, fuel_remaining: 98 }"
            );
            session
                .try_accept(
                    &host.identity(1, 1),
                    host.reply(ScalarValue::Integer(42)),
                    &host,
                )
                .unwrap();
            assert!(!hooks.events.borrow().contains(&Stage::Project));
            if phase == 2 {
                hooks.fault.set((Stage::Project, Mode::Unbounded));
                assert!(session.poll(&host).is_err());
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                );
            } else {
                assert!(matches!(
                    session.poll(&host).unwrap(),
                    EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(42)))
                ));
                assert_eq!(
                    session.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::Completed)
                );
            }
        }
        assert_eq!(hooks.capture_drops.get(), 0);
    }
}
