use super::*;
use crate::successor::{CompletionLease, invalid, validate_pair};

impl Journal {
    pub fn validate_result_output(
        &self,
        image: &ContinuationImage,
        value: &crate::Value,
    ) -> Result<(), Fault> {
        crate::validate_bound_value(image, value)?;
        // A pending step's committed prefix cannot change. Check raw host output before
        // scalar projection can hide it, and before a successor or early exit is committed.
        let mut items = validate_value(value, 0)?;
        match self {
            Self::Ephemeral(journal) => {
                if let Some(group) = journal
                    .merge_groups
                    .values()
                    .find(|group| group.branch_tokens.contains(&image.token))
                {
                    for token in group
                        .branch_tokens
                        .iter()
                        .take_while(|token| *token != &image.token)
                    {
                        let step = journal
                            .dispatches
                            .get(token)
                            .and_then(|dispatch| dispatch.terminal_step.as_ref())
                            .ok_or_else(invalid)?;
                        accumulate_output(&mut items, step)?;
                    }
                }
            }
            Self::Sqlite(journal) => {
                let mut statement = journal.connection.prepare(
                    "SELECT e.terminal_step FROM vm_merge_branches own
                     JOIN vm_merge_branches prior ON prior.group_token = own.group_token AND prior.position < own.position
                     JOIN vm_effects e ON e.token = prior.branch_token WHERE own.branch_token = ?1 ORDER BY prior.position"
                ).map_err(|error| journal_error("LSV4034", "failed to inspect result output", error))?;
                for row in statement
                    .query_map([image.token.as_str()], |row| row.get::<_, Vec<u8>>(0))
                    .map_err(|error| {
                        journal_error("LSV4034", "failed to load result output", error)
                    })?
                {
                    let step = decode_bounded(
                        &row.map_err(|error| {
                            journal_error("LSV4034", "invalid result output", error)
                        })?,
                        MAX_JOURNAL_ENTRY_BYTES,
                    )?;
                    accumulate_output(&mut items, &step)?;
                }
            }
        }
        if items > DEFAULT_MAX_OUTPUT_ITEMS {
            return Err(journal_fault(
                "LSV2404",
                "sequential output exceeds runtime bounds",
            ));
        }
        Ok(())
    }

    pub fn effect_request(&self, token: &ContinuationToken) -> Result<EffectRequest, Fault> {
        match self {
            Self::Ephemeral(journal) => journal
                .dispatches
                .get(token)
                .map(|dispatch| dispatch.request.clone())
                .ok_or_else(invalid),
            Self::Sqlite(journal) => request_for(&journal.connection, token.as_str()),
        }
    }

    pub fn record_successor(
        &mut self,
        parent: &EffectRequest,
        step: &Step,
        next: &EffectRequest,
        lease: CompletionLease<'_>,
    ) -> Result<Step, Fault> {
        validate_pair(parent, step, next)?;
        if let Some((lease, now_ms)) = lease {
            validate_lease_clock(now_ms, 1)?;
            if lease.request != *parent
                || lease.attempt == 0
                || lease.attempt > MAX_DISPATCH_ATTEMPTS
            {
                return Err(invalid());
            }
        }
        let plan = if matches!(
            parent.continuation.schema_version,
            crate::DATAFLOW_CONTINUATION_SCHEMA_VERSION
                | crate::CONDITIONAL_CONTINUATION_SCHEMA_VERSION
        ) {
            MergePlan {
                order: ExecutionOrder::Dataflow,
                branches: vec!["step_1".into(), "step_2".into()],
                result_binding: None,
            }
        } else {
            MergePlan {
                order: ExecutionOrder::ResultChain,
                branches: vec!["result".into(), "successor".into()],
                result_binding: None,
            }
        };
        let sequence =
            crate::canonical_continuation_sequence(&next.continuation.token).ok_or_else(invalid)?;
        let group = ContinuationToken(format!("merge-{sequence}"));
        match self {
            Self::Ephemeral(journal) => {
                validate_continuation_encoding_size(&next.continuation)?;
                encode_json_capped(next, MAX_JOURNAL_ENTRY_BYTES, "successor request")?;
                let first = &parent.continuation.token;
                let second = &next.continuation.token;
                let dispatch = journal.dispatches.get(first).ok_or_else(invalid)?;
                if dispatch.request != *parent {
                    return Err(invalid());
                }
                if dispatch.acknowledged {
                    return dispatch.terminal_step.clone().ok_or_else(invalid);
                }
                match lease {
                    Some((lease, now_ms)) => validate_retry_lease(dispatch, lease, now_ms)?,
                    None if dispatch.lease_expires_at_ms.is_some() => {
                        return Err(journal_fault(
                            "LSV4024",
                            "leased effect must be completed through dispatch acknowledgement",
                        ));
                    }
                    None => {}
                }
                let existing = journal
                    .merge_groups
                    .iter()
                    .find(|(_, group)| group.branch_tokens.contains(first));
                let (group, plan, mut tokens) = if let Some((token, existing)) = existing {
                    if plan.order != ExecutionOrder::Dataflow
                        || existing.terminal_step.is_some()
                        || existing.branch_tokens.last() != Some(first)
                    {
                        return Err(invalid());
                    }
                    let plan = append_plan(&existing.plan, existing.branch_tokens.len() - 1)?;
                    let prior = existing
                        .branch_tokens
                        .iter()
                        .filter(|token| *token != first)
                        .map(|token| {
                            journal
                                .dispatches
                                .get(token)
                                .and_then(|dispatch| dispatch.terminal_step.as_ref())
                                .ok_or_else(invalid)
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if let Some(stop) =
                        sequence_stop(prior.into_iter().chain(std::iter::once(step)))?
                    {
                        return match lease {
                            Some((lease, now_ms)) => {
                                journal.acknowledge_dispatch(lease, now_ms, &stop)
                            }
                            None => journal.record_completed(&parent.continuation, &stop),
                        };
                    }
                    (token.clone(), plan, existing.branch_tokens.clone())
                } else {
                    (group, plan, vec![first.clone()])
                };
                if journal.dispatches.contains_key(second)
                    || journal.dispatches.contains_key(&group)
                    || journal.merge_groups.contains_key(second)
                    || journal
                        .merge_groups
                        .get(&group)
                        .is_some_and(|old| !old.branch_tokens.contains(first))
                {
                    return Err(invalid());
                }
                tokens.push(second.clone());
                encode_json_capped(&plan, MAX_CONTINUATION_BYTES, "dataflow plan")?;
                let dispatch = journal.dispatches.get_mut(first).ok_or_else(invalid)?;
                dispatch.acknowledged = true;
                dispatch.lease_expires_at_ms = None;
                dispatch.terminal_step = Some(step.clone());
                journal.dispatches.insert(
                    second.clone(),
                    EphemeralDispatch {
                        request: next.clone(),
                        attempt: 0,
                        lease_expires_at_ms: None,
                        ready_at_ms: 0,
                        retry_count: 0,
                        last_error: None,
                        acknowledged: false,
                        terminal_step: None,
                    },
                );
                journal.merge_groups.insert(
                    group,
                    EphemeralMergeGroup {
                        plan,
                        branch_tokens: tokens,
                        terminal_step: None,
                    },
                );
                Ok(step.clone())
            }
            Self::Sqlite(journal) => {
                journal.record_successor(parent, step, next, lease, &group, &plan)
            }
        }
    }
}

fn accumulate_output(items: &mut usize, step: &Step) -> Result<(), Fault> {
    let Step::Done(value) = step else {
        return Err(invalid());
    };
    *items = items.saturating_add(validate_value(value, 0)?);
    if *items > DEFAULT_MAX_OUTPUT_ITEMS {
        return Err(journal_fault(
            "LSV2404",
            "sequential output exceeds runtime bounds",
        ));
    }
    Ok(())
}

impl SqliteJournal {
    fn record_successor(
        &mut self,
        parent: &EffectRequest,
        step: &Step,
        next: &EffectRequest,
        lease: CompletionLease<'_>,
        group: &ContinuationToken,
        plan: &MergePlan,
    ) -> Result<Step, Fault> {
        let image_bytes =
            encode_json_capped(&parent.continuation, MAX_CONTINUATION_BYTES, "continuation")?;
        let step_bytes = encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "terminal step")?;
        let next_image =
            encode_json_capped(&next.continuation, MAX_CONTINUATION_BYTES, "continuation")?;
        let next_request = encode_json_capped(next, MAX_JOURNAL_ENTRY_BYTES, "successor request")?;
        let transaction = self
            .connection
            .transaction_with_behavior(TransactionBehavior::Immediate)
            .map_err(|error| journal_error("LSV4034", "failed to lock result chain", error))?;
        let first = parent.continuation.token.as_str();
        let second = next.continuation.token.as_str();
        let existing = load_record(&transaction, first)?.ok_or_else(invalid)?;
        if existing.image != image_bytes || request_for(&transaction, first)? != *parent {
            return Err(invalid());
        }
        // A competing acknowledgement owns both the result and the chosen successor.
        if existing.state == "completed" {
            let authoritative: Step = decode_bounded(
                &existing.terminal_step.ok_or_else(invalid)?,
                MAX_JOURNAL_ENTRY_BYTES,
            )?;
            validate_terminal_step(&authoritative)?;
            transaction
                .commit()
                .map_err(|error| journal_error("LSV4034", "failed to close result chain", error))?;
            return Ok(authoritative);
        }
        let dispatch = load_dispatch(&transaction, first)?.ok_or_else(invalid)?;
        match lease {
            Some((lease, now_ms)) => {
                if dispatch.state != "leased"
                    || dispatch.attempt != lease.attempt
                    || dispatch.retry_count != lease.retry_count
                    || dispatch.ready_at_ms > now_ms
                    || dispatch.lease_expires_at_ms != Some(lease.lease_expires_at_ms)
                {
                    return Err(journal_fault(
                        "LSV4022",
                        "dispatch lease has been superseded",
                    ));
                }
                if lease.lease_expires_at_ms < now_ms {
                    return Err(journal_fault("LSV4023", "dispatch lease has expired"));
                }
            }
            None if dispatch.state == "leased" => {
                return Err(journal_fault(
                    "LSV4024",
                    "leased effect must be completed through dispatch acknowledgement",
                ));
            }
            None => {}
        }
        let context: Option<(String, Vec<u8>, i64, String)> = transaction
            .query_row(
                "SELECT g.token, g.plan, b.position, g.state FROM vm_merge_branches b
             JOIN vm_merge_groups g ON g.token = b.group_token WHERE b.branch_token = ?1",
                [first],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()
            .map_err(|error| journal_error("LSV4034", "failed to locate dataflow chain", error))?;
        let (group, plan, old_plan_size, extending) = if let Some((token, bytes, position, state)) =
            context
        {
            if plan.order != ExecutionOrder::Dataflow || state != "pending" {
                return Err(invalid());
            }
            let old: MergePlan = decode_bounded(&bytes, MAX_CONTINUATION_BYTES)?;
            let plan = append_plan(&old, usize::try_from(position).map_err(|_| invalid())?)?;
            let prior = completed_prefix(&transaction, &token, first)?;
            if let Some(stop) = sequence_stop(prior.iter().chain(std::iter::once(step)))? {
                let bytes =
                    encode_json_capped(&stop, MAX_JOURNAL_ENTRY_BYTES, "dataflow output fault")?;
                ensure_growth(&transaction, bytes.len())?;
                complete_terminal_record(&transaction, first, &bytes)?;
                finalize_merge_group(&transaction, first)?;
                transaction
                    .commit()
                    .map_err(|error| journal_error("LSV4034", "failed to stop dataflow", error))?;
                return Ok(stop);
            }
            (ContinuationToken(token), plan, bytes.len(), true)
        } else {
            (group.clone(), plan.clone(), 0, false)
        };
        let plan_bytes = encode_json_capped(&plan, MAX_CONTINUATION_BYTES, "result chain")?;
        let conflict: bool = transaction
            .query_row(
                "SELECT EXISTS (
               SELECT 1 FROM vm_effects WHERE token IN (?1, ?2)
               UNION ALL SELECT 1 FROM vm_merge_groups WHERE token = ?1 OR (token = ?2 AND NOT ?3)
             )",
                params![second, group.as_str(), extending],
                |row| row.get(0),
            )
            .map_err(|error| {
                journal_error("LSV4034", "failed to check result-chain identity", error)
            })?;
        if conflict {
            return Err(invalid());
        }
        if journal_record_count(&transaction)?.saturating_add(if extending { 1 } else { 2 })
            > MAX_JOURNAL_RECORDS
        {
            return Err(journal_fault("LSV4006", "journal record limit reached"));
        }
        ensure_growth(
            &transaction,
            step_bytes.len()
                + next_image.len()
                + next_request.len()
                + plan_bytes.len().saturating_sub(old_plan_size),
        )?;
        if extending {
            transaction
                .execute(
                    "UPDATE vm_merge_groups SET plan = ?2 WHERE token = ?1 AND state = 'pending'",
                    params![group.as_str(), plan_bytes],
                )
                .map_err(|error| {
                    journal_error("LSV4034", "failed to extend dataflow plan", error)
                })?;
        } else {
            transaction
            .execute(
                "INSERT INTO vm_merge_groups(token, plan, state, terminal_step, execution_order)
             VALUES (?1, ?2, 'pending', NULL, 'sequential')",
                params![group.as_str(), plan_bytes],
            )
            .map_err(|error| journal_error("LSV4034", "failed to store result chain", error))?;
        }
        complete_terminal_record(&transaction, first, &step_bytes)?;
        transaction
            .execute(
                "INSERT INTO vm_effects(token, state, image, deadline_at_ms, terminal_step)
             VALUES (?1, 'pending', ?2, ?3, NULL)",
                params![
                    second,
                    next_image,
                    next.continuation
                        .deadline_at_ms
                        .map(i64::try_from)
                        .transpose()
                        .map_err(|_| invalid())?
                ],
            )
            .map_err(|error| journal_error("LSV4034", "failed to store successor effect", error))?;
        transaction
            .execute(
                "INSERT INTO vm_dispatches(token, request, state, attempt, lease_expires_at_ms)
             VALUES (?1, ?2, 'ready', 0, NULL)",
                params![second, next_request],
            )
            .map_err(|error| journal_error("LSV4034", "failed to enqueue successor", error))?;
        let last = plan.branches.len() - 1;
        for (position, token) in [(last - 1, first), (last, second)] {
            if extending && token == first {
                continue;
            }
            transaction.execute(
                "INSERT INTO vm_merge_branches(group_token, branch_token, branch_name, position)
                 VALUES (?1, ?2, ?3, ?4)", params![group.as_str(), token, plan.branches[position], position as i64],
            ).map_err(|error| journal_error("LSV4034", "failed to link result chain", error))?;
        }
        transaction
            .commit()
            .map_err(|error| journal_error("LSV4034", "failed to commit result chain", error))?;
        Ok(step.clone())
    }
}

fn append_plan(plan: &MergePlan, position: usize) -> Result<MergePlan, Fault> {
    validate_merge_plan(plan)?;
    if plan.order != ExecutionOrder::Dataflow
        || position + 1 != plan.branches.len()
        || plan.branches.len() >= crate::MAX_MERGE_BRANCHES
    {
        return Err(invalid());
    }
    let mut plan = plan.clone();
    plan.branches
        .push(format!("step_{}", plan.branches.len() + 1));
    Ok(plan)
}

fn completed_prefix(
    connection: &Connection,
    group: &str,
    current: &str,
) -> Result<Vec<Step>, Fault> {
    let mut statement = connection.prepare(
        "SELECT e.terminal_step FROM vm_merge_branches b JOIN vm_effects e ON e.token = b.branch_token
         WHERE b.group_token = ?1 AND b.branch_token != ?2 ORDER BY b.position",
    ).map_err(|error| journal_error("LSV4034", "failed to inspect dataflow output", error))?;
    statement
        .query_map(params![group, current], |row| row.get::<_, Vec<u8>>(0))
        .map_err(|error| journal_error("LSV4034", "failed to load dataflow output", error))?
        .map(|row| {
            let bytes =
                row.map_err(|error| journal_error("LSV4034", "invalid dataflow output", error))?;
            decode_bounded(&bytes, MAX_JOURNAL_ENTRY_BYTES)
        })
        .collect()
}

fn request_for(connection: &Connection, token: &str) -> Result<EffectRequest, Fault> {
    let dispatch = load_dispatch(connection, token)?.ok_or_else(invalid)?;
    let request = decode_bounded(&dispatch.request, MAX_JOURNAL_ENTRY_BYTES)?;
    validate_effect_request(&request)?;
    Ok(request)
}

pub(super) fn validate_record(
    connection: &Connection,
    image: &ContinuationImage,
    terminal: Option<&Step>,
) -> Result<(), Fault> {
    if !matches!(
        image.schema_version,
        crate::SUCCESSOR_CONTINUATION_SCHEMA_VERSION
            | crate::DATAFLOW_CONTINUATION_SCHEMA_VERSION
            | crate::CONDITIONAL_CONTINUATION_SCHEMA_VERSION
    ) {
        return Ok(());
    }
    if request_for(connection, image.token.as_str())?.continuation != *image {
        return Err(invalid());
    }
    let group: Option<(Vec<u8>, i64)> = connection
        .query_row(
            "SELECT g.plan, b.position FROM vm_merge_branches b
         JOIN vm_merge_groups g ON g.token = b.group_token WHERE b.branch_token = ?1",
            [image.token.as_str()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .optional()
        .map_err(|error| {
            journal_error("LSV4034", "failed to validate result-chain context", error)
        })?;
    if matches!(
        image.schema_version,
        crate::DATAFLOW_CONTINUATION_SCHEMA_VERSION
            | crate::CONDITIONAL_CONTINUATION_SCHEMA_VERSION
    ) {
        let binding = image.result_binding.as_ref().ok_or_else(invalid)?;
        let needs_successor = match terminal {
            Some(Step::Done(crate::Value::Scalar { value })) => {
                if !binding.body.can_return_without_suspending()
                    || (!binding.body.is_pure()
                        && image.schema_version != crate::CONDITIONAL_CONTINUATION_SCHEMA_VERSION)
                    || binding.validate(&image.pending_effect)?
                        != leselang_hir::Type::Scalar(value.scalar_type())
                {
                    return Err(invalid());
                }
                false
            }
            Some(Step::Done(_)) if binding.body.is_pure() => return Err(invalid()),
            Some(Step::Done(_)) => true,
            _ => false,
        };
        return match group {
            Some((bytes, position)) => {
                let plan: MergePlan = decode_bounded(&bytes, MAX_CONTINUATION_BYTES)?;
                validate_merge_plan(&plan)?;
                let position = usize::try_from(position).map_err(|_| invalid())?;
                if plan.order != ExecutionOrder::Dataflow
                    || position >= plan.branches.len()
                    || needs_successor != (position + 1 < plan.branches.len())
                {
                    return Err(invalid());
                }
                Ok(())
            }
            None if needs_successor => Err(invalid()),
            None => Ok(()),
        };
    }
    match (terminal, group) {
        (Some(Step::Done(_)), Some((bytes, 0))) => {
            let plan: MergePlan = decode_bounded(&bytes, MAX_CONTINUATION_BYTES)?;
            if plan.order != ExecutionOrder::ResultChain {
                return Err(invalid());
            }
            validate_merge_plan(&plan)
        }
        (Some(Step::Done(_)), _) | (_, Some(_)) => Err(invalid()),
        (_, None) => Ok(()),
    }
}

pub(super) fn validate_chain(
    connection: &Connection,
    group: &ContinuationToken,
) -> Result<(), Fault> {
    let mut statement = connection
        .prepare(
            "SELECT b.branch_token, e.terminal_step FROM vm_merge_branches b
         JOIN vm_effects e ON e.token = b.branch_token
         WHERE b.group_token = ?1 ORDER BY b.position",
        )
        .map_err(|error| journal_error("LSV4034", "failed to inspect result chain", error))?;
    let records = statement
        .query_map([group.as_str()], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, Option<Vec<u8>>>(1)?))
        })
        .map_err(|error| journal_error("LSV4034", "failed to load result chain", error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| journal_error("LSV4034", "invalid result-chain record", error))?;
    if records.len() < 2 || records.len() > crate::MAX_MERGE_BRANCHES {
        return Err(invalid());
    }
    let mut parent = request_for(connection, &records[0].0)?;
    for pair in records.windows(2) {
        let result: Step = decode_bounded(
            pair[0].1.as_deref().ok_or_else(invalid)?,
            MAX_JOURNAL_ENTRY_BYTES,
        )?;
        let next = request_for(connection, &pair[1].0)?;
        validate_pair(&parent, &result, &next)?;
        parent = next;
    }
    Ok(())
}
