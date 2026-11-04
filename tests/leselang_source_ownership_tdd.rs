use serde_json::{Value, json};
use std::fs;
use std::path::{Path, PathBuf};

#[path = "support/leselang_upstream.rs"]
pub mod upstream;

fn state() -> Value {
    serde_json::from_str(include_str!("../project/leselang-repository.json")).unwrap()
}

struct AcceptanceScratch(PathBuf);

impl AcceptanceScratch {
    fn new() -> Self {
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "leselang-acceptance-{}-{stamp}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}

impl Drop for AcceptanceScratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn acceptance_sources_require_recorded_native_bytes_without_redirecting_product_owners() {
    let scratch = AcceptanceScratch::new();
    let product = scratch.0.join("product");
    let native = scratch.0.join("native");
    let original = "crates/leselang-runtime-core/src/lib.rs";
    let recorded: gewyvern::project_status::SourceInventory = serde_json::from_value(json!({
        "schema_version": 1, "repository": "https://example.invalid/native", "revision": "1".repeat(40),
        "files": [{"path": original, "bytes": 3,
            "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"}]
    })).unwrap();
    recorded.validate().unwrap();
    for root in [&product, &native] {
        fs::create_dir_all(root.join(original).parent().unwrap()).unwrap();
        fs::write(root.join(original), b"abc").unwrap();
    }
    assert_eq!(
        upstream::acceptance_source(original, &product, &native, &recorded).unwrap(),
        b"abc"
    );
    fs::remove_file(product.join(original)).unwrap();
    assert_eq!(
        upstream::acceptance_source(original, &product, &native, &recorded).unwrap(),
        b"abc"
    );
    fs::write(product.join(original), b"abc").unwrap();
    fs::write(native.join(original), b"abd").unwrap();
    assert!(
        upstream::acceptance_source(original, &product, &native, &recorded)
            .unwrap_err()
            .contains("SHA-256")
    );
    fs::write(native.join(original), b"abcd").unwrap();
    assert!(upstream::acceptance_source(original, &product, &native, &recorded).is_err());
    fs::remove_file(native.join(original)).unwrap();
    assert!(
        upstream::acceptance_source(original, &product, &native, &recorded)
            .unwrap_err()
            .contains("unavailable")
    );
    for missing in [
        "crates/leselang-runtime-core/src/other.rs",
        "crates/leselang-syntax/src/lib.rs",
    ] {
        assert!(
            upstream::acceptance_source(missing, &product, &native, &recorded)
                .unwrap_err()
                .contains("unrecorded")
        );
    }
    for original in [
        "crates/leselang-vm/src/native.rs",
        "crates/leserpent-hir/src/lib.rs",
        "crates/leselang-hir-core-product/src/lib.rs",
    ] {
        for root in [&product, &native] {
            fs::create_dir_all(root.join(original).parent().unwrap()).unwrap();
        }
        fs::write(product.join(original), b"local-owner").unwrap();
        fs::write(native.join(original), b"not-the-owner").unwrap();
        assert_eq!(
            upstream::acceptance_source(original, &product, &native, &recorded).unwrap(),
            b"local-owner"
        );
    }
    for path in [
        "../outside.rs",
        "crates/leselang-runtime-core/../leserpent-hir/src/lib.rs",
    ] {
        assert!(upstream::acceptance_source(path, &product, &native, &recorded).is_err());
    }
}

#[test]
fn status_native_source_evidence_is_owned_by_the_pinned_inventory() {
    let state = state();
    assert_eq!(
        state["compatibility_cleanup"]["status_source_evidence_uses_inventory"],
        true
    );
    let catalog = gewyvern::project_status::StatusCatalog::load(
        upstream::product_root().join("project/status/catalog.json"),
    )
    .unwrap();
    let recorded = upstream::source_inventory();
    let mut references = 0;
    let mut paths = std::collections::BTreeSet::new();
    for evidence in catalog.cells.iter().flat_map(|cell| &cell.evidence) {
        let core_file = upstream::NATIVE_CORES.iter().any(|name| {
            ["src", "tests"].iter().any(|directory| {
                evidence
                    .path
                    .starts_with(&format!("crates/{name}/{directory}/"))
            })
        });
        if core_file {
            assert_eq!(
                evidence.inventory.as_deref(),
                state["source_inventory"].as_str()
            );
            assert!(recorded.file(&evidence.path).is_some());
            references += 1;
            paths.insert(&evidence.path);
        } else {
            assert!(
                !upstream::NATIVE_CORES
                    .iter()
                    .any(|name| { evidence.path == format!("crates/{name}") }),
                "a retained navigation directory is not native source evidence"
            );
            assert!(
                evidence.inventory.is_none(),
                "product evidence must remain local"
            );
        }
    }
    assert_eq!(references, 304);
    assert_eq!(paths.len(), 161);
    // Retiring core copies does not retire the legacy facade compatibility proofs.
    assert_eq!(
        state["compatibility_cleanup"]["frozen_snapshots_retired"],
        true
    );
    assert_eq!(
        state["compatibility_cleanup"]["local_facades_retired"],
        false
    );
}

#[test]
fn runtime_documentation_has_one_pinned_owner_without_a_product_body_copy() {
    let state = state();
    assert_eq!(
        state["compatibility_cleanup"]["runtime_documentation_snapshot_retired"],
        true
    );
    assert_eq!(
        state["documentation_inspection"]["gate"],
        "tests/documentation_system_tdd.rs"
    );
    let original = upstream::runtime_baseline_document();
    assert_eq!(original, upstream::read(upstream::RUNTIME_BASELINE_PATH));
    let pointer = include_str!("../crates/leselang-runtime-core/README.md");
    assert!(
        pointer.lines().count() <= 40,
        "local README is navigation, not a second manual"
    );
    assert!(!pointer.contains("## Owned Capture Data"));
    let base = format!(
        "{}/blob/{}/",
        state["remote_url"].as_str().unwrap(),
        state["pinned_revision"].as_str().unwrap()
    );
    for path in [
        upstream::RUNTIME_BASELINE_PATH,
        "crates/leselang-runtime-core/README.md",
    ] {
        assert!(pointer.contains(&format!("{base}{path}")));
        assert!(upstream::checkout().join(path).is_file());
    }
    assert!(pointer.contains("historical provenance"));
    assert!(pointer.contains("not current build instructions"));
    assert!(pointer.contains("cargo dev check leselang"));

    let suite = include_str!("documentation_system_tdd.rs");
    // Current ownership prose stays local; all 17 historical semantic checks use
    // the immutable archive, never a local README masquerading as the core manual.
    assert_eq!(
        suite
            .matches("upstream::runtime_baseline_document()")
            .count(),
        17
    );
    assert_eq!(
        suite
            .matches("../crates/leselang-runtime-core/README.md")
            .count(),
        1
    );
    let catalog: Value =
        serde_json::from_str(include_str!("../project/status/catalog.json")).unwrap();
    for cell in catalog["cells"].as_array().unwrap() {
        let evidence = cell["evidence"].as_array().unwrap();
        if evidence
            .iter()
            .any(|e| e["path"] == "crates/leselang-runtime-core/README.md")
        {
            for path in [
                "project/leselang-repository.json",
                "tests/documentation_system_tdd.rs",
            ] {
                assert!(
                    evidence
                        .iter()
                        .any(|e| e["path"] == path && e["state"] == "present"),
                    "{} must link the real documentation owner and gate",
                    cell["id"]
                );
            }
        }
    }
}

#[test]
fn runtime_documentation_rejects_mutable_paths_wrong_hashes_and_missing_upstream() {
    let original = state();
    for (field, replacement) in [
        ("path", json!("../runtime-core-baseline.md")),
        ("path", json!("/tmp/runtime-core-baseline.md")),
        (
            "path",
            json!("docs/provenance/../provenance/runtime-core-baseline.md"),
        ),
        ("path", json!("crates/leselang-runtime-core/README.md")),
        ("revision", json!("main")),
        ("revision", Value::Null),
        ("sha256", Value::Null),
        ("sha256", json!("not-a-digest")),
        ("sha256", json!("0".repeat(64))),
    ] {
        let mut changed = original.clone();
        changed["documentation_inspection"]["runtime_baseline"][field] = replacement;
        assert!(
            upstream::read_runtime_baseline(upstream::checkout(), &changed).is_err(),
            "{field}"
        );
    }
    for (field, value) in [
        ("owner", json!("local-snapshot")),
        ("snapshot_fallback", json!(true)),
    ] {
        let mut changed = original.clone();
        changed["documentation_inspection"][field] = value;
        assert!(upstream::read_runtime_baseline(upstream::checkout(), &changed).is_err());
    }
    let missing = upstream::product_root().join("target/leselang-missing-documentation-checkout");
    assert!(!missing.exists());
    assert!(
        upstream::read_runtime_baseline(&missing, &original)
            .unwrap_err()
            .contains("unavailable")
    );
    assert!(
        upstream::read_runtime_baseline(upstream::product_root(), &original).is_err(),
        "the product's retained README must not substitute for upstream provenance"
    );
}

#[test]
fn retired_oracles_keep_complete_recorded_results_and_the_original_owner_inventory() {
    let root = upstream::product_root();
    let state = state();
    for flag in [
        "frozen_core_compilation_retired",
        "migration_oracle_harness_retired",
    ] {
        assert_eq!(state["compatibility_cleanup"][flag], true);
    }
    for retired in [
        "crates/leselang-hir/tests/migration_oracles.rs",
        "crates/leselang-hir/tests/support/migration_computation.rs",
        "crates/leselang-hir/tests/support/oracle_capture.rs",
    ] {
        assert!(!root.join(retired).exists());
    }
    let proof: Value = serde_json::from_slice(
        &fs::read(root.join(state["migration_oracle_acceptance"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    // Capture provenance stays historical when a later native pin is reviewed.
    assert_eq!(
        proof["native_revision"],
        "5f86dfb8dd79a28986fe5eb4cbfe6b044796f935"
    );
    assert_eq!(proof["original_tests"], 121);
    let mut names = std::collections::BTreeSet::new();
    for (field, count) in [
        ("product_tests", 93),
        ("native_tests", 25),
        ("boundary_tests", 3),
    ] {
        let tests = proof[field].as_array().unwrap();
        assert_eq!(tests.len(), count);
        for name in tests {
            assert!(names.insert(name.as_str().unwrap()));
        }
    }
    assert_eq!(names.len(), 121);
    let reader =
        fs::read_to_string(root.join("crates/leserpent-hir/src/migration_baseline.rs")).unwrap();
    for forbidden in ["fs::write", "File::create", "std::env", "ORACLE_CAPTURE"] {
        assert!(
            !reader.contains(forbidden),
            "baseline must not auto-update: {forbidden}"
        );
    }
    for name in proof["native_tests"].as_array().unwrap() {
        let name = name.as_str().unwrap();
        let function = name.rsplit("::").next().unwrap();
        let path = proof["native_test_sources"][name].as_str().unwrap();
        assert!(path.starts_with("crates/leselang-hir-core/src/"));
        let code = upstream::read(path);
        assert!(
            code.contains(&format!("fn {function}(")),
            "lost native owner test {name}"
        );
    }
    let matrices = proof["matrices"].as_array().unwrap();
    assert_eq!(matrices.len(), 3);
    for (matrix, (name, cases)) in
        matrices
            .iter()
            .zip([("scalar", 3840), ("fold", 1920), ("loop", 1152)])
    {
        assert_eq!(matrix["name"], name);
        assert_eq!(matrix["cases"], cases);
        let bytes = fs::read(root.join(matrix["path"].as_str().unwrap())).unwrap();
        let digest = ring::digest::digest(&ring::digest::SHA256, &bytes);
        let hex: String = digest.as_ref().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(matrix["sha256"], hex, "recorded baseline was rewritten");
        let recorded: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(recorded["cases"].as_array().unwrap().len(), cases as usize);
        assert_eq!(
            recorded["origin"]["native_revision"],
            proof["native_revision"]
        );
        assert_eq!(
            recorded["origin"]["product_revision"],
            proof["product_revision"]
        );
        assert_eq!(
            recorded["origin"]["oracle_source_sha256"],
            matrix["oracle_source_sha256"]
        );
    }
    let workflow =
        fs::read_to_string(root.join("crates/gewyvern-dev/src/leselang_check.rs")).unwrap();
    assert!(workflow.contains("pinned-native-hir-oracles"));
    assert!(!include_str!("../.github/workflows/ci.yml").contains("--test migration_oracles"));
    for flag in ["complete_vm", "full_extraction_gate"] {
        assert_eq!(proof[flag], false);
    }
}

fn includes_frozen_core(source: &str) -> bool {
    let mut directive = String::new();
    for line in source.lines().map(str::trim) {
        if line.starts_with("#[path") || line.starts_with("include!(") {
            directive.clear();
            directive.push_str(line);
        } else if !directive.is_empty() {
            directive.push_str(line);
        }
        if !directive.is_empty() && (line.ends_with(']') || line.ends_with(";")) {
            if upstream::NATIVE_CORES
                .iter()
                .any(|core| directive.contains(&format!("{core}/")))
            {
                return true;
            }
            directive.clear();
        }
    }
    false
}

#[test]
fn product_workspace_never_compiles_frozen_core_sources_even_for_tests() {
    assert!(includes_frozen_core(
        "#[path = \"../../leselang-hir-core/src/ir.rs\"]\nmod ir;"
    ));
    assert!(includes_frozen_core(
        "include!(\n\"../../leselang-runtime-core/src/lib.rs\"\n);"
    ));
    assert!(!includes_frozen_core(
        "include_str!(\"../crates/leselang-hir-core/README.md\");"
    ));
    assert!(!includes_frozen_core(
        "include!(\"../../leserpent-hir/src/leserpent_profile.rs\");"
    ));
    let metadata = upstream::product_metadata();
    let members = metadata["workspace_members"].as_array().unwrap();
    for package in metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| members.contains(&p["id"]))
    {
        let root = Path::new(package["manifest_path"].as_str().unwrap())
            .parent()
            .unwrap();
        let mut pending: Vec<_> = ["src", "tests", "examples", "benches", "build.rs"]
            .map(|p| root.join(p))
            .into();
        while let Some(path) = pending.pop() {
            if !path.exists() {
                continue;
            }
            assert!(!path.is_symlink());
            if path.is_dir() {
                pending.extend(fs::read_dir(path).unwrap().map(|e| e.unwrap().path()));
            } else if path.extension().is_some_and(|e| e == "rs") {
                assert!(
                    !includes_frozen_core(&fs::read_to_string(&path).unwrap()),
                    "frozen source inclusion: {}",
                    path.display()
                );
            }
        }
    }
}

#[test]
fn retired_manifests_cannot_reintroduce_a_second_native_build_tree() {
    for name in upstream::NATIVE_CORES {
        let manifest = upstream::product_root().join(format!("crates/{name}/Cargo.toml"));
        assert!(
            !manifest.exists(),
            "{name} must remain a non-build navigation directory"
        );
        assert_eq!(
            upstream::source_path(&manifest).unwrap(),
            upstream::checkout().join(format!("crates/{name}/Cargo.toml"))
        );
        assert!(
            upstream::read_source(manifest)
                .unwrap()
                .contains("[package]")
        );
    }
    let state = state();
    for flag in [
        "frozen_core_manifests_retired",
        "core_source_inspection_uses_pinned_upstream",
    ] {
        assert_eq!(state["compatibility_cleanup"][flag], true);
    }
    assert_eq!(
        state["compatibility_cleanup"]["frozen_snapshots_retired"],
        true
    );
    assert_eq!(
        state["compatibility_cleanup"]["local_facades_retired"],
        false
    );
    assert_eq!(state["source_inspection"]["snapshot_fallback"], false);
    assert_eq!(
        state["source_inspection"]["owner"],
        "cargo-resolved-pinned-native-cores"
    );
    assert_eq!(
        state["source_inspection"]["gates"],
        json!([
            "tests/language_independence_tdd.rs",
            "tests/leselang_native_profile_tdd.rs",
            "tests/leselang_vm_profile_tdd.rs",
            "tests/reliability_gate_tdd.rs",
        ])
    );
    assert_eq!(
        state["source_inspection"]["acceptance"],
        "tests/leselang_source_ownership_tdd.rs"
    );
    assert!(
        include_str!("../.github/workflows/ci.yml")
            .contains("--test leselang_source_ownership_tdd")
    );
    for flag in ["complete_vm", "full_extraction_gate"] {
        assert_eq!(state[flag], false);
    }
}

#[test]
fn every_core_inspection_uses_the_cargo_resolved_source_not_the_frozen_copy() {
    for name in upstream::NATIVE_CORES {
        let relative = format!("crates/{name}/src/lib.rs");
        let product = upstream::product_root().join(&relative);
        let resolved = upstream::source_path(&product).unwrap();
        assert_eq!(resolved, upstream::checkout().join(&relative));
        assert!(!resolved.starts_with(upstream::product_root()));
        assert_eq!(
            upstream::read_source(product).unwrap(),
            fs::read_to_string(resolved).unwrap()
        );
    }
    for relative in [
        "crates/leselang-hir/src/lib.rs",
        "crates/leselang-vm/src/native.rs",
        "crates/leserpent-hir/src/lib.rs",
        "crates/leserpent-vm/src/lib.rs",
        "crates/leselang-command/src/lib.rs",
        "docs/leselang-repository-handoff.md",
    ] {
        let product = upstream::product_root().join(relative);
        assert_eq!(upstream::source_path(&product).unwrap(), product);
        assert_eq!(
            upstream::read_source(&product).unwrap(),
            fs::read_to_string(product).unwrap()
        );
    }
}

#[test]
fn source_routing_rejects_escapes_and_only_matches_complete_core_components() {
    let root = upstream::product_root();
    for path in [
        root.to_path_buf(),
        root.join("../outside.rs"),
        root.join("crates/leselang-hir-core/../leserpent-hir/src/lib.rs"),
        std::env::temp_dir().join("outside.rs"),
        Path::new("crates/leselang-hir-core/src/lib.rs").to_path_buf(),
    ] {
        assert!(upstream::source_path(path).is_err());
    }
    for name in [
        "leselang-hir",
        "leselang-hir-core-product",
        "leselang-syntax-backup",
    ] {
        let path = root.join("crates").join(name).join("src/lib.rs");
        assert_eq!(upstream::source_path(&path).unwrap(), path);
    }
}

#[test]
fn source_selection_does_not_require_or_fall_back_to_a_product_snapshot() {
    let product = upstream::product_root();
    let missing =
        std::env::temp_dir().join(format!("leselang-absent-checkout-{}", std::process::id()));
    assert!(!missing.exists());
    for name in upstream::NATIVE_CORES {
        let relative = format!("crates/{name}/src/lib.rs");
        let local = product.join(&relative);
        assert!(!local.exists(), "core source copies have been retired");
        let resolved = upstream::resolve_source_path(&local, product, &missing).unwrap();
        assert_eq!(resolved, missing.join(&relative));
        assert_eq!(
            fs::read_to_string(resolved).unwrap_err().kind(),
            std::io::ErrorKind::NotFound
        );

        let absent_product = missing.join("product");
        let resolved = upstream::resolve_source_path(
            &absent_product.join(&relative),
            &absent_product,
            upstream::checkout(),
        )
        .unwrap();
        assert!(
            fs::read_to_string(resolved).is_ok(),
            "native inspection needs no frozen copy"
        );
    }
}

#[test]
fn source_identity_rejects_missing_duplicate_or_substituted_native_cores() {
    let original = upstream::product_metadata();
    assert_eq!(
        upstream::resolve_checkout(original, &state(), upstream::product_root()).unwrap(),
        upstream::checkout()
    );
    for name in upstream::NATIVE_CORES {
        let index = original["packages"]
            .as_array()
            .unwrap()
            .iter()
            .position(|p| p["name"] == *name)
            .unwrap();
        let mut missing = original.clone();
        missing["packages"].as_array_mut().unwrap().remove(index);
        assert!(upstream::resolve_checkout(&missing, &state(), upstream::product_root()).is_err());
        let mut duplicate = original.clone();
        duplicate["packages"]
            .as_array_mut()
            .unwrap()
            .push(original["packages"][index].clone());
        assert!(
            upstream::resolve_checkout(&duplicate, &state(), upstream::product_root()).is_err()
        );
        for (field, value) in [
            ("source", Value::Null),
            (
                "source",
                json!("git+https://github.com/Team-silvortex/leselang?branch=main"),
            ),
            ("version", json!("2.5.0")),
            (
                "manifest_path",
                json!(upstream::product_root().join(format!("crates/{name}/Cargo.toml"))),
            ),
            (
                "manifest_path",
                json!(upstream::checkout().join(format!("other/crates/{name}/Cargo.toml"))),
            ),
            (
                "manifest_path",
                json!(upstream::checkout().join(format!("crates/../crates/{name}/Cargo.toml"))),
            ),
            ("manifest_path", json!(format!("crates/{name}/Cargo.toml"))),
        ] {
            let mut changed = original.clone();
            changed["packages"][index][field] = value;
            assert!(
                upstream::resolve_checkout(&changed, &state(), upstream::product_root()).is_err(),
                "{name}: {field}"
            );
        }
        let mut member = original.clone();
        member["workspace_members"]
            .as_array_mut()
            .unwrap()
            .push(original["packages"][index]["id"].clone());
        assert!(upstream::resolve_checkout(&member, &state(), upstream::product_root()).is_err());
    }
}
