use std::{fs, path::Path, process::Command};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn assert_complete_record_recovery_order() {
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/record_recovery.rs"))
            .unwrap();
    let body = core
        .split_once("pub fn inspect_record_recovery<")
        .unwrap()
        .1
        .split_whitespace()
        .collect::<String>();
    let mut offset = 0;
    for stage in [
        "host.admit_image()",
        "host.sequence(&image)",
        "host.validate_identity(&image)",
        "host.validate_deadline(&image)",
        "host.validate_owner(&image)",
        "host.stored_state()",
        "StoredRecordState::Pending",
        "host.validate_successor(&image, None)",
        "host.pending_payload(image)",
        "StoredRecordState::Completed",
        "host.admit_completed()",
        "host.validate_successor(&image, Some(&completed))",
        "host.completed_payload(image, completed)",
        "Ok(RecoveryRecord",
    ] {
        let compact = stage.split_whitespace().collect::<String>();
        offset += body[offset..]
            .find(&compact)
            .unwrap_or_else(|| panic!("missing/reordered {stage}"))
            + compact.len();
    }
    for absent in [
        "rusqlite",
        "leselang_command",
        "leselang_vm",
        "ContinuationImage",
        "serde_json",
        "Fuel::",
        ".clone()",
        "unwrap(",
        "expect(",
    ] {
        assert!(!core.contains(absent), "{absent}");
    }
}

#[test]
fn complete_effect_record_recovery_is_shared_before_opaque_checkpoint_staging() {
    assert_complete_record_recovery_order();
    let code = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(
        code.contains("impl leselang_runtime_core::RecordRecoveryHost for SqliteRecoveryRecord")
    );
    assert!(code.contains("inspect_record_recovery(&mut SqliteRecoveryRecord"));
    let frozen = fs::read_to_string(
        root().join("crates/leselang-vm/src/journal/record_recovery_contract_tests.rs"),
    )
    .unwrap();
    assert!(!frozen.contains("inspect_record_recovery("));
    assert!(frozen.contains("assert_eq!(comparisons, 784)"));
    assert!(frozen.contains("assert_eq!(snapshot(&tx), before)"));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-runtime-core/tests/checkpoint.rs"))
            .unwrap();
    for test in [
        "stored_record_moves_original_noncomparable_local_owners_and_defers_pending_receipts",
        "stored_record_errors_keep_original_ownership_and_exact_native_stage",
        "stored_record_every_native_unwind_and_late_cleanup_panic_prevent_payload_handoff",
        "stored_record_late_refusal_or_commit_failure_never_publishes_checkpoint_prefix",
    ] {
        assert!(consumer.contains(test), "{test}");
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("crates/leselang-runtime-core/src/record_recovery.rs"));
    assert!(exporter.contains("consumer/tests/checkpoint_recovery.rs"));
}

fn assert_complete_group_recovery_order() {
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/group_recovery.rs"))
            .unwrap();
    let ordered = core.split_once("pub fn inspect_group_recovery<").unwrap().1;
    let mut offset = 0;
    for stage in [
        "validate_group_graph(",
        "rows.iter().all(|row| host.is_completed(row))",
        "host.validate_chain()",
        "host.validate_pending_sequence(rows)",
        "host.stored_state()",
        "validate_group_receipt(",
        "host.validate_records(receipt.as_ref())",
        "Ok(receipt)",
    ] {
        offset += ordered[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("lost recovery order: {stage}"))
            + stage.len();
    }
}

fn assert_complete_group_batch_order() {
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/group_recovery_batch.rs"))
            .unwrap();
    let body = core
        .split_once("pub fn inspect_group_recovery_batch<")
        .unwrap()
        .1;
    let mut previous = 0;
    for step in [
        "groups.len() > limits.max_groups",
        "SequenceReservations::new(",
        "for stored in groups",
        ".admit_group(stored)",
        "host.highest_sequence(&group)",
        "host.reserved_sequences(&group)",
        ".insert(sequence)",
        "host.validate_order(&group)",
        "host.inspect_group(group)",
        ".finish()",
        "Ok(GroupRecoveryBatchSummary",
    ] {
        let position = body.find(step).unwrap_or_else(|| panic!("missing {step}"));
        assert!(position >= previous, "reordered {step}");
        previous = position;
    }
    for absent in [
        "rusqlite",
        "leselang_command",
        "leselang_vm",
        "Fuel::",
        ".clone()",
        "unwrap(",
        "expect(",
    ] {
        assert!(!core.contains(absent), "{absent}");
    }
}

#[test]
fn complete_stored_group_batch_uses_shared_global_reservations_before_checkpoint_commit() {
    assert_complete_group_batch_order();
    let journal = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    for required in [
        "inspect_group_recovery_batch(",
        "max_groups: MAX_JOURNAL_RECORDS",
        "GroupRecoveryBatchError::Reservation(_) => crate::group_binding::invalid()",
        "impl leselang_runtime_core::GroupRecoveryBatchHost<RawMergeGroup>",
        "validate_recovered_group(self.0, &token, &plan, &branches, &state, terminal)",
    ] {
        assert!(journal.contains(required), "{required}");
    }
    let frozen = fs::read_to_string(
        root().join("crates/leselang-vm/src/journal/group_recovery_contract_tests.rs"),
    )
    .unwrap();
    assert!(frozen.contains("assert_eq!(comparisons, 364)"));
    assert!(!frozen.contains("inspect_group_recovery_batch("));
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("crates/leselang-runtime-core/src/group_recovery_batch.rs"));
    assert!(exporter.contains("consumer/tests/recovery_fences.rs"));
}

fn assert_complete_successor_admission_order() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_admission.rs"))
            .unwrap();
    let body = adapter
        .split_once("pub fn validate_successor_pair")
        .unwrap()
        .1;
    let mut offset = 0;
    for stage in [
        "validate_effect_request(",
        ".map_err(Parent)?",
        "validate_effect_request(",
        ".map_err(Next)?",
        "completed.ok_or(Completion)?",
        "matches!(value, Value::Scalar",
        "host.validate_completed_value(",
        "validate_preflighted_successor_pair(",
    ] {
        offset += body[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out-of-order {stage}"))
            + stage.len();
    }
}

#[test]
fn complete_successor_admission_shares_order_without_native_storage_or_error_payloads() {
    assert_complete_successor_admission_order();
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_admission.rs"))
            .unwrap();
    for required in [
        "pub trait SuccessorAdmissionHost<Token>",
        "pub enum SuccessorAdmissionError<Native>",
        "completed: Option<&Value>",
        "Host: SuccessorAdmissionHost<Token> + ?Sized",
        "impl<Native> std::fmt::Debug",
        "impl<Native> std::error::Error",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm",
        "serde_json",
        "Fuel::",
        "Journal",
        ".clone()",
        "impl Default",
        "LSV",
    ] {
        assert!(
            !adapter.contains(absent),
            "successor admission grew {absent}"
        );
    }
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    for required in [
        "leselang_command::validate_successor_pair(",
        "validate_bound_value(parent.continuation, value)",
        "SuccessorAdmissionError::Value(error) => error",
        "request_admission_fault(error)",
    ] {
        assert!(vm.contains(required), "{required}");
    }
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/successor_pair_contract_tests.rs"))
            .unwrap();
    assert!(frozen.contains("frozen_validate_effect_request(parent)?"));
    assert!(frozen.contains("frozen_validate_effect_request(next)?"));
    assert!(frozen.contains("assert_eq!(comparisons, 5_400)"));
    assert!(!frozen.contains("validate_bound_value("));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    assert!(
        consumer
            .contains("complete_successor_admission_rechecks_requests_before_completion_and_value")
    );
    assert!(consumer.contains(
        "complete_successor_admission_borrows_local_owners_and_preserves_each_native_unwind"
    ));
}

#[test]
fn complete_request_admission_is_one_shared_ordered_gate_without_native_authority() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/request_admission.rs"))
            .unwrap();
    for required in [
        "pub enum RequestAdmissionError",
        "pub fn validate_effect_request<Token>",
        "request: EffectRequestView<'_, Token>",
        "limits: ContinuationAdmissionLimits",
        "group_owner_text: impl FnOnce(&Token) -> &str",
        "validate_token: impl FnOnce(&Token) -> bool",
        "canonical_sequence: impl FnOnce(&Token) -> Option<u64>",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    let ordered = adapter
        .split_once("pub fn validate_effect_request")
        .unwrap()
        .1;
    let stages = [
        "validate_continuation_frame(",
        ".map_err(RequestAdmissionError::Frame)?",
        "validate_preflighted_effect_request(",
    ]
    .map(|stage| ordered.find(stage).unwrap());
    assert!(stages.windows(2).all(|pair| pair[0] < pair[1]));
    for absent in [
        "leselang_vm",
        "serde_json",
        "Fuel::",
        "Journal",
        ".clone()",
        "impl Default",
        "LSV",
        "canonical_source(",
    ] {
        assert!(!adapter.contains(absent), "request admission grew {absent}");
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let admission = vm
        .split_once("pub fn validate_effect_request(")
        .unwrap()
        .1
        .split_once("fn request_correspondence_fault(")
        .unwrap()
        .0;
    for required in [
        "leselang_command::validate_effect_request(",
        "continuation_admission_limits()",
        "ContinuationToken::as_str",
        "valid_continuation_token,",
        "valid_merge_branch_name,",
        "canonical_continuation_sequence,",
        "RequestAdmissionError::Frame(error)",
        "RequestAdmissionError::Correspondence(error)",
    ] {
        assert!(admission.contains(required), "{required}");
    }
    assert!(!admission.contains("validate_image("));
    assert!(!admission.contains("validate_preflighted_effect_request("));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/operation_request_tests.rs"))
            .unwrap();
    assert!(frozen.contains(
        "crate::continuation_schema_tests::frozen_validate_image(&request.continuation)?"
    ));
    assert!(frozen.contains(
        "complete_request_admission_preserves_frozen_frame_before_correspondence_and_decode"
    ));
    assert!(frozen.contains("4_200"));
    let proofs =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    for proof in [
        "checked_request_admission_covers_seventy_operations_without_copying_original_frames",
        "checked_request_admission_keeps_frame_before_cold_authority_and_canonical_identity",
        "checked_request_admission_borrows_opaque_local_owners_in_original_hook_order",
        "checked_request_admission_propagates_each_native_unwind_and_drops_unused_hooks",
        "checked_request_admission_rechecks_mutable_data_without_promoting_live_grants",
    ] {
        assert!(proofs.contains(proof), "{proof}");
    }
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test host_protocol"));
}

#[test]
fn successor_pair_correspondence_is_shared_after_native_request_and_value_admission() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_correspondence.rs"))
            .unwrap();
    for required in [
        "pub fn validate_preflighted_successor_pair<Token>",
        "pub enum SuccessorCorrespondenceError",
        "parent: EffectRequestView<'_, Token>",
        "limits: ResultBindingLimits",
        "token_text: impl Fn(&Token) -> &str",
        "parent.has_same_inherited_context(&next)",
        "limits.captures.max_saved_bindings",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm::",
        "Fuel::",
        "serde_json",
        ".clone()",
        "Journal",
        "impl Default",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let source = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    let pair = source
        .split_once("pub(super) fn validate_pair(")
        .unwrap()
        .1
        .split_once("enum PreparedResult")
        .unwrap()
        .0;
    assert!(pair.contains("leselang_command::validate_successor_pair("));
    assert_complete_successor_admission_order();
    for absent in [
        "let schema_valid",
        "first.fuel_remaining <=",
        "validate_extension(",
        "prior.validate(",
    ] {
        assert!(!pair.contains(absent), "duplicated {absent}");
    }
    assert!(pair.contains("SuccessorCorrespondenceError::Mismatch => invalid()"));
    assert!(pair.contains("SuccessorCorrespondenceError::InvalidBinding"));
    assert!(pair.contains("crate::result_binding::invalid()"));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/successor_pair_contract_tests.rs"))
            .unwrap();
    assert!(frozen.contains("assert_eq!(pairs.len(), 9)"));
    assert!(frozen.contains("assert_eq!(comparisons, 576)"));
    assert!(!frozen.contains("validate_preflighted_successor_pair"));
    assert!(!frozen.contains("has_same_inherited_context"));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    for proof in [
        "source_successor_pairs_share_wire_reentry_without_extra_fuel_or_preparation",
        "successor_pair_refuses_torn_rows_without_mutating_original_frames",
        "successor_pair_borrows_opaque_local_tokens_and_preserves_hook_order_and_unwind",
    ] {
        assert!(consumer.contains(proof));
    }
}

#[test]
fn group_requests_share_borrowed_correspondence_without_replacing_recovery_admission() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/group_requests.rs")).unwrap();
    for required in [
        "pub struct GroupRequestContext",
        "pub struct InvalidGroupRequests",
        "pub fn validate_preflighted_group_requests",
        "Fn(&'a Request) -> EffectRequestView<'a, Token>",
        "pub fn preflighted_capture_accepts_type_at",
        "validate_result_binding(",
        "saved.group.matches_signature(branches)",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "leselang_vm::",
        "Fuel::",
        "serde_json",
        ".clone()",
        ".collect::<",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    let admission = vm
        .split_once("impl ReferenceGroupResultBinding")
        .unwrap()
        .1
        .split_once("fn validate_requests(")
        .unwrap()
        .1
        .split_once("fn validate_member(")
        .unwrap()
        .0;
    assert!(
        admission.find("self.validate(plan)?").unwrap()
            < admission
                .find("validate_preflighted_group_requests(")
                .unwrap()
    );
    assert!(admission.contains("crate::canonical_continuation_sequence"));
    assert!(admission.contains(".map_err(|_| invalid())"));
    assert!(!admission.contains("let mut previous_fuel"));
    assert!(!vm.contains("fn capture_accepts_type_at("));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_request_contract_tests.rs"))
            .unwrap();
    assert!(frozen.contains("assert_eq!(comparisons, 2880)"));
    assert!(frozen.contains("assert_eq!(snapshots_count, 40)"));
    assert!(!frozen.contains("validate_preflighted_group_requests"));
    assert!(!frozen.contains("has_same_inherited_context"));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    assert!(consumer.contains("leselang_command::validate_merge_graph("));
    assert!(consumer.contains("verify_requests(&active_plan, &requests, &fuel, &prepared)"));
}

#[test]
fn request_frame_correspondence_is_shared_without_weakening_checked_deserialization() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/request_correspondence.rs"))
            .unwrap();
    for required in [
        "pub struct EffectRequestView<'a, Token>",
        "pub enum RequestCorrespondenceError",
        "pub fn validate_preflighted_effect_request<Token>",
        "canonical_sequence: impl FnOnce(&Token) -> Option<u64>",
        "&'a ContinuationFrame<Token, Revision, Type, Effect, ResultBinding>",
    ] {
        assert!(adapter.contains(required));
    }
    assert_eq!(adapter.matches("canonical_sequence(&").count(), 1);
    for absent in [
        "leselang_vm::",
        "Fuel::",
        "serde_json",
        ".clone()",
        "Journal",
    ] {
        assert!(!adapter.contains(absent));
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert!(vm.contains("impl<'de> Deserialize<'de> for EffectRequest"));
    assert!(vm.contains("validate_effect_request(&request)"));
    let admission = vm
        .split_once("pub fn validate_effect_request(")
        .unwrap()
        .1
        .split_once("fn generated_effect_identity_fault(")
        .unwrap()
        .0;
    assert!(admission.contains("leselang_command::validate_effect_request("));
    let complete =
        fs::read_to_string(root().join("crates/leselang-command/src/request_admission.rs"))
            .unwrap();
    let ordered = complete
        .split_once("pub fn validate_effect_request")
        .unwrap()
        .1;
    assert!(
        ordered.find("validate_continuation_frame(").unwrap()
            < ordered
                .find("validate_preflighted_effect_request(")
                .unwrap()
    );
    assert!(admission.contains("canonical_continuation_sequence,"));
    for absent in ["required_capabilities()", "format!", ".validate_request("] {
        assert!(!admission.contains(absent));
    }
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/operation_request_tests.rs"))
            .unwrap();
    assert!(!frozen.contains("validate_preflighted_effect_request"));
    assert!(frozen.contains(
        "request_correspondence_preserves_cold_authority_identity_and_budget_fault_order"
    ));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    assert!(consumer.contains("validate_effect_request("));
    assert!(!consumer.contains("validate_preflighted_effect_request("));
    assert!(consumer.contains(
        "source_atomic_requests_share_frame_correspondence_before_wire_restored_reentry"
    ));
}

#[test]
fn adapter_captured_execution_proof_reuses_shared_hir_without_vm_or_storage() {
    let source =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    let compact = source.split_whitespace().collect::<String>();
    for required in [
        "leselang_hir::lower(&leselang_syntax::parse(source))",
        "evaluate_effects_in_scope(",
        "resume_preflighted_result_binding_in_scope(",
        "PreflightedResultEnvironment::new(",
        "impl ResultExecutionHost for Adapter",
        "validate_result_binding(",
        "validate_group_capture_correspondence(",
        "validate_result_binding_extension(",
        "validate_host_result(",
        "validate_continuation_frame(",
        "impl ResultValueHost for Adapter",
        "ResultValueDomain {",
        "into_effect_value(EffectValueContext",
        "prepare_preflighted_effect_in_scope(",
        "preparation_failure(error)",
        "scalar_copy_cost",
        "0..complete.spent",
        "locals.is_empty()",
        "adapter_executes_group_aliases_captured_successors_and_scalar_tail_without_vm",
        "adapter_reentry_and_wire_restoration_use_one_meter_at_every_fuel_boundary",
        "adapter_restores_all_five_closed_vocabularies_without_upgrading_successor_captures",
        "invalid_frames_cold_fields_replies_and_revoked_admission_never_spend_or_prepare",
        "scalar_recovery_and_conditional_successors_do_not_catch_native_projection_or_fuel_failures",
        "malformed_group_receipts_are_rejected_before_scope_restore_with_retryable_original_data",
        "computed_groups_share_product_preparation_through_wire_reentry_and_every_fuel_cutoff",
        "computed_group_failures_keep_cold_preflight_and_late_failure_boundaries",
        "preparation_borrows_admitted_host_payloads_and_redacts_non_debug_native_errors",
        "source_restored_frames_refuse_torn_admission_without_repair",
    ] {
        assert!(
            compact.contains(&required.split_whitespace().collect::<String>()),
            "adapter execution proof lost {required}"
        );
    }
    for absent in [
        "leselang_vm::",
        "rusqlite::",
        "apply_binary(",
        "apply_unary(",
    ] {
        assert!(
            !source.contains(absent),
            "adapter fixture duplicated or coupled {absent}"
        );
    }
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(
        workflow
            .contains("cargo +1.98.0 test -p leselang-command --locked --test captured_execution")
    );
    let output = Command::new("cargo")
        .args([
            "tree",
            "--locked",
            "--offline",
            "-p",
            "leselang-command",
            "--edges",
            "normal,build,dev",
            "--prefix",
            "none",
        ])
        .env("CARGO_TERM_COLOR", "never")
        .current_dir(root())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let dependencies = String::from_utf8(output.stdout).unwrap();
    for absent in ["leselang-vm ", "rusqlite ", "libsqlite3-sys "] {
        assert!(
            !dependencies.lines().any(|line| line.starts_with(absent)),
            "capture proof linked {absent}"
        );
    }
    for required in [
        "leselang-command ",
        "leselang-hir ",
        "leselang-runtime-core ",
        "leselang-syntax ",
    ] {
        assert!(dependencies.lines().any(|line| line.starts_with(required)));
    }
}

#[test]
fn product_execution_environment_shares_policy_and_mechanics_without_a_new_interpreter() {
    let source =
        fs::read_to_string(root().join("crates/leselang-command/src/execution_environment.rs"))
            .unwrap();
    let compact = source.split_whitespace().collect::<String>();
    for required in [
        "pub trait ResultExecutionHost: ResultCaptureHost",
        "pub struct ResultExecutionLimits",
        "host: &'host Host",
        "Host: ResultExecutionHost + ?Sized",
        "PureEvaluationEnvironment<ResultField, HostOperation>",
        "EffectEvaluationEnvironment<'value, ResultField, HostOperation, Effect, Type>",
        "self.host.preflight_effect(expression)",
        "self.host.before_prepare_effect(expression)?",
        "self.host.prepared_effect(&effect)?",
        "self.host.before_projection(*field)?",
        "result.project_scalar(*field)",
        "group.select_member(name, *operation)",
        "prepare_preflighted_effect_in_scope(expression, scope, self, fuel, self.limits.effects)",
        "snapshot_preflighted_result_scope(",
        "self.host.valid_group_name(name)",
        "self.host.preparation_failure(failure)",
        "self.host.capture_failure(failure)",
    ] {
        assert!(
            compact.contains(&required.split_whitespace().collect::<String>()),
            "shared environment lost {required}"
        );
    }
    for absent in [
        "leselang_vm",
        "rusqlite",
        "LSV",
        "Fuel::new",
        "impl Default",
        "fuel.charge",
        "apply_binary(",
        "match expression",
        "evaluate_effects_in_scope(",
    ] {
        assert!(
            !source.contains(absent),
            "environment gained policy or interpreter {absent}"
        );
    }
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/computation.rs")).unwrap();
    let production = vm.split_once("#[cfg(test)]").unwrap().0;
    assert!(production.contains("leselang_command::PreflightedResultEnvironment::new("));
    assert!(
        production.contains("impl leselang_command::ResultExecutionHost for ReferenceCaptureHost")
    );
    for absent in [
        "struct ResultEnvironment",
        "impl PureEvaluationEnvironment",
        "EffectEvaluationEnvironment<'a",
        "prepare_preflighted_effect_in_scope(",
        "snapshot_preflighted_result_scope(",
    ] {
        assert!(
            !production.contains(absent),
            "VM still duplicates environment {absent}"
        );
    }
    let proof =
        fs::read_to_string(root().join("crates/leselang-command/tests/capture_scope.rs")).unwrap();
    for required in [
        "shared_environment_borrows_one_unsized_host_for_recursive_group_preparation",
        "shared_environment_refusals_preserve_native_ownership_and_completed_work",
        "shared_environment_projection_policy_is_not_scalar_recovery",
        "shared_environment_unwind_keeps_the_original_meter_and_cleans_local_prefix",
        "dyn ResultExecutionHost<Error = Native>",
    ] {
        assert!(proof.contains(required));
    }
}

#[test]
fn capture_scope_bridge_is_adapter_owned_with_explicit_host_policy() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/capture_scope.rs")).unwrap();
    for required in [
        "pub trait ResultCaptureHost",
        "pub struct ResultCaptureLimits",
        "pub fn snapshot_preflighted_result_scope",
        "pub fn resume_preflighted_result_binding_in_scope",
        "snapshot_captured_scope_state(",
        "resume_captured_effects_in_scope(",
        "host.charge_scalar(value, fuel)?",
        "host.charge_result(&result, fuel)?",
        "host.charge_group(&group, fuel)?",
        "host.allow_saved_projection()?",
        "drop(validate_group_name)",
        "ResultView::Projected(result)",
        "ResultView::ProjectedGroup(group)",
    ] {
        assert!(adapter.contains(required), "capture bridge lost {required}");
    }
    for absent in [
        "leselang_vm",
        "LSV",
        "Journal",
        "Fuel::new",
        "impl Default",
        "Principal",
        "scalar_copy_cost",
        "fuel.charge",
        "apply_binary(",
    ] {
        assert!(
            !adapter.contains(absent),
            "capture bridge acquired host policy {absent}"
        );
    }
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/computation.rs")).unwrap();
    let production = vm.split_once("#[cfg(test)]").unwrap().0;
    for absent in [
        "CaptureSnapshotHooks",
        "CapturedEffectHooks",
        "snapshot_captured_scope_state(",
        "resume_captured_effects_in_scope(",
        "CapturedValue::",
    ] {
        assert!(
            !production.contains(absent),
            "VM duplicated capture bridge {absent}"
        );
    }
    for required in [
        "impl leselang_command::ResultCaptureHost",
        "charge(fuel, 1 + string_cost(value))",
        "if self.restore_saved_results",
        "view_fault(error)",
    ] {
        assert!(
            production.contains(required),
            "VM lost native policy {required}"
        );
    }
    let proof =
        fs::read_to_string(root().join("crates/leselang-command/tests/capture_scope.rs")).unwrap();
    for required in [
        "snapshot_joins_mixed_views_with_explicit_accounting_and_closed_saved_bytes",
        "snapshot_preflight_and_projection_refusal_preserve_native_charge_order",
        "reentry_borrows_original_projections_and_preserves_parent_scope_and_wire",
        "reentry_refusals_keep_row_order_native_payloads_and_duplicate_cleanup",
        "native_unwind_never_refills_retries_or_leaks_restored_rows",
        "predicate_cleanup_unwind_drops_the_locally_owned_native_error_before_handoff",
    ] {
        assert!(proof.contains(required));
    }
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test captured_execution --test capture_scope"));
}

#[test]
fn product_request_preparation_is_adapter_owned_over_one_hir_executor() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/effect_preparation.rs"))
            .unwrap();
    for required in [
        "pub enum EffectPreparationError<Native>",
        "pub fn prepare_preflighted_effect_in_scope",
        "limits: EffectEvaluationLimits",
        "evaluate_effects_in_scope(",
        ".evaluate_computed_arguments(",
        "operation.resolve(&values)",
        "Cow::Borrowed(effect)",
        "branch.result_type != operation.result_type()",
    ] {
        assert!(adapter.contains(required), "adapter lost {required}");
    }
    for absent in [
        "leselang_vm",
        "LSV",
        "Journal",
        "serde",
        "Fuel::new",
        "impl Default",
        "apply_binary(",
        "apply_unary(",
    ] {
        assert!(!adapter.contains(absent), "preparation acquired {absent}");
    }
    let source = fs::read_to_string(root().join("crates/leselang-vm/src/computation.rs")).unwrap();
    let production = source.split_once("#[cfg(test)]").unwrap().0;
    assert!(production.contains("leselang_command::PreflightedResultEnvironment::new("));
    assert!(
        production
            .contains("EffectPreparationError::Evaluation(failure) => effect_failure(failure)")
    );
    assert!(production.contains("call_failure(failure, operation)"));
    assert!(production.contains("diagnostic_code"));
    for absent in [
        ".evaluate_computed_arguments(",
        "operation.resolve(&values)",
        "resolved.push(HirBranch",
    ] {
        assert!(!production.contains(absent), "VM still owns {absent}");
    }
    let baseline =
        fs::read_to_string(root().join("crates/leselang-vm/src/effect_preparation_tests.rs"))
            .unwrap();
    assert!(
        baseline
            .contains("effect_preparation_preserves_frozen_requests_fuel_fault_order_and_scope")
    );
    assert!(baseline.contains("for grant in 0..160"));
}

#[test]
fn product_result_views_are_adapter_owned_with_vm_budget_and_fault_mapping() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/result_view.rs")).unwrap();
    for required in [
        "pub enum ResultView<'a>",
        "pub enum ResultViewError",
        "pub fn project_scalar(",
        "pub fn select_member(",
        "pub fn capture_result(",
        "pub fn capture_group(",
        "limits: GroupCaptureLimits",
        "validate_name: impl FnMut(&str) -> bool",
        "member.name == name && member.result.operation == operation",
        "Self::Projected(result) => Ok(result.clone())",
        "Self::ProjectedGroup(group) => Ok(group.clone())",
    ] {
        assert!(adapter.contains(required), "result view lost {required}");
    }
    for absent in [
        "leselang_vm",
        "LSV",
        "Fuel",
        "Journal",
        "serde",
        "MAX_MERGE_BRANCHES",
        "Principal",
        "impl Default",
    ] {
        assert!(!adapter.contains(absent), "result view acquired {absent}");
    }
    let source = fs::read_to_string(root().join("crates/leselang-vm/src/computation.rs")).unwrap();
    let production = source.split_once("#[cfg(test)]").unwrap().0;
    for required in [
        "pub(super) use leselang_command::ResultView;",
        "leselang_command::PreflightedResultEnvironment::new(",
        "ResultViewError::InvalidAccess => invalid()",
        "ResultViewError::InvalidProjection => crate::result_binding::invalid()",
        "error(\"LSV1402\"",
        "resume_preflighted_result_binding_in_scope(",
        "impl leselang_command::ResultCaptureHost",
        "charge_projection(fuel, result)",
        "charge_group_projection(fuel, group)",
    ] {
        assert!(production.contains(required), "VM lost {required}");
    }
    for absent in [
        "enum ResultView",
        "ProjectedResult::capture(",
        "ProjectedGroup::capture(",
        "member.name == name",
        "field.name == name",
        "result.field(field)",
    ] {
        assert!(!production.contains(absent), "VM duplicated {absent}");
    }
    let baseline =
        fs::read_to_string(root().join("crates/leselang-vm/src/result_view_tests.rs")).unwrap();
    assert!(
        baseline.contains(
            "result_views_preserve_frozen_projection_selection_copy_and_fault_boundaries"
        )
    );
    let matrix =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    assert!(matrix.contains(
        "adapter_result_view_lifecycle_covers_seventy_operations_and_five_saved_vocabularies"
    ));
    let tests =
        fs::read_to_string(root().join("crates/leselang-command/tests/result_view.rs")).unwrap();
    for proof in [
        "views_borrow_original_members_and_copy_only_at_explicit_projection_boundaries",
        "views_preserve_saved_bytes_and_first_match_without_claiming_admission",
        "raw_capture_policy_keeps_projection_before_names_and_propagates_native_unwind",
        "diagnostics_are_payload_free_and_legacy_fields_are_never_filled_from_raw_catalogs",
    ] {
        assert!(tests.contains(proof));
    }
}

#[test]
fn product_capture_replay_policy_is_adapter_owned_without_moving_authority_or_fault_codes() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/capture_replay.rs")).unwrap();
    for required in [
        "pub enum CaptureReplayError",
        "pub fn validate_result_binding_extension(",
        "pub fn validate_group_capture_correspondence(",
        "max_saved_bindings: usize",
        "limits: CaptureCorrespondenceLimits",
        "group_limits: GroupCaptureLimits",
        "validate_group_name: impl FnMut(&str) -> bool",
        "fields.len() != operations.len()",
    ] {
        assert!(adapter.contains(required));
    }
    for absent in [
        "leselang_vm",
        "LSV",
        "Fuel",
        "Journal",
        "serde",
        ".clone()",
        "MAX_EFFECT_NESTING_DEPTH",
        "MAX_MERGE_BRANCHES",
        "Continuation",
        "Principal",
    ] {
        assert!(
            !adapter.contains(absent),
            "capture adapter acquired {absent}"
        );
    }
    let successor = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    let successor = successor
        .split_once("#[cfg(test)]\nmod extension_tests {")
        .unwrap()
        .0;
    assert!(!successor.contains("impl CaptureExtensionDomain"));
    assert!(!successor.contains("validate_capture_extension("));
    assert!(successor.contains("leselang_command::validate_successor_pair("));
    assert_complete_successor_admission_order();
    let correspondence =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_correspondence.rs"))
            .unwrap();
    assert!(correspondence.contains("validate_result_binding_extension("));
    assert!(successor.contains("crate::result_binding::invalid()"));
    assert!(successor.contains("code: \"LSV1407\""));
    let group = fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    let group = group.split_once("#[cfg(all(test").unwrap().0;
    assert!(!group.contains("validate_capture_correspondence("));
    assert!(group.contains("leselang_command::validate_group_capture_correspondence("));
    assert!(group.contains("crate::result_binding::invalid()"));
    assert!(group.contains("code: \"LSV1409\""));
    let proofs =
        fs::read_to_string(root().join("crates/leselang-command/tests/capture_replay.rs")).unwrap();
    for proof in [
        "extension_keeps_aliases_named_retention_and_distinct_late_projection_errors",
        "correspondence_keeps_ordered_groups_full_row_scans_and_error_categories",
        "caller_limits_and_row_shapes_precede_callbacks_without_implicit_vm_defaults",
        "name_callback_refusal_and_unwind_keep_prefix_order_without_mutating_captures",
    ] {
        assert!(proofs.contains(proof));
    }
}

#[test]
fn complete_product_binding_validation_is_adapter_owned_with_explicit_vm_limits() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/result_binding.rs")).unwrap();
    for required in [
        "pub type ResultBinding =",
        "pub struct ResultBindingLimits",
        "pub fn validate_result_binding_structure(",
        "pub fn validate_result_binding(",
        "captures: CaptureValidationLimits",
        "groups: GroupCaptureLimits",
        "accesses: ProjectionAccessLimits",
        "validate_group_name: impl FnMut(&str) -> bool",
        "validate_captured_bindings(",
        "validate_captured_projection_accesses(",
        "validate_in_group_scope(&scope, &groups)",
        "validate_in_scope(&[])",
        "binding.locals.iter().rev()",
    ] {
        assert!(
            adapter.contains(required),
            "binding adapter lost {required}"
        );
    }
    for absent in [
        "leselang_vm",
        "LSV",
        "Fuel",
        "Journal",
        "CONTINUATION_SCHEMA_VERSION",
        "impl Default",
        "MAX_MERGE_BRANCHES",
    ] {
        assert!(
            !adapter.contains(absent),
            "binding adapter acquired {absent}"
        );
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/result_binding.rs")).unwrap();
    let production = reference.split_once("#[cfg(test)]\nmod tests {").unwrap().0;
    for required in [
        "pub use leselang_command::{",
        "ResultBinding,",
        "fn schema_version(&self)",
        "leselang_command::continuation_schema::preflighted_result_binding_schema_version(self)",
        "leselang_command::validate_result_binding(",
        "leselang_command::validate_result_binding_structure(",
        "result_binding_limits()",
        "crate::valid_merge_branch_name",
        ".map_err(projection_fault)",
    ] {
        assert!(production.contains(required));
    }
    for absent in [
        "pub type ResultBinding",
        "validate_captured_bindings(",
        "GroupLocalType",
        "validate_in_group_scope(",
        "validate_in_scope(",
        "Computation::Bind",
        "self.body.clone()",
    ] {
        assert!(!production.contains(absent), "VM duplicated {absent}");
    }
    assert!(
        reference
            .contains("restored_binding_keeps_scalar_result_group_types_and_canonical_refusals")
    );
    let proofs =
        fs::read_to_string(root().join("crates/leselang-command/tests/result_binding.rs")).unwrap();
    for proof in [
        "complete_validation_keeps_scalar_literal_scope_and_typed_saved_scope_paths",
        "complete_validation_rejects_legacy_widening_names_and_forged_group_types",
        "explicit_limits_and_ordered_name_hooks_precede_access_and_canonical_work",
    ] {
        assert!(proofs.contains(proof));
    }
}

#[test]
fn group_plan_correspondence_is_adapter_owned_between_structure_and_deep_vm_gates() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/group_plan.rs")).unwrap();
    for required in [
        "pub struct GroupPlanLimits",
        "pub struct InvalidGroupPlan",
        "pub fn plan_branches(",
        "pub fn plan_successor_name_at(",
        "pub fn validate_preflighted_plan(",
        "limits.max_branches",
        "limits.max_effect_sequence",
        "limits.max_fuel",
        "HostOperation::for_effect(&branch.effect)",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for forbidden in [
        "leselang_vm",
        "Journal",
        "EffectRequest",
        "Fuel::",
        "LSV",
        "MAX_MERGE_BRANCHES",
        "MAX_EFFECT_SEQUENCE",
        "MAX_EXECUTION_FUEL",
        "impl Default",
        "serde",
        ".clone()",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    let implementation = reference
        .split_once("impl ReferenceGroupResultBinding for GroupResultBinding {")
        .unwrap()
        .1;
    let validate = implementation
        .split_once("fn validate(")
        .unwrap()
        .1
        .split_once("fn validate_requests(")
        .unwrap()
        .0;
    let mut offset = 0;
    for stage in [
        "leselang_command::validate_group_binding(",
        "admission_limits()",
        "crate::valid_merge_branch_name",
        "validate_plan_encoding",
        ".map_err(admission_fault)",
    ] {
        offset += validate[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out-of-order {stage}"))
            + stage.len();
    }
    for helper in [
        "max_branches: crate::MAX_MERGE_BRANCHES",
        "max_effect_sequence: crate::MAX_EFFECT_SEQUENCE",
        "max_fuel: crate::MAX_EXECUTION_FUEL",
        "binding: crate::result_binding::result_binding_limits()",
        "crate::validate_json_size_capped(",
        "GroupBindingError::Binding => crate::result_binding::invalid()",
        "GroupBindingError::Plan => invalid()",
        "GroupBindingError::Encoding(fault) => fault",
    ] {
        assert!(reference.contains(helper), "{helper}");
    }
    for forbidden in [
        "HostOperation::",
        "atomic_flow_bound()",
        "reserved_sequences()",
        "ExecutionOrder::",
    ] {
        assert!(
            !validate.contains(forbidden),
            "duplicated policy: {forbidden}"
        );
    }
    assert!(
        implementation.contains("self.plan_successor_name_at(position, crate::MAX_MERGE_BRANCHES)")
    );
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_plan_contract_tests.rs"))
            .unwrap();
    assert!(frozen.contains("assert_eq!(comparisons, 3360)"));
    assert!(!frozen.contains("validate_preflighted_plan"));
    assert!(!frozen.contains("plan_successor_name_at"));
    let vertical =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    assert!(vertical.contains(
        "fn adapter_saved_group_plan_gates_source_reentry_without_vm_or_resource_changes()"
    ));
    assert!(vertical.contains("leselang_command::validate_merge_plan("));
    assert!(vertical.contains("leselang_command::validate_merge_graph("));
    assert!(!vertical.contains("leselang_vm"));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("GroupPlanLimits"));
    assert!(docs.contains("group-plan correspondence is not recovery admission"));
}

#[test]
fn complete_group_binding_admission_is_shared_with_explicit_native_encoding_policy() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/group_admission.rs")).unwrap();
    for required in [
        "pub struct GroupBindingLimits",
        "pub enum GroupBindingError<Encoding>",
        "pub fn validate_group_binding<Encoding>",
        "validate_encoding: impl FnOnce(&MergePlan) -> Result<(), Encoding>",
        "mut validate_group_name: impl FnMut(&str) -> bool",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    let entry = adapter
        .split_once("pub fn validate_group_binding<Encoding>")
        .unwrap()
        .1;
    let mut offset = 0;
    for stage in [
        "validate_result_binding_structure(",
        ".map_err(|_| GroupBindingError::Binding)?",
        ".validate_preflighted_plan(plan, limits.plan)",
        "validate_encoding(plan).map_err(GroupBindingError::Encoding)?",
        "let ty = validate_result_binding(",
        "!matches!(ty, Type::Scalar(_))",
    ] {
        offset += entry[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out-of-order {stage}"))
            + stage.len();
    }
    for forbidden in [
        "leselang_vm",
        "Journal",
        "EffectRequest",
        "Fuel::",
        "MAX_",
        "serde",
        ".clone()",
        "impl Default",
        "LSV",
    ] {
        assert!(!adapter.contains(forbidden), "{forbidden}");
    }
    let vertical =
        fs::read_to_string(root().join("crates/leselang-command/tests/captured_execution.rs"))
            .unwrap();
    let admit = vertical
        .split_once("fn admit(plan: &MergePlan)")
        .unwrap()
        .1
        .split_once("for mode in")
        .unwrap()
        .0;
    assert!(admit.contains("leselang_command::validate_merge_plan("));
    assert!(admit.contains("plan_policy()"));
    assert!(!admit.contains("validate_result_binding_structure("));
    assert!(!admit.contains("validate_result_binding("));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    assert!(docs.contains("GroupBindingLimits"));
    assert!(docs.contains("native encoding errors move unchanged"));
}

#[test]
fn structured_plan_protocol_and_shape_admission_are_adapter_owned_with_deep_vm_gates() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/merge_plan.rs")).unwrap();
    for required in [
        "pub struct MergePlan",
        "pub enum ExecutionOrder",
        "pub struct MergePlanShapeLimits",
        "pub enum MergePlanShapeError",
        "pub fn validate_shape(",
        "pub fn valid_merge_branch_name(",
        "pub fn new(",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm",
        "Journal",
        "EffectRequest",
        "Fuel::",
        "binding.validate(",
        "validate_merge_plan(",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let constructor = adapter
        .split_once("pub fn new(")
        .unwrap()
        .1
        .split_once("/// Explicit shape policy")
        .unwrap()
        .0;
    assert!(
        constructor.find(".collect()").unwrap() < constructor.find("plan.validate_shape(").unwrap()
    );
    assert!(constructor.contains("code: \"LSV2401\""));
    let shape = adapter.split_once("pub fn validate_shape(").unwrap().1;
    assert!(!shape.contains("LSV"));
    assert!(!shape.contains("MAX_MERGE_BRANCHES"));
    assert!(!shape.contains("MAX_MERGE_BRANCH_NAME_BYTES"));
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    for absent in [
        "pub struct MergePlan",
        "pub enum ExecutionOrder",
        "impl MergePlan",
        "pub const MAX_MERGE_BRANCHES",
        "pub const MAX_MERGE_BRANCH_NAME_BYTES",
    ] {
        assert!(!vm.contains(absent), "VM duplicated {absent}");
    }
    let validation = vm
        .split_once("pub(crate) fn validate_merge_plan(")
        .unwrap()
        .1
        .split_once("fn valid_merge_branch_name(")
        .unwrap()
        .0;
    let mut offset = 0;
    for stage in [
        "leselang_command::validate_merge_plan(",
        "merge_plan_admission_limits()",
        "group_binding::validate_plan_encoding",
        ".map_err(merge_plan_admission_fault)",
        "max_branches: MAX_MERGE_BRANCHES",
        "max_name_bytes: MAX_MERGE_BRANCH_NAME_BYTES",
        "MergePlanShapeError::GroupBinding => group_binding::invalid()",
        "MergePlanShapeError::Successor => successor::invalid()",
        "code: \"LSV2401\"",
    ] {
        offset += validation[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out-of-order {stage}"))
            + stage.len();
    }
    for absent in ["HashSet", "format!", "ExecutionOrder::", "minimum"] {
        assert!(!validation.contains(absent));
    }
    assert!(
        vm.contains("leselang_command::valid_merge_branch_name(name, MAX_MERGE_BRANCH_NAME_BYTES)")
    );
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/merge_plan_contract_tests.rs"))
            .unwrap();
    assert!(frozen.contains("struct FrozenMergePlan"));
    assert!(frozen.contains("enum FrozenExecutionOrder"));
    assert!(frozen.contains("assert_eq!(comparisons, 1120)"));
    assert!(!frozen.contains("validate_shape"));
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test result_binding --test merge_plan"));
}

#[test]
fn group_continuation_data_and_successor_versions_are_product_owned_not_vm_owned() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/group_continuation.rs"))
            .unwrap();
    for required in [
        "pub struct GroupResultBinding",
        "#[serde(deny_unknown_fields)]",
        "pub fn is_conditional(",
        "pub fn is_dataflow(",
        "pub fn captures_successor(",
        "pub fn reserved_sequences(",
        "pub fn continuation_schema(",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm",
        "Journal",
        "Fuel::",
        "LSV",
        "MAX_EFFECT_SEQUENCE",
        "MAX_EXECUTION_FUEL",
        "EffectRequest",
        "MergePlan",
        ".clone()",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    assert!(reference.contains("pub use leselang_command::GroupResultBinding;"));
    assert!(reference.contains("pub(super) trait ReferenceGroupResultBinding"));
    assert!(reference.contains("impl ReferenceGroupResultBinding for GroupResultBinding"));
    assert!(!reference.contains("pub struct GroupResultBinding"));
    let production = reference.split_once("#[cfg(all(test").unwrap().0;
    for required in [
        "leselang_command::validate_group_binding(",
        "crate::MAX_EXECUTION_FUEL",
        "crate::MAX_EFFECT_SEQUENCE",
        "self.validate_saved_capture(request, prefix, &operations)?",
        "crate::successor::validate_pair(",
    ] {
        assert!(production.contains(required), "{required}");
    }
    assert!(reference.contains("#[cfg(test)]\n    fn validate_requests("));
    let shared_graph =
        fs::read_to_string(root().join("crates/leselang-command/src/merge_admission.rs")).unwrap();
    assert!(shared_graph.contains("validate_preflighted_group_requests("));
    for absent in [
        "fn is_conditional(",
        "fn is_dataflow(",
        "fn captures_successor(",
        "fn continuation_schema(",
    ] {
        assert!(!production.contains(absent), "VM duplicated {absent}");
    }
    let successor = fs::read_to_string(root().join("crates/leselang-vm/src/successor.rs")).unwrap();
    let pair = successor
        .split_once("pub(super) fn validate_pair(")
        .unwrap()
        .1
        .split_once("enum PreparedResult")
        .unwrap()
        .0;
    assert!(pair.contains("leselang_command::validate_successor_pair("));
    assert_complete_successor_admission_order();
    let shared =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_correspondence.rs"))
            .unwrap();
    let mut offset = 0;
    for stage in [
        "let schema_valid = successor_schema_matches(",
        "if !schema_valid",
        "parent.has_same_inherited_context(&next)",
        "first.fuel_remaining <= second.fuel_remaining",
        "canonical_sequence(&first.token)",
        "validate_result_binding_extension(",
    ] {
        offset += shared[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("missing/out-of-order {stage}"))
            + stage.len();
    }
    assert!(!pair.contains("match first.schema_version"));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_protocol_tests.rs")).unwrap();
    assert!(frozen.contains("struct FrozenGroupResultBinding"));
    assert!(frozen.contains("assert_eq!(comparisons, 288)"));
    assert!(!frozen.contains("leselang_command"));
}

#[test]
fn continuation_schema_policy_is_adapter_owned_after_structure_preflight() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/continuation_schema.rs"))
            .unwrap();
    for required in [
        "pub enum ContinuationSchemaError",
        "pub fn preflighted_result_binding_schema_version(",
        "pub fn validate_preflighted_continuation_schema(",
        "max_sequence: u64",
        "binding: Option<&ResultBinding>",
        "GROUP_CONDITIONAL_CONTINUATION_SCHEMA_VERSION: u32 = 11",
    ] {
        assert!(adapter.contains(required), "{required}");
    }
    for absent in [
        "leselang_vm",
        "Journal",
        "LSV",
        "MAX_EFFECT_SEQUENCE",
        "impl Default",
        "pub struct ContinuationImage",
    ] {
        assert!(!adapter.contains(absent), "{absent}");
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert!(reference.contains("pub use leselang_command::continuation_schema::{"));
    assert!(!reference.contains("pub const CONTINUATION_SCHEMA_VERSION"));
    let validation = reference
        .split_once("fn validate_image(")
        .unwrap()
        .1
        .split_once("pub fn validate_effect_request(")
        .unwrap()
        .0;
    let admission =
        fs::read_to_string(root().join("crates/leselang-command/src/continuation_admission.rs"))
            .unwrap();
    let ordered = admission
        .split_once("pub fn validate_continuation_frame")
        .unwrap()
        .1;
    let mut offset = 0;
    for step in [
        "validate_result_binding_structure(",
        "validate_preflighted_continuation_schema(",
        "image.result_binding.as_deref()",
        "image.group_result.as_ref().map(group_owner_text)",
        "validate_token(&image.token)",
        "image.program_counter != 1",
        "HostOperation::for_effect(",
        "validate_result_binding(",
        ".resource_budget()",
    ] {
        offset += ordered[offset..]
            .find(step)
            .unwrap_or_else(|| panic!("missing/out-of-order {step}"))
            + step.len();
    }
    for required in [
        "leselang_command::validate_continuation_frame(",
        "result_binding::result_binding_limits()",
        "max_group_sequence: MAX_EFFECT_SEQUENCE",
        "ContinuationToken::as_str",
        "valid_continuation_token,",
        "valid_merge_branch_name,",
        "group_binding::invalid()",
        "ContinuationSchemaError::Version { .. })",
        "\"LSV2001\"",
    ] {
        assert!(validation.contains(required), "reference lost {required}");
    }
    for absent in [
        "strip_prefix(\"merge-\")",
        "binding.schema_version()",
        "binding.body.is_pure()",
        "expected_version",
        "image.program_counter != 1",
        "HostOperation::for_effect(",
        "image.result_type !=",
    ] {
        assert!(!validation.contains(absent), "VM duplicated {absent}");
    }
    let restore = reference
        .split_once("fn validate_restorable_image(")
        .unwrap()
        .1
        .split_once("fn validate_continuation_encoding_size(")
        .unwrap()
        .0;
    let stages = [
        "validate_image(image)?",
        "canonical_continuation_sequence(",
        "validate_pending_effect_contract(image)",
    ]
    .map(|stage| restore.find(stage).unwrap());
    assert!(stages.windows(2).all(|pair| pair[0] < pair[1]));
    let proofs =
        fs::read_to_string(root().join("crates/leselang-vm/src/continuation_schema_tests.rs"))
            .unwrap();
    assert!(
        proofs.contains("continuation_schema_preserves_frozen_selection_and_frame_refusal_order")
    );
    assert!(proofs.contains("continuation_structure_precedes_schema_even_for_oversized_body"));
    assert!(proofs.contains("22_848"));
    assert!(
        proofs.contains(
            "frame_admission_preserves_all_operations_budget_edges_and_conflicting_faults"
        )
    );
    assert!(proofs.contains("1_260"));
    assert!(!proofs.contains("leselang_command::continuation_schema"));
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test host_protocol --test result_binding"));
    assert!(workflow.contains("--test continuation_admission"));
    for absent in [
        "leselang_vm",
        "serde_json",
        "Journal",
        "Fuel::",
        "impl Default",
        "LSV",
    ] {
        assert!(
            !admission.contains(absent),
            "shared admission grew {absent}"
        );
    }
}

#[test]
fn product_capture_access_policy_is_adapter_owned_without_bypassing_vm_admission() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/projection_access.rs"))
            .unwrap();
    for required in [
        "pub fn validate_captured_projection_accesses(",
        "limits: ProjectionAccessLimits",
        "with_captured_scope(",
        "observe_projection_accesses(",
        "left & right",
        "self.call(operation)",
        "member.result.operation == *operation",
    ] {
        assert!(
            adapter.contains(required),
            "product access policy lost {required}"
        );
    }
    for absent in [
        "leselang_vm",
        "LSV",
        "Fuel",
        "Journal",
        "serde",
        ".clone()",
        "MAX_EFFECT_NESTING_DEPTH",
        "MAX_COMPUTATION_NODES",
    ] {
        assert!(
            !adapter.contains(absent),
            "product access policy acquired {absent}"
        );
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/result_binding.rs")).unwrap();
    let production = reference.split_once("#[cfg(test)]\nmod tests {").unwrap().0;
    let binding =
        fs::read_to_string(root().join("crates/leselang-command/src/result_binding.rs")).unwrap();
    assert!(production.contains("leselang_command::validate_result_binding("));
    let composed = binding
        .split_once("pub fn validate_result_binding(")
        .unwrap()
        .1;
    let delegated = composed
        .find("validate_captured_projection_accesses(")
        .unwrap();
    assert!(composed.find("validate_result_binding_structure(").unwrap() < delegated);
    assert!(
        delegated
            < composed
                .find("validate_canonical_binding_type(binding, pending)")
                .unwrap()
    );
    for retained in [
        "max_nodes: leselang_hir::computation::MAX_COMPUTATION_NODES",
        "max_depth: MAX_EFFECT_NESTING_DEPTH",
        ".map_err(projection_fault)",
        "LSV1405",
    ] {
        assert!(production.contains(retained));
    }
    for absent in [
        "fn field_bit(",
        "fn operation_fields(",
        "AvailableProjection",
        "ProjectionAccessPolicy",
        "with_captured_scope(",
        "fn validate_projected_accesses(",
    ] {
        assert!(!production.contains(absent), "VM duplicated {absent}");
    }
    let proofs =
        fs::read_to_string(root().join("crates/leselang-command/tests/projection_access.rs"))
            .unwrap();
    for proof in [
        "closed_fields_are_checked_in_cold_recovery_loops_folds_and_call_arguments",
        "capture_names_share_one_namespace_and_lexical_siblings_do_not_leak",
        "saved_members_require_exact_names_and_operations_but_fallback_is_not_type_admission",
        "explicit_physical_walk_limits_are_inclusive_and_cover_unselected_branches",
    ] {
        assert!(proofs.contains(proof));
    }
    let vm_proofs =
        fs::read_to_string(root().join("crates/leselang-vm/tests/projection_scope.rs")).unwrap();
    assert!(
        vm_proofs
            .contains("adapter_access_success_never_bypasses_vm_schema_or_canonical_group_checks")
    );
}

#[test]
fn product_group_capture_contract_is_adapter_owned_with_explicit_vm_policy() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/projected_group.rs")).unwrap();
    assert!(adapter.contains("impl ProjectedGroupExt for ProjectedGroup"));
    assert!(adapter.contains("limits: GroupCaptureLimits"));
    assert!(adapter.contains("validate_name: impl FnMut(&str) -> bool"));
    assert!(adapter.contains(
        "validate_projected_group(self, limits, validate_name, ProjectedResult::validate)"
    ));
    let matching = adapter.rsplit_once("fn matches_members(").unwrap().1;
    assert!(
        matching
            .find("self.validate(limits, validate_name)?")
            .unwrap()
            < matching.find("self.matches_signature(branches)").unwrap()
    );
    for absent in [
        "leselang_vm",
        "MAX_MERGE",
        "Fuel",
        "LSV",
        "Journal",
        "serde",
        "pub struct ProjectedGroup",
    ] {
        assert!(!adapter.contains(absent), "adapter acquired {absent}");
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/result_binding.rs")).unwrap();
    let production = reference.split_once("#[cfg(test)]\nmod tests {").unwrap().0;
    assert!(production.contains(
        "#[cfg(test)]\npub(super) use leselang_command::ProjectedGroupExt as ReferenceProjectedGroup;"
    ));
    assert!(production.contains("max_members: crate::MAX_MERGE_BRANCHES - 1"));
    assert!(production.contains("min_members: 1"));
    assert!(production.contains("crate::valid_merge_branch_name"));
    assert!(!production.contains("impl ReferenceProjectedGroup"));
    assert!(!production.contains("fn matches_members("));
    assert!(!production.contains("fn matches_signature("));
    assert!(reference.contains("group_capture_preserves_original_limits_names_and_refusal_order"));
    assert!(
        reference.contains(
            "group_capture_keeps_legacy_versions_order_and_projection_only_correspondence"
        )
    );
    let cases =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    assert!(cases.contains(
        "group_captures_cover_seventy_operations_with_mixed_versions_and_exact_member_order"
    ));
    assert!(cases.contains(
        "group_capture_policy_is_explicit_and_name_hooks_keep_original_validation_order"
    ));
}

#[test]
fn product_scalar_projection_is_adapter_owned_without_moving_execution_policy() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/scalar_projection.rs"))
            .unwrap();
    assert!(
        adapter.contains("pub fn project_scalar(&self, field: ResultField) -> Option<ScalarValue>")
    );
    for absent in [
        "serde_json",
        "Fault",
        "Fuel",
        "Journal",
        "ProjectedResult",
        "projection_version",
        "leselang_vm",
    ] {
        assert!(!adapter.contains(absent), "adapter acquired {absent}");
    }
    let binding =
        fs::read_to_string(root().join("crates/leselang-vm/src/result_binding.rs")).unwrap();
    assert!(!binding.contains("pub(super) fn project("));
    let view =
        fs::read_to_string(root().join("crates/leselang-command/src/result_view.rs")).unwrap();
    assert!(view.contains(".project_scalar(field)"));
    assert!(view.contains(".ok_or(ResultViewError::InvalidProjection)"));
    for retained in [
        "leselang_command::validate_result_binding_structure(",
        ".map_err(projection_fault)",
        "code: \"LSV1405\"",
    ] {
        assert!(
            binding.contains(retained),
            "VM lost capture policy: {retained}"
        );
    }
    let computation =
        fs::read_to_string(root().join("crates/leselang-vm/src/computation.rs")).unwrap();
    assert!(computation.contains("self.project_scalar(field).map_err(view_fault)"));
    assert!(
        computation.contains("fn charge_projection(fuel: &mut Fuel, result: &ProjectedResult)")
    );
    for test in [
        "scalar_projection_keeps_five_capture_versions_and_frozen_field_wire",
        "scalar_projection_keeps_invalid_frame_fault_and_capture_correspondence",
    ] {
        assert!(binding.contains(test));
    }
    let cases =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    assert!(
        cases.contains(
            "scalar_projection_covers_all_seventy_seven_types_and_twelve_explicit_fields"
        )
    );
    assert!(cases.contains("scalar_projection_is_not_bounds_validation_or_execution_admission"));
}

#[test]
fn product_capture_frames_and_validation_live_in_adapter_with_exact_reference_reexports() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/projected_result.rs")).unwrap();
    for required in [
        "pub struct ProjectedResult",
        "pub struct InvalidProjectedResult",
        "pub type ProjectedField = ScalarProjectionField<ResultField>",
        "validate_scalar_projection(&self.fields, expected)",
        "projection_fields(self.projection_version)",
        "valid_kind_token(self.operation, &stored.value)",
        "default = \"legacy_projection_version\"",
        "skip_serializing_if = \"is_legacy_projection\"",
    ] {
        assert!(adapter.contains(required), "adapter lost {required}");
    }
    for absent in [
        "leselang_vm",
        "Fault",
        "LSV",
        "Journal",
        "Fuel",
        "rusqlite",
        "ContinuationImage",
    ] {
        assert!(
            !adapter.contains(absent),
            "product capture acquired execution policy: {absent}"
        );
    }
    let reference =
        fs::read_to_string(root().join("crates/leselang-vm/src/result_binding.rs")).unwrap();
    assert!(reference.contains("pub use leselang_command::{"));
    let production = reference.split_once("#[cfg(test)]\nmod tests {").unwrap().0;
    for absent in [
        "pub struct ProjectedResult",
        "impl ProjectedResult",
        "fn projection_fields",
        "fn valid_kind_token",
        "serde",
        "UiSemantic",
        "UiFormInputKind",
    ] {
        assert!(!production.contains(absent), "VM retained {absent}");
    }
    assert!(
        reference
            .contains("projection_fault(_: leselang_command::InvalidProjectedResult) -> Fault")
    );
    assert!(reference.contains("leselang_command::validate_result_binding_structure("));
    assert!(reference.contains("leselang_command::validate_result_binding("));
    let cases =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    assert!(cases.contains(
        "product_captures_preserve_seventy_operations_five_versions_and_exact_wire_order"
    ));
    assert!(
        cases.contains(
            "product_capture_shape_never_certifies_raw_identity_or_trusted_restore_version"
        )
    );
    let proof =
        fs::read_to_string(root().join("crates/leselang-vm/tests/capture_contract.rs")).unwrap();
    assert!(proof.contains(
        "product_capture_reexports_are_exact_adapter_types_without_conversion_or_buffer_copies"
    ));
}

#[test]
fn continuation_type_validation_reuses_host_signatures_without_owning_product_mapping() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/continuation_admission.rs"))
            .unwrap();
    let validation = adapter
        .split_once("pub fn validate_continuation_frame")
        .unwrap()
        .1;
    assert_eq!(
        validation
            .matches("HostOperation::for_effect(&image.pending_effect)")
            .count(),
        1
    );
    assert!(validation.contains(".map(|operation| operation.result_type())"));
    for absent in [
        "Effect::",
        "Type::",
        ".clone()",
        "lower_effect(",
        "canonical_source(",
    ] {
        assert!(
            !validation.contains(absent),
            "frame validator acquired {absent}"
        );
    }
    let stages = [
        "image.program_counter != 1",
        "HostOperation::for_effect(",
        "image.result_type != expected_type",
        "validate_result_binding(",
        ".resource_budget()",
    ]
    .map(|stage| validation.find(stage).unwrap());
    assert!(stages.windows(2).all(|pair| pair[0] < pair[1]));
    let proofs =
        fs::read_to_string(root().join("crates/leselang-vm/src/continuation_contract_tests.rs"))
            .unwrap();
    for proof in [
        "continuation_signatures_keep_all_seventy_wire_and_restore_round_trips",
        "continuation_signatures_reject_every_cross_operation_and_non_host_result_type",
        "continuation_type_refusals_do_not_mutate_live_state_or_advance_identity",
        "continuation_signature_faults_keep_frame_before_type_before_budget_order",
        "continuation_signature_lookup_does_not_admit_structured_or_compute_frames",
        "continuation_signature_lookup_leaves_canonical_validation_at_the_restore_boundary",
        "continuation_signatures_reopen_all_operations_and_refuse_tampered_journal_types",
    ] {
        assert!(proofs.contains(proof));
    }
    assert!(!proofs.contains("HostOperation::"));
}

#[test]
fn control_plane_reply_identity_rules_are_adapter_owned_but_context_fences_stay_in_vm() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/control_plane_reply.rs"))
            .unwrap();
    for required in [
        "pub fn matches_control_plane_effect(",
        "saved_operation: Option<&EffectOperation>",
        "result.command_id == command.command_id",
        "result.status == CommandStatus::Applied",
        "result.observed_at_ms <= i64::MAX as u64",
    ] {
        assert!(adapter.contains(required));
    }
    for absent in [
        "use leselang_vm",
        "ContinuationImage",
        "Fuel::",
        "Journal",
        ".clone()",
        "serde_json",
        "Type::",
        "Value::",
        "LSV210",
        "max_output_items",
        "expected_revision",
        "canonical_source(",
    ] {
        assert!(
            !adapter.contains(absent),
            "borrowed reply checker acquired {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let projection = vm
        .split_once("fn atomic_step_from_effect_result(")
        .unwrap()
        .1
        .split_once("struct CappedWriter")
        .unwrap()
        .0;
    for absent in [
        "Effect::",
        "CommandStatus::",
        "command.command_id",
        "entries.iter().all",
        "result.session_id",
        "observed_at_ms <=",
    ] {
        assert!(
            !projection.contains(absent),
            "VM retained reply product predicate {absent}"
        );
    }
    assert!(projection.contains("result.into_effect_value(leselang_command::EffectValueContext"));
    let projection =
        fs::read_to_string(root().join("crates/leselang-command/src/effect_value.rs")).unwrap();
    assert_eq!(
        projection
            .matches("self.matches_control_plane_effect(context.effect, context.operation)")
            .count(),
        1
    );
    let signature = projection.find("HostOperation::for_effect(").unwrap();
    let correspondence = projection
        .find("self.matches_control_plane_effect(")
        .unwrap();
    let conversion = projection
        .find("self.into_control_plane_value(context.result_type)")
        .unwrap();
    let revision = projection.find("expected != revision").unwrap();
    let items = projection.find("count > limit").unwrap();
    assert!(
        signature < correspondence
            && correspondence < revision
            && revision < items
            && items < conversion
    );
    assert!(projection.contains("MAX_RUNTIME_LOG_QUERY_ENTRIES"));
    let proofs =
        fs::read_to_string(root().join("crates/leselang-vm/src/control_plane_reply_tests.rs"))
            .unwrap();
    for proof in [
        "control_plane_replies_freeze_cross_category_type_and_legacy_image_only_matrix",
        "control_plane_reply_fault_order_is_correspondence_then_revision_then_item_limit",
        "control_plane_reply_identity_status_and_timestamp_rules_are_preserved",
        "control_plane_replies_keep_dispatch_only_mutations_and_once_terminal_replay",
        "control_plane_replies_keep_journal_restart_and_changed_receipt_replay",
    ] {
        assert!(proofs.contains(proof));
    }
}

#[test]
fn presentation_reply_correspondence_is_borrowed_and_adapter_owned() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/presentation_reply.rs"))
            .unwrap();
    assert!(adapter.contains(
        "pub fn matches_effect(&self, effect: &Effect, operation: Option<&EffectOperation>) -> bool"
    ));
    assert_eq!(adapter.matches("Effect::Ui").count(), 62);
    for absent in [
        "use leselang_vm",
        "ContinuationImage",
        "Fuel::",
        "Journal",
        "serde_json",
        ".clone()",
        "from_effect(",
    ] {
        assert!(
            !adapter.contains(absent),
            "borrowed product checker acquired {absent}"
        );
    }
    let projection =
        fs::read_to_string(root().join("crates/leselang-command/src/effect_value.rs")).unwrap();
    assert_eq!(
        projection
            .matches("reply.matches_effect(context.effect, context.operation)")
            .count(),
        1
    );
    assert!(
        projection.find("reply.matches_effect(").unwrap()
            < projection
                .find("self.into_control_plane_value(context.result_type)")
                .unwrap()
    );
    assert!(!projection.contains("operation.is_none_or"));
    assert!(!projection.contains("PresentationEnvelope {"));
    assert!(!projection.contains("Effect::Ui"));
    assert!(!projection.contains("Type::Ui"));
    let correspondence = projection.find("reply.matches_effect(").unwrap();
    let conversion = projection.find("Value::from(reply)").unwrap();
    let type_gate = projection
        .find("value.result_type() != context.result_type")
        .unwrap();
    let done = projection.find("return Ok(value)").unwrap();
    assert!(correspondence < conversion && conversion < type_gate && type_gate < done);
    let cases =
        fs::read_to_string(root().join("crates/leselang-vm/src/presentation_request_tests.rs"))
            .unwrap();
    assert!(
        cases.contains(
            "presentation_reply_projection_keeps_legacy_restore_types_and_mismatch_faults"
        )
    );
    assert!(cases.contains("presentation_reply_projection_moves_owned_strings_without_copying"));
}

#[test]
fn released_product_values_and_exhaustive_reply_projection_are_adapter_owned() {
    let adapter = fs::read_to_string(root().join("crates/leselang-command/src/value.rs")).unwrap();
    let projection =
        fs::read_to_string(root().join("crates/leselang-command/src/presentation_value.rs"))
            .unwrap();
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    for declaration in ["pub enum Value", "pub struct StructuredField"] {
        assert!(adapter.contains(declaration));
        assert!(!vm.contains(declaration));
    }
    assert!(adapter.contains("pub fn result_type(&self) -> Type"));
    assert!(projection.contains("impl From<PresentationResult> for Value"));
    assert_eq!(projection.matches("PresentationResult::").count(), 62);
    for source in [&adapter, &projection] {
        for absent in [
            "use leselang_vm",
            "ContinuationImage",
            "Fuel::",
            "Journal",
            "serde_json",
            ".clone()",
            "_ =>",
        ] {
            assert!(
                !source.contains(absent),
                "product data mapping acquired {absent}"
            );
        }
    }
    let group = fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    assert!(
        group.contains(
            "pub(super) fn value_type(value: &Value) -> Type {\n    value.result_type()\n}"
        )
    );
    assert!(!group.contains("macro_rules! atomic_types"));
}

#[test]
fn control_plane_value_conversion_follows_vm_gates_without_owning_execution_policy() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/control_plane_value.rs"))
            .unwrap();
    assert!(
        adapter
            .contains("pub fn into_control_plane_value(self, result_type: Type) -> Option<Value>")
    );
    assert_eq!(adapter.matches("Type::").count(), 8);
    assert_eq!(adapter.matches("Box::new(").count(), 1);
    assert!(adapter.contains("_ => return None"));
    for absent in [
        ".clone()",
        "serde_json",
        "use leselang_vm",
        "ContinuationImage",
        "Fuel::",
        "Journal",
        "expected_revision",
        "max_output_items",
        "CommandStatus",
        "matches_control_plane_effect",
        "LSV210",
    ] {
        assert!(
            !adapter.contains(absent),
            "owned converter acquired {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let projection = vm
        .split_once("fn atomic_step_from_effect_result(")
        .unwrap()
        .1
        .split_once("struct CappedWriter")
        .unwrap()
        .0;
    for absent in [
        "Value::Runtime",
        "Value::DebuggerCancel",
        "Type::",
        "Box::new(",
    ] {
        assert!(
            !projection.contains(absent),
            "VM retained product conversion {absent}"
        );
    }
    let projection =
        fs::read_to_string(root().join("crates/leselang-command/src/effect_value.rs")).unwrap();
    assert_eq!(projection.matches("expected != revision").count(), 1);
    assert_eq!(projection.matches("count > limit").count(), 1);
    assert_eq!(projection.matches("into_control_plane_value(").count(), 1);
    assert!(
        projection.find("count > limit").unwrap()
            < projection.find("into_control_plane_value(").unwrap()
    );
    let proof =
        fs::read_to_string(root().join("crates/leselang-vm/src/control_plane_reply_tests.rs"))
            .unwrap();
    assert!(
        proof.contains(
            "control_plane_projection_keeps_query_revision_and_cardinality_policy_matrix"
        )
    );
    assert!(
        proof.contains(
            "control_plane_projection_moves_original_collections_strings_and_command_boxes"
        )
    );
}

#[test]
fn complete_effect_value_conversion_is_adapter_owned_with_explicit_original_context() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/effect_value.rs")).unwrap();
    for required in [
        "pub struct EffectValueContext<'a>",
        "pub enum EffectValueError",
        "pub fn into_effect_value(",
        "RevisionConflict",
        "OutputLimit",
        "MAX_RUNTIME_LOG_QUERY_ENTRIES",
        "formatter.write_str(\"EffectValueContext\")",
    ] {
        assert!(
            adapter.contains(required),
            "effect value boundary lost {required}"
        );
    }
    for absent in [
        "leselang_vm",
        "ContinuationImage",
        "Fuel",
        "Journal",
        "serde",
        ".clone()",
        "impl Default",
        "LSV",
    ] {
        assert!(
            !adapter.contains(absent),
            "effect value boundary acquired {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let boundary = vm
        .split_once("fn atomic_step_from_effect_result(")
        .unwrap()
        .1
        .split_once("struct CappedWriter")
        .unwrap()
        .0;
    for required in [
        "effect: &image.pending_effect",
        "result_type: image.result_type",
        "expected_revision: image.expected_revision",
        "max_output_items: image.max_output_items",
        "EffectValueError::Mismatch",
        "EffectValueError::RevisionConflict",
        "EffectValueError::OutputLimit",
        "LSV2101",
        "LSV2102",
        "LSV2103",
    ] {
        assert!(
            boundary.contains(required),
            "reference mapping lost {required}"
        );
    }
    for absent in [
        "matches_control_plane_effect",
        "matches_effect(",
        "query_bounds",
        "into_control_plane_value",
        "Value::from(",
        "HostOperation",
        "QueryResult",
    ] {
        assert!(!boundary.contains(absent), "VM still composes {absent}");
    }
    let proof =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    for required in [
        "composed_effect_values_cover_seventy_operations_types_and_legacy_context",
        "composed_effect_values_preserve_query_revision_count_and_mismatch_precedence",
        "composed_effect_values_move_payloads_without_granting_admission",
    ] {
        assert!(proof.contains(required));
    }
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/control_plane_reply_tests.rs"))
            .unwrap();
    assert!(frozen.contains("fn legacy_atomic_step_from_effect_result("));
    assert!(
        frozen.contains("composed_reply_conversion_matches_frozen_native_wire_and_refusal_order")
    );
}

#[test]
fn control_plane_request_correspondence_is_adapter_owned_but_identity_generation_stays_in_vm() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/control_plane_request.rs"))
            .unwrap();
    for surface in [
        "pub fn query_matches_effect(",
        "pub fn command_matches_effect(",
        "expected_command_id: &str",
        "expected_idempotency_key: &str",
        "expected_revision: Option<Revision>",
    ] {
        assert!(adapter.contains(surface));
    }
    for absent in [
        "use leselang_vm",
        "ContinuationImage",
        "Fuel::",
        "Journal",
        "serde_json",
        ".clone()",
        "format!(",
        "lower_effect(",
        "validate_effect_identity(",
    ] {
        assert!(
            !adapter.contains(absent),
            "borrowed control-plane matcher acquired {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let admission = vm
        .split_once("pub fn validate_effect_request(")
        .unwrap()
        .1
        .split_once("fn generated_effect_identity_fault(")
        .unwrap()
        .0;
    for absent in [
        "Query::",
        "Command::",
        "CommandOrigin::",
        "Confirmation::",
        "command.dry_run",
    ] {
        assert!(
            !admission.contains(absent),
            "VM admission retained product predicate {absent}"
        );
    }
    let correspondence =
        fs::read_to_string(root().join("crates/leselang-command/src/request_correspondence.rs"))
            .unwrap();
    let token = correspondence.find("canonical_sequence(&").unwrap();
    let command_id = correspondence
        .find("format!(\"leselang-command-{sequence}\")")
        .unwrap();
    let key = correspondence
        .find("format!(\"leselang-effect-{sequence}\")")
        .unwrap();
    let operation = correspondence.find(".validate_request(").unwrap();
    assert!(token < command_id && command_id < key && key < operation);
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/operation_request.rs"))
            .unwrap();
    let query = policy
        .split_once("EffectOperation::Query(query)")
        .unwrap()
        .1;
    assert!(
        query.find("validate_effect_identity(").unwrap()
            < query
                .find("query_matches_effect(&query.query, effect)")
                .unwrap()
    );
    assert_eq!(policy.matches("command_matches_effect(").count(), 3);
    for branch in policy.split("EffectOperation::Command(command)").skip(1) {
        assert!(
            branch
                .split("command_matches_effect(")
                .next()
                .unwrap()
                .contains("validate_effect_identity(")
        );
    }
    for absent in [
        "validate_effect_identity(",
        "command_matches_effect(",
        "query_matches_effect(",
    ] {
        assert!(!admission.contains(absent));
    }
    let proof =
        fs::read_to_string(root().join("crates/leselang-vm/src/control_plane_request_tests.rs"))
            .unwrap();
    for name in [
        "control_plane_admission_keeps_image_identity_authority_payload_and_budget_order",
        "control_plane_admission_preserves_query_command_category_faults",
        "control_plane_admission_preserves_command_identity_confirmation_and_log_policy",
    ] {
        assert!(proof.contains(name));
    }
}

#[test]
fn composed_operation_request_policy_is_vm_free_without_bypassing_reference_admission() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/operation_request.rs"))
            .unwrap();
    for required in [
        "pub struct OperationRequestContext",
        "pub enum OperationRequestError",
        "pub fn validate_request(",
        "pub fn authority(",
    ] {
        assert!(adapter.contains(required));
    }
    let manifest = fs::read_to_string(root().join("crates/leselang-command/Cargo.toml")).unwrap();
    for absent in ["leselang-vm", "rusqlite"] {
        assert!(!manifest.contains(absent));
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    assert!(!vm.contains("fn validate_effect_identity("));
    for mapping in [
        "RequestCorrespondenceError::Operation(OperationRequestError::Capability) => \"LSV2011\"",
        "RequestCorrespondenceError::Operation(OperationRequestError::Envelope) => \"LSV2012\"",
        "RequestCorrespondenceError::Operation(OperationRequestError::Mismatch)",
        "RequestCorrespondenceError::Budget => \"LSV2013\"",
        "validate_effect_request(&request)",
    ] {
        assert!(vm.contains(mapping));
    }
    let tests =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    for test in [
        "operation_request_admission_covers_seventy_operations_without_vm_or_wire_changes",
        "operation_request_category_precedes_authority_and_authority_precedes_payload",
        "operation_request_identity_limits_and_diagnostics_remain_closed_and_redacted",
        "operation_request_uses_supplied_command_expectations_without_inventing_identity",
        "operation_request_correspondence_is_not_source_or_live_authority_admission",
    ] {
        assert!(tests.contains(test));
    }
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/operation_request_tests.rs"))
            .unwrap();
    assert!(frozen.contains("fn frozen_validate_effect_request("));
    assert!(
        frozen.contains("operation_request_adapter_preserves_frozen_seventy_operation_admission")
    );
}

#[test]
fn presentation_request_payload_checks_are_adapter_owned_without_reordering_vm_admission() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/presentation_request.rs"))
            .unwrap();
    assert!(adapter.contains("pub fn matches_effect(&self, effect: &Effect) -> bool"));
    assert_eq!(adapter.matches("Effect::Ui").count(), 62);
    for absent in [
        "use leselang_vm",
        "ContinuationImage",
        "Fuel::",
        "Journal",
        "serde_json",
        ".clone()",
        "from_effect(",
        "_ =>",
    ] {
        assert!(
            !adapter.contains(absent),
            "borrowed request matcher acquired {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let admission = vm
        .split_once("pub fn validate_effect_request(")
        .unwrap()
        .1
        .split_once("fn generated_effect_identity_fault(")
        .unwrap()
        .0;
    assert!(!admission.contains("Effect::Ui"));
    assert!(!admission.contains("PresentationOperation::"));
    assert!(admission.contains("leselang_command::validate_effect_request("));
    let complete =
        fs::read_to_string(root().join("crates/leselang-command/src/request_admission.rs"))
            .unwrap();
    let ordered = complete
        .split_once("pub fn validate_effect_request")
        .unwrap()
        .1;
    assert!(
        ordered.find("validate_continuation_frame(").unwrap()
            < ordered
                .find("validate_preflighted_effect_request(")
                .unwrap()
    );
    let correspondence =
        fs::read_to_string(root().join("crates/leselang-command/src/request_correspondence.rs"))
            .unwrap();
    let successor = correspondence.find(".required_capabilities()").unwrap();
    let token = correspondence.find("canonical_sequence(&").unwrap();
    let identity = correspondence
        .find("request.effect_id != expected_effect_id")
        .unwrap();
    let operation = correspondence.find(".validate_request(").unwrap();
    let budget = correspondence
        .find("if *request.budget != request.continuation.resource_budget()")
        .unwrap();
    assert!(successor < token && token < identity && identity < operation && operation < budget);
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/operation_request.rs"))
            .unwrap();
    let gui = policy
        .split_once("(effect, EffectOperation::Presentation(presentation))")
        .unwrap()
        .1;
    let category = gui.find("HostOperation::for_effect(effect)").unwrap();
    let capability = gui
        .find("operation.required_capability() == CAPABILITY_UI_PRESENTATION")
        .unwrap();
    let authority = gui.find("validate_effect_identity(").unwrap();
    let payload = gui
        .find("presentation.operation.matches_effect(effect)")
        .unwrap();
    assert!(category < capability && capability < authority && authority < payload);
    assert_eq!(
        policy
            .matches("presentation.operation.matches_effect(effect)")
            .count(),
        1
    );
    for absent in [
        "ContinuationImage",
        "leselang_vm",
        "Fuel::",
        "Journal",
        ".clone()",
        "format!(",
    ] {
        assert!(!policy.contains(absent));
    }
    let cases =
        fs::read_to_string(root().join("crates/leselang-vm/src/presentation_request_tests.rs"))
            .unwrap();
    for proof in [
        "presentation_admission_keeps_image_identity_authority_payload_and_budget_order",
        "presentation_admission_does_not_apply_gui_authority_to_non_gui_effects",
    ] {
        assert!(cases.contains(proof));
    }
}

#[test]
fn product_value_bounds_are_adapter_owned_but_execution_gates_stay_in_vm() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/value_validation.rs")).unwrap();
    assert!(
        adapter
            .contains("pub fn bounded_leaf_items(&self, max_query_items: usize) -> Option<usize>")
    );
    assert_eq!(adapter.matches("Value::Ui").count(), 62);
    for absent in [
        "use leselang_vm",
        "ContinuationImage",
        "Fuel::",
        "Journal",
        "serde_json",
        ".clone()",
        "BTreeSet",
        "fn validate_value(",
    ] {
        assert!(
            !adapter.contains(absent),
            "borrowed product leaf checker acquired {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let traversal = vm
        .split_once("pub(crate) fn validate_value(")
        .unwrap()
        .1
        .split_once("fn validate_retention_policy(")
        .unwrap()
        .0;
    assert!(!traversal.contains("Value::Ui"));
    assert!(!traversal.contains("Value::Runtime"));
    for required in [
        "leselang_command::validate_value_items(",
        "max_depth: MAX_STRUCTURED_VALUE_DEPTH",
        "max_fields: MAX_MERGE_BRANCHES",
        "max_query_items: DEFAULT_MAX_OUTPUT_ITEMS",
        "ValueBoundsError::Nesting",
        "ValueBoundsError::Fields",
        "ValueBoundsError::Value",
    ] {
        assert!(
            traversal.contains(required),
            "VM lost value policy {required}"
        );
    }
    assert!(!traversal.contains("for field in fields"));
    assert!(!traversal.contains("BTreeSet"));
    let tree =
        fs::read_to_string(root().join("crates/leselang-command/src/structured_value.rs")).unwrap();
    for absent in [
        "leselang_vm",
        "LSV",
        "ContinuationImage",
        "Journal",
        "serde_json",
        ".clone()",
        "Fuel",
        "impl Default",
        "MAX_STRUCTURED_VALUE_DEPTH",
        "MAX_MERGE_BRANCHES",
        "DEFAULT_MAX_OUTPUT_ITEMS",
    ] {
        assert!(
            !tree.contains(absent),
            "value traversal acquired host policy {absent}"
        );
    }
    for required in [
        "pub struct ValueValidationLimits",
        "pub enum ValueBoundsError",
        "parents: Vec<Fields<'_>>",
        "depth > limits.max_depth",
        "fields.is_empty() || fields.len() > limits.max_fields",
        "!validate_name(&field.name) || !parent.names.insert(field.name.as_str())",
        "depth.checked_add(1)",
        ".bounded_leaf_items(limits.max_query_items)",
    ] {
        assert!(
            tree.contains(required),
            "shared value traversal lost {required}"
        );
    }
    assert_eq!(
        tree.matches("validate_value_items(").count(),
        1,
        "no recursive value traversal"
    );
    let bound =
        fs::read_to_string(root().join("crates/leselang-command/src/result_domain.rs")).unwrap();
    let leaf = bound
        .find("let count = validate_value_items(reply, 0, self.value_limits")
        .unwrap();
    let budget = bound
        .find("count > self.max_output_items || self.max_output_items == 0")
        .unwrap();
    let bytes = bound.find(".validate_encoded_value(reply)").unwrap();
    assert!(leaf < budget && budget < bytes);
    let cases =
        fs::read_to_string(root().join("crates/leselang-vm/src/presentation_request_tests.rs"))
            .unwrap();
    for proof in [
        "product_leaf_validation_keeps_counts_and_saved_value_compatibility",
        "product_leaf_validation_keeps_recursive_fault_order_and_messages",
    ] {
        assert!(cases.contains(proof));
    }
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/value_validation_tests.rs"))
            .unwrap();
    assert!(
        frozen
            .contains("value_traversal_matches_frozen_counts_fault_precedence_and_unchanged_bytes")
    );
    let tests =
        fs::read_to_string(root().join("crates/leselang-command/tests/structured_value.rs"))
            .unwrap();
    for required in [
        "declaration_order_and_local_duplicate_scopes_preserve_refusal_precedence",
        "borrowed_names_payloads_and_native_unwind_do_not_mutate_input_or_retry_hooks",
        "deep_explicit_policy_uses_no_recursive_rust_stack",
    ] {
        assert!(tests.contains(required));
    }
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test capture_scope --test structured_value"));
}

#[test]
fn received_result_domain_is_adapter_owned_with_explicit_native_policy_and_pending_ownership() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/result_domain.rs")).unwrap();
    for required in [
        "pub trait ResultValueHost",
        "pub struct ResultValueDomain<'host, Host: ?Sized>",
        "pub enum ResultValueError<Native>",
        "impl<Host: ResultValueHost + ?Sized> HostResultDomain<Value>",
        "reply.result_type() == self.result_type",
        "pub value_limits: ValueValidationLimits",
        "pub max_output_items: usize",
        "pub host: &'host Host",
        "self.host.valid_field_name(name)",
        ".map_err(ResultValueError::Encoding)",
    ] {
        assert!(
            adapter.contains(required),
            "result-domain adapter lost {required}"
        );
    }
    for absent in [
        "leselang_vm",
        "ContinuationImage",
        "Journal",
        "serde_json",
        ".clone()",
        "Fuel",
        "impl Default",
        "LSV",
        "fn authorize",
        "PendingReply::new",
    ] {
        assert!(
            !adapter.contains(absent),
            "result-domain adapter acquired {absent}"
        );
    }
    let vm = fs::read_to_string(root().join("crates/leselang-vm/src/host_result.rs")).unwrap();
    let production = vm.split_once("#[cfg(test)]").unwrap().0;
    for required in [
        "result_type: image.result_type",
        "value_limits: crate::value_validation_limits()",
        "max_output_items: image.max_output_items",
        "host: &BoundResultHost",
        "crate::valid_merge_branch_name(name)",
        "crate::validate_json_size_capped(reply, MAX_JOURNAL_ENTRY_BYTES",
        "ResultValueError::Bounds(error) => crate::value_bounds_fault(error)",
        "ResultValueError::Encoding(error) => error",
    ] {
        assert!(
            production.contains(required),
            "reference policy lost {required}"
        );
    }
    for absent in [
        "impl HostResultDomain",
        "crate::validate_value(reply",
        "items >",
        "reply.result_type()",
    ] {
        assert!(!production.contains(absent), "VM duplicated {absent}");
    }
    assert!(vm.contains("bound_value_domain_matches_frozen_fault_order_bytes_and_original_owners"));
    let proof =
        fs::read_to_string(root().join("crates/leselang-command/tests/result_domain.rs")).unwrap();
    for required in [
        "result_domain_preserves_type_tree_output_encoding_order_and_zero_limit",
        "dynamic_host_keeps_native_error_ownership_without_debug_or_clone_bounds",
        "pending_rejection_keeps_original_reply_and_frame_until_explicit_once_only_handoff",
        "native_unwind_seals_pending_state_and_scope_before_frame_cleanup",
        "scope_cancellation_wins_over_native_refusal_and_never_rearms",
    ] {
        assert!(proof.contains(required));
    }
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test structured_value --test result_domain"));
}

#[test]
fn presentation_payload_mapping_is_adapter_owned_but_vm_gates_remain_in_order() {
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/presentation.rs")).unwrap();
    assert!(adapter.contains("pub fn from_effect(effect: &Effect) -> Option<Self>"));
    for absent in [
        "use leselang_vm",
        "Fuel::new",
        "ContinuationImage",
        "Journal",
        "LoweringContext",
        "_ =>",
    ] {
        assert!(
            !adapter.contains(absent),
            "pure payload mapper acquired {absent}"
        );
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let build = vm
        .split_once("fn build_effect_request(")
        .unwrap()
        .1
        .split_once("pub fn restore(")
        .unwrap()
        .0;
    assert!(!build.contains("PresentationEnvelope {"));
    assert!(!build.contains("Effect::Ui"));
    let debit = build.find("fuel.charge(1)").unwrap();
    let image = build.find("let image = ContinuationImage").unwrap();
    let map = build
        .find("leselang_command::lower_effect_operation(")
        .unwrap();
    let validation = build.find("validate_effect_request(&request)?").unwrap();
    assert!(debit < image && image < map && map < validation);
    assert!(!build.contains("lower_effect("));
    assert!(!build.contains("origin: CommandOrigin::Leselang"));
    assert!(!build.contains("Confirmation::Confirmed"));
    assert!(!build.contains("image.clone()"));
    let lowering =
        fs::read_to_string(root().join("crates/leselang-command/src/operation_lowering.rs"))
            .unwrap();
    for required in [
        "PresentationOperation::from_effect(effect)",
        "lower_effect(",
        "origin: CommandOrigin::Leselang",
        "Confirmation::Confirmed",
        "dry_run: false",
    ] {
        assert!(lowering.contains(required));
    }
    let gui = lowering
        .find("PresentationOperation::from_effect(effect)")
        .unwrap();
    let identity = lowering.find("command_identity()").unwrap();
    let control = lowering.find("let plan = lower_effect(").unwrap();
    assert!(gui < identity && identity < control);
    let cases =
        fs::read_to_string(root().join("crates/leselang-vm/src/presentation_request_tests.rs"))
            .unwrap();
    assert!(
        cases.contains("presentation_request_mapping_keeps_frozen_bytes_and_single_budget_debit")
    );
    assert!(
        cases.contains(
            "presentation_request_refusals_keep_fuel_shape_type_and_authority_precedence"
        )
    );
}

#[test]
fn composed_operation_lowering_is_adapter_owned_without_moving_vm_identity_or_fuel() {
    let policy =
        fs::read_to_string(root().join("crates/leselang-command/src/operation_lowering.rs"))
            .unwrap();
    for absent in [
        "leselang_vm",
        "ContinuationImage",
        "Fuel",
        "Journal",
        "serde_json",
        "leselang-command-",
        "leselang-effect-",
    ] {
        assert!(!policy.contains(absent));
    }
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    let build = vm
        .split_once("fn build_effect_request(")
        .unwrap()
        .1
        .split_once("pub fn restore(")
        .unwrap()
        .0;
    for required in [
        "fuel.charge(1)",
        "CommandId::new(format!(\"leselang-command-{sequence}\"))",
        "IdempotencyKey::new(format!(\"leselang-effect-{sequence}\"))",
        "OperationLoweringError::Identity",
        "OperationLoweringError::Lowering",
        "validate_effect_request(&request)?",
    ] {
        assert!(build.contains(required));
    }
    let tests =
        fs::read_to_string(root().join("crates/leselang-command/tests/host_protocol.rs")).unwrap();
    for name in [
        "operation_lowering_composes_seventy_wire_shapes_with_lazy_identity_and_original_gui_owner",
        "operation_lowering_identity_refusal_precedes_control_plane_capability_checks",
        "operation_lowering_rejects_structured_effects_after_exactly_one_identity_attempt",
        "operation_lowering_gui_construction_is_not_authorization_or_request_admission",
        "operation_lowering_identity_factory_unwind_does_not_retry_or_retain_owned_capture",
    ] {
        assert!(tests.contains(name));
    }
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/operation_request_tests.rs"))
            .unwrap();
    assert!(frozen.contains("fn frozen_build_effect_request("));
    assert!(
        frozen.contains("operation_lowering_preserves_frozen_build_wire_fuel_and_refusal_order")
    );
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("--test result_domain --test host_protocol"));
}

#[test]
fn product_wire_consumers_do_not_import_the_language_vm_or_its_journal_backend() {
    for package in ["leselang-command", "leserpent-protocol"] {
        let output = Command::new(env!("CARGO"))
            .args([
                "tree",
                "--locked",
                "--offline",
                "-p",
                package,
                "--edges",
                "normal",
                "--prefix",
                "none",
            ])
            .current_dir(root())
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let dependencies = String::from_utf8(output.stdout).unwrap();
        assert!(dependencies.contains("leselang-command v"));
        for absent in ["leselang-vm v", "rusqlite v", "libsqlite3-sys v"] {
            assert!(
                !dependencies.contains(absent),
                "{package} still imports {absent}"
            );
        }
    }
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/host_protocol.rs")).unwrap();
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    for declaration in [
        "enum EffectOperation",
        "enum EffectResult",
        "struct PresentationEnvelope",
        "enum PresentationOperation",
        "enum PresentationResult",
        "struct DebuggerCancelResult",
    ] {
        assert!(adapter.contains(&format!("pub {declaration}")));
        assert!(!vm.contains(&format!("pub {declaration}")));
    }
    assert!(vm.contains("pub use leselang_command::{"));
    assert!(
        fs::read_to_string(root().join("crates/leserpent-protocol/src/lib.rs"))
            .unwrap()
            .contains("use leselang_command::PresentationOperation;")
    );
    let native = fs::read_to_string(root().join("crates/leselang-vm/src/native.rs")).unwrap();
    assert!(!native.contains("leselang_command"));
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
    let vm =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
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
    assert!(
        helper
            .split_whitespace()
            .collect::<String>()
            .contains("original.as_view().has_same_inherited_context(&saved.as_view())")
    );
    let shared_context =
        fs::read_to_string(root().join("crates/leselang-command/src/request_correspondence.rs"))
            .unwrap();
    let shared_context = shared_context
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    for required in [
        "validate_inherited_context(",
        "self.continuation.resource_budget()",
        "saved.continuation.resource_budget()",
        "self.operation.authority() == saved.operation.authority()",
        "self.continuation.expected_revision == saved.continuation.expected_revision",
    ] {
        assert!(
            shared_context.contains(required),
            "context adapter lost {required}"
        );
    }
    assert!(!helper.contains("Fuel::new"));
    let pair = source
        .split("pub(super) fn validate_pair(")
        .nth(1)
        .unwrap()
        .split("enum PreparedResult")
        .next()
        .unwrap();
    assert!(pair.contains("leselang_command::validate_successor_pair("));
    assert_complete_successor_admission_order();
    let pair =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_correspondence.rs"))
            .unwrap();
    let context = pair
        .find("parent.has_same_inherited_context(&next)")
        .unwrap();
    assert!(pair.find("!schema_valid").unwrap() < context);
    assert!(
        context
            < pair
                .find("first.fuel_remaining <= second.fuel_remaining")
                .unwrap()
    );
    assert!(context < pair.find("canonical_sequence(&first.token)").unwrap());
    let groups =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_binding.rs")).unwrap();
    let admission = groups
        .split_once("impl ReferenceGroupResultBinding for GroupResultBinding {")
        .unwrap()
        .1
        .split("fn validate_requests(")
        .nth(1)
        .unwrap()
        .split("fn validate_member(")
        .next()
        .unwrap();
    assert!(admission.contains("leselang_command::validate_preflighted_group_requests("));
    let admission =
        fs::read_to_string(root().join("crates/leselang-command/src/group_requests.rs")).unwrap();
    assert_eq!(
        admission
            .matches("first.has_same_inherited_context(&request)")
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
    assert!(pair.contains("leselang_command::validate_successor_pair("));
    assert_complete_successor_admission_order();
    let correspondence =
        fs::read_to_string(root().join("crates/leselang-command/src/successor_correspondence.rs"))
            .unwrap();
    let correspondence = correspondence.split_whitespace().collect::<String>();
    let extension = correspondence
        .find("validate_result_binding_extension(prior,")
        .unwrap();
    for guard in [
        "parent.has_same_inherited_context(&next)",
        "first.fuel_remaining <= second.fuel_remaining",
        "canonical_sequence(&first.token)",
        "validate_result_binding(prior, &first.pending_effect, limits, &mut validate_group_name,",
    ] {
        assert!(
            correspondence
                .find(&guard.split_whitespace().collect::<String>())
                .unwrap()
                < extension,
            "extension displaced {guard}"
        );
    }
    for retained in [
        "leselang_command::validate_result_binding_extension(",
        "leselang_hir::MAX_EFFECT_NESTING_DEPTH",
        "leselang_command::CaptureReplayError::InvalidProjection",
        "leselang_command::CaptureReplayError::Mismatch => invalid()",
        "capture_extension_preserves_frozen_native_alias_retention_and_late_fault_order",
    ] {
        assert!(source.contains(retained), "VM extension lost {retained}");
    }
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/capture_replay.rs")).unwrap();
    for retained in [
        "validate_capture_extension(",
        "old.group == saved.group",
        "old.result == saved.result",
        "saved.name == self.prior.name",
        "CaptureExtensionError::Native { error, .. } => error",
    ] {
        assert!(
            adapter.contains(retained),
            "extension adapter lost {retained}"
        );
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
        .split_once("impl ReferenceGroupResultBinding for GroupResultBinding {")
        .unwrap()
        .1
        .split("fn validate_saved_capture(")
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
        "leselang_command::validate_group_capture_correspondence(",
        "group_capture_limits()",
        "crate::valid_merge_branch_name",
        "leselang_command::CaptureReplayError::InvalidProjection",
        "leselang_command::CaptureReplayError::Mismatch => invalid()",
    ] {
        assert!(gate.contains(required), "native capture lost {required}");
    }
    assert!(
        gate.find(".ok_or_else(invalid)?").unwrap()
            < gate
                .find("leselang_command::validate_group_capture_correspondence(")
                .unwrap()
    );
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/capture_replay.rs")).unwrap();
    for retained in [
        "validate_capture_correspondence(",
        ".matches_members(",
        "&fields[..branches.len()]",
        "matches_capture(operations[original], &field.value)",
        "CaptureCorrespondenceError::Native { error, .. } => error",
    ] {
        assert!(adapter.contains(retained));
    }
    assert!(
        source
            .split_once("impl ReferenceGroupResultBinding for GroupResultBinding {")
            .unwrap()
            .1
            .split("fn advance_capture(")
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
        .split("fn invalid_group_receipt_state(")
        .next()
        .unwrap();
    assert!(
        graph.contains(
            "require_pending_sequence: plan.order.is_sequential() && state == \"pending\""
        )
    );
    assert!(graph.contains("validate_pending_sequence(self.connection, self.token, branches)"));
    assert_complete_group_recovery_order();
    let frontier = journal
        .split("\nfn validate_pending_sequence(")
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
fn complete_stored_group_recovery_is_core_owned_without_native_storage_or_product_types() {
    assert_complete_group_recovery_order();
    let core =
        fs::read_to_string(root().join("crates/leselang-runtime-core/src/group_recovery.rs"))
            .unwrap();
    for required in [
        "pub trait GroupRecoveryHost",
        "pub struct GroupRecoveryPolicy",
        "pub enum GroupRecoveryError",
        "Host: GroupRecoveryHost<Row, Key> + ?Sized",
    ] {
        assert!(core.contains(required), "{required}");
    }
    for forbidden in [
        "leselang_command",
        "leselang_vm",
        "rusqlite",
        "Fuel::",
        "LSV",
        "serde_json",
        ".clone()",
        "impl Default",
    ] {
        assert!(!core.contains(forbidden), "{forbidden}");
    }
    let reference = fs::read_to_string(root().join("crates/leselang-vm/src/journal.rs")).unwrap();
    assert!(
        reference
            .contains("impl leselang_runtime_core::GroupRecoveryHost<RecoveredMergeBranch, str>")
    );
    assert!(
        reference
            .contains("#[cfg(all(test, feature = \"sqlite-journal\"))]\nfn recover_group_receipt(")
    );
    let frozen = fs::read_to_string(
        root().join("crates/leselang-vm/src/journal/group_recovery_contract_tests.rs"),
    )
    .unwrap();
    assert!(frozen.contains("assert_eq!(comparisons, 374)"));
    assert!(frozen.contains("assert_eq!(snapshot(&tx), before)"));
    assert!(!frozen.contains("inspect_group_recovery("));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-runtime-core/tests/recovery_fence.rs"))
            .unwrap();
    for required in [
        "complete_group_recovery_preserves_state_matrix_short_circuit_and_original_receipt",
        "group_recovery_keeps_each_native_failure_owned_redacted_and_stops_before_later_gates",
        "group_recovery_unwind_at_every_hook_never_retries_or_returns_a_partial_receipt",
        "native_checkpoint_publishes_only_after_group_records_and_commit_with_distinct_native_keys",
    ] {
        assert!(consumer.contains(required), "{required}");
    }
    let exporter = fs::read_to_string(root().join("crates/gewyvern-dev/src/leselang.rs")).unwrap();
    assert!(exporter.contains("crates/leselang-runtime-core/src/group_recovery.rs"));
    assert!(exporter.contains("consumer/tests/recovery_fences.rs"));
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
        "inspect_group_recovery_batch(",
        "MAX_JOURNAL_RECORDS * crate::MAX_MERGE_BRANCHES",
        "inspect_group_recovery(",
        "validate_unbound_group_receipt(self.plan, self.branches, &terminal)",
        "validate_terminal_step(&terminal)?",
        "GroupRecoveryError::Receipt(GroupReceiptError::Native(error)) => error",
        "GroupRecoveryError::Receipt(GroupReceiptError::State) => invalid_group_receipt_state()",
        "summary.highest_sequence",
        "group_binding::validate_records(self.connection, self.token, self.plan, terminal)",
    ] {
        assert!(graph.contains(boundary), "native graph lost {boundary}");
    }
    assert!(!graph.contains("advance_sequence_watermark("));
    assert!(!graph.contains("resume_group("));
    assert_complete_group_batch_order();
    assert_complete_group_recovery_order();
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
        .split("fn admit_receipt(")
        .nth(1)
        .unwrap()
        .split("fn validate_records(")
        .next()
        .unwrap();
    assert!(
        gate.find("validate_terminal_step(&terminal)?").unwrap()
            < gate
                .find("validate_unbound_group_receipt(self.plan, self.branches, &terminal)?")
                .unwrap()
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
        actual.find("inspect_group_recovery(").unwrap()
            < actual.find("validate_unbound_group_receipt(").unwrap()
    );
    assert!(
        actual.find("validate_unbound_group_receipt(").unwrap()
            < actual.find("group_binding::validate_records(").unwrap()
    );
    assert_complete_group_recovery_order();
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
        "leselang_command::validate_merge_graph(",
        "plan: crate::merge_plan_admission_limits()",
        "frame: crate::continuation_admission_limits()",
        "|request| request.as_view()",
        "&MergeGraphHost",
        "crate::merge_plan_admission_fault(error)",
        "GroupGraphError::Native {",
        "crate::request_admission_fault(error)",
        "crate::group_binding::admission_fault(error)",
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
        "inspect_group_recovery(",
        "usize::try_from(branch.1).ok()",
        "identity: branch.4.as_str()",
        "inspect_group_recovery_batch(",
        "successor::validate_chain(self.connection, self.token)",
        "sequence_stop(terminal_steps.iter())?",
        "validate_terminal_step(&terminal)?",
        "group_binding::validate_records(self.connection, self.token, self.plan, terminal)",
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
fn complete_merge_admission_is_shared_without_importing_reference_recovery_authority() {
    let shared =
        fs::read_to_string(root().join("crates/leselang-command/src/merge_admission.rs")).unwrap();
    let plan = shared
        .split_once("pub fn validate_merge_plan<Native>")
        .unwrap()
        .1;
    assert!(
        plan.find("plan.validate_shape(").unwrap() < plan.find("validate_group_binding(").unwrap()
    );
    let graph = shared.split_once("pub fn validate_merge_graph<").unwrap().1;
    let mut offset = 0;
    for stage in [
        "validate_merge_plan(",
        "if !host.valid_token(owner)",
        "validate_group_graph(",
        "validate_effect_request(",
        "Ok(GroupGraphMember {",
        "let result_type = validate_group_binding(",
        "validate_preflighted_group_requests(",
        "view(row).continuation.group_result.is_some()",
    ] {
        offset += graph[offset..]
            .find(stage)
            .unwrap_or_else(|| panic!("lost ordered gate: {stage}"))
            + stage.len();
    }
    for forbidden in [
        "leselang_vm",
        "rusqlite",
        "Fuel::",
        "LSV",
        ".clone()",
        ".collect()",
        "serde_json",
        "impl Default",
    ] {
        assert!(!shared.contains(forbidden), "{forbidden}");
    }
    assert!(shared.contains("Host: MergeGraphAdmissionHost<Token> + ?Sized"));
    assert!(shared.contains("Token: PartialEq"));
    let frozen =
        fs::read_to_string(root().join("crates/leselang-vm/src/group_request_contract_tests.rs"))
            .unwrap();
    assert!(frozen.contains("assert_eq!(comparisons, 2_880)"));
    assert!(frozen.contains("assert_eq!(comparisons, 14)"));
    let oracle = frozen
        .split_once("fn frozen_graph(")
        .unwrap()
        .1
        .split_once("fn frozen_validate_requests(")
        .unwrap()
        .0;
    assert!(!oracle.contains("leselang_command::validate_merge_graph("));
    assert!(!oracle.contains("leselang_command::validate_merge_plan("));
    assert!(oracle.contains("frozen_validate_effect_request(request)?"));
    let consumer =
        fs::read_to_string(root().join("crates/leselang-command/tests/merge_plan.rs")).unwrap();
    for proof in [
        "complete_graph_borrows_local_opaque_rows_and_rechecks_all_ten_bound_plan_forms",
        "complete_graph_keeps_plan_owner_count_request_identity_and_correspondence_order",
        "complete_graph_preserves_both_owned_encoding_failures_and_each_unwind_prefix",
        "complete_unbound_plan_skips_encoding_and_rejects_bound_members_without_authorizing_them",
    ] {
        assert!(consumer.contains(proof));
    }
    assert!(!consumer.contains("leselang_vm"));
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
        "leselang_command::resume_preflighted_result_binding_in_scope(",
        "ReferenceCaptureHost",
        "restore_saved_results",
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
    let adapter =
        fs::read_to_string(root().join("crates/leselang-command/src/capture_scope.rs")).unwrap();
    let reentry = adapter
        .split("pub fn resume_preflighted_result_binding_in_scope")
        .nth(1)
        .unwrap();
    for boundary in [
        "resume_captured_effects_in_scope(",
        "CapturedEffectHooks {",
        "host.charge_scalar(value, fuel)?",
        "host.charge_result(result, fuel)?",
        "host.charge_group(group, fuel)?",
        "host.allow_saved_projection()?",
    ] {
        assert!(
            reentry.contains(boundary),
            "adapter reentry lost {boundary}"
        );
    }
    assert!(!reentry.contains("body:"));
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
    let code =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
    for boundary in [
        "pub type ContinuationImage = leselang_runtime_core::ContinuationFrame<",
        "pub use leselang_runtime_core::ResourceBudget;",
        "leselang_command::validate_continuation_frame(",
        "budget: ResourceBudgetLimits",
        "ContinuationAdmissionError::Budget(ResourceBudgetError::Fuel)",
        "ContinuationAdmissionError::Budget(ResourceBudgetError::AbsoluteDeadline)",
        "leselang_command::validate_effect_request(",
        "budget: &self.budget,",
    ] {
        assert!(code.contains(boundary), "reference frame lost {boundary}");
    }
    let correspondence =
        fs::read_to_string(root().join("crates/leselang-command/src/request_correspondence.rs"))
            .unwrap();
    assert!(
        correspondence.contains("if *request.budget != request.continuation.resource_budget()")
    );
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
        ".push(recover_effect_record(&transaction, row)?)",
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
    let record = code
        .split("fn recover_effect_record(")
        .nth(1)
        .unwrap()
        .split("fn checkpoint_fault(")
        .next()
        .unwrap();
    for boundary in [
        "inspect_record_recovery(",
        "validate_image(&image)?",
        "MAX_CONTINUATION_BYTES",
        "canonical_continuation_sequence(&image.token)",
        "image.token.as_str() != self.token",
        "stored_deadline != image.deadline_at_ms",
        "u64::try_from(value)",
        "group_binding::validate_owner(self.connection, image)",
        "successor::validate_record(self.connection, image, completed)",
        "MAX_JOURNAL_ENTRY_BYTES",
        "validate_terminal_step(&step)?",
    ] {
        assert!(record.contains(boundary), "{boundary}");
    }
    let cleanup = record.split("fn completed_payload(").nth(1).unwrap();
    assert!(cleanup.find("drop((").unwrap() < cleanup.find("(token, completed)").unwrap());
    assert_complete_record_recovery_order();
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
    let dependencies = tree(&["--no-default-features", "--features", "leserpent-profile"]);
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
fn native_vm_facade_dependency_closure_omits_all_product_storage_and_gui_contracts() {
    let dependencies = tree(&["--no-default-features"]);
    for required in [
        "leselang-vm",
        "leselang-hir",
        "leselang-runtime-core",
        "leselang-syntax",
    ] {
        assert!(dependencies.lines().any(|line| line.starts_with(required)));
    }
    for forbidden in [
        "leselang-command",
        "leselang-host-contract",
        "leserpent",
        "silvortex",
        "rusqlite",
        "libsqlite3-sys",
        "tokio",
        "gewyvern",
        "serde_json",
    ] {
        assert!(
            !dependencies.lines().any(|line| line.starts_with(forbidden)),
            "native VM retained {forbidden}"
        );
    }
    let root_source = fs::read_to_string(root().join("crates/leselang-vm/src/lib.rs")).unwrap();
    assert!(
        root_source.contains(
            "#[cfg(feature = \"leserpent-profile\")]\ninclude!(\"leserpent_profile.rs\");"
        )
    );
    let native = fs::read_to_string(root().join("crates/leselang-vm/src/native.rs")).unwrap();
    assert!(
        native.contains("pub use leselang_hir::effect_stack_session::StackedControlSession as Vm;")
    );
    assert!(!native.contains("fn poll"));
    assert!(!native.contains("include!(\"leserpent_profile"));
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(
        workflow.contains("cargo +1.98.0 test -p leselang-vm --locked --no-default-features\n")
    );
    assert!(workflow.contains("cargo +1.98.0 clippy -p leselang-vm --locked --no-default-features --all-targets -- -D warnings"));
    assert!(workflow.contains(
        "rustfmt +1.98.0 --edition 2024 --check crates/leselang-vm/src/leserpent_profile.rs"
    ));
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
    assert!(manifest.contains("sqlite-journal = [\"leserpent-profile\", \"dep:rusqlite\"]"));
    assert!(manifest.contains("rusqlite = { workspace = true, optional = true }"));
    let code =
        fs::read_to_string(root().join("crates/leselang-vm/src/leserpent_profile.rs")).unwrap();
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
    let mut native = 0;
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
        } else if matches!(
            target["name"].as_str(),
            Some("native_embedding" | "native_host_pipeline")
        ) {
            assert!(
                target["required-features"].is_null()
                    || target["required-features"] == serde_json::json!([])
            );
            assert!(source.contains("leselang_vm::native"));
            assert!(!source.contains("leserpent_domain"));
            native += 1;
        } else {
            assert_eq!(
                target["required-features"],
                serde_json::json!(["leserpent-profile"])
            );
            memory += 1;
        }
    }
    assert_eq!(durable, 37);
    assert_eq!(memory, 16);
    assert_eq!(native, 2);
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
    assert!(workflow.contains("cargo +1.98.0 test -p leselang-vm --locked --no-default-features --features leserpent-profile"));
    assert!(workflow.contains("cargo +1.98.0 clippy -p leselang-vm --locked --no-default-features --features leserpent-profile --all-targets -- -D warnings"));
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
