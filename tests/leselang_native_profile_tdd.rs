use std::{fs, path::Path, process::Command};

fn root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
}

fn tree(features: &[&str]) -> String {
    let output = Command::new(env!("CARGO"))
        .args([
            "tree",
            "--locked",
            "--offline",
            "-p",
            "leselang-hir",
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
    assert!(output.status.success(), "{output:?}");
    String::from_utf8(output.stdout).unwrap()
}

#[test]
fn native_profile_has_only_language_workspace_dependencies_and_no_product_contract() {
    let dependency_tree = tree(&["--no-default-features"]);
    for required in ["leselang-hir", "leselang-runtime-core", "leselang-syntax"] {
        assert!(
            dependency_tree
                .lines()
                .any(|line| line.starts_with(required))
        );
    }
    for forbidden in [
        "leselang-host-contract",
        "silvortex-identity",
        "leserpent",
        "leselang-vm",
        "rusqlite",
        "gewyvern",
    ] {
        assert!(
            !dependency_tree
                .lines()
                .any(|line| line.starts_with(forbidden)),
            "native dependency closure gained {forbidden}"
        );
    }
}

#[test]
fn default_and_explicit_profiles_retain_the_existing_host_contract() {
    for flags in [
        &[][..],
        &["--no-default-features", "--features", "leserpent-profile"][..],
    ] {
        let dependency_tree = tree(flags);
        assert!(dependency_tree.contains("leselang-host-contract v"));
        assert!(dependency_tree.contains("silvortex-identity v"));
    }
}

#[test]
fn reference_implementation_is_optional_without_changing_released_root_paths() {
    let manifest = fs::read_to_string(root().join("crates/leselang-hir/Cargo.toml")).unwrap();
    assert!(manifest.contains("default = [\"leserpent-profile\"]"));
    assert!(manifest.contains("leserpent-profile = [\"dep:leselang-host-contract\"]"));
    assert!(manifest.contains("path = \"../leselang-host-contract\", optional = true"));
    let modules = fs::read_to_string(root().join("crates/leselang-hir/src/lib.rs")).unwrap();
    assert!(
        modules.contains(
            "#[cfg(feature = \"leserpent-profile\")]\ninclude!(\"leserpent_profile.rs\");"
        )
    );
    for module in [
        "catalog_program",
        "catalog_group_source",
        "catalog_source",
        "effect_session",
        "effect_stack",
        "effect_stack_session",
        "native_group_preparation",
        "prepared_parallel",
        "prepared_sequence",
        "prepared_call_evaluation",
        "parallel_evaluation",
        "group_reply",
        "ir",
    ] {
        assert!(modules.contains(&format!("pub mod {module};")));
    }
    let profile =
        fs::read_to_string(root().join("crates/leselang-hir/src/leserpent_profile.rs")).unwrap();
    for released in [
        "pub mod computation;",
        "pub mod host_call;",
        "pub mod result_field;",
        "pub enum Effect",
        "pub fn lower(",
        "pub fn authorize(",
        "pub fn canonical_source(",
    ] {
        assert!(
            profile.contains(released),
            "missing released path {released}"
        );
    }
}

#[test]
fn ci_and_docs_keep_native_verticals_separate_from_reference_compatibility() {
    let workflow = fs::read_to_string(root().join(".github/workflows/ci.yml")).unwrap();
    assert!(workflow.contains("cargo +1.98.0 test -p leselang-hir --locked --no-default-features"));
    assert!(workflow.contains(
        "cargo +1.98.0 clippy -p leselang-hir --locked --no-default-features --all-targets -- -D warnings"
    ));
    assert!(workflow.contains("cargo test --workspace --all-features --locked"));
    assert!(workflow.contains(
        "rustfmt +1.98.0 --edition 2024 --check crates/leselang-hir/src/leserpent_profile.rs"
    ));
    let docs = fs::read_to_string(root().join("docs/leselang-embedding.md")).unwrap();
    for boundary in [
        "default-features = false",
        "leserpent-profile",
        "Cargo features are additive",
        "not the complete independent VM or repository extraction gate",
    ] {
        assert!(docs.contains(boundary));
    }
    for name in [
        "catalog_source",
        "catalog_group_source",
        "bound_projection_session",
        "effect_session",
        "effect_stack",
        "parallel_evaluation",
        "group_reply",
        "group_reply_reentry",
        "group_source",
        "native_group_preparation",
        "prepared_parallel",
        "prepared_sequence",
        "prepared_call_evaluation",
        "sequence_evaluation",
    ] {
        let source =
            fs::read_to_string(root().join(format!("crates/leselang-hir/tests/{name}.rs")))
                .unwrap();
        assert!(!source.contains("#![cfg(feature = \"leserpent-profile\")]"));
    }
}
