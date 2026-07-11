use super::data_api::{
    ApiRenderedTarget, ApiSnapshot, ApiTargetSnapshot, api_response_for_request,
    api_snapshot_meta_json, update_api_snapshot_for_scan, update_api_snapshot_for_single,
};
use super::external_analysis::{ExternalAnalysisConfig, set_external_analysis_config, test_guard};
use super::helpers::process_matches_pid;
use super::{
    AnalysisAugmenter, AnalysisSnapshot, Cli, IngestMode, ReportFormat, analysis_snapshot,
    analysis_snapshot_json, analysis_snapshot_with_augmenters, annotate_export_trust,
    collect_analyses, collect_cli_outputs, filter_export_by_pid, findings_json,
    findings_json_with_analysis, http_transactions_json, http_transactions_text,
    list_entries_json, list_entries_text, list_protocols_json, list_protocols_text,
    protocol_dsl_path, push_analysis_augmentation,
    render_debug_session_outputs, render_debugger_console_outputs, render_report_outputs,
    route_fact, run_binding_demo, scan_analysis_json_array, scan_report_html, scan_report_html_with_analyses,
    scan_report_json, scan_report_json_with_analyses, scan_report_text,
    scan_report_text_with_analyses, scan_targets_for_cli,
    single_target_report_html_with_analysis, single_target_report_json_with_analysis, summary_json, summary_line,
    training_example_json, training_example_json_array, training_example_json_with_analysis,
};
use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::http::HttpTransactionView;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId,
};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;
use std::time::Instant;
use std::time::SystemTime;

pub(super) fn env_test_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

pub(super) fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

pub(super) fn protocol_fixture_path(relative: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("protocols")
        .join(relative)
        .to_string_lossy()
        .into_owned()
}

#[path = "tests/ambiguity.rs"]
mod ambiguity;
#[cfg(target_family = "unix")]
#[path = "tests/analysis.rs"]
mod analysis;
#[path = "tests/api_bench.rs"]
mod api_bench;
#[path = "tests/api_debugger.rs"]
mod api_debugger;
#[path = "tests/api_multi_instance.rs"]
mod api_multi_instance;
#[path = "tests/api_persistence.rs"]
mod api_persistence;
#[path = "tests/api_protocol_catalog.rs"]
mod api_protocol_catalog;
#[path = "tests/api_protocol_surface_semantics.rs"]
mod api_protocol_surface_semantics;
#[path = "tests/api_protocol_surface_semantics_amqp.rs"]
mod api_protocol_surface_semantics_amqp;
#[path = "tests/api_protocol_surface_semantics_database.rs"]
mod api_protocol_surface_semantics_database;
#[path = "tests/api_protocol_surface_semantics_http3_hy2.rs"]
mod api_protocol_surface_semantics_http3_hy2;
#[path = "tests/api_protocol_surface_semantics_ldap.rs"]
mod api_protocol_surface_semantics_ldap;
#[path = "tests/api_protocol_surface_semantics_mysql.rs"]
mod api_protocol_surface_semantics_mysql;
#[path = "tests/api_protocol_surface_semantics_postgres.rs"]
mod api_protocol_surface_semantics_postgres;
#[path = "tests/api_protocol_surface_semantics_quic.rs"]
mod api_protocol_surface_semantics_quic;
#[path = "tests/api_protocol_surface_semantics_radius.rs"]
mod api_protocol_surface_semantics_radius;
#[path = "tests/api_protocol_surface_semantics_tls.rs"]
mod api_protocol_surface_semantics_tls;
#[path = "tests/api_protocol_surface_semantics_wireguard.rs"]
mod api_protocol_surface_semantics_wireguard;
#[path = "tests/api_sidecar.rs"]
mod api_sidecar;
#[path = "tests/certificate_api.rs"]
mod certificate_api;
#[path = "tests/cli_security.rs"]
mod cli_security;
#[path = "tests/debugger_console_cli.rs"]
mod debugger_console_cli;
#[path = "tests/demo_cli.rs"]
mod demo_cli;
#[path = "tests/directory_protocols.rs"]
mod directory_protocols;
#[path = "tests/failure_labels.rs"]
mod failure_labels;
#[path = "tests/ftp.rs"]
mod ftp;
#[path = "tests/history_cli.rs"]
mod history_cli;
#[path = "tests/machine_surface_matrix.rs"]
mod machine_surface_matrix;
#[path = "tests/mail_delivery.rs"]
mod mail_delivery;
#[path = "tests/mail_flow.rs"]
mod mail_flow;
#[path = "tests/management_udp_direct_signal_semantics.rs"]
mod management_udp_direct_signal_semantics;
#[path = "tests/management_udp_failure_semantics.rs"]
mod management_udp_failure_semantics;
#[path = "tests/management_udp_result_semantics.rs"]
mod management_udp_result_semantics;
#[path = "tests/persisted_machine_surface_matrix.rs"]
mod persisted_machine_surface_matrix;
#[path = "tests/pop_kerberos.rs"]
mod pop_kerberos;
#[path = "tests/redis_failure_semantics.rs"]
mod redis_failure_semantics;
#[path = "tests/relay_and_mail.rs"]
mod relay_and_mail;
#[path = "tests/reports.rs"]
mod reports;
#[path = "tests/resilience_api.rs"]
mod resilience_api;
#[path = "tests/rtsp_http_tls.rs"]
mod rtsp_http_tls;
#[path = "tests/runtime_config.rs"]
mod runtime_config;
#[path = "tests/runtime_migration.rs"]
mod runtime_migration;
#[path = "tests/scan_precomputed_bench.rs"]
mod scan_precomputed_bench;
#[path = "tests/serve_runtime_target_naming.rs"]
mod serve_runtime_target_naming;
#[path = "tests/snmp_failure_semantics.rs"]
mod snmp_failure_semantics;
#[path = "tests/snmp_result_semantics.rs"]
mod snmp_result_semantics;
#[path = "tests/ssh_tls.rs"]
mod ssh_tls;
#[path = "tests/support_external.rs"]
mod support_external;
#[path = "tests/support_facts.rs"]
mod support_facts;
#[path = "tests/training_surface.rs"]
mod training_surface;

use self::support_external::*;
use self::support_facts::*;
