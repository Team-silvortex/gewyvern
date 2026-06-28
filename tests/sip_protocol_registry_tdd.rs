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
fn sip_response_and_denied_entries_resolve_to_packaged_paths() {
    assert_eq!(
        protocol_dsl_path("sip", Some("response")),
        Some(protocol_fixture_path("sip/response").to_string())
    );
    assert_eq!(
        protocol_dsl_path("sip", Some("reply")),
        Some(protocol_fixture_path("sip/response").to_string())
    );
    assert_eq!(
        protocol_dsl_path("sip", Some("denied")),
        Some(protocol_fixture_path("sip/denied").to_string())
    );
    assert_eq!(
        protocol_dsl_path("sip", Some("4xx")),
        Some(protocol_fixture_path("sip/denied").to_string())
    );
}

#[test]
fn sip_default_stays_register_while_response_entries_grow() {
    assert_eq!(protocol_default_entry("sip"), Some("register".to_string()));

    let entries = protocol_entries("sip").expect("sip entries should resolve");
    for entry in ["register", "invite", "bye", "response", "denied"] {
        assert!(
            entries.contains(&entry.to_string()),
            "sip entries should include `{entry}`"
        );
    }
}

#[test]
fn sip_response_and_denied_surfaces_expose_shelves_and_semantics() {
    let response = protocol_surface("sip", "response").expect("sip response surface");
    assert_eq!(response.shelf.expect("response shelf").key, "response");
    let response_semantics = response.entry_semantics.expect("response semantics");
    assert_eq!(response_semantics.category, "response-path");
    assert_eq!(
        response_semantics.typical_signal.as_deref(),
        Some("SIP/2.0")
    );

    let denied = protocol_surface("sip", "denied").expect("sip denied surface");
    assert_eq!(denied.shelf.expect("denied shelf").key, "denied");
    let denied_semantics = denied.entry_semantics.expect("denied semantics");
    assert_eq!(denied_semantics.category, "failure-path");
    assert_eq!(
        denied_semantics.primary_failure_detail.as_deref(),
        Some("session_control_rejected")
    );
}

#[test]
fn sip_response_and_denied_dsl_files_compile_into_expected_operations() {
    let response = compile_file(&dsl_fixture_path("sip_response_path.gewy")).unwrap();
    assert_eq!(response.template.id, "sip_response_path");
    assert_eq!(
        response
            .template
            .program_model
            .as_ref()
            .map(|program| &program.operation),
        Some(&ProgramOperation::Custom("sip_response".into()))
    );

    let denied = compile_file(&dsl_fixture_path("sip_denied_path.gewy")).unwrap();
    assert_eq!(denied.template.id, "sip_denied_path");
    assert_eq!(
        denied
            .template
            .program_model
            .as_ref()
            .map(|program| &program.operation),
        Some(&ProgramOperation::Custom("sip_denied".into()))
    );
}
