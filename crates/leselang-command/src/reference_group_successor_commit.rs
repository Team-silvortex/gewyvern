//! Reference group-successor registration inside the original settlement owner.

pub use crate::reference_group_execution::CaptureProgress;
use crate::reference_merge::{invalid_group_binding as invalid, validate_merge_plan};
use crate::retention_policy::MAX_JOURNAL_RECORDS;
use crate::{
    ContinuationImage, ContinuationToken, EffectRequest, MAX_MERGE_BRANCHES, MergePlan, Step, Value,
};
use leselang_runtime_core::Fault;

/// Trusted original owner: ordered request loading keeps native streaming decode
/// and errors, and preparation reuses the existing binding evaluator/reservations.
/// Only preparation errors become terminal receipts; storage/proof errors escape.
pub trait GroupSuccessorOwner {
    fn requests(&mut self, owner: &ContinuationToken) -> Result<Vec<EffectRequest>, Fault>;
    fn prepare(
        &mut self,
        owner: &ContinuationToken,
        plan: &MergePlan,
        requests: &[EffectRequest],
        value: &Value,
    ) -> Result<CaptureProgress, Fault>;
    fn validate_graph(
        &mut self,
        owner: &ContinuationToken,
        plan: &MergePlan,
        graph: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault>;
    fn encode_request(&mut self, request: &EffectRequest) -> Result<Vec<u8>, Fault>;
}

pub struct MemorySuccessorGroup<'a> {
    pub plan: &'a mut MergePlan,
    pub branches: &'a mut Vec<ContinuationToken>,
}
pub trait MemoryGroupSuccessor: GroupSuccessorOwner {
    fn contains_dispatch(&self, token: &ContinuationToken) -> bool;
    fn contains_group(&self, token: &ContinuationToken) -> bool;
    fn insert_dispatch(&mut self, token: ContinuationToken, request: EffectRequest);
    fn group_mut(&mut self, owner: &ContinuationToken) -> Option<MemorySuccessorGroup<'_>>;
}

/// Prefix bypass precedes all reads. Graph proof precedes conflicts/encoding,
/// then insertion precedes the final owner lookup. Late failure retains the
/// original memory prefix; this is neither rollback nor idempotent replay.
pub fn admit_memory<Host: MemoryGroupSuccessor + ?Sized>(
    host: &mut Host,
    owner: &ContinuationToken,
    plan: &MergePlan,
    prefix: &Step,
) -> Result<Option<Step>, Fault> {
    prepare(host, owner, plan, prefix, |host, request, next| {
        let token = request.continuation.token.clone();
        if host.contains_dispatch(&token) || host.contains_group(&token) {
            return Err(invalid());
        }
        host.encode_request(&request)?;
        host.insert_dispatch(token.clone(), request);
        let group = host.group_mut(owner).ok_or_else(invalid)?;
        *group.plan = next;
        group.branches.push(token);
        Ok(None)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SuccessorPlanEncoding {
    Next,
    Prefix,
}
/// Borrow the caller's settlement transaction. Counts, codecs, quota and SQL
/// retain native diagnostics. No commit, retry, allocation or dispatch is exposed.
pub trait DurableGroupSuccessor: GroupSuccessorOwner {
    fn record_count(&mut self) -> Result<i64, Fault>;
    fn encode_image(&mut self, image: &ContinuationImage) -> Result<Vec<u8>, Fault>;
    fn encode_plan(
        &mut self,
        plan: &MergePlan,
        kind: SuccessorPlanEncoding,
    ) -> Result<Vec<u8>, Fault>;
    fn ensure_growth(&mut self, additional: usize) -> Result<(), Fault>;
    fn insert_effect(
        &mut self,
        token: &str,
        image: &[u8],
        deadline: Option<i64>,
    ) -> Result<(), Fault>;
    fn insert_dispatch(&mut self, token: &str, bytes: &[u8]) -> Result<(), Fault>;
    fn update_plan(&mut self, owner: &str, bytes: &[u8]) -> Result<usize, Fault>;
    fn link_branch(
        &mut self,
        owner: &str,
        token: &str,
        name: &str,
        position: usize,
    ) -> Result<(), Fault>;
}

/// Keep graph proof, record bound, four bounded encodings, signed deadline and
/// incremental quota in order. Effect/dispatch/plan/link writes share the outer
/// transaction; exactly one pending plan must update before linking the branch.
pub fn admit_durable<Host: DurableGroupSuccessor + ?Sized>(
    host: &mut Host,
    owner: &ContinuationToken,
    plan: &MergePlan,
    prefix: &Step,
) -> Result<Option<Step>, Fault> {
    prepare(host, owner, plan, prefix, |host, request, next| {
        if host.record_count()? >= MAX_JOURNAL_RECORDS as i64 {
            return Err(Fault {
                code: "LSV4008".into(),
                message: "journal record limit exceeded".into(),
            });
        }
        let image = host.encode_image(&request.continuation)?;
        let bytes = host.encode_request(&request)?;
        let plan_bytes = host.encode_plan(&next, SuccessorPlanEncoding::Next)?;
        let previous = host.encode_plan(plan, SuccessorPlanEncoding::Prefix)?;
        let deadline = request
            .continuation
            .deadline_at_ms
            .map(i64::try_from)
            .transpose()
            .map_err(|_| invalid())?;
        host.ensure_growth(
            image
                .len()
                .saturating_add(bytes.len())
                .saturating_add(plan_bytes.len().saturating_sub(previous.len())),
        )?;
        host.insert_effect(request.continuation.token.as_str(), &image, deadline)?;
        host.insert_dispatch(request.continuation.token.as_str(), &bytes)?;
        if host.update_plan(owner.as_str(), &plan_bytes)? != 1 {
            return Err(Fault {
                code: "LSV4033".into(),
                message: "group successor lost its pending owner".into(),
            });
        }
        host.link_branch(
            owner.as_str(),
            request.continuation.token.as_str(),
            next.branches.last().ok_or_else(invalid)?,
            plan.branches.len(),
        )?;
        Ok(None)
    })
}

fn prepare<Host: GroupSuccessorOwner + ?Sized>(
    host: &mut Host,
    owner: &ContinuationToken,
    plan: &MergePlan,
    prefix: &Step,
    register: impl FnOnce(&mut Host, EffectRequest, MergePlan) -> Result<Option<Step>, Fault>,
) -> Result<Option<Step>, Fault> {
    let Step::Done(value) = prefix else {
        return Ok(Some(prefix.clone()));
    };
    let mut requests = host.requests(owner)?;
    let request = match host.prepare(owner, plan, &requests, value) {
        Ok(CaptureProgress::Pending(request)) => *request,
        Ok(CaptureProgress::Done(value)) => return Ok(Some(Step::Done(Value::Scalar { value }))),
        Err(error) => return Ok(Some(Step::Fault(error))),
    };
    let mut next = plan.clone();
    let binding = next.result_binding.as_ref().ok_or_else(invalid)?;
    let position = plan.branches.len() - binding.plan_branches().map_err(|_| invalid())?.len();
    next.branches.push(
        binding
            .plan_successor_name_at(position, MAX_MERGE_BRANCHES)
            .map_err(|_| invalid())?,
    );
    validate_merge_plan(&next)?;
    requests.push(request.clone());
    let graph = next
        .branches
        .iter()
        .zip(&requests)
        .map(|(name, r)| (name.as_str(), r))
        .collect::<Vec<_>>();
    host.validate_graph(owner, &next, &graph)?;
    register(host, request, next)
}
