use std::borrow::Borrow;
use std::cell::{Cell, RefCell};
use std::rc::Rc;

use leselang_hir::bound_projection_source::*;
use leselang_hir::call_evaluation::*;
use leselang_hir::call_typing::{CallTypeHost, CallTypeLimits};
use leselang_hir::control_source::*;
use leselang_hir::effect_evaluation::*;
use leselang_hir::effect_reentry::*;
use leselang_hir::effect_session::*;
use leselang_hir::flow_typing::{CallFlowEnvironment, CallFlowTypeLimits, infer_call_flow_type};
use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::{PureEvaluationEnvironment, PureEvaluationLimits, PureValue};
use leselang_hir::pure_typing::{PureType, PureTypeEnvironment, TypeInferenceLimits};
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::{
    SourceCallHost, SourceCallLimits, SourceSchema, lower_source_call,
};
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
impl<'source, 'schema> BoundProjectionSourceEnvironment<'source, u8, &'static str>
    for &Host<'_, 'schema>
{
    type Result = &'schema Layout;
    type Error = PrivateError;
    fn field(
        &mut self,
        result: &&'schema Layout,
        name: &'source str,
    ) -> Result<Option<(u8, ScalarType)>, PrivateError> {
        Ok(self.field_key(result, name))
    }
    fn member(
        &mut self,
        _: &&'schema Layout,
        _: &'source str,
        _: &'source str,
    ) -> Result<Option<(&'static str, &'schema Layout)>, PrivateError> {
        Ok(None)
    }
}
impl<'schema> Host<'_, 'schema> {
    fn scalar<'source>(
        &self,
        source: &'source Expression,
        bindings: &[(&'source str, &PureType<&'schema Layout>)],
    ) -> Result<(Node, ScalarType), PrivateError> {
        let prefix = bindings
            .iter()
            .map(|(name, ty)| {
                (
                    *name,
                    match ty {
                        PureType::Scalar(ty) => *ty,
                        PureType::Result(_) => ScalarType::None,
                    },
                )
            })
            .collect::<Vec<_>>();
        let mut projection = self;
        lower_scalar_source_with_scope(
            source,
            ScalarSourceLimits {
                source: SOURCE,
                max_bindings: 8,
            },
            &prefix,
            |child, _| {
                let (node, ty) = lower_bound_projection_source(
                    child,
                    bindings,
                    BoundProjectionSourceLimits {
                        source: SOURCE,
                        max_bindings: 8,
                    },
                    &mut projection,
                )
                .map_err(|_| PrivateError("bound native projection source rejected"))?;
                let PureType::Scalar(ty) = ty else {
                    return Err(PrivateError("group result is not scalar"));
                };
                Ok((node, Some(ty)))
            },
        )
        .map_err(|_| PrivateError("native scalar source rejected"))
    }
}
impl<'source, 'schema> ControlSourceAdapter<'source, u8, &'static str, Rc<()>, ()>
    for Host<'_, 'schema>
{
    type Type = PureType<&'schema Layout>;
    type Error = PrivateError;
    fn lower_leaf(
        &mut self,
        source: &'source Expression,
        scope: ControlSourceScope<'_, 'source, Self::Type>,
    ) -> Result<(Node, Self::Type), PrivateError> {
        if matches!(source, Expression::Call { callee, .. } if callee.starts_with("device.")) {
            let lowered = lower_source_call(source, &self.source, SOURCE, |arg| {
                self.scalar(&arg.value, scope.bindings())
                    .map(|(node, ty)| (node, Some(ty)))
            })
            .map_err(|_| PrivateError("native source catalog rejected"))?;
            let schema = lowered.schema();
            let ty = self
                .call_result_type(&schema.key, &schema.result)
                .ok_or(PrivateError("missing result observation"))?;
            return Ok((
                Node::Call {
                    operation: schema.key,
                    arguments: lowered.into_arguments(),
                },
                ty,
            ));
        }
        if matches!(source, Expression::Call { callee, .. } if matches!(callee.as_str(), "field" | "member"))
        {
            return lower_bound_projection_source(
                source,
                scope.bindings(),
                BoundProjectionSourceLimits {
                    source: SOURCE,
                    max_bindings: 8,
                },
                &mut &*self,
            )
            .map_err(|_| PrivateError("bound projection source rejected"));
        }
        self.scalar(source, scope.bindings())
            .map(|(node, ty)| (node, PureType::Scalar(ty)))
    }
    fn boolean(&mut self, _: &Node, ty: &Self::Type) -> Result<bool, PrivateError> {
        Ok(matches!(ty, PureType::Scalar(ScalarType::Boolean)))
    }
    fn same(&mut self, left: &Self::Type, right: &Self::Type) -> Result<bool, PrivateError> {
        Ok(left == right)
    }
    fn admit(
        &mut self,
        _: &'source Expression,
        node: &Node,
        ty: &Self::Type,
        scope: ControlSourceScope<'_, 'source, Self::Type>,
    ) -> Result<(), PrivateError> {
        let prefix = scope
            .bindings()
            .iter()
            .map(|(name, ty)| (*name, (*ty).clone()))
            .collect::<Vec<_>>();
        let host = CallTypeHost {
            catalog: self.source.catalog,
            version: self.source.version,
            granted: self.source.granted,
            environment: &*self,
        };
        let actual = infer_call_flow_type(
            node,
            &prefix,
            &*self,
            &host,
            CallFlowTypeLimits {
                call: CallTypeLimits {
                    pure: TypeInferenceLimits {
                        max_nodes: 256,
                        max_depth: 32,
                        max_bindings: 8,
                    },
                    max_arguments: 64,
                },
                max_calls: 64,
            },
        )
        .map_err(|_| PrivateError("whole native field flow admission rejected"))?;
        if &actual == ty {
            Ok(())
        } else {
            Err(PrivateError("wrong result observation"))
        }
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
        let (program, ty) = lower_control_source(
            &input,
            &[],
            ControlSourceLimits {
                source: SOURCE,
                max_bindings: 8,
            },
            host,
        )
        .unwrap();
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
        let (program, _) = lower_control_source(
            &expression(PROGRAM),
            &[],
            ControlSourceLimits {
                source: SOURCE,
                max_bindings: 8,
            },
            host,
        )
        .unwrap();
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
            assert!(
                lower_control_source(
                    &expression(text),
                    &[],
                    ControlSourceLimits {
                        source: SOURCE,
                        max_bindings: 8
                    },
                    host
                )
                .is_err()
            );
        }
        host.source.granted = &[31];
        assert!(
            lower_control_source(
                &expression(PROGRAM),
                &[],
                ControlSourceLimits {
                    source: SOURCE,
                    max_bindings: 8
                },
                host
            )
            .is_err()
        );
        host.source.granted = &[31, 32];
        host.source.version = 8;
        let error = lower_control_source(
            &expression(PROGRAM),
            &[],
            ControlSourceLimits {
                source: SOURCE,
                max_bindings: 8,
            },
            host,
        )
        .err()
        .unwrap();
        assert!(!format!("{error:?}").contains("catalog"));
        let ControlSourceError::Native {
            error: PrivateError(reason),
            ..
        } = error
        else {
            panic!()
        };
        assert_eq!(reason, "native source catalog rejected");
        assert!(host.calls.borrow().is_empty());
        assert!(host.queries.borrow().is_empty());
    });
}

#[test]
fn post_acceptance_live_policy_and_native_view_identity_fail_before_write_without_retry() {
    with_host(|host, foreign| {
        let original = host.layout;
        let (program, _) = lower_control_source(
            &expression(PROGRAM),
            &[],
            ControlSourceLimits {
                source: SOURCE,
                max_bindings: 8,
            },
            host,
        )
        .unwrap();
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
