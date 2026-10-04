use leselang_hir::computation::{Computation, GroupLocalType, ScalarType, ScalarValue};
use leselang_hir::host_call::HostOperation;
use leselang_hir::{Effect, MAX_EFFECT_NESTING_DEPTH, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn sibling_scopes_reuse_closed_names_without_exporting_them() {
    for body in [
        "add(left: bind(tmp: 1, body: tmp), right: bind(tmp: 2, body: tmp))",
        "choose(when: true, then: bind(tmp: 1, body: tmp), otherwise: bind(tmp: 2, body: tmp))",
        "add(left: loop(tmp: 1, while: false, next: tmp, limit: 0), right: bind(tmp: 2, body: tmp))",
        r#"add(left: fold(tmp: 0, items: strings(a: "x"), item: "entry", next: add(left: tmp, right: len(value: entry)), limit: 1), right: bind(entry: 2, body: entry))"#,
        r#"all(a: bind(tmp: "a", body: ui.focus(node_id: tmp)), b: bind(tmp: "b", body: ui.focus(node_id: tmp)))"#,
    ] {
        let program = lower(&parse(&format!("fn main() = {body}"))).unwrap();
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(lower(&parse(&canonical)).unwrap(), program, "{body}");
    }
    for body in [
        "add(left: bind(tmp: 1, body: tmp), right: tmp)",
        "choose(when: true, then: bind(tmp: 1, body: tmp), otherwise: tmp)",
        "add(left: loop(tmp: 1, while: false, next: tmp, limit: 0), right: tmp)",
        r#"add(left: fold(tmp: 0, items: strings(), item: "entry", next: tmp, limit: 0), right: entry)"#,
    ] {
        let errors = lower(&parse(&format!("fn main() = {body}"))).unwrap_err();
        assert!(errors.iter().any(|error| error.code == "LSH1403"), "{body}");
    }
}

#[test]
fn failed_group_member_restores_names_before_later_diagnostics() {
    let source = r#"fn main() = all(
        a: bind(tmp: "a", body: ui.focus(node_id: missing)),
        b: bind(tmp: "b", body: ui.focus(node_id: other)))"#;
    let errors = lower(&parse(source)).unwrap_err();
    assert_eq!(errors.len(), 2);
    for (error, name) in errors.iter().zip(["missing", "other"]) {
        assert_eq!(error.code, "LSH1403");
        assert_eq!(error.message, format!("undefined local '{name}'"));
        let span = error.span.unwrap();
        assert_eq!(&source[span.start..span.end], name);
    }
}

#[test]
fn external_scalar_scope_is_unchanged_on_success_and_cold_failure() {
    let environment = vec![("saved".into(), Type::Scalar(ScalarType::Integer))];
    let original = environment.clone();
    let expression = Computation::Local {
        name: "saved".into(),
    };
    assert_eq!(
        expression.validate_in_scope(&environment).unwrap(),
        environment[0].1
    );
    for body in [
        "bind(tmp: saved, body: tmp)",
        "choose(when: true, then: saved, otherwise: bind(tmp: saved, body: tmp))",
    ] {
        let program = lower(&parse(&format!("fn main() = bind(saved: 1, body: {body})"))).unwrap();
        let Effect::Compute { expression } = program.function.effect else {
            panic!()
        };
        let Computation::Bind { body, .. } = *expression else {
            panic!()
        };
        assert_eq!(
            body.validate_in_scope(&environment).unwrap(),
            environment[0].1
        );
    }
    let missing = Computation::Choose {
        when: Box::new(Computation::Literal {
            value: ScalarValue::Boolean(true),
        }),
        then: Box::new(expression.clone()),
        otherwise: Box::new(Computation::Local { name: "tmp".into() }),
    };
    assert!(missing.validate_in_scope(&environment).is_err());
    assert!(
        expression
            .validate_in_scope(&vec![("saved".into(), environment[0].1); 2])
            .is_err()
    );
    assert_eq!(environment, original);
}

#[test]
fn external_prefix_still_spends_nesting_depth_even_for_a_literal() {
    let expression = Computation::Literal {
        value: ScalarValue::Integer(1),
    };
    let mut environment = (0..MAX_EFFECT_NESTING_DEPTH)
        .map(|index| (format!("saved_{index}"), Type::Scalar(ScalarType::Integer)))
        .collect::<Vec<_>>();
    assert_eq!(
        expression.validate_in_scope(&environment).unwrap(),
        Type::Scalar(ScalarType::Integer)
    );
    environment.push(("one_more".into(), Type::Scalar(ScalarType::Integer)));
    assert!(expression.validate_in_scope(&environment).is_err());
}

#[test]
fn owned_helper_hir_outlives_parameter_and_source_names_without_changing_wire() {
    let program = {
        let source = String::from(
            r#"fn prepare(node: string) = bind(tmp: node, body: ui.focus(node_id: tmp))
            fn main() = all(a: prepare(node: "a"), b: prepare(node: "b"))"#,
        );
        lower(&parse(&source)).unwrap()
    };
    let wire = serde_json::to_vec(&program.function.effect).unwrap();
    let restored: Effect = serde_json::from_slice(&wire).unwrap();
    assert_eq!(restored, program.function.effect);
    assert_eq!(serde_json::to_vec(&restored).unwrap(), wire);
    assert_eq!(
        lower(&parse(&canonical_source(&restored).unwrap())).unwrap(),
        program
    );
    authorize(&program, &CapabilitySet::new(["ui.presentation"])).unwrap();
}

#[test]
fn closed_group_aliases_and_cold_capabilities_remain_adapter_owned() {
    let program = lower(&parse(r#"fn main() = bind(g: seq(a: ui.focus(node_id: "a")), body:
        choose(when: true,
            then: bind(alias: g, body: bind(r: runtime.list(), body: field(value: member(value: alias, name: "a"), name: "node_id"))),
            otherwise: bind(alias: g, body: "b")))"#)).unwrap();
    assert!(authorize(&program, &CapabilitySet::new(["ui.presentation"])).is_err());
    let Effect::Compute { expression } = &program.function.effect else {
        panic!()
    };
    let Computation::Bind { body, .. } = expression.as_ref() else {
        panic!()
    };
    let group = GroupLocalType {
        name: "g".into(),
        members: vec![("a".into(), HostOperation::UiFocus)],
    };
    assert_eq!(
        body.validate_in_group_scope(&[], std::slice::from_ref(&group))
            .unwrap(),
        Type::Scalar(ScalarType::String)
    );
    let mut forged = group.clone();
    forged.members[0].1 = HostOperation::RuntimeList;
    assert!(body.validate_in_group_scope(&[], &[forged]).is_err());
    assert!(
        body.validate_in_group_scope(&[("g".into(), Type::Structured)], &[group])
            .is_err()
    );
}
