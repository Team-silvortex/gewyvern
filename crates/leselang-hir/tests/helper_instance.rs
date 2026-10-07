use std::cell::{Cell, RefCell};
use std::collections::VecDeque;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::bound_projection_source::{
    BoundProjectionSourceEnvironment, BoundProjectionSourceLimits, lower_bound_projection_source,
};
use leselang_hir::helper_body::{HelperBody, HelperBodyLimits, LoweredHelperBody};
use leselang_hir::helper_expansion::HelperExpansionError;
use leselang_hir::helper_hygiene::HelperHygieneError;
use leselang_hir::helper_instance::*;
use leselang_hir::helper_templates::{HelperTemplateError, HelperTemplateLimits};
use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::*;
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, TypeInferenceLimits, infer_pure_type,
};
use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
use leselang_hir::source_call::SourceCallLimits;
use leselang_hir::source_cost::{SourceCostExtra, SourceCostLimits};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, NamedArgument, parse};

// Native slots, result observations and errors have no Clone/Debug/serde/Send.
struct Native {
    tag: &'static str,
    buffer: Vec<u8>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Native {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct ReturnObservation {
    scalar: Option<ScalarType>,
    owner: Rc<()>,
}
struct PrivateError(&'static str);
type Node = Computation<Native, Native, Native, Native>;
type Body = HelperBody<Node, ReturnObservation>;
const SOURCE: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 256,
    max_source_depth: 32,
    max_lowered_nodes: 256,
    max_lowered_depth: 32,
    max_arguments: 64,
};
const LIMITS: HelperInstanceLimits = HelperInstanceLimits {
    source: SOURCE,
    max_parameters: 8,
    max_bindings: 16,
    max_reserved_names: 256,
};
const TYPING: TypeInferenceLimits = TypeInferenceLimits {
    max_nodes: 256,
    max_depth: 32,
    max_bindings: 16,
};
const PURE: PureEvaluationLimits = PureEvaluationLimits {
    max_nodes: 256,
    max_depth: 32,
    max_bindings: 16,
};
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
fn bind(name: &str, value: Node, body: Node) -> Node {
    Node::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn expression(source: &str) -> Expression {
    let tree = parse(&format!("fn main() = {source}"));
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    tree.function.unwrap().body
}
fn native(tag: &'static str, drops: &Rc<Cell<usize>>) -> Native {
    Native {
        tag,
        buffer: vec![19; 37],
        drops: Rc::clone(drops),
    }
}
fn prepare(parameters: Vec<(String, ScalarType)>, value: Node, scalar: Option<ScalarType>) -> Body {
    LoweredHelperBody {
        parameters,
        expression: value,
        result_type: ReturnObservation {
            scalar,
            owner: Rc::new(()),
        },
        scalar_result: scalar,
    }
    .prepare(
        HelperBodyLimits {
            template: HelperTemplateLimits {
                max_nodes: 256,
                max_depth: 32,
                max_bindings: 16,
                max_parameters: 8,
            },
            source_cost: SourceCostLimits {
                max_nodes: 256,
                max_depth: 32,
            },
        },
        |_, _, _, _| Ok::<_, PrivateError>(()),
        |_| Ok::<_, PrivateError>(SourceCostExtra { nodes: 0, depth: 0 }),
    )
    .unwrap()
}
fn work_body() -> Body {
    prepare(
        vec![("n".into(), ScalarType::Integer)],
        add(local("n"), integer(1)),
        Some(ScalarType::Integer),
    )
}
fn selected<'body, 'names>(
    body: &'body Body,
    reserved: &'names [&'names str],
) -> SelectedHelper<'body, 'names, Node, ReturnObservation> {
    SelectedHelper {
        name: "work",
        body,
        reserved_names: reserved,
        caller_depth: 0,
    }
}
fn denied<T>(
    result: Result<T, HelperInstanceError<PrivateError>>,
) -> HelperInstanceError<PrivateError> {
    match result {
        Ok(_) => panic!("expected rejection"),
        Err(error) => error,
    }
}
struct Host {
    fields: RefCell<Vec<&'static str>>,
}
struct RecordExports<'a> {
    drops: &'a Rc<Cell<usize>>,
    buffers: &'a mut Vec<*const u8>,
}
impl<'source> BoundProjectionSourceEnvironment<'source, Native, Native> for RecordExports<'_> {
    type Result = ();
    type Error = PrivateError;
    fn field(
        &mut self,
        _: &(),
        name: &'source str,
    ) -> Result<Option<(Native, ScalarType)>, PrivateError> {
        if name != "count" {
            return Ok(None);
        }
        let field = native("argument", self.drops);
        self.buffers.push(field.buffer.as_ptr());
        Ok(Some((field, ScalarType::Integer)))
    }
    fn member(
        &mut self,
        _: &(),
        _: &'source str,
        _: &'source str,
    ) -> Result<Option<(Native, ())>, PrivateError> {
        Ok(None)
    }
}
impl PureTypeEnvironment<Native, Native> for Host {
    type Result = ();
    fn field_type(&self, _: &(), field: &Native) -> Option<ScalarType> {
        (field.tag == "argument").then_some(ScalarType::Integer)
    }
    fn member_result(&self, _: &(), _: &str, _: &Native) -> Option<()> {
        None
    }
}
impl PureEvaluationEnvironment<Native, Native> for Host {
    type Result = ();
    type Error = PrivateError;
    fn field(&self, _: &(), field: &Native) -> Result<ScalarValue, PrivateError> {
        self.fields.borrow_mut().push(field.tag);
        Ok(ScalarValue::Integer(7))
    }
    fn member(&self, _: &(), _: &str, _: &Native) -> Result<(), PrivateError> {
        Err(PrivateError("private member"))
    }
}
struct Adapter {
    events: Vec<String>,
    source_arguments: Vec<*const NamedArgument>,
    argument_buffers: Vec<*const u8>,
    copied_buffer: Option<*const u8>,
    aliases: VecDeque<String>,
    next_name: usize,
    fail: Option<&'static str>,
    unwind: Option<&'static str>,
    depth: Option<usize>,
    corrupt_copy: bool,
    enlarge_copy: bool,
    lie: bool,
    drops: Rc<Cell<usize>>,
    original_body: *const Node,
    original_result: *const ReturnObservation,
    original_owner: Rc<()>,
}
impl Adapter {
    fn new(body: &Body) -> Self {
        Self {
            events: vec![],
            source_arguments: vec![],
            argument_buffers: vec![],
            copied_buffer: None,
            aliases: VecDeque::new(),
            next_name: 1,
            fail: None,
            unwind: None,
            depth: None,
            corrupt_copy: false,
            enlarge_copy: false,
            lie: false,
            drops: Rc::new(Cell::new(0)),
            original_body: body.template().body(),
            original_result: body.template().result_type(),
            original_owner: Rc::clone(&body.template().result_type().owner),
        }
    }
    fn gate(&mut self, phase: &'static str) -> Result<(), PrivateError> {
        self.events.push(phase.into());
        assert_ne!(self.unwind, Some(phase), "trusted native unwind");
        if self.fail == Some(phase) {
            Err(PrivateError("secret native payload"))
        } else {
            Ok(())
        }
    }
    // Explicit bounded test-host copying policy, not a language/native Clone ABI.
    fn copy(&mut self, source: &Node) -> Node {
        match source {
            Node::Literal { value } => Node::Literal {
                value: value.clone(),
            },
            Node::Local { name } => local(name),
            Node::Binary {
                operator,
                left,
                right,
            } => Node::Binary {
                operator: *operator,
                left: Box::new(self.copy(left)),
                right: Box::new(self.copy(right)),
            },
            Node::Bind { name, value, body } => bind(name, self.copy(value), self.copy(body)),
            Node::Host { effect } => {
                let effect = native(effect.tag, &self.drops);
                self.copied_buffer = Some(effect.buffer.as_ptr());
                Node::Host {
                    effect: Box::new(effect),
                }
            }
            _ => panic!("outside the closed test-host copy profile"),
        }
    }
}
impl<'source> HelperInstanceAdapter<'source, Native, Native, Native, Native, ReturnObservation>
    for Adapter
{
    type Error = PrivateError;
    fn lower_argument(
        &mut self,
        argument: &'source NamedArgument,
    ) -> HelperInstanceOperand<Node, PrivateError> {
        self.gate("lower")?;
        self.events.push(format!("argument:{}", argument.name));
        self.source_arguments.push(argument);
        let names = [
            ("caller", ScalarType::Integer),
            ("later", ScalarType::Integer),
            ("unused", ScalarType::Integer),
            ("_fresh0", ScalarType::Integer),
        ];
        let (value, ty) = lower_scalar_source_with_scope(
            &argument.value,
            ScalarSourceLimits {
                source: SOURCE,
                max_bindings: 16,
            },
            &names,
            |source, _| {
                let Expression::Call { callee, .. } = source else {
                    return Err(PrivateError("private unknown alias"));
                };
                if callee != "field" {
                    return Err(PrivateError("private undeclared operation"));
                }
                let original = PureType::Result(());
                let (value, ty) = lower_bound_projection_source(
                    source,
                    &[("record", &original)],
                    BoundProjectionSourceLimits {
                        source: SOURCE,
                        max_bindings: 16,
                    },
                    &mut RecordExports {
                        drops: &self.drops,
                        buffers: &mut self.argument_buffers,
                    },
                )
                .map_err(|_| PrivateError("private declared projection"))?;
                let PureType::Scalar(ty) = ty else {
                    return Err(PrivateError("private field result"));
                };
                Ok::<_, PrivateError>((value, Some(ty)))
            },
        )
        .map_err(|_| PrivateError("private scalar source"))?;
        Ok((value, Some(if self.lie { ScalarType::Integer } else { ty })))
    }
    fn argument_depth(&mut self, value: &Node) -> Result<usize, PrivateError> {
        self.gate("depth")?;
        if let Some(depth) = self.depth {
            return Ok(depth);
        }
        let mut pending = vec![(value, 0)];
        let mut maximum = 0;
        while let Some((node, depth)) = pending.pop() {
            maximum = maximum.max(depth);
            pending.extend(node.children().map(|child| (child, depth + 1)));
        }
        Ok(maximum)
    }
    fn materialize(&mut self, body: &Node) -> Result<Node, PrivateError> {
        self.gate("copy")?;
        assert_eq!(body as *const Node, self.original_body);
        let mut value = self.copy(body);
        if self.corrupt_copy {
            let Node::Binary { right, .. } = &mut value else {
                panic!()
            };
            **right = Node::Literal {
                value: ScalarValue::Boolean(false),
            };
        }
        if self.enlarge_copy {
            value = bind("changed", integer(0), value);
        }
        Ok(value)
    }
    fn fresh_name(&mut self) -> Result<String, PrivateError> {
        self.gate("name")?;
        if let Some(name) = self.aliases.pop_front() {
            return Ok(name);
        }
        let name = format!("_fresh{}", self.next_name);
        self.next_name += 1;
        Ok(name)
    }
    fn admit(&mut self, value: &Node, result: &ReturnObservation) -> Result<(), PrivateError> {
        self.gate("admit")?;
        assert_eq!(result as *const ReturnObservation, self.original_result);
        assert!(Rc::ptr_eq(&result.owner, &self.original_owner));
        if value.is_pure() {
            let host = Host {
                fields: RefCell::new(vec![]),
            };
            let names = [
                ("record", PureType::Result(())),
                ("caller", PureType::Scalar(ScalarType::Integer)),
                ("later", PureType::Scalar(ScalarType::Integer)),
                ("unused", PureType::Scalar(ScalarType::Integer)),
                ("_fresh0", PureType::Scalar(ScalarType::Integer)),
            ];
            let inferred = infer_pure_type(value, &names, &host, TYPING)
                .map_err(|_| PrivateError("private cold type failure"))?;
            if inferred != PureType::Scalar(result.scalar.ok_or(PrivateError("private return"))?) {
                return Err(PrivateError("private mismatched result"));
            }
        } else {
            // The only opaque profile is one closed original 'host' payload.
            let mut tail = value;
            while let Node::Bind { body, .. } = tail {
                tail = body;
            }
            if !matches!(tail, Node::Host { effect } if effect.tag == "host")
                || result.scalar.is_some()
            {
                return Err(PrivateError("private opaque profile"));
            }
        }
        Ok(())
    }
}

#[test]
fn parsed_reordered_arguments_run_once_in_declaration_order_with_original_result_and_hygienic_scope()
 {
    let tree = parse(
        "fn work(n: integer, m: integer) = bind(temp: add(left: n, right: 1), body: add(left: temp, right: m))\nfn main() = work(m: 5, n: field(value: record, name: \"count\"))",
    );
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    let function = &tree.helpers[0];
    let parameters = function
        .parameters
        .iter()
        .map(|parameter| (parameter.name.clone(), ScalarType::Integer))
        .collect();
    let value = lower_scalar_source_with_scope(
        &function.body,
        ScalarSourceLimits {
            source: SOURCE,
            max_bindings: 16,
        },
        &[("n", ScalarType::Integer), ("m", ScalarType::Integer)],
        |_, _| -> Result<(Node, Option<ScalarType>), PrivateError> {
            panic!("no native helper body leaves")
        },
    )
    .unwrap()
    .0;
    let body = prepare(parameters, value, Some(ScalarType::Integer));
    let source = &tree.function.as_ref().unwrap().body;
    let mut adapter = Adapter::new(&body);
    adapter.next_name = 1;
    let mut used = 7;
    let reserved = ["caller", "record", "unused", "_fresh0"];
    let (output, result) = lower_helper_instance(
        source,
        selected(&body, &reserved),
        LIMITS,
        &mut used,
        &mut adapter,
    )
    .unwrap();
    assert!(std::ptr::eq(result, body.template().result_type()));
    assert_eq!(used, 7 + body.source_cost().nodes + 2);
    assert_eq!(
        adapter.events,
        [
            "lower",
            "argument:n",
            "lower",
            "argument:m",
            "depth",
            "depth",
            "copy",
            "name",
            "name",
            "name",
            "admit"
        ]
    );
    let Expression::Call { arguments, .. } = source else {
        panic!()
    };
    assert_eq!(
        adapter.source_arguments,
        [&arguments[1] as *const _, &arguments[0] as *const _]
    );
    let Node::Bind {
        name,
        value,
        body: second,
    } = &output
    else {
        panic!()
    };
    assert_eq!(name, "_fresh1");
    let Node::Field { field, .. } = value.as_ref() else {
        panic!()
    };
    assert_eq!(field.buffer.as_ptr(), adapter.argument_buffers[0]);
    let Node::Bind {
        name: second_name,
        body: inner,
        ..
    } = second.as_ref()
    else {
        panic!()
    };
    assert_eq!(second_name, "_fresh2");
    assert!(matches!(inner.as_ref(), Node::Bind { name, .. } if name == "_fresh3"));
    let host = Host {
        fields: RefCell::new(vec![]),
    };
    let mut values = vec![
        ("record", PureValue::Result(())),
        ("_fresh0", PureValue::Scalar(ScalarValue::Integer(91))),
    ];
    let mut scope = ScopeFrame::new(&mut values);
    let mut fuel = Fuel::new(100);
    let result = evaluate_pure_in_scope(&output, &mut scope, &host, &mut fuel, PURE).unwrap();
    assert!(matches!(
        result,
        PureValue::Scalar(ScalarValue::Integer(13))
    ));
    assert_eq!(*host.fields.borrow(), ["argument"]);
    assert_eq!(scope.len(), 2);
    assert!(scope.get("_fresh1").is_none());
    assert_eq!(adapter.drops.get(), 0);
    drop(scope);
    drop(values);
    drop(output);
    assert_eq!(adapter.drops.get(), 1);
}

#[test]
fn all_six_scalar_results_support_zero_parameter_instances_without_native_metadata_copy() {
    for value in [
        ScalarValue::Integer(7),
        ScalarValue::Boolean(true),
        ScalarValue::String("text".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::StringList(StringListValue(vec!["one".into()])),
    ] {
        let body = prepare(
            vec![],
            Node::Literal {
                value: value.clone(),
            },
            Some(value.scalar_type()),
        );
        let mut adapter = Adapter::new(&body);
        let mut used = 1;
        let (output, result) = lower_helper_instance(
            &expression("work()"),
            selected(&body, &[]),
            HelperInstanceLimits {
                max_parameters: 0,
                max_reserved_names: 0,
                max_bindings: 0,
                ..LIMITS
            },
            &mut used,
            &mut adapter,
        )
        .unwrap();
        assert_eq!(result.scalar, Some(value.scalar_type()));
        assert_eq!(adapter.events, ["copy", "admit"]);
        assert_eq!(used, 1 + body.source_cost().nodes);
        let mut values = vec![];
        let result = evaluate_pure_in_scope(
            &output,
            &mut ScopeFrame::new(&mut values),
            &Host {
                fields: RefCell::new(vec![]),
            },
            &mut Fuel::new(100),
            PURE,
        )
        .unwrap();
        let PureValue::Scalar(actual) = result else {
            panic!("expected scalar")
        };
        assert_eq!(actual, value);
    }
}

#[test]
fn malformed_selection_and_all_named_keys_fail_before_argument_callbacks() {
    let body = work_body();
    for source in ["other(n: 1)", "work()", "work(m: 1)", "work(n: 1, n: 2)"] {
        let mut adapter = Adapter::new(&body);
        let mut used = 2;
        assert!(matches!(
            denied(lower_helper_instance(
                &expression(source),
                selected(&body, &[]),
                LIMITS,
                &mut used,
                &mut adapter
            )),
            HelperInstanceError::Source(_)
        ));
        assert!(adapter.events.is_empty());
        assert_eq!(used, 2);
    }
}

#[test]
fn literal_type_facts_cannot_be_overridden_by_a_native_observation() {
    let body = work_body();
    let mut adapter = Adapter::new(&body);
    adapter.lie = true;
    let mut used = 2;
    assert!(matches!(
        denied(lower_helper_instance(
            &expression("work(n: false)"),
            selected(&body, &[]),
            LIMITS,
            &mut used,
            &mut adapter
        )),
        HelperInstanceError::Source(_)
    ));
    assert_eq!(adapter.events, ["lower", "argument:n"]);
    assert_eq!(used, 2);
}

#[test]
fn exact_combined_output_and_source_reservations_precede_factory_work() {
    let body = prepare(
        vec![
            ("n".into(), ScalarType::Integer),
            ("m".into(), ScalarType::Integer),
        ],
        add(local("n"), local("m")),
        Some(ScalarType::Integer),
    );
    let source = expression("work(m: 2, n: 1)");
    for (nodes, depth) in [(6, 3), (7, 2), (0, 0)] {
        let mut adapter = Adapter::new(&body);
        let mut used = 5;
        assert!(matches!(
            denied(lower_helper_instance(
                &source,
                selected(&body, &[]),
                HelperInstanceLimits {
                    source: SourceCallLimits {
                        max_lowered_nodes: nodes,
                        max_lowered_depth: depth,
                        ..SOURCE
                    },
                    ..LIMITS
                },
                &mut used,
                &mut adapter
            )),
            HelperInstanceError::Output(_)
                | HelperInstanceError::Source(_)
                | HelperInstanceError::Template(_)
        ));
        assert!(
            !adapter
                .events
                .iter()
                .any(|event| matches!(event.as_str(), "depth" | "copy" | "name" | "admit"))
        );
        assert_eq!(used, 5);
    }
    let mut adapter = Adapter::new(&body);
    let mut used = 5;
    let output = lower_helper_instance(
        &source,
        selected(&body, &[]),
        HelperInstanceLimits {
            source: SourceCallLimits {
                max_source_nodes: 10,
                max_lowered_nodes: 7,
                max_lowered_depth: 3,
                ..SOURCE
            },
            ..LIMITS
        },
        &mut used,
        &mut adapter,
    )
    .unwrap()
    .0;
    assert_eq!(used, 10);
    assert!(matches!(output, Node::Bind { .. }));
}

#[test]
fn source_node_depth_and_checked_caller_offsets_deny_before_materialization_without_refund() {
    let body = work_body();
    for policy in [0, 1, 2, 3] {
        let mut adapter = Adapter::new(&body);
        let mut used = 2;
        let mut selection = selected(&body, &[]);
        let mut limits = LIMITS;
        match policy {
            0 => limits.source.max_source_nodes = 5,
            1 => adapter.depth = Some(32),
            2 => selection.caller_depth = usize::MAX,
            _ => adapter.fail = Some("depth"),
        }
        assert!(matches!(
            denied(lower_helper_instance(
                &expression("work(n: 1)"),
                selection,
                limits,
                &mut used,
                &mut adapter
            )),
            HelperInstanceError::Expansion(_)
        ));
        assert!(!adapter.events.iter().any(|event| event == "copy"));
        assert_eq!(used, 2);
    }
}

#[test]
fn caller_unused_names_and_cold_operand_names_cannot_be_captured_by_parameter_aliases() {
    let body = work_body();
    for (source, reserved, alias) in [
        ("work(n: 1)", vec!["unused"], "unused"),
        (
            "work(n: choose(when: true, then: 1, otherwise: later))",
            vec![],
            "later",
        ),
        (
            "work(n: bind(local_unused: 1, body: local_unused))",
            vec![],
            "local_unused",
        ),
        ("work(n: 1)", vec![], "n"),
    ] {
        let mut adapter = Adapter::new(&body);
        adapter.aliases.push_back(alias.into());
        let mut used = 2;
        assert!(matches!(
            denied(lower_helper_instance(
                &expression(source),
                selected(&body, &reserved),
                LIMITS,
                &mut used,
                &mut adapter
            )),
            HelperInstanceError::Hygiene(HelperHygieneError::InvalidParameters)
        ));
        assert!(!adapter.events.iter().any(|event| event == "admit"));
        assert_eq!(used, 6);
    }
}

#[test]
fn invalid_reservations_and_operand_name_growth_are_fenced_before_expansion() {
    let body = work_body();
    for reserved in [vec!["bad-name"], vec!["one", "two"]] {
        let mut adapter = Adapter::new(&body);
        let mut used = 2;
        assert!(matches!(
            denied(lower_helper_instance(
                &expression("work(n: 1)"),
                selected(&body, &reserved),
                HelperInstanceLimits {
                    max_reserved_names: 1,
                    ..LIMITS
                },
                &mut used,
                &mut adapter
            )),
            HelperInstanceError::ReservedNames
        ));
        assert!(adapter.events.is_empty());
        assert_eq!(used, 2);
    }
    let mut adapter = Adapter::new(&body);
    let mut used = 2;
    assert!(matches!(
        denied(lower_helper_instance(
            &expression("work(n: add(left: caller, right: later))"),
            selected(&body, &[]),
            HelperInstanceLimits {
                max_reserved_names: 1,
                ..LIMITS
            },
            &mut used,
            &mut adapter
        )),
        HelperInstanceError::ReservedNames
    ));
    assert_eq!(adapter.events, ["lower", "argument:n"]);
    assert_eq!(used, 2);
}

#[test]
fn successful_reservations_survive_later_errors_and_unwind_without_retries_or_partial_publication()
{
    let body = work_body();
    for phase in ["copy", "name", "admit"] {
        for unwind in [false, true] {
            let mut adapter = Adapter::new(&body);
            if unwind {
                adapter.unwind = Some(phase);
            } else {
                adapter.fail = Some(phase);
            }
            let mut used = 2;
            let result = catch_unwind(AssertUnwindSafe(|| {
                lower_helper_instance(
                    &expression("work(n: 1)"),
                    selected(&body, &[]),
                    LIMITS,
                    &mut used,
                    &mut adapter,
                )
            }));
            if unwind {
                assert!(result.is_err());
            } else {
                let error = denied(result.unwrap());
                assert!(!format!("{error:?} {error}").contains("secret"));
                let payload = match error {
                    HelperInstanceError::Template(HelperTemplateError::Factory(error))
                    | HelperInstanceError::Native { error, .. } => error.0,
                    _ => panic!("wrong phase"),
                };
                assert_eq!(payload, "secret native payload");
            }
            assert_eq!(used, 6);
            assert_eq!(
                adapter
                    .events
                    .iter()
                    .filter(|event| *event == phase)
                    .count(),
                1
            );
            assert_eq!(adapter.events.last().map(String::as_str), Some(phase));
            assert_eq!(body.template().shape().nodes, 3);
        }
    }
}

#[test]
fn factory_shape_changes_and_same_shape_cold_type_corruption_never_publish_an_instance() {
    let body = work_body();
    for changed_shape in [true, false] {
        let mut adapter = Adapter::new(&body);
        adapter.enlarge_copy = changed_shape;
        adapter.corrupt_copy = !changed_shape;
        let mut used = 2;
        let error = denied(lower_helper_instance(
            &expression("work(n: 1)"),
            selected(&body, &[]),
            LIMITS,
            &mut used,
            &mut adapter,
        ));
        if changed_shape {
            assert!(matches!(
                error,
                HelperInstanceError::Template(HelperTemplateError::ShapeChanged { .. })
            ));
        } else {
            assert!(matches!(
                error,
                HelperInstanceError::Native {
                    phase: HelperInstancePhase::Admit,
                    ..
                }
            ));
        }
        assert_eq!(used, 6);
        assert_eq!(body.template().shape().nodes, 3);
    }
}

#[test]
fn native_operand_and_materialized_effect_buffers_move_once_while_cached_body_stays_owned() {
    let original_drops = Rc::new(Cell::new(0));
    let effect = native("host", &original_drops);
    let original_buffer = effect.buffer.as_ptr();
    let body = prepare(
        vec![("n".into(), ScalarType::Integer)],
        Node::Host {
            effect: Box::new(effect),
        },
        None,
    );
    let mut adapter = Adapter::new(&body);
    let mut used = 4;
    let (output, result) = lower_helper_instance(
        &expression("work(n: field(value: record, name: \"count\"))"),
        selected(&body, &["record"]),
        LIMITS,
        &mut used,
        &mut adapter,
    )
    .unwrap();
    assert!(std::ptr::eq(result, body.template().result_type()));
    let Node::Bind {
        value,
        body: copied,
        ..
    } = &output
    else {
        panic!()
    };
    let Node::Field { field, .. } = value.as_ref() else {
        panic!()
    };
    assert_eq!(field.buffer.as_ptr(), adapter.argument_buffers[0]);
    let Node::Host { effect } = copied.as_ref() else {
        panic!()
    };
    assert_eq!(Some(effect.buffer.as_ptr()), adapter.copied_buffer);
    assert_ne!(effect.buffer.as_ptr(), original_buffer);
    assert_eq!(original_drops.get(), 0);
    drop(output);
    assert_eq!(adapter.drops.get(), 2);
    assert_eq!(original_drops.get(), 0);
    drop(body);
    assert_eq!(original_drops.get(), 1);
}

#[test]
fn rejected_owned_arguments_drop_once_and_do_not_drop_cached_native_slots() {
    let body = work_body();
    for phase in ["depth", "copy", "name", "admit"] {
        let mut adapter = Adapter::new(&body);
        adapter.fail = Some(phase);
        let mut used = 4;
        let error = denied(lower_helper_instance(
            &expression("work(n: field(value: record, name: \"count\"))"),
            selected(&body, &["record"]),
            LIMITS,
            &mut used,
            &mut adapter,
        ));
        assert_eq!(adapter.drops.get(), 1);
        assert_eq!(used, if phase == "depth" { 4 } else { 8 });
        if phase == "depth" {
            assert!(matches!(
                error,
                HelperInstanceError::Expansion(HelperExpansionError::Observation { .. })
            ));
        }
    }
}

#[test]
fn invalid_physical_ceilings_invoke_no_callbacks_or_reservation_updates() {
    let body = work_body();
    for policy in 0..5 {
        let mut limits = LIMITS;
        match policy {
            0 => limits.source.max_source_nodes = 16_385,
            1 => limits.source.max_lowered_depth = 65,
            2 => limits.max_parameters = 9,
            3 => limits.max_bindings = 1_025,
            _ => limits.max_reserved_names = 16_385,
        }
        let mut adapter = Adapter::new(&body);
        let mut used = 2;
        assert!(matches!(
            denied(lower_helper_instance(
                &expression("work(n: 1)"),
                selected(&body, &[]),
                limits,
                &mut used,
                &mut adapter
            )),
            HelperInstanceError::InvalidLimits
        ));
        assert!(adapter.events.is_empty());
        assert_eq!(used, 2);
    }
}

#[test]
fn cached_body_under_current_limits_is_checked_before_any_native_argument_work() {
    let body = work_body();
    for limits in [
        HelperInstanceLimits {
            max_parameters: 0,
            ..LIMITS
        },
        HelperInstanceLimits {
            max_bindings: 0,
            ..LIMITS
        },
        HelperInstanceLimits {
            source: SourceCallLimits {
                max_lowered_nodes: 2,
                ..SOURCE
            },
            ..LIMITS
        },
        HelperInstanceLimits {
            source: SourceCallLimits {
                max_lowered_depth: 0,
                ..SOURCE
            },
            ..LIMITS
        },
    ] {
        let mut adapter = Adapter::new(&body);
        let mut used = 2;
        assert!(matches!(
            denied(lower_helper_instance(
                &expression("work(n: 1)"),
                selected(&body, &[]),
                limits,
                &mut used,
                &mut adapter
            )),
            HelperInstanceError::Template(_)
        ));
        assert!(adapter.events.is_empty());
        assert_eq!(used, 2);
    }
}

#[test]
fn duplicate_parameter_aliases_and_bad_local_names_stop_before_final_admission() {
    let body = prepare(
        vec![
            ("n".into(), ScalarType::Integer),
            ("m".into(), ScalarType::Integer),
        ],
        bind("inner", local("n"), add(local("inner"), local("m"))),
        Some(ScalarType::Integer),
    );
    for aliases in [
        vec!["_same", "_same"],
        vec!["_first", "_second", "bad-name"],
        vec!["_first", "_second", "_second"],
    ] {
        let mut adapter = Adapter::new(&body);
        adapter.aliases = aliases.into_iter().map(str::to_owned).collect();
        let mut used = 3;
        assert!(matches!(
            denied(lower_helper_instance(
                &expression("work(n: 1, m: 2)"),
                selected(&body, &[]),
                LIMITS,
                &mut used,
                &mut adapter
            )),
            HelperInstanceError::Hygiene(_)
        ));
        assert_eq!(used, 3 + body.source_cost().nodes + 2);
        assert!(!adapter.events.iter().any(|event| event == "admit"));
    }
}

#[test]
fn unrelated_host_requires_exact_declared_projection_instead_of_an_unknown_source_fallback() {
    let body = work_body();
    for source in [
        "work(n: unknown())",
        "work(n: unknown_alias)",
        "work(n: field(value: record, name: 7))",
        "work(n: field(value: caller, name: \"count\"))",
        "work(n: field(value: record, name: \"secret\"))",
    ] {
        let mut adapter = Adapter::new(&body);
        let mut used = 2;
        assert!(matches!(
            denied(lower_helper_instance(
                &expression(source),
                selected(&body, &[]),
                LIMITS,
                &mut used,
                &mut adapter
            )),
            HelperInstanceError::Source(_)
        ));
        assert_eq!(adapter.events, ["lower", "argument:n"]);
        assert!(adapter.argument_buffers.is_empty());
        assert_eq!(used, 2);
    }
}
