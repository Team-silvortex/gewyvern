use std::cell::{Cell, RefCell};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::rc::Rc;

use leselang_hir::choice_source::{
    ChoiceSourceError, ChoiceSourcePhase, ChoiceSourceResult, lower_choice_source,
};
use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::pure_typing::PureTypeError;
use leselang_hir::source_call::{SourceCallError, SourceCallLimits};
use leselang_runtime_core::{ScalarValue, ScopeFrame, StructureError};
use leselang_syntax::{Expression, parse};

struct Token {
    key: u8,
    bytes: Vec<u8>,
    drops: Rc<Cell<usize>>,
}
impl Drop for Token {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}
struct PrivateError(&'static str);
type Node = Computation<Token, Token, Token, Token>;
const LIMITS: SourceCallLimits = SourceCallLimits {
    max_source_nodes: 256,
    max_source_depth: 32,
    max_lowered_nodes: 256,
    max_lowered_depth: 32,
    max_arguments: 64,
};

fn expression(source: &str) -> Expression {
    let parsed = parse(&format!("fn main() = {source}"));
    assert!(parsed.diagnostics.is_empty(), "{:?}", parsed.diagnostics);
    parsed.function.unwrap().body
}
fn source() -> Expression {
    expression("choose(when: true, then: device.read(), otherwise: device.write())")
}
fn literal(value: u64) -> Node {
    Node::Literal {
        value: ScalarValue::Integer(value),
    }
}
fn token(key: u8, drops: &Rc<Cell<usize>>) -> Token {
    Token {
        key,
        bytes: vec![key; 33],
        drops: drops.clone(),
    }
}
struct Adapter<'source> {
    source: &'source Expression,
    events: Vec<ChoiceSourcePhase>,
    drops: Rc<Cell<usize>>,
    nodes: [Option<Node>; 3],
    keys: [u8; 3],
    buffers: [*const u8; 3],
    boolean: bool,
    fail: Option<ChoiceSourcePhase>,
    unwind: Option<ChoiceSourcePhase>,
    scope: Vec<(&'source str, u8)>,
}
impl<'source> Adapter<'source> {
    fn new(source: &'source Expression) -> RefCell<Self> {
        RefCell::new(Self {
            source,
            events: Vec::new(),
            drops: Rc::new(Cell::new(0)),
            nodes: [None, None, None],
            keys: [0, 1, 1],
            buffers: [std::ptr::null(); 3],
            boolean: true,
            fail: None,
            unwind: None,
            scope: Vec::new(),
        })
    }
    fn step(&mut self, phase: ChoiceSourcePhase) -> Result<(), PrivateError> {
        self.events.push(phase);
        let mut scope = ScopeFrame::new(&mut self.scope);
        let mut local = scope.nested();
        local.push("scratch", 7).unwrap();
        if self.unwind == Some(phase) {
            panic!("private native unwind");
        }
        if self.fail == Some(phase) {
            return Err(PrivateError("secret credential"));
        }
        Ok(())
    }
}
fn lower(
    adapter: &RefCell<Adapter<'_>>,
    limits: SourceCallLimits,
) -> ChoiceSourceResult<Node, Token, PrivateError> {
    let input = adapter.borrow().source;
    lower_choice_source(
        input,
        limits,
        |phase, child| {
            let mut adapter = adapter.borrow_mut();
            adapter.step(phase)?;
            let (index, name) = match phase {
                ChoiceSourcePhase::When => (0, "when"),
                ChoiceSourcePhase::Then => (1, "then"),
                ChoiceSourcePhase::Otherwise => (2, "otherwise"),
                _ => panic!(),
            };
            let Expression::Call { arguments, .. } = input else {
                panic!()
            };
            let original = &arguments
                .iter()
                .find(|value| value.name == name)
                .unwrap()
                .value;
            assert!(std::ptr::eq(child, original));
            let node = adapter.nodes[index].take().unwrap_or_else(|| {
                if index == 0 {
                    Node::Literal {
                        value: ScalarValue::Boolean(true),
                    }
                } else {
                    Node::Host {
                        effect: Box::new(token(9, &adapter.drops)),
                    }
                }
            });
            let ty = token(adapter.keys[index], &adapter.drops);
            adapter.buffers[index] = ty.bytes.as_ptr();
            Ok((node, ty))
        },
        |_, ty| {
            let mut adapter = adapter.borrow_mut();
            adapter.step(ChoiceSourcePhase::Condition)?;
            assert_eq!(ty.bytes.as_ptr(), adapter.buffers[0]);
            Ok(adapter.boolean && ty.key == 0)
        },
        |left, right| {
            let mut adapter = adapter.borrow_mut();
            adapter.step(ChoiceSourcePhase::Compare)?;
            assert_eq!(left.bytes.as_ptr(), adapter.buffers[1]);
            assert_eq!(right.bytes.as_ptr(), adapter.buffers[2]);
            Ok(left.key == right.key)
        },
    )
}
fn no_hooks(input: &Expression, limits: SourceCallLimits) {
    let adapter = Adapter::new(input);
    assert!(lower(&adapter, limits).is_err());
    assert!(adapter.borrow().events.is_empty());
}

#[test]
fn original_source_order_and_move_only_type_buffers_are_preserved() {
    for text in [
        "choose(when: true, then: device.read(), otherwise: device.write())",
        "choose(otherwise: device.write(), then: device.read(), when: false)",
    ] {
        let input = expression(text);
        let adapter = Adapter::new(&input);
        let (output, ty) = lower(&adapter, LIMITS).unwrap();
        assert_eq!(
            adapter.borrow().events,
            [
                ChoiceSourcePhase::When,
                ChoiceSourcePhase::Condition,
                ChoiceSourcePhase::Then,
                ChoiceSourcePhase::Otherwise,
                ChoiceSourcePhase::Compare
            ]
        );
        assert_eq!(ty.bytes.as_ptr(), adapter.borrow().buffers[1]);
        assert_eq!(adapter.borrow().drops.get(), 2);
        assert!(matches!(output, Node::Choose { .. }));
        assert!(adapter.borrow().scope.is_empty());
        drop((output, ty));
        assert_eq!(adapter.borrow().drops.get(), 5);
    }
}

#[test]
fn all_four_native_slots_and_original_boxes_and_vector_buffers_move_unchanged() {
    let input = source();
    let adapter = Adapter::new(&input);
    let drops = adapter.borrow().drops.clone();
    let field = token(1, &drops);
    let field_ptr = field.bytes.as_ptr();
    let field_child = Box::new(literal(1));
    let child_ptr = field_child.as_ref() as *const Node;
    let operation = token(2, &drops);
    let operation_ptr = operation.bytes.as_ptr();
    let result = token(3, &drops);
    let result_ptr = result.bytes.as_ptr();
    let effect = Box::new(token(4, &drops));
    let effect_ptr = effect.as_ref() as *const Token;
    let arguments = vec![ComputedArgument {
        name: "value".into(),
        value: Node::Field {
            value: field_child,
            field,
        },
    }];
    let arguments_ptr = arguments.as_ptr();
    let branches = vec![
        ComputedBranch {
            name: "first".into(),
            value: Node::Call {
                operation,
                arguments,
            },
            result_type: result,
        },
        ComputedBranch {
            name: "second".into(),
            value: Node::Host { effect },
            result_type: token(5, &drops),
        },
    ];
    let branches_ptr = branches.as_ptr();
    adapter.borrow_mut().nodes[1] = Some(Node::Group {
        group_kind: GroupKind::Parallel,
        branches,
    });
    let (output, ty) = lower(&adapter, LIMITS).unwrap();
    let Node::Choose { then, .. } = &output else {
        panic!()
    };
    let Node::Group { branches, .. } = then.as_ref() else {
        panic!()
    };
    assert_eq!(branches.as_ptr(), branches_ptr);
    assert_eq!(branches[0].result_type.bytes.as_ptr(), result_ptr);
    let Node::Call {
        operation,
        arguments,
    } = &branches[0].value
    else {
        panic!()
    };
    assert_eq!(operation.bytes.as_ptr(), operation_ptr);
    assert_eq!(arguments.as_ptr(), arguments_ptr);
    let Node::Field { value, field } = &arguments[0].value else {
        panic!()
    };
    assert_eq!(field.bytes.as_ptr(), field_ptr);
    assert_eq!(value.as_ref() as *const Node, child_ptr);
    let Node::Host { effect } = &branches[1].value else {
        panic!()
    };
    assert_eq!(effect.as_ref() as *const Token, effect_ptr);
    drop((output, ty));
    assert_eq!(drops.get(), 9);
}

#[test]
fn invalid_safety_ceilings_precede_all_hooks() {
    let input = source();
    for limits in [
        SourceCallLimits {
            max_source_nodes: 16_385,
            ..LIMITS
        },
        SourceCallLimits {
            max_lowered_nodes: 16_385,
            ..LIMITS
        },
        SourceCallLimits {
            max_source_depth: 65,
            ..LIMITS
        },
        SourceCallLimits {
            max_lowered_depth: 65,
            ..LIMITS
        },
        SourceCallLimits {
            max_arguments: 65,
            ..LIMITS
        },
    ] {
        let adapter = Adapter::new(&input);
        assert!(matches!(
            lower(&adapter, limits),
            Err(ChoiceSourceError::Source(SourceCallError::InvalidLimits))
        ));
        assert!(adapter.borrow().events.is_empty());
    }
}

#[test]
fn zero_and_minimum_output_budgets_stop_before_hooks() {
    let input = source();
    for nodes in 0..4 {
        no_hooks(
            &input,
            SourceCallLimits {
                max_lowered_nodes: nodes,
                ..LIMITS
            },
        );
    }
    no_hooks(
        &input,
        SourceCallLimits {
            max_lowered_depth: 0,
            ..LIMITS
        },
    );
    let adapter = Adapter::new(&input);
    assert!(
        lower(
            &adapter,
            SourceCallLimits {
                max_lowered_nodes: 4,
                max_lowered_depth: 1,
                ..LIMITS
            }
        )
        .is_ok()
    );
}

#[test]
fn whole_cold_source_nodes_depth_and_argument_limits_precede_hooks() {
    let input = expression("choose(when: true, then: 1, otherwise: device.read(value: 2))");
    for limits in [
        SourceCallLimits {
            max_source_nodes: 4,
            ..LIMITS
        },
        SourceCallLimits {
            max_source_depth: 1,
            ..LIMITS
        },
        SourceCallLimits {
            max_arguments: 2,
            ..LIMITS
        },
    ] {
        no_hooks(&input, limits);
    }
}

#[test]
fn hidden_cold_text_and_names_precede_hooks() {
    let text = format!(
        "choose(when: true, then: 1, otherwise: device.read(value: \"{}\"))",
        "x".repeat(4097)
    );
    no_hooks(&expression(&text), LIMITS);
    let mut input = source();
    let Expression::Call { arguments, .. } = &mut input else {
        panic!()
    };
    let Expression::Call { callee, .. } = &mut arguments[2].value else {
        panic!()
    };
    *callee = "bad-credential".into();
    no_hooks(&input, LIMITS);
}

#[test]
fn every_nested_choose_signature_is_checked_before_hooks() {
    for cold in [
        "choose(when: true, then: 1)",
        "choose(when: true, then: 1, otherwise: 2, extra: 3)",
    ] {
        no_hooks(
            &expression(&format!("choose(when: true, then: 1, otherwise: {cold})")),
            LIMITS,
        );
    }
}

#[test]
fn root_shape_required_unique_names_and_wrong_callee_fail_closed() {
    for text in [
        "1",
        "device.read()",
        "choose(when: true, then: 1)",
        "choose(when: true, then: 1, then: 2)",
        "choose(when: true, then: 1, wrong: 2)",
    ] {
        no_hooks(&expression(text), LIMITS);
    }
}

#[test]
fn literal_truth_never_prunes_cold_branch_compilation() {
    for truth in [false, true] {
        let input = source();
        let adapter = Adapter::new(&input);
        adapter.borrow_mut().nodes[0] = Some(Node::Literal {
            value: ScalarValue::Boolean(truth),
        });
        lower(&adapter, LIMITS).unwrap();
        assert_eq!(adapter.borrow().events.len(), 5);
    }
}

#[test]
fn impure_when_is_rejected_before_boolean_or_branch_hooks() {
    let input = source();
    let adapter = Adapter::new(&input);
    let drops = adapter.borrow().drops.clone();
    adapter.borrow_mut().nodes[0] = Some(Node::Host {
        effect: Box::new(token(8, &drops)),
    });
    assert!(matches!(
        lower(&adapter, LIMITS),
        Err(ChoiceSourceError::ProducedWhen {
            error: PureTypeError::Impure,
            ..
        })
    ));
    assert_eq!(adapter.borrow().events, [ChoiceSourcePhase::When]);
}

#[test]
fn zero_limit_loop_cannot_hide_a_when_effect() {
    let input = source();
    let adapter = Adapter::new(&input);
    let drops = adapter.borrow().drops.clone();
    adapter.borrow_mut().nodes[0] = Some(Node::Loop {
        name: "state".into(),
        initial: Box::new(literal(0)),
        condition: Box::new(Node::Literal {
            value: ScalarValue::Boolean(false),
        }),
        next: Box::new(Node::Call {
            operation: token(2, &drops),
            arguments: Vec::new(),
        }),
        limit: 0,
    });
    assert!(matches!(
        lower(&adapter, LIMITS),
        Err(ChoiceSourceError::ProducedWhen {
            error: PureTypeError::Impure,
            ..
        })
    ));
    assert_eq!(adapter.borrow().events.len(), 1);
}

#[test]
fn forged_boolean_metadata_cannot_accept_an_integer_literal() {
    let input = source();
    let adapter = Adapter::new(&input);
    adapter.borrow_mut().nodes[0] = Some(literal(1));
    assert!(matches!(
        lower(&adapter, LIMITS),
        Err(ChoiceSourceError::Condition { .. })
    ));
    assert_eq!(adapter.borrow().events, [ChoiceSourcePhase::When]);
    assert_eq!(adapter.borrow().drops.get(), 1);
}

#[test]
fn unbounded_when_literals_and_invalid_local_names_stop_before_type_hooks() {
    let input = source();
    for node in [
        Node::Literal {
            value: ScalarValue::String("x".repeat(4097)),
        },
        Node::Local {
            name: "bad-name".into(),
        },
    ] {
        let adapter = Adapter::new(&input);
        adapter.borrow_mut().nodes[0] = Some(node);
        assert!(matches!(
            lower(&adapter, LIMITS),
            Err(ChoiceSourceError::ProducedWhen { .. })
        ));
        assert_eq!(adapter.borrow().events.len(), 1);
    }
}

#[test]
fn explicit_boolean_query_rejects_before_branch_lowering() {
    let input = source();
    let adapter = Adapter::new(&input);
    adapter.borrow_mut().nodes[0] = Some(Node::Local {
        name: "flag".into(),
    });
    adapter.borrow_mut().boolean = false;
    assert!(matches!(
        lower(&adapter, LIMITS),
        Err(ChoiceSourceError::Condition { .. })
    ));
    assert_eq!(
        adapter.borrow().events,
        [ChoiceSourcePhase::When, ChoiceSourcePhase::Condition]
    );
}

#[test]
fn explicit_comparison_does_not_union_or_default_native_types() {
    let input = source();
    let adapter = Adapter::new(&input);
    adapter.borrow_mut().keys[2] = 2;
    assert!(matches!(
        lower(&adapter, LIMITS),
        Err(ChoiceSourceError::BranchTypes { .. })
    ));
    assert_eq!(adapter.borrow().events.len(), 5);
    assert_eq!(adapter.borrow().drops.get(), 5);
}

#[test]
fn aggregate_when_budget_reserves_both_future_branch_roots() {
    let input = source();
    let adapter = Adapter::new(&input);
    adapter.borrow_mut().nodes[0] = Some(Node::Unary {
        operator: leselang_runtime_core::UnaryOperator::Not,
        value: Box::new(Node::Literal {
            value: ScalarValue::Boolean(false),
        }),
    });
    assert!(matches!(
        lower(
            &adapter,
            SourceCallLimits {
                max_lowered_nodes: 4,
                ..LIMITS
            }
        ),
        Err(ChoiceSourceError::Output {
            phase: ChoiceSourcePhase::When,
            ..
        })
    ));
    assert_eq!(adapter.borrow().events.len(), 1);
}

#[test]
fn aggregate_then_budget_reserves_otherwise_before_its_compiler() {
    let input = source();
    let adapter = Adapter::new(&input);
    let drops = adapter.borrow().drops.clone();
    adapter.borrow_mut().nodes[1] = Some(Node::Field {
        value: Box::new(literal(1)),
        field: token(3, &drops),
    });
    assert!(matches!(
        lower(
            &adapter,
            SourceCallLimits {
                max_lowered_nodes: 4,
                ..LIMITS
            }
        ),
        Err(ChoiceSourceError::Output {
            phase: ChoiceSourcePhase::Then,
            ..
        })
    ));
    assert_eq!(adapter.borrow().events.len(), 3);
}

#[test]
fn aggregate_otherwise_budget_rejects_before_comparison() {
    let input = source();
    let adapter = Adapter::new(&input);
    adapter.borrow_mut().nodes[2] = Some(Node::Strings {
        items: vec![literal(1)],
    });
    assert!(matches!(
        lower(
            &adapter,
            SourceCallLimits {
                max_lowered_nodes: 4,
                ..LIMITS
            }
        ),
        Err(ChoiceSourceError::Output {
            phase: ChoiceSourcePhase::Otherwise,
            ..
        })
    ));
    assert_eq!(adapter.borrow().events.len(), 4);
}

#[test]
fn shifted_output_depth_is_checked_in_every_child() {
    let input = source();
    for index in 0..3 {
        let adapter = Adapter::new(&input);
        adapter.borrow_mut().nodes[index] = Some(Node::Strings {
            items: vec![literal(1)],
        });
        let error = lower(
            &adapter,
            SourceCallLimits {
                max_lowered_depth: 1,
                ..LIMITS
            },
        )
        .err()
        .unwrap();
        assert!(matches!(
            error,
            ChoiceSourceError::ProducedWhen {
                error: PureTypeError::Structure(StructureError::DepthLimit),
                ..
            } | ChoiceSourceError::Output {
                error: StructureError::DepthLimit,
                ..
            }
        ));
        assert_eq!(adapter.borrow().events.len(), [1, 3, 4][index]);
    }
}

#[test]
fn every_native_failure_stops_later_hooks_redacts_payload_and_drops_once() {
    let input = source();
    for (phase, visits, drops) in [
        (ChoiceSourcePhase::When, 1, 0),
        (ChoiceSourcePhase::Condition, 2, 1),
        (ChoiceSourcePhase::Then, 3, 1),
        (ChoiceSourcePhase::Otherwise, 4, 3),
        (ChoiceSourcePhase::Compare, 5, 5),
    ] {
        let adapter = Adapter::new(&input);
        adapter.borrow_mut().fail = Some(phase);
        let error = lower(&adapter, LIMITS).err().unwrap();
        assert!(!format!("{error:?} {error}").contains("secret credential"));
        assert!(std::error::Error::source(&error).is_none());
        let ChoiceSourceError::Native {
            phase: actual,
            span,
            error: PrivateError(payload),
        } = error
        else {
            panic!()
        };
        assert_eq!(actual, phase);
        assert_eq!(payload, "secret credential");
        let Expression::Call {
            arguments,
            span: root,
            ..
        } = &input
        else {
            panic!()
        };
        let expected = match phase {
            ChoiceSourcePhase::When | ChoiceSourcePhase::Condition => arguments[0].span,
            ChoiceSourcePhase::Then => arguments[1].span,
            ChoiceSourcePhase::Otherwise => arguments[2].span,
            ChoiceSourcePhase::Compare => *root,
        };
        assert!(span.start >= expected.start && span.end <= expected.end);
        assert_eq!(adapter.borrow().events.len(), visits);
        assert_eq!(adapter.borrow().drops.get(), drops);
        assert!(adapter.borrow().scope.is_empty());
    }
}

#[test]
fn every_native_unwind_releases_owned_parts_and_guarded_frames_without_retry() {
    let input = source();
    for (phase, visits, drops) in [
        (ChoiceSourcePhase::When, 1, 0),
        (ChoiceSourcePhase::Condition, 2, 1),
        (ChoiceSourcePhase::Then, 3, 1),
        (ChoiceSourcePhase::Otherwise, 4, 3),
        (ChoiceSourcePhase::Compare, 5, 5),
    ] {
        let adapter = Adapter::new(&input);
        adapter.borrow_mut().unwind = Some(phase);
        assert!(catch_unwind(AssertUnwindSafe(|| lower(&adapter, LIMITS))).is_err());
        assert_eq!(adapter.borrow().events.len(), visits);
        assert_eq!(adapter.borrow().drops.get(), drops);
        assert!(adapter.borrow().scope.is_empty());
    }
    assert!(
        lower(&Adapter::new(&input), LIMITS).is_ok(),
        "borrowed source remains reusable"
    );
}

#[test]
fn reference_scalar_native_group_and_helper_choices_keep_canonical_wire_and_authority() {
    for text in [
        "fn main() = choose(when: true, then: 1, otherwise: 2)",
        "fn main() = choose(when: true, then: runtime.list(), otherwise: runtime.list())",
        "fn main() = choose(when: false, then: seq(rows: runtime.list()), otherwise: seq(rows: runtime.list()))",
        "fn pick(flag: boolean) = choose(when: flag, then: runtime.list(), otherwise: runtime.list()) fn main() = pick(flag: true)",
    ] {
        let program = leselang_hir::lower(&parse(text)).unwrap();
        let canonical = leselang_hir::canonical_source(&program.function.effect).unwrap();
        assert_eq!(program, leselang_hir::lower(&parse(&canonical)).unwrap());
        let wire = serde_json::to_string(&program).unwrap();
        let decoded = serde_json::from_str(&wire).unwrap();
        assert_eq!(program, decoded);
        assert!(wire.contains("\"choose\""));
        let grants = leselang_host_contract::CapabilitySet::new(["runtime.read"]);
        assert!(leselang_hir::authorize(&program, &grants).is_ok());
        if text.contains("runtime.list()") {
            assert!(
                leselang_hir::authorize(
                    &program,
                    &leselang_host_contract::CapabilitySet::default()
                )
                .is_err()
            );
        }
    }
}

#[test]
fn reference_cold_unknown_type_and_impure_conditions_keep_diagnostics() {
    for (text, code) in [
        (
            "fn main() = choose(when: true, then: runtime.list(), otherwise: unknown.host())",
            "LSH1003",
        ),
        (
            "fn main() = choose(when: true, then: runtime.list(), otherwise: 1)",
            "LSH1404",
        ),
        (
            "fn main() = choose(when: runtime.list(), then: 1, otherwise: 2)",
            "LSH1402",
        ),
        (
            "fn main() = choose(when: 1, then: 1, otherwise: 2)",
            "LSH1402",
        ),
    ] {
        let errors = leselang_hir::lower(&parse(text)).unwrap_err();
        assert!(errors.iter().any(|error| error.code == code), "{errors:?}");
    }
}

mod counter {
    use super::*;
    use leselang_hir::call_evaluation::{
        CallEvaluationLimits, PreparedCall, evaluate_call_arguments_in_scope,
    };
    use leselang_hir::effect_evaluation::{
        EffectEvaluationEnvironment, EffectEvaluationLimits, EffectEvaluationOutcome,
        evaluate_effects_in_scope,
    };
    use leselang_hir::pure_evaluation::{
        PureEvaluationEnvironment, PureEvaluationLimits, PureValue,
    };
    use leselang_hir::scalar_source::{ScalarSourceLimits, lower_scalar_source_with_scope};
    use leselang_hir::source_call::{SourceCallHost, SourceSchema, lower_source_call};
    use leselang_runtime_core::*;

    struct Declaration {
        maximum: u64,
    }
    impl HostResultDomain<ScalarValue> for Declaration {
        type Error = PrivateError;
        fn matches_type(&self, value: &ScalarValue) -> bool {
            matches!(value, ScalarValue::Integer(_))
        }
        fn validate_value(&self, value: &ScalarValue) -> Result<(), PrivateError> {
            match value {
                ScalarValue::Integer(value) if *value <= self.maximum => Ok(()),
                _ => Err(PrivateError("private reply domain")),
            }
        }
    }
    type Ir = Computation<(), &'static str, (), ()>;
    type Schema<'a> = SourceSchema<'a, &'static str, ScalarTypeSet, Declaration, u8>;
    enum Observation<'a> {
        Boolean,
        Native(&'a Schema<'a>),
    }
    const PURE: PureEvaluationLimits = PureEvaluationLimits {
        max_nodes: 256,
        max_depth: 32,
        max_bindings: 8,
    };
    const EFFECT: EffectEvaluationLimits = EffectEvaluationLimits {
        pure: PURE,
        max_arguments: 64,
        max_branches: 8,
    };

    fn compile<'schema>(
        input: &Expression,
        host: &SourceCallHost<'_, 'schema, &'static str, ScalarTypeSet, Declaration, u8>,
    ) -> ChoiceSourceResult<Ir, Observation<'schema>, PrivateError> {
        lower_choice_source(
            input,
            LIMITS,
            |phase, source| {
                if phase == ChoiceSourcePhase::When {
                    let (node, ty) = lower_scalar_source_with_scope(
                        source,
                        ScalarSourceLimits {
                            source: LIMITS,
                            max_bindings: 8,
                        },
                        &[],
                        |_, _| -> Result<(Ir, Option<ScalarType>), PrivateError> {
                            Err(PrivateError("unknown scalar alias"))
                        },
                    )
                    .map_err(|_| PrivateError("invalid scalar source"))?;
                    if ty != ScalarType::Boolean {
                        return Err(PrivateError("not boolean"));
                    }
                    Ok((node, Observation::Boolean))
                } else {
                    let call = lower_source_call(source, host, LIMITS, |argument| {
                        lower_scalar_source_with_scope(
                            &argument.value,
                            ScalarSourceLimits {
                                source: LIMITS,
                                max_bindings: 8,
                            },
                            &[],
                            |_, _| -> Result<(Ir, Option<ScalarType>), PrivateError> {
                                Err(PrivateError("unknown scalar alias"))
                            },
                        )
                        .map(|(node, ty)| (node, Some(ty)))
                    })
                    .map_err(|_| PrivateError("native schema rejected"))?;
                    let schema = call.schema();
                    Ok((
                        Ir::Call {
                            operation: schema.key,
                            arguments: call.into_arguments(),
                        },
                        Observation::Native(schema),
                    ))
                }
            },
            |_, ty| Ok(matches!(ty, Observation::Boolean)),
            |left, right| {
                // The catalog declares one integer result interface for both operations.
                Ok(matches!(
                    (left, right),
                    (Observation::Native(_), Observation::Native(_))
                ))
            },
        )
    }
    struct Environment<'a> {
        catalog: &'a OperationCatalog<'a, &'static str, &'a str, ScalarTypeSet, Declaration, u8>,
        version: u32,
        grants: &'a [u8],
        cold: RefCell<Vec<&'static str>>,
        selected: RefCell<Vec<&'static str>>,
    }
    impl PureEvaluationEnvironment<(), &'static str> for Environment<'_> {
        type Result = ();
        type Error = PrivateError;
        fn field(&self, _: &(), _: &()) -> Result<ScalarValue, PrivateError> {
            Err(PrivateError("no fields"))
        }
        fn member(&self, _: &(), _: &str, _: &&'static str) -> Result<(), PrivateError> {
            Err(PrivateError("no groups"))
        }
    }
    impl<'expression, 'schema> EffectEvaluationEnvironment<'expression, (), &'static str, (), ()>
        for Environment<'schema>
    {
        type Request = PreparedCall<'schema, &'static str, ScalarTypeSet, Declaration, u8>;
        type Capture = ();
        fn preflight_effect(&self, expression: &'expression Ir) -> Result<(), PrivateError> {
            let Ir::Call {
                operation,
                arguments,
            } = expression
            else {
                return Err(PrivateError("not atomic"));
            };
            self.cold.borrow_mut().push(operation);
            let schema = self
                .catalog
                .authorize(operation, self.version, self.grants)
                .map_err(|_| PrivateError("live schema rejected"))?;
            schema
                .bind_arguments(
                    &arguments
                        .iter()
                        .map(|argument| argument.name.as_str())
                        .collect::<Vec<_>>(),
                )
                .map_err(|_| PrivateError("names rejected"))?;
            Ok(())
        }
        fn prepare_effect(
            &self,
            expression: &'expression Ir,
            scope: &mut ScopeFrame<'_, 'expression, PureValue<()>>,
            fuel: &mut Fuel,
        ) -> Result<Self::Request, CalculationFailure<PrivateError>> {
            let Ir::Call {
                operation,
                arguments,
            } = expression
            else {
                return Err(PrivateError("not atomic").into());
            };
            self.selected.borrow_mut().push(operation);
            let schema = self
                .catalog
                .authorize(operation, self.version, self.grants)
                .map_err(|_| PrivateError("live schema rejected"))?;
            evaluate_call_arguments_in_scope(
                arguments,
                schema,
                scope,
                self,
                fuel,
                CallEvaluationLimits {
                    pure: PURE,
                    max_arguments: 64,
                },
            )
            .map_err(|_| PrivateError("call argument rejected").into())
        }
        fn capture(
            &self,
            _: &'expression str,
            _: &'expression Ir,
            _: &ScopeFrame<'_, 'expression, PureValue<()>>,
            _: &mut Fuel,
        ) -> Result<(), PrivateError> {
            Err(PrivateError("counter proof has no suspension"))
        }
    }

    #[test]
    fn parsed_native_choice_checks_both_schemas_but_prepares_only_selected_call_with_exact_fuel() {
        let parameters = [NamedParameter::required(
            "value",
            ScalarTypeSet::only(ScalarType::Integer),
        )];
        let schemas = [
            OperationSchema {
                key: "counter.left",
                parameters: &parameters,
                required_capability: 31,
                result: Declaration { maximum: 100 },
            },
            OperationSchema {
                key: "counter.right",
                parameters: &parameters,
                required_capability: 32,
                result: Declaration { maximum: 100 },
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
        for truth in [false, true] {
            let input = expression(&format!(
                "choose(when: {truth}, then: counter.left(value: 11), otherwise: counter.right(value: add(left: 40, right: 2)))"
            ));
            let host = SourceCallHost {
                catalog: &catalog,
                version: 7,
                granted: &[31, 32],
            };
            let (output, observation) = compile(&input, &host).unwrap();
            let Observation::Native(declared) = observation else {
                panic!()
            };
            assert!(
                std::ptr::eq(declared, &schemas[0]),
                "exact Then observation, not the runtime selection"
            );
            let environment = Environment {
                catalog: &catalog,
                version: 7,
                grants: &[31, 32],
                cold: RefCell::new(Vec::new()),
                selected: RefCell::new(Vec::new()),
            };
            let mut values = Vec::new();
            let mut scope = ScopeFrame::new(&mut values);
            let mut fuel = Fuel::new(100);
            let outcome =
                evaluate_effects_in_scope(&output, &mut scope, &environment, &mut fuel, EFFECT)
                    .unwrap();
            let EffectEvaluationOutcome::Request(request) = outcome else {
                panic!()
            };
            let index = usize::from(!truth);
            assert!(std::ptr::eq(request.schema(), &schemas[index]));
            assert_eq!(
                *environment.cold.borrow(),
                ["counter.left", "counter.right"]
            );
            assert_eq!(*environment.selected.borrow(), [schemas[index].key]);
            assert_eq!(
                request.arguments()[0].value,
                ScalarValue::Integer(if truth { 11 } else { 42 })
            );
            let Ir::Choose {
                then, otherwise, ..
            } = &output
            else {
                panic!()
            };
            let selected = if truth { then } else { otherwise };
            let Ir::Call { arguments, .. } = selected.as_ref() else {
                panic!()
            };
            let mut direct_fuel = Fuel::new(100);
            let direct = evaluate_call_arguments_in_scope(
                arguments,
                &schemas[index],
                &mut scope,
                &environment,
                &mut direct_fuel,
                CallEvaluationLimits {
                    pure: PURE,
                    max_arguments: 64,
                },
            )
            .unwrap();
            assert!(std::ptr::eq(direct.schema(), request.schema()));
            assert_eq!(
                direct_fuel.remaining() - fuel.remaining(),
                3,
                "Choose, When and Call roots each cost one"
            );
            request
                .schema()
                .check_result(&ScalarValue::Integer(42))
                .unwrap();
            assert!(
                request
                    .schema()
                    .check_result(&ScalarValue::Boolean(true))
                    .is_err()
            );
            assert!(
                request
                    .schema()
                    .check_result(&ScalarValue::Integer(101))
                    .is_err()
            );
            assert!(scope.is_empty());
        }
    }

    #[test]
    fn native_choice_never_confers_future_version_or_cold_branch_grants() {
        let schemas = [
            OperationSchema {
                key: "counter.left",
                parameters: &[],
                required_capability: 31,
                result: Declaration { maximum: 100 },
            },
            OperationSchema {
                key: "counter.right",
                parameters: &[],
                required_capability: 32,
                result: Declaration { maximum: 100 },
            },
        ];
        let catalog = OperationCatalog::new(
            7,
            &schemas,
            OperationCatalogLimits {
                max_operations: 2,
                max_parameters_per_operation: 0,
            },
        )
        .unwrap();
        let input =
            expression("choose(when: true, then: counter.left(), otherwise: counter.right())");
        for (version, granted, phase) in [
            (8, &[31, 32][..], ChoiceSourcePhase::Then),
            (7, &[31][..], ChoiceSourcePhase::Otherwise),
        ] {
            assert!(
                matches!(compile(&input, &SourceCallHost { catalog: &catalog, version, granted }),
                Err(ChoiceSourceError::Native { phase: actual, .. }) if actual == phase)
            );
        }
        let (output, _) = compile(
            &input,
            &SourceCallHost {
                catalog: &catalog,
                version: 7,
                granted: &[31, 32],
            },
        )
        .unwrap();
        for (version, grants) in [(8, &[31, 32][..]), (7, &[31][..])] {
            let environment = Environment {
                catalog: &catalog,
                version,
                grants,
                cold: RefCell::new(Vec::new()),
                selected: RefCell::new(Vec::new()),
            };
            let mut values = Vec::new();
            let mut scope = ScopeFrame::new(&mut values);
            let mut fuel = Fuel::new(100);
            assert!(
                evaluate_effects_in_scope(&output, &mut scope, &environment, &mut fuel, EFFECT)
                    .is_err()
            );
            assert!(environment.selected.borrow().is_empty());
            assert_eq!(fuel.remaining(), 100);
        }
    }
}
