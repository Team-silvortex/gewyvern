#[path = "main/binding_demo.rs"]
mod binding_demo;
#[path = "main/certificate_state_cli.rs"]
mod certificate_state_cli;
#[path = "main/cli.rs"]
mod cli;
#[path = "main/cli_validation.rs"]
mod cli_validation;
mod data_api;
mod diagnosis_runtime;
#[path = "main/diagnostics_mode.rs"]
mod diagnostics_mode;
mod external_analysis;
mod gewyvern_install;
#[path = "main/helpers.rs"]
mod helpers;
#[path = "main/history_catalog_delta.rs"]
mod history_catalog_delta;
#[path = "main/history_view.rs"]
mod history_view;
#[path = "main/output_collection.rs"]
mod output_collection;
#[path = "main/preflight.rs"]
mod preflight;
#[path = "main/render_dispatch.rs"]
mod render_dispatch;
mod render_utils;
mod report_runtime;
#[path = "main/runtime_config.rs"]
mod runtime_config;
#[path = "main/runtime_events.rs"]
mod runtime_events;
#[path = "main/runtime_logging.rs"]
mod runtime_logging;
#[path = "main/runtime_migration.rs"]
mod runtime_migration;
mod serve_runtime;
#[path = "main/socket_resilience.rs"]
mod socket_resilience;
#[path = "main/startup.rs"]
mod startup;
#[cfg(test)]
#[path = "main/tests.rs"]
mod tests;
#[path = "main/ui_locale.rs"]
mod ui_locale;

#[cfg(test)]
use gewyvern::protocol_profiles::protocol_dsl_path;
use gewyvern::socket_input::{
    bind_unix_socket_listener, collect_tcp_socket_facts_on_listener,
    collect_unix_socket_facts_on_listener, remove_unix_socket_file,
    run_tcp_socket_session_on_listener, run_tcp_socket_session_on_listener_with_binding,
    run_unix_socket_session_on_listener, run_unix_socket_session_on_listener_with_binding,
};
use std::env;
use std::fs;
use std::time::{Duration, SystemTime};

use crate::diagnosis_runtime::*;
use crate::report_runtime::{
    findings_json, findings_json_with_analysis, findings_text, http_transactions_json,
    http_transactions_text, render_debug_session_outputs, render_debugger_console_outputs,
    render_report_outputs, render_scan_outputs, scan_analysis_json_array,
    scan_report_html_with_analyses, scan_report_json_with_analyses, scan_report_text_with_analyses,
    single_target_report_html_with_analysis, single_target_report_json_with_analysis, summary_json,
    summary_json_with_analysis, summary_line, summary_line_with_analysis,
    training_example_json_array, training_example_json_with_analysis,
};
#[cfg(test)]
use crate::report_runtime::{
    scan_report_html, scan_report_json, scan_report_text, training_example_json,
};
use crate::runtime_events::EVENT_SCAN_TARGET_RESOLVE_FAILED;
use crate::runtime_logging::log_error_event;
use crate::startup::bootstrap_cli;

pub(crate) use self::binding_demo::run_binding_demo;
pub(crate) use self::diagnostics_mode::render_diagnostics_mode;
pub(crate) use self::output_collection::collect_cli_outputs;
pub(crate) use self::preflight::handle_cli_preflight;
pub(crate) use self::render_dispatch::render_cli_outputs;
#[cfg(test)]
pub(crate) use self::report_runtime::collect_analyses;
pub(crate) use self::ui_locale::UiLocale;

fn main() {
    let locale = UiLocale::detect();
    let args = env::args().skip(1).collect::<Vec<_>>();
    if args.first().map(String::as_str) == Some("gewyvern-install-v1") {
        if args.len() != 1 {
            eprintln!("gewyvern: gewyvern-install-v1 accepts no command-line arguments");
            std::process::exit(1);
        }
        if let Err(error) = gewyvern_install::run_gewyvern_install_stdio() {
            eprintln!("gewyvern: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.first().map(String::as_str) == Some("gewyvern-activate-v1") {
        if args.len() != 1 {
            eprintln!("gewyvern: gewyvern-activate-v1 accepts no command-line arguments");
            std::process::exit(1);
        }
        if let Err(error) = gewyvern_install::run_gewyvern_activate_stdio() {
            eprintln!("gewyvern: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.first().map(String::as_str) == Some("gewyvern-retire-v1") {
        if args.len() != 1 {
            eprintln!("gewyvern: gewyvern-retire-v1 accepts no command-line arguments");
            std::process::exit(1);
        }
        if let Err(error) = gewyvern_install::run_gewyvern_retire_stdio() {
            eprintln!("gewyvern: {error}");
            std::process::exit(1);
        }
        return;
    }
    if args.first().map(String::as_str) == Some("gewyvern-service-v1") {
        if args.len() != 1 {
            eprintln!("gewyvern: gewyvern-service-v1 accepts no command-line arguments");
            std::process::exit(1);
        }
        if let Err(error) = gewyvern_install::run_gewyvern_service() {
            eprintln!("gewyvern: {error}");
            std::process::exit(1);
        }
        return;
    }
    if matches!(args.as_slice(), [flag] if flag == "--version" || flag == "-V") {
        println!("gewyvern {}", env!("CARGO_PKG_VERSION"));
        return;
    }
    let cli = bootstrap_cli(args);
    if handle_cli_preflight(&cli, locale) {
        return;
    }
    let rendered = run_cli_main(&cli, locale);
    write_or_print(&rendered, cli.out_path.as_deref(), locale);
}

fn run_cli_main(cli: &Cli, locale: UiLocale) -> String {
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_000_000);
    let scan_targets = scan_targets_for_cli(cli).unwrap_or_else(|err| {
        log_error_event(
            "runtime",
            EVENT_SCAN_TARGET_RESOLVE_FAILED,
            &[("error", err.clone())],
            "failed to resolve scan targets",
        );
        eprintln!("{err}");
        std::process::exit(2);
    });

    if cli.diagnostics {
        return render_diagnostics_mode(cli, locale);
    }

    let outputs = collect_cli_outputs(cli, base, &scan_targets, locale);
    render_cli_outputs(cli, outputs)
}

pub(crate) use self::cli::{Cli, IngestMode, ReportFormat, ScanTarget, SocketTarget};

pub(crate) use self::helpers::{
    annotate_export_trust, api_socket_addr_is_local, export_has_operation, filter_export_by_pid,
    ingest_mode_for_export, ingest_mode_note_for_export, list_entries_json, list_entries_text,
    list_protocols_json, list_protocols_text, pid_attribution_note_for_export,
    pid_attribution_status_for_export, route_fact, run_binding_session, run_session,
    scan_targets_for_cli, selected_scan_target_for_cli, socket_target_is_local, write_or_print,
};
fn usage() -> &'static str {
    UiLocale::detect().usage()
}
