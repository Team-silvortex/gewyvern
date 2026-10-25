//! Reference debugger-audit recovery policy over one native checkpoint snapshot.
//!
//! A successful check is not cancellation, replay permission or live authority.

use crate::continuation_identity::valid_continuation_token;
use crate::debugger_audit::{parse_command_origin, validate_debugger_audit};
use crate::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use crate::{Cancellation, CancellationReason, ContinuationToken, DebuggerAuditContext, Step};
use leselang_host_contract::{Principal, Revision};
use leselang_runtime_core::Fault;
use leserpent_domain::{CommandId, IdempotencyKey};

/// Columns of the released reference audit join, not a portable storage schema.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditTextColumn {
    Command,
    Principal,
    IdempotencyKey,
    Origin,
    Session,
    Continuation,
    EffectState,
}

impl AuditTextColumn {
    pub fn index(self) -> usize {
        match self {
            Self::Command => 0,
            Self::Principal => 1,
            Self::IdempotencyKey => 2,
            Self::Origin => 3,
            Self::Session => 4,
            Self::Continuation => 7,
            Self::EffectState => 8,
        }
    }
    pub fn error_context(self) -> &'static str {
        match self {
            Self::Command => "invalid debugger audit command",
            Self::Principal => "invalid debugger audit principal",
            Self::IdempotencyKey => "invalid debugger audit idempotency key",
            Self::Origin => "invalid debugger audit origin",
            Self::Session => "invalid debugger audit session",
            Self::Continuation => "invalid debugger audit continuation",
            Self::EffectState => "invalid debugger audit effect state",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AuditIntegerColumn {
    Revision,
    ObservedAt,
}
impl AuditIntegerColumn {
    pub fn index(self) -> usize {
        match self {
            Self::Revision => 5,
            Self::ObservedAt => 6,
        }
    }
    pub fn error_context(self) -> &'static str {
        match self {
            Self::Revision => "invalid debugger audit revision",
            Self::ObservedAt => "invalid debugger audit clock",
        }
    }
}

/// Lazy reads from the same joined row. Native read faults retain their original
/// diagnostics. The invalid-origin error preserves the owner's existing native
/// representation without importing a storage engine. Bytes may be borrowed;
/// neither Clone, Send nor an owned/static checkpoint is required.
pub trait ReferenceAuditRow {
    type Bytes: AsRef<[u8]>;
    type InvalidOrigin: std::fmt::Display;
    fn read_text(&mut self, column: AuditTextColumn) -> Result<String, Fault>;
    fn read_integer(&mut self, column: AuditIntegerColumn) -> Result<i64, Fault>;
    fn read_terminal(&mut self) -> Result<Self::Bytes, Fault>;
    fn invalid_origin(&mut self) -> Self::InvalidOrigin;
}

/// Read and inspect exactly one joined record in released fault-precedence order.
/// Invalid context or token skips later state/payload reads. No normalization,
/// repair, publication, execution or broader terminal validation is introduced.
pub fn inspect_audit_record<Row: ReferenceAuditRow + ?Sized>(row: &mut Row) -> Result<(), Fault> {
    let command_id = CommandId::new(row.read_text(AuditTextColumn::Command)?)
        .map_err(|_| invalid("debugger audit command is invalid"))?;
    let principal_id = row.read_text(AuditTextColumn::Principal)?;
    let idempotency_key = IdempotencyKey::new(row.read_text(AuditTextColumn::IdempotencyKey)?)
        .map_err(|_| invalid("debugger audit idempotency key is invalid"))?;
    let origin =
        parse_command_origin(&row.read_text(AuditTextColumn::Origin)?).ok_or_else(|| {
            invalid(format!(
                "debugger audit origin is invalid: {}",
                row.invalid_origin()
            ))
        })?;
    let session_id = row.read_text(AuditTextColumn::Session)?;
    let expected_revision = u64::try_from(row.read_integer(AuditIntegerColumn::Revision)?)
        .map_err(|_| invalid("debugger audit revision is invalid"))?;
    let observed_at_ms = u64::try_from(row.read_integer(AuditIntegerColumn::ObservedAt)?)
        .map_err(|_| invalid("debugger audit clock is invalid"))?;
    let token = ContinuationToken::from_wire(row.read_text(AuditTextColumn::Continuation)?);
    let context = DebuggerAuditContext {
        command_id,
        idempotency_key,
        principal: Principal { id: principal_id },
        origin,
        session_id,
        expected_revision: Revision(expected_revision),
    };
    validate_debugger_audit(&context).map_err(|_| invalid("debugger audit identity is invalid"))?;
    if !valid_continuation_token(&token)
        || row.read_text(AuditTextColumn::EffectState)? != "completed"
    {
        return Err(invalid("debugger audit continuation is invalid"));
    }
    let terminal_bytes = row.read_terminal()?;
    let terminal: Step = decode_journal_value(terminal_bytes.as_ref(), MAX_JOURNAL_ENTRY_BYTES)?;
    if !matches!(terminal, Step::Cancelled(Cancellation {
        continuation, reason: CancellationReason::Requested, observed_at_ms: terminal_observed_at,
    }) if continuation == token && terminal_observed_at == observed_at_ms)
    {
        return Err(invalid(
            "debugger audit does not match its terminal cancellation",
        ));
    }
    Ok(())
}

/// All hooks must observe the original checkpoint transaction. Scan in command-id
/// order, call inspect_audit_record once per joined row, stop at its first error,
/// and return only the number of successfully inspected rows. Release cursors
/// before return. A permissive callback is not evidence of valid persisted audits.
/// Native reads remain bounded by the owner; no hook may repair, commit or dispatch.
pub trait ReferenceAuditSource {
    fn index_count(&mut self) -> Result<u32, Fault>;
    fn audit_count(&mut self) -> Result<u32, Fault>;
    fn inspect_records(&mut self) -> Result<u32, Fault>;
}

/// Validate schema presence, then the joined stream and its complete cardinality.
/// Errors/unwinds propagate without retries, compensation or allocator mutation.
pub fn validate_audit_records<Source: ReferenceAuditSource + ?Sized>(
    source: &mut Source,
) -> Result<(), Fault> {
    if source.index_count()? != 1 {
        return Err(invalid("debugger audit schema is incomplete"));
    }
    let expected = source.audit_count()?;
    if source.inspect_records()? != expected {
        return Err(invalid("debugger audit references a missing continuation"));
    }
    Ok(())
}

fn invalid(message: impl Into<String>) -> Fault {
    Fault {
        code: "LSV4007".into(),
        message: message.into(),
    }
}
