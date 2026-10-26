//! Closed original-row reply assembly, not a receipt or group execution grant.

use std::{convert::Infallible, fmt};

use leselang_runtime_core::Fuel;

use crate::group_exports::{
    GroupExportError, GroupExportLimits, MAX_GROUP_EXPORT_MEMBERS, atomic_candidate, physical,
};
use crate::ir::{Computation, GroupKind};
use crate::native_group_lookup::{NativeGroupLookupError, lookup_native_group_member};
use crate::pure_typing::{MAX_TYPE_INFERENCE_DEPTH, MAX_TYPE_INFERENCE_NODES};
use crate::sequence_evaluation::{SequenceAccepted, SequenceEvaluationLimits};

type Node<Field, Operation, HostEffect, IrResult> =
    Computation<Field, Operation, HostEffect, IrResult>;
pub type GroupReplyResult<'expression, Node, IrResult, Identity, Declaration, Reply, Error> =
    Result<
        GroupReply<'expression, Node, IrResult, Identity, Declaration, Reply>,
        GroupReplyError<Error>,
    >;

pub enum GroupReplyError<Native> {
    InvalidLimits,
    NotGroup,
    Shape(GroupExportError<Infallible>),
    Candidate { index: usize },
    Count,
    Row { index: usize },
    Fuel,
    Native(Native),
}
impl<Native> fmt::Display for GroupReplyError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidLimits => "group reply limits exceed safety ceilings",
            Self::NotGroup => "group reply requires an original flat group",
            Self::Shape(_) => "group reply original IR has invalid shape",
            Self::Candidate { .. } => "group reply member is not an atomic candidate",
            Self::Count => "group reply must contain the complete original member count",
            Self::Row { .. } => "group reply order or original row identity differs",
            Self::Fuel => "group reply assembly fuel exhausted",
            Self::Native(_) => "group reply fresh whole-batch native admission failed",
        })
    }
}
impl<Native> fmt::Debug for GroupReplyError<Native> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for GroupReplyError<Native> {}

/// Complete once-ordered accepted inputs tied to one original Seq/All IR.
/// Private membership prevents post-assembly truncation/reordering without
/// consuming this object. It is move-only data, not authentic/global authority.
/// Original row pointer agreement does not prove identity generation, provenance,
/// declaration authenticity, current native value/type or live grants. Mandatory
/// assembly/restore admission checks those facts; reads/projections must recheck relevant
/// current policy, since interior native data can change after any observation.
/// No native Clone/Debug/equality/serde/Send/Sync bound or implicit coercion exists.
///
/// ```compile_fail
/// use leselang_hir_core::group_reply::GroupReply;
/// fn duplicate(reply: GroupReply<'_, (), (), u64, (), ()>) { let copy = reply.clone(); }
/// ```
///
/// ```compile_fail
/// use leselang_hir_core::group_reply::GroupReply;
/// fn wire(reply: GroupReply<'_, (), (), u64, (), ()>) { let wire = serde_json::to_string(&reply); }
/// ```
#[must_use = "retain complete accepted input until projection or deliberate cleanup"]
pub struct GroupReply<'expression, Node, IrResult, Identity, Declaration: ?Sized, Reply> {
    group: &'expression Node,
    kind: GroupKind,
    members: Vec<SequenceAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>>,
}
impl<'expression, Node, IrResult, Identity, Declaration: ?Sized, Reply>
    GroupReply<'expression, Node, IrResult, Identity, Declaration, Reply>
{
    pub fn group(&self) -> &'expression Node {
        self.group
    }
    pub fn kind(&self) -> GroupKind {
        self.kind
    }
    pub fn members(
        &self,
    ) -> &[SequenceAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>] {
        &self.members
    }

    /// Fresh bounded closed-name lookup and mandatory native selected-row policy.
    /// Returns the same borrowed accepted input, never a copied/coerced result.
    #[expect(
        clippy::type_complexity,
        reason = "lookup borrows the exact accepted native row without erasing domain ownership"
    )]
    pub fn lookup<Error>(
        &self,
        max_members: usize,
        name: &str,
        admit: impl FnOnce(
            &SequenceAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>,
        ) -> Result<bool, Error>,
    ) -> Result<
        Option<&SequenceAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>>,
        NativeGroupLookupError<Error>,
    > {
        lookup_native_group_member(
            self.kind,
            &self.members,
            max_members,
            name,
            |member| Ok(member.as_parts().1.name.as_str()),
            admit,
        )
    }

    pub fn into_members(
        self,
    ) -> Vec<SequenceAccepted<'expression, Node, IrResult, Identity, Declaration, Reply>> {
        self.members
    }
}
impl<Node, IrResult, Identity, Declaration: ?Sized, Reply> fmt::Debug
    for GroupReply<'_, Node, IrResult, Identity, Declaration, Reply>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GroupReply")
            .field("kind", &self.kind)
            .field("members", &self.members.len())
            .finish()
    }
}

/// Consume complete accepted results against their exact original group. Whole
/// bounded physical/language shape, every atomic candidate, inclusive arity and
/// all index/row borrows precede fuel and every native admission hook. No sorting,
/// fallback row selection, partial output, identity equality or native copying.
/// After the cold gates one root plus one unit per member is charged atomically;
/// the mandatory FnOnce sees the complete ordered object and the same meter.
/// It must validate expected owner/generation, exact row/declaration correspondence,
/// fresh type/value/schema/live policy and aggregate native bounds before return.
/// No earlier acceptance or pointer comparison substitutes for this native gate.
///
/// Failures/unwind consume and clean up inputs without retry/refund; callbacks and
/// destructors are trusted native work and cannot be preempted/rolled back. A second
/// cleanup panic can abort. Ingress, provenance, external effects and cancellation
/// remain host-owned. This is typed ownership assembly, not group-value projection,
/// continuation selection/restoration, journal persistence or exactly-once dispatch.
#[expect(
    clippy::type_complexity,
    reason = "accepted result retains all original IR and unsized domain ownership"
)]
pub fn assemble_group_reply<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Identity,
    Declaration: ?Sized,
    Reply,
    Error,
>(
    group: &'expression Node<Field, Operation, HostEffect, IrResult>,
    accepted: Vec<
        SequenceAccepted<
            'expression,
            Node<Field, Operation, HostEffect, IrResult>,
            IrResult,
            Identity,
            Declaration,
            Reply,
        >,
    >,
    limits: SequenceEvaluationLimits,
    fuel: &mut Fuel,
    admit: impl FnOnce(
        &GroupReply<
            'expression,
            Node<Field, Operation, HostEffect, IrResult>,
            IrResult,
            Identity,
            Declaration,
            Reply,
        >,
        &mut Fuel,
    ) -> Result<(), Error>,
) -> GroupReplyResult<
    'expression,
    Node<Field, Operation, HostEffect, IrResult>,
    IrResult,
    Identity,
    Declaration,
    Reply,
    Error,
> {
    let kind = check_group_reply(group, &accepted, limits)?;
    fuel.charge(1 + accepted.len() as u64)
        .map_err(|_| GroupReplyError::Fuel)?;
    let reply = GroupReply {
        group,
        kind,
        members: accepted,
    };
    admit(&reply, fuel).map_err(GroupReplyError::Native)?;
    Ok(reply)
}

/// Rebind an already-assembled saved group through the same complete cold checks
/// as live assembly, without paying its assembly charge again. Accepted rows must
/// already refer to this admitted graph, for example through the completed saved
/// Seq/All collectors. No decoded row, accepted wrapper or pointer comparison is
/// evidence that assembly was paid or that a captured value is still authorized.
///
/// Mandatory whole-group admission authenticates the committed original graph,
/// exact domains, identities, values, assembly completion and saved budget, then
/// rechecks live policy and aggregate bounds. The host must distinguish a saved
/// assembled value from merely completed receipts; use assemble_group_reply for
/// the latter. Reads/projections still require fresh selected-row admission.
///
/// This entry moves original owners without Clone/Debug/equality/serde bounds.
/// It does not create/refill a meter, dispatch, evaluate, restore captures, settle
/// work or install a codec. Native restore work/ingress must be bounded by the
/// host, not treated as free execution. Refusal/unwind consumes inputs without
/// retry/refund; callbacks and Drop are trusted, and double panic can abort.
#[expect(
    clippy::type_complexity,
    reason = "saved group admission preserves original IR and native owned inputs"
)]
pub fn restore_group_reply<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Identity,
    Declaration: ?Sized,
    Reply,
    Error,
>(
    group: &'expression Node<Field, Operation, HostEffect, IrResult>,
    accepted: Vec<
        SequenceAccepted<
            'expression,
            Node<Field, Operation, HostEffect, IrResult>,
            IrResult,
            Identity,
            Declaration,
            Reply,
        >,
    >,
    limits: SequenceEvaluationLimits,
    admit: impl FnOnce(
        &GroupReply<
            'expression,
            Node<Field, Operation, HostEffect, IrResult>,
            IrResult,
            Identity,
            Declaration,
            Reply,
        >,
    ) -> Result<(), Error>,
) -> GroupReplyResult<
    'expression,
    Node<Field, Operation, HostEffect, IrResult>,
    IrResult,
    Identity,
    Declaration,
    Reply,
    Error,
> {
    let kind = check_group_reply(group, &accepted, limits)?;
    let reply = GroupReply {
        group,
        kind,
        members: accepted,
    };
    admit(&reply).map_err(GroupReplyError::Native)?;
    Ok(reply)
}

#[expect(
    clippy::type_complexity,
    reason = "live and restored assembly share exact native row and shape checks"
)]
fn check_group_reply<
    'expression,
    Field,
    Operation,
    HostEffect,
    IrResult,
    Identity,
    Declaration: ?Sized,
    Reply,
    Error,
>(
    group: &'expression Node<Field, Operation, HostEffect, IrResult>,
    accepted: &[SequenceAccepted<
        'expression,
        Node<Field, Operation, HostEffect, IrResult>,
        IrResult,
        Identity,
        Declaration,
        Reply,
    >],
    limits: SequenceEvaluationLimits,
) -> Result<GroupKind, GroupReplyError<Error>> {
    if limits.max_nodes > MAX_TYPE_INFERENCE_NODES
        || limits.max_depth > MAX_TYPE_INFERENCE_DEPTH
        || limits.max_branches > MAX_GROUP_EXPORT_MEMBERS
    {
        return Err(GroupReplyError::InvalidLimits);
    }
    let Computation::Group {
        group_kind,
        branches,
    } = group
    else {
        return Err(GroupReplyError::NotGroup);
    };
    physical(
        group,
        GroupExportLimits {
            max_nodes: limits.max_nodes,
            max_depth: limits.max_depth,
            max_groups: 1,
            max_members: limits.max_branches,
        },
    )
    .map_err(GroupReplyError::Shape)?;
    for (index, branch) in branches.iter().enumerate() {
        if !atomic_candidate(&branch.value) {
            return Err(GroupReplyError::Candidate { index });
        }
    }
    if accepted.len() != branches.len() {
        return Err(GroupReplyError::Count);
    }
    for (index, member) in accepted.iter().enumerate() {
        let (position, original, _, _, _) = member.as_parts();
        if position != index || !std::ptr::eq(original, &branches[index]) {
            return Err(GroupReplyError::Row { index });
        }
    }
    Ok(*group_kind)
}
