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

#[path = "support/native_checkpoint.rs"]
mod checkpoint_acceptance;

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
    max_branches: group_exports::MAX_GROUP_EXPORT_MEMBERS,
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
    max_functions: leselang_vm::native::syntax::MAX_FUNCTIONS,
    max_parameters: leselang_vm::native::syntax::MAX_FUNCTION_PARAMETERS,
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

type PipelineVm<'ir, 's, P> = leselang_vm::native::Vm<
    'ir,
    's,
    Node,
    <P as Profile>::Value,
    RestoredEffectBindings<'ir, <P as Profile>::Value>,
    Identity,
    Domain<P>,
    Reply<<P as Profile>::Value>,
>;

// The host retains its scope; this moves real VM state, not a fabricated saved
// frame or an external-effect durability claim. LocalVm cannot escape its owner.
fn transfer_pending<'ir, 's, P: Profile>(
    vm: PipelineVm<'ir, 's, P>,
    host: &Host<'_, 's, P>,
) -> PipelineVm<'ir, 's, P> {
    let remaining = vm.fuel_remaining();
    let serial = host.serial.get();
    let invocations = host.invoked.borrow().len();
    let saved = vm.try_into_pending_binding().unwrap();
    assert_eq!(format!("{saved:?}"), "PendingStackedBinding");
    assert_eq!(saved.fuel.remaining(), remaining);
    assert_eq!(saved.limits, STACK);
    let declaration = saved.declaration;
    let frames: Vec<_> = saved
        .continuation
        .into_frames()
        .into_iter()
        .map(|frame| {
            let (name, body, capture) = frame.into_parts();
            effect_stack::SavedEffectReturnFrame {
                name,
                body,
                capture,
            }
        })
        .collect();
    let originals: Vec<_> = frames
        .iter()
        .map(|frame| {
            (
                frame.name.as_ptr(),
                frame.body as *const Node,
                frame.capture.as_ptr(),
            )
        })
        .collect();
    let mut restored = PipelineVm::restore_pending(
        saved.identity,
        declaration,
        frames,
        saved.fuel,
        saved.limits,
        |identity, domain, frames, fuel| {
            host.authorize(identity)?;
            assert_eq!(identity.serial, serial);
            assert!(std::ptr::eq(domain, declaration));
            assert_eq!(fuel.remaining(), remaining);
            assert_eq!(frames.len(), originals.len());
            for (frame, original) in frames.iter().zip(&originals) {
                assert_eq!(
                    (
                        frame.name.as_ptr(),
                        frame.body as *const Node,
                        frame.capture.as_ptr()
                    ),
                    *original
                );
            }
            Ok::<_, Refusal>(())
        },
    )
    .unwrap();
    assert_eq!(host.serial.get(), serial);
    assert_eq!(host.invoked.borrow().len(), invocations);
    assert!(matches!(
        restored.poll(host).unwrap(),
        EffectSessionPoll::Awaiting
    ));
    assert_eq!(restored.fuel_remaining(), remaining);
    restored
}

#[derive(Debug, PartialEq)]
struct TransferOutcome {
    value: Option<ScalarValue>,
    remaining: u64,
    invocations: Vec<ScalarValue>,
    requests: u64,
}

fn transferred_pipeline<P: Profile>(budget: u64, mask: u8) -> TransferOutcome {
    let mut output = None;
    fixture::<P>(|host, _scope| {
        let node = compile(P::PROGRAM, &host).unwrap();
        let mut vm = PipelineVm::new(&node, Vec::new(), Fuel::new(budget), STACK);
        vm = vm.try_into_pending_binding().unwrap_err();
        let mut previous = None;
        let mut value = None;
        for index in 0..=2 {
            match vm.poll(&host) {
                Ok(EffectSessionPoll::Request(dispatch)) => {
                    assert!(index < 2);
                    if mask & (1 << index) != 0 {
                        vm = transfer_pending(vm, &host);
                    }
                    let remaining = vm.fuel_remaining();
                    if let Some(old) = &previous {
                        let input = Reply {
                            value: P::invalid_value(),
                            buffer: Box::new(91),
                        };
                        let address = &*input.buffer as *const u8;
                        let refused = vm.try_accept(old, input, &host).unwrap_err();
                        assert!(matches!(
                            refused.error,
                            effect_session::EffectSessionReplyError::Reply(
                                ReplyAcceptanceError::IdentityMismatch
                            )
                        ));
                        assert_eq!(&*refused.reply.buffer as *const u8, address);
                    }
                    let identity = dispatch.identity.clone();
                    vm.try_accept(&identity, host.dispatch(dispatch).unwrap(), &host)
                        .unwrap();
                    // An accepted receipt must never escape as an awaiting frame.
                    vm = vm.try_into_pending_binding().unwrap_err();
                    assert_eq!(vm.status(), EffectSessionStatus::Ready);
                    assert_eq!(vm.fuel_remaining(), remaining);
                    previous = Some(identity);
                }
                Ok(EffectSessionPoll::Value(PureValue::Scalar(result))) => {
                    assert_eq!(result, P::expected());
                    value = Some(result);
                    break;
                }
                Err(_) => {
                    assert_eq!(
                        vm.status(),
                        EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                    );
                    break;
                }
                _ => panic!("unexpected native pipeline state"),
            }
        }
        let terminal = if value.is_some() {
            EffectSessionEnd::Completed
        } else {
            EffectSessionEnd::Failed
        };
        vm = vm.try_into_pending_binding().unwrap_err();
        assert_eq!(vm.status(), EffectSessionStatus::Terminal(terminal));
        assert!(
            matches!(vm.poll(&host).unwrap(), EffectSessionPoll::Terminal(end) if end == terminal)
        );
        output = Some(TransferOutcome {
            value,
            remaining: vm.fuel_remaining(),
            invocations: host.invoked.borrow().clone(),
            requests: host.serial.get(),
        });
    });
    output.unwrap()
}

#[test]
fn both_hosts_move_actual_pending_vm_frames_and_recover_without_budget_reset_or_replay() {
    fn check<P: Profile>() {
        let baseline = transferred_pipeline::<P>(10_000, 0);
        assert!(baseline.value.is_some());
        assert_eq!(baseline.invocations, P::arguments());
        let used = 10_000 - baseline.remaining;
        for budget in 0..=used + 1 {
            let original = transferred_pipeline::<P>(budget, 0);
            for mask in 1..=3 {
                assert_eq!(
                    transferred_pipeline::<P>(budget, mask),
                    original,
                    "budget={budget} mask={mask}"
                );
            }
        }
    }
    check::<Counter>();
    check::<Panel>();
}

#[test]
fn both_hosts_recheck_policy_and_keep_cancelled_transferred_sessions_terminal() {
    fn check<P: Profile>() {
        for phase in 0..3 {
            fixture::<P>(|host, scope| {
                let node = compile(P::PROGRAM, &host).unwrap();
                let mut vm = PipelineVm::new(&node, Vec::new(), Fuel::new(10_000), STACK);
                let EffectSessionPoll::Request(dispatch) = vm.poll(&host).unwrap() else {
                    panic!()
                };
                vm = transfer_pending(vm, &host);
                let identity = dispatch.identity.clone();
                if phase == 0 {
                    host.granted.set(false);
                    assert!(host.dispatch(dispatch).is_err());
                    assert!(host.invoked.borrow().is_empty());
                } else {
                    let reply = host.dispatch(dispatch).unwrap();
                    if phase == 1 {
                        host.revision.set(2);
                        assert!(vm.try_accept(&identity, reply, &host).is_err());
                        assert_eq!(vm.status(), EffectSessionStatus::Awaiting);
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
                let end = if phase == 2 {
                    EffectSessionEnd::Failed
                } else {
                    EffectSessionEnd::Cancelled
                };
                scope.close();
                vm.cancel();
                let remaining = vm.fuel_remaining();
                vm = vm.try_into_pending_binding().unwrap_err();
                assert_eq!(vm.status(), EffectSessionStatus::Terminal(end));
                assert!(
                    matches!(vm.poll(&host).unwrap(), EffectSessionPoll::Terminal(reason) if reason == end)
                );
                assert_eq!(vm.fuel_remaining(), remaining);
            });
        }
    }
    check::<Counter>();
    check::<Panel>();
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

mod final_pipeline {
    use super::*;

    trait FinalProfile: Profile {
        const DIRECT: &'static str;
        const CAPTURED: &'static str;
        fn returned(value: PureValue<Self::Value>) -> ScalarValue;
        fn expected_final(captured: bool) -> ScalarValue;
    }
    impl FinalProfile for Counter {
        const DIRECT: &'static str = "fn main() = counter.add(amount: 5)";
        const CAPTURED: &'static str = "fn update(n: integer) = counter.add(amount: n)\nfn main() = bind(first: update(n: 5), body: update(n: add(left: first, right: 1)))";
        fn returned(value: PureValue<ScalarValue>) -> ScalarValue {
            let PureValue::Scalar(value) = value else {
                panic!("scalar final reply")
            };
            value
        }
        fn expected_final(captured: bool) -> ScalarValue {
            ScalarValue::Integer(if captured { 11 } else { 5 })
        }
    }
    impl FinalProfile for Panel {
        const DIRECT: &'static str = "fn main() = panel.set_text(text: \"ready\")";
        const CAPTURED: &'static str = "fn render(text: string) = panel.set_text(text: text)\nfn main() = bind(first: render(text: \"ready\"), body: render(text: concat(left: field(value: first, name: \"text\"), right: \"!\")))";
        fn returned(value: PureValue<WidgetAck>) -> ScalarValue {
            let PureValue::Result(value) = value else {
                panic!("structured final reply")
            };
            assert!(value.visible);
            ScalarValue::String(value.text)
        }
        fn expected_final(captured: bool) -> ScalarValue {
            ScalarValue::String(if captured { "ready!" } else { "ready" }.into())
        }
    }

    fn transfer<'ir, 's, P: Profile>(
        vm: PipelineVm<'ir, 's, P>,
        host: &Host<'_, 's, P>,
    ) -> PipelineVm<'ir, 's, P> {
        let vm = vm.try_into_pending_binding().unwrap_err();
        let remaining = vm.fuel_remaining();
        let saved = vm.try_into_pending_final().unwrap();
        let declaration = saved.declaration;
        let serial = host.serial.get();
        let invoked = host.invoked.borrow().len();
        let mut restored = PipelineVm::restore_pending_final(
            saved.identity,
            saved.declaration,
            saved.fuel,
            saved.limits,
            |identity, domain, fuel| {
                host.authorize(identity)?;
                assert!(std::ptr::eq(domain, declaration));
                assert_eq!(identity.serial, serial);
                assert_eq!(fuel.remaining(), remaining);
                Ok::<_, Refusal>(())
            },
        )
        .unwrap();
        assert!(matches!(
            restored.poll(host).unwrap(),
            EffectSessionPoll::Awaiting
        ));
        assert_eq!(host.serial.get(), serial);
        assert_eq!(host.invoked.borrow().len(), invoked);
        assert_eq!(restored.fuel_remaining(), remaining);
        restored
    }

    fn run<P: FinalProfile>(captured: bool, budget: u64, mode: u8) -> TransferOutcome {
        let mut output = None;
        fixture::<P>(|host, _scope| {
            let node = compile(if captured { P::CAPTURED } else { P::DIRECT }, &host).unwrap();
            let mut vm = PipelineVm::new(&node, Vec::new(), Fuel::new(budget), STACK);
            let mut value = None;
            let mut previous = None;
            let last = usize::from(captured);
            for index in 0..=last {
                let dispatch = match vm.poll(&host) {
                    Ok(EffectSessionPoll::Request(dispatch)) => dispatch,
                    Err(_) => break,
                    _ => panic!("expected request"),
                };
                let identity = dispatch.identity.clone();
                let reply = if index != last {
                    vm = vm.try_into_pending_final().unwrap_err();
                    if mode != 0 {
                        vm = transfer_pending(vm, &host);
                    }
                    host.dispatch(dispatch).unwrap()
                } else if mode == 2 {
                    let reply = host.dispatch(dispatch).unwrap();
                    vm = transfer(vm, &host);
                    reply
                } else {
                    if mode == 1 {
                        vm = transfer(vm, &host);
                    }
                    host.dispatch(dispatch).unwrap()
                };
                let fuel = vm.fuel_remaining();
                if let Some(stale) = &previous {
                    let bad = Reply {
                        value: P::invalid_value(),
                        buffer: Box::new(9),
                    };
                    let error = vm.try_accept(stale, bad, &host).unwrap_err();
                    assert!(matches!(
                        error.error,
                        effect_session::EffectSessionReplyError::Reply(
                            ReplyAcceptanceError::IdentityMismatch
                        )
                    ));
                }
                let bad = Reply {
                    value: P::invalid_value(),
                    buffer: Box::new(10),
                };
                let pointer = &*bad.buffer as *const u8;
                let bad = vm.try_accept(&identity, bad, &host).unwrap_err();
                assert_eq!(&*bad.reply.buffer as *const u8, pointer);
                assert_eq!(vm.fuel_remaining(), fuel);
                vm.try_accept(&identity, reply, &host).unwrap();
                vm = vm.try_into_pending_final().unwrap_err();
                vm = vm.try_into_pending_binding().unwrap_err();
                assert_eq!(vm.status(), EffectSessionStatus::Ready);
                assert_eq!(vm.fuel_remaining(), fuel);
                previous = Some(identity);
                if index == last {
                    match vm.poll(&host) {
                        Ok(EffectSessionPoll::Value(result)) => value = Some(P::returned(result)),
                        Err(_) => {}
                        _ => panic!("final request produced a successor"),
                    }
                }
            }
            let end = if value.is_some() {
                EffectSessionEnd::Completed
            } else {
                EffectSessionEnd::Failed
            };
            assert_eq!(vm.status(), EffectSessionStatus::Terminal(end));
            vm = vm.try_into_pending_final().unwrap_err();
            assert!(
                matches!(vm.poll(&host).unwrap(), EffectSessionPoll::Terminal(actual) if actual == end)
            );
            output = Some(TransferOutcome {
                value,
                remaining: vm.fuel_remaining(),
                invocations: host.invoked.borrow().clone(),
                requests: host.serial.get(),
            });
        });
        output.unwrap()
    }

    #[test]
    fn two_hosts_restore_direct_and_captured_final_requests_before_or_after_dispatch_at_every_fuel_cutoff()
     {
        fn check<P: FinalProfile>() {
            for captured in [false, true] {
                let baseline = run::<P>(captured, 10_000, 0);
                assert_eq!(baseline.value, Some(P::expected_final(captured)));
                assert_eq!(baseline.requests, 1 + u64::from(captured));
                assert_eq!(baseline.invocations.len(), 1 + usize::from(captured));
                for budget in 0..=10_001 - baseline.remaining {
                    let expected = run::<P>(captured, budget, 0);
                    for mode in 1..=2 {
                        assert_eq!(
                            run::<P>(captured, budget, mode),
                            expected,
                            "captured={captured} budget={budget} mode={mode}"
                        );
                    }
                }
            }
        }
        check::<Counter>();
        check::<Panel>();
    }

    #[test]
    fn two_hosts_final_restore_and_local_ownership_preserve_revocation_and_cancellation() {
        fn check<P: FinalProfile>() {
            for phase in 0..6 {
                fixture::<P>(|host, scope| {
                    let node = compile(P::DIRECT, &host).unwrap();
                    let mut vm = PipelineVm::new(&node, Vec::new(), Fuel::new(100), STACK);
                    let EffectSessionPoll::Request(dispatch) = vm.poll(&host).unwrap() else {
                        panic!()
                    };
                    let identity = dispatch.identity.clone();
                    let saved = vm.try_into_pending_final().unwrap();
                    let remaining = saved.fuel.remaining();
                    if phase == 0 {
                        host.granted.set(false);
                    }
                    let restored = LocalVm::restore_pending_final(
                        scope,
                        saved.identity,
                        saved.declaration,
                        saved.fuel,
                        saved.limits,
                        |id, _, _| host.authorize(id),
                    );
                    if phase == 0 {
                        assert!(restored.is_err());
                        assert!(host.dispatch(dispatch).is_err());
                        assert!(host.invoked.borrow().is_empty());
                        return;
                    }
                    let mut vm = restored.unwrap();
                    let lease = vm.lease();
                    let reply = host.dispatch(dispatch).unwrap();
                    if phase == 1 {
                        host.revision.set(2);
                        assert!(vm.try_accept(&identity, reply, &host).is_err());
                        assert_eq!(vm.status(), EffectSessionStatus::Awaiting);
                        vm.cancel();
                    } else if phase == 3 {
                        vm.cancel();
                        assert!(vm.try_accept(&identity, reply, &host).is_err());
                    } else {
                        vm.try_accept(&identity, reply, &host).unwrap();
                        if phase == 2 {
                            host.granted.set(false);
                            assert!(vm.poll(&host).is_err());
                        } else if phase == 4 {
                            vm.cancel();
                        } else {
                            let EffectSessionPoll::Value(value) = vm.poll(&host).unwrap() else {
                                panic!()
                            };
                            assert_eq!(P::returned(value), P::expected_final(false));
                        }
                    }
                    let end = match phase {
                        2 => EffectSessionEnd::Failed,
                        5 => EffectSessionEnd::Completed,
                        _ => EffectSessionEnd::Cancelled,
                    };
                    assert_eq!(vm.status(), EffectSessionStatus::Terminal(end));
                    assert!(
                        matches!(vm.poll(&host).unwrap(), EffectSessionPoll::Terminal(actual) if actual == end)
                    );
                    assert_eq!(vm.fuel_remaining(), remaining);
                    assert!(!lease.is_open());
                    assert_eq!(host.invoked.borrow().len(), 1);
                    assert_eq!(host.serial.get(), 1);
                });
            }
        }
        check::<Counter>();
        check::<Panel>();
    }
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
    mod disk_reopen {
        include!("support/native_group_checkpoint.rs");
    }
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
        compile_with_limits(source, host, BATCH.max_branches, 2 * BATCH.max_branches)
    }

    fn compile_with_limits<P: Profile>(
        source: &str,
        host: &Host<'_, '_, P>,
        max_branches: usize,
        max_calls: usize,
    ) -> Result<Node, String> {
        let mut limits = group_program_limits();
        limits.max_branches = max_branches;
        limits.program.expression.max_calls = max_calls;
        compile_with_policy(source, host, limits)
    }

    fn group_program_limits() -> CatalogGroupProgramLimits {
        CatalogGroupProgramLimits {
            program: CatalogProgramLimits {
                expression: CatalogSourceLimits {
                    // Counts both cold helper alternatives, not selected effects.
                    max_calls: 2 * BATCH.max_branches,
                    control: ControlSourceLimits {
                        source: SourceCallLimits {
                            // Group labels share source syntax, not operation arity.
                            max_arguments: source_call::MAX_SOURCE_CALL_ARGUMENTS,
                            ..PROGRAM.expression.control.source
                        },
                        ..PROGRAM.expression.control
                    },
                },
                ..PROGRAM
            },
            max_groups: 4,
            max_branches: BATCH.max_branches,
        }
    }

    fn compile_with_policy<P: Profile>(
        source: &str,
        host: &Host<'_, '_, P>,
        limits: CatalogGroupProgramLimits,
    ) -> Result<Node, String> {
        let tree = parse(source);
        if !tree.diagnostics.is_empty() {
            return Err(format!("{:?}", tree.diagnostics));
        }
        let functions: Vec<_> = tree.function.iter().chain(&tree.helpers).collect();
        compile_catalog_prepared_group_helper_program(
            &functions,
            "main",
            limits,
            &host.source(),
            &Types(host),
        )
        .map(|(node, _)| node)
        .map_err(compile_diagnostic)
    }

    fn compile_diagnostic(error: catalog_source::CatalogSourceError<Refusal>) -> String {
        match error {
            catalog_source::CatalogSourceError::Scalar { span, error } => match *error {
                scalar_source::ScalarSourceError::Native { error, .. } => {
                    format!("{span:?}/scalar: {}", compile_diagnostic(error))
                }
                error => format!("{span:?}/scalar: {error:?}"),
            },
            catalog_source::CatalogSourceError::ComputedProjection(error) => match *error {
                projection_source::ProjectionSourceError::Native { error, .. } => {
                    compile_diagnostic(error)
                }
                error => format!("computed field: {error:?}"),
            },
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
    type SavedSequence<'ir, 's, P> = sequence_evaluation::SavedSequenceMember<
        'ir,
        Identity,
        Domain<P>,
        Dispatch<'s, P>,
        Reply<<P as Profile>::Value>,
    >;
    type SavedParallel<'ir, P> = parallel_evaluation::SavedParallelMember<
        'ir,
        Identity,
        Domain<P>,
        Reply<<P as Profile>::Value>,
    >;
    const BATCH: ParallelEvaluationLimits = ParallelEvaluationLimits {
        max_nodes: 1024,
        max_depth: 64,
        max_branches: group_exports::MAX_GROUP_EXPORT_MEMBERS,
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
    #[derive(Clone, Copy, PartialEq)]
    enum TransferPhase {
        Parent,
        ChildAdmission,
    }
    struct TransferCheckpoint<'scope, Hook> {
        scope: Option<&'scope LocalExecutionScope>,
        hook: Hook,
    }
    impl<Hook: FnMut(TransferPhase) -> Result<(), Refusal>> TransferCheckpoint<'_, Hook> {
        fn run(&mut self, phase: TransferPhase) -> Result<(), Refusal> {
            (self.hook)(phase)
        }
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
            self.collect_with_transfer(active, false)
        }
        fn transfer_sequence(
            &self,
            queue: Sequence<'ir, 's, P>,
            prefix: Vec<Accepted<'ir, P>>,
            plan: &Plan<'ir, P>,
            checkpoint: &mut TransferCheckpoint<
                '_,
                impl FnMut(TransferPhase) -> Result<(), Refusal>,
            >,
        ) -> Result<(Sequence<'ir, 's, P>, Vec<Accepted<'ir, P>>), Refusal> {
            use sequence_evaluation::SavedSequenceReply;
            let status = queue.status();
            let invoked = self.host.invoked.borrow().len();
            let serial = self.host.serial.get();
            let pointers: Vec<_> = prefix
                .iter()
                .map(|row| &*row.as_parts().4.buffer as *const u8)
                .collect();
            let saved = queue.try_into_saved(prefix).map_err(|_| Refusal)?;
            checkpoint.run(TransferPhase::Parent)?;
            let scope = checkpoint.scope;
            let admit = |rows: &[SavedSequence<'_, 's, P>]| {
                checkpoint.run(TransferPhase::ChildAdmission)?;
                self.authorize(&plan.id())?;
                let members = plan.members.borrow();
                for row in rows {
                    let id = row.reply.identity();
                    if members.get(row.index) != Some(id) {
                        return Err(Refusal);
                    }
                    self.authorize(id)?;
                    let domain = match &row.reply {
                        SavedSequenceReply::Prepared(member) => {
                            if member.identity != member.request.identity {
                                return Err(Refusal);
                            }
                            self.admit(&member.request.request)?;
                            if !std::ptr::eq(
                                member.declaration,
                                &member.request.request.schema().result,
                            ) {
                                return Err(Refusal);
                            }
                            member.declaration
                        }
                        SavedSequenceReply::Awaiting { declaration, .. } => *declaration,
                        SavedSequenceReply::Accepted(reply) => {
                            let (_, (), domain, input) = reply.as_parts();
                            if !domain.matches_type(&input.value) {
                                return Err(Refusal);
                            }
                            domain.validate_value(&input.value)?;
                            domain
                        }
                    };
                    if !std::ptr::eq(domain, plan.domain) {
                        return Err(Refusal);
                    }
                }
                Ok(())
            };
            let (queue, prefix) = match scope {
                Some(scope) => Sequence::restore_in_scope(plan.node, saved, BATCH, admit, scope),
                None => Sequence::restore(plan.node, saved, BATCH, admit),
            }
            .map_err(|_| Refusal)?;
            assert_eq!(
                prefix
                    .iter()
                    .map(|row| &*row.as_parts().4.buffer as *const u8)
                    .collect::<Vec<_>>(),
                pointers
            );
            assert_eq!(queue.status(), status);
            assert_eq!(self.host.serial.get(), serial);
            assert_eq!(self.host.invoked.borrow().len(), invoked);
            Ok((queue, prefix))
        }
        fn transfer_parallel(
            &self,
            barrier: Parallel<'ir, P>,
            plan: &Plan<'ir, P>,
            checkpoint: &mut TransferCheckpoint<
                '_,
                impl FnMut(TransferPhase) -> Result<(), Refusal>,
            >,
        ) -> Result<Parallel<'ir, P>, Refusal> {
            use parallel_evaluation::SavedParallelReply;
            let status = barrier.status();
            let invoked = self.host.invoked.borrow().len();
            let serial = self.host.serial.get();
            let rows = barrier.try_into_saved().map_err(|_| Refusal)?;
            checkpoint.run(TransferPhase::Parent)?;
            let pointers: Vec<_> = rows
                .iter()
                .filter_map(|row| match &row.reply {
                    SavedParallelReply::Accepted(reply) => {
                        Some((row.index, &*reply.as_parts().3.buffer as *const u8))
                    }
                    _ => None,
                })
                .collect();
            let scope = checkpoint.scope;
            let admit = |rows: &[SavedParallel<'_, P>]| {
                checkpoint.run(TransferPhase::ChildAdmission)?;
                self.authorize(&plan.id())?;
                let members = plan.members.borrow();
                for row in rows {
                    let identity = row.reply.identity();
                    if members.get(row.index) != Some(identity) {
                        return Err(Refusal);
                    }
                    self.authorize(identity)?;
                    let domain = match &row.reply {
                        SavedParallelReply::Awaiting { declaration, .. } => *declaration,
                        SavedParallelReply::Accepted(accepted) => {
                            let (_, (), domain, reply) = accepted.as_parts();
                            if !domain.matches_type(&reply.value) {
                                return Err(Refusal);
                            }
                            domain.validate_value(&reply.value)?;
                            assert_eq!(
                                &*reply.buffer as *const u8,
                                pointers.iter().find(|(i, _)| *i == row.index).unwrap().1
                            );
                            domain
                        }
                    };
                    if !std::ptr::eq(domain, plan.domain) {
                        return Err(Refusal);
                    }
                }
                Ok(())
            };
            let barrier = match scope {
                Some(scope) => Parallel::restore_in_scope(plan.node, rows, BATCH, admit, scope),
                None => Parallel::restore(plan.node, rows, BATCH, admit),
            }
            .map_err(|_| Refusal)?;
            assert_eq!(barrier.status(), status);
            assert_eq!(self.host.serial.get(), serial);
            assert_eq!(self.host.invoked.borrow().len(), invoked);
            Ok(barrier)
        }
        fn collect_with_transfer(
            &self,
            active: Active<'_, 'ir, 's, P>,
            transfer: bool,
        ) -> Result<Receipt<'ir, P>, Refusal> {
            self.collect_with_checkpoint(active, transfer, None, |_| Ok(()))
        }
        fn collect_with_checkpoint(
            &self,
            active: Active<'_, 'ir, 's, P>,
            transfer: bool,
            scope: Option<&LocalExecutionScope>,
            checkpoint: impl FnMut(TransferPhase) -> Result<(), Refusal>,
        ) -> Result<Receipt<'ir, P>, Refusal> {
            let mut checkpoint = TransferCheckpoint {
                scope,
                hook: checkpoint,
            };
            self.authorize(&active.plan.id())?;
            checkpoint.run(TransferPhase::Parent)?;
            let response = match active.work {
                Work::Atom(request) => {
                    let reply = self.host.dispatch(request)?;
                    checkpoint.run(TransferPhase::Parent)?;
                    Response::Atom(reply)
                }
                Work::Sequence(mut sequence) => {
                    let Node::Group { branches, .. } = active.plan.node else {
                        return Err(Refusal);
                    };
                    let mut rows = Vec::new();
                    if transfer {
                        (*sequence, rows) =
                            self.transfer_sequence(*sequence, rows, active.plan, &mut checkpoint)?;
                    }
                    for expected in 0..branches.len() {
                        let SequencePoll::Request { index, request } =
                            self.publish(&mut sequence)?
                        else {
                            return Err(Refusal);
                        };
                        assert_eq!(index, expected);
                        if transfer {
                            (*sequence, rows) = self.transfer_sequence(
                                *sequence,
                                rows,
                                active.plan,
                                &mut checkpoint,
                            )?;
                        }
                        assert!(matches!(
                            self.publish(&mut sequence)?,
                            SequencePoll::Awaiting { .. }
                        ));
                        let identity = request.identity.clone();
                        let reply = self.host.dispatch(request)?;
                        if transfer {
                            (*sequence, rows) = self.transfer_sequence(
                                *sequence,
                                rows,
                                active.plan,
                                &mut checkpoint,
                            )?;
                        }
                        rows.push(
                            sequence
                                .try_accept(&identity, reply, self.host)
                                .map_err(|_| Refusal)?,
                        );
                        if transfer {
                            (*sequence, rows) = self.transfer_sequence(
                                *sequence,
                                rows,
                                active.plan,
                                &mut checkpoint,
                            )?;
                        }
                    }
                    Response::Group(rows)
                }
                Work::Parallel {
                    mut barrier,
                    requests,
                } => {
                    if transfer {
                        *barrier =
                            self.transfer_parallel(*barrier, active.plan, &mut checkpoint)?;
                    }
                    let mut replies = Vec::new();
                    for request in requests {
                        let identity = request.request.identity.clone();
                        replies.push((
                            request.index,
                            identity,
                            self.host.dispatch(request.request)?,
                        ));
                        if transfer {
                            *barrier =
                                self.transfer_parallel(*barrier, active.plan, &mut checkpoint)?;
                        }
                    }
                    for (index, identity, reply) in replies.into_iter().rev() {
                        barrier
                            .try_accept(index, &identity, reply, self.host)
                            .map_err(|_| Refusal)?;
                        if transfer {
                            *barrier =
                                self.transfer_parallel(*barrier, active.plan, &mut checkpoint)?;
                        }
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
            checkpoint.run(TransferPhase::Parent)?;
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
                            max_branches: BATCH.max_branches,
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

    mod coupled_transfer {
        use super::*;
        use effect_stack::SavedEffectReturnFrame;
        use std::panic::{AssertUnwindSafe, catch_unwind};

        type Vm<'r, 'ir, P> = leselang_vm::native::Vm<
            'ir,
            'r,
            Node,
            View<'ir, P>,
            RestoredEffectBindings<'ir, View<'ir, P>>,
            Identity,
            Plan<'ir, P>,
            Receipt<'ir, P>,
        >;
        type Frame<'ir, P> =
            SavedEffectReturnFrame<'ir, Node, RestoredEffectBindings<'ir, View<'ir, P>>>;

        #[derive(Clone, Copy, Debug)]
        enum Fault {
            None,
            Closed,
            Revoked,
            Stale,
            WrongRoot,
            Capture,
            Shape,
            Unwind,
            ChildRefused,
            ChildRevoked,
            ChildUnwind,
        }
        impl Fault {
            fn is_child(self) -> bool {
                matches!(
                    self,
                    Self::ChildRefused | Self::ChildRevoked | Self::ChildUnwind
                )
            }
        }
        #[derive(Default)]
        struct Cuts {
            parent: usize,
            child: usize,
        }

        // Pointer fingerprints observe moves, not a portable codec or authenticator.
        // Real admission below also checks live policy and retained native results.
        fn fingerprint<P: Profile>(frames: &[Frame<'_, P>]) -> Vec<usize> {
            let mut pointers = Vec::new();
            for frame in frames {
                pointers.extend([
                    frame.name.as_ptr() as usize,
                    frame.body as *const Node as usize,
                    frame.capture.as_ptr() as usize,
                    frame.capture.len(),
                ]);
                for (name, value) in &frame.capture {
                    pointers.push(name.as_ptr() as usize);
                    if let PureValue::Result(View::Group(_, group)) = value {
                        pointers.push(Rc::as_ptr(group) as usize);
                        for row in group.members() {
                            pointers.push(&*row.as_parts().4.buffer as *const u8 as usize);
                        }
                    }
                }
            }
            pointers
        }

        fn handoff<'r, 'ir: 'r, P: Profile>(
            slot: &mut Option<Vm<'r, 'ir, P>>,
            scope: &LocalExecutionScope,
            runtime: &Runtime<'r, 'ir, '_, '_, P>,
            plan: &'r Plan<'ir, P>,
            fault: Fault,
        ) -> Result<(), Refusal> {
            let vm = slot.take().unwrap();
            let remaining = vm.fuel_remaining();
            let serial = runtime.host.serial.get();
            let invoked = runtime.host.invoked.borrow().len();
            let (mut identity, mut declaration, mut frames, fuel, limits, final_request) =
                match vm.try_into_pending_binding() {
                    Ok(pending) => {
                        let frames = pending
                            .continuation
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
                        (
                            pending.identity,
                            pending.declaration,
                            frames,
                            pending.fuel,
                            pending.limits,
                            false,
                        )
                    }
                    Err(vm) => {
                        let pending = vm.try_into_pending_final().unwrap();
                        (
                            pending.identity,
                            pending.declaration,
                            Vec::new(),
                            pending.fuel,
                            pending.limits,
                            true,
                        )
                    }
                };
            assert_eq!(limits, STACK);
            assert!(identity == plan.id());
            assert!(std::ptr::eq(declaration, plan));
            let original = fingerprint(&frames);
            match fault {
                Fault::Closed => {
                    scope.close();
                }
                Fault::Revoked => runtime.host.granted.set(false),
                Fault::Stale => identity.revision += 1,
                Fault::WrongRoot => {
                    declaration = runtime
                        .plans
                        .iter()
                        .find(|candidate| !std::ptr::eq(*candidate, plan))
                        .unwrap();
                }
                Fault::Capture => {
                    frames[0].name = "foreign-capture-site";
                }
                Fault::Shape => frames[0].name = "",
                Fault::None | Fault::Unwind => {}
                Fault::ChildRefused | Fault::ChildRevoked | Fault::ChildUnwind => {
                    panic!("child fault must run inside collector admission");
                }
            }
            let admissions = Cell::new(0);
            let admit = |identity: &Identity,
                         declaration: &Plan<'ir, P>,
                         frames: &[Frame<'ir, P>],
                         fuel: &Fuel| {
                admissions.set(admissions.get() + 1);
                let admitted = (|| {
                    runtime.authorize(identity)?;
                    if *identity != plan.id()
                        || !std::ptr::eq(declaration, plan)
                        || fuel.remaining() != remaining
                        || fingerprint(frames) != original
                    {
                        return Err(Refusal);
                    }
                    for frame in frames {
                        for (_, value) in &frame.capture {
                            match value {
                                PureValue::Result(View::Atom(id, value)) => {
                                    runtime.authorize(id)?;
                                    P::validate(value)?;
                                }
                                PureValue::Result(View::Group(id, group)) => {
                                    runtime.authorize(id)?;
                                    let source = runtime
                                        .plans
                                        .iter()
                                        .find(|p| p.identity.borrow().as_ref() == Some(id))
                                        .ok_or(Refusal)?;
                                    source.validate_rows(group.members())?;
                                    for row in group.members() {
                                        runtime.authorize(row.as_parts().2)?;
                                    }
                                }
                                PureValue::Scalar(_) => {}
                            }
                        }
                    }
                    Ok(())
                })();
                assert!(
                    !matches!(fault, Fault::Unwind),
                    "injected native admission unwind"
                );
                admitted
            };
            let restored = if final_request {
                assert!(frames.is_empty());
                Vm::restore_pending_final_in_scope(
                    identity,
                    declaration,
                    fuel,
                    limits,
                    |identity, declaration, fuel| admit(identity, declaration, &[], fuel),
                    scope,
                )
            } else {
                Vm::restore_pending_in_scope(
                    identity,
                    declaration,
                    frames,
                    fuel,
                    limits,
                    admit,
                    scope,
                )
            };
            assert_eq!(
                admissions.get(),
                usize::from(!matches!(fault, Fault::Closed | Fault::Shape))
            );
            let mut restored = match restored {
                Ok(vm) => vm,
                Err(_) => {
                    assert!(!scope.is_open());
                    return Err(Refusal);
                }
            };
            assert!(matches!(
                restored.poll_in_scope(runtime, scope).unwrap(),
                EffectSessionPoll::Awaiting
            ));
            assert_eq!(restored.fuel_remaining(), remaining);
            assert_eq!(runtime.host.serial.get(), serial);
            assert_eq!(runtime.host.invoked.borrow().len(), invoked);
            *slot = Some(restored);
            Ok(())
        }

        fn run<P: MixedProfile>(
            first: &str,
            last: &str,
            budget: u64,
            fault: Option<(usize, Fault)>,
        ) -> (TransferOutcome, Cuts, bool) {
            run_pipeline::<P>(
                &Pipeline {
                    source: P::program(first, last),
                    output: P::result(first),
                    inputs: P::inputs(first),
                    groups: vec![false, true, false, true],
                },
                budget,
                true,
                fault,
            )
        }

        struct Pipeline {
            source: String,
            output: ScalarValue,
            inputs: Vec<ScalarValue>,
            groups: Vec<bool>,
        }
        impl From<ConditionalCase> for Pipeline {
            fn from(case: ConditionalCase) -> Self {
                let mut groups = vec![false, true];
                if case.inputs.len() == 4 {
                    groups.push(false);
                }
                Self {
                    source: case.source,
                    output: case.output,
                    inputs: case.inputs,
                    groups,
                }
            }
        }

        fn run_pipeline<P: Profile>(
            case: &Pipeline,
            budget: u64,
            transfer: bool,
            fault: Option<(usize, Fault)>,
        ) -> (TransferOutcome, Cuts, bool) {
            let mut output = None;
            fixture::<P>(|host, scope| {
                let sibling = LocalExecutionScope::new();
                let node = compile(&case.source, &host)
                    .unwrap_or_else(|error| panic!("{error}\n{}", case.source));
                let plans = plans(&node, &host);
                let runtime = Runtime {
                    host: &host,
                    plans: &plans,
                };
                let mut owner =
                    scope.bind(Some(Vm::new(&node, Vec::new(), Fuel::new(budget), STACK)));
                let mut checkpoints = Cuts::default();
                let mut refused = false;
                let mut remaining = budget;
                let mut result = None;
                for phase in 0..=case.groups.len() {
                    let (scope, slot) = owner.as_parts_mut();
                    let polled = slot.as_mut().unwrap().poll_in_scope(&runtime, scope);
                    remaining = owner.value().as_ref().unwrap().fuel_remaining();
                    match polled {
                        Ok(EffectSessionPoll::Request(active)) => {
                            assert!(phase < case.groups.len());
                            assert_eq!(
                                matches!(active.work, Work::Sequence(_) | Work::Parallel { .. }),
                                case.groups[phase]
                            );
                            let plan = active.plan;
                            let mut child_failed = false;
                            let mut failure_counts = None;
                            let (scope, slot) = owner.as_parts_mut();
                            let collected = catch_unwind(AssertUnwindSafe(|| {
                                runtime.collect_with_checkpoint(
                                    active,
                                    transfer,
                                    Some(scope),
                                    |phase| {
                                        if !transfer {
                                            assert!(fault.is_none());
                                            return Ok(());
                                        }
                                        if phase == TransferPhase::ChildAdmission {
                                            let injected = fault
                                                .filter(|(cut, fault)| {
                                                    fault.is_child() && *cut == checkpoints.child
                                                })
                                                .map_or(Fault::None, |(_, fault)| fault);
                                            checkpoints.child += 1;
                                            assert!(scope.is_open());
                                            assert_eq!(
                                                slot.as_ref().unwrap().status(),
                                                EffectSessionStatus::Awaiting
                                            );
                                            child_failed = injected.is_child();
                                            if child_failed {
                                                failure_counts = Some((
                                                    host.invoked.borrow().len(),
                                                    host.serial.get(),
                                                ));
                                            }
                                            match injected {
                                                Fault::ChildRefused => return Err(Refusal),
                                                Fault::ChildRevoked => {
                                                    scope.close();
                                                }
                                                Fault::ChildUnwind => {
                                                    panic!("injected child admission unwind");
                                                }
                                                Fault::None => {}
                                                _ => unreachable!(),
                                            }
                                            return Ok(());
                                        }
                                        let injected = fault
                                            .filter(|(cut, fault)| {
                                                !fault.is_child() && *cut == checkpoints.parent
                                            })
                                            .map_or(Fault::None, |(_, fault)| fault);
                                        checkpoints.parent += 1;
                                        if !matches!(injected, Fault::None) {
                                            failure_counts = Some((
                                                host.invoked.borrow().len(),
                                                host.serial.get(),
                                            ));
                                        }
                                        // Repeated replacement while the child collector's
                                        // original saved rows are parked on the same stack.
                                        handoff(slot, scope, &runtime, plan, injected)?;
                                        handoff(slot, scope, &runtime, plan, Fault::None)
                                    },
                                )
                            }));
                            let reply = match collected {
                                Ok(Ok(reply)) => reply,
                                Ok(Err(_)) | Err(_) => {
                                    assert!(!owner.is_open());
                                    assert_eq!(owner.value().is_some(), child_failed);
                                    assert!(runtime.authorize(&plan.id()).is_err());
                                    if let Some((invocations, requests)) = failure_counts {
                                        assert_eq!(host.invoked.borrow().len(), invocations);
                                        assert_eq!(host.serial.get(), requests);
                                    }
                                    let (scope, slot) = owner.as_parts_mut();
                                    if let Some(vm) = slot.as_mut() {
                                        // Revocation does not pretend to cancel or destroy
                                        // the restored parent. Reconcile it explicitly.
                                        assert_eq!(vm.status(), EffectSessionStatus::Awaiting);
                                        assert_eq!(vm.fuel_remaining(), remaining);
                                        assert!(vm.cancel_in_scope(scope));
                                    }
                                    refused = true;
                                    break;
                                }
                            };
                            let pointer = &*reply.buffer as *const u8;
                            let identity = reply.identity.clone();
                            let mut stale = identity.clone();
                            stale.serial = 0;
                            let (scope, slot) = owner.as_parts_mut();
                            let vm = slot.as_mut().unwrap();
                            let reply = vm
                                .try_accept_in_scope(&stale, reply, &runtime, scope)
                                .unwrap_err()
                                .reply;
                            assert_eq!(&*reply.buffer as *const u8, pointer);
                            assert_eq!(vm.status(), EffectSessionStatus::Awaiting);
                            assert_eq!(vm.fuel_remaining(), remaining);
                            assert!(scope.is_open());
                            let reply = if let Some(cold) = plans.iter().find(|candidate| {
                                matches!(plan.node, Node::Group { .. })
                                    && matches!(candidate.node, Node::Group { .. })
                                    && candidate.identity.borrow().is_none()
                            }) {
                                let mut reply = reply;
                                let original = reply.node;
                                reply.node = cold.node;
                                let mut reply = vm
                                    .try_accept_in_scope(&identity, reply, &runtime, scope)
                                    .unwrap_err()
                                    .reply;
                                assert_eq!(&*reply.buffer as *const u8, pointer);
                                assert_eq!(vm.status(), EffectSessionStatus::Awaiting);
                                assert_eq!(vm.fuel_remaining(), remaining);
                                assert!(scope.is_open());
                                assert!(cold.members.borrow().is_empty());
                                reply.node = original;
                                reply
                            } else {
                                reply
                            };
                            vm.try_accept_in_scope(&identity, reply, &runtime, scope)
                                .unwrap();
                        }
                        Ok(EffectSessionPoll::Value(PureValue::Scalar(value))) => {
                            assert_eq!(phase, case.groups.len());
                            assert_eq!(value, case.output);
                            assert_eq!(*host.invoked.borrow(), case.inputs);
                            result = Some(value);
                            assert_eq!(
                                owner.value().as_ref().unwrap().status(),
                                EffectSessionStatus::Terminal(EffectSessionEnd::Completed)
                            );
                            break;
                        }
                        Err(_) => {
                            assert_eq!(
                                owner.value().as_ref().unwrap().status(),
                                EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                            );
                            break;
                        }
                        _ => panic!("unexpected coupled pipeline state"),
                    }
                }
                assert!(!owner.is_open());
                let invocations = host.invoked.borrow().clone();
                assert_eq!(invocations, case.inputs[..invocations.len()]);
                let active_groups = plans
                    .iter()
                    .filter(|plan| matches!(plan.node, Node::Group { .. }))
                    .filter(|plan| {
                        if plan.identity.borrow().is_none() {
                            assert!(plan.members.borrow().is_empty());
                            false
                        } else {
                            true
                        }
                    })
                    .count();
                if result.is_some() {
                    assert_eq!(
                        active_groups,
                        case.groups.iter().filter(|group| **group).count()
                    );
                    assert_eq!(
                        host.serial.get(),
                        (case.inputs.len() + active_groups) as u64
                    );
                }
                let requests = host.serial.get();
                let (scope, slot) = owner.as_parts_mut();
                if let Some(vm) = slot.as_mut() {
                    let status = vm.status();
                    assert!(!vm.cancel_in_scope(scope));
                    assert!(matches!(
                        vm.poll_in_scope(&runtime, scope).unwrap(),
                        EffectSessionPoll::Terminal(_)
                    ));
                    assert_eq!(vm.status(), status);
                    assert_eq!(vm.fuel_remaining(), remaining);
                }
                assert!(!host.lease.is_open());
                assert!(sibling.is_open());
                assert_eq!(host.serial.get(), requests);
                assert_eq!(*host.invoked.borrow(), invocations);
                output = Some((
                    TransferOutcome {
                        value: result,
                        remaining,
                        invocations,
                        requests,
                    },
                    checkpoints,
                    refused,
                ));
            });
            output.unwrap()
        }

        fn selected_cases<P: Profile>(kind: &str, left: bool, early: bool) -> Vec<ConditionalCase> {
            let mut cases = vec![conditional_case::<P>(kind, left, early)];
            for operand in [false, true] {
                if early && operand {
                    continue;
                }
                for form in 0..4 {
                    cases.push(computed_projection_case::<P>(
                        kind, left, early, form, operand,
                    ));
                }
                for form in 0..3 {
                    cases.push(iteration_case::<P>(kind, left, early, form, operand));
                }
            }
            cases
        }

        #[test]
        fn selected_result_control_flow_survives_coupled_parent_and_collector_replacement() {
            fn check<P: Profile>() {
                let mut count = 0;
                for kind in ["seq", "all"] {
                    for left in [false, true] {
                        for early in [false, true] {
                            for case in selected_cases::<P>(kind, left, early) {
                                let local = run_conditional::<P>(&case, 10_000).unwrap();
                                let case = Pipeline::from(case);
                                let (live, _, refused) =
                                    run_pipeline::<P>(&case, 10_000, false, None);
                                assert!(!refused);
                                assert_eq!(live.remaining, local);
                                let (restored, cuts, refused) =
                                    run_pipeline::<P>(&case, 10_000, true, None);
                                assert!(!refused);
                                assert_eq!(restored, live);
                                assert_eq!(restored.value, Some(case.output));
                                assert_eq!(cuts.child, if kind == "seq" { 7 } else { 5 });
                                assert_eq!(cuts.parent, cuts.child + 5 + if early { 0 } else { 3 });
                                count += 1;
                            }
                        }
                    }
                }
                assert_eq!(count, 92);
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn selected_result_replacement_preserves_failure_prefixes_at_every_fuel_cutoff() {
            fn check<P: Profile>() {
                for kind in ["seq", "all"] {
                    let cases =
                        std::iter::once(computed_projection_case::<P>(kind, true, false, 3, true))
                            .chain(
                                (0..3)
                                    .map(|form| iteration_case::<P>(kind, true, false, form, true)),
                            );
                    for (form, case) in cases.enumerate() {
                        let case = Pipeline::from(case);
                        let (live, _, _) = run_pipeline::<P>(&case, 10_000, false, None);
                        assert!(live.value.is_some());
                        let used = 10_000 - live.remaining;
                        for budget in 0..=used + 1 {
                            let (restored, _, refused) =
                                run_pipeline::<P>(&case, budget, true, None);
                            assert!(!refused);
                            assert_eq!(
                                restored,
                                run_pipeline::<P>(&case, budget, false, None).0,
                                "{kind}/{form}/{budget}"
                            );
                            assert_eq!(restored.value.is_some(), budget >= used);
                            if budget >= used {
                                assert_eq!(restored.remaining, budget - used);
                            }
                        }
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn selected_result_restore_refusals_block_tail_work_at_every_parent_and_child_cut() {
            fn check<P: Profile>() {
                let mut count = 0;
                for kind in ["seq", "all"] {
                    for early in [false, true] {
                        let case =
                            Pipeline::from(iteration_case::<P>(kind, false, early, 2, !early));
                        let (baseline, cuts, _) = run_pipeline::<P>(&case, 10_000, true, None);
                        assert!(baseline.value.is_some());
                        for fault in [
                            Fault::Closed,
                            Fault::Revoked,
                            Fault::Stale,
                            Fault::WrongRoot,
                            Fault::Capture,
                            Fault::Shape,
                            Fault::Unwind,
                            Fault::ChildRefused,
                            Fault::ChildRevoked,
                            Fault::ChildUnwind,
                        ] {
                            let frontier = if fault.is_child() {
                                cuts.child
                            } else {
                                cuts.parent
                            };
                            for cut in 0..frontier {
                                let (outcome, seen, refused) =
                                    run_pipeline::<P>(&case, 10_000, true, Some((cut, fault)));
                                assert!(refused, "{kind}/{early}/{cut}/{fault:?}");
                                assert!(outcome.value.is_none());
                                assert_eq!(
                                    if fault.is_child() {
                                        seen.child
                                    } else {
                                        seen.parent
                                    },
                                    cut + 1
                                );
                                count += 1;
                            }
                        }
                    }
                }
                assert_eq!(count, 422);
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[derive(Clone, Copy, Debug, PartialEq)]
        enum CancelFinal {
            Pending,
            Collected,
            Accepted,
        }
        #[derive(Debug, PartialEq)]
        struct FinalOutcome {
            rows: Option<Vec<ScalarValue>>,
            remaining: u64,
            invocations: Vec<ScalarValue>,
            requests: u64,
            end: EffectSessionStatus,
        }

        fn final_program<P: Profile>(kind: &str, prefix: bool) -> String {
            let argument = match P::ARGUMENT {
                ScalarType::Integer => "1",
                ScalarType::String => "\"leaf\"",
                _ => unreachable!(),
            };
            let group = if kind == "repeat" {
                "repeat(times: 3, body: step())".to_string()
            } else {
                format!("{kind}(first: step(), second: step(), third: step())")
            };
            let main = if prefix {
                "bind(seed: step(), body: batch())"
            } else {
                "batch()"
            };
            format!(
                "fn step() = {}({}: {argument})\nfn batch() = {group}\nfn main() = {main}",
                P::OPERATION,
                P::PARAMETER,
            )
        }

        fn run_final<P: Profile>(
            kind: &str,
            prefix: bool,
            budget: u64,
            transfer: bool,
            fault: Option<(usize, Fault)>,
            cancel: Option<CancelFinal>,
        ) -> (FinalOutcome, Cuts, bool) {
            let mut output = None;
            fixture::<P>(|host, scope| {
                let sibling = LocalExecutionScope::new();
                let node = compile(&final_program::<P>(kind, prefix), &host).unwrap();
                let plans = plans(&node, &host);
                let runtime = Runtime {
                    host: &host,
                    plans: &plans,
                };
                let mut owner =
                    scope.bind(Some(Vm::new(&node, Vec::new(), Fuel::new(budget), STACK)));
                let mut cuts = Cuts::default();
                let mut refused = false;
                let mut rows = None;
                let mut remaining = budget;
                for phase in 0..=usize::from(prefix) + 1 {
                    let (scope, slot) = owner.as_parts_mut();
                    let polled = slot.as_mut().unwrap().poll_in_scope(&runtime, scope);
                    remaining = owner.value().as_ref().unwrap().fuel_remaining();
                    match polled {
                        Ok(EffectSessionPoll::Request(active)) => {
                            let final_request = matches!(active.plan.node, Node::Group { .. });
                            assert_eq!(final_request, phase == usize::from(prefix));
                            if final_request {
                                let vm = owner.value_mut().take().unwrap();
                                // A final group has no invented binding continuation.
                                *owner.value_mut() =
                                    Some(vm.try_into_pending_binding().unwrap_err());
                            }
                            if final_request && cancel == Some(CancelFinal::Pending) {
                                if transfer {
                                    let (scope, slot) = owner.as_parts_mut();
                                    handoff(slot, scope, &runtime, active.plan, Fault::None)
                                        .unwrap();
                                }
                                let (scope, slot) = owner.as_parts_mut();
                                assert!(slot.as_mut().unwrap().cancel_in_scope(scope));
                                drop(active);
                                break;
                            }
                            let plan = active.plan;
                            let (scope, slot) = owner.as_parts_mut();
                            let mut child_failed = false;
                            let mut failure_counts = None;
                            let collected = catch_unwind(AssertUnwindSafe(|| {
                                runtime.collect_with_checkpoint(
                                    active,
                                    transfer,
                                    Some(scope),
                                    |phase| {
                                        if !transfer {
                                            return Ok(());
                                        }
                                        let child = phase == TransferPhase::ChildAdmission;
                                        let cut = if child {
                                            &mut cuts.child
                                        } else {
                                            &mut cuts.parent
                                        };
                                        let injected = fault
                                            .filter(|(at, f)| *at == *cut && f.is_child() == child)
                                            .map_or(Fault::None, |(_, f)| f);
                                        *cut += 1;
                                        if !matches!(injected, Fault::None) {
                                            failure_counts = Some((
                                                host.invoked.borrow().len(),
                                                host.serial.get(),
                                            ));
                                        }
                                        if child {
                                            assert_eq!(
                                                slot.as_ref().unwrap().status(),
                                                EffectSessionStatus::Awaiting
                                            );
                                            child_failed = injected.is_child();
                                            match injected {
                                                Fault::ChildRefused => return Err(Refusal),
                                                Fault::ChildRevoked => {
                                                    scope.close();
                                                }
                                                Fault::ChildUnwind => {
                                                    panic!("final collector admission unwind")
                                                }
                                                Fault::None => {}
                                                _ => unreachable!(),
                                            }
                                            return Ok(());
                                        }
                                        handoff(slot, scope, &runtime, plan, injected)?;
                                        handoff(slot, scope, &runtime, plan, Fault::None)
                                    },
                                )
                            }));
                            let reply = match collected {
                                Ok(Ok(reply)) => reply,
                                Ok(Err(_)) | Err(_) => {
                                    assert!(!owner.is_open());
                                    assert_eq!(owner.value().is_some(), child_failed);
                                    assert!(runtime.authorize(&plan.id()).is_err());
                                    let (invocations, requests) = failure_counts.unwrap();
                                    assert_eq!(host.invoked.borrow().len(), invocations);
                                    assert_eq!(host.serial.get(), requests);
                                    let (scope, slot) = owner.as_parts_mut();
                                    if let Some(vm) = slot.as_mut() {
                                        assert_eq!(vm.status(), EffectSessionStatus::Awaiting);
                                        assert_eq!(vm.fuel_remaining(), remaining);
                                        assert!(vm.cancel_in_scope(scope));
                                    }
                                    refused = true;
                                    break;
                                }
                            };
                            if final_request && cancel == Some(CancelFinal::Collected) {
                                let (scope, slot) = owner.as_parts_mut();
                                assert!(slot.as_mut().unwrap().cancel_in_scope(scope));
                                drop(reply);
                                break;
                            }
                            let identity = reply.identity.clone();
                            let pointer = &*reply.buffer as *const u8;
                            let mut stale = identity.clone();
                            stale.serial = 0;
                            let (scope, slot) = owner.as_parts_mut();
                            let vm = slot.as_mut().unwrap();
                            let reply = vm
                                .try_accept_in_scope(&stale, reply, &runtime, scope)
                                .unwrap_err()
                                .reply;
                            assert_eq!(&*reply.buffer as *const u8, pointer);
                            assert_eq!(vm.status(), EffectSessionStatus::Awaiting);
                            assert_eq!(vm.fuel_remaining(), remaining);
                            let reply = if final_request {
                                let mut reply = reply;
                                let Response::Group(rows) = &mut reply.response else {
                                    panic!("final group receipt")
                                };
                                rows.swap(0, 1);
                                let mut reply = vm
                                    .try_accept_in_scope(&identity, reply, &runtime, scope)
                                    .unwrap_err()
                                    .reply;
                                assert_eq!(&*reply.buffer as *const u8, pointer);
                                assert_eq!(vm.status(), EffectSessionStatus::Awaiting);
                                assert_eq!(vm.fuel_remaining(), remaining);
                                let Response::Group(rows) = &mut reply.response else {
                                    unreachable!()
                                };
                                rows.swap(0, 1);
                                reply
                            } else {
                                reply
                            };
                            assert!(scope.is_open());
                            vm.try_accept_in_scope(&identity, reply, &runtime, scope)
                                .unwrap();
                            assert_eq!(vm.status(), EffectSessionStatus::Ready);
                            assert_eq!(vm.fuel_remaining(), remaining);
                            if final_request && cancel == Some(CancelFinal::Accepted) {
                                assert!(vm.cancel_in_scope(scope));
                                break;
                            }
                        }
                        Ok(EffectSessionPoll::Value(PureValue::Result(View::Group(_, group)))) => {
                            assert_eq!(phase, usize::from(prefix) + 1);
                            assert_eq!(group.members().len(), 3);
                            let mut values = Vec::new();
                            for row in group.members() {
                                values.push(match P::bind(row.as_parts().4.value.clone()) {
                                    PureValue::Scalar(value) => value,
                                    PureValue::Result(value) => P::field(&value, "text").unwrap(),
                                });
                            }
                            rows = Some(values);
                            break;
                        }
                        Err(_) => {
                            assert_eq!(
                                owner.value().as_ref().unwrap().status(),
                                EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                            );
                            break;
                        }
                        _ => panic!("unexpected final-group state"),
                    }
                }
                assert!(!owner.is_open());
                let (scope, slot) = owner.as_parts_mut();
                let end = if let Some(vm) = slot.as_mut() {
                    let status = vm.status();
                    assert!(!vm.cancel_in_scope(scope));
                    assert!(matches!(
                        vm.poll_in_scope(&runtime, scope).unwrap(),
                        EffectSessionPoll::Terminal(_)
                    ));
                    assert_eq!(vm.status(), status);
                    assert_eq!(vm.fuel_remaining(), remaining);
                    status
                } else {
                    assert!(refused);
                    EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                };
                assert!(!host.lease.is_open());
                assert!(sibling.is_open());
                output = Some((
                    FinalOutcome {
                        rows,
                        remaining,
                        invocations: host.invoked.borrow().clone(),
                        requests: host.serial.get(),
                        end,
                    },
                    cuts,
                    refused,
                ));
            });
            output.unwrap()
        }

        #[test]
        fn two_hosts_restore_final_seq_all_repeat_with_original_collectors_at_every_fuel_cutoff() {
            fn check<P: Profile>() {
                for kind in ["seq", "all", "repeat"] {
                    for prefix in [false, true] {
                        let (baseline, _, _) =
                            run_final::<P>(kind, prefix, 10_000, false, None, None);
                        assert!(baseline.rows.is_some());
                        assert_eq!(baseline.invocations.len(), 3 + usize::from(prefix));
                        assert_eq!(baseline.requests, 4 + u64::from(prefix));
                        let (moved, cuts, refused) =
                            run_final::<P>(kind, prefix, 10_000, true, None, None);
                        assert_eq!(moved, baseline);
                        assert!(!refused);
                        assert!(cuts.parent > 5 && cuts.child > 3);
                        for budget in 0..=10_001 - baseline.remaining {
                            assert_eq!(
                                run_final::<P>(kind, prefix, budget, true, None, None).0,
                                run_final::<P>(kind, prefix, budget, false, None, None).0,
                                "{kind} prefix={prefix} budget={budget}",
                            );
                        }
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn final_parent_and_child_restore_refusal_close_the_same_owner_at_every_frontier() {
            fn check<P: Profile>() {
                for kind in ["seq", "all", "repeat"] {
                    for prefix in [false, true] {
                        let (_, cuts, _) = run_final::<P>(kind, prefix, 10_000, true, None, None);
                        for fault in [
                            Fault::Closed,
                            Fault::Revoked,
                            Fault::Stale,
                            Fault::WrongRoot,
                            Fault::Unwind,
                            Fault::ChildRefused,
                            Fault::ChildRevoked,
                            Fault::ChildUnwind,
                        ] {
                            let count = if fault.is_child() {
                                cuts.child
                            } else {
                                cuts.parent
                            };
                            for cut in 0..count {
                                let (outcome, seen, refused) = run_final::<P>(
                                    kind,
                                    prefix,
                                    10_000,
                                    true,
                                    Some((cut, fault)),
                                    None,
                                );
                                assert!(refused, "{kind} prefix={prefix} {cut} {fault:?}");
                                assert!(outcome.rows.is_none());
                                assert_eq!(
                                    if fault.is_child() {
                                        seen.child
                                    } else {
                                        seen.parent
                                    },
                                    cut + 1
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
        fn restored_final_groups_cancel_pending_collected_and_accepted_results_without_projection()
        {
            fn check<P: Profile>() {
                for kind in ["seq", "all", "repeat"] {
                    for prefix in [false, true] {
                        for point in [
                            CancelFinal::Pending,
                            CancelFinal::Collected,
                            CancelFinal::Accepted,
                        ] {
                            let (actual, _, refused) =
                                run_final::<P>(kind, prefix, 10_000, true, None, Some(point));
                            assert_eq!(
                                actual,
                                run_final::<P>(kind, prefix, 10_000, false, None, Some(point)).0
                            );
                            assert!(!refused);
                            assert!(actual.rows.is_none());
                            assert_eq!(
                                actual.end,
                                EffectSessionStatus::Terminal(EffectSessionEnd::Cancelled)
                            );
                            assert_eq!(
                                actual.invocations.len(),
                                usize::from(prefix)
                                    + if point == CancelFinal::Pending { 0 } else { 3 }
                            );
                        }
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn coupled_vm_and_collectors_keep_original_captures_and_budget_at_every_cutoff() {
            fn check<P: MixedProfile>() {
                for first in ["seq", "all", "repeat"] {
                    for last in ["seq", "all"] {
                        let baseline = run_with_transfer::<P>(first, last, 10_000, false);
                        let (transferred, cuts, refused) = run::<P>(first, last, 10_000, None);
                        assert!(!refused);
                        assert!(cuts.parent > 10);
                        assert!(cuts.child > 0);
                        assert_eq!(transferred, baseline);
                        for budget in 0..=10_001 - baseline.remaining {
                            let (transferred, _, refused) = run::<P>(first, last, budget, None);
                            assert!(!refused);
                            assert_eq!(
                                transferred,
                                run_with_transfer::<P>(first, last, budget, false),
                                "{first}/{last} budget={budget}"
                            );
                        }
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn coupled_vm_and_collector_transfer_refuses_every_revoked_or_corrupt_cut() {
            fn check<P: MixedProfile>() {
                for first in ["seq", "all", "repeat"] {
                    for last in ["seq", "all"] {
                        let (_, cuts, _) = run::<P>(first, last, 10_000, None);
                        for cut in 0..cuts.parent {
                            for fault in [
                                Fault::Closed,
                                Fault::Revoked,
                                Fault::Stale,
                                Fault::WrongRoot,
                                Fault::Capture,
                                Fault::Shape,
                            ] {
                                let (outcome, seen, refused) =
                                    run::<P>(first, last, 10_000, Some((cut, fault)));
                                assert!(refused, "{first}/{last} cut={cut} {fault:?}");
                                assert_eq!(seen.parent, cut + 1);
                                assert!(outcome.value.is_none());
                            }
                        }
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn coupled_native_admission_unwind_closes_owner_without_rearming_root_or_children() {
            fn check<P: MixedProfile>() {
                for (first, last) in [("seq", "all"), ("all", "seq"), ("repeat", "all")] {
                    let (_, cuts, _) = run::<P>(first, last, 10_000, None);
                    for cut in [0, cuts.parent / 2, cuts.parent - 1] {
                        let (outcome, seen, refused) =
                            run::<P>(first, last, 10_000, Some((cut, Fault::Unwind)));
                        assert!(refused);
                        assert_eq!(seen.parent, cut + 1);
                        assert!(outcome.value.is_none());
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        #[test]
        fn child_restore_failure_revokes_the_already_restored_parent_at_every_frontier() {
            fn check<P: MixedProfile>() {
                for first in ["seq", "all", "repeat"] {
                    for last in ["seq", "all"] {
                        let (_, cuts, _) = run::<P>(first, last, 10_000, None);
                        for cut in 0..cuts.child {
                            for fault in
                                [Fault::ChildRefused, Fault::ChildRevoked, Fault::ChildUnwind]
                            {
                                let (outcome, seen, refused) =
                                    run::<P>(first, last, 10_000, Some((cut, fault)));
                                assert!(refused, "{first}/{last} child={cut} {fault:?}");
                                assert_eq!(seen.child, cut + 1);
                                assert!(outcome.value.is_none());
                            }
                        }
                    }
                }
            }
            check::<Counter>();
            check::<Panel>();
        }

        mod guarded_work {
            use super::*;
            use leselang_vm::native::HostWorkOutcome;

            #[derive(Clone, Copy, Debug)]
            enum Program {
                Mixed(&'static str, &'static str),
                Final(&'static str, bool),
            }
            fn programs() -> Vec<Program> {
                let mut programs = Vec::new();
                for first in ["seq", "all", "repeat"] {
                    for last in ["seq", "all"] {
                        programs.push(Program::Mixed(first, last));
                    }
                    for prefix in [false, true] {
                        programs.push(Program::Final(first, prefix));
                    }
                }
                programs
            }
            #[derive(Clone, Copy, Debug, PartialEq)]
            enum WorkFault {
                Refused,
                Revoked,
                Cancelled,
                Unwind,
            }
            #[derive(Default, Debug, PartialEq)]
            struct WorkTrace {
                boundaries: usize,
                children: usize,
                charges: u64,
                entered: usize,
                stopped: Option<(usize, u64)>,
                injected: Option<WorkFault>,
            }

            fn baseline<P: MixedProfile>(program: Program, budget: u64) -> FinalOutcome {
                match program {
                    Program::Mixed(first, last) => {
                        let result = run_with_transfer::<P>(first, last, budget, false);
                        FinalOutcome {
                            end: EffectSessionStatus::Terminal(if result.value.is_some() {
                                EffectSessionEnd::Completed
                            } else {
                                EffectSessionEnd::Failed
                            }),
                            rows: result.value.map(|value| vec![value]),
                            remaining: result.remaining,
                            invocations: result.invocations,
                            requests: result.requests,
                        }
                    }
                    Program::Final(kind, prefix) => {
                        run_final::<P>(kind, prefix, budget, false, None, None).0
                    }
                }
            }

            // The parent moves only while quiescent. Once native work borrows the
            // VM and its meter, only child collectors may checkpoint inside it.
            fn run<P: MixedProfile>(
                program: Program,
                budget: u64,
                restore_parent: bool,
                meter_work: bool,
                fault: Option<(usize, WorkFault)>,
            ) -> (FinalOutcome, WorkTrace) {
                let mut output = None;
                fixture::<P>(|host, scope| {
                    let sibling = LocalExecutionScope::new();
                    let source = match program {
                        Program::Mixed(first, last) => P::program(first, last),
                        Program::Final(kind, prefix) => final_program::<P>(kind, prefix),
                    };
                    let node = compile(&source, &host).unwrap();
                    let plans = plans(&node, &host);
                    let runtime = Runtime {
                        host: &host,
                        plans: &plans,
                    };
                    let mut owner =
                        scope.bind(Some(Vm::new(&node, Vec::new(), Fuel::new(budget), STACK)));
                    let mut trace = WorkTrace::default();
                    let mut values = None;
                    for _ in 0..=4 {
                        let (scope, slot) = owner.as_parts_mut();
                        let polled = slot.as_mut().unwrap().poll_in_scope(&runtime, scope);
                        match polled {
                            Ok(EffectSessionPoll::Request(active)) => {
                                let identity = active.plan.id();
                                if restore_parent {
                                    handoff(slot, scope, &runtime, active.plan, Fault::None)
                                        .unwrap();
                                }
                                let vm = slot.as_mut().unwrap();
                                let collected = catch_unwind(AssertUnwindSafe(|| {
                                    vm.with_metered_host_outcome_in_scope(
                                        active,
                                        |active, fuel| {
                                            trace.entered += 1;
                                            let result = runtime.collect_with_checkpoint(
                                                active,
                                                true,
                                                Some(scope),
                                                |phase| {
                                                    let cut = trace.boundaries;
                                                    trace.boundaries += 1;
                                                    trace.children += usize::from(
                                                        phase == TransferPhase::ChildAdmission,
                                                    );
                                                    let stopped = (
                                                        host.invoked.borrow().len(),
                                                        host.serial.get(),
                                                    );
                                                    if meter_work {
                                                        if fuel.charge(1).is_err() {
                                                            trace.stopped = Some(stopped);
                                                            return Err(Refusal);
                                                        }
                                                        trace.charges += 1;
                                                    }
                                                    if let Some((at, injected)) = fault
                                                        && at == cut
                                                    {
                                                        trace.stopped = Some(stopped);
                                                        trace.injected = Some(injected);
                                                        match injected {
                                                            WorkFault::Revoked => {
                                                                scope.close();
                                                            }
                                                            WorkFault::Unwind => {
                                                                panic!("native collection unwind")
                                                            }
                                                            WorkFault::Refused
                                                            | WorkFault::Cancelled => {}
                                                        }
                                                        return Err(Refusal);
                                                    }
                                                    Ok(())
                                                },
                                            );
                                            match result {
                                                Ok(reply) => HostWorkOutcome::Pending(Ok(reply)),
                                                Err(error) => {
                                                    if trace.injected == Some(WorkFault::Cancelled)
                                                    {
                                                        HostWorkOutcome::Cancelled(Err(error))
                                                    } else {
                                                        HostWorkOutcome::Failed(Err(error))
                                                    }
                                                }
                                            }
                                        },
                                        scope,
                                    )
                                }));
                                let reply = match collected {
                                    Ok(Ok(Ok(reply))) => reply,
                                    Ok(Ok(Err(_))) | Err(_) => {
                                        let end = match trace.injected {
                                            Some(WorkFault::Unwind) => {
                                                EffectSessionEnd::HostUncertain
                                            }
                                            Some(WorkFault::Cancelled) => {
                                                EffectSessionEnd::Cancelled
                                            }
                                            _ => EffectSessionEnd::Failed,
                                        };
                                        assert_eq!(vm.status(), EffectSessionStatus::Terminal(end));
                                        assert!(!scope.is_open());
                                        assert!(runtime.authorize(&identity).is_err());
                                        assert_eq!(
                                            trace.stopped,
                                            Some((host.invoked.borrow().len(), host.serial.get()))
                                        );
                                        break;
                                    }
                                    Ok(Err(_)) => panic!("pending work was not entered"),
                                };
                                assert!(scope.is_open());
                                assert_eq!(vm.status(), EffectSessionStatus::Awaiting);
                                let remaining = vm.fuel_remaining();
                                let address = &*reply.buffer as *const u8;
                                let mut stale = identity.clone();
                                stale.serial = 0;
                                let reply = vm
                                    .try_accept_in_scope(&stale, reply, &runtime, scope)
                                    .unwrap_err()
                                    .reply;
                                assert_eq!(&*reply.buffer as *const u8, address);
                                assert_eq!(vm.fuel_remaining(), remaining);
                                assert!(scope.is_open());
                                vm.try_accept_in_scope(&identity, reply, &runtime, scope)
                                    .unwrap();
                                assert_eq!(vm.status(), EffectSessionStatus::Ready);
                                assert_eq!(vm.fuel_remaining(), remaining);
                            }
                            Ok(EffectSessionPoll::Value(PureValue::Scalar(value))) => {
                                assert!(matches!(program, Program::Mixed(_, _)));
                                values = Some(vec![value]);
                                break;
                            }
                            Ok(EffectSessionPoll::Value(PureValue::Result(View::Group(
                                _,
                                group,
                            )))) => {
                                assert!(matches!(program, Program::Final(_, _)));
                                values = Some(
                                    group
                                        .members()
                                        .iter()
                                        .map(|row| match P::bind(row.as_parts().4.value.clone()) {
                                            PureValue::Scalar(value) => value,
                                            PureValue::Result(value) => {
                                                P::field(&value, "text").unwrap()
                                            }
                                        })
                                        .collect(),
                                );
                                break;
                            }
                            Err(_) => {
                                assert_eq!(
                                    slot.as_ref().unwrap().status(),
                                    EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                                );
                                break;
                            }
                            _ => panic!("unexpected guarded pipeline state"),
                        }
                    }
                    assert!(!owner.is_open());
                    assert!(sibling.is_open());
                    let invocations = host.invoked.borrow().clone();
                    let requests = host.serial.get();
                    let (scope, slot) = owner.as_parts_mut();
                    let vm = slot.as_mut().unwrap();
                    let end = vm.status();
                    let remaining = vm.fuel_remaining();
                    // Verify the retained parent is already sealed, without a
                    // coordinator close/cancel call repairing its lifecycle.
                    let input = Box::new(19);
                    let address = &*input as *const i32;
                    let input = vm
                        .with_host_work_in_scope::<_, ()>(
                            input,
                            |_| panic!("terminal work must not replay"),
                            scope,
                        )
                        .unwrap_err();
                    assert_eq!(&*input as *const i32, address);
                    assert!(matches!(
                        vm.poll_in_scope(&runtime, scope).unwrap(),
                        EffectSessionPoll::Terminal(_)
                    ));
                    assert_eq!(vm.status(), end);
                    assert_eq!(vm.fuel_remaining(), remaining);
                    assert_eq!(*host.invoked.borrow(), invocations);
                    assert_eq!(host.serial.get(), requests);
                    output = Some((
                        FinalOutcome {
                            rows: values,
                            remaining,
                            invocations,
                            requests,
                            end,
                        },
                        trace,
                    ));
                });
                output.unwrap()
            }

            #[test]
            fn restored_guarded_work_matches_original_source_pipelines_at_every_fuel_cutoff() {
                fn check<P: MixedProfile>() {
                    for program in programs() {
                        let expected = baseline::<P>(program, 10_000);
                        let (actual, trace) = run::<P>(program, 10_000, true, false, None);
                        assert_eq!(actual, expected);
                        assert!(trace.children > 0);
                        assert!(trace.entered > 0);
                        assert_eq!(trace.charges, 0);
                        for budget in 0..=10_001 - expected.remaining {
                            assert_eq!(
                                run::<P>(program, budget, true, false, None).0,
                                baseline::<P>(program, budget),
                                "{program:?} fuel={budget}",
                            );
                        }
                    }
                }
                check::<Counter>();
                check::<Panel>();
            }

            #[test]
            fn restored_guarded_native_work_and_reentry_share_every_meter_cutoff() {
                fn check<P: MixedProfile>() {
                    for program in programs() {
                        let unmetered = baseline::<P>(program, 10_000);
                        let (baseline, trace) = run::<P>(program, 10_000, false, true, None);
                        assert_eq!(baseline.rows, unmetered.rows);
                        assert_eq!(baseline.invocations, unmetered.invocations);
                        assert_eq!(baseline.requests, unmetered.requests);
                        assert_eq!(baseline.remaining + trace.charges, unmetered.remaining);
                        assert_eq!(trace.charges, trace.boundaries as u64);
                        let needed = 10_000 - baseline.remaining;
                        assert_eq!(
                            run::<P>(program, 10_000, true, true, None),
                            (baseline, trace)
                        );
                        for budget in 0..=needed + 1 {
                            assert_eq!(
                                run::<P>(program, budget, true, true, None),
                                run::<P>(program, budget, false, true, None),
                                "{program:?} fuel={budget}",
                            );
                        }
                    }
                }
                check::<Counter>();
                check::<Panel>();
            }

            #[test]
            fn every_native_work_fault_seals_restored_parents_without_manual_cancellation() {
                fn check<P: MixedProfile>() {
                    for program in programs() {
                        for metered in [false, true] {
                            let (_, trace) = run::<P>(program, 10_000, true, metered, None);
                            for cut in 0..trace.boundaries {
                                for fault in [
                                    WorkFault::Refused,
                                    WorkFault::Revoked,
                                    WorkFault::Cancelled,
                                    WorkFault::Unwind,
                                ] {
                                    let (result, observed) = run::<P>(
                                        program,
                                        10_000,
                                        true,
                                        metered,
                                        Some((cut, fault)),
                                    );
                                    assert!(result.rows.is_none());
                                    assert_eq!(observed.boundaries, cut + 1);
                                    assert_eq!(observed.injected, Some(fault));
                                    assert_eq!(
                                        (result, observed),
                                        run::<P>(
                                            program,
                                            10_000,
                                            false,
                                            metered,
                                            Some((cut, fault)),
                                        ),
                                        "{program:?} cut={cut} {fault:?} metered={metered}",
                                    );
                                }
                            }
                        }
                    }
                }
                check::<Counter>();
                check::<Panel>();
            }
        }
    }

    mod saved_mixed {
        use super::*;
        use effect_stack::{
            SavedEffectReturnFrame, StackedEffectOutcome, evaluate_stacked_effects_in_scope,
            restore_effect_return_stack, resume_accepted_stacked_effects,
        };
        use parallel_evaluation::SavedParallelReply;
        use sequence_evaluation::{SavedSequenceReply, SequenceEnd};

        type Queued<'ir, P> = PreparedSequenceSession<'ir, Node, (), Identity, Domain<P>, usize>;

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
                    let mut accepted = Vec::new();
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
                        assert!(std::ptr::eq(row.as_parts().1, branch));
                        accepted.push(row);
                    }
                    let mut flying = None;
                    if cut % 2 == 1 {
                        let SequencePoll::Request { index, request } =
                            inventory.publish(runtime, &mut queue)?
                        else {
                            return Err(Refusal);
                        };
                        assert_eq!(index, prefix);
                        flying = Some(inventory.invoke(runtime, request)?);
                    }
                    let mut saved = queue.try_into_saved(accepted).map_err(|_| Refusal)?;
                    if matches!(fault, Fault::QueuedHandle) {
                        for row in &mut saved {
                            if let SavedSequenceReply::Prepared(member) = &mut row.reply {
                                member.request = 8;
                            }
                        }
                    }
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
                    for (index, reply) in replies.iter_mut().enumerate() {
                        let identity = plan.members.borrow()[index].clone();
                        if cut & (1 << index) != 0 {
                            let reply = reply.take().ok_or(Refusal)?;
                            barrier
                                .try_accept(index, &identity, reply, runtime.host)
                                .map_err(|_| Refusal)?;
                        }
                    }
                    let saved = barrier.try_into_saved().map_err(|_| Refusal)?;
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
        let outcome = run_with_transfer::<P>(first, last, budget, false);
        outcome.value.map(|_| outcome.remaining).ok_or(())
    }
    fn run_with_transfer<P: MixedProfile>(
        first: &str,
        last: &str,
        budget: u64,
        transfer: bool,
    ) -> TransferOutcome {
        let mut result = None;
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
                    result = Some(TransferOutcome {
                        value: None,
                        remaining,
                        invocations: host.invoked.borrow().clone(),
                        requests: host.serial.get(),
                    });
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
                let reply = runtime.collect_with_transfer(active, transfer).unwrap();
                let identity = reply.identity.clone();
                vm.try_accept(&identity, reply, &runtime).unwrap();
                assert_eq!(vm.fuel_remaining(), fuel);
                assert_eq!(
                    *host.invoked.borrow(),
                    P::inputs(first)[..host.invoked.borrow().len()]
                );
            }
            let value = match vm.poll(&runtime) {
                Ok(EffectSessionPoll::Value(PureValue::Scalar(value))) => {
                    assert_eq!(value, P::result(first));
                    assert!(matches!(
                        vm.poll(&runtime).unwrap(),
                        EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
                    ));
                    Some(value)
                }
                Err(_) => {
                    assert_eq!(
                        vm.status(),
                        EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                    );
                    None
                }
                _ => panic!("unexpected mixed result"),
            };
            assert!(!vm.lease().is_open());
            assert_eq!(*host.invoked.borrow(), P::inputs(first));
            result = Some(TransferOutcome {
                value,
                remaining: vm.fuel_remaining(),
                invocations: host.invoked.borrow().clone(),
                requests: host.serial.get(),
            });
        });
        result.unwrap()
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

    #[test]
    fn two_hosts_transfer_real_all_barriers_through_one_vm_at_every_fuel_cutoff() {
        fn check<P: MixedProfile>() {
            for (first, last) in [
                ("all", "all"),
                ("seq", "all"),
                ("repeat", "all"),
                ("all", "seq"),
            ] {
                let remaining = run::<P>(first, last, 10_000).unwrap();
                for budget in (0..=10_001 - remaining).chain([10_000]) {
                    assert_eq!(
                        run_with_transfer::<P>(first, last, budget, true),
                        run_with_transfer::<P>(first, last, budget, false)
                    );
                }
            }
        }
        check::<Counter>();
        check::<Panel>();
    }

    #[test]
    fn two_hosts_transfer_seq_frontiers_and_original_prefixes_at_every_vm_fuel_cutoff() {
        fn check<P: MixedProfile>() {
            for (first, last) in [
                ("seq", "seq"),
                ("repeat", "seq"),
                ("seq", "all"),
                ("all", "seq"),
            ] {
                let remaining = run::<P>(first, last, 10_000).unwrap();
                for budget in (0..=10_001 - remaining).chain([10_000]) {
                    assert_eq!(
                        run_with_transfer::<P>(first, last, budget, true),
                        run_with_transfer::<P>(first, last, budget, false)
                    );
                }
            }
        }
        check::<Counter>();
        check::<Panel>();
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
            run_with_requests::<P>(case, budget, if case.selected == 2 { 1 } else { 2 })
        }

        fn run_with_requests<P: Profile>(case: &Case, budget: u64, requests: usize) -> Outcome {
            run_with_policy::<P>(case, budget, requests, group_program_limits(), STACK)
        }

        fn run_with_policy<P: Profile>(
            case: &Case,
            budget: u64,
            requests: usize,
            compiler: CatalogGroupProgramLimits,
            execution: EffectStackLimits,
        ) -> Outcome {
            let mut outcome = None;
            fixture::<P>(|host, scope| {
                let node = compile_with_policy(&case.source, &host, compiler).unwrap();
                let plans = plans(&node, &host);
                let runtime = Runtime {
                    host: &host,
                    plans: &plans,
                };
                let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(budget), execution);
                for phase in 0..=requests {
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
                            assert_eq!(active, requests);
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

        mod program_boundary_acceptance {
            use super::*;

            type CompilerBoundary = (
                &'static str,
                usize,
                fn(&mut CatalogGroupProgramLimits, usize),
            );
            type RuntimeBoundary = (&'static str, usize, fn(&mut EffectStackLimits, usize));

            fn compiler_boundaries() -> [CompilerBoundary; 13] {
                [
                    ("source nodes", 11, |p, n| {
                        p.program.expression.control.source.max_source_nodes = n
                    }),
                    ("source depth", 3, |p, n| {
                        p.program.expression.control.source.max_source_depth = n
                    }),
                    ("lowered nodes", 23, |p, n| {
                        p.program.expression.control.source.max_lowered_nodes = n
                    }),
                    ("lowered depth", 7, |p, n| {
                        p.program.expression.control.source.max_lowered_depth = n
                    }),
                    // Standalone helper checks cost 1 + 5 + 23; main and expansions cost 31.
                    ("compilation work", 60, |p, n| {
                        p.program.max_compilation_nodes = n
                    }),
                    ("expansion depth", 3, |p, n| {
                        p.program.max_expansion_depth = n
                    }),
                    ("bindings", 3, |p, n| {
                        p.program.expression.control.max_bindings = n
                    }),
                    ("groups", 1, |p, n| p.max_groups = n),
                    ("cold calls", 3, |p, n| p.program.expression.max_calls = n),
                    ("functions", 4, |p, n| p.program.max_functions = n),
                    ("parameters", 1, |p, n| p.program.max_parameters = n),
                    ("branches", 2, |p, n| p.max_branches = n),
                    ("source arguments", 2, |p, n| {
                        p.program.expression.control.source.max_arguments = n
                    }),
                ]
            }

            fn runtime_boundaries() -> [RuntimeBoundary; 6] {
                [
                    ("nodes", 23, |p, n| p.effects.pure.max_nodes = n),
                    ("depth", 7, |p, n| p.effects.pure.max_depth = n),
                    // Prepared operands have their own host policy; the VM owns two locals.
                    ("bindings", 2, |p, n| p.effects.pure.max_bindings = n),
                    ("frames", 2, |p, n| p.max_frames = n),
                    ("arguments", 1, |p, n| p.effects.max_arguments = n),
                    ("branches", 2, |p, n| p.effects.max_branches = n),
                ]
            }

            fn source_shape(source: &str) -> (usize, usize) {
                let tree = parse(source);
                assert!(tree.diagnostics.is_empty());
                let (mut largest, mut deepest) = (0, 0);
                for function in tree.function.iter().chain(&tree.helpers) {
                    let mut nodes = 0;
                    let mut pending = vec![(&function.body, 0)];
                    while let Some((node, depth)) = pending.pop() {
                        nodes += 1;
                        deepest = deepest.max(depth);
                        if let leselang_vm::native::syntax::Expression::Call { arguments, .. } =
                            node
                        {
                            pending.extend(arguments.iter().map(|a| (&a.value, depth + 1)));
                        }
                    }
                    largest = largest.max(nodes);
                }
                (largest, deepest)
            }

            fn pipeline<P: Profile>(kind: &str) -> Case {
                let counter = P::ARGUMENT == ScalarType::Integer;
                let (ty, literal, field) = if counter {
                    ("integer", "1", "value")
                } else {
                    ("string", "\"x\"", "text")
                };
                let call = |argument| format!("{}({}: {argument})", P::OPERATION, P::PARAMETER);
                let tail = call("answer");
                let source = format!(
                    "fn identity(value: {ty}) = value\nfn step(value: {ty}) = {}\nfn gathered(value: {ty}) = bind(batch: {kind}(left: step(value: value), right: step(value: value)), body: field(value: member(value: batch, name: \"right\"), name: \"{field}\"))\nfn main() = bind(answer: gathered(value: {literal}), body: bind(ack: {tail}, body: answer))",
                    call("identity(value: value)")
                );
                Case {
                    source,
                    selected: 1,
                    output: if counter {
                        ScalarValue::Integer(2)
                    } else {
                        ScalarValue::String("x".into())
                    },
                    inputs: if counter {
                        vec![
                            ScalarValue::Integer(1),
                            ScalarValue::Integer(1),
                            ScalarValue::Integer(2),
                        ]
                    } else {
                        vec![ScalarValue::String("x".into()); 3]
                    },
                    tail,
                }
            }

            fn cold_refusal<P: Profile>(
                case: &Case,
                policy: CatalogGroupProgramLimits,
                label: &str,
            ) {
                fixture::<P>(|host, _| {
                    assert!(
                        compile_with_policy(&case.source, &host, policy).is_err(),
                        "{label}"
                    );
                    assert_eq!(host.serial.get(), 0);
                    assert!(host.invoked.borrow().is_empty());
                });
            }

            fn runtime_refusal<P: Profile>(case: &Case, policy: EffectStackLimits, label: &str) {
                fixture::<P>(|host, scope| {
                    let node = compile(&case.source, &host).unwrap();
                    let plans = plans(&node, &host);
                    let runtime = Runtime {
                        host: &host,
                        plans: &plans,
                    };
                    let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(10_000), policy);
                    assert!(vm.poll(&runtime).is_err(), "{label}");
                    assert_eq!(host.serial.get(), 0, "{label}");
                    assert!(host.invoked.borrow().is_empty());
                    assert!(plans.iter().all(|p| p.identity.borrow().is_none() && p.members.borrow().is_empty()));
                    assert!(!vm.lease().is_open());
                    let fuel = vm.fuel_remaining();
                    assert!(matches!(
                        vm.poll(&runtime).unwrap(),
                        EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
                    ));
                    assert_eq!(vm.fuel_remaining(), fuel);
                });
            }

            #[test]
            fn whole_program_compile_budgets_are_inclusive_and_independent() {
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for kind in ["seq", "all"] {
                        let case = pipeline::<P>(kind);
                        assert_eq!(source_shape(&case.source), (11, 3));
                        fixture::<P>(|host, _| {
                            let node = compile(&case.source, &host).unwrap();
                            let mut pending = vec![(&node, 0)];
                            let (mut nodes, mut depth) = (0, 0);
                            while let Some((node, level)) = pending.pop() {
                                nodes += 1;
                                depth = depth.max(level);
                                pending.extend(node.children().map(|child| (child, level + 1)));
                            }
                            assert_eq!((nodes, depth), (23, 7));
                        });
                        let baseline = run::<P>(&case, 10_000);
                        assert!(baseline.complete);
                        for (label, required, set) in compiler_boundaries() {
                            let mut policy = group_program_limits();
                            set(&mut policy, required - 1);
                            cold_refusal::<P>(&case, policy, label);
                            for extra in 0..=1 {
                                set(&mut policy, required + extra);
                                assert_eq!(
                                    run_with_policy::<P>(&case, 10_000, 2, policy, STACK),
                                    baseline,
                                    "{label}/{extra}"
                                );
                            }
                            count += 1;
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 52);
            }

            #[test]
            fn runtime_budgets_do_not_borrow_compiler_allowances_or_dispatch_partial_work() {
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for kind in ["seq", "all"] {
                        let case = pipeline::<P>(kind);
                        let baseline = run::<P>(&case, 10_000);
                        for (label, required, set) in runtime_boundaries() {
                            let mut policy = STACK;
                            set(&mut policy, required - 1);
                            runtime_refusal::<P>(&case, policy, label);
                            for extra in 0..=1 {
                                set(&mut policy, required + extra);
                                assert_eq!(
                                    run_with_policy::<P>(
                                        &case,
                                        10_000,
                                        2,
                                        group_program_limits(),
                                        policy
                                    ),
                                    baseline,
                                    "{label}/{extra}"
                                );
                            }
                            count += 1;
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 24);
            }

            fn balanced_sum(leaves: usize) -> String {
                if leaves == 1 {
                    return "1".into();
                }
                format!(
                    "add(left: {}, right: {})",
                    balanced_sum(leaves / 2),
                    balanced_sum(leaves - leaves / 2)
                )
            }

            #[test]
            fn unused_bodies_obey_source_output_and_nonrefunding_work_budgets() {
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for kind in ["seq", "all"] {
                        let baseline = pipeline::<P>(kind);
                        let expected = run::<P>(&baseline, 10_000);
                        for (body, nodes, depth) in [
                            ("div(left: 1, right: 0)".into(), 3, 1),
                            (format!("to_string(value: {})", balanced_sum(512)), 1024, 10),
                            (
                                (0..leselang_vm::native::syntax::MAX_CALL_DEPTH)
                                    .fold("true".into(), |body, _| format!("not(value: {body})")),
                                leselang_vm::native::syntax::MAX_CALL_DEPTH + 1,
                                leselang_vm::native::syntax::MAX_CALL_DEPTH,
                            ),
                        ] {
                            let mut case = pipeline::<P>(kind);
                            case.source.push_str(&format!("\nfn unused() = {body}"));
                            assert_eq!(source_shape(&case.source), (nodes.max(11), depth.max(3)));
                            let mut policy = group_program_limits();
                            policy.program.max_compilation_nodes = 60 + nodes;
                            assert_eq!(
                                run_with_policy::<P>(&case, 10_000, 2, policy, STACK),
                                expected
                            );
                            policy.program.max_compilation_nodes -= 1;
                            cold_refusal::<P>(&case, policy, "unused compilation work");
                            policy.program.max_compilation_nodes += 1;
                            if nodes >= 17 {
                                for lowered in [false, true] {
                                    let mut limited = policy;
                                    let source = &mut limited.program.expression.control.source;
                                    if depth == leselang_vm::native::syntax::MAX_CALL_DEPTH {
                                        if lowered {
                                            source.max_lowered_depth = depth - 1;
                                        } else {
                                            source.max_source_depth = depth - 1;
                                        }
                                    } else if lowered {
                                        source.max_lowered_nodes = nodes - 1;
                                    } else {
                                        source.max_source_nodes = nodes - 1;
                                    }
                                    cold_refusal::<P>(&case, limited, "unused physical boundary");
                                }
                                let oversized = if nodes == 1024 {
                                    format!("parse_integer(value: {body})")
                                } else {
                                    format!("not(value: {body})")
                                };
                                let mut excess = pipeline::<P>(kind);
                                excess
                                    .source
                                    .push_str(&format!("\nfn unused() = {oversized}"));
                                if depth == leselang_vm::native::syntax::MAX_CALL_DEPTH {
                                    assert!(
                                        parse(&excess.source)
                                            .diagnostics
                                            .iter()
                                            .any(|d| d.code == "LSE1110")
                                    );
                                }
                                cold_refusal::<P>(
                                    &excess,
                                    group_program_limits(),
                                    "unused source exceeds fixture/parser ceiling",
                                );
                            }
                            count += 1;
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 12);
            }

            #[test]
            fn over_ceiling_configuration_refuses_even_small_valid_programs() {
                fn check<P: Profile>() -> usize {
                    use pure_typing::{
                        MAX_TYPE_INFERENCE_BINDINGS, MAX_TYPE_INFERENCE_DEPTH,
                        MAX_TYPE_INFERENCE_NODES,
                    };
                    let mut count = 0;
                    for kind in ["seq", "all"] {
                        let case = pipeline::<P>(kind);
                        let expected = run::<P>(&case, 10_000);
                        for (label, _, set) in compiler_boundaries() {
                            let ceiling = match label {
                                "source nodes" | "lowered nodes" | "compilation work"
                                | "cold calls" | "groups" => MAX_TYPE_INFERENCE_NODES,
                                "source depth" | "lowered depth" | "expansion depth" => {
                                    MAX_TYPE_INFERENCE_DEPTH
                                }
                                "bindings" => MAX_TYPE_INFERENCE_BINDINGS,
                                "functions" => leselang_vm::native::syntax::MAX_FUNCTIONS,
                                "parameters" => {
                                    leselang_vm::native::syntax::MAX_FUNCTION_PARAMETERS
                                }
                                "branches" => group_exports::MAX_GROUP_EXPORT_MEMBERS,
                                "source arguments" => source_call::MAX_SOURCE_CALL_ARGUMENTS,
                                _ => unreachable!(),
                            };
                            let mut policy = group_program_limits();
                            set(&mut policy, ceiling);
                            assert_eq!(
                                run_with_policy::<P>(&case, 10_000, 2, policy, STACK),
                                expected
                            );
                            for denied in [ceiling + 1, usize::MAX] {
                                set(&mut policy, denied);
                                cold_refusal::<P>(&case, policy, label);
                                count += 1;
                            }
                        }
                        for (label, _, set) in runtime_boundaries() {
                            let ceiling = match label {
                                "nodes" => MAX_TYPE_INFERENCE_NODES,
                                "depth" => MAX_TYPE_INFERENCE_DEPTH,
                                "bindings" => MAX_TYPE_INFERENCE_BINDINGS,
                                "frames" => effect_stack::MAX_EFFECT_RETURN_FRAMES,
                                "arguments" => call_typing::MAX_CALL_ARGUMENTS,
                                "branches" => flow_typing::MAX_TYPED_GROUP_BRANCHES,
                                _ => unreachable!(),
                            };
                            let mut policy = STACK;
                            set(&mut policy, ceiling);
                            assert_eq!(
                                run_with_policy::<P>(
                                    &case,
                                    10_000,
                                    2,
                                    group_program_limits(),
                                    policy
                                ),
                                expected
                            );
                            for denied in [ceiling + 1, usize::MAX] {
                                set(&mut policy, denied);
                                runtime_refusal::<P>(&case, policy, label);
                                count += 1;
                            }
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 152);
            }
        }

        mod branch_ceiling_acceptance {
            use super::*;

            const SHAPES: [&str; 4] = ["seq", "all", "repeat", "nested-seq"];
            const LIMIT: usize = group_exports::MAX_GROUP_EXPORT_MEMBERS;

            fn pipeline<P: Profile>(
                shape: &str,
                members: usize,
                alternate: bool,
                successor: bool,
                fault: Option<usize>,
            ) -> Case {
                let counter = P::ARGUMENT == ScalarType::Integer;
                let (ty, value, bad) = if counter {
                    ("integer", "1", "div(left: 1, right: 0)")
                } else {
                    (
                        "string",
                        "\"x\"",
                        "to_string(value: div(left: 1, right: 0))",
                    )
                };
                let member = |index| {
                    format!(
                        "step(value: {})",
                        if fault == Some(index) { bad } else { value }
                    )
                };
                let rows = |range: std::ops::Range<usize>| {
                    range
                        .map(|i| format!("m{}: {}", i + 1, member(i)))
                        .collect::<Vec<_>>()
                        .join(", ")
                };
                let (group, selected) = match shape {
                    "seq" | "all" => (
                        format!("{shape}({})", rows(0..members)),
                        format!("m{members}"),
                    ),
                    "repeat" => (
                        format!("repeat(times: {members}, body: {})", member(0)),
                        format!("iteration_{members}"),
                    ),
                    "nested-seq" => (
                        format!(
                            "seq(head: seq({}), tail: seq({}))",
                            rows(0..members / 2),
                            rows(members / 2..members)
                        ),
                        format!("tail__m{members}"),
                    ),
                    _ => unreachable!(),
                };
                let operation = |value| format!("{}({}: {value})", P::OPERATION, P::PARAMETER);
                let (left, right) = if alternate {
                    ("value", bad)
                } else {
                    (bad, "value")
                };
                let helper = format!(
                    "fn step(value: {ty}) = choose(when: {alternate}, then: {}, otherwise: {})",
                    operation(left),
                    operation(right)
                );
                let projection = format!(
                    "field(value: member(value: batch, name: \"{selected}\"), name: \"{}\")",
                    if counter { "value" } else { "text" }
                );
                let expected = if counter {
                    members.to_string()
                } else {
                    "\"x\"".into()
                };
                let next = if counter { "1" } else { "\"done\"" };
                let tail = operation(&format!(
                    "choose(when: eq(left: answer, right: {expected}), then: {next}, otherwise: {bad})"
                ));
                let body = if successor {
                    format!("bind(ack: {tail}, body: answer)")
                } else {
                    "answer".into()
                };
                let source = format!(
                    "{helper}\nfn main() = bind(batch: {group}, body: bind(answer: {projection}, body: {body}))"
                );
                let mut inputs = vec![
                    if counter {
                        ScalarValue::Integer(1)
                    } else {
                        ScalarValue::String("x".into())
                    };
                    members
                ];
                if successor {
                    inputs.push(if counter {
                        ScalarValue::Integer(1)
                    } else {
                        ScalarValue::String("done".into())
                    });
                }
                Case {
                    source,
                    selected: 1,
                    output: if counter {
                        ScalarValue::Integer(members as u64)
                    } else {
                        ScalarValue::String("x".into())
                    },
                    inputs,
                    tail,
                }
            }

            fn unpublished_failure<P: Profile>(case: &Case, budget: u64, preparing: bool) -> u64 {
                let mut remaining = None;
                fixture::<P>(|host, scope| {
                    let node = compile(&case.source, &host).unwrap();
                    let plans = plans(&node, &host);
                    let runtime = Runtime {
                        host: &host,
                        plans: &plans,
                    };
                    let mut vm = LocalVm::new(scope, &node, Vec::new(), Fuel::new(budget), STACK);
                    assert!(vm.poll(&runtime).is_err());
                    let fuel = vm.fuel_remaining();
                    assert!(fuel < budget);
                    let serial = host.serial.get();
                    assert_eq!(
                        vm.status(),
                        EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                    );
                    assert!(!vm.lease().is_open());
                    assert!(matches!(
                        vm.poll(&runtime).unwrap(),
                        EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
                    ));
                    assert_eq!(vm.fuel_remaining(), fuel);
                    assert_eq!(host.serial.get(), serial);
                    assert!(host.invoked.borrow().is_empty());
                    if preparing {
                        assert_eq!(host.serial.get(), 0);
                        assert!(plans.iter().all(
                            |p| p.identity.borrow().is_none() && p.members.borrow().is_empty()
                        ));
                    }
                    remaining = Some(fuel);
                });
                remaining.unwrap()
            }

            #[test]
            fn full_groups_and_captured_successors_reach_the_public_branch_ceiling() {
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for shape in SHAPES {
                        for alternate in [false, true] {
                            for successor in [false, true] {
                                let case = pipeline::<P>(
                                    shape,
                                    LIMIT - usize::from(successor),
                                    alternate,
                                    successor,
                                    None,
                                );
                                assert_eq!(case.inputs.len(), LIMIT);
                                let outcome = run_with_requests::<P>(
                                    &case,
                                    100_000,
                                    1 + usize::from(successor),
                                );
                                assert!(outcome.complete, "{shape}/{alternate}/{successor}");
                                assert_eq!(outcome.invoked, LIMIT);
                                count += 1;
                            }
                        }
                    }
                    count
                }
                assert_eq!(LIMIT, 64);
                assert_eq!(check::<Counter>() + check::<Panel>(), 32);
            }

            #[test]
            fn full_group_completion_keeps_the_exact_fuel_threshold() {
                fn check<P: Profile>() {
                    for shape in SHAPES {
                        for alternate in [false, true] {
                            for successor in [false, true] {
                                let case = pipeline::<P>(
                                    shape,
                                    LIMIT - usize::from(successor),
                                    alternate,
                                    successor,
                                    None,
                                );
                                let requests = 1 + usize::from(successor);
                                let mut first_request_cost = None;
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
                                        Fuel::new(100_000),
                                        STACK,
                                    );
                                    assert!(matches!(
                                        vm.poll(&runtime).unwrap(),
                                        EffectSessionPoll::Request(_)
                                    ));
                                    assert!(host.invoked.borrow().is_empty());
                                    first_request_cost = Some(100_000 - vm.fuel_remaining());
                                });
                                // Correlation can consume identities after preparation; no request
                                // escapes and the closed scope does not roll those identities back.
                                unpublished_failure::<P>(
                                    &case,
                                    first_request_cost.unwrap() - 1,
                                    false,
                                );
                                let ample = run_with_requests::<P>(&case, 100_000, requests);
                                assert!(ample.complete);
                                let used = 100_000 - ample.remaining;
                                assert!(
                                    !run_with_requests::<P>(&case, used - 1, requests).complete
                                );
                                for extra in 0..=1 {
                                    assert_eq!(
                                        run_with_requests::<P>(&case, used + extra, requests),
                                        Outcome {
                                            complete: true,
                                            remaining: extra,
                                            invoked: LIMIT
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
            fn oversized_groups_and_stricter_host_limits_refuse_before_dispatch() {
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for shape in SHAPES {
                        for alternate in [false, true] {
                            let full = pipeline::<P>(shape, LIMIT, alternate, false, None);
                            let oversized = pipeline::<P>(shape, LIMIT + 1, alternate, false, None);
                            let hidden = |source: &str| {
                                source.replace("fn main() = ", "fn hidden() = ")
                                    + &format!(
                                        "\nfn main() = choose(when: true, then: {}, otherwise: hidden())",
                                        if P::ARGUMENT == ScalarType::Integer {
                                            "0"
                                        } else {
                                            "\"unused\""
                                        }
                                    )
                            };
                            fixture::<P>(|host, _| {
                                compile_with_limits(&hidden(&full.source), &host, LIMIT, 2 * LIMIT)
                                    .unwrap();
                                assert_eq!(host.serial.get(), 0);
                                assert!(host.invoked.borrow().is_empty());
                            });
                            let hidden = hidden(&oversized.source);
                            for (source, branches, calls) in [
                                (&oversized.source, LIMIT, 2 * (LIMIT + 1)),
                                (&hidden, LIMIT, 2 * (LIMIT + 1)),
                                (&full.source, LIMIT - 1, 2 * LIMIT),
                                (&full.source, LIMIT, 2 * LIMIT - 1),
                            ] {
                                fixture::<P>(|host, _| {
                                    assert!(
                                        compile_with_limits(source, &host, branches, calls)
                                            .is_err(),
                                        "{shape}/{branches}/{calls}"
                                    );
                                    assert!(host.invoked.borrow().is_empty());
                                    assert_eq!(host.serial.get(), 0);
                                });
                                count += 1;
                            }
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 64);
            }

            #[test]
            fn preparation_failure_at_the_full_group_boundary_never_publishes_partial_work() {
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for shape in SHAPES {
                        let sites = if shape == "repeat" {
                            vec![0]
                        } else {
                            vec![0, LIMIT / 2 - 1, LIMIT - 1]
                        };
                        for alternate in [false, true] {
                            for site in &sites {
                                let case =
                                    pipeline::<P>(shape, LIMIT, alternate, false, Some(*site));
                                assert!(unpublished_failure::<P>(&case, 100_000, true) > 0);
                                count += 1;
                            }
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 40);
            }
        }

        mod prepared_acceptance {
            use super::*;

            const SHAPES: [&str; 4] = ["seq", "all", "repeat", "nested-seq"];
            #[derive(Clone, Copy, Debug)]
            enum Fault {
                Arithmetic,
                Parse,
                IterationLimit,
                HelperBody,
                OperandOverflow,
            }
            const FAULTS: [Fault; 5] = [
                Fault::Arithmetic,
                Fault::Parse,
                Fault::IterationLimit,
                Fault::HelperBody,
                Fault::OperandOverflow,
            ];
            struct Program {
                case: Case,
                signature: String,
                helper: String,
            }

            fn sites(shape: &str) -> usize {
                match shape {
                    "seq" | "all" => 2,
                    "repeat" => 1,
                    "nested-seq" => 3,
                    _ => panic!("unknown prepared group shape"),
                }
            }

            fn pipeline<P: Profile>(
                shape: &str,
                alternate: bool,
                reversed: bool,
                ty: ScalarType,
                fault: Option<(usize, Fault)>,
            ) -> Program {
                let counter = P::ARGUMENT == ScalarType::Integer;
                let mode = if shape == "all" { "all" } else { "seq" };
                let mut case = case::<P>(mode, 1, ty);
                let original = if counter {
                    format!("{mode}(left: counter.add(amount: 3), right: counter.add(amount: 4))")
                } else {
                    format!(
                        "{mode}(left: panel.set_text(text: \"3\"), right: panel.set_text(text: \"7\"))"
                    )
                };
                let literal = |site: usize| {
                    if counter {
                        match shape {
                            "seq" | "all" => (site + 3).to_string(),
                            _ => "1".into(),
                        }
                    } else if shape != "repeat" && site == 0 {
                        "\"3\"".into()
                    } else {
                        "\"7\"".into()
                    }
                };
                let member = |site: usize| {
                    let mut value = literal(site);
                    let mut check = "0";
                    if let Some((_, fault)) = fault.filter(|(target, _)| *target == site) {
                        match fault {
                            Fault::Arithmetic => check = "div(left: 1, right: 0)",
                            Fault::Parse => check = "parse_integer(value: \"invalid\")",
                            Fault::IterationLimit => {
                                check = "loop(state: 0, while: true, next: state, limit: 0)"
                            }
                            Fault::HelperBody => check = "1",
                            Fault::OperandOverflow => {
                                value = if counter {
                                    format!("add(left: {}, right: 1)", u64::MAX)
                                } else {
                                    format!(
                                        "concat(left: \"{}\", right: \"x\")",
                                        "x".repeat(MAX_SCALAR_STRING_BYTES)
                                    )
                                };
                            }
                        }
                    }
                    let mut arguments = [
                        format!("value: {value}"),
                        format!("alternate: {alternate}"),
                        format!("check: {check}"),
                    ];
                    if reversed {
                        arguments.reverse();
                    }
                    format!("prepared({})", arguments.join(", "))
                };
                let (group, selected, expanded) = match shape {
                    "seq" | "all" => (
                        format!("{shape}(left: {}, right: {})", member(0), member(1)),
                        "right",
                        vec![0, 1],
                    ),
                    "repeat" => (
                        format!("repeat(times: 7, body: {})", member(0)),
                        "iteration_7",
                        vec![0; 7],
                    ),
                    "nested-seq" => (
                        format!(
                            "seq(head: {}, tail: repeat(times: 3, body: seq(left: {}, right: {})))",
                            member(0),
                            member(1),
                            member(2)
                        ),
                        "tail__iteration_3__right",
                        vec![0, 1, 2, 1, 2, 1, 2],
                    ),
                    _ => panic!("unknown prepared group shape"),
                };
                assert_eq!(case.source.matches(&original).count(), 1);
                case.source = case.source.replace(&original, &group).replace(
                    "member(value: batch, name: \"right\")",
                    &format!("member(value: batch, name: \"{selected}\")"),
                );
                let tail = case.inputs.pop().unwrap();
                case.inputs = expanded
                    .into_iter()
                    .map(|site| {
                        if counter {
                            ScalarValue::Integer(literal(site).parse().unwrap())
                        } else {
                            ScalarValue::String(literal(site).trim_matches('"').into())
                        }
                    })
                    .collect();
                case.inputs.push(tail);
                let type_name = if counter { "integer" } else { "string" };
                let signature =
                    format!("fn prepared(value: {type_name}, alternate: boolean, check: integer)");
                let bad = if counter {
                    "div(left: 1, right: 0)"
                } else {
                    "to_string(value: div(left: 1, right: 0))"
                };
                let operation =
                    |value: &str| format!("{}({}: {value})", P::OPERATION, P::PARAMETER);
                let (left, right) = if alternate {
                    ("caller", bad)
                } else {
                    (bad, "caller")
                };
                let helper = format!(
                    "{signature} = bind(caller: value, body: bind(checked: choose(when: eq(left: check, right: 0), then: 0, otherwise: div(left: 1, right: 0)), body: choose(when: alternate, then: {}, otherwise: {})))",
                    operation(left),
                    operation(right)
                );
                case.source.push_str(&format!("\n{helper}\n"));
                Program {
                    case,
                    signature,
                    helper,
                }
            }

            #[test]
            fn prepared_helpers_cross_flattened_groups_and_six_typed_successor_returns() {
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for shape in SHAPES {
                        for alternate in [false, true] {
                            for ty in TYPES {
                                let baseline = run::<P>(
                                    &pipeline::<P>(shape, alternate, false, ty, None).case,
                                    100_000,
                                );
                                assert!(baseline.complete, "{shape}/{ty:?}");
                                for reversed in [false, true] {
                                    let program =
                                        pipeline::<P>(shape, alternate, reversed, ty, None);
                                    assert_eq!(run::<P>(&program.case, 100_000), baseline);
                                    count += 1;
                                }
                            }
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 192);
            }

            #[test]
            fn prepared_group_calls_keep_the_exact_completion_threshold() {
                fn check<P: Profile>() {
                    for shape in SHAPES {
                        for alternate in [false, true] {
                            for reversed in [false, true] {
                                for ty in TYPES {
                                    let case =
                                        pipeline::<P>(shape, alternate, reversed, ty, None).case;
                                    let ample = run::<P>(&case, 100_000);
                                    assert!(ample.complete);
                                    let used = 100_000 - ample.remaining;
                                    assert!(!run::<P>(&case, used - 1).complete);
                                    for extra in 0..=1 {
                                        assert_eq!(
                                            run::<P>(&case, used + extra),
                                            Outcome {
                                                complete: true,
                                                remaining: extra,
                                                invoked: case.inputs.len(),
                                            }
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                check::<Counter>();
                check::<Panel>();
            }

            #[test]
            fn failed_member_preparation_publishes_no_group_identity_or_partial_work() {
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for shape in SHAPES {
                        for site in 0..sites(shape) {
                            for alternate in [false, true] {
                                for reversed in [false, true] {
                                    for fault in FAULTS {
                                        let program = pipeline::<P>(
                                            shape,
                                            alternate,
                                            reversed,
                                            ScalarType::String,
                                            Some((site, fault)),
                                        );
                                        fixture::<P>(|host, scope| {
                                            let node =
                                                compile(&program.case.source, &host).unwrap();
                                            let plans = plans(&node, &host);
                                            let runtime = Runtime {
                                                host: &host,
                                                plans: &plans,
                                            };
                                            let mut vm = LocalVm::new(
                                                scope,
                                                &node,
                                                Vec::new(),
                                                Fuel::new(100_000),
                                                STACK,
                                            );
                                            assert!(
                                                vm.poll(&runtime).is_err(),
                                                "{shape}/{site}/{fault:?}"
                                            );
                                            let remaining = vm.fuel_remaining();
                                            assert!(remaining > 0 && remaining < 100_000);
                                            assert_eq!(
                                                vm.status(),
                                                EffectSessionStatus::Terminal(
                                                    EffectSessionEnd::Failed
                                                )
                                            );
                                            assert!(!vm.lease().is_open());
                                            assert!(matches!(
                                                vm.poll(&runtime).unwrap(),
                                                EffectSessionPoll::Terminal(
                                                    EffectSessionEnd::Failed
                                                )
                                            ));
                                            assert_eq!(vm.fuel_remaining(), remaining);
                                            assert_eq!(host.serial.get(), 0);
                                            assert!(host.invoked.borrow().is_empty());
                                            assert!(
                                                plans.iter().all(|p| p.identity.borrow().is_none()
                                                    && p.members.borrow().is_empty())
                                            );
                                        });
                                        count += 1;
                                    }
                                }
                            }
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 320);
            }

            #[test]
            fn cold_prepared_members_must_return_exactly_one_call_on_every_path() {
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for shape in SHAPES {
                        let program = pipeline::<P>(shape, true, false, ScalarType::String, None);
                        let call = format!("{}({}: value)", P::OPERATION, P::PARAMETER);
                        for body in [
                            "value".to_owned(),
                            format!("seq(only: {call})"),
                            format!("bind(first: {call}, body: {call})"),
                            format!("{}({}: caller)", P::OPERATION, P::PARAMETER),
                            format!("choose(when: alternate, then: {call}, otherwise: value)"),
                        ] {
                            let source = program.case.source.replace(
                                &program.helper,
                                &format!("{} = {body}", program.signature),
                            );
                            assert_ne!(source, program.case.source);
                            fixture::<P>(|host, _| {
                                assert!(compile(&source, &host).is_err(), "{shape}/{body}");
                                assert!(host.invoked.borrow().is_empty());
                                assert_eq!(host.serial.get(), 0);
                            });
                            count += 1;
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 40);
            }
        }

        mod declaration_acceptance {
            use super::*;

            const PARAMETERS: &str = "number: integer, flag: boolean, text: string, absent: none, option: optional_string, entries: string_list, unused: integer, caller: string";
            const ARGUMENTS: [&str; 8] = [
                "number: parse_integer(value: text)",
                "flag: eq(left: text, right: \"7\")",
                "text: text",
                "absent: none",
                "option: optional_string(value: text)",
                "entries: strings(first: text, second: \"tail\")",
                "unused: 1",
                "caller: \"inner\"",
            ];

            fn extend(
                mut case: Case,
                helper: &str,
                before: bool,
                body: impl FnOnce(&str) -> String,
            ) -> Case {
                let original = case
                    .source
                    .lines()
                    .find(|line| line.starts_with("fn convert(text: string) = "))
                    .unwrap()
                    .to_owned();
                let value = original
                    .strip_prefix("fn convert(text: string) = ")
                    .unwrap();
                assert_eq!(case.source.matches(&original).count(), 1);
                case.source = case.source.replace(
                    &original,
                    &format!("fn convert(text: string) = {}", body(value)),
                );
                case.source = if before {
                    format!("{helper}\n{}", case.source)
                } else {
                    format!("{}\n{helper}\n", case.source)
                };
                case
            }

            fn mixed_case<P: Profile>(
                kind: &str,
                selected: usize,
                ty: ScalarType,
                order: usize,
                before: bool,
            ) -> Case {
                let value = match ty {
                    ScalarType::Integer => "number",
                    ScalarType::Boolean => "flag",
                    ScalarType::String => "concat(left: text, right: \"!\")",
                    ScalarType::None => "absent",
                    ScalarType::OptionalString => "option",
                    ScalarType::StringList => "entries",
                };
                let helper = format!(
                    "fn combine({PARAMETERS}) = choose(when: eq(left: caller, right: \"inner\"), then: {value}, otherwise: bind(fault: div(left: 1, right: 0), body: {value}))"
                );
                let mut arguments = ARGUMENTS;
                match order {
                    0 => (),
                    1 => arguments.reverse(),
                    2 => arguments.rotate_left(3),
                    _ => panic!("unknown named argument order"),
                }
                extend(case::<P>(kind, selected, ty), &helper, before, |_| {
                    format!("combine({})", arguments.join(", "))
                })
            }

            #[test]
            fn eight_mixed_parameters_preserve_values_hygiene_and_named_order_fuel() {
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for kind in ["seq", "all"] {
                        for selected in 0..3 {
                            for ty in TYPES {
                                let baseline = run::<P>(
                                    &mixed_case::<P>(kind, selected, ty, 0, false),
                                    10_000,
                                );
                                assert!(baseline.complete);
                                for order in 0..3 {
                                    for before in [false, true] {
                                        let case =
                                            mixed_case::<P>(kind, selected, ty, order, before);
                                        assert_eq!(
                                            run::<P>(&case, 10_000),
                                            baseline,
                                            "{kind}/{selected}/{ty:?}/{order}/{before}"
                                        );
                                        count += 1;
                                    }
                                }
                            }
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 432);
            }

            #[test]
            fn mixed_parameter_calls_keep_the_exact_completion_threshold() {
                fn check<P: Profile>() {
                    for kind in ["seq", "all"] {
                        for selected in 0..3 {
                            for ty in TYPES {
                                for order in 0..3 {
                                    for before in [false, true] {
                                        let case =
                                            mixed_case::<P>(kind, selected, ty, order, before);
                                        let ample = run::<P>(&case, 10_000);
                                        assert!(ample.complete);
                                        let used = 10_000 - ample.remaining;
                                        assert!(!run::<P>(&case, used - 1).complete);
                                        for extra in 0..=1 {
                                            assert_eq!(
                                                run::<P>(&case, used + extra),
                                                Outcome {
                                                    complete: true,
                                                    remaining: extra,
                                                    invoked: case.inputs.len(),
                                                }
                                            );
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                check::<Counter>();
                check::<Panel>();
            }

            #[test]
            fn unused_arguments_follow_declaration_order_not_written_order_or_body_usage() {
                fn ordered_case<P: Profile>(
                    kind: &str,
                    selected: usize,
                    ty: ScalarType,
                    recoverable_first: bool,
                    reverse: bool,
                    unselected: bool,
                ) -> Case {
                    let (type_name, _) = super::control_acceptance::scalar_source(ty);
                    let parameters = if recoverable_first {
                        "arithmetic: integer, limited: integer"
                    } else {
                        "limited: integer, arithmetic: integer"
                    };
                    let helper = format!("fn ordered({parameters}, value: {type_name}) = value");
                    extend(case::<P>(kind, selected, ty), &helper, false, |value| {
                        let mut arguments = [
                            "arithmetic: div(left: 1, right: 0)".to_owned(),
                            "limited: loop(state: 0, while: true, next: state, limit: 0)"
                                .to_owned(),
                            format!("value: {value}"),
                        ];
                        if reverse {
                            arguments.reverse();
                        }
                        let call = format!("ordered({})", arguments.join(", "));
                        if unselected {
                            format!("choose(when: false, then: {call}, otherwise: {value})")
                        } else {
                            format!("recover(value: {call}, fallback: {value})")
                        }
                    })
                }
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for kind in ["seq", "all"] {
                        for selected in 0..3 {
                            for ty in TYPES {
                                for recoverable_first in [false, true] {
                                    for reverse in [false, true] {
                                        for unselected in [false, true] {
                                            let case = ordered_case::<P>(
                                                kind,
                                                selected,
                                                ty,
                                                recoverable_first,
                                                reverse,
                                                unselected,
                                            );
                                            let outcome = run::<P>(&case, 10_000);
                                            // Arithmetic can recover; an iteration limit cannot.
                                            let complete = unselected || recoverable_first;
                                            assert_eq!(
                                                outcome.complete, complete,
                                                "{ty:?}/{recoverable_first}/{reverse}/{unselected}"
                                            );
                                            assert_eq!(
                                                outcome.invoked,
                                                case.inputs.len() - usize::from(!complete)
                                            );
                                            assert!(
                                                outcome.remaining > 0 && outcome.remaining < 10_000
                                            );
                                            count += 1;
                                        }
                                    }
                                }
                            }
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 576);
            }

            #[test]
            fn full_declaration_ceiling_and_cold_helpers_are_checked_before_host_work() {
                assert_eq!(PROGRAM.max_functions, 32);
                assert_eq!(PROGRAM.max_parameters, ARGUMENTS.len());
                fn check<P: Profile>() -> (usize, usize) {
                    let mut admitted = 0;
                    let mut refused = 0;
                    for kind in ["seq", "all"] {
                        for selected in 0..3 {
                            for ty in TYPES {
                                let mut case = mixed_case::<P>(kind, selected, ty, 0, false);
                                let original = case.source.clone();
                                let tree = parse(&case.source);
                                let count =
                                    usize::from(tree.function.is_some()) + tree.helpers.len();
                                for index in count..PROGRAM.max_functions {
                                    case.source.push_str(&format!(
                                        "\nfn padding_{index}() = div(left: 1, right: 0)\n"
                                    ));
                                }
                                assert!(run::<P>(&case, 10_000).complete);
                                admitted += 1;
                                let ninth_parameter = original
                                    .replace(PARAMETERS, &format!("{PARAMETERS}, extra: integer"))
                                    .replace(ARGUMENTS[7], &format!("{}, extra: 0", ARGUMENTS[7]));
                                let invalid = [
                                    format!("{}\nfn overflow() = 0\n", case.source),
                                    ninth_parameter,
                                    original.replace(&format!(", {}", ARGUMENTS[6]), ""),
                                    original.replace(ARGUMENTS[1], "number: 7"),
                                    original.replace(ARGUMENTS[6], "unknown: 1"),
                                    original.replace(ARGUMENTS[0], "number: true"),
                                    original.replace("unused: integer", "number: integer"),
                                    format!("{original}\nfn unbound() = caller\n"),
                                    format!("{original}\nfn recurse() = recurse()\n"),
                                    format!(
                                        "{original}\nfn recurse() = another()\nfn another() = recurse()\n"
                                    ),
                                    format!(
                                        "{original}\nfn recurse() = choose(when: false, then: recurse(), otherwise: 0)\n"
                                    ),
                                    format!("{original}\nfn back() = main()\n"),
                                    format!("{original}\nfn add() = 0\n"),
                                ];
                                for source in invalid {
                                    assert_ne!(source, original);
                                    fixture::<P>(|host, _| {
                                        assert!(compile(&source, &host).is_err(), "{source}");
                                        assert!(host.invoked.borrow().is_empty());
                                        assert_eq!(host.serial.get(), 0);
                                    });
                                    refused += 1;
                                }
                            }
                        }
                    }
                    (admitted, refused)
                }
                let counter = check::<Counter>();
                let panel = check::<Panel>();
                assert_eq!((counter.0 + panel.0, counter.1 + panel.1), (72, 936));
            }
        }

        mod control_acceptance {
            use super::*;

            const FORMS: [&str; 8] = [
                "helper",
                "loop",
                "zero-loop",
                "fold",
                "empty-fold",
                "nested",
                "recover-loop",
                "recover-fold",
            ];

            pub(super) fn scalar_source(ty: ScalarType) -> (&'static str, &'static str) {
                match ty {
                    ScalarType::Integer => ("integer", "0"),
                    ScalarType::Boolean => ("boolean", "false"),
                    ScalarType::String => ("string", "\"\""),
                    ScalarType::None => ("none", "none"),
                    ScalarType::OptionalString => {
                        ("optional_string", "optional_string(value: none)")
                    }
                    ScalarType::StringList => ("string_list", "strings()"),
                }
            }

            fn expression(ty: ScalarType, value: &str, form: &str) -> String {
                let (_, initial) = scalar_source(ty);
                let fault = "bind(fault: div(left: 1, right: 0), body: state)";
                let items = "strings(last: \"left\", first: \"right\")";
                let step = format!(
                    "choose(when: eq(left: piece, right: \"right\"), then: {value}, otherwise: {initial})"
                );
                match form {
                    "helper" => format!("typed(value: {value})"),
                    "loop" => format!(
                        "loop(state: {initial}, while: ne(left: state, right: {value}), next: typed(value: {value}), limit: 1)"
                    ),
                    "zero-loop" => {
                        format!("loop(state: {value}, while: false, next: {fault}, limit: 0)")
                    }
                    "fold" => format!(
                        "fold(state: {initial}, items: {items}, item: \"piece\", next: typed(value: {step}), limit: 2)"
                    ),
                    "empty-fold" => format!(
                        "fold(state: {value}, items: strings(), item: \"piece\", next: {fault}, limit: 0)"
                    ),
                    "nested" => format!(
                        "fold(state: {initial}, items: {items}, item: \"piece\", next: loop(cursor: state, while: ne(left: cursor, right: {step}), next: typed(value: {step}), limit: 1), limit: 2)"
                    ),
                    "recover-loop" => format!(
                        "recover(value: loop(state: {initial}, while: true, next: {fault}, limit: 1), fallback: typed(value: {value}))"
                    ),
                    "recover-fold" => format!(
                        "recover(value: fold(state: {initial}, items: {items}, item: \"piece\", next: {fault}, limit: 2), fallback: typed(value: {value}))"
                    ),
                    _ => panic!("unknown control acceptance form"),
                }
            }

            fn pipeline<P: Profile>(
                kind: &str,
                selected: usize,
                ty: ScalarType,
                make: impl FnOnce(&str) -> String,
            ) -> Case {
                let mut case = case::<P>(kind, selected, ty);
                let original = case
                    .source
                    .lines()
                    .find(|line| line.starts_with("fn convert(text: string) = "))
                    .unwrap()
                    .to_owned();
                let value = original
                    .strip_prefix("fn convert(text: string) = ")
                    .unwrap();
                assert_eq!(case.source.matches(&original).count(), 1);
                case.source = case.source.replace(
                    &original,
                    &format!("fn convert(text: string) = {}", make(value)),
                );
                let (name, _) = scalar_source(ty);
                // The helper's local deliberately collides with the caller's captured local.
                case.source.push_str(&format!(
                    "\nfn typed(value: {name}) = bind(caller: value, body: caller)\n"
                ));
                case
            }

            #[test]
            fn all_six_state_types_cross_helpers_iterations_and_successor_capture() {
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for kind in ["seq", "all"] {
                        for selected in 0..3 {
                            for ty in TYPES {
                                for form in FORMS {
                                    let case = pipeline::<P>(kind, selected, ty, |value| {
                                        expression(ty, value, form)
                                    });
                                    assert!(
                                        run::<P>(&case, 10_000).complete,
                                        "{kind}/{selected}/{ty:?}/{form}"
                                    );
                                    count += 1;
                                }
                            }
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 576);
            }

            #[test]
            fn control_state_paths_keep_the_exact_shared_fuel_threshold() {
                fn check<P: Profile>() {
                    for kind in ["seq", "all"] {
                        for selected in 0..3 {
                            for ty in TYPES {
                                for form in FORMS {
                                    let case = pipeline::<P>(kind, selected, ty, |value| {
                                        expression(ty, value, form)
                                    });
                                    let ample = run::<P>(&case, 10_000);
                                    assert!(ample.complete, "{ty:?}/{form}");
                                    let used = 10_000 - ample.remaining;
                                    assert!(!run::<P>(&case, used - 1).complete, "{ty:?}/{form}");
                                    for extra in 0..=1 {
                                        assert_eq!(
                                            run::<P>(&case, used + extra),
                                            Outcome {
                                                complete: true,
                                                remaining: extra,
                                                invoked: case.inputs.len(),
                                            },
                                            "{kind}/{selected}/{ty:?}/{form}"
                                        );
                                    }
                                }
                            }
                        }
                    }
                }
                check::<Counter>();
                check::<Panel>();
            }

            #[test]
            fn typed_loop_and_fold_limits_cannot_recover_into_a_successor() {
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for kind in ["seq", "all"] {
                        for selected in 0..3 {
                            for ty in TYPES {
                                let (_, initial) = scalar_source(ty);
                                for exhausted in [
                                    format!(
                                        "loop(state: {initial}, while: true, next: typed(value: state), limit: 1)"
                                    ),
                                    format!(
                                        "fold(state: {initial}, items: strings(a: \"a\", b: \"b\"), item: \"piece\", next: typed(value: state), limit: 1)"
                                    ),
                                ] {
                                    for recover in [false, true] {
                                        let case = pipeline::<P>(kind, selected, ty, |value| {
                                            if recover {
                                                format!(
                                                    "recover(value: {exhausted}, fallback: {value})"
                                                )
                                            } else {
                                                exhausted.clone()
                                            }
                                        });
                                        let outcome = run::<P>(&case, 10_000);
                                        assert!(!outcome.complete, "{ty:?}/{exhausted}/{recover}");
                                        assert!(
                                            outcome.remaining > 0 && outcome.remaining < 10_000
                                        );
                                        assert_eq!(outcome.invoked, case.inputs.len() - 1);
                                        count += 1;
                                    }
                                }
                            }
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 288);
            }

            #[test]
            fn cold_control_types_purity_and_lexical_scope_fail_before_host_work() {
                fn check<P: Profile>() -> usize {
                    let mut count = 0;
                    for kind in ["seq", "all"] {
                        for selected in 0..3 {
                            for ty in TYPES {
                                let (_, initial) = scalar_source(ty);
                                let wrong = if ty == ScalarType::Boolean {
                                    "0"
                                } else {
                                    "true"
                                };
                                let effect = if P::ARGUMENT == ScalarType::Integer {
                                    "counter.add(amount: 1)"
                                } else {
                                    "panel.set_text(text: \"forbidden\")"
                                };
                                for invalid in [
                                    format!(
                                        "loop(state: {initial}, while: 0, next: state, limit: 0)"
                                    ),
                                    format!(
                                        "loop(state: {initial}, while: false, next: {wrong}, limit: 0)"
                                    ),
                                    format!(
                                        "fold(state: {initial}, items: strings(), item: \"piece\", next: {wrong}, limit: 0)"
                                    ),
                                    format!(
                                        "fold(state: {initial}, items: false, item: \"piece\", next: state, limit: 0)"
                                    ),
                                    format!(
                                        "loop(state: {initial}, while: false, next: bind(blocked: {effect}, body: state), limit: 0)"
                                    ),
                                    format!(
                                        "fold(state: {initial}, items: strings(), item: \"piece\", next: bind(blocked: {effect}, body: state), limit: 0)"
                                    ),
                                    format!(
                                        "bind(done: loop(state: {initial}, while: false, next: state, limit: 0), body: state)"
                                    ),
                                    format!(
                                        "bind(done: fold(state: {initial}, items: strings(), item: \"piece\", next: state, limit: 0), body: piece)"
                                    ),
                                    format!("typed(value: {wrong})"),
                                ] {
                                    let case =
                                        pipeline::<P>(kind, selected, ty, |_| invalid.clone());
                                    fixture::<P>(|host, _| {
                                        assert!(compile(&case.source, &host).is_err(), "{invalid}");
                                        assert!(host.invoked.borrow().is_empty());
                                        assert_eq!(host.serial.get(), 0);
                                    });
                                    count += 1;
                                }
                            }
                        }
                    }
                    count
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 648);
            }
        }

        mod scalar_acceptance {
            use super::*;

            const BINARY: [BinaryOperator; 23] = [
                BinaryOperator::Add,
                BinaryOperator::Sub,
                BinaryOperator::Mul,
                BinaryOperator::Div,
                BinaryOperator::Rem,
                BinaryOperator::Eq,
                BinaryOperator::Ne,
                BinaryOperator::Lt,
                BinaryOperator::Le,
                BinaryOperator::Gt,
                BinaryOperator::Ge,
                BinaryOperator::And,
                BinaryOperator::Or,
                BinaryOperator::Concat,
                BinaryOperator::ValueOr,
                BinaryOperator::Contains,
                BinaryOperator::StartsWith,
                BinaryOperator::EndsWith,
                BinaryOperator::CharAt,
                BinaryOperator::Split,
                BinaryOperator::Join,
                BinaryOperator::Append,
                BinaryOperator::ItemAt,
            ];
            const UNARY: [UnaryOperator; 7] = [
                UnaryOperator::Not,
                UnaryOperator::Len,
                UnaryOperator::ToString,
                UnaryOperator::ParseInteger,
                UnaryOperator::ParseBoolean,
                UnaryOperator::OptionalString,
                UnaryOperator::HasValue,
            ];
            const INTEGER: &str = "parse_integer(value: text)";
            const BOOLEAN: &str = "eq(left: text, right: \"7\")";
            const NONE: &str =
                "choose(when: eq(left: text, right: \"7\"), then: none, otherwise: none)";
            const OPTIONAL: &str = "optional_string(value: text)";
            const LIST: &str = "strings(item: text)";

            fn optional(value: &str) -> ScalarValue {
                ScalarValue::OptionalString(OptionalStringValue(Some(value.into())))
            }
            fn list(values: &[&str]) -> ScalarValue {
                ScalarValue::StringList(StringListValue(
                    values.iter().map(|s| (*s).into()).collect(),
                ))
            }

            // Expected values are fixed acceptance data, not results from apply_*.
            fn signatures() -> Vec<(String, ScalarValue)> {
                use BinaryOperator::*;
                let mut probes = Vec::new();
                for operator in BINARY {
                    if matches!(operator, Eq | Ne) {
                        for value in [INTEGER, BOOLEAN, "text", NONE, OPTIONAL, LIST] {
                            probes.push((
                                format!("{}(left: {value}, right: {value})", operator.name()),
                                ScalarValue::Boolean(operator == Eq),
                            ));
                        }
                        continue;
                    }
                    let (left, right, expected) = match operator {
                        Add => (INTEGER, "3", ScalarValue::Integer(10)),
                        Sub => (INTEGER, "3", ScalarValue::Integer(4)),
                        Mul => (INTEGER, "3", ScalarValue::Integer(21)),
                        Div => (INTEGER, "3", ScalarValue::Integer(2)),
                        Rem => (INTEGER, "3", ScalarValue::Integer(1)),
                        Lt | Le => (INTEGER, "3", ScalarValue::Boolean(false)),
                        Gt | Ge => (INTEGER, "3", ScalarValue::Boolean(true)),
                        And => (BOOLEAN, "false", ScalarValue::Boolean(false)),
                        Or => (BOOLEAN, "false", ScalarValue::Boolean(true)),
                        Concat => ("text", "\"!\"", ScalarValue::String("7!".into())),
                        ValueOr => (OPTIONAL, "\"fallback\"", ScalarValue::String("7".into())),
                        Contains | StartsWith | EndsWith => {
                            ("text", "\"7\"", ScalarValue::Boolean(true))
                        }
                        CharAt => ("text", "0", optional("7")),
                        Split => (
                            "concat(left: text, right: \",x\")",
                            "\",\"",
                            list(&["7", "x"]),
                        ),
                        Join => (
                            "strings(a: text, b: \"x\")",
                            "\"/\"",
                            ScalarValue::String("7/x".into()),
                        ),
                        Append => (LIST, "\"x\"", list(&["7", "x"])),
                        ItemAt => (LIST, "0", optional("7")),
                        Eq | Ne => unreachable!("overloaded signatures handled above"),
                    };
                    probes.push((
                        format!("{}(left: {left}, right: {right})", operator.name()),
                        expected,
                    ));
                }
                assert_eq!(probes.len(), 33);
                for operator in UNARY {
                    let values = match operator {
                        UnaryOperator::Not => vec![(BOOLEAN, ScalarValue::Boolean(false))],
                        UnaryOperator::Len => vec![
                            ("text", ScalarValue::Integer(1)),
                            (LIST, ScalarValue::Integer(1)),
                        ],
                        UnaryOperator::ToString => vec![
                            (INTEGER, ScalarValue::String("7".into())),
                            (BOOLEAN, ScalarValue::String("true".into())),
                            ("text", ScalarValue::String("7".into())),
                        ],
                        UnaryOperator::ParseInteger => vec![("text", ScalarValue::Integer(7))],
                        UnaryOperator::ParseBoolean => vec![(
                            "choose(when: eq(left: text, right: \"7\"), then: \"true\", otherwise: \"false\")",
                            ScalarValue::Boolean(true),
                        )],
                        UnaryOperator::OptionalString => vec![
                            ("text", optional("7")),
                            (NONE, ScalarValue::OptionalString(OptionalStringValue(None))),
                        ],
                        UnaryOperator::HasValue => vec![(OPTIONAL, ScalarValue::Boolean(true))],
                    };
                    for (value, expected) in values {
                        probes.push((format!("{}(value: {value})", operator.name()), expected));
                    }
                }
                assert_eq!(probes.len(), 44);
                probes
            }

            fn pipeline<P: Profile>(
                kind: &str,
                selected: usize,
                expression: &str,
                expected: ScalarValue,
            ) -> Case {
                let mut case = case::<P>(kind, selected, ScalarType::String);
                let original = "fn convert(text: string) = concat(left: text, right: \"!\")";
                assert_eq!(case.source.matches(original).count(), 1);
                case.source = case.source.replace(
                    original,
                    &format!("fn convert(text: string) = {expression}"),
                );
                let (tail, input) = if P::ARGUMENT == ScalarType::Integer {
                    ("counter.add(amount: 1)", ScalarValue::Integer(1))
                } else {
                    (
                        "panel.set_text(text: \"ok\")",
                        ScalarValue::String("ok".into()),
                    )
                };
                let old_tail = format!("ack: {},", case.tail);
                assert_eq!(case.source.matches(&old_tail).count(), 1);
                case.source = case.source.replace(&old_tail, &format!("ack: {tail},"));
                case.tail = tail.into();
                *case.inputs.last_mut().unwrap() = input;
                case.output = expected;
                case
            }

            #[test]
            fn all_44_declared_scalar_signatures_cross_two_hosts_and_three_return_routes() {
                fn check<P: Profile>() -> usize {
                    let mut cases = 0;
                    for kind in ["seq", "all"] {
                        for selected in 0..3 {
                            for (expression, expected) in signatures() {
                                let case = pipeline::<P>(kind, selected, &expression, expected);
                                assert!(
                                    run::<P>(&case, 10_000).complete,
                                    "{kind}/{selected}/{expression}"
                                );
                                cases += 1;
                            }
                        }
                    }
                    cases
                }
                assert_eq!(check::<Counter>() + check::<Panel>(), 528);
            }

            #[test]
            fn all_scalar_signatures_preserve_the_exact_completion_fuel_threshold() {
                fn check<P: Profile>() {
                    for kind in ["seq", "all"] {
                        for selected in 0..3 {
                            for (expression, expected) in signatures() {
                                let case = pipeline::<P>(kind, selected, &expression, expected);
                                let ample = run::<P>(&case, 10_000);
                                assert!(ample.complete, "{expression}");
                                let used = 10_000 - ample.remaining;
                                assert!(!run::<P>(&case, used - 1).complete, "{expression}");
                                for extra in 0..=1 {
                                    assert_eq!(
                                        run::<P>(&case, used + extra),
                                        Outcome {
                                            complete: true,
                                            remaining: extra,
                                            invoked: case.inputs.len(),
                                        },
                                        "{kind}/{selected}/{expression}"
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
            fn scalar_shape_and_type_errors_are_cold_for_every_operator_and_host_route() {
                let mut valid = Vec::new();
                let mut invalid = Vec::new();
                for operator in BINARY {
                    use BinaryOperator::*;
                    let (left, right) = match operator {
                        And | Or => (BOOLEAN, "false"),
                        ValueOr => (OPTIONAL, "text"),
                        Concat | Contains | StartsWith | EndsWith | Split => ("text", "\"7\""),
                        CharAt => ("text", "0"),
                        Join | Append => (LIST, "text"),
                        ItemAt => (LIST, "0"),
                        Add | Sub | Mul | Div | Rem | Eq | Ne | Lt | Le | Gt | Ge => (INTEGER, "3"),
                    };
                    valid.push(format!("{}(left: {left}, right: {right})", operator.name()));
                    for arguments in [
                        format!("left: {left}"),
                        format!("left: {left}, left: {left}, right: {right}"),
                        format!("left: {left}, right: {right}, extra: 0"),
                        "left: true, right: 7".into(),
                    ] {
                        invalid.push(format!("{}({arguments})", operator.name()));
                    }
                }
                for operator in UNARY {
                    let value = match operator {
                        UnaryOperator::Not => BOOLEAN,
                        UnaryOperator::HasValue => OPTIONAL,
                        UnaryOperator::ParseBoolean => "\"true\"",
                        UnaryOperator::ToString
                        | UnaryOperator::Len
                        | UnaryOperator::ParseInteger
                        | UnaryOperator::OptionalString => "text",
                    };
                    valid.push(format!("{}(value: {value})", operator.name()));
                    let wrong_type = match operator {
                        UnaryOperator::Not | UnaryOperator::HasValue => "7",
                        UnaryOperator::ToString => "none",
                        UnaryOperator::Len
                        | UnaryOperator::ParseInteger
                        | UnaryOperator::ParseBoolean
                        | UnaryOperator::OptionalString => "true",
                    };
                    for arguments in [
                        String::new(),
                        format!("value: {value}, value: {value}"),
                        format!("value: {value}, extra: 0"),
                        format!("value: {wrong_type}"),
                    ] {
                        invalid.push(format!("{}({arguments})", operator.name()));
                    }
                }
                assert_eq!(valid.len(), 30);
                assert_eq!(invalid.len(), 120);
                fn check<P: Profile>(valid: &[String], invalid: &[String]) -> usize {
                    let mut cases = 0;
                    for kind in ["seq", "all"] {
                        for selected in 0..3 {
                            // Prove the operands compile before mutating only the call shape.
                            for expression in valid {
                                let case =
                                    pipeline::<P>(kind, selected, expression, ScalarValue::None);
                                fixture::<P>(|host, _| {
                                    assert!(compile(&case.source, &host).is_ok(), "{expression}");
                                    assert!(host.invoked.borrow().is_empty());
                                    assert_eq!(host.serial.get(), 0);
                                });
                            }
                            for expression in invalid {
                                let case =
                                    pipeline::<P>(kind, selected, expression, ScalarValue::None);
                                fixture::<P>(|host, _| {
                                    assert!(compile(&case.source, &host).is_err(), "{expression}");
                                    assert!(host.invoked.borrow().is_empty());
                                    assert_eq!(host.serial.get(), 0);
                                });
                                cases += 1;
                            }
                        }
                    }
                    cases
                }
                assert_eq!(
                    check::<Counter>(&valid, &invalid) + check::<Panel>(&valid, &invalid),
                    1440
                );
            }

            #[test]
            fn scalar_dynamic_failures_after_a_reply_never_dispatch_the_successor() {
                let large = "x".repeat(MAX_SCALAR_STRING_BYTES);
                let failures = [
                    format!("div(left: {INTEGER}, right: 0)"),
                    format!("rem(left: {INTEGER}, right: 0)"),
                    format!("sub(left: 0, right: {INTEGER})"),
                    format!("add(left: {INTEGER}, right: {})", u64::MAX),
                    format!("mul(left: {INTEGER}, right: {})", u64::MAX),
                    "parse_integer(value: concat(left: text, right: \"x\"))".into(),
                    "parse_boolean(value: text)".into(),
                    format!("concat(left: text, right: \"{large}\")"),
                    format!("append(left: strings(item: \"{large}\"), right: text)"),
                ];
                fn check<P: Profile>(failures: &[String]) {
                    for kind in ["seq", "all"] {
                        for selected in 0..3 {
                            for expression in failures {
                                let case =
                                    pipeline::<P>(kind, selected, expression, ScalarValue::None);
                                let outcome = run::<P>(&case, 1_000_000);
                                assert!(!outcome.complete, "{expression}");
                                assert!(outcome.remaining > 0, "dynamic failure, not exhaustion");
                                assert_eq!(outcome.invoked, case.inputs.len() - 1, "{expression}");
                            }
                        }
                    }
                }
                check::<Counter>(&failures);
                check::<Panel>(&failures);
            }

            #[test]
            fn scalar_lazy_and_recovery_paths_survive_capture_without_hiding_cold_errors() {
                let probes = [
                    (
                        "and(left: false, right: parse_boolean(value: text))",
                        ScalarValue::Boolean(false),
                    ),
                    (
                        "or(left: true, right: parse_boolean(value: text))",
                        ScalarValue::Boolean(true),
                    ),
                    (
                        "value_or(left: optional_string(value: text), right: to_string(value: div(left: 1, right: 0)))",
                        ScalarValue::String("7".into()),
                    ),
                    (
                        "recover(value: div(left: 1, right: 0), fallback: parse_integer(value: text))",
                        ScalarValue::Integer(7),
                    ),
                    (
                        "recover(value: parse_integer(value: text), fallback: div(left: 1, right: 0))",
                        ScalarValue::Integer(7),
                    ),
                ];
                fn check<P: Profile>(probes: &[(&str, ScalarValue)]) {
                    for kind in ["seq", "all"] {
                        for selected in 0..3 {
                            for (expression, expected) in probes {
                                let case =
                                    pipeline::<P>(kind, selected, expression, expected.clone());
                                assert!(run::<P>(&case, 10_000).complete, "{expression}");
                                let invalid = case.source.replace(
                                    "fn main() =",
                                    "fn unused() = and(left: false, right: missing)\nfn main() =",
                                );
                                fixture::<P>(|host, _| {
                                    assert!(compile(&invalid, &host).is_err());
                                    assert!(host.invoked.borrow().is_empty());
                                    assert_eq!(host.serial.get(), 0);
                                });
                            }
                        }
                    }
                }
                check::<Counter>(&probes);
                check::<Panel>(&probes);
            }
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
            let node = compile(&case.source, &host)
                .unwrap_or_else(|error| panic!("{error}\n{}", case.source));
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

    fn computed_projection_case<P: Profile>(
        kind: &str,
        left: bool,
        early: bool,
        form: usize,
        operand: bool,
    ) -> ConditionalCase {
        let mut case = conditional_case::<P>(kind, left, early);
        let right = "member(value: alias, name: \"right\")";
        let other = "member(value: alias, name: \"left\")";
        let input = match form {
            0 => format!("choose(when: true, then: {right}, otherwise: {other})"),
            1 => format!("choose(when: false, then: {other}, otherwise: {right})"),
            2 => format!(
                "bind(picked: {right}, body: choose(when: true, then: picked, otherwise: {other}))"
            ),
            3 => format!(
                "choose(when: recover(value: eq(left: div(left: 1, right: 0), right: 0), fallback: true), then: {right}, otherwise: {other})"
            ),
            _ => panic!(),
        };
        let field = if P::ARGUMENT == ScalarType::Integer {
            "value"
        } else {
            "text"
        };
        let projection = format!("field(value: {input}, name: \"{field}\")");
        case.source = case.source.replace(
            &format!("field(value: {right}, name: \"{field}\")"),
            &projection,
        );
        if operand {
            case.source = case.source.replace(
                ": selected, seed: seed)",
                &format!(": {projection}, seed: seed)"),
            );
        }
        case
    }

    #[test]
    fn computed_result_fields_join_two_host_group_reply_and_successor_pipelines() {
        fn check<P: Profile>() {
            let mut cases = 0;
            for kind in ["seq", "all"] {
                for left in [false, true] {
                    for early in [false, true] {
                        for form in 0..4 {
                            for operand in [false, true] {
                                if early && operand {
                                    continue;
                                }
                                let case =
                                    computed_projection_case::<P>(kind, left, early, form, operand);
                                assert!(run_conditional::<P>(&case, 10_000).is_ok());
                                cases += 1;
                            }
                        }
                    }
                }
            }
            assert_eq!(cases, 48);
        }
        check::<Counter>();
        check::<Panel>();
    }

    #[test]
    fn computed_result_fields_keep_the_original_meter_at_every_pipeline_cutoff() {
        fn check<P: Profile>() {
            for kind in ["seq", "all"] {
                for form in 0..4 {
                    let case = computed_projection_case::<P>(kind, true, false, form, true);
                    let used = 10_000 - run_conditional::<P>(&case, 10_000).unwrap();
                    for budget in 0..used {
                        assert!(
                            run_conditional::<P>(&case, budget).is_err(),
                            "{kind}/{form}/{budget}"
                        );
                    }
                    assert_eq!(run_conditional::<P>(&case, used), Ok(0));
                    assert_eq!(run_conditional::<P>(&case, used + 1), Ok(1));
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

    fn iteration_expression<P: Profile>(form: usize) -> String {
        let field = if P::ARGUMENT == ScalarType::Integer {
            "value"
        } else {
            "text"
        };
        let initial = if P::ARGUMENT == ScalarType::Integer {
            "0"
        } else {
            "\"\""
        };
        let projection = |condition: &str, otherwise: &str| {
            format!(
                "field(value: bind(picked: choose(when: {condition}, then: member(value: alias, name: \"right\"), otherwise: member(value: alias, name: \"{otherwise}\")), body: picked), name: \"{field}\")"
            )
        };
        match form {
            0 => {
                let index = if P::ARGUMENT == ScalarType::Integer {
                    "cursor"
                } else {
                    "len(value: cursor)"
                };
                let selected = projection(&format!("eq(left: {index}, right: 0)"), "right");
                let (bound, next) = if P::ARGUMENT == ScalarType::Integer {
                    (selected, "add(left: cursor, right: 1)".into())
                } else {
                    (
                        format!("len(value: {selected})"),
                        format!(
                            "concat(left: cursor, right: value_or(left: char_at(left: {selected}, right: {index}), right: \"\"))"
                        ),
                    )
                };
                format!(
                    "loop(cursor: {initial}, while: lt(left: {index}, right: {bound}), next: {next}, limit: 8)"
                )
            }
            1 | 2 => {
                let condition = if form == 1 {
                    "eq(left: piece, right: \"right\")"
                } else {
                    "pick_piece(piece: piece)"
                };
                let selected = projection(condition, "left");
                let next = if form == 1 {
                    selected
                } else {
                    format!(
                        "loop(cursor: state, while: ne(left: cursor, right: {selected}), next: {selected}, limit: 1)"
                    )
                };
                format!(
                    "fold(state: {initial}, items: strings(a: \"left\", b: \"right\"), item: \"piece\", next: {next}, limit: 2)"
                )
            }
            _ => panic!(),
        }
    }

    fn iteration_case<P: Profile>(
        kind: &str,
        left: bool,
        early: bool,
        form: usize,
        operand: bool,
    ) -> ConditionalCase {
        let mut case = conditional_case::<P>(kind, left, early);
        let field = if P::ARGUMENT == ScalarType::Integer {
            "value"
        } else {
            "text"
        };
        let original =
            format!("field(value: member(value: alias, name: \"right\"), name: \"{field}\")");
        let calculation = iteration_expression::<P>(form);
        assert_eq!(case.source.matches(&original).count(), 1);
        case.source = case.source.replace(&original, &calculation);
        if operand {
            case.source = case.source.replace(
                ": selected, seed: seed)",
                &format!(": {calculation}, seed: seed)"),
            );
        }
        if form == 2 {
            case.source
                .push_str("\nfn pick_piece(piece: string) = eq(left: piece, right: \"right\")\n");
        }
        case
    }

    #[test]
    fn captured_group_iterations_join_result_selection_and_successor_operands_for_two_hosts() {
        fn check<P: Profile>() {
            let mut cases = 0;
            for kind in ["seq", "all"] {
                for left in [false, true] {
                    for early in [false, true] {
                        for form in 0..3 {
                            for operand in [false, true] {
                                if early && operand {
                                    continue;
                                }
                                let case = iteration_case::<P>(kind, left, early, form, operand);
                                assert!(run_conditional::<P>(&case, 10_000).is_ok());
                                cases += 1;
                            }
                        }
                    }
                }
            }
            assert_eq!(cases, 36);
        }
        check::<Counter>();
        check::<Panel>();
    }

    #[test]
    fn captured_group_iterations_keep_the_original_meter_at_every_pipeline_cutoff() {
        fn check<P: Profile>() {
            for kind in ["seq", "all"] {
                for form in 0..3 {
                    let case = iteration_case::<P>(kind, true, false, form, true);
                    let used = 10_000 - run_conditional::<P>(&case, 10_000).unwrap();
                    for budget in 0..used {
                        assert!(
                            run_conditional::<P>(&case, budget).is_err(),
                            "{kind}/{form}/{budget}"
                        );
                    }
                    assert_eq!(run_conditional::<P>(&case, used), Ok(0));
                    assert_eq!(run_conditional::<P>(&case, used + 1), Ok(1));
                }
            }
        }
        check::<Counter>();
        check::<Panel>();
    }

    #[test]
    fn captured_group_iteration_limits_cannot_be_recovered_into_a_successor() {
        fn check<P: Profile>() {
            for kind in ["seq", "all"] {
                for early in [false, true] {
                    for form in 0..3 {
                        for recover in [false, true] {
                            let mut case = iteration_case::<P>(kind, true, early, form, false);
                            let expression = iteration_expression::<P>(form);
                            let limit = match form {
                                0 => "limit: 8",
                                1 => "limit: 2",
                                _ => "limit: 1",
                            };
                            assert!(expression.contains(limit));
                            let mut refused = expression.replace(limit, "limit: 0");
                            if recover {
                                let fallback = if P::ARGUMENT == ScalarType::Integer {
                                    "0"
                                } else {
                                    "\"fallback\""
                                };
                                refused =
                                    format!("recover(value: {refused}, fallback: {fallback})");
                            }
                            case.source = case.source.replace(&expression, &refused);
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
                                for _ in 0..2 {
                                    let EffectSessionPoll::Request(active) =
                                        vm.poll(&runtime).unwrap()
                                    else {
                                        panic!()
                                    };
                                    let reply = runtime.collect(active).unwrap();
                                    vm.try_accept(&reply.identity.clone(), reply, &runtime)
                                        .unwrap();
                                }
                                let before = vm.fuel_remaining();
                                assert!(vm.poll(&runtime).is_err());
                                assert!(vm.fuel_remaining() < before);
                                let remaining = vm.fuel_remaining();
                                assert_eq!(
                                    vm.status(),
                                    EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
                                );
                                assert!(!vm.lease().is_open());
                                assert!(matches!(
                                    vm.poll(&runtime).unwrap(),
                                    EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
                                ));
                                assert_eq!(vm.fuel_remaining(), remaining);
                                assert_eq!(*host.invoked.borrow(), case.inputs[..3]);
                            });
                        }
                    }
                }
            }
        }
        check::<Counter>();
        check::<Panel>();
    }

    #[test]
    fn captured_group_iterations_reject_cold_exports_effects_and_escaped_locals() {
        fn check<P: Profile>() {
            for kind in ["seq", "all"] {
                for form in 0..3 {
                    let case = iteration_case::<P>(kind, true, false, form, false);
                    let expression = iteration_expression::<P>(form);
                    let field = if P::ARGUMENT == ScalarType::Integer {
                        "value"
                    } else {
                        "text"
                    };
                    let initial = if P::ARGUMENT == ScalarType::Integer {
                        "0"
                    } else {
                        "\"\""
                    };
                    let invalid = format!(
                        "field(value: choose(when: true, then: member(value: alias, name: \"right\"), otherwise: member(value: alias, name: \"missing\")), name: \"{field}\")"
                    );
                    let effect = if P::ARGUMENT == ScalarType::Integer {
                        "step(n: 1)"
                    } else {
                        "step(text: \"unsafe\")"
                    };
                    for replacement in [
                        format!("loop(cursor: {initial}, while: false, next: {invalid}, limit: 0)"),
                        format!(
                            "fold(state: {initial}, items: strings(), item: \"piece\", next: {invalid}, limit: 0)"
                        ),
                        format!("loop(cursor: {initial}, while: false, next: {effect}, limit: 0)"),
                        "cursor".into(),
                        "piece".into(),
                    ] {
                        fixture::<P>(|host, _scope| {
                            let source = case.source.replace(&expression, &replacement);
                            assert_ne!(source, case.source);
                            assert!(compile(&source, &host).is_err());
                            assert!(host.invoked.borrow().is_empty());
                            assert_eq!(host.serial.get(), 0);
                        });
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
                        for case in
                            std::iter::once(conditional_case::<P>(kind, false, early))
                                .chain((0..4).map(|form| {
                                    computed_projection_case::<P>(kind, false, early, form, true)
                                }))
                                .chain((0..3).map(|form| {
                                    iteration_case::<P>(kind, false, early, form, true)
                                }))
                        {
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
                                for _ in 0..2 {
                                    let EffectSessionPoll::Request(active) =
                                        vm.poll(&runtime).unwrap()
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
