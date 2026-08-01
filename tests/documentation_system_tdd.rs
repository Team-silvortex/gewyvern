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
fn documentation_tree_has_no_dangling_local_links() {
    let root = repository_root();
    let mut documents = vec![root.join("README.md")];
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
        "The 2.0 target scope is frozen",
        "may not add new core\ncapability families",
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
    for freeze_rule in [
        "## 2.0 Scope Freeze",
        "The core 2.0 capability set is closed",
        "No new core capability family may enter",
        "Remaining minor versions are allowed to\nfinish only the already-declared",
        "Accepted work after the freeze is closure work",
        "Rejected work is scope expansion",
        "moving\nEtragon into the release gate",
        "claiming Windows native parity",
        "making GUI frameworks\nautomatically compatible",
        "WinRM is explicitly outside the 2.0 evidence gate",
        "physical device release parity is deferred",
        "full mobile device release parity beyond the declared entry/lifecycle contract",
    ] {
        assert!(
            roadmap.contains(freeze_rule),
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
