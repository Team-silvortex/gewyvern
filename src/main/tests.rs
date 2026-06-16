use super::data_api::{
    ApiRenderedTarget, ApiSnapshot, api_response_for_request, api_snapshot_meta_json,
    update_api_snapshot_for_scan, update_api_snapshot_for_single,
};
use super::external_analysis::{ExternalAnalysisConfig, set_external_analysis_config, test_guard};
use super::helpers::process_matches_pid;
use super::{
    AnalysisAugmenter, AnalysisSnapshot, Cli, IngestMode, ReportFormat, analysis_snapshot,
    analysis_snapshot_json, analysis_snapshot_with_augmenters, annotate_export_trust,
    filter_export_by_pid, findings_json, findings_json_with_analysis, http_transactions_json,
    http_transactions_text, list_entries_json, list_entries_text, list_protocols_json,
    list_protocols_text, protocol_dsl_path, push_analysis_augmentation, render_report_outputs,
    route_fact, run_binding_demo, scan_report_html, scan_report_json, scan_report_text,
    scan_targets_for_cli, summary_json, summary_line, training_example_json,
    training_example_json_array, training_example_json_with_analysis,
};
use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::flow::{ProcessView, ProgramFinding, ProgramFindingCause, ProgramOperation};
use gewyvern::http::{
    HttpComponentKind, HttpComponentRef, HttpTransactionId, HttpTransactionVerdict,
    HttpTransactionView,
};
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId, SockLineageFact,
    TcpStateFact,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::template::TemplateBinding;
use std::fs;
#[cfg(target_family = "unix")]
use std::os::unix::fs::PermissionsExt;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use std::time::Instant;
use std::time::{SystemTime, UNIX_EPOCH};

#[path = "tests/ambiguity.rs"]
mod ambiguity;
#[cfg(target_family = "unix")]
#[path = "tests/analysis.rs"]
mod analysis;
#[path = "tests/api_bench.rs"]
mod api_bench;
#[path = "tests/api_persistence.rs"]
mod api_persistence;
#[path = "tests/api_sidecar.rs"]
mod api_sidecar;
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
#[path = "tests/mail_delivery.rs"]
mod mail_delivery;
#[path = "tests/mail_flow.rs"]
mod mail_flow;
#[path = "tests/pop_kerberos.rs"]
mod pop_kerberos;
#[path = "tests/relay_and_mail.rs"]
mod relay_and_mail;
#[path = "tests/reports.rs"]
mod reports;
#[path = "tests/rtsp_http_tls.rs"]
mod rtsp_http_tls;
#[path = "tests/runtime_config.rs"]
mod runtime_config;
#[path = "tests/runtime_migration.rs"]
mod runtime_migration;
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
