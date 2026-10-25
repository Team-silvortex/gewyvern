use std::{fs, path::Path, process::Command};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn tree(features: &[&str]) -> String {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--locked",
            "--offline",
            "-p",
            "leselang-hir",
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
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn native_profile_has_only_language_workspace_dependencies_and_no_product_contract() {
    let dependency_tree = tree(&["--no-default-features"]);
    for required in ["leselang-hir", "leselang-runtime-core", "leselang-syntax"] {
        assert!(
            dependency_tree
                .lines()
                .any(|line| line.starts_with(required))
        );
    }
    for forbidden in [
        "leselang-host-contract",
        "silvortex-identity",
        "leserpent",
        "leselang-vm",
        "rusqlite",
        "gewyvern",
    ] {
        assert!(
            !dependency_tree
                .lines()
                .any(|line| line.starts_with(forbidden)),
            "native dependency closure gained {forbidden}"
        );
    }
}

#[test]
fn native_source_export_is_a_rust_workflow_with_unchanged_code_and_isolated_consumer_proofs() {
    let workflow = fs::read_to_string(root().join("crates/gewyvern-dev/src/main.rs")).unwrap();
    assert!(workflow.contains("cargo dev package leselang [--output DIR] [--verify] [--dry-run]"));
    assert!(workflow.contains("leselang::export(&root, &options)?"));
    let source = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    for retained in [
        "fn module_names(",
        "fn crate_manifest(",
        "fn registry_lock(",
        "fn check_inventory(",
        "fn publish(",
        "read_regular(root, source)?",
        "publish = false",
        "source symlink refused",
        "export destination already exists",
        "--all-features",
        "--all-targets",
        "--locked",
        "--offline",
        "--no-deps",
        "source export",
        "leselang-native-source-consumer",
        "complete_vm\": false",
        "full_extraction_gate\": false",
        "dry_run_plans_real_sources_without_files_or_verifier_calls",
        "failed_isolated_verification_leaves_no_published_directory_or_private_stage",
        "changed_production_bytes_after_verification_are_not_sealed_or_published",
        "unplanned_verifier_files_cannot_enter_a_verified_source_inventory",
        "publication_serializes_exporters_and_never_replaces_preexisting_destination",
    ] {
        assert!(
            source.contains(retained),
            "native source export lost {retained}"
        );
    }
    for forbidden in [
        "Command::new(\"python",
        "Command::new(\"node",
        "Command::new(\"sh",
        "Command::new(\"bash",
        "cargo clean",
        "git commit",
        "git push",
    ] {
        assert!(!source.contains(forbidden));
    }
    let ci = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(
        ci.contains("package leselang --output \"$RUNNER_TEMP/leselang-native-source\" --verify")
    );
    assert!(ci.contains("cargo test --workspace --all-features --locked"));
    assert!(ci.contains("cargo +1.98.0 test -p leselang-vm --locked --no-default-features"));
}

#[test]
fn source_verification_owns_disposable_cache_without_monorepo_build_artifacts() {
    let source = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    for required in [
        "struct ExportDirectory",
        "impl Drop for ExportDirectory",
        "std::env::temp_dir().join(",
        "verifier(&stage.path, &target.path)",
        "finish_export_cleanup(result, target.cleanup())?",
        "finish_export_cleanup(result, stage.cleanup())?",
        "verified_exports_use_fresh_private_targets_and_remove_them_before_publication",
        "overlapping_exports_never_share_or_remove_each_others_verification_cache",
        "verifier_unwind_cleans_both_owned_directories_without_publishing",
        "failed_cache_cleanup_preserves_verification_error_and_blocks_publication",
        "owned_cache_cleanup_does_not_follow_links_into_unrelated_directories",
    ] {
        assert!(source.contains(required), "missing {required}");
    }
    let export = source
        .split("fn export_with_verifier(")
        .nth(1)
        .unwrap()
        .split("fn unique_suffix(")
        .next()
        .unwrap();
    assert!(!export.contains("root.join(\"target\")"));
    let cleanup = export
        .find("finish_export_cleanup(result, target.cleanup())?")
        .unwrap();
    assert!(cleanup < export.find("check_inventory(&stage.path, &plan)?").unwrap());
    assert!(cleanup < export.find("publish(&stage.path, &output)?").unwrap());
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("Overlapping exports have distinct targets"));
    assert!(docs.contains("Abrupt process termination can still leave temporary directories"));
}

#[test]
fn default_and_explicit_profiles_retain_the_existing_host_contract() {
    for flags in [
        &[][..],
        &["--no-default-features", "--features", "leserpent-profile"][..],
    ] {
        let dependency_tree = tree(flags);
        assert!(dependency_tree.contains("leselang-host-contract v"));
        assert!(dependency_tree.contains("silvortex-identity v"));
    }
}

#[test]
fn native_vm_source_checkpoint_keeps_direct_consumer_proof_and_closed_reference_projection() {
    let source = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    for required in [
        "fn native_vm_manifest(",
        "reference_tests.contains(test_name)",
        "native_vm_facade\": true",
        "consumer/tests/native_vm_embedding.rs",
        "vm_manifest_projection_preserves_native_dependencies_and_omits_every_reference_target",
        "vm_manifest_projection_refuses_unreviewed_features_dependencies_sections_and_targets",
        "crates/leselang-vm/src/leserpent_profile.rs",
        "crates/leselang-vm/src/journal.rs",
    ] {
        assert!(source.contains(required), "VM export lost {required}");
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    let section = docs
        .split("### Native Source Checkpoint")
        .nth(1)
        .unwrap()
        .split("### Remaining Product Boundaries")
        .next()
        .unwrap()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for required in [
        "native VM facade source",
        "closed reviewed shape",
        "no enableable reference or SQLite feature",
        "native_vm_facade = true",
        "complete_vm = false",
        "full_extraction_gate = false",
        "separate consumer's direct VM dependency",
    ] {
        assert!(section.contains(required), "VM export docs lost {required}");
    }
}

#[test]
fn continuous_host_pipeline_is_exported_without_promoting_the_complete_vm_gate() {
    let proof = "crates/leselang-vm/tests/native_host_pipeline.rs";
    let code = fs::read_to_string(root().join(proof)).unwrap();
    assert!(code.contains("use leselang_vm::native::{"));
    assert!(code.contains("compile_catalog_program("));
    assert!(code.contains("LocalVm::new("));
    for required in [
        "impl Profile for Counter",
        "impl Profile for Panel",
        "pipeline_matrix_covers_atomic_control_nodes_without_claiming_group_or_opaque_host_coverage",
        "both_hosts_preserve_one_budget_at_every_pipeline_debit_without_hidden_refills",
        "both_hosts_share_one_compiled_ir_without_cross_acceptance_or_sibling_cancellation",
        "both_hosts_recheck_authority_at_dispatch_acceptance_and_captured_reentry",
        "two_hosts_mix_atomic_seq_all_repeat_and_captured_successors_through_one_vm",
        "mixed_group_wrong_site_order_count_and_stale_root_receipts_preserve_retryable_ownership",
        "mixed_partial_and_complete_groups_cannot_escape_parent_cancellation_or_start_a_tail",
        "mixed_accepted_group_reentry_rechecks_grants_and_revision_before_the_atomic_successor",
        "mixed_preparation_correlation_assembly_and_successors_share_exactly_one_fuel_budget",
        "mixed_shared_ir_rejects_foreign_group_members_even_with_a_matching_root_label",
        "mixed_invalid_late_group_and_cold_policy_are_rejected_before_the_initial_effect",
        "two_hosts_select_group_helpers_alias_results_and_take_early_or_captured_returns",
        "computed_result_fields_join_two_host_group_reply_and_successor_pipelines",
        "computed_result_fields_keep_the_original_meter_at_every_pipeline_cutoff",
        "captured_group_iterations_join_result_selection_and_successor_operands_for_two_hosts",
        "captured_group_iterations_keep_the_original_meter_at_every_pipeline_cutoff",
        "captured_group_iteration_limits_cannot_be_recovered_into_a_successor",
        "captured_group_iterations_reject_cold_exports_effects_and_escaped_locals",
        "selected_result_control_flow_survives_coupled_parent_and_collector_replacement",
        "selected_result_replacement_preserves_failure_prefixes_at_every_fuel_cutoff",
        "selected_result_restore_refusals_block_tail_work_at_every_parent_and_child_cut",
        "selected_group_and_early_exit_keep_exact_fuel_at_every_pipeline_cutoff",
        "unselected_group_helpers_are_typed_but_never_evaluated_or_dispatched",
        "selected_group_reentry_cannot_bypass_revocation_even_when_returning_early",
        "both_hosts_join_atomic_group_and_pure_returns_for_all_six_data_types",
        "selected_data_return_and_caller_capture_share_every_fuel_boundary",
        "selected_data_arguments_remain_lazy_but_selected_unused_arguments_still_run",
        "cold_selected_data_types_names_and_pure_arguments_fail_before_dispatch",
        "selected_data_receipt_reentry_cannot_cross_revocation_or_cancellation",
        "selected_data_captures_distinguish_absent_empty_and_maximum_bounded_payloads",
        "selected_data_payload_overflow_cannot_dispatch_the_caller_tail",
        "compile_catalog_prepared_group_helper_program(",
        "prepare_atomic_group_in_scope(",
        "assemble_group_reply(",
    ] {
        assert!(code.contains(required), "pipeline proof lost {required}");
    }
    for forbidden in ["leserpent_domain", "leselang_command", "rusqlite", "#[cfg("] {
        assert!(!code.contains(forbidden));
    }
    let export = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(export.contains(proof));
    assert!(export.contains("consumer/tests/native_host_pipeline.rs"));
    assert!(export.contains("\"complete_vm\": false"));
    assert!(export.contains("\"full_extraction_gate\": false"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    let section = docs
        .split("### Native Host Pipeline Matrix")
        .nth(1)
        .unwrap()
        .split("### Optional Reference VM Storage")
        .next()
        .unwrap();
    assert!(section.contains("not the full language gate"));
    assert!(section.contains("mixed multi-group/atomic programs"));
    assert!(section.contains("twelve"));
    assert!(section.contains("not a new generic scheduler"));
    assert!(section.contains("sixteen conditional host/program paths"));
    assert!(section.contains("field(value: member("));
    assert!(section.contains("72 host/program/data combinations"));
    assert!(section.contains("Another 60"));
    assert!(section.contains("not new\nlanguage syntax"));
}

#[test]
fn computed_result_projection_reuses_shared_compilation_and_pure_budget_kernels() {
    let compiler =
        fs::read_to_string(root().join("crates/leselang-hir/src/catalog_source.rs")).unwrap();
    for required in [
        "fn projection<",
        "lower_catalog_projection_source(",
        "lower_preflighted_control_source(child, prefix, limits, self)",
        "preflight_names(source)",
        "form.lower_preflighted(",
        "self.projection(child, &extended)",
        "CatalogSourceError::ComputedProjection",
    ] {
        assert!(
            compiler.contains(required),
            "computed projection lost {required}"
        );
    }
    let projection =
        fs::read_to_string(root().join("crates/leselang-hir/src/projection_source.rs")).unwrap();
    assert!(projection.contains("preflight_with_budget(&value, 1, budget)"));
    assert_eq!(
        projection
            .matches("pub(crate) fn lower_preflighted<")
            .count(),
        1
    );
    let proof =
        fs::read_to_string(root().join("crates/leselang-hir/tests/catalog_source.rs")).unwrap();
    for required in [
        "computed_fields_preserve_result_and_scalar_lexical_scopes",
        "computed_fields_reject_cold_effects_types_and_metadata_before_outer_export",
        "computed_field_root_and_cold_children_share_inclusive_output_limits",
        "computed_field_native_denial_errors_and_unwind_are_not_reinterpreted",
    ] {
        assert!(proof.contains(required));
    }
}

#[test]
fn saved_return_stack_consumer_reuses_native_reentry_without_promoting_durable_vm() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/tests/durable_control.rs")).unwrap();
    for required in [
        "restore_effect_return_stack(",
        "resume_accepted_stacked_effects(",
        "two_native_hosts_restore_serialized_helper_return_stacks_without_recompilation_or_replay",
        "restored_nested_helpers_keep_one_budget_and_settle_every_accepted_fuel_refusal",
        "saved_return_stacks_reject_forged_order_budget_reply_and_live_policy_before_restoration",
        "revoked_outer_saved_body_is_settled_without_restoring_or_preparing_inner_work",
    ] {
        assert!(
            source.contains(required),
            "saved return proof lost {required}"
        );
    }
    let export = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(export.contains("crates/leselang-hir/tests/durable_control.rs"));
    assert!(export.contains("crates/leselang-hir/tests/effect_stack.rs"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    let section = docs
        .split("### Restored Native Return Stacks")
        .nth(1)
        .unwrap()
        .split("### Shared Flat Group Graph Inspection")
        .next()
        .unwrap();
    for required in [
        "three pending return frames",
        "without recompilation",
        "not authenticate",
        "not a production durable backend",
    ] {
        assert!(section.contains(required));
    }
}

#[test]
fn restored_parallel_consumer_uses_the_existing_barrier_without_promoting_durable_graphs() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/src/prepared_parallel.rs")).unwrap();
    assert!(source.contains("pub fn restore<Error>"));
    assert!(source.contains("ParallelReplyCore::restore(branches, rows, admit, scope)"));
    let restore = source
        .split("pub fn restore<Error>")
        .nth(1)
        .unwrap()
        .split("/// Consume all")
        .next()
        .unwrap();
    for forbidden in [
        "Fuel::new",
        "fuel.charge",
        "prepare_effect",
        "enum State",
        "serde",
        "dispatch(",
    ] {
        assert!(!restore.contains(forbidden));
    }
    let proof =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_parallel.rs")).unwrap();
    for required in [
        "every_partial_mask_restores_the_original_barrier_and_order_without_a_new_meter",
        "duplicate_saved_identity_including_accepted_rows_refuses_before_host_admission",
        "whole_saved_admission_rechecks_accepted_authority_and_unwind_publishes_nothing",
        "restored_cancel_scope_and_acceptance_unwind_keep_existing_terminal_fences",
        "two_serialized_host_models_restore_every_partial_mask_without_repreparing_or_redispatching",
        "serialized_graph_budget_receipt_and_live_refusals_leave_committed_rows_unchanged",
    ] {
        assert!(proof.contains(required));
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("crates/leselang-hir/tests/saved_parallel.rs"));
    assert!(exporter.contains("consumer/tests/saved_parallel.rs"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    let section = docs
        .split("### Restored Parallel Reply Barriers")
        .nth(1)
        .unwrap()
        .split("### Shared Flat Group Graph Inspection")
        .next()
        .unwrap();
    for boundary in [
        "same private `ParallelReplyCore`",
        "all eight completion masks",
        "not process-crash acceptance",
        "cancelled or settled work must never be rearmed",
        "Complete mixed-group continuation recovery",
    ] {
        assert!(section.contains(boundary));
    }
}

#[test]
fn saved_sequence_consumer_derives_frontiers_through_the_original_reply_core() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/src/prepared_sequence.rs")).unwrap();
    assert!(source.contains("SequenceReplyCore::restore(branches, rows, admit, scope)"));
    let restore = source
        .split("pub fn restore<Reply, Error>")
        .nth(1)
        .unwrap()
        .split("pub fn start<")
        .next()
        .unwrap();
    for forbidden in [
        "Fuel::new",
        "fuel.charge",
        "enum State",
        "prepare_effect",
        "dispatch(",
        "serde",
    ] {
        assert!(!restore.contains(forbidden));
    }
    let core =
        fs::read_to_string(root().join("crates/leselang-hir/src/sequence_evaluation.rs")).unwrap();
    assert_eq!(core.matches("enum State<").count(), 1);
    assert!(
        core.contains(".take_while(|row| matches!(row.reply, SavedSequenceReply::Accepted(_)))")
    );
    assert!(core.contains("SequenceRestoreError::Frontier"));
    let proof =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_sequence.rs")).unwrap();
    for name in [
        "all_seven_saved_frontiers_resume_without_republishing_or_copying_the_prefix",
        "all_twenty_noncontiguous_frontiers_refuse_before_identity_or_native_admission",
        "duplicate_prefix_pending_and_queued_identities_never_reach_snapshot_admission",
        "restored_publication_and_acceptance_reuse_live_policy_and_terminal_cleanup",
        "two_serialized_models_resume_every_frontier_without_repreparation_or_duplicate_publication",
    ] {
        assert!(proof.contains(name));
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("crates/leselang-hir/tests/saved_sequence.rs"));
    assert!(exporter.contains("consumer/tests/saved_sequence.rs"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    let section = docs
        .split("### Restored Sequential Frontiers")
        .nth(1)
        .unwrap()
        .split("### Shared Flat Group Graph Inspection")
        .next()
        .unwrap();
    for boundary in [
        "derives the cursor",
        "all seven valid",
        "all twenty invalid",
        "not process-crash acceptance",
        "Complete mixed-group/helper",
        "extraction\ncompletion claim",
    ] {
        assert!(section.contains(boundary));
    }
}

#[test]
fn saved_mixed_consumer_composes_existing_walkers_without_claiming_portable_recovery() {
    let source =
        fs::read_to_string(root().join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    let saved = source
        .split("mod saved_mixed {")
        .nth(1)
        .unwrap()
        .split("trait MixedProfile:")
        .next()
        .unwrap();
    for required in [
        "Queued::restore(",
        "Parallel::restore(",
        "restore_effect_return_stack(",
        "resume_accepted_stacked_effects(",
        "queue.try_into_saved(accepted)",
        "barrier.try_into_saved()",
        "every_seq_all_frontier_composes_with_native_helper_return_stacks",
        "restored_mixed_helpers_keep_exact_original_fuel_at_every_cutoff",
        "mixed_snapshot_admission_rejects_stale_revoked_and_closed_hosts_before_tail_work",
        "saved_sequence_handle_correspondence_cannot_redirect_unpublished_mixed_work",
    ] {
        assert!(
            saved.contains(required),
            "saved mixed proof lost {required}"
        );
    }
    assert_eq!(saved.matches("Fuel::new(").count(), 1);
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    let section = docs
        .split("### Composed Native Mixed Recovery")
        .nth(1)
        .unwrap()
        .split("### Shared Flat Group Graph Inspection")
        .next()
        .unwrap();
    for boundary in [
        "252 two-host paths",
        "at least three return frames",
        "exactly six dispatches",
        "eight original identities",
        "in-memory composition proof",
        "not serialized mixed-graph recovery",
        "Full extraction remains open",
    ] {
        assert!(
            section.contains(boundary),
            "mixed recovery docs lost {boundary}"
        );
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("252 two-host frontier combinations"));
}

#[test]
fn saved_call_operands_restore_without_an_evaluator_or_native_inventory_dependency() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/src/call_evaluation.rs")).unwrap();
    let restore = source
        .split("pub fn restore<Error>")
        .nth(1)
        .unwrap()
        .split("pub enum PreparedCallRestoreError")
        .next()
        .unwrap();
    for required in [
        "preflight_signature",
        "valid_argument_name",
        "row.value.is_bounded()",
        "check_argument_type",
        "admit(schema, &arguments)",
    ] {
        assert!(restore.contains(required));
    }
    assert!(
        restore.find("row.value.is_bounded()").unwrap()
            < restore.find("admit(schema, &arguments)").unwrap()
    );
    assert!(
        restore.find("admit(schema, &arguments)").unwrap()
            < restore.find("check_argument_type(").unwrap()
    );
    for forbidden in [
        "Fuel::new",
        "fuel.charge",
        "evaluate_",
        "prepare_",
        "dispatch(",
        "serde",
        ".clone()",
    ] {
        assert!(
            !restore.contains(forbidden),
            "saved operands gained {forbidden}"
        );
    }
    let proof = fs::read_to_string(root().join("crates/leselang-hir/tests/saved_call.rs")).unwrap();
    for required in [
        "restored_operands_move_buffers_and_rebind_names_after_decoded_storage_is_dropped",
        "malformed_complete_rows_and_late_scalar_overflow_precede_all_native_callbacks",
        "two_byte_codec_hosts_restore_real_prepared_calls_after_all_original_objects_are_dropped",
        "decoded_calls_still_need_current_authority_at_restore_and_queued_publication",
        "forged_saved_operands_and_fuel_do_not_enter_native_restoration_or_dispatch",
    ] {
        assert!(proof.contains(required));
    }
    let export = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(export.contains("consumer/tests/saved_call.rs"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    let section = docs
        .split("### Restored Native Call Operands")
        .nth(1)
        .unwrap()
        .split("### Shared Flat Group Graph Inspection")
        .next()
        .unwrap();
    for boundary in [
        "original submission positions",
        "Names rebind",
        "All seven three-member",
        "exactly three calls",
        "not a production durable backend",
        "no runtime wire format",
    ] {
        assert!(section.contains(boundary));
    }
}

#[test]
fn reference_implementation_is_optional_without_changing_released_root_paths() {
    let manifest = fs::read_to_string(root().join("crates/leselang-hir/Cargo.toml")).unwrap();
    assert!(manifest.contains("default = [\"leserpent-profile\"]"));
    assert!(manifest.contains("leserpent-profile = [\"dep:leselang-host-contract\"]"));
    assert!(manifest.contains("path = \"../leselang-host-contract\", optional = true"));
    let modules = fs::read_to_string(root().join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(
        modules.contains(
            "#[cfg(feature = \"leserpent-profile\")]\ninclude!(\"leserpent_profile.rs\");"
        )
    );
    for module in [
        "catalog_program",
        "catalog_group_source",
        "catalog_source",
        "effect_session",
        "effect_stack",
        "effect_stack_session",
        "native_group_preparation",
        "prepared_parallel",
        "prepared_sequence",
        "prepared_call_evaluation",
        "parallel_evaluation",
        "group_reply",
        "ir",
        "projection_access",
    ] {
        assert!(modules.contains(&format!("pub mod {module};")));
    }
    let profile =
        fs::read_to_string(root().join("crates/leselang-hir/src/leserpent_profile.rs")).unwrap();
    for released in [
        "pub mod computation;",
        "pub mod host_call;",
        "pub mod result_field;",
        "pub enum Effect",
        "pub fn lower(",
        "pub fn authorize(",
        "pub fn canonical_source(",
    ] {
        assert!(
            profile.contains(released),
            "missing released path {released}"
        );
    }
}

#[test]
fn assembled_group_restore_reuses_cold_checks_without_a_second_meter_or_execution_path() {
    let source = fs::read_to_string(root().join("crates/leselang-hir/src/group_reply.rs")).unwrap();
    assert_eq!(
        source
            .matches("check_group_reply(group, &accepted, limits)?")
            .count(),
        2
    );
    let restore = source
        .split("pub fn restore_group_reply<")
        .nth(1)
        .unwrap()
        .split("fn check_group_reply<")
        .next()
        .unwrap();
    assert!(restore.contains("admit(&reply).map_err(GroupReplyError::Native)?"));
    for forbidden in [
        "Fuel::new",
        "fuel.charge",
        "evaluate_",
        "dispatch(",
        "Identity: PartialEq",
    ] {
        assert!(!restore.contains(forbidden));
    }
    let proof =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_group.rs")).unwrap();
    for case in [
        "restored_groups_move_unsized_domain_rows_without_charging_assembly_again",
        "complete_cold_membership_checks_precede_restore_admission",
        "maximum_arity_and_all_shape_limits_match_live_assembly",
        "accepted_receipts_are_not_saved_assembly_or_fresh_native_authority",
        "restored_lookup_rechecks_live_policy_after_whole_group_admission",
        "owned_native_error_and_callback_cleanup_survive_refusal_cold_failure_and_unwind",
        "two_byte_codec_hosts_restore_assembled_seq_and_all_after_old_objects_are_dropped",
        "forged_saved_assembly_graph_payload_or_fuel_never_reaches_rebinding",
    ] {
        assert!(proof.contains(case));
    }
    let export = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(export.contains("consumer/tests/saved_group.rs"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Restored Assembled Group Values"));
    assert!(docs.contains("authenticate that assembly\nwas already committed"));
}

#[test]
fn byte_capture_composition_uses_existing_snapshot_group_stack_and_reply_entries() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_group.rs")).unwrap();
    let composition = source.split("mod captured {").nth(1).unwrap();
    for entry in [
        "snapshot_captured_scope_state",
        "validate_captured_bindings",
        "restore_effect_return_stack",
        "restore_group_reply",
        "PendingReply::new",
        "evaluate_stacked_effects_in_scope",
        "resume_accepted_stacked_effects",
        "Rc::ptr_eq",
        "candidate != committed",
    ] {
        assert!(composition.contains(entry));
    }
    for case in [
        "byte_captured_scalar_atom_group_alias_and_two_returns_use_the_original_walker",
        "captured_byte_reentry_matches_every_original_fuel_cutoff_without_refill",
        "denied_outer_cold_effect_blocks_all_decoded_capture_materialization",
        "saved_capture_authority_and_cached_group_policy_are_rechecked_per_return",
        "forged_capture_alias_scalar_frame_order_body_and_budget_fail_before_reentry",
        "restored_pending_rejects_wrong_identity_without_losing_input_or_replaying_captures",
    ] {
        assert!(composition.contains(case));
    }
    for forbidden in ["leserpent_domain", "rusqlite", "leselang_vm::Vm"] {
        assert!(!composition.contains(forbidden));
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Byte-Backed Captured Re-entry Composition"));
    assert!(docs.contains("fixed typed-IR capture model"));
    assert!(docs.contains(
        "Full\nportable mixed-checkpoint, production process-crash and extraction gates stay open"
    ));
}

#[test]
fn captured_inflight_sequence_reuses_prepared_calls_and_the_original_reentry_kernel() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_group.rs")).unwrap();
    let model = source.split("mod inflight {").nth(1).unwrap();
    for required in [
        "prepare_native_group_members",
        "evaluate_call_arguments_in_scope",
        "PreparedCall::restore",
        "Queue::restore",
        "restore_effect_return_stack",
        "resume_accepted_stacked_effects",
        "candidate != committed",
        "captured_seq_all_seven_frontiers_rebuild_operands_not_old_requests",
        "captured_seq_assembly_and_return_fuel_match_every_cutoff_at_every_frontier",
        "captured_seq_revocation_blocks_restore_or_publication_and_cold_returns",
        "captured_seq_forged_operands_frontier_captures_graph_and_budget_are_not_committed",
    ] {
        assert!(model.contains(required), "missing {required}");
    }
    for forbidden in ["leselang_vm::Vm", "leserpent_domain", "rusqlite"] {
        assert!(!model.contains(forbidden));
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Captured In-Flight Sequence Recovery"));
    assert!(docs.contains("28 combinations"));
    assert!(docs.contains("process-crash settlement and full extraction remain open"));
}

#[test]
fn captured_partial_parallel_reuses_the_barrier_and_shared_assembly_return_path() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_group.rs")).unwrap();
    let model = source.split("mod parallel {").nth(1).unwrap();
    for required in [
        "Barrier::restore",
        "assemble_and_resume",
        "restore_effect_return_stack",
        "LocalExecutionScope::new",
        "poll_in_scope",
        "mask.count_ones()",
        "captured_all_eight_masks_and_six_orders_restore_without_requests_or_replay",
        "captured_all_every_assembly_return_fuel_cutoff_matches_live_at_every_mask_and_order",
        "captured_all_authority_domain_and_cold_return_refusals_do_not_restore_captures_early",
        "captured_all_cancelled_partial_or_ready_barriers_drop_results_without_parent_reentry",
        "captured_all_forged_completion_graph_captures_and_budget_never_rearm_a_batch",
    ] {
        assert!(model.contains(required), "missing {required}");
    }
    assert!(source.contains("Barrier::start"));
    assert!(source.contains("fn assemble_and_resume"));
    for forbidden in ["leselang_vm::Vm", "leserpent_domain", "rusqlite"] {
        assert!(!model.contains(forbidden));
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Captured Partial Parallel Recovery"));
    assert!(docs.contains("192 combinations"));
    assert!(docs.contains("process-crash settlement and full extraction remain open"));
}

#[test]
fn source_compiled_capture_reuses_the_compiler_and_existing_recovery_adapters() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_group.rs")).unwrap();
    let model = source
        .split("mod source {")
        .nth(1)
        .unwrap()
        .split("mod whole_program {")
        .next()
        .unwrap();
    for required in [
        "leselang_syntax::parse",
        "compile_catalog_prepared_group_source",
        "OperationCatalog::new",
        "initial_with_tree",
        "parallel::restored",
        "super::restored",
        "source_compiled_seq_and_all_restore_bytes_through_the_original_walker",
        "source_compiled_recovery_matches_every_assembly_and_return_fuel_cutoff",
        "source_compiled_cold_and_current_authority_checks_still_guard_return_frames",
        "source_compiler_refuses_bad_names_types_and_cold_capabilities_before_execution",
    ] {
        assert!(model.contains(required), "missing {required}");
    }
    for forbidden in [
        "leselang_vm::Vm",
        "leserpent_domain",
        "rusqlite",
        "Node::Bind",
        "Node::Group",
    ] {
        assert!(!model.contains(forbidden));
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Source-Compiled Captured Recovery"));
    assert!(docs.contains("220 combinations"));
    assert!(docs.contains("explicit-prefix source-expression checkpoint model"));
    assert!(docs.contains("Helper-origin whole-program recovery"));
}

#[test]
fn helper_origin_checkpoint_runs_source_before_reusing_the_existing_recovery_path() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_group.rs")).unwrap();
    let model = source
        .split("mod whole_program {")
        .nth(1)
        .unwrap()
        .split("mod parallel {")
        .next()
        .unwrap();
    for required in [
        "source::compile_model",
        "let mut bindings = Vec::new()",
        "evaluate_stacked_effects_in_scope",
        "resume_accepted_stacked_effects",
        "PendingReply::new",
        "collect_sequence",
        "parallel::checkpoint",
        "source::resume",
        "helper_origin_groups_create_all_captures_before_byte_checkpoint_recovery",
        "helper_origin_recovery_preserves_every_assembly_and_return_fuel_cutoff",
        "helper_origin_source_and_cold_helpers_are_checked_without_external_prefix",
        "helper_origin_checkpoint_rechecks_current_and_cold_return_authority",
        "helper_origin_forged_graph_capture_identity_and_budget_do_not_enter_recovery",
    ] {
        assert!(model.contains(required), "missing {required}");
    }
    assert!(source.contains("compile_catalog_prepared_group_helper_program"));
    for forbidden in [
        "leselang_vm::Vm",
        "leserpent_domain",
        "rusqlite",
        "complete::<",
        "Node::Bind {",
    ] {
        assert!(!model.contains(forbidden), "unexpected {forbidden}");
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Helper-Origin Program Recovery"));
    assert!(docs.contains("eight\nprogram variants and 220 frontier/order combinations"));
    assert!(docs.contains("not arbitrary source-program recovery"));
}

#[test]
fn multiple_source_captures_use_shared_snapshots_and_native_collectors() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_group.rs")).unwrap();
    let model = source
        .split("mod multiple {")
        .nth(1)
        .unwrap()
        .split("mod repeat {")
        .next()
        .unwrap();
    for required in [
        "source::compile_model_with_budget",
        "let mut bindings = Vec::new()",
        "evaluate_stacked_effects_in_scope",
        "resume_accepted_stacked_effects",
        "collect_group",
        "parallel::checkpoint",
        "restored_queue",
        "parallel::restore_barrier",
        "two_source_captures_restore_distinct_graphs_and_shared_aliases",
        "two_source_captures_match_every_pending_assembly_and_return_fuel_cutoff",
        "two_source_captures_recheck_each_group_and_both_return_authorities",
        "two_cached_groups_remain_separate_and_revalidate_policy_on_every_alias",
        "multi_capture_source_rejects_missing_aliases_and_insufficient_graph_budgets",
        "multi_capture_checkpoint_rejects_cross_graph_refs_owners_and_budget_changes",
    ] {
        assert!(model.contains(required), "missing {required}");
    }
    for forbidden in [
        "leselang_vm::Vm",
        "leserpent_domain",
        "rusqlite",
        "Node::Bind {",
    ] {
        assert!(!model.contains(forbidden), "unexpected {forbidden}");
    }
    assert!(source.contains("snapshots_for_groups"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Two-Capture Program Recovery"));
    assert!(docs.contains("440 frontier/order combinations"));
    assert!(docs.contains("not arbitrary captured-graph collections"));
}

#[test]
fn pending_transfer_uses_original_owners_and_cannot_escape_local_scope() {
    let session =
        fs::read_to_string(root().join("crates/leselang-hir/src/effect_stack_session.rs")).unwrap();
    for required in [
        "pub struct PendingStackedBinding",
        "pub fn try_into_pending_binding(",
        "core.try_into_pending_binding()",
        "Err(core) => Err(Self { core, limits })",
    ] {
        assert!(session.contains(required), "missing {required}");
    }
    let core =
        fs::read_to_string(root().join("crates/leselang-hir/src/effect_session.rs")).unwrap();
    assert!(core.contains("State::AwaitingBinding(pending) => match pending.try_into_parts()"));
    assert!(core.contains("state => Err(Self { state, fuel })"));
    let local =
        fs::read_to_string(root().join("crates/leselang-hir/src/local_effect_session.rs")).unwrap();
    assert!(!local.contains("pub fn try_into_pending_binding"));
    assert!(local.contains("let pending = session.try_into_pending_binding();"));
    let tests =
        fs::read_to_string(root().join("crates/leselang-hir/tests/effect_session.rs")).unwrap();
    assert!(tests.contains(
        "pending_transfer_refuses_initial_final_accepted_and_all_terminal_states_unchanged"
    ));
    assert!(
        tests.contains("transferred_binding_owns_all_move_only_frames_even_when_disposal_unwinds")
    );
    let fixture = tests
        .split("fn saved_session")
        .nth(1)
        .unwrap()
        .split("#[test]")
        .next()
        .unwrap();
    assert!(fixture.contains("session.poll(host)"));
    assert!(fixture.contains("session.try_into_pending_binding()"));
    assert!(!fixture.contains("evaluate_stacked_effects_in_scope"));
    let pipeline =
        fs::read_to_string(root().join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    assert!(pipeline.contains(
        "both_hosts_move_actual_pending_vm_frames_and_recover_without_budget_reset_or_replay"
    ));
    assert!(
        pipeline
            .contains("both_hosts_recheck_policy_and_keep_cancelled_transferred_sessions_terminal")
    );
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Moving Actual Pending VM State"));
}

#[test]
fn final_request_transfer_reuses_final_state_without_dropping_binding_frames() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/src/effect_stack_session.rs")).unwrap();
    for required in [
        "pub struct PendingStackedFinal",
        "pub fn try_into_pending_final(",
        "pub fn restore_pending_final<Error>",
        "core.try_into_pending_final()",
        "preflight_final_return_limits(limits)",
        "SessionCore::from_pending_final(identity, declaration, fuel)",
    ] {
        assert!(source.contains(required), "missing {required}");
    }
    let shared =
        fs::read_to_string(root().join("crates/leselang-hir/src/effect_session.rs")).unwrap();
    let transfer = shared
        .split("pub(crate) fn try_into_pending_final(")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn cancel(")
        .next()
        .unwrap();
    assert!(transfer.contains("State::AwaitingFinal(pending) => match pending.try_into_parts()"));
    assert!(transfer.contains("state => Err(Self { state, fuel })"));
    assert!(!transfer.contains("Fuel::new"));
    assert!(!transfer.contains(".clone()"));
    let local =
        fs::read_to_string(root().join("crates/leselang-hir/src/local_effect_session.rs")).unwrap();
    assert!(local.contains("StackedControlSession::restore_pending_final_with_scope("));
    assert!(!local.contains("pub fn try_into_pending_final"));
    assert!(local.contains("let pending = session.try_into_pending_final();"));
    let tests =
        fs::read_to_string(root().join("crates/leselang-hir/tests/effect_session.rs")).unwrap();
    for proof in [
        "actual_final_request_roundtrip_preserves_zero_fuel_domain_and_once_only_projection",
        "binding_waiters_cannot_be_misclassified_as_final_requests",
        "final_policy_limits_and_closed_scope_precede_native_admission_and_cleanup",
        "final_admission_refusal_and_unwind_release_owned_identity_and_errors",
        "final_local_acceptance_and_projection_panics_revoke_before_retained_cleanup",
        "final_local_cancel_drop_and_cleanup_unwind_never_project_or_rearm",
    ] {
        assert!(tests.contains(proof), "missing {proof}");
    }
    let pipeline =
        fs::read_to_string(root().join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    assert!(pipeline.contains("two_hosts_restore_direct_and_captured_final_requests_before_or_after_dispatch_at_every_fuel_cutoff"));
    assert!(pipeline.contains(
        "two_hosts_final_restore_and_local_ownership_preserve_revocation_and_cancellation"
    ));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Final-Request Ownership And Restoration"));
}

#[test]
fn pending_restore_uses_original_session_core_and_exports_ownership_regressions() {
    let session =
        fs::read_to_string(root().join("crates/leselang-hir/src/effect_stack_session.rs")).unwrap();
    for required in [
        "pub fn restore_pending<Error>",
        "preflight_saved_return_frames(&frames, limits)",
        "SessionCore::from_pending(",
        "StackedSessionRestoreError",
    ] {
        assert!(session.contains(required), "missing {required}");
    }
    let local =
        fs::read_to_string(root().join("crates/leselang-hir/src/local_effect_session.rs")).unwrap();
    assert!(local.contains("StackedControlSession::restore_pending_with_scope("));
    let facade = fs::read_to_string(root().join("crates/leselang-vm/src/native.rs")).unwrap();
    assert!(facade.contains("StackedSessionRestoreError"));
    let tests =
        fs::read_to_string(root().join("crates/leselang-hir/tests/effect_session.rs")).unwrap();
    for required in [
        "restored_session_preserves_original_frames_budget_and_reply_gated_successor",
        "restored_session_structure_precedes_admission_and_revokes_before_unused_hook_cleanup",
        "restored_session_closed_scope_native_refusal_and_unwind_never_publish_owner",
        "restored_session_cancel_or_drop_before_or_after_receipt_revokes_without_reentry",
        "restored_session_acceptance_unwind_keeps_original_terminal_fence",
        "restored_session_local_success_keeps_reply_rejection_recoverable_and_closes_on_completion",
        "restored_session_refusal_cleanup_unwind_keeps_scope_closed_and_drops_native_error_once",
        "restored_session_rechecks_cold_authority_and_exact_remaining_fuel_without_refill",
    ] {
        assert!(tests.contains(required), "missing {required}");
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Restoring Pending Session Ownership"));
    assert!(docs.contains("does\nnot yet combine their collector inventory and meter"));
}

#[test]
fn parallel_transfer_reuses_original_collectors_and_removes_fixture_reply_copies() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/src/prepared_parallel.rs")).unwrap();
    assert!(source.contains("pub fn try_into_saved("));
    assert!(source.contains("self.core.try_into_saved().map_err(|core| Self { core })"));
    let core =
        fs::read_to_string(root().join("crates/leselang-hir/src/parallel_evaluation.rs")).unwrap();
    let transfer = core
        .split("pub(crate) fn try_into_saved(")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn restore<")
        .next()
        .unwrap();
    assert!(transfer.contains("State::Awaiting { members, .. } | State::ReadyResults(members)"));
    assert!(transfer.contains("members[row.index] = Member::from_saved(row.reply)"));
    for forbidden in [
        ".clone()",
        ".try_accept",
        ".authorize",
        "Fuel::new",
        ".charge(",
    ] {
        assert!(!transfer.contains(forbidden), "unexpected {forbidden}");
    }
    let pipeline =
        fs::read_to_string(root().join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    assert!(
        pipeline
            .contains("two_hosts_transfer_real_all_barriers_through_one_vm_at_every_fuel_cutoff")
    );
    let saved = pipeline
        .split("mod saved_mixed {")
        .nth(1)
        .unwrap()
        .split("trait MixedProfile")
        .next()
        .unwrap();
    assert!(saved.contains("barrier.try_into_saved()"));
    assert!(!saved.contains("value: reply.value.clone()"));
    assert!(!saved.contains("buffer: Box::new(*reply.buffer)"));
    let tests =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_parallel.rs")).unwrap();
    for proof in [
        "actual_parallel_owners_roundtrip_every_mask_without_callbacks_clones_or_meter_changes",
        "transferred_parallel_admission_rejects_revocation_without_recreating_accepted_data",
        "dropping_transferred_parallel_rows_releases_each_owner_once_without_external_cancellation",
    ] {
        assert!(tests.contains(proof), "missing {proof}");
    }
    let writer = tests
        .split("fn publish<T: Data>")
        .nth(1)
        .unwrap()
        .split("fn recover<T: Data>")
        .next()
        .unwrap();
    assert!(writer.contains("session.try_into_saved()"));
    assert!(!writer.contains("Some(T::sample("));
    assert!(
        fs::read_to_string(root().join("docs/leselang-embedding.md"))
            .unwrap()
            .contains("### Moving Actual Parallel Collector State")
    );
}

#[test]
fn sequence_transfer_preserves_original_frontier_and_separate_prefix_ownership() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/src/prepared_sequence.rs")).unwrap();
    assert!(source.contains("pub fn try_into_saved<Reply>("));
    assert!(source.contains(".try_into_saved(self.prepared, accepted)"));
    let core =
        fs::read_to_string(root().join("crates/leselang-hir/src/sequence_evaluation.rs")).unwrap();
    let transfer = core
        .split("pub(crate) fn try_into_saved<Request, Reply>(")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn restore<Request, Reply, Error>(")
        .next()
        .unwrap();
    for required in [
        "accepted.len() != self.next",
        "!std::ptr::eq(row.branch, &self.branches[index])",
        "State::Terminal(SequenceEnd::Completed)",
        "pending.try_into_parts()",
        "return Err((self, prepared, accepted))",
        "SavedSequenceReply::Accepted(row.accepted)",
    ] {
        assert!(transfer.contains(required), "missing {required}");
    }
    for forbidden in [
        ".clone()",
        "PendingReply::new",
        ".try_accept",
        ".authorize",
        "Fuel::new",
        ".charge(",
        "panic!",
        ".unwrap(",
        ".expect(",
    ] {
        assert!(!transfer.contains(forbidden), "unexpected {forbidden}");
    }
    let pipeline =
        fs::read_to_string(root().join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    assert!(pipeline.contains(
        "two_hosts_transfer_seq_frontiers_and_original_prefixes_at_every_vm_fuel_cutoff"
    ));
    let mixed = pipeline
        .split("mod saved_mixed {")
        .nth(1)
        .unwrap()
        .split("trait MixedProfile")
        .next()
        .unwrap();
    assert!(mixed.contains("queue.try_into_saved(accepted)"));
    assert!(!mixed.contains("fn accept<"));
    assert!(!mixed.contains("SavedSequenceMember {"));
    let tests =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_sequence.rs")).unwrap();
    for proof in [
        "actual_seq_frontiers_move_original_prefix_pending_and_suffix_without_native_callbacks",
        "malformed_seq_prefix_returns_both_owners_unchanged_before_native_equality",
        "same_ir_seq_prefix_still_requires_original_execution_admission",
        "seq_transfer_rejects_cancelled_failed_and_uncertain_owners_without_losing_prefix",
        "transferred_seq_native_refusal_and_cleanup_unwind_release_each_retained_owner_once",
    ] {
        assert!(tests.contains(proof), "missing {proof}");
    }
    let checkpoint = tests
        .split("fn checkpoint<T: Data>")
        .nth(1)
        .unwrap()
        .split("fn recover<T: Data>")
        .next()
        .unwrap();
    assert!(checkpoint.contains("session.try_into_saved(prefix)"));
    assert!(!checkpoint.contains("value: T::sample("));
    assert!(
        fs::read_to_string(root().join("docs/leselang-embedding.md"))
            .unwrap()
            .contains("### Moving Actual Sequential Collector State")
    );
}

#[test]
fn scoped_collector_restore_reuses_original_cores_and_fences_retained_cleanup() {
    for (prepared, core, tests, name) in [
        (
            "prepared_sequence",
            "sequence_evaluation",
            "saved_sequence",
            "Sequence",
        ),
        (
            "prepared_parallel",
            "parallel_evaluation",
            "saved_parallel",
            "Parallel",
        ),
    ] {
        let wrapper =
            fs::read_to_string(root().join(format!("crates/leselang-hir/src/{prepared}.rs")))
                .unwrap();
        let source =
            fs::read_to_string(root().join(format!("crates/leselang-hir/src/{core}.rs"))).unwrap();
        let proof =
            fs::read_to_string(root().join(format!("crates/leselang-hir/tests/{tests}.rs")))
                .unwrap();
        assert!(wrapper.contains("pub fn restore_in_scope<"));
        assert!(wrapper.contains("Self::restore_with_scope(group, rows, limits, admit, None)"));
        assert!(
            wrapper.contains("Self::restore_with_scope(group, rows, limits, admit, Some(scope))")
        );
        assert!(wrapper.contains(&format!(
            "{name}ReplyCore::restore(branches, rows, admit, scope)"
        )));
        assert_eq!(source.matches("enum State<").count(), 1);
        let restore = source
            .split("pub(crate) fn restore<")
            .nth(1)
            .unwrap()
            .split("fence.finish();\n        output")
            .next()
            .unwrap();
        assert!(restore.contains("let fence = CallFence::new(scope)"));
        assert!(restore.contains("let duplicate = row.reply.identity() =="));
        assert!(restore.contains("scope.is_some_and(|scope| !scope.is_open())"));
        assert!(restore.contains("drop((admit, rows))"));
        assert!(restore.contains("let admitted = admit(&rows)"));
        assert!(restore.contains("drop(admitted)"));
        for forbidden in [
            "Fuel::new",
            ".clone()",
            "dispatch(",
            "prepare_effect",
            "serde",
        ] {
            assert!(
                !restore.contains(forbidden),
                "{core}: unexpected {forbidden}"
            );
        }
        for required in [
            "restore_in_scope(",
            "observe_cleanup",
            "equality_closes",
            "preflight_closes_before_rows_and_unused_hook_cleanup",
            "equality_revocation_and_unwind_stop_before_admission",
            "native_refusal_revocation_and_cleanup_unwind_preserve_fencing",
        ] {
            assert!(proof.contains(required), "{tests}: missing {required}");
        }
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Scoped Collector Restoration"));
    assert!(docs.contains("Callback-internal cleanup remains trusted"));
}

#[test]
fn final_shared_owner_restoration_delegates_without_inventing_frames_or_a_meter() {
    let session =
        fs::read_to_string(root().join("crates/leselang-hir/src/effect_stack_session.rs")).unwrap();
    let entry = session
        .split("pub fn restore_pending_final_in_scope<")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn restore_pending_final_with_scope<")
        .next()
        .unwrap();
    assert!(entry.contains("Self::restore_pending_final_with_scope("));
    assert!(entry.contains("Some(scope)"));
    for forbidden in [
        "Fuel::new",
        ".clone()",
        "dispatch(",
        "SavedEffectReturnFrame",
        "LocalExecutionScope::new",
    ] {
        assert!(!entry.contains(forbidden), "unexpected {forbidden}");
    }
    let pipeline =
        fs::read_to_string(root().join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    for required in [
        "Vm::restore_pending_final_in_scope(",
        "vm.try_into_pending_final()",
        "two_hosts_restore_final_seq_all_repeat_with_original_collectors_at_every_fuel_cutoff",
        "final_parent_and_child_restore_refusal_close_the_same_owner_at_every_frontier",
        "restored_final_groups_cancel_pending_collected_and_accepted_results_without_projection",
        "rows.swap(0, 1)",
    ] {
        assert!(pipeline.contains(required), "missing {required}");
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Shared-Owner Final Group Restoration"));
    assert!(docs.contains("in-memory final-group composition, not a durable codec"));
}

#[test]
fn shared_owner_restoration_lends_one_fence_without_another_lifecycle() {
    let owner =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/local_execution.rs"))
            .unwrap();
    assert!(owner.contains("pub fn as_parts_mut(&mut self) -> (&LocalExecutionScope, &mut Value)"));
    assert!(owner.contains("(&self.scope, &mut self.value)"));
    let session =
        fs::read_to_string(root().join("crates/leselang-hir/src/effect_stack_session.rs")).unwrap();
    let entry = session
        .split("pub fn restore_pending_in_scope<")
        .nth(1)
        .unwrap()
        .split("pub(crate) fn restore_pending_with_scope<")
        .next()
        .unwrap();
    assert!(entry.contains("Self::restore_pending_with_scope("));
    assert!(entry.contains("Some(scope)"));
    for forbidden in [
        "Fuel::new",
        ".clone()",
        "dispatch(",
        "State::",
        "LocalExecutionScope::new",
    ] {
        assert!(!entry.contains(forbidden), "unexpected {forbidden}");
    }
    let source =
        fs::read_to_string(root().join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    for required in [
        "owner.as_parts_mut()",
        "Sequence::restore_in_scope(",
        "Parallel::restore_in_scope(",
        "Vm::restore_pending_in_scope(",
        "child_restore_failure_revokes_the_already_restored_parent_at_every_frontier",
        "assert_eq!(owner.value().is_some(), child_failed)",
        "assert!(vm.cancel())",
        "assert!(sibling.is_open())",
    ] {
        assert!(source.contains(required), "missing {required}");
    }
    let handoff = source
        .split("mod coupled_transfer {")
        .nth(1)
        .unwrap()
        .split("fn handoff<")
        .nth(1)
        .unwrap()
        .split("fn run<")
        .next()
        .unwrap();
    // Only the explicit closed-scope fault may close before the core is entered.
    assert_eq!(handoff.matches("scope.close()").count(), 1);
    assert!(!handoff.contains("owner.close()"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Shared-Owner Graph Restoration"));
    assert!(docs.contains("does not mistake revocation for automatic cancellation"));
}

#[test]
fn coupled_transfer_uses_original_vm_and_collectors_without_a_second_graph_runtime() {
    let source =
        fs::read_to_string(root().join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    let coupled = source
        .split("mod coupled_transfer {")
        .nth(1)
        .unwrap()
        .split("mod saved_mixed {")
        .next()
        .unwrap();
    for required in [
        "slot.take().unwrap()",
        "vm.try_into_pending_binding()",
        "Vm::restore_pending_in_scope(",
        "pending.fuel",
        "pending.limits",
        "scope.bind(Some(Vm::new(",
        "runtime.collect_with_checkpoint(",
        "owner.as_parts_mut()",
        "Rc::as_ptr(group)",
        "source.validate_rows(group.members())?",
        "coupled_vm_and_collectors_keep_original_captures_and_budget_at_every_cutoff",
        "coupled_vm_and_collector_transfer_refuses_every_revoked_or_corrupt_cut",
        "coupled_native_admission_unwind_closes_owner_without_rearming_root_or_children",
    ] {
        assert!(coupled.contains(required), "missing {required}");
    }
    let handoff = coupled
        .split("fn handoff<")
        .nth(1)
        .unwrap()
        .split("fn run<")
        .next()
        .unwrap();
    for forbidden in [
        "Fuel::new",
        ".clone()",
        "SavedSequenceMember {",
        "SavedParallelMember {",
        ".dispatch(",
    ] {
        assert!(!handoff.contains(forbidden), "unexpected {forbidden}");
    }
    assert!(source.contains("let saved = queue.try_into_saved(prefix).map_err(|_| Refusal)?;\n            checkpoint.run(TransferPhase::Parent)?;"));
    assert!(source.contains(
        "let rows = barrier.try_into_saved().map_err(|_| Refusal)?;\n            checkpoint.run(TransferPhase::Parent)?;"
    ));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Coupled VM And Collector Transfer"));
    assert!(docs.contains("No public\nAPI changed: native contract `0.166.0`"));
}

#[test]
fn metered_owned_recovery_joins_existing_byte_codecs_collectors_and_session_core() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_group.rs")).unwrap();
    let owned = source
        .split("mod owned {")
        .nth(1)
        .unwrap()
        .split("fn cases(")
        .next()
        .unwrap();
    for entry in [
        "Session::restore_pending(",
        "Collector::restore(saved, &env)",
        "with_metered_host_outcome(collector",
        "assemble_group_reply(",
        "collector.cancel(&env, &lease)",
        ".try_accept(&999, reply, &active)",
        "Self::Owned(vm) => vm.try_accept(identity, reply, authority)",
        "vm.try_accept_in_scope(identity, reply, authority, scope)",
        "session.poll(&env)",
        "owned_checkpoint_uses_one_meter_from_collectors_through_both_capture_returns",
        "owned_checkpoint_all_fuel_cutoffs_match_the_original_manual_reentry",
        "owned_checkpoint_cancellation_hands_back_collectors_after_revocation",
        "owned_checkpoint_live_collector_capture_and_cold_return_denials_close_parent",
    ] {
        assert!(owned.contains(entry), "missing {entry}");
    }
    let core =
        fs::read_to_string(root().join("crates/leselang-hir/src/effect_session.rs")).unwrap();
    assert!(core.contains("self.with_metered_host_work(input, |input, _| work(input))"));
    assert_eq!(
        core.matches("let output = work(input, &mut self.fuel)")
            .count(),
        1
    );
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Metered Owned Checkpoint Recovery"));
    assert!(docs.contains("must not be used to replace/refill Fuel"));
}

#[test]
fn repeated_source_checkpoints_reuse_the_existing_compiler_and_receipt_boundaries() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_group.rs")).unwrap();
    let model = source
        .split("mod repeat {")
        .nth(1)
        .unwrap()
        .split("mod parallel {")
        .next()
        .unwrap();
    for required in [
        "initial_source::<T>",
        "source::program_with_group",
        "source::resume",
        "restored_queue",
        "tail__iteration_1",
        "branch_operation(branch)",
        "repeated_source_groups_restore_expanded_names_operations_and_original_returns",
        "repeated_source_recovery_preserves_every_assembly_and_return_fuel_cutoff",
        "repeated_source_bounds_and_cold_authority_fail_before_native_preparation",
        "repeated_source_recovery_rechecks_current_cold_and_second_return_authority",
        "identical_operations_cannot_exchange_member_receipts_after_restore",
        "repeated_source_forged_expansion_operands_and_graphs_do_not_enter_recovery",
    ] {
        assert!(model.contains(required), "missing {required}");
    }
    for forbidden in [
        "leselang_vm::Vm",
        "leserpent_domain",
        "rusqlite",
        "Node::Bind {",
        "compile_catalog_prepared_group_helper_program",
    ] {
        assert!(!model.contains(forbidden), "unexpected {forbidden}");
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Bounded Repeat Program Recovery"));
    assert!(docs.contains("332 additional frontier/order"));
    assert!(docs.contains("not proof of arbitrary untrusted-codec validation"));
}

#[test]
fn dispatch_lease_fence_is_shared_without_exporting_reference_storage() {
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/dispatch_lease.rs"))
            .unwrap();
    for forbidden in ["leserpent", "rusqlite", "serde", "unsafe", "Vec<", "Box<"] {
        assert!(!core.contains(forbidden));
    }
    assert!(core.contains("pub fn validate_dispatch_lease("));
    assert!(core.contains("submitted.expires_at_ms < now_ms"));
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_dispatch.rs"))
            .unwrap();
    assert!(journal.contains("use leselang_command::reference_dispatch::{"));
    assert_eq!(journal.matches("validate_observed_lease(").count(), 2);
    assert!(policy.contains("validate_dispatch_lease("));
    assert!(policy.contains("DispatchLeaseError::Superseded"));
    let successor =
        fs::read_to_string(root().join("crates/leselang-vm/src/journal/successor.rs")).unwrap();
    assert!(successor.contains("validate_stored_lease(&dispatch, lease, now_ms, true)?"));
    for source in [&journal, &successor, &policy] {
        assert!(!source.contains("lease.lease_expires_at_ms < now_ms"));
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/tests/dispatch_lease.rs"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Shared Dispatch Lease Fencing"));
    assert!(docs.contains("does not turn `LocalVm` restoration into a journal"));
}

#[test]
fn semantic_retry_planning_reuses_numeric_kernels_and_stays_outside_host_storage() {
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/retry_schedule.rs"))
            .unwrap();
    for forbidden in [
        "leserpent",
        "rusqlite",
        "serde",
        "unsafe",
        "EffectError",
        "std::time",
    ] {
        assert!(!core.contains(forbidden));
    }
    for required in [
        "pub fn plan_retry_schedule(",
        "pub fn validate_retry_schedule(",
        "capped_exponential_delay(",
        "checked_clock_add(",
        ".checked_add(1)",
    ] {
        assert!(core.contains(required));
    }
    let profile =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert!(profile.contains("leselang_command::reference_retry::report_effect_error("));
    let entry =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_retry.rs")).unwrap();
    assert!(entry.contains("crate::retry_policy::plan_effect_retry("));
    assert!(!entry.contains("plan_retry_schedule("));
    assert!(!entry.contains("rusqlite"));
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/retry_policy.rs")).unwrap();
    assert!(adapter.contains("plan_retry_schedule("));
    assert!(!adapter.contains("fn retry_delay("));
    assert!(!profile.contains("fn retry_delay("));
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let commit =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_retry_commit.rs"))
            .unwrap();
    assert_eq!(commit.matches("validate_proposed_retry(").count(), 2);
    assert!(journal.contains("reference_retry_commit::report_memory("));
    assert!(journal.contains("reference_retry_commit::report_durable("));
    let dispatch =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_dispatch.rs"))
            .unwrap();
    assert!(journal.contains("use leselang_command::reference_dispatch::{"));
    assert!(dispatch.contains("validate_retry_schedule("));
    assert!(!dispatch.contains("schedule.retry_count + 1"));
    assert!(!journal.contains("dispatch.retry_count + 1"));
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/tests/retry_schedule.rs"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Shared Semantic Retry Planning"));
    assert!(docs.contains("not a retry registry, durable owner or crash settlement"));
}

#[test]
fn dispatch_capacity_is_shared_without_moving_storage_or_product_policy_into_core() {
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/dispatch_capacity.rs"))
            .unwrap();
    for forbidden in [
        "leserpent",
        "rusqlite",
        "serde",
        "unsafe",
        "MAX_JOURNAL",
        "std::sync",
    ] {
        assert!(!core.contains(forbidden));
    }
    assert!(core.contains("pub fn check_dispatch_admission("));
    assert!(core.contains("pub fn dispatch_lease_capacity_exhausted("));
    assert!(core.contains(".checked_add(additional)"));
    let scheduling =
        fs::read_to_string(root().join("crates/leselang-vm/src/scheduling.rs")).unwrap();
    assert!(scheduling.contains("check_scheduler_admission(self, pending, additional)"));
    assert!(scheduling.contains("scheduler_lease_capacity_exhausted(self)"));
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/scheduler_policy.rs")).unwrap();
    assert!(
        policy.contains(
            "check_dispatch_admission(pending, additional, limits.max_pending_dispatches)"
        )
    );
    assert!(policy.contains(
        "dispatch_lease_capacity_exhausted(pressure.active_leases, pressure.limits.max_active_leases)"
    ));
    assert!(!scheduling.contains("pending.saturating_add"));
    assert!(scheduling.contains("leselang_command::reference_startup::validate_limits(self)"));
    let startup =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_startup.rs"))
            .unwrap();
    assert!(startup.contains("LSV2500"));
    for code in ["LSV2501", "LSV2503"] {
        assert!(scheduling.contains(code));
    }
    assert!(scheduling.contains("leselang_command::reference_claim::lease_capacity_fault(self)"));
    let claim =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_claim.rs")).unwrap();
    assert!(claim.contains("LSV2502"));
    assert!(!claim.contains("dispatch_lease_capacity_exhausted("));
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(journal.contains("Ok(dispatch_lease_capacity_exhausted(active, limit))"));
    let bounded_query = journal
        .split("fn sql_lease_capacity_exhausted(")
        .nth(1)
        .unwrap()
        .split("fn sql_scheduler_pressure(")
        .next()
        .unwrap();
    assert!(bounded_query.contains("LIMIT ?2"));
    assert!(bounded_query.contains("d.lease_expires_at_ms > ?1"));
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/tests/dispatch_capacity.rs"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Shared Dispatch Capacity"));
    assert!(docs.contains("not a reservation, queue or"));
    assert!(docs.contains("atomic admission owner"));
}

#[test]
fn lease_issuance_planning_stays_separate_from_existing_lease_settlement() {
    let core = fs::read_to_string(root().join("crates/leselang-runtime-core/src/dispatch_plan.rs"))
        .unwrap();
    for forbidden in [
        "leserpent",
        "rusqlite",
        "serde",
        "unsafe",
        "MAX_DISPATCH",
        "std::time",
    ] {
        assert!(!core.contains(forbidden));
    }
    for required in [
        "pub fn plan_dispatch_lease(",
        "pub fn next_dispatch_attempt(",
        "checked_clock_add(now_ms, 0)",
        "checked_clock_add(now_ms, lease_ms)",
        ".checked_add(1)",
    ] {
        assert!(core.contains(required));
    }
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/reference_dispatch.rs"))
            .unwrap();
    assert!(journal.contains("use leselang_command::reference_dispatch::{"));
    assert!(journal.contains("lease_expiration(now_ms, lease_ms)?"));
    assert!(journal.contains("next_attempt(dispatch.attempt)?"));
    assert!(journal.contains("next_attempt(attempt)?"));
    assert!(policy.contains("plan_dispatch_lease(now_ms, lease_ms, MAX_DISPATCH_LEASE_MS)"));
    assert!(policy.contains("next_dispatch_attempt(current, MAX_DISPATCH_ATTEMPTS)"));
    assert_eq!(
        journal
            .matches("journal.claim_dispatch(now_ms, expires_at_ms, limits)")
            .count(),
        2
    );
    let successor =
        fs::read_to_string(root().join("crates/leselang-vm/src/journal/successor.rs")).unwrap();
    assert!(successor.contains("validate_dispatch_clock(now_ms)?;"));
    assert!(!successor.contains("validate_lease_clock("));
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/tests/dispatch_plan.rs"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Shared Dispatch Issuance Planning"));
    assert!(docs.contains("not a lease grant, durable owner or crash settlement"));
}

#[test]
fn dispatch_data_is_host_owned_and_reference_types_are_not_mirrored() {
    let core = fs::read_to_string(root().join("crates/leselang-runtime-core/src/dispatch_data.rs"))
        .unwrap();
    for name in [
        "DispatchLease<Request>",
        "RetrySchedule<Error>",
        "FailedEffect<Error>",
    ] {
        assert!(core.contains(&format!("pub struct {name}")));
    }
    for forbidden in [
        "leserpent",
        "leselang_command",
        "EffectRequest",
        "EffectErrorClass",
        "rusqlite",
        "std::time",
        "pub enum RetryDisposition",
    ] {
        assert!(!core.contains(forbidden), "{forbidden}");
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/execution_outcome.rs"))
            .unwrap();
    for (name, payload) in [
        ("DispatchLease", "EffectRequest"),
        ("RetrySchedule", "EffectError"),
        ("FailedEffect", "EffectError"),
    ] {
        assert!(adapter.contains(&format!(
            "pub type {name} = leselang_runtime_core::{name}<{payload}>;"
        )));
        assert!(!vm.contains(&format!("pub struct {name} {{")));
    }
    assert!(vm.contains(
        "pub use leselang_command::{DispatchLease, FailedEffect, RetryDisposition, RetrySchedule}"
    ));
    assert!(adapter.contains("pub enum RetryDisposition {"));
    assert!(!adapter.contains("pub type RetryDisposition"));
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("consumer/tests/dispatch_data.rs"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Host-Owned Dispatch Data"));
    assert!(docs.contains("data\nownership extraction, not a generic scheduler"));
}

#[test]
fn borrowed_vm_lifecycle_reuses_local_owner_fences_after_restore() {
    let session =
        fs::read_to_string(root().join("crates/leselang-hir/src/effect_stack_session.rs")).unwrap();
    for required in [
        "pub fn cancel_in_scope(",
        "pub fn try_accept_in_scope<",
        "pub fn poll_in_scope<",
        "pub(crate) fn call_in_scope<",
        ".try_accept_with_scope(identity, reply, authority, Some(scope))",
        "session.poll_with_scope(environment, Some(scope))",
    ] {
        assert!(session.contains(required), "missing {required}");
    }
    let local =
        fs::read_to_string(root().join("crates/leselang-hir/src/local_effect_session.rs")).unwrap();
    for required in [
        "self.session.cancel_in_scope(&self.scope)",
        ".with_host_outcome_in_scope(input, work, &self.scope)",
        ".try_accept_in_scope(identity, reply, authority, &self.scope)",
        "self.session.poll_in_scope(environment, &self.scope)",
    ] {
        assert!(
            local.contains(required),
            "missing shared delegation {required}"
        );
    }
    assert!(!local.contains("pub fn try_into_pending"));
    let pipeline =
        fs::read_to_string(root().join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    let coupled = pipeline
        .split("mod coupled_transfer {")
        .nth(1)
        .unwrap()
        .split("#[test]")
        .next()
        .unwrap();
    for forbidden in [".poll(", ".try_accept(", ".cancel()", "owner.close()"] {
        assert!(
            !coupled.contains(forbidden),
            "unfenced coordinator {forbidden}"
        );
    }
    for required in [
        ".poll_in_scope(",
        ".try_accept_in_scope(",
        ".cancel_in_scope(",
    ] {
        assert!(coupled.contains(required), "missing {required}");
    }
    let tests =
        fs::read_to_string(root().join("crates/leselang-hir/tests/effect_session.rs")).unwrap();
    for proof in [
        "restored_scoped_acceptance_unwind_revokes_before_retained_cleanup",
        "restored_closed_scope_cancels_before_poll_or_reply_validation",
        "restored_scoped_rejection_retries_original_reply_then_completion_closes",
        "restored_scoped_reentry_refusal_and_unwind_keep_terminal_reason",
        "restored_scoped_cancel_revokes_before_pending_or_accepted_cleanup_unwind",
        "restored_raw_vm_borrows_owner_and_bound_drop_revokes_first",
    ] {
        assert!(tests.contains(proof), "missing {proof}");
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Borrowed-Scope VM Lifecycle"));
    assert!(docs.contains("the raw VM does not remember or authenticate its owner"));
}

#[test]
fn borrowed_host_work_uses_the_same_core_meter_and_dual_owner_checkpoint_fixture() {
    let session =
        fs::read_to_string(root().join("crates/leselang-hir/src/effect_stack_session.rs")).unwrap();
    for required in [
        "pub fn with_host_work_in_scope<",
        "pub fn with_host_outcome_in_scope<",
        "pub fn with_metered_host_outcome_in_scope<",
        "session.core.with_host_work(input, |input|",
        "session.core.with_metered_host_work(input, |input, fuel|",
    ] {
        assert!(session.contains(required), "missing {required}");
    }
    let local =
        fs::read_to_string(root().join("crates/leselang-hir/src/local_effect_session.rs")).unwrap();
    for required in [
        ".with_host_work_in_scope(input, work, &self.scope)",
        ".with_host_outcome_in_scope(input, work, &self.scope)",
        ".with_metered_host_outcome_in_scope(input, work, &self.scope)",
    ] {
        assert!(
            local.contains(required),
            "missing shared delegation {required}"
        );
    }
    assert!(!local.contains("CallFence"));
    assert!(!local.contains("fn call<"));
    let tests =
        fs::read_to_string(root().join("crates/leselang-hir/tests/effect_session.rs")).unwrap();
    for proof in [
        "restored_scoped_host_work_preserves_move_only_input_and_original_meter",
        "restored_scoped_host_work_refuses_every_nonawaiting_state_without_entering",
        "restored_scoped_host_outcomes_and_unwind_revoke_before_retained_cleanup",
        "restored_scoped_host_work_cannot_reenter_after_callback_revokes_owner",
    ] {
        assert!(tests.contains(proof), "missing {proof}");
    }
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_group.rs")).unwrap();
    let model = source
        .split("mod owned {")
        .nth(1)
        .unwrap()
        .split("\n                fn cases(")
        .next()
        .unwrap();
    for required in [
        "Borrowed(ScopedLocalExecution<RawSession",
        "RawSession::restore_pending_in_scope(",
        "scope.bind(vm)",
        "vm.with_host_outcome_in_scope(input, work, scope)",
        "vm.with_metered_host_outcome_in_scope(input, work, scope)",
        "Collector::restore(saved, &env)",
        "assemble_group_reply(",
    ] {
        assert!(model.contains(required), "missing {required}");
    }
    let original_checkpoint = model.split("mod successor {").next().unwrap();
    assert_eq!(
        original_checkpoint
            .matches("for borrowed in [false, true]")
            .count(),
        4
    );
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Borrowed-Scope Host Work And Metering"));
    assert!(docs.contains("normal native Err is not an automatic terminal failure"));
}

#[test]
fn guarded_restored_source_pipeline_composes_existing_owners_without_repair_cancellation() {
    let source =
        fs::read_to_string(root().join("crates/leselang-vm/tests/native_host_pipeline.rs"))
            .unwrap();
    let pipeline = source
        .split("mod guarded_work {")
        .nth(1)
        .unwrap()
        .split("mod saved_mixed {")
        .next()
        .unwrap();
    for required in [
        "compile(&source, &host)",
        "scope.bind(Some(Vm::new(",
        "handoff(slot, scope, &runtime, active.plan, Fault::None)",
        "vm.with_metered_host_outcome_in_scope(",
        "runtime.collect_with_checkpoint(",
        "HostWorkOutcome::Pending(Ok(reply))",
        "HostWorkOutcome::Cancelled(Err(error))",
        "HostWorkOutcome::Failed(Err(error))",
        "EffectSessionEnd::HostUncertain",
        "vm.try_accept_in_scope(&identity, reply, &runtime, scope)",
        "vm.poll_in_scope(&runtime, scope)",
        "baseline.remaining + trace.charges",
        "restored_guarded_work_matches_original_source_pipelines_at_every_fuel_cutoff",
        "restored_guarded_native_work_and_reentry_share_every_meter_cutoff",
        "every_native_work_fault_seals_restored_parents_without_manual_cancellation",
    ] {
        assert!(pipeline.contains(required), "missing {required}");
    }
    assert_eq!(pipeline.matches("Fuel::new(").count(), 1);
    for forbidden in [
        "owner.close()",
        ".cancel(",
        ".cancel_in_scope(",
        "evaluate_stacked",
        "restore_pending_with_scope(",
        "leserpent_domain",
        "rusqlite",
    ] {
        assert!(!pipeline.contains(forbidden), "unexpected {forbidden}");
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Guarded Restored Source Pipelines"));
    assert!(docs.contains("No public API or native contract version changes"));
}

#[test]
fn decoded_source_checkpoints_resume_native_effects_without_a_second_evaluator() {
    let source =
        fs::read_to_string(root().join("crates/leselang-hir/tests/saved_group.rs")).unwrap();
    let successor = source
        .split("mod successor {")
        .nth(1)
        .unwrap()
        .split("\n                fn cases(")
        .next()
        .unwrap();
    let compact: String = successor.split_whitespace().collect();
    for required in [
        "source::compile_model_with_result",
        "initial_tree::<T>(",
        "decode::<T>(&bytes, &bytes)",
        "Session::restore_pending(",
        "Collector::restore(saved, &current)",
        "self.next.prepare_effect(node, scope, fuel)",
        "CorrelatedEffectRequest",
        "std::ptr::eq(node, self.domain.node)",
        "session.with_metered_host_outcome(",
        "session.try_accept(&hosts[3].key(90)",
        "session.poll(&env)",
        "Rc::as_ptr(&refused.reply)",
        "Stop::BeforeReentry",
        "Stop::AfterRequest",
        "HostWorkOutcome::Cancelled",
        "assert_eq!(&counts[..2], &[(0, 0), (0, 0)])",
        "decoded_source_frames_continue_to_a_distinct_host_without_replaying_settled_groups",
        "decoded_successors_share_the_checkpoint_meter_at_every_cutoff",
        "decoded_successor_revocation_and_cancellation_stop_before_new_dispatch",
    ] {
        assert!(
            compact.contains(&required.split_whitespace().collect::<String>()),
            "missing {required}"
        );
    }
    assert_eq!(successor.matches("Fuel::new(").count(), 1);
    for forbidden in [
        "evaluate_stacked",
        "resume_accepted",
        ".cancel(",
        "scope.close(",
        "leserpent_domain",
        "rusqlite",
    ] {
        assert!(!successor.contains(forbidden), "unexpected {forbidden}");
    }
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("### Decoded Checkpoint Successor Effects"));
    assert!(docs.contains("does not establish cross-process exactly-once effects"));
}

#[test]
fn ci_and_docs_keep_native_verticals_separate_from_reference_compatibility() {
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("cargo +1.98.0 test -p leselang-hir --locked --no-default-features"));
    assert!(workflow.contains(
        "cargo +1.98.0 clippy -p leselang-hir --locked --no-default-features --all-targets -- -D warnings"
    ));
    assert!(workflow.contains("cargo test --workspace --all-features --locked"));
    assert!(workflow.contains(
        "rustfmt +1.98.0 --edition 2024 --check crates/leselang-hir/src/leserpent_profile.rs"
    ));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    for boundary in [
        "default-features = false",
        "leserpent-profile",
        "Cargo features are additive",
        "not the complete independent VM or repository extraction gate",
    ] {
        assert!(docs.contains(boundary));
    }
    for name in [
        "catalog_source",
        "catalog_group_source",
        "bound_projection_session",
        "effect_session",
        "effect_stack",
        "parallel_evaluation",
        "group_reply",
        "group_reply_reentry",
        "group_source",
        "native_group_preparation",
        "prepared_parallel",
        "prepared_sequence",
        "prepared_call_evaluation",
        "sequence_evaluation",
        "prepared_gui_host",
    ] {
        let source =
            fs::read_to_string(root().join(format!("crates/leselang-hir/tests/{name}.rs")))
                .unwrap();
        assert!(!source.contains("#![cfg(feature = \"leserpent-profile\")]"));
    }
}
