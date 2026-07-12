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

    assert!(build_script.contains("RELEASE_LINE=\"${GEWY_RELEASE_LINE:-v1.0.0}\""));
    assert!(build_script.contains("LAYOUT_VERSION=\"${GEWY_LAYOUT_VERSION:-1}\""));
    assert!(build_script.contains("CONFIG_SCHEMA_VERSION=\"${GEWY_CONFIG_SCHEMA_VERSION:-1}\""));
    assert!(build_script.contains("TARGET_ROOT=\"${CARGO_TARGET_DIR:-${ROOT}/target}\""));
    assert!(
        build_script
            .contains("RELEASE_BIN_DIR=\"${GEWY_PACKAGE_BINARIES_ROOT:-${TARGET_ROOT}/release}\"")
    );
    assert!(build_script.contains("/usr/share/gewyvern/package-compat.toml"));
    assert!(build_script.contains("release_line = \"${RELEASE_LINE}\""));
    assert!(build_script.contains("layout_version = ${LAYOUT_VERSION}"));
    assert!(build_script.contains("config_schema_version = ${CONFIG_SCHEMA_VERSION}"));
    assert!(build_script.contains("${RELEASE_BIN_DIR}/gewyvern"));
    assert!(build_script.contains("/usr/share/gewyvern/examples/gewyvern.toml.example"));
    assert!(build_script.contains("copy-forward-without-overwrite"));
    assert!(build_script.contains("build_all_formats()"));
    assert!(build_script.contains("build_deb \"${version}\" \"${deb_arch}\" \"${stage_root}\" &"));
    assert!(build_script.contains("build_rpm \"${version}\" \"${rpm_arch}\" \"${stage_root}\" &"));
    assert!(build_script.contains("local stage_root=\"$3\""));
    assert!(build_script.contains("GEWY_TEMPLATE_STAGE_ROOT=\"${stage_root}\""));
    assert!(build_script.contains("SOURCE_DATE_EPOCH_VALUE=\"$(resolve_source_date_epoch)\""));
    assert!(build_script.contains("export SOURCE_DATE_EPOCH=\"${SOURCE_DATE_EPOCH_VALUE}\""));
    assert!(build_script.contains("normalize_stage_timestamps()"));
    assert!(build_script.contains("os.utime(path, (epoch, epoch), follow_symlinks=False)"));
    assert!(build_script.contains("Path(sys.argv[1]).stat().st_mtime"));
    assert!(build_script.contains("build-cache-key.txt"));
    assert!(build_script.contains("compute_package_cache_key()"));
    assert!(build_script.contains("can_reuse_cached_packages()"));
    assert!(build_script.contains("reusing cached package artifacts..."));
    assert!(build_script.contains("write_cache_key"));
    assert!(build_script.contains("--define \"use_source_date_epoch_as_buildtime 1\""));
    assert!(build_script.contains("--define \"clamp_mtime_to_source_date_epoch 1\""));
}

#[test]
fn rpm_template_matches_deb_staged_compat_contract() {
    let spec = read_repo_file("packaging/rpm/gewyvern.spec.in");

    assert!(spec.contains("/usr/share/gewyvern/package-compat.toml"));
    assert!(spec.contains("cp -a @STAGE_ROOT@/. %{buildroot}/"));
    assert!(!spec.contains("@SOURCE_ROOT@/dsl"));
    assert!(!spec.contains("@BINARIES_ROOT@/gewyvern"));
}

#[test]
fn install_smoke_validates_packaged_compat_artifacts() {
    let harness = read_repo_file("src/validation_harness/container_packaging.rs");
    let smoke = read_repo_file("scripts/packaging/package_install_smoke.sh");

    assert!(harness.contains("run_package_install_smoke"));
    assert!(harness.contains("GEWY_DEB_SMOKE_IMAGE"));
    assert!(harness.contains("GEWY_RPM_SMOKE_IMAGE"));
    assert!(harness.contains("RELEASE_LINE=\"${GEWY_RELEASE_LINE:-v1.0.0}\""));
    assert!(harness.contains("test -f /usr/share/gewyvern/package-compat.toml"));
    assert!(harness.contains("grep -q '^schema_version = 1$'"));
    assert!(harness.contains("release_line = \\\"${RELEASE_LINE}\\\""));
    assert!(harness.contains("test -f /usr/share/gewyvern/examples/gewyvern.toml.example"));
    assert!(harness.contains("/usr/share/doc/gewyvern/LICENSE"));
    assert!(harness.contains("dpkg-deb -c"));
    assert!(harness.contains("rpm -qpl"));
    assert!(smoke.contains("gewyvern_validate"));
    assert!(smoke.contains("package-install-smoke"));
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
    assert!(packaging.contains("SOURCE_DATE_EPOCH"));
    assert!(packaging.contains("GEWY_CONTAINER_VALIDATION_TIMEOUT_SECONDS"));
    assert!(packaging.contains("local `rpm -Uvh` first"));
    assert!(layout.contains("read-only layout marker"));
}
