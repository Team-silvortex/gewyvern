use std::fs;
use std::path::Path;

#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;

fn read_repo_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read {}: {}", path.display(), err);
    })
}

#[test]
fn field_validation_socket_roundtrip_uses_short_cross_platform_socket_path() {
    let harness = read_repo_file("src/validation_harness/field_smoke.rs");
    let script = read_repo_file("scripts/validation/field_validation_smoke.sh");

    assert!(harness.contains("env::temp_dir()"));
    assert!(harness.contains("gewyvern-field-validation-{}.sock"));
    assert!(!script.contains("/private/tmp/gewyvern-field-validation.sock"));
    assert!(!script.contains("ROUNDTRIP_SOCKET=\"${TMP_DIR}/gewyvern-field-validation.sock\""));
}

#[test]
fn resilience_log_evidence_no_longer_depends_on_shell_grep() {
    let harness = read_repo_file("src/validation_harness/resilience.rs");
    let script = read_repo_file("scripts/validation/runtime_resilience_log_evidence.sh");
    let fault = read_repo_file("scripts/validation/runtime_resilience_fault_injection.sh");

    assert!(harness.contains("line_has_resilience_signal"));
    assert!(harness.contains("backoff_ms="));
    assert!(harness.contains("bounded_tcp_connect"));
    assert!(script.contains("gewyvern_validate"));
    assert!(script.contains("resilience-log-evidence"));
    assert!(fault.contains("resilience-drive-bad-json"));
    assert!(!script.contains("command -v rg"));
    assert!(!script.contains("grep -E"));
    assert!(!fault.contains("command -v nc"));
    assert!(!fault.contains(" nc "));
}

#[test]
fn demo_roundtrip_wrappers_do_not_require_curl_or_python() {
    let socket = read_repo_file("scripts/demos/socket_roundtrip_demo.sh");
    let training = read_repo_file("scripts/demos/training_dataset_roundtrip_demo.sh");
    let external = read_repo_file("scripts/demos/external_engine_roundtrip_demo.sh");

    assert!(socket.contains("socket-roundtrip"));
    assert!(training.contains("training-roundtrip"));
    assert!(external.contains("external-engine-roundtrip"));
    assert!(!socket.contains("curl"));
    assert!(!training.contains("curl"));
    assert!(!external.contains("curl"));
    assert!(!training.contains("python3"));
    assert!(!external.contains("python3"));
}

#[test]
fn linux_probe_docs_call_out_required_privileges() {
    let entrypoints = read_repo_file("docs/script-entrypoints.md");
    let cli_recipes = read_repo_file("docs/cli-recipes.md");

    for doc in [&entrypoints, &cli_recipes] {
        assert!(doc.contains("sudo"));
        assert!(doc.contains("BPF attach privileges"));
        assert!(doc.contains("Operation not permitted"));
    }
}

#[test]
fn native_developer_workflow_owns_locked_build_package_and_deploy_routes() {
    let cargo_config = read_repo_file(".cargo/config.toml");
    let workspace = read_repo_file("Cargo.toml");
    let workflow = read_repo_file("crates/gewyvern-dev/src/main.rs");
    let development = read_repo_file("docs/development.md");
    let packaging = read_repo_file("docs/packaging.md");
    let entrypoints = read_repo_file("docs/script-entrypoints.md");

    assert!(cargo_config.contains("dev = \"run --quiet --locked -p gewyvern-dev --\""));
    assert!(workspace.contains("\"crates/gewyvern-dev\""));
    for command in [
        "cargo dev doctor",
        "cargo dev version check",
        "cargo dev version set",
        "cargo dev check",
        "cargo dev build",
        "cargo dev package linux",
        "cargo dev package control",
        "cargo dev package desktop",
        "cargo dev deploy control",
        "cargo dev deploy desktop",
    ] {
        assert!(
            workflow.contains(command),
            "missing workflow route: {command}"
        );
    }
    assert!(workflow.contains("fn compile_specs"));
    assert!(workflow.contains("if check_only { \"check\" } else { \"build\" }"));
    assert!(workflow.contains("\"--locked\""));
    assert!(workflow.contains("\"--workspace\""));
    assert!(workflow.contains("apps/leserpent/src/Leserpent/Leserpent.csproj"));
    assert!(!workflow.contains("apps/leserpent/leserpent.slnx"));
    assert!(workflow.contains("arguments.push(\"--no-restore\")"));
    assert!(workflow.contains("RestoreLockedMode=true"));
    assert!(workflow.contains("desktop-signature-verify"));
    assert!(workflow.contains("control_bundle_manifest"));
    assert!(workflow.contains("CONTROL_BUNDLE_SUMS"));
    assert!(workflow.contains("SkipRustCompatibilityBridge=true"));
    assert!(workspace.contains("[profile.dev]"));
    assert!(workspace.contains("[profile.test]"));
    assert!(workspace.matches("debug = \"line-tables-only\"").count() >= 2);
    assert!(development.contains("cargo dev check"));
    assert!(development.contains("cargo dev build"));
    assert!(packaging.contains("cargo dev package linux --format layout --skip-build"));
    assert!(packaging.contains("cargo dev package control"));
    assert!(development.contains("cargo dev deploy control"));
    assert!(entrypoints.contains("cargo dev package control"));
    assert!(entrypoints.contains("cargo dev deploy desktop --launch"));
}

#[test]
fn lean_validation_is_opt_in_and_does_not_change_default_profiles() {
    let workspace = read_repo_file("Cargo.toml");
    let lean = workspace
        .split_once("[profile.lean]")
        .expect("missing disk-constrained profile")
        .1
        .split("\n[")
        .next()
        .unwrap();
    assert!(lean.contains("inherits = \"dev\""));
    assert!(lean.contains("debug = 0"));
    assert!(lean.contains("incremental = false"));
    assert!(!lean.contains("debug-assertions = false"));
    assert!(!lean.contains("overflow-checks = false"));
    assert!(workspace.matches("debug = \"line-tables-only\"").count() >= 2);

    let development = read_repo_file("docs/development.md");
    assert!(development.contains("--profile lean -p leselang-hir --test catalog_source"));
    assert!(development.contains("trim_workspace_disk.sh --apply"));
    assert!(development.contains("target/validation/"));
}

#[cfg(unix)]
struct DiskTrimFixture {
    base: std::path::PathBuf,
    root: std::path::PathBuf,
}

#[cfg(unix)]
impl DiskTrimFixture {
    fn new() -> Self {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let id = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let base =
            std::env::temp_dir().join(format!("gewyvern disk-trim-{}-{id}", std::process::id()));
        fs::create_dir(&base).unwrap();
        let base = fs::canonicalize(base).unwrap();
        let fixture = Self {
            root: base.join("repo"),
            base,
        };
        fs::create_dir(&fixture.root).unwrap();
        fixture.write(
            "scripts/perf/trim_workspace_disk.sh",
            &read_repo_file("scripts/perf/trim_workspace_disk.sh"),
        );
        fixture.write("Cargo.toml", "[workspace]\n");
        fixture.write(
            ".gitignore",
            "/target/\n/artifacts/\napps/**/bin/\napps/**/obj/\nnode_modules/\n__pycache__/\n!unignored/__pycache__/\n",
        );
        fixture.git(&["init", "--quiet"]);
        for path in [
            "target/debug/cache",
            "target/lean/cache",
            "target/release/keep",
            "target/validation/keep",
            "target/package/keep",
            "artifacts/release/keep",
            "artifacts/release/__pycache__/keep",
            "artifacts/Packaged.csproj",
            "artifacts/bin/keep",
            "apps/leserpent-avalonia/Leserpent.app/node_modules/keep",
            "apps/leserpent-avalonia/osx-arm64/__pycache__/keep",
            "apps/demo/Demo.csproj",
            "apps/demo/bin/cache",
            "apps/demo/obj/cache",
            "apps/tracked/Tracked.csproj",
            "apps/tracked/bin/keep",
            "apps/linked/Linked.csproj",
            "apps/leserpent/src/Leserpent/data/__pycache__/keep",
            "frontend/node_modules/cache",
            "python/__pycache__/cache",
            "unignored/__pycache__/keep",
        ] {
            fixture.write(path, "fixture\n");
        }
        fixture.git(&["add", "--force", "apps/tracked/bin/keep"]);

        fs::create_dir(fixture.base.join("shared-cargo")).unwrap();
        fs::write(fixture.base.join("shared-cargo/keep"), "shared\n").unwrap();
        std::os::unix::fs::symlink(
            fixture.base.join("shared-cargo"),
            fixture.root.join("apps/linked/bin"),
        )
        .unwrap();
        fs::create_dir(fixture.base.join("mock-bin")).unwrap();
        let cargo = fixture.base.join("mock-bin/cargo");
        fs::write(
            &cargo,
            r##"#!/usr/bin/env bash
set -euo pipefail
printf '%s\n' "$@" >> "$TRIM_TEST_CARGO_LOG"
profile=''
target=''
while [ "$#" -gt 0 ]; do
  case "$1" in
    --profile) profile="$2"; shift ;;
    --target-dir) target="$2"; shift ;;
  esac
  shift
done
case "$profile" in
  dev) rm -rf -- "$target/debug" ;;
  lean) rm -rf -- "$target/lean" ;;
  *) exit 3 ;;
esac
"##,
        )
        .unwrap();
        fs::set_permissions(cargo, fs::Permissions::from_mode(0o755)).unwrap();
        fixture
    }

    fn write(&self, relative: &str, content: &str) {
        let path = self.root.join(relative);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, content).unwrap();
    }

    fn git(&self, arguments: &[&str]) {
        let output = std::process::Command::new("git")
            .arg("-C")
            .arg(&self.root)
            .args(arguments)
            .output()
            .unwrap();
        assert!(output.status.success(), "{output:?}");
    }

    fn run(&self, arguments: &[&str]) -> std::process::Output {
        let path = std::env::join_paths(
            std::iter::once(self.base.join("mock-bin"))
                .chain(std::env::split_paths(&std::env::var_os("PATH").unwrap())),
        )
        .unwrap();
        std::process::Command::new("bash")
            .arg(self.root.join("scripts/perf/trim_workspace_disk.sh"))
            .args(arguments)
            .env("PATH", path)
            .env("CARGO_TARGET_DIR", self.base.join("shared-cargo"))
            .env("TRIM_TEST_CARGO_LOG", self.base.join("cargo.log"))
            .output()
            .unwrap()
    }
}

#[cfg(unix)]
impl Drop for DiskTrimFixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[cfg(unix)]
#[test]
fn disk_trim_requires_apply_and_preserves_evidence_shared_and_tracked_paths() {
    let fixture = DiskTrimFixture::new();
    for arguments in [&[][..], &["--dry-run"][..]] {
        let output = fixture.run(arguments);
        assert!(output.status.success(), "{output:?}");
        assert!(fixture.root.join("target/debug/cache").exists());
        assert!(fixture.root.join("apps/demo/bin/cache").exists());
        assert!(!fixture.base.join("cargo.log").exists());
    }
    let output = fixture.run(&["--apply"]);
    assert!(output.status.success(), "{output:?}");
    for removed in [
        "target/debug",
        "target/lean",
        "apps/demo/bin",
        "apps/demo/obj",
        "frontend/node_modules",
        "python/__pycache__",
    ] {
        assert!(!fixture.root.join(removed).exists(), "{removed}");
    }
    for retained in [
        "target/release/keep",
        "target/validation/keep",
        "target/package/keep",
        "artifacts/release/keep",
        "artifacts/release/__pycache__/keep",
        "artifacts/bin/keep",
        "apps/leserpent-avalonia/Leserpent.app/node_modules/keep",
        "apps/leserpent-avalonia/osx-arm64/__pycache__/keep",
        "apps/tracked/bin/keep",
        "apps/linked/bin/keep",
        "apps/leserpent/src/Leserpent/data/__pycache__/keep",
        "unignored/__pycache__/keep",
    ] {
        assert!(fixture.root.join(retained).exists(), "{retained}");
    }
    assert!(fixture.base.join("shared-cargo/keep").exists());
    let arguments = fs::read_to_string(fixture.base.join("cargo.log")).unwrap();
    assert_eq!(arguments.matches("--locked\n--offline\n").count(), 2);
    assert_eq!(
        arguments
            .matches(&format!(
                "--target-dir\n{}\n",
                fixture.root.join("target").display()
            ))
            .count(),
        2
    );
}

#[cfg(unix)]
#[test]
fn disk_trim_rejects_unknown_arguments_and_symlinked_cargo_outputs() {
    let fixture = DiskTrimFixture::new();
    for arguments in [&["--all"][..], &["--apply", "--dry-run"][..]] {
        assert_eq!(fixture.run(arguments).status.code(), Some(2));
    }
    for relative in ["target/debug", "target/lean", "target"] {
        let path = fixture.root.join(relative);
        fs::remove_dir_all(&path).unwrap();
        std::os::unix::fs::symlink(fixture.base.join("shared-cargo"), &path).unwrap();
        assert_eq!(fixture.run(&["--apply"]).status.code(), Some(2));
        assert!(fixture.base.join("shared-cargo/keep").exists());
        assert!(!fixture.base.join("cargo.log").exists());
        fs::remove_file(path).unwrap();
    }
}

#[test]
fn protocol_runtime_ir_cases_share_one_cargo_integration_target() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut targets = fs::read_dir(&tests)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            name.ends_with("protocol_runtime_ir_tdd.rs")
                .then(|| name.to_string())
        })
        .collect::<Vec<_>>();
    targets.sort();
    assert_eq!(targets, ["protocol_runtime_ir_tdd.rs"]);

    let harness = read_repo_file("tests/protocol_runtime_ir_tdd.rs");
    let cases = tests.join("protocol_runtime_cases");
    let mut case_count = 0;
    for entry in fs::read_dir(cases).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        case_count += 1;
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let source = fs::read_to_string(&path).unwrap();
        assert!(harness.contains(&format!("mod {stem};")));
        assert!(source.contains("use crate::support;"));
        assert!(!source.contains("mod support;"));
    }
    assert!(
        case_count >= 24,
        "protocol runtime case shelf became vacuous"
    );
}

#[cfg(unix)]
#[test]
fn documented_shell_entrypoints_are_executable() {
    let entrypoints = [
        "scripts/demos/external_engine_roundtrip_demo.sh",
        "scripts/demos/socket_roundtrip_demo.sh",
        "scripts/demos/training_dataset_roundtrip_demo.sh",
        "scripts/linux/linux_attach_smoke.sh",
        "scripts/linux/linux_kprobe_smoke.sh",
        "scripts/linux/linux_tc_smoke.sh",
        "scripts/packaging/container_operator_path_validation.sh",
        "scripts/packaging/container_protocol_validation.sh",
        "scripts/packaging/container_runtime_validation.sh",
        "scripts/packaging/container_validation_summary.sh",
        "scripts/packaging/package_install_smoke.sh",
        "scripts/packaging/release_container_check.sh",
        "scripts/packaging/release_gate.sh",
        "scripts/validation/debugger_cross_validation.sh",
        "scripts/validation/field_validation_smoke.sh",
        "scripts/validation/ftp_denied_container_validation.sh",
        "scripts/validation/high_frequency_validation.sh",
        "scripts/validation/juice_shop_container_validation.sh",
        "scripts/validation/leserpent_linux_bundle_smoke.sh",
        "scripts/validation/ldap_bind_denied_container_validation.sh",
        "scripts/validation/pathological_container_validation.sh",
        "scripts/remote/headless_linux.sh",
        "scripts/remote/run_on_linux_host.sh",
        "scripts/validation/registry_validation.sh",
        "scripts/validation/runtime_lifecycle_validation.sh",
        "scripts/validation/runtime_operator_validation.sh",
        "scripts/validation/runtime_resilience_fault_injection.sh",
        "scripts/validation/runtime_resilience_log_evidence.sh",
        "scripts/validation/runtime_resilience_roundtrip.sh",
        "scripts/validation/runtime_resilience_validation.sh",
        "scripts/validation/three_module_stack_smoke.sh",
    ];

    for relative in entrypoints {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
        let mode = fs::metadata(&path)
            .unwrap_or_else(|err| panic!("failed to stat {}: {}", path.display(), err))
            .permissions()
            .mode();
        assert_ne!(mode & 0o111, 0, "{} should be executable", relative);
    }
}

#[test]
fn protocol_registry_cases_share_one_cargo_integration_target() {
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests");
    let mut targets = fs::read_dir(&tests)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_file())
        .filter_map(|path| {
            let name = path.file_name()?.to_str()?;
            (name.ends_with("protocol_registry_tdd.rs") || name == "protocol_registry_gap_tdd.rs")
                .then(|| name.to_string())
        })
        .collect::<Vec<_>>();
    targets.sort();
    assert_eq!(targets, ["protocol_registry_tdd.rs"]);

    let harness = read_repo_file("tests/protocol_registry_tdd.rs");
    let mut case_count = 0;
    for entry in fs::read_dir(tests.join("protocol_registry_cases")).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("rs") {
            continue;
        }
        case_count += 1;
        let stem = path.file_stem().unwrap().to_str().unwrap();
        let source = fs::read_to_string(&path).unwrap();
        assert!(harness.contains(&format!("mod {stem};")), "{stem}");
        assert!(
            harness.contains(&format!("protocol_registry_cases/{stem}.rs")),
            "{stem}"
        );
        assert!(!source.contains("mod support;"), "{stem}");
    }
    assert!(
        case_count >= 52,
        "protocol registry case shelf became vacuous"
    );
    assert!(harness.contains("mod support;"));
}

#[test]
fn container_entrypoints_default_to_a_bounded_remote_linux_workspace() {
    let dispatcher = read_repo_file("scripts/remote/container_execution.sh");
    let runner = read_repo_file("scripts/remote/run_on_linux_host.sh");
    let remote_docs = read_repo_file("docs/remote-docker.md");

    assert!(dispatcher.contains("GEWY_DOCKER_EXECUTION"));
    assert!(dispatcher.contains("host_os") && dispatcher.contains("Darwin"));
    assert!(runner.contains("BatchMode=yes"));
    assert!(runner.contains("/.cache/gewyvern/docker-workspace"));
    assert!(runner.contains("REMOTE_WORKSPACE") && runner.contains("--delete-excluded"));
    assert!(runner.contains("flock -o -w 120"));
    assert!(runner.contains("--exclude='apps/**/bin/'"));
    assert!(runner.contains("--exclude='apps/**/obj/'"));
    assert!(!runner.contains("--exclude='**/bin/'"));
    assert!(!runner.contains("--exclude='**/obj/'"));
    assert!(runner.contains("--exclude='**/TestResults/'"));
    assert!(runner.contains("control-plane-state.json"));
    assert!(!runner.contains("DOCKER_HOST="));
    assert!(!runner.contains("SSHPASS"));
    assert!(remote_docs.contains("GEWY_DOCKER_EXECUTION=local"));
    assert!(remote_docs.contains("Host gewyvern-lab"));
    assert!(remote_docs.contains("IdentityFile ~/.ssh/gewyvern_lab_ed25519"));

    for entrypoint in [
        "scripts/packaging/build_packages_in_container.sh",
        "scripts/packaging/release_container_check.sh",
        "scripts/validation/three_module_stack_smoke.sh",
        "scripts/validation/juice_shop_container_validation.sh",
        "scripts/validation/ftp_denied_container_validation.sh",
        "scripts/validation/ldap_bind_denied_container_validation.sh",
        "scripts/validation/pathological_container_validation.sh",
    ] {
        let source = read_repo_file(entrypoint);
        assert!(source.contains("container_execution.sh"), "{entrypoint}");
        assert!(
            source.contains("gewy_container_maybe_run_remote"),
            "{entrypoint}"
        );
    }
}

#[test]
fn markdown_docs_do_not_embed_local_checkout_paths() {
    let roots = [
        Path::new(env!("CARGO_MANIFEST_DIR")).join("README.md"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("ROADMAP.md"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("docs"),
        Path::new(env!("CARGO_MANIFEST_DIR")).join("apps/leserpent/README.md"),
    ];
    let forbidden = [
        local_path(&["Users", "Shared", "chroot", "dev", "gewyvern"]),
        local_path(&["Users", "Shared", "chroot", "dev", "etragon"]),
        local_path(&["Users", "Shared", "chroot", "dev", "leserpent"]),
        local_path(&["Users", "seis"]),
        local_path(&["var", "folders"]),
        local_path(&["home", "chiharukiryu", "work", "gewyvern-server-test"]),
        local_path(&["home", "gewyvern-lab", "work", "gewyvern"]),
        local_path(&["home", "user"]),
        local_path(&["absolute", "path"]),
        local_path(&["path", "to"]),
    ];
    let mut failures = Vec::new();

    for root in roots {
        collect_markdown_path_failures(&root, &forbidden, &mut failures);
    }

    assert!(
        failures.is_empty(),
        "markdown docs should not embed local checkout paths:\n{}",
        failures.join("\n")
    );
}

#[test]
fn release_docs_prefer_native_validation_entrypoints() {
    let field_validation = read_repo_file("docs/field-validation.md");
    let field_findings = read_repo_file("docs/field-findings.md");
    let v100 = read_repo_file("docs/history/v1.0.0.md");

    assert!(
        field_validation
            .contains("cargo run --quiet --bin gewyvern_validate -- release-container-check")
    );
    assert!(
        field_validation
            .contains("cargo run --quiet --bin gewyvern_validate -- container-runtime-validation")
    );
    assert!(
        field_findings
            .contains("cargo run --quiet --bin gewyvern_validate -- release-gate --skip-build")
    );
    assert!(field_findings.contains("target/validation/release-gate-artifacts.json"));
    assert!(field_findings.contains("target/validation/release-gate-artifacts.txt"));
    assert!(v100.contains("cargo run --quiet --bin gewyvern_validate -- release-container-check"));
    assert!(
        v100.contains("cargo run --quiet --bin gewyvern_validate -- release-gate --skip-build")
    );
    assert!(v100.contains("target/validation/release-gate-artifacts.json"));
    assert!(v100.contains("target/validation/release-gate-artifacts.txt"));
}

fn local_path(parts: &[&str]) -> String {
    format!("/{}", parts.join("/"))
}

fn collect_markdown_path_failures(path: &Path, forbidden: &[String], failures: &mut Vec<String>) {
    if path.is_dir() {
        for entry in fs::read_dir(path)
            .unwrap_or_else(|err| panic!("failed to read directory {}: {}", path.display(), err))
        {
            let entry = entry.unwrap_or_else(|err| panic!("failed to read dir entry: {err}"));
            collect_markdown_path_failures(&entry.path(), forbidden, failures);
        }
        return;
    }

    if path.extension().and_then(|ext| ext.to_str()) != Some("md") {
        return;
    }

    let body = fs::read_to_string(path)
        .unwrap_or_else(|err| panic!("failed to read {}: {}", path.display(), err));
    for needle in forbidden {
        if body.contains(needle) {
            failures.push(format!("{} contains {}", path.display(), needle));
        }
    }
}
