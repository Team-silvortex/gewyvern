use leselang_hir::ir::Computation;
use leselang_hir::pure_typing::{
    MAX_LOCAL_NAME_BYTES, PureType, PureTypeEnvironment, PureTypeError, TypeInferenceLimits,
    infer_pure_type, valid_local_name,
};
use leselang_runtime_core::{ScalarType, ScalarValue};

#[test]
fn ascii_start_and_continuation_alphabets_are_exact() {
    for byte in 0..=127u8 {
        let character = char::from(byte);
        let start = character.to_string();
        let expected_start = matches!(byte, b'a'..=b'z' | b'A'..=b'Z' | b'_');
        let expected_tail = expected_start || byte.is_ascii_digit();
        assert_eq!(valid_local_name(&start), expected_start, "start {byte}");
        assert_eq!(
            valid_local_name(&format!("_{character}")),
            expected_tail,
            "tail {byte}"
        );
        assert_eq!(
            valid_local_name(&format!("{character}0")),
            expected_start,
            "prefix {byte}"
        );
    }
}

#[test]
fn reserved_tokens_are_exact_and_contextual_labels_remain_caller_policy() {
    for name in ["body", "fn", "true", "false", "none"] {
        assert!(!valid_local_name(name));
        assert!(valid_local_name(&name.to_ascii_uppercase()));
        assert!(valid_local_name(&format!("_{name}")));
        assert!(valid_local_name(&format!("{name}_")));
    }
    for name in ["while", "next", "limit", "items", "item", "runtime", "ui"] {
        assert!(valid_local_name(name));
    }
}

#[test]
fn inclusive_byte_limit_and_non_ascii_rejection_do_not_normalize_input() {
    assert_eq!(MAX_LOCAL_NAME_BYTES, 64);
    assert!(!valid_local_name(""));
    for length in 1..=MAX_LOCAL_NAME_BYTES {
        assert!(valid_local_name(&"a".repeat(length)));
    }
    assert!(!valid_local_name(&"a".repeat(MAX_LOCAL_NAME_BYTES + 1)));
    for name in [
        "a-b",
        "a.b",
        " a",
        "a ",
        "a\0b",
        "\u{e9}",
        "a\u{301}",
        "\u{ff41}",
        "\u{202e}a",
    ] {
        assert!(!valid_local_name(name), "{name:?}");
    }
}

struct NoHost;

impl PureTypeEnvironment<(), ()> for NoHost {
    type Result = ();

    fn field_type(&self, _: &(), _: &()) -> Option<ScalarType> {
        None
    }

    fn member_result(&self, _: &(), _: &str, _: &()) -> Option<()> {
        None
    }
}

#[test]
fn public_name_predicate_agrees_with_ir_admission_without_replacing_scope_checks() {
    let limits = TypeInferenceLimits {
        max_nodes: 16,
        max_depth: 8,
        max_bindings: 2,
    };
    let candidates: Vec<_> = (0..=127u8)
        .map(|b| format!("a{}", char::from(b)))
        .chain(["body", "fn", "true", "false", "none", "_", "next"].map(str::to_owned))
        .chain(["a".repeat(64), "a".repeat(65)])
        .collect();
    for name in candidates {
        let ir: Computation<(), (), (), ()> = Computation::Bind {
            name: name.clone(),
            value: Box::new(Computation::Literal {
                value: ScalarValue::Integer(1),
            }),
            body: Box::new(Computation::Local { name: name.clone() }),
        };
        let expected = if valid_local_name(&name) {
            Ok(PureType::Scalar(ScalarType::Integer))
        } else {
            Err(PureTypeError::InvalidName)
        };
        assert_eq!(
            infer_pure_type(&ir, &[], &NoHost, limits),
            expected,
            "{name:?}"
        );
        if valid_local_name(&name) {
            assert_eq!(
                infer_pure_type(
                    &ir,
                    &[(&name, PureType::Scalar(ScalarType::Integer))],
                    &NoHost,
                    limits
                ),
                Err(PureTypeError::ShadowedBinding),
            );
        }
    }
}
