use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use gewyvern::project_status::{
    EvidenceState, STATUS_SCHEMA_VERSION, SourceInventory, StatusCatalog, StatusEvidence,
};
use serde_json::{Value, json};

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "gewyvern-source-inventory-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        std::fs::write(root.join("README.md"), "fixture").unwrap();
        Self(root)
    }

    fn inventory(&self, value: &Value) {
        std::fs::write(
            self.0.join("inventory.json"),
            serde_json::to_vec(value).unwrap(),
        )
        .unwrap();
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn manifest() -> Value {
    json!({
        "schema_version": 1,
        "repository": "https://example.invalid/native",
        "revision": "1".repeat(40),
        "files": [{
            "path": "src/lib.rs",
            "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "bytes": 3
        }]
    })
}

fn catalog() -> StatusCatalog {
    serde_json::from_value(json!({
        "schema_version": STATUS_SCHEMA_VERSION,
        "project": "fixture", "checkpoint": "fixture",
        "calibration": {"model": "priority-weighted-strength-v1", "as_of": "2026-11-04"},
        "dimensions": {
            "architectures": [{"id": "native", "label": "Native", "summary": "Fixture"}],
            "modules": [{"id": "runtime", "label": "Runtime", "summary": "Fixture"}],
            "features": [{"id": "source", "label": "Source", "summary": "Fixture"}]
        },
        "coverage_requirements": [{
            "id": "native-source", "architecture": "native", "kind": "proof-shelf",
            "summary": "Fixture", "source": "README.md", "cells": ["native/runtime/source"]
        }],
        "cells": [{
            "id": "native/runtime/source", "architecture": "native", "module": "runtime",
            "feature": "source", "lifecycle": "current", "priority": "maintenance",
            "maturity": "mature", "completion": 100, "confidence": "high",
            "independence": "standalone-tool",
            "contract": {"id": "source-contract", "version": "1.0.0", "stability": "stable", "surfaces": ["source"]},
            "depends_on": [], "blockers": [], "consumers": ["tests"],
            "evidence": [{"kind": "test", "path": "src/lib.rs", "state": "present", "inventory": "inventory.json"}],
            "next_gate": "Verify actual source bytes and execution separately"
        }]
    })).unwrap()
}

fn invalid_inventory(value: Value) {
    assert!(
        serde_json::from_value::<SourceInventory>(value)
            .map_or(true, |inventory| inventory.validate().is_err())
    );
}

#[test]
fn status_lookup_needs_the_local_record_but_not_cargo_network_or_an_upstream_copy() {
    let scratch = Scratch::new();
    scratch.inventory(&manifest());
    let catalog = catalog();
    assert!(!scratch.0.join("src").exists());
    assert!(!scratch.0.join("Cargo.toml").exists());
    catalog.validate(&scratch.0).unwrap();
    let inventory = SourceInventory::load_from_repository(&scratch.0, "inventory.json").unwrap();
    inventory.verify_file("src/lib.rs", b"abc").unwrap();
    assert!(
        inventory
            .verify_file("src/lib.rs", b"abd")
            .unwrap_err()
            .contains("SHA-256")
    );
    assert!(
        inventory
            .verify_file("src/lib.rs", b"abcd")
            .unwrap_err()
            .contains("byte length")
    );
    assert!(inventory.verify_file("src/missing.rs", b"abc").is_err());
}

#[test]
fn inventory_only_presence_never_substitutes_for_missing_local_evidence() {
    let scratch = Scratch::new();
    scratch.inventory(&manifest());
    let mut catalog = catalog();
    catalog.cells[0].evidence[0].inventory = None;
    assert!(
        catalog
            .validate(&scratch.0)
            .unwrap_err()
            .join("\n")
            .contains("present evidence does not exist")
    );
    catalog.cells[0].evidence[0].path = "README.md".into();
    catalog.validate(&scratch.0).unwrap();
    catalog.cells[0].evidence[0].state = EvidenceState::Planned;
    catalog.cells[0].evidence[0].path = "future.rs".into();
    catalog.cells[0].maturity = gewyvern::project_status::Maturity::Developing;
    catalog.validate(&scratch.0).unwrap();
}

#[test]
fn external_evidence_requires_a_present_valid_inventory_and_an_exact_entry() {
    let scratch = Scratch::new();
    let mut catalog = catalog();
    assert!(catalog.validate(&scratch.0).is_err());
    scratch.inventory(&manifest());
    catalog.cells[0].evidence[0].path = "src/other.rs".into();
    assert!(
        catalog
            .validate(&scratch.0)
            .unwrap_err()
            .join("\n")
            .contains("not recorded in inventory")
    );
    catalog.cells[0].evidence[0].path = "src/lib.rs".into();
    catalog.cells[0].evidence[0].state = EvidenceState::Planned;
    assert!(
        catalog
            .validate(&scratch.0)
            .unwrap_err()
            .join("\n")
            .contains("inventory evidence must be present")
    );
    catalog.cells[0].evidence[0].state = EvidenceState::Present;
    let mut invalid = manifest();
    invalid["files"][0]["sha256"] = json!("invalid");
    scratch.inventory(&invalid);
    assert!(
        catalog
            .validate(&scratch.0)
            .unwrap_err()
            .join("\n")
            .contains("invalid source inventory SHA-256")
    );
}

#[test]
fn serialization_preserves_external_identity_and_does_not_relabel_local_evidence() {
    let local = json!({"kind": "source", "path": "src/lib.rs", "state": "present"});
    let evidence: StatusEvidence = serde_json::from_value(local.clone()).unwrap();
    assert!(evidence.inventory.is_none());
    assert_eq!(serde_json::to_value(evidence).unwrap(), local);
    let mut external = local.clone();
    external["inventory"] = json!("inventory.json");
    let evidence: StatusEvidence = serde_json::from_value(external.clone()).unwrap();
    assert_eq!(serde_json::to_value(evidence).unwrap(), external);
    for value in [Value::Null, json!(false), json!({"path": "inventory.json"})] {
        external["inventory"] = value;
        assert!(serde_json::from_value::<StatusEvidence>(external.clone()).is_err());
    }
    let mut typo = local;
    typo["inventroy"] = json!("inventory.json");
    assert!(serde_json::from_value::<StatusEvidence>(typo).is_err());
}

#[test]
fn inventory_identity_cannot_use_mutable_revisions_credentials_or_ambiguous_urls() {
    for repository in [
        "http://example.invalid/native",
        "file:///tmp/native",
        "../native",
        "https://user:secret@example.invalid/native",
        "https://example.invalid/native?branch=main",
        "https://example.invalid/native#main",
        "https://example.invalid/",
        "https://example.invalid/a/../native",
        "https://example.invalid//native",
        "https://example.invalid/%6eative",
    ] {
        let mut value = manifest();
        value["repository"] = json!(repository);
        invalid_inventory(value);
    }
    for revision in ["main", "v0.1.0", "1234567", "", &"A".repeat(40)] {
        let mut value = manifest();
        value["revision"] = json!(revision);
        invalid_inventory(value);
    }
    let mut sha256_git = manifest();
    sha256_git["revision"] = json!("a".repeat(64));
    serde_json::from_value::<SourceInventory>(sha256_git)
        .unwrap()
        .validate()
        .unwrap();
}

#[test]
fn source_inventory_paths_are_portable_canonical_and_bounded() {
    let scratch = Scratch::new();
    scratch.inventory(&manifest());
    for path in [
        "",
        "/tmp/file",
        "../outside",
        "a/../file",
        "a/.. /file",
        "a./file",
        "a /file",
        "./file",
        "a/./file",
        "a//file",
        "a/",
        "C:\\file",
        "a\\file",
        "a\nfile",
        "a\u{0085}file",
        &"x".repeat(1025),
    ] {
        let mut value = manifest();
        value["files"][0]["path"] = json!(path);
        invalid_inventory(value);
        let mut changed = catalog();
        changed.cells[0].evidence[0].inventory = Some(path.into());
        assert!(
            changed.validate(&scratch.0).is_err(),
            "inventory path {path:?}"
        );
        changed.cells[0].evidence[0].inventory = None;
        changed.cells[0].evidence[0].path = path.into();
        changed.cells[0].evidence[0].state = EvidenceState::Planned;
        assert!(
            changed
                .validate(&scratch.0)
                .unwrap_err()
                .join("\n")
                .contains("stay inside the repository")
        );
    }
}

#[test]
fn inventory_rejects_duplicate_unsorted_or_excessive_entries_and_unknown_fields() {
    for field in ["schema_version", "repository", "revision", "files"] {
        let mut value = manifest();
        value.as_object_mut().unwrap().remove(field);
        invalid_inventory(value);
    }
    let mut value = manifest();
    value["schema_version"] = json!(2);
    invalid_inventory(value);
    let mut value = manifest();
    value["files"] = json!([]);
    invalid_inventory(value);
    let mut value = manifest();
    value["extra"] = json!(true);
    invalid_inventory(value);
    let mut value = manifest();
    value["files"][0]["extra"] = json!(true);
    invalid_inventory(value);
    let mut value = manifest();
    value["files"][0]["bytes"] = json!(4194305);
    invalid_inventory(value);
    for hash in ["", "ABC", &"A".repeat(64), &"g".repeat(64)] {
        let mut value = manifest();
        value["files"][0]["sha256"] = json!(hash);
        invalid_inventory(value);
    }
    let row = manifest()["files"][0].clone();
    let mut value = manifest();
    value["files"] = json!([row.clone(), row.clone()]);
    invalid_inventory(value);
    let mut value = manifest();
    value["files"] = json!([row.clone(), row.clone()]);
    value["files"][1]["path"] = json!("a.rs");
    invalid_inventory(value);
    let mut value = manifest();
    value["files"] = json!(vec![row; 4097]);
    invalid_inventory(value);
}

#[test]
fn inventory_loads_are_bounded_regular_utf8_files() {
    let scratch = Scratch::new();
    let path = scratch.0.join("inventory.json");
    assert!(SourceInventory::load(&path).is_err());
    assert!(SourceInventory::load(&scratch.0).is_err());
    for bytes in [
        vec![],
        vec![0xff],
        b"{".to_vec(),
        vec![b' '; 1024 * 1024 + 1],
    ] {
        std::fs::write(&path, bytes).unwrap();
        assert!(SourceInventory::load(&path).is_err());
    }
}

#[cfg(unix)]
#[test]
fn inventory_paths_cannot_follow_file_or_directory_symlinks() {
    let scratch = Scratch::new();
    let outside = Scratch::new();
    outside.inventory(&manifest());
    std::os::unix::fs::symlink(
        outside.0.join("inventory.json"),
        scratch.0.join("inventory.json"),
    )
    .unwrap();
    assert!(SourceInventory::load(scratch.0.join("inventory.json")).is_err());
    assert!(
        catalog()
            .validate(&scratch.0)
            .unwrap_err()
            .join("\n")
            .contains("must not follow symlinks")
    );
    std::os::unix::fs::symlink(&outside.0, scratch.0.join("linked")).unwrap();
    assert!(SourceInventory::load_from_repository(&scratch.0, "linked/inventory.json").is_err());
}

#[test]
fn distinct_inventory_count_is_bounded_without_limiting_reuse() {
    let scratch = Scratch::new();
    scratch.inventory(&manifest());
    let mut catalog = catalog();
    let evidence = catalog.cells[0].evidence[0].clone();
    catalog.cells[0].evidence = vec![evidence; 33];
    catalog.validate(&scratch.0).unwrap();
    for (index, evidence) in catalog.cells[0].evidence.iter_mut().enumerate() {
        let name = format!("inventory-{index}.json");
        std::fs::write(
            scratch.0.join(&name),
            serde_json::to_vec(&manifest()).unwrap(),
        )
        .unwrap();
        evidence.inventory = Some(name);
    }
    assert!(
        catalog
            .validate(&scratch.0)
            .unwrap_err()
            .join("\n")
            .contains("more than 32 source inventories")
    );
}

#[test]
fn protocol_schemas_and_ci_name_the_new_evidence_contract() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let status: Value =
        serde_json::from_str(include_str!("../project/status/schema.json")).unwrap();
    let inventory: Value = serde_json::from_str(include_str!(
        "../project/status/source-inventory.schema.json"
    ))
    .unwrap();
    assert_eq!(
        status["properties"]["schema_version"]["const"],
        STATUS_SCHEMA_VERSION
    );
    assert_eq!(inventory["properties"]["schema_version"]["const"], 1);
    assert_eq!(
        status["$defs"]["repository_path"],
        inventory["$defs"]["repository_path"]
    );
    assert_eq!(inventory["properties"]["files"]["maxItems"], 4096);
    assert_eq!(
        inventory["properties"]["files"]["items"]["properties"]["bytes"]["maximum"],
        4194304
    );
    let schema = status["$defs"]["evidence"].to_string();
    assert!(schema.contains("inventory"));
    assert!(schema.contains("present"));
    assert!(
        std::fs::read_to_string(root.join(".github/workflows/ci.yml"))
            .unwrap()
            .contains("--test project_status_inventory_tdd")
    );
    let mut old = catalog();
    old.schema_version = 3;
    assert!(
        old.validate(root)
            .unwrap_err()
            .join("\n")
            .contains("unsupported schema_version")
    );
}
