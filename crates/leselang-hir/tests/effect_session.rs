use std::borrow::Borrow;
use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::effect_evaluation::*;
use leselang_hir::effect_reentry::*;
use leselang_hir::effect_session::*;
use leselang_hir::ir::{Computation, ComputedArgument};
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
}
impl Drop for View {
    fn drop(&mut self) {
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
        self.hooks
            .capture_drops
            .set(self.hooks.capture_drops.get() + 1);
        assert!(
            !self.hooks.capture_drop_panic.get(),
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
    assert!(catch_unwind(AssertUnwindSafe(|| session.poll(&host))).is_err());
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
