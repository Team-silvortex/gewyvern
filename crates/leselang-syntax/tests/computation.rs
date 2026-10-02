use leselang_syntax::{Expression, SyntaxTree, TokenKind, format, parse};

#[test]
fn literals_and_local_references_are_lossless_and_canonical() {
    for source in [
        "fn main() = true",
        "fn main() = false",
        "fn main() = 42",
        "fn main() = none",
        "fn main() = \"hello\"",
        "fn main() = local",
        "fn true() = all(true: runtime.list(), false: runtime.list())",
        "// comment\nfn main() = bind(count: 3, body: choose(when: true, then: count /*not a comment*/, otherwise: 0))",
        "fn main() = bind(count: 3, body: add(left: count // reference\n, right: 2))",
    ] {
        let tree = parse(source);
        assert_eq!(tree.reconstruct().as_deref(), Some(source));
        let bytes = serde_json::to_vec(&tree).unwrap();
        assert_eq!(serde_json::from_slice::<SyntaxTree>(&bytes).unwrap(), tree);
        if tree.diagnostics.is_empty() {
            let canonical = format(&tree).unwrap();
            assert_eq!(format(&parse(&canonical)).unwrap(), canonical);
        }
    }
    assert!(matches!(
        parse("fn main() = false").function.unwrap().body,
        Expression::Boolean { value: false, .. }
    ));
    assert!(
        parse("fn main() = true")
            .tokens
            .iter()
            .any(|token| token.kind == TokenKind::Boolean)
    );
    assert!(
        matches!(parse("fn main() = local").function.unwrap().body, Expression::Reference { name, .. } if name == "local")
    );
}

#[test]
fn computation_uses_the_existing_call_depth_fence() {
    let mut expression = "true".to_string();
    for _ in 0..leselang_syntax::MAX_CALL_DEPTH {
        expression = format!("not(value: {expression})");
    }
    let source = format!("fn main() = {expression}");
    assert!(parse(&source).diagnostics.is_empty());
    assert!(
        !parse(&format!("fn main() = not(value: {expression})"))
            .diagnostics
            .is_empty()
    );
    for source in [
        "fn main() = true()",
        "fn main() = add(left: 1, right:)",
        "fn main() = a.b",
    ] {
        assert!(!parse(source).diagnostics.is_empty(), "{source}");
    }
}
