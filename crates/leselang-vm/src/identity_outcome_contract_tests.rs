use super::*;
use serde::de::DeserializeOwned;
use serde_json::json;

fn tokens() -> Vec<String> {
    let suffixes = [
        "".to_string(),
        "0".into(),
        "00".into(),
        "1".into(),
        "01".into(),
        "+1".into(),
        "-1".into(),
        "2".into(),
        (MAX_EFFECT_SEQUENCE - 1).to_string(),
        MAX_EFFECT_SEQUENCE.to_string(),
        (MAX_EFFECT_SEQUENCE + 1).to_string(),
        (i64::MAX as u64).to_string(),
        u64::MAX.to_string(),
        "18446744073709551616".into(),
        "1.0".into(),
        "1 ".into(),
        " 1".into(),
        "a".into(),
    ];
    let mut result: Vec<_> = ["continuation-", "merge-", "legacy-"]
        .into_iter()
        .flat_map(|prefix| {
            suffixes
                .iter()
                .map(move |suffix| format!("{prefix}{suffix}"))
        })
        .collect();
    result.extend([
        "".into(),
        "opaque".into(),
        ".".into(),
        "_".into(),
        "A-._9".into(),
        "a/b".into(),
        " x".into(),
        "x\n".into(),
        "\u{7fbd}\u{86c7}".into(),
        "x".repeat(128),
        "x".repeat(129),
    ]);
    assert_eq!(result.len(), 65);
    result
}

fn decode<T: DeserializeOwned + Serialize>(bytes: &[u8]) -> Result<Vec<u8>, String> {
    serde_json::from_slice::<T>(bytes)
        .map(|value| serde_json::to_vec(&value).unwrap())
        .map_err(|error| error.to_string())
}

fn compare_wire<T: DeserializeOwned + Serialize, F: DeserializeOwned + Serialize>(
    value: &T,
) -> usize {
    let original = serde_json::to_vec(value).unwrap();
    assert_eq!(decode::<T>(&original), Ok(original.clone()));
    for defect in 0..8 {
        let mut bytes = original.clone();
        match defect {
            1 => {
                bytes.pop();
            }
            2 => bytes.extend_from_slice(b" null"),
            3 => bytes = [b"[".as_slice(), &original, b"]"].concat(),
            4 => bytes = b"null".to_vec(),
            5..=7 => {
                let mut wire: serde_json::Value = serde_json::from_slice(&original).unwrap();
                if let Some(fields) = wire.as_object_mut() {
                    let key = fields.keys().next().unwrap().clone();
                    match defect {
                        5 => {
                            fields.insert("extra".into(), json!(true));
                        }
                        6 => {
                            bytes.pop();
                            bytes.extend_from_slice(
                                format!(",{}:", serde_json::to_string(&key).unwrap()).as_bytes(),
                            );
                            bytes.extend_from_slice(&serde_json::to_vec(&fields[&key]).unwrap());
                            bytes.push(b'}');
                        }
                        7 => {
                            fields.remove(&key);
                        }
                        _ => unreachable!(),
                    }
                    if defect != 6 {
                        bytes = serde_json::to_vec(&wire).unwrap();
                    }
                } else {
                    bytes = [b"false".as_slice(), b"{}", b"\"\xff\""][defect - 5].to_vec();
                }
            }
            _ => {}
        }
        assert_eq!(
            decode::<T>(&bytes),
            decode::<F>(&bytes),
            "defect {defect}: {original:?}"
        );
    }
    8
}

#[test]
fn identity_migration_keeps_frozen_admission_observations_and_both_orderings() {
    let tokens = tokens();
    for text in &tokens {
        let current = ContinuationToken::from_wire(text.clone());
        let old: frozen::ContinuationToken = serde_json::from_value(json!(text)).unwrap();
        assert_eq!(current.as_str(), old.as_str());
        assert_eq!(format!("{current:?}"), format!("{old:?}"));
        assert_eq!(
            valid_continuation_token(&current),
            frozen::valid_continuation_token(&old)
        );
        assert_eq!(
            canonical_continuation_sequence(&current),
            frozen::canonical_continuation_sequence(&old)
        );
        assert_eq!(
            leselang_command::continuation_identity::canonical_merge_sequence(&current),
            frozen::canonical_merge_sequence(&old)
        );
        for other in &tokens {
            let right = ContinuationToken::from_wire(other.clone());
            let frozen_right: frozen::ContinuationToken =
                serde_json::from_value(json!(other)).unwrap();
            assert_eq!(current.cmp(&right), old.cmp(&frozen_right));
            assert_eq!(
                continuation_age_order(&current, &right),
                frozen::continuation_age_order(&old, &frozen_right)
            );
        }
    }
    assert_eq!(tokens.len() * tokens.len(), 4_225);
}

#[test]
fn terminal_data_migration_keeps_frozen_wire_and_field_acceptance_policy() {
    let mut count = 0;
    let mut outcomes = vec![
        BranchOutcome::Value(Value::UiFocus {
            node_id: "node".into(),
        }),
        BranchOutcome::Value(Value::Scalar {
            value: ScalarValue::Boolean(true),
        }),
    ];
    for class in [EffectErrorClass::Permanent, EffectErrorClass::Transient] {
        outcomes.push(BranchOutcome::Failed(FailedEffect {
            error: EffectError {
                class,
                code: "HOST".into(),
                message: "failed".into(),
            },
            retry_count: MAX_SEMANTIC_RETRIES,
        }));
    }
    for code in ["HOST", ""] {
        outcomes.push(BranchOutcome::Fault(Fault {
            code: code.into(),
            message: "fault".into(),
        }));
    }
    for text in tokens() {
        let token = ContinuationToken::from_wire(text);
        count += compare_wire::<_, frozen::ContinuationToken>(&token);
        for reason in [
            CancellationReason::Requested,
            CancellationReason::DeadlineExceeded,
        ] {
            for observed_at_ms in [0, 1, i64::MAX as u64, i64::MAX as u64 + 1, u64::MAX] {
                let cancelled = Cancellation {
                    continuation: token.clone(),
                    reason: reason.clone(),
                    observed_at_ms,
                };
                count += compare_wire::<_, frozen::Cancellation>(&cancelled);
                outcomes.push(BranchOutcome::Cancelled(cancelled));
            }
        }
    }
    for reason in [
        CancellationReason::Requested,
        CancellationReason::DeadlineExceeded,
    ] {
        count += compare_wire::<_, frozen::CancellationReason>(&reason);
    }
    assert_eq!(outcomes.len(), 656);
    for outcome in outcomes {
        count += compare_wire::<_, frozen::BranchOutcome>(&outcome);
        count += compare_wire::<_, frozen::BranchCompletion>(&BranchCompletion {
            branch: "a".into(),
            outcome,
        });
    }
    assert_eq!(count, 16_232);
}

#[test]
fn adapter_data_keeps_original_vm_refusals_without_allocating_or_cancelling_work() {
    use crate::CancellationReason::Requested;
    use leselang_command::BranchOutcome::Cancelled;

    let request = continuation_contract_tests::requests().remove(0);
    let token: leselang_command::ContinuationToken = request.continuation.token.clone();
    let _: crate::ContinuationToken = token.clone();
    let cancellation: leselang_command::Cancellation = Cancellation {
        continuation: token,
        reason: Requested,
        observed_at_ms: 1,
    };
    let outcome: crate::BranchOutcome = Cancelled(cancellation);
    assert_eq!(validate_branch_outcome(&outcome), Ok(0));
    for text in ["", "invalid/token", "continuation-01"] {
        let raw = ContinuationToken::from_wire(text.into());
        let mut candidate = request.continuation.clone();
        candidate.token = raw.clone();
        let mut vm = Vm::new(100);
        let before = vm.next_effect_id;
        let expected = validate_restorable_image(&candidate).unwrap_err();
        assert_eq!(vm.restore(candidate), Err(expected));
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.next_effect_id, before);
        let outcome = BranchOutcome::Cancelled(Cancellation {
            continuation: raw,
            reason: CancellationReason::Requested,
            observed_at_ms: u64::MAX,
        });
        assert_eq!(
            validate_branch_outcome(&outcome),
            Err(Fault {
                code: "LSV2403".into(),
                message: "structured merge branch outcome is invalid".into(),
            })
        );
    }
}

// Frozen released DTOs and identity rules. No adapter policy/identity delegation.
mod frozen {
    use super::{Deserialize, EffectError, Fault, Serialize, Value};
    type FailedEffect = leselang_runtime_core::FailedEffect<EffectError>;
    const MAX_EFFECT_SEQUENCE: u64 = i64::MAX as u64 - 1;

    #[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
    #[serde(transparent)]
    pub struct ContinuationToken(String);
    impl ContinuationToken {
        pub fn as_str(&self) -> &str {
            &self.0
        }
    }
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum CancellationReason {
        Requested,
        DeadlineExceeded,
    }
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    pub struct Cancellation {
        pub continuation: ContinuationToken,
        pub reason: CancellationReason,
        pub observed_at_ms: u64,
    }
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
    pub enum BranchOutcome {
        Value(Value),
        Cancelled(Cancellation),
        Failed(FailedEffect),
        Fault(Fault),
    }
    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    pub struct BranchCompletion {
        pub branch: String,
        pub outcome: BranchOutcome,
    }
    pub fn valid_continuation_token(token: &ContinuationToken) -> bool {
        !token.as_str().is_empty()
            && token.as_str().len() <= 128
            && token
                .as_str()
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    }
    pub fn canonical_continuation_sequence(token: &ContinuationToken) -> Option<u64> {
        let suffix = token.as_str().strip_prefix("continuation-")?;
        let sequence = suffix.parse::<u64>().ok()?;
        (sequence > 0 && sequence <= MAX_EFFECT_SEQUENCE && sequence.to_string() == suffix)
            .then_some(sequence)
    }
    pub fn canonical_merge_sequence(token: &ContinuationToken) -> Option<u64> {
        token.as_str().strip_prefix("merge-").and_then(|suffix| {
            let sequence = suffix.parse::<u64>().ok()?;
            (sequence > 0 && sequence <= MAX_EFFECT_SEQUENCE && sequence.to_string() == suffix)
                .then_some(sequence)
        })
    }
    pub fn continuation_age_order(
        left: &ContinuationToken,
        right: &ContinuationToken,
    ) -> std::cmp::Ordering {
        let sequence = |token: &ContinuationToken| {
            token
                .as_str()
                .strip_prefix("continuation-")
                .and_then(|value| value.parse::<u64>().ok())
        };
        match (sequence(left), sequence(right)) {
            (Some(left), Some(right)) => left.cmp(&right),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => left.cmp(right),
        }
    }
}
