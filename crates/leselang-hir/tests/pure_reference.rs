use leselang_hir::computation::{Computation, GroupLocalType};
use leselang_hir::host_call::HostOperation;
use leselang_hir::pure_typing::{
    PureType, PureTypeEnvironment, TypeInferenceLimits, infer_pure_type,
};
use leselang_hir::{CanonicalSourceError, Effect, Type, canonical_source, lower};
use leselang_runtime_core::{ScalarType, ScalarValue, UnaryOperator};
use leselang_syntax::parse;

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
const LIMITS: TypeInferenceLimits = TypeInferenceLimits {
    max_nodes: 1024,
    max_depth: 16,
    max_bindings: 48,
};

#[test]
fn reference_lowered_data_control_flow_matches_generic_inference_without_wire_changes() {
    for body in [
        "bind(x: 1, body: choose(when: eq(left: x, right: 1), then: add(left: x, right: 2), otherwise: 0))",
        "loop(n: 0, while: lt(left: n, right: 3), next: add(left: n, right: 1), limit: 3)",
        r#"fold(n: 0, items: strings(a: "x"), item: "entry", next: add(left: n, right: len(value: entry)), limit: 0)"#,
        "recover(value: div(left: 1, right: 0), fallback: 7)",
        r#"value_or(left: optional_string(value: none), right: "fallback")"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {body}"))).unwrap();
        let Effect::Compute { expression } = &program.function.effect else {
            panic!("expected computation")
        };
        let wire = serde_json::to_vec(expression).unwrap();
        let Type::Scalar(ty) = program.function.result_type else {
            panic!("expected scalar")
        };
        assert_eq!(
            infer_pure_type(expression, &[], &NoHost, LIMITS),
            Ok(PureType::Scalar(ty))
        );
        assert_eq!(
            expression.validate_in_scope(&[]).unwrap(),
            program.function.result_type
        );
        assert_eq!(serde_json::to_vec(expression).unwrap(), wire);
        let source = canonical_source(&program.function.effect).unwrap();
        assert_eq!(lower(&parse(&source)).unwrap(), program);
    }
}

#[test]
fn native_type_success_does_not_bypass_reference_canonicalization() {
    let expression = Computation::Unary {
        operator: UnaryOperator::OptionalString,
        value: Box::new(Computation::Literal {
            value: ScalarValue::None,
        }),
    };
    assert_eq!(
        infer_pure_type(&expression, &[], &NoHost, LIMITS),
        Ok(PureType::Scalar(ScalarType::OptionalString))
    );
    assert!(matches!(
        expression.validate_in_scope(&[]),
        Err(CanonicalSourceError::RoundTripMismatch)
    ));
    let expression = Computation::Strings {
        items: vec![Computation::Literal {
            value: ScalarValue::String("ready".into()),
        }],
    };
    assert_eq!(
        infer_pure_type(&expression, &[], &NoHost, LIMITS),
        Ok(PureType::Scalar(ScalarType::StringList))
    );
    assert!(matches!(
        expression.validate_in_scope(&[]),
        Err(CanonicalSourceError::RoundTripMismatch)
    ));
}

#[test]
fn reference_external_result_fields_and_closed_group_aliases_keep_their_old_types() {
    let program = lower(&parse(r#"fn main() = bind(r: ui.focus(node_id: "a"), body: concat(left: field(value: r, name: "node_id"), right: "-b"))"#)).unwrap();
    let Effect::Compute { expression } = program.function.effect else {
        panic!("expected computation")
    };
    let Computation::Bind { body, .. } = *expression else {
        panic!("expected binding")
    };
    let scope = [("r".into(), Type::UiFocus)];
    assert_eq!(
        body.validate_in_scope(&scope).unwrap(),
        Type::Scalar(ScalarType::String)
    );
    let group = GroupLocalType {
        name: "g".into(),
        members: vec![("move".into(), HostOperation::UiNavigateFocus)],
    };
    let source = r#"fn main() = bind(g: seq(move: ui.navigate_focus(node_id: "a", direction: "next")), body: bind(alias: g, body: field(value: member(value: alias, name: "move"), name: "focused_node_id")))"#;
    let program = lower(&parse(source)).unwrap();
    let Effect::Compute { expression } = program.function.effect else {
        panic!("expected computation")
    };
    let Computation::Bind { body, .. } = *expression else {
        panic!("expected binding")
    };
    assert_eq!(
        body.validate_in_group_scope(&[], &[group]).unwrap(),
        Type::Scalar(ScalarType::String)
    );
}

#[test]
fn reference_legacy_group_type_identity_does_not_merge_or_widen_exported_members() {
    let groups = [
        GroupLocalType {
            name: "first".into(),
            members: vec![("a".into(), HostOperation::UiFocus)],
        },
        GroupLocalType {
            name: "second".into(),
            members: vec![("b".into(), HostOperation::UiScrollIntoView)],
        },
    ];
    let selected = || Computation::Choose {
        when: Box::new(Computation::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(Computation::Local {
            name: "first".into(),
        }),
        otherwise: Box::new(Computation::Local {
            name: "second".into(),
        }),
    };
    assert_eq!(
        selected().validate_in_group_scope(&[], &groups).unwrap(),
        Type::Structured
    );
    let constant_body = Computation::Bind {
        name: "alias".into(),
        value: Box::new(selected()),
        body: Box::new(Computation::Literal {
            value: ScalarValue::Integer(1),
        }),
    };
    assert_eq!(
        constant_body.validate_in_group_scope(&[], &groups).unwrap(),
        Type::Scalar(ScalarType::Integer)
    );
    let member_body = Computation::Bind {
        name: "alias".into(),
        value: Box::new(selected()),
        body: Box::new(Computation::Member {
            group: "alias".into(),
            name: "a".into(),
            operation: HostOperation::UiFocus,
        }),
    };
    assert!(member_body.validate_in_group_scope(&[], &groups).is_err());
}

#[test]
fn forged_cold_member_tags_still_fail_closed_before_reference_execution() {
    let group = GroupLocalType {
        name: "g".into(),
        members: vec![("a".into(), HostOperation::UiFocus)],
    };
    let expression = Computation::Choose {
        when: Box::new(Computation::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(Computation::Member {
            group: "g".into(),
            name: "a".into(),
            operation: HostOperation::UiFocus,
        }),
        otherwise: Box::new(Computation::Member {
            group: "g".into(),
            name: "a".into(),
            operation: HostOperation::UiScrollIntoView,
        }),
    };
    assert!(matches!(
        expression.validate_in_group_scope(&[], &[group]),
        Err(CanonicalSourceError::RoundTripMismatch)
    ));
}
