use super::*;
use leselang_runtime_core::MAX_CLOCK_MS;
use serde::de::DeserializeOwned;

fn error(class: EffectErrorClass, code: &str, message: &str) -> EffectError {
    EffectError {
        class,
        code: code.into(),
        message: message.into(),
    }
}

fn original_error(error: &EffectError) -> frozen::EffectError {
    frozen::EffectError {
        class: match error.class {
            EffectErrorClass::Transient => frozen::EffectErrorClass::Transient,
            EffectErrorClass::Permanent => frozen::EffectErrorClass::Permanent,
        },
        code: error.code.clone(),
        message: error.message.clone(),
    }
}

#[test]
fn product_retry_planning_preserves_frozen_decisions_and_fault_precedence() {
    let mut comparisons = 0;
    for max_retries in [0, 3, 16, 17, u32::MAX] {
        for base_delay_ms in [0, 1, 250, 3_600_000, 3_600_001, u64::MAX] {
            for max_delay_ms in [0, 1, 30_000, 3_600_000, 3_600_001, u64::MAX] {
                let policy = RetryPolicy {
                    max_retries,
                    base_delay_ms,
                    max_delay_ms,
                };
                let original_policy = frozen::RetryPolicy {
                    max_retries,
                    base_delay_ms,
                    max_delay_ms,
                };
                for class in [EffectErrorClass::Transient, EffectErrorClass::Permanent] {
                    for (code, message) in [
                        ("host_busy", "temporarily unavailable"),
                        ("", "temporarily unavailable"),
                        ("host_busy", ""),
                        ("", ""),
                    ] {
                        let error = error(class, code, message);
                        let original_error = original_error(&error);
                        for count in [0, 2, 3, 15, 16, u32::MAX] {
                            for now in [
                                0,
                                1,
                                MAX_CLOCK_MS - 3_600_000,
                                MAX_CLOCK_MS - 250,
                                MAX_CLOCK_MS,
                                MAX_CLOCK_MS + 1,
                                u64::MAX,
                            ] {
                                assert_eq!(
                                    leselang_command::retry_policy::plan_effect_retry(
                                        count, now, &error, &policy
                                    )
                                    .map_err(effect_retry_fault),
                                    frozen::plan(count, now, &original_error, &original_policy),
                                    "{policy:?} {error:?} count={count} now={now}"
                                );
                                comparisons += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(comparisons, 60_480);
}

#[test]
fn product_retry_metadata_preserves_native_byte_limits_and_exact_faults() {
    let mut codes = (0..=255u8)
        .map(|byte| format!("a{}z", char::from(byte)))
        .collect::<Vec<_>>();
    codes.extend([
        String::new(),
        "x".repeat(64),
        "x".repeat(65),
        "A0._-".into(),
        "x".repeat(4096),
        ":".into(),
        "\u{e9}".into(),
    ]);
    let mut comparisons = 0;
    for class in [EffectErrorClass::Transient, EffectErrorClass::Permanent] {
        for code in &codes {
            for message in [
                String::new(),
                "x".into(),
                "x".repeat(1024),
                "x".repeat(1025),
                "\u{e9}".repeat(512),
                "\u{e9}".repeat(513),
            ] {
                let error = error(class, code, &message);
                assert_eq!(
                    validate_effect_error(&error),
                    frozen::validate_effect_error(&original_error(&error))
                );
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 3_156);
}

fn compare_wire<A: DeserializeOwned + Serialize, B: DeserializeOwned + Serialize>(input: &str) {
    let actual = serde_json::from_str::<A>(input)
        .map(|value| serde_json::to_vec(&value).unwrap())
        .map_err(|error| error.to_string());
    let original = serde_json::from_str::<B>(input)
        .map(|value| serde_json::to_vec(&value).unwrap())
        .map_err(|error| error.to_string());
    assert_eq!(actual, original, "{input}");
}

fn compare_shapes<A: DeserializeOwned + Serialize, B: DeserializeOwned + Serialize>(
    value: serde_json::Value,
) -> usize {
    let original = value.to_string();
    compare_wire::<A, B>(&original);
    let mut comparisons = 1;
    for (key, field) in value.as_object().unwrap() {
        let mut missing = value.clone();
        missing.as_object_mut().unwrap().remove(key);
        compare_wire::<A, B>(&missing.to_string());
        let mut wrong = value.clone();
        wrong[key] = serde_json::Value::Null;
        compare_wire::<A, B>(&wrong.to_string());
        compare_wire::<A, B>(&format!(
            "{{{}:{},{}",
            serde_json::to_string(key).unwrap(),
            field,
            &original[1..]
        ));
        comparisons += 3;
    }
    let mut unknown = value;
    unknown["future_metadata"] = true.into();
    compare_wire::<A, B>(&unknown.to_string());
    compare_wire::<A, B>(&original[..original.len() - 1]);
    compare_wire::<A, B>(&format!("{original} null"));
    comparisons + 3
}

#[test]
fn product_retry_types_are_exact_aliases_with_unchanged_wire_and_defaults() {
    let default: leselang_command::RetryPolicy = RetryPolicy::default();
    assert_eq!(
        serde_json::to_vec(&default).unwrap(),
        serde_json::to_vec(&frozen::RetryPolicy::default()).unwrap()
    );
    let mut comparisons = 0;
    for policy in [
        default,
        RetryPolicy {
            max_retries: 0,
            base_delay_ms: 0,
            max_delay_ms: 0,
        },
        RetryPolicy {
            max_retries: 16,
            base_delay_ms: 1,
            max_delay_ms: 3_600_000,
        },
        RetryPolicy {
            max_retries: u32::MAX,
            base_delay_ms: u64::MAX,
            max_delay_ms: u64::MAX,
        },
    ] {
        let original = frozen::RetryPolicy {
            max_retries: policy.max_retries,
            base_delay_ms: policy.base_delay_ms,
            max_delay_ms: policy.max_delay_ms,
        };
        assert_eq!(
            serde_json::to_vec(&policy).unwrap(),
            serde_json::to_vec(&original).unwrap()
        );
        comparisons += compare_shapes::<RetryPolicy, frozen::RetryPolicy>(
            serde_json::to_value(policy).unwrap(),
        );
    }
    for class in [EffectErrorClass::Transient, EffectErrorClass::Permanent] {
        let _: leselang_command::EffectErrorClass = class;
        for (code, message) in [
            ("busy", "retry"),
            ("", ""),
            ("bad code", "x"),
            ("ok", "\u{e9}"),
        ] {
            let error: leselang_command::EffectError = error(class, code, message);
            assert_eq!(
                serde_json::to_vec(&error).unwrap(),
                serde_json::to_vec(&original_error(&error)).unwrap()
            );
            comparisons += compare_shapes::<EffectError, frozen::EffectError>(
                serde_json::to_value(error).unwrap(),
            );
        }
    }
    for input in [
        "\"transient\"",
        "\"permanent\"",
        "\"Transient\"",
        "\"retry\"",
        "null",
        "0",
        "{}",
        "[]",
    ] {
        compare_wire::<EffectErrorClass, frozen::EffectErrorClass>(input);
        comparisons += 1;
    }
    assert_eq!(comparisons, 164);
}

// Frozen product protocol and planning policy from before this migration.
mod frozen {
    use super::*;
    use leselang_runtime_core::{RetryPlanError, RetryTiming, plan_retry_schedule};
    const DEFAULT_MAX_SEMANTIC_RETRIES: u32 = 3;
    const MAX_SEMANTIC_RETRIES: u32 = 16;
    const DEFAULT_RETRY_BASE_DELAY_MS: u64 = 250;
    const DEFAULT_RETRY_MAX_DELAY_MS: u64 = 30_000;
    const MAX_RETRY_DELAY_MS: u64 = 60 * 60 * 1_000;

    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    pub struct RetryPolicy {
        pub max_retries: u32,
        pub base_delay_ms: u64,
        pub max_delay_ms: u64,
    }

    #[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(rename_all = "snake_case")]
    pub enum EffectErrorClass {
        Transient,
        Permanent,
    }

    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    pub struct EffectError {
        pub class: EffectErrorClass,
        pub code: String,
        pub message: String,
    }

    impl Default for RetryPolicy {
        fn default() -> Self {
            Self {
                max_retries: DEFAULT_MAX_SEMANTIC_RETRIES,
                base_delay_ms: DEFAULT_RETRY_BASE_DELAY_MS,
                max_delay_ms: DEFAULT_RETRY_MAX_DELAY_MS,
            }
        }
    }

    pub(super) fn validate_retry_policy(policy: &RetryPolicy) -> Result<(), Fault> {
        if policy.max_retries > MAX_SEMANTIC_RETRIES
            || policy.base_delay_ms == 0
            || policy.base_delay_ms > MAX_RETRY_DELAY_MS
            || policy.max_delay_ms < policy.base_delay_ms
            || policy.max_delay_ms > MAX_RETRY_DELAY_MS
        {
            return Err(Fault {
                code: "LSV2201".to_string(),
                message: "semantic retry policy exceeds runtime bounds".to_string(),
            });
        }
        Ok(())
    }

    pub(super) fn validate_effect_error(error: &EffectError) -> Result<(), Fault> {
        if error.code.is_empty()
            || error.code.len() > 64
            || !error
                .code
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_'))
            || error.message.is_empty()
            || error.message.len() > 1_024
        {
            return Err(Fault {
                code: "LSV2202".to_string(),
                message: "effect error metadata is invalid".to_string(),
            });
        }
        Ok(())
    }

    pub(super) fn plan(
        current_retry_count: u32,
        now_ms: u64,
        error: &EffectError,
        policy: &RetryPolicy,
    ) -> Result<Option<leselang_runtime_core::RetrySchedulePoint>, Fault> {
        validate_effect_error(error)?;
        let schedule = if error.class == EffectErrorClass::Transient {
            validate_retry_policy(policy)?;
            plan_retry_schedule(
                current_retry_count,
                now_ms,
                RetryTiming {
                    max_retries: policy.max_retries,
                    base_delay_ms: policy.base_delay_ms,
                    max_delay_ms: policy.max_delay_ms,
                },
            )
            .map_err(|error| match error {
                RetryPlanError::Timing => Fault {
                    code: "LSV2201".into(),
                    message: "semantic retry policy exceeds runtime bounds".into(),
                },
                RetryPlanError::Clock(_) => Fault {
                    code: "LSV2203".into(),
                    message: "semantic retry clock overflow".into(),
                },
            })?
        } else {
            None
        };
        Ok(schedule)
    }
}
