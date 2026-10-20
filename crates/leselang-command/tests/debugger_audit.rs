use leselang_command::debugger_audit::*;
use leselang_host_contract::{CommandOrigin, Principal, Revision};
use leserpent_domain::{CommandId, IdempotencyKey};

fn context() -> DebuggerAuditContext {
    DebuggerAuditContext {
        command_id: CommandId::new("command-1").unwrap(),
        idempotency_key: IdempotencyKey::new("key-1").unwrap(),
        principal: Principal::new("operator").unwrap(),
        origin: CommandOrigin::Gui,
        session_id: "session-1".into(),
        expected_revision: Revision(7),
    }
}

#[test]
fn metadata_validation_is_bounded_but_not_host_authorization() {
    let mut audit = context();
    audit.principal.id = "A0-_.:z".repeat(18);
    audit.session_id = "x".repeat(128);
    audit.expected_revision = Revision(u64::MAX);
    assert_eq!(validate_debugger_audit(&audit), Ok(()));
    for identity in ["", "a b", "bad\nid", "bad/id", "\u{e9}", &"x".repeat(129)] {
        for field in 0..2 {
            let mut invalid = audit.clone();
            if field == 0 {
                invalid.principal.id = identity.into();
            } else {
                invalid.session_id = identity.into();
            }
            let error = validate_debugger_audit(&invalid).unwrap_err();
            assert_eq!(error, InvalidDebuggerAudit);
            assert_eq!(error.to_string(), "invalid debugger audit identity");
        }
    }
}

#[test]
fn untrusted_record_projection_does_not_validate_or_commit() {
    let mut audit = context();
    audit.session_id.clear();
    audit.principal.id.clear();
    assert!(validate_debugger_audit(&audit).is_err());
    let record = debugger_audit_record(&audit, u64::MAX);
    assert_eq!(record.principal_id, "");
    assert_eq!(record.session_id, "");
    assert_eq!(record.observed_at_ms, u64::MAX);
    assert_eq!(record.command_id, audit.command_id);
}

#[test]
fn attribution_match_is_not_an_idempotency_or_timestamp_check() {
    let mut audit = context();
    let record = debugger_audit_record(&audit, 100);
    assert!(audit_matches_record(&audit, &record));
    audit.idempotency_key = IdempotencyKey::new("different-key").unwrap();
    assert!(audit_matches_record(&audit, &record));
    let mut changed_time = record.clone();
    changed_time.observed_at_ms = 999;
    assert!(audit_matches_record(&audit, &changed_time));
    audit.session_id = "different-session".into();
    assert!(!audit_matches_record(&audit, &record));
}

#[test]
fn exact_origin_labels_match_json_and_reject_aliases() {
    for (origin, label) in [
        (CommandOrigin::Gui, "gui"),
        (CommandOrigin::Cli, "cli"),
        (CommandOrigin::Leselang, "leselang"),
        (CommandOrigin::Model, "model"),
        (CommandOrigin::CompatibilityAdapter, "compatibility_adapter"),
    ] {
        assert_eq!(command_origin_label(origin), label);
        assert_eq!(parse_command_origin(label), Some(origin));
        assert_eq!(
            serde_json::to_string(&origin).unwrap(),
            format!("\"{label}\"")
        );
        for invalid in [
            label.to_uppercase(),
            format!(" {label}"),
            format!("{label} "),
            format!("{label}\0"),
        ] {
            assert_eq!(parse_command_origin(&invalid), None);
        }
    }
    for invalid in [
        "",
        "compatibility-adapter",
        "agent",
        "shell",
        "web",
        "native",
    ] {
        assert_eq!(parse_command_origin(invalid), None);
    }
}

#[test]
fn closed_wire_round_trips_without_importing_the_vm() {
    let audit = context();
    let record = debugger_audit_record(&audit, 100);
    let audit_bytes = serde_json::to_vec(&audit).unwrap();
    let record_bytes = serde_json::to_vec(&record).unwrap();
    assert_eq!(
        serde_json::from_slice::<DebuggerAuditContext>(&audit_bytes).unwrap(),
        audit
    );
    assert_eq!(
        serde_json::from_slice::<DebuggerAuditRecord>(&record_bytes).unwrap(),
        record
    );
    for is_context in [true, false] {
        let mut value = if is_context {
            serde_json::to_value(&audit).unwrap()
        } else {
            serde_json::to_value(&record).unwrap()
        };
        value["authority"] = true.into();
        if is_context {
            assert!(serde_json::from_value::<DebuggerAuditContext>(value).is_err());
        } else {
            assert!(serde_json::from_value::<DebuggerAuditRecord>(value).is_err());
        }
    }
}
