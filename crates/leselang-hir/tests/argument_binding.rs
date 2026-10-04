use leselang_hir::computation::Computation;
use leselang_hir::{CanonicalSourceError, Effect, Type, authorize, canonical_source, lower};
use leselang_host_contract::CapabilitySet;
use leselang_syntax::parse;

#[test]
fn every_optional_filter_subset_and_source_permutation_keeps_declaration_order() {
    let names = ["environment", "cluster", "role"];
    for bits in 0u8..8 {
        let canonical = names
            .iter()
            .enumerate()
            .filter(|(index, _)| bits & (1 << index) != 0)
            .map(|(_, name)| format!(r#"{name}: concat(left: "{name}", right: "-a")"#))
            .collect::<Vec<_>>();
        let expected = lower(&parse(&format!(
            "fn main() = runtime.list({})",
            canonical.join(", ")
        )))
        .unwrap();
        for permutation in [
            [0, 1, 2],
            [0, 2, 1],
            [1, 0, 2],
            [1, 2, 0],
            [2, 0, 1],
            [2, 1, 0],
        ] {
            let submitted = permutation
                .into_iter()
                .filter(|index| bits & (1 << index) != 0)
                .map(|index| {
                    format!(
                        r#"{}: concat(left: "{}", right: "-a")"#,
                        names[index], names[index]
                    )
                })
                .collect::<Vec<_>>();
            let actual = lower(&parse(&format!(
                "fn main() = runtime.list({})",
                submitted.join(", ")
            )))
            .unwrap();
            assert_eq!(actual, expected);
            let source = canonical_source(&actual.function.effect).unwrap();
            assert_eq!(lower(&parse(&source)).unwrap(), expected);
            if let Effect::Compute { expression } = &actual.function.effect {
                let Computation::Call { arguments, .. } = expression.as_ref() else {
                    panic!("expected call")
                };
                assert_eq!(
                    arguments
                        .iter()
                        .map(|arg| arg.name.as_str())
                        .collect::<Vec<_>>(),
                    names
                        .into_iter()
                        .enumerate()
                        .filter(|(index, _)| bits & (1 << index) != 0)
                        .map(|(_, name)| name)
                        .collect::<Vec<_>>()
                );
            }
        }
    }
}

#[test]
fn noncanonical_hir_argument_order_is_rejected_not_silently_normalized() {
    let mut program = lower(&parse(
        r#"fn main() = ui.assert_text(
        expected: concat(left: "re", right: "ady"),
        node_id: concat(left: "tar", right: "get"))"#,
    ))
    .unwrap();
    let Effect::Compute { expression } = &mut program.function.effect else {
        panic!("expected computation")
    };
    let Computation::Call { arguments, .. } = expression.as_mut() else {
        panic!("expected call")
    };
    arguments.reverse();
    assert!(matches!(
        expression.validate_in_scope(&[]),
        Err(CanonicalSourceError::RoundTripMismatch)
    ));
    let wire = serde_json::to_vec(&program.function.effect).unwrap();
    let decoded: Effect = serde_json::from_slice(&wire).unwrap();
    assert_eq!(serde_json::to_vec(&decoded).unwrap(), wire);
    assert!(matches!(
        canonical_source(&decoded),
        Err(CanonicalSourceError::RoundTripMismatch)
    ));
    let error = authorize(&program, &CapabilitySet::new(["ui.presentation"])).unwrap_err();
    assert_eq!(error.code, "LSH1405");
    assert_eq!(error.message, "invalid computation HIR");
    assert_eq!(program.function.result_type, Type::UiAssertText);
}
