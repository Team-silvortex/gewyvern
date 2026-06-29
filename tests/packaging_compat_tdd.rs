use std::fs;
use std::path::Path;

fn read_repo_file(relative: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    fs::read_to_string(&path).unwrap_or_else(|err| {
        panic!("failed to read {}: {}", path.display(), err);
    })
}

#[test]
fn package_layout_writes_compat_manifest() {
    let build_script = read_repo_file("scripts/packaging/build_packages.sh");

    assert!(build_script.contains("RELEASE_LINE=\"${GEWY_RELEASE_LINE:-v0.18.x}\""));
    assert!(build_script.contains("LAYOUT_VERSION=\"${GEWY_LAYOUT_VERSION:-1}\""));
    assert!(build_script.contains("CONFIG_SCHEMA_VERSION=\"${GEWY_CONFIG_SCHEMA_VERSION:-1}\""));
    assert!(build_script.contains("/usr/share/gewyvern/package-compat.toml"));
    assert!(build_script.contains("release_line = \"${RELEASE_LINE}\""));
    assert!(build_script.contains("layout_version = ${LAYOUT_VERSION}"));
    assert!(build_script.contains("config_schema_version = ${CONFIG_SCHEMA_VERSION}"));
    assert!(build_script.contains("/usr/share/gewyvern/examples/gewyvern.toml.example"));
    assert!(build_script.contains("copy-forward-without-overwrite"));
}

#[test]
fn rpm_template_matches_deb_staged_compat_contract() {
    let spec = read_repo_file("packaging/rpm/gewyvern.spec.in");

    assert!(spec.contains("/usr/share/gewyvern/package-compat.toml"));
    assert!(spec.contains("package_name = \"@PACKAGE_NAME@\""));
    assert!(spec.contains("package_version = \"@VERSION@\""));
    assert!(spec.contains("package_release = \"@RELEASE@\""));
    assert!(spec.contains("release_line = \"@RELEASE_LINE@\""));
    assert!(spec.contains("layout_version = @LAYOUT_VERSION@"));
    assert!(spec.contains("config_schema_version = @CONFIG_SCHEMA_VERSION@"));
    assert!(
        spec.contains("config_example = \"/usr/share/gewyvern/examples/gewyvern.toml.example\"")
    );
    assert!(spec.contains("upgrade_policy = \"copy-forward-without-overwrite\""));
}

#[test]
fn install_smoke_validates_packaged_compat_artifacts() {
    let smoke = read_repo_file("scripts/packaging/package_install_smoke.sh");

    assert!(smoke.contains("source \"${ROOT}/scripts/packaging/container_validation_common.sh\""));
    assert!(smoke.contains("container_validation_require_docker \"package install smoke\""));
    assert!(smoke.contains("container_validation_docker_run"));
    assert!(!smoke.contains("docker run --rm"));
    assert!(smoke.contains("rpm -Uvh /packages/$(basename \"${rpm_path}\")"));
    assert!(smoke.contains("|| dnf install -y /packages/$(basename \"${rpm_path}\")"));
    assert!(smoke.contains("RELEASE_LINE=\"${GEWY_RELEASE_LINE:-v0.18.x}\""));
    assert_eq!(
        smoke
            .matches("test -f /usr/share/gewyvern/package-compat.toml")
            .count(),
        2
    );
    assert_eq!(smoke.matches("grep -q '^schema_version = 1$'").count(), 2);
    assert_eq!(
        smoke
            .matches("grep -q '^release_line = \\\"${RELEASE_LINE}\\\"$'")
            .count(),
        2
    );
    assert_eq!(
        smoke
            .matches("test -f /usr/share/gewyvern/examples/gewyvern.toml.example")
            .count(),
        2
    );
    assert_eq!(smoke.matches("/usr/share/doc/gewyvern/LICENSE").count(), 2);
    assert!(smoke.contains("dpkg-deb -c"));
    assert!(smoke.contains("rpm -qpl"));
}

#[test]
fn container_validation_runner_is_bounded_and_cleans_up() {
    let common = read_repo_file("scripts/packaging/container_validation_common.sh");

    assert!(common.contains("container_validation_docker_run()"));
    assert!(common.contains("GEWY_CONTAINER_VALIDATION_TIMEOUT_SECONDS:-900"));
    assert!(common.contains("docker run --name \"${container_name}\" --rm"));
    assert!(common.contains("docker rm -f \"${container_name}\""));
    assert!(common.contains("rpm -Uvh /packages/$(basename \"${package_path}\")"));
    assert!(common.contains("|| dnf install -y /packages/$(basename \"${package_path}\")"));
}

#[test]
fn docs_record_the_install_compatibility_contract() {
    let packaging = read_repo_file("docs/packaging.md");
    let layout = read_repo_file("docs/book/reference-runtime-layout.md");

    for doc in [&packaging, &layout] {
        assert!(doc.contains("/usr/share/gewyvern/package-compat.toml"));
        assert!(doc.contains("/usr/share/gewyvern/examples/gewyvern.toml.example"));
        assert!(doc.contains("copy-forward-without-overwrite"));
    }

    assert!(packaging.contains("GEWY_RELEASE_LINE"));
    assert!(packaging.contains("GEWY_LAYOUT_VERSION"));
    assert!(packaging.contains("GEWY_CONFIG_SCHEMA_VERSION"));
    assert!(packaging.contains("GEWY_CONTAINER_VALIDATION_TIMEOUT_SECONDS"));
    assert!(packaging.contains("local `rpm -Uvh` first"));
    assert!(layout.contains("read-only layout marker"));
}
