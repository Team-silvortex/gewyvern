//! Product bound-group terminal correspondence, without storage or evaluation.

use leselang_runtime_core::{Fault, OrderedOutcomeSummary};

use crate::reference_merge::{invalid_group_binding as invalid, validate_group_binding};
use crate::reference_merge_output::merge_output_fault;
use crate::{MergePlan, ResultBinding, Step};

/// Check a stored terminal using original, already-admitted graph observations.
/// The last binding must come from the last original request, and the outcome pass
/// must use the product order/limits and original member results. The owner still
/// proves graph/capture/reservation authority on one snapshot before this stage.
///
/// Pending or unbound groups do not finish the summary. For bound terminals,
/// aggregate/first-stop precedence comes before successor/scalar/fault policy.
/// Saved bodies are classified and type-checked only, never evaluated. Accepted
/// scalar values are not recomputed or authenticated, and success cannot publish
/// a receipt, authorize a dispatch or replace the owner's durable recovery gates.
pub fn validate_bound_group_receipt(
    plan: &MergePlan,
    last_binding: Option<&ResultBinding>,
    last_step: Option<&Step>,
    outcomes: OrderedOutcomeSummary<Step>,
    terminal: Option<&Step>,
) -> Result<(), Fault> {
    let Some(binding) = &plan.result_binding else {
        return Ok(());
    };
    if let Some(terminal) = terminal {
        let summary = outcomes
            .finish(|items, limit| Step::Fault(merge_output_fault(items, limit)))
            .map_err(|_| invalid())?;
        if let Some(failure) = summary.first_stop {
            if *terminal != failure {
                return Err(invalid());
            }
        } else {
            if binding.has_successor(plan) {
                if binding.captures_successor() {
                    return match terminal {
                        Step::Done(crate::Value::Scalar { value })
                            if last_binding.is_some_and(|saved| {
                                saved.body.is_pure()
                                    || (binding.is_conditional()
                                        && saved.body.can_return_without_suspending())
                            }) && validate_group_binding(binding, plan)?
                                == leserpent_hir::Type::Scalar(value.scalar_type()) =>
                        {
                            Ok(())
                        }
                        Step::Fault(fault)
                            if matches!(
                                fault.code.as_str(),
                                "LSV1001"
                                    | "LSV1401"
                                    | "LSV1403"
                                    | "LSV1405"
                                    | "LSV1406"
                                    | "LSV1408"
                                    | "LSV3002"
                            ) || ((binding.is_dataflow() || binding.is_conditional())
                                && matches!(
                                    fault.code.as_str(),
                                    "LSV1002" | "LSV1402" | "LSV1404" | "LSV3001"
                                )) =>
                        {
                            Ok(())
                        }
                        _ => Err(invalid()),
                    };
                }
                if last_step != Some(terminal) {
                    return Err(invalid());
                }
                return Ok(());
            }
            // Replay the committed scalar/fault; never re-evaluate source during recovery.
            match terminal {
                Step::Done(crate::Value::Scalar { value })
                    if (binding.successor_sequence.is_none()
                        || (binding.is_conditional()
                            && binding.binding.body.can_return_without_suspending()))
                        && validate_group_binding(binding, plan)?
                            == leserpent_hir::Type::Scalar(value.scalar_type()) => {}
                Step::Fault(fault)
                    if matches!(
                        fault.code.as_str(),
                        "LSV1001" | "LSV1401" | "LSV1403" | "LSV1406" | "LSV1408" | "LSV3002"
                    ) => {}
                Step::Fault(fault)
                    if binding.successor_sequence.is_some()
                        && matches!(
                            fault.code.as_str(),
                            "LSV1002" | "LSV1402" | "LSV1404" | "LSV1405" | "LSV3001" | "LSV3002"
                        ) => {}
                _ => return Err(invalid()),
            }
        }
    }
    Ok(())
}
