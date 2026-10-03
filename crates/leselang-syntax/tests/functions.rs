use leselang_syntax::{MAX_FUNCTION_PARAMETERS, MAX_FUNCTIONS, SyntaxTree, format, parse};

#[test]
fn typed_helpers_preserve_source_order_and_select_main_as_entry() {
    for source in [
        "fn bump(value: integer) = add(left: value, right: 1)\nfn main() = bump(value: 7)",
        "fn main() = bump(value: 7)\nfn bump(value: integer) = add(left: value, right: 1)",
        "fn identity(value: none, enabled: boolean, text: string,) = text\nfn main() = identity(value: none, enabled: true, text: \"ok\")",
    ] {
        let tree = parse(source);
        assert!(tree.diagnostics.is_empty(), "{:?}", tree.diagnostics);
        assert_eq!(tree.reconstruct().as_deref(), Some(source));
        assert_eq!(tree.function.as_ref().unwrap().name, "main");
        assert_eq!(tree.helpers.len(), 1);
        let canonical = format(&tree).unwrap();
        assert_eq!(format(&parse(&canonical)).unwrap(), canonical);
        assert_eq!(
            serde_json::from_slice::<SyntaxTree>(&serde_json::to_vec(&tree).unwrap()).unwrap(),
            tree
        );
        let first_name = source
            .split_whitespace()
            .nth(1)
            .unwrap()
            .split('(')
            .next()
            .unwrap();
        assert!(canonical.starts_with(&format!("fn {first_name}(")));
    }
}

#[test]
fn legacy_single_function_encoding_and_format_are_unchanged() {
    let tree = parse("fn main() = runtime.list()");
    assert!(tree.helpers.is_empty());
    assert!(tree.function.as_ref().unwrap().parameters.is_empty());
    let encoded = serde_json::to_value(&tree).unwrap();
    assert!(encoded.get("helpers").is_none());
    assert!(encoded["function"].get("parameters").is_none());
    assert_eq!(format(&tree).unwrap(), "fn main() = runtime.list()\n");
    assert_eq!(serde_json::from_value::<SyntaxTree>(encoded).unwrap(), tree);
}

#[test]
fn function_and_parameter_limits_reject_before_expansion() {
    let declarations = |count: usize| {
        (0..count)
            .map(|index| format!("fn f{index}() = {index}\n"))
            .collect::<String>()
    };
    assert!(parse(&declarations(MAX_FUNCTIONS)).diagnostics.is_empty());
    assert!(
        parse(&declarations(MAX_FUNCTIONS + 1))
            .diagnostics
            .iter()
            .any(|error| error.code == "LSE1201")
    );
    let signature = |count: usize| {
        format!(
            "fn main({}) = 0",
            (0..count)
                .map(|index| format!("p{index}: integer"))
                .collect::<Vec<_>>()
                .join(", ")
        )
    };
    assert!(
        parse(&signature(MAX_FUNCTION_PARAMETERS))
            .diagnostics
            .is_empty()
    );
    assert!(
        parse(&signature(MAX_FUNCTION_PARAMETERS + 1))
            .diagnostics
            .iter()
            .any(|error| error.code == "LSE1202")
    );
}

#[test]
fn invalid_signatures_and_trailing_source_have_bounded_diagnostics() {
    for source in [
        "fn main(x integer) = x",
        "fn main(x: ) = x",
        "fn main() = 1 garbage",
        "fn main() = 1\nfn bad(x: integer = x",
        "fn main() = 1\nfn bad() = @",
    ] {
        let tree = parse(source);
        assert!(!tree.diagnostics.is_empty(), "{source}");
        assert!(format(&tree).is_err());
        for error in tree.diagnostics {
            assert!(error.span.start <= error.span.end);
            assert!(error.span.end <= source.len());
            assert!(source.is_char_boundary(error.span.start));
            assert!(source.is_char_boundary(error.span.end));
        }
    }
}

#[test]
fn serialized_helpers_validate_parameter_spans_and_declaration_counts() {
    let tree = parse("fn f(n: integer) = n\nfn main() = f(n: 2)");
    let mut encoded = serde_json::to_value(&tree).unwrap();
    encoded["helpers"][0]["parameters"][0]["span"]["end"] = usize::MAX.into();
    assert!(serde_json::from_value::<SyntaxTree>(encoded).is_err());
    let mut oversized = tree.clone();
    oversized.helpers = vec![tree.helpers[0].clone(); MAX_FUNCTIONS];
    assert!(
        serde_json::from_value::<SyntaxTree>(serde_json::to_value(&oversized).unwrap()).is_err()
    );
    assert!(format(&oversized).is_err());
    let mut oversized = tree;
    oversized.helpers[0].parameters =
        vec![oversized.helpers[0].parameters[0].clone(); MAX_FUNCTION_PARAMETERS + 1];
    assert!(
        serde_json::from_value::<SyntaxTree>(serde_json::to_value(&oversized).unwrap()).is_err()
    );
    assert!(format(&oversized).is_err());
}
