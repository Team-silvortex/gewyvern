use gewyvern::dsl::compile_file;
use gewyvern::protocol_profiles::{
    protocol_default_entry, protocol_dsl_path, protocol_entries, protocol_surface,
};
use std::collections::BTreeSet;

fn protocol_fixture_path(relative: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("protocols")
        .join(relative)
        .to_string_lossy()
        .into_owned()
}

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[test]
fn mssql_registry_entries_resolve_to_packaged_paths() {
    assert_eq!(protocol_default_entry("mssql"), Some("query".to_string()));
    assert_eq!(
        protocol_dsl_path("mssql", None),
        Some(protocol_fixture_path("mssql/query"))
    );
    assert_eq!(
        protocol_dsl_path("tds", None),
        Some(protocol_fixture_path("mssql/prelogin"))
    );
    assert_eq!(
        protocol_dsl_path("sqlserver", None),
        Some(protocol_fixture_path("mssql/query"))
    );
    assert_eq!(
        protocol_dsl_path("mssql", Some("auth")),
        Some(protocol_fixture_path("mssql/login"))
    );
    assert_eq!(
        protocol_dsl_path("mssql", Some("tabular")),
        Some(protocol_fixture_path("mssql/response"))
    );
    assert_eq!(
        protocol_dsl_path("mssql", Some("result-shape")),
        Some(protocol_fixture_path("mssql/colmetadata"))
    );
    assert_eq!(
        protocol_dsl_path("mssql", Some("data-row")),
        Some(protocol_fixture_path("mssql/row"))
    );
    assert_eq!(
        protocol_dsl_path("mssql", Some("done-token")),
        Some(protocol_fixture_path("mssql/done"))
    );
    assert_eq!(
        protocol_dsl_path("mssql", Some("session-change")),
        Some(protocol_fixture_path("mssql/envchange"))
    );
    assert_eq!(
        protocol_dsl_path("mssql", Some("error-token")),
        Some(protocol_fixture_path("mssql/error"))
    );
}

#[test]
fn mssql_surface_exposes_database_cluster_shelves_and_semantics() {
    let entries = protocol_entries("mssql")
        .expect("mssql entries should resolve")
        .into_iter()
        .collect::<BTreeSet<_>>();
    assert_eq!(
        entries,
        [
            "prelogin",
            "login",
            "query",
            "response",
            "colmetadata",
            "row",
            "done",
            "envchange",
            "error"
        ]
        .into_iter()
        .map(String::from)
        .collect()
    );

    for (entry, shelf_key) in [
        ("prelogin", "session-query"),
        ("login", "session-query"),
        ("query", "session-query"),
        ("response", "session-query"),
        ("colmetadata", "token"),
        ("row", "token"),
        ("done", "token"),
        ("envchange", "token"),
        ("error", "error"),
    ] {
        let surface = protocol_surface("mssql", entry).expect("mssql surface should exist");
        assert_eq!(
            surface
                .cluster_hint
                .as_ref()
                .expect("mssql cluster should exist")
                .key,
            "database-query-session"
        );
        assert_eq!(
            surface.shelf.expect("mssql shelf should exist").key,
            shelf_key
        );
        assert!(
            surface.entry_semantics.is_some(),
            "mssql {entry} should expose debugger semantics"
        );
    }
}

#[test]
fn mssql_stable_subset_dsl_files_compile() {
    for file in [
        "mssql_prelogin_path.gewy",
        "mssql_login_path.gewy",
        "mssql_query_path.gewy",
        "mssql_response_path.gewy",
        "mssql_colmetadata_path.gewy",
        "mssql_row_path.gewy",
        "mssql_done_path.gewy",
        "mssql_envchange_path.gewy",
        "mssql_error_path.gewy",
    ] {
        let binding = compile_file(&dsl_fixture_path(file)).expect("mssql dsl should compile");
        assert!(
            binding.template.id.starts_with("mssql_"),
            "mssql template id should be protocol-specific"
        );
    }
}
