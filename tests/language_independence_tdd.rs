use std::collections::{BTreeMap, BTreeSet};
use std::process::Command;

fn workspace_dependency_graph() -> BTreeMap<String, BTreeSet<String>> {
    let output = Command::new(env!("CARGO"))
        .args(["metadata", "--format-version", "1", "--no-deps"])
        .current_dir(env!("CARGO_MANIFEST_DIR"))
        .output()
        .expect("cargo metadata should start");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );

    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).expect("cargo metadata should be valid JSON");
    let packages = metadata["packages"]
        .as_array()
        .expect("metadata should contain packages");
    let workspace_names = packages
        .iter()
        .filter(|package| package["source"].is_null())
        .filter_map(|package| package["name"].as_str())
        .collect::<BTreeSet<_>>();

    packages
        .iter()
        .filter_map(|package| {
            let name = package["name"].as_str()?;
            if !workspace_names.contains(name) {
                return None;
            }
            let dependencies = package["dependencies"]
                .as_array()?
                .iter()
                .filter(|dependency| dependency["kind"].is_null())
                .filter_map(|dependency| dependency["name"].as_str())
                .filter(|dependency| workspace_names.contains(dependency))
                .map(str::to_string)
                .collect();
            Some((name.to_string(), dependencies))
        })
        .collect()
}

fn dependency_closure(graph: &BTreeMap<String, BTreeSet<String>>, root: &str) -> BTreeSet<String> {
    let mut pending = vec![root.to_string()];
    let mut visited = BTreeSet::new();
    while let Some(package) = pending.pop() {
        if !visited.insert(package.clone()) {
            continue;
        }
        if let Some(dependencies) = graph.get(&package) {
            pending.extend(dependencies.iter().cloned());
        }
    }
    visited.remove(root);
    visited
}

fn assert_workspace_closure(root: &str, expected: &[&str]) {
    let graph = workspace_dependency_graph();
    assert!(
        graph.contains_key(root),
        "workspace package '{root}' is missing"
    );
    let actual = dependency_closure(&graph, root);
    let expected = expected
        .iter()
        .map(|dependency| (*dependency).to_string())
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, expected,
        "'{root}' gained a workspace dependency; review the standalone language boundary"
    );
}

#[test]
fn gewylang_frontend_and_compiler_have_no_product_dependency() {
    assert_workspace_closure("gewylang-contract", &[]);
    assert_workspace_closure("gewylang-ir", &["gewylang-contract"]);
    assert_workspace_closure("gewylang-syntax", &["gewylang-contract"]);
    assert_workspace_closure(
        "gewylang-compiler",
        &["gewylang-contract", "gewylang-syntax"],
    );
}

#[test]
fn leselang_frontend_and_host_contract_have_no_product_dependency() {
    assert_workspace_closure("leselang-syntax", &[]);
    assert_workspace_closure("leselang-host-contract", &["silvortex-identity"]);
    assert_workspace_closure(
        "leselang-hir",
        &[
            "leselang-host-contract",
            "leselang-syntax",
            "silvortex-identity",
        ],
    );
}
