use std::{fs, path::Path, process::Command};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
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
