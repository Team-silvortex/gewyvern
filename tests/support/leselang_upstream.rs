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
        let packages = product_metadata()["packages"].as_array().unwrap();
        let mut matches = packages.iter().filter(|p| p["name"] == "leselang-hir-core");
        let package = matches.next().unwrap();
        assert!(matches.next().is_none());
        let state: Value =
            serde_json::from_str(include_str!("../../project/leselang-repository.json")).unwrap();
        let revision = state["pinned_revision"].as_str().unwrap();
        assert_eq!(
            package["source"],
            format!("git+https://github.com/Team-silvortex/leselang?rev={revision}#{revision}")
        );
        assert_eq!(package["version"], state["package_version"]);
        let manifest = Path::new(package["manifest_path"].as_str().unwrap());
        let checkout = manifest.ancestors().nth(3).unwrap();
        assert!(!checkout.starts_with(product_root()));
        checkout.to_path_buf()
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

pub fn assert_same_source(path: &str) {
    let path = relative(path);
    assert!(
        fs::read(product_root().join(path)).unwrap() == fs::read(checkout().join(path)).unwrap(),
        "upstream source differs: {}",
        path.display()
    );
}

pub fn assert_consumer_file(local: &str, path: &str, kind: Option<&str>) {
    let actual = checkout().join("consumer").join(relative(path));
    assert!(
        fs::read(product_root().join(relative(local))).unwrap() == fs::read(&actual).unwrap(),
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
