//! Complete product plan and input-graph consistency, not journal recovery authority.

use leselang_hir::Type;
use leselang_runtime_core::{GroupGraphError, GroupGraphMember, validate_group_graph};

use crate::{
    ContinuationAdmissionLimits, EffectRequestView, GroupBindingError, GroupBindingLimits,
    GroupRequestContext, MergePlan, MergePlanShapeError, MergePlanShapeLimits,
    RequestAdmissionError, validate_effect_request, validate_group_binding,
    validate_preflighted_group_requests,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MergePlanAdmissionLimits {
    pub shape: MergePlanShapeLimits,
    pub group: GroupBindingLimits,
}

#[derive(Eq, PartialEq)]
pub enum MergePlanAdmissionError<Native> {
    Shape(MergePlanShapeError),
    Binding(GroupBindingError<Native>),
}

impl<Native> std::fmt::Display for MergePlanAdmissionError<Native> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Shape(_) => "invalid merge plan shape",
            Self::Binding(_) => "invalid merge plan binding",
        })
    }
}
impl<Native> std::fmt::Debug for MergePlanAdmissionError<Native> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for MergePlanAdmissionError<Native> {}

/// Validate outer shape before selecting and admitting the original saved binding.
/// The encoder runs only for a binding, once after its early structure/plan gates.
/// Its error retains ownership; callbacks are not retried or rolled back on unwind.
/// `None` means an unbound plan, not an untyped binding or an execution grant.
/// Callers bound ingress, native work and pending payloads, and retain request,
/// receipt, authority, live-budget and durable-state admission.
pub fn validate_merge_plan<Native>(
    plan: &MergePlan,
    limits: MergePlanAdmissionLimits,
    validate_group_name: impl FnMut(&str) -> bool,
    validate_encoding: impl FnOnce(&MergePlan) -> Result<(), Native>,
) -> Result<Option<Type>, MergePlanAdmissionError<Native>> {
    plan.validate_shape(limits.shape)
        .map_err(MergePlanAdmissionError::Shape)?;
    plan.result_binding
        .as_deref()
        .map(|binding| {
            validate_group_binding(
                binding,
                plan,
                limits.group,
                validate_group_name,
                validate_encoding,
            )
            .map_err(MergePlanAdmissionError::Binding)
        })
        .transpose()
}

/// Explicit ceilings, not authenticated original resource grants. Graph member
/// bounds use `plan.shape.max_branches`; successor naming uses the group plan ceiling.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MergeGraphAdmissionLimits {
    pub plan: MergePlanAdmissionLimits,
    pub frame: ContinuationAdmissionLimits,
}

/// Native spelling, identity, name and bounded encoding policy for a stable snapshot.
/// `valid_token` applies to both the group owner and individual frame tokens; the
/// canonical sequence hook must reject zero/noncanonical/out-of-policy request IDs.
/// Token text must faithfully identify the original owner, never normalize it.
pub trait MergeGraphAdmissionHost<Token> {
    type Error;
    fn token_text<'a>(&self, token: &'a Token) -> &'a str;
    fn valid_token(&self, token: &Token) -> bool;
    fn canonical_sequence(&self, token: &Token) -> Option<u64>;
    fn valid_group_name(&self, name: &str) -> bool;
    fn validate_plan_encoding(&self, plan: &MergePlan) -> Result<(), Self::Error>;
}

/// Ordered refusal provenance; native errors move unchanged and diagnostics never
/// format their payload. Request errors retain the original failing row index.
pub enum MergeGraphAdmissionError<Native> {
    Plan(MergePlanAdmissionError<Native>),
    Owner,
    Graph(GroupGraphError<RequestAdmissionError>),
    Binding(GroupBindingError<Native>),
    Correspondence,
    UnexpectedGroupOwner,
}

impl<Native> std::fmt::Display for MergeGraphAdmissionError<Native> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Plan(_) => "invalid merge graph plan",
            Self::Owner => "invalid merge graph owner",
            Self::Graph(_) => "invalid merge graph members",
            Self::Binding(_) => "invalid merge graph binding",
            Self::Correspondence => "invalid merge graph correspondence",
            Self::UnexpectedGroupOwner => "unbound merge graph has a bound member",
        })
    }
}
impl<Native> std::fmt::Debug for MergeGraphAdmissionError<Native> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}
impl<Native> std::error::Error for MergeGraphAdmissionError<Native> {}

/// Admit plan, owner, complete ordered request graph, then binding/correspondence.
/// Each request is admitted before checking its row name, owner exclusion and
/// identity uniqueness. Count refusal precedes row observation. Bound plans repeat
/// binding admission (including native encoding) after the graph, preserving the
/// reference gate order; no cached result is treated as an admission certificate.
///
/// Views, token text, equality and callbacks are trusted and must observe the same
/// immutable snapshot. Later work is skipped on refusal; unwind propagates without
/// retry or rollback. Rows/errors need no Clone, Debug, serde or thread traits;
/// tokens additionally need PartialEq for group-owner correspondence, not Hash.
/// Original rows are borrowed without building another request collection.
///
/// This never executes saved code, debits fuel, allocates an identity or publishes
/// journal state. Callers bound ingress, policy and native work, authenticate original
/// authority/budgets and receipts, check current deadlines/leases and atomically
/// settle storage. Consistency cannot establish any of those grants. Decode or
/// mutation requires rechecking. Unbound plans deliberately skip group correspondence.
pub fn validate_merge_graph<
    Token: PartialEq,
    Request,
    Host: MergeGraphAdmissionHost<Token> + ?Sized,
>(
    owner: &Token,
    plan: &MergePlan,
    requests: &[(&str, Request)],
    limits: MergeGraphAdmissionLimits,
    view: impl for<'a> Fn(&'a Request) -> EffectRequestView<'a, Token>,
    host: &Host,
) -> Result<(), MergeGraphAdmissionError<Host::Error>> {
    use MergeGraphAdmissionError::{
        Binding, Correspondence, Graph, Owner, Plan, UnexpectedGroupOwner,
    };
    validate_merge_plan(
        plan,
        limits.plan,
        |name| host.valid_group_name(name),
        |plan| host.validate_plan_encoding(plan),
    )
    .map_err(Plan)?;
    if !host.valid_token(owner) {
        return Err(Owner);
    }
    validate_group_graph(
        host.token_text(owner),
        &plan.branches,
        requests,
        limits.plan.shape.max_branches,
        |position, (name, row)| {
            let request = view(row);
            validate_effect_request(
                EffectRequestView { ..request },
                limits.frame,
                |token| host.token_text(token),
                |token| host.valid_token(token),
                |name| host.valid_group_name(name),
                |token| host.canonical_sequence(token),
            )?;
            Ok(GroupGraphMember {
                name,
                position: Some(position),
                identity: host.token_text(&request.continuation.token),
            })
        },
    )
    .map_err(Graph)?;
    if let Some(binding) = &plan.result_binding {
        let result_type = validate_group_binding(
            binding,
            plan,
            limits.plan.group,
            |name| host.valid_group_name(name),
            |plan| host.validate_plan_encoding(plan),
        )
        .map_err(Binding)?;
        validate_preflighted_group_requests(
            binding,
            GroupRequestContext {
                owner,
                plan,
                result_type,
                max_successor_names: limits.plan.group.plan.max_branches,
                binding_limits: limits.plan.group.binding,
            },
            requests,
            view,
            |token| host.token_text(token),
            |token| host.canonical_sequence(token),
            |name| host.valid_group_name(name),
        )
        .map_err(|_| Correspondence)?;
    } else if requests
        .iter()
        .any(|(_, row)| view(row).continuation.group_result.is_some())
    {
        return Err(UnexpectedGroupOwner);
    }
    Ok(())
}
