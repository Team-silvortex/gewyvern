use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::call_evaluation::{CallEvaluationLimits, evaluate_call_arguments_in_scope};
use leselang_hir::ir::{Computation, ComputedArgument};
use leselang_hir::projection_source::{
    ProjectionSourceEnvironment, ProjectionSourceError, lower_projection_source,
};
use leselang_hir::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationFault, PureEvaluationLimits, PureValue,
    evaluate_pure_in_scope,
};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits, infer_pure_type,
};
use leselang_hir::scalar_source::{ScalarSourceError, lower_scalar_source};
use leselang_hir::source_call::{SourceCallError, SourceCallLimits, SourceShapeError};
use leselang_runtime_core::*;
use leselang_syntax::{Expression, Span, parse};

// Native slots, declarations and errors have no Clone/Debug/serde/Send contract.
struct Field {
    index: usize,
    owner: Rc<()>,
    buffer: Vec<u8>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Field {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Operation {
    owner: Rc<()>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Operation {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Effect;
struct IrResult;
struct PrivateError(&'static str);
type Node = Computation<Field, Operation, Effect, IrResult>;

struct Declaration {
    owner: Rc<()>,
}
impl PartialEq for Declaration {
    fn eq(&self, other: &Self) -> bool {
        std::ptr::eq(self, other)
    }
}
struct Host {
    panel: Declaration,
    group: Declaration,
    foreign: Declaration,
    names: [&'static str; 6],
    values: [ScalarValue; 6],
    drops: Rc<Cell<usize>>,
    queries: RefCell<Vec<&'static str>>,
    fail_value: bool,
}
impl Host {
    fn new(names: [&'static str; 6]) -> Self {
        Self {
            panel: Declaration { owner: Rc::new(()) },
            group: Declaration { owner: Rc::new(()) },
            foreign: Declaration { owner: Rc::new(()) },
            names,
            values: [
                ScalarValue::Integer(7),
                ScalarValue::Boolean(true),
                ScalarValue::String("native caption".into()),
                ScalarValue::None,
                ScalarValue::OptionalString(OptionalStringValue(Some("present".into()))),
                ScalarValue::StringList(StringListValue(vec!["a".into(), "b".into()])),
            ],
            drops: Rc::new(Cell::new(0)),
            queries: RefCell::new(Vec::new()),
            fail_value: false,
        }
    }
    fn field(&self, index: usize) -> Field {
        Field {
            index,
            owner: self.panel.owner.clone(),
            buffer: vec![9; 17],
            drops: self.drops.clone(),
        }
    }
    fn operation(&self) -> Operation {
        Operation {
            owner: self.group.owner.clone(),
            drops: self.drops.clone(),
        }
    }
}
const NAMES: [&str; 6] = ["size", "open", "caption", "empty", "hint", "entries"];
const LIMITS: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 64,
    max_source_depth: 16,
    max_lowered_nodes: 64,
    max_lowered_depth: 16,
    max_arguments: 8,
};
const TYPING: TypeInferenceLimits = TypeInferenceLimits {
    max_nodes: 64,
    max_depth: 16,
    max_bindings: 8,
};
const PURE: PureEvaluationLimits = PureEvaluationLimits {
    max_nodes: 64,
    max_depth: 16,
    max_bindings: 8,
};

impl<'host> PureTypeEnvironment<Field, Operation> for &'host Host {
    type Result = &'host Declaration;
    fn field_type(&self, result: &&'host Declaration, field: &Field) -> Option<ScalarType> {
        (std::ptr::eq(*result, &self.panel) && Rc::ptr_eq(&field.owner, &self.panel.owner))
            .then(|| self.values.get(field.index).map(ScalarValue::scalar_type))
            .flatten()
    }
    fn member_result(
        &self,
        group: &&'host Declaration,
        name: &str,
        operation: &Operation,
    ) -> Option<&'host Declaration> {
        (std::ptr::eq(*group, &self.group)
            && name == "ready-step"
            && Rc::ptr_eq(&operation.owner, &self.group.owner))
        .then_some(&self.panel)
    }
}
impl<'host> PureEvaluationEnvironment<Field, Operation> for &'host Host {
    type Result = &'host Declaration;
    type Error = PrivateError;
    fn field(
        &self,
        result: &&'host Declaration,
        field: &Field,
    ) -> Result<ScalarValue, PrivateError> {
        self.queries.borrow_mut().push("field");
        if self.fail_value || self.field_type(result, field).is_none() {
            return Err(PrivateError("private field failure"));
        }
        Ok(self.values[field.index].clone())
    }
    fn member(
        &self,
        group: &&'host Declaration,
        name: &str,
        operation: &Operation,
    ) -> Result<&'host Declaration, PrivateError> {
        self.queries.borrow_mut().push("member");
        self.member_result(group, name, operation)
            .ok_or(PrivateError("private member failure"))
    }
}

#[derive(Clone, Copy)]
enum Input {
    Normal,
    Impure,
    Tracked,
    Scalar,
    Forged,
    WrongResult,
}
struct Adapter<'host> {
    host: &'host Host,
    events: Vec<&'static str>,
    input: Input,
    fail: &'static str,
    panic_field: bool,
    value_address: usize,
    name_address: usize,
    buffer_address: usize,
}
impl<'host> Adapter<'host> {
    fn new(host: &'host Host) -> Self {
        Self {
            host,
            events: vec![],
            input: Input::Normal,
            fail: "",
            panic_field: false,
            value_address: 0,
            name_address: 0,
            buffer_address: 0,
        }
    }
}
impl<'source, 'host> ProjectionSourceEnvironment<'source, Field, Operation, Effect, IrResult>
    for Adapter<'host>
{
    type Result = &'host Declaration;
    type Error = PrivateError;
    fn lower_value(
        &mut self,
        expression: &'source Expression,
    ) -> Result<(Node, PureType<Self::Result>), PrivateError> {
        self.events.push("lower");
        self.value_address = expression as *const Expression as usize;
        if self.fail == "lower" {
            return Err(PrivateError("private lowering failure"));
        }
        let local = || Node::Local {
            name: "reply".into(),
        };
        match self.input {
            Input::Impure => {
                return Ok((
                    Node::Host {
                        effect: Box::new(Effect),
                    },
                    PureType::Result(&self.host.panel),
                ));
            }
            Input::Tracked => {
                return Ok((
                    Node::Bind {
                        name: "scratch".into(),
                        value: Box::new(Node::Field {
                            value: Box::new(local()),
                            field: self.host.field(0),
                        }),
                        body: Box::new(local()),
                    },
                    PureType::Result(&self.host.panel),
                ));
            }
            Input::Scalar => {
                return Ok((
                    Node::Literal {
                        value: ScalarValue::Integer(1),
                    },
                    PureType::Scalar(ScalarType::Integer),
                ));
            }
            Input::Forged => {
                return Ok((
                    Node::Local {
                        name: "missing".into(),
                    },
                    PureType::Result(&self.host.panel),
                ));
            }
            Input::WrongResult => {
                return Ok((
                    Node::Local {
                        name: "foreign".into(),
                    },
                    PureType::Result(&self.host.foreign),
                ));
            }
            Input::Normal => {}
        }
        match expression {
            Expression::Reference { name, .. } if name == "reply" => {
                Ok((local(), PureType::Result(&self.host.panel)))
            }
            Expression::Call { callee, .. } if callee == "member" => {
                lower_projection_source(expression, LIMITS, self)
                    .map_err(|_| PrivateError("private member source rejection"))
            }
            _ => Err(PrivateError("private unknown value")),
        }
    }
    fn field(
        &mut self,
        result: &Self::Result,
        name: &'source str,
    ) -> Result<Option<(Field, ScalarType)>, PrivateError> {
        self.events.push("field");
        self.name_address = name.as_ptr() as usize;
        assert!(self.host.queries.borrow().is_empty());
        if self.panic_field {
            panic!("private field metadata unwind");
        }
        if self.fail == "field" {
            return Err(PrivateError("private export lookup failure"));
        }
        if !std::ptr::eq(*result, &self.host.panel) {
            return Ok(None);
        }
        let Some(index) = self
            .host
            .names
            .iter()
            .position(|candidate| *candidate == name)
        else {
            return Ok(None);
        };
        let field = self.host.field(index);
        self.buffer_address = field.buffer.as_ptr() as usize;
        Ok(Some((field, self.host.values[index].scalar_type())))
    }
    fn member(
        &mut self,
        group: &'source str,
        name: &'source str,
    ) -> Result<Option<(Operation, Self::Result)>, PrivateError> {
        self.events.push("member");
        self.name_address = name.as_ptr() as usize;
        if self.fail == "member" {
            return Err(PrivateError("private member metadata failure"));
        }
        if group != "group" || name != "ready-step" {
            return Ok(None);
        }
        Ok(Some((self.host.operation(), &self.host.panel)))
    }
}
fn expression(source: &str) -> Expression {
    let tree = parse(&format!("fn main() = {source}"));
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    tree.function.unwrap().body
}
fn span(expression: &Expression) -> Span {
    match expression {
        Expression::Call { span, .. }
        | Expression::Reference { span, .. }
        | Expression::Integer { span, .. }
        | Expression::Boolean { span, .. }
        | Expression::None { span }
        | Expression::String { span, .. } => *span,
    }
}
fn execute(
    node: &Node,
    host: &Host,
    fuel: &mut Fuel,
) -> Result<ScalarValue, CalculationFailure<PureEvaluationFault<PrivateError>>> {
    let mut bindings = vec![
        ("reply", PureValue::Result(&host.panel)),
        ("group", PureValue::Result(&host.group)),
    ];
    let mut scope = ScopeFrame::new(&mut bindings);
    let value = evaluate_pure_in_scope(node, &mut scope, &host, fuel, PURE)?;
    assert_eq!(scope.len(), 2);
    assert!(std::ptr::eq(
        match scope.get("reply").unwrap() {
            PureValue::Result(value) => *value,
            _ => unreachable!(),
        },
        &host.panel
    ));
    match value {
        PureValue::Scalar(value) => Ok(value),
        _ => panic!("expected scalar output"),
    }
}

#[test]
fn two_distinct_native_field_schemas_cover_all_six_scalar_types_without_product_fields() {
    for names in [
        NAMES,
        ["total", "visible", "title", "nil", "tooltip", "children"],
    ] {
        let host = Host::new(names);
        for (index, name) in names.iter().enumerate() {
            let source = expression(&format!("field(name: \"{name}\", value: reply)"));
            let mut adapter = Adapter::new(&host);
            let (node, observed) = lower_projection_source(&source, LIMITS, &mut adapter).unwrap();
            assert!(
                matches!(observed, PureType::Scalar(ty) if ty == host.values[index].scalar_type())
            );
            assert!(
                matches!(infer_pure_type(&node, &[("reply", PureType::Result(&host.panel))], &&host, TYPING).unwrap(),
                PureType::Scalar(ty) if ty == host.values[index].scalar_type())
            );
            assert_eq!(
                execute(&node, &host, &mut Fuel::new(100)).unwrap(),
                host.values[index]
            );
            host.queries.borrow_mut().clear();
        }
        assert_eq!(host.drops.get(), 6);
    }
}

#[test]
fn original_value_ast_literal_name_and_move_only_field_buffer_are_preserved() {
    let host = Host::new(NAMES);
    for source in [
        "field(value: reply, name: \"size\")",
        "field(name: \"size\", value: reply)",
    ] {
        let source = expression(source);
        let Expression::Call { arguments, .. } = &source else {
            unreachable!()
        };
        let input = &arguments
            .iter()
            .find(|argument| argument.name == "value")
            .unwrap()
            .value;
        let Expression::String { value: label, .. } = &arguments
            .iter()
            .find(|argument| argument.name == "name")
            .unwrap()
            .value
        else {
            unreachable!()
        };
        let mut adapter = Adapter::new(&host);
        let (node, _) = lower_projection_source(&source, LIMITS, &mut adapter).unwrap();
        assert_eq!(adapter.events, ["lower", "field"]);
        assert_eq!(adapter.value_address, input as *const Expression as usize);
        assert_eq!(adapter.name_address, label.as_ptr() as usize);
        let Node::Field { value, field } = node else {
            unreachable!()
        };
        assert!(matches!(*value, Node::Local { ref name } if name == "reply"));
        assert_eq!(field.buffer.as_ptr() as usize, adapter.buffer_address);
    }
    assert_eq!(host.drops.get(), 2);
}

#[test]
fn member_is_one_original_ir_node_without_value_lowering_or_synthetic_local() {
    let host = Host::new(NAMES);
    let source = expression("member(name: \"ready-step\", value: group)");
    let mut adapter = Adapter::new(&host);
    let (node, ty) = lower_projection_source(
        &source,
        SourceCallLimits {
            max_lowered_nodes: 1,
            max_lowered_depth: 0,
            ..LIMITS
        },
        &mut adapter,
    )
    .unwrap();
    assert_eq!(adapter.events, ["member"]);
    assert!(matches!(ty, PureType::Result(result) if std::ptr::eq(result, &host.panel)));
    assert!(
        matches!(&node, Node::Member { group, name, operation } if group == "group" && name == "ready-step" && Rc::ptr_eq(&operation.owner, &host.group.owner))
    );
    assert!(
        matches!(infer_pure_type(&node, &[("group", PureType::Result(&host.group))], &&host, TYPING).unwrap(), PureType::Result(result) if std::ptr::eq(result, &host.panel))
    );
    drop(node);
    assert_eq!(host.drops.get(), 1);
}

#[test]
fn parsed_field_over_member_runs_exact_native_member_then_field_queries_and_fuel() {
    let host = Host::new(NAMES);
    let source =
        expression("field(value: member(value: group, name: \"ready-step\"), name: \"size\")");
    let mut adapter = Adapter::new(&host);
    let (node, _) = lower_projection_source(
        &source,
        SourceCallLimits {
            max_lowered_nodes: 2,
            max_lowered_depth: 1,
            ..LIMITS
        },
        &mut adapter,
    )
    .unwrap();
    assert_eq!(adapter.events, ["lower", "member", "field"]);
    assert!(matches!(
        infer_pure_type(
            &node,
            &[("group", PureType::Result(&host.group))],
            &&host,
            TYPING
        )
        .unwrap(),
        PureType::Scalar(ScalarType::Integer)
    ));
    let mut fuel = Fuel::new(2);
    assert_eq!(
        execute(&node, &host, &mut fuel).unwrap(),
        ScalarValue::Integer(7)
    );
    assert_eq!(fuel.remaining(), 0);
    assert_eq!(*host.queries.borrow(), ["member", "field"]);
    host.queries.borrow_mut().clear();
    assert!(matches!(
        execute(&node, &host, &mut Fuel::new(1)),
        Err(CalculationFailure::External(
            PureEvaluationFault::FuelExhausted
        ))
    ));
    assert!(host.queries.borrow().is_empty());
}

#[test]
fn all_cold_projection_shapes_and_literals_reject_before_native_callbacks() {
    let host = Host::new(NAMES);
    for source in [
        "field()",
        "field(value: reply)",
        "field(value: reply, name: \"size\", extra: 1)",
        "field(value: reply, value: reply)",
        "field(value: reply, name: computed())",
        "member(value: native(), name: \"ready-step\")",
        "member(value: group, name: 1)",
        "member(value: group, name: \"\")",
        "member(value: group, name: \"bad.name\")",
        "field(value: native(cold: field(value: reply, name: 1)), name: \"size\")",
        "field(value: native(cold: member(value: group, name: 1)), name: \"size\")",
    ] {
        let mut adapter = Adapter::new(&host);
        assert!(
            lower_projection_source(&expression(source), LIMITS, &mut adapter).is_err(),
            "{source}"
        );
        assert!(adapter.events.is_empty(), "{source}");
    }
}

#[test]
fn malformed_ingress_and_explicit_source_limits_reject_before_native_callbacks() {
    let host = Host::new(NAMES);
    let source = expression("field(value: reply, name: \"size\")");
    for limits in [
        SourceCallLimits {
            max_source_nodes: 2,
            ..LIMITS
        },
        SourceCallLimits {
            max_source_depth: 0,
            ..LIMITS
        },
        SourceCallLimits {
            max_arguments: 1,
            ..LIMITS
        },
        SourceCallLimits {
            max_lowered_nodes: 16_385,
            ..LIMITS
        },
        SourceCallLimits {
            max_lowered_depth: 65,
            ..LIMITS
        },
    ] {
        let mut adapter = Adapter::new(&host);
        assert!(lower_projection_source(&source, limits, &mut adapter).is_err());
        assert!(adapter.events.is_empty());
    }
    let mut oversized = source.clone();
    let Expression::Call { arguments, .. } = &mut oversized else {
        unreachable!()
    };
    let Expression::String { value, .. } = &mut arguments[1].value else {
        unreachable!()
    };
    *value = "private".repeat(600);
    let mut adapter = Adapter::new(&host);
    assert!(matches!(
        lower_projection_source(&oversized, LIMITS, &mut adapter),
        Err(ProjectionSourceError::Source(SourceCallError::Shape {
            error: SourceShapeError::UnboundedText,
            ..
        }))
    ));
    assert!(adapter.events.is_empty());
}

#[test]
fn field_and_member_metadata_charge_source_ast_but_not_generated_ir() {
    let host = Host::new(NAMES);
    for (source, nodes, depth) in [
        ("field(value: reply, name: \"size\")", 2, 1),
        ("member(value: group, name: \"ready-step\")", 1, 0),
    ] {
        let source = expression(source);
        let mut adapter = Adapter::new(&host);
        assert!(
            lower_projection_source(
                &source,
                SourceCallLimits {
                    max_source_nodes: 3,
                    max_lowered_nodes: nodes,
                    max_lowered_depth: depth,
                    ..LIMITS
                },
                &mut adapter
            )
            .is_ok()
        );
        for limits in [
            SourceCallLimits {
                max_lowered_nodes: nodes - 1,
                ..LIMITS
            },
            SourceCallLimits {
                max_lowered_nodes: 0,
                ..LIMITS
            },
        ] {
            let mut adapter = Adapter::new(&host);
            assert!(matches!(
                lower_projection_source(&source, limits, &mut adapter),
                Err(ProjectionSourceError::Generation { .. })
            ));
            assert!(adapter.events.is_empty());
        }
    }
    let mut adapter = Adapter::new(&host);
    assert!(matches!(
        lower_projection_source(
            &expression("field(value: reply, name: \"size\")"),
            SourceCallLimits {
                max_lowered_depth: 0,
                ..LIMITS
            },
            &mut adapter
        ),
        Err(ProjectionSourceError::Generation {
            error: StructureError::DepthLimit,
            ..
        })
    ));
    assert!(adapter.events.is_empty());
}

#[test]
fn native_input_ir_budget_and_purity_precede_export_queries_and_drop_partial_nodes() {
    let host = Host::new(NAMES);
    let source = expression("field(value: reply, name: \"size\")");
    let mut adapter = Adapter {
        input: Input::Impure,
        ..Adapter::new(&host)
    };
    assert!(matches!(
        lower_projection_source(&source, LIMITS, &mut adapter),
        Err(ProjectionSourceError::Produced {
            error: PureTypeError::Impure,
            ..
        })
    ));
    assert_eq!(adapter.events, ["lower"]);
    for limits in [
        SourceCallLimits {
            max_lowered_nodes: 4,
            ..LIMITS
        },
        SourceCallLimits {
            max_lowered_depth: 2,
            ..LIMITS
        },
    ] {
        let mut adapter = Adapter {
            input: Input::Tracked,
            ..Adapter::new(&host)
        };
        assert!(matches!(
            lower_projection_source(&source, limits, &mut adapter),
            Err(ProjectionSourceError::Produced {
                error: PureTypeError::Structure(_),
                ..
            })
        ));
        assert_eq!(adapter.events, ["lower"]);
    }
    assert_eq!(host.drops.get(), 2);
}

#[test]
fn scalar_inputs_and_wrong_result_exports_never_gain_native_field_authority() {
    let host = Host::new(NAMES);
    let source = expression("field(value: reply, name: \"size\")");
    let mut adapter = Adapter {
        input: Input::Scalar,
        ..Adapter::new(&host)
    };
    assert!(matches!(
        lower_projection_source(&source, LIMITS, &mut adapter),
        Err(ProjectionSourceError::NonResult { .. })
    ));
    assert_eq!(adapter.events, ["lower"]);
    let mut adapter = Adapter {
        input: Input::WrongResult,
        ..Adapter::new(&host)
    };
    assert!(matches!(
        lower_projection_source(&source, LIMITS, &mut adapter),
        Err(ProjectionSourceError::FieldNotExported { .. })
    ));
    assert_eq!(adapter.events, ["lower", "field"]);
    for source in [
        "field(value: reply, name: \"private\")",
        "member(value: foreign, name: \"ready-step\")",
        "member(value: group, name: \"absent\")",
    ] {
        assert!(
            lower_projection_source(&expression(source), LIMITS, &mut Adapter::new(&host)).is_err()
        );
    }
    assert_eq!(host.drops.get(), 0);
}

#[test]
fn observations_are_not_certificates_and_original_operation_identity_is_cold_checked() {
    let host = Host::new(NAMES);
    let source = expression("field(value: reply, name: \"size\")");
    let mut adapter = Adapter {
        input: Input::Forged,
        ..Adapter::new(&host)
    };
    let (node, _) = lower_projection_source(&source, LIMITS, &mut adapter).unwrap();
    assert!(matches!(
        infer_pure_type(&node, &[], &&host, TYPING),
        Err(PureTypeError::UnknownLocal)
    ));
    let source = expression("member(value: group, name: \"ready-step\")");
    let (mut node, _) = lower_projection_source(&source, LIMITS, &mut Adapter::new(&host)).unwrap();
    assert!(matches!(
        infer_pure_type(
            &node,
            &[("group", PureType::Result(&host.foreign))],
            &&host,
            TYPING
        ),
        Err(PureTypeError::MemberNotExported)
    ));
    let Node::Member { operation, .. } = &mut node else {
        unreachable!()
    };
    operation.owner = Rc::new(());
    assert!(matches!(
        infer_pure_type(
            &node,
            &[("group", PureType::Result(&host.group))],
            &&host,
            TYPING
        ),
        Err(PureTypeError::MemberNotExported)
    ));
    assert!(host.queries.borrow().is_empty());
}

#[test]
fn native_errors_are_payload_free_and_errors_unwind_drop_prepared_input_once() {
    let host = Host::new(NAMES);
    let source = expression("field(value: reply, name: \"size\")");
    for fail in ["lower", "field"] {
        let mut adapter = Adapter {
            input: Input::Tracked,
            fail,
            ..Adapter::new(&host)
        };
        let Err(error) = lower_projection_source(&source, LIMITS, &mut adapter) else {
            panic!("expected native error")
        };
        assert!(!format!("{error:?} {error}").contains("private"));
        assert!(
            matches!(error, ProjectionSourceError::Native { error: PrivateError(message), .. } if message.starts_with("private"))
        );
    }
    assert_eq!(host.drops.get(), 1);
    let mut adapter = Adapter {
        input: Input::Tracked,
        panic_field: true,
        ..Adapter::new(&host)
    };
    assert!(
        catch_unwind(AssertUnwindSafe(|| lower_projection_source(
            &source,
            LIMITS,
            &mut adapter
        )))
        .is_err()
    );
    assert_eq!(host.drops.get(), 2);
    let mut adapter = Adapter {
        fail: "member",
        ..Adapter::new(&host)
    };
    assert!(matches!(
        lower_projection_source(
            &expression("member(value: group, name: \"ready-step\")"),
            LIMITS,
            &mut adapter
        ),
        Err(ProjectionSourceError::Native { .. })
    ));
    assert_eq!(adapter.events, ["member"]);
}

#[test]
fn selected_runtime_native_failure_is_not_calculation_fallback_or_prefix_mutation() {
    let mut host = Host::new(NAMES);
    let (field, _) = lower_projection_source(
        &expression("field(value: reply, name: \"size\")"),
        LIMITS,
        &mut Adapter::new(&host),
    )
    .unwrap();
    let node = Node::Recover {
        value: Box::new(field),
        fallback: Box::new(Node::Literal {
            value: ScalarValue::Integer(9),
        }),
    };
    host.fail_value = true;
    let mut bindings = vec![("reply", PureValue::Result(&host.panel))];
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    assert!(matches!(
        evaluate_pure_in_scope(&node, &mut scope, &&host, &mut fuel, PURE),
        Err(CalculationFailure::External(PureEvaluationFault::Native(
            PrivateError(_)
        )))
    ));
    assert_eq!(scope.len(), 1);
    assert_eq!(*host.queries.borrow(), ["field"]);
    assert!(fuel.remaining() < 100);
}

#[test]
fn shared_scalar_source_extension_uses_native_projection_without_a_mini_field_grammar() {
    let host = Host::new(NAMES);
    let source = expression("add(left: field(value: reply, name: \"size\"), right: 1)");
    let mut adapter = Adapter::new(&host);
    let (node, ty): (Node, _) = lower_scalar_source(&source, LIMITS, |expression| {
        let (node, ty) = lower_projection_source(expression, LIMITS, &mut adapter)
            .map_err(|_| PrivateError("private projection rejection"))?;
        Ok::<_, PrivateError>((node, ty.scalar_type()))
    })
    .unwrap();
    assert_eq!(ty, ScalarType::Integer);
    assert!(matches!(
        infer_pure_type(
            &node,
            &[("reply", PureType::Result(&host.panel))],
            &&host,
            TYPING
        )
        .unwrap(),
        PureType::Scalar(ScalarType::Integer)
    ));
    assert_eq!(
        execute(&node, &host, &mut Fuel::new(100)).unwrap(),
        ScalarValue::Integer(8)
    );
    let bad = expression("field(value: reply, name: 1)");
    let result: Result<(Node, _), ScalarSourceError<PrivateError>> =
        lower_scalar_source(&bad, LIMITS, |expression| {
            lower_projection_source(expression, LIMITS, &mut Adapter::new(&host))
                .map(|(node, ty)| (node, ty.scalar_type()))
                .map_err(|_| PrivateError("private projection rejection"))
        });
    assert!(matches!(result, Err(ScalarSourceError::Native { .. })));
}

struct IntegerDomain;
impl ScalarArgumentDomain for IntegerDomain {
    fn scalar_types(&self) -> ScalarTypeSet {
        ScalarTypeSet::only(ScalarType::Integer)
    }
    fn accepts_literal(&self, value: &ScalarValue) -> bool {
        matches!(value, ScalarValue::Integer(_))
    }
}
#[test]
fn parsed_native_projection_prepares_an_atomic_argument_against_the_original_schema() {
    let host = Host::new(NAMES);
    let (value, _) = lower_projection_source(
        &expression("field(value: reply, name: \"size\")"),
        LIMITS,
        &mut Adapter::new(&host),
    )
    .unwrap();
    let operation = host.operation();
    let parameters = [NamedParameter::required("size", IntegerDomain)];
    let schemas = [OperationSchema {
        key: operation,
        parameters: &parameters,
        result: (),
        required_capability: "panel.write",
    }];
    let arguments: Vec<ComputedArgument<Node>> = vec![ComputedArgument {
        name: "size".into(),
        value,
    }];
    let mut bindings = vec![("reply", PureValue::Result(&host.panel))];
    let mut scope = ScopeFrame::new(&mut bindings);
    let prepared = evaluate_call_arguments_in_scope(
        &arguments,
        &schemas[0],
        &mut scope,
        &&host,
        &mut Fuel::new(100),
        CallEvaluationLimits {
            pure: PURE,
            max_arguments: 1,
        },
    )
    .unwrap();
    assert_eq!(prepared.arguments()[0].value, ScalarValue::Integer(7));
    assert_eq!(scope.len(), 1);
    assert!(std::ptr::eq(prepared.schema(), &schemas[0]));
}

#[test]
fn reference_projection_diagnostics_spans_child_order_wire_and_authority_are_unchanged() {
    for (source, code, message, selected) in [
        (
            "field(value: unknown, name: 1)",
            "LSH1403",
            "undefined local 'unknown'",
            "unknown",
        ),
        (
            "field(value: 1, name: 1)",
            "LSH1409",
            "field name must be a string literal",
            "1)",
        ),
        (
            "field(value: 1, name: \"unknown\")",
            "LSH1409",
            "unknown result field",
            "root",
        ),
        (
            "field(value: 1, name: \"count\")",
            "LSH1409",
            "field is not exported by this bound result type",
            "root",
        ),
        (
            "field(value: 1)",
            "LSH1401",
            "expected exactly the named arguments: value, name",
            "root",
        ),
        (
            "member(value: unknown, name: 1)",
            "LSH1412",
            "member requires a bound group reference and a literal step name",
            "root",
        ),
        (
            "member(value: unknown, name: \"step\")",
            "LSH1412",
            "member is not exported by this bound group",
            "root",
        ),
        (
            "member(value: group)",
            "LSH1401",
            "expected exactly the named arguments: value, name",
            "root",
        ),
    ] {
        let tree = parse(&format!("fn main() = {source}"));
        let body = &tree.function.as_ref().unwrap().body;
        let error = &leselang_hir::lower(&tree).unwrap_err()[0];
        assert_eq!(error.code, code, "{source}");
        assert_eq!(error.message, message, "{source}");
        let expected = if selected == "root" {
            span(body)
        } else {
            let Expression::Call { arguments, .. } = body else {
                unreachable!()
            };
            span(
                &arguments
                    .iter()
                    .find(|argument| {
                        argument.name
                            == if selected == "unknown" {
                                "value"
                            } else {
                                "name"
                            }
                    })
                    .unwrap()
                    .value,
            )
        };
        assert_eq!(error.span, Some(expected), "{source}");
    }
    for source in [
        "fn main() = bind(r: runtime.list(), body: field(name: \"count\", value: r))",
        "fn main() = bind(g: all(first: runtime.list(), second: runtime.list()), body: field(value: member(name: \"first\", value: g), name: \"count\"))",
        "fn id(value: integer) = value\nfn main() = bind(r: runtime.list(), body: id(value: field(value: r, name: \"count\")))",
    ] {
        let program = leselang_hir::lower(&parse(source)).unwrap();
        assert_eq!(program.function.required_capabilities, ["runtime.read"]);
        let wire = serde_json::to_vec(&program).unwrap();
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        assert_eq!(
            serde_json::to_vec(&leselang_hir::lower(&parse(&canonical)).unwrap()).unwrap(),
            wire
        );
    }
}

#[test]
fn source_construction_does_not_require_clone_or_debug_for_result_observations() {
    struct Observation;
    struct Adapter;
    impl<'source> ProjectionSourceEnvironment<'source, (), (), (), ()> for Adapter {
        type Result = Observation;
        type Error = PrivateError;
        fn lower_value(
            &mut self,
            _: &'source Expression,
        ) -> Result<(Computation<(), (), (), ()>, PureType<Observation>), PrivateError> {
            Ok((
                Computation::Local {
                    name: "reply".into(),
                },
                PureType::Result(Observation),
            ))
        }
        fn field(
            &mut self,
            _: &Observation,
            _: &'source str,
        ) -> Result<Option<((), ScalarType)>, PrivateError> {
            Ok(Some(((), ScalarType::Integer)))
        }
        fn member(
            &mut self,
            _: &'source str,
            _: &'source str,
        ) -> Result<Option<((), Observation)>, PrivateError> {
            Ok(Some(((), Observation)))
        }
    }
    assert!(matches!(
        lower_projection_source(
            &expression("field(value: reply, name: \"size\")"),
            LIMITS,
            &mut Adapter
        )
        .unwrap()
        .1,
        PureType::Scalar(ScalarType::Integer)
    ));
    assert!(matches!(
        lower_projection_source(
            &expression("member(value: group, name: \"ready-step\")"),
            LIMITS,
            &mut Adapter
        )
        .unwrap()
        .1,
        PureType::Result(Observation)
    ));
}
