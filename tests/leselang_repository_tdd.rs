use serde_json::Value;
use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

const REVISION: &str = "5f86dfb8dd79a28986fe5eb4cbfe6b044796f935";
const REPOSITORY: &str = "https://github.com/Team-silvortex/leselang";
const CORES: [&str; 3] = [
    "leselang-syntax",
    "leselang-runtime-core",
    "leselang-hir-core",
];

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn metadata(manifest: &Path) -> Value {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--locked",
            "--offline",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(manifest)
        .current_dir(root())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn product_metadata() -> &'static Value {
    static METADATA: OnceLock<Value> = OnceLock::new();
    METADATA.get_or_init(|| metadata(&root().join("Cargo.toml")))
}

fn validate_identity(metadata: &Value) -> Result<(), String> {
    validate_packages(metadata, &CORES)
}

fn validate_packages(metadata: &Value, names: &[&str]) -> Result<(), String> {
    let source = format!("git+{REPOSITORY}?rev={REVISION}#{REVISION}");
    let packages = metadata["packages"].as_array().ok_or("missing packages")?;
    let members = metadata["workspace_members"]
        .as_array()
        .ok_or("missing members")?;
    for &name in names {
        let matches: Vec<_> = packages.iter().filter(|p| p["name"] == name).collect();
        if matches.len() != 1 {
            return Err(format!("{name} must have exactly one package identity"));
        }
        let package = matches[0];
        if package["version"] != "0.1.0" || package["source"] != source {
            return Err(format!(
                "{name} is not the reviewed immutable native package"
            ));
        }
        if members.contains(&package["id"]) {
            return Err(format!("{name} must not be a product workspace member"));
        }
        let manifest = Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or("missing manifest")?,
        );
        if manifest.starts_with(root()) {
            return Err(format!(
                "{name} still resolves to an in-tree implementation"
            ));
        }
    }
    Ok(())
}

fn validate_compatibility_isolation(metadata: &Value) -> Result<(), String> {
    let packages = metadata["packages"].as_array().ok_or("missing packages")?;
    for package in packages
        .iter()
        .filter(|package| package["source"].is_null())
    {
        let consumer = package["name"].as_str().ok_or("missing consumer name")?;
        for dependency in package["dependencies"]
            .as_array()
            .ok_or("missing dependencies")?
        {
            let target = dependency["name"]
                .as_str()
                .ok_or("missing dependency name")?;
            if !dependency["source"].is_null() || !matches!(target, "leselang-hir" | "leselang-vm")
            {
                continue;
            }
            // Only the old VM's own native compatibility surface may use the
            // old HIR. Dev/build dependencies and aliases are not exemptions.
            if consumer != "leselang-vm"
                || target != "leselang-hir"
                || !dependency["kind"].is_null()
                || !dependency["rename"].is_null()
                || dependency["optional"] != false
            {
                return Err(format!(
                    "{consumer} still depends on local compatibility package {target}"
                ));
            }
        }
    }
    Ok(())
}

fn inventory(base: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    let mut result = BTreeMap::new();
    let mut pending = vec![base.join("src"), base.join("tests")];
    while let Some(path) = pending.pop() {
        if !path.exists() {
            continue;
        }
        for entry in fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            assert!(!kind.is_symlink(), "source snapshot must not follow links");
            if kind.is_dir() {
                pending.push(entry.path());
            } else {
                assert!(kind.is_file());
                result.insert(
                    entry.path().strip_prefix(base).unwrap().into(),
                    fs::read(entry.path()).unwrap(),
                );
            }
        }
    }
    result
}

#[test]
fn product_and_foreign_crate_fixture_use_one_pinned_native_identity() {
    validate_identity(product_metadata()).unwrap();
    let fixture = metadata(&root().join("tests/fixtures/leselang-product-hir-boundary/Cargo.toml"));
    validate_identity(&fixture).unwrap();
    validate_packages(&fixture, &["leselang-hir", "leselang-vm"]).unwrap();
    for name in CORES {
        let id = |metadata: &Value| {
            metadata["packages"]
                .as_array()
                .unwrap()
                .iter()
                .find(|p| p["name"] == name)
                .unwrap()["id"]
                .clone()
        };
        assert_eq!(id(product_metadata()), id(&fixture));
    }
}

#[test]
fn foreign_fixture_uses_default_native_facades_without_product_features() {
    let fixture = metadata(&root().join("tests/fixtures/leselang-product-hir-boundary/Cargo.toml"));
    validate_packages(&fixture, &["leselang-hir", "leselang-vm"]).unwrap();
    for name in ["leselang-hir", "leselang-vm"] {
        let index = fixture["packages"]
            .as_array()
            .unwrap()
            .iter()
            .position(|p| p["name"] == name)
            .unwrap();
        let package = &fixture["packages"][index];
        for feature in ["leserpent-profile", "sqlite-journal"] {
            assert!(package["features"].get(feature).is_none());
        }
        for dependency in package["dependencies"].as_array().unwrap() {
            assert!(
                !dependency["name"]
                    .as_str()
                    .unwrap()
                    .starts_with("leserpent")
            );
            assert_ne!(dependency["name"], "rusqlite");
        }
        let mut local = fixture.clone();
        local["packages"][index]["source"] = Value::Null;
        assert!(validate_packages(&local, &[name]).is_err());
    }
}

#[test]
fn compatibility_facades_are_isolated_even_from_product_dev_and_build_dependencies() {
    validate_compatibility_isolation(product_metadata()).unwrap();
    let root_index = product_metadata()["packages"]
        .as_array()
        .unwrap()
        .iter()
        .position(|p| p["name"] == "gewyvern")
        .unwrap();
    for facade in ["leselang-hir", "leselang-vm"] {
        for kind in [
            Value::Null,
            Value::String("dev".into()),
            Value::String("build".into()),
        ] {
            let mut changed = product_metadata().clone();
            changed["packages"][root_index]["dependencies"]
                .as_array_mut()
                .unwrap()
                .push(serde_json::json!({
                    "name": facade,
                    "source": null,
                    "kind": kind,
                    "rename": "apparently_native",
                    "optional": true
                }));
            assert!(validate_compatibility_isolation(&changed).is_err());
        }
    }
    let vm_index = product_metadata()["packages"]
        .as_array()
        .unwrap()
        .iter()
        .position(|p| p["name"] == "leselang-vm")
        .unwrap();
    let edge_index = product_metadata()["packages"][vm_index]["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .position(|d| d["name"] == "leselang-hir")
        .unwrap();
    for (field, value) in [
        ("kind", Value::String("build".into())),
        ("kind", Value::String("dev".into())),
        ("rename", Value::String("hidden_facade".into())),
        ("optional", Value::Bool(true)),
    ] {
        let mut changed = product_metadata().clone();
        changed["packages"][vm_index]["dependencies"][edge_index][field] = value;
        assert!(validate_compatibility_isolation(&changed).is_err());
    }
}

#[test]
fn identity_gate_rejects_duplicate_local_mutable_or_wrong_version_cores() {
    let original = product_metadata();
    for name in CORES {
        let index = original["packages"]
            .as_array()
            .unwrap()
            .iter()
            .position(|p| p["name"] == name)
            .unwrap();
        for (field, replacement) in [
            ("source", Value::Null),
            (
                "source",
                Value::String(format!("git+{REPOSITORY}?branch=main#{REVISION}")),
            ),
            ("version", Value::String("2.5.0".into())),
            (
                "manifest_path",
                Value::String(
                    root()
                        .join(format!("crates/{name}/Cargo.toml"))
                        .display()
                        .to_string(),
                ),
            ),
        ] {
            let mut changed = original.clone();
            changed["packages"][index][field] = replacement;
            assert!(validate_identity(&changed).is_err(), "{name}: {field}");
        }
        let mut duplicate = original.clone();
        duplicate["packages"]
            .as_array_mut()
            .unwrap()
            .push(original["packages"][index].clone());
        assert!(validate_identity(&duplicate).is_err());
        let mut member = original.clone();
        member["workspace_members"]
            .as_array_mut()
            .unwrap()
            .push(original["packages"][index]["id"].clone());
        assert!(validate_identity(&member).is_err());
    }
}

#[test]
fn frozen_acceptance_sources_match_the_actual_resolved_upstream_files() {
    validate_identity(product_metadata()).unwrap();
    for name in CORES {
        let package = product_metadata()["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == name)
            .unwrap();
        let upstream = Path::new(package["manifest_path"].as_str().unwrap())
            .parent()
            .unwrap();
        let expected = inventory(upstream);
        let snapshot = inventory(&root().join(format!("crates/{name}")));
        assert!(!expected.is_empty());
        assert_eq!(
            expected.keys().collect::<Vec<_>>(),
            snapshot.keys().collect::<Vec<_>>(),
            "{name}: inventory drift"
        );
        for (path, bytes) in expected {
            assert!(
                snapshot[&path] == bytes,
                "{name}/{}: frozen source differs from upstream",
                path.display()
            );
        }
    }
}

#[test]
fn every_product_core_edge_inherits_the_shared_pin() {
    let metadata = product_metadata();
    for package in metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["source"].is_null())
    {
        let manifest = fs::read_to_string(package["manifest_path"].as_str().unwrap()).unwrap();
        for dependency in package["dependencies"].as_array().unwrap() {
            let name = dependency["name"].as_str().unwrap();
            if CORES.contains(&name) {
                assert!(dependency["path"].is_null(), "local edge to {name}");
                assert!(
                    manifest.contains(&format!("{name}.workspace = true")),
                    "{} must inherit {name}",
                    package["name"]
                );
            }
        }
    }
    let workspace = fs::read_to_string(root().join("Cargo.toml")).unwrap();
    let excluded = workspace
        .split_once("exclude = [")
        .unwrap()
        .1
        .split_once(']')
        .unwrap()
        .0;
    for name in CORES {
        assert!(excluded.contains(&format!("\"crates/{name}\"")));
    }
}

#[test]
fn handoff_and_ci_track_the_verified_dependency_not_full_vm_acceptance() {
    let state: Value =
        serde_json::from_str(include_str!("../project/leselang-repository.json")).unwrap();
    assert_eq!(state["pinned_revision"], REVISION);
    assert_eq!(state["product_dependency_cutover"], true);
    assert_eq!(state["product_source_state"], "frozen-acceptance-snapshots");
    let cleanup = &state["compatibility_cleanup"];
    assert_eq!(cleanup["root_tests_use_product_owners"], true);
    assert_eq!(cleanup["foreign_consumer_uses_published_facades"], true);
    assert_eq!(
        cleanup["allowed_local_facade_edge"],
        "leselang-vm -> leselang-hir"
    );
    for retained in [
        "local_facades_retired",
        "frozen_snapshots_retired",
        "source_exporter_retired",
    ] {
        assert_eq!(cleanup[retained], false);
    }
    for flag in ["registry_published", "complete_vm", "full_extraction_gate"] {
        assert_eq!(state[flag], false);
    }
    let workflow = include_str!("../.github/workflows/ci.yml");
    assert!(workflow.contains("--test leselang_repository_tdd"));
    let members = product_metadata()["workspace_members"].as_array().unwrap();
    for facade in ["leselang-hir", "leselang-vm"] {
        let package = product_metadata()["packages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|p| p["name"] == facade)
            .unwrap();
        assert!(members.contains(&package["id"]));
        assert!(package["source"].is_null());
    }
}
