use leselang_command::retry_policy::*;
use leselang_runtime_core::MAX_CLOCK_MS;

fn transient() -> EffectError {
    EffectError {
        class: EffectErrorClass::Transient,
        code: "host_busy".into(),
        message: "temporarily unavailable".into(),
    }
}

#[test]
fn pure_proposals_share_numeric_backoff_without_consuming_payloads_or_refilling_budgets() {
    let error = transient();
    let policy = RetryPolicy {
        max_retries: 4,
        base_delay_ms: 7,
        max_delay_ms: 25,
    };
    let pointer = error.message.as_ptr();
    for (count, delay) in [(0, 7), (1, 14), (2, 25), (3, 25)] {
        let proposed = plan_effect_retry(count, 100, &error, &policy)
            .unwrap()
            .unwrap();
        assert_eq!(proposed.retry_count, count + 1);
        assert_eq!(proposed.ready_at_ms, 100 + delay);
        assert_eq!(error.message.as_ptr(), pointer);
    }
    for count in [4, 5, u32::MAX] {
        assert_eq!(plan_effect_retry(count, 100, &error, &policy), Ok(None));
    }
    assert_eq!(error, transient());
}

#[test]
fn metadata_classification_policy_and_clock_keep_distinct_precedence() {
    let invalid = RetryPolicy {
        max_retries: u32::MAX,
        base_delay_ms: 0,
        max_delay_ms: 0,
    };
    let mut error = transient();
    error.code.clear();
    assert_eq!(
        plan_effect_retry(0, u64::MAX, &error, &invalid),
        Err(EffectRetryError::Metadata)
    );
    error = transient();
    assert_eq!(
        plan_effect_retry(u32::MAX, u64::MAX, &error, &invalid),
        Err(EffectRetryError::Policy)
    );
    error.class = EffectErrorClass::Permanent;
    assert_eq!(
        plan_effect_retry(u32::MAX, u64::MAX, &error, &invalid),
        Ok(None)
    );
    error.message.clear();
    assert_eq!(
        plan_effect_retry(0, 0, &error, &invalid),
        Err(EffectRetryError::Metadata)
    );
    assert_eq!(
        plan_effect_retry(
            u32::MAX,
            MAX_CLOCK_MS + 1,
            &transient(),
            &RetryPolicy::default()
        ),
        Err(EffectRetryError::Clock)
    );
}

#[test]
fn clock_edges_are_exact_and_never_clamped_even_at_exhaustion() {
    let policy = RetryPolicy::default();
    let proposed = plan_effect_retry(0, MAX_CLOCK_MS - 250, &transient(), &policy)
        .unwrap()
        .unwrap();
    assert_eq!(proposed.ready_at_ms, MAX_CLOCK_MS);
    assert_eq!(
        plan_effect_retry(0, MAX_CLOCK_MS - 249, &transient(), &policy),
        Err(EffectRetryError::Clock)
    );
    assert_eq!(
        plan_effect_retry(3, MAX_CLOCK_MS, &transient(), &policy),
        Ok(None)
    );
    assert_eq!(
        plan_effect_retry(3, MAX_CLOCK_MS + 1, &transient(), &policy),
        Err(EffectRetryError::Clock)
    );
}

#[test]
fn decoded_metadata_is_not_admission_and_keeps_released_unknown_field_behavior() {
    let policy: RetryPolicy = serde_json::from_str(
        r#"{"max_retries":17,"base_delay_ms":0,"max_delay_ms":0,"future":true}"#,
    )
    .unwrap();
    assert_eq!(
        validate_retry_policy(&policy),
        Err(EffectRetryError::Policy)
    );
    let error: EffectError =
        serde_json::from_str(r#"{"class":"permanent","code":"","message":"","future":true}"#)
            .unwrap();
    assert_eq!(
        validate_effect_error(&error),
        Err(EffectRetryError::Metadata)
    );
    assert!(
        serde_json::from_str::<EffectError>(
            r#"{"class":"retryable","code":"busy","message":"busy"}"#
        )
        .is_err()
    );
}

#[test]
fn product_defaults_limits_and_error_messages_remain_explicit() {
    assert_eq!(
        RetryPolicy::default(),
        RetryPolicy {
            max_retries: 3,
            base_delay_ms: 250,
            max_delay_ms: 30_000
        }
    );
    assert_eq!(MAX_SEMANTIC_RETRIES, 16);
    assert_eq!(MAX_RETRY_DELAY_MS, 3_600_000);
    assert_eq!(
        EffectRetryError::Metadata.to_string(),
        "effect error metadata is invalid"
    );
    assert_eq!(
        EffectRetryError::Policy.to_string(),
        "semantic retry policy exceeds runtime bounds"
    );
    assert_eq!(
        EffectRetryError::Clock.to_string(),
        "semantic retry clock overflow"
    );
}
