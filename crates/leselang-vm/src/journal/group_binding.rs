use super::*;
use crate::group_binding::invalid;

pub(super) fn validate_owner(
    connection: &Connection,
    image: &ContinuationImage,
) -> Result<(), Fault> {
    let Some(owner) = &image.group_result else {
        return Ok(());
    };
    let stored: Option<(String, Vec<u8>)> = connection
        .query_row(
            "SELECT g.token, g.plan FROM vm_merge_branches b
         JOIN vm_merge_groups g ON g.token = b.group_token WHERE b.branch_token = ?1",
            [image.token.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| journal_error("LSV4007", "failed to validate group owner", error))?;
    let (token, bytes) = stored.ok_or_else(invalid)?;
    let plan: MergePlan = decode_bounded(&bytes, MAX_CONTINUATION_BYTES)?;
    if token != owner.as_str() || plan.result_binding.is_none() {
        return Err(invalid());
    }
    Ok(())
}

pub(super) fn validate_records(
    connection: &Connection,
    token: &ContinuationToken,
    plan: &MergePlan,
    terminal: Option<&Step>,
) -> Result<(), Fault> {
    let Some(binding) = &plan.result_binding else {
        return Ok(());
    };
    let mut statement = connection
        .prepare(
            "SELECT b.branch_name, d.request, e.terminal_step
         FROM vm_merge_branches b
         LEFT JOIN vm_dispatches d ON d.token = b.branch_token
         JOIN vm_effects e ON e.token = b.branch_token
         WHERE b.group_token = ?1 ORDER BY b.position ASC",
        )
        .map_err(|error| journal_error("LSV4007", "failed to load bound group requests", error))?;
    let rows = statement
        .query_map([token.as_str()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, Option<Vec<u8>>>(1)?,
                row.get::<_, Option<Vec<u8>>>(2)?,
            ))
        })
        .map_err(|error| journal_error("LSV4007", "failed to read bound group requests", error))?;
    let mut requests = Vec::with_capacity(plan.branches.len());
    let mut first_failure = None;
    let mut output_items = 0usize;
    for row in rows {
        let (name, request, step) =
            row.map_err(|error| journal_error("LSV4007", "invalid bound group record", error))?;
        let request: EffectRequest =
            decode_bounded(&request.ok_or_else(invalid)?, MAX_JOURNAL_ENTRY_BYTES)?;
        if let Some(bytes) = step {
            let step: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
            if let Step::Done(value) = &step {
                binding.validate_member(&name, value)?;
                output_items = output_items.saturating_add(validate_value(value, 0)?);
                if plan.order.is_sequential()
                    && output_items > DEFAULT_MAX_OUTPUT_ITEMS
                    && first_failure.is_none()
                {
                    first_failure = Some(Step::Fault(journal_fault(
                        "LSV2404",
                        "sequential output exceeds runtime bounds",
                    )));
                }
            } else if first_failure.is_none() {
                first_failure = Some(step);
            }
        }
        requests.push((name, request));
    }
    let graph = requests
        .iter()
        .map(|(name, request)| (name.as_str(), request))
        .collect::<Vec<_>>();
    validate_merge_graph_input(token, plan, &graph)?;
    if let Some(terminal) = terminal {
        if plan.order.is_parallel() && output_items > DEFAULT_MAX_OUTPUT_ITEMS {
            return Err(invalid());
        }
        if let Some(failure) = first_failure {
            if *terminal != failure {
                return Err(invalid());
            }
        } else {
            // Replay the committed scalar/fault; never re-evaluate source during recovery.
            match terminal {
                Step::Done(crate::Value::Scalar { value })
                    if binding.validate(plan)?
                        == leselang_hir::Type::Scalar(value.scalar_type()) => {}
                Step::Fault(fault)
                    if matches!(
                        fault.code.as_str(),
                        "LSV1001" | "LSV1401" | "LSV1403" | "LSV1406" | "LSV1408"
                    ) => {}
                _ => return Err(invalid()),
            }
        }
    }
    Ok(())
}
