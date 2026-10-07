use std::cell::Cell;
use std::convert::Infallible;
use std::error::Error;
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::call_evaluation::{CallEvaluationLimits, evaluate_call_arguments_in_scope};
use leselang_hir::call_typing::{CallTypeLimits, check_call_arguments};
use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::{PureEvaluationEnvironment, PureEvaluationLimits};
use leselang_hir::pure_typing::{PureTypeEnvironment, TypeInferenceLimits};
use leselang_hir::source_call::*;
use leselang_runtime_core::*;
use leselang_syntax::{Expression, NamedArgument, parse};

const SOURCE: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 64,
    max_source_depth: 8,
    max_lowered_nodes: 64,
    max_lowered_depth: 8,
    max_arguments: 3,
};
const FINISH: SourceCallFinishLimits = SourceCallFinishLimits {
    max_nodes: 64,
    max_depth: 8,
    max_arguments: 3,
};
struct Key(&'static str);
impl std::borrow::Borrow<str> for Key {
    fn borrow(&self) -> &str {
        self.0
    }
}
impl PartialEq for Key {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}
struct Domain(Cell<ScalarType>);
impl ScalarArgumentDomain for Domain {
    fn scalar_types(&self) -> ScalarTypeSet {
        ScalarTypeSet::only(self.0.get())
    }
    fn accepts_literal(&self, _: &ScalarValue) -> bool {
        true
    }
}
struct Tracked {
    bytes: Box<[u8]>,
    drops: Rc<Cell<usize>>,
}
impl Tracked {
    fn new(drops: &Rc<Cell<usize>>) -> Self {
        Self {
            bytes: vec![7; 17].into_boxed_slice(),
            drops: drops.clone(),
        }
    }
}
impl Drop for Tracked {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct Opcode {
    row: &'static str,
    owned: Tracked,
}
struct HostEffect;
struct IrResult;
struct Observation {
    row: &'static str,
    owned: Tracked,
}
struct PrivateError(&'static str);
type Node = Computation<Tracked, Opcode, HostEffect, IrResult>;
type Schema<'a> = SourceSchema<'a, Key, Domain, &'static str, u8>;
type Prepared<'source, 'schema> =
    LoweredSourceCall<'source, 'schema, Key, Domain, &'static str, u8, Node>;

fn expression(source: &str) -> Expression {
    let tree = parse(source);
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    tree.function.unwrap().body
}
fn parameters(ty: ScalarType) -> [NamedParameter<&'static str, Domain>; 3] {
    [
        NamedParameter::required("a", Domain(Cell::new(ty))),
        NamedParameter::required("b", Domain(Cell::new(ty))),
        NamedParameter::optional("c", Domain(Cell::new(ty))),
    ]
}
fn schema<'a>(parameters: &'a [NamedParameter<&'a str, Domain>]) -> Schema<'a> {
    SourceSchema {
        key: Key("device.move"),
        parameters,
        result: "native-reply",
        required_capability: 3,
    }
}
fn prepare<'source, 'schema>(
    source: &'source Expression,
    schema: &'schema Schema<'schema>,
    lower: impl FnMut(&NamedArgument) -> Result<(Node, Option<ScalarType>), PrivateError>,
) -> Prepared<'source, 'schema> {
    let catalog = OperationCatalog::new(
        7,
        std::slice::from_ref(schema),
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 3,
        },
    )
    .unwrap();
    lower_source_call(
        source,
        &SourceCallHost {
            catalog: &catalog,
            version: 7,
            granted: &[3],
        },
        SOURCE,
        lower,
    )
    .unwrap()
}
fn tracked_operand(drops: &Rc<Cell<usize>>) -> (Node, Option<ScalarType>) {
    (
        Node::Field {
            value: Box::new(Node::Local {
                name: "saved".into(),
            }),
            field: Tracked::new(drops),
        },
        Some(ScalarType::Integer),
    )
}
fn mapping(
    row: &'static str,
    operation_drops: &Rc<Cell<usize>>,
    output_drops: &Rc<Cell<usize>>,
) -> (Opcode, Observation) {
    (
        Opcode {
            row,
            owned: Tracked::new(operation_drops),
        },
        Observation {
            row,
            owned: Tracked::new(output_drops),
        },
    )
}
struct Environment;
impl PureTypeEnvironment<Tracked, Opcode> for Environment {
    type Result = ();
    fn field_type(&self, _: &(), _: &Tracked) -> Option<ScalarType> {
        None
    }
    fn member_result(&self, _: &(), _: &str, _: &Opcode) -> Option<()> {
        None
    }
}
impl PureEvaluationEnvironment<Tracked, Opcode> for Environment {
    type Result = ();
    type Error = PrivateError;
    fn field(&self, _: &(), _: &Tracked) -> Result<ScalarValue, PrivateError> {
        Err(PrivateError("private field"))
    }
    fn member(&self, _: &(), _: &str, _: &Opcode) -> Result<(), PrivateError> {
        Err(PrivateError("private member"))
    }
}
fn admit_types(call: &Node, schema: &Schema<'_>) -> Result<(), PrivateError> {
    let Node::Call { arguments, .. } = call else {
        return Err(PrivateError("not a call"));
    };
    check_call_arguments(
        arguments,
        schema,
        &[],
        &Environment,
        CallTypeLimits {
            pure: TypeInferenceLimits {
                max_nodes: 64,
                max_depth: 8,
                max_bindings: 0,
            },
            max_arguments: 3,
        },
    )
    .map(|_| ())
    .map_err(|_| PrivateError("private type failure"))
}

#[test]
fn exact_original_operands_schema_and_move_only_outputs_reach_once_only_admission() {
    let source = expression("fn main() = device.move(b: 2, a: 1)");
    let parameters = parameters(ScalarType::Integer);
    let schema = schema(&parameters);
    let fields = Rc::new(Cell::new(0));
    let operations = Rc::new(Cell::new(0));
    let outputs = Rc::new(Cell::new(0));
    let charges = Cell::new(0);
    let prepared = prepare(&source, &schema, |_| {
        charges.set(charges.get() + 1);
        Ok(tracked_operand(&fields))
    });
    let pointers = prepared
        .arguments()
        .iter()
        .map(|argument| match &argument.value {
            Node::Field { value, field } => (&**value as *const Node, field.bytes.as_ptr()),
            _ => panic!("expected field"),
        })
        .collect::<Vec<_>>();
    let mapped = mapping(schema.key.0, &operations, &outputs);
    let operation_pointer = mapped.0.owned.bytes.as_ptr();
    let output_pointer = mapped.1.owned.bytes.as_ptr();
    let maps = Cell::new(0);
    let admissions = Cell::new(0);
    let (call, output) = prepared
        .finish_call(
            FINISH,
            |original| {
                assert!(std::ptr::eq(original, &schema));
                maps.set(maps.get() + 1);
                Ok::<_, PrivateError>(mapped)
            },
            |call, output, original| {
                admissions.set(admissions.get() + 1);
                assert!(std::ptr::eq(original, &schema));
                assert_eq!(output.owned.bytes.as_ptr(), output_pointer);
                let Node::Call {
                    operation,
                    arguments,
                } = call
                else {
                    panic!()
                };
                assert_eq!(operation.owned.bytes.as_ptr(), operation_pointer);
                assert_eq!(operation.row, original.key.0);
                assert_eq!(output.row, original.key.0);
                assert_eq!(
                    arguments
                        .iter()
                        .map(|a| a.name.as_str())
                        .collect::<Vec<_>>(),
                    ["a", "b"]
                );
                for (argument, pointers) in arguments.iter().zip(&pointers) {
                    let Node::Field { value, field } = &argument.value else {
                        panic!()
                    };
                    assert_eq!(&**value as *const Node, pointers.0);
                    assert_eq!(field.bytes.as_ptr(), pointers.1);
                }
                Ok::<_, PrivateError>(())
            },
        )
        .unwrap();
    assert_eq!((maps.get(), admissions.get(), charges.get()), (1, 1, 2));
    assert_eq!((fields.get(), operations.get(), outputs.get()), (0, 0, 0));
    drop((call, output));
    assert_eq!((fields.get(), operations.get(), outputs.get()), (2, 1, 1));
}

#[test]
fn every_safety_ceiling_precedes_native_mapping_and_admission() {
    for limits in [
        SourceCallFinishLimits {
            max_nodes: 16_385,
            ..FINISH
        },
        SourceCallFinishLimits {
            max_depth: 65,
            ..FINISH
        },
        SourceCallFinishLimits {
            max_arguments: 65,
            ..FINISH
        },
    ] {
        let source = expression("fn main() = device.move()");
        let schema = schema(&[]);
        let prepared = prepare(&source, &schema, |_| panic!("no operands"));
        let result = prepared.finish_call(
            limits,
            |_| -> Result<(Opcode, ()), Infallible> { panic!("mapping") },
            |_, _, _| -> Result<(), Infallible> { panic!("admission") },
        );
        assert!(matches!(result, Err(SourceCallFinishError::InvalidLimits)));
    }
}

#[test]
fn current_schema_parameter_limit_includes_unsubmitted_optional_parameters() {
    let source = expression("fn main() = device.move(b: 2, a: 1)");
    let parameters = parameters(ScalarType::Integer);
    let schema = schema(&parameters);
    let prepared = prepare(&source, &schema, |argument| {
        leselang_hir::scalar_source::lower_scalar_source(&argument.value, SOURCE, |_| {
            Err(PrivateError("unsupported"))
        })
        .map(|(node, ty)| (node, Some(ty)))
        .map_err(|_| PrivateError("lowering"))
    });
    let result = prepared.finish_call(
        SourceCallFinishLimits {
            max_arguments: 2,
            ..FINISH
        },
        |_| -> Result<(Opcode, ()), Infallible> { panic!("mapping") },
        |_, _, _| -> Result<(), Infallible> { panic!("admission") },
    );
    assert!(matches!(result, Err(SourceCallFinishError::ParameterLimit)));
}

#[test]
fn complete_cold_forest_uses_one_current_root_node_and_depth_budget() {
    for (nodes, depth, accepts) in [(0, 8, false), (4, 8, false), (5, 1, false), (5, 2, true)] {
        let source = expression("fn main() = device.move(b: 2, a: 1)");
        let parameters = parameters(ScalarType::Integer);
        let schema = schema(&parameters);
        let fields = Rc::new(Cell::new(0));
        let operations = Rc::new(Cell::new(0));
        let outputs = Rc::new(Cell::new(0));
        let prepared = prepare(&source, &schema, |_| Ok(tracked_operand(&fields)));
        let maps = Cell::new(0);
        let admissions = Cell::new(0);
        let result = prepared.finish_call(
            SourceCallFinishLimits {
                max_nodes: nodes,
                max_depth: depth,
                ..FINISH
            },
            |original| {
                maps.set(maps.get() + 1);
                Ok::<_, Infallible>(mapping(original.key.0, &operations, &outputs))
            },
            |_, _, _| {
                admissions.set(admissions.get() + 1);
                Ok::<_, Infallible>(())
            },
        );
        assert_eq!(result.is_ok(), accepts);
        assert_eq!(
            (maps.get(), admissions.get()),
            if accepts { (1, 1) } else { (0, 0) }
        );
        if nodes == 5 && depth == 1 {
            assert!(matches!(
                &result,
                Err(SourceCallFinishError::Output {
                    argument_index: 1,
                    ..
                })
            ));
        }
        drop(result);
        assert_eq!(fields.get(), 2);
    }
}

#[test]
fn parameterless_call_requires_one_root_but_allows_zero_depth_and_arguments() {
    for nodes in [0, 1] {
        let source = expression("fn main() = device.move()");
        let schema = schema(&[]);
        let operations = Rc::new(Cell::new(0));
        let outputs = Rc::new(Cell::new(0));
        let prepared = prepare(&source, &schema, |_| panic!("no operands"));
        let result = prepared.finish_call(
            SourceCallFinishLimits {
                max_nodes: nodes,
                max_depth: 0,
                max_arguments: 0,
            },
            |original| Ok::<_, Infallible>(mapping(original.key.0, &operations, &outputs)),
            |call, _, _| {
                assert_eq!(call.children().count(), 0);
                Ok::<_, Infallible>(())
            },
        );
        assert_eq!(result.is_ok(), nodes == 1);
    }
}

#[test]
fn mapping_failure_releases_original_operands_without_admission_retry_or_refund() {
    let source = expression("fn main() = device.move(b: 2, a: 1)");
    let parameters = parameters(ScalarType::Integer);
    let schema = schema(&parameters);
    let fields = Rc::new(Cell::new(0));
    let charge = Cell::new(0);
    let prepared = prepare(&source, &schema, |_| {
        charge.set(charge.get() + 1);
        Ok(tracked_operand(&fields))
    });
    let result = prepared.finish_call(
        FINISH,
        |_| Err::<(Opcode, ()), _>(PrivateError("private mapping payload")),
        |_, _, _| -> Result<(), PrivateError> { panic!("admission") },
    );
    assert_eq!(fields.get(), 2);
    assert_eq!(charge.get(), 2);
    let Err(error) = result else { panic!() };
    assert!(!format!("{error:?}: {error}").contains("payload"));
    assert!(error.source().is_none());
    let SourceCallFinishError::Mapping(native) = error else {
        panic!()
    };
    assert_eq!(native.0, "private mapping payload");
}

#[test]
fn admission_failure_releases_whole_call_and_result_without_partial_handoff() {
    let source = expression("fn main() = device.move(b: 2, a: 1)");
    let parameters = parameters(ScalarType::Integer);
    let schema = schema(&parameters);
    let fields = Rc::new(Cell::new(0));
    let operations = Rc::new(Cell::new(0));
    let outputs = Rc::new(Cell::new(0));
    let admissions = Cell::new(0);
    let prepared = prepare(&source, &schema, |_| Ok(tracked_operand(&fields)));
    let result = prepared.finish_call(
        FINISH,
        |original| Ok::<_, PrivateError>(mapping(original.key.0, &operations, &outputs)),
        |_, _, _| {
            admissions.set(admissions.get() + 1);
            Err(PrivateError("private admission payload"))
        },
    );
    assert_eq!(admissions.get(), 1);
    assert_eq!((fields.get(), operations.get(), outputs.get()), (2, 1, 1));
    let Err(error) = result else { panic!() };
    assert!(!format!("{error:?}: {error}").contains("payload"));
    assert!(error.source().is_none());
    let SourceCallFinishError::Admission(native) = error else {
        panic!()
    };
    assert_eq!(native.0, "private admission payload");
}

#[test]
fn native_mapping_unwind_releases_prepared_operands_and_captured_native_state() {
    let source = expression("fn main() = device.move(b: 2, a: 1)");
    let parameters = parameters(ScalarType::Integer);
    let schema = schema(&parameters);
    let fields = Rc::new(Cell::new(0));
    let operations = Rc::new(Cell::new(0));
    let outputs = Rc::new(Cell::new(0));
    let prepared = prepare(&source, &schema, |_| Ok(tracked_operand(&fields)));
    let owned = mapping(schema.key.0, &operations, &outputs);
    assert!(
        catch_unwind(AssertUnwindSafe(|| prepared.finish_call(
            FINISH,
            |_| -> Result<(Opcode, Observation), PrivateError> {
                let _owned = owned;
                panic!("mapping unwind")
            },
            |_, _, _| -> Result<(), PrivateError> { panic!("admission") },
        )))
        .is_err()
    );
    assert_eq!((fields.get(), operations.get(), outputs.get()), (2, 1, 1));
}

#[test]
fn native_admission_unwind_releases_whole_owned_output_without_refunding_external_work() {
    let source = expression("fn main() = device.move(b: 2, a: 1)");
    let parameters = parameters(ScalarType::Integer);
    let schema = schema(&parameters);
    let fields = Rc::new(Cell::new(0));
    let operations = Rc::new(Cell::new(0));
    let outputs = Rc::new(Cell::new(0));
    let charge = Cell::new(0);
    let prepared = prepare(&source, &schema, |_| {
        charge.set(charge.get() + 1);
        Ok(tracked_operand(&fields))
    });
    assert!(
        catch_unwind(AssertUnwindSafe(|| prepared.finish_call(
            FINISH,
            |original| Ok::<_, PrivateError>(mapping(original.key.0, &operations, &outputs)),
            |_, _, _| -> Result<(), PrivateError> {
                charge.set(charge.get() + 1);
                panic!("admission unwind")
            },
        )))
        .is_err()
    );
    assert_eq!(charge.get(), 3);
    assert_eq!((fields.get(), operations.get(), outputs.get()), (2, 1, 1));
}

#[test]
fn fresh_lexical_admission_rejects_forged_prepared_scalar_facts() {
    let source = expression("fn main() = device.move(b: 2, a: 1)");
    let parameters = parameters(ScalarType::Integer);
    let schema = schema(&parameters);
    let fields = Rc::new(Cell::new(0));
    let operations = Rc::new(Cell::new(0));
    let outputs = Rc::new(Cell::new(0));
    let prepared = prepare(&source, &schema, |_| Ok(tracked_operand(&fields)));
    let result = prepared.finish_call(
        FINISH,
        |original| Ok::<_, Infallible>(mapping(original.key.0, &operations, &outputs)),
        |call, _, original| admit_types(call, original),
    );
    assert!(matches!(result, Err(SourceCallFinishError::Admission(_))));
    assert_eq!((fields.get(), operations.get(), outputs.get()), (2, 1, 1));
}

#[test]
fn same_shape_foreign_operation_or_result_is_rejected_by_explicit_native_row_admission() {
    for wrong_operation in [true, false] {
        let source = expression("fn main() = device.move()");
        let schema = schema(&[]);
        let operations = Rc::new(Cell::new(0));
        let outputs = Rc::new(Cell::new(0));
        let prepared = prepare(&source, &schema, |_| panic!("no operands"));
        let result = prepared.finish_call(
            FINISH,
            |original| {
                let (mut operation, mut output) = mapping(original.key.0, &operations, &outputs);
                if wrong_operation {
                    operation.row = "foreign";
                } else {
                    output.row = "foreign";
                }
                Ok::<_, Infallible>((operation, output))
            },
            |call, output, original| {
                let Node::Call { operation, .. } = call else {
                    panic!()
                };
                if operation.row == original.key.0 && output.row == original.key.0 {
                    Ok(())
                } else {
                    Err(PrivateError("private foreign row"))
                }
            },
        );
        assert!(matches!(result, Err(SourceCallFinishError::Admission(_))));
        assert_eq!((operations.get(), outputs.get()), (1, 1));
    }
}

#[test]
fn original_schema_borrow_does_not_freeze_live_native_domains_version_or_grants() {
    for changed in 0..3 {
        let source = expression("fn main() = device.move(b: 2, a: 1)");
        let parameters = parameters(ScalarType::Integer);
        let schema = schema(&parameters);
        let operations = Rc::new(Cell::new(0));
        let outputs = Rc::new(Cell::new(0));
        let version = Cell::new(7);
        let granted = Cell::new(true);
        let prepared = prepare(&source, &schema, |argument| {
            leselang_hir::scalar_source::lower_scalar_source(&argument.value, SOURCE, |_| {
                Err(PrivateError("unsupported"))
            })
            .map(|(node, ty)| (node, Some(ty)))
            .map_err(|_| PrivateError("lowering"))
        });
        match changed {
            0 => parameters[1].domain.0.set(ScalarType::String),
            1 => version.set(8),
            _ => granted.set(false),
        }
        let result = prepared.finish_call(
            FINISH,
            |original| Ok::<_, Infallible>(mapping(original.key.0, &operations, &outputs)),
            |call, _, original| {
                if version.get() != 7 || !granted.get() {
                    return Err(PrivateError("private live policy"));
                }
                admit_types(call, original)
            },
        );
        assert!(matches!(result, Err(SourceCallFinishError::Admission(_))));
    }
}

#[test]
fn unrelated_numeric_and_panel_hosts_complete_original_calls_and_prepare_actual_values() {
    for (name, ty, source, expected) in [
        (
            "device.move",
            ScalarType::Integer,
            "fn main() = device.move(b: 6, a: add(left: 1, right: 2))",
            vec![ScalarValue::Integer(3), ScalarValue::Integer(6)],
        ),
        (
            "panel.label",
            ScalarType::String,
            "fn main() = panel.label(b: \"ok\", a: concat(left: \"he\", right: \"llo\"))",
            vec![
                ScalarValue::String("hello".into()),
                ScalarValue::String("ok".into()),
            ],
        ),
    ] {
        let source = expression(source);
        let parameters = parameters(ty);
        let mut schema = schema(&parameters);
        schema.key = Key(name);
        let operations = Rc::new(Cell::new(0));
        let outputs = Rc::new(Cell::new(0));
        let prepared = prepare(&source, &schema, |argument| {
            leselang_hir::scalar_source::lower_scalar_source(&argument.value, SOURCE, |_| {
                Err(PrivateError("unsupported"))
            })
            .map(|(node, ty)| (node, Some(ty)))
            .map_err(|_| PrivateError("lowering"))
        });
        let (call, output) = prepared
            .finish_call(
                FINISH,
                |original| Ok::<_, Infallible>(mapping(original.key.0, &operations, &outputs)),
                |call, output, original| {
                    let Node::Call { operation, .. } = call else {
                        panic!()
                    };
                    assert_eq!(operation.row, original.key.0);
                    assert_eq!(output.row, original.key.0);
                    admit_types(call, original)
                },
            )
            .unwrap();
        let Node::Call {
            operation,
            arguments,
        } = &call
        else {
            panic!()
        };
        assert_eq!(operation.row, name);
        {
            let mut values = Vec::new();
            let mut scope = ScopeFrame::new(&mut values);
            let mut fuel = Fuel::new(100);
            let values = evaluate_call_arguments_in_scope(
                arguments,
                &schema,
                &mut scope,
                &Environment,
                &mut fuel,
                CallEvaluationLimits {
                    pure: PureEvaluationLimits {
                        max_nodes: 64,
                        max_depth: 8,
                        max_bindings: 0,
                    },
                    max_arguments: 3,
                },
            )
            .unwrap();
            assert!(std::ptr::eq(values.schema(), &schema));
            assert_eq!(
                values
                    .arguments()
                    .iter()
                    .map(|a| a.value.clone())
                    .collect::<Vec<_>>(),
                expected
            );
            assert_eq!(
                fuel.remaining(),
                if ty == ScalarType::String { 89 } else { 96 }
            );
        }
        drop((call, output));
        assert_eq!((operations.get(), outputs.get()), (1, 1));
    }
}

#[test]
fn reference_computed_call_keeps_full_width_group_scope_and_canonical_wire_policy() {
    for width in [63, 64] {
        let branches = (0..width)
            .map(|index| format!("b{index}: ui.assert_text(node_id: \"status\", expected: \"ok\")"))
            .collect::<Vec<_>>()
            .join(", ");
        let last = width - 1;
        let source = format!(
            "fn main() = bind(group: seq({branches}), body: ui.focus(node_id: field(value: member(value: group, name: \"b{last}\"), name: \"expected\")))"
        );
        let result = leselang_hir::lower(&parse(&source));
        if width == 64 {
            // Full native scope typing must succeed before the separate 65-effect
            // chain limit rejects the additional successor, not report LSH1405.
            assert!(
                result
                    .unwrap_err()
                    .iter()
                    .all(|diagnostic| diagnostic.code == "LSH1412")
            );
            continue;
        }
        let program = result.unwrap();
        assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        let restored = leselang_hir::lower(&parse(&canonical)).unwrap();
        assert_eq!(
            serde_json::to_vec(&program).unwrap(),
            serde_json::to_vec(&restored).unwrap()
        );
        assert!(
            leselang_hir::authorize(&program, &leselang_host_contract::CapabilitySet::default())
                .is_err()
        );
    }
}

#[test]
fn reference_computed_call_does_not_double_charge_bounded_type_prefix_as_expression_depth() {
    let mut source = "ui.focus(node_id: concat(left: \"a\", right: \"b\"))".to_owned();
    for index in 0..12 {
        source = format!("bind(s{index}: \"prefix\", body: {source})");
    }
    let program = leselang_hir::lower(&parse(&format!("fn main() = {source}"))).unwrap();
    let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
    let restored = leselang_hir::lower(&parse(&canonical)).unwrap();
    assert_eq!(
        serde_json::to_vec(&program).unwrap(),
        serde_json::to_vec(&restored).unwrap()
    );
}
