//! Leserpent debugger audit metadata, not cancellation or authorization policy.

use leselang_host_contract::{CommandOrigin, Principal, Revision};
use leserpent_domain::{CommandId, IdempotencyKey};
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InvalidDebuggerAudit;

impl std::fmt::Display for InvalidDebuggerAudit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("invalid debugger audit identity")
    }
}

impl std::error::Error for InvalidDebuggerAudit {}

/// Checks metadata only. The host must separately check authority, time, the
/// pending operation, and persisted command/idempotency identity before mutation.
pub fn validate_debugger_audit(audit: &DebuggerAuditContext) -> Result<(), InvalidDebuggerAudit> {
    let valid = |value: &str| {
        !value.is_empty()
            && value.len() <= 128
            && value.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.')
            })
    };
    if !valid(&audit.session_id) || !valid(&audit.principal.id) {
        return Err(InvalidDebuggerAudit);
    }
    Ok(())
}

/// Projects caller-supplied metadata without validation, authorization or commit.
/// A returned record is not evidence that a cancellation happened.
pub fn debugger_audit_record(
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

/// Compares attribution only, intentionally excluding the original commit time.
/// A match is not replay permission: the host must also match the persisted
/// continuation and idempotency key, which are not part of this record.
pub fn audit_matches_record(audit: &DebuggerAuditContext, record: &DebuggerAuditRecord) -> bool {
    record.command_id == audit.command_id
        && record.principal_id == audit.principal.id
        && record.origin == audit.origin
        && record.session_id == audit.session_id
        && record.expected_revision == audit.expected_revision
}

/// Exact product persistence labels; independent of any storage engine.
pub fn command_origin_label(origin: CommandOrigin) -> &'static str {
    match origin {
        CommandOrigin::Gui => "gui",
        CommandOrigin::Cli => "cli",
        CommandOrigin::Leselang => "leselang",
        CommandOrigin::Model => "model",
        CommandOrigin::CompatibilityAdapter => "compatibility_adapter",
    }
}

/// No aliases or normalization: unknown persisted labels remain invalid.
pub fn parse_command_origin(value: &str) -> Option<CommandOrigin> {
    match value {
        "gui" => Some(CommandOrigin::Gui),
        "cli" => Some(CommandOrigin::Cli),
        "leselang" => Some(CommandOrigin::Leselang),
        "model" => Some(CommandOrigin::Model),
        "compatibility_adapter" => Some(CommandOrigin::CompatibilityAdapter),
        _ => None,
    }
}
