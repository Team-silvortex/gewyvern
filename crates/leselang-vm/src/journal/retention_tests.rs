use super::*;

fn fixture() -> (EphemeralJournal, Vec<(ContinuationToken, Step)>) {
    let mut journal = EphemeralJournal::default();
    let mut local = Vec::new();
    let original = crate::continuation_contract_tests::requests().remove(0);
    for sequence in 1..=7 {
        let token = ContinuationToken::from_wire(format!("continuation-{sequence}"));
        let mut request = original.clone();
        request.continuation.token = token.clone();
        let step = Step::Done(crate::Value::Scalar {
            value: crate::ScalarValue::Integer(sequence),
        });
        let completed = sequence != 6;
        if completed {
            local.push((token.clone(), step.clone()));
        }
        journal.dispatches.insert(
            token.clone(),
            EphemeralDispatch {
                request,
                attempt: 1,
                lease_expires_at_ms: None,
                ready_at_ms: 0,
                retry_count: 0,
                last_error: None,
                acknowledged: completed,
                terminal_step: completed.then_some(step),
            },
        );
        journal.debugger_audits.insert(
            format!("audit-{sequence}"),
            EphemeralDebuggerAudit {
                token,
                idempotency_key: format!("key-{sequence}"),
                principal_id: "operator".into(),
                record: DebuggerAuditRecord {
                    command_id: leserpent_domain::CommandId::new(format!("audit-{sequence}"))
                        .unwrap(),
                    principal_id: "operator".into(),
                    origin: leselang_host_contract::CommandOrigin::Gui,
                    session_id: "session".into(),
                    expected_revision: leselang_host_contract::Revision(1),
                    observed_at_ms: 0,
                },
            },
        );
    }
    for (name, first, completed) in [("finished", 3, true), ("live", 5, false)] {
        journal.merge_groups.insert(
            ContinuationToken::from_wire(name.into()),
            EphemeralMergeGroup {
                plan: MergePlan {
                    order: ExecutionOrder::Parallel,
                    branches: vec!["first".into(), "second".into()],
                    result_binding: None,
                },
                branch_tokens: vec![
                    ContinuationToken::from_wire(format!("continuation-{first}")),
                    ContinuationToken::from_wire(format!("continuation-{}", first + 1)),
                ],
                terminal_step: completed.then_some(Step::Done(crate::Value::Scalar {
                    value: crate::ScalarValue::Integer(7),
                })),
            },
        );
    }
    (journal, local)
}

fn snapshot(journal: &EphemeralJournal) -> Vec<u8> {
    let dispatches = journal
        .dispatches
        .iter()
        .map(|(token, row)| {
            (
                token,
                &row.request,
                row.attempt,
                row.lease_expires_at_ms,
                row.ready_at_ms,
                row.retry_count,
                &row.last_error,
                row.acknowledged,
                &row.terminal_step,
            )
        })
        .collect::<Vec<_>>();
    let groups = journal
        .merge_groups
        .iter()
        .map(|(token, row)| (token, &row.plan, &row.branch_tokens, &row.terminal_step))
        .collect::<Vec<_>>();
    let audits = journal
        .debugger_audits
        .iter()
        .map(|(key, row)| {
            (
                key,
                &row.token,
                &row.idempotency_key,
                &row.principal_id,
                &row.record,
            )
        })
        .collect::<Vec<_>>();
    serde_json::to_vec(&(dispatches, groups, audits)).unwrap()
}

fn report(report: &JournalCompaction) -> (usize, usize, usize, &[ContinuationToken]) {
    (
        report.removed_records,
        report.remaining_completed,
        report.reclaimed_logical_bytes,
        &report.pruned_tokens,
    )
}

#[test]
fn successful_compaction_keeps_frozen_reports_units_protection_and_audit_cascade() {
    for retained in [1, 2, 3, 4, 10_000] {
        for batch in [1, 2, 3, 1_000] {
            let policy = RetentionPolicy {
                max_completed_records: retained,
                max_delete_per_run: batch,
            };
            let (mut expected, expected_local) = fixture();
            let (mut actual, actual_local) = fixture();
            let original_local = serde_json::to_vec(&actual_local).unwrap();
            let old = frozen_compact(&mut expected, &expected_local, &policy).unwrap();
            let new = actual.compact_completed(&actual_local, &policy).unwrap();
            assert_eq!(report(&new), report(&old));
            assert_eq!(snapshot(&actual), snapshot(&expected));
            assert_eq!(serde_json::to_vec(&actual_local).unwrap(), original_local);
            for token in ["continuation-5", "continuation-6"] {
                assert!(
                    actual
                        .dispatches
                        .contains_key(&ContinuationToken::from_wire(token.into()))
                );
            }
            assert!(
                actual
                    .merge_groups
                    .contains_key(&ContinuationToken::from_wire("live".into()))
            );
        }
    }
}

#[test]
fn late_encoding_refusal_keeps_the_entire_memory_batch_and_audits_unchanged() {
    for defect in 0..5 {
        let (mut journal, mut local) = fixture();
        let group = ContinuationToken::from_wire("finished".into());
        let branch = ContinuationToken::from_wire("continuation-4".into());
        let oversized_step = || {
            Step::Fault(Fault {
                code: "fixture".into(),
                message: "x".repeat(MAX_JOURNAL_ENTRY_BYTES),
            })
        };
        // Isolated corrupt-state fixtures exercise failure atomicity, not public admission.
        match defect {
            0 => {
                journal.merge_groups.get_mut(&group).unwrap().plan.branches[0] =
                    "x".repeat(MAX_CONTINUATION_BYTES)
            }
            1 => {
                journal.merge_groups.get_mut(&group).unwrap().terminal_step = Some(oversized_step())
            }
            2 => {
                local
                    .iter_mut()
                    .find(|(token, _)| *token == branch)
                    .unwrap()
                    .1 = oversized_step()
            }
            3 => {
                journal
                    .dispatches
                    .get_mut(&branch)
                    .unwrap()
                    .request
                    .effect_id = "x".repeat(MAX_JOURNAL_ENTRY_BYTES)
            }
            _ => {
                journal.dispatches.get_mut(&branch).unwrap().last_error = Some(EffectError {
                    code: "fixture".into(),
                    message: "x".repeat(MAX_JOURNAL_ENTRY_BYTES),
                    class: crate::EffectErrorClass::Transient,
                })
            }
        }
        let before = snapshot(&journal);
        let original_local = serde_json::to_vec(&local).unwrap();
        let result = journal.compact_completed(
            &local,
            &RetentionPolicy {
                max_completed_records: 1,
                max_delete_per_run: 3,
            },
        );
        let error = result.err().expect("oversized fixture must refuse");
        let (label, limit) = match defect {
            0 => ("merge plan", MAX_CONTINUATION_BYTES),
            1 => ("merge result", MAX_JOURNAL_ENTRY_BYTES),
            2 => ("completed step", MAX_JOURNAL_ENTRY_BYTES),
            3 => ("effect request", MAX_JOURNAL_ENTRY_BYTES),
            _ => ("effect error", MAX_JOURNAL_ENTRY_BYTES),
        };
        assert_eq!(
            error,
            Fault {
                code: "LSV3002".into(),
                message: format!("{label} exceeds {limit} bytes"),
            }
        );
        assert!(
            snapshot(&journal) == before,
            "defect {defect} partially mutated the batch"
        );
        assert!(serde_json::to_vec(&local).unwrap() == original_local);
    }
}

#[test]
fn adapter_policy_uses_the_released_vm_api_and_refuses_before_local_or_journal_mutation() {
    for retained in [0, 1, 500, 5_000, 10_000, 10_001, usize::MAX] {
        for batch in [0, 1, 500, 1_000, 1_001, usize::MAX] {
            let policy: leselang_command::RetentionPolicy = RetentionPolicy {
                max_completed_records: retained,
                max_delete_per_run: batch,
            };
            let valid = retained != 0
                && retained <= MAX_JOURNAL_RECORDS
                && batch != 0
                && batch <= crate::MAX_COMPACTION_BATCH;
            let expected = if valid {
                Ok(())
            } else {
                Err(Fault {
                    code: "LSV2301".into(),
                    message: "journal retention policy exceeds runtime bounds".into(),
                })
            };
            assert_eq!(crate::validate_retention_policy(&policy), expected);
            if !valid {
                let (journal, local) = fixture();
                let before = snapshot(&journal);
                let local_before = serde_json::to_vec(&local).unwrap();
                let mut vm = crate::Vm {
                    journal: Journal::Ephemeral(journal),
                    completed: local.into_iter().collect(),
                    ..crate::Vm::default()
                };
                assert_eq!(
                    vm.compact_journal(&policy).unwrap_err(),
                    expected.unwrap_err()
                );
                #[cfg(feature = "sqlite-journal")]
                let journal = match &vm.journal {
                    Journal::Ephemeral(journal) => journal,
                    Journal::Sqlite(_) => unreachable!(),
                };
                #[cfg(not(feature = "sqlite-journal"))]
                let Journal::Ephemeral(journal) = &vm.journal;
                assert_eq!(snapshot(journal), before);
                assert_eq!(
                    serde_json::to_vec(&vm.completed.iter().collect::<Vec<_>>()).unwrap(),
                    local_before
                );
            }
        }
    }
    let (journal, local) = fixture();
    let mut vm = crate::Vm {
        journal: Journal::Ephemeral(journal),
        completed: local.into_iter().collect(),
        ..crate::Vm::default()
    };
    let report: leselang_command::CompactionReport = vm
        .compact_journal(&RetentionPolicy {
            max_completed_records: 1,
            max_delete_per_run: 3,
        })
        .unwrap();
    assert_eq!(report.removed_records, 3);
    assert_eq!(report.remaining_completed, 1);
    assert_eq!(vm.completed_count(), 2);
}

// Frozen pre-migration successful compaction, including byte accounting and order.
fn frozen_compact(
    journal: &mut EphemeralJournal,
    local_completed: &[(ContinuationToken, Step)],
    policy: &RetentionPolicy,
) -> Result<JournalCompaction, Fault> {
    let protected = journal
        .merge_groups
        .values()
        .flat_map(|group| group.branch_tokens.iter())
        .collect::<BTreeSet<_>>();
    let mut units = local_completed
        .iter()
        .filter(|(token, _)| !protected.contains(token))
        .map(|(token, _)| EphemeralCompactionUnit::Effect(token.clone()))
        .collect::<Vec<_>>();
    for (token, group) in &journal.merge_groups {
        if group.terminal_step.is_some() {
            let age_token = group
                .branch_tokens
                .first()
                .ok_or_else(|| journal_fault("LSV4007", "completed merge group has no branches"))?;
            units.push(EphemeralCompactionUnit::Merge {
                group_token: token.clone(),
                age_token: age_token.clone(),
            });
        }
    }
    units.sort_by(|left, right| continuation_age_order(left.age_token(), right.age_token()));
    let completed_count = units.len();
    let remove_count = completed_count
        .saturating_sub(policy.max_completed_records)
        .min(policy.max_delete_per_run);
    let mut reclaimed_logical_bytes = 0usize;
    let mut removed_tokens = Vec::with_capacity(remove_count);
    for unit in units.into_iter().take(remove_count) {
        let tokens = match unit {
            EphemeralCompactionUnit::Effect(token) => vec![token],
            EphemeralCompactionUnit::Merge { group_token, .. } => {
                let group = journal
                    .merge_groups
                    .remove(&group_token)
                    .ok_or_else(|| journal_fault("LSV4033", "selected merge group disappeared"))?;
                let terminal_step = group.terminal_step.as_ref().ok_or_else(|| {
                    journal_fault("LSV4007", "selected merge group is not completed")
                })?;
                reclaimed_logical_bytes = reclaimed_logical_bytes
                    .saturating_add(
                        encode_json_capped(&group.plan, MAX_CONTINUATION_BYTES, "merge plan")?
                            .len(),
                    )
                    .saturating_add(
                        encode_json_capped(terminal_step, MAX_JOURNAL_ENTRY_BYTES, "merge result")?
                            .len(),
                    );
                group.branch_tokens
            }
        };
        for token in tokens {
            if let Some((_, step)) = local_completed.iter().find(|(local, _)| local == &token) {
                reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(
                    encode_json_capped(step, MAX_JOURNAL_ENTRY_BYTES, "completed step")?.len(),
                );
            }
            if let Some(dispatch) = journal.dispatches.remove(&token) {
                reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(
                    encode_json_capped(
                        &dispatch.request,
                        MAX_JOURNAL_ENTRY_BYTES,
                        "effect request",
                    )?
                    .len(),
                );
                if let Some(error) = &dispatch.last_error {
                    reclaimed_logical_bytes = reclaimed_logical_bytes.saturating_add(
                        encode_json_capped(error, MAX_JOURNAL_ENTRY_BYTES, "effect error")?.len(),
                    );
                }
            }
            removed_tokens.push(token);
        }
    }
    if !removed_tokens.is_empty() {
        // Match SQLite's audit cascade once per batch, retaining live group members.
        journal
            .debugger_audits
            .retain(|_, audit| journal.dispatches.contains_key(&audit.token));
    }
    Ok(JournalCompaction {
        removed_records: remove_count,
        pruned_tokens: removed_tokens,
        remaining_completed: completed_count.saturating_sub(remove_count),
        reclaimed_logical_bytes,
    })
}
