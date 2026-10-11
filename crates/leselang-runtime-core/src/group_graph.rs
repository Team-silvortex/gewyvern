use std::{collections::HashSet, fmt, hash::Hash};

pub const MAX_GROUP_GRAPH_MEMBERS: usize = 64;

/// Original adapter-owned row observations, not a decoded or authentic receipt.
/// A missing/unrepresentable position is invalid; it is never renumbered.
pub struct GroupGraphMember<'graph, Key: ?Sized> {
    pub name: &'graph str,
    pub position: Option<usize>,
    pub identity: &'graph Key,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupGraphMismatch {
    Position,
    Name,
    Owner,
    Duplicate,
}

/// Native errors are moved unchanged, never formatted or exposed as Error::source.
pub enum GroupGraphError<Native> {
    InvalidLimits,
    Count,
    Member {
        index: usize,
        mismatch: GroupGraphMismatch,
    },
    Native {
        index: usize,
        error: Native,
    },
}

impl<Native> fmt::Display for GroupGraphError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidLimits => "group graph limit exceeds safety ceiling",
            Self::Count => "group graph must match the complete declared member count",
            Self::Member { .. } => "group graph member order or identity differs",
            Self::Native { .. } => "group graph native row observation failed",
        })
    }
}

impl<Native> fmt::Debug for GroupGraphError<Native> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, formatter)
    }
}

impl<Native> std::error::Error for GroupGraphError<Native> {}

/// Inspect one complete flat group against the original ordered declaration.
/// Inclusive count bounds precede allocation and every native hook. Empty groups
/// are structurally allowed: language arity and declaration-name policy remain
/// host-owned. A zero limit admits only an empty group; limits never expand.
///
/// Observe each original row once, in order, before checking its position, name,
/// owner exclusion and duplicate identity. This preserves adapters that validate
/// a request before detecting its identity/name mismatch. The callback must bound
/// native work and expose original native identity/position without normalization.
/// Later rows are not observed after refusal or unwind. Keys need only Eq + Hash;
/// rows/errors need no Clone/Debug/serde/Send/Sync/Default or owned conversion.
/// Borrowed key indexing is bounded to one group, with no payload copies or sort.
/// The callback owner is released while the result/error remains local, before
/// handoff. A second cleanup panic may abort; native work cannot be rolled back.
///
/// Success is a transient structural observation, not a durable graph certificate
/// or execution grant. It does not authenticate keys, bind expected owner metadata,
/// validate values/types, detect cross-group reuse, reserve sequence identities,
/// inspect nested graphs or reconstruct fuel. Storage adapters must keep the same
/// snapshot, validate native schema/policy, reservations, dispatch/terminal state
/// and receipts, then commit before publishing recovery. No source/body execution,
/// fallback, dispatch, persistence, replay or callback retry happens here.
pub fn validate_group_graph<
    'graph,
    Key: ?Sized + Eq + Hash + 'graph,
    Name: AsRef<str>,
    Row,
    Native,
>(
    owner: &Key,
    declared: &[Name],
    rows: &'graph [Row],
    max_members: usize,
    mut observe: impl FnMut(usize, &'graph Row) -> Result<GroupGraphMember<'graph, Key>, Native>,
) -> Result<(), GroupGraphError<Native>> {
    let outcome = (|| {
        if max_members > MAX_GROUP_GRAPH_MEMBERS {
            return Err(GroupGraphError::InvalidLimits);
        }
        if declared.len() > max_members || rows.len() != declared.len() {
            return Err(GroupGraphError::Count);
        }
        let mut identities = HashSet::with_capacity(rows.len());
        for (index, row) in rows.iter().enumerate() {
            let member =
                observe(index, row).map_err(|error| GroupGraphError::Native { index, error })?;
            let mismatch = if member.position != Some(index) {
                Some(GroupGraphMismatch::Position)
            } else if member.name != declared[index].as_ref() {
                Some(GroupGraphMismatch::Name)
            } else if member.identity == owner {
                Some(GroupGraphMismatch::Owner)
            } else if !identities.insert(member.identity) {
                Some(GroupGraphMismatch::Duplicate)
            } else {
                None
            };
            if let Some(mismatch) = mismatch {
                return Err(GroupGraphError::Member { index, mismatch });
            }
        }
        Ok(())
    })();
    drop(observe);
    outcome
}
