use std::fs;
use std::path::Path;

fn read_repo_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read {}: {}", path.display(), err);
    })
}

#[test]
fn etragon_inherits_the_workspace_version() {
    let root_manifest = read_repo_file("Cargo.toml");
    let etragon_manifest = read_repo_file("apps/etragon/Cargo.toml");
    let gewyc_manifest = read_repo_file("crates/gewyc/Cargo.toml");
    let lockfile = read_repo_file("Cargo.lock");

    assert!(root_manifest.contains("[workspace.package]"));
    assert!(root_manifest.contains("version = \"0.19.0\""));
    assert!(etragon_manifest.contains("version.workspace = true"));
    assert!(gewyc_manifest.contains("version.workspace = true"));
    assert!(!etragon_manifest.contains("version = \"0.1.0\""));
    assert!(lockfile.contains("name = \"etragon\"\nversion = \"0.19.0\""));
    assert!(lockfile.contains("name = \"gewyc\"\nversion = \"0.19.0\""));
}

#[test]
fn leserpent_uses_the_root_dotnet_version_without_app_specific_version() {
    let root_props = read_repo_file("Directory.Build.props");
    let project = read_repo_file("apps/leserpent/src/Leserpent/Leserpent.csproj");
    let frontend_package = read_repo_file("apps/leserpent/package.json");
    let frontend_lock = read_repo_file("apps/leserpent/package-lock.json");

    assert!(root_props.contains("<Version>0.19.0</Version>"));
    assert!(!project.contains("<Version>0.1.9</Version>"));
    assert!(!project.contains("<Version>"));
    assert!(!frontend_package.contains("\"version\""));
    assert!(!frontend_lock.contains("\"version\": \"0.1.9\""));
}

#[test]
fn docs_describe_one_shared_mainline_version() {
    let readme = read_repo_file("README.md");
    let monorepo = read_repo_file("docs/monorepo-stack.md");
    let leserpent_readme = read_repo_file("apps/leserpent/README.md");

    assert!(readme.contains("follows the root `gewyvern` version"));
    assert!(monorepo.contains("one shared mainline version"));
    assert!(monorepo.contains("longer carry independent release numbers"));
    assert!(leserpent_readme.contains("follows the root `gewyvern` version line"));
    assert!(!readme.contains("version `0.1.0`"));
    assert!(!readme.contains("version `0.1.9`"));
    assert!(!monorepo.contains("`leserpent`: `0.1.9`"));
    assert!(!monorepo.contains("`etragon`: `0.1.0`"));
}
