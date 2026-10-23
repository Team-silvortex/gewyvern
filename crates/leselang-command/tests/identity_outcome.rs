use leselang_command::continuation_identity::{
    MAX_EFFECT_SEQUENCE, canonical_continuation_sequence, canonical_merge_sequence,
    continuation_age_order, valid_continuation_token,
};
use leselang_command::{
    BranchCompletion, BranchOutcome, Cancellation, CancellationReason, ContinuationToken,
};
use std::cmp::Ordering;

#[test]
fn raw_identity_construction_moves_storage_without_claiming_admission() {
    let raw = String::from("not/an/admitted/token");
    let pointer = raw.as_ptr();
    let token = ContinuationToken::from_wire(raw);
    assert_eq!(token.as_str().as_ptr(), pointer);
    assert!(!valid_continuation_token(&token));
    assert_eq!(canonical_continuation_sequence(&token), None);
    let bytes = serde_json::to_vec(&token).unwrap();
    let decoded: ContinuationToken = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded, token);
    assert_eq!(
        format!("{decoded:?}"),
        "ContinuationToken(\"not/an/admitted/token\")"
    );
}

#[test]
fn lexical_identity_bounds_preserve_ascii_and_byte_rules() {
    let mut comparisons = 0;
    for byte in 0..=127_u8 {
        for size in [0, 1, 127, 128, 129] {
            let text = (byte as char).to_string().repeat(size);
            let token = ContinuationToken::from_wire(text);
            let expected = (1..=128).contains(&size)
                && (byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
            assert_eq!(valid_continuation_token(&token), expected);
            comparisons += 1;
        }
    }
    assert_eq!(comparisons, 640);
    for text in ["\u{7fbd}\u{86c7}", "a/b", "a b", "a\nb"] {
        assert!(!valid_continuation_token(&ContinuationToken::from_wire(
            text.into()
        )));
    }
}

#[test]
fn canonical_sequences_and_legacy_age_order_are_different_observations() {
    for prefix in ["continuation-", "merge-"] {
        for (suffix, expected) in [
            ("0".to_string(), None),
            ("00".into(), None),
            ("1".into(), Some(1)),
            ("01".into(), None),
            ("+1".into(), None),
            (MAX_EFFECT_SEQUENCE.to_string(), Some(MAX_EFFECT_SEQUENCE)),
            ((MAX_EFFECT_SEQUENCE + 1).to_string(), None),
            (u64::MAX.to_string(), None),
            ("18446744073709551616".into(), None),
        ] {
            let token = ContinuationToken::from_wire(format!("{prefix}{suffix}"));
            assert_eq!(
                canonical_continuation_sequence(&token),
                if prefix == "continuation-" {
                    expected
                } else {
                    None
                }
            );
            assert_eq!(
                canonical_merge_sequence(&token),
                if prefix == "merge-" { expected } else { None }
            );
        }
    }
    let token = |s: &str| ContinuationToken::from_wire(s.into());
    assert_eq!(
        continuation_age_order(&token("continuation-2"), &token("continuation-10")),
        Ordering::Less
    );
    assert!(token("continuation-2") > token("continuation-10"));
    assert_eq!(
        continuation_age_order(&token("continuation-01"), &token("continuation-1")),
        Ordering::Equal
    );
    assert_ne!(token("continuation-01"), token("continuation-1"));
    assert_eq!(
        canonical_continuation_sequence(&token("continuation-01")),
        None
    );
    assert_eq!(
        continuation_age_order(&token("continuation-0"), &token("legacy")),
        Ordering::Less
    );
}

#[test]
fn terminal_data_keeps_concrete_variant_imports_and_is_not_a_receipt() {
    use leselang_command::BranchOutcome::Cancelled;
    use leselang_command::CancellationReason::DeadlineExceeded;
    use leselang_command::terminal_outcome::BranchOutcome::Fault;

    let cancellation = Cancellation {
        continuation: ContinuationToken::from_wire(String::new()),
        reason: DeadlineExceeded,
        observed_at_ms: u64::MAX,
    };
    let completion = BranchCompletion {
        branch: String::new(),
        outcome: Cancelled(cancellation),
    };
    let bytes = serde_json::to_vec(&completion).unwrap();
    let decoded: BranchCompletion = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(decoded, completion);
    let Cancelled(cancelled) = decoded.outcome else {
        panic!("variant changed")
    };
    assert!(!valid_continuation_token(&cancelled.continuation));
    assert_eq!(cancelled.reason, CancellationReason::DeadlineExceeded);
    assert_eq!(cancelled.observed_at_ms, u64::MAX);
    // Preserve the original permissive struct field policy; do not normalize it
    // into a stricter wire contract during the ownership move.
    let cancellation: Cancellation = serde_json::from_str(
        r#"{"continuation":"legacy","reason":"requested","observed_at_ms":1,"extra":true}"#,
    )
    .unwrap();
    assert_eq!(cancellation.reason, CancellationReason::Requested);
    assert!(matches!(
        Fault(leselang_runtime_core::Fault {
            code: String::new(),
            message: String::new(),
        }),
        BranchOutcome::Fault(_)
    ));
}
