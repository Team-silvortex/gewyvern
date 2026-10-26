use super::*;
use leselang_command::reference_compaction_records::{
    CompactionRecordSource, GroupCompactionOwner, StoredCompactionBranch, StoredCompactionGroup,
};

pub(super) struct SqliteCompactionRead<'a>(pub &'a Connection);
impl CompactionRecordSource for SqliteCompactionRead<'_> {
    fn effect_bytes(&mut self, token: &str) -> Result<i64, Fault> {
        self.0
            .query_row(
                "SELECT length(e.image) + length(e.terminal_step) +
                    COALESCE(length(d.request), 0) +
                    COALESCE(length(d.last_error), 0)
             FROM vm_effects e
             LEFT JOIN vm_dispatches d ON d.token = e.token
             WHERE e.token = ?1 AND e.state = 'completed'",
                [token],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error("LSV4030", "failed to size completed journal record", error)
            })
    }
    fn completed_tokens(&mut self) -> Result<BTreeSet<String>, Fault> {
        let mut statement = self
            .0
            .prepare("SELECT token FROM vm_effects WHERE state = 'completed'")
            .map_err(|error| {
                journal_error(
                    "LSV4030",
                    "failed to prepare completed journal reconciliation",
                    error,
                )
            })?;
        statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(|error| {
                journal_error(
                    "LSV4030",
                    "failed to scan completed journal reconciliation",
                    error,
                )
            })?
            .collect::<Result<BTreeSet<_>, _>>()
            .map_err(|error| {
                journal_error(
                    "LSV4030",
                    "failed to read completed journal reconciliation",
                    error,
                )
            })
    }
}

pub(super) struct SqliteGroupCompaction<'a, 'connection>(
    pub &'a rusqlite::Transaction<'connection>,
);
impl GroupCompactionOwner for SqliteGroupCompaction<'_, '_> {
    fn group(&mut self, token: &str) -> Result<StoredCompactionGroup, Fault> {
        self.0
            .query_row(
                "SELECT length(plan) + length(terminal_step), plan
             FROM vm_merge_groups WHERE token = ?1 AND state = 'completed'",
                [token],
                |row| {
                    Ok(StoredCompactionGroup {
                        bytes: row.get(0)?,
                        plan: row.get(1)?,
                    })
                },
            )
            .map_err(|error| {
                journal_error("LSV4030", "failed to size completed merge group", error)
            })
    }
    fn branches(&mut self, token: &str) -> Result<Vec<StoredCompactionBranch>, Fault> {
        let mut statement = self
            .0
            .prepare(
                "SELECT e.token,
                        length(e.image) + length(e.terminal_step) +
                        COALESCE(length(d.request), 0) +
                        COALESCE(length(d.last_error), 0),
                        e.state
                 FROM vm_merge_branches b
                 JOIN vm_effects e ON e.token = b.branch_token
                 LEFT JOIN vm_dispatches d ON d.token = e.token
                 WHERE b.group_token = ?1
                 ORDER BY b.position ASC",
            )
            .map_err(|error| {
                journal_error("LSV4030", "failed to prepare merge group deletion", error)
            })?;
        statement
            .query_map([token], |row| {
                Ok(StoredCompactionBranch {
                    token: row.get(0)?,
                    bytes: row.get(1)?,
                    state: row.get(2)?,
                })
            })
            .map_err(|error| {
                journal_error("LSV4030", "failed to scan merge group deletion", error)
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|error| journal_error("LSV4030", "failed to read merge group deletion", error))
    }
    fn delete_group(&mut self, token: &str) -> Result<usize, Fault> {
        self.0
            .execute(
                "DELETE FROM vm_merge_groups WHERE token = ?1 AND state = 'completed'",
                [token],
            )
            .map_err(|error| {
                journal_error("LSV4030", "failed to delete completed merge group", error)
            })
    }
    fn delete_branch(&mut self, token: &str) -> Result<usize, Fault> {
        self.0
            .execute(
                "DELETE FROM vm_effects WHERE token = ?1 AND state = 'completed'",
                [token],
            )
            .map_err(|error| journal_error("LSV4030", "failed to delete merge branch", error))
    }
}
