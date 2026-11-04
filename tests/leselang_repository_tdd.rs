use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

#[path = "support/leselang_upstream.rs"]
pub mod upstream;

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

const FACADE_TEST_IMPORTS: &[(&str, &[&str])] = &[
    (
        "leselang-hir",
        &["leselang-runtime-core", "leselang-syntax", "serde"],
    ),
    ("leselang-vm", &["serde", "serde_json"]),
];

fn validate_facade_test_dependencies(metadata: &Value) -> Result<(), String> {
    let packages = metadata["packages"].as_array().ok_or("missing packages")?;
    for (facade, imports) in FACADE_TEST_IMPORTS {
        let package = packages
            .iter()
            .find(|package| package["name"] == *facade && package["source"].is_null())
            .ok_or("missing legacy facade")?;
        let dependencies = package["dependencies"]
            .as_array()
            .ok_or("missing dependencies")?;
        for name in *imports {
            let matches: Vec<_> = dependencies
                .iter()
                .filter(|dep| dep["name"] == *name)
                .collect();
            if matches.len() != 1 || matches[0]["kind"] != "dev" {
                return Err(format!(
                    "{facade} must use {name} only as a test dependency"
                ));
            }
        }
    }
    Ok(())
}

fn validate_gewyvern_runtime_isolation(metadata: &Value) -> Result<(), String> {
    let mut packages = BTreeMap::new();
    for package in metadata["packages"].as_array().ok_or("missing packages")? {
        let id = package["id"].as_str().ok_or("missing package id")?;
        if packages.insert(id, package).is_some() {
            return Err("duplicate package id".into());
        }
    }
    let roots: Vec<_> = packages
        .values()
        .filter(|package| package["name"] == "gewyvern")
        .collect();
    if roots.len() != 1 {
        return Err("expected one Gewyvern package".into());
    }
    let mut nodes = BTreeMap::new();
    for node in metadata["resolve"]["nodes"]
        .as_array()
        .ok_or("missing resolved graph")?
    {
        let id = node["id"].as_str().ok_or("missing node id")?;
        if nodes.insert(id, node).is_some() {
            return Err("duplicate resolved node".into());
        }
    }
    let mut pending = vec![roots[0]["id"].as_str().ok_or("missing root id")?];
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id) {
            continue;
        }
        let package = packages.get(id).ok_or("resolved package is absent")?;
        let name = package["name"].as_str().ok_or("missing package name")?;
        if name.starts_with("leselang-")
            || name.starts_with("leserpent-")
            || matches!(name, "leselang" | "leserpent" | "leserpentd")
        {
            return Err(format!("Gewyvern runtime/build closure contains {name}"));
        }
        let node = nodes.get(id).ok_or("resolved node is absent")?;
        for dependency in node["deps"]
            .as_array()
            .ok_or("missing resolved dependencies")?
        {
            let kinds = dependency["dep_kinds"]
                .as_array()
                .ok_or("missing dependency kinds")?;
            if kinds.is_empty() {
                return Err("empty dependency kinds".into());
            }
            let mut runtime = false;
            for kind in kinds {
                match kind.get("kind") {
                    Some(Value::Null) => runtime = true,
                    Some(Value::String(value)) if value == "build" => runtime = true,
                    Some(Value::String(value)) if value == "dev" => {}
                    _ => return Err("unknown dependency kind".into()),
                }
            }
            // Follow resolved package IDs, not dependency aliases. Target-specific
            // normal/build edges count too; product integration tests do not.
            if runtime {
                pending.push(
                    dependency["pkg"]
                        .as_str()
                        .ok_or("missing dependency package id")?,
                );
            }
        }
    }
    Ok(())
}

#[test]
fn legacy_facades_keep_acceptance_only_imports_out_of_their_runtime_dependencies() {
    let original = upstream::product_metadata();
    validate_facade_test_dependencies(original).unwrap();
    for (facade, imports) in FACADE_TEST_IMPORTS {
        let index = original["packages"]
            .as_array()
            .unwrap()
            .iter()
            .position(|p| p["name"] == *facade)
            .unwrap();
        for name in *imports {
            let dep = original["packages"][index]["dependencies"]
                .as_array()
                .unwrap()
                .iter()
                .position(|d| d["name"] == *name)
                .unwrap();
            for kind in [Value::Null, Value::String("build".into())] {
                let mut changed = original.clone();
                changed["packages"][index]["dependencies"][dep]["kind"] = kind;
                assert!(validate_facade_test_dependencies(&changed).is_err());
            }
        }
    }
}

#[test]
fn gewyvern_runtime_and_build_closure_are_independent_of_the_control_stack() {
    validate_gewyvern_runtime_isolation(upstream::product_metadata()).unwrap();
}

#[test]
fn product_cleanup_closeout_does_not_promote_native_vm_or_retire_legacy_apis() {
    use serde_json::json;
    let state: Value =
        serde_json::from_str(include_str!("../project/leselang-repository.json")).unwrap();
    let closeout = &state["product_cleanup"];
    assert_eq!(closeout["scope"], "post-split-product-integration");
    assert_eq!(closeout["state"], "complete");
    assert_eq!(closeout["gate"], "tests/leselang_repository_tdd.rs");
    assert_eq!(
        closeout["retained_compatibility"],
        json!(["leselang-hir", "leselang-vm"])
    );
    assert_eq!(
        closeout["retained_product_adapters"],
        json!([
            "leselang-command",
            "leselang-host-contract",
            "leselang-observe",
            "leselang-ui"
        ])
    );
    assert_eq!(
        closeout["non_blocking_followups"],
        json!([
            "legacy-api-retirement",
            "native-full-vm-parity",
            "crates-io-publication"
        ])
    );
    assert_eq!(
        closeout["next_focus"],
        "gewyvern-capability-field-validation"
    );
    assert_eq!(closeout["field_validation"], "docs/field-validation.md");
    assert!(
        root()
            .join(closeout["field_validation"].as_str().unwrap())
            .is_file()
    );
    for key in ["retained_compatibility", "retained_product_adapters"] {
        for name in closeout[key].as_array().unwrap() {
            assert!(
                upstream::product_metadata()["packages"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|package| { package["name"] == *name && package["source"].is_null() })
            );
        }
    }
    for flag in [
        "source_exporter_retired",
        "frozen_snapshots_retired",
        "frozen_core_manifests_retired",
        "frozen_core_compilation_retired",
        "migration_oracle_harness_retired",
        "consumer_acceptance_uses_inventory",
    ] {
        assert_eq!(state["compatibility_cleanup"][flag], true);
    }
    for flag in ["registry_published", "complete_vm", "full_extraction_gate"] {
        assert_eq!(state[flag], false);
    }
    assert_eq!(
        state["compatibility_cleanup"]["local_facades_retired"],
        false
    );
    validate_identity(upstream::product_metadata()).unwrap();
    validate_compatibility_isolation(upstream::product_metadata()).unwrap();
    validate_gewyvern_runtime_isolation(upstream::product_metadata()).unwrap();
    validate_facade_test_dependencies(upstream::product_metadata()).unwrap();
}

#[test]
fn runtime_isolation_follows_transitive_aliases_and_build_edges_but_not_test_edges() {
    use serde_json::json;
    let original = json!({
        "packages": [
            {"id": "root", "name": "gewyvern"},
            {"id": "adapter", "name": "ordinary-adapter"},
            {"id": "language", "name": "leselang-hir-core"}
        ],
        "resolve": {"nodes": [
            {"id": "root", "deps": [{"name": "alias", "pkg": "adapter", "dep_kinds": [{"kind": null}]}]},
            {"id": "adapter", "deps": [{"name": "not_obviously_language", "pkg": "language", "dep_kinds": [{"kind": "dev"}]}]},
            {"id": "language", "deps": []}
        ]}
    });
    validate_gewyvern_runtime_isolation(&original).unwrap();
    for name in [
        "leselang-hir-core",
        "leserpent-vm",
        "leserpentd",
        "leselang",
        "leserpent",
    ] {
        for kinds in [
            json!([{"kind": null}]),
            json!([{"kind": "build", "target": "cfg(target_os = \"linux\")"}]),
            json!([{"kind": "dev"}, {"kind": null}]),
        ] {
            let mut changed = original.clone();
            changed["packages"][2]["name"] = json!(name);
            changed["resolve"]["nodes"][1]["deps"][0]["dep_kinds"] = kinds;
            assert!(
                validate_gewyvern_runtime_isolation(&changed)
                    .unwrap_err()
                    .contains(name)
            );
        }
    }
    for kinds in [json!([]), json!([{}]), json!([{"kind": "unknown"}])] {
        let mut changed = original.clone();
        changed["resolve"]["nodes"][1]["deps"][0]["dep_kinds"] = kinds;
        assert!(validate_gewyvern_runtime_isolation(&changed).is_err());
    }
    let mut missing = original.clone();
    missing["resolve"] = Value::Null;
    assert!(validate_gewyvern_runtime_isolation(&missing).is_err());
    let mut missing = original.clone();
    missing["resolve"]["nodes"][0]["deps"][0]["pkg"] = json!("missing");
    assert!(validate_gewyvern_runtime_isolation(&missing).is_err());
    let mut cycle = original;
    cycle["resolve"]["nodes"][1]["deps"]
        .as_array_mut()
        .unwrap()
        .push(json!({"pkg": "root", "dep_kinds": [{"kind": null}]}));
    validate_gewyvern_runtime_isolation(&cycle).unwrap();
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
fn source_inventory_covers_every_pinned_core_file_without_reading_local_snapshots() {
    let recorded = upstream::source_inventory();
    let mut actual = BTreeMap::new();
    for name in CORES {
        for (path, bytes) in inventory(&upstream::checkout().join("crates").join(name)) {
            let relative = format!(
                "crates/{name}/{}",
                path.to_str().unwrap().replace('\\', "/")
            );
            recorded.verify_file(&relative, &bytes).unwrap();
            assert!(actual.insert(relative, bytes.len() as u64).is_none());
        }
    }
    assert_eq!(actual.len(), 166);
    assert_eq!(
        recorded
            .files
            .iter()
            .map(|file| (file.path.clone(), file.bytes))
            .collect::<BTreeMap<_, _>>(),
        actual,
        "native file additions/removals require an explicit evidence review"
    );
}

#[test]
fn retired_core_directories_are_navigation_only_not_hidden_source_copies() {
    validate_identity(product_metadata()).unwrap();
    for name in CORES {
        let directory = root().join("crates").join(name);
        let entries: Vec<_> = fs::read_dir(&directory)
            .unwrap()
            .map(|entry| {
                let entry = entry.unwrap();
                assert!(
                    entry.file_type().unwrap().is_file(),
                    "no nested native source tree may return"
                );
                entry.file_name()
            })
            .collect();
        assert_eq!(entries, [std::ffi::OsString::from("README.md")]);
        let pointer = fs::read_to_string(directory.join("README.md")).unwrap();
        assert!(pointer.lines().count() <= 40);
        assert!(pointer.contains(&format!("{REPOSITORY}/tree/{REVISION}/crates/{name}")));
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
    assert_eq!(state["product_source_state"], "pinned-source-inventory");
    let cleanup = &state["compatibility_cleanup"];
    assert_eq!(cleanup["root_tests_use_product_owners"], true);
    assert_eq!(cleanup["foreign_consumer_uses_published_facades"], true);
    assert_eq!(cleanup["ci_uses_pinned_upstream_consumer"], true);
    assert_eq!(
        cleanup["allowed_local_facade_edge"],
        "leselang-vm -> leselang-hir"
    );
    assert_eq!(cleanup["source_exporter_retired"], true);
    assert_eq!(cleanup["local_facades_retired"], false);
    assert_eq!(cleanup["frozen_snapshots_retired"], true);
    for flag in ["registry_published", "complete_vm", "full_extraction_gate"] {
        assert_eq!(state[flag], false);
    }
    let workflow = include_str!("../.github/workflows/ci.yml");
    assert!(workflow.contains("--test leselang_repository_tdd"));
    assert!(workflow.contains("check leselang"));
    assert!(!workflow.contains("package leselang"));
    let proof: Value = serde_json::from_slice(
        &fs::read(root().join(state["pinned_consumer_verification"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    assert_eq!(proof["pinned_revision"], state["pinned_revision"]);
    assert_eq!(proof["ci_uses_pinned_upstream_consumer"], true);
    assert_eq!(proof["source_exporter_is_ci_stage"], false);
    for flag in [
        "source_exporter_retired",
        "complete_vm",
        "full_extraction_gate",
    ] {
        assert_eq!(proof[flag], false);
    }
    assert!(include_str!("../docs/development.md").contains("cargo dev check leselang"));
    let retirement: Value = serde_json::from_slice(
        &fs::read(root().join(state["cleanup_verification"].as_str().unwrap())).unwrap(),
    )
    .unwrap();
    assert_eq!(retirement["pinned_revision"], state["pinned_revision"]);
    assert_eq!(retirement["source_exporter_retired"], true);
    assert_eq!(retirement["local_facades_retired"], false);
    assert_eq!(retirement["frozen_snapshots_retired"], false);
    assert_eq!(retirement["complete_vm"], false);
    assert_eq!(retirement["full_extraction_gate"], false);
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
