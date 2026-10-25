//! Leserpent reference journal schema and ordered compatibility migrations.
//!
//! SQL is product schema data, not a portable native storage implementation.

use leselang_runtime_core::Fault;

pub const JOURNAL_SCHEMA_VERSION: u32 = 10;

/// Execute the released schema upgrade path on one caller-owned connection.
/// Read the version only after native path, permission, connection and safety
/// configuration gates. The trusted callback executes each exact SQL batch and
/// maps its native failure using the supplied diagnostic context.
///
/// Each historical upgrade commits separately. Stop on the first fault or unwind
/// without retry, compensation or pretending earlier committed steps rolled back.
/// The final idempotent index repair also runs for the current version. This is
/// not checkpoint recovery, live authority validation or a storage transaction.
/// Callback cleanup is also trusted and may unwind after successful commits.
/// No Send, static owner, timer or SQLite dependency is imposed.
pub fn migrate(
    version: u32,
    mut execute: impl FnMut(&'static str, &'static str) -> Result<(), Fault>,
) -> Result<(), Fault> {
    if version == 0 {
        execute(
            "BEGIN IMMEDIATE;
                     CREATE TABLE IF NOT EXISTS vm_metadata (
                       singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                       next_sequence INTEGER NOT NULL CHECK (next_sequence >= 1)
                     ) STRICT;
                     INSERT OR IGNORE INTO vm_metadata(singleton, next_sequence) VALUES (1, 1);
                     CREATE TABLE IF NOT EXISTS vm_effects (
                       token TEXT PRIMARY KEY,
                       state TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
                       image BLOB NOT NULL,
                       deadline_at_ms INTEGER CHECK (deadline_at_ms IS NULL OR deadline_at_ms >= 0),
                       terminal_step BLOB,
                       CHECK ((state = 'pending' AND terminal_step IS NULL) OR
                              (state = 'completed' AND terminal_step IS NOT NULL))
                     ) STRICT;
                     CREATE TABLE IF NOT EXISTS vm_dispatches (
                       token TEXT PRIMARY KEY REFERENCES vm_effects(token) ON DELETE CASCADE,
                       request BLOB NOT NULL,
                       state TEXT NOT NULL CHECK (state IN ('ready', 'leased', 'acknowledged')),
                       attempt INTEGER NOT NULL CHECK (attempt >= 0),
                       lease_expires_at_ms INTEGER,
                       ready_at_ms INTEGER NOT NULL DEFAULT 0 CHECK (ready_at_ms >= 0),
                       retry_count INTEGER NOT NULL DEFAULT 0
                         CHECK (retry_count >= 0 AND retry_count <= 16),
                       last_error BLOB,
                       CHECK ((state = 'ready' AND lease_expires_at_ms IS NULL) OR
                              (state = 'leased' AND lease_expires_at_ms IS NOT NULL) OR
                              (state = 'acknowledged' AND lease_expires_at_ms IS NULL))
                     ) STRICT;
                     CREATE INDEX vm_effect_deadline_idx
                       ON vm_effects(state, deadline_at_ms);
                     CREATE INDEX vm_dispatch_ready_idx
                       ON vm_dispatches(state, ready_at_ms, lease_expires_at_ms);
                     PRAGMA user_version = 4;
                     COMMIT;",
            "failed to initialize journal",
        )?;
    } else if version == 1 {
        execute(
            "BEGIN IMMEDIATE;
                     CREATE TABLE vm_dispatches (
                       token TEXT PRIMARY KEY REFERENCES vm_effects(token) ON DELETE CASCADE,
                       request BLOB NOT NULL,
                       state TEXT NOT NULL CHECK (state IN ('ready', 'leased', 'acknowledged')),
                       attempt INTEGER NOT NULL CHECK (attempt >= 0),
                       lease_expires_at_ms INTEGER,
                       ready_at_ms INTEGER NOT NULL DEFAULT 0 CHECK (ready_at_ms >= 0),
                       retry_count INTEGER NOT NULL DEFAULT 0
                         CHECK (retry_count >= 0 AND retry_count <= 16),
                       last_error BLOB,
                       CHECK ((state = 'ready' AND lease_expires_at_ms IS NULL) OR
                              (state = 'leased' AND lease_expires_at_ms IS NOT NULL) OR
                              (state = 'acknowledged' AND lease_expires_at_ms IS NULL))
                     ) STRICT;
                     ALTER TABLE vm_effects ADD COLUMN deadline_at_ms INTEGER
                       CHECK (deadline_at_ms IS NULL OR deadline_at_ms >= 0);
                     CREATE INDEX vm_effect_deadline_idx
                       ON vm_effects(state, deadline_at_ms);
                     CREATE INDEX vm_dispatch_ready_idx
                       ON vm_dispatches(state, ready_at_ms, lease_expires_at_ms);
                     PRAGMA user_version = 4;
                     COMMIT;",
            "failed to migrate journal",
        )?;
    } else if version == 2 {
        execute(
            "BEGIN IMMEDIATE;
                     ALTER TABLE vm_effects ADD COLUMN deadline_at_ms INTEGER
                       CHECK (deadline_at_ms IS NULL OR deadline_at_ms >= 0);
                     CREATE INDEX vm_effect_deadline_idx
                       ON vm_effects(state, deadline_at_ms);
                     ALTER TABLE vm_dispatches ADD COLUMN ready_at_ms INTEGER NOT NULL DEFAULT 0
                       CHECK (ready_at_ms >= 0);
                     ALTER TABLE vm_dispatches ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0
                       CHECK (retry_count >= 0 AND retry_count <= 16);
                     ALTER TABLE vm_dispatches ADD COLUMN last_error BLOB;
                     CREATE INDEX vm_dispatch_ready_idx
                       ON vm_dispatches(state, ready_at_ms, lease_expires_at_ms);
                     PRAGMA user_version = 4;
                     COMMIT;",
            "failed to migrate journal",
        )?;
    } else if version == 3 {
        execute(
            "BEGIN IMMEDIATE;
                     ALTER TABLE vm_dispatches ADD COLUMN ready_at_ms INTEGER NOT NULL DEFAULT 0
                       CHECK (ready_at_ms >= 0);
                     ALTER TABLE vm_dispatches ADD COLUMN retry_count INTEGER NOT NULL DEFAULT 0
                       CHECK (retry_count >= 0 AND retry_count <= 16);
                     ALTER TABLE vm_dispatches ADD COLUMN last_error BLOB;
                     CREATE INDEX vm_dispatch_ready_idx
                       ON vm_dispatches(state, ready_at_ms, lease_expires_at_ms);
                     PRAGMA user_version = 4;
                     COMMIT;",
            "failed to migrate journal",
        )?;
    } else if !matches!(version, 4 | 5 | 6 | 7 | 8 | 9 | JOURNAL_SCHEMA_VERSION) {
        return Err(fault(
            "LSV4004",
            format!(
                "unsupported journal version {version}, expected 1 through {JOURNAL_SCHEMA_VERSION}"
            ),
        ));
    }

    if version < 5 {
        execute(
            "BEGIN IMMEDIATE;
                 CREATE TABLE vm_merge_groups (
                   token TEXT PRIMARY KEY,
                   plan BLOB NOT NULL,
                   state TEXT NOT NULL CHECK (state IN ('pending', 'completed')),
                   terminal_step BLOB,
                   CHECK ((state = 'pending' AND terminal_step IS NULL) OR
                          (state = 'completed' AND terminal_step IS NOT NULL))
                 ) STRICT;
                 CREATE TABLE vm_merge_branches (
                   group_token TEXT NOT NULL
                     REFERENCES vm_merge_groups(token) ON DELETE CASCADE,
                   branch_token TEXT NOT NULL UNIQUE
                     REFERENCES vm_effects(token) ON DELETE CASCADE,
                   branch_name TEXT NOT NULL,
                   position INTEGER NOT NULL CHECK (position >= 0 AND position < 64),
                   PRIMARY KEY (group_token, branch_name),
                   UNIQUE (group_token, position)
                 ) STRICT;
                 CREATE INDEX vm_merge_branch_group_idx
                   ON vm_merge_branches(group_token, position);
                 PRAGMA user_version = 5;
                 COMMIT;",
            "failed to migrate merge journal",
        )?;
    }

    if version < 6 {
        execute("BEGIN IMMEDIATE;
                     CREATE TABLE vm_debugger_audit (
                       command_id TEXT PRIMARY KEY,
                       principal_id TEXT NOT NULL,
                       idempotency_key TEXT NOT NULL,
                       origin TEXT NOT NULL CHECK (origin IN ('gui', 'cli', 'leselang', 'model', 'compatibility_adapter')),
                       session_id TEXT NOT NULL,
                       expected_revision INTEGER NOT NULL CHECK (expected_revision >= 0),
                       observed_at_ms INTEGER NOT NULL CHECK (observed_at_ms >= 0),
                       continuation_token TEXT NOT NULL
                         REFERENCES vm_effects(token) ON DELETE CASCADE,
                       UNIQUE (principal_id, idempotency_key)
                     ) STRICT;
                     CREATE INDEX vm_debugger_audit_token_idx
                       ON vm_debugger_audit(continuation_token);
                     PRAGMA user_version = 6;
                     COMMIT;", "failed to migrate debugger audit journal")?;
    }

    if version < 7 {
        execute("BEGIN IMMEDIATE;
                 ALTER TABLE vm_merge_groups ADD COLUMN execution_order TEXT NOT NULL DEFAULT 'parallel'
                   CHECK (execution_order IN ('parallel', 'sequential'));
                 PRAGMA user_version = 7;
                 COMMIT;", "failed to migrate control flow journal")?;
    }

    if version < 8 {
        execute(
            "BEGIN IMMEDIATE; PRAGMA user_version = 8; COMMIT;",
            "failed to migrate result-chain journal",
        )?;
    }

    if version < 9 {
        execute(
            "BEGIN IMMEDIATE; PRAGMA user_version = 9; COMMIT;",
            "failed to migrate dataflow journal",
        )?;
    }

    if version < 10 {
        execute(
            "BEGIN IMMEDIATE; PRAGMA user_version = 10; COMMIT;",
            "failed to migrate conditional-exit journal",
        )?;
    }

    // Canonical tokens share one prefix and have no leading zeroes, so length/text
    // order is numeric admission order without casts or a new persisted sequence.
    execute(
        "CREATE INDEX IF NOT EXISTS vm_dispatch_attempt_order_idx
                 ON vm_dispatches(attempt, length(token), token)
                 WHERE state IN ('ready', 'leased');",
        "failed to index dispatch ordering",
    )?;
    Ok(())
}

fn fault(code: &str, message: impl Into<String>) -> Fault {
    Fault {
        code: code.into(),
        message: message.into(),
    }
}
