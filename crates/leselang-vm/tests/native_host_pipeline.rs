//! Continuous two-host atomic and mixed-group pipelines through the native VM facade.
//! Durable recovery, full language parity and real GUI integration remain separate gates.
use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    marker::PhantomData,
};

use call_evaluation::{CallEvaluationLimits, PreparedCall, evaluate_call_arguments_in_scope};
use catalog_program::{CatalogProgramLimits, compile_catalog_program};
use catalog_source::{CatalogSourceEnvironment, CatalogSourceLimits};
use control_source::ControlSourceLimits;
use effect_evaluation::{EffectEvaluationEnvironment, EffectEvaluationLimits};
use effect_reentry::{EffectReentryEnvironment, RestoredEffectBindings};
use effect_session::{
    CorrelatedEffectRequest, EffectCorrelation, EffectSessionEnd, EffectSessionEnvironment,
    EffectSessionPoll, EffectSessionStatus,
};
use effect_stack::EffectStackLimits;
use flow_typing::CallFlowEnvironment;
use ir::Computation;
use leselang_vm::native::{LocalVm, core::*, hir::*, syntax::parse};
use pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationLimits, PureValue, scalar_copy_cost,
};
use pure_typing::{PureType, PureTypeEnvironment};
use source_call::{SourceCallHost, SourceCallLimits};

#[derive(Debug, PartialEq)]
struct Refusal;

// This is fixture adapter policy, not a language trait or an alternate evaluator.
trait Profile: Default {
    type Value: Clone;
    const OPERATION: &'static str;
    const PARAMETER: &'static str;
    const ARGUMENT: ScalarType;
    const PROGRAM: &'static str;
    fn result_type() -> PureType<()>;
    fn field_type(name: &str) -> Option<ScalarType>;
    fn field(value: &Self::Value, name: &str) -> Result<ScalarValue, Refusal>;
    fn matches(value: &Self::Value) -> bool;
    fn validate(value: &Self::Value) -> Result<(), Refusal>;
    fn invoke(&self, input: ScalarValue) -> Self::Value;
    fn bind(value: Self::Value) -> PureValue<Self::Value>;
    fn invalid_value() -> Self::Value;
    fn expected() -> ScalarValue;
    fn arguments() -> Vec<ScalarValue>;
}

#[derive(Default)]
struct Counter(Cell<u64>);
impl Profile for Counter {
    type Value = ScalarValue;
    const OPERATION: &'static str = "counter.add";
    const PARAMETER: &'static str = "amount";
    const ARGUMENT: ScalarType = ScalarType::Integer;
    const PROGRAM: &'static str = r#"
fn count(n: integer) = bind(answer: counter.add(amount: n), body: add(left: answer, right: 1))
fn main() = bind(step: loop(i: 0, while: lt(left: i, right: 3), next: add(left: i, right: 1), limit: 3),
 body: bind(seed: fold(total: step, items: strings(a: "a", b: "b"), item: "word", next: add(left: total, right: 1), limit: 2),
 body: bind(first: count(n: seed),
 body: bind(second: count(n: recover(value: div(left: first, right: 0), fallback: add(left: first, right: 1))),
 body: add(left: first, right: second)))))
"#;
    fn result_type() -> PureType<()> {
        PureType::Scalar(ScalarType::Integer)
    }
    fn field_type(_: &str) -> Option<ScalarType> {
        None
    }
    fn field(_: &ScalarValue, _: &str) -> Result<ScalarValue, Refusal> {
        Err(Refusal)
    }
    fn matches(value: &ScalarValue) -> bool {
        value.scalar_type() == ScalarType::Integer
    }
    fn validate(value: &ScalarValue) -> Result<(), Refusal> {
        if matches!(value, ScalarValue::Integer(n) if *n <= 100) {
            Ok(())
        } else {
            Err(Refusal)
        }
    }
    fn invoke(&self, input: ScalarValue) -> ScalarValue {
        let ScalarValue::Integer(amount) = input else {
            panic!("typed counter input")
        };
        self.0.set(self.0.get() + amount);
        ScalarValue::Integer(self.0.get())
    }
    fn bind(value: ScalarValue) -> PureValue<ScalarValue> {
        PureValue::Scalar(value)
    }
    fn invalid_value() -> ScalarValue {
        ScalarValue::Integer(101)
    }
    fn expected() -> ScalarValue {
        ScalarValue::Integer(19)
    }
    fn arguments() -> Vec<ScalarValue> {
        vec![ScalarValue::Integer(5), ScalarValue::Integer(7)]
    }
}

#[derive(Clone)]
struct WidgetAck {
    text: String,
    visible: bool,
}
#[derive(Default)]
struct Panel(RefCell<String>);
impl Profile for Panel {
    type Value = WidgetAck;
    const OPERATION: &'static str = "panel.set_text";
    const PARAMETER: &'static str = "text";
    const ARGUMENT: ScalarType = ScalarType::String;
    const PROGRAM: &'static str = r#"
fn render(text: string) = bind(ack: panel.set_text(text: text),
 body: choose(when: field(value: ack, name: "visible"), then: field(value: ack, name: "text"), otherwise: "hidden"))
fn main() = bind(word: "ready", body: bind(prefix: fold(text: "", items: strings(a: word, b: "."), item: "part", next: concat(left: text, right: part), limit: 2),
 body: bind(suffix: loop(text: "", while: lt(left: len(value: text), right: 2), next: concat(left: text, right: "!"), limit: 2),
 body: bind(first: render(text: prefix),
 body: bind(second: render(text: concat(left: first, right: suffix)),
 body: concat(left: first, right: concat(left: "/", right: second)))))))
"#;
    fn result_type() -> PureType<()> {
        PureType::Result(())
    }
    fn field_type(name: &str) -> Option<ScalarType> {
        match name {
            "text" => Some(ScalarType::String),
            "visible" => Some(ScalarType::Boolean),
            _ => None,
        }
    }
    fn field(value: &WidgetAck, name: &str) -> Result<ScalarValue, Refusal> {
        match name {
            "text" => Ok(ScalarValue::String(value.text.clone())),
            "visible" => Ok(ScalarValue::Boolean(value.visible)),
            _ => Err(Refusal),
        }
    }
    fn matches(_: &WidgetAck) -> bool {
        true
    }
    fn validate(value: &WidgetAck) -> Result<(), Refusal> {
        if value.text.len() <= 64 {
            Ok(())
        } else {
            Err(Refusal)
        }
    }
    fn invoke(&self, input: ScalarValue) -> WidgetAck {
        let ScalarValue::String(text) = input else {
            panic!("typed panel input")
        };
        self.0.replace(text.clone());
        WidgetAck {
            text,
            visible: true,
        }
    }
    fn bind(value: WidgetAck) -> PureValue<WidgetAck> {
        PureValue::Result(value)
    }
    fn invalid_value() -> WidgetAck {
        WidgetAck {
            text: "x".repeat(65),
            visible: true,
        }
    }
    fn expected() -> ScalarValue {
        ScalarValue::String("ready./ready.!!".into())
    }
    fn arguments() -> Vec<ScalarValue> {
        vec![
            ScalarValue::String("ready.".into()),
            ScalarValue::String("ready.!!".into()),
        ]
    }
}

struct Domain<P>(PhantomData<P>);
impl<P: Profile> HostResultDomain<P::Value> for Domain<P> {
    type Error = Refusal;
    fn matches_type(&self, value: &P::Value) -> bool {
        P::matches(value)
    }
    fn validate_value(&self, value: &P::Value) -> Result<(), Refusal> {
        P::validate(value)
    }
}
type Node = Computation<String, String, (), ()>;
type Catalog<'s, P> = OperationCatalog<'s, String, &'s str, ScalarTypeSet, Domain<P>, u8>;
type Request<'s, P> = PreparedCall<'s, String, ScalarTypeSet, Domain<P>, u8>;
struct Reply<Value> {
    value: Value,
    buffer: Box<u8>,
}
impl<Value> Borrow<Value> for Reply<Value> {
    fn borrow(&self) -> &Value {
        &self.value
    }
}
#[derive(Clone)]
struct Identity {
    owner: LocalExecutionLease,
    serial: u64,
    revision: u64,
}
impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        self.owner.same_scope(&other.owner)
            && self.serial == other.serial
            && self.revision == other.revision
    }
}
struct Dispatch<'s, P> {
    identity: Identity,
    request: Request<'s, P>,
}
struct Host<'h, 's, P> {
    catalog: &'h Catalog<'s, P>,
    profile: P,
    lease: LocalExecutionLease,
    version: Cell<u32>,
    revision: Cell<u64>,
    granted: Cell<bool>,
    serial: Cell<u64>,
    invoked: RefCell<Vec<ScalarValue>>,
}
impl<'s, P: Profile> Host<'_, 's, P> {
    fn source(&self) -> SourceCallHost<'_, 's, String, ScalarTypeSet, Domain<P>, u8> {
        SourceCallHost {
            catalog: self.catalog,
            version: self.version.get(),
            granted: if self.granted.get() { &[7] } else { &[] },
        }
    }
    fn dispatch(&self, dispatch: Dispatch<'_, P>) -> Result<Reply<P::Value>, Refusal> {
        self.authorize(&dispatch.identity)?;
        let source = self.source();
        let schema = source
            .catalog
            .authorize(
                &dispatch.request.schema().key,
                source.version,
                source.granted,
            )
            .map_err(|_| Refusal)?;
        if !std::ptr::eq(schema, dispatch.request.schema()) {
            return Err(Refusal);
        }
        let (_, mut args) = dispatch.request.into_parts();
        let value = args.pop().ok_or(Refusal)?.value;
        self.invoked.borrow_mut().push(value.clone());
        Ok(Reply {
            value: self.profile.invoke(value),
            buffer: Box::new(23),
        })
    }
}
impl<P: Profile> PureTypeEnvironment<String, String> for Host<'_, '_, P> {
    type Result = ();
    fn field_type(&self, _: &(), field: &String) -> Option<ScalarType> {
        P::field_type(field)
    }
    fn member_result(&self, _: &(), _: &str, _: &String) -> Option<()> {
        None
    }
}
impl<'s, P: Profile> CallFlowEnvironment<'s, String, String, Domain<P>> for Host<'_, 's, P> {
    fn call_result_type(&self, _: &String, _: &'s Domain<P>) -> Option<PureType<()>> {
        Some(P::result_type())
    }
}
impl<'source, 's, P: Profile> CatalogSourceEnvironment<'source, 's, String, String, Domain<P>>
    for Host<'_, 's, P>
{
    type Error = Refusal;
    fn source_operation(&self, original: &String) -> Result<String, Refusal> {
        Ok(original.clone())
    }
    fn source_field(
        &self,
        _: &(),
        name: &'source str,
    ) -> Result<Option<(String, ScalarType)>, Refusal> {
        Ok(P::field_type(name).map(|ty| (name.into(), ty)))
    }
    fn source_member(
        &self,
        _: &(),
        _: &'source str,
        _: &'source str,
    ) -> Result<Option<(String, ())>, Refusal> {
        Ok(None)
    }
}
impl<P: Profile> PureEvaluationEnvironment<String, String> for Host<'_, '_, P> {
    type Result = P::Value;
    type Error = Refusal;
    fn field(&self, value: &P::Value, field: &String) -> Result<ScalarValue, Refusal> {
        P::field(value, field)
    }
    fn member(&self, _: &P::Value, _: &str, _: &String) -> Result<P::Value, Refusal> {
        Err(Refusal)
    }
}
const PURE: PureEvaluationLimits = PureEvaluationLimits {
    max_nodes: 1024,
    max_depth: 64,
    max_bindings: 32,
};
const EFFECTS: EffectEvaluationLimits = EffectEvaluationLimits {
    pure: PURE,
    max_arguments: 8,
    max_branches: 8,
};
const STACK: EffectStackLimits = EffectStackLimits {
    effects: EFFECTS,
    max_frames: 16,
};
const PROGRAM: CatalogProgramLimits = CatalogProgramLimits {
    expression: CatalogSourceLimits {
        control: ControlSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 1024,
                max_source_depth: 64,
                max_lowered_nodes: 1024,
                max_lowered_depth: 64,
                max_arguments: 8,
            },
            max_bindings: 32,
        },
        max_calls: 16,
    },
    max_functions: 8,
    max_parameters: 8,
    max_compilation_nodes: 8192,
    max_expansion_depth: 32,
};
impl<'ir, 's, P: Profile> EffectEvaluationEnvironment<'ir, String, String, (), ()>
    for Host<'_, 's, P>
{
    type Request = Request<'s, P>;
    type Capture = RestoredEffectBindings<'ir, P::Value>;
    fn preflight_effect(&self, node: &'ir Node) -> Result<(), Refusal> {
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err(Refusal);
        };
        let source = self.source();
        let schema = source
            .catalog
            .authorize(operation, source.version, source.granted)
            .map_err(|_| Refusal)?;
        schema
            .bind_arguments(
                &arguments
                    .iter()
                    .map(|a| a.name.as_str())
                    .collect::<Vec<_>>(),
            )
            .map_err(|_| Refusal)?;
        Ok(())
    }
    fn prepare_effect(
        &self,
        node: &'ir Node,
        scope: &mut ScopeFrame<'_, 'ir, PureValue<P::Value>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Request, CalculationFailure<Refusal>> {
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err(Refusal.into());
        };
        let source = self.source();
        let schema = source
            .catalog
            .authorize(operation, source.version, source.granted)
            .map_err(|_| Refusal)?;
        evaluate_call_arguments_in_scope(
            arguments,
            schema,
            scope,
            self,
            fuel,
            CallEvaluationLimits {
                pure: PURE,
                max_arguments: 8,
            },
        )
        .map_err(|_| Refusal.into())
    }
    fn capture(
        &self,
        _: &'ir str,
        _: &'ir Node,
        scope: &ScopeFrame<'_, 'ir, PureValue<P::Value>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Capture, Refusal> {
        let mut saved = Vec::new();
        for (name, value) in scope.bindings() {
            let extra = match value {
                PureValue::Scalar(value) => scalar_copy_cost(value),
                _ => 0,
            };
            fuel.charge(1 + extra).map_err(|_| Refusal)?;
            saved.push((*name, value.clone()));
        }
        Ok(saved)
    }
}
impl<P: Profile> ReplyAuthority<Identity> for Host<'_, '_, P> {
    type Error = Refusal;
    fn authorize(&self, identity: &Identity) -> Result<(), Refusal> {
        if !identity.owner.same_scope(&self.lease)
            || !self.lease.is_open()
            || identity.revision != self.revision.get()
            || identity.serial == 0
            || identity.serial > self.serial.get()
            || !self.granted.get()
            || self.version.get() != self.catalog.version()
        {
            Err(Refusal)
        } else {
            Ok(())
        }
    }
}
impl<'ir, 's, P: Profile>
    EffectReentryEnvironment<'ir, String, String, (), (), Identity, Domain<P>, Reply<P::Value>>
    for Host<'_, 's, P>
{
    fn restore_capture(
        &self,
        identity: &Identity,
        _: &Domain<P>,
        capture: Self::Capture,
        _: &mut Fuel,
        _: EffectEvaluationLimits,
    ) -> Result<Self::Capture, CalculationFailure<Refusal>> {
        self.authorize(identity)?;
        Ok(capture)
    }
    fn bind_reply(
        &self,
        identity: &Identity,
        domain: &Domain<P>,
        reply: Reply<P::Value>,
        _: &mut Fuel,
    ) -> Result<PureValue<P::Value>, CalculationFailure<Refusal>> {
        self.authorize(identity)?;
        domain.validate_value(&reply.value)?;
        Ok(P::bind(reply.value))
    }
}
impl<'ir, 's, P: Profile> EffectSessionEnvironment<'ir, 's, String, String, (), ()>
    for Host<'_, 's, P>
{
    type Identity = Identity;
    type Declaration = Domain<P>;
    type Reply = Reply<P::Value>;
    type Dispatch = Dispatch<'s, P>;
    fn correlate_request(
        &self,
        request: Self::Request,
        _: &mut Fuel,
    ) -> EffectCorrelation<'s, Identity, Domain<P>, Self::Dispatch, Refusal> {
        self.serial
            .set(self.serial.get().checked_add(1).ok_or(Refusal)?);
        let identity = Identity {
            owner: self.lease.clone(),
            serial: self.serial.get(),
            revision: self.revision.get(),
        };
        self.authorize(&identity)?;
        Ok(CorrelatedEffectRequest {
            identity: identity.clone(),
            declaration: &request.schema().result,
            dispatch: Dispatch { identity, request },
        })
    }
}
fn compile<P: Profile>(source: &str, host: &Host<'_, '_, P>) -> Result<Node, String> {
    let tree = parse(source);
    if !tree.diagnostics.is_empty() {
        return Err(format!("syntax: {:?}", tree.diagnostics));
    }
    let functions: Vec<_> = tree.function.iter().chain(&tree.helpers).collect();
    compile_catalog_program(&functions, "main", PROGRAM, &host.source(), host)
        .map(|(node, _)| node)
        .map_err(|error| error.to_string())
}
fn fixture<P: Profile>(test: impl FnOnce(Host<'_, '_, P>, LocalExecutionScope)) {
    let parameters = [NamedParameter::required(
        P::PARAMETER,
        ScalarTypeSet::only(P::ARGUMENT),
    )];
    let schemas = [OperationSchema {
        key: P::OPERATION.into(),
        parameters: &parameters,
        result: Domain::<P>(PhantomData),
        required_capability: 7,
    }];
    let catalog = OperationCatalog::new(
        31,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let scope = LocalExecutionScope::new();
    let host = Host {
        catalog: &catalog,
        profile: P::default(),
        lease: scope.lease(),
        version: Cell::new(31),
        revision: Cell::new(1),
        granted: Cell::new(true),
        serial: Cell::new(0),
        invoked: RefCell::new(Vec::new()),
    };
    test(host, scope);
}

fn complete<P: Profile>(budget: u64) -> Result<u64, ()> {
    let outcome = Cell::new(Err(()));
    fixture::<P>(|host, scope| {
        let node = compile(P::PROGRAM, &host).unwrap();
        assert!(host.invoked.borrow().is_empty());
        let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(budget), STACK);
        for index in 0..2 {
            let Ok(EffectSessionPoll::Request(dispatch)) = vm.poll(&host) else {
                assert_eq!(
                    vm.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                );
                assert!(!vm.lease().is_open());
                let remaining = vm.fuel_remaining();
                assert!(matches!(
                    vm.poll(&host).unwrap(),
                    EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
                ));
                assert_eq!(vm.fuel_remaining(), remaining);
                assert_eq!(host.invoked.borrow().len(), index);
                return;
            };
            let remaining = vm.fuel_remaining();
            assert!(matches!(
                vm.poll(&host).unwrap(),
                EffectSessionPoll::Awaiting
            ));
            assert_eq!(vm.fuel_remaining(), remaining);
            let identity = dispatch.identity.clone();
            let reply = host.dispatch(dispatch).unwrap();
            assert_eq!(*host.invoked.borrow(), P::arguments()[..=index]);
            vm.try_accept(&identity, reply, &host).unwrap();
            assert_eq!(vm.fuel_remaining(), remaining);
        }
        match vm.poll(&host) {
            Ok(EffectSessionPoll::Value(PureValue::Scalar(value))) => {
                assert_eq!(value, P::expected());
                outcome.set(Ok(vm.fuel_remaining()));
                assert!(matches!(
                    vm.poll(&host).unwrap(),
                    EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
                ));
            }
            Err(_) => {
                assert_eq!(
                    vm.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                );
                let remaining = vm.fuel_remaining();
                assert!(matches!(
                    vm.poll(&host).unwrap(),
                    EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
                ));
                assert_eq!(vm.fuel_remaining(), remaining);
            }
            _ => panic!("unexpected pipeline result"),
        }
        assert_eq!(*host.invoked.borrow(), P::arguments());
        assert!(!vm.lease().is_open());
    });
    outcome.get()
}

#[test]
fn both_hosts_run_continuous_control_helpers_and_two_replies_through_native_vm() {
    assert!(complete::<Counter>(10_000).is_ok());
    assert!(complete::<Panel>(10_000).is_ok());
}

#[test]
fn pipeline_matrix_covers_atomic_control_nodes_without_claiming_group_or_opaque_host_coverage() {
    fn covered<P: Profile>() -> std::collections::BTreeSet<&'static str> {
        let mut covered = std::collections::BTreeSet::new();
        fixture::<P>(|host, _scope| {
            let root = compile(P::PROGRAM, &host).unwrap();
            let mut pending = vec![&root];
            while let Some(node) = pending.pop() {
                covered.insert(match node {
                    Node::Literal { .. } => "literal",
                    Node::Strings { .. } => "strings",
                    Node::Local { .. } => "local",
                    Node::Field { .. } => "field",
                    Node::Binary { .. } => "binary",
                    Node::Unary { .. } => "unary",
                    Node::Bind { .. } => "bind",
                    Node::Loop { .. } => "loop",
                    Node::Fold { .. } => "fold",
                    Node::Choose { .. } => "choose",
                    Node::Recover { .. } => "recover",
                    Node::Call { .. } => "call",
                    Node::Group { .. } | Node::Member { .. } | Node::Host { .. } => {
                        panic!("new pipeline surface requires its own acceptance cases")
                    }
                });
                pending.extend(node.children());
            }
        });
        covered
    }
    let mut nodes = covered::<Counter>();
    nodes.extend(covered::<Panel>());
    assert_eq!(
        nodes,
        [
            "literal", "strings", "local", "field", "binary", "unary", "bind", "loop", "fold",
            "choose", "recover", "call"
        ]
        .into_iter()
        .collect()
    );
}

fn rejected_reply<P: Profile>() {
    fixture::<P>(|host, scope| {
        let node = compile(P::PROGRAM, &host).unwrap();
        let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(10_000), STACK);
        let EffectSessionPoll::Request(dispatch) = vm.poll(&host).unwrap() else {
            panic!()
        };
        let identity = dispatch.identity.clone();
        let good = host.dispatch(dispatch).unwrap();
        let bad = Reply {
            value: P::invalid_value(),
            buffer: Box::new(99),
        };
        let address = &*bad.buffer as *const u8;
        let remaining = vm.fuel_remaining();
        let rejected = vm.try_accept(&identity, bad, &host).unwrap_err();
        let bad = rejected.reply;
        assert_eq!(&*bad.buffer as *const u8, address);
        assert_eq!(vm.fuel_remaining(), remaining);
        assert!(matches!(
            vm.poll(&host).unwrap(),
            EffectSessionPoll::Awaiting
        ));
        assert_eq!(host.invoked.borrow().len(), 1);
        vm.try_accept(&identity, good, &host).unwrap();
        let EffectSessionPoll::Request(dispatch) = vm.poll(&host).unwrap() else {
            panic!()
        };
        assert_eq!(dispatch.identity.serial, 2);
        assert_eq!(host.invoked.borrow().len(), 1);
        vm.cancel();
        assert!(host.dispatch(dispatch).is_err());
    });
}
#[test]
fn both_hosts_reject_invalid_owned_receipts_without_replay_or_losing_pending_work() {
    rejected_reply::<Counter>();
    rejected_reply::<Panel>();
}

fn revocation<P: Profile>() {
    for phase in 0..3 {
        fixture::<P>(|host, scope| {
            let node = compile(P::PROGRAM, &host).unwrap();
            let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(10_000), STACK);
            let EffectSessionPoll::Request(dispatch) = vm.poll(&host).unwrap() else {
                panic!()
            };
            if phase == 0 {
                host.granted.set(false);
                assert!(host.dispatch(dispatch).is_err());
                assert!(host.invoked.borrow().is_empty());
                vm.cancel();
            } else {
                let identity = dispatch.identity.clone();
                let reply = host.dispatch(dispatch).unwrap();
                if phase == 1 {
                    host.revision.set(2);
                    assert!(vm.try_accept(&identity, reply, &host).is_err());
                    assert!(matches!(
                        vm.poll(&host).unwrap(),
                        EffectSessionPoll::Awaiting
                    ));
                    vm.cancel();
                } else {
                    vm.try_accept(&identity, reply, &host).unwrap();
                    host.version.set(32);
                    assert!(vm.poll(&host).is_err());
                    assert_eq!(
                        vm.status(),
                        EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                    );
                }
                assert_eq!(host.invoked.borrow().len(), 1);
            }
            assert!(!vm.lease().is_open());
        });
    }
}
#[test]
fn both_hosts_recheck_authority_at_dispatch_acceptance_and_captured_reentry() {
    revocation::<Counter>();
    revocation::<Panel>();
}

fn cancellation<P: Profile>() {
    for phase in 0..4 {
        fixture::<P>(|host, scope| {
            let node = compile(P::PROGRAM, &host).unwrap();
            let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(10_000), STACK);
            let mut escaped = None;
            if phase > 0 {
                let EffectSessionPoll::Request(dispatch) = vm.poll(&host).unwrap() else {
                    panic!()
                };
                if phase == 1 {
                    escaped = Some(dispatch);
                } else {
                    let identity = dispatch.identity.clone();
                    let reply = host.dispatch(dispatch).unwrap();
                    vm.try_accept(&identity, reply, &host).unwrap();
                    if phase == 3 {
                        let EffectSessionPoll::Request(dispatch) = vm.poll(&host).unwrap() else {
                            panic!()
                        };
                        escaped = Some(dispatch);
                    }
                }
            }
            let remaining = vm.fuel_remaining();
            assert!(vm.cancel());
            assert!(!vm.cancel());
            if let Some(dispatch) = escaped {
                assert!(host.dispatch(dispatch).is_err());
            }
            assert!(matches!(
                vm.poll(&host).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
            ));
            assert_eq!(vm.fuel_remaining(), remaining);
            assert_eq!(host.invoked.borrow().len(), usize::from(phase >= 2));
        });
    }
}
#[test]
fn both_hosts_cancel_cold_pending_accepted_and_successor_work_without_rollback_claims() {
    cancellation::<Counter>();
    cancellation::<Panel>();
}

fn cold_refusal<P: Profile>() {
    fixture::<P>(|host, _scope| {
        for source in [
            P::PROGRAM.replace(P::OPERATION, "foreign.unknown"),
            P::PROGRAM.replace(&format!("{}:", P::PARAMETER), "wrong:"),
            format!("fn main() = {}({}: false)", P::OPERATION, P::PARAMETER),
            format!(
                "fn main() = choose(when: true, then: {}({}: false), otherwise: foreign.unknown())",
                P::OPERATION,
                P::PARAMETER
            ),
        ] {
            assert!(compile(&source, &host).is_err());
        }
        host.version.set(32);
        assert!(compile(P::PROGRAM, &host).is_err());
        host.version.set(31);
        host.granted.set(false);
        assert!(compile(P::PROGRAM, &host).is_err());
        assert!(host.invoked.borrow().is_empty());
        assert_eq!(host.serial.get(), 0);
    });
}
#[test]
fn both_hosts_fail_closed_before_dispatch_for_unknown_operations_signatures_and_policy() {
    cold_refusal::<Counter>();
    cold_refusal::<Panel>();
}

fn fuel_boundaries<P: Profile>() {
    let used = 10_000 - complete::<P>(10_000).unwrap();
    for budget in 0..used {
        assert!(complete::<P>(budget).is_err(), "budget {budget} < {used}");
    }
    assert_eq!(complete::<P>(used), Ok(0));
    assert_eq!(complete::<P>(used + 1), Ok(1));
}
#[test]
fn both_hosts_preserve_one_budget_at_every_pipeline_debit_without_hidden_refills() {
    fuel_boundaries::<Counter>();
    fuel_boundaries::<Panel>();
}

fn isolated<P: Profile>() {
    fixture::<P>(|left, left_scope| {
        let node = compile(P::PROGRAM, &left).unwrap();
        fixture::<P>(|right, right_scope| {
            let mut a = LocalVm::new(left_scope, &node, Vec::new(), Fuel::new(10_000), STACK);
            let mut b = LocalVm::new(right_scope, &node, Vec::new(), Fuel::new(10_000), STACK);
            let EffectSessionPoll::Request(a_dispatch) = a.poll(&left).unwrap() else {
                panic!()
            };
            let EffectSessionPoll::Request(b_dispatch) = b.poll(&right).unwrap() else {
                panic!()
            };
            assert_eq!(a_dispatch.identity.serial, b_dispatch.identity.serial);
            let b_id = b_dispatch.identity.clone();
            let reply = right.dispatch(b_dispatch).unwrap();
            let address = &*reply.buffer as *const u8;
            let a_fuel = a.fuel_remaining();
            let b_fuel = b.fuel_remaining();
            let rejected = a.try_accept(&b_id, reply, &left).unwrap_err();
            assert_eq!(&*rejected.reply.buffer as *const u8, address);
            assert_eq!(a.fuel_remaining(), a_fuel);
            assert_eq!(b.fuel_remaining(), b_fuel);
            b.try_accept(&b_id, rejected.reply, &right).unwrap();
            assert!(a.cancel());
            assert!(left.dispatch(a_dispatch).is_err());
            assert!(b.lease().is_open());
            let EffectSessionPoll::Request(dispatch) = b.poll(&right).unwrap() else {
                panic!()
            };
            let identity = dispatch.identity.clone();
            b.try_accept(&identity, right.dispatch(dispatch).unwrap(), &right)
                .unwrap();
            let EffectSessionPoll::Value(PureValue::Scalar(result)) = b.poll(&right).unwrap()
            else {
                panic!()
            };
            assert_eq!(result, P::expected());
            assert!(left.invoked.borrow().is_empty());
            assert_eq!(*right.invoked.borrow(), P::arguments());
        });
    });
}
#[test]
fn both_hosts_share_one_compiled_ir_without_cross_acceptance_or_sibling_cancellation() {
    isolated::<Counter>();
    isolated::<Panel>();
}

fn stale_reply<P: Profile>() {
    fixture::<P>(|host, scope| {
        let node = compile(P::PROGRAM, &host).unwrap();
        let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(10_000), STACK);
        let EffectSessionPoll::Request(first) = vm.poll(&host).unwrap() else {
            panic!()
        };
        let old_id = first.identity.clone();
        let reply = host.dispatch(first).unwrap();
        let stale = Reply {
            value: reply.value.clone(),
            buffer: Box::new(89),
        };
        let address = &*stale.buffer as *const u8;
        vm.try_accept(&old_id, reply, &host).unwrap();
        let EffectSessionPoll::Request(second) = vm.poll(&host).unwrap() else {
            panic!()
        };
        let remaining = vm.fuel_remaining();
        let rejected = vm.try_accept(&old_id, stale, &host).unwrap_err();
        assert_eq!(&*rejected.reply.buffer as *const u8, address);
        assert_eq!(vm.fuel_remaining(), remaining);
        assert!(vm.lease().is_open());
        assert_eq!(host.invoked.borrow().len(), 1);
        let identity = second.identity.clone();
        vm.try_accept(&identity, host.dispatch(second).unwrap(), &host)
            .unwrap();
        let EffectSessionPoll::Value(PureValue::Scalar(value)) = vm.poll(&host).unwrap() else {
            panic!()
        };
        assert_eq!(value, P::expected());
        assert_eq!(*host.invoked.borrow(), P::arguments());
    });
}
#[test]
fn both_hosts_reject_a_settled_predecessor_receipt_without_consuming_the_successor() {
    stale_reply::<Counter>();
    stale_reply::<Panel>();
}

mod mixed {
    use super::*;
    use call_evaluation::CallEvaluationHost;
    use catalog_group_source::{
        CatalogGroupProgramLimits, CatalogGroupSourceEnvironment,
        compile_catalog_prepared_group_helper_program,
    };
    use flow_typing::{GroupFlowEnvironment, GroupMemberType};
    use group_reply::{GroupReply, assemble_group_reply};
    use ir::{ComputedBranch, GroupKind};
    use native_group_lookup::NativeGroupLookupError;
    use native_group_preparation::PreparedNativeGroup;
    use parallel_evaluation::{ParallelEvaluationLimits, ParallelPoll, ParallelRequest};
    use prepared_call_evaluation::{PreparedGroupEvaluationLimits, prepare_atomic_group_in_scope};
    use prepared_parallel::PreparedParallelSession;
    use prepared_sequence::PreparedSequenceSession;
    use sequence_evaluation::{PreparedSequenceMember, SequenceAccepted, SequencePoll};
    use std::rc::Rc;

    #[derive(Clone, PartialEq)]
    enum Meta {
        Atom,
        Group(GroupKind, Vec<(String, String)>),
    }
    struct Types<'a, 'h, 's, P>(&'a Host<'h, 's, P>);
    impl<P: Profile> Types<'_, '_, '_, P> {
        fn atom(&self) -> PureType<Meta> {
            match P::result_type() {
                PureType::Scalar(ty) => PureType::Scalar(ty),
                PureType::Result(()) => PureType::Result(Meta::Atom),
            }
        }
    }
    impl<P: Profile> PureTypeEnvironment<String, String> for Types<'_, '_, '_, P> {
        type Result = Meta;
        fn field_type(&self, value: &Meta, field: &String) -> Option<ScalarType> {
            if *value != Meta::Atom {
                return None;
            }
            match P::result_type() {
                PureType::Scalar(ty) => (field == "value").then_some(ty),
                PureType::Result(()) => P::field_type(field),
            }
        }
        fn member_result(&self, value: &Meta, name: &str, operation: &String) -> Option<Meta> {
            let Meta::Group(_, rows) = value else {
                return None;
            };
            rows.iter()
                .any(|(n, op)| n == name && op == operation)
                .then_some(Meta::Atom)
        }
    }
    impl<'s, P: Profile> CallFlowEnvironment<'s, String, String, Domain<P>> for Types<'_, '_, 's, P> {
        fn call_result_type(
            &self,
            key: &String,
            declaration: &'s Domain<P>,
        ) -> Option<PureType<Meta>> {
            let source = self.0.source();
            let schema = source
                .catalog
                .authorize(key, source.version, source.granted)
                .ok()?;
            std::ptr::eq(&schema.result, declaration).then(|| self.atom())
        }
    }
    impl<'source, 's, P: Profile> CatalogSourceEnvironment<'source, 's, String, String, Domain<P>>
        for Types<'_, '_, 's, P>
    {
        type Error = Refusal;
        fn source_operation(&self, original: &String) -> Result<String, Refusal> {
            Ok(original.clone())
        }
        fn source_field(
            &self,
            result: &Meta,
            name: &'source str,
        ) -> Result<Option<(String, ScalarType)>, Refusal> {
            Ok(self
                .field_type(result, &name.into())
                .map(|ty| (name.into(), ty)))
        }
        fn source_member(
            &self,
            result: &Meta,
            _: &'source str,
            name: &'source str,
        ) -> Result<Option<(String, Meta)>, Refusal> {
            let Meta::Group(_, rows) = result else {
                return Ok(None);
            };
            Ok(rows
                .iter()
                .find(|(n, _)| n == name)
                .map(|(_, op)| (op.clone(), Meta::Atom)))
        }
    }
    impl<'e, 's, P: Profile> GroupFlowEnvironment<'e, 's, String, String, Domain<P>, ()>
        for Types<'_, '_, 's, P>
    {
        fn group_branch_type_matches(&self, _: &(), inferred: &PureType<Meta>) -> bool {
            *inferred == self.atom()
        }
        fn group_result_type(
            &self,
            kind: GroupKind,
            members: &[GroupMemberType<'e, String, Meta>],
        ) -> Option<Meta> {
            let mut rows = Vec::new();
            for row in members {
                if row.operation != P::OPERATION || row.inferred != self.atom() {
                    return None;
                }
                rows.push((row.name.into(), row.operation.clone()));
            }
            Some(Meta::Group(kind, rows))
        }
    }
    impl<'source, 's, P: Profile>
        CatalogGroupSourceEnvironment<'source, 's, String, String, Domain<P>, ()>
        for Types<'_, '_, 's, P>
    {
        fn source_group_branch(
            &self,
            _: &'source leselang_vm::native::syntax::NamedArgument,
            _: &String,
            _: &'s Domain<P>,
            _: &PureType<Meta>,
        ) -> Result<(), Refusal> {
            Ok(())
        }
    }
    fn compile<P: Profile>(source: &str, host: &Host<'_, '_, P>) -> Result<Node, String> {
        let tree = parse(source);
        if !tree.diagnostics.is_empty() {
            return Err(format!("{:?}", tree.diagnostics));
        }
        let functions: Vec<_> = tree.function.iter().chain(&tree.helpers).collect();
        compile_catalog_prepared_group_helper_program(
            &functions,
            "main",
            CatalogGroupProgramLimits {
                program: PROGRAM,
                max_groups: 4,
                max_branches: 8,
            },
            &host.source(),
            &Types(host),
        )
        .map(|(node, _)| node)
        .map_err(compile_diagnostic)
    }

    fn compile_diagnostic(error: catalog_source::CatalogSourceError<Refusal>) -> String {
        match error {
            catalog_source::CatalogSourceError::Group(error) => group_diagnostic(*error),
            catalog_source::CatalogSourceError::Control(error) => match *error {
                control_source::ControlSourceError::Native { span, phase, error } => {
                    format!("{span:?}/{phase:?}: {}", compile_diagnostic(error))
                }
                error => error.to_string(),
            },
            error => error.to_string(),
        }
    }

    fn group_diagnostic(error: catalog_group_source::CatalogGroupSourceError<Refusal>) -> String {
        match error {
            catalog_group_source::CatalogGroupSourceError::Source(error) => {
                compile_diagnostic(*error)
            }
            catalog_group_source::CatalogGroupSourceError::Control(error) => match *error {
                control_source::ControlSourceError::Native { span, phase, error } => {
                    format!("{span:?}/{phase:?}: {}", group_diagnostic(error))
                }
                error => error.to_string(),
            },
            catalog_group_source::CatalogGroupSourceError::Type(error) => {
                format!("group type: {error:?}")
            }
            error => error.to_string(),
        }
    }

    type Branch = ComputedBranch<Node, ()>;
    type Accepted<'ir, P> =
        SequenceAccepted<'ir, Node, (), Identity, Domain<P>, Reply<<P as Profile>::Value>>;
    type Assembled<'ir, P> =
        GroupReply<'ir, Node, (), Identity, Domain<P>, Reply<<P as Profile>::Value>>;
    type Sequence<'ir, 's, P> =
        PreparedSequenceSession<'ir, Node, (), Identity, Domain<P>, Dispatch<'s, P>>;
    type Parallel<'ir, P> =
        PreparedParallelSession<'ir, Node, (), Identity, Domain<P>, Reply<<P as Profile>::Value>>;
    const BATCH: ParallelEvaluationLimits = ParallelEvaluationLimits {
        max_nodes: 1024,
        max_depth: 64,
        max_branches: 8,
    };

    enum View<'ir, P: Profile> {
        Atom(Identity, P::Value),
        Group(Identity, Rc<Assembled<'ir, P>>),
    }
    impl<P: Profile> Clone for View<'_, P> {
        fn clone(&self) -> Self {
            match self {
                Self::Atom(id, value) => Self::Atom(id.clone(), value.clone()),
                Self::Group(id, rows) => Self::Group(id.clone(), rows.clone()),
            }
        }
    }
    enum Response<'ir, P: Profile> {
        Atom(Reply<P::Value>),
        Group(Vec<Accepted<'ir, P>>),
    }
    struct Receipt<'ir, P: Profile> {
        node: &'ir Node,
        identity: Identity,
        response: Response<'ir, P>,
        buffer: Box<u8>,
    }
    // Stable fixture domains tie each root and all its member receipts to one IR site.
    struct Plan<'ir, P> {
        node: &'ir Node,
        domain: &'ir Domain<P>,
        identity: RefCell<Option<Identity>>,
        members: RefCell<Vec<Identity>>,
    }
    impl<'ir, P: Profile> HostResultDomain<Receipt<'ir, P>> for Plan<'ir, P> {
        type Error = Refusal;
        fn matches_type(&self, reply: &Receipt<'ir, P>) -> bool {
            std::ptr::eq(self.node, reply.node)
                && matches!(
                    (self.node, &reply.response),
                    (Node::Call { .. }, Response::Atom(_))
                        | (Node::Group { .. }, Response::Group(_))
                )
        }
        fn validate_value(&self, reply: &Receipt<'ir, P>) -> Result<(), Refusal> {
            if !self.matches_type(reply) || self.identity.borrow().as_ref() != Some(&reply.identity)
            {
                return Err(Refusal);
            }
            match &reply.response {
                Response::Atom(input) => {
                    if !self.domain.matches_type(&input.value) {
                        return Err(Refusal);
                    }
                    self.domain.validate_value(&input.value)
                }
                Response::Group(rows) => self.validate_rows(rows),
            }
        }
    }
    impl<'ir, P: Profile> Plan<'ir, P> {
        fn validate_rows(&self, rows: &[Accepted<'ir, P>]) -> Result<(), Refusal> {
            let Node::Group { branches, .. } = self.node else {
                return Err(Refusal);
            };
            let members = self.members.borrow();
            if rows.len() != branches.len() || members.len() != branches.len() {
                return Err(Refusal);
            }
            for (i, row) in rows.iter().enumerate() {
                let (index, original, identity, domain, input) = row.as_parts();
                if index != i
                    || !std::ptr::eq(original, &branches[i])
                    || *identity != members[i]
                    || !std::ptr::eq(domain, self.domain)
                    || !domain.matches_type(&input.value)
                {
                    return Err(Refusal);
                }
                domain.validate_value(&input.value)?;
            }
            Ok(())
        }
        fn id(&self) -> Identity {
            self.identity.borrow().as_ref().unwrap().clone()
        }
    }
    fn plans<'ir, P: Profile>(node: &'ir Node, host: &Host<'_, 'ir, P>) -> Vec<Plan<'ir, P>> {
        let source = host.source();
        let domain = &source
            .catalog
            .authorize(P::OPERATION, source.version, source.granted)
            .unwrap()
            .result;
        let mut plans = Vec::new();
        let mut pending = vec![node];
        let mut seen = 0;
        while let Some(node) = pending.pop() {
            seen += 1;
            assert!(seen <= PURE.max_nodes);
            if matches!(node, Node::Call { .. } | Node::Group { .. }) {
                plans.push(Plan {
                    node,
                    domain,
                    identity: RefCell::new(None),
                    members: RefCell::new(Vec::new()),
                });
            }
            pending.extend(node.children());
        }
        plans
    }
    enum Preparation<'ir, 's, P> {
        Atom(Request<'s, P>),
        Group(Box<PreparedNativeGroup<'ir, Branch, Request<'s, P>>>),
    }
    struct Prepared<'ir, 's, P> {
        node: &'ir Node,
        preparation: Preparation<'ir, 's, P>,
    }
    enum Work<'ir, 's, P: Profile> {
        Atom(Dispatch<'s, P>),
        Sequence(Box<Sequence<'ir, 's, P>>),
        Parallel {
            barrier: Box<Parallel<'ir, P>>,
            requests: Vec<ParallelRequest<Dispatch<'s, P>>>,
        },
    }
    struct Active<'r, 'ir, 's, P: Profile> {
        plan: &'r Plan<'ir, P>,
        work: Work<'ir, 's, P>,
    }
    struct Runtime<'r, 'ir, 'h, 's, P> {
        host: &'r Host<'h, 's, P>,
        plans: &'r [Plan<'ir, P>],
    }
    impl<'r, 'ir, 's, P: Profile> Runtime<'r, 'ir, '_, 's, P> {
        fn plan(&self, node: &Node) -> Result<&'r Plan<'ir, P>, Refusal> {
            self.plans
                .iter()
                .find(|p| std::ptr::eq(p.node, node))
                .ok_or(Refusal)
        }
        fn identity(&self) -> Result<Identity, Refusal> {
            self.host
                .serial
                .set(self.host.serial.get().checked_add(1).ok_or(Refusal)?);
            let identity = Identity {
                owner: self.host.lease.clone(),
                serial: self.host.serial.get(),
                revision: self.host.revision.get(),
            };
            self.host.authorize(&identity)?;
            Ok(identity)
        }
        fn admit(&self, request: &Request<'s, P>) -> Result<(), Refusal> {
            let source = self.host.source();
            let schema = source
                .catalog
                .authorize(&request.schema().key, source.version, source.granted)
                .map_err(|_| Refusal)?;
            if !std::ptr::eq(schema, request.schema()) || !self.host.lease.is_open() {
                return Err(Refusal);
            }
            Ok(())
        }
        fn correlate_member(
            &self,
            plan: &Plan<'ir, P>,
            index: usize,
            request: Request<'s, P>,
        ) -> Result<PreparedSequenceMember<'ir, Identity, Domain<P>, Dispatch<'s, P>>, Refusal>
        {
            self.admit(&request)?;
            if !std::ptr::eq(plan.domain, &request.schema().result)
                || plan.members.borrow().len() != index
            {
                return Err(Refusal);
            }
            let identity = self.identity()?;
            plan.members.borrow_mut().push(identity.clone());
            Ok(PreparedSequenceMember {
                identity: identity.clone(),
                declaration: plan.domain,
                request: Dispatch { identity, request },
            })
        }
        fn publish(
            &self,
            sequence: &mut Sequence<'ir, 's, P>,
        ) -> Result<SequencePoll<Dispatch<'s, P>>, Refusal> {
            sequence
                .poll(|_, _, identity, domain, request| {
                    self.host.authorize(identity)?;
                    if *identity != request.identity
                        || !std::ptr::eq(domain, &request.request.schema().result)
                    {
                        return Err(Refusal);
                    }
                    self.admit(&request.request)
                })
                .map_err(|_| Refusal)
        }
        fn collect(&self, active: Active<'_, 'ir, 's, P>) -> Result<Receipt<'ir, P>, Refusal> {
            self.authorize(&active.plan.id())?;
            let response = match active.work {
                Work::Atom(request) => Response::Atom(self.host.dispatch(request)?),
                Work::Sequence(mut sequence) => {
                    let Node::Group { branches, .. } = active.plan.node else {
                        return Err(Refusal);
                    };
                    let mut rows = Vec::new();
                    for expected in 0..branches.len() {
                        let SequencePoll::Request { index, request } =
                            self.publish(&mut sequence)?
                        else {
                            return Err(Refusal);
                        };
                        assert_eq!(index, expected);
                        assert!(matches!(
                            self.publish(&mut sequence)?,
                            SequencePoll::Awaiting { .. }
                        ));
                        let identity = request.identity.clone();
                        rows.push(
                            sequence
                                .try_accept(&identity, self.host.dispatch(request)?, self.host)
                                .map_err(|_| Refusal)?,
                        );
                    }
                    Response::Group(rows)
                }
                Work::Parallel {
                    mut barrier,
                    requests,
                } => {
                    let mut replies = Vec::new();
                    for request in requests {
                        let identity = request.request.identity.clone();
                        replies.push((
                            request.index,
                            identity,
                            self.host.dispatch(request.request)?,
                        ));
                    }
                    for (index, identity, reply) in replies.into_iter().rev() {
                        barrier
                            .try_accept(index, &identity, reply, self.host)
                            .map_err(|_| Refusal)?;
                        if index > 0 {
                            assert!(
                                matches!(barrier.poll().unwrap(), ParallelPoll::Awaiting { remaining } if remaining == index)
                            );
                        }
                    }
                    let ParallelPoll::Results(rows) = barrier.poll().map_err(|_| Refusal)? else {
                        return Err(Refusal);
                    };
                    Response::Group(rows)
                }
            };
            Ok(Receipt {
                node: active.plan.node,
                identity: active.plan.id(),
                response,
                buffer: Box::new(57),
            })
        }
    }
    impl<'ir, P: Profile> PureEvaluationEnvironment<String, String> for Runtime<'_, 'ir, '_, '_, P> {
        type Result = View<'ir, P>;
        type Error = Refusal;
        fn field(&self, value: &Self::Result, field: &String) -> Result<ScalarValue, Refusal> {
            let View::Atom(identity, value) = value else {
                return Err(Refusal);
            };
            self.host.authorize(identity)?;
            P::validate(value)?;
            match P::bind(value.clone()) {
                PureValue::Scalar(value) if field == "value" => Ok(value),
                PureValue::Result(value) => P::field(&value, field),
                _ => Err(Refusal),
            }
        }
        fn member(
            &self,
            value: &Self::Result,
            name: &str,
            operation: &String,
        ) -> Result<Self::Result, Refusal> {
            let View::Group(identity, group) = value else {
                return Err(Refusal);
            };
            self.host.authorize(identity)?;
            if operation != P::OPERATION {
                return Err(Refusal);
            }
            let row = group
                .lookup(BATCH.max_branches, name, |row| {
                    let (_, _, identity, domain, reply) = row.as_parts();
                    self.host.authorize(identity)?;
                    domain.validate_value(&reply.value)?;
                    Ok(true)
                })
                .map_err(|_: NativeGroupLookupError<Refusal>| Refusal)?
                .ok_or(Refusal)?;
            let (_, _, identity, _, reply) = row.as_parts();
            Ok(View::Atom(identity.clone(), reply.value.clone()))
        }
    }
    impl<'ir, 's, P: Profile> EffectEvaluationEnvironment<'ir, String, String, (), ()>
        for Runtime<'_, 'ir, '_, 's, P>
    {
        type Request = Prepared<'ir, 's, P>;
        type Capture = RestoredEffectBindings<'ir, View<'ir, P>>;
        fn preflight_effect(&self, node: &'ir Node) -> Result<(), Refusal> {
            self.plan(node)?;
            match node {
                Node::Call { .. } => self.host.preflight_effect(node),
                Node::Group { .. } => Ok(()),
                _ => Err(Refusal),
            }
        }
        fn prepare_effect(
            &self,
            node: &'ir Node,
            scope: &mut ScopeFrame<'_, 'ir, PureValue<Self::Result>>,
            fuel: &mut Fuel,
        ) -> Result<Self::Request, CalculationFailure<Refusal>> {
            self.plan(node)?;
            let source = self.host.source();
            let limits = CallEvaluationLimits {
                pure: PURE,
                max_arguments: 8,
            };
            let preparation = match node {
                Node::Call {
                    operation,
                    arguments,
                } => {
                    let schema = source
                        .catalog
                        .authorize(operation, source.version, source.granted)
                        .map_err(|_| Refusal)?;
                    Preparation::Atom(
                        evaluate_call_arguments_in_scope(
                            arguments, schema, scope, self, fuel, limits,
                        )
                        .map_err(|_| Refusal)?,
                    )
                }
                Node::Group { .. } => Preparation::Group(Box::new(
                    prepare_atomic_group_in_scope(
                        node,
                        scope,
                        &CallEvaluationHost {
                            catalog: source.catalog,
                            version: source.version,
                            granted: source.granted,
                            environment: self,
                        },
                        fuel,
                        PreparedGroupEvaluationLimits {
                            call: limits,
                            max_branches: 8,
                        },
                        |_, _, schema| {
                            if !std::ptr::eq(&schema.result, self.plan(node)?.domain) {
                                return Err(Refusal);
                            }
                            Ok(())
                        },
                    )
                    .map_err(|_| Refusal)?,
                )),
                _ => return Err(Refusal.into()),
            };
            Ok(Prepared { node, preparation })
        }
        fn capture(
            &self,
            _: &'ir str,
            _: &'ir Node,
            scope: &ScopeFrame<'_, 'ir, PureValue<Self::Result>>,
            fuel: &mut Fuel,
        ) -> Result<Self::Capture, Refusal> {
            let mut saved = Vec::new();
            for (name, value) in scope.bindings() {
                let extra = match value {
                    PureValue::Scalar(s) => scalar_copy_cost(s),
                    _ => 0,
                };
                fuel.charge(1 + extra).map_err(|_| Refusal)?;
                saved.push((*name, value.clone()));
            }
            Ok(saved)
        }
    }
    impl<P: Profile> ReplyAuthority<Identity> for Runtime<'_, '_, '_, '_, P> {
        type Error = Refusal;
        fn authorize(&self, identity: &Identity) -> Result<(), Refusal> {
            self.host.authorize(identity)
        }
    }
    impl<'ir, P: Profile>
        EffectReentryEnvironment<
            'ir,
            String,
            String,
            (),
            (),
            Identity,
            Plan<'ir, P>,
            Receipt<'ir, P>,
        > for Runtime<'_, 'ir, '_, '_, P>
    {
        fn restore_capture(
            &self,
            identity: &Identity,
            _: &Plan<'ir, P>,
            capture: Self::Capture,
            _: &mut Fuel,
            _: EffectEvaluationLimits,
        ) -> Result<Self::Capture, CalculationFailure<Refusal>> {
            self.authorize(identity)?;
            Ok(capture)
        }
        fn bind_reply(
            &self,
            identity: &Identity,
            plan: &Plan<'ir, P>,
            reply: Receipt<'ir, P>,
            fuel: &mut Fuel,
        ) -> Result<PureValue<Self::Result>, CalculationFailure<Refusal>> {
            self.authorize(identity)?;
            plan.validate_value(&reply)?;
            match reply.response {
                Response::Atom(reply) => Ok(match P::bind(reply.value) {
                    PureValue::Scalar(v) => PureValue::Scalar(v),
                    PureValue::Result(v) => PureValue::Result(View::Atom(identity.clone(), v)),
                }),
                Response::Group(rows) => {
                    let group = assemble_group_reply(plan.node, rows, BATCH, fuel, |whole, _| {
                        self.authorize(identity)?;
                        plan.validate_rows(whole.members())
                    })
                    .map_err(|_| Refusal)?;
                    Ok(PureValue::Result(View::Group(
                        identity.clone(),
                        Rc::new(group),
                    )))
                }
            }
        }
    }
    impl<'r, 'ir: 'r, 's, P: Profile> EffectSessionEnvironment<'ir, 'r, String, String, (), ()>
        for Runtime<'r, 'ir, '_, 's, P>
    {
        type Identity = Identity;
        type Declaration = Plan<'ir, P>;
        type Reply = Receipt<'ir, P>;
        type Dispatch = Active<'r, 'ir, 's, P>;
        fn correlate_request(
            &self,
            request: Self::Request,
            fuel: &mut Fuel,
        ) -> EffectCorrelation<'r, Identity, Plan<'ir, P>, Self::Dispatch, Refusal> {
            let plan = self.plan(request.node)?;
            if plan.identity.borrow().is_some() {
                return Err(Refusal.into());
            }
            let identity = self.identity()?;
            plan.identity.replace(Some(identity.clone()));
            let work = match request.preparation {
                Preparation::Atom(request) => {
                    self.admit(&request)?;
                    Work::Atom(Dispatch {
                        identity: identity.clone(),
                        request,
                    })
                }
                Preparation::Group(prepared) => {
                    let prepared = *prepared;
                    let Node::Group { group_kind, .. } = request.node else {
                        return Err(Refusal.into());
                    };
                    match group_kind {
                        GroupKind::Sequence => Work::Sequence(Box::new(
                            Sequence::start(
                                request.node,
                                prepared,
                                fuel,
                                BATCH,
                                |_, _, request| self.admit(request),
                                |i, _, request, _| self.correlate_member(plan, i, request),
                            )
                            .map_err(|_| Refusal)?,
                        )),
                        GroupKind::Parallel => {
                            let (barrier, requests) = Parallel::start(
                                request.node,
                                prepared,
                                fuel,
                                BATCH,
                                |_, _, request| self.admit(request),
                                |i, _, request, _| self.correlate_member(plan, i, request),
                            )
                            .map_err(|_| Refusal)?;
                            Work::Parallel {
                                barrier: Box::new(barrier),
                                requests,
                            }
                        }
                    }
                }
            };
            Ok(CorrelatedEffectRequest {
                identity,
                declaration: plan,
                dispatch: Active { plan, work },
            })
        }
    }

    mod saved_mixed {
        use super::*;
        use effect_stack::{
            SavedEffectReturnFrame, StackedEffectOutcome, evaluate_stacked_effects_in_scope,
            restore_effect_return_stack, resume_accepted_stacked_effects,
        };
        use parallel_evaluation::{SavedParallelMember, SavedParallelReply};
        use sequence_evaluation::{SavedSequenceMember, SavedSequenceReply, SequenceEnd};

        type Queued<'ir, P> = PreparedSequenceSession<'ir, Node, (), Identity, Domain<P>, usize>;
        type MemberReceipt<'ir, P> =
            AcceptedReply<'ir, Identity, (), Domain<P>, Reply<<P as Profile>::Value>>;

        #[derive(Clone, Copy, Default)]
        enum Fault {
            #[default]
            None,
            Stale,
            Revoked,
            Closed,
            QueuedHandle,
        }

        // This bounded host inventory survives controller replacement. Handles are
        // not requests or authority; dispatch consumes each original operand once.
        struct Inventory<'s, P> {
            requests: Vec<Option<Dispatch<'s, P>>>,
        }
        impl<'s, P: Profile> Inventory<'s, P> {
            fn correlate<'ir>(
                &mut self,
                runtime: &Runtime<'_, 'ir, '_, 's, P>,
                plan: &Plan<'ir, P>,
                index: usize,
                request: Request<'s, P>,
            ) -> Result<PreparedSequenceMember<'ir, Identity, Domain<P>, usize>, Refusal>
            {
                let row = runtime.correlate_member(plan, index, request)?;
                assert_eq!(index, self.requests.len());
                self.requests.push(Some(row.request));
                Ok(PreparedSequenceMember {
                    identity: row.identity,
                    declaration: row.declaration,
                    request: index,
                })
            }
            fn admit(
                &self,
                runtime: &Runtime<'_, '_, '_, 's, P>,
                index: usize,
                identity: &Identity,
                domain: &Domain<P>,
                handle: usize,
            ) -> Result<(), Refusal> {
                runtime.authorize(identity)?;
                let request = self
                    .requests
                    .get(handle)
                    .and_then(Option::as_ref)
                    .ok_or(Refusal)?;
                if handle != index
                    || request.identity != *identity
                    || !std::ptr::eq(domain, &request.request.schema().result)
                {
                    return Err(Refusal);
                }
                runtime.admit(&request.request)
            }
            fn publish(
                &self,
                runtime: &Runtime<'_, '_, '_, 's, P>,
                queue: &mut Queued<'_, P>,
            ) -> Result<SequencePoll<usize>, Refusal> {
                queue
                    .poll(|index, _, identity, domain, handle| {
                        self.admit(runtime, index, identity, domain, *handle)
                    })
                    .map_err(|_| Refusal)
            }
            fn invoke(
                &mut self,
                runtime: &Runtime<'_, '_, '_, 's, P>,
                handle: usize,
            ) -> Result<Reply<P::Value>, Refusal> {
                runtime.host.dispatch(
                    self.requests
                        .get_mut(handle)
                        .and_then(Option::take)
                        .ok_or(Refusal)?,
                )
            }
        }

        fn invalidate<P: Profile>(
            runtime: &Runtime<'_, '_, '_, '_, P>,
            scope: &LocalExecutionScope,
            fault: Fault,
        ) {
            match fault {
                Fault::Stale => runtime.host.revision.set(runtime.host.revision.get() + 1),
                Fault::Revoked => runtime.host.granted.set(false),
                Fault::Closed => {
                    scope.close();
                }
                Fault::None | Fault::QueuedHandle => (),
            }
        }

        fn accept<'ir, P: Profile>(
            runtime: &Runtime<'_, 'ir, '_, '_, P>,
            plan: &Plan<'ir, P>,
            index: usize,
            reply: Reply<P::Value>,
        ) -> Result<MemberReceipt<'ir, P>, Refusal> {
            let identity = plan.members.borrow()[index].clone();
            PendingReply::new(identity.clone(), (), plan.domain)
                .try_accept(&identity, reply, runtime.host)
                .map_err(|_| Refusal)
        }

        fn collect<'ir, 's, P: Profile>(
            runtime: &Runtime<'_, 'ir, '_, 's, P>,
            scope: &LocalExecutionScope,
            request: Prepared<'ir, 's, P>,
            fuel: &mut Fuel,
            cut: usize,
            fault: Fault,
        ) -> Result<Receipt<'ir, P>, Refusal> {
            let plan = runtime.plan(request.node)?;
            let Preparation::Group(prepared) = request.preparation else {
                return runtime.collect(
                    runtime
                        .correlate_request(request, fuel)
                        .map_err(|_| Refusal)?
                        .dispatch,
                );
            };
            assert!(plan.identity.borrow().is_none());
            plan.identity.replace(Some(runtime.identity()?));
            let Node::Group {
                group_kind,
                branches,
            } = plan.node
            else {
                return Err(Refusal);
            };
            assert_eq!(branches.len(), 2);
            let mut inventory = Inventory {
                requests: Vec::new(),
            };
            let rows = match group_kind {
                GroupKind::Sequence => {
                    assert!(cut <= 4);
                    let mut queue = Queued::start(
                        plan.node,
                        *prepared,
                        fuel,
                        BATCH,
                        |_, _, request| runtime.admit(request),
                        |i, _, request, _| inventory.correlate(runtime, plan, i, request),
                    )
                    .map_err(|_| Refusal)?;
                    let prefix = cut / 2;
                    let mut saved = Vec::new();
                    for (index, branch) in branches.iter().enumerate().take(prefix) {
                        let SequencePoll::Request {
                            index: actual,
                            request,
                        } = inventory.publish(runtime, &mut queue)?
                        else {
                            return Err(Refusal);
                        };
                        assert_eq!(actual, index);
                        let reply = inventory.invoke(runtime, request)?;
                        let identity = plan.members.borrow()[index].clone();
                        let row = queue
                            .try_accept(&identity, reply, runtime.host)
                            .map_err(|_| Refusal)?;
                        let (_, original, _, _, reply) = row.into_parts();
                        assert!(std::ptr::eq(original, branch));
                        saved.push(SavedSequenceMember {
                            index,
                            name: &branch.name,
                            reply: SavedSequenceReply::Accepted(accept(
                                runtime, plan, index, reply,
                            )?),
                        });
                    }
                    let mut flying = None;
                    for (index, branch) in branches.iter().enumerate().skip(prefix) {
                        let identity = plan.members.borrow()[index].clone();
                        let reply = if index == prefix && cut % 2 == 1 {
                            let SequencePoll::Request {
                                index: actual,
                                request,
                            } = inventory.publish(runtime, &mut queue)?
                            else {
                                return Err(Refusal);
                            };
                            assert_eq!(actual, index);
                            flying = Some(inventory.invoke(runtime, request)?);
                            SavedSequenceReply::Awaiting {
                                identity,
                                declaration: plan.domain,
                            }
                        } else {
                            SavedSequenceReply::Prepared(PreparedSequenceMember {
                                identity,
                                declaration: plan.domain,
                                request: if matches!(fault, Fault::QueuedHandle) {
                                    8
                                } else {
                                    index
                                },
                            })
                        };
                        saved.push(SavedSequenceMember {
                            index,
                            name: &branch.name,
                            reply,
                        });
                    }
                    drop(queue);
                    let remaining = fuel.remaining();
                    invalidate(runtime, scope, fault);
                    let (mut queue, mut rows) =
                        Queued::restore(plan.node, saved, BATCH, |snapshot| {
                            runtime.authorize(&plan.id())?;
                            for row in snapshot {
                                if *row.reply.identity() != plan.members.borrow()[row.index] {
                                    return Err(Refusal);
                                }
                                runtime.authorize(row.reply.identity())?;
                                match &row.reply {
                                    SavedSequenceReply::Prepared(p) => inventory.admit(
                                        runtime,
                                        row.index,
                                        &p.identity,
                                        p.declaration,
                                        p.request,
                                    )?,
                                    SavedSequenceReply::Awaiting { declaration, .. } => {
                                        if !std::ptr::eq(*declaration, plan.domain)
                                            || flying.is_none()
                                        {
                                            return Err(Refusal);
                                        }
                                    }
                                    SavedSequenceReply::Accepted(reply) => {
                                        let (_, (), domain, reply) = reply.as_parts();
                                        if !std::ptr::eq(domain, plan.domain) {
                                            return Err(Refusal);
                                        }
                                        domain.validate_value(&reply.value)?;
                                    }
                                }
                            }
                            Ok(())
                        })
                        .map_err(|_| Refusal)?;
                    assert_eq!(fuel.remaining(), remaining);
                    for index in prefix..branches.len() {
                        let reply = if let Some(reply) = flying.take() {
                            assert!(
                                matches!(inventory.publish(runtime, &mut queue)?, SequencePoll::Awaiting { index: i } if i == index)
                            );
                            reply
                        } else {
                            let SequencePoll::Request {
                                index: actual,
                                request,
                            } = inventory.publish(runtime, &mut queue)?
                            else {
                                return Err(Refusal);
                            };
                            assert_eq!(actual, index);
                            inventory.invoke(runtime, request)?
                        };
                        let identity = plan.members.borrow()[index].clone();
                        rows.push(
                            queue
                                .try_accept(&identity, reply, runtime.host)
                                .map_err(|_| Refusal)?,
                        );
                    }
                    assert!(matches!(
                        inventory.publish(runtime, &mut queue)?,
                        SequencePoll::Terminal(SequenceEnd::Completed)
                    ));
                    rows
                }
                GroupKind::Parallel => {
                    assert!(cut < 4);
                    let (mut barrier, requests) = Parallel::start(
                        plan.node,
                        *prepared,
                        fuel,
                        BATCH,
                        |_, _, request| runtime.admit(request),
                        |i, _, request, _| inventory.correlate(runtime, plan, i, request),
                    )
                    .map_err(|_| Refusal)?;
                    let mut replies = Vec::new();
                    for request in requests {
                        replies.push(Some(inventory.invoke(runtime, request.request)?));
                    }
                    let mut saved = Vec::new();
                    for (index, branch) in branches.iter().enumerate() {
                        let identity = plan.members.borrow()[index].clone();
                        let reply = if cut & (1 << index) != 0 {
                            let reply = replies[index].take().ok_or(Refusal)?;
                            // Fixture-native snapshot copy, not a core Clone/codec requirement.
                            let retained = Reply {
                                value: reply.value.clone(),
                                buffer: Box::new(*reply.buffer),
                            };
                            barrier
                                .try_accept(index, &identity, reply, runtime.host)
                                .map_err(|_| Refusal)?;
                            SavedParallelReply::Accepted(accept(runtime, plan, index, retained)?)
                        } else {
                            SavedParallelReply::Awaiting {
                                identity,
                                declaration: plan.domain,
                            }
                        };
                        saved.push(SavedParallelMember {
                            index,
                            name: &branch.name,
                            reply,
                        });
                    }
                    drop(barrier);
                    let remaining = fuel.remaining();
                    invalidate(runtime, scope, fault);
                    let mut barrier = Parallel::restore(plan.node, saved, BATCH, |snapshot| {
                        runtime.authorize(&plan.id())?;
                        for row in snapshot {
                            if *row.reply.identity() != plan.members.borrow()[row.index] {
                                return Err(Refusal);
                            }
                            runtime.authorize(row.reply.identity())?;
                            match &row.reply {
                                SavedParallelReply::Awaiting { declaration, .. } => {
                                    if !std::ptr::eq(*declaration, plan.domain)
                                        || replies[row.index].is_none()
                                    {
                                        return Err(Refusal);
                                    }
                                }
                                SavedParallelReply::Accepted(reply) => {
                                    let (_, (), domain, reply) = reply.as_parts();
                                    if !std::ptr::eq(domain, plan.domain) {
                                        return Err(Refusal);
                                    }
                                    domain.validate_value(&reply.value)?;
                                }
                            }
                        }
                        Ok(())
                    })
                    .map_err(|_| Refusal)?;
                    assert_eq!(fuel.remaining(), remaining);
                    for (index, reply) in replies.into_iter().enumerate().rev() {
                        if let Some(reply) = reply {
                            assert!(matches!(
                                barrier.poll().unwrap(),
                                ParallelPoll::Awaiting { .. }
                            ));
                            barrier
                                .try_accept(
                                    index,
                                    &plan.members.borrow()[index],
                                    reply,
                                    runtime.host,
                                )
                                .map_err(|_| Refusal)?;
                        }
                    }
                    let ParallelPoll::Results(rows) = barrier.poll().map_err(|_| Refusal)? else {
                        return Err(Refusal);
                    };
                    rows
                }
            };
            assert!(inventory.requests.iter().all(Option::is_none));
            Ok(Receipt {
                node: plan.node,
                identity: plan.id(),
                response: Response::Group(rows),
                buffer: Box::new(57),
            })
        }

        fn program<P: MixedProfile>(first: &str, last: &str) -> String {
            let mut source = P::program(first, last).replace("fn main()", "fn chain()");
            source.push_str("\nfn forward() = bind(inner: chain(), body: inner)\nfn main() = bind(outer: forward(), body: outer)\n");
            source
        }

        fn run<P: MixedProfile>(
            first: &str,
            last: &str,
            cuts: Option<[usize; 2]>,
            budget: u64,
            fault: Fault,
        ) -> Result<u64, ()> {
            let result = Cell::new(Err(()));
            fixture::<P>(|host, scope| {
                let node = compile(&program::<P>(first, last), &host).unwrap();
                let plans = plans(&node, &host);
                let runtime = Runtime {
                    host: &host,
                    plans: &plans,
                };
                let mut fuel = Fuel::new(budget);
                let mut locals = Vec::new();
                let mut outcome = evaluate_stacked_effects_in_scope(
                    &node,
                    &mut ScopeFrame::new(&mut locals),
                    &runtime,
                    &mut fuel,
                    STACK,
                );
                for phase in 0..=4 {
                    let Ok(current) = outcome else {
                        return;
                    };
                    match current {
                        StackedEffectOutcome::Value(PureValue::Scalar(value)) => {
                            assert_eq!(phase, 4);
                            assert_eq!(value, P::result(first));
                            assert_eq!(*host.invoked.borrow(), P::inputs(first));
                            assert_eq!(host.serial.get(), 8);
                            result.set(Ok(fuel.remaining()));
                            return;
                        }
                        StackedEffectOutcome::Suspended {
                            request,
                            continuation,
                        } => {
                            assert!(phase < 4);
                            assert!(continuation.frame_count() >= 3);
                            let (live, saved) = if cuts.is_some() {
                                let saved = continuation
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
                                    .collect();
                                (None, Some(saved))
                            } else {
                                (Some(continuation), None)
                            };
                            let plan = runtime.plan(request.node).unwrap();
                            let reply = if let Some(cuts) = cuts {
                                collect(
                                    &runtime,
                                    &scope,
                                    request,
                                    &mut fuel,
                                    cuts[usize::from(phase >= 2)],
                                    if phase == 1 { fault } else { Fault::None },
                                )
                            } else {
                                runtime
                                    .correlate_request(request, &mut fuel)
                                    .map_err(|_| Refusal)
                                    .and_then(|active| runtime.collect(active.dispatch))
                            };
                            let Ok(reply) = reply else {
                                if !matches!(fault, Fault::None) && phase == 1 {
                                    assert_eq!(
                                        host.invoked.borrow().len(),
                                        if first == "all" { 3 } else { 2 }
                                    );
                                    assert_eq!(host.serial.get(), 4);
                                }
                                return;
                            };
                            let stack = match saved {
                                Some(saved) => restore_effect_return_stack(saved, STACK).unwrap(),
                                None => live.unwrap(),
                            };
                            let mut pending = PendingReply::new(plan.id(), stack, plan);
                            let remaining = fuel.remaining();
                            // A wrong root identity cannot consume the saved stack or receipt.
                            let original = &*reply.buffer as *const u8;
                            let mut wrong = plan.id();
                            wrong.serial += 100;
                            let failed = pending.try_accept(&wrong, reply, &runtime).unwrap_err();
                            let reply = failed.reply;
                            assert!(std::ptr::eq(&*reply.buffer, original));
                            assert_eq!(fuel.remaining(), remaining);
                            let accepted = pending.try_accept(&plan.id(), reply, &runtime).unwrap();
                            outcome = resume_accepted_stacked_effects(
                                accepted, &runtime, &mut fuel, STACK,
                            );
                        }
                        _ => panic!("mixed helper chain must retain its return frames"),
                    }
                }
                panic!("saved mixed chain did not finish");
            });
            result.get()
        }

        fn cuts(kind: &str) -> std::ops::Range<usize> {
            0..if kind == "all" { 4 } else { 5 }
        }

        #[test]
        fn every_seq_all_frontier_composes_with_native_helper_return_stacks() {
            fn check<P: MixedProfile>() {
                for first in ["seq", "all", "repeat"] {
                    for last in ["seq", "all"] {
                        let live = run::<P>(first, last, None, 10_000, Fault::None).unwrap();
                        for a in cuts(first) {
                            for b in cuts(last) {
                                assert_eq!(
                                    run::<P>(first, last, Some([a, b]), 10_000, Fault::None),
                                    Ok(live),
                                    "{first}/{last}/{a}/{b}"
                                );
                            }
                        }
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn restored_mixed_helpers_keep_exact_original_fuel_at_every_cutoff() {
            fn check<P: MixedProfile>() {
                for (first, last) in [("seq", "all"), ("all", "seq")] {
                    let used = 10_000 - run::<P>(first, last, None, 10_000, Fault::None).unwrap();
                    for budget in 0..used {
                        assert_eq!(
                            run::<P>(first, last, Some([1, 2]), budget, Fault::None),
                            Err(())
                        );
                    }
                    for budget in used..=used + 1 {
                        assert_eq!(
                            run::<P>(first, last, Some([1, 2]), budget, Fault::None),
                            Ok(budget - used)
                        );
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn mixed_snapshot_admission_rejects_stale_revoked_and_closed_hosts_before_tail_work() {
            fn check<P: MixedProfile>() {
                for kind in ["seq", "all"] {
                    for fault in [Fault::Stale, Fault::Revoked, Fault::Closed] {
                        assert_eq!(run::<P>(kind, "seq", Some([1, 0]), 10_000, fault), Err(()));
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn saved_sequence_handle_correspondence_cannot_redirect_unpublished_mixed_work() {
            assert_eq!(
                run::<Counter>("seq", "all", Some([1, 0]), 10_000, Fault::QueuedHandle),
                Err(())
            );
            assert_eq!(
                run::<Panel>("seq", "all", Some([1, 0]), 10_000, Fault::QueuedHandle),
                Err(())
            );
        }
    }

    trait MixedProfile: Profile {
        fn program(first: &str, last: &str) -> String;
        fn result(first: &str) -> ScalarValue;
        fn inputs(first: &str) -> Vec<ScalarValue>;
    }
    impl MixedProfile for Counter {
        fn program(first: &str, last: &str) -> String {
            let (group, selected) = if first == "repeat" {
                (
                    "repeat(times: 2, body: step(n: seed))".into(),
                    "iteration_2",
                )
            } else {
                (
                    format!(
                        "{first}(left: step(n: seed), right: step(n: add(left: seed, right: 1)))"
                    ),
                    "right",
                )
            };
            format!(
                r#"
fn step(n: integer) = counter.add(amount: n)
fn main() = bind(seed: step(n: 1),
 body: bind(batch: {group},
 body: bind(selected: member(value: batch, name: "{selected}"),
 body: bind(middle: step(n: field(value: selected, name: "value")),
 body: bind(tail: {last}(left: step(n: middle), right: step(n: 1)),
 body: bind(final: member(value: tail, name: "left"),
 body: add(left: field(value: final, name: "value"), right: seed)))))))
"#
            )
        }
        fn result(first: &str) -> ScalarValue {
            ScalarValue::Integer(if first == "repeat" { 13 } else { 17 })
        }
        fn inputs(first: &str) -> Vec<ScalarValue> {
            let values = if first == "repeat" {
                [1, 1, 1, 3, 6, 1]
            } else {
                [1, 1, 2, 4, 8, 1]
            };
            values.into_iter().map(ScalarValue::Integer).collect()
        }
    }
    impl MixedProfile for Panel {
        fn program(first: &str, last: &str) -> String {
            let (group, selected) = if first == "repeat" {
                (
                    "repeat(times: 2, body: step(text: concat(left: seed, right: \"R\")))".into(),
                    "iteration_2",
                )
            } else {
                (
                    format!(
                        "{first}(left: step(text: concat(left: seed, right: \"L\")), right: step(text: concat(left: seed, right: \"R\")))"
                    ),
                    "right",
                )
            };
            format!(
                r#"
fn step(text: string) = panel.set_text(text: text)
fn main() = bind(initial: step(text: "seed"), body: bind(seed: field(value: initial, name: "text"),
 body: bind(batch: {group},
 body: bind(selected: member(value: batch, name: "{selected}"),
 body: bind(middle: step(text: concat(left: field(value: selected, name: "text"), right: "M")),
 body: bind(tail: {last}(left: step(text: concat(left: field(value: middle, name: "text"), right: "A")), right: step(text: "other")),
 body: bind(final: member(value: tail, name: "left"),
 body: concat(left: seed, right: concat(left: "/", right: field(value: final, name: "text"))))))))))
"#
            )
        }
        fn result(_: &str) -> ScalarValue {
            ScalarValue::String("seed/seedRMA".into())
        }
        fn inputs(first: &str) -> Vec<ScalarValue> {
            [
                "seed",
                if first == "repeat" { "seedR" } else { "seedL" },
                "seedR",
                "seedRM",
                "seedRMA",
                "other",
            ]
            .into_iter()
            .map(|s| ScalarValue::String(s.into()))
            .collect()
        }
    }
    fn run<P: MixedProfile>(first: &str, last: &str, budget: u64) -> Result<u64, ()> {
        let result = Cell::new(Err(()));
        fixture::<P>(|host, scope| {
            let node = compile(&P::program(first, last), &host).unwrap();
            let plans = plans(&node, &host);
            let runtime = Runtime {
                host: &host,
                plans: &plans,
            };
            let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(budget), STACK);
            for phase in 0..4 {
                let Ok(EffectSessionPoll::Request(active)) = vm.poll(&runtime) else {
                    assert_eq!(
                        vm.status(),
                        EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                    );
                    assert!(!vm.lease().is_open());
                    let remaining = vm.fuel_remaining();
                    assert!(matches!(
                        vm.poll(&runtime).unwrap(),
                        EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
                    ));
                    assert_eq!(vm.fuel_remaining(), remaining);
                    assert_eq!(
                        *host.invoked.borrow(),
                        P::inputs(first)[..host.invoked.borrow().len()]
                    );
                    return;
                };
                assert!(matches!(
                    (&active.work, phase),
                    (Work::Atom(_), 0 | 2) | (Work::Sequence(_) | Work::Parallel { .. }, 1 | 3)
                ));
                let fuel = vm.fuel_remaining();
                assert!(matches!(
                    vm.poll(&runtime).unwrap(),
                    EffectSessionPoll::Awaiting
                ));
                let reply = runtime.collect(active).unwrap();
                let identity = reply.identity.clone();
                vm.try_accept(&identity, reply, &runtime).unwrap();
                assert_eq!(vm.fuel_remaining(), fuel);
                assert_eq!(
                    *host.invoked.borrow(),
                    P::inputs(first)[..host.invoked.borrow().len()]
                );
            }
            match vm.poll(&runtime) {
                Ok(EffectSessionPoll::Value(PureValue::Scalar(value))) => {
                    assert_eq!(value, P::result(first));
                    result.set(Ok(vm.fuel_remaining()));
                    assert!(matches!(
                        vm.poll(&runtime).unwrap(),
                        EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
                    ));
                }
                Err(_) => assert_eq!(
                    vm.status(),
                    EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                ),
                _ => panic!("unexpected mixed result"),
            }
            assert!(!vm.lease().is_open());
            assert_eq!(*host.invoked.borrow(), P::inputs(first));
        });
        result.get()
    }
    #[test]
    fn two_hosts_mix_atomic_seq_all_repeat_and_captured_successors_through_one_vm() {
        for first in ["seq", "all", "repeat"] {
            for last in ["seq", "all"] {
                assert!(run::<Counter>(first, last, 10_000).is_ok());
                assert!(run::<Panel>(first, last, 10_000).is_ok());
            }
        }
    }

    fn reject_group_receipts<P: MixedProfile>(first: &str) {
        fixture::<P>(|host, scope| {
            let node = compile(&P::program(first, "all"), &host).unwrap();
            let plans = plans(&node, &host);
            let runtime = Runtime {
                host: &host,
                plans: &plans,
            };
            let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(10_000), STACK);
            let mut predecessor = None;
            for phase in 0..4 {
                let EffectSessionPoll::Request(active) = vm.poll(&runtime).unwrap() else {
                    panic!()
                };
                let mut reply = runtime.collect(active).unwrap();
                let identity = reply.identity.clone();
                let fuel = vm.fuel_remaining();
                let pointer = &*reply.buffer as *const u8;
                if phase == 1 {
                    let original = reply.node;
                    reply.node = plans
                        .iter()
                        .find(|p| {
                            matches!(p.node, Node::Group { .. }) && !std::ptr::eq(p.node, original)
                        })
                        .unwrap()
                        .node;
                    let rejected = vm.try_accept(&identity, reply, &runtime).unwrap_err();
                    reply = rejected.reply;
                    assert_eq!(&*reply.buffer as *const u8, pointer);
                    reply.node = original;
                    let Response::Group(rows) = &mut reply.response else {
                        panic!()
                    };
                    rows.swap(0, 1);
                    reply = vm.try_accept(&identity, reply, &runtime).unwrap_err().reply;
                    let Response::Group(rows) = &mut reply.response else {
                        panic!()
                    };
                    rows.swap(0, 1);
                    let last = rows.pop().unwrap();
                    reply = vm.try_accept(&identity, reply, &runtime).unwrap_err().reply;
                    let Response::Group(rows) = &mut reply.response else {
                        panic!()
                    };
                    rows.push(last);
                    predecessor = Some(identity.clone());
                }
                if phase == 3 {
                    reply = vm
                        .try_accept(predecessor.as_ref().unwrap(), reply, &runtime)
                        .unwrap_err()
                        .reply;
                }
                assert_eq!(&*reply.buffer as *const u8, pointer);
                assert_eq!(vm.fuel_remaining(), fuel);
                assert!(vm.lease().is_open());
                assert!(matches!(
                    vm.poll(&runtime).unwrap(),
                    EffectSessionPoll::Awaiting
                ));
                vm.try_accept(&identity, reply, &runtime).unwrap();
                assert_eq!(vm.fuel_remaining(), fuel);
            }
            let EffectSessionPoll::Value(PureValue::Scalar(value)) = vm.poll(&runtime).unwrap()
            else {
                panic!()
            };
            assert_eq!(value, P::result(first));
            assert_eq!(*host.invoked.borrow(), P::inputs(first));
        });
    }
    #[test]
    fn mixed_group_wrong_site_order_count_and_stale_root_receipts_preserve_retryable_ownership() {
        for first in ["seq", "all", "repeat"] {
            reject_group_receipts::<Counter>(first);
            reject_group_receipts::<Panel>(first);
        }
    }

    fn cancel_group<P: MixedProfile>(first: &str, received: usize) {
        fixture::<P>(|host, scope| {
            let node = compile(&P::program(first, "all"), &host).unwrap();
            let plans = plans(&node, &host);
            let runtime = Runtime {
                host: &host,
                plans: &plans,
            };
            let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(10_000), STACK);
            let EffectSessionPoll::Request(initial) = vm.poll(&runtime).unwrap() else {
                panic!()
            };
            let reply = runtime.collect(initial).unwrap();
            vm.try_accept(&reply.identity.clone(), reply, &runtime)
                .unwrap();
            let EffectSessionPoll::Request(active) = vm.poll(&runtime).unwrap() else {
                panic!()
            };
            let fuel = vm.fuel_remaining();
            if received == 3 {
                let reply = runtime.collect(active).unwrap();
                vm.try_accept(&reply.identity.clone(), reply, &runtime)
                    .unwrap();
                assert!(vm.cancel());
                assert!(matches!(
                    vm.poll(&runtime).unwrap(),
                    EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
                ));
                assert_eq!(vm.fuel_remaining(), fuel);
                assert_eq!(*host.invoked.borrow(), P::inputs(first)[..3]);
                return;
            }
            let rows = match active.work {
                Work::Sequence(mut sequence) => {
                    let mut rows = Vec::new();
                    for _ in 0..received {
                        let SequencePoll::Request { request, .. } =
                            runtime.publish(&mut sequence).unwrap()
                        else {
                            panic!()
                        };
                        let identity = request.identity.clone();
                        rows.push(
                            sequence
                                .try_accept(&identity, host.dispatch(request).unwrap(), &host)
                                .unwrap(),
                        );
                    }
                    assert!(vm.cancel());
                    if received < 2 {
                        assert!(runtime.publish(&mut sequence).is_err());
                    }
                    rows
                }
                Work::Parallel {
                    mut barrier,
                    requests,
                } => {
                    let mut pending = requests.into_iter();
                    for _ in 0..received {
                        let request = pending.next().unwrap();
                        let identity = request.request.identity.clone();
                        barrier
                            .try_accept(
                                request.index,
                                &identity,
                                host.dispatch(request.request).unwrap(),
                                &host,
                            )
                            .unwrap();
                    }
                    assert!(vm.cancel());
                    for request in pending {
                        assert!(host.dispatch(request.request).is_err());
                    }
                    if received == 2 {
                        let ParallelPoll::Results(rows) = barrier.poll().unwrap() else {
                            panic!()
                        };
                        rows
                    } else {
                        assert!(barrier.cancel());
                        Vec::new()
                    }
                }
                Work::Atom(_) => panic!(),
            };
            let reply = Receipt {
                node: active.plan.node,
                identity: active.plan.id(),
                response: Response::Group(rows),
                buffer: Box::new(91),
            };
            let pointer = &*reply.buffer as *const u8;
            let rejected = vm
                .try_accept(&reply.identity.clone(), reply, &runtime)
                .unwrap_err();
            assert_eq!(&*rejected.reply.buffer as *const u8, pointer);
            assert_eq!(vm.fuel_remaining(), fuel);
            assert!(matches!(
                vm.poll(&runtime).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
            ));
            assert_eq!(*host.invoked.borrow(), P::inputs(first)[..1 + received]);
        });
    }
    #[test]
    fn mixed_partial_and_complete_groups_cannot_escape_parent_cancellation_or_start_a_tail() {
        for first in ["seq", "all", "repeat"] {
            for received in 0..=3 {
                cancel_group::<Counter>(first, received);
                cancel_group::<Panel>(first, received);
            }
        }
    }

    fn revoked_successor<P: MixedProfile>(first: &str, phase: usize) {
        fixture::<P>(|host, scope| {
            let node = compile(&P::program(first, "all"), &host).unwrap();
            let plans = plans(&node, &host);
            let runtime = Runtime {
                host: &host,
                plans: &plans,
            };
            let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(10_000), STACK);
            for _ in 0..2 {
                let EffectSessionPoll::Request(active) = vm.poll(&runtime).unwrap() else {
                    panic!()
                };
                let reply = runtime.collect(active).unwrap();
                vm.try_accept(&reply.identity.clone(), reply, &runtime)
                    .unwrap();
            }
            if phase == 0 {
                host.granted.set(false);
            } else {
                host.revision.set(2);
            }
            assert!(vm.poll(&runtime).is_err());
            assert_eq!(
                vm.status(),
                EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
            );
            assert!(!vm.lease().is_open());
            assert_eq!(*host.invoked.borrow(), P::inputs(first)[..3]);
        });
    }
    #[test]
    fn mixed_accepted_group_reentry_rechecks_grants_and_revision_before_the_atomic_successor() {
        for first in ["seq", "all", "repeat"] {
            for phase in 0..2 {
                revoked_successor::<Counter>(first, phase);
                revoked_successor::<Panel>(first, phase);
            }
        }
    }

    fn fuel<P: MixedProfile>(first: &str, last: &str) {
        let used = 10_000 - run::<P>(first, last, 10_000).unwrap();
        for budget in 0..used {
            assert!(
                run::<P>(first, last, budget).is_err(),
                "{first}/{last}: {budget} < {used}"
            );
        }
        assert_eq!(run::<P>(first, last, used), Ok(0));
        assert_eq!(run::<P>(first, last, used + 1), Ok(1));
    }
    #[test]
    fn mixed_preparation_correlation_assembly_and_successors_share_exactly_one_fuel_budget() {
        for first in ["seq", "all", "repeat"] {
            for last in ["seq", "all"] {
                fuel::<Counter>(first, last);
                fuel::<Panel>(first, last);
            }
        }
    }

    fn interleaved<P: MixedProfile>(first: &str) {
        fixture::<P>(|left, left_scope| {
            let node = compile(&P::program(first, "all"), &left).unwrap();
            fixture::<P>(|right, right_scope| {
                let left_plans = plans(&node, &left);
                let right_plans = plans(&node, &right);
                let a_host = Runtime {
                    host: &left,
                    plans: &left_plans,
                };
                let b_host = Runtime {
                    host: &right,
                    plans: &right_plans,
                };
                let mut a = LocalVm::new(left_scope, &node, Vec::new(), Fuel::new(10_000), STACK);
                let mut b = LocalVm::new(right_scope, &node, Vec::new(), Fuel::new(10_000), STACK);
                let EffectSessionPoll::Request(seed) = a.poll(&a_host).unwrap() else {
                    panic!()
                };
                let reply = a_host.collect(seed).unwrap();
                a.try_accept(&reply.identity.clone(), reply, &a_host)
                    .unwrap();
                let EffectSessionPoll::Request(seed) = b.poll(&b_host).unwrap() else {
                    panic!()
                };
                let reply = b_host.collect(seed).unwrap();
                b.try_accept(&reply.identity.clone(), reply, &b_host)
                    .unwrap();
                let EffectSessionPoll::Request(a_group) = a.poll(&a_host).unwrap() else {
                    panic!()
                };
                let EffectSessionPoll::Request(b_group) = b.poll(&b_host).unwrap() else {
                    panic!()
                };
                assert!(std::ptr::eq(a_group.plan.node, b_group.plan.node));
                assert_eq!(a_group.plan.id().serial, b_group.plan.id().serial);
                let mut reply = b_host.collect(b_group).unwrap();
                let b_identity = reply.identity.clone();
                let pointer = &*reply.buffer as *const u8;
                let a_fuel = a.fuel_remaining();
                let b_fuel = b.fuel_remaining();
                reply = a.try_accept(&b_identity, reply, &a_host).unwrap_err().reply;
                // Matching root/IR labels cannot launder another execution's accepted members.
                reply.identity = a_group.plan.id();
                reply = a
                    .try_accept(&reply.identity.clone(), reply, &a_host)
                    .unwrap_err()
                    .reply;
                assert_eq!(&*reply.buffer as *const u8, pointer);
                assert_eq!(a.fuel_remaining(), a_fuel);
                assert_eq!(b.fuel_remaining(), b_fuel);
                reply.identity = b_identity.clone();
                b.try_accept(&b_identity, reply, &b_host).unwrap();
                assert!(a.cancel());
                assert!(a_host.collect(a_group).is_err());
                assert!(b.lease().is_open());
                for _ in 0..2 {
                    let EffectSessionPoll::Request(active) = b.poll(&b_host).unwrap() else {
                        panic!()
                    };
                    let reply = b_host.collect(active).unwrap();
                    b.try_accept(&reply.identity.clone(), reply, &b_host)
                        .unwrap();
                }
                let EffectSessionPoll::Value(PureValue::Scalar(value)) = b.poll(&b_host).unwrap()
                else {
                    panic!()
                };
                assert_eq!(value, P::result(first));
                assert_eq!(*left.invoked.borrow(), P::inputs(first)[..1]);
                assert_eq!(*right.invoked.borrow(), P::inputs(first));
            });
        });
    }
    #[test]
    fn mixed_shared_ir_rejects_foreign_group_members_even_with_a_matching_root_label() {
        for first in ["seq", "all", "repeat"] {
            interleaved::<Counter>(first);
            interleaved::<Panel>(first);
        }
    }

    mod selected_data {
        use super::*;

        const TYPES: [ScalarType; 6] = [
            ScalarType::Integer,
            ScalarType::Boolean,
            ScalarType::String,
            ScalarType::None,
            ScalarType::OptionalString,
            ScalarType::StringList,
        ];

        struct Case {
            source: String,
            selected: usize,
            output: ScalarValue,
            inputs: Vec<ScalarValue>,
            tail: String,
        }

        fn case<P: Profile>(kind: &str, selected: usize, ty: ScalarType) -> Case {
            let (conversion, output, integer_tail, text_tail, integer, text) = match ty {
                ScalarType::Integer => (
                    "parse_integer(value: text)",
                    ScalarValue::Integer(7),
                    "answer",
                    "to_string(value: answer)",
                    7,
                    "7",
                ),
                ScalarType::Boolean => (
                    "eq(left: text, right: \"7\")",
                    ScalarValue::Boolean(true),
                    "choose(when: answer, then: 7, otherwise: 0)",
                    "to_string(value: answer)",
                    7,
                    "true",
                ),
                ScalarType::String => (
                    "concat(left: text, right: \"!\")",
                    ScalarValue::String("7!".into()),
                    "len(value: answer)",
                    "answer",
                    2,
                    "7!",
                ),
                ScalarType::None => (
                    "choose(when: eq(left: text, right: \"7\"), then: none, otherwise: none)",
                    ScalarValue::None,
                    "choose(when: eq(left: answer, right: none), then: 7, otherwise: 0)",
                    "choose(when: eq(left: answer, right: none), then: \"none\", otherwise: \"wrong\")",
                    7,
                    "none",
                ),
                ScalarType::OptionalString => (
                    "char_at(left: text, right: 0)",
                    ScalarValue::OptionalString(OptionalStringValue(Some("7".into()))),
                    "len(value: value_or(left: answer, right: \"missing\"))",
                    "value_or(left: answer, right: \"missing\")",
                    1,
                    "7",
                ),
                ScalarType::StringList => (
                    "strings(first: text, second: \"tail\")",
                    ScalarValue::StringList(StringListValue(vec!["7".into(), "tail".into()])),
                    "len(value: answer)",
                    "join(left: answer, right: \"/\")",
                    2,
                    "7/tail",
                ),
            };
            let counter = P::ARGUMENT == ScalarType::Integer;
            let (single, group, projection, tail) = if counter {
                (
                    "counter.add(amount: 7)",
                    format!("{kind}(left: counter.add(amount: 3), right: counter.add(amount: 4))"),
                    "to_string(value: field(value: member(value: batch, name: \"right\"), name: \"value\"))",
                    format!("counter.add(amount: {integer_tail})"),
                )
            } else {
                (
                    "panel.set_text(text: \"7\")",
                    format!(
                        "{kind}(left: panel.set_text(text: \"3\"), right: panel.set_text(text: \"7\"))"
                    ),
                    "field(value: member(value: batch, name: \"right\"), name: \"text\")",
                    format!("panel.set_text(text: {text_tail})"),
                )
            };
            let atomic_text = if counter {
                "to_string(value: caller)"
            } else {
                "field(value: caller, name: \"text\")"
            };
            let source = format!(
                r#"
fn convert(text: string) = {conversion}
fn single(unused: integer) = bind(caller: {single}, body: convert(text: {atomic_text}))
fn gathered(unused: integer) = bind(batch: {group}, body: convert(text: {projection}))
fn main() = bind(caller: "outer", body: bind(answer:
 choose(when: eq(left: {selected}, right: 0), then: single(unused: 7),
 otherwise: choose(when: eq(left: {selected}, right: 1), then: gathered(unused: 7),
 otherwise: bind(local_text: "7", body: convert(text: local_text)))),
 body: bind(ack: {tail}, body: choose(when: eq(left: caller, right: "outer"),
 then: answer, otherwise: convert(text: "wrong")))))
"#
            );
            let mut inputs = match (counter, selected) {
                (true, 0) => vec![ScalarValue::Integer(7)],
                (true, 1) => vec![ScalarValue::Integer(3), ScalarValue::Integer(4)],
                (false, 0) => vec![ScalarValue::String("7".into())],
                (false, 1) => vec![
                    ScalarValue::String("3".into()),
                    ScalarValue::String("7".into()),
                ],
                (_, 2) => Vec::new(),
                _ => panic!("unknown selected data path"),
            };
            inputs.push(if counter {
                ScalarValue::Integer(integer)
            } else {
                ScalarValue::String(text.into())
            });
            Case {
                source,
                selected,
                output,
                inputs,
                tail,
            }
        }

        fn payload_case<P: Profile>(
            kind: &str,
            selected: usize,
            variant: usize,
            overflow: bool,
        ) -> Case {
            let optional = variant < 3;
            let mut case = case::<P>(
                kind,
                selected,
                if optional {
                    ScalarType::OptionalString
                } else {
                    ScalarType::StringList
                },
            );
            let (conversion, value) = match variant {
                0 => (
                    "optional_string(value: none)".into(),
                    ScalarValue::OptionalString(OptionalStringValue(None)),
                ),
                1 => (
                    "optional_string(value: \"\")".into(),
                    ScalarValue::OptionalString(OptionalStringValue(Some(String::new()))),
                ),
                2 => {
                    let text = "x".repeat(MAX_SCALAR_STRING_BYTES);
                    let conversion = if overflow {
                        format!("optional_string(value: concat(left: \"{text}\", right: \"x\"))")
                    } else {
                        format!("optional_string(value: \"{text}\")")
                    };
                    (
                        conversion,
                        ScalarValue::OptionalString(OptionalStringValue(Some(text))),
                    )
                }
                3 => (
                    "strings()".into(),
                    ScalarValue::StringList(StringListValue(Vec::new())),
                ),
                4 => {
                    let text = "x".repeat(MAX_SCALAR_STRING_BYTES / MAX_STRING_LIST_ITEMS);
                    let count = MAX_STRING_LIST_ITEMS + usize::from(overflow);
                    (
                        format!(
                            "loop(parts: strings(), while: lt(left: len(value: parts), right: {count}), next: append(left: parts, right: \"{text}\"), limit: {count})"
                        ),
                        ScalarValue::StringList(StringListValue(vec![text; MAX_STRING_LIST_ITEMS])),
                    )
                }
                _ => panic!("unknown data payload variant"),
            };
            let original = if optional {
                "char_at(left: text, right: 0)"
            } else {
                "strings(first: text, second: \"tail\")"
            };
            assert_eq!(case.source.matches(original).count(), 1);
            case.source = case.source.replace(original, &conversion);
            let (tail, input) = if P::ARGUMENT == ScalarType::Integer {
                ("counter.add(amount: 1)", ScalarValue::Integer(1))
            } else {
                (
                    "panel.set_text(text: \"ok\")",
                    ScalarValue::String("ok".into()),
                )
            };
            case.source = case
                .source
                .replace(&format!("ack: {},", case.tail), &format!("ack: {tail},"));
            *case.inputs.last_mut().unwrap() = input;
            case.output = value;
            case.tail = tail.into();
            case
        }

        #[derive(Debug, PartialEq)]
        struct Outcome {
            complete: bool,
            remaining: u64,
            invoked: usize,
        }

        fn run<P: Profile>(case: &Case, budget: u64) -> Outcome {
            let mut outcome = None;
            fixture::<P>(|host, scope| {
                let node = compile(&case.source, &host).unwrap();
                let plans = plans(&node, &host);
                let runtime = Runtime {
                    host: &host,
                    plans: &plans,
                };
                let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(budget), STACK);
                for phase in 0..3 {
                    match vm.poll(&runtime) {
                        Ok(EffectSessionPoll::Request(active)) => {
                            assert_eq!(
                                matches!(active.work, Work::Sequence(_) | Work::Parallel { .. }),
                                phase == 0 && case.selected == 1,
                            );
                            let fuel = vm.fuel_remaining();
                            assert!(matches!(
                                vm.poll(&runtime).unwrap(),
                                EffectSessionPoll::Awaiting
                            ));
                            let reply = runtime.collect(active).unwrap();
                            let identity = reply.identity.clone();
                            let mut foreign = identity.clone();
                            foreign.serial += 100;
                            let buffer = &*reply.buffer as *const u8;
                            let reply = vm.try_accept(&foreign, reply, &runtime).unwrap_err().reply;
                            assert_eq!(&*reply.buffer as *const u8, buffer);
                            assert_eq!(vm.fuel_remaining(), fuel);
                            assert!(vm.lease().is_open());
                            vm.try_accept(&identity, reply, &runtime).unwrap();
                            assert_eq!(vm.fuel_remaining(), fuel);
                        }
                        Ok(EffectSessionPoll::Value(PureValue::Scalar(value))) => {
                            assert_eq!(value, case.output);
                            assert_eq!(*host.invoked.borrow(), case.inputs);
                            let active = plans
                                .iter()
                                .filter(|plan| plan.identity.borrow().is_some())
                                .count();
                            assert_eq!(active, if case.selected == 2 { 1 } else { 2 });
                            assert!(
                                plans
                                    .iter()
                                    .filter(|plan| plan.identity.borrow().is_none())
                                    .all(|plan| plan.members.borrow().is_empty())
                            );
                            outcome = Some(Outcome {
                                complete: true,
                                remaining: vm.fuel_remaining(),
                                invoked: case.inputs.len(),
                            });
                            assert!(matches!(
                                vm.poll(&runtime).unwrap(),
                                EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
                            ));
                            break;
                        }
                        Err(_) => {
                            assert_eq!(
                                vm.status(),
                                EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                            );
                            outcome = Some(Outcome {
                                complete: false,
                                remaining: vm.fuel_remaining(),
                                invoked: host.invoked.borrow().len(),
                            });
                            assert!(matches!(
                                vm.poll(&runtime).unwrap(),
                                EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
                            ));
                            break;
                        }
                        _ => panic!("unexpected selected data outcome"),
                    }
                }
                assert!(!vm.lease().is_open());
                assert_eq!(vm.fuel_remaining(), outcome.as_ref().unwrap().remaining);
                assert_eq!(
                    *host.invoked.borrow(),
                    case.inputs[..host.invoked.borrow().len()]
                );
            });
            outcome.unwrap()
        }

        #[test]
        fn both_hosts_join_atomic_group_and_pure_returns_for_all_six_data_types() {
            fn check<P: Profile>() {
                for kind in ["seq", "all"] {
                    for selected in 0..3 {
                        for ty in TYPES {
                            let case = case::<P>(kind, selected, ty);
                            assert!(run::<P>(&case, 10_000).complete, "{kind}/{selected}/{ty:?}");
                        }
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn selected_data_return_and_caller_capture_share_every_fuel_boundary() {
            fn check<P: Profile>() {
                for kind in ["seq", "all"] {
                    for selected in 0..3 {
                        for ty in TYPES {
                            let case = case::<P>(kind, selected, ty);
                            let used = 10_000 - run::<P>(&case, 10_000).remaining;
                            for budget in 0..used {
                                assert!(
                                    !run::<P>(&case, budget).complete,
                                    "{kind}/{selected}/{ty:?}: {budget} < {used}"
                                );
                            }
                            for extra in 0..=1 {
                                assert_eq!(
                                    run::<P>(&case, used + extra),
                                    Outcome {
                                        complete: true,
                                        remaining: extra,
                                        invoked: case.inputs.len()
                                    }
                                );
                            }
                        }
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn selected_data_arguments_remain_lazy_but_selected_unused_arguments_still_run() {
            fn check<P: Profile>() {
                for kind in ["seq", "all"] {
                    for selected in 0..3 {
                        for ty in TYPES {
                            let mut case = case::<P>(kind, selected, ty);
                            let original = run::<P>(&case, 10_000);
                            case.source = case.source.replace(
                                "single(unused: 7)",
                                "single(unused: div(left: 1, right: 0))",
                            );
                            let result = run::<P>(&case, 10_000);
                            if selected == 0 {
                                assert!(!result.complete);
                                assert_eq!(result.invoked, 0);
                            } else {
                                assert_eq!(result, original);
                            }
                        }
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn selected_data_captures_distinguish_absent_empty_and_maximum_bounded_payloads() {
            fn check<P: Profile>() {
                for kind in ["seq", "all"] {
                    for selected in 0..3 {
                        for variant in 0..5 {
                            let case = payload_case::<P>(kind, selected, variant, false);
                            let ample = run::<P>(&case, 1_000_000);
                            assert!(ample.complete);
                            let used = 1_000_000 - ample.remaining;
                            assert!(!run::<P>(&case, used - 1).complete);
                            assert_eq!(
                                run::<P>(&case, used),
                                Outcome {
                                    complete: true,
                                    remaining: 0,
                                    invoked: case.inputs.len()
                                }
                            );
                        }
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn selected_data_payload_overflow_cannot_dispatch_the_caller_tail() {
            fn check<P: Profile>() {
                for kind in ["seq", "all"] {
                    for selected in 0..3 {
                        for variant in [2, 4] {
                            let case = payload_case::<P>(kind, selected, variant, true);
                            let outcome = run::<P>(&case, 1_000_000);
                            assert!(!outcome.complete);
                            assert_eq!(outcome.invoked, case.inputs.len() - 1);
                            assert!(outcome.remaining > 0, "overflow is not fuel exhaustion");
                        }
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn cold_selected_data_types_names_and_pure_arguments_fail_before_dispatch() {
            fn check<P: Profile>() {
                for kind in ["seq", "all"] {
                    for ty in TYPES {
                        let case = case::<P>(kind, 2, ty);
                        fixture::<P>(|host, _scope| {
                            let wrong = if ty == ScalarType::Boolean {
                                "0"
                            } else {
                                "false"
                            };
                            for (from, replacement) in [
                                ("body: convert(text: local_text)", format!("body: {wrong}")),
                                ("single(unused: 7)", "single(unused: false)".into()),
                                ("single(unused: 7)", "single(extra: 7)".into()),
                                (
                                    "single(unused: 7)",
                                    "single(unused: single(unused: 7))".into(),
                                ),
                                (P::OPERATION, "foreign.unknown".into()),
                            ] {
                                let invalid = case.source.replace(from, &replacement);
                                assert_ne!(invalid, case.source);
                                assert!(compile(&invalid, &host).is_err(), "{kind}/{ty:?}: {from}");
                                assert!(host.invoked.borrow().is_empty());
                                assert_eq!(host.serial.get(), 0);
                            }
                            // Cold effects still require the current host's authority.
                            host.granted.set(false);
                            assert!(compile(&case.source, &host).is_err());
                            assert_eq!(host.serial.get(), 0);
                        });
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn selected_data_receipt_reentry_cannot_cross_revocation_or_cancellation() {
            fn check<P: Profile>() {
                for kind in ["seq", "all"] {
                    for selected in 0..2 {
                        for ty in TYPES {
                            for revoke in 0..4 {
                                let case = case::<P>(kind, selected, ty);
                                fixture::<P>(|host, scope| {
                                    let node = compile(&case.source, &host).unwrap();
                                    let plans = plans(&node, &host);
                                    let runtime = Runtime {
                                        host: &host,
                                        plans: &plans,
                                    };
                                    let mut vm = LocalVm::new(
                                        scope,
                                        &node,
                                        Vec::new(),
                                        Fuel::new(10_000),
                                        STACK,
                                    );
                                    let EffectSessionPoll::Request(active) =
                                        vm.poll(&runtime).unwrap()
                                    else {
                                        panic!()
                                    };
                                    let reply = runtime.collect(active).unwrap();
                                    let identity = reply.identity.clone();
                                    vm.try_accept(&identity, reply, &runtime).unwrap();
                                    match revoke {
                                        0 => host.granted.set(false),
                                        1 => host.revision.set(2),
                                        2 => host.version.set(32),
                                        3 => {
                                            assert!(vm.cancel());
                                        }
                                        _ => unreachable!(),
                                    }
                                    if revoke == 3 {
                                        assert!(matches!(
                                            vm.poll(&runtime).unwrap(),
                                            EffectSessionPoll::Terminal(
                                                EffectSessionEnd::Cancelled
                                            )
                                        ));
                                    } else {
                                        assert!(vm.poll(&runtime).is_err());
                                        assert_eq!(
                                            vm.status(),
                                            EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                                        );
                                    }
                                    assert!(!vm.lease().is_open());
                                    assert_eq!(
                                        *host.invoked.borrow(),
                                        case.inputs[..case.inputs.len() - 1]
                                    );
                                    assert_eq!(
                                        plans
                                            .iter()
                                            .filter(|plan| plan.identity.borrow().is_some())
                                            .count(),
                                        1
                                    );
                                });
                            }
                        }
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }
    }

    struct ConditionalCase {
        source: String,
        output: ScalarValue,
        inputs: Vec<ScalarValue>,
        cold_call: String,
        invalid_call: String,
        cold_fault_call: String,
    }

    fn conditional_case<P: Profile>(kind: &str, left: bool, early: bool) -> ConditionalCase {
        match P::ARGUMENT {
            ScalarType::Integer => {
                let selected = if left { 4 } else { 8 };
                let mut inputs = if left { vec![1, 1, 2] } else { vec![1, 3, 4] };
                if !early {
                    inputs.push(selected);
                }
                ConditionalCase {
                    source: format!(
                        r#"
fn step(n: integer) = counter.add(amount: n)
fn finish(n: integer, seed: integer) = bind(last: step(n: n), body: add(left: last, right: seed))
fn batch(seed: integer, side: boolean) = choose(when: side,
 then: {kind}(left: step(n: seed), right: step(n: add(left: seed, right: 1))),
 otherwise: {kind}(left: step(n: add(left: seed, right: 2)), right: step(n: add(left: seed, right: 3))))
fn main() = bind(seed: step(n: 1),
 body: bind(group: batch(seed: seed, side: eq(left: seed, right: {select_guard})),
 body: bind(alias: group,
 body: bind(selected: field(value: member(value: alias, name: "right"), name: "value"),
 body: choose(when: eq(left: seed, right: {early_guard}),
 then: add(left: selected, right: seed), otherwise: finish(n: selected, seed: seed))))))
"#,
                        select_guard = if left { 1 } else { 2 },
                        early_guard = if early { 1 } else { 2 }
                    ),
                    output: ScalarValue::Integer(if early {
                        selected + 1
                    } else {
                        2 * selected + 1
                    }),
                    inputs: inputs.into_iter().map(ScalarValue::Integer).collect(),
                    cold_call: "step(n: add(left: seed, right: 3))".into(),
                    invalid_call: "step(n: false)".into(),
                    cold_fault_call: "step(n: div(left: 1, right: 0))".into(),
                }
            }
            ScalarType::String => {
                let selected = if left { "seedR" } else { "seedY" };
                let mut inputs = if left {
                    vec!["seed", "seedL", "seedR"]
                } else {
                    vec!["seed", "seedX", "seedY"]
                }
                .into_iter()
                .map(|value| ScalarValue::String(value.into()))
                .collect::<Vec<_>>();
                if !early {
                    inputs.push(ScalarValue::String(format!("{selected}!")));
                }
                ConditionalCase {
                    source: format!(
                        r#"
fn step(text: string) = panel.set_text(text: text)
fn finish(text: string, seed: string) = bind(last: step(text: concat(left: text, right: "!")),
 body: concat(left: seed, right: concat(left: "/", right: field(value: last, name: "text"))))
fn batch(seed: string, side: boolean) = choose(when: side,
 then: {kind}(left: step(text: concat(left: seed, right: "L")), right: step(text: concat(left: seed, right: "R"))),
 otherwise: {kind}(left: step(text: concat(left: seed, right: "X")), right: step(text: concat(left: seed, right: "Y"))))
fn main() = bind(initial: step(text: "seed"), body: bind(seed: field(value: initial, name: "text"),
 body: bind(group: batch(seed: seed, side: eq(left: seed, right: "{select_guard}")),
 body: bind(alias: group,
 body: bind(selected: field(value: member(value: alias, name: "right"), name: "text"),
 body: choose(when: eq(left: seed, right: "{early_guard}"),
 then: concat(left: seed, right: concat(left: "/", right: selected)), otherwise: finish(text: selected, seed: seed)))))))
"#,
                        select_guard = if left { "seed" } else { "other" },
                        early_guard = if early { "seed" } else { "other" }
                    ),
                    output: ScalarValue::String(format!(
                        "seed/{selected}{}",
                        if early { "" } else { "!" }
                    )),
                    inputs,
                    cold_call: "step(text: concat(left: seed, right: \"Y\"))".into(),
                    invalid_call: "step(text: false)".into(),
                    cold_fault_call: "step(text: to_string(value: div(left: 1, right: 0)))".into(),
                }
            }
            _ => panic!("unexpected fixture profile"),
        }
    }

    fn run_conditional<P: Profile>(case: &ConditionalCase, budget: u64) -> Result<u64, ()> {
        let result = Cell::new(Err(()));
        fixture::<P>(|host, scope| {
            let node = compile(&case.source, &host).unwrap();
            let plans = plans(&node, &host);
            assert_eq!(
                plans
                    .iter()
                    .filter(|plan| matches!(plan.node, Node::Group { .. }))
                    .count(),
                2
            );
            let runtime = Runtime {
                host: &host,
                plans: &plans,
            };
            let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(budget), STACK);
            for phase in 0..=3 {
                match vm.poll(&runtime) {
                    Ok(EffectSessionPoll::Request(active)) => {
                        assert!(matches!(
                            (&active.work, phase),
                            (Work::Atom(_), 0 | 2) | (Work::Sequence(_) | Work::Parallel { .. }, 1)
                        ));
                        let fuel = vm.fuel_remaining();
                        assert!(matches!(
                            vm.poll(&runtime).unwrap(),
                            EffectSessionPoll::Awaiting
                        ));
                        assert_eq!(vm.fuel_remaining(), fuel);
                        let mut reply = runtime.collect(active).unwrap();
                        let identity = reply.identity.clone();
                        if phase == 1 {
                            let original = reply.node;
                            let cold = plans
                                .iter()
                                .find(|plan| {
                                    matches!(plan.node, Node::Group { .. })
                                        && !std::ptr::eq(plan.node, original)
                                })
                                .unwrap();
                            assert!(cold.identity.borrow().is_none());
                            assert!(cold.members.borrow().is_empty());
                            let pointer = &*reply.buffer as *const u8;
                            reply.node = cold.node;
                            reply = vm.try_accept(&identity, reply, &runtime).unwrap_err().reply;
                            assert_eq!(&*reply.buffer as *const u8, pointer);
                            assert_eq!(vm.fuel_remaining(), fuel);
                            assert!(vm.lease().is_open());
                            reply.node = original;
                        }
                        vm.try_accept(&identity, reply, &runtime).unwrap();
                        assert_eq!(vm.fuel_remaining(), fuel);
                        assert_eq!(
                            *host.invoked.borrow(),
                            case.inputs[..host.invoked.borrow().len()]
                        );
                    }
                    Ok(EffectSessionPoll::Value(PureValue::Scalar(value))) => {
                        assert_eq!(value, case.output);
                        assert_eq!(*host.invoked.borrow(), case.inputs);
                        assert_eq!(phase, if case.inputs.len() == 3 { 2 } else { 3 });
                        assert_eq!(
                            plans
                                .iter()
                                .filter(|plan| matches!(plan.node, Node::Group { .. })
                                    && plan.identity.borrow().is_some())
                                .count(),
                            1
                        );
                        let fuel = vm.fuel_remaining();
                        assert!(matches!(
                            vm.poll(&runtime).unwrap(),
                            EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
                        ));
                        assert_eq!(vm.fuel_remaining(), fuel);
                        assert!(!vm.lease().is_open());
                        result.set(Ok(fuel));
                        return;
                    }
                    Err(_) => {
                        assert_eq!(
                            vm.status(),
                            EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                        );
                        assert!(!vm.lease().is_open());
                        let fuel = vm.fuel_remaining();
                        assert!(matches!(
                            vm.poll(&runtime).unwrap(),
                            EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
                        ));
                        assert_eq!(vm.fuel_remaining(), fuel);
                        assert_eq!(
                            *host.invoked.borrow(),
                            case.inputs[..host.invoked.borrow().len()]
                        );
                        return;
                    }
                    _ => panic!("unexpected conditional pipeline outcome"),
                }
            }
            panic!("conditional pipeline did not terminate");
        });
        result.get()
    }

    #[test]
    fn two_hosts_select_group_helpers_alias_results_and_take_early_or_captured_returns() {
        fn check<P: Profile>() {
            for kind in ["seq", "all"] {
                for left in [false, true] {
                    for early in [false, true] {
                        let mut case = conditional_case::<P>(kind, left, early);
                        assert!(run_conditional::<P>(&case, 10_000).is_ok());
                        if !early {
                            let field = if P::ARGUMENT == ScalarType::Integer {
                                "value"
                            } else {
                                "text"
                            };
                            case.source = case.source.replace(
                                ": selected, seed: seed)",
                                &format!(": field(value: member(value: alias, name: \"right\"), name: \"{field}\"), seed: seed)"),
                            );
                            assert!(run_conditional::<P>(&case, 10_000).is_ok());
                        }
                    }
                }
            }
        }
        check::<Counter>();
        check::<Panel>();
    }

    #[test]
    fn selected_group_and_early_exit_keep_exact_fuel_at_every_pipeline_cutoff() {
        fn check<P: Profile>() {
            for kind in ["seq", "all"] {
                for left in [false, true] {
                    for early in [false, true] {
                        let case = conditional_case::<P>(kind, left, early);
                        let used = 10_000 - run_conditional::<P>(&case, 10_000).unwrap();
                        for budget in 0..used {
                            assert!(
                                run_conditional::<P>(&case, budget).is_err(),
                                "{kind}/{left}/{early}: {budget} < {used}"
                            );
                        }
                        assert_eq!(run_conditional::<P>(&case, used), Ok(0));
                        assert_eq!(run_conditional::<P>(&case, used + 1), Ok(1));
                    }
                }
            }
        }
        check::<Counter>();
        check::<Panel>();
    }

    #[test]
    fn unselected_group_helpers_are_typed_but_never_evaluated_or_dispatched() {
        fn check<P: Profile>() {
            for kind in ["seq", "all"] {
                let mut case = conditional_case::<P>(kind, true, false);
                assert_eq!(case.source.matches(&case.cold_call).count(), 1);
                fixture::<P>(|host, _scope| {
                    for replacement in [&case.invalid_call, "foreign.unknown()"] {
                        let invalid = case.source.replace(&case.cold_call, replacement);
                        assert!(compile(&invalid, &host).is_err());
                        assert!(host.invoked.borrow().is_empty());
                        assert_eq!(host.serial.get(), 0);
                    }
                    // Even an unselected alternative must retain the exact group mode/export.
                    let other = if kind == "seq" { "all" } else { "seq" };
                    let invalid = case.source.replace(
                        &format!("otherwise: {kind}("),
                        &format!("otherwise: {other}("),
                    );
                    assert_ne!(invalid, case.source);
                    assert!(compile(&invalid, &host).is_err());
                });
                let ordinary = run_conditional::<P>(&case, 10_000).unwrap();
                case.source = case.source.replace(&case.cold_call, &case.cold_fault_call);
                // Static typing accepts this expression; evaluating it would divide by zero.
                assert_eq!(run_conditional::<P>(&case, 10_000), Ok(ordinary));
            }
        }
        check::<Counter>();
        check::<Panel>();
    }

    #[test]
    fn selected_group_reentry_cannot_bypass_revocation_even_when_returning_early() {
        fn check<P: Profile>() {
            for kind in ["seq", "all"] {
                for early in [false, true] {
                    for revoke in 0..4 {
                        let case = conditional_case::<P>(kind, false, early);
                        fixture::<P>(|host, scope| {
                            let node = compile(&case.source, &host).unwrap();
                            let plans = plans(&node, &host);
                            let runtime = Runtime {
                                host: &host,
                                plans: &plans,
                            };
                            let mut vm =
                                LocalVm::new(scope, &node, Vec::new(), Fuel::new(10_000), STACK);
                            for _ in 0..2 {
                                let EffectSessionPoll::Request(active) = vm.poll(&runtime).unwrap()
                                else {
                                    panic!()
                                };
                                let reply = runtime.collect(active).unwrap();
                                vm.try_accept(&reply.identity.clone(), reply, &runtime)
                                    .unwrap();
                            }
                            let before = vm.fuel_remaining();
                            match revoke {
                                0 => host.granted.set(false),
                                1 => host.revision.set(2),
                                2 => host.version.set(32),
                                _ => {
                                    assert!(vm.cancel());
                                }
                            }
                            let end = if revoke == 3 {
                                assert_eq!(vm.fuel_remaining(), before);
                                EffectSessionEnd::Cancelled
                            } else {
                                assert!(vm.poll(&runtime).is_err());
                                EffectSessionEnd::Failed
                            };
                            assert_eq!(vm.status(), EffectSessionStatus::Terminal(end));
                            let fuel = vm.fuel_remaining();
                            assert!(fuel <= before);
                            assert!(
                                matches!(vm.poll(&runtime).unwrap(), EffectSessionPoll::Terminal(actual) if actual == end)
                            );
                            assert_eq!(vm.fuel_remaining(), fuel);
                            assert!(!vm.lease().is_open());
                            assert_eq!(*host.invoked.borrow(), case.inputs[..3]);
                        });
                    }
                }
            }
        }
        check::<Counter>();
        check::<Panel>();
    }

    fn cold<P: MixedProfile>() {
        fixture::<P>(|host, _scope| {
            let original = P::program("seq", "all");
            let mut unknown = original.clone();
            let index = unknown.rfind("right: step(").unwrap();
            unknown.replace_range(
                index..index + "right: step(".len(),
                "right: foreign.missing(",
            );
            assert!(compile(&unknown, &host).is_err());
            host.version.set(32);
            assert!(compile(&original, &host).is_err());
            host.version.set(31);
            host.granted.set(false);
            assert!(compile(&original, &host).is_err());
            assert_eq!(host.serial.get(), 0);
            assert!(host.invoked.borrow().is_empty());
        });
    }
    #[test]
    fn mixed_invalid_late_group_and_cold_policy_are_rejected_before_the_initial_effect() {
        cold::<Counter>();
        cold::<Panel>();
    }
}
