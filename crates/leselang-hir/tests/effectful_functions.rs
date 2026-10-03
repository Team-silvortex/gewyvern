use leselang_hir::computation::ScalarType;
use leselang_hir::{Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::{format, parse};

fn compile(source: &str) -> leselang_hir::HirProgram {
    let program = lower(&parse(source)).unwrap();
    assert_eq!(
        lower(&parse(&canonical_source(&program.function.effect).unwrap())).unwrap(),
        program
    );
    let formatted = format(&parse(source)).unwrap();
    assert_eq!(lower(&parse(&formatted)).unwrap(), program);
    program
}

#[test]
fn host_functions_return_atomic_results_and_compose_through_explicit_bind() {
    for (source, ty) in [
        (
            r#"fn focus(node: string) = ui.focus(node_id: node)
            fn main() = focus(node: "a")"#,
            Type::UiFocus,
        ),
        (
            r#"fn focus(node: string) = ui.focus(node_id: node)
            fn main() = bind(r: focus(node: "a"), body: field(value: r, name: "node_id"))"#,
            Type::Scalar(ScalarType::String),
        ),
        (
            r#"fn read(node: string) = bind(r: ui.assert_text(node_id: node, expected: "ready"), body: field(value: r, name: "expected"))
            fn main() = bind(value: read(node: "status"), body: ui.set_form_value(node_id: "form", field: "value", value: value))"#,
            Type::UiSetFormValue,
        ),
        (
            r#"fn stage(node: string) = bind(r: ui.focus(node_id: node), body: ui.assert_visible(node_id: field(value: r, name: "node_id")))
            fn main() = bind(visible: stage(node: "a"), body: field(value: visible, name: "node_id"))"#,
            Type::Scalar(ScalarType::String),
        ),
        (
            r#"fn text() = bind(r: ui.assert_text(node_id: "a", expected: "x,y"), body: split(left: field(value: r, name: "expected"), right: ","))
            fn main() = bind(parts: text(), body: join(left: parts, right: ";"))"#,
            Type::Scalar(ScalarType::String),
        ),
    ] {
        let program = compile(source);
        assert_eq!(program.function.result_type, ty);
        assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
        assert!(authorize(&program, &CapabilitySet::default()).is_err());
        authorize(&program, &CapabilitySet::new(["ui.presentation"])).unwrap();
    }
}

#[test]
fn conditional_function_returns_join_the_caller_on_every_normal_path() {
    let source = r#"fn probe(node: string, skip: boolean) = choose(when: skip, then: false, otherwise:
        bind(r: ui.assert_child_count(node_id: node, count: "3"), body: ge(left: field(value: r, name: "count"), right: 3)))
        fn main() = bind(ok: probe(node: "rows", skip: false), body:
            bind(written: ui.set_form_value(node_id: "form", field: "ready", value: to_string(value: ok)), body: ok))"#;
    let program = compile(source);
    assert_eq!(
        program.function.result_type,
        Type::Scalar(ScalarType::Boolean)
    );
    let Effect::Compute { expression } = program.function.effect else {
        panic!()
    };
    assert!(expression.is_result_flow());
    assert_eq!(expression.atomic_flow_bound(), Some(2));
}

#[test]
fn nested_host_functions_and_returning_groups_preserve_named_projections() {
    for source in [
        r#"fn focus(node: string) = ui.focus(node_id: node)
            fn found(node: string) = bind(r: focus(node: node), body: field(value: r, name: "node_id"))
            fn main() = bind(value: found(node: "a"), body: ui.focus(node_id: value))"#,
        r#"fn rows(node: string) = seq(first: ui.focus(node_id: node), second: ui.assert_visible(node_id: node))
            fn main() = bind(group: rows(node: "a"), body: field(value: member(value: group, name: "second"), name: "node_id"))"#,
        r#"fn found(node: string) = bind(group: all(first: ui.assert_text(node_id: node, expected: "x"), second: ui.assert_text(node_id: "b", expected: "y")), body: concat(left: field(value: member(value: group, name: "first"), name: "expected"), right: field(value: member(value: group, name: "second"), name: "expected")))
            fn main() = bind(value: found(node: "a"), body: ui.set_form_value(node_id: "form", field: "value", value: value))"#,
    ] {
        compile(source);
    }
}

#[test]
fn all_function_locals_are_hygienic_including_results_groups_and_folds() {
    let source = r#"fn aggregate(node: string) = bind(result: ui.assert_text(node_id: node, expected: "2,3"), body:
        fold(total: 0, items: split(left: field(value: result, name: "expected"), right: ","), item: "part", next: add(left: total, right: parse_integer(value: part)), limit: 2))
        fn main() = bind(_lf0: "outer", body: bind(result: 7, body: bind(total: 8, body: bind(part: "outer", body:
            bind(first: aggregate(node: "a"), body: bind(second: aggregate(node: "b"), body: add(left: first, right: second)))))))"#;
    let program = compile(source);
    assert_eq!(
        program.function.result_type,
        Type::Scalar(ScalarType::Integer)
    );
    // Quoted fold item names are declarations even when the item is unused.
    compile(
        r#"fn identity(n: integer) = n
        fn work(n: integer) = bind(r: ui.focus(node_id: "a"), body: n)
        fn main() = work(n: fold(total: 0, items: strings(a: "x"), item: "_lf0", next: identity(n: total), limit: 1))"#,
    );
}

#[test]
fn unused_host_functions_are_checked_but_called_cold_paths_require_authority() {
    let unused = compile(
        r#"fn read() = runtime.list()
        fn main() = 0"#,
    );
    assert!(unused.function.required_capabilities.is_empty());
    let cold = compile(
        r#"fn count() = bind(r: runtime.list(), body: field(value: r, name: "count"))
        fn main() = choose(when: true, then: 0, otherwise: count())"#,
    );
    assert_eq!(cold.function.required_capabilities, ["runtime.read"]);
    assert!(authorize(&cold, &CapabilitySet::default()).is_err());
    for source in [
        r#"fn bad() = ui.focus(node_id: "bad node")
            fn main() = 0"#,
        r#"fn recursive() = bind(r: ui.focus(node_id: "a"), body: recursive())
            fn main() = 0"#,
        r#"fn captures() = ui.focus(node_id: caller)
            fn main() = bind(caller: "a", body: 0)"#,
    ] {
        assert!(lower(&parse(source)).is_err());
    }
}

#[test]
fn effects_cannot_be_hidden_inside_pure_operands_parameters_loops_or_recovery() {
    let declarations = r#"fn count() = bind(r: ui.assert_child_count(node_id: "rows", count: "2"), body: field(value: r, name: "count"))
        fn focus(node: string) = ui.focus(node_id: node)
        fn same(n: integer) = n
        "#;
    for body in [
        "add(left: count(), right: 1)",
        "same(n: count())",
        "recover(value: count(), fallback: 0)",
        "loop(n: 0, while: true, next: count(), limit: 0)",
        r#"fold(n: 0, items: strings(), item: "part", next: count(), limit: 0)"#,
        r#"seq(first: count())"#,
        r#"all(first: count(), second: focus(node: "b"))"#,
        r#"repeat(times: 2, body: count())"#,
    ] {
        assert!(
            lower(&parse(&format!("{declarations}fn main() = {body}"))).is_err(),
            "{body}"
        );
    }
    // Arbitrary nested effectful expressions are not a new function-call ABI.
    assert!(
        lower(&parse(
            r#"fn main() = bind(answer: bind(r: ui.focus(node_id: "a"), body: 1), body: answer)"#
        ))
        .is_err()
    );
}

#[test]
fn continuation_cloning_cannot_bypass_node_depth_or_canonical_text_bounds() {
    let mut branching = r#"bind(r: ui.focus(node_id: "a"), body: 0)"#.to_owned();
    for _ in 0..7 {
        branching = format!("choose(when: true, then: 0, otherwise: {branching})");
    }
    let mut body = "answer".to_owned();
    for index in 0..10 {
        body = format!("bind(v{index}: 0, body: {body})");
    }
    let source = format!("fn many() = {branching}\nfn main() = bind(answer: many(), body: {body})");
    assert!(
        lower(&parse(&source))
            .unwrap_err()
            .iter()
            .any(|error| error.code == "LSH1405")
    );
    let mut branching = r#"bind(r: ui.focus(node_id: "a"), body: 0)"#.to_owned();
    for _ in 0..7 {
        branching = format!("choose(when: true, then: {branching}, otherwise: 0)");
    }
    let source = format!(
        "fn many() = {branching}\nfn main() = bind(answer: many(), body: \"{}\")",
        "x".repeat(4096)
    );
    // This small linear example remains valid; cold branches reserve caller continuations.
    compile(&source);
    let mut source = r#"fn f0() = bind(r: ui.focus(node_id: "a"), body: 0)
        "#
    .to_owned();
    for index in 1..10 {
        source.push_str(&format!(
            "fn f{index}() = choose(when: true, then: f{}(), otherwise: f{}())\n",
            index - 1,
            index - 1
        ));
    }
    source.push_str("fn main() = f9()");
    assert!(lower(&parse(&source)).is_err());
}

#[test]
fn every_cold_return_reserves_caller_nodes_and_text_before_expansion() {
    let mut definitions = r#"fn f0() = bind(r: ui.focus(node_id: "a"), body: 0)
        "#
    .to_owned();
    for index in 1..=6 {
        definitions.push_str(&format!(
            "fn f{index}() = choose(when: true, then: f{}(), otherwise: f{}())\n",
            index - 1,
            index - 1
        ));
    }
    compile(&format!("{definitions}fn main() = f6()"));
    let list = (0..64)
        .map(|index| format!("i{index}: \"\""))
        .collect::<Vec<_>>()
        .join(", ");
    for body in [
        format!("strings({list})"),
        format!("\"{}\"", "x".repeat(4096)),
    ] {
        let source = format!("{definitions}fn main() = bind(answer: f6(), body: {body})");
        assert!(
            lower(&parse(&source))
                .unwrap_err()
                .iter()
                .any(|error| error.code == "LSH1405")
        );
    }
}
