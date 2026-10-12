use std::{fs, path::Path, process::Command};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn shared_watermark_planning_keeps_native_validation_reads_and_commit_before_publish() {
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let advance = journal
        .split("fn advance_sequence_watermark(")
        .nth(1)
        .unwrap()
        .split("pub(super) fn validate_lease_clock")
        .next()
        .unwrap();
    let requirement = advance
        .find("SequenceWatermarkRequirement::after_observation(")
        .unwrap();
    let read = advance
        .find("SELECT next_sequence FROM vm_metadata")
        .unwrap();
    let signed = advance.find("u64::try_from(stored)").unwrap();
    let reconcile = advance.find(".reconcile(stored)").unwrap();
    let write = advance
        .find("UPDATE vm_metadata SET next_sequence")
        .unwrap();
    assert!(requirement < read && read < signed && signed < reconcile && reconcile < write);
    for required in [
        "i64::MAX as u64",
        "if update.write_required",
        "Ok(update.next_sequence)",
        "LSV4009",
        "LSV4007",
        "LSV4008",
    ] {
        assert!(advance.contains(required));
    }
    let allocation = journal
        .rsplit("fn allocate_sequence(&mut self, local_next:")
        .next()
        .unwrap()
        .split("fn record_pending(")
        .next()
        .unwrap();
    let numeric = allocation
        .find("SequenceWatermarkRequirement::after_observation(")
        .unwrap();
    assert!(allocation.find("TransactionBehavior::Immediate").unwrap() < numeric);
    assert!(allocation.find("SELECT next_sequence").unwrap() < numeric);
    assert!(numeric < allocation.find("UPDATE vm_metadata").unwrap());
    assert!(allocation.find(".commit()").unwrap() < allocation.find("Ok(sequence)").unwrap());
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/lib.rs")).unwrap();
    let restore = vm
        .split("fn restore_inner(")
        .nth(1)
        .unwrap()
        .split("pub fn resume(")
        .next()
        .unwrap();
    let next = restore
        .find("SequenceWatermarkRequirement::after_observation(")
        .unwrap();
    assert!(
        restore
            .find("canonical_continuation_sequence(&image.token)")
            .unwrap()
            < next
    );
    assert!(next < restore.rfind(".record_pending(").unwrap());
    assert!(
        restore.rfind(".record_pending(").unwrap() < restore.find("self.next_effect_id =").unwrap()
    );
    assert!(journal.contains("watermark_planning_preserves_frozen_sqlite_fault_order_and_writes"));
    assert!(
        fs::read_to_string(root().join("crates/leselang-vm/tests/restore_allocator.rs"))
            .unwrap()
            .contains(
                "watermark_planning_preserves_every_table_on_import_recovery_and_exhaustion_refusal"
            )
    );
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/sequence_watermark.rs"))
            .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
        "UPDATE",
        "INSERT",
    ] {
        assert!(!core.contains(absent), "shared watermark imported {absent}");
    }
    assert!(core.contains("stored_watermark.max(self.required)"));
    assert!(core.contains("write_required: self.required > stored_watermark"));
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("consumer/tests/sequence_watermark.rs")
    );
}

#[test]
fn continuation_context_kernel_preserves_native_phase_order_in_every_succession_path() {
    let source = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    let helper = source
        .split("pub(super) fn same_inherited_context(")
        .nth(1)
        .unwrap()
        .split("pub(super) fn validate_pair(")
        .next()
        .unwrap()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for required in [
        "validate_inherited_context(",
        "original.continuation.resource_budget()",
        "saved.continuation.resource_budget()",
        "authority(&original.operation) == authority(&saved.operation)",
        "original.continuation.expected_revision == saved.continuation.expected_revision",
    ] {
        assert!(helper.contains(required), "context adapter lost {required}");
    }
    assert!(!helper.contains("Fuel::new"));
    let pair = source
        .split("pub(super) fn validate_pair(")
        .nth(1)
        .unwrap()
        .split("enum PreparedResult")
        .next()
        .unwrap();
    let context = pair.find("same_inherited_context(parent, next)").unwrap();
    assert!(pair.find("!schema_valid").unwrap() < context);
    assert!(pair.find("validate_bound_value(first, value)?").unwrap() < context);
    assert!(
        context
            < pair
                .find("first.fuel_remaining <= second.fuel_remaining")
                .unwrap()
    );
    assert!(context < pair.find("canonical_continuation_sequence").unwrap());
    let groups =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    let admission = groups
        .split("pub(super) fn validate_requests(")
        .nth(1)
        .unwrap()
        .split("pub(super) fn validate_member(")
        .next()
        .unwrap();
    assert_eq!(
        admission
            .matches("same_inherited_context(first, request)")
            .count(),
        2
    );
    let (prefix, tail) = admission.split_once("let mut previous_fuel").unwrap();
    assert!(
        prefix
            .find("Some(image.fuel_remaining) != expected_fuel")
            .unwrap()
            < prefix.find("same_inherited_context").unwrap()
    );
    assert!(
        tail.find("image.fuel_remaining >= previous_fuel").unwrap()
            < tail.find("same_inherited_context").unwrap()
    );
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/continuation_context.rs"))
            .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
        "fuel_remaining",
    ] {
        assert!(!core.contains(absent), "shared context imported {absent}");
    }
    for required in [
        "drop(same_authority)",
        "drop(same_revision)",
        "original.deadline_at_ms != saved.deadline_at_ms",
    ] {
        assert!(core.contains(required));
    }
    assert!(
        source.contains("inherited_context_keeps_frozen_native_agreement_across_operation_domains")
    );
    assert!(fs::read_to_string(root().join("crates/leselang-vm/tests/successor.rs")).unwrap()
        .contains("inherited_context_refusal_preserves_journal_and_imported_watermark_for_every_succession_kind"));
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("consumer/tests/continuation_context.rs")
    );
}

#[test]
fn successor_capture_extension_shares_retention_without_authority_or_saved_body_execution() {
    let source = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    let pair = source
        .split("pub(super) fn validate_pair(")
        .nth(1)
        .unwrap()
        .split("enum PreparedResult")
        .next()
        .unwrap();
    let shared = pair.find("validate_extension(prior, binding,").unwrap();
    for guard in [
        "validate_effect_request(parent)?",
        "validate_effect_request(next)?",
        "validate_bound_value(first, value)?",
        "same_inherited_context(parent, next)",
        "first.fuel_remaining <= second.fuel_remaining",
        "canonical_continuation_sequence",
        "prior.validate(&first.pending_effect)?",
    ] {
        assert!(
            pair.find(guard).unwrap() < shared,
            "extension displaced {guard}"
        );
    }
    for adapter in [
        "validate_capture_extension(",
        "leselang_hir::MAX_EFFECT_NESTING_DEPTH",
        "old.group == saved.group",
        "old.result == saved.result",
        "saved.name == self.prior.name",
        "CaptureExtensionError::Native { error, .. } => error",
        "capture_extension_preserves_frozen_native_alias_retention_and_late_fault_order",
    ] {
        assert!(source.contains(adapter), "extension adapter lost {adapter}");
    }
    assert!(!pair.contains("Fuel::new"));
    let journal =
        fs::read_to_string(root().join("crates/leselang-vm/src/journal/successor.rs")).unwrap();
    assert!(journal.contains("validate_pair(parent, step, next)?"));
    assert!(journal.contains("validate_pair(parent, &result, &next)?"));
    assert!(fs::read_to_string(root().join("crates/leselang-vm/tests/dataflow.rs")).unwrap()
        .contains("successor_capture_extension_refusal_preserves_every_journal_table_and_original_budget"));
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/capture_extension.rs"))
            .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
    ] {
        assert!(!core.contains(absent));
    }
    assert!(core.contains("drop(domain)"));
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("consumer/tests/capture_extension.rs")
    );
}

#[test]
fn live_and_recovered_capture_correspondence_delegates_traversal_without_projection_or_authority_policy()
 {
    let source =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    let gate = source
        .split("pub(super) fn validate_saved_capture(")
        .nth(1)
        .unwrap()
        .split("mod correspondence_tests")
        .next()
        .unwrap();
    for required in [
        "fields.len() < branches.len()",
        "fields.len() != operations.len()",
        "max_saved_bindings: leselang_hir::MAX_EFFECT_NESTING_DEPTH",
        "max_original_rows: crate::MAX_MERGE_BRANCHES",
        "validate_capture_correspondence(",
        "matches_members(branches, &fields[..branches.len()])",
        "matches_capture(operations[original], &field.value)",
        "CaptureCorrespondenceError::Native { error, .. } => error",
    ] {
        assert!(gate.contains(required), "native capture lost {required}");
    }
    assert!(
        gate.find(".ok_or_else(invalid)?").unwrap()
            < gate.find("validate_capture_correspondence(").unwrap()
    );
    assert!(
        source
            .split("pub(super) fn advance_capture(")
            .nth(1)
            .unwrap()
            .contains("self.validate_saved_capture(request, prefix, &operations)?")
    );
    assert!(
        fs::read_to_string(root().join("crates/leselang-vm/src/journal/group_binding.rs"))
            .unwrap()
            .contains(
                "binding.validate_saved_capture(&request, &prefix_fields, &prefix_operations)?"
            )
    );
    assert!(source.contains("saved_capture_correspondence_preserves_frozen_native_fault_order"));
    assert!(fs::read_to_string(root().join("crates/leselang-vm/tests/parallel_group_tail.rs")).unwrap().contains("false_group_alias_and_result_capture_correspondence_blocks_recovery_without_mutation"));
    let core = fs::read_to_string(
        root().join("crates/leselang-runtime-core/src/capture_correspondence.rs"),
    )
    .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
    ] {
        assert!(!core.contains(absent));
    }
    assert!(core.contains("matched |= match_result("));
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("consumer/tests/capture_correspondence.rs")
    );
}

#[test]
fn live_sequence_and_bound_recovery_share_stop_precedence_without_replay_or_settlement() {
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let sequence = journal
        .split("fn sequence_stop<'a>(")
        .nth(1)
        .unwrap()
        .split("fn ensure_sequence_ready(")
        .next()
        .unwrap();
    for required in [
        "OrderedOutcomeSummary::new(",
        "OutcomeOrder::Sequential",
        "validate_terminal_step(step)?",
        "observe_success(validate_value(value, 0)?",
        "observe_stop(step.clone())",
        "if outcomes.has_stop()",
        "outcomes.finish(overflow)",
    ] {
        assert!(sequence.contains(required), "sequence lost {required}");
    }
    let bound =
        fs::read_to_string(root().join("crates/leselang-vm/src/journal/group_binding.rs")).unwrap();
    let recovery = bound
        .split("pub(super) fn validate_records(")
        .nth(1)
        .unwrap();
    for required in [
        "OrderedOutcomeSummary::new(",
        "OutcomeOrder::Sequential",
        "OutcomeOrder::Parallel",
        "observe_success(validate_value(&value, 0)?",
        "outcomes.observe_stop(step)",
        ".finish(|items, limit|",
        "if let Some(failure) = summary.first_stop",
        "validate_saved_capture(",
        "validate_reservation_proof(",
    ] {
        assert!(
            recovery.contains(required),
            "bound recovery lost {required}"
        );
    }
    assert!(
        recovery.find("if let Some(terminal) = terminal").unwrap()
            < recovery.find(".finish(|items, limit|").unwrap()
    );
    for source in [sequence, recovery] {
        for absent in ["Fuel::new", "resume_group(", "commit()"] {
            assert!(!source.contains(absent));
        }
    }
    assert!(
        journal.contains("sequence_stop_preserves_frozen_early_exit_and_native_fault_precedence")
    );
    assert!(
        fs::read_to_string(root().join("crates/leselang-vm/tests/group_receipts.rs"))
            .unwrap()
            .contains("bound_output_precedence_receipts_cannot_replace_aggregate_or_first_stop")
    );
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/outcome_summary.rs"))
            .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
    ] {
        assert!(!core.contains(absent));
    }
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("consumer/tests/outcome_summary.rs")
    );
}

#[test]
fn pending_sequence_recovery_delegates_frontier_phases_without_dispatch_or_evaluation() {
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let graph = journal
        .split("fn validate_merge_graph_records(")
        .nth(1)
        .unwrap()
        .split("fn validate_pending_sequence(")
        .next()
        .unwrap();
    assert!(graph.contains("if plan.order.is_sequential() && state == \"pending\""));
    assert!(
        graph
            .find("validate_pending_sequence(connection, &token, &branches)?")
            .unwrap()
            < graph.find("let terminal = recover_group_receipt(").unwrap()
    );
    let frontier = journal
        .split("fn validate_pending_sequence(")
        .nth(1)
        .unwrap()
        .split("fn recover_group_receipt(")
        .next()
        .unwrap();
    for required in [
        "inspect_sequential_frontier(",
        "crate::MAX_MERGE_BRANCHES",
        "MAX_JOURNAL_ENTRY_BYTES",
        "sequence_stop(terminal_steps.iter())?",
        "b.position > ?2",
        "d.attempt != 0 OR d.retry_count != 0",
        "SequenceFrontierError::Native { error, .. } => error",
        "SequenceFrontierError::OutOfOrder",
    ] {
        assert!(
            frontier.contains(required),
            "native frontier lost {required}"
        );
    }
    assert!(
        frontier.find("decode_bounded::<Step>(").unwrap()
            < frontier.find("connection.query_row(").unwrap()
    );
    for absent in [
        "Fuel::new",
        "resume(",
        "commit()",
        "advance_sequence_watermark(",
    ] {
        assert!(!frontier.contains(absent));
    }
    for proof in [
        "sequence_frontier_preserves_frozen_sqlite_phase_and_fault_order",
        "invalid_sequence_frontier_blocks_snapshot_and_watermark_repair_without_mutation",
    ] {
        assert!(journal.contains(proof));
    }
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/sequence_frontier.rs"))
            .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
    ] {
        assert!(
            !core.contains(absent),
            "frontier core grew policy: {absent}"
        );
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/tests/sequence_frontier.rs"));
}

#[test]
fn native_chain_recovery_delegates_order_not_policy_or_execution() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let code =
        fs::read_to_string(root.join("crates/leselang-vm/src/journal/successor.rs")).unwrap();
    let chain = code.split("pub(super) fn validate_chain(").nth(1).unwrap();
    for required in [
        "ORDER BY b.position",
        "leselang_runtime_core::inspect_successor_chain(",
        "crate::MAX_MERGE_BRANCHES",
        "previous.1.as_deref().ok_or_else(invalid)?",
        "MAX_JOURNAL_ENTRY_BYTES",
        "validate_pair(parent, &result, &next)?",
        "SuccessorChainError::Native { error, .. } => error",
        "_ => invalid()",
    ] {
        assert!(
            chain.contains(required),
            "missing native boundary: {required}"
        );
    }
    assert!(
        chain.find("let result: Step = decode_bounded(").unwrap()
            < chain
                .find("let next = request_for(connection, &following.0)?")
                .unwrap()
    );
    for absent in [
        "windows(2)",
        "Fuel::new",
        "resume(",
        "commit()",
        "advance_sequence_watermark",
    ] {
        assert!(
            !chain.contains(absent),
            "chain wrapper grew behavior: {absent}"
        );
    }
    let core = fs::read_to_string(root.join("crates/leselang-runtime-core/src/successor_chain.rs"))
        .unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "expect(",
        "unwrap(",
    ] {
        assert!(!core.contains(absent), "chain core grew policy: {absent}");
    }
    let journal = fs::read_to_string(root.join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(journal.contains("successor_chain_inspection_preserves_frozen_sqlite_fault_order"));
    let exporter = fs::read_to_string(root.join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/tests/successor_chain.rs"));
}

#[test]
fn sqlite_reservation_and_completion_fences_are_shared_without_repair_or_execution_authority() {
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let graph = code
        .split("fn validate_merge_graph_records(")
        .nth(1)
        .unwrap()
        .split("fn merge_reserved_sequence(")
        .next()
        .unwrap();
    for boundary in [
        "SequenceReservations::new(",
        "MAX_JOURNAL_RECORDS * crate::MAX_MERGE_BRANCHES",
        ".insert(sequence)",
        "let terminal = recover_group_receipt(",
        "validate_unbound_group_receipt(&plan, &branches, terminal)",
        "validate_group_receipt(state, all_completed, terminal_bytes",
        "validate_terminal_step(&terminal)?",
        "GroupReceiptError::Native(error) => error",
        "GroupReceiptError::State => invalid_state()",
        ".finish()",
        "reservations.highest_sequence",
        "group_binding::validate_records(connection, &token, &plan, terminal.as_ref())?",
    ] {
        assert!(graph.contains(boundary), "native graph lost {boundary}");
    }
    assert!(!graph.contains("advance_sequence_watermark("));
    assert!(!graph.contains("resume_group("));
    let bound =
        fs::read_to_string(root().join("crates/leselang-vm/src/journal/group_binding.rs")).unwrap();
    for boundary in [
        "validate_reservation_proof(",
        "allocation_watermark: u64::try_from(next_sequence).ok()",
        "record_exists: exists",
        "aliases_owner: aliases_group",
        "index < plan.branches.len() - binding.branches()?.len()",
    ] {
        assert!(bound.contains(boundary));
    }
    assert!(!bound.contains("advance_sequence_watermark("));
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/recovery_fence.rs"))
            .unwrap();
    for absent in [
        "leserpent",
        "rusqlite",
        "Fuel::new",
        "Serialize",
        "Deserialize",
        "impl Default",
    ] {
        assert!(
            !core.contains(absent),
            "recovery fence grew policy: {absent}"
        );
    }
    for proof in [
        "group_receipt_recovery_preserves_frozen_state_and_decode_fault_precedence",
        "cross_group_reservation_refusal_blocks_snapshot_and_imported_watermark_repair",
        "cold_slot_allocation_proof_uses_original_watermark_before_high_identity_repair",
    ] {
        assert!(
            code.contains(proof)
                || fs::read_to_string(root().join("crates/leselang-vm/tests/group_tail.rs"))
                    .unwrap()
                    .contains(proof)
        );
    }
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("consumer/tests/recovery_fences.rs")
    );
}

#[test]
fn native_unbound_receipt_correspondence_uses_original_data_merge_without_evaluation() {
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let gate = code
        .split("fn recover_group_receipt(")
        .nth(1)
        .unwrap()
        .split("fn validate_unbound_group_receipt(")
        .next()
        .unwrap();
    assert!(
        gate.find("validate_terminal_step(&terminal)?").unwrap()
            < gate.find("admit(&terminal)?").unwrap()
    );
    let correspondence = code
        .split("fn validate_unbound_group_receipt(")
        .nth(1)
        .unwrap()
        .split("fn merge_reserved_sequence(")
        .next()
        .unwrap();
    for required in [
        "if plan.result_binding.is_some()",
        "return Ok(())",
        "branches.len() != plan.branches.len()",
        "branches.len() > crate::MAX_MERGE_BRANCHES",
        "decode_bounded::<Step>(",
        "MAX_JOURNAL_ENTRY_BYTES",
        "sequence_stop(steps.iter())?",
        "terminal_step_outcome(step)?",
        "group_binding::finish(plan, completions)?",
        "if &expected != terminal",
        "merge group receipt does not match original branch results",
    ] {
        assert!(
            correspondence.contains(required),
            "receipt correspondence lost {required}"
        );
    }
    for absent in [
        "Fuel::new",
        "resume_captured_effects",
        ".finish(value)",
        "restore_request",
        "advance_sequence_watermark(",
        "execute(",
    ] {
        assert!(!correspondence.contains(absent));
    }
    let actual = code
        .split("fn validate_merge_graph_records(")
        .nth(1)
        .unwrap()
        .split("fn recover_group_receipt(")
        .next()
        .unwrap();
    assert!(
        actual.find("validate_group_graph(").unwrap()
            < actual.find("validate_unbound_group_receipt(").unwrap()
    );
    assert!(
        actual.find("validate_unbound_group_receipt(").unwrap()
            < actual.find("group_binding::validate_records(").unwrap()
    );
    assert!(code.contains(
        "group_receipt_native_admission_runs_once_after_state_decode_and_terminal_validation"
    ));
    let tests =
        fs::read_to_string(root().join("crates/leselang-vm/tests/group_receipts.rs")).unwrap();
    for proof in [
        "unbound_completed_receipt_must_match_original_members_before_watermark_repair",
        "original_field_values_names_and_declared_order_are_not_interchangeable",
        "structurally_valid_fault_cannot_replace_success_or_change_original_refusal",
        "original_cancellation_and_failure_metadata_are_part_of_receipt_correspondence",
        "parallel_failure_choice_follows_declared_order_not_completion_chronology",
    ] {
        assert!(tests.contains(proof));
    }
    assert!(
        fs::read_to_string(root().join("crates/leselang-vm/Cargo.toml"))
            .unwrap()
            .contains("name = \"group_receipts\"\nrequired-features = [\"sqlite-journal\"]")
    );
}

#[test]
fn actual_group_request_and_recovery_graphs_use_the_shared_bounded_identity_kernel() {
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let input = code
        .split("fn validate_merge_graph_input(")
        .nth(1)
        .unwrap()
        .split("fn validate_merge_graph_records(")
        .next()
        .unwrap();
    for boundary in [
        "validate_merge_plan(plan)?",
        "valid_continuation_token(group_token)",
        "validate_group_graph(",
        "validate_effect_request(request)?",
        "GroupGraphError::Native { error, .. } => error",
        "binding.validate_requests(group_token, plan, branches)?",
    ] {
        assert!(input.contains(boundary), "group admission lost {boundary}");
    }
    assert!(!input.contains("HashSet"));
    let records = code
        .split("fn validate_merge_graph_records(")
        .nth(1)
        .unwrap()
        .split("fn merge_reserved_sequence(")
        .next()
        .unwrap();
    for boundary in [
        "validate_group_graph(",
        "usize::try_from(branch.1).ok()",
        "identity: branch.4.as_str()",
        "SequenceReservations::new(",
        ".insert(sequence)",
        "successor::validate_chain(connection, &token)?",
        "sequence_stop(terminal_steps.iter())?",
        "validate_terminal_step(&terminal)?",
        "group_binding::validate_records(connection, &token, &plan, terminal.as_ref())?",
    ] {
        assert!(
            records.contains(boundary),
            "native recovery lost {boundary}"
        );
    }
    for proof in [
        "group_graph_admission_preserves_frozen_fault_precedence",
        "late_group_graph_refusal_rolls_back_recovery_without_repairing_watermark",
    ] {
        assert!(code.contains(proof));
    }
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/group_graph.rs")).unwrap();
    for absent in [
        "leserpent",
        "rusqlite",
        "EffectRequest",
        "Fuel::new",
        "Serialize",
        "Deserialize",
    ] {
        assert!(
            !core.contains(absent),
            "graph kernel grew native policy: {absent}"
        );
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/tests/group_graph.rs"));
}

#[test]
fn actual_atomic_and_group_saved_bodies_use_the_shared_hir_entry_without_body_replacement() {
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/computation.rs")).unwrap();
    let reentry = code
        .split("fn resume_captured_outcome")
        .nth(1)
        .unwrap()
        .split("fn charge_projection")
        .next()
        .unwrap();
    for boundary in [
        "resume_captured_effects_in_scope(",
        "CapturedEffectHooks {",
        "charge(fuel, 1 + string_cost(value))?",
        "charge_projection(fuel, result)?",
        "charge_group_projection(fuel, group)?",
        "if !restore_saved_results",
        "effect_outcome(outcome.into_legacy())",
        "calculation_fault(effect_failure(error))",
    ] {
        assert!(
            reentry.contains(boundary),
            "saved execution lost {boundary}"
        );
    }
    assert!(!reentry.contains("body:"));
    assert!(!reentry.contains("evaluate_inner("));
    let shared =
        fs::read_to_string(root().join("crates/leselang-hir/src/effect_reentry.rs")).unwrap();
    for boundary in [
        "with_captured_scope_state(",
        "evaluate_resumable_effects_in_scope(body, scope, environment, fuel, limits)",
        "original Fuel",
        "after restoration and before body execution",
        "not saved-frame admission or reply acceptance",
    ] {
        assert!(shared.contains(boundary));
    }
    let consumer =
        fs::read_to_string(root().join("crates/leselang-hir/tests/durable_control.rs")).unwrap();
    assert!(consumer.contains("let outcome = resume_captured_effects_in_scope("));
    assert!(!consumer.contains("with_captured_scope("));
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/tests/captured_reentry.rs"));
}

#[test]
fn saved_frame_is_shared_data_while_reference_restore_and_authority_remain_native() {
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/lib.rs")).unwrap();
    for boundary in [
        "pub type ContinuationImage = leselang_runtime_core::ContinuationFrame<",
        "pub use leselang_runtime_core::ResourceBudget;",
        "image\n        .resource_budget()\n        .validate(ResourceBudgetLimits",
        "ResourceBudgetError::Fuel =>",
        "ResourceBudgetError::AbsoluteDeadline =>",
        "request.budget != request.continuation.resource_budget()",
    ] {
        assert!(code.contains(boundary), "reference frame lost {boundary}");
    }
    for removed in ["pub struct ContinuationImage", "pub struct ResourceBudget"] {
        assert!(!code.contains(removed));
    }
    let core = fs::read_to_string(root().join("crates/leselang-runtime-core/src/continuation.rs"))
        .unwrap();
    for absent in [
        "leserpent",
        "leselang_hir",
        "rusqlite",
        "Fuel::new",
        "impl Default",
    ] {
        assert!(!core.contains(absent), "saved data grew policy: {absent}");
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    for proof in [
        "crates/leselang-runtime-core/tests/continuation.rs",
        "crates/leselang-hir/tests/durable_control.rs",
        "consumer/tests/continuation_frame.rs",
        "consumer/tests/durable_control.rs",
    ] {
        assert!(exporter.contains(proof));
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    for boundary in [
        "Shared Saved Frames And Budgets",
        "mutable unchecked data",
        "consumer-owned",
        "crash-atomic",
        "complete independent",
    ] {
        assert!(docs.contains(boundary));
    }
}

#[test]
fn actual_sqlite_recovery_uses_host_neutral_staging_without_moving_frame_or_authority_policy() {
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let load = code
        .split("fn load(&mut self)")
        .nth(1)
        .unwrap()
        .split("fn completed_exists")
        .next()
        .unwrap();
    for boundary in [
        "CheckpointRecovery::new(MAX_JOURNAL_RECORDS, crate::MAX_EFFECT_SEQUENCE)",
        "TransactionBehavior::Immediate",
        "validate_image(&image)?",
        "stored_deadline != image.deadline_at_ms",
        "group_binding::validate_owner(&transaction, &image)?",
        "successor::validate_record(&transaction, &image",
        "push(RecoveryRecord",
        ".finish(Some(transaction)",
        "validate_dispatch_records(connection)?",
        "validate_merge_graph_records(connection)?",
        "validate_debugger_audit_records(connection)?",
        "advance_sequence_watermark(connection, highest_sequence)?",
        "transaction.commit()",
        "snapshot.into_parts()",
    ] {
        assert!(load.contains(boundary), "actual recovery lost {boundary}");
    }
    assert!(!load.contains("let mut pending = BTreeMap::new()"));
    assert!(code.contains("recovery_commit_failure_releases_staging_and_rolls_back_the_watermark"));
    assert!(code.contains("recovery_rejects_a_late_invalid_row_without_exposing_the_valid_prefix"));
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/checkpoint.rs")).unwrap();
    for absent in [
        "rusqlite",
        "leserpent",
        "ContinuationImage",
        "EffectRequest",
        "Serialize",
        "Deserialize",
    ] {
        assert!(!core.contains(absent), "checkpoint core retained {absent}");
    }
    assert!(
        fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs"))
            .unwrap()
            .contains("crates/leselang-runtime-core/tests/checkpoint.rs")
    );
}

fn tree(features: &[&str]) -> String {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--locked",
            "--offline",
            "-p",
            "leselang-vm",
            "--edges",
            "normal,build",
            "--prefix",
            "none",
        ])
        .args(features)
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(root())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn memory_vm_dependency_closure_omits_native_sqlite_without_claiming_product_independence() {
    let dependencies = tree(&["--no-default-features"]);
    for absent in ["rusqlite", "libsqlite3-sys"] {
        assert!(
            !dependencies.lines().any(|l| l.starts_with(absent)),
            "memory VM retained {absent}"
        );
    }
    for retained in [
        "leselang-hir",
        "leselang-runtime-core",
        "leselang-command",
        "leserpent-domain",
        "leselang-host-contract",
    ] {
        assert!(
            dependencies.lines().any(|l| l.starts_with(retained)),
            "reference VM boundary changed: {retained}"
        );
    }
}

#[test]
fn default_and_explicit_sqlite_features_keep_the_durable_backend_and_released_api() {
    for flags in [
        &[][..],
        &["--no-default-features", "--features", "sqlite-journal"][..],
    ] {
        let dependencies = tree(flags);
        assert!(dependencies.contains("rusqlite v"));
        assert!(dependencies.contains("libsqlite3-sys v"));
    }
    let manifest = fs::read_to_string(root().join("crates/leselang-vm/Cargo.toml")).unwrap();
    assert!(manifest.contains("default = [\"sqlite-journal\"]"));
    assert!(manifest.contains("sqlite-journal = [\"dep:rusqlite\"]"));
    assert!(manifest.contains("rusqlite = { workspace = true, optional = true }"));
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/lib.rs")).unwrap();
    assert!(code.contains("journal: Journal::ephemeral()"));
    for method in ["open_journal(", "open_journal_with_limits("] {
        assert!(code.contains(&format!(
            "#[cfg(feature = \"sqlite-journal\")]\n    pub fn {method}"
        )));
    }
    assert!(code.contains("must-not-fall-back.sqlite3"));
    assert!(code.contains("let (journal, snapshot) = Journal::open(path.as_ref())?"));
}

#[test]
fn only_database_dependent_integration_suites_require_sqlite_and_memory_verticals_stay_enabled() {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--locked",
            "--offline",
            "--format-version",
            "1",
            "--no-deps",
        ])
        .current_dir(root())
        .output()
        .unwrap();
    assert!(output.status.success());
    let metadata: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let vm = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "leselang-vm")
        .unwrap();
    let mut durable = 0;
    let mut memory = 0;
    for target in vm["targets"].as_array().unwrap() {
        if target["kind"] != serde_json::json!(["test"]) {
            continue;
        }
        let source = fs::read_to_string(target["src_path"].as_str().unwrap()).unwrap();
        if source.contains("open_journal") || source.contains("rusqlite") {
            assert_eq!(
                target["required-features"],
                serde_json::json!(["sqlite-journal"])
            );
            durable += 1;
        } else {
            assert!(
                target["required-features"].is_null()
                    || target["required-features"] == serde_json::json!([])
            );
            memory += 1;
        }
    }
    assert_eq!(durable, 36);
    assert_eq!(memory, 15);
    let proof =
        fs::read_to_string(root().join("crates/leselang-vm/tests/ephemeral_profile.rs")).unwrap();
    assert!(!proof.contains("#[cfg("));
    assert!(!proof.contains("std::fs"));
    for name in [
        "memory_profile_executes_typed_helpers_and_preserves_legacy_scalar_wire",
        "memory_profile_seq_keeps_reply_gates_one_budget_and_once_only_completion",
        "memory_profile_all_reenters_captured_successor_only_after_complete_original_barrier",
        "memory_profile_fences_forged_images_wrong_reply_types_and_independent_local_roots",
        "memory_profile_cancel_or_deadline_seals_captured_tail_and_replays_terminal_receipt",
        "memory_profile_retry_backpressure_and_compaction_keep_original_dispatch_authority",
    ] {
        assert!(proof.contains(name));
    }
}

#[test]
fn ci_and_documentation_distinguish_database_free_reference_vm_from_independent_language_vm() {
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("cargo +1.98.0 test -p leselang-vm --locked --no-default-features"));
    assert!(workflow.contains("cargo +1.98.0 clippy -p leselang-vm --locked --no-default-features --all-targets -- -D warnings"));
    assert!(workflow.contains("cargo test --workspace --all-features --locked"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    let section = docs
        .split("### Optional Reference VM Storage")
        .nth(1)
        .unwrap();
    for boundary in [
        "optional VM SQLite backend",
        "default-features = false",
        "no silent memory fallback",
        "Cargo features are additive",
        "not a capability or security boundary",
        "still depends on Leserpent command/result contracts",
        "not the complete product-free independent VM",
    ] {
        assert!(
            section.contains(boundary),
            "VM storage docs lost {boundary}"
        );
    }
}
