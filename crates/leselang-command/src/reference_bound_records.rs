//! Product bound-record stream and capture-prefix coordination, without storage or execution.

use leselang_runtime_core::{Fault, OrderedOutcomeSummary, OutcomeOrder};
use leserpent_hir::host_call::HostOperation;

use crate::reference_bound_receipt::validate_bound_group_receipt;
use crate::reference_merge::invalid_group_binding as invalid;
use crate::reference_recovery::{MAX_JOURNAL_ENTRY_BYTES, decode_journal_value};
use crate::reference_request::DEFAULT_MAX_OUTPUT_ITEMS;
use crate::reference_value::validate_value;
use crate::{
    ContinuationToken, EffectRequest, GroupResultBinding, MAX_MERGE_BRANCHES, MergePlan, Step,
    StructuredField, Value,
};

/// Original name, optional request bytes and optional terminal bytes. Not admitted data.
pub type StoredBoundRecord<Bytes = Vec<u8>> = (String, Option<Bytes>, Option<Bytes>);

/// Original product proofs, retaining native diagnostics and current snapshot authority.
/// Hooks must bound their work, preserve the supplied ordered data and never evaluate
/// saved code, dispatch, repair or commit. Unwind propagates; native side effects are
/// not rolled back. Borrowed local owners need no Clone, Send or Sync implementation.
pub trait ReferenceBoundRecordProofs {
    fn validate_capture(
        &mut self,
        binding: &GroupResultBinding,
        request: &EffectRequest,
        fields: &[StructuredField],
        operations: &[HostOperation],
    ) -> Result<(), Fault>;
    fn validate_pair(
        &mut self,
        previous: &EffectRequest,
        completed: &Step,
        request: &EffectRequest,
    ) -> Result<(), Fault>;
    fn validate_member(
        &mut self,
        binding: &GroupResultBinding,
        name: &str,
        value: &Value,
    ) -> Result<(), Fault>;
    fn validate_graph(
        &mut self,
        token: &ContinuationToken,
        plan: &MergePlan,
        requests: &[(&str, &EffectRequest)],
    ) -> Result<(), Fault>;
    fn validate_reservations(&mut self, plan: &MergePlan) -> Result<(), Fault>;
}

/// Preserve lazy row -> request -> capture/pair -> terminal/member -> prefix ordering.
/// A missing terminal clears the predecessor; only successful values extend captures.
/// After all rows: graph, successful-prefix completeness, original reservations, receipt.
///
/// Before entry the owner must bound/admit the plan and graph (at most 64 original
/// rows), authenticate their common snapshot and preserve field-read order in each
/// row. Each encoded payload is capped here; iterator work/aggregate ingress remain
/// owner obligations. Unbound plans do not even enter the iterator. No partial state
/// escapes on refusal or unwind, and Ok is not a recovery/publication certificate.
pub fn inspect_bound_group_records<
    Bytes: AsRef<[u8]>,
    Rows: IntoIterator<Item = Result<StoredBoundRecord<Bytes>, Fault>>,
    Proofs: ReferenceBoundRecordProofs + ?Sized,
>(
    token: &ContinuationToken,
    plan: &MergePlan,
    rows: Rows,
    terminal: Option<&Step>,
    proofs: &mut Proofs,
) -> Result<(), Fault> {
    let Some(binding) = &plan.result_binding else {
        return Ok(());
    };
    let mut requests: Vec<(String, EffectRequest)> = Vec::with_capacity(plan.branches.len());
    let mut outcomes = OrderedOutcomeSummary::new(
        if plan.order.is_sequential() {
            OutcomeOrder::Sequential
        } else {
            OutcomeOrder::Parallel
        },
        MAX_MERGE_BRANCHES,
        DEFAULT_MAX_OUTPUT_ITEMS,
    )
    .map_err(|_| invalid())?;
    let mut last_step = None;
    let mut prefix_fields = Vec::new();
    let mut prefix_operations = Vec::new();
    let mut previous_step = None;
    let mut successful_prefix = 0usize;
    for row in rows {
        let (name, request, step) = row?;
        let request: EffectRequest = decode_journal_value(
            request.ok_or_else(invalid)?.as_ref(),
            MAX_JOURNAL_ENTRY_BYTES,
        )?;
        if binding.captures_successor()
            && requests.len() >= binding.plan_branches().map_err(|_| invalid())?.len()
        {
            proofs.validate_capture(binding, &request, &prefix_fields, &prefix_operations)?;
            if requests.len() > binding.plan_branches().map_err(|_| invalid())?.len() {
                proofs.validate_pair(
                    &requests.last().ok_or_else(invalid)?.1,
                    previous_step.as_ref().ok_or_else(invalid)?,
                    &request,
                )?;
            }
        }
        previous_step = None;
        if let Some(bytes) = step {
            let step: Step = decode_journal_value(bytes.as_ref(), MAX_JOURNAL_ENTRY_BYTES)?;
            previous_step = Some(step.clone());
            if requests.len() + 1 == plan.branches.len() {
                last_step = Some(step.clone());
            }
            if let Step::Done(value) = step {
                proofs.validate_member(binding, &name, &value)?;
                if requests.len() < binding.plan_branches().map_err(|_| invalid())?.len() {
                    successful_prefix += 1;
                }
                outcomes
                    .observe_success(validate_value(&value, 0)?, |_, _| {
                        Step::Fault(Fault {
                            code: "LSV2404".into(),
                            message: "sequential output exceeds runtime bounds".into(),
                        })
                    })
                    .map_err(|_| invalid())?;
                if binding.captures_successor() && binding.has_successor(plan) {
                    prefix_operations.push(
                        leserpent_hir::host_call::HostOperation::for_effect(
                            &request.continuation.pending_effect,
                        )
                        .ok_or_else(invalid)?,
                    );
                    prefix_fields.push(StructuredField {
                        name: name.clone(),
                        value: Box::new(value),
                    });
                }
            } else {
                outcomes.observe_stop(step).map_err(|_| invalid())?;
            }
        }
        requests.push((name, request));
    }
    let graph = requests
        .iter()
        .map(|(name, request)| (name.as_str(), request))
        .collect::<Vec<_>>();
    proofs.validate_graph(token, plan, &graph)?;
    if binding.has_successor(plan)
        && successful_prefix != binding.plan_branches().map_err(|_| invalid())?.len()
    {
        return Err(invalid());
    }
    proofs.validate_reservations(plan)?;
    validate_bound_group_receipt(
        plan,
        requests
            .last()
            .and_then(|(_, request)| request.continuation.result_binding.as_deref()),
        last_step.as_ref(),
        outcomes,
        terminal,
    )
}
