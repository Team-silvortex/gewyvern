use serde_json::Value;
use std::collections::BTreeSet;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;

pub const NATIVE_CRATES: &[&str] = &[
    "leselang-syntax",
    "leselang-runtime-core",
    "leselang-hir-core",
    "leselang-hir",
    "leselang-vm",
];

pub const NATIVE_CORES: &[&str] = &[
    "leselang-syntax",
    "leselang-runtime-core",
    "leselang-hir-core",
];

pub const CONSUMER_TESTS: &[(&str, &str)] = &[
    (
        "core_identity",
        "crates/leselang-hir/tests/core_identity.rs",
    ),
    ("local_names", "crates/leselang-hir/tests/local_names.rs"),
    ("scalar_form", "crates/leselang-hir/tests/scalar_form.rs"),
    (
        "fold_preparation",
        "crates/leselang-hir/tests/fold_preparation.rs",
    ),
    (
        "loop_preparation",
        "crates/leselang-hir/tests/loop_preparation.rs",
    ),
    (
        "source_preparation",
        "crates/leselang-hir/tests/source_preparation.rs",
    ),
    (
        "argument_binding",
        "crates/leselang-hir/tests/argument_binding.rs",
    ),
    (
        "declaration_source",
        "crates/leselang-hir/tests/declaration_source.rs",
    ),
    (
        "computation_profile",
        "crates/leselang-hir/tests/computation_profile.rs",
    ),
    (
        "checkpoint_recovery",
        "crates/leselang-runtime-core/tests/checkpoint.rs",
    ),
    (
        "continuation_frame",
        "crates/leselang-runtime-core/tests/continuation.rs",
    ),
    (
        "continuation_context",
        "crates/leselang-runtime-core/tests/continuation_context.rs",
    ),
    (
        "group_graph",
        "crates/leselang-runtime-core/tests/group_graph.rs",
    ),
    (
        "recovery_fences",
        "crates/leselang-runtime-core/tests/recovery_fence.rs",
    ),
    (
        "dispatch_lease",
        "crates/leselang-runtime-core/tests/dispatch_lease.rs",
    ),
    (
        "dispatch_capacity",
        "crates/leselang-runtime-core/tests/dispatch_capacity.rs",
    ),
    (
        "dispatch_plan",
        "crates/leselang-runtime-core/tests/dispatch_plan.rs",
    ),
    (
        "dispatch_data",
        "crates/leselang-runtime-core/tests/dispatch_data.rs",
    ),
    (
        "retry_schedule",
        "crates/leselang-runtime-core/tests/retry_schedule.rs",
    ),
    (
        "sequence_watermark",
        "crates/leselang-runtime-core/tests/sequence_watermark.rs",
    ),
    (
        "successor_chain",
        "crates/leselang-runtime-core/tests/successor_chain.rs",
    ),
    (
        "sequence_frontier",
        "crates/leselang-runtime-core/tests/sequence_frontier.rs",
    ),
    (
        "outcome_summary",
        "crates/leselang-runtime-core/tests/outcome_summary.rs",
    ),
    (
        "capture_correspondence",
        "crates/leselang-runtime-core/tests/capture_correspondence.rs",
    ),
    (
        "capture_extension",
        "crates/leselang-runtime-core/tests/capture_extension.rs",
    ),
    (
        "durable_control",
        "crates/leselang-hir/tests/durable_control.rs",
    ),
    (
        "captured_reentry",
        "crates/leselang-hir/tests/captured_reentry.rs",
    ),
    (
        "owned_scope",
        "crates/leselang-runtime-core/tests/owned_scope.rs",
    ),
    (
        "local_execution",
        "crates/leselang-runtime-core/tests/local_execution.rs",
    ),
    (
        "control_session",
        "crates/leselang-hir/tests/effect_session.rs",
    ),
    ("return_stack", "crates/leselang-hir/tests/effect_stack.rs"),
    (
        "saved_parallel",
        "crates/leselang-hir/tests/saved_parallel.rs",
    ),
    (
        "saved_sequence",
        "crates/leselang-hir/tests/saved_sequence.rs",
    ),
    ("saved_call", "crates/leselang-hir/tests/saved_call.rs"),
    ("saved_group", "crates/leselang-hir/tests/saved_group.rs"),
    (
        "prepared_counter_host",
        "crates/leselang-hir/tests/catalog_group_source.rs",
    ),
    (
        "prepared_gui_host",
        "crates/leselang-hir/tests/prepared_gui_host.rs",
    ),
    (
        "native_vm_embedding",
        "crates/leselang-vm/tests/native_embedding.rs",
    ),
    (
        "native_host_pipeline",
        "crates/leselang-vm/tests/native_host_pipeline.rs",
    ),
];

pub fn product_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

pub fn metadata(manifest: impl AsRef<Path>) -> Value {
    let output = Command::new(env!("CARGO"))
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--offline",
            "--all-features",
            "--manifest-path",
        ])
        .arg(manifest.as_ref())
        .current_dir(product_root())
        .output()
        .expect("Cargo metadata should start");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("Cargo metadata must be JSON")
}

pub fn product_metadata() -> &'static Value {
    static METADATA: OnceLock<Value> = OnceLock::new();
    METADATA.get_or_init(|| metadata(product_root().join("Cargo.toml")))
}

pub fn checkout() -> &'static Path {
    static CHECKOUT: OnceLock<PathBuf> = OnceLock::new();
    CHECKOUT.get_or_init(|| {
        let state: Value =
            serde_json::from_str(include_str!("../../project/leselang-repository.json")).unwrap();
        resolve_checkout(product_metadata(), &state, product_root()).unwrap()
    })
}

pub fn resolve_checkout(
    metadata: &Value,
    state: &Value,
    product: &Path,
) -> Result<PathBuf, String> {
    let packages = metadata["packages"].as_array().ok_or("missing packages")?;
    let members = metadata["workspace_members"]
        .as_array()
        .ok_or("missing members")?;
    let repository = state["remote_url"].as_str().ok_or("missing repository")?;
    let revision = state["pinned_revision"]
        .as_str()
        .ok_or("missing revision")?;
    let version = state["package_version"].as_str().ok_or("missing version")?;
    let source = format!("git+{repository}?rev={revision}#{revision}");
    let mut shared = None;
    for name in NATIVE_CORES {
        let mut matches = packages.iter().filter(|p| p["name"] == *name);
        let package = matches
            .next()
            .ok_or_else(|| format!("missing core {name}"))?;
        if matches.next().is_some()
            || package["source"] != source
            || package["version"] != version
            || members.contains(&package["id"])
        {
            return Err(format!(
                "{name} must have one pinned, external package identity"
            ));
        }
        let manifest = Path::new(
            package["manifest_path"]
                .as_str()
                .ok_or("missing manifest")?,
        );
        let checkout = manifest.ancestors().nth(3).ok_or("missing checkout")?;
        if !manifest.is_absolute()
            || manifest
                .components()
                .any(|part| matches!(part, Component::ParentDir))
            || manifest.starts_with(product)
            || manifest != checkout.join("crates").join(name).join("Cargo.toml")
            || shared.as_deref().is_some_and(|path| path != checkout)
        {
            return Err(format!("{name} does not share the pinned native checkout"));
        }
        shared = Some(checkout.to_path_buf());
    }
    shared.ok_or_else(|| "no native cores".into())
}

fn source_relative<'a>(path: &'a Path, product: &Path) -> std::io::Result<&'a Path> {
    let relative = path.strip_prefix(product).map_err(|_| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "source must be rooted in the product checkout",
        )
    })?;
    if relative.as_os_str().is_empty()
        || !relative
            .components()
            .all(|part| matches!(part, Component::Normal(_)))
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "invalid source path",
        ));
    }
    Ok(relative)
}

fn is_core_source(relative: &Path) -> bool {
    NATIVE_CORES
        .iter()
        .any(|name| relative.starts_with(Path::new("crates").join(name)))
}

pub fn resolve_source_path(path: &Path, product: &Path, native: &Path) -> std::io::Result<PathBuf> {
    let relative = source_relative(path, product)?;
    // Product adapters/facades remain local; core inspection must follow Cargo,
    // never a sibling checkout or the frozen historical source snapshots.
    Ok(if is_core_source(relative) {
        native.join(relative)
    } else {
        product.join(relative)
    })
}

pub fn source_path(path: impl AsRef<Path>) -> std::io::Result<PathBuf> {
    resolve_source_path(path.as_ref(), product_root(), checkout())
}

pub fn read_source(path: impl AsRef<Path>) -> std::io::Result<String> {
    fs::read_to_string(source_path(path)?)
}

pub const RUNTIME_BASELINE_PATH: &str = "docs/provenance/runtime-core-baseline.md";

pub fn read_runtime_baseline(native: &Path, state: &Value) -> Result<String, String> {
    let inspection = &state["documentation_inspection"];
    let evidence = &inspection["runtime_baseline"];
    let revision = state["pinned_revision"].as_str().ok_or("missing pin")?;
    if inspection["owner"] != "cargo-resolved-pinned-upstream"
        || inspection["snapshot_fallback"] != false
        || evidence["revision"] != revision
        || evidence["path"] != RUNTIME_BASELINE_PATH
    {
        return Err("runtime documentation must use the reviewed upstream provenance".into());
    }
    let expected = evidence["sha256"].as_str().ok_or("missing document hash")?;
    if expected.len() != 64
        || !expected
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err("invalid runtime documentation hash".into());
    }
    let source = silvortex_bounded_io::read_bounded_utf8_regular_file(
        &native.join(RUNTIME_BASELINE_PATH),
        256 * 1024,
    )
    .map_err(|error| format!("pinned runtime documentation is unavailable: {error}"))?;
    let digest = ring::digest::digest(&ring::digest::SHA256, source.as_bytes());
    let actual: String = digest
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    if actual != expected {
        return Err("pinned runtime documentation SHA-256 mismatch".into());
    }
    Ok(source)
}

pub fn runtime_baseline_document() -> &'static str {
    static DOCUMENT: OnceLock<String> = OnceLock::new();
    DOCUMENT.get_or_init(|| {
        let state: Value =
            serde_json::from_str(include_str!("../../project/leselang-repository.json")).unwrap();
        read_runtime_baseline(checkout(), &state).unwrap()
    })
}

pub fn source_inventory() -> &'static gewyvern::project_status::SourceInventory {
    static INVENTORY: OnceLock<gewyvern::project_status::SourceInventory> = OnceLock::new();
    INVENTORY.get_or_init(|| {
        let state: Value =
            serde_json::from_str(include_str!("../../project/leselang-repository.json")).unwrap();
        let inventory = gewyvern::project_status::SourceInventory::load_from_repository(
            product_root(),
            state["source_inventory"].as_str().unwrap(),
        )
        .unwrap();
        assert_eq!(inventory.repository, state["remote_url"]);
        assert_eq!(inventory.revision, state["pinned_revision"]);
        inventory
    })
}

pub fn native_metadata() -> &'static Value {
    static METADATA: OnceLock<Value> = OnceLock::new();
    METADATA.get_or_init(|| metadata(checkout().join("Cargo.toml")))
}

pub fn consumer_metadata() -> &'static Value {
    static METADATA: OnceLock<Value> = OnceLock::new();
    METADATA.get_or_init(|| metadata(checkout().join("consumer/Cargo.toml")))
}

fn relative(path: &str) -> &Path {
    let path = Path::new(path);
    assert!(!path.as_os_str().is_empty());
    assert!(
        path.components()
            .all(|part| matches!(part, Component::Normal(_)))
    );
    path
}

pub fn read(path: &str) -> String {
    fs::read_to_string(checkout().join(relative(path))).unwrap()
}

pub fn acceptance_source(
    original: &str,
    product: &Path,
    native: &Path,
    recorded: &gewyvern::project_status::SourceInventory,
) -> Result<Vec<u8>, String> {
    let path = product.join(original);
    let relative = source_relative(&path, product).map_err(|error| error.to_string())?;
    let actual = resolve_source_path(&path, product, native).map_err(|error| error.to_string())?;
    if is_core_source(relative) {
        let key = relative
            .to_str()
            .ok_or("non-UTF8 source path")?
            .replace('\\', "/");
        let file = recorded
            .file(&key)
            .ok_or_else(|| format!("unrecorded core source: {key}"))?;
        let bytes = silvortex_bounded_io::read_bounded_utf8_regular_file(&actual, file.bytes)
            .map_err(|error| format!("pinned acceptance source is unavailable: {error}"))?
            .into_bytes();
        recorded.verify_file(&key, &bytes)?;
        Ok(bytes)
    } else {
        // Legacy facade/product acceptance still belongs here, not upstream.
        fs::read(actual).map_err(|error| error.to_string())
    }
}

pub fn assert_recorded_source(path: &str) {
    assert!(is_core_source(relative(path)));
    acceptance_source(path, product_root(), checkout(), source_inventory()).unwrap();
}

pub fn assert_consumer_file(original: &str, path: &str, kind: Option<&str>) {
    let actual = checkout().join("consumer").join(relative(path));
    let expected =
        acceptance_source(original, product_root(), checkout(), source_inventory()).unwrap();
    assert!(
        expected == fs::read(&actual).unwrap(),
        "upstream acceptance differs: {path}"
    );
    if let Some(kind) = kind {
        let package = consumer_package();
        assert!(
            package["targets"].as_array().unwrap().iter().any(|target| {
                target["kind"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|value| value == kind)
                    && Path::new(target["src_path"].as_str().unwrap()) == actual
            }),
            "upstream consumer does not execute {path}"
        );
    }
}

pub fn consumer_package() -> &'static Value {
    consumer_metadata()["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "leselang-native-source-consumer")
        .unwrap()
}

pub fn assert_consumer_test(name: &str) {
    let (_, original) = CONSUMER_TESTS
        .iter()
        .find(|(candidate, _)| *candidate == name)
        .unwrap_or_else(|| panic!("unmapped upstream test: {name}"));
    assert_consumer_file(original, &format!("tests/{name}.rs"), Some("test"));
}

pub fn assert_complete_consumer_inventory() {
    let mut expected: BTreeSet<_> = CONSUMER_TESTS.iter().map(|(name, _)| *name).collect();
    assert_eq!(expected.len(), CONSUMER_TESTS.len());
    expected.insert("repository_boundary");
    let actual: BTreeSet<_> = consumer_package()["targets"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|target| {
            target["kind"]
                .as_array()
                .unwrap()
                .iter()
                .any(|kind| kind == "test")
        })
        .map(|target| target["name"].as_str().unwrap())
        .collect();
    assert_eq!(actual, expected, "upstream consumer test inventory changed");
    for (name, _) in CONSUMER_TESTS {
        assert_consumer_test(name);
    }
    assert_consumer_file(
        "crates/leselang-vm/examples/native_counter.rs",
        "examples/native_counter.rs",
        Some("example"),
    );
    for name in ["native_checkpoint", "native_group_checkpoint"] {
        assert_consumer_file(
            &format!("crates/leselang-vm/tests/support/{name}.rs"),
            &format!("tests/support/{name}.rs"),
            None,
        );
    }
}
