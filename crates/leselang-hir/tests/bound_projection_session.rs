use std::borrow::Borrow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use leselang_hir::call_evaluation::*;
use leselang_hir::catalog_program::{CatalogProgramLimits, compile_catalog_program};
use leselang_hir::catalog_source::*;
use leselang_hir::control_source::*;
use leselang_hir::effect_evaluation::*;
use leselang_hir::effect_reentry::*;
use leselang_hir::effect_session::*;
use leselang_hir::flow_typing::CallFlowEnvironment;
use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::{PureEvaluationEnvironment, PureEvaluationLimits, PureValue};
use leselang_hir::pure_typing::{PureType, PureTypeEnvironment};
use leselang_hir::source_call::{SourceCallError, SourceCallHost, SourceCallLimits, SourceSchema};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, parse};

struct Layout {
    owner: Rc<()>,
}
impl PartialEq for Layout {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
struct Record<'schema> {
    layout: &'schema Layout,
    count: u64,
    ready: bool,
}
enum ReplyData<'schema> {
    Record(Rc<Record<'schema>>),
    Scalar(ScalarValue),
}
struct Input<'schema> {
    data: ReplyData<'schema>,
    buffer: Box<[u8]>,
}
impl<'schema> Borrow<ReplyData<'schema>> for Input<'schema> {
    fn borrow(&self) -> &ReplyData<'schema> {
        &self.data
    }
}
struct Domain<'schema> {
    layout: Option<&'schema Layout>,
    checks: Rc<Cell<usize>>,
}
struct PrivateError(&'static str);
impl<'schema> HostResultDomain<ReplyData<'schema>> for Domain<'schema> {
    type Error = PrivateError;
    fn matches_type(&self, value: &ReplyData<'schema>) -> bool {
        matches!(
            (self.layout, value),
            (Some(_), ReplyData::Record(_)) | (None, ReplyData::Scalar(ScalarValue::Integer(_)))
        )
    }
    fn validate_value(&self, value: &ReplyData<'schema>) -> Result<(), PrivateError> {
        self.checks.set(self.checks.get() + 1);
        match (self.layout, value) {
            (Some(layout), ReplyData::Record(record))
                if std::ptr::eq(layout, record.layout) && record.count <= 100 =>
            {
                Ok(())
            }
            (None, ReplyData::Scalar(ScalarValue::Integer(value))) if *value <= 200 => Ok(()),
            _ => Err(PrivateError("foreign schema or native value domain")),
        }
    }
}
type Node = Computation<u8, &'static str, Rc<()>, ()>;
type Schema<'schema> = SourceSchema<'schema, &'static str, ScalarTypeSet, Domain<'schema>, u8>;
const SOURCE: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 256,
    max_source_depth: 32,
    max_lowered_nodes: 256,
    max_lowered_depth: 32,
    max_arguments: 64,
};
const EXECUTION: EffectEvaluationLimits = EffectEvaluationLimits {
    pure: PureEvaluationLimits {
        max_nodes: 256,
        max_depth: 32,
        max_bindings: 8,
    },
    max_arguments: 64,
    max_branches: 64,
};
struct Host<'host, 'schema> {
    source: SourceCallHost<'host, 'schema, &'static str, ScalarTypeSet, Domain<'schema>, u8>,
    layout: &'schema Layout,
    current: Cell<u64>,
    ready: Cell<bool>,
    calls: RefCell<Vec<&'static str>>,
    queries: RefCell<Vec<&'static str>>,
}
impl Host<'_, '_> {
    fn field_key(&self, result: &Layout, name: &str) -> Option<(u8, ScalarType)> {
        if !std::ptr::eq(result, self.layout) || !Rc::ptr_eq(&result.owner, &self.layout.owner) {
            return None;
        }
        match name {
            "count" => Some((0, ScalarType::Integer)),
            "ready" => Some((1, ScalarType::Boolean)),
            _ => None,
        }
    }
}
impl<'schema> PureTypeEnvironment<u8, &'static str> for Host<'_, 'schema> {
    type Result = &'schema Layout;
    fn field_type(&self, result: &&'schema Layout, field: &u8) -> Option<ScalarType> {
        if !std::ptr::eq(*result, self.layout) {
            return None;
        }
        match field {
            0 => Some(ScalarType::Integer),
            1 => Some(ScalarType::Boolean),
            _ => None,
        }
    }
    fn member_result(
        &self,
        _: &&'schema Layout,
        _: &str,
        _: &&'static str,
    ) -> Option<&'schema Layout> {
        None
    }
}
impl<'schema> CallFlowEnvironment<'schema, u8, &'static str, Domain<'schema>>
    for Host<'_, 'schema>
{
    fn call_result_type(
        &self,
        _: &&'static str,
        domain: &'schema Domain<'schema>,
    ) -> Option<PureType<&'schema Layout>> {
        Some(match domain.layout {
            Some(layout) => PureType::Result(layout),
            None => PureType::Scalar(ScalarType::Integer),
        })
    }
}
impl<'source, 'schema> CatalogSourceEnvironment<'source, 'schema, u8, &'static str, Domain<'schema>>
    for Host<'_, 'schema>
{
    type Error = PrivateError;
    fn source_operation(&self, original: &&'static str) -> Result<&'static str, PrivateError> {
        Ok(*original)
    }
    fn source_field(
        &self,
        result: &&'schema Layout,
        name: &'source str,
    ) -> Result<Option<(u8, ScalarType)>, PrivateError> {
        Ok(self.field_key(result, name))
    }
    fn source_member(
        &self,
        _: &&'schema Layout,
        _: &'source str,
        _: &'source str,
    ) -> Result<Option<(&'static str, &'schema Layout)>, PrivateError> {
        Ok(None)
    }
}
impl<'schema> Host<'_, 'schema> {
    fn compile(
        &self,
        source: &Expression,
    ) -> CatalogSourceResult<Node, &'schema Layout, PrivateError> {
        compile_catalog_source(
            source,
            &[],
            CatalogSourceLimits {
                control: ControlSourceLimits {
                    source: SOURCE,
                    max_bindings: 8,
                },
                max_calls: 64,
            },
            &self.source,
            self,
        )
    }
}

impl<'schema> PureEvaluationEnvironment<u8, &'static str> for Host<'_, 'schema> {
    type Result = Rc<Record<'schema>>;
    type Error = PrivateError;
    fn field(&self, result: &Self::Result, field: &u8) -> Result<ScalarValue, PrivateError> {
        if !std::ptr::eq(result.layout, self.layout) {
            return Err(PrivateError("foreign native view"));
        }
        match field {
            0 => {
                self.queries.borrow_mut().push("count");
                Ok(ScalarValue::Integer(result.count))
            }
            1 => {
                self.queries.borrow_mut().push("ready");
                Ok(ScalarValue::Boolean(result.ready))
            }
            _ => Err(PrivateError("unknown native field")),
        }
    }
    fn member(
        &self,
        _: &Self::Result,
        _: &str,
        _: &&'static str,
    ) -> Result<Self::Result, PrivateError> {
        Err(PrivateError("no groups"))
    }
}
impl<'ir, 'schema> EffectEvaluationEnvironment<'ir, u8, &'static str, Rc<()>, ()>
    for Host<'_, 'schema>
{
    type Request = PreparedCall<'schema, &'static str, ScalarTypeSet, Domain<'schema>, u8>;
    type Capture = RestoredEffectBindings<'ir, Rc<Record<'schema>>>;
    fn preflight_effect(&self, node: &Node) -> Result<(), PrivateError> {
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err(PrivateError("unsupported effect"));
        };
        let schema = self
            .source
            .catalog
            .authorize(operation, self.source.version, self.source.granted)
            .map_err(|_| PrivateError("cold native policy"))?;
        schema
            .bind_arguments(
                &arguments
                    .iter()
                    .map(|arg| arg.name.as_str())
                    .collect::<Vec<_>>(),
            )
            .map_err(|_| PrivateError("cold native signature"))?;
        Ok(())
    }
    fn prepare_effect(
        &self,
        node: &'ir Node,
        scope: &mut ScopeFrame<'_, 'ir, PureValue<Rc<Record<'schema>>>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Request, CalculationFailure<PrivateError>> {
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err(PrivateError("unsupported effect").into());
        };
        let schema = self
            .source
            .catalog
            .authorize(operation, self.source.version, self.source.granted)
            .map_err(|_| PrivateError("live native policy"))?;
        evaluate_call_arguments_in_scope(
            arguments,
            schema,
            scope,
            self,
            fuel,
            CallEvaluationLimits {
                pure: EXECUTION.pure,
                max_arguments: 64,
            },
        )
        .map_err(|_| PrivateError("native call preparation rejected").into())
    }
    fn capture(
        &self,
        _: &'ir str,
        _: &'ir Node,
        scope: &ScopeFrame<'_, 'ir, PureValue<Rc<Record<'schema>>>>,
        _: &mut Fuel,
    ) -> Result<Self::Capture, PrivateError> {
        if !scope.is_empty() {
            return Err(PrivateError("this host only supports empty native capture"));
        }
        Ok(Vec::new())
    }
}
impl ReplyAuthority<(u64, &'static str)> for Host<'_, '_> {
    type Error = PrivateError;
    fn authorize(&self, identity: &(u64, &'static str)) -> Result<(), PrivateError> {
        if identity.0 != 17 {
            return Err(PrivateError("wrong execution generation"));
        }
        self.source
            .catalog
            .authorize(&identity.1, self.source.version, self.source.granted)
            .map(|_| ())
            .map_err(|_| PrivateError("live native authority"))
    }
}
impl<'ir, 'schema>
    EffectReentryEnvironment<
        'ir,
        u8,
        &'static str,
        Rc<()>,
        (),
        (u64, &'static str),
        Domain<'schema>,
        Input<'schema>,
    > for Host<'_, 'schema>
{
    fn restore_capture(
        &self,
        identity: &(u64, &'static str),
        _: &Domain<'schema>,
        capture: Self::Capture,
        _: &mut Fuel,
        _: EffectEvaluationLimits,
    ) -> Result<Self::Capture, CalculationFailure<PrivateError>> {
        self.authorize(identity)?;
        Ok(capture)
    }
    fn bind_reply(
        &self,
        identity: &(u64, &'static str),
        domain: &Domain<'schema>,
        input: Input<'schema>,
        _: &mut Fuel,
    ) -> Result<PureValue<Rc<Record<'schema>>>, CalculationFailure<PrivateError>> {
        let original = self
            .source
            .catalog
            .authorize(&identity.1, self.source.version, self.source.granted)
            .map_err(|_| PrivateError("live native projection policy"))?;
        if !std::ptr::eq(domain, &original.result) {
            return Err(PrivateError("wrong original result domain").into());
        }
        domain.validate_value(&input.data)?;
        Ok(match input.data {
            ReplyData::Record(value) => PureValue::Result(value),
            ReplyData::Scalar(value) => PureValue::Scalar(value),
        })
    }
}
impl<'ir, 'schema> EffectSessionEnvironment<'ir, 'schema, u8, &'static str, Rc<()>, ()>
    for Host<'_, 'schema>
{
    type Identity = (u64, &'static str);
    type Declaration = Domain<'schema>;
    type Reply = Input<'schema>;
    type Dispatch = Self::Request;
    fn correlate_request(
        &self,
        request: Self::Request,
        _: &mut Fuel,
    ) -> EffectCorrelation<'schema, Self::Identity, Self::Declaration, Self::Dispatch, PrivateError>
    {
        let schema = self
            .source
            .catalog
            .authorize(
                &request.schema().key,
                self.source.version,
                self.source.granted,
            )
            .map_err(|_| PrivateError("live native correlation policy"))?;
        if !std::ptr::eq(schema, request.schema()) {
            return Err(PrivateError("wrong original schema").into());
        }
        Ok(CorrelatedEffectRequest {
            identity: (17, schema.key),
            declaration: &schema.result,
            dispatch: request,
        })
    }
}
impl<'schema> Host<'_, 'schema> {
    fn invoke(
        &self,
        request: PreparedCall<'schema, &'static str, ScalarTypeSet, Domain<'schema>, u8>,
    ) -> Input<'schema> {
        let schema = self
            .source
            .catalog
            .authorize(
                &request.schema().key,
                self.source.version,
                self.source.granted,
            )
            .unwrap();
        assert!(std::ptr::eq(schema, request.schema()));
        self.calls.borrow_mut().push(schema.key);
        let data = match schema.key {
            "device.read" => ReplyData::Record(Rc::new(Record {
                layout: self.layout,
                count: self.current.get(),
                ready: self.ready.get(),
            })),
            "device.write" => {
                let ScalarValue::Integer(value) = request.arguments()[0].value else {
                    panic!()
                };
                self.current.set(value);
                ReplyData::Scalar(ScalarValue::Integer(value))
            }
            _ => panic!(),
        };
        Input {
            data,
            buffer: vec![7; 33].into_boxed_slice(),
        }
    }
}

fn expression(source: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {source}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}

const PROGRAM: &str = "bind(snapshot: device.read(), body: choose(when: field(value: snapshot, name: \"ready\"), then: device.write(value: add(left: field(value: snapshot, name: \"count\"), right: 1)), otherwise: device.write(value: 0)))";
fn with_host(test: impl for<'host, 'schema> FnOnce(&mut Host<'host, 'schema>, &'schema Layout)) {
    let layout = Layout { owner: Rc::new(()) };
    let foreign = Layout { owner: Rc::new(()) };
    let checks = Rc::new(Cell::new(0));
    let parameters = [NamedParameter::required(
        "value",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas: [Schema<'_>; 2] = [
        OperationSchema {
            key: "device.read",
            parameters: &[],
            required_capability: 31,
            result: Domain {
                layout: Some(&layout),
                checks: checks.clone(),
            },
        },
        OperationSchema {
            key: "device.write",
            parameters: &parameters,
            required_capability: 32,
            result: Domain {
                layout: None,
                checks: checks.clone(),
            },
        },
    ];
    let catalog = OperationCatalog::new(
        7,
        &schemas,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let mut host = Host {
        source: SourceCallHost {
            catalog: &catalog,
            version: 7,
            granted: &[31, 32],
        },
        layout: &layout,
        current: Cell::new(41),
        ready: Cell::new(true),
        calls: RefCell::new(Vec::new()),
        queries: RefCell::new(Vec::new()),
    };
    test(&mut host, &foreign);
}

#[test]
fn parsed_record_reply_fields_drive_native_branch_arguments_with_exact_fuel_and_no_snapshot_copy() {
    with_host(|host, _| {
        let input = expression(PROGRAM);
        let (program, ty) = host.compile(&input).unwrap();
        assert!(matches!(ty, PureType::Scalar(ScalarType::Integer)));
        for ready in [true, false] {
            host.current.set(41);
            host.ready.set(ready);
            host.calls.borrow_mut().clear();
            host.queries.borrow_mut().clear();
            let mut session = EffectSession::new(&program, Vec::new(), Fuel::new(100), EXECUTION);
            let EffectSessionPoll::Request(read) = session.poll(host).unwrap() else {
                panic!()
            };
            let schema = read.schema();
            let reply = host.invoke(read);
            let pointer = reply.buffer.as_ptr();
            let ReplyData::Record(record) = &reply.data else {
                panic!()
            };
            let weak = Rc::downgrade(record);
            let rejected = session
                .try_accept(&(18, "device.read"), reply, host)
                .unwrap_err();
            assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
            assert!(std::ptr::eq(schema.result.layout.unwrap(), host.layout));
            assert!(matches!(
                session.poll(host).unwrap(),
                EffectSessionPoll::Awaiting
            ));
            session
                .try_accept(&(17, "device.read"), rejected.reply, host)
                .unwrap();
            assert_eq!(session.fuel_remaining(), 98);
            assert!(
                host.queries.borrow().is_empty(),
                "acceptance must not project fields"
            );
            assert_eq!(weak.strong_count(), 1);
            let EffectSessionPoll::Request(write) = session.poll(host).unwrap() else {
                panic!()
            };
            assert_eq!(
                write.arguments()[0].value,
                ScalarValue::Integer(if ready { 42 } else { 0 })
            );
            assert_eq!(session.fuel_remaining(), if ready { 90 } else { 93 });
            assert_eq!(
                weak.strong_count(),
                0,
                "native record views must not outlive pure argument preparation"
            );
            assert_eq!(
                *host.queries.borrow(),
                if ready {
                    vec!["ready", "count"]
                } else {
                    vec!["ready"]
                }
            );
            session
                .try_accept(&(17, "device.write"), host.invoke(write), host)
                .unwrap();
            assert!(
                matches!(session.poll(host).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(actual))) if actual == if ready { 42 } else { 0 })
            );
            assert_eq!(*host.calls.borrow(), ["device.read", "device.write"]);
        }
    });
}

#[test]
fn foreign_record_domain_and_cancellation_before_field_projection_preserve_native_reply_ownership()
{
    with_host(|host, foreign| {
        let (program, _) = host.compile(&expression(PROGRAM)).unwrap();
        let mut session = EffectSession::new(&program, Vec::new(), Fuel::new(100), EXECUTION);
        let EffectSessionPoll::Request(read) = session.poll(host).unwrap() else {
            panic!()
        };
        let reply = host.invoke(read);
        let wrong = Input {
            data: ReplyData::Record(Rc::new(Record {
                layout: foreign,
                count: 41,
                ready: true,
            })),
            buffer: vec![2; 33].into_boxed_slice(),
        };
        let pointer = wrong.buffer.as_ptr();
        let rejected = session
            .try_accept(&(17, "device.read"), wrong, host)
            .unwrap_err();
        assert_eq!(rejected.reply.buffer.as_ptr(), pointer);
        assert!(matches!(session.status(), EffectSessionStatus::Awaiting));
        assert!(host.queries.borrow().is_empty());
        drop(rejected);
        let ReplyData::Record(record) = &reply.data else {
            panic!()
        };
        let weak = Rc::downgrade(record);
        session
            .try_accept(&(17, "device.read"), reply, host)
            .unwrap();
        assert!(session.cancel());
        assert_eq!(weak.strong_count(), 0);
        assert!(matches!(
            session.poll(host).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
        ));
        assert!(host.queries.borrow().is_empty());
        assert_eq!(*host.calls.borrow(), ["device.read"]);
        assert_eq!(host.current.get(), 41);
    });
}

#[test]
fn whole_native_source_typing_rejects_foreign_fields_unbound_aliases_and_cold_grants_before_calls()
{
    with_host(|host, _| {
        for text in [
            "bind(snapshot: device.read(), body: field(value: snapshot, name: \"missing\"))",
            "field(value: snapshot, name: \"count\")",
            "bind(value: device.write(value: 0), body: field(value: value, name: \"count\"))",
            "bind(snapshot: device.read(), body: choose(when: true, then: device.write(value: 0), otherwise: device.write(value: field(value: snapshot, name: \"ready\"))))",
        ] {
            assert!(host.compile(&expression(text)).is_err());
        }
        host.source.granted = &[31];
        assert!(host.compile(&expression(PROGRAM)).is_err());
        host.source.granted = &[31, 32];
        host.source.version = 8;
        let error = host.compile(&expression(PROGRAM)).err().unwrap();
        let CatalogSourceError::Call { error, .. } = error else {
            panic!()
        };
        assert!(matches!(
            *error,
            SourceCallError::Catalog(OperationCatalogError::UnsupportedVersion)
        ));
        assert!(host.calls.borrow().is_empty());
        assert!(host.queries.borrow().is_empty());
    });
}

#[test]
fn post_acceptance_live_policy_and_native_view_identity_fail_before_write_without_retry() {
    with_host(|host, foreign| {
        let original = host.layout;
        let (program, _) = host.compile(&expression(PROGRAM)).unwrap();
        for revoke in [true, false] {
            host.layout = original;
            host.source.granted = &[31, 32];
            host.calls.borrow_mut().clear();
            host.queries.borrow_mut().clear();
            let mut session = EffectSession::new(&program, Vec::new(), Fuel::new(100), EXECUTION);
            let EffectSessionPoll::Request(read) = session.poll(host).unwrap() else {
                panic!()
            };
            let input = host.invoke(read);
            let ReplyData::Record(record) = &input.data else {
                panic!()
            };
            let weak = Rc::downgrade(record);
            session
                .try_accept(&(17, "device.read"), input, host)
                .unwrap();
            if revoke {
                host.source.granted = &[31];
            } else {
                host.layout = foreign;
            }
            assert!(session.poll(host).is_err());
            assert!(matches!(
                session.status(),
                EffectSessionStatus::Terminal(EffectSessionEnd::Failed)
            ));
            assert!(matches!(
                session.poll(host).unwrap(),
                EffectSessionPoll::Terminal(EffectSessionEnd::Failed)
            ));
            assert_eq!(weak.strong_count(), 0);
            assert!(host.queries.borrow().is_empty());
            assert_eq!(*host.calls.borrow(), ["device.read"]);
            assert_eq!(host.current.get(), 41);
        }
    });
}

#[test]
fn catalog_native_error_payload_is_matchable_but_never_formatted_or_chained() {
    let error = CatalogSourceError::Mapping {
        span: leselang_syntax::Span { start: 0, end: 1 },
        error: PrivateError("sensitive native mapping detail"),
    };
    assert!(!format!("{error:?}").contains("sensitive"));
    assert!(!error.to_string().contains("sensitive"));
    assert!(std::error::Error::source(&error).is_none());
    let CatalogSourceError::Mapping {
        error: PrivateError(reason),
        ..
    } = error
    else {
        panic!()
    };
    assert_eq!(reason, "sensitive native mapping detail");
}

#[test]
fn multifunction_record_host_preserves_original_reply_fields_live_policy_and_cancel_ownership() {
    with_host(|host, _| {
        let parsed = parse(
            "fn main() = bind(snapshot: read(), body: choose(when: field(value: snapshot, name: \"ready\"), then: write(value: add(left: field(value: snapshot, name: \"count\"), right: 1)), otherwise: write(value: 0)))\nfn read() = device.read()\nfn write(value: integer) = device.write(value: value)",
        );
        assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
        let declarations: Vec<_> = parsed.function.iter().chain(&parsed.helpers).collect();
        let (program, ty): (Node, _) = compile_catalog_program(
            &declarations,
            "main",
            CatalogProgramLimits {
                expression: CatalogSourceLimits {
                    control: ControlSourceLimits {
                        source: SOURCE,
                        max_bindings: 8,
                    },
                    max_calls: 64,
                },
                max_functions: 32,
                max_parameters: 8,
                max_compilation_nodes: 4096,
                max_expansion_depth: 32,
            },
            &host.source,
            host,
        )
        .unwrap();
        assert!(matches!(ty, PureType::Scalar(ScalarType::Integer)));
        for ready in [true, false] {
            host.ready.set(ready);
            host.current.set(41);
            host.calls.borrow_mut().clear();
            host.queries.borrow_mut().clear();
            let mut session = EffectSession::new(&program, Vec::new(), Fuel::new(1000), EXECUTION);
            let EffectSessionPoll::Request(read) = session.poll(host).unwrap() else {
                panic!()
            };
            let reply = host.invoke(read);
            let ReplyData::Record(record) = &reply.data else {
                panic!()
            };
            let weak = Rc::downgrade(record);
            session
                .try_accept(&(17, "device.read"), reply, host)
                .unwrap();
            assert!(host.queries.borrow().is_empty());
            let EffectSessionPoll::Request(write) = session.poll(host).unwrap() else {
                panic!()
            };
            assert_eq!(
                write.arguments()[0].value,
                ScalarValue::Integer(if ready { 42 } else { 0 })
            );
            assert_eq!(weak.strong_count(), 0);
            session
                .try_accept(&(17, "device.write"), host.invoke(write), host)
                .unwrap();
            assert!(
                matches!(session.poll(host).unwrap(), EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(n))) if n == if ready {42} else {0})
            );
            assert_eq!(*host.calls.borrow(), ["device.read", "device.write"]);
        }
        host.queries.borrow_mut().clear();
        let mut cancelled = EffectSession::new(&program, Vec::new(), Fuel::new(1000), EXECUTION);
        let EffectSessionPoll::Request(read) = cancelled.poll(host).unwrap() else {
            panic!()
        };
        let reply = host.invoke(read);
        let ReplyData::Record(record) = &reply.data else {
            panic!()
        };
        let weak = Rc::downgrade(record);
        cancelled
            .try_accept(&(17, "device.read"), reply, host)
            .unwrap();
        assert!(cancelled.cancel());
        assert_eq!(weak.strong_count(), 0);
        assert!(host.queries.borrow().is_empty());
        assert!(matches!(
            cancelled.poll(host).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
        ));
        let mut revoked = EffectSession::new(&program, Vec::new(), Fuel::new(1000), EXECUTION);
        let EffectSessionPoll::Request(read) = revoked.poll(host).unwrap() else {
            panic!()
        };
        revoked
            .try_accept(&(17, "device.read"), host.invoke(read), host)
            .unwrap();
        host.source.granted = &[31];
        let before = host.calls.borrow().len();
        assert!(revoked.poll(host).is_err());
        assert_eq!(host.calls.borrow().len(), before);
        host.source.granted = &[31, 32];
    });
}
