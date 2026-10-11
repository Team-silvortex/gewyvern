use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    panic::{AssertUnwindSafe, catch_unwind},
    rc::Rc,
};

use leselang_hir::call_evaluation::{
    CallEvaluationLimits, PreparedCall, evaluate_call_arguments_in_scope,
};
use leselang_hir::catalog_program::{CatalogProgramLimits, compile_catalog_program};
use leselang_hir::catalog_source::*;
use leselang_hir::control_source::ControlSourceLimits;
use leselang_hir::effect_evaluation::{EffectEvaluationEnvironment, EffectEvaluationLimits};
use leselang_hir::effect_reentry::{EffectReentryEnvironment, RestoredEffectBindings};
use leselang_hir::effect_session::*;
use leselang_hir::flow_typing::{CallFlowEnvironment, CallFlowTypeError};
use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationLimits, PureValue, evaluate_pure_in_scope,
    scalar_copy_cost,
};
use leselang_hir::pure_typing::{PureType, PureTypeEnvironment};
use leselang_hir::source_call::{SourceCallError, SourceCallHost, SourceCallLimits};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, parse};

struct Field;
struct Opaque;
struct Tag;
struct Private(&'static str);
struct Key {
    name: &'static str,
    owner: Rc<()>,
}
impl Borrow<str> for Key {
    fn borrow(&self) -> &str {
        self.name
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && Rc::ptr_eq(&self.owner, &other.owner)
    }
}
struct Declaration {
    ty: ScalarType,
    owner: Rc<()>,
}
impl HostResultDomain<ScalarValue> for Declaration {
    type Error = Private;
    fn matches_type(&self, value: &ScalarValue) -> bool {
        value.scalar_type() == self.ty
    }
    fn validate_value(&self, value: &ScalarValue) -> Result<(), Private> {
        if !self.matches_type(value) || matches!(value, ScalarValue::Integer(n) if *n > 100) {
            Err(Private("private result domain"))
        } else {
            Ok(())
        }
    }
}
type Node = Computation<Field, Key, Opaque, Tag>;
type SourceHost<'host, 'schema> =
    SourceCallHost<'host, 'schema, Key, ScalarTypeSet, Declaration, u8>;
const LIMITS: CatalogSourceLimits = CatalogSourceLimits {
    control: ControlSourceLimits {
        source: SourceCallLimits {
            max_source_nodes: 256,
            max_source_depth: 32,
            max_lowered_nodes: 256,
            max_lowered_depth: 32,
            max_arguments: 64,
        },
        max_bindings: 8,
    },
    max_calls: 16,
};
struct Environment {
    owner: Rc<()>,
    hooks: RefCell<Vec<&'static str>>,
    mode: Cell<u8>,
}
impl PureTypeEnvironment<Field, Key> for Environment {
    type Result = ();
    fn field_type(&self, _: &(), _: &Field) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &Key) -> Option<()> {
        None
    }
}
impl<'schema> CallFlowEnvironment<'schema, Field, Key, Declaration> for Environment {
    fn call_result_type(&self, _: &Key, declaration: &'schema Declaration) -> Option<PureType<()>> {
        self.hooks.borrow_mut().push("result");
        Rc::ptr_eq(&self.owner, &declaration.owner).then_some(PureType::Scalar(declaration.ty))
    }
}
impl<'source, 'schema> CatalogSourceEnvironment<'source, 'schema, Field, Key, Declaration>
    for Environment
{
    type Error = Private;
    fn source_operation(&self, original: &Key) -> Result<Key, Private> {
        self.hooks.borrow_mut().push("map");
        match self.mode.get() {
            2 => return Err(Private("private mapping failure")),
            3 => panic!("native mapping unwind"),
            _ => {}
        }
        Ok(Key {
            name: if self.mode.get() == 1 {
                "counter.other"
            } else {
                original.name
            },
            owner: original.owner.clone(),
        })
    }
    fn source_field(
        &self,
        _: &(),
        _: &'source str,
    ) -> Result<Option<(Field, ScalarType)>, Private> {
        self.hooks.borrow_mut().push("field");
        Ok(None)
    }
    fn source_member(
        &self,
        _: &(),
        _: &'source str,
        _: &'source str,
    ) -> Result<Option<(Key, ())>, Private> {
        self.hooks.borrow_mut().push("member");
        Ok(None)
    }
}
fn expression(text: &str) -> Expression {
    let tree = parse(&format!("fn main() = {text}"));
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    tree.function.unwrap().body
}
fn with_host(
    name: &'static str,
    ty: ScalarType,
    test: impl for<'host, 'schema> FnOnce(&SourceHost<'host, 'schema>, &Environment),
) {
    let owner = Rc::new(());
    let parameters = [NamedParameter::required("n", ScalarTypeSet::only(ty))];
    let schemas = [
        OperationSchema {
            key: Key {
                name,
                owner: owner.clone(),
            },
            parameters: &parameters,
            result: Declaration {
                ty,
                owner: owner.clone(),
            },
            required_capability: 7u8,
        },
        OperationSchema {
            key: Key {
                name: "counter.other",
                owner: owner.clone(),
            },
            parameters: &parameters,
            result: Declaration {
                ty,
                owner: owner.clone(),
            },
            required_capability: 8u8,
        },
    ];
    let catalog = OperationCatalog::new(
        9,
        &schemas,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let host = SourceCallHost {
        catalog: &catalog,
        version: 9,
        granted: &[7, 8],
    };
    let environment = Environment {
        owner,
        hooks: RefCell::new(Vec::new()),
        mode: Cell::new(0),
    };
    test(&host, &environment);
}
fn compile(
    source: &Expression,
    host: &SourceHost<'_, '_>,
    environment: &Environment,
    limits: CatalogSourceLimits,
) -> CatalogSourceResult<Node, (), Private> {
    compile_catalog_source(source, &[], limits, host, environment)
}

#[test]
fn one_shared_compiler_uses_unrelated_catalogs_with_nonclone_native_slots() {
    for (name, ty, operand) in [
        (
            "counter.add",
            ScalarType::Integer,
            "add(left: 20, right: 21)",
        ),
        (
            "panel.echo",
            ScalarType::String,
            "concat(right: \"y\", left: \"read\")",
        ),
    ] {
        with_host(name, ty, |host, env| {
            let input = expression(&format!(
                "choose(otherwise: {name}(n: {operand}), when: true, then: {name}(n: {operand}))"
            ));
            let before = Rc::strong_count(&env.owner);
            let (node, actual) = compile(&input, host, env, LIMITS).unwrap();
            assert_eq!(actual, PureType::Scalar(ty));
            assert_eq!(
                env.hooks
                    .borrow()
                    .iter()
                    .filter(|hook| **hook == "map")
                    .count(),
                2
            );
            assert_eq!(Rc::strong_count(&env.owner), before + 2);
            drop(node);
            assert_eq!(Rc::strong_count(&env.owner), before);
        });
    }
}

#[test]
fn complete_cold_catalog_versions_grants_and_names_precede_every_source_hook() {
    with_host("counter.add", ScalarType::Integer, |host, env| {
        for (source, version, grants, kind) in [
            (
                "choose(when: true, then: counter.add(n: 1), otherwise: missing(n: 2))",
                9,
                &[7, 8][..],
                0,
            ),
            (
                "choose(when: true, then: counter.add(n: 1), otherwise: counter.other(n: 2))",
                9,
                &[7][..],
                1,
            ),
            ("counter.add(n: 1)", 10, &[7, 8][..], 2),
            (
                "choose(when: true, then: counter.add(n: 1), otherwise: counter.other(wrong: 2))",
                9,
                &[7, 8][..],
                3,
            ),
        ] {
            let policy = SourceCallHost {
                catalog: host.catalog,
                version,
                granted: grants,
            };
            let error = compile(&expression(source), &policy, env, LIMITS)
                .err()
                .unwrap();
            let CatalogSourceError::Call { error, .. } = error else {
                panic!()
            };
            assert!(matches!(
                (*error, kind),
                (
                    SourceCallError::Catalog(OperationCatalogError::UnknownOperation),
                    0
                ) | (
                    SourceCallError::Catalog(OperationCatalogError::CapabilityDenied),
                    1
                ) | (
                    SourceCallError::Catalog(OperationCatalogError::UnsupportedVersion),
                    2
                ) | (SourceCallError::Names(_), 3)
            ));
            assert!(env.hooks.borrow().is_empty());
        }
    });
}

#[test]
fn whole_physical_lexical_and_intrinsic_signatures_stop_native_mapping() {
    with_host("counter.add", ScalarType::Integer, |host, env| {
        for text in [
            "choose(when: true, then: counter.add(n: 1), wrong: 2)",
            "bind(x: 1, body: bind(x: 2, body: counter.add(n: x)))",
            "bind(x: 1, body: add(left: counter.add(n: x), left: 2))",
            "seq(step: counter.add(n: 1))",
        ] {
            assert!(compile(&expression(text), host, env, LIMITS).is_err());
            assert!(env.hooks.borrow().is_empty());
        }
        let input = expression("counter.add(n: 1)");
        let mut small = LIMITS;
        small.control.source.max_source_nodes = 1;
        assert!(compile(&input, host, env, small).is_err());
        assert!(env.hooks.borrow().is_empty());
        small = LIMITS;
        small.max_calls = 0;
        assert!(matches!(
            compile(&input, host, env, small),
            Err(CatalogSourceError::Type(CallFlowTypeError::CallLimit))
        ));
        assert!(env.hooks.borrow().is_empty());
        small.max_calls = usize::MAX;
        assert!(matches!(
            compile(&input, host, env, small),
            Err(CatalogSourceError::Type(CallFlowTypeError::InvalidLimits))
        ));
        assert!(env.hooks.borrow().is_empty());
    });
}

#[test]
fn scalar_bind_loop_fold_and_pure_tail_have_one_catalog_compilation_entry() {
    with_host("counter.add", ScalarType::Integer, |host, env| {
        for text in [
            "bind(x: 40, body: counter.add(n: add(left: x, right: 1)))",
            "bind(receipt: counter.add(n: 41), body: add(left: receipt, right: 1))",
            "counter.add(n: loop(x: 0, while: lt(left: x, right: 2), next: add(left: x, right: 1), limit: 3))",
            "counter.add(n: fold(total: 0, items: strings(a: \"a\", b: \"b\"), item: \"part\", next: add(left: total, right: 1), limit: 2))",
        ] {
            env.hooks.borrow_mut().clear();
            let (node, ty) = compile(&expression(text), host, env, LIMITS).unwrap();
            assert_eq!(ty, PureType::Scalar(ScalarType::Integer));
            assert!(!node.is_pure());
            assert_eq!(
                env.hooks
                    .borrow()
                    .iter()
                    .filter(|hook| **hook == "map")
                    .count(),
                1
            );
        }
        let mut pure = LIMITS;
        pure.max_calls = 0;
        let (node, ty) = compile(&expression("add(left: 20, right: 22)"), host, env, pure).unwrap();
        assert!(node.is_pure());
        assert_eq!(ty, PureType::Scalar(ScalarType::Integer));
    });
}

#[test]
fn mapped_operation_identity_failure_unwind_and_type_rejection_never_return_partial_ir() {
    with_host("counter.add", ScalarType::Integer, |host, env| {
        let input = expression("counter.add(n: 1)");
        let before = Rc::strong_count(&env.owner);
        for mode in [1, 2] {
            env.mode.set(mode);
            env.hooks.borrow_mut().clear();
            let error = compile(&input, host, env, LIMITS).err().unwrap();
            assert!(!format!("{error:?}").contains("private"));
            assert!(std::error::Error::source(&error).is_none());
            assert_eq!(env.hooks.borrow().as_slice(), ["map"]);
            assert_eq!(Rc::strong_count(&env.owner), before);
            if mode == 2 {
                let CatalogSourceError::Control(error) = error else {
                    panic!()
                };
                let leselang_hir::control_source::ControlSourceError::Native { error, .. } = *error
                else {
                    panic!()
                };
                let CatalogSourceError::Mapping {
                    error: Private(reason),
                    ..
                } = error
                else {
                    panic!()
                };
                assert_eq!(reason, "private mapping failure");
            }
        }
        env.mode.set(3);
        assert!(catch_unwind(AssertUnwindSafe(|| compile(&input, host, env, LIMITS))).is_err());
        assert_eq!(Rc::strong_count(&env.owner), before);
        env.mode.set(0);
        for text in [
            "counter.add(n: true)",
            "missing_local",
            "counter.add(n: counter.other(n: 1))",
            "bind(receipt: counter.add(n: 1), body: counter.add(n: receipt))",
        ] {
            // A scalar reply reference is valid; only the first three must fail.
            let result = compile(&expression(text), host, env, LIMITS);
            if text.starts_with("bind(") {
                assert!(result.is_ok());
            } else {
                assert!(result.is_err());
            }
            drop(result);
            assert_eq!(Rc::strong_count(&env.owner), before);
        }
    });
}

const EXECUTION: EffectEvaluationLimits = EffectEvaluationLimits {
    pure: PureEvaluationLimits {
        max_nodes: 256,
        max_depth: 32,
        max_bindings: 8,
    },
    max_arguments: 64,
    max_branches: 64,
};
struct Reply {
    value: ScalarValue,
    marker: Box<u8>,
}
impl Borrow<ScalarValue> for Reply {
    fn borrow(&self) -> &ScalarValue {
        &self.value
    }
}
type Request<'schema> = PreparedCall<'schema, Key, ScalarTypeSet, Declaration, u8>;
struct Runtime<'host, 'schema> {
    source: SourceHost<'host, 'schema>,
    generation: Cell<u64>,
}
impl PureEvaluationEnvironment<Field, Key> for Runtime<'_, '_> {
    type Result = ();
    type Error = Private;
    fn field(&self, _: &(), _: &Field) -> Result<ScalarValue, Private> {
        Err(Private("no scalar-host fields"))
    }
    fn member(&self, _: &(), _: &str, _: &Key) -> Result<(), Private> {
        Err(Private("no scalar-host members"))
    }
}
impl<'ir, 'schema> EffectEvaluationEnvironment<'ir, Field, Key, Opaque, Tag>
    for Runtime<'_, 'schema>
{
    type Request = Request<'schema>;
    type Capture = RestoredEffectBindings<'ir, ()>;
    fn preflight_effect(&self, node: &'ir Node) -> Result<(), Private> {
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err(Private("only catalog calls"));
        };
        let schema = self
            .source
            .catalog
            .authorize(operation, self.source.version, self.source.granted)
            .map_err(|_| Private("cold policy"))?;
        schema
            .bind_arguments(
                &arguments
                    .iter()
                    .map(|arg| arg.name.as_str())
                    .collect::<Vec<_>>(),
            )
            .map_err(|_| Private("cold signature"))?;
        Ok(())
    }
    fn prepare_effect(
        &self,
        node: &'ir Node,
        scope: &mut ScopeFrame<'_, 'ir, PureValue<()>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Request, CalculationFailure<Private>> {
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err(Private("only catalog calls").into());
        };
        let schema = self
            .source
            .catalog
            .authorize(operation, self.source.version, self.source.granted)
            .map_err(|_| Private("live preparation policy"))?;
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
        .map_err(|_| Private("actual parameter domain").into())
    }
    fn capture(
        &self,
        _: &'ir str,
        _: &'ir Node,
        scope: &ScopeFrame<'_, 'ir, PureValue<()>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Capture, Private> {
        let mut cost = scope.len() as u64;
        for (_, value) in scope.bindings() {
            if let PureValue::Scalar(value) = value {
                cost = cost
                    .checked_add(scalar_copy_cost(value))
                    .ok_or(Private("capture cost"))?;
            }
        }
        fuel.charge(cost).map_err(|_| Private("capture fuel"))?;
        Ok(scope.bindings().to_vec())
    }
}
impl ReplyAuthority<(u64, &'static str)> for Runtime<'_, '_> {
    type Error = Private;
    fn authorize(&self, identity: &(u64, &'static str)) -> Result<(), Private> {
        if identity.0 != self.generation.get() {
            return Err(Private("stale generation"));
        }
        self.source
            .catalog
            .authorize(identity.1, self.source.version, self.source.granted)
            .map(|_| ())
            .map_err(|_| Private("reply policy"))
    }
}
impl<'ir, 'schema>
    EffectReentryEnvironment<'ir, Field, Key, Opaque, Tag, (u64, &'static str), Declaration, Reply>
    for Runtime<'_, 'schema>
{
    fn restore_capture(
        &self,
        identity: &(u64, &'static str),
        _: &Declaration,
        capture: Self::Capture,
        _: &mut Fuel,
        _: EffectEvaluationLimits,
    ) -> Result<Self::Capture, CalculationFailure<Private>> {
        self.authorize(identity)?;
        Ok(capture)
    }
    fn bind_reply(
        &self,
        identity: &(u64, &'static str),
        declaration: &Declaration,
        reply: Reply,
        _: &mut Fuel,
    ) -> Result<PureValue<()>, CalculationFailure<Private>> {
        self.authorize(identity)?;
        let schema = self
            .source
            .catalog
            .authorize(identity.1, self.source.version, self.source.granted)
            .map_err(|_| Private("reentry policy"))?;
        if !std::ptr::eq(declaration, &schema.result) {
            return Err(Private("foreign result declaration").into());
        }
        declaration.validate_value(&reply.value)?;
        Ok(PureValue::Scalar(reply.value))
    }
}
impl<'ir, 'schema> EffectSessionEnvironment<'ir, 'schema, Field, Key, Opaque, Tag>
    for Runtime<'_, 'schema>
{
    type Identity = (u64, &'static str);
    type Declaration = Declaration;
    type Reply = Reply;
    type Dispatch = Request<'schema>;
    fn correlate_request(
        &self,
        request: Self::Request,
        _: &mut Fuel,
    ) -> EffectCorrelation<'schema, Self::Identity, Declaration, Self::Dispatch, Private> {
        let schema = self
            .source
            .catalog
            .authorize(
                &request.schema().key,
                self.source.version,
                self.source.granted,
            )
            .map_err(|_| Private("dispatch policy"))?;
        if !std::ptr::eq(schema, request.schema()) {
            return Err(Private("foreign original schema").into());
        }
        let generation = self
            .generation
            .get()
            .checked_add(1)
            .ok_or(Private("generation exhausted"))?;
        self.generation.set(generation);
        Ok(CorrelatedEffectRequest {
            identity: (generation, schema.key.name),
            declaration: &schema.result,
            dispatch: request,
        })
    }
}

#[test]
fn compiled_scalar_catalog_program_suspends_accepts_typed_reply_reenters_and_cancels_without_product_vm()
 {
    with_host("counter.add", ScalarType::Integer, |host, env| {
        let source = expression(
            "bind(seed: 20, body: bind(receipt: counter.add(n: add(left: seed, right: 21)), body: add(left: receipt, right: 1)))",
        );
        let (node, _) = compile(&source, host, env, LIMITS).unwrap();
        let runtime = Runtime {
            source: SourceCallHost {
                catalog: host.catalog,
                version: host.version,
                granted: host.granted,
            },
            generation: Cell::new(0),
        };
        let mut session = EffectSession::new(&node, Vec::new(), Fuel::new(100), EXECUTION);
        let EffectSessionPoll::Request(request) = session.poll(&runtime).unwrap() else {
            panic!()
        };
        assert!(matches!(
            request.arguments()[0].value,
            ScalarValue::Integer(41)
        ));
        let identity = (runtime.generation.get(), request.schema().key.name);
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Awaiting
        ));
        let reply = Reply {
            value: ScalarValue::Boolean(true),
            marker: Box::new(1),
        };
        let pointer = reply.marker.as_ref() as *const u8;
        let rejected = session
            .try_accept(&identity, reply, &runtime)
            .err()
            .unwrap();
        assert_eq!(rejected.reply.marker.as_ref() as *const u8, pointer);
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Awaiting
        ));
        let stale = session
            .try_accept(
                &(0, identity.1),
                Reply {
                    value: ScalarValue::Integer(41),
                    marker: Box::new(2),
                },
                &runtime,
            )
            .err()
            .unwrap();
        session
            .try_accept(&identity, stale.reply, &runtime)
            .unwrap();
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(42)))
        ));
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
        ));
        assert_eq!(runtime.generation.get(), 1);
        let mut cancelled = EffectSession::new(&node, Vec::new(), Fuel::new(100), EXECUTION);
        assert!(matches!(
            cancelled.poll(&runtime).unwrap(),
            EffectSessionPoll::Request(_)
        ));
        assert!(cancelled.cancel());
        assert!(matches!(
            cancelled.poll(&runtime).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
        ));
    });
}

const PROGRAM: CatalogProgramLimits = CatalogProgramLimits {
    expression: LIMITS,
    max_functions: 32,
    max_parameters: 8,
    max_compilation_nodes: 4096,
    max_expansion_depth: 32,
};
fn compile_functions(
    text: &str,
    entry: &str,
    host: &SourceHost<'_, '_>,
    env: &Environment,
    limits: CatalogProgramLimits,
) -> CatalogSourceResult<Node, (), Private> {
    let tree = parse(text);
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    let declarations: Vec<_> = tree.function.iter().chain(&tree.helpers).collect();
    compile_catalog_program(&declarations, entry, limits, host, env)
}
fn pure_integer(node: &Node, host: &SourceHost<'_, '_>) -> u64 {
    let runtime = Runtime {
        source: SourceCallHost {
            catalog: host.catalog,
            version: host.version,
            granted: host.granted,
        },
        generation: Cell::new(0),
    };
    let value = evaluate_pure_in_scope(
        node,
        &mut ScopeFrame::new(&mut Vec::new()),
        &runtime,
        &mut Fuel::new(10000),
        EXECUTION.pure,
    )
    .unwrap();
    let PureValue::Scalar(ScalarValue::Integer(value)) = value else {
        panic!()
    };
    value
}
fn closed_error(error: &CatalogSourceError<Private>) -> String {
    use leselang_hir::{
        catalog_program::CatalogProgramError, control_source::ControlSourceError,
        helper_source::HelperSourceError, scalar_source::ScalarSourceError,
    };
    match error {
        CatalogSourceError::Control(error) => match error.as_ref() {
            ControlSourceError::Native { error, .. } => closed_error(error),
            _ => format!("control: {error}"),
        },
        CatalogSourceError::Scalar { error, .. } => match error.as_ref() {
            ScalarSourceError::Native { error, .. } => closed_error(error),
            ScalarSourceError::Produced { error, .. } => format!("produced: {error:?}"),
            _ => format!("scalar: {error}"),
        },
        CatalogSourceError::Type(error) => format!("type: {error:?}"),
        CatalogSourceError::Program(error) => match error.as_ref() {
            CatalogProgramError::Arguments { error, .. } => match error.as_ref() {
                HelperSourceError::Lowering { error, .. } => closed_error(error),
                _ => format!("arguments: {error}"),
            },
            CatalogProgramError::Hygiene { error, .. } => format!("hygiene: {error}"),
            CatalogProgramError::Bindings { error, .. } => format!("bindings: {error:?}"),
            _ => format!("program: {error}"),
        },
        _ => error.to_string(),
    }
}

#[test]
fn shared_multifunction_compiler_owns_forward_calls_scalar_parameters_and_explicit_entry() {
    with_host("counter.add", ScalarType::Integer, |host, env| {
        let text = "fn start() = outer(right: 21, left: 20)\n\
            fn outer(left: integer, right: integer) = plus(a: left, b: right)\n\
            fn plus(a: integer, b: integer) = add(left: add(left: a, right: b), right: 1)";
        let (node, ty) = compile_functions(text, "start", host, env, PROGRAM).unwrap();
        assert_eq!(ty, PureType::Scalar(ScalarType::Integer));
        assert!(node.is_pure());
        assert_eq!(pure_integer(&node, host), 42);
        assert!(env.hooks.borrow().is_empty());
        let tree = parse(text);
        let declarations: Vec<_> = tree.function.iter().chain(&tree.helpers).collect();
        let before = format!("{declarations:?}");
        let _: (Node, _) =
            compile_catalog_program(&declarations, "start", PROGRAM, host, env).unwrap();
        assert_eq!(format!("{declarations:?}"), before);
    });
}

#[test]
fn whole_unused_helper_policy_names_and_cycles_precede_every_native_compilation_hook() {
    with_host("counter.add", ScalarType::Integer, |host, env| {
        for text in [
            "fn main() = counter.add(n: 1)\nfn unused() = unknown(n: 0)",
            "fn main() = counter.add(n: 1)\nfn unused() = counter.other(n: 0)",
            "fn main() = counter.add(n: 1)\nfn unused() = counter.other(wrong: 0)",
            "fn main() = counter.add(n: 1)\nfn unused() = helper(wrong: 0)\nfn helper(n: integer) = n",
            "fn main() = counter.add(n: 1)\nfn unused() = choose(when: false, then: helper(n: 0, n: 1), otherwise: 0)\nfn helper(n: integer) = n",
            "fn main() = counter.add(n: 1)\nfn a() = counter.add(n: 1)\nfn y() = z()\nfn z() = y()",
            "fn main() = counter.add(n: 1)\nfn unused() = main()",
            "fn main() = main()",
            "fn main() = counter.add(n: 1)\nfn unused() = add(left: 1, wrong: 2)",
            "fn main() = counter.add(n: 1)\nfn add(n: integer) = n",
        ] {
            env.hooks.borrow_mut().clear();
            let denied = SourceCallHost {
                catalog: host.catalog,
                version: host.version,
                granted: &[7],
            };
            assert!(
                compile_functions(text, "main", &denied, env, PROGRAM).is_err(),
                "{text}"
            );
            assert!(env.hooks.borrow().is_empty(), "{text}");
        }
        let wrong_version = SourceCallHost {
            catalog: host.catalog,
            version: host.version + 1,
            granted: host.granted,
        };
        assert!(
            compile_functions(
                "fn main() = 0\nfn unused() = counter.add(n: 1)",
                "main",
                &wrong_version,
                env,
                PROGRAM
            )
            .is_err()
        );
        assert!(env.hooks.borrow().is_empty());
    });
}

#[test]
fn helper_hygiene_preserves_caller_aliases_and_nested_scalar_loop_fold_scopes() {
    with_host("counter.add", ScalarType::Integer, |host, env| {
        for (text, expected) in [
            (
                "fn inc(n: integer) = bind(local: add(left: n, right: 1), body: local)\nfn main() = bind(n: 40, body: bind(_lp0: 1, body: add(left: inc(n: n), right: _lp0)))",
                42,
            ),
            (
                "fn inc(n: integer) = add(left: n, right: 1)\nfn main() = loop(i: 0, while: lt(left: i, right: 3), next: inc(n: i), limit: 3)",
                3,
            ),
            (
                "fn inc(n: integer) = add(left: n, right: 1)\nfn main() = fold(items: strings(a: \"a\", b: \"b\", c: \"c\"), state: 0, item: \"word\", next: inc(n: state), limit: 3)",
                3,
            ),
            (
                "fn inc(n: integer) = add(left: n, right: 1)\nfn main() = add(left: bind(_lp1: 40, body: inc(n: _lp1)), right: 1)",
                42,
            ),
        ] {
            let (node, _) = compile_functions(text, "main", host, env, PROGRAM)
                .unwrap_or_else(|error| panic!("{text}: {}", closed_error(&error)));
            assert_eq!(pure_integer(&node, host), expected, "{text}");
        }
        for text in [
            "fn leak() = n\nfn main() = bind(n: 42, body: leak())",
            "fn leak(n: integer) = bind(n: 2, body: n)\nfn main() = leak(n: 1)",
            "fn leak() = missing\nfn main() = 0",
            "fn id(n: integer) = n\nfn main() = id(n: true)",
            "fn bad() = counter.add(n: 1)\nfn main() = counter.add(n: bad())",
            "fn bad() = counter.add(n: 1)\nfn main() = recover(value: bad(), fallback: 1)",
        ] {
            assert!(
                compile_functions(text, "main", host, env, PROGRAM).is_err(),
                "{text}"
            );
        }
    });
}

#[test]
fn effectful_helper_compilation_reenters_shared_session_and_cancels_without_native_ir_copy() {
    with_host("counter.add", ScalarType::Integer, |host, env| {
        let text = "fn main() = bind(n: 20, body: route(n: add(left: n, right: 21)))\n\
            fn route(n: integer) = bump(n: n)\n\
            fn bump(n: integer) = bind(receipt: counter.add(n: n), body: add(left: receipt, right: 1))";
        let before = Rc::strong_count(&env.owner);
        let (node, ty) = compile_functions(text, "main", host, env, PROGRAM)
            .unwrap_or_else(|error| panic!("{}", closed_error(&error)));
        assert_eq!(ty, PureType::Scalar(ScalarType::Integer));
        assert_eq!(Rc::strong_count(&env.owner), before + 1);
        let runtime = Runtime {
            source: SourceCallHost {
                catalog: host.catalog,
                version: host.version,
                granted: host.granted,
            },
            generation: Cell::new(0),
        };
        let mut session = EffectSession::new(&node, Vec::new(), Fuel::new(1000), EXECUTION);
        let EffectSessionPoll::Request(request) = session.poll(&runtime).unwrap() else {
            panic!()
        };
        assert_eq!(request.arguments()[0].value, ScalarValue::Integer(41));
        let identity = (runtime.generation.get(), request.schema().key.name);
        let reply = Reply {
            value: ScalarValue::Boolean(true),
            marker: Box::new(17),
        };
        let marker = reply.marker.as_ref() as *const u8;
        let rejected = session
            .try_accept(&identity, reply, &runtime)
            .err()
            .unwrap();
        assert_eq!(rejected.reply.marker.as_ref() as *const u8, marker);
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Awaiting
        ));
        session
            .try_accept(
                &identity,
                Reply {
                    value: ScalarValue::Integer(41),
                    marker: Box::new(18),
                },
                &runtime,
            )
            .unwrap();
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Value(PureValue::Scalar(ScalarValue::Integer(42)))
        ));
        assert!(matches!(
            session.poll(&runtime).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Completed)
        ));
        assert_eq!(runtime.generation.get(), 1);
        let mut cancelled = EffectSession::new(&node, Vec::new(), Fuel::new(1000), EXECUTION);
        assert!(matches!(
            cancelled.poll(&runtime).unwrap(),
            EffectSessionPoll::Request(_)
        ));
        assert!(cancelled.cancel());
        assert!(matches!(
            cancelled.poll(&runtime).unwrap(),
            EffectSessionPoll::Terminal(EffectSessionEnd::Cancelled)
        ));
        drop(cancelled);
        drop(session);
        drop(node);
        assert_eq!(Rc::strong_count(&env.owner), before);
    });
}

#[test]
fn helper_recompilation_is_bounded_before_exponential_source_work_or_partial_output() {
    with_host("counter.add", ScalarType::Integer, |host, env| {
        let text = "fn main() = f4()\nfn f0() = 1\n\
            fn f1() = add(left: f0(), right: f0())\n\
            fn f2() = add(left: f1(), right: f1())\n\
            fn f3() = add(left: f2(), right: f2())\n\
            fn f4() = add(left: f3(), right: f3())";
        let mut bounded = PROGRAM;
        bounded.max_compilation_nodes = 32;
        assert!(compile_functions(text, "main", host, env, bounded).is_err());
        assert!(env.hooks.borrow().is_empty());
        let (node, _) = compile_functions(text, "main", host, env, PROGRAM).unwrap();
        assert_eq!(pure_integer(&node, host), 16);
        bounded = PROGRAM;
        bounded.max_expansion_depth = 0;
        assert!(
            compile_functions(
                "fn f(n: integer) = n\nfn main() = f(n: 1)",
                "main",
                host,
                env,
                bounded
            )
            .is_err()
        );
        bounded = PROGRAM;
        bounded.max_compilation_nodes = 0;
        assert!(
            compile_functions("fn main() = counter.add(n: 1)", "main", host, env, bounded).is_err()
        );
        assert!(env.hooks.borrow().is_empty());
        bounded = PROGRAM;
        bounded.expression.control.source.max_lowered_nodes = 3;
        assert!(
            compile_functions(
                "fn f(n: integer) = add(left: n, right: 1)\nfn main() = f(n: 1)",
                "main",
                host,
                env,
                bounded
            )
            .is_err()
        );
        bounded = PROGRAM;
        bounded.expression.max_calls = 1;
        assert!(
            compile_functions(
                "fn f() = counter.add(n: 1)\nfn main() = bind(first: f(), body: f())",
                "main",
                host,
                env,
                bounded
            )
            .is_err()
        );
    });
}

#[test]
fn multifunction_catalog_hosts_remain_unrelated_and_failure_unwind_releases_native_slots() {
    for (name, ty, text) in [
        (
            "counter.add",
            ScalarType::Integer,
            "fn invoke(n: integer) = counter.add(n: n)\nfn main() = invoke(n: 42)",
        ),
        (
            "panel.echo",
            ScalarType::String,
            "fn invoke(n: string) = panel.echo(n: n)\nfn main() = invoke(n: \"ready\")",
        ),
    ] {
        with_host(name, ty, |host, env| {
            let before = Rc::strong_count(&env.owner);
            let (node, inferred) = compile_functions(text, "main", host, env, PROGRAM).unwrap();
            assert_eq!(inferred, PureType::Scalar(ty));
            assert_eq!(Rc::strong_count(&env.owner), before + 1);
            drop(node);
            assert_eq!(Rc::strong_count(&env.owner), before);
            for mode in [1, 2] {
                env.mode.set(mode);
                let error = compile_functions(text, "main", host, env, PROGRAM)
                    .err()
                    .unwrap();
                assert!(!format!("{error:?}").contains("private"));
                assert!(std::error::Error::source(&error).is_none());
                assert_eq!(Rc::strong_count(&env.owner), before);
            }
            env.mode.set(3);
            assert!(
                catch_unwind(AssertUnwindSafe(|| compile_functions(
                    text, "main", host, env, PROGRAM
                )))
                .is_err()
            );
            assert_eq!(Rc::strong_count(&env.owner), before);
            env.mode.set(0);
            assert!(compile_functions(text, "main", host, env, PROGRAM).is_ok());
        });
    }
}
