use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn protocol_fixture_path(relative: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("protocols")
        .join(relative)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn tftp_registry_entries_and_aliases_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("tftp", Some("read")),
        Some(protocol_fixture_path("tftp/read"))
    );
    assert_eq!(
        protocol_dsl_path("tftp", Some("rrq")),
        Some(protocol_fixture_path("tftp/read"))
    );
    assert_eq!(
        protocol_dsl_path("tftp-read", None),
        Some(protocol_fixture_path("tftp/read"))
    );
    assert_eq!(
        protocol_dsl_path("tftp", Some("upload")),
        Some(protocol_fixture_path("tftp/write"))
    );
    assert_eq!(
        protocol_dsl_path("tftp-wrq", None),
        Some(protocol_fixture_path("tftp/write"))
    );
    assert_eq!(
        protocol_dsl_path("tftp", Some("error-packet")),
        Some(protocol_fixture_path("tftp/error"))
    );
    assert_eq!(
        protocol_dsl_path("tftp-error", None),
        Some(protocol_fixture_path("tftp/error"))
    );
}

#[test]
fn tftp_default_and_entries_are_transfer_or_failure_oriented() {
    assert_eq!(protocol_default_entry("tftp"), Some("read".to_string()));

    let entries = protocol_entries("tftp").expect("tftp entries should resolve");
    assert!(entries.contains(&"read".to_string()));
    assert!(entries.contains(&"write".to_string()));
    assert!(entries.contains(&"error".to_string()));
}

#[test]
fn tftp_surface_exposes_transfer_and_failure_shelves() {
    let read = protocol_surface("tftp", "read").expect("tftp read surface should exist");
    assert_eq!(
        read.cluster_hint
            .as_ref()
            .map(|cluster| cluster.key.as_str()),
        Some("network-control-discovery")
    );
    assert_eq!(
        read.shelf.as_ref().map(|shelf| shelf.key.as_str()),
        Some("transfer")
    );
    assert_eq!(
        read.entry_semantics
            .as_ref()
            .map(|semantics| semantics.category.as_str()),
        Some("tftp-read-path")
    );

    let write = protocol_surface("tftp", "write").expect("tftp write surface should exist");
    assert_eq!(
        write.shelf.as_ref().map(|shelf| shelf.key.as_str()),
        Some("transfer")
    );
    assert_eq!(
        write
            .entry_semantics
            .as_ref()
            .and_then(|semantics| semantics.typical_signal.as_deref()),
        Some("WRQ + ACK")
    );

    let error = protocol_surface("tftp", "error").expect("tftp error surface should exist");
    assert_eq!(
        error.shelf.as_ref().map(|shelf| shelf.key.as_str()),
        Some("failure")
    );
    assert_eq!(
        error
            .entry_semantics
            .as_ref()
            .and_then(|semantics| semantics.primary_failure_mode.as_deref()),
        Some("server_denied")
    );
}

#[test]
fn tftp_dsl_files_compile_into_expected_operations() {
    for (fixture, operation) in [
        ("tftp_read_path.gewy", "tftp_read"),
        ("tftp_write_path.gewy", "tftp_write"),
        ("tftp_error_path.gewy", "tftp_error"),
    ] {
        let binding = compile_file(&dsl_fixture_path(fixture)).unwrap();
        assert_eq!(
            binding.template.program_model.as_ref().unwrap().operation,
            ProgramOperation::Custom(operation.into())
        );
    }
}
