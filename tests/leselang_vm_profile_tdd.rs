use std::{fs, path::Path, process::Command};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
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
    assert_eq!(durable, 35);
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
