use super::*;
use serde::de::DeserializeOwned;

const ORIGINS: [CommandOrigin; 5] = [
    CommandOrigin::Gui,
    CommandOrigin::Cli,
    CommandOrigin::Leselang,
    CommandOrigin::Model,
    CommandOrigin::CompatibilityAdapter,
];

fn context(origin: CommandOrigin, identity: &str, revision: u64) -> DebuggerAuditContext {
    DebuggerAuditContext {
        command_id: CommandId::new("audit-command").unwrap(),
        idempotency_key: IdempotencyKey::new("audit-key").unwrap(),
        principal: Principal {
            id: identity.into(),
        },
        origin,
        session_id: identity.into(),
        expected_revision: Revision(revision),
    }
}

fn frozen_context(audit: &DebuggerAuditContext) -> frozen::DebuggerAuditContext {
    frozen::DebuggerAuditContext {
        command_id: audit.command_id.clone(),
        idempotency_key: audit.idempotency_key.clone(),
        principal: audit.principal.clone(),
        origin: audit.origin,
        session_id: audit.session_id.clone(),
        expected_revision: audit.expected_revision,
    }
}

fn compare_decode<Actual: DeserializeOwned + Serialize, Original: DeserializeOwned + Serialize>(
    bytes: &[u8],
) {
    let actual = serde_json::from_slice::<Actual>(bytes)
        .map(|value| serde_json::to_vec(&value).unwrap())
        .map_err(|error| error.to_string());
    let original = serde_json::from_slice::<Original>(bytes)
        .map(|value| serde_json::to_vec(&value).unwrap())
        .map_err(|error| error.to_string());
    assert_eq!(actual, original, "{}", String::from_utf8_lossy(bytes));
}

#[test]
fn debugger_audit_aliases_preserve_wire_bytes_and_closed_decode_errors() {
    let mut valid = 0;
    let mut malformed = 0;
    for origin in ORIGINS {
        for identity in ["operator".into(), "A0-_.:z".into(), "x".repeat(128)] {
            for revision in [0, i64::MAX as u64, u64::MAX] {
                let audit = context(origin, &identity, revision);
                // The released path is the same type, not a conversion wrapper.
                let adapter: &leselang_command::DebuggerAuditContext = &audit;
                let original = frozen_context(&audit);
                let context_bytes = serde_json::to_vec(adapter).unwrap();
                assert_eq!(context_bytes, serde_json::to_vec(&original).unwrap());
                for now in [0, i64::MAX as u64, u64::MAX] {
                    let record: DebuggerAuditRecord =
                        leselang_command::debugger_audit::debugger_audit_record(&audit, now);
                    let record_bytes = serde_json::to_vec(&record).unwrap();
                    assert_eq!(
                        record_bytes,
                        serde_json::to_vec(&frozen::debugger_audit_record(&original, now)).unwrap()
                    );
                    for (is_context, bytes) in [(true, &context_bytes), (false, &record_bytes)] {
                        let compare = |input: &[u8]| {
                            if is_context {
                                compare_decode::<DebuggerAuditContext, frozen::DebuggerAuditContext>(
                                    input,
                                );
                            } else {
                                compare_decode::<DebuggerAuditRecord, frozen::DebuggerAuditRecord>(
                                    input,
                                );
                            }
                        };
                        compare(bytes);
                        valid += 1;
                        let value: serde_json::Value = serde_json::from_slice(bytes).unwrap();
                        let fields = value.as_object().unwrap();
                        for (key, field) in fields {
                            let mut missing = value.clone();
                            missing.as_object_mut().unwrap().remove(key);
                            compare(&serde_json::to_vec(&missing).unwrap());
                            let mut wrong = value.clone();
                            wrong[key] = serde_json::Value::Null;
                            compare(&serde_json::to_vec(&wrong).unwrap());
                            let duplicate = format!(
                                "{{{}:{},{}",
                                serde_json::to_string(key).unwrap(),
                                field,
                                std::str::from_utf8(&bytes[1..]).unwrap()
                            );
                            compare(duplicate.as_bytes());
                            malformed += 3;
                        }
                        let mut unknown = value.clone();
                        unknown["unexpected"] = true.into();
                        compare(&serde_json::to_vec(&unknown).unwrap());
                        compare(&bytes[..bytes.len() - 1]);
                        compare(&[bytes.as_slice(), b" null"].concat());
                        malformed += 3;
                    }
                }
            }
        }
    }
    assert_eq!(valid, 270);
    assert_eq!(malformed, 5_670);
}

#[test]
fn debugger_audit_native_identity_validation_keeps_exact_faults() {
    let mut candidates = (0..=255u8)
        .map(|byte| format!("a{}z", char::from(byte)))
        .collect::<Vec<_>>();
    candidates.extend([
        String::new(),
        "x".repeat(127),
        "x".repeat(128),
        "x".repeat(129),
        "a".repeat(4096),
        "A0-_.:z".into(),
        "\u{754c}".repeat(43),
    ]);
    let mut comparisons = 0;
    for origin in ORIGINS {
        for value in &candidates {
            for field in 0..3 {
                let mut audit = context(origin, "operator", u64::MAX);
                if field != 1 {
                    audit.principal.id = value.clone();
                }
                if field != 0 {
                    audit.session_id = value.clone();
                }
                assert_eq!(
                    validate_debugger_audit(&audit),
                    frozen::validate_debugger_audit(&frozen_context(&audit))
                );
                comparisons += 1;
            }
        }
    }
    assert_eq!(comparisons, 3_945);
}

#[test]
fn debugger_audit_projection_and_attribution_keep_non_authoritative_boundaries() {
    for origin in ORIGINS {
        let audit = context(origin, "operator", 7);
        let original = frozen_context(&audit);
        let record = leselang_command::debugger_audit::debugger_audit_record(&audit, 100);
        let frozen_record = frozen::debugger_audit_record(&original, 100);
        for field in 0..7 {
            let mut changed = audit.clone();
            match field {
                0 => changed.command_id = CommandId::new("other-command").unwrap(),
                1 => changed.principal.id = "other-operator".into(),
                2 => {
                    changed.origin = if origin == CommandOrigin::Gui {
                        CommandOrigin::Cli
                    } else {
                        CommandOrigin::Gui
                    }
                }
                3 => changed.session_id = "other-session".into(),
                4 => changed.expected_revision = Revision(8),
                5 => changed.idempotency_key = IdempotencyKey::new("other-key").unwrap(),
                6 => {}
                _ => unreachable!(),
            }
            let matched = leselang_command::debugger_audit::audit_matches_record(&changed, &record);
            assert_eq!(
                matched,
                frozen::audit_matches_record(&frozen_context(&changed), &frozen_record)
            );
            assert_eq!(matched, field >= 5);
        }
        let mut changed_time = record;
        changed_time.observed_at_ms = u64::MAX;
        assert!(leselang_command::debugger_audit::audit_matches_record(
            &audit,
            &changed_time
        ));
    }
}

// Frozen pre-migration product audit protocol; no calls into the new adapter.
mod frozen {
    use super::*;

    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct DebuggerAuditContext {
        pub command_id: CommandId,
        pub idempotency_key: IdempotencyKey,
        pub principal: Principal,
        pub origin: CommandOrigin,
        pub session_id: String,
        pub expected_revision: Revision,
    }

    #[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
    #[serde(deny_unknown_fields)]
    pub struct DebuggerAuditRecord {
        pub command_id: CommandId,
        pub principal_id: String,
        pub origin: CommandOrigin,
        pub session_id: String,
        pub expected_revision: Revision,
        pub observed_at_ms: u64,
    }

    pub(super) fn validate_debugger_audit(audit: &DebuggerAuditContext) -> Result<(), Fault> {
        let valid_session = !audit.session_id.is_empty()
            && audit.session_id.len() <= 128
            && audit.session_id.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
            });
        let valid_principal = !audit.principal.id.is_empty()
            && audit.principal.id.len() <= 128
            && audit.principal.id.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
            });
        if !valid_session || !valid_principal {
            return Err(Fault {
                code: "LSV2015".to_string(),
                message: "invalid debugger audit identity".to_string(),
            });
        }
        Ok(())
    }

    pub(super) fn debugger_audit_record(
        audit: &DebuggerAuditContext,
        observed_at_ms: u64,
    ) -> DebuggerAuditRecord {
        DebuggerAuditRecord {
            command_id: audit.command_id.clone(),
            principal_id: audit.principal.id.clone(),
            origin: audit.origin,
            session_id: audit.session_id.clone(),
            expected_revision: audit.expected_revision,
            observed_at_ms,
        }
    }

    pub(super) fn audit_matches_record(
        audit: &DebuggerAuditContext,
        record: &DebuggerAuditRecord,
    ) -> bool {
        record.command_id == audit.command_id
            && record.principal_id == audit.principal.id
            && record.origin == audit.origin
            && record.session_id == audit.session_id
            && record.expected_revision == audit.expected_revision
    }
}
