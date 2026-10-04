use std::{cell::Cell, panic::AssertUnwindSafe, rc::Rc};

use leselang_hir::ir::{Computation, ComputedArgument, ComputedBranch, GroupKind};
use leselang_hir::pure_typing::{
    MAX_TYPE_INFERENCE_BINDINGS, MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES, PureType,
    PureTypeEnvironment, PureTypeError, TypeInferenceLimits, infer_pure_type,
};
use leselang_runtime_core::{
    BinaryOperator, MAX_LOOP_ITERATIONS, MAX_SCALAR_STRING_BYTES, MAX_STRING_LIST_ITEMS,
    NamedParameter, OperationCatalog, OperationCatalogError, OperationCatalogLimits,
    OperationSchema, OptionalStringValue, ScalarType, ScalarTypeSet, ScalarValue, StringListValue,
    StructureError, UnaryOperator,
};

type Ir = Computation<u8, u32, Rc<()>, ()>;
const LIMITS: TypeInferenceLimits = TypeInferenceLimits {
    max_nodes: 1024,
    max_depth: 16,
    max_bindings: 16,
};

struct NoHost;
impl<Field, Operation> PureTypeEnvironment<Field, Operation> for NoHost {
    type Result = ();
    fn field_type(&self, _: &(), _: &Field) -> Option<ScalarType> {
        None
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
fn boolean(value: bool) -> Ir {
    literal(ScalarValue::Boolean(value))
}
fn text(value: &str) -> Ir {
    literal(ScalarValue::String(value.into()))
}
fn local(name: &str) -> Ir {
    Ir::Local { name: name.into() }
}
fn binary(operator: BinaryOperator, left: Ir, right: Ir) -> Ir {
    Ir::Binary {
        operator,
        left: Box::new(left),
        right: Box::new(right),
    }
}
fn choose(when: Ir, then: Ir, otherwise: Ir) -> Ir {
    Ir::Choose {
        when: Box::new(when),
        then: Box::new(then),
        otherwise: Box::new(otherwise),
    }
}
fn binding(name: &str, value: Ir, body: Ir) -> Ir {
    Ir::Bind {
        name: name.into(),
        value: Box::new(value),
        body: Box::new(body),
    }
}
fn checked(expression: &Ir) -> Result<PureType<()>, PureTypeError> {
    infer_pure_type(expression, &[], &NoHost, LIMITS)
}

#[test]
fn all_scalar_literals_and_collection_constructors_infer_without_evaluation() {
    for value in [
        ScalarValue::Integer(0),
        ScalarValue::Boolean(false),
        ScalarValue::String(String::new()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::StringList(StringListValue(vec![])),
    ] {
        let expected = value.scalar_type();
        assert_eq!(checked(&literal(value)), Ok(PureType::Scalar(expected)));
    }
    assert_eq!(
        checked(&Ir::Strings {
            items: vec![text("a"), text("b")]
        }),
        Ok(PureType::Scalar(ScalarType::StringList))
    );
    assert_eq!(
        checked(&Ir::Strings {
            items: vec![integer(1)]
        }),
        Err(PureTypeError::StringItem)
    );
    assert_eq!(
        checked(&binary(BinaryOperator::Div, integer(1), integer(0))),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    assert_eq!(
        checked(&Ir::Unary {
            operator: UnaryOperator::ParseInteger,
            value: Box::new(text("not-an-integer"))
        }),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
}

#[test]
fn every_closed_operator_signature_is_used_for_all_six_scalar_types() {
    let values = [
        ScalarValue::Integer(1),
        ScalarValue::Boolean(false),
        ScalarValue::String("x".into()),
        ScalarValue::None,
        ScalarValue::OptionalString(OptionalStringValue(None)),
        ScalarValue::StringList(StringListValue(vec![])),
    ];
    for name in [
        "add",
        "sub",
        "mul",
        "div",
        "rem",
        "eq",
        "ne",
        "lt",
        "le",
        "gt",
        "ge",
        "and",
        "or",
        "concat",
        "value_or",
        "starts_with",
        "ends_with",
        "contains",
        "split",
        "join",
        "char_at",
        "append",
        "item_at",
    ] {
        let operator = BinaryOperator::parse(name).unwrap();
        for left in &values {
            for right in &values {
                let expected = operator
                    .result_type(left.scalar_type(), right.scalar_type())
                    .map(PureType::Scalar)
                    .ok_or(PureTypeError::BinaryOperands);
                assert_eq!(
                    checked(&binary(
                        operator,
                        literal(left.clone()),
                        literal(right.clone())
                    )),
                    expected,
                    "{name}"
                );
            }
        }
    }
    for name in [
        "not",
        "len",
        "to_string",
        "parse_integer",
        "parse_boolean",
        "optional_string",
        "has_value",
    ] {
        let operator = UnaryOperator::parse(name).unwrap();
        for value in &values {
            let expected = operator
                .result_type(value.scalar_type())
                .map(PureType::Scalar)
                .ok_or(PureTypeError::UnaryOperand);
            assert_eq!(
                checked(&Ir::Unary {
                    operator,
                    value: Box::new(literal(value.clone()))
                }),
                expected,
                "{name}"
            );
        }
    }
}

#[test]
fn cold_branches_short_circuit_operands_and_recovery_fallback_are_still_typed() {
    for value in [
        choose(boolean(true), integer(1), text("cold-private")),
        choose(boolean(false), text("cold-private"), integer(1)),
    ] {
        assert_eq!(checked(&value), Err(PureTypeError::BranchTypes));
    }
    assert_eq!(
        checked(&choose(integer(0), integer(1), integer(1))),
        Err(PureTypeError::ConditionType)
    );
    assert_eq!(
        checked(&binary(BinaryOperator::And, boolean(false), integer(1))),
        Err(PureTypeError::BinaryOperands)
    );
    assert_eq!(
        checked(&Ir::Recover {
            value: Box::new(integer(1)),
            fallback: Box::new(local("private_missing"))
        }),
        Err(PureTypeError::UnknownLocal)
    );
    assert_eq!(
        checked(&Ir::Recover {
            value: Box::new(integer(1)),
            fallback: Box::new(text("bad"))
        }),
        Err(PureTypeError::RecoveryTypes)
    );
    assert_eq!(
        checked(&Ir::Recover {
            value: Box::new(binary(BinaryOperator::Div, integer(1), integer(0))),
            fallback: Box::new(integer(7))
        }),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
}

#[test]
fn sibling_scopes_reuse_names_but_do_not_leak_or_shadow_active_bindings() {
    let value = binary(
        BinaryOperator::Add,
        binding("tmp", integer(1), local("tmp")),
        binding("tmp", integer(2), local("tmp")),
    );
    assert_eq!(checked(&value), Ok(PureType::Scalar(ScalarType::Integer)));
    assert_eq!(
        checked(&binary(
            BinaryOperator::Add,
            binding("tmp", integer(1), local("tmp")),
            local("tmp")
        )),
        Err(PureTypeError::UnknownLocal)
    );
    let scope = [("outer", PureType::Scalar(ScalarType::Integer))];
    assert_eq!(
        infer_pure_type(
            &binding("outer", integer(2), local("outer")),
            &scope,
            &NoHost,
            LIMITS
        ),
        Err(PureTypeError::ShadowedBinding)
    );
    assert_eq!(scope, [("outer", PureType::Scalar(ScalarType::Integer))]);
}

fn loop_value(condition: Ir, next: Ir, limit: u64) -> Ir {
    Ir::Loop {
        name: "state".into(),
        initial: Box::new(integer(0)),
        condition: Box::new(condition),
        next: Box::new(next),
        limit,
    }
}
fn fold_value(items: Ir, next: Ir, limit: u64) -> Ir {
    Ir::Fold {
        name: "state".into(),
        item: "entry".into(),
        items: Box::new(items),
        initial: Box::new(integer(0)),
        next: Box::new(next),
        limit,
    }
}

#[test]
fn zero_limit_loops_and_folds_preserve_state_and_check_unexecuted_bodies() {
    assert_eq!(
        checked(&loop_value(boolean(false), local("state"), 0)),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    assert_eq!(
        checked(&loop_value(boolean(false), text("bad"), 0)),
        Err(PureTypeError::LoopState)
    );
    assert_eq!(
        checked(&loop_value(integer(0), local("state"), 0)),
        Err(PureTypeError::ConditionType)
    );
    assert_eq!(
        checked(&loop_value(boolean(false), local("missing"), 0)),
        Err(PureTypeError::UnknownLocal)
    );
    assert_eq!(
        checked(&loop_value(
            boolean(false),
            local("state"),
            MAX_LOOP_ITERATIONS + 1
        )),
        Err(PureTypeError::LoopLimit)
    );
    let items = || Ir::Strings {
        items: vec![text("a")],
    };
    let next = binary(
        BinaryOperator::Add,
        local("state"),
        Ir::Unary {
            operator: UnaryOperator::Len,
            value: Box::new(local("entry")),
        },
    );
    assert_eq!(
        checked(&fold_value(items(), next, 0)),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    assert_eq!(
        checked(&fold_value(items(), local("entry"), 0)),
        Err(PureTypeError::FoldState)
    );
    assert_eq!(
        checked(&fold_value(text("not-a-list"), local("state"), 0)),
        Err(PureTypeError::FoldItems)
    );
    assert_eq!(
        checked(&fold_value(
            items(),
            local("state"),
            MAX_STRING_LIST_ITEMS as u64 + 1
        )),
        Err(PureTypeError::FoldLimit)
    );
}

#[test]
fn explicit_physical_node_depth_and_binding_limits_have_inclusive_edges() {
    let mut limits = TypeInferenceLimits {
        max_nodes: 1,
        max_depth: 0,
        max_bindings: 0,
    };
    assert_eq!(
        infer_pure_type(&integer(1), &[], &NoHost, limits),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    limits.max_nodes = 0;
    assert_eq!(
        infer_pure_type(&integer(1), &[], &NoHost, limits),
        Err(PureTypeError::Structure(StructureError::NodeLimit))
    );
    let nested = Ir::Unary {
        operator: UnaryOperator::Not,
        value: Box::new(boolean(true)),
    };
    limits.max_nodes = 2;
    assert_eq!(
        infer_pure_type(&nested, &[], &NoHost, limits),
        Err(PureTypeError::Structure(StructureError::DepthLimit))
    );
    limits.max_depth = 1;
    assert_eq!(
        infer_pure_type(&nested, &[], &NoHost, limits),
        Ok(PureType::Scalar(ScalarType::Boolean))
    );
    let bound = binding("x", integer(1), local("x"));
    limits.max_nodes = 3;
    assert_eq!(
        infer_pure_type(&bound, &[], &NoHost, limits),
        Err(PureTypeError::BindingLimit)
    );
    limits.max_bindings = 1;
    assert_eq!(
        infer_pure_type(&bound, &[], &NoHost, limits),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    let wide = Ir::Strings {
        items: vec![text("x"), text("y")],
    };
    limits.max_nodes = 2;
    assert_eq!(
        infer_pure_type(&wide, &[], &NoHost, limits),
        Err(PureTypeError::Structure(StructureError::NodeLimit))
    );
}

#[test]
fn hard_limit_ceilings_invalid_prefixes_and_names_fail_before_native_work() {
    for limits in [
        TypeInferenceLimits {
            max_nodes: MAX_TYPE_INFERENCE_NODES + 1,
            ..LIMITS
        },
        TypeInferenceLimits {
            max_depth: MAX_TYPE_INFERENCE_DEPTH + 1,
            ..LIMITS
        },
        TypeInferenceLimits {
            max_bindings: MAX_TYPE_INFERENCE_BINDINGS + 1,
            ..LIMITS
        },
    ] {
        assert_eq!(
            infer_pure_type(&integer(1), &[], &NoHost, limits),
            Err(PureTypeError::InvalidLimits)
        );
    }
    for name in [
        "",
        "body",
        "fn",
        "true",
        "false",
        "none",
        "2bad",
        "private-name",
        "\u{754c}",
    ] {
        assert_eq!(checked(&local(name)), Err(PureTypeError::InvalidName));
        assert_eq!(
            infer_pure_type(
                &integer(1),
                &[(name, PureType::Scalar(ScalarType::Integer))],
                &NoHost,
                LIMITS
            ),
            Err(PureTypeError::InvalidScope)
        );
    }
    let long = "x".repeat(65);
    assert_eq!(checked(&local(&long)), Err(PureTypeError::InvalidName));
    assert_eq!(
        infer_pure_type(
            &integer(1),
            &[
                ("same", PureType::Scalar(ScalarType::Integer)),
                ("same", PureType::Scalar(ScalarType::Integer))
            ],
            &NoHost,
            LIMITS
        ),
        Err(PureTypeError::InvalidScope)
    );
    assert_eq!(
        infer_pure_type(
            &integer(1),
            &[("x", PureType::Scalar(ScalarType::Integer))],
            &NoHost,
            TypeInferenceLimits {
                max_bindings: 0,
                ..LIMITS
            }
        ),
        Err(PureTypeError::InvalidScope)
    );
}

#[test]
fn literal_and_collection_bounds_are_checked_before_type_inference() {
    for value in [
        ScalarValue::String("x".repeat(MAX_SCALAR_STRING_BYTES + 1)),
        ScalarValue::OptionalString(OptionalStringValue(Some(
            "x".repeat(MAX_SCALAR_STRING_BYTES + 1),
        ))),
        ScalarValue::StringList(StringListValue(vec![
            String::new();
            MAX_STRING_LIST_ITEMS + 1
        ])),
        ScalarValue::StringList(StringListValue(vec![
            "x".repeat(MAX_SCALAR_STRING_BYTES),
            "y".into(),
        ])),
    ] {
        assert_eq!(
            checked(&literal(value)),
            Err(PureTypeError::UnboundedLiteral)
        );
    }
    assert_eq!(
        checked(&Ir::Strings {
            items: (0..=MAX_STRING_LIST_ITEMS).map(|_| text("x")).collect()
        }),
        Err(PureTypeError::StringListLimit)
    );
}

#[test]
fn all_host_node_kinds_are_rejected_even_if_empty_or_cold_without_payload_inspection() {
    let native = Rc::new(());
    for value in [
        Ir::Host {
            effect: Box::new(native.clone()),
        },
        Ir::Call {
            operation: 17,
            arguments: vec![],
        },
        Ir::Group {
            group_kind: GroupKind::Parallel,
            branches: vec![],
        },
        Ir::Group {
            group_kind: GroupKind::Sequence,
            branches: vec![ComputedBranch {
                name: "a".into(),
                value: integer(1),
                result_type: (),
            }],
        },
    ] {
        assert_eq!(
            checked(&choose(boolean(true), integer(1), value)),
            Err(PureTypeError::Impure)
        );
    }
    assert_eq!(Rc::strong_count(&native), 1);
}

struct NativeEnvironment {
    field_reads: Cell<usize>,
    member_reads: Cell<usize>,
}
impl NativeEnvironment {
    fn new() -> Self {
        Self {
            field_reads: Cell::new(0),
            member_reads: Cell::new(0),
        }
    }
}
impl PureTypeEnvironment<u8, u32> for NativeEnvironment {
    type Result = u8;
    fn field_type(&self, result: &u8, field: &u8) -> Option<ScalarType> {
        self.field_reads.set(self.field_reads.get() + 1);
        match (*result, *field) {
            (1, 7) => Some(ScalarType::Integer),
            (1, 8) => Some(ScalarType::String),
            _ => None,
        }
    }
    fn member_result(&self, group: &u8, name: &str, operation: &u32) -> Option<u8> {
        self.member_reads.set(self.member_reads.get() + 1);
        (*group == 2 && name == "move" && *operation == 17).then_some(1)
    }
}
fn field(value: Ir, field: u8) -> Ir {
    Ir::Field {
        value: Box::new(value),
        field,
    }
}
fn member(group: &str, name: &str, operation: u32) -> Ir {
    Ir::Member {
        group: group.into(),
        name: name.into(),
        operation,
    }
}

#[test]
fn closed_host_projections_and_group_aliases_require_exact_member_operation_tags() {
    let environment = NativeEnvironment::new();
    let scope = [
        ("batch", PureType::Result(2)),
        ("receipt", PureType::Result(1)),
    ];
    let value = binding(
        "alias",
        local("batch"),
        field(member("alias", "move", 17), 7),
    );
    assert_eq!(
        infer_pure_type(&value, &scope, &environment, LIMITS),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    assert_eq!(
        infer_pure_type(&field(local("receipt"), 99), &scope, &environment, LIMITS),
        Err(PureTypeError::FieldNotExported)
    );
    assert_eq!(
        infer_pure_type(&field(integer(1), 7), &scope, &environment, LIMITS),
        Err(PureTypeError::FieldNotExported)
    );
    for value in [
        member("batch", "wrong", 17),
        member("batch", "move", 18),
        member("receipt", "move", 17),
        member("missing", "move", 17),
    ] {
        assert_eq!(
            infer_pure_type(&value, &scope, &environment, LIMITS),
            Err(PureTypeError::MemberNotExported)
        );
    }
    assert_eq!(
        scope,
        [
            ("batch", PureType::Result(2)),
            ("receipt", PureType::Result(1))
        ]
    );
}

#[test]
fn native_type_queries_run_for_both_valid_cold_branches_but_not_invalid_physical_trees() {
    let environment = NativeEnvironment::new();
    let scope = [("receipt", PureType::Result(1))];
    let value = choose(
        boolean(true),
        field(local("receipt"), 7),
        field(local("receipt"), 7),
    );
    assert_eq!(
        infer_pure_type(&value, &scope, &environment, LIMITS),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    assert_eq!(environment.field_reads.get(), 2);
    let cold_invalid = choose(
        boolean(true),
        field(local("receipt"), 7),
        field(local("receipt"), 99),
    );
    assert_eq!(
        infer_pure_type(&cold_invalid, &scope, &environment, LIMITS),
        Err(PureTypeError::FieldNotExported)
    );
    assert_eq!(environment.field_reads.get(), 4);
    let impure = choose(
        boolean(true),
        field(local("receipt"), 7),
        Ir::Host {
            effect: Box::new(Rc::new(())),
        },
    );
    assert_eq!(
        infer_pure_type(&impure, &scope, &environment, LIMITS),
        Err(PureTypeError::Impure)
    );
    assert_eq!(environment.field_reads.get(), 4);
}

#[test]
fn native_result_metadata_is_not_implicitly_scalar_or_recoverable() {
    let environment = NativeEnvironment::new();
    let scope = [("a", PureType::Result(1)), ("b", PureType::Result(2))];
    assert_eq!(
        infer_pure_type(&local("a"), &scope, &environment, LIMITS),
        Ok(PureType::Result(1))
    );
    assert_eq!(
        infer_pure_type(
            &choose(boolean(true), local("a"), local("b")),
            &scope,
            &environment,
            LIMITS
        ),
        Err(PureTypeError::BranchTypes)
    );
    assert_eq!(
        infer_pure_type(
            &binary(BinaryOperator::Eq, local("a"), local("a")),
            &scope,
            &environment,
            LIMITS
        ),
        Err(PureTypeError::NonScalar)
    );
    assert_eq!(
        infer_pure_type(
            &Ir::Recover {
                value: Box::new(local("a")),
                fallback: Box::new(local("a"))
            },
            &scope,
            &environment,
            LIMITS
        ),
        Err(PureTypeError::RecoveryTypes)
    );
}

#[test]
fn explicit_host_type_joins_preserve_only_common_exports_not_branch_unions() {
    struct Environment;
    impl PureTypeEnvironment<u8, u32> for Environment {
        type Result = u8;
        fn field_type(&self, result: &u8, field: &u8) -> Option<ScalarType> {
            match *field {
                7 if result & 1 != 0 => Some(ScalarType::Integer),
                8 if result & 2 != 0 => Some(ScalarType::String),
                _ => None,
            }
        }
        fn member_result(&self, _: &u8, _: &str, _: &u32) -> Option<u8> {
            None
        }
        fn join_results(&self, left: u8, right: u8) -> Option<u8> {
            Some(left & right)
        }
    }
    let scope = [
        ("left", PureType::Result(3)),
        ("right", PureType::Result(1)),
    ];
    let join = || choose(boolean(true), local("left"), local("right"));
    assert_eq!(
        infer_pure_type(
            &binding("alias", join(), field(local("alias"), 7)),
            &scope,
            &Environment,
            LIMITS
        ),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
    assert_eq!(
        infer_pure_type(
            &binding("alias", join(), field(local("alias"), 8)),
            &scope,
            &Environment,
            LIMITS
        ),
        Err(PureTypeError::FieldNotExported)
    );
}

#[test]
fn structural_preflight_precedes_native_metadata_cloning_and_equality() {
    struct Bomb;
    impl Clone for Bomb {
        fn clone(&self) -> Self {
            panic!("metadata clone must not run")
        }
    }
    impl PartialEq for Bomb {
        fn eq(&self, _: &Self) -> bool {
            panic!("metadata equality must not run")
        }
    }
    struct Environment;
    impl PureTypeEnvironment<u8, u32> for Environment {
        type Result = Bomb;
        fn field_type(&self, _: &Bomb, _: &u8) -> Option<ScalarType> {
            panic!("query must not run")
        }
        fn member_result(&self, _: &Bomb, _: &str, _: &u32) -> Option<Bomb> {
            panic!("query must not run")
        }
    }
    let scope = [("receipt", PureType::Result(Bomb))];
    for value in [
        choose(
            boolean(true),
            field(local("receipt"), 7),
            Ir::Host {
                effect: Box::new(Rc::new(())),
            },
        ),
        choose(
            boolean(true),
            field(local("receipt"), 7),
            text(&"x".repeat(MAX_SCALAR_STRING_BYTES + 1)),
        ),
    ] {
        let result = infer_pure_type(&value, &scope, &Environment, LIMITS);
        assert!(matches!(
            result,
            Err(PureTypeError::Impure | PureTypeError::UnboundedLiteral)
        ));
    }
}

#[test]
fn native_query_unwind_releases_temporary_metadata_and_does_not_mutate_caller_scope() {
    #[derive(Clone, PartialEq)]
    struct Tag(Rc<()>);
    struct Environment;
    impl PureTypeEnvironment<u8, u32> for Environment {
        type Result = Tag;
        fn field_type(&self, _: &Tag, _: &u8) -> Option<ScalarType> {
            panic!("native query unwind")
        }
        fn member_result(&self, _: &Tag, _: &str, _: &u32) -> Option<Tag> {
            None
        }
    }
    let tag = Rc::new(());
    let scope = [("receipt", PureType::Result(Tag(tag.clone())))];
    let value = binding("alias", local("receipt"), field(local("alias"), 7));
    assert!(
        std::panic::catch_unwind(AssertUnwindSafe(|| infer_pure_type(
            &value,
            &scope,
            &Environment,
            LIMITS
        )))
        .is_err()
    );
    assert_eq!(Rc::strong_count(&tag), 2);
    assert_eq!(scope[0].0, "receipt");
    assert_eq!(
        checked(&integer(1)),
        Ok(PureType::Scalar(ScalarType::Integer))
    );
}

#[test]
fn inference_errors_do_not_echo_local_names_literals_or_native_keys() {
    for value in [
        local("private_missing"),
        choose(boolean(true), integer(1), text("private-text")),
        field(integer(1), 99),
    ] {
        let error = checked(&value).unwrap_err();
        assert!(!format!("{error:?}: {error}").contains("private"));
    }
}

#[test]
fn two_unrelated_native_hosts_infer_projection_control_flow_without_cloning_payloads() {
    enum GuiField {
        Caption,
    }
    enum GuiOperation {
        Caption,
    }
    #[derive(Clone, PartialEq)]
    enum GuiResult {
        Group,
        Receipt,
    }
    struct GuiEnvironment;
    impl PureTypeEnvironment<GuiField, GuiOperation> for GuiEnvironment {
        type Result = GuiResult;
        fn field_type(&self, result: &GuiResult, _: &GuiField) -> Option<ScalarType> {
            (*result == GuiResult::Receipt).then_some(ScalarType::String)
        }
        fn member_result(
            &self,
            group: &GuiResult,
            name: &str,
            _: &GuiOperation,
        ) -> Option<GuiResult> {
            (*group == GuiResult::Group && name == "caption").then_some(GuiResult::Receipt)
        }
    }
    struct Opaque(Rc<()>);
    type GuiIr = Computation<GuiField, GuiOperation, Opaque, Opaque>;
    let gui = GuiIr::Bind {
        name: "receipt".into(),
        value: Box::new(GuiIr::Member {
            group: "batch".into(),
            name: "caption".into(),
            operation: GuiOperation::Caption,
        }),
        body: Box::new(GuiIr::Choose {
            when: Box::new(GuiIr::Literal {
                value: ScalarValue::Boolean(false),
            }),
            then: Box::new(GuiIr::Field {
                value: Box::new(GuiIr::Local {
                    name: "receipt".into(),
                }),
                field: GuiField::Caption,
            }),
            otherwise: Box::new(GuiIr::Literal {
                value: ScalarValue::String("ready".into()),
            }),
        }),
    };
    let scope = [("batch", PureType::Result(GuiResult::Group))];
    assert!(matches!(
        infer_pure_type(&gui, &scope, &GuiEnvironment, LIMITS),
        Ok(PureType::Scalar(ScalarType::String))
    ));
    let opaque = Rc::new(());
    let host = GuiIr::Host {
        effect: Box::new(Opaque(opaque.clone())),
    };
    assert!(matches!(
        infer_pure_type(&host, &scope, &GuiEnvironment, LIMITS),
        Err(PureTypeError::Impure)
    ));
    let GuiIr::Host { effect } = host else {
        panic!("expected host")
    };
    assert!(Rc::ptr_eq(&effect.0, &opaque));

    #[derive(PartialEq)]
    struct DeviceTag {
        native_id: u32,
    }
    struct DeviceEnvironment<'a> {
        receipt: &'a DeviceTag,
    }
    impl<'a> PureTypeEnvironment<u8, u32> for DeviceEnvironment<'a> {
        type Result = &'a DeviceTag;
        fn field_type(&self, result: &&DeviceTag, field: &u8) -> Option<ScalarType> {
            (*result == self.receipt && *field == 7).then_some(ScalarType::Integer)
        }
        fn member_result(&self, _: &&DeviceTag, _: &str, _: &u32) -> Option<&'a DeviceTag> {
            None
        }
    }
    let receipt = DeviceTag { native_id: 42 };
    let environment = DeviceEnvironment { receipt: &receipt };
    let scope = [("receipt", PureType::Result(&receipt))];
    let device = Ir::Loop {
        name: "position".into(),
        initial: Box::new(field(local("receipt"), 7)),
        condition: Box::new(binary(BinaryOperator::Lt, local("position"), integer(100))),
        next: Box::new(binary(BinaryOperator::Add, local("position"), integer(1))),
        limit: 10,
    };
    assert!(matches!(
        infer_pure_type(&device, &scope, &environment, LIMITS),
        Ok(PureType::Scalar(ScalarType::Integer))
    ));
    let inferred = infer_pure_type(&local("receipt"), &scope, &environment, LIMITS).unwrap();
    let PureType::Result(original) = inferred else {
        panic!("expected native tag")
    };
    assert!(std::ptr::eq(original, &receipt));
}

#[test]
fn native_catalog_pure_inference_and_argument_typing_form_one_preparation_path() {
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17u32,
        parameters: &parameters,
        result: 1u8,
        required_capability: 31u8,
    }];
    let catalog = OperationCatalog::new(
        9,
        &schemas,
        OperationCatalogLimits {
            max_operations: 1,
            max_parameters_per_operation: 1,
        },
    )
    .unwrap();
    let environment = NativeEnvironment::new();
    let scope = [("receipt", PureType::Result(1))];
    let request = Ir::Call {
        operation: 17,
        arguments: vec![ComputedArgument {
            name: "position".into(),
            value: binary(BinaryOperator::Add, field(local("receipt"), 7), integer(1)),
        }],
    };
    let Ir::Call {
        operation,
        arguments,
    } = &request
    else {
        panic!("expected call")
    };
    assert_eq!(
        catalog.authorize(operation, 8, &[31]).unwrap_err(),
        OperationCatalogError::UnsupportedVersion
    );
    assert_eq!(
        catalog.authorize(operation, 9, &[]).unwrap_err(),
        OperationCatalogError::CapabilityDenied
    );
    assert_eq!(environment.field_reads.get(), 0);
    let schema = catalog.authorize(operation, 9, &[31]).unwrap();
    let names = arguments
        .iter()
        .map(|argument| argument.name.as_str())
        .collect::<Vec<_>>();
    schema.bind_arguments(&names).unwrap();
    let types = arguments
        .iter()
        .map(|argument| {
            let inferred = infer_pure_type(&argument.value, &scope, &environment, LIMITS).unwrap();
            argument.value.scalar_argument_type(inferred.scalar_type())
        })
        .collect::<Vec<_>>();
    schema.check_argument_types(&names, &types).unwrap();
    assert_eq!(environment.field_reads.get(), 1);
    assert_eq!(schema.result, 1);
    // This path checks preparation only. The enclosing Call is not pure or dispatched.
    assert!(matches!(
        infer_pure_type(&request, &scope, &environment, LIMITS),
        Err(PureTypeError::Impure)
    ));
}
