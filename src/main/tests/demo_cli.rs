use super::{
    Cli, IngestMode, ReportFormat, collect_cli_outputs, dsl_fixture_path, list_entries_json,
    list_entries_text, list_protocols_json, list_protocols_text, protocol_dsl_path,
    protocol_fixture_path, run_binding_demo, scan_targets_for_cli,
};
use crate::helpers::scan_targets_from_set_file;
use crate::serve_runtime::SOCKET_SESSION_TARGET_NAME;
use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use std::fs;
use std::time::{SystemTime, UNIX_EPOCH};

#[test]
fn http_request_demo_produces_healthy_cross_transport_path() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http_request_path DSL should compile");
    let bundle = run_binding_demo(binding);
    assert_eq!(bundle.debug_summary.accepted_facts, 6);
    assert_eq!(bundle.program_findings.len(), 0);
    assert_eq!(bundle.module_findings.len(), 0);
    assert_eq!(bundle.program_flows.len(), 1);
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_request"))
    );
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response"))
    );
    assert_eq!(
        bundle.program_flows[0].operation,
        ProgramOperation::Custom("http_request".into())
    );
}

#[test]
fn tls_client_demo_produces_healthy_packet_phase() {
    let binding = compile_file(&dsl_fixture_path("tls_client_path.gewy"))
        .expect("tls_client_path DSL should compile");
    let bundle = run_binding_demo(binding);
    assert_eq!(bundle.program_findings.len(), 0);
    assert_eq!(bundle.module_findings.len(), 0);
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_client_hello"))
    );
    assert_eq!(
        bundle.program_flows[0].operation,
        ProgramOperation::Custom("tls_client".into())
    );
}

#[test]
fn http_server_response_demo_produces_healthy_server_path() {
    let binding = compile_file(&dsl_fixture_path("http_server_response_path.gewy"))
        .expect("http_server_response_path DSL should compile");
    let bundle = run_binding_demo(binding);
    assert_eq!(bundle.program_findings.len(), 0);
    assert_eq!(bundle.module_findings.len(), 0);
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_request"))
    );
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_response"))
    );
    assert_eq!(
        bundle.program_flows[0].operation,
        ProgramOperation::Custom("http_server_response".into())
    );
}

#[test]
fn http3_request_demo_produces_healthy_quic_path() {
    let binding = compile_file(&dsl_fixture_path("http3_request_path.gewy"))
        .expect("http3_request_path DSL should compile");
    let bundle = run_binding_demo(binding);
    assert_eq!(bundle.program_findings.len(), 0);
    assert_eq!(bundle.module_findings.len(), 0);
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_request_stream"))
    );
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_response_stream"))
    );
    assert_eq!(
        bundle.program_flows[0].operation,
        ProgramOperation::Custom("http3_request".into())
    );
}

#[test]
fn http3_server_response_demo_produces_healthy_quic_server_path() {
    let binding = compile_file(&dsl_fixture_path("http3_server_response_path.gewy"))
        .expect("http3_server_response_path DSL should compile");
    let bundle = run_binding_demo(binding);
    assert_eq!(bundle.program_findings.len(), 0);
    assert_eq!(bundle.module_findings.len(), 0);
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_request_stream"))
    );
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_response_stream"))
    );
    assert_eq!(
        bundle.program_flows[0].operation,
        ProgramOperation::Custom("http3_server_response".into())
    );
}

#[test]
fn hy2_auth_demo_produces_healthy_quic_auth_path() {
    let binding = compile_file(&dsl_fixture_path("hy2_auth_path.gewy"))
        .expect("hy2_auth_path DSL should compile");
    let bundle = run_binding_demo(binding);
    assert_eq!(bundle.program_findings.len(), 0);
    assert_eq!(bundle.module_findings.len(), 0);
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_auth_request_stream"))
    );
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_auth_ok_stream"))
    );
    assert_eq!(
        bundle.program_flows[0].operation,
        ProgramOperation::Custom("hy2_auth".into())
    );
}

#[test]
fn hy2_udp_relay_demo_produces_healthy_quic_datagram_path() {
    let binding = compile_file(&dsl_fixture_path("hy2_udp_relay_path.gewy"))
        .expect("hy2_udp_relay_path DSL should compile");
    let bundle = run_binding_demo(binding);
    assert_eq!(bundle.program_findings.len(), 0);
    assert_eq!(bundle.module_findings.len(), 0);
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_udp_relay_datagram"))
    );
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_udp_relay_datagram"))
    );
    assert_eq!(
        bundle.program_flows[0].operation,
        ProgramOperation::Custom("hy2_udp_relay".into())
    );
}

#[test]
fn hy2_tcp_relay_demo_produces_healthy_quic_tcp_path() {
    let binding = compile_file(&dsl_fixture_path("hy2_tcp_relay_path.gewy"))
        .expect("hy2_tcp_relay_path DSL should compile");
    let bundle = run_binding_demo(binding);
    assert_eq!(bundle.program_findings.len(), 0);
    assert_eq!(bundle.module_findings.len(), 0);
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("send_tcp_request_stream"))
    );
    assert!(
        bundle.program_flows[0]
            .stages
            .iter()
            .any(|stage| stage.phase.as_deref() == Some("receive_tcp_response_stream"))
    );
    assert_eq!(
        bundle.program_flows[0].operation,
        ProgramOperation::Custom("hy2_tcp_relay".into())
    );
}

#[test]
fn cli_rejects_pid_until_live_process_capture_is_wired() {
    let err = Cli::from_args([
        "--protocol".to_string(),
        "mysql".to_string(),
        "--entry".to_string(),
        "session".to_string(),
        "--pid".to_string(),
        "4242".to_string(),
        "--json".to_string(),
    ])
    .unwrap_err();
    assert!(err.contains("--pid"));
    assert!(err.contains("live process") || err.contains("活进程"));
}

#[test]
fn cli_rejects_combined_dsl_and_protocol() {
    let err = Cli::from_args([
        "--dsl".to_string(),
        "/tmp/demo.gewy".to_string(),
        "--protocol".to_string(),
        "mysql-query".to_string(),
    ])
    .unwrap_err();
    assert!(err.contains("--dsl"));
    assert!(err.contains("--protocol"));
}

#[test]
fn protocol_lookup_covers_mysql_session() {
    assert_eq!(
        protocol_dsl_path("mysql", Some("session")),
        Some(protocol_fixture_path("mysql/session").to_string())
    );
}

#[test]
fn protocol_lookup_uses_default_entry_when_none_is_provided() {
    assert_eq!(
        protocol_dsl_path("mysql", None),
        Some(protocol_fixture_path("mysql/session").to_string())
    );
    assert_eq!(
        protocol_dsl_path("amqp", None),
        Some(protocol_fixture_path("amqp/session").to_string())
    );
}

#[test]
fn cli_rejects_entry_without_protocol() {
    let err = Cli::from_args(["--entry".to_string(), "session".to_string()]).unwrap_err();
    assert!(err.contains("--entry"));
    assert!(err.contains("--protocol"));
}

#[test]
fn legacy_protocol_alias_still_resolves() {
    assert_eq!(
        protocol_dsl_path("mysql-session", None),
        Some(protocol_fixture_path("mysql/session").to_string())
    );
}

#[test]
fn cli_accepts_list_protocols_mode() {
    let cli = Cli::from_args(["--list-protocols".to_string(), "--json".to_string()]).unwrap();
    assert!(cli.list_protocols);
    assert_eq!(cli.list_entries, None);
}

#[test]
fn cli_accepts_list_entries_mode() {
    let cli = Cli::from_args(["--list-entries".to_string(), "mysql".to_string()]).unwrap();
    assert!(!cli.list_protocols);
    assert_eq!(cli.list_entries.as_deref(), Some("mysql"));
}

#[test]
fn cli_accepts_scan_all_mode() {
    let cli = Cli::from_args(["--scan-all".to_string(), "--json".to_string()]).unwrap();
    assert!(cli.scan_all);
    assert_eq!(cli.protocol_set_path, None);
}

#[test]
fn cli_accepts_html_report_format_for_scan_all() {
    let cli = Cli::from_args([
        "--scan-all".to_string(),
        "--report-format".to_string(),
        "html".to_string(),
    ])
    .unwrap();
    assert_eq!(cli.report_format, Some(ReportFormat::Html));
}

#[test]
fn cli_accepts_summary_only_html_report_without_json_flag() {
    let cli = Cli::from_args([
        "--scan-all".to_string(),
        "--summary-only".to_string(),
        "--report-format".to_string(),
        "html".to_string(),
    ])
    .unwrap();
    assert!(cli.summary_only);
    assert_eq!(cli.report_format, Some(ReportFormat::Html));
    assert!(!cli.json);
}

#[test]
fn cli_accepts_protocol_html_report_without_scan_all() {
    let cli = Cli::from_args([
        "--protocol".to_string(),
        "mysql".to_string(),
        "--entry".to_string(),
        "session".to_string(),
        "--report-format".to_string(),
        "html".to_string(),
    ])
    .unwrap();
    assert_eq!(cli.report_format, Some(ReportFormat::Html));
    assert_eq!(cli.protocol.as_deref(), Some("mysql"));
}

#[test]
fn protocol_selector_uses_scan_style_target_label_for_demo_outputs() {
    let cli = Cli::from_args([
        "--protocol".to_string(),
        "mysql".to_string(),
        "--entry".to_string(),
        "session".to_string(),
    ])
    .unwrap();
    let outputs = collect_cli_outputs(&cli, SystemTime::UNIX_EPOCH, &[], crate::UiLocale::detect());
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].0, "scan:mysql:session");
}

#[test]
fn protocol_selector_uses_default_entry_in_target_label_when_entry_is_omitted() {
    let cli = Cli::from_args(["--protocol".to_string(), "mysql".to_string()]).unwrap();
    let outputs = collect_cli_outputs(&cli, SystemTime::UNIX_EPOCH, &[], crate::UiLocale::detect());
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].0, "scan:mysql:session");
}

#[test]
fn dsl_selector_falls_back_to_runtime_target_name_for_unknown_template() {
    let cli = Cli::from_args([
        "--dsl".to_string(),
        dsl_fixture_path("udp_process_debug.gewy"),
    ])
    .unwrap();
    let outputs = collect_cli_outputs(&cli, SystemTime::UNIX_EPOCH, &[], crate::UiLocale::detect());
    assert_eq!(outputs.len(), 1);
    assert_eq!(outputs[0].0, SOCKET_SESSION_TARGET_NAME);
}

#[test]
fn cli_rejects_protocol_set_without_scan_all() {
    let err = Cli::from_args([
        "--protocol-set".to_string(),
        "/tmp/protocols.txt".to_string(),
    ])
    .unwrap_err();
    assert!(err.contains("--protocol-set"));
    assert!(err.contains("--scan-all"));
}

#[test]
fn cli_rejects_scan_all_with_protocol_selector() {
    let err = Cli::from_args([
        "--scan-all".to_string(),
        "--protocol".to_string(),
        "mysql".to_string(),
    ])
    .unwrap_err();
    assert!(err.contains("--scan-all"));
    assert!(err.contains("--protocol"));
}

#[test]
fn cli_rejects_combined_list_modes() {
    let err = Cli::from_args([
        "--list-protocols".to_string(),
        "--list-entries".to_string(),
        "mysql".to_string(),
    ])
    .unwrap_err();
    assert!(err.contains("--list-protocols"));
    assert!(err.contains("--list-entries"));
}

#[test]
fn list_protocols_output_includes_mysql_default_entry() {
    let text = list_protocols_text();
    assert!(text.contains("mysql (default: session)"));

    let json = list_protocols_json();
    assert!(json.contains("\"protocol\":\"mysql\""));
    assert!(json.contains("\"default_entry\":\"session\""));
    assert!(json.contains("\"entries\":["));
}

#[test]
fn list_entries_output_marks_default_entry() {
    let text = list_entries_text("ldap").expect("ldap should be present");
    assert!(text.contains("sync (default)"));
    assert!(text.contains("bind"));

    let text = list_entries_text("mysql").expect("mysql should be present");
    assert!(text.contains("query"));
    assert!(text.contains("session (default)"));
    assert!(text.contains("aliases: mysql-query, mysql_query"));

    let json = list_entries_json("mysql").expect("mysql should be present");
    assert!(json.contains("\"protocol\":\"mysql\""));
    assert!(json.contains(
        "\"mode\":\"query\",\"default\":false,\"aliases\":[\"mysql-query\",\"mysql_query\"]"
    ));
    assert!(json.contains(
        "\"mode\":\"session\",\"default\":true,\"aliases\":[\"mysql-session\",\"mysql_session\"]"
    ));
}

#[test]
fn default_scan_targets_include_protocol_defaults() {
    let cli = Cli::from_args(["--scan-all".to_string()]).unwrap();
    let targets = scan_targets_for_cli(&cli).unwrap();
    assert!(
        targets
            .iter()
            .any(|target| { target.protocol == "mysql" && target.entry == "session" })
    );
    assert!(
        targets
            .iter()
            .any(|target| { target.protocol == "amqp" && target.entry == "session" })
    );
    assert!(
        targets
            .iter()
            .any(|target| { target.protocol == "mysql" && target.entry == "connect" })
    );
    assert!(
        targets
            .iter()
            .any(|target| { target.protocol == "hy2" && target.entry == "tcp" })
    );
}

#[test]
fn protocol_set_file_parses_comments_defaults_and_explicit_entries() {
    let path = std::env::temp_dir().join(format!(
        "gewyvern-protocol-set-{}.txt",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::write(&path, "# comment\nmysql\namqp:publish\nldap bind\nmysql\n").unwrap();
    let targets = scan_targets_from_set_file(path.to_str().unwrap()).unwrap();
    fs::remove_file(&path).unwrap();

    assert_eq!(targets.len(), 3);
    assert_eq!(targets[0].protocol, "mysql");
    assert_eq!(targets[0].entry, "session");
    assert_eq!(targets[1].protocol, "amqp");
    assert_eq!(targets[1].entry, "publish");
    assert_eq!(targets[2].protocol, "ldap");
    assert_eq!(targets[2].entry, "bind");
}

#[test]
fn protocol_set_directory_scans_registered_gewy_projects() {
    let root = std::env::temp_dir().join(format!(
        "gewyvern-protocol-registry-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let package_dir = root.join("mysql").join("session");
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
            package_dir.join("gewy.pkg"),
            "name=mysql_session\nversion=0.5.0\nentry=main.gewy\nregister.protocol=mysql\nregister.entry=session\nregister.default=true\n",
        )
        .unwrap();
    fs::write(
            package_dir.join("main.gewy"),
            "template(:mysql_session)\n|> window(:default_5s)\n|> reason(:udp_datagram_l1)\n|> fragment(:udp_packet_meta_fragment)\n|> fragment(:route_meta_fragment)\n",
        )
        .unwrap();

    let targets = scan_targets_from_set_file(root.to_str().unwrap()).unwrap();
    fs::remove_dir_all(&root).unwrap();

    assert_eq!(targets.len(), 1);
    assert_eq!(targets[0].protocol, "mysql");
    assert_eq!(targets[0].entry, "session");
    assert!(targets[0].dsl_path.ends_with("/mysql/session"));
}

#[test]
fn protocol_set_directory_surfaces_manifest_diagnostics() {
    let root = std::env::temp_dir().join(format!(
        "gewyvern-invalid-protocol-registry-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    let package_dir = root.join("mysql").join("session");
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(package_dir.join("main.gewy"), "fragment packet_meta {}").unwrap();
    fs::write(package_dir.join("gewy.pkg"), "entry=main.gewy\n").unwrap();

    let error = scan_targets_from_set_file(root.to_str().unwrap()).unwrap_err();
    fs::remove_dir_all(&root).unwrap();

    assert!(error.contains("missing register.protocol"));
    assert!(!error.contains("did not resolve any scan targets"));
}

#[test]
fn cli_rejects_remote_tcp_socket_without_explicit_flag() {
    let err = Cli::from_args(["--tcp-socket".to_string(), "0.0.0.0:9000".to_string()]).unwrap_err();
    assert!(err.contains("--allow-remote-socket"));
}

#[test]
fn cli_rejects_malformed_unix_socket_targets() {
    assert!(
        Cli::from_args([
            "--unix-socket".to_string(),
            " /tmp/gewyvern.sock".to_string()
        ])
        .is_err()
    );
    assert!(
        Cli::from_args([
            "--unix-socket".to_string(),
            "/tmp/gewyvern.sock ".to_string()
        ])
        .is_err()
    );
    assert!(
        Cli::from_args([
            "--unix-socket".to_string(),
            "bad\u{0007}socket.sock".to_string(),
        ])
        .is_err()
    );
}

#[test]
fn cli_rejects_malformed_tcp_socket_targets() {
    assert!(Cli::from_args(["--tcp-socket".to_string(), " 127.0.0.1:9000".to_string()]).is_err());
    assert!(Cli::from_args(["--tcp-socket".to_string(), "127.0.0.1:9000 ".to_string()]).is_err());
    assert!(Cli::from_args(["--tcp-socket".to_string(), "127.0.0.1:\n9000".to_string(),]).is_err());
}

#[test]
fn cli_rejects_malformed_api_socket_targets() {
    assert!(
        Cli::from_args([
            "--serve".to_string(),
            "--api-socket".to_string(),
            " 127.0.0.1:9100".to_string()
        ])
        .is_err()
    );
    assert!(
        Cli::from_args([
            "--serve".to_string(),
            "--api-socket".to_string(),
            "127.0.0.1:9100 ".to_string()
        ])
        .is_err()
    );
    assert!(
        Cli::from_args([
            "--serve".to_string(),
            "--api-socket".to_string(),
            "127.0.0.1:\n9100".to_string(),
        ])
        .is_err()
    );
}

#[test]
fn cli_accepts_remote_tcp_socket_with_explicit_flag() {
    let cli = Cli::from_args([
        "--tcp-socket".to_string(),
        "0.0.0.0:9000".to_string(),
        "--allow-remote-socket".to_string(),
    ])
    .unwrap();
    assert_eq!(cli.ingest_mode, IngestMode::RemoteAdvisory);
}

#[test]
fn cli_accepts_loopback_tcp_socket_without_remote_flag() {
    let cli = Cli::from_args(["--tcp-socket".to_string(), "127.0.0.1:9000".to_string()]).unwrap();
    assert_eq!(cli.ingest_mode, IngestMode::LocalAdvisory);
}

#[test]
fn cli_accepts_explicit_ingest_mode() {
    let cli = Cli::from_args([
        "--tcp-socket".to_string(),
        "0.0.0.0:9000".to_string(),
        "--ingest-mode".to_string(),
        "remote-advisory".to_string(),
    ])
    .unwrap();
    assert_eq!(cli.ingest_mode, IngestMode::RemoteAdvisory);
}

#[test]
fn cli_accepts_legacy_socket_trust_alias() {
    let cli = Cli::from_args([
        "--tcp-socket".to_string(),
        "0.0.0.0:9000".to_string(),
        "--socket-trust".to_string(),
        "unsafe-remote".to_string(),
    ])
    .unwrap();
    assert_eq!(cli.ingest_mode, IngestMode::RemoteAdvisory);
}

#[test]
fn cli_rejects_unknown_ingest_mode() {
    let err = Cli::from_args(["--ingest-mode".to_string(), "mystery".to_string()]).unwrap_err();
    assert!(err.contains("ingest mode") || err.contains("采集模式"));
}

#[test]
fn cli_rejects_pid_filter_for_socket_ingest() {
    let err = Cli::from_args([
        "--tcp-socket".to_string(),
        "127.0.0.1:9000".to_string(),
        "--pid".to_string(),
        "4242".to_string(),
    ])
    .unwrap_err();
    assert!(err.contains("--pid"));
    assert!(err.contains("live process") || err.contains("活进程"));
}

#[test]
fn cli_rejects_api_socket_without_serve() {
    let err = Cli::from_args([
        "--tcp-socket".to_string(),
        "127.0.0.1:9000".to_string(),
        "--api-socket".to_string(),
        "127.0.0.1:9100".to_string(),
    ])
    .unwrap_err();
    assert!(err.contains("--api-socket"));
    assert!(err.contains("--serve"));
}

#[test]
fn cli_rejects_remote_api_socket_without_explicit_flag() {
    let err = Cli::from_args([
        "--tcp-socket".to_string(),
        "127.0.0.1:9000".to_string(),
        "--serve".to_string(),
        "--api-socket".to_string(),
        "0.0.0.0:9100".to_string(),
    ])
    .unwrap_err();
    assert!(err.contains("--allow-remote-api"));
}

#[test]
fn cli_accepts_remote_api_socket_with_explicit_flag() {
    let cli = Cli::from_args([
        "--tcp-socket".to_string(),
        "127.0.0.1:9000".to_string(),
        "--serve".to_string(),
        "--api-socket".to_string(),
        "0.0.0.0:9100".to_string(),
        "--api-admin-token".to_string(),
        "this_is_a_valid_admin_token_32_ch".to_string(),
        "--allow-remote-api".to_string(),
    ])
    .unwrap();
    assert!(cli.serve);
    assert_eq!(cli.api_socket.as_deref(), Some("0.0.0.0:9100"));
    assert!(cli.allow_remote_api);
}

#[test]
fn cli_rejects_remote_api_socket_without_admin_token() {
    let err = Cli::from_args([
        "--tcp-socket".to_string(),
        "127.0.0.1:9000".to_string(),
        "--serve".to_string(),
        "--api-socket".to_string(),
        "0.0.0.0:9100".to_string(),
        "--allow-remote-api".to_string(),
    ])
    .unwrap_err();
    assert!(err.contains("admin token") || err.contains("GEWY_API_ADMIN_TOKEN"));
}

#[test]
fn cli_rejects_invalid_api_admin_token() {
    let err = Cli::from_args([
        "--tcp-socket".to_string(),
        "127.0.0.1:9000".to_string(),
        "--api-admin-token".to_string(),
        "short".to_string(),
        "--serve".to_string(),
        "--api-socket".to_string(),
        "127.0.0.1:9100".to_string(),
    ])
    .unwrap_err();
    assert!(
        err.contains("--api-admin-token is invalid; use 32-256 non-whitespace characters")
            || err.contains("--api-admin-token is invalid")
    );
}

#[test]
fn cli_rejects_api_admin_token_with_control_characters() {
    let err = Cli::from_args([
        "--tcp-socket".to_string(),
        "127.0.0.1:9000".to_string(),
        "--api-admin-token".to_string(),
        "valid_admin_token_with_control_\u{0007}_characters__xxyyzz".to_string(),
        "--serve".to_string(),
        "--api-socket".to_string(),
        "127.0.0.1:9100".to_string(),
    ])
    .unwrap_err();
    assert!(
        err.contains("--api-admin-token is invalid; use 32-256 non-whitespace characters")
            || err.contains("--api-admin-token is invalid")
    );
}

#[test]
fn api_admin_token_resolution_accepts_environment_and_prefers_configuration() {
    assert_eq!(
        crate::cli::resolve_api_admin_token(None, Some("bad\u{0007}token".into())),
        None
    );
    assert_eq!(
        crate::cli::resolve_api_admin_token(
            None,
            Some("  runtime-api-token-abcdefghijklmnopqrstuvwxyz  ".into()),
        )
        .as_deref(),
        Some("runtime-api-token-abcdefghijklmnopqrstuvwxyz")
    );
    assert_eq!(
        crate::cli::resolve_api_admin_token(
            Some("configured-admin-token-0123456789".into()),
            Some("runtime-api-token-abcdefghijklmnopqrstuvwxyz".into()),
        )
        .as_deref(),
        Some("configured-admin-token-0123456789")
    );
    assert_eq!(
        crate::cli::resolve_api_admin_token(None, Some("   ".into())),
        None
    );
}
