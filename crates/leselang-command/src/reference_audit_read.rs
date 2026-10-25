//! Reference audit-row projections, without query or cancellation authority.

use crate::debugger_audit::{DebuggerAuditRecord, parse_command_origin};
use leselang_host_contract::Revision;
use leserpent_domain::CommandId;

/// Original replay projection: command, continuation, key, attribution.
pub type StoredAudit = (String, String, String, DebuggerAuditRecord);

/// Lazy native reads from one original projection. Indices describe the released
/// record/replay SELECT layouts, not a portable database schema. Preserve native
/// type errors and map invalid fields to the owner's existing invalid-data error.
/// No Clone, Display, Send, or static error/owner bounds are required.
pub trait ReferenceAuditReadRow {
    type Error;
    fn text(&mut self, column: usize) -> Result<String, Self::Error>;
    fn integer(&mut self, column: usize) -> Result<i64, Self::Error>;
    fn invalid_data(&mut self) -> Self::Error;
}

/// Command admission is immediate; later native reads must not mask its error.
/// Principal/session are projected verbatim, not revalidated or normalized here.
/// Successful decoding does not prove a cancellation or authorize replay.
pub fn decode_record<Row: ReferenceAuditReadRow + ?Sized>(
    row: &mut Row,
) -> Result<DebuggerAuditRecord, Row::Error> {
    let command_id = CommandId::new(row.text(0)?).map_err(|_| row.invalid_data())?;
    let principal_id = row.text(1)?;
    let origin = parse_command_origin(&row.text(2)?).ok_or_else(|| row.invalid_data())?;
    let session_id = row.text(3)?;
    let expected_revision = u64::try_from(row.integer(4)?).map_err(|_| row.invalid_data())?;
    let observed_at_ms = u64::try_from(row.integer(5)?).map_err(|_| row.invalid_data())?;
    Ok(DebuggerAuditRecord {
        command_id,
        principal_id,
        origin,
        session_id,
        expected_revision: Revision(expected_revision),
        observed_at_ms,
    })
}

/// Replay lookup reads the three identity strings before command admission. Keep
/// this precedence separate from record lookup. The raw token/key are observations
/// only; identity matching and terminal evidence belong to the later commit gate.
pub fn decode_replay<Row: ReferenceAuditReadRow + ?Sized>(
    row: &mut Row,
) -> Result<StoredAudit, Row::Error> {
    let command_id = row.text(0)?;
    let token = row.text(1)?;
    let key = row.text(2)?;
    let record = DebuggerAuditRecord {
        command_id: CommandId::new(command_id.clone()).map_err(|_| row.invalid_data())?,
        principal_id: row.text(3)?,
        origin: parse_command_origin(&row.text(4)?).ok_or_else(|| row.invalid_data())?,
        session_id: row.text(5)?,
        expected_revision: Revision(
            u64::try_from(row.integer(6)?).map_err(|_| row.invalid_data())?,
        ),
        observed_at_ms: u64::try_from(row.integer(7)?).map_err(|_| row.invalid_data())?,
    };
    Ok((command_id, token, key, record))
}
