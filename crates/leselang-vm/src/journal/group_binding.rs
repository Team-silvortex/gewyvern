use super::*;
use crate::group_binding::invalid;

pub(super) fn needs_successor(plan: &MergePlan) -> bool {
    plan.order.has_group_successor()
        && plan
            .result_binding
            .as_ref()
            .is_some_and(|binding| !binding.has_successor(plan))
}

pub(super) fn merge_prefix(
    plan: &MergePlan,
    completions: Vec<BranchCompletion>,
) -> Result<Step, Fault> {
    terminal_merge(plan, crate::merge_group_prefix(plan, completions))
}

pub(super) fn finish(plan: &MergePlan, completions: Vec<BranchCompletion>) -> Result<Step, Fault> {
    terminal_merge(
        plan,
        crate::merge_declared(plan, completions, DEFAULT_MAX_OUTPUT_ITEMS),
    )
}

fn terminal_merge(plan: &MergePlan, result: Result<Step, Fault>) -> Result<Step, Fault> {
    // Returned-data limits must commit with the receipt, unlike failed journal writes.
    match result {
        Err(error)
            if error.code == "LSV3002" || (plan.order.is_parallel() && error.code == "LSV2404") =>
        {
            Ok(Step::Fault(error))
        }
        result => result,
    }
}

fn successor_plan(plan: &MergePlan) -> Result<MergePlan, Fault> {
    let mut next = plan.clone();
    let binding = next.result_binding.as_ref().ok_or_else(invalid)?;
    next.branches
        .push(binding.successor_name_at(plan.branches.len() - binding.branches()?.len())?);
    validate_merge_plan(&next)?;
    Ok(next)
}

fn prepare_admission(
    owner: &ContinuationToken,
    plan: &MergePlan,
    requests: &[EffectRequest],
    value: &crate::Value,
) -> Result<crate::group_binding::CaptureProgress, Fault> {
    let binding = plan.result_binding.as_ref().ok_or_else(invalid)?;
    if binding.has_successor(plan) {
        binding.advance_capture(owner, requests, value)
    } else {
        binding.prepare_initial(owner, requests.first().ok_or_else(invalid)?, value)
    }
}

pub(super) fn admit_ephemeral(
    journal: &mut EphemeralJournal,
    owner: &ContinuationToken,
    plan: &MergePlan,
    prefix: &Step,
) -> Result<Option<Step>, Fault> {
    let Step::Done(value) = prefix else {
        return Ok(Some(prefix.clone()));
    };
    let group = journal.merge_groups.get(owner).ok_or_else(invalid)?;
    let mut requests = group
        .branch_tokens
        .iter()
        .map(|token| {
            journal
                .dispatches
                .get(token)
                .map(|dispatch| dispatch.request.clone())
                .ok_or_else(invalid)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let request = match prepare_admission(owner, plan, &requests, value) {
        Ok(crate::group_binding::CaptureProgress::Pending(request)) => *request,
        Ok(crate::group_binding::CaptureProgress::Done(value)) => {
            return Ok(Some(Step::Done(crate::Value::Scalar { value })));
        }
        Err(error) => return Ok(Some(Step::Fault(error))),
    };
    let next = successor_plan(plan)?;
    requests.push(request.clone());
    let graph = next
        .branches
        .iter()
        .zip(&requests)
        .map(|(name, request)| (name.as_str(), request))
        .collect::<Vec<_>>();
    validate_merge_graph_input(owner, &next, &graph)?;
    let token = request.continuation.token.clone();
    if journal.dispatches.contains_key(&token) || journal.merge_groups.contains_key(&token) {
        return Err(invalid());
    }
    encode_json_capped(&request, MAX_JOURNAL_ENTRY_BYTES, "group successor request")?;
    journal.dispatches.insert(
        token.clone(),
        EphemeralDispatch {
            request,
            attempt: 0,
            lease_expires_at_ms: None,
            ready_at_ms: 0,
            retry_count: 0,
            last_error: None,
            acknowledged: false,
            terminal_step: None,
        },
    );
    let group = journal.merge_groups.get_mut(owner).ok_or_else(invalid)?;
    group.plan = next;
    group.branch_tokens.push(token);
    Ok(None)
}

pub(super) fn admit_successor(
    transaction: &rusqlite::Transaction<'_>,
    owner: &ContinuationToken,
    plan: &MergePlan,
    prefix: &Step,
) -> Result<Option<Step>, Fault> {
    let Step::Done(value) = prefix else {
        return Ok(Some(prefix.clone()));
    };
    let mut statement = transaction.prepare(
        "SELECT d.request FROM vm_merge_branches b JOIN vm_dispatches d ON d.token = b.branch_token
         WHERE b.group_token = ?1 ORDER BY b.position"
    ).map_err(|error| journal_error("LSV4033", "failed to prepare group successor", error))?;
    let mut requests = statement
        .query_map([owner.as_str()], |row| row.get::<_, Vec<u8>>(0))
        .map_err(|error| journal_error("LSV4033", "failed to load group authority", error))?
        .map(|row| {
            decode_bounded::<EffectRequest>(
                &row.map_err(|error| journal_error("LSV4033", "invalid group authority", error))?,
                MAX_JOURNAL_ENTRY_BYTES,
            )
        })
        .collect::<Result<Vec<_>, Fault>>()?;
    drop(statement);
    let request = match prepare_admission(owner, plan, &requests, value) {
        Ok(crate::group_binding::CaptureProgress::Pending(request)) => *request,
        Ok(crate::group_binding::CaptureProgress::Done(value)) => {
            return Ok(Some(Step::Done(crate::Value::Scalar { value })));
        }
        Err(error) => return Ok(Some(Step::Fault(error))),
    };
    let next = successor_plan(plan)?;
    requests.push(request.clone());
    let graph = next
        .branches
        .iter()
        .zip(&requests)
        .map(|(name, request)| (name.as_str(), request))
        .collect::<Vec<_>>();
    validate_merge_graph_input(owner, &next, &graph)?;
    let records: i64 = transaction
        .query_row(
            "SELECT (SELECT COUNT(*) FROM vm_effects) + (SELECT COUNT(*) FROM vm_merge_groups)",
            [],
            |row| row.get(0),
        )
        .map_err(|error| journal_error("LSV4033", "failed to bound group successor", error))?;
    if records >= MAX_JOURNAL_RECORDS as i64 {
        return Err(journal_fault("LSV4008", "journal record limit exceeded"));
    }
    let image = encode_json_capped(
        &request.continuation,
        MAX_CONTINUATION_BYTES,
        "group successor image",
    )?;
    let request_bytes =
        encode_json_capped(&request, MAX_JOURNAL_ENTRY_BYTES, "group successor request")?;
    let plan_bytes = encode_json_capped(&next, MAX_CONTINUATION_BYTES, "group successor plan")?;
    let previous_plan_bytes =
        encode_json_capped(plan, MAX_CONTINUATION_BYTES, "group prefix plan")?;
    let deadline = request
        .continuation
        .deadline_at_ms
        .map(i64::try_from)
        .transpose()
        .map_err(|_| invalid())?;
    ensure_growth(
        transaction,
        image
            .len()
            .saturating_add(request_bytes.len())
            .saturating_add(plan_bytes.len().saturating_sub(previous_plan_bytes.len())),
    )?;
    transaction.execute(
        "INSERT INTO vm_effects(token, state, image, deadline_at_ms, terminal_step) VALUES (?1, 'pending', ?2, ?3, NULL)",
        params![request.continuation.token.as_str(), image, deadline]
    ).map_err(|error| journal_error("LSV4033", "failed to create group successor", error))?;
    transaction.execute(
        "INSERT INTO vm_dispatches(token, request, state, attempt, lease_expires_at_ms) VALUES (?1, ?2, 'ready', 0, NULL)",
        params![request.continuation.token.as_str(), request_bytes]
    ).map_err(|error| journal_error("LSV4033", "failed to create group successor dispatch", error))?;
    let updated = transaction
        .execute(
            "UPDATE vm_merge_groups SET plan = ?2 WHERE token = ?1 AND state = 'pending'",
            params![owner.as_str(), plan_bytes],
        )
        .map_err(|error| {
            journal_error("LSV4033", "failed to append group successor plan", error)
        })?;
    if updated != 1 {
        return Err(journal_fault(
            "LSV4033",
            "group successor lost its pending owner",
        ));
    }
    transaction.execute(
        "INSERT INTO vm_merge_branches(group_token, branch_token, branch_name, position) VALUES (?1, ?2, ?3, ?4)",
        params![owner.as_str(), request.continuation.token.as_str(), next.branches.last().ok_or_else(invalid)?, plan.branches.len() as i64]
    ).map_err(|error| journal_error("LSV4033", "failed to link group successor", error))?;
    Ok(None)
}

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
    let mut requests: Vec<(String, EffectRequest)> = Vec::with_capacity(plan.branches.len());
    let mut first_failure = None;
    let mut last_step = None;
    let mut output_items = 0usize;
    let mut prefix_fields = Vec::new();
    let mut prefix_operations = Vec::new();
    let mut previous_step = None;
    let mut successful_prefix = 0usize;
    for row in rows {
        let (name, request, step) =
            row.map_err(|error| journal_error("LSV4007", "invalid bound group record", error))?;
        let request: EffectRequest =
            decode_bounded(&request.ok_or_else(invalid)?, MAX_JOURNAL_ENTRY_BYTES)?;
        if binding.captures_successor() && requests.len() >= binding.branches()?.len() {
            binding.validate_saved_capture(&request, &prefix_fields, &prefix_operations)?;
            if requests.len() > binding.branches()?.len() {
                crate::successor::validate_pair(
                    &requests.last().ok_or_else(invalid)?.1,
                    previous_step.as_ref().ok_or_else(invalid)?,
                    &request,
                )?;
            }
        }
        previous_step = None;
        if let Some(bytes) = step {
            let step: Step = decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)?;
            previous_step = Some(step.clone());
            if requests.len() + 1 == plan.branches.len() {
                last_step = Some(step.clone());
            }
            if let Step::Done(value) = step {
                binding.validate_member(&name, &value)?;
                if requests.len() < binding.branches()?.len() {
                    successful_prefix += 1;
                }
                output_items = output_items.saturating_add(validate_value(&value, 0)?);
                if plan.order.is_sequential()
                    && output_items > DEFAULT_MAX_OUTPUT_ITEMS
                    && first_failure.is_none()
                {
                    first_failure = Some(Step::Fault(journal_fault(
                        "LSV2404",
                        "sequential output exceeds runtime bounds",
                    )));
                }
                if binding.captures_successor() && binding.has_successor(plan) {
                    prefix_operations.push(
                        leselang_hir::host_call::HostOperation::for_effect(
                            &request.continuation.pending_effect,
                        )
                        .ok_or_else(invalid)?,
                    );
                    prefix_fields.push(crate::StructuredField {
                        name: name.clone(),
                        value: Box::new(value),
                    });
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
    if binding.has_successor(plan) && successful_prefix != binding.branches()?.len() {
        return Err(invalid());
    }
    if binding.successor_sequence.is_some() {
        let next_sequence: i64 = connection
            .query_row(
                "SELECT next_sequence FROM vm_metadata WHERE singleton = 1",
                [],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error("LSV4007", "failed to validate successor reservation", error)
            })?;
        for (index, sequence) in binding.reserved_sequences().enumerate() {
            let exists: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM vm_effects WHERE token = ?1)",
                    [format!("continuation-{sequence}")],
                    |row| row.get(0),
                )
                .map_err(|error| {
                    journal_error("LSV4007", "failed to validate reserved successor", error)
                })?;
            let aliases_group: bool = connection
                .query_row(
                    "SELECT EXISTS(SELECT 1 FROM vm_merge_groups WHERE token = ?1)",
                    [format!("merge-{sequence}")],
                    |row| row.get(0),
                )
                .map_err(|error| {
                    journal_error(
                        "LSV4007",
                        "failed to validate reserved group identity",
                        error,
                    )
                })?;
            if next_sequence <= 0
                || sequence >= next_sequence as u64
                || exists != (index < plan.branches.len() - binding.branches()?.len())
                || aliases_group
            {
                return Err(invalid());
            }
        }
    }
    if let Some(terminal) = terminal {
        if plan.order.is_parallel() && output_items > DEFAULT_MAX_OUTPUT_ITEMS {
            first_failure = Some(Step::Fault(crate::merge_output_fault(
                output_items,
                DEFAULT_MAX_OUTPUT_ITEMS,
            )));
        }
        if let Some(failure) = first_failure {
            if *terminal != failure {
                return Err(invalid());
            }
        } else {
            if binding.has_successor(plan) {
                if binding.captures_successor() {
                    return match terminal {
                        Step::Done(crate::Value::Scalar { value })
                            if requests.last().is_some_and(|(_, request)| {
                                request
                                    .continuation
                                    .result_binding
                                    .as_ref()
                                    .is_some_and(|saved| {
                                        saved.body.is_pure()
                                            || (binding.is_conditional()
                                                && saved.body.can_return_without_suspending())
                                    })
                            }) && binding.validate(plan)?
                                == leselang_hir::Type::Scalar(value.scalar_type()) =>
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
                if last_step.as_ref() != Some(terminal) {
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
                        && binding.validate(plan)?
                            == leselang_hir::Type::Scalar(value.scalar_type()) => {}
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
