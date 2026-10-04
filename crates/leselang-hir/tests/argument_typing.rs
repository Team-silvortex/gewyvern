use std::rc::Rc;

use leselang_hir::host_call::HostOperation;
use leselang_hir::ir::{Computation, ComputedArgument};
use leselang_hir::{CanonicalSourceError, Effect, authorize, lower};
use leselang_host_contract::CapabilitySet;
use leselang_runtime_core::{
    ArgumentTypeError, NamedParameter, OperationCatalog, OperationCatalogLimits, OperationSchema,
    ScalarType, ScalarTypeSet, ScalarValue, check_argument_type,
};
use leselang_syntax::parse;

#[test]
fn native_ir_argument_bridge_borrows_literals_without_host_trait_requirements() {
    struct Native(Rc<()>);
    type Ir = Computation<Native, Native, Native, Native>;
    let literal = Ir::Literal {
        value: ScalarValue::String("ready".into()),
    };
    let facts = literal.scalar_argument_type(Some(ScalarType::String));
    let Ir::Literal { value } = &literal else {
        panic!("expected literal")
    };
    assert!(std::ptr::eq(facts.literal.unwrap(), value));
    check_argument_type(&ScalarTypeSet::only(ScalarType::String), facts).unwrap();
    assert_eq!(
        check_argument_type(
            &ScalarTypeSet::only(ScalarType::Boolean),
            literal.scalar_argument_type(Some(ScalarType::Boolean))
        ),
        Err(ArgumentTypeError::InconsistentLiteralType)
    );
    let native = Rc::new(());
    let host = Ir::Host {
        effect: Box::new(Native(native.clone())),
    };
    assert!(!host.scalar_argument_type(Some(ScalarType::String)).is_pure);
    let Ir::Host { effect } = host else {
        panic!("expected host")
    };
    assert!(Rc::ptr_eq(&effect.0, &native));
    assert_eq!(Rc::strong_count(&native), 2);
}

#[test]
fn generic_signature_bridge_never_hides_effects_in_unselected_children() {
    type Ir = Computation<(), u32, (), ()>;
    let value = Ir::Choose {
        when: Box::new(Ir::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(Ir::Literal {
            value: ScalarValue::String("ready".into()),
        }),
        otherwise: Box::new(Ir::Call {
            operation: 17,
            arguments: vec![],
        }),
    };
    assert_eq!(
        check_argument_type(
            &ScalarTypeSet::only(ScalarType::String),
            value.scalar_argument_type(Some(ScalarType::String))
        ),
        Err(ArgumentTypeError::Impure)
    );
    let pure = Ir::Local {
        name: "not-yet-resolved".into(),
    };
    // This bridge is not scope/type inference: the adapter must reject unknown locals.
    assert!(
        pure.scalar_argument_type(Some(ScalarType::String))
            .literal
            .is_none()
    );
    assert_eq!(
        check_argument_type(
            &ScalarTypeSet::only(ScalarType::String),
            pure.scalar_argument_type(None)
        ),
        Err(ArgumentTypeError::NonScalar)
    );
}

#[test]
fn unrelated_native_catalogs_check_arguments_from_the_same_shared_ir() {
    #[derive(PartialEq)]
    enum GuiOperation {
        Caption,
    }
    type GuiIr = Computation<(), GuiOperation, Rc<()>, ()>;
    let parameters = [NamedParameter::required(
        "caption",
        ScalarTypeSet::only(ScalarType::String),
    )];
    let schemas = [OperationSchema {
        key: GuiOperation::Caption,
        parameters: &parameters,
        result: "view-receipt",
        required_capability: "view.edit",
    }];
    let limits = OperationCatalogLimits {
        max_operations: 1,
        max_parameters_per_operation: 1,
    };
    let catalog = OperationCatalog::new(7, &schemas, limits).unwrap();
    let ir = GuiIr::Call {
        operation: GuiOperation::Caption,
        arguments: vec![ComputedArgument {
            name: "caption".into(),
            value: GuiIr::Literal {
                value: ScalarValue::String("ready".into()),
            },
        }],
    };
    let GuiIr::Call {
        operation,
        arguments,
    } = &ir
    else {
        panic!("expected call")
    };
    let schema = catalog.authorize(operation, 7, &["view.edit"]).unwrap();
    schema
        .check_argument_types(
            &[arguments[0].name.as_str()],
            &[arguments[0]
                .value
                .scalar_argument_type(Some(ScalarType::String))],
        )
        .unwrap();
    assert_eq!(schema.result, "view-receipt");

    type DeviceIr = Computation<u16, u32, (), ScalarType>;
    let parameters = [NamedParameter::required(
        "position",
        ScalarTypeSet::only(ScalarType::Integer),
    )];
    let schemas = [OperationSchema {
        key: 17u32,
        parameters: &parameters,
        result: ScalarType::Integer,
        required_capability: 31u8,
    }];
    let catalog = OperationCatalog::new(9, &schemas, limits).unwrap();
    let ir = DeviceIr::Call {
        operation: 17,
        arguments: vec![ComputedArgument {
            name: "position".into(),
            value: DeviceIr::Literal {
                value: ScalarValue::Integer(100),
            },
        }],
    };
    let DeviceIr::Call {
        operation,
        arguments,
    } = &ir
    else {
        panic!("expected call")
    };
    let schema = catalog.authorize(operation, 9, &[31]).unwrap();
    schema
        .check_argument_types(
            &[arguments[0].name.as_str()],
            &[arguments[0]
                .value
                .scalar_argument_type(Some(ScalarType::Integer))],
        )
        .unwrap();
    assert_eq!(schema.result, ScalarType::Integer);
}

#[test]
fn reference_lowering_keeps_exact_host_argument_diagnostics_and_original_spans() {
    for (body, code, message, argument) in [
        (
            r#"ui.focus(node_id: add(left: 1, right: 2))"#,
            "LSH1407",
            "invalid ui.focus argument 'node_id'",
            "node_id: add(left: 1, right: 2)",
        ),
        (
            r#"ui.focus(node_id: ui.focus(node_id: "target"))"#,
            "LSH1402",
            "expected a pure scalar expression, not a host operation",
            r#"node_id: ui.focus(node_id: "target")"#,
        ),
        (
            r#"ui.navigate_focus(direction: "private-invalid", node_id: concat(left: "tar", right: "get"))"#,
            "LSH1407",
            "invalid ui.navigate_focus argument 'direction'",
            r#"direction: "private-invalid""#,
        ),
    ] {
        let source = format!("fn main() = {body}");
        let errors = lower(&parse(&source)).unwrap_err();
        assert_eq!(errors.len(), 1);
        assert_eq!(errors[0].code, code);
        assert_eq!(errors[0].message, message);
        let span = errors[0].span.unwrap();
        assert_eq!(&source[span.start..span.end], argument);
        assert!(
            !serde_json::to_string(&errors)
                .unwrap()
                .contains("private-invalid")
        );
    }
}

#[test]
fn static_checks_keep_cold_rejections_and_evaluated_value_domain_fences() {
    let invalid_cold = r#"fn main() = choose(when: true,
        then: ui.focus(node_id: concat(left: "tar", right: "get")),
        otherwise: ui.focus(node_id: add(left: 1, right: 2)))"#;
    let error = lower(&parse(invalid_cold)).unwrap_err();
    assert_eq!(error[0].code, "LSH1407");
    let source = r#"fn main() = ui.navigate_focus(node_id: concat(left: "tar", right: "get"), direction: concat(left: "invalid", right: "-direction"))"#;
    let program = lower(&parse(source)).unwrap();
    authorize(&program, &CapabilitySet::new(["ui.presentation"])).unwrap();
    let errors = HostOperation::UiNavigateFocus
        .resolve(&[
            ("node_id".into(), ScalarValue::String("target".into())),
            (
                "direction".into(),
                ScalarValue::String("invalid-direction".into()),
            ),
        ])
        .unwrap_err();
    assert!(!errors.is_empty());
    let Effect::Compute { mut expression } = program.function.effect else {
        panic!("expected computation")
    };
    let leselang_hir::computation::Computation::Call { arguments, .. } = expression.as_mut() else {
        panic!("expected call")
    };
    arguments[0].value = leselang_hir::computation::Computation::Literal {
        value: ScalarValue::Boolean(false),
    };
    assert!(matches!(
        expression.validate_in_scope(&[]),
        Err(CanonicalSourceError::InvalidEffect(_))
    ));
}
