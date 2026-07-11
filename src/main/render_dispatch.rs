use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::http::compose_http_transactions;
use gewyvern::protocol_profiles::resolve_built_in_dsl_path;

use crate::{
    Cli, export_has_operation, findings_json, findings_text, http_transactions_json,
    http_transactions_text, render_debug_session_outputs, render_debugger_console_outputs,
    render_report_outputs, render_scan_outputs, run_binding_demo, summary_json, summary_line,
};

pub(crate) fn render_cli_outputs(cli: &Cli, outputs: Vec<(String, ExportBundle)>) -> String {
    if cli.http_transactions {
        return render_http_transaction_outputs(cli, &outputs);
    }
    render_standard_cli_outputs(cli, outputs)
}

fn render_http_transaction_outputs(cli: &Cli, outputs: &[(String, ExportBundle)]) -> String {
    let needs_http_response_companions = cli.dsl_path.is_some()
        && outputs
            .iter()
            .any(|(_, export)| export_has_operation(export, "http_request"));
    let needs_http3_response_companion = cli.dsl_path.is_some()
        && outputs
            .iter()
            .any(|(_, export)| export_has_operation(export, "http3_request"));
    let companion_count =
        (if needs_http_response_companions { 2 } else { 0 }) + usize::from(needs_http3_response_companion);
    let mut composed_exports = Vec::with_capacity(outputs.len() + companion_count);
    composed_exports.extend(outputs.iter().map(|(_, export)| export.clone()));

    if cli.dsl_path.is_some() {
        if needs_http_response_companions {
            let dns_path = resolve_built_in_dsl_path("dsl/dns_udp_process.gewy");
            composed_exports.push(run_binding_demo(
                compile_file(&dns_path).expect("dns dsl should compile"),
            ));
            let http_response_path =
                resolve_built_in_dsl_path("dsl/http_server_response_path.gewy");
            composed_exports.push(run_binding_demo(
                compile_file(&http_response_path).expect("http server dsl should compile"),
            ));
        }
        if needs_http3_response_companion {
            let http3_response_path =
                resolve_built_in_dsl_path("dsl/http3_server_response_path.gewy");
            composed_exports.push(run_binding_demo(
                compile_file(&http3_response_path).expect("http3 server dsl should compile"),
            ));
        }
    }

    let transactions = compose_http_transactions(&composed_exports);

    if cli.json {
        http_transactions_json(&transactions)
    } else {
        http_transactions_text(&transactions)
    }
}

fn render_standard_cli_outputs(cli: &Cli, outputs: Vec<(String, ExportBundle)>) -> String {
    if cli.debugger_console {
        return render_debugger_console_outputs(cli, &outputs);
    }
    if cli.debug_session {
        return render_debug_session_outputs(cli, &outputs);
    }
    if cli.findings {
        return render_findings_outputs(cli, outputs);
    }
    render_summary_or_export_outputs(cli, outputs)
}

fn render_findings_outputs(cli: &Cli, outputs: Vec<(String, ExportBundle)>) -> String {
    if cli.scan_all {
        return render_scan_outputs(cli, &outputs);
    }
    if cli.report_format.is_some() {
        return render_report_outputs(cli, &outputs);
    }
    outputs
        .into_iter()
        .map(|(name, export)| {
            if cli.json {
                findings_json(&name, &export)
            } else {
                findings_text(&name, &export)
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_summary_or_export_outputs(cli: &Cli, outputs: Vec<(String, ExportBundle)>) -> String {
    if cli.json {
        if cli.scan_all && cli.summary_only {
            return render_scan_outputs(cli, &outputs);
        }
        if cli.report_format.is_some() {
            return render_report_outputs(cli, &outputs);
        }
        return outputs
            .into_iter()
            .map(|(name, export)| {
                if cli.summary_only {
                    summary_json(&name, &export)
                } else {
                    export.to_json()
                }
            })
            .collect::<Vec<_>>()
            .join("\n");
    }
    if cli.scan_all {
        return render_scan_outputs(cli, &outputs);
    }
    if cli.report_format.is_some() {
        return render_report_outputs(cli, &outputs);
    }
    outputs
        .into_iter()
        .map(|(name, export)| summary_line(&name, &export))
        .collect::<Vec<_>>()
        .join("\n")
}
