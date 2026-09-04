use std::fs;
use std::path::{Path, PathBuf};

const MODULES: &[&str] = &[
    "runtime.md",
    "gewylang.md",
    "leselang.md",
    "protocols.md",
    "operations.md",
    "project.md",
];

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

#[test]
fn leselang_reference_separates_current_contract_from_roadmap_design() {
    let root = repository_root();
    let reference = fs::read_to_string(root.join("docs/leselang-language.md"))
        .expect("Leselang language reference must exist");
    let module = fs::read_to_string(root.join("docs/modules/leselang.md"))
        .expect("Leselang documentation module must exist");
    let roadmap = fs::read_to_string(root.join("docs/leserpent-2-roadmap.md"))
        .expect("Leserpent 2.0 roadmap must exist");

    for invariant in [
        "fn main() = runtime.list(",
        "runtime.read",
        "protocolized GUI and control automation",
        "not a general-purpose language runtime",
        "hostable Rust crate",
        "narrow FFI boundary",
        "no GUI framework is automatically compatible",
        "developer-owned adapter",
        "generated framework binding",
        "UiAdapterManifest",
        "Effect",
        "64 KiB",
        "durable continuation guarantee",
        "SQLite effect journal",
        "LSE",
        "LSH",
        "LSV",
    ] {
        assert!(
            reference.contains(invariant),
            "Leselang reference must preserve current invariant: {invariant}"
        );
    }

    assert!(reference.contains("do not expose\n`async`/`await`"));
    assert!(module.contains("(../leselang-language.md)"));
    assert!(roadmap.contains("(leselang-language.md)"));
}

#[test]
fn documentation_index_routes_to_each_small_domain_module() {
    let root = repository_root();
    let index = fs::read_to_string(root.join("docs/index.md")).expect("docs index must exist");

    for module in MODULES {
        assert!(
            index.contains(&format!("(modules/{module})")),
            "docs index must route to {module}"
        );

        let path = root.join("docs/modules").join(module);
        let source = fs::read_to_string(&path).expect("documentation module must exist");
        assert!(
            source.lines().count() <= 60,
            "module must stay compact: {module}"
        );
        assert!(
            source.contains("## Start"),
            "module needs a start path: {module}"
        );
        assert!(
            markdown_link_targets(&source).len() >= 5,
            "module needs useful routing: {module}"
        );
    }
}

#[test]
fn architecture_shelf_defines_one_protocolized_debugging_fabric() {
    let root = repository_root();
    let blueprint = fs::read_to_string(root.join("docs/architecture-blueprint.md"))
        .expect("canonical architecture blueprint must exist");
    let coordination = fs::read_to_string(root.join("docs/architecture-coordination.md"))
        .expect("architecture coordination contract must exist");
    let evolution = fs::read_to_string(root.join("docs/architecture-evolution.md"))
        .expect("architecture evolution contract must exist");
    let monorepo = fs::read_to_string(root.join("docs/monorepo-stack.md"))
        .expect("monorepo stack guide must exist");
    let bridge = fs::read_to_string(root.join("apps/leserpent/README.md"))
        .expect("Web compatibility bridge documentation must exist");

    for invariant in [
        "replayable, protocolized network debugging fabric",
        "## Four Planes",
        "### Evidence Plane",
        "### Authority Plane",
        "### Intent Plane",
        "### Presentation Plane",
        "### Advisory Sideplane",
        "one kernel/container boundary -> one Gewyvern service",
        "one leserpentd authority      -> many Gewyvern services",
        "one Leserpent client          -> many independent leserpentd authorities",
        "## The Advantage Zone",
        "## Scope Guardrails",
        "silvortex-bounded-io",
        "silvortex-identity",
        "gewyvern-install-contract",
    ] {
        assert!(
            blueprint.contains(invariant),
            "canonical blueprint lacks invariant: {invariant}"
        );
    }

    for invariant in [
        "## Compatibility Bridge Rule",
        "## Current 2.0.x Priorities",
        "Horizon 2: Remove Boundary Debt",
        "A shared checkout does not imply shared\nauthority",
        "Managed persistence must not become a second authority",
    ] {
        assert!(
            coordination.contains(invariant)
                || evolution.contains(invariant)
                || monorepo.contains(invariant),
            "architecture shelf lacks invariant: {invariant}"
        );
    }

    assert!(bridge.contains("ASP.NET/TypeScript compatibility bridge"));
    assert!(bridge.contains("不再新增只存在于 managed runtime 的 control-plane 语义"));
    for stale in [
        "2.0 目标",
        "目标 2.0",
        "即时 gRPC",
        "最小 ASP.NET Core control-plane 骨架",
    ] {
        assert!(
            !bridge.contains(stale),
            "Web bridge documentation restored stale architecture: {stale}"
        );
    }
}

#[test]
fn root_product_navigation_exposes_leserpent_as_a_first_class_entry() {
    let root = repository_root();
    let gewyvern = fs::read_to_string(root.join("README.md")).expect("root README must exist");
    let leserpent =
        fs::read_to_string(root.join("LESERPENT.md")).expect("Leserpent product page must exist");
    let implementation = fs::read_to_string(root.join("apps/leserpent/README.md"))
        .expect("Leserpent implementation README must exist");

    assert!(gewyvern.contains("href=\"LESERPENT.md\""));
    assert!(leserpent.contains("href=\"README.md\""));
    assert!(implementation.contains("(../../LESERPENT.md)"));
    let (minor_line, _) = env!("CARGO_PKG_VERSION")
        .rsplit_once('.')
        .expect("workspace version must be semantic");
    assert!(leserpent.contains(&format!("# Leserpent v{minor_line}.x")));
    for invariant in [
        "one or more leserpentd authorities",
        "One Leserpent client can manage multiple independent `leserpentd` authorities",
        "Credentials protect infrastructure authority",
        "does not require a remote connection before local Orchestra",
        "cargo dev package desktop",
        "cargo dev package control",
        "not Apple-notarized",
    ] {
        assert!(
            leserpent.contains(invariant),
            "Leserpent product page lacks invariant: {invariant}"
        );
    }
}

#[test]
fn tutorial_shelf_covers_cli_desktop_languages_and_remote_lifecycle() {
    let root = repository_root();
    let shelf =
        fs::read_to_string(root.join("docs/book/tutorials.md")).expect("tutorial shelf must exist");
    let contracts: &[(&str, &[&str])] = &[
        (
            "tutorial-first-run.md",
            &[
                "--list-protocols",
                "--list-entries quic",
                "--protocol postgres --entry query",
                "--scan-all",
            ],
        ),
        (
            "tutorial-leserpent-desktop.md",
            &[
                "Local Orchestra",
                "+ Add daemon",
                "Workspace Leselang",
                "--verify-desktop-tutorial",
            ],
        ),
        (
            "tutorial-gewylang-package.md",
            &["gewyc -- init", "gewy.pkg", "frontend", "use(...)"],
        ),
        (
            "tutorial-leselang-gui-automation.md",
            &[
                "--export-leselang",
                "--export-plan",
                "opens no socket",
                "ui.presentation",
                "Run live",
            ],
        ),
        (
            "tutorial-remote-deployment-lab.md",
            &[
                "vault:ssh:*",
                "bootstrap deploy",
                "bootstrap bind",
                "runtime provision",
                "runtime retire",
                "bootstrap retire",
            ],
        ),
    ];

    for (file, markers) in contracts {
        assert!(
            shelf.contains(&format!("({file})")),
            "tutorial shelf must route to {file}"
        );
        let source = fs::read_to_string(root.join("docs/book").join(file))
            .expect("tutorial page must exist");
        assert!(source.starts_with("# Tutorial:"), "invalid title in {file}");
        assert!(
            source.contains("## Prerequisites"),
            "tutorial must name prerequisites: {file}"
        );
        assert!(
            source.contains("## Completion Checkpoint"),
            "tutorial must name its observed finish: {file}"
        );
        assert!(
            !source.contains("](docs/"),
            "book tutorial must use local relative links: {file}"
        );
        for marker in *markers {
            assert!(
                source.contains(marker),
                "{file} lacks contract marker {marker}"
            );
        }
    }

    let root_usage = fs::read_to_string(root.join("src/main/ui_locale/catalog.rs"))
        .expect("Gewyvern usage catalog must exist");
    for option in ["--list-protocols", "--list-entries", "--scan-all"] {
        assert!(root_usage.contains(option), "Gewyvern CLI lacks {option}");
    }

    let gewyc_usage = fs::read_to_string(root.join("crates/gewyc/src/main.rs"))
        .expect("gewyc CLI source must exist");
    for command in ["gewyc init", "explain|frontend", "stages|envelope"] {
        assert!(gewyc_usage.contains(command), "gewyc CLI lacks {command}");
    }

    let leserpent_usage = fs::read_to_string(root.join("crates/leserpent-cli/src/lib.rs"))
        .expect("Leserpent CLI source must exist");
    for command in [
        "bootstrap deploy",
        "bootstrap inspect",
        "bootstrap bind",
        "bootstrap retire",
        "runtime provision",
        "runtime inspect",
        "runtime logs",
        "runtime retire",
        "--export-leselang",
        "--export-plan",
    ] {
        assert!(
            leserpent_usage.contains(command),
            "Leserpent CLI lacks tutorial command {command}"
        );
    }

    let remote = fs::read_to_string(root.join("docs/book/tutorial-remote-deployment-lab.md"))
        .expect("remote tutorial must exist");
    let runtime_retirement = remote
        .find("runtime retire \"$RUNTIME_ID\"")
        .expect("runtime retirement step must exist");
    let daemon_retirement = remote
        .find("bootstrap retire \"$BOOTSTRAP_ID\"")
        .expect("daemon retirement step must exist");
    assert!(
        runtime_retirement < daemon_retirement,
        "remote tutorial must retire the runtime before its daemon"
    );
    for forbidden in ["--password", "--private-key", "--sudo-password", "sshpass"] {
        assert!(
            !remote.contains(forbidden),
            "remote tutorial must not introduce raw secret input {forbidden}"
        );
    }
}

#[test]
fn documentation_tree_has_no_dangling_local_links() {
    let root = repository_root();
    let mut documents = vec![root.join("README.md"), root.join("LESERPENT.md")];
    collect_markdown(&root.join("docs"), &mut documents);

    let mut checked = 0usize;
    for document in documents {
        let source = fs::read_to_string(&document).expect("markdown document must be readable");
        for target in markdown_link_targets(&source) {
            let target = target.trim().trim_start_matches('<').trim_end_matches('>');
            if target.starts_with('#')
                || target.starts_with("http://")
                || target.starts_with("https://")
                || target.starts_with("mailto:")
            {
                continue;
            }

            let path = target.split('#').next().unwrap_or_default();
            if path.is_empty() {
                continue;
            }
            checked += 1;

            let relative = document.parent().unwrap_or(&root).join(path);
            let repository_relative = root.join(path);
            assert!(
                relative.exists() || repository_relative.exists(),
                "broken local link in {}: {target}",
                document.strip_prefix(&root).unwrap_or(&document).display()
            );
        }
    }

    assert!(
        checked >= 2_500,
        "expected to validate the full documentation tree"
    );
}

#[test]
fn leserpent_next_major_has_one_architecture_and_one_delivery_roadmap() {
    let root = repository_root();
    let architecture = fs::read_to_string(root.join("docs/leserpent-2-architecture.md"))
        .expect("Leserpent 2.0 architecture must exist");
    let roadmap = fs::read_to_string(root.join("docs/leserpent-2-roadmap.md"))
        .expect("Leserpent 2.0 roadmap must exist");
    let project = fs::read_to_string(root.join("docs/modules/project.md"))
        .expect("project module must exist");
    let root_roadmap =
        fs::read_to_string(root.join("ROADMAP.md")).expect("root roadmap must exist");

    for invariant in [
        "Non-Negotiable Invariants",
        "GUI, CLI, and Leselang",
        "protocolized GUI/control automation runtime",
        "not a general-purpose VM",
        "Rust crate",
        "FFI boundary",
        "No GUI framework becomes compatible automatically",
        "developer-owned adapter",
        "generated binding",
        "UiAdapterManifest",
        "2.0 Scope Boundary",
        "The 2.0 scope is frozen",
        "released architecture",
        "without silently adding a new core\ncapability family",
        "Etragon advisory",
        "Windows native parity",
        "automatic GUI framework compatibility",
        "This closes the 2.0 reverse-bootstrap scope",
        "optional post-2.0 work",
        "mobile retains its minimum entry/lifecycle conformance contract",
        "synchronous source semantics",
        "CommandEnvelope",
        "EffectRequest",
        "UiDocument",
        "atomic replaceability",
    ] {
        assert!(
            architecture.contains(invariant),
            "2.0 architecture must preserve invariant: {invariant}"
        );
    }

    for gate in 1..=7 {
        assert!(
            roadmap.contains(&format!("## Gate {gate}:")),
            "2.0 roadmap must preserve delivery gate {gate}"
        );
    }
    let normalized_roadmap = roadmap.split_whitespace().collect::<Vec<_>>().join(" ");
    for freeze_rule in [
        "## 2.0 Scope Freeze",
        "The core 2.0 capability set is closed",
        "No new core capability family may enter",
        "Remaining minor versions are allowed to finish only the already-declared",
        "Accepted work after the freeze is closure work",
        "Rejected work is scope expansion",
        "moving Etragon into the release gate",
        "claiming Windows native parity",
        "making GUI frameworks automatically compatible",
        "WinRM is explicitly outside the 2.0 evidence gate",
        "physical device release parity is deferred",
        "full mobile device release parity beyond the declared entry/lifecycle contract",
        "Every capability inside this frozen scope is part of the MIT open-source free core",
        "Future commercial work is limited to newly introduced hosted service extensions",
    ] {
        assert!(
            normalized_roadmap.contains(freeze_rule),
            "2.0 roadmap must preserve scope-freeze rule: {freeze_rule}"
        );
    }

    for retired_gate in [
        "desktop and one mobile target pass release tests",
        "desktop and one mobile target pass the same semantic conformance suite",
        "WinRM is the remaining deferred evidence gate",
    ] {
        assert!(
            !architecture.contains(retired_gate) && !roadmap.contains(retired_gate),
            "2.0 documentation must not restore retired release gate: {retired_gate}"
        );
    }

    assert!(project.contains("(../leserpent-2-architecture.md)"));
    assert!(project.contains("(../leserpent-2-roadmap.md)"));
    assert!(root_roadmap.contains("(docs/leserpent-2-architecture.md)"));
    assert!(root_roadmap.contains("(docs/leserpent-2-roadmap.md)"));
}

fn collect_markdown(directory: &Path, output: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(directory).expect("documentation directory must be readable") {
        let path = entry.expect("documentation entry must be readable").path();
        if path.is_dir() {
            collect_markdown(&path, output);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") {
            output.push(path);
        }
    }
}

fn markdown_link_targets(source: &str) -> Vec<&str> {
    let mut targets = Vec::new();
    let mut remaining = source;
    while let Some(start) = remaining.find("](") {
        remaining = &remaining[start + 2..];
        let Some(end) = remaining.find(')') else {
            break;
        };
        let raw = remaining[..end].trim();
        let target = raw.split_whitespace().next().unwrap_or_default();
        if !target.is_empty() {
            targets.push(target);
        }
        remaining = &remaining[end + 1..];
    }
    targets
}
