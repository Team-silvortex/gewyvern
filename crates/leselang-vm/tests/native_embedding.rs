//! Two scalar host schemas through the native facade, not complete VM extraction.
use std::{borrow::Borrow, cell::Cell, rc::Rc};

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
use leselang_vm::native::{LocalVm, Vm, core::*, hir::*, syntax::parse};
use pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationLimits, PureValue, scalar_copy_cost,
};
use pure_typing::{PureType, PureTypeEnvironment};
use source_call::{SourceCallHost, SourceCallLimits};

struct Field;

#[test]
fn native_local_vm_revokes_before_initial_value_cleanup_and_keeps_cancel_sticky_on_unwind() {
    struct Native<'a> {
        lease: LocalExecutionLease,
        drops: &'a Cell<u8>,
        unwind: bool,
    }
    impl Drop for Native<'_> {
        fn drop(&mut self) {
            assert!(!self.lease.is_open());
            self.drops.set(self.drops.get() + 1);
            assert!(!self.unwind, "native cleanup unwind");
        }
    }
    for phase in 0..3 {
        let root = ();
        let drops = Cell::new(0);
        let scope = LocalExecutionScope::new();
        let lease = scope.lease();
        let mut session: LocalVm<'_, '_, (), Native<'_>, (), u8, (), ()> = LocalVm::new(
            scope,
            &root,
            vec![(
                "native",
                PureValue::Result(Native {
                    lease: lease.clone(),
                    drops: &drops,
                    unwind: phase == 2,
                }),
            )],
            Fuel::new(10),
            STACK,
        );
        assert!(lease.is_open());
        assert!(format!("{session:?}").contains("fuel_remaining: 10"));
        if phase == 0 {
            drop(session);
        } else {
            let result =
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| session.cancel()));
            if phase == 2 {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap());
            }
            assert_eq!(
                session.status(),
                EffectSessionStatus::Terminal(EffectSessionEnd::Cancelled)
            );
            assert!(!session.cancel());
            assert_eq!(session.fuel_remaining(), 10);
        }
        assert!(!lease.is_open());
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn opt_in_local_vm_does_not_remove_send_from_the_original_generic_vm() {
    fn assert_send<T: Send>() {}
    assert_send::<Vm<'static, 'static, (), u8, (), u8, (), ()>>();
}

struct Opaque;
struct Tag;
struct Refusal;
struct Domain(ScalarType);
impl HostResultDomain<ScalarValue> for Domain {
    type Error = Refusal;
    fn matches_type(&self, reply: &ScalarValue) -> bool {
        reply.scalar_type() == self.0
    }
    fn validate_value(&self, reply: &ScalarValue) -> Result<(), Refusal> {
        ScalarTypeSet::only(self.0)
            .validate(reply)
            .map_err(|_| Refusal)
    }
}
type Node = Computation<Field, String, Opaque, Tag>;
type Catalog<'s> = OperationCatalog<'s, String, &'s str, ScalarTypeSet, Domain, u8>;
type Request<'s> = PreparedCall<'s, String, ScalarTypeSet, Domain, u8>;

// Opaque receipt/identity ownership is not Clone, serde, Send, or Debug constrained.
struct Reply {
    value: ScalarValue,
    buffer: Box<u8>,
}
impl Borrow<ScalarValue> for Reply {
    fn borrow(&self) -> &ScalarValue {
        &self.value
    }
}
struct Identity {
    owner: Rc<()>,
    serial: u64,
}
impl PartialEq for Identity {
    fn eq(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.owner, &other.owner) && self.serial == other.serial
    }
}
struct Host<'h, 's> {
    catalog: &'h Catalog<'s>,
    owner: Rc<()>,
    version: Cell<u32>,
    granted: Cell<bool>,
    serial: Cell<u64>,
    invoked: Cell<usize>,
}
impl<'h, 's> Host<'h, 's> {
    fn new(catalog: &'h Catalog<'s>) -> Self {
        Self {
            catalog,
            owner: Rc::new(()),
            version: Cell::new(catalog.version()),
            granted: Cell::new(true),
            serial: Cell::new(0),
            invoked: Cell::new(0),
        }
    }
    fn source(&self) -> SourceCallHost<'_, 's, String, ScalarTypeSet, Domain, u8> {
        SourceCallHost {
            catalog: self.catalog,
            version: self.version.get(),
            granted: if self.granted.get() { &[7] } else { &[] },
        }
    }
    fn identity(&self) -> Identity {
        Identity {
            owner: self.owner.clone(),
            serial: self.serial.get(),
        }
    }
    fn invoke(&self, request: Request<'_>) -> Reply {
        let schema = self
            .source()
            .catalog
            .authorize(
                &request.schema().key,
                self.version.get(),
                self.source().granted,
            )
            .unwrap();
        assert!(std::ptr::eq(schema, request.schema()));
        self.invoked.set(self.invoked.get() + 1);
        let (_, arguments) = request.into_parts();
        Reply {
            value: arguments.into_iter().next().unwrap().value,
            buffer: Box::new(9),
        }
    }
}
impl PureTypeEnvironment<Field, String> for Host<'_, '_> {
    type Result = ();
    fn field_type(&self, _: &(), _: &Field) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &String) -> Option<()> {
        None
    }
}
impl<'s> CallFlowEnvironment<'s, Field, String, Domain> for Host<'_, 's> {
    fn call_result_type(&self, _: &String, declaration: &'s Domain) -> Option<PureType<()>> {
        Some(PureType::Scalar(declaration.0))
    }
}
impl<'source, 's> CatalogSourceEnvironment<'source, 's, Field, String, Domain> for Host<'_, 's> {
    type Error = Refusal;
    fn source_operation(&self, original: &String) -> Result<String, Refusal> {
        Ok(original.clone())
    }
    fn source_field(
        &self,
        _: &(),
        _: &'source str,
    ) -> Result<Option<(Field, ScalarType)>, Refusal> {
        Ok(None)
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
impl PureEvaluationEnvironment<Field, String> for Host<'_, '_> {
    type Result = ();
    type Error = Refusal;
    fn field(&self, _: &(), _: &Field) -> Result<ScalarValue, Refusal> {
        Err(Refusal)
    }
    fn member(&self, _: &(), _: &str, _: &String) -> Result<(), Refusal> {
        Err(Refusal)
    }
}
const EFFECTS: EffectEvaluationLimits = EffectEvaluationLimits {
    pure: PureEvaluationLimits {
        max_nodes: 256,
        max_depth: 32,
        max_bindings: 8,
    },
    max_arguments: 8,
    max_branches: 8,
};
const STACK: EffectStackLimits = EffectStackLimits {
    effects: EFFECTS,
    max_frames: 8,
};
const PROGRAM: CatalogProgramLimits = CatalogProgramLimits {
    expression: CatalogSourceLimits {
        control: ControlSourceLimits {
            source: SourceCallLimits {
                max_source_nodes: 256,
                max_source_depth: 32,
                max_lowered_nodes: 256,
                max_lowered_depth: 32,
                max_arguments: 8,
            },
            max_bindings: 8,
        },
        max_calls: 8,
    },
    max_functions: 8,
    max_parameters: 8,
    max_compilation_nodes: 1024,
    max_expansion_depth: 16,
};
impl<'ir, 's> EffectEvaluationEnvironment<'ir, Field, String, Opaque, Tag> for Host<'_, 's> {
    type Request = Request<'s>;
    type Capture = RestoredEffectBindings<'ir, ()>;
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
                    .map(|arg| arg.name.as_str())
                    .collect::<Vec<_>>(),
            )
            .map_err(|_| Refusal)?;
        Ok(())
    }
    fn prepare_effect(
        &self,
        node: &'ir Node,
        scope: &mut ScopeFrame<'_, 'ir, PureValue<()>>,
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
                pure: EFFECTS.pure,
                max_arguments: 8,
            },
        )
        .map_err(|_| Refusal.into())
    }
    fn capture(
        &self,
        _: &'ir str,
        _: &'ir Node,
        scope: &ScopeFrame<'_, 'ir, PureValue<()>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Capture, Refusal> {
        let cost = scope
            .bindings()
            .iter()
            .map(|(_, value)| match value {
                PureValue::Scalar(value) => 1 + scalar_copy_cost(value),
                _ => 1,
            })
            .sum();
        fuel.charge(cost).map_err(|_| Refusal)?;
        Ok(scope.bindings().to_vec())
    }
}
impl ReplyAuthority<Identity> for Host<'_, '_> {
    type Error = Refusal;
    fn authorize(&self, identity: &Identity) -> Result<(), Refusal> {
        if !Rc::ptr_eq(&identity.owner, &self.owner)
            || identity.serial == 0
            || identity.serial > self.serial.get()
            || !self.granted.get()
            || self.version.get() != self.catalog.version()
        {
            return Err(Refusal);
        }
        Ok(())
    }
}
impl<'ir, 's> EffectReentryEnvironment<'ir, Field, String, Opaque, Tag, Identity, Domain, Reply>
    for Host<'_, 's>
{
    fn restore_capture(
        &self,
        identity: &Identity,
        _: &Domain,
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
        declaration: &Domain,
        reply: Reply,
        _: &mut Fuel,
    ) -> Result<PureValue<()>, CalculationFailure<Refusal>> {
        self.authorize(identity)?;
        declaration.validate_value(&reply.value)?;
        Ok(PureValue::Scalar(reply.value))
    }
}
impl<'ir, 's> EffectSessionEnvironment<'ir, 's, Field, String, Opaque, Tag> for Host<'_, 's> {
    type Identity = Identity;
    type Declaration = Domain;
    type Reply = Reply;
    type Dispatch = Request<'s>;
    fn correlate_request(
        &self,
        request: Self::Request,
        _: &mut Fuel,
    ) -> EffectCorrelation<'s, Identity, Domain, Self::Dispatch, Refusal> {
        let source = self.source();
        let schema = source
            .catalog
            .authorize(&request.schema().key, source.version, source.granted)
            .map_err(|_| Refusal)?;
        if !std::ptr::eq(schema, request.schema()) {
            return Err(Refusal.into());
        }
        self.serial
            .set(self.serial.get().checked_add(1).ok_or(Refusal)?);
        Ok(CorrelatedEffectRequest {
            identity: self.identity(),
            declaration: &schema.result,
            dispatch: request,
        })
    }
}
fn compile(text: &str, host: &Host<'_, '_>) -> Result<Node, String> {
    let tree = parse(text);
    if !tree.diagnostics.is_empty() {
        return Err("syntax".into());
    }
    let declarations: Vec<_> = tree.function.iter().chain(&tree.helpers).collect();
    compile_catalog_program(&declarations, "main", PROGRAM, &host.source(), host)
        .map(|(node, _)| node)
        .map_err(|error| error.to_string())
}
fn with_host(ty: ScalarType, name: &str, test: impl FnOnce(&Host<'_, '_>)) {
    let parameters = [NamedParameter::required("n", ScalarTypeSet::only(ty))];
    let schemas = [OperationSchema {
        key: name.to_owned(),
        parameters: &parameters,
        result: Domain(ty),
        required_capability: 7,
    }];
    let catalog = OperationCatalog::new(
        19,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    test(&Host::new(&catalog));
}

#[test]
fn native_facade_two_scalar_hosts_compile_helpers_yield_capture_and_reenter_one_meter() {
    for (ty, name, text, expected) in [
        (
            ScalarType::Integer,
            "counter.add",
            "fn bump(n: integer) = bind(receipt: counter.add(n: n), body: add(left: receipt, right: 1))\nfn main() = bind(seed: 20, body: bind(first: bump(n: add(left: seed, right: 20)), body: bump(n: first)))",
            ScalarValue::Integer(42),
        ),
        (
            ScalarType::String,
            "panel.echo",
            "fn echo(n: string) = bind(receipt: panel.echo(n: n), body: receipt)\nfn main() = bind(first: echo(n: \"ready\"), body: echo(n: first))",
            ScalarValue::String("ready".into()),
        ),
    ] {
        with_host(ty, name, |host| {
            let node = compile(text, host).unwrap();
            let mut session = Vm::new(&node, Vec::new(), Fuel::new(1000), STACK);
            let mut remaining = session.fuel_remaining();
            for _ in 0..2 {
                let EffectSessionPoll::Request(request) = session.poll(host).unwrap() else {
                    panic!()
                };
                assert!(session.fuel_remaining() < remaining);
                remaining = session.fuel_remaining();
                assert!(matches!(
                    session.poll(host).unwrap(),
                    EffectSessionPoll::Awaiting
                ));
                assert_eq!(session.fuel_remaining(), remaining);
                let identity = host.identity();
                session
                    .try_accept(&identity, host.invoke(request), host)
                    .unwrap();
                assert_eq!(session.fuel_remaining(), remaining);
            }
            let EffectSessionPoll::Value(PureValue::Scalar(value)) = session.poll(host).unwrap()
            else {
                panic!()
            };
            assert_eq!(value, expected);
            assert_eq!(host.invoked.get(), 2);
            assert!(session.fuel_remaining() < remaining);
            assert!(matches!(
                session.poll(host).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
            ));
            assert_eq!(host.invoked.get(), 2);
        });
    }
}

#[test]
fn native_facade_cold_unknown_calls_wrong_domains_versions_and_denied_capabilities_never_dispatch()
{
    with_host(ScalarType::Integer, "counter.add", |host| {
        for text in [
            "fn main() = choose(when: true, then: 1, otherwise: missing(n: 1))",
            "fn main() = counter.add(n: true)",
            "fn main() = counter.add(wrong: 1)",
            "fn main() = 1\nfn unused() = missing()",
        ] {
            assert!(compile(text, host).is_err());
        }
        host.version.set(20);
        assert!(compile("fn main() = counter.add(n: 1)", host).is_err());
        host.version.set(19);
        host.granted.set(false);
        assert!(compile("fn main() = counter.add(n: 1)", host).is_err());
        assert_eq!(host.serial.get(), 0);
        assert_eq!(host.invoked.get(), 0);
    });
}

#[test]
fn native_facade_rejected_receipts_retain_owned_buffers_and_original_pending_request() {
    with_host(ScalarType::Integer, "counter.add", |host| {
        let node = compile(
            "fn main() = bind(receipt: counter.add(n: 41), body: add(left: receipt, right: 1))",
            host,
        )
        .unwrap();
        let mut session = Vm::new(&node, Vec::new(), Fuel::new(1000), STACK);
        let EffectSessionPoll::Request(request) = session.poll(host).unwrap() else {
            panic!()
        };
        let fuel = session.fuel_remaining();
        let identity = host.identity();
        let reply = Reply {
            value: ScalarValue::Boolean(true),
            buffer: Box::new(3),
        };
        let pointer = reply.buffer.as_ref() as *const u8;
        let rejected = session.try_accept(&identity, reply, host).err().unwrap();
        assert_eq!(rejected.reply.buffer.as_ref() as *const u8, pointer);
        let stale = Identity {
            owner: host.owner.clone(),
            serial: 0,
        };
        let reply = host.invoke(request);
        let pointer = reply.buffer.as_ref() as *const u8;
        let rejected = session.try_accept(&stale, reply, host).err().unwrap();
        assert_eq!(rejected.reply.buffer.as_ref() as *const u8, pointer);
        assert_eq!(session.status(), EffectSessionStatus::Awaiting);
        assert_eq!(session.fuel_remaining(), fuel);
        session.try_accept(&identity, rejected.reply, host).unwrap();
        assert!(matches!(
            session.poll(host).unwrap(),
            EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(42)))
        ));
    });
}

#[test]
fn native_facade_live_reentry_rechecks_policy_without_replaying_settled_effects() {
    with_host(ScalarType::Integer, "counter.add", |host| {
        let node = compile(
            "fn main() = bind(first: counter.add(n: 1), body: counter.add(n: first))",
            host,
        )
        .unwrap();
        for revoke_version in [false, true] {
            host.version.set(19);
            host.granted.set(true);
            let mut session = Vm::new(&node, Vec::new(), Fuel::new(1000), STACK);
            let EffectSessionPoll::Request(request) = session.poll(host).unwrap() else {
                panic!()
            };
            session
                .try_accept(&host.identity(), host.invoke(request), host)
                .unwrap();
            let invocations = host.invoked.get();
            let serial = host.serial.get();
            if revoke_version {
                host.version.set(20);
            } else {
                host.granted.set(false);
            }
            assert!(session.poll(host).is_err());
            assert_eq!(
                session.status(),
                EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
            );
            assert!(matches!(
                session.poll(host).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
            ));
            assert_eq!(host.invoked.get(), invocations);
            assert_eq!(host.serial.get(), serial);
        }
    });
}

#[test]
fn native_facade_cancel_before_dispatch_pending_or_accepted_never_enters_the_tail() {
    with_host(ScalarType::Integer, "counter.add", |host| {
        let node = compile(
            "fn main() = bind(first: counter.add(n: 1), body: counter.add(n: first))",
            host,
        )
        .unwrap();
        for phase in 0..3 {
            let mut session = Vm::new(&node, Vec::new(), Fuel::new(1000), STACK);
            if phase > 0 {
                let EffectSessionPoll::Request(request) = session.poll(host).unwrap() else {
                    panic!()
                };
                if phase == 2 {
                    session
                        .try_accept(&host.identity(), host.invoke(request), host)
                        .unwrap();
                }
            }
            let fuel = session.fuel_remaining();
            let serial = host.serial.get();
            let invoked = host.invoked.get();
            assert!(session.cancel());
            assert!(!session.cancel());
            assert!(matches!(
                session.poll(host).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
            ));
            assert_eq!(session.fuel_remaining(), fuel);
            assert_eq!(host.serial.get(), serial);
            assert_eq!(host.invoked.get(), invoked);
        }
    });
}

#[test]
fn native_facade_fuel_exhaustion_is_terminal_without_hidden_refill_or_dispatch() {
    with_host(ScalarType::Integer, "counter.add", |host| {
        let node = compile("fn main() = counter.add(n: 1)", host).unwrap();
        let mut session = Vm::new(&node, Vec::new(), Fuel::new(0), STACK);
        assert!(session.poll(host).is_err());
        assert_eq!(session.fuel_remaining(), 0);
        assert!(matches!(
            session.poll(host).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
        ));
        assert_eq!(host.serial.get(), 0);
        assert_eq!(host.invoked.get(), 0);
    });
}

#[test]
fn native_facade_independent_roots_cannot_cross_accept_equal_local_serials() {
    with_host(ScalarType::Integer, "counter.add", |host| {
        let other = Host::new(host.catalog);
        let node = compile("fn main() = counter.add(n: 7)", host).unwrap();
        let mut first = Vm::new(&node, Vec::new(), Fuel::new(1000), STACK);
        let mut second = Vm::new(&node, Vec::new(), Fuel::new(1000), STACK);
        let EffectSessionPoll::Request(request) = first.poll(host).unwrap() else {
            panic!()
        };
        let EffectSessionPoll::Request(_) = second.poll(&other).unwrap() else {
            panic!()
        };
        assert_eq!(host.serial.get(), other.serial.get());
        let identity = host.identity();
        let reply = host.invoke(request);
        let pointer = reply.buffer.as_ref() as *const u8;
        let rejected = second.try_accept(&identity, reply, &other).err().unwrap();
        assert_eq!(rejected.reply.buffer.as_ref() as *const u8, pointer);
        assert_eq!(second.status(), EffectSessionStatus::Awaiting);
        first.try_accept(&identity, rejected.reply, host).unwrap();
        assert!(matches!(
            first.poll(host).unwrap(),
            EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(7)))
        ));
        assert!(second.cancel());
        assert_eq!(other.invoked.get(), 0);
    });
}

#[test]
fn native_facade_string_reply_bounds_cannot_be_bypassed_by_correct_type() {
    with_host(ScalarType::String, "panel.echo", |host| {
        let node = compile("fn main() = panel.echo(n: \"ready\")", host).unwrap();
        let mut session = Vm::new(&node, Vec::new(), Fuel::new(1000), STACK);
        let EffectSessionPoll::Request(request) = session.poll(host).unwrap() else {
            panic!()
        };
        let identity = host.identity();
        let reply = Reply {
            value: ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1)),
            buffer: Box::new(1),
        };
        let pointer = reply.buffer.as_ref() as *const u8;
        let rejected = session.try_accept(&identity, reply, host).err().unwrap();
        assert_eq!(rejected.reply.buffer.as_ref() as *const u8, pointer);
        assert_eq!(session.status(), EffectSessionStatus::Awaiting);
        session
            .try_accept(&identity, host.invoke(request), host)
            .unwrap();
        assert!(
            matches!(session.poll(host).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::String(value))) if value == "ready")
        );
    });
}

#[test]
fn native_facade_two_pending_roots_on_one_host_can_complete_out_of_order() {
    with_host(ScalarType::Integer, "counter.add", |host| {
        let one = compile("fn main() = counter.add(n: 1)", host).unwrap();
        let two = compile("fn main() = counter.add(n: 2)", host).unwrap();
        let mut first = Vm::new(&one, Vec::new(), Fuel::new(1000), STACK);
        let mut second = Vm::new(&two, Vec::new(), Fuel::new(1000), STACK);
        let EffectSessionPoll::Request(first_request) = first.poll(host).unwrap() else {
            panic!()
        };
        let first_id = host.identity();
        let EffectSessionPoll::Request(second_request) = second.poll(host).unwrap() else {
            panic!()
        };
        let second_id = host.identity();
        assert_ne!(first_id.serial, second_id.serial);
        second
            .try_accept(&second_id, host.invoke(second_request), host)
            .unwrap();
        assert!(matches!(
            second.poll(host).unwrap(),
            EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(2)))
        ));
        assert_eq!(first.status(), EffectSessionStatus::Awaiting);
        first
            .try_accept(&first_id, host.invoke(first_request), host)
            .unwrap();
        assert!(matches!(
            first.poll(host).unwrap(),
            EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(1)))
        ));
        assert_eq!(host.invoked.get(), 2);
    });
}
