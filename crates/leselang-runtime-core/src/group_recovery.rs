//! Ordered inspection of one stored group inside an adapter-owned recovery transaction.

use std::{fmt, hash::Hash};

use crate::{
    GroupGraphError, GroupGraphMember, GroupReceiptError, StoredGroupState, validate_group_graph,
    validate_group_receipt,
};

/// Derived from the already-admitted original plan/state, not an execution grant.
/// These flags select native proof obligations; untrusted input must not choose
/// which checks to skip. Limits never expand or repair stored data.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GroupRecoveryPolicy {
    pub max_members: usize,
    pub require_chain: bool,
    pub require_pending_sequence: bool,
}

/// Native schema, codec, chain, dispatch, receipt and reservation proof policy.
/// All methods must inspect the same original snapshot. Member identities and
/// completion classification must be faithful; native work and ingress are bounded
/// by the adapter. Admission may inspect receipts but must not execute saved code.
pub trait GroupRecoveryHost<Row, Key: ?Sized> {
    type Encoded;
    type Receipt;
    type Error;

    fn member<'a>(
        &mut self,
        index: usize,
        row: &'a Row,
    ) -> Result<GroupGraphMember<'a, Key>, Self::Error>
    where
        Key: 'a;
    fn is_completed(&mut self, row: &Row) -> bool;
    fn validate_chain(&mut self) -> Result<(), Self::Error>;
    fn validate_pending_sequence(&mut self, rows: &[Row]) -> Result<(), Self::Error>;
    fn stored_state(&mut self) -> Result<StoredGroupState, Self::Error>;
    fn admit_receipt(&mut self, encoded: Self::Encoded) -> Result<Self::Receipt, Self::Error>;
    fn validate_records(&mut self, receipt: Option<&Self::Receipt>) -> Result<(), Self::Error>;
}

/// Refusal stage plus the unchanged native error, without private diagnostic data.
pub enum GroupRecoveryError<Native> {
    Graph(GroupGraphError<Native>),
    Chain(Native),
    Sequence(Native),
    State(Native),
    Receipt(GroupReceiptError<Native>),
    Records(Native),
}
impl<Native> fmt::Display for GroupRecoveryError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Graph(_) => "invalid recovered group members",
            Self::Chain(_) => "invalid recovered group chain",
            Self::Sequence(_) => "invalid recovered sequence frontier",
            Self::State(_) => "invalid recovered group state",
            Self::Receipt(_) => "invalid recovered group receipt",
            Self::Records(_) => "invalid recovered group records",
        })
    }
}
impl<Native> fmt::Debug for GroupRecoveryError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for GroupRecoveryError<Native> {}

/// Inspect members, completion, optional chain, optional sequence, state/receipt,
/// then final native records. Graph limits precede all host calls. Completion
/// observation short-circuits on the first incomplete row. State decoding is
/// deliberately late; an invalid state cannot hide an earlier chain refusal.
/// Only completed groups with complete members and a receipt reach receipt
/// admission. Final records are checked even for pending groups with no receipt.
///
/// Opaque rows, encoded inputs, receipts and errors need no Clone/Debug/serde/thread
/// traits. Only borrowed keys need Eq + Hash. Receipt ownership is returned exactly
/// once after the last gate; late refusal drops it without publishing partial state.
/// The host remains caller-owned. Hooks and destructors are trusted: unwind stops
/// the pass without retry/rollback, and a second cleanup panic may abort.
///
/// This is not a durable recovery certificate. The adapter must admit owner/plan,
/// check cross-group identities and original allocation/authority/budgets, bound
/// all decoded data, and hold one transaction through final commit before publishing
/// recovered state. No fuel, allocator, evaluator, dispatch, replay or storage write
/// is introduced here; use checkpoint staging around the complete recovery pass.
pub fn inspect_group_recovery<
    Key: ?Sized + Eq + Hash,
    Name: AsRef<str>,
    Row,
    Host: GroupRecoveryHost<Row, Key> + ?Sized,
>(
    owner: &Key,
    declared: &[Name],
    rows: &[Row],
    policy: GroupRecoveryPolicy,
    encoded: Option<Host::Encoded>,
    host: &mut Host,
) -> Result<Option<Host::Receipt>, GroupRecoveryError<Host::Error>> {
    validate_group_graph(owner, declared, rows, policy.max_members, |index, row| {
        host.member(index, row)
    })
    .map_err(GroupRecoveryError::Graph)?;
    let all_completed = rows.iter().all(|row| host.is_completed(row));
    if policy.require_chain {
        host.validate_chain().map_err(GroupRecoveryError::Chain)?;
    }
    if policy.require_pending_sequence {
        host.validate_pending_sequence(rows)
            .map_err(GroupRecoveryError::Sequence)?;
    }
    let state = host.stored_state().map_err(GroupRecoveryError::State)?;
    let receipt = validate_group_receipt(state, all_completed, encoded, |input| {
        host.admit_receipt(input)
    })
    .map_err(GroupRecoveryError::Receipt)?;
    host.validate_records(receipt.as_ref())
        .map_err(GroupRecoveryError::Records)?;
    Ok(receipt)
}
