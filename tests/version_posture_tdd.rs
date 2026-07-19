use std::fs;
use std::path::Path;

fn read_repo_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read {}: {}", path.display(), err);
    })
}

fn count_protocol_catalog() -> (usize, usize) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("protocols");
    let families: Vec<_> = fs::read_dir(&root)
        .unwrap_or_else(|err| panic!("failed to read {}: {}", root.display(), err))
        .map(|entry| {
            entry
                .expect("protocol family entry should be readable")
                .path()
        })
        .filter(|path| path.is_dir())
        .collect();
    let entries = families
        .iter()
        .map(|family| {
            fs::read_dir(family)
                .unwrap_or_else(|err| panic!("failed to read {}: {}", family.display(), err))
                .filter(|entry| {
                    entry
                        .as_ref()
                        .map(|item| item.path().is_dir())
                        .unwrap_or(false)
                })
                .count()
        })
        .sum();

    (families.len(), entries)
}

fn section_version(document: &str, section: &str) -> String {
    let section_header = format!("[{section}]");
    let mut in_section = false;
    for line in document.lines() {
        if line == section_header {
            in_section = true;
            continue;
        }
        if in_section && line.starts_with('[') {
            break;
        }
        if in_section {
            if let Some(version) = line
                .strip_prefix("version = \"")
                .and_then(|rest| rest.strip_suffix('"'))
            {
                return version.to_string();
            }
        }
    }
    panic!("missing version in [{section}]");
}

fn xml_value(document: &str, tag: &str) -> String {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let value = document
        .split(&open)
        .nth(1)
        .and_then(|rest| rest.split(&close).next())
        .unwrap_or_else(|| panic!("missing XML tag <{tag}>"));
    value.to_string()
}

#[test]
fn etragon_inherits_the_workspace_version() {
    let root_manifest = read_repo_file("Cargo.toml");
    let etragon_manifest = read_repo_file("apps/etragon/Cargo.toml");
    let gewyc_manifest = read_repo_file("crates/gewyc/Cargo.toml");
    let lockfile = read_repo_file("Cargo.lock");
    let workspace_version = section_version(&root_manifest, "workspace.package");
    let package_version = section_version(&root_manifest, "package");

    assert!(root_manifest.contains("[workspace.package]"));
    assert_eq!(workspace_version, package_version);
    assert!(etragon_manifest.contains("version.workspace = true"));
    assert!(gewyc_manifest.contains("version.workspace = true"));
    assert!(!etragon_manifest.contains("version = \"0.1.0\""));
    assert!(lockfile.contains(&format!(
        "name = \"etragon\"\nversion = \"{workspace_version}\""
    )));
    assert!(lockfile.contains(&format!(
        "name = \"gewyc\"\nversion = \"{workspace_version}\""
    )));
}

#[test]
fn leserpent_uses_the_root_dotnet_version_without_app_specific_version() {
    let root_manifest = read_repo_file("Cargo.toml");
    let root_props = read_repo_file("Directory.Build.props");
    let project = read_repo_file("apps/leserpent/src/Leserpent/Leserpent.csproj");
    let desktop =
        read_repo_file("apps/leserpent-avalonia/src/Leserpent.Avalonia/Leserpent.Avalonia.csproj");
    let android = read_repo_file(
        "apps/leserpent-mobile/src/Leserpent.Mobile.Android/Leserpent.Mobile.Android.csproj",
    );
    let ios = read_repo_file(
        "apps/leserpent-mobile/src/Leserpent.Mobile.iOS/Leserpent.Mobile.iOS.csproj",
    );
    let frontend_package = read_repo_file("apps/leserpent/package.json");
    let frontend_lock = read_repo_file("apps/leserpent/package-lock.json");
    let workspace_version = section_version(&root_manifest, "workspace.package");
    let dotnet_version = xml_value(&root_props, "Version");

    assert_eq!(dotnet_version, workspace_version);
    assert!(!project.contains("<Version>0.1.9</Version>"));
    assert!(!project.contains("<Version>"));
    assert!(!desktop.contains("<Version>"));
    assert!(!android.contains("<Version>"));
    assert!(!ios.contains("<Version>"));
    assert!(android.contains("<ApplicationDisplayVersion>$(Version)</ApplicationDisplayVersion>"));
    assert!(!android.contains("<ApplicationDisplayVersion>1.4.6</ApplicationDisplayVersion>"));
    assert!(!frontend_package.contains("\"version\""));
    assert!(!frontend_lock.contains("\"version\": \"0.1.9\""));
}

#[test]
fn macos_bundle_inherits_the_workspace_version_by_default() {
    let bundler = read_repo_file("src/bin/gewyvern_leserpent_bundle.rs");
    let release = read_repo_file("src/bin/gewyvern_leserpent_release.rs");
    let app_readme = read_repo_file("apps/leserpent-avalonia/README.md");
    let entrypoints = read_repo_file("docs/script-entrypoints.md");

    assert!(bundler.contains("env!(\"CARGO_PKG_VERSION\").to_string()"));
    assert!(bundler.contains("<key>CFBundleShortVersionString</key>"));
    assert!(bundler.contains("<key>CFBundleVersion</key>"));
    assert!(bundler.contains("plist != info_plist(version)"));
    assert!(release.contains("const PRODUCT_VERSION: &str = env!(\"CARGO_PKG_VERSION\")"));
    assert!(release.contains("Info.plist contains duplicate {key}"));
    assert!(release.contains("CFBundleShortVersionString"));
    assert!(release.contains("CFBundleVersion"));
    assert!(app_readme.contains("inherit the root Rust workspace release automatically"));
    assert!(entrypoints.contains("inherits the root Rust workspace version"));
    assert!(!app_readme.contains("--version 1.4.6"));
    assert!(!entrypoints.contains("--version 1.4.6"));
}

#[test]
fn docs_describe_one_shared_mainline_version() {
    let readme = read_repo_file("README.md");
    let monorepo = read_repo_file("docs/monorepo-stack.md");
    let leserpent_readme = read_repo_file("apps/leserpent/README.md");
    let root_manifest = read_repo_file("Cargo.toml");
    let workspace_version = section_version(&root_manifest, "workspace.package");

    assert_eq!(workspace_version, "1.4.6");
    assert!(readme.starts_with("# gewyvern v1.4.6\n"));
    assert!(readme.contains("project version: `1.4.6`"));

    assert!(readme.contains("follows the root `gewyvern` version"));
    assert!(monorepo.contains("one shared mainline version"));
    assert!(monorepo.contains("longer carry independent release numbers"));
    assert!(leserpent_readme.contains("follows the root `gewyvern` version line"));
    assert!(!readme.contains("version `0.1.0`"));
    assert!(!readme.contains("version `0.1.9`"));
    assert!(!monorepo.contains("`leserpent`: `0.1.9`"));
    assert!(!monorepo.contains("`etragon`: `0.1.0`"));
}

#[test]
fn docs_catalog_anchor_matches_packaged_protocol_tree() {
    let (families, entries) = count_protocol_catalog();
    let readme = read_repo_file("README.md");
    let history = read_repo_file("docs/history/v1.0.0.md");

    assert!(
        readme.contains(&format!(
            "protocol registry coverage: {families} protocol families and {entries} package entries"
        )),
        "README protocol registry count should match protocols/ tree"
    );
    assert!(
        history.contains(&format!(
            "{families} protocol family directories under `protocols/`"
        )),
        "v1.0.0 history anchor should match protocol family count"
    );
    assert!(
        history.contains(&format!(
            "{entries} packaged protocol entries under those family directories"
        )),
        "v1.0.0 history anchor should match protocol entry count"
    );
}

#[test]
fn release_checklist_uses_version_template_for_package_artifacts() {
    let root_manifest = read_repo_file("Cargo.toml");
    let checklist = read_repo_file("docs/release-checklist.md");
    let workspace_version = section_version(&root_manifest, "workspace.package");

    assert!(checklist.contains("target/packages/gewyvern_<version>-1_<deb-arch>.deb"));
    assert!(checklist.contains("target/packages/rpm/gewyvern-<version>-1.<rpm-arch>.rpm"));
    assert!(checklist.contains("root `gewyvern` package metadata"));
    assert!(checklist.contains(&format!("that resolves to `{workspace_version}`")));
    assert!(checklist.contains(&format!("gewyvern_{workspace_version}-1_<deb-arch>.deb")));
    assert!(checklist.contains(&format!("gewyvern-{workspace_version}-1.<rpm-arch>.rpm")));
    assert!(!checklist.contains("target/packages/gewyvern_0.20.0-1_<arch>.deb"));
    assert!(!checklist.contains("target/packages/rpm/gewyvern-0.20.0-1.<arch>.rpm"));
}
