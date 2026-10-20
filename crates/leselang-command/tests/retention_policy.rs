use leselang_command::retention_policy::{
    CompactionReport, DEFAULT_COMPACTION_BATCH, DEFAULT_COMPLETED_RETENTION,
    InvalidRetentionPolicy, MAX_COMPACTION_BATCH, RetentionPolicy, validate_retention_policy,
};
use serde::{Deserialize, Serialize};

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct FrozenPolicy {
    max_completed_records: usize,
    max_delete_per_run: usize,
}

#[derive(Debug, Deserialize, Eq, PartialEq, Serialize)]
struct FrozenReport {
    removed_records: usize,
    remaining_completed: usize,
    reclaimed_logical_bytes: usize,
}

#[test]
fn default_and_wire_keep_released_fields_unknown_field_policy_and_exact_errors() {
    assert_eq!(DEFAULT_COMPLETED_RETENTION, 5_000);
    assert_eq!(DEFAULT_COMPACTION_BATCH, 500);
    assert_eq!(MAX_COMPACTION_BATCH, 1_000);
    assert_eq!(
        RetentionPolicy::default(),
        RetentionPolicy {
            max_completed_records: 5_000,
            max_delete_per_run: 500,
        }
    );
    for source in [
        r#"{"max_completed_records":5000,"max_delete_per_run":500}"#,
        r#"{"max_completed_records":0,"max_delete_per_run":0,"extra":true}"#,
        r#"{"max_completed_records":1}"#,
        r#"{"max_completed_records":-1,"max_delete_per_run":2}"#,
        r#"{"max_completed_records":1,"max_delete_per_run":"2"}"#,
        r#"{"max_completed_records":1,"max_delete_per_run":2,"max_delete_per_run":3}"#,
    ] {
        let actual = serde_json::from_str::<RetentionPolicy>(source)
            .map(|p| serde_json::to_vec(&p).unwrap())
            .map_err(|e| e.to_string());
        let expected = serde_json::from_str::<FrozenPolicy>(source)
            .map(|p| serde_json::to_vec(&p).unwrap())
            .map_err(|e| e.to_string());
        assert_eq!(actual, expected);
    }
    for source in [
        r#"{"removed_records":2,"remaining_completed":3,"reclaimed_logical_bytes":256}"#,
        r#"{"removed_records":0,"remaining_completed":0,"reclaimed_logical_bytes":0,"extra":1}"#,
        r#"{"removed_records":1,"remaining_completed":2}"#,
        r#"{"removed_records":-1,"remaining_completed":2,"reclaimed_logical_bytes":3}"#,
    ] {
        let actual = serde_json::from_str::<CompactionReport>(source)
            .map(|p| serde_json::to_vec(&p).unwrap())
            .map_err(|e| e.to_string());
        let expected = serde_json::from_str::<FrozenReport>(source)
            .map(|p| serde_json::to_vec(&p).unwrap())
            .map_err(|e| e.to_string());
        assert_eq!(actual, expected);
    }
}

#[test]
fn policy_admission_and_batch_sizing_preserve_full_width_boundaries() {
    let mut count = 0;
    for retained in [0, 1, 500, 5_000, 10_000, 10_001, usize::MAX] {
        for batch in [0, 1, 500, 1_000, 1_001, usize::MAX] {
            for ceiling in [0, 1, 10_000, usize::MAX] {
                let policy = RetentionPolicy {
                    max_completed_records: retained,
                    max_delete_per_run: batch,
                };
                assert_eq!(
                    validate_retention_policy(&policy, ceiling).is_ok(),
                    retained != 0 && retained <= ceiling && batch != 0 && batch <= 1_000
                );
                for completed in [0, 1, 499, 5_000, 10_000, usize::MAX] {
                    assert_eq!(
                        policy.removal_count(completed),
                        completed.saturating_sub(retained).min(batch)
                    );
                    count += 1;
                }
            }
        }
    }
    assert_eq!(count, 1_008);
    assert_eq!(
        InvalidRetentionPolicy.to_string(),
        "journal retention policy exceeds runtime bounds"
    );
}

#[test]
fn decoding_and_numeric_batch_sizing_do_not_grant_eviction_authority() {
    let policy: RetentionPolicy =
        serde_json::from_str(r#"{"max_completed_records":0,"max_delete_per_run":99999}"#).unwrap();
    assert_eq!(policy.removal_count(12), 12);
    assert_eq!(
        validate_retention_policy(&policy, 10_000),
        Err(InvalidRetentionPolicy)
    );
    let report = CompactionReport {
        removed_records: 2,
        remaining_completed: 7,
        reclaimed_logical_bytes: usize::MAX,
    };
    assert_eq!(
        serde_json::from_slice::<CompactionReport>(&serde_json::to_vec(&report).unwrap()).unwrap(),
        report
    );
}
