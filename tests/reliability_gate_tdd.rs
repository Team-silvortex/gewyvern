use std::fs;
use std::path::{Path, PathBuf};

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn production_source(path: impl AsRef<Path>) -> String {
    let source = fs::read_to_string(path).expect("source file must be readable");
    production_source_text(&source)
}

fn production_source_text(source: &str) -> String {
    source
        .lines()
        .take_while(|line| *line != "#[cfg(test)]" && !line.starts_with("#[cfg(all(test, "))
        .filter(|line| !line.trim_start().starts_with("//"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn production_scan_excludes_test_only_suffixes_but_keeps_feature_gated_production() {
    for gate in ["#[cfg(test)]", "#[cfg(all(test, feature = \"vm-bridge\"))]"] {
        let source = format!("fn production() {{}}\n{gate}\nmod tests {{ panic!(); }}");
        assert_eq!(production_source_text(&source), "fn production() {}");
    }
    for gate in [
        "#[cfg(feature = \"vm-bridge\")]",
        "#[cfg(any(test, feature = \"vm-bridge\"))]",
        "#[cfg(all(feature = \"vm-bridge\", unix))]",
    ] {
        let source = format!("{gate}\nfn production() {{ panic!(); }}");
        assert!(production_source_text(&source).contains("panic!("));
    }
    assert!(
        production_source_text("fn production() { panic!(); }\n#[cfg(test)]\nmod tests {}")
            .contains("panic!(")
    );
}

#[test]
fn critical_runtime_sources_do_not_use_panic_shortcuts_in_production() {
    let root = repository_root();
    for relative in [
        "src/runtime.rs",
        "src/dsl/materializer.rs",
        "crates/gewylang-contract/src/lib.rs",
        "crates/gewylang-syntax/src/entry.rs",
        "crates/gewylang-syntax/src/package.rs",
        "crates/gewylang-syntax/src/parser.rs",
        "crates/gewylang-syntax/src/source_graph.rs",
        "crates/gewylang-compiler/src/lib.rs",
        "crates/gewylang-compiler/src/lowering.rs",
        "crates/gewylang-ir/src/analysis.rs",
        "crates/gewylang-ir/src/binding.rs",
        "crates/gewylang-ir/src/diagnostics.rs",
        "crates/gewylang-ir/src/projection.rs",
        "src/gewyc/projection_host.rs",
        "src/data_api/routing.rs",
        "crates/leselang-command/src/lib.rs",
        "crates/leselang-hir/src/lib.rs",
        "crates/leselang-hir-core/src/lib.rs",
        "crates/leserpent-hir/src/lib.rs",
        "crates/leserpent-hir/src/leserpent_profile.rs",
        "crates/leselang-hir-core/src/parallel_evaluation.rs",
        "crates/leselang-hir-core/src/group_reply.rs",
        "crates/leselang-hir-core/src/catalog_group_source.rs",
        "crates/leselang-hir-core/src/catalog_atomic_source.rs",
        "crates/leselang-hir-core/src/prepared_typing.rs",
        "crates/leselang-hir-core/src/effect_stack.rs",
        "crates/leselang-hir-core/src/effect_stack_session.rs",
        "crates/leselang-hir-core/src/native_group_preparation.rs",
        "crates/leselang-hir-core/src/prepared_parallel.rs",
        "crates/leselang-hir-core/src/prepared_sequence.rs",
        "crates/leselang-hir-core/src/prepared_call_evaluation.rs",
        "crates/leselang-hir-core/src/sequence_evaluation.rs",
        "crates/leselang-host-contract/src/lib.rs",
        "crates/leselang-observe/src/lib.rs",
        "crates/leselang-ui/src/lib.rs",
        "crates/leselang-vm/src/lib.rs",
        "crates/leselang-vm/src/native.rs",
        "crates/leserpent-vm/src/leserpent_profile.rs",
        "crates/leserpent-vm/src/journal.rs",
        "crates/leserpent-runtime/src/lib.rs",
        "crates/leserpent-cli/src/lib.rs",
        "crates/leserpent-cli/src/main.rs",
        "crates/leserpent-domain/src/lib.rs",
        "crates/silvortex-bounded-io/src/lib.rs",
        "crates/leserpentd/src/main.rs",
        "crates/leserpentd/src/debugger.rs",
        "crates/leserpentd/src/events.rs",
        "crates/leserpentd/src/remote.rs",
        "crates/leserpentd/src/web_console_error.rs",
        "crates/leserpentd/src/web_console_orchestra.rs",
        "crates/leserpentd/src/wire.rs",
    ] {
        let production = production_source(root.join(relative));
        for forbidden in ["panic!(", ".expect(", ".unwrap(", "unreachable!("] {
            assert!(
                !production.contains(forbidden),
                "{relative} production code contains forbidden panic shortcut '{forbidden}'"
            );
        }
    }
}

#[test]
fn language_crates_forbid_unsafe_code() {
    let root = repository_root();
    for relative in [
        "crates/gewylang-contract/src/lib.rs",
        "crates/gewylang-syntax/src/lib.rs",
        "crates/gewylang-compiler/src/lib.rs",
        "crates/gewylang-ir/src/lib.rs",
        "crates/leselang-syntax/src/lib.rs",
        "crates/leselang-hir/src/lib.rs",
        "crates/leselang-hir-core/src/lib.rs",
        "crates/leserpent-hir/src/lib.rs",
        "crates/leselang-command/src/lib.rs",
        "crates/leselang-host-contract/src/lib.rs",
        "crates/leselang-vm/src/lib.rs",
        "crates/leselang-ui/src/lib.rs",
        "crates/leselang-observe/src/lib.rs",
    ] {
        let source = fs::read_to_string(root.join(relative)).expect("source file must be readable");
        assert!(
            source.starts_with("#![forbid(unsafe_code)]"),
            "{relative} must forbid unsafe code at the crate boundary"
        );
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
        "cargo audit --no-yanked -D warnings",
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

    for action in [
        "actions/checkout",
        "actions/setup-node",
        "actions/setup-dotnet",
    ] {
        let prefix = format!("uses: {action}@");
        let mut observed = false;
        for line in workflow
            .lines()
            .filter(|line| line.trim().starts_with(&prefix))
        {
            observed = true;
            let reference = line
                .trim()
                .strip_prefix(&prefix)
                .and_then(|value| value.split_whitespace().next())
                .expect("action reference must follow the uses declaration");
            assert!(
                reference.len() == 40 && reference.bytes().all(|byte| byte.is_ascii_hexdigit()),
                "{action} must be pinned to an immutable full commit SHA"
            );
        }
        assert!(observed, "CI workflow must use {action}");
    }

    let dotnet_policy = fs::read_to_string(repository_root().join("Directory.Build.props"))
        .expect("shared .NET build policy must exist");
    for required in [
        "<NuGetAudit>true</NuGetAudit>",
        "<NuGetAuditMode>all</NuGetAuditMode>",
        "NU1901;NU1902;NU1903;NU1904",
    ] {
        assert!(
            dotnet_policy.contains(required),
            ".NET CI policy is missing required dependency gate '{required}'"
        );
    }
}

// A focused delimiter guard, not a replacement for parsing the complete YAML.
fn ambiguous_plain_run(line: &str) -> bool {
    let line = line.trim_start();
    let Some(value) = line
        .strip_prefix("run: ")
        .or_else(|| line.strip_prefix("- run: "))
    else {
        return false;
    };
    let value = value.trim();
    if value.starts_with(['\'', '"', '|', '>']) {
        return false;
    }
    value.ends_with(':')
        || value
            .as_bytes()
            .windows(2)
            .any(|pair| pair[0] == b':' && pair[1].is_ascii_whitespace())
}

#[test]
fn workflow_run_delimiters_require_yaml_quoting_or_block_scalars() {
    for bad in [
        "run: cargo test --lib restore_contract_tests::",
        "- run: cargo test --lib restore_contract_tests::  ",
        "run: echo 'status: ready'",
        "run: cargo test --lib restore_contract_tests:: # restore",
    ] {
        assert!(ambiguous_plain_run(bad), "{bad}");
    }
    for valid in [
        "run: |",
        "run: >-",
        "run: 'cargo test --lib restore_contract_tests::'",
        "run: \"cargo test --lib restore_contract_tests::\"",
        "run: cargo test --lib restore_contract_tests::restore",
        "run: curl https://example.invalid",
        "name: Test restore",
    ] {
        assert!(!ambiguous_plain_run(valid), "{valid}");
    }
    for entry in fs::read_dir(repository_root().join(".github/workflows")).unwrap() {
        let path = entry.unwrap().path();
        if !path
            .extension()
            .is_some_and(|ext| ext == "yml" || ext == "yaml")
        {
            continue;
        }
        for (line, value) in fs::read_to_string(&path).unwrap().lines().enumerate() {
            assert!(
                !ambiguous_plain_run(value),
                "{}:{}: quote the complete run value or use a YAML block scalar",
                path.display(),
                line + 1
            );
        }
    }
}

#[test]
fn locked_native_ssh_dependencies_retain_reviewed_security_patches() {
    let lock = fs::read_to_string(repository_root().join("Cargo.lock")).unwrap();
    for (name, minimum) in [
        ("ctutils", "0.4.4"),
        ("tokio", "1.53.3"),
        ("crypto-bigint", "0.7.6"),
    ] {
        let versions = lock
            .split("[[package]]")
            .filter(|block| {
                block
                    .lines()
                    .any(|line| line == format!("name = \"{name}\""))
            })
            .map(|block| {
                let version = block
                    .lines()
                    .find_map(|line| {
                        line.strip_prefix("version = \"")
                            .and_then(|value| value.strip_suffix('"'))
                    })
                    .unwrap();
                semver::Version::parse(version).unwrap()
            })
            .collect::<Vec<_>>();
        assert!(!versions.is_empty(), "missing reviewed dependency {name}");
        for version in versions {
            assert!(
                version.pre.is_empty() && version >= semver::Version::parse(minimum).unwrap(),
                "{name} {version} predates the reviewed security fix"
            );
            if name == "tokio" {
                assert_ne!(
                    version,
                    semver::Version::new(1, 54, 0),
                    "RUSTSEC-2026-0384: do not reintroduce the interval race"
                );
            }
        }
    }
}
