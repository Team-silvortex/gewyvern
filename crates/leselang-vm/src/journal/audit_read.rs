use leselang_command::DebuggerAuditRecord;
use leselang_command::reference_audit_read::{self, ReferenceAuditReadRow, StoredAudit};

struct SqliteAuditReadRow<'a, 'row>(&'a rusqlite::Row<'row>);

impl ReferenceAuditReadRow for SqliteAuditReadRow<'_, '_> {
    type Error = rusqlite::Error;
    fn text(&mut self, column: usize) -> Result<String, Self::Error> {
        self.0.get(column)
    }
    fn integer(&mut self, column: usize) -> Result<i64, Self::Error> {
        self.0.get(column)
    }
    fn invalid_data(&mut self) -> Self::Error {
        rusqlite::Error::InvalidQuery
    }
}

pub(super) fn decode_record(row: &rusqlite::Row<'_>) -> rusqlite::Result<DebuggerAuditRecord> {
    reference_audit_read::decode_record(&mut SqliteAuditReadRow(row))
}

pub(super) fn decode_replay(row: &rusqlite::Row<'_>) -> rusqlite::Result<StoredAudit> {
    reference_audit_read::decode_replay(&mut SqliteAuditReadRow(row))
}
