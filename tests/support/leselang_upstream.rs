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

pub const CONSUMER_TESTS: &[&str] = &[
    "core_identity",
    "local_names",
    "scalar_form",
    "fold_preparation",
    "loop_preparation",
    "source_preparation",
    "argument_binding",
    "declaration_source",
    "computation_profile",
    "checkpoint_recovery",
    "continuation_frame",
    "continuation_context",
    "group_graph",
    "recovery_fences",
    "dispatch_lease",
    "dispatch_capacity",
    "dispatch_plan",
    "dispatch_data",
    "retry_schedule",
    "sequence_watermark",
    "successor_chain",
    "sequence_frontier",
    "outcome_summary",
    "capture_correspondence",
    "capture_extension",
    "durable_control",
    "captured_reentry",
    "owned_scope",
    "local_execution",
    "control_session",
    "return_stack",
    "saved_parallel",
    "saved_sequence",
    "saved_call",
    "saved_group",
    "prepared_counter_host",
    "prepared_gui_host",
    "native_vm_embedding",
    "native_host_pipeline",
    "repository_boundary",
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

pub fn load_pinned_inventory(
    product: &Path,
    state: &Value,
    field: &str,
) -> Result<gewyvern::project_status::SourceInventory, String> {
    let path = state[field]
        .as_str()
        .ok_or("missing source inventory path")?;
    let inventory = gewyvern::project_status::SourceInventory::load_from_repository(product, path)?;
    if state["remote_url"] != inventory.repository || state["pinned_revision"] != inventory.revision
    {
        return Err("source inventory does not match the pinned repository and revision".into());
    }
    Ok(inventory)
}

pub fn source_inventory() -> &'static gewyvern::project_status::SourceInventory {
    static INVENTORY: OnceLock<gewyvern::project_status::SourceInventory> = OnceLock::new();
    INVENTORY.get_or_init(|| {
        let state: Value =
            serde_json::from_str(include_str!("../../project/leselang-repository.json")).unwrap();
        load_pinned_inventory(product_root(), &state, "source_inventory").unwrap()
    })
}

pub fn consumer_inventory() -> &'static gewyvern::project_status::SourceInventory {
    static INVENTORY: OnceLock<gewyvern::project_status::SourceInventory> = OnceLock::new();
    INVENTORY.get_or_init(|| {
        let state: Value =
            serde_json::from_str(include_str!("../../project/leselang-repository.json")).unwrap();
        load_pinned_inventory(product_root(), &state, "consumer_inventory").unwrap()
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

pub fn recorded_source(
    native: &Path,
    path: &str,
    recorded: &gewyvern::project_status::SourceInventory,
) -> Result<Vec<u8>, String> {
    recorded.validate()?;
    let file = recorded
        .file(path)
        .ok_or_else(|| format!("unrecorded acceptance source: {path}"))?;
    let mut actual = native.to_path_buf();
    for component in path.split('/') {
        actual.push(component);
        let metadata = fs::symlink_metadata(&actual)
            .map_err(|error| format!("pinned acceptance source is unavailable: {error}"))?;
        if metadata.file_type().is_symlink() {
            return Err("pinned acceptance source must not follow symlinks".into());
        }
    }
    let bytes = silvortex_bounded_io::read_bounded_utf8_regular_file(&actual, file.bytes)
        .map_err(|error| format!("pinned acceptance source is unavailable: {error}"))?
        .into_bytes();
    recorded.verify_file(path, &bytes)?;
    Ok(bytes)
}

pub fn assert_recorded_source(path: &str) {
    assert!(is_core_source(relative(path)));
    recorded_source(checkout(), path, source_inventory()).unwrap();
}

pub fn assert_consumer_file(path: &str, kind: Option<&str>) {
    let actual = checkout().join("consumer").join(relative(path));
    recorded_source(
        checkout(),
        &format!("consumer/{path}"),
        consumer_inventory(),
    )
    .unwrap();
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
    assert!(
        CONSUMER_TESTS.contains(&name),
        "unmapped upstream test: {name}"
    );
    assert_consumer_file(&format!("tests/{name}.rs"), Some("test"));
}

pub fn assert_complete_consumer_inventory() {
    let expected: BTreeSet<_> = CONSUMER_TESTS.iter().copied().collect();
    assert_eq!(expected.len(), CONSUMER_TESTS.len());
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
    let mut paths: BTreeSet<_> = CONSUMER_TESTS
        .iter()
        .map(|name| format!("consumer/tests/{name}.rs"))
        .collect();
    paths.extend([
        "consumer/examples/native_counter.rs".into(),
        "consumer/tests/support/native_checkpoint.rs".into(),
        "consumer/tests/support/native_group_checkpoint.rs".into(),
    ]);
    assert_eq!(
        consumer_inventory()
            .files
            .iter()
            .map(|file| file.path.clone())
            .collect::<BTreeSet<_>>(),
        paths,
        "recorded consumer source inventory changed"
    );
    for name in CONSUMER_TESTS {
        assert_consumer_test(name);
    }
    assert_consumer_file("examples/native_counter.rs", Some("example"));
    for name in ["native_checkpoint", "native_group_checkpoint"] {
        assert_consumer_file(&format!("tests/support/{name}.rs"), None);
    }
}
