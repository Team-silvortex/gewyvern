use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn production_source(path: impl AsRef<Path>) -> String {
    let source = fs::read_to_string(path).expect("source file must be readable");
    source
        .split_once("\n#[cfg(test)]\n")
        .map_or(source.as_str(), |(production, _)| production)
        .to_string()
}

#[test]
fn critical_runtime_sources_do_not_use_panic_shortcuts_in_production() {
    let root = repository_root();
    for relative in [
        "src/runtime.rs",
        "crates/gewylang-compiler/src/lowering.rs",
        "crates/leserpent-runtime/src/lib.rs",
    ] {
        let production = production_source(root.join(relative));
        for forbidden in ["panic!(", ".expect(", ".unwrap("] {
            assert!(
                !production.contains(forbidden),
                "{relative} production code contains forbidden panic shortcut '{forbidden}'"
            );
        }
    }
}

#[test]
fn recoverable_startup_modules_do_not_terminate_the_process() {
    let root = repository_root();
    for relative in [
        "src/main/cli.rs",
        "src/main/startup.rs",
        "src/main/cli_validation.rs",
        "src/main/helpers.rs",
        "src/main/preflight.rs",
        "src/main/binding_demo.rs",
        "src/main/diagnostics_mode.rs",
        "src/main/output_collection.rs",
        "src/main/render_dispatch.rs",
        "src/serve_runtime.rs",
        "src/data_api/service.rs",
    ] {
        let source = fs::read_to_string(root.join(relative)).expect("source file must be readable");
        assert!(
            !source.contains("process::exit(") && !source.contains("std::process::exit("),
            "{relative} must return errors to the main process boundary"
        );
    }
}

#[test]
fn critical_cli_pipeline_does_not_use_panic_shortcuts_in_production() {
    let root = repository_root();
    for relative in [
        "src/main/cli.rs",
        "src/main/helpers.rs",
        "src/main/binding_demo.rs",
        "src/main/diagnostics_mode.rs",
        "src/main/output_collection.rs",
        "src/main/render_dispatch.rs",
        "src/serve_runtime.rs",
        "src/data_api/service.rs",
    ] {
        let production = production_source(root.join(relative));
        for forbidden in ["panic!(", ".expect(", ".unwrap("] {
            assert!(
                !production.contains(forbidden),
                "{relative} production code contains forbidden panic shortcut '{forbidden}'"
            );
        }
    }
}

#[test]
fn continuous_integration_enforces_all_primary_product_gates() {
    let workflow = fs::read_to_string(repository_root().join(".github/workflows/ci.yml"))
        .expect("CI workflow must exist");
    for required in [
        "cargo fmt --all -- --check",
        "cargo clippy --workspace --all-targets --all-features --locked -- -D warnings",
        "cargo build --workspace --all-targets --all-features --locked",
        "cargo test --workspace --all-features --locked",
        "npm run check:frontend --prefix apps/leserpent",
        "npm run verify:frontend-package --prefix apps/leserpent",
        "dotnet test apps/leserpent/leserpent.slnx",
        "dotnet build apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj",
    ] {
        assert!(
            workflow.contains(required),
            "CI workflow is missing required gate '{required}'"
        );
    }
}
