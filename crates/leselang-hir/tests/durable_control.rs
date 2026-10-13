//! Consumer-owned codec/journal model using the actual native compiler and walker.
//! This is not a core storage backend or an exactly-once external-effects promise.
use std::{
    borrow::Borrow,
    cell::{Cell, RefCell},
    collections::BTreeMap,
    fmt::Debug,
};

use leselang_hir::{
    catalog_program::{CatalogProgramLimits, compile_catalog_program},
    catalog_source::{CatalogSourceEnvironment, CatalogSourceLimits, compile_catalog_source},
    control_source::ControlSourceLimits,
    effect_evaluation::{
        EffectEvaluationEnvironment, EffectEvaluationLimits, evaluate_resumable_effects_in_scope,
    },
    effect_reentry::{
        CapturedEffectHooks, ResumableEffectOutcome, resume_captured_effects_in_scope,
    },
    flow_typing::CallFlowEnvironment,
    ir::Computation,
    pure_evaluation::{
        PureEvaluationEnvironment, PureEvaluationLimits, PureValue, evaluate_pure_in_scope,
        scalar_copy_cost,
    },
    pure_typing::{PureType, PureTypeEnvironment},
    source_call::{SourceCallHost, SourceCallLimits},
};
use leselang_runtime_core::*;
use serde::{Deserialize, Serialize, de::DeserializeOwned};

const EXEC: EffectEvaluationLimits = EffectEvaluationLimits {
    pure: PureEvaluationLimits {
        max_nodes: 128,
        max_depth: 16,
        max_bindings: 8,
    },
    max_arguments: 1,
    max_branches: 2,
};
const BUDGET: ResourceBudgetLimits = ResourceBudgetLimits {
    max_fuel: 1000,
    max_deadline_ms: 1000,
    max_deadline_at_ms: 10_000,
    max_output_items: 16,
};
const SOURCE: CatalogSourceLimits = CatalogSourceLimits {
    control: ControlSourceLimits {
        source: SourceCallLimits {
            max_source_nodes: 128,
            max_source_depth: 16,
            max_lowered_nodes: 128,
            max_lowered_depth: 16,
            max_arguments: 2,
        },
        max_bindings: 8,
    },
    max_calls: 8,
};

trait Key: Clone + Debug + Eq + Serialize + DeserializeOwned + Borrow<str> {
    fn read() -> Self;
    fn write() -> Self;
    fn scalar() -> ScalarType;
    fn initial() -> ScalarValue;
    fn source() -> &'static str;
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Counter {
    Read,
    Write,
}
impl Borrow<str> for Counter {
    fn borrow(&self) -> &str {
        match self {
            Self::Read => "counter.read",
            Self::Write => "counter.write",
        }
    }
}
impl Key for Counter {
    fn read() -> Self {
        Self::Read
    }
    fn write() -> Self {
        Self::Write
    }
    fn scalar() -> ScalarType {
        ScalarType::Integer
    }
    fn initial() -> ScalarValue {
        ScalarValue::Integer(41)
    }
    fn source() -> &'static str {
        "fn main() = bind(x: counter.read(), body: bind(y: counter.write(value: add(left: x, right: 1)), body: add(left: x, right: y)))"
    }
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
enum Panel {
    ReadText,
    SetText,
}
impl Borrow<str> for Panel {
    fn borrow(&self) -> &str {
        match self {
            Self::ReadText => "panel.read_text",
            Self::SetText => "panel.set_text",
        }
    }
}
impl Key for Panel {
    fn read() -> Self {
        Self::ReadText
    }
    fn write() -> Self {
        Self::SetText
    }
    fn scalar() -> ScalarType {
        ScalarType::String
    }
    fn initial() -> ScalarValue {
        ScalarValue::String("ready".into())
    }
    fn source() -> &'static str {
        "fn main() = bind(x: panel.read_text(), body: bind(y: panel.set_text(value: concat(left: x, right: \"!\")), body: concat(left: x, right: y)))"
    }
}
type Node<K> = Computation<(), K, (), ()>;
type Capture<K> = CapturedBindings<Node<K>, (), ()>;
type Image<K> = ContinuationFrame<u64, u64, ScalarType, Request<K>, Capture<K>>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
struct Request<K> {
    operation: K,
    value: Option<ScalarValue>,
}
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", deny_unknown_fields)]
enum Step<K> {
    Effect(Box<Image<K>>),
    Done(ScalarValue),
}
struct Domain(ScalarType);
impl HostResultDomain<ScalarValue> for Domain {
    type Error = &'static str;
    fn matches_type(&self, value: &ScalarValue) -> bool {
        value.scalar_type() == self.0
    }
    fn validate_value(&self, value: &ScalarValue) -> Result<(), Self::Error> {
        ScalarTypeSet::only(self.0)
            .validate(value)
            .map_err(|_| "domain")
    }
}
struct Host<K> {
    generation: Cell<u64>,
    grants: Cell<u8>,
    now: Cell<u64>,
    expected: Cell<u64>,
    value: RefCell<ScalarValue>,
    invocations: RefCell<Vec<K>>,
    preparations: Cell<usize>,
    restorations: Cell<usize>,
    projections: Cell<usize>,
}
impl<K: Key> Host<K> {
    fn new() -> Self {
        Self {
            generation: Cell::new(7),
            grants: Cell::new(3),
            now: Cell::new(1),
            expected: Cell::new(0),
            value: RefCell::new(K::initial()),
            invocations: RefCell::new(vec![]),
            preparations: Cell::new(0),
            restorations: Cell::new(0),
            projections: Cell::new(0),
        }
    }
    fn admit(&self, operation: &K) -> Result<(), &'static str> {
        let bit = if operation == &K::read() {
            1
        } else if operation == &K::write() {
            2
        } else {
            return Err("operation");
        };
        if self.generation.get() != 7 || self.grants.get() & bit == 0 {
            Err("authority")
        } else {
            Ok(())
        }
    }
    fn invoke(&self, request: &Request<K>) -> ScalarValue {
        self.admit(&request.operation).unwrap();
        self.invocations
            .borrow_mut()
            .push(request.operation.clone());
        if request.operation == K::write() {
            *self.value.borrow_mut() = request.value.clone().unwrap();
        }
        self.value.borrow().clone()
    }
}
impl<K: Key> ReplyAuthority<u64> for Host<K> {
    type Error = &'static str;
    fn authorize(&self, identity: &u64) -> Result<(), Self::Error> {
        if *identity != self.expected.get() || self.generation.get() != 7 {
            Err("identity")
        } else {
            Ok(())
        }
    }
}
impl<K: Key> PureEvaluationEnvironment<(), K> for Host<K> {
    type Result = ();
    type Error = &'static str;
    fn field(&self, _: &(), _: &()) -> Result<ScalarValue, Self::Error> {
        Err("field")
    }
    fn member(&self, _: &(), _: &str, _: &K) -> Result<(), Self::Error> {
        Err("member")
    }
}
impl<K: Key> PureTypeEnvironment<(), K> for Host<K> {
    type Result = ();
    fn field_type(&self, _: &(), _: &()) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &K) -> Option<()> {
        None
    }
}
impl<'s, K: Key> CallFlowEnvironment<'s, (), K, Domain> for Host<K> {
    fn call_result_type(&self, key: &K, domain: &'s Domain) -> Option<PureType<()>> {
        self.admit(key)
            .ok()
            .filter(|_| domain.0 == K::scalar())
            .map(|_| PureType::Scalar(domain.0))
    }
}
impl<'a, 's, K: Key> CatalogSourceEnvironment<'a, 's, (), K, Domain> for Host<K> {
    type Error = &'static str;
    fn source_operation(&self, original: &K) -> Result<K, Self::Error> {
        self.admit(original)?;
        Ok(original.clone())
    }
    fn source_field(&self, _: &(), _: &'a str) -> Result<Option<((), ScalarType)>, Self::Error> {
        Ok(None)
    }
    fn source_member(
        &self,
        _: &(),
        _: &'a str,
        _: &'a str,
    ) -> Result<Option<(K, ())>, Self::Error> {
        Ok(None)
    }
}
impl<'a, K: Key> EffectEvaluationEnvironment<'a, (), K, (), ()> for Host<K> {
    type Request = Request<K>;
    type Capture = Capture<K>;
    fn preflight_effect(&self, node: &'a Node<K>) -> Result<(), Self::Error> {
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err("call");
        };
        self.admit(operation)?;
        if (operation == &K::read() && !arguments.is_empty())
            || (operation == &K::write() && (arguments.len() != 1 || arguments[0].name != "value"))
        {
            return Err("arguments");
        }
        Ok(())
    }
    fn prepare_effect(
        &self,
        node: &'a Node<K>,
        scope: &mut ScopeFrame<'_, 'a, PureValue<()>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Request, CalculationFailure<Self::Error>> {
        self.preflight_effect(node)?;
        let Node::Call {
            operation,
            arguments,
        } = node
        else {
            return Err("call".into());
        };
        let value = if let Some(argument) = arguments.first() {
            let PureValue::Scalar(value) =
                evaluate_pure_in_scope(&argument.value, scope, self, fuel, EXEC.pure)
                    .map_err(|_| "operand")?
            else {
                return Err("operand type".into());
            };
            ScalarTypeSet::only(K::scalar())
                .validate(&value)
                .map_err(|_| "operand domain")?;
            Some(value)
        } else {
            None
        };
        self.preparations.set(self.preparations.get() + 1);
        Ok(Request {
            operation: operation.clone(),
            value,
        })
    }
    fn capture(
        &self,
        name: &'a str,
        body: &'a Node<K>,
        scope: &ScopeFrame<'_, 'a, PureValue<()>>,
        fuel: &mut Fuel,
    ) -> Result<Self::Capture, Self::Error> {
        let mut locals = vec![];
        for (name, value) in scope.bindings() {
            let PureValue::Scalar(value) = value else {
                return Err("capture type");
            };
            fuel.charge(1 + scalar_copy_cost(value))
                .map_err(|_| "capture fuel")?;
            locals.push(ScalarBinding {
                name: (*name).into(),
                value: value.clone(),
            });
        }
        Ok(CapturedBindings {
            name: name.into(),
            locals,
            results: vec![],
            groups: vec![],
            body: body.clone(),
        })
    }
}

fn compile<K: Key>(host: &Host<K>) -> Node<K> {
    compile_text(host, K::source())
}
fn compile_text<K: Key>(host: &Host<K>, source: &str) -> Node<K> {
    let params = [NamedParameter::required(
        "value",
        ScalarTypeSet::only(K::scalar()),
    )];
    let schemas = [
        OperationSchema {
            key: K::read(),
            parameters: &[],
            result: Domain(K::scalar()),
            required_capability: 1u8,
        },
        OperationSchema {
            key: K::write(),
            parameters: &params,
            result: Domain(K::scalar()),
            required_capability: 2u8,
        },
    ];
    let catalog = OperationCatalog::new(
        1,
        &schemas,
        OperationCatalogLimits {
            max_operations: 2,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let source_host = SourceCallHost {
        catalog: &catalog,
        version: 1,
        granted: &[1, 2],
    };
    let tree = leselang_syntax::parse(source);
    assert!(tree.diagnostics.is_empty());
    let functions: Vec<_> = tree.function.iter().chain(&tree.helpers).collect();
    let compiled = if functions.len() == 1 {
        compile_catalog_source(&functions[0].body, &[], SOURCE, &source_host, host)
    } else {
        compile_catalog_program(
            &functions,
            "main",
            CatalogProgramLimits {
                expression: SOURCE,
                max_functions: 8,
                max_parameters: 2,
                max_compilation_nodes: 2048,
                max_expansion_depth: 16,
            },
            &source_host,
            host,
        )
    };
    compiled
        .unwrap_or_else(|error| panic!("native compilation: {error:?}"))
        .0
}

#[derive(Default)]
struct Store {
    pending: BTreeMap<u64, Vec<u8>>,
    completed: BTreeMap<u64, Vec<u8>>,
    next: u64,
}
impl Store {
    fn restart(self) -> Self {
        let (pending, completed) = self.recover();
        let next = pending
            .keys()
            .chain(completed.keys())
            .copied()
            .max()
            .unwrap_or(0);
        Self {
            pending,
            completed,
            next,
        }
    }
    fn recover(&self) -> (BTreeMap<u64, Vec<u8>>, BTreeMap<u64, Vec<u8>>) {
        let mut staging = CheckpointRecovery::new(16, 1000);
        for (&key, bytes) in &self.pending {
            staging
                .push(RecoveryRecord {
                    key,
                    sequence: key,
                    payload: RecoveryPayload::Pending(bytes.clone()),
                })
                .unwrap();
        }
        for (&key, bytes) in &self.completed {
            staging
                .push(RecoveryRecord {
                    key,
                    sequence: key,
                    payload: RecoveryPayload::Completed(bytes.clone()),
                })
                .unwrap();
        }
        staging
            .finish((), |_, _| Ok::<_, ()>(()))
            .unwrap_or_else(|_| panic!())
            .0
            .into_parts()
    }
    fn outcome<K: Key>(
        &mut self,
        outcome: ResumableEffectOutcome<'_, Node<K>, (), Request<K>, Capture<K>>,
        fuel: &Fuel,
        budget: ResourceBudget,
        revision: Option<u64>,
    ) -> Step<K> {
        let (request, result_binding) = match outcome {
            ResumableEffectOutcome::Value(PureValue::Scalar(value)) => return Step::Done(value),
            ResumableEffectOutcome::Value(_) => panic!("native scalar output"),
            ResumableEffectOutcome::Request(request) => (request, None),
            ResumableEffectOutcome::Suspended {
                request,
                continuation,
            } => {
                let (_, _, capture) = continuation.into_parts();
                (request, Some(Box::new(capture)))
            }
        };
        self.next += 1;
        let image = ContinuationFrame {
            schema_version: 1,
            token: self.next,
            program_counter: 1,
            expected_revision: revision,
            result_type: K::scalar(),
            pending_effect: request,
            fuel_remaining: fuel.remaining(),
            deadline_ms: budget.deadline_ms,
            deadline_at_ms: budget.deadline_at_ms,
            max_output_items: budget.max_output_items,
            result_binding,
            group_result: None,
        };
        self.pending
            .insert(image.token, serde_json::to_vec(&image).unwrap());
        Step::Effect(Box::new(image))
    }
    fn start<K: Key>(
        &mut self,
        host: &Host<K>,
        granted_fuel: u64,
    ) -> Result<Step<K>, &'static str> {
        let node = compile(host);
        let mut fuel = Fuel::new(granted_fuel);
        let mut bindings = vec![];
        let result = evaluate_resumable_effects_in_scope(
            &node,
            &mut ScopeFrame::new(&mut bindings),
            host,
            &mut fuel,
            EXEC,
        )
        .map_err(|_| "execution")?;
        Ok(self.outcome(
            result,
            &fuel,
            ResourceBudget {
                fuel_remaining: granted_fuel,
                deadline_ms: 1000,
                deadline_at_ms: Some(10_000),
                max_output_items: 16,
            },
            Some(host.generation.get()),
        ))
    }
    fn resume<K: Key>(
        &mut self,
        host: &Host<K>,
        candidate: Image<K>,
        actual_identity: u64,
        reply: ScalarValue,
    ) -> Result<Step<K>, &'static str> {
        if let Some(bytes) = self.completed.get(&candidate.token) {
            let settled: Result<Step<K>, String> =
                serde_json::from_slice(bytes).map_err(|_| "terminal")?;
            return settled.map_err(|reason| match reason.as_str() {
                "capture" => "capture",
                "reentry" => "reentry",
                _ => "terminal",
            });
        }
        let original = self.pending.get(&candidate.token).ok_or("missing")?;
        if original.len() > 64 * 1024
            || serde_json::to_vec(&candidate).map_err(|_| "encode")? != *original
        {
            return Err("original");
        }
        if candidate.schema_version != 1
            || candidate.program_counter != 1
            || candidate.group_result.is_some()
            || candidate.result_type != K::scalar()
        {
            return Err("schema");
        }
        candidate
            .resource_budget()
            .validate(BUDGET)
            .map_err(|_| "budget")?;
        if candidate.expected_revision != Some(host.generation.get())
            || candidate
                .deadline_at_ms
                .is_some_and(|deadline| host.now.get() >= deadline)
        {
            return Err("live");
        }
        host.admit(&candidate.pending_effect.operation)?;
        let token = candidate.token;
        let domain = Domain(candidate.result_type);
        host.expected.set(token);
        let mut pending = PendingReply::new(token, candidate, &domain);
        let accepted = pending
            .try_accept(&actual_identity, reply, host)
            .map_err(|_| "reply")?;
        let (_, image, _, reply) = accepted.into_parts();
        let mut fuel = Fuel::new(image.fuel_remaining);
        let result: Result<Step<K>, &'static str> = (|| {
            if let Some(binding) = &image.result_binding {
                validate_captured_bindings(
                    binding,
                    CaptureValidationLimits {
                        max_saved_bindings: 8,
                        max_name_bytes: 64,
                    },
                    |_: &()| Ok::<_, &'static str>(()),
                    |_: &()| Ok::<_, &'static str>(()),
                    |_| Ok::<_, &'static str>(()),
                )
                .map_err(|_| "capture")?;
                let mut bindings = vec![];
                let outcome = resume_captured_effects_in_scope(
                    binding,
                    &mut ScopeFrame::new(&mut bindings),
                    host,
                    &mut fuel,
                    EXEC,
                    CapturedEffectHooks {
                        scalar: |value: &ScalarValue, fuel: &mut Fuel| {
                            fuel.charge(1).map_err(|_| "restore fuel")?;
                            Ok(PureValue::Scalar(value.clone()))
                        },
                        result: |_: &(), _: &mut Fuel| Err("result".into()),
                        group: |_: &(), _: &mut Fuel| Err("group".into()),
                        pending: |fuel: &mut Fuel| {
                            fuel.charge(1).map_err(|_| "restore fuel")?;
                            Ok(PureValue::Scalar(reply))
                        },
                    },
                )
                .map_err(|_| "reentry")?;
                Ok(self.outcome(
                    outcome,
                    &fuel,
                    image.resource_budget(),
                    image.expected_revision,
                ))
            } else {
                Ok(Step::Done(reply))
            }
        })();
        // This model commits journal data only; external invocation is host-owned.
        // Accepted reentry failures settle too: a receipt never refills/re-arms them.
        self.completed.insert(
            token,
            serde_json::to_vec(&result.as_ref().map_err(|error| error.to_string())).unwrap(),
        );
        self.pending.remove(&token);
        result
    }
}
mod saved_stack {
    use super::*;
    use leselang_hir::{
        effect_reentry::{EffectReentryEnvironment, RestoredEffectBindings},
        effect_stack::*,
    };

    const LIMITS: EffectStackLimits = EffectStackLimits {
        effects: EXEC,
        max_frames: 8,
    };
    trait Program: Key {
        fn program() -> &'static str;
        fn expected() -> ScalarValue;
    }
    impl Program for Counter {
        fn program() -> &'static str {
            r#"
fn load() = bind(read: counter.read(), body: add(left: read, right: 1))
fn forward() = bind(data: load(), body: counter.write(value: data))
fn main() = bind(first: forward(), body: bind(last: counter.write(value: add(left: first, right: 1)), body: add(left: first, right: last)))
"#
        }
        fn expected() -> ScalarValue {
            ScalarValue::Integer(85)
        }
    }
    impl Program for Panel {
        fn program() -> &'static str {
            r#"
fn load() = bind(read: panel.read_text(), body: concat(left: read, right: "!"))
fn forward() = bind(data: load(), body: panel.set_text(value: data))
fn main() = bind(first: forward(), body: bind(last: panel.set_text(value: concat(left: first, right: "?")), body: concat(left: first, right: concat(left: "/", right: last))))
"#
        }
        fn expected() -> ScalarValue {
            ScalarValue::String("ready!/ready!?".into())
        }
    }

    enum Saved<'a> {
        Owned(ScalarValue),
        Borrowed(&'a ScalarValue),
    }
    type Snapshot<'a> = Vec<(&'a str, Saved<'a>)>;
    struct Runtime<'h, K>(&'h Host<K>);
    impl<K: Key> PureEvaluationEnvironment<(), K> for Runtime<'_, K> {
        type Result = ();
        type Error = &'static str;
        fn field(&self, _: &(), _: &()) -> Result<ScalarValue, Self::Error> {
            Err("field")
        }
        fn member(&self, _: &(), _: &str, _: &K) -> Result<(), Self::Error> {
            Err("member")
        }
    }
    impl<'a, K: Key> EffectEvaluationEnvironment<'a, (), K, (), ()> for Runtime<'_, K> {
        type Request = Request<K>;
        type Capture = Snapshot<'a>;
        fn preflight_effect(&self, node: &'a Node<K>) -> Result<(), Self::Error> {
            self.0.preflight_effect(node)
        }
        fn prepare_effect(
            &self,
            node: &'a Node<K>,
            scope: &mut ScopeFrame<'_, 'a, PureValue<()>>,
            fuel: &mut Fuel,
        ) -> Result<Request<K>, CalculationFailure<Self::Error>> {
            self.0.prepare_effect(node, scope, fuel)
        }
        fn capture(
            &self,
            _: &'a str,
            _: &'a Node<K>,
            scope: &ScopeFrame<'_, 'a, PureValue<()>>,
            fuel: &mut Fuel,
        ) -> Result<Snapshot<'a>, Self::Error> {
            scope
                .bindings()
                .iter()
                .map(|(name, value)| {
                    let PureValue::Scalar(value) = value else {
                        return Err("capture type");
                    };
                    fuel.charge(1 + scalar_copy_cost(value))
                        .map_err(|_| "capture fuel")?;
                    Ok((*name, Saved::Owned(value.clone())))
                })
                .collect()
        }
    }
    impl<'a, K: Key> EffectReentryEnvironment<'a, (), K, (), (), u64, Domain, ScalarValue>
        for Runtime<'_, K>
    {
        fn restore_capture(
            &self,
            identity: &u64,
            _: &Domain,
            capture: Snapshot<'a>,
            fuel: &mut Fuel,
            _: EffectEvaluationLimits,
        ) -> Result<RestoredEffectBindings<'a, ()>, CalculationFailure<Self::Error>> {
            self.0.authorize(identity)?;
            self.0.restorations.set(self.0.restorations.get() + 1);
            capture
                .into_iter()
                .map(|(name, saved)| {
                    let value = match saved {
                        Saved::Owned(value) => value,
                        Saved::Borrowed(value) => {
                            fuel.charge(scalar_copy_cost(value))
                                .map_err(|_| "restore fuel")?;
                            value.clone()
                        }
                    };
                    Ok((name, PureValue::Scalar(value)))
                })
                .collect()
        }
        fn bind_reply(
            &self,
            identity: &u64,
            domain: &Domain,
            reply: ScalarValue,
            fuel: &mut Fuel,
        ) -> Result<PureValue<()>, CalculationFailure<Self::Error>> {
            self.0.authorize(identity)?;
            validate_host_result(domain, &reply).map_err(|_| "domain")?;
            fuel.charge(1).map_err(|_| "projection fuel")?;
            self.0.projections.set(self.0.projections.get() + 1);
            Ok(PureValue::Scalar(reply))
        }
    }

    type Image<K> = ContinuationFrame<u64, u64, ScalarType, Request<K>, Vec<Capture<K>>>;
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(tag = "kind", content = "payload", deny_unknown_fields)]
    enum Step<K> {
        Effect(Box<Image<K>>),
        Done { value: ScalarValue, remaining: u64 },
    }
    #[derive(Default)]
    struct Journal(Store);
    impl Journal {
        fn restart(self) -> Self {
            Self(self.0.restart())
        }
        fn record<K: Key>(
            &mut self,
            outcome: StackedEffectOutcome<'_, Node<K>, (), Request<K>, Snapshot<'_>>,
            fuel: &Fuel,
            budget: ResourceBudget,
            revision: Option<u64>,
        ) -> Step<K> {
            let (request, frames) = match outcome {
                StackedEffectOutcome::Value(PureValue::Scalar(value)) => {
                    return Step::Done {
                        value,
                        remaining: fuel.remaining(),
                    };
                }
                StackedEffectOutcome::Value(_) => panic!("scalar return required"),
                StackedEffectOutcome::Request(request) => (request, Vec::new()),
                StackedEffectOutcome::Suspended {
                    request,
                    continuation,
                } => {
                    let frames = continuation
                        .into_frames()
                        .into_iter()
                        .map(|frame| {
                            let (name, body, saved) = frame.into_parts();
                            Capture {
                                name: name.into(),
                                body: body.clone(),
                                results: vec![],
                                groups: vec![],
                                locals: saved
                                    .into_iter()
                                    .map(|(name, value)| ScalarBinding {
                                        name: name.into(),
                                        value: match value {
                                            Saved::Owned(v) => v,
                                            Saved::Borrowed(v) => v.clone(),
                                        },
                                    })
                                    .collect(),
                            }
                        })
                        .collect();
                    (request, frames)
                }
            };
            self.0.next += 1;
            let image = ContinuationFrame {
                schema_version: 1,
                token: self.0.next,
                program_counter: 1,
                expected_revision: revision,
                result_type: K::scalar(),
                pending_effect: request,
                fuel_remaining: fuel.remaining(),
                deadline_ms: budget.deadline_ms,
                deadline_at_ms: budget.deadline_at_ms,
                max_output_items: budget.max_output_items,
                result_binding: (!frames.is_empty()).then(|| Box::new(frames)),
                group_result: None,
            };
            let bytes = serde_json::to_vec(&image).unwrap();
            assert!(bytes.len() <= 64 * 1024);
            self.0.pending.insert(image.token, bytes);
            Step::Effect(Box::new(image))
        }
        fn start<K: Program>(
            &mut self,
            host: &Host<K>,
            budget: u64,
        ) -> Result<Step<K>, &'static str> {
            let node = compile_text(host, K::program());
            let runtime = Runtime(host);
            let mut fuel = Fuel::new(budget);
            let mut locals = Vec::new();
            let outcome = evaluate_stacked_effects_in_scope(
                &node,
                &mut ScopeFrame::new(&mut locals),
                &runtime,
                &mut fuel,
                LIMITS,
            )
            .map_err(|_| "start")?;
            Ok(self.record(
                outcome,
                &fuel,
                ResourceBudget {
                    fuel_remaining: budget,
                    deadline_ms: 1000,
                    deadline_at_ms: Some(10_000),
                    max_output_items: 16,
                },
                Some(host.generation.get()),
            ))
        }
        fn resume<K: Program>(
            &mut self,
            host: &Host<K>,
            candidate: Image<K>,
            actual: u64,
            reply: ScalarValue,
        ) -> Result<Step<K>, &'static str> {
            if let Some(bytes) = self.0.completed.get(&candidate.token) {
                let settled: Result<Step<K>, String> =
                    serde_json::from_slice(bytes).map_err(|_| "terminal")?;
                return settled.map_err(|_| "reentry");
            }
            let original = self.0.pending.get(&candidate.token).ok_or("missing")?;
            if original.len() > 64 * 1024
                || serde_json::to_vec(&candidate).map_err(|_| "encode")? != *original
            {
                return Err("original");
            }
            if candidate.schema_version != 1
                || candidate.program_counter != 1
                || candidate.group_result.is_some()
                || candidate.result_type != K::scalar()
            {
                return Err("schema");
            }
            candidate
                .resource_budget()
                .validate(BUDGET)
                .map_err(|_| "budget")?;
            if candidate.expected_revision != Some(host.generation.get())
                || candidate
                    .deadline_at_ms
                    .is_some_and(|deadline| host.now.get() >= deadline)
            {
                return Err("live");
            }
            host.admit(&candidate.pending_effect.operation)?;
            let token = candidate.token;
            host.expected.set(token);
            let domain = Domain(K::scalar());
            let runtime = Runtime(host);
            let mut fuel = Fuel::new(candidate.fuel_remaining);
            let result = if let Some(saved) = &candidate.result_binding {
                if saved.len() > LIMITS.max_frames {
                    return Err("frames");
                }
                let mut frames = Vec::new();
                for row in saved.iter() {
                    validate_captured_bindings(
                        row,
                        CaptureValidationLimits {
                            max_saved_bindings: 8,
                            max_name_bytes: 64,
                        },
                        |_: &()| Err("result"),
                        |_: &()| Err("group"),
                        |_| Ok::<_, &'static str>(()),
                    )
                    .map_err(|_| "capture")?;
                    frames.push(SavedEffectReturnFrame {
                        name: &row.name,
                        body: &row.body,
                        capture: row
                            .locals
                            .iter()
                            .map(|v| (v.name.as_str(), Saved::Borrowed(&v.value)))
                            .collect(),
                    });
                }
                let stack = restore_effect_return_stack(frames, LIMITS).map_err(|_| "frames")?;
                let mut pending = PendingReply::new(token, stack, &domain);
                let accepted = pending
                    .try_accept(&actual, reply, host)
                    .map_err(|_| "reply")?;
                resume_accepted_stacked_effects(accepted, &runtime, &mut fuel, LIMITS)
                    .map(|outcome| {
                        self.record(
                            outcome,
                            &fuel,
                            candidate.resource_budget(),
                            candidate.expected_revision,
                        )
                    })
                    .map_err(|_| "reentry")
            } else {
                let mut pending = PendingReply::new(token, (), &domain);
                let accepted = pending
                    .try_accept(&actual, reply, host)
                    .map_err(|_| "reply")?;
                Ok(Step::Done {
                    value: accepted.into_parts().3,
                    remaining: fuel.remaining(),
                })
            };
            self.0.completed.insert(
                token,
                serde_json::to_vec(&result.as_ref().map_err(|error| error.to_string())).unwrap(),
            );
            self.0.pending.remove(&token);
            result
        }
    }

    fn run<K: Program>(budget: u64) -> Result<u64, &'static str> {
        let host = Host::<K>::new();
        let mut journal = Journal::default();
        let mut step = journal.start(&host, budget)?;
        let mut old = Vec::new();
        for turn in 0..4 {
            match step {
                Step::Effect(image) => {
                    assert!(turn < 3);
                    if turn == 0 {
                        assert_eq!(image.result_binding.as_ref().unwrap().len(), 3);
                    }
                    let reply = host.invoke(&image.pending_effect);
                    journal = journal.restart();
                    let restored: Image<K> =
                        serde_json::from_slice(&journal.0.pending[&image.token]).unwrap();
                    assert_eq!(restored, *image);
                    let result = journal.resume(&host, restored, image.token, reply);
                    let work = (
                        host.preparations.get(),
                        host.restorations.get(),
                        host.projections.get(),
                    );
                    journal = journal.restart();
                    assert_eq!(
                        journal.resume(&host, *image.clone(), image.token, ScalarValue::None),
                        result
                    );
                    assert_eq!(
                        work,
                        (
                            host.preparations.get(),
                            host.restorations.get(),
                            host.projections.get()
                        )
                    );
                    step = result?;
                    if let Step::Effect(next) = &step {
                        assert!(next.fuel_remaining < image.fuel_remaining);
                        assert_eq!(next.expected_revision, image.expected_revision);
                        assert_eq!(next.resource_budget().deadline_at_ms, image.deadline_at_ms);
                        assert_eq!(next.max_output_items, image.max_output_items);
                    }
                    old.push(*image);
                }
                Step::Done { value, remaining } => {
                    assert_eq!(turn, 3);
                    assert_eq!(value, K::expected());
                    assert_eq!(host.invocations.borrow().len(), 3);
                    assert_eq!(host.preparations.get(), 3);
                    assert_eq!(host.projections.get(), 3);
                    assert!(host.restorations.get() > 3);
                    assert!(journal.0.pending.is_empty());
                    for previous in old {
                        assert!(
                            journal
                                .resume(&host, previous.clone(), previous.token, ScalarValue::None)
                                .is_ok()
                        );
                    }
                    assert_eq!(host.preparations.get(), 3);
                    return Ok(remaining);
                }
            }
        }
        panic!("saved return pipeline did not finish");
    }

    #[test]
    fn two_native_hosts_restore_serialized_helper_return_stacks_without_recompilation_or_replay() {
        assert!(run::<Counter>(1000).is_ok());
        assert!(run::<Panel>(1000).is_ok());
    }
    #[test]
    fn restored_nested_helpers_keep_one_budget_and_settle_every_accepted_fuel_refusal() {
        fn check<K: Program>() {
            let used = 1000 - run::<K>(1000).unwrap();
            for budget in 0..used {
                assert!(run::<K>(budget).is_err(), "{budget} < {used}");
            }
            assert_eq!(run::<K>(used), Ok(0));
            assert_eq!(run::<K>(used + 1), Ok(1));
        }
        check::<Counter>();
        check::<Panel>();
    }
    #[test]
    fn saved_return_stacks_reject_forged_order_budget_reply_and_live_policy_before_restoration() {
        fn check<K: Program>() {
            for mode in 0..8 {
                let host = Host::<K>::new();
                let mut journal = Journal::default();
                let Step::Effect(mut image) = journal.start(&host, 1000).unwrap() else {
                    panic!()
                };
                let mut identity = image.token;
                let mut reply = K::initial();
                match mode {
                    0 => image.fuel_remaining += 1,
                    1 => image.result_binding.as_mut().unwrap().swap(0, 1),
                    2 => identity += 1,
                    3 => reply = ScalarValue::None,
                    4 => host.grants.set(0),
                    5 => host.generation.set(8),
                    6 => host.now.set(10_000),
                    7 => {
                        image.result_binding.as_mut().unwrap()[2].body = Node::Literal {
                            value: ScalarValue::None,
                        }
                    }
                    _ => unreachable!(),
                }
                journal = journal.restart();
                let original = journal.0.pending.clone();
                assert!(journal.resume(&host, *image, identity, reply).is_err());
                assert_eq!(journal.0.pending, original);
                assert!(journal.0.completed.is_empty());
                assert_eq!(host.preparations.get(), 1);
                assert_eq!(host.restorations.get(), 0);
                assert_eq!(host.projections.get(), 0);
                assert!(host.invocations.borrow().is_empty());
            }
        }
        check::<Counter>();
        check::<Panel>();
    }
    #[test]
    fn revoked_outer_saved_body_is_settled_without_restoring_or_preparing_inner_work() {
        fn check<K: Program>() {
            let host = Host::<K>::new();
            let mut journal = Journal::default();
            let Step::Effect(image) = journal.start(&host, 1000).unwrap() else {
                panic!()
            };
            let reply = host.invoke(&image.pending_effect);
            host.grants.set(1);
            journal = journal.restart();
            assert_eq!(
                journal.resume(&host, *image.clone(), image.token, reply),
                Err("reentry")
            );
            assert_eq!(host.restorations.get(), 0);
            assert_eq!(host.projections.get(), 0);
            assert_eq!(host.preparations.get(), 1);
            journal = journal.restart();
            host.grants.set(3);
            assert_eq!(
                journal.resume(&host, *image.clone(), image.token, K::initial()),
                Err("reentry")
            );
            assert_eq!(host.preparations.get(), 1);
            assert_eq!(host.invocations.borrow().len(), 1);
        }
        check::<Counter>();
        check::<Panel>();
    }
}

fn image<K: Key>(step: Step<K>) -> Image<K> {
    let Step::Effect(image) = step else {
        panic!("effect expected")
    };
    *image
}

fn round_trip<K: Key>(expected: ScalarValue) {
    let host = Host::<K>::new();
    let mut store = Store::default();
    let first = image(store.start(&host, 1000).unwrap());
    let original_fuel = first.fuel_remaining;
    let first_reply = host.invoke(&first.pending_effect);
    store = store.restart();
    let restored: Image<K> = serde_json::from_slice(&store.pending[&first.token]).unwrap();
    assert_eq!(restored.fuel_remaining, original_fuel);
    let second = image(
        store
            .resume(&host, restored, first.token, first_reply)
            .unwrap(),
    );
    assert!(second.fuel_remaining < original_fuel);
    assert_eq!(second.expected_revision, first.expected_revision);
    assert_eq!(second.deadline_at_ms, first.deadline_at_ms);
    let preparations = host.preparations.get();
    assert_eq!(
        store
            .resume(&host, first.clone(), first.token, ScalarValue::None)
            .unwrap(),
        Step::Effect(Box::new(second.clone()))
    );
    assert_eq!(host.preparations.get(), preparations);
    let second_reply = host.invoke(&second.pending_effect);
    store = store.restart();
    let restored: Image<K> = serde_json::from_slice(&store.pending[&second.token]).unwrap();
    assert_eq!(
        store
            .resume(&host, restored, second.token, second_reply)
            .unwrap(),
        Step::Done(expected.clone())
    );
    for old in [first, second] {
        // Earlier committed successor receipts remain data, not redispatch authority.
        assert!(
            store
                .resume(&host, old.clone(), old.token, ScalarValue::None)
                .is_ok()
        );
    }
    assert!(store.pending.is_empty());
    assert_eq!(host.invocations.borrow().len(), 2);
}

#[test]
fn two_parsed_native_schemas_restart_saved_control_frames_without_replaying_effects() {
    round_trip::<Counter>(ScalarValue::Integer(83));
    round_trip::<Panel>(ScalarValue::String("readyready!".into()));
}

#[test]
fn forged_frames_stale_identity_wrong_reply_denied_capability_and_deadline_never_reenter() {
    for mode in 0..7 {
        let host = Host::<Counter>::new();
        let mut store = Store::default();
        let first = image(store.start(&host, 1000).unwrap());
        let mut candidate = first.clone();
        let mut identity = first.token;
        let mut reply = ScalarValue::Integer(41);
        match mode {
            0 => candidate.fuel_remaining += 1,
            1 => {
                candidate.result_binding.as_mut().unwrap().body = Node::Literal {
                    value: ScalarValue::Integer(999),
                }
            }
            2 => identity += 1,
            3 => reply = ScalarValue::String("wrong".into()),
            4 => host.grants.set(0),
            5 => host.generation.set(8),
            _ => host.now.set(10_000),
        }
        let original = store.pending.clone();
        let preparations = host.preparations.get();
        assert!(store.resume(&host, candidate, identity, reply).is_err());
        assert_eq!(store.pending, original);
        assert!(store.completed.is_empty());
        assert_eq!(host.preparations.get(), preparations);
        assert!(host.invocations.borrow().is_empty());
    }
}

#[test]
fn restarted_control_uses_the_saved_budget_instead_of_a_fresh_grant() {
    let mut completed = false;
    let mut reentry_refused = false;
    for granted in 0..32 {
        let host = Host::<Counter>::new();
        let mut store = Store::default();
        let Ok(first) = store.start(&host, granted) else {
            continue;
        };
        let first = image(first);
        assert!(first.fuel_remaining < granted);
        let reply = host.invoke(&first.pending_effect);
        let restored: Image<Counter> =
            serde_json::from_slice(&store.pending[&first.token]).unwrap();
        match store.resume(&host, restored, first.token, reply) {
            Ok(Step::Effect(second)) => {
                assert!(second.fuel_remaining < first.fuel_remaining);
                let reply = host.invoke(&second.pending_effect);
                match store.resume(&host, *second.clone(), second.token, reply) {
                    Ok(Step::Done(value)) => {
                        assert_eq!(value, ScalarValue::Integer(83));
                        completed = true;
                    }
                    Err(_) => {
                        reentry_refused = true;
                        assert_refusal_replay(&mut store, &host, &second);
                    }
                    _ => panic!("terminal expected"),
                }
            }
            Err(_) => {
                reentry_refused = true;
                assert_eq!(host.invocations.borrow().len(), 1);
                assert_refusal_replay(&mut store, &host, &first);
            }
            _ => panic!("successor expected"),
        }
    }
    assert!(completed && reentry_refused);
}

fn assert_refusal_replay(store: &mut Store, host: &Host<Counter>, frame: &Image<Counter>) {
    let preparations = host.preparations.get();
    let invocations = host.invocations.borrow().len();
    let owned = std::mem::take(store);
    *store = owned.restart();
    assert!(!store.pending.contains_key(&frame.token));
    assert!(store.completed.contains_key(&frame.token));
    for _ in 0..3 {
        assert_eq!(
            store.resume(host, frame.clone(), frame.token, ScalarValue::Integer(41)),
            Err("reentry")
        );
    }
    assert_eq!(host.preparations.get(), preparations);
    assert_eq!(host.invocations.borrow().len(), invocations);
}

#[test]
fn successor_inherits_original_policy_and_recovered_sequence_watermark() {
    let host = Host::<Counter>::new();
    let mut store = Store::default();
    let mut first = image(store.start(&host, 1000).unwrap());
    // Simulate the host's original persisted policy, not a submitted forged frame.
    first.deadline_ms = 42;
    first.deadline_at_ms = Some(500);
    first.max_output_items = 3;
    store
        .pending
        .insert(first.token, serde_json::to_vec(&first).unwrap());
    store = store.restart();
    let reply = host.invoke(&first.pending_effect);
    let second = image(
        store
            .resume(&host, first.clone(), first.token, reply)
            .unwrap(),
    );
    assert_eq!(second.token, first.token + 1);
    assert_eq!(second.deadline_ms, 42);
    assert_eq!(second.deadline_at_ms, Some(500));
    assert_eq!(second.max_output_items, 3);
    assert!(second.fuel_remaining < first.fuel_remaining);
    assert_eq!(second.expected_revision, first.expected_revision);
}

#[test]
fn settled_capture_refusal_retains_its_original_reason_after_checkpoint_recovery() {
    let host = Host::<Counter>::new();
    let mut store = Store::default();
    let mut first = image(store.start(&host, 1000).unwrap());
    // Model a malformed persisted row, not authority for a submitted replacement.
    let locals = &mut first.result_binding.as_mut().unwrap().locals;
    for n in 0..9 {
        locals.push(ScalarBinding {
            name: format!("invalid{n}"),
            value: ScalarValue::Integer(n),
        });
    }
    store
        .pending
        .insert(first.token, serde_json::to_vec(&first).unwrap());
    let preparations = host.preparations.get();
    assert_eq!(
        store.resume(&host, first.clone(), first.token, ScalarValue::Integer(41)),
        Err("capture")
    );
    store = store.restart();
    assert_eq!(
        store.resume(&host, first.clone(), first.token, ScalarValue::None),
        Err("capture")
    );
    assert!(store.pending.is_empty());
    assert_eq!(host.preparations.get(), preparations);
    assert!(host.invocations.borrow().is_empty());
}
