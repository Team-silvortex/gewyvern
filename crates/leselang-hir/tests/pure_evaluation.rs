use std::cell::Cell;
use std::panic::{AssertUnwindSafe, catch_unwind};

use leselang_hir::ir::Computation;
use leselang_hir::pure_evaluation::{
    PureEvaluationEnvironment, PureEvaluationFailure, PureEvaluationFault, PureEvaluationLimits,
    PureValue, evaluate_pure_in_scope,
};
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits, infer_pure_type,
};
use leselang_runtime_core::{
    BinaryOperator, CalculationFailure, Fuel, HostResultDomain, LoopError, MAX_SCALAR_STRING_BYTES,
    ScalarError, ScalarType, ScalarValue, ScopeFrame, UnaryOperator, validate_host_result,
};

// Native slots intentionally have no Clone, Debug, serde or Send requirement.
struct Field(&'static str);
struct Operation;
struct OpaqueEffect;
struct NativeType;
type Ir = Computation<Field, Operation, OpaqueEffect, NativeType>;

const LIMITS: PureEvaluationLimits = PureEvaluationLimits {
    max_nodes: 256,
    max_depth: 32,
    max_bindings: 16,
};

struct NativeError(String);
struct Editor {
    calls: Cell<usize>,
}
struct Caption(String);
impl PureEvaluationEnvironment<Field, Operation> for Editor {
    type Result = std::rc::Rc<Caption>;
    type Error = NativeError;

    fn field(&self, result: &Self::Result, field: &Field) -> Result<ScalarValue, NativeError> {
        self.calls.set(self.calls.get() + 1);
        match field.0 {
            "caption" => Ok(ScalarValue::String(result.0.clone())),
            "panic" => panic!("native unwind"),
            _ => Err(NativeError("private native payload".into())),
        }
    }

    fn member(
        &self,
        _: &Self::Result,
        _: &str,
        _: &Operation,
    ) -> Result<Self::Result, NativeError> {
        self.calls.set(self.calls.get() + 1);
        Err(NativeError("not an editor group".into()))
    }
}

struct CaptionDomain;
impl HostResultDomain<Caption> for CaptionDomain {
    type Error = ();
    fn matches_type(&self, _: &Caption) -> bool {
        true
    }
    fn validate_value(&self, reply: &Caption) -> Result<(), ()> {
        (reply.0.len() <= 64).then_some(()).ok_or(())
    }
}

struct EditorTypes;
impl PureTypeEnvironment<Field, Operation> for EditorTypes {
    type Result = ();
    fn field_type(&self, _: &(), field: &Field) -> Option<ScalarType> {
        (field.0 == "caption").then_some(ScalarType::String)
    }
    fn member_result(&self, _: &(), _: &str, _: &Operation) -> Option<()> {
        None
    }
}

fn literal(value: ScalarValue) -> Ir {
    Ir::Literal { value }
}
fn integer(value: u64) -> Ir {
    literal(ScalarValue::Integer(value))
}
fn text(value: &str) -> Ir {
    literal(ScalarValue::String(value.into()))
}
fn local(name: &str) -> Ir {
    Ir::Local { name: name.into() }
}
fn field(name: &'static str) -> Ir {
    Ir::Field {
        value: Box::new(local("reply")),
        field: Field(name),
    }
}
fn binary(operator: BinaryOperator, left: Ir, right: Ir) -> Ir {
    Ir::Binary {
        operator,
        left: Box::new(left),
        right: Box::new(right),
    }
}
fn unary(operator: UnaryOperator, value: Ir) -> Ir {
    Ir::Unary {
        operator,
        value: Box::new(value),
    }
}
fn recover(value: Ir, fallback: Ir) -> Ir {
    Ir::Recover {
        value: Box::new(value),
        fallback: Box::new(fallback),
    }
}
fn editor() -> Editor {
    Editor {
        calls: Cell::new(0),
    }
}

fn run(
    expression: &Ir,
    fuel: &mut Fuel,
) -> Result<PureValue<std::rc::Rc<Caption>>, PureEvaluationFailure<NativeError>> {
    let mut bindings = Vec::new();
    let mut scope = ScopeFrame::new(&mut bindings);
    evaluate_pure_in_scope(expression, &mut scope, &editor(), fuel, LIMITS)
}
fn assert_scalar(value: PureValue<std::rc::Rc<Caption>>, expected: ScalarValue) {
    assert!(matches!(value, PureValue::Scalar(value) if value == expected));
}

#[test]
fn typed_editor_ir_consumes_a_validated_actual_reply_without_product_vm() {
    let expression = binary(BinaryOperator::Concat, field("caption"), text("!"));
    let type_limits = TypeInferenceLimits {
        max_nodes: 256,
        max_depth: 32,
        max_bindings: 16,
    };
    assert_eq!(
        infer_pure_type(
            &expression,
            &[("reply", PureType::Result(()))],
            &EditorTypes,
            type_limits
        ),
        Ok(PureType::Scalar(ScalarType::String))
    );
    let reply = std::rc::Rc::new(Caption("ready".into()));
    validate_host_result(&CaptionDomain, reply.as_ref()).unwrap();
    let original = reply.as_ref() as *const Caption;
    let mut bindings = vec![("reply", PureValue::Result(reply.clone()))];
    let mut scope = ScopeFrame::new(&mut bindings);
    let host = editor();
    let mut fuel = Fuel::new(100);
    assert_scalar(
        evaluate_pure_in_scope(&expression, &mut scope, &host, &mut fuel, LIMITS).unwrap(),
        ScalarValue::String("ready!".into()),
    );
    assert_eq!(host.calls.get(), 1);
    assert_eq!(std::rc::Rc::strong_count(&reply), 2);
    let PureValue::Result(saved) = scope.get("reply").unwrap() else {
        panic!()
    };
    assert_eq!(saved.as_ref() as *const Caption, original);
    assert_eq!(fuel.remaining(), 92);
}

struct Device<'a>(&'a [u64]);
impl<'a> PureEvaluationEnvironment<u8, u32> for Device<'a> {
    type Result = &'a u64;
    type Error = u16;
    fn field(&self, result: &Self::Result, field: &u8) -> Result<ScalarValue, u16> {
        if *field != 2 {
            return Err(7);
        }
        Ok(ScalarValue::Integer(**result))
    }
    fn member(
        &self,
        group: &Self::Result,
        name: &str,
        operation: &u32,
    ) -> Result<Self::Result, u16> {
        if !std::ptr::eq(*group, &self.0[0]) || name != "position" || *operation != 41 {
            return Err(9);
        }
        Ok(&self.0[1])
    }
}

#[test]
fn unrelated_borrowed_device_group_enforces_exact_member_operation_and_identity() {
    let expression: Computation<u8, u32, (), ()> = Computation::Binary {
        operator: BinaryOperator::Add,
        left: Box::new(Computation::Field {
            value: Box::new(Computation::Member {
                group: "reply".into(),
                name: "position".into(),
                operation: 41,
            }),
            field: 2,
        }),
        right: Box::new(Computation::Literal {
            value: ScalarValue::Integer(1),
        }),
    };
    let forged = Computation::<u8, u32, (), ()>::Member {
        group: "reply".into(),
        name: "position".into(),
        operation: 42,
    };
    let values = [0, 42];
    let host = Device(&values);
    let mut bindings = vec![("reply", PureValue::Result(&values[0]))];
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    assert!(matches!(
        evaluate_pure_in_scope(&expression, &mut scope, &host, &mut fuel, LIMITS).unwrap(),
        PureValue::Scalar(ScalarValue::Integer(43))
    ));
    assert!(matches!(
        evaluate_pure_in_scope(&forged, &mut scope, &host, &mut fuel, LIMITS),
        Err(CalculationFailure::External(PureEvaluationFault::Native(9)))
    ));
}

#[test]
fn short_circuit_and_conditional_arms_are_lazy_but_not_structurally_unchecked() {
    let poison = || binary(BinaryOperator::Div, integer(1), integer(0));
    let expressions = [
        binary(
            BinaryOperator::And,
            literal(ScalarValue::Boolean(false)),
            poison(),
        ),
        binary(
            BinaryOperator::Or,
            literal(ScalarValue::Boolean(true)),
            poison(),
        ),
        Ir::Choose {
            when: Box::new(literal(ScalarValue::Boolean(true))),
            then: Box::new(integer(7)),
            otherwise: Box::new(poison()),
        },
    ];
    for (expression, expected, consumed) in [
        (&expressions[0], ScalarValue::Boolean(false), 2),
        (&expressions[1], ScalarValue::Boolean(true), 2),
        (&expressions[2], ScalarValue::Integer(7), 3),
    ] {
        let mut fuel = Fuel::new(100);
        assert_scalar(run(expression, &mut fuel).unwrap(), expected);
        assert_eq!(fuel.remaining(), 100 - consumed);
    }
    let impure = Ir::Choose {
        when: Box::new(literal(ScalarValue::Boolean(true))),
        then: Box::new(integer(1)),
        otherwise: Box::new(Ir::Host {
            effect: Box::new(OpaqueEffect),
        }),
    };
    let mut fuel = Fuel::new(100);
    assert!(matches!(
        run(&impure, &mut fuel),
        Err(CalculationFailure::External(
            PureEvaluationFault::Preflight(PureTypeError::Impure)
        ))
    ));
    assert_eq!(fuel.remaining(), 100);
}

#[test]
fn scalar_recovery_is_typed_and_preserves_exact_fuel() {
    let expression = recover(
        binary(BinaryOperator::Div, integer(1), integer(0)),
        integer(7),
    );
    let mut fuel = Fuel::new(100);
    assert_scalar(
        run(&expression, &mut fuel).unwrap(),
        ScalarValue::Integer(7),
    );
    assert_eq!(fuel.remaining(), 95);
    let invalid = recover(unary(UnaryOperator::Not, integer(0)), integer(7));
    let failure = run(&invalid, &mut Fuel::new(100)).unwrap_err();
    assert!(matches!(
        failure,
        CalculationFailure::Scalar(ScalarError::TypeMismatch)
    ));
    assert!(!failure.is_recoverable());
}

#[test]
fn fuel_exhaustion_never_runs_fallback_or_refunds_debits() {
    let expression = recover(
        binary(BinaryOperator::Div, integer(1), integer(0)),
        integer(7),
    );
    for available in 0..5 {
        let mut fuel = Fuel::new(available);
        let failure = run(&expression, &mut fuel).unwrap_err();
        assert!(matches!(
            failure,
            CalculationFailure::External(PureEvaluationFault::FuelExhausted)
        ));
        assert!(!failure.is_recoverable());
        assert_eq!(fuel.remaining(), 0);
    }
}

#[test]
fn loop_is_condition_first_and_retains_the_reference_fuel_schedule() {
    let expression = Ir::Loop {
        name: "state".into(),
        initial: Box::new(integer(0)),
        condition: Box::new(binary(BinaryOperator::Lt, local("state"), integer(3))),
        next: Box::new(binary(BinaryOperator::Add, local("state"), integer(1))),
        limit: 3,
    };
    let mut fuel = Fuel::new(100);
    assert_scalar(
        run(&expression, &mut fuel).unwrap(),
        ScalarValue::Integer(3),
    );
    assert_eq!(fuel.remaining(), 77);
}

#[test]
fn iteration_exhaustion_is_not_recoverable_and_drops_temporary_state() {
    let expression = recover(
        Ir::Loop {
            name: "state".into(),
            initial: Box::new(integer(0)),
            condition: Box::new(literal(ScalarValue::Boolean(true))),
            next: Box::new(integer(1)),
            limit: 0,
        },
        integer(7),
    );
    let mut bindings = vec![("prefix", PureValue::Scalar(ScalarValue::Integer(3)))];
    let mut scope = ScopeFrame::new(&mut bindings);
    let failure = evaluate_pure_in_scope(
        &expression,
        &mut scope,
        &editor(),
        &mut Fuel::new(100),
        LIMITS,
    )
    .unwrap_err();
    assert!(matches!(
        failure,
        CalculationFailure::External(PureEvaluationFault::Loop(LoopError::IterationLimit))
    ));
    assert!(!failure.is_recoverable());
    assert_eq!(scope.len(), 1);
    assert!(scope.get("state").is_none());
}

#[test]
fn ordered_fold_moves_text_and_charges_list_and_item_work() {
    let expression = Ir::Fold {
        name: "state".into(),
        item: "entry".into(),
        items: Box::new(Ir::Strings {
            items: vec![text("a"), text("b")],
        }),
        initial: Box::new(integer(0)),
        next: Box::new(binary(
            BinaryOperator::Add,
            local("state"),
            unary(UnaryOperator::Len, local("entry")),
        )),
        limit: 2,
    };
    let mut fuel = Fuel::new(100);
    assert_scalar(
        run(&expression, &mut fuel).unwrap(),
        ScalarValue::Integer(2),
    );
    assert_eq!(fuel.remaining(), 69);
}

#[test]
fn native_failure_is_original_and_never_invokes_scalar_fallback() {
    let expression = recover(field("private"), field("caption"));
    let host = editor();
    let reply = std::rc::Rc::new(Caption("not a fallback".into()));
    let mut bindings = vec![("reply", PureValue::Result(reply))];
    let mut scope = ScopeFrame::new(&mut bindings);
    let failure =
        evaluate_pure_in_scope(&expression, &mut scope, &host, &mut Fuel::new(100), LIMITS)
            .unwrap_err();
    assert_eq!(host.calls.get(), 1);
    assert!(!failure.is_recoverable());
    assert!(!format!("{failure:?}").contains("private"));
    let CalculationFailure::External(PureEvaluationFault::Native(NativeError(message))) = failure
    else {
        panic!()
    };
    assert_eq!(message, "private native payload");
}

#[test]
fn native_unwind_cleans_nested_bindings_without_replaying_or_refilling() {
    let expression = Ir::Bind {
        name: "temporary".into(),
        value: Box::new(integer(1)),
        body: Box::new(field("panic")),
    };
    let host = editor();
    let reply = std::rc::Rc::new(Caption("private".into()));
    let mut bindings = vec![("reply", PureValue::Result(reply.clone()))];
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _ = evaluate_pure_in_scope(&expression, &mut scope, &host, &mut fuel, LIMITS);
        }))
        .is_err()
    );
    assert_eq!(host.calls.get(), 1);
    assert_eq!(fuel.remaining(), 96);
    assert_eq!(scope.len(), 1);
    assert!(scope.get("temporary").is_none());
    assert_eq!(std::rc::Rc::strong_count(&reply), 2);
}

#[test]
fn physical_and_scope_limits_reject_before_native_queries_or_fuel() {
    let expression = field("caption");
    let host = editor();
    let reply = std::rc::Rc::new(Caption("private".into()));
    for limits in [
        PureEvaluationLimits {
            max_nodes: 0,
            ..LIMITS
        },
        PureEvaluationLimits {
            max_depth: 0,
            ..LIMITS
        },
        PureEvaluationLimits {
            max_bindings: 0,
            ..LIMITS
        },
        PureEvaluationLimits {
            max_depth: 65,
            ..LIMITS
        },
    ] {
        let mut bindings = vec![("reply", PureValue::Result(reply.clone()))];
        let mut scope = ScopeFrame::new(&mut bindings);
        let mut fuel = Fuel::new(100);
        assert!(evaluate_pure_in_scope(&expression, &mut scope, &host, &mut fuel, limits).is_err());
        assert_eq!(fuel.remaining(), 100);
        assert_eq!(host.calls.get(), 0);
    }
    assert_eq!(std::rc::Rc::strong_count(&reply), 1);
}

#[test]
fn bounded_recursion_preflight_rejects_a_deep_tree_even_in_a_cold_arm() {
    let mut deep = integer(0);
    for _ in 0..65 {
        deep = unary(UnaryOperator::ToString, deep);
    }
    let expression = Ir::Choose {
        when: Box::new(literal(ScalarValue::Boolean(true))),
        then: Box::new(integer(1)),
        otherwise: Box::new(deep),
    };
    let mut fuel = Fuel::new(100);
    assert!(matches!(
        run(&expression, &mut fuel),
        Err(CalculationFailure::External(
            PureEvaluationFault::Preflight(PureTypeError::Structure(_))
        ))
    ));
    assert_eq!(fuel.remaining(), 100);
}

#[test]
fn unbounded_native_scalar_and_prefix_are_not_calculation_recoverable() {
    let expression = recover(field("caption"), text("safe"));
    let leaf = integer(1);
    let host = editor();
    let mut bindings = vec![(
        "reply",
        PureValue::Result(std::rc::Rc::new(Caption(
            "x".repeat(MAX_SCALAR_STRING_BYTES + 1),
        ))),
    )];
    let mut scope = ScopeFrame::new(&mut bindings);
    let failure =
        evaluate_pure_in_scope(&expression, &mut scope, &host, &mut Fuel::new(100), LIMITS)
            .unwrap_err();
    assert!(matches!(
        failure,
        CalculationFailure::External(PureEvaluationFault::InvalidContract)
    ));
    assert_eq!(host.calls.get(), 1);
    let mut bindings = vec![(
        "prefix",
        PureValue::Scalar(ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1))),
    )];
    let mut scope = ScopeFrame::new(&mut bindings);
    let mut fuel = Fuel::new(100);
    assert!(matches!(
        evaluate_pure_in_scope(&leaf, &mut scope, &host, &mut fuel, LIMITS),
        Err(CalculationFailure::External(
            PureEvaluationFault::InvalidScope
        ))
    ));
    assert_eq!(fuel.remaining(), 100);
}

#[test]
fn active_binding_limit_and_shadowing_preserve_the_borrowed_prefix() {
    for (name, limit, binding_limit) in [("second", 1, true), ("prefix", 16, false)] {
        let expression = Ir::Bind {
            name: name.into(),
            value: Box::new(integer(2)),
            body: Box::new(integer(3)),
        };
        let mut bindings = vec![("prefix", PureValue::Scalar(ScalarValue::Integer(1)))];
        let mut scope = ScopeFrame::new(&mut bindings);
        let failure = evaluate_pure_in_scope(
            &expression,
            &mut scope,
            &editor(),
            &mut Fuel::new(100),
            PureEvaluationLimits {
                max_bindings: limit,
                ..LIMITS
            },
        )
        .unwrap_err();
        if binding_limit {
            assert!(matches!(
                failure,
                CalculationFailure::External(PureEvaluationFault::BindingLimit)
            ));
        } else {
            assert!(matches!(
                failure,
                CalculationFailure::External(PureEvaluationFault::InvalidContract)
            ));
        }
        assert_eq!(scope.len(), 1);
        assert!(matches!(
            scope.get("prefix"),
            Some(PureValue::Scalar(ScalarValue::Integer(1)))
        ));
    }
}

#[test]
fn value_and_fault_debug_never_use_native_or_scalar_payload_formatters() {
    struct Poison;
    impl std::fmt::Debug for Poison {
        fn fmt(&self, _: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            panic!("must not format native payload")
        }
    }
    assert_eq!(
        format!("{:?}", PureValue::Result(Poison)),
        "Result(<native>)"
    );
    assert_eq!(
        format!(
            "{:?}",
            PureValue::<()>::Scalar(ScalarValue::String("secret".into()))
        ),
        "Scalar(String)"
    );
    let failure = PureEvaluationFault::Native(Poison);
    assert!(!format!("{failure:?}").contains("Poison"));
    assert!(std::error::Error::source(&failure).is_none());
}

#[test]
fn zero_limit_with_false_condition_never_evaluates_next() {
    let expression = Ir::Loop {
        name: "state".into(),
        initial: Box::new(integer(7)),
        condition: Box::new(literal(ScalarValue::Boolean(false))),
        next: Box::new(binary(BinaryOperator::Div, integer(1), integer(0))),
        limit: 0,
    };
    let mut fuel = Fuel::new(100);
    assert_scalar(
        run(&expression, &mut fuel).unwrap(),
        ScalarValue::Integer(7),
    );
    assert_eq!(fuel.remaining(), 97);
}

#[test]
fn fold_rejects_insufficient_limit_without_truncation_or_next_evaluation() {
    let expression = recover(
        Ir::Fold {
            name: "state".into(),
            item: "entry".into(),
            items: Box::new(Ir::Strings {
                items: vec![text("a"), text("b")],
            }),
            initial: Box::new(integer(0)),
            next: Box::new(binary(BinaryOperator::Div, integer(1), integer(0))),
            limit: 1,
        },
        integer(7),
    );
    let mut fuel = Fuel::new(100);
    let failure = run(&expression, &mut fuel).unwrap_err();
    assert!(matches!(
        failure,
        CalculationFailure::External(PureEvaluationFault::Fold(_))
    ));
    assert!(!failure.is_recoverable());
    assert_eq!(fuel.remaining(), 88);
}

#[test]
fn duplicate_or_noncanonical_prefix_names_fail_before_native_work() {
    let expression = field("caption");
    let host = editor();
    for names in [["reply", "reply"], ["reply", "bad-name"], ["reply", "fn"]] {
        let mut bindings = names
            .into_iter()
            .map(|name| {
                (
                    name,
                    PureValue::Result(std::rc::Rc::new(Caption("private".into()))),
                )
            })
            .collect();
        let mut scope = ScopeFrame::new(&mut bindings);
        let mut fuel = Fuel::new(100);
        assert!(matches!(
            evaluate_pure_in_scope(&expression, &mut scope, &host, &mut fuel, LIMITS),
            Err(CalculationFailure::External(
                PureEvaluationFault::InvalidScope
            ))
        ));
        assert_eq!(fuel.remaining(), 100);
        assert_eq!(host.calls.get(), 0);
        assert_eq!(scope.len(), 2);
    }
}
