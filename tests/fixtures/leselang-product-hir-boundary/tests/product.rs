use leselang_product_hir_boundary::{
    CapabilitySet, Effect, HirProgram, Type, authorize, canonical_source, lower,
};
use leselang_runtime_core::ScalarType;
use leselang_syntax::parse;

#[test]
fn foreign_product_types_reuse_the_language_ir_without_an_orphan_impl() {
    let program = lower(&parse(
        "fn main() = bind(x: 1, body: add(left: x, right: 2))",
    ))
    .unwrap();
    assert_eq!(
        program.function.result_type,
        Type::Scalar(ScalarType::Integer)
    );
    let Effect::Compute { expression } = &program.function.effect else {
        panic!("expected scalar computation");
    };
    let native: &leselang_hir::ir::Computation<_, _, Effect, Type> = expression;
    assert!(native.is_pure());
    native.validate_structure().unwrap();
    assert_eq!(
        native.validate_in_scope(&[]),
        Ok(Type::Scalar(ScalarType::Integer))
    );
    assert!(native.required_capabilities().is_empty());
}

#[test]
fn foreign_product_lowering_preserves_canonical_and_wire_round_trips() {
    for source in [
        "fn main() = runtime.list()",
        "fn main() = runtime.inspect(runtime_id: \"runtime-a\")",
        "fn main() = ui.focus(node_id: \"runtime-card\")",
        "fn main() = bind(x: 1, body: add(left: x, right: 2))",
        "fn main() = loop(n: 0, while: false, next: n, limit: 0)",
        "fn main() = fold(n: 0, items: strings(), item: \"entry\", next: n, limit: 0)",
        "fn main() = all(first: runtime.list(), second: runtime.list())",
        "fn main() = seq(first: runtime.list(), second: runtime.list())",
        "fn plus(x: integer) = add(left: x, right: 1)\nfn main() = plus(x: 2)",
    ] {
        let program = lower(&parse(source)).unwrap_or_else(|e| panic!("{source}: {e:?}"));
        let canonical = canonical_source(&program.function.effect).unwrap();
        assert_eq!(lower(&parse(&canonical)).unwrap(), program, "{source}");
        let wire = serde_json::to_string(&program).unwrap();
        assert_eq!(serde_json::from_str::<HirProgram>(&wire).unwrap(), program);
    }
}

#[test]
fn foreign_product_keeps_authority_checks_in_the_host_profile() {
    let mut program = lower(&parse("fn main() = runtime.list()")).unwrap();
    assert_eq!(
        authorize(&program, &CapabilitySet::default())
            .unwrap_err()
            .code,
        "LSH2001"
    );
    authorize(
        &program,
        &CapabilitySet::new([leselang_host_contract::CAPABILITY_RUNTIME_READ]),
    )
    .unwrap();
    program.function.required_capabilities.clear();
    assert_eq!(
        authorize(&program, &CapabilitySet::default())
            .unwrap_err()
            .code,
        "LSH2002"
    );
}

#[test]
fn foreign_product_still_checks_cold_operands_and_form_specific_local_names() {
    for source in [
        "fn main() = and(left: false, right: missing)",
        "fn main() = recover(value: 1, fallback: missing)",
        "fn main() = loop(n: 0, while: false, next: missing, limit: 0)",
        "fn main() = fold(n: 0, items: strings(), item: \"entry\", next: missing, limit: 0)",
        "fn main() = loop(next: 0, while: false, next: next, limit: 0)",
        "fn main() = bind(x: 1, body: bind(x: 2, body: x))",
    ] {
        assert!(lower(&parse(source)).is_err(), "{source}");
    }
}
