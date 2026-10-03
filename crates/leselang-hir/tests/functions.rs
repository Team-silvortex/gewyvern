use leselang_hir::computation::ScalarType;
use leselang_hir::{Effect, HirProgram, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::{format, parse};

fn compile(source: &str) -> HirProgram {
    let tree = parse(source);
    assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
    lower(&tree).unwrap()
}

#[test]
fn scalar_helpers_inline_to_existing_hir_and_canonical_source() {
    for (source, ty) in [
        (
            "fn bump(n: integer) = add(left: n, right: 1)\nfn main() = bump(n: 7)",
            ScalarType::Integer,
        ),
        (
            "fn main() = same(text: \"ok\")\nfn same(text: string) = text",
            ScalarType::String,
        ),
        (
            "fn flip(value: boolean) = not(value: value)\nfn main() = flip(value: false)",
            ScalarType::Boolean,
        ),
        (
            "fn nil(value: none) = value\nfn main() = nil(value: none)",
            ScalarType::None,
        ),
        (
            "fn constant() = 7\nfn main() = constant()",
            ScalarType::Integer,
        ),
        (
            "fn twice(value: integer) = bump(n: bump(n: value))\nfn main() = twice(value: 7)\nfn bump(n: integer) = add(left: n, right: 1)",
            ScalarType::Integer,
        ),
    ] {
        let program = compile(source);
        assert_eq!(program.function.name, "main");
        assert_eq!(program.function.result_type, Type::Scalar(ty));
        assert!(program.function.required_capabilities.is_empty());
        authorize(&program, &CapabilitySet::default()).unwrap();
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(compile(&canonical), program);
        let formatted = format(&parse(source)).unwrap();
        assert_eq!(compile(&formatted), program);
    }
}

#[test]
fn invalid_declarations_are_checked_even_when_unused() {
    for helper in [
        "fn f(n: signed) = n",
        "fn f(n: integer, n: integer) = n",
        "fn f(body: integer) = body",
        "fn f(true: integer) = 0",
        "fn add(n: integer) = n",
        "fn f(n: integer) = n\nfn f() = 2",
        "fn main() = 0",
    ] {
        let errors = lower(&parse(&format!("fn main() = 1\n{helper}"))).unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "LSH1501"),
            "{helper}: {errors:?}"
        );
    }
    assert!(lower(&parse("fn f(n: integer) = n")).is_err());
    assert!(lower(&parse("fn main(n: integer) = n")).is_err());
    for helper in [
        "fn f() = runtime.list()",
        "fn f() = bind(r: ui.focus(node_id: \"a\"), body: 0)",
        "fn f() = choose(when: true, then: 0, otherwise: bind(r: runtime.list(), body: 1))",
    ] {
        let errors = lower(&parse(&format!("fn main() = 1\n{helper}"))).unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "LSH1503"),
            "{errors:?}"
        );
    }
}

#[test]
fn direct_mutual_cold_and_unused_recursion_are_rejected() {
    for source in [
        "fn f() = f()\nfn main() = 0",
        "fn a() = b()\nfn b() = a()\nfn main() = 0",
        "fn f() = choose(when: true, then: 0, otherwise: f())\nfn main() = 0",
        "fn f() = main()\nfn main() = 0",
    ] {
        assert!(
            lower(&parse(source))
                .unwrap_err()
                .iter()
                .any(|error| error.code == "LSH1502")
        );
    }
}

#[test]
fn helper_arguments_require_exact_names_scalar_types_and_purity() {
    for call in [
        "f()",
        "f(other: 1)",
        "f(n: 1, n: 2)",
        "f(n: 1, extra: 2)",
        "f(n: \"1\")",
        "f(n: true)",
        "f(n: none)",
        "f(n: runtime.list())",
        "f(n: bind(r: runtime.list(), body: 1))",
        "choose(when: true, then: 1, otherwise: f(n: false))",
    ] {
        let source = format!("fn f(n: integer) = 0\nfn main() = {call}");
        let errors = lower(&parse(&source)).unwrap_err();
        assert!(
            errors.iter().any(|error| error.code == "LSH1504"),
            "{call}: {errors:?}"
        );
        assert!(
            errors
                .iter()
                .all(|error| error.span.is_some_and(|span| span.end <= source.len()))
        );
    }
    assert!(
        lower(&parse(
            "fn f() = caller\nfn main() = bind(caller: 2, body: f())"
        ))
        .unwrap_err()
        .iter()
        .any(|error| error.code == "LSH1403")
    );
}

#[test]
fn helpers_compose_with_host_arguments_results_and_group_preparation() {
    for body in [
        "ui.focus(node_id: target(text: \"a\"))",
        "bind(r: ui.focus(node_id: \"a\"), body: target(text: field(value: r, name: \"node_id\")))",
        "seq(first: ui.focus(node_id: target(text: \"a\")), second: ui.focus(node_id: target(text: \"b\")))",
        "repeat(times: 2, body: ui.focus(node_id: target(text: \"a\")))",
        "all(first: ui.focus(node_id: target(text: \"a\")), second: ui.focus(node_id: target(text: \"b\")))",
    ] {
        let program = compile(&format!(
            "fn target(text: string) = concat(left: \"runtime-\", right: text)\nfn main() = {body}"
        ));
        assert_eq!(program.function.required_capabilities, ["ui.presentation"]);
        assert!(authorize(&program, &CapabilitySet::default()).is_err());
        assert_eq!(
            compile(&canonical_source(&program.function.effect).unwrap()),
            program
        );
    }
}

#[test]
fn expanded_node_and_depth_limits_stop_helper_amplification() {
    let mut source = "fn f0() = 1\n".to_string();
    for index in 1..12 {
        source.push_str(&format!(
            "fn f{index}() = add(left: f{}(), right: f{}())\n",
            index - 1,
            index - 1
        ));
    }
    source.push_str("fn main() = f11()");
    assert!(
        lower(&parse(&source))
            .unwrap_err()
            .iter()
            .any(|error| error.code == "LSH1405")
    );

    let mut body = "n".to_string();
    for _ in 0..15 {
        body = format!("not(value: {body})");
    }
    let source = format!("fn f(n: boolean) = {body}\nfn main() = not(value: f(n: true))");
    assert!(parse(&source).diagnostics.is_empty());
    assert!(
        lower(&parse(&source))
            .unwrap_err()
            .iter()
            .any(|error| error.code == "LSH1405")
    );

    let source = "fn f0(n: integer) = n\nfn f1(n: integer) = f0(n: n)\nfn main() = repeat(times: 64, body: ui.focus(node_id: to_string(value: f1(n: 1))))";
    let program = compile(source);
    assert!(matches!(program.function.effect, Effect::Compute { .. }));
}

#[test]
fn public_ast_mutations_cannot_bypass_function_source_preflight() {
    let mut tree = parse("fn f(n: integer) = n\nfn main() = f(n: 1)");
    let leselang_syntax::Expression::Call { arguments, .. } =
        &mut tree.function.as_mut().unwrap().body
    else {
        panic!("expected helper call");
    };
    *arguments = vec![arguments[0].clone(); leselang_hir::computation::MAX_COMPUTATION_NODES + 1];
    let errors = lower(&tree).unwrap_err();
    assert!(errors.iter().any(|error| error.code == "LSH1405"));
}

#[test]
fn unused_helpers_do_not_change_literal_host_effect_bytes() {
    for body in [
        "ui.focus(node_id: \"a\")",
        "runtime.list()",
        "seq(first: ui.focus(node_id: \"a\"), second: ui.focus(node_id: \"b\"))",
    ] {
        let plain = compile(&format!("fn main() = {body}"));
        let module = compile(&format!("fn constant() = 7\nfn main() = {body}"));
        assert_eq!(module, plain);
        assert_eq!(
            compile(&canonical_source(&module.function.effect).unwrap()),
            module
        );
    }
}

#[test]
fn string_literal_amplification_respects_canonical_source_bounds_before_execution() {
    let mut source = format!("fn f0() = \"{}\"\n", "x".repeat(4096));
    for index in 1..7 {
        source.push_str(&format!(
            "fn f{index}() = concat(left: f{}(), right: f{}())\n",
            index - 1,
            index - 1
        ));
    }
    source.push_str("fn main() = f6()");
    assert!(source.len() < leselang_syntax::MAX_SOURCE_BYTES);
    assert!(parse(&source).diagnostics.is_empty());
    let errors = lower(&parse(&source)).unwrap_err();
    assert!(errors.iter().any(|error| error.code == "LSH1405"));
    assert!(errors.iter().all(|error| error.span.is_some()));
}
