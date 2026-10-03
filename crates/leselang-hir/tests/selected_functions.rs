use leselang_hir::computation::ScalarType;
use leselang_hir::{Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::{format, parse};

fn compile(source: &str) -> leselang_hir::HirProgram {
    let program = lower(&parse(source)).unwrap();
    assert_eq!(
        lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
        program
    );
    assert_eq!(
        lower(&parse(&format(&parse(source)).unwrap())).unwrap(),
        program
    );
    program
}

#[test]
fn choices_of_data_returning_atomic_and_group_helpers_join_the_caller() {
    let program = compile(
        r#"fn single(node: string) = bind(result: ui.assert_text(node_id: node, expected: "ready"), body: field(value: result, name: "expected"))
        fn gathered(node: string) = bind(group: seq(first: ui.assert_text(node_id: node, expected: "re"), second: ui.assert_text(node_id: "b", expected: "ady")), body: concat(left: field(value: member(value: group, name: "first"), name: "expected"), right: field(value: member(value: group, name: "second"), name: "expected")))
        fn main() = bind(answer: choose(when: false, then: single(node: "a"), otherwise: gathered(node: "a")), body: bind(written: ui.set_form_value(node_id: "form", field: "answer", value: answer), body: eq(left: field(value: written, name: "value"), right: "ready")))"#,
    );
    assert_eq!(
        program.function.result_type,
        Type::Scalar(ScalarType::Boolean)
    );
    assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
}

#[test]
fn pure_fallbacks_and_nested_named_choices_preserve_all_bounded_data_types() {
    for (returned, fallback, ty) in [
        (
            r#"field(value: result, name: "expected")"#,
            "\"fallback\"",
            ScalarType::String,
        ),
        ("true", "false", ScalarType::Boolean),
        ("7", "0", ScalarType::Integer),
        ("none", "none", ScalarType::None),
        (
            r#"optional_string(value: field(value: result, name: "expected"))"#,
            "optional_string(value: none)",
            ScalarType::OptionalString,
        ),
        (
            r#"split(left: field(value: result, name: "expected"), right: ",")"#,
            r#"strings(first: "fallback")"#,
            ScalarType::StringList,
        ),
    ] {
        let source = format!(
            r#"fn work(node: string) = bind(result: ui.assert_text(node_id: node, expected: "a,b"), body: {returned})
            fn main() = bind(answer: choose(otherwise: {fallback}, then: choose(otherwise: {fallback}, then: work(node: "a"), when: false), when: true), body: answer)"#
        );
        assert_eq!(compile(&source).function.result_type, Type::Scalar(ty));
    }
}

#[test]
fn pure_branch_locals_and_helper_locals_cannot_capture_the_caller() {
    compile(
        r#"fn work() = bind(scratch: "inner", body: bind(result: ui.focus(node_id: "a"), body: scratch))
        fn main() = bind(scratch: "outer", body: bind(answer: choose(when: false, then: work(), otherwise: bind(scratch2: "fallback", body: scratch2)), body: bind(scratch2: "caller", body: concat(left: scratch, right: concat(left: answer, right: scratch2)))))"#,
    );
    assert!(lower(&parse(r#"fn work() = bind(scratch: "inner", body: bind(result: ui.focus(node_id: "a"), body: scratch))
        fn main() = bind(answer: choose(when: true, then: work(), otherwise: "fallback"), body: scratch)"#)).is_err());
}

#[test]
fn called_cold_helpers_require_authority_and_all_definitions_remain_checked() {
    let program = compile(
        r#"fn count() = bind(result: runtime.list(), body: field(value: result, name: "count"))
        fn main() = bind(answer: choose(when: false, then: count(), otherwise: 0), body: ui.focus(node_id: to_string(value: answer)))"#,
    );
    assert_eq!(
        program.function.required_capabilities,
        ["runtime.read", "ui.presentation"]
    );
    assert!(authorize(&program, &CapabilitySet::new(["ui.presentation"])).is_err());
    authorize(
        &program,
        &CapabilitySet::new(["runtime.read", "ui.presentation"]),
    )
    .unwrap();
    for source in [
        r#"fn invalid() = bind(result: ui.focus(node_id: "bad node"), body: true)
            fn main() = bind(answer: choose(when: false, then: invalid(), otherwise: false), body: answer)"#,
        r#"fn recursive() = bind(result: ui.focus(node_id: "a"), body: recursive())
            fn main() = bind(answer: choose(when: false, then: recursive(), otherwise: false), body: answer)"#,
    ] {
        assert!(lower(&parse(source)).is_err());
    }
}

#[test]
fn conditional_call_join_does_not_admit_arbitrary_nested_effects_or_impure_operands() {
    let declarations = r#"fn work(node: string) = bind(result: ui.focus(node_id: node), body: true)
        fn echo(value: boolean) = value
        "#;
    for body in [
        r#"bind(answer: choose(when: true, then: work(node: "a"), otherwise: bind(hidden: ui.focus(node_id: "b"), body: true)), body: answer)"#,
        r#"bind(answer: bind(local: "a", body: choose(when: true, then: work(node: local), otherwise: false)), body: answer)"#,
        r#"bind(answer: choose(when: work(node: "a"), then: work(node: "a"), otherwise: false), body: answer)"#,
        r#"echo(value: choose(when: true, then: work(node: "a"), otherwise: false))"#,
        r#"recover(value: choose(when: true, then: work(node: "a"), otherwise: false), fallback: false)"#,
        r#"loop(n: false, while: false, next: choose(when: true, then: work(node: "a"), otherwise: false), limit: 0)"#,
        r#"seq(first: choose(when: true, then: work(node: "a"), otherwise: false))"#,
    ] {
        assert!(
            lower(&parse(&format!("{declarations}fn main() = {body}"))).is_err(),
            "{body}"
        );
    }
}

#[test]
fn every_selected_and_cold_return_reserves_the_caller_before_cloning() {
    let mut definitions = r#"fn f0() = bind(result: ui.focus(node_id: "a"), body: 0)
        "#
    .to_string();
    for index in 1..=5 {
        definitions.push_str(&format!(
            "fn f{index}() = choose(when: true, then: f{}(), otherwise: f{}())\n",
            index - 1,
            index - 1
        ));
    }
    compile(&format!(
        "{definitions}fn main() = bind(answer: choose(when: true, then: f5(), otherwise: f5()), body: answer)"
    ));
    for body in [
        format!("\"{}\"", "x".repeat(4096)),
        format!(
            "strings({})",
            (0..64)
                .map(|index| format!("i{index}: \"\""))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ] {
        let source = format!(
            "{definitions}fn main() = bind(answer: choose(when: true, then: f5(), otherwise: f5()), body: {body})"
        );
        assert!(
            lower(&parse(&source))
                .unwrap_err()
                .iter()
                .any(|error| error.code == "LSH1405")
        );
    }
}
