use leselang_syntax::{Expression, format, parse};

#[test]
fn repeat_counts_are_lossless_bounded_integer_literals() {
    let source = "// repeat\nfn main() = repeat(times: 3, body: ui.focus(node_id: \"field\"))";
    let tree = parse(source);
    assert!(tree.diagnostics.is_empty());
    assert_eq!(tree.reconstruct().as_deref(), Some(source));
    let Expression::Call { arguments, .. } = &tree.function.as_ref().unwrap().body else {
        panic!("expected call");
    };
    assert!(matches!(
        arguments[0].value,
        Expression::Integer { value: 3, .. }
    ));
    let canonical = format(&tree).unwrap();
    assert_eq!(format(&parse(&canonical)).unwrap(), canonical);
    let encoded = serde_json::to_vec(&tree).unwrap();
    let restored = serde_json::from_slice::<leselang_syntax::SyntaxTree>(&encoded).unwrap();
    assert_eq!(restored, tree);
}

#[test]
fn repeat_integer_syntax_rejects_ambiguous_or_overflowing_numbers() {
    for count in ["01", "-1", "+1", "1.0", "0x10", "18446744073709551616"] {
        let tree = parse(&format!(
            "fn main() = repeat(times: {count}, body: runtime.list())"
        ));
        assert!(!tree.diagnostics.is_empty(), "accepted {count}");
        assert!(format(&tree).is_err());
    }
}
