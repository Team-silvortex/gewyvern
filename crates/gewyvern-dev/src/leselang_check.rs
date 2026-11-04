use std::collections::BTreeSet;
use std::ffi::OsString;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use serde_json::Value;

use super::{ProcessSpec, run_one};

const REPOSITORY: &str = "https://github.com/Team-silvortex/leselang";
const CORES: &[&str] = &[
    "leselang-syntax",
    "leselang-runtime-core",
    "leselang-hir-core",
];
const NATIVE: &[&str] = &[
    "leselang-syntax",
    "leselang-runtime-core",
    "leselang-hir-core",
    "leselang-hir",
    "leselang-vm",
];
const CONSUMER: &str = "leselang-native-source-consumer";
const FIXTURE: &str = "tests/fixtures/leselang-product-hir-boundary/Cargo.toml";

#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Options {
    dry_run: bool,
}

impl Options {
    pub(crate) fn parse(arguments: impl Iterator<Item = String>) -> Result<Self, String> {
        let mut dry_run = false;
        for argument in arguments {
            match argument.as_str() {
                "--dry-run" if !dry_run => dry_run = true,
                _ => {
                    return Err(format!(
                        "unknown or repeated Leselang check option `{argument}`"
                    ));
                }
            }
        }
        Ok(Self { dry_run })
    }
}

struct Pin {
    source: String,
    version: String,
}

impl Pin {
    fn read(state: &Value) -> Result<Self, String> {
        let revision = text(state, "pinned_revision")?;
        let version = text(state, "package_version")?;
        if text(state, "remote_url")? != REPOSITORY
            || revision.len() != 40
            || !revision
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            || version.split('.').count() != 3
            || !version
                .split('.')
                .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return Err(
                "Leselang handoff must name the reviewed repository, full revision and version"
                    .into(),
            );
        }
        Ok(Self {
            source: format!("git+{REPOSITORY}?rev={revision}#{revision}"),
            version: version.into(),
        })
    }
}

fn text<'a>(value: &'a Value, key: &str) -> Result<&'a str, String> {
    value[key].as_str().ok_or_else(|| format!("missing {key}"))
}

fn array<'a>(value: &'a Value, key: &str) -> Result<&'a [Value], String> {
    value[key]
        .as_array()
        .map(Vec::as_slice)
        .ok_or_else(|| format!("missing {key}"))
}

fn absolute(value: &Value, key: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(text(value, key)?);
    if !path.is_absolute()
        || path
            .components()
            .any(|part| matches!(part, Component::ParentDir | Component::CurDir))
    {
        return Err(format!("{key} must be an absolute normalized Cargo path"));
    }
    Ok(path)
}

fn unique<'a>(metadata: &'a Value, name: &str) -> Result<&'a Value, String> {
    let mut matches = array(metadata, "packages")?
        .iter()
        .filter(|p| p["name"] == name);
    let package = matches
        .next()
        .ok_or_else(|| format!("missing package {name}"))?;
    if matches.next().is_some() {
        return Err(format!("duplicate package identity for {name}"));
    }
    Ok(package)
}

fn pinned_root(
    metadata: &Value,
    names: &[&str],
    pin: &Pin,
    product: &Path,
) -> Result<PathBuf, String> {
    let mut checkout = None;
    for name in names {
        let package = unique(metadata, name)?;
        if text(package, "source")? != pin.source
            || text(package, "version")? != pin.version
            || array(metadata, "workspace_members")?.contains(&package["id"])
        {
            return Err(format!("{name} is not the pinned external native package"));
        }
        let manifest = absolute(package, "manifest_path")?;
        let root = manifest
            .ancestors()
            .nth(3)
            .ok_or("invalid native crate layout")?;
        if manifest != root.join("crates").join(name).join("Cargo.toml")
            || root.starts_with(product)
            || checkout.as_ref().is_some_and(|previous| previous != root)
        {
            return Err(format!(
                "{name} must come from the same external Cargo checkout"
            ));
        }
        checkout = Some(root.to_path_buf());
    }
    checkout.ok_or_else(|| "no native package identities requested".into())
}

fn validate_consumer(metadata: &Value, checkout: &Path, pin: &Pin) -> Result<(), String> {
    let consumer = unique(metadata, CONSUMER)?;
    if array(metadata, "workspace_members")? != [consumer["id"].clone()]
        || absolute(metadata, "workspace_root")? != checkout.join("consumer")
    {
        return Err("native consumer must own a separate upstream workspace".into());
    }
    let mut ids = BTreeSet::new();
    for name in NATIVE.iter().copied().chain([CONSUMER]) {
        let package = unique(metadata, name)?;
        let expected = if name == CONSUMER {
            checkout.join("consumer/Cargo.toml")
        } else {
            checkout.join("crates").join(name).join("Cargo.toml")
        };
        if !package["source"].is_null()
            || text(package, "version")? != pin.version
            || text(package, "license")? != "MIT"
            || absolute(package, "manifest_path")? != expected
        {
            return Err(format!("upstream consumer substituted {name}"));
        }
        ids.insert(text(package, "id")?);
        let features = package["features"].as_object().ok_or("missing features")?;
        if ["leserpent-profile", "sqlite-journal"]
            .iter()
            .any(|key| features.contains_key(*key))
        {
            return Err(format!("{name} still exposes a product feature"));
        }
        for dependency in array(package, "dependencies")? {
            let dependency_name = text(dependency, "name")?;
            if NATIVE.contains(&dependency_name) {
                if absolute(dependency, "path")? != checkout.join("crates").join(dependency_name) {
                    return Err(format!("{name} has a foreign native dependency"));
                }
            } else if !["serde", "serde_json"].contains(&dependency_name)
                || !dependency["path"].is_null()
            {
                return Err(format!(
                    "{name} has an unreviewed dependency: {dependency_name}"
                ));
            }
        }
    }
    for package in array(metadata, "packages")? {
        if !ids.contains(text(package, "id")?)
            && (!text(package, "source")?.starts_with("registry+")
                || text(package, "name")?.starts_with("leserpent")
                || text(package, "name")? == "rusqlite")
        {
            return Err(
                "native consumer closure contains a non-registry or product package".into(),
            );
        }
    }
    Ok(())
}

fn cargo() -> OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

fn metadata(root: &Path, manifest: &Path) -> Result<Value, String> {
    let output = Command::new(cargo())
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--offline",
            "--all-features",
            "--manifest-path",
        ])
        .arg(manifest)
        .current_dir(root)
        .output()
        .map_err(|error| format!("read Cargo metadata for {}: {error}", manifest.display()))?;
    if !output.status.success() {
        return Err(format!(
            "locked offline metadata failed for {}: {}. Fetch the locked dependencies before checking; no fallback to local snapshots is allowed",
            manifest.display(),
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|error| format!("invalid Cargo metadata: {error}"))
}

fn specs(root: &Path, checkout: &Path, target: &Path) -> Vec<ProcessSpec> {
    // The former product migration harness also compiled private core tests.
    // Execute their real owner, not another copied language implementation.
    let mut arguments: Vec<OsString> = [
        "test",
        "-p",
        "leselang-hir-core",
        "--lib",
        "--all-features",
        "--locked",
        "--offline",
        "--profile",
        "lean",
        "--manifest-path",
    ]
    .into_iter()
    .map(Into::into)
    .collect();
    arguments.push(checkout.join("Cargo.toml").into_os_string());
    arguments.push("--target-dir".into());
    arguments.push(target.as_os_str().into());
    let mut stages = vec![ProcessSpec::new(
        "pinned-native-hir-oracles",
        cargo(),
        arguments,
        root,
    )];
    for (label, manifest) in [
        (
            "pinned-native-consumer",
            checkout.join("consumer/Cargo.toml"),
        ),
        ("pinned-product-compatibility", root.join(FIXTURE)),
    ] {
        for action in ["test", "clippy"] {
            let mut arguments: Vec<OsString> = [
                action,
                "--all-targets",
                "--all-features",
                "--locked",
                "--offline",
                "--profile",
                "lean",
                "--manifest-path",
            ]
            .into_iter()
            .map(Into::into)
            .collect();
            arguments.push(manifest.as_os_str().into());
            arguments.push("--target-dir".into());
            arguments.push(target.as_os_str().into());
            if action == "clippy" {
                arguments.extend(["--", "-D", "warnings"].map(Into::into));
            }
            stages.push(ProcessSpec::new(label, cargo(), arguments, root));
        }
    }
    for mode in [
        None,
        Some("--cancel"),
        Some("--queued"),
        Some("--queued-cancel-pending"),
        Some("--queued-cancel-reply"),
    ] {
        let mut arguments: Vec<OsString> = [
            "run",
            "--example",
            "native_counter",
            "--locked",
            "--offline",
            "--profile",
            "lean",
            "--manifest-path",
        ]
        .into_iter()
        .map(Into::into)
        .collect();
        arguments.push(checkout.join("consumer/Cargo.toml").into_os_string());
        arguments.push("--target-dir".into());
        arguments.push(target.as_os_str().into());
        if let Some(mode) = mode {
            arguments.extend(["--", mode].map(Into::into));
        }
        stages.push(ProcessSpec::new(
            "pinned-native-example",
            cargo(),
            arguments,
            root,
        ));
    }
    stages
}

pub(crate) fn check(root: &Path, options: &Options) -> Result<(), String> {
    let state = std::fs::read(root.join("project/leselang-repository.json"))
        .map_err(|error| format!("read Leselang handoff: {error}"))?;
    let pin = Pin::read(
        &serde_json::from_slice(&state)
            .map_err(|error| format!("invalid Leselang handoff: {error}"))?,
    )?;
    let product = metadata(root, &root.join("Cargo.toml"))?;
    let fixture = metadata(root, &root.join(FIXTURE))?;
    let checkout = pinned_root(&product, CORES, &pin, root)?;
    if pinned_root(&fixture, NATIVE, &pin, root)? != checkout {
        return Err("product and foreign consumer resolve different Leselang checkouts".into());
    }
    let canonical = checkout
        .canonicalize()
        .map_err(|error| format!("resolve native checkout: {error}"))?;
    let canonical_product = root
        .canonicalize()
        .map_err(|error| format!("resolve product root: {error}"))?;
    if canonical.starts_with(canonical_product) {
        return Err("native checkout resolves inside the product tree".into());
    }
    let consumer = metadata(root, &checkout.join("consumer/Cargo.toml"))?;
    validate_consumer(&consumer, &checkout, &pin)?;
    let target = absolute(&product, "target_directory")?;
    if target.starts_with(&checkout) {
        return Err("verification target must not write inside the native checkout".into());
    }
    eprintln!(
        "[leselang] source={}, version={}, cache={}",
        pin.source,
        pin.version,
        target.display()
    );
    // Validate every identity before running either consumer. Never copy or rewrite upstream sources.
    for stage in specs(root, &checkout, &target) {
        run_one(stage, options.dry_run)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn pin() -> Pin {
        Pin::read(&json!({"remote_url": REPOSITORY, "pinned_revision": "a".repeat(40), "package_version": "0.1.0"})).unwrap()
    }

    fn checkout() -> PathBuf {
        std::env::temp_dir().join("leselang-cargo-checkout")
    }

    fn product() -> PathBuf {
        std::env::temp_dir().join("gewyvern-product")
    }

    fn fixture() -> Value {
        json!({
            "workspace_members": [],
            "packages": NATIVE.iter().map(|name| json!({
                "name": name, "id": name, "source": pin().source, "version": "0.1.0",
                "manifest_path": checkout().join("crates").join(name).join("Cargo.toml")
            })).collect::<Vec<_>>()
        })
    }

    fn consumer() -> Value {
        let mut value = fixture();
        value["workspace_root"] = json!(checkout().join("consumer"));
        value["workspace_members"] = json!([CONSUMER]);
        let packages = value["packages"].as_array_mut().unwrap();
        packages.push(json!({"name": CONSUMER, "id": CONSUMER, "version": "0.1.0", "manifest_path": checkout().join("consumer/Cargo.toml")}));
        for package in packages {
            package["source"] = Value::Null;
            package["license"] = json!("MIT");
            package["features"] = json!({});
            package["dependencies"] = json!([]);
        }
        value
    }

    #[test]
    fn workflow_routes_without_changing_existing_check_options() {
        for flags in [vec![], vec!["--dry-run"]] {
            let mut args = vec!["check".into(), "leselang".into()];
            args.extend(flags.into_iter().map(String::from));
            assert!(matches!(
                super::super::Workflow::parse(args).unwrap(),
                super::super::Workflow::CheckLeselang(_)
            ));
        }
        for flags in [
            vec!["--dry-run", "--dry-run"],
            vec!["--output", "somewhere"],
            vec!["--restore"],
            vec!["--scope", "all"],
        ] {
            assert!(Options::parse(flags.into_iter().map(String::from)).is_err());
        }
    }

    #[test]
    fn retired_export_command_never_silently_verifies_or_creates_an_output() {
        for suffix in [
            vec![],
            vec!["--verify"],
            vec!["--output", "old-source", "--dry-run"],
        ] {
            let mut arguments = vec!["package".into(), "leselang".into()];
            arguments.extend(suffix.into_iter().map(String::from));
            let error = super::super::Workflow::parse(arguments).unwrap_err();
            assert!(error.contains("source export is retired"));
            assert!(error.contains("cargo dev check leselang"));
            assert!(error.contains("no files were exported"));
        }
    }

    #[test]
    fn pin_rejects_mutable_revisions_and_foreign_sources() {
        let state = json!({"remote_url": REPOSITORY, "pinned_revision": "a".repeat(40), "package_version": "0.1.0"});
        for (key, replacement) in [
            ("remote_url", "../leselang"),
            ("pinned_revision", "main"),
            ("pinned_revision", "abcd1234"),
            ("package_version", "*"),
        ] {
            let mut changed = state.clone();
            changed[key] = json!(replacement);
            assert!(Pin::read(&changed).is_err());
        }
    }

    #[test]
    fn pinned_identity_requires_every_native_package_once() {
        assert_eq!(
            pinned_root(&fixture(), NATIVE, &pin(), &product()).unwrap(),
            checkout()
        );
        for field in ["source", "version", "manifest_path"] {
            let mut changed = fixture();
            changed["packages"][4][field] = json!("wrong");
            assert!(pinned_root(&changed, NATIVE, &pin(), &product()).is_err());
        }
        let mut duplicate = fixture();
        let package = duplicate["packages"][0].clone();
        duplicate["packages"].as_array_mut().unwrap().push(package);
        assert!(pinned_root(&duplicate, NATIVE, &pin(), &product()).is_err());
        let mut missing = fixture();
        missing["packages"].as_array_mut().unwrap().pop();
        assert!(pinned_root(&missing, NATIVE, &pin(), &product()).is_err());
    }

    #[test]
    fn pinned_identity_rejects_local_and_split_checkouts() {
        for location in [product(), checkout().join("other")] {
            let mut changed = fixture();
            changed["packages"][0]["manifest_path"] =
                json!(location.join("crates/leselang-syntax/Cargo.toml"));
            assert!(pinned_root(&changed, NATIVE, &pin(), &product()).is_err());
        }
        let mut changed = fixture();
        changed["workspace_members"] = json!(["leselang-syntax"]);
        assert!(pinned_root(&changed, NATIVE, &pin(), &product()).is_err());
    }

    #[test]
    fn consumer_cannot_enable_product_features_or_optional_foreign_dependencies() {
        validate_consumer(&consumer(), &checkout(), &pin()).unwrap();
        for feature in ["leserpent-profile", "sqlite-journal"] {
            let mut changed = consumer();
            changed["packages"][4]["features"] = json!({feature: []});
            assert!(validate_consumer(&changed, &checkout(), &pin()).is_err());
        }
        for dependency in [
            json!({"name": "leserpent-vm", "optional": true}),
            json!({"name": "rusqlite", "kind": "build"}),
            json!({"name": "leselang-hir", "path": product(), "rename": "native"}),
            json!({"name": "serde", "path": product()}),
        ] {
            let mut changed = consumer();
            changed["packages"][4]["dependencies"] = json!([dependency]);
            assert!(validate_consumer(&changed, &checkout(), &pin()).is_err());
        }
    }

    #[test]
    fn consumer_requires_upstream_layout_and_version_for_every_crate() {
        for field in ["source", "manifest_path", "version", "license"] {
            let mut changed = consumer();
            changed["packages"][2][field] = json!("substituted");
            assert!(validate_consumer(&changed, &checkout(), &pin()).is_err());
        }
        let mut changed = consumer();
        changed["workspace_members"] = json!([CONSUMER, "leselang-syntax"]);
        assert!(validate_consumer(&changed, &checkout(), &pin()).is_err());
        changed = consumer();
        changed["packages"].as_array_mut().unwrap().push(
            json!({"id": "hidden-product", "name": "leserpent-vm", "source": "registry+test"}),
        );
        assert!(validate_consumer(&changed, &checkout(), &pin()).is_err());
    }

    #[test]
    fn plan_uses_shared_cache_without_exports_or_sibling_checkouts() {
        let target = product().join("target");
        let existed = target.exists();
        let stages = specs(&product(), &checkout(), &target);
        assert_eq!(stages.len(), 10);
        let native = &stages[0].arguments;
        assert_eq!(
            &native[..4],
            ["test", "-p", "leselang-hir-core", "--lib"].map(OsString::from)
        );
        let manifest = native
            .iter()
            .position(|arg| arg == "--manifest-path")
            .unwrap();
        assert_eq!(native[manifest + 1], checkout().join("Cargo.toml"));
        for stage in &stages {
            assert_eq!(stage.current_dir, product());
            for required in ["--locked", "--offline", "--profile", "lean"] {
                assert!(stage.arguments.contains(&OsString::from(required)));
            }
            let index = stage
                .arguments
                .iter()
                .position(|arg| arg == "--target-dir")
                .unwrap();
            assert_eq!(stage.arguments[index + 1], target);
            assert!(!stage.rendered().contains("package leselang"));
        }
        for mode in [
            "--cancel",
            "--queued",
            "--queued-cancel-pending",
            "--queued-cancel-reply",
        ] {
            assert_eq!(
                stages
                    .iter()
                    .filter(|stage| stage.arguments.contains(&OsString::from(mode)))
                    .count(),
                1
            );
        }
        for stage in stages {
            run_one(stage, true).unwrap();
        }
        assert_eq!(target.exists(), existed);
    }
}
