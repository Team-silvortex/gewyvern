use gewyvern::export::ExportBundle;
use gewyvern::flow::{FlowId, ProcessView, ProgramFlowId};
use gewyvern::ledger::{CpuId, FactEnvelope, FactId, FactKind, RouteDecisionFact, SessionId};
use gewyvern::protocol_profiles::{
    default_protocol_scan_set, default_protocol_scan_set_from_dir, protocol_summaries,
    protocol_summary, resolve_protocol_profile,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::template::TemplateBinding;
use std::collections::HashSet;
use std::fs;
use std::net::ToSocketAddrs;
use std::path::Path;
use std::time::SystemTime;

use crate::runtime_events::EVENT_WRITE_FAILED;
use crate::runtime_logging::log_error_event;
use crate::{Cli, IngestMode, ScanTarget, SocketTarget, UiLocale};

pub(crate) fn process_matches_pid(process: Option<&ProcessView>, pid: u32) -> bool {
    process.is_some_and(|process| process.pid == pid)
}

pub(crate) fn ingest_trust_mode_for_cli(cli: &Cli) -> &'static str {
    match cli.socket_target {
        Some(_) => match cli.ingest_mode {
            IngestMode::LocalAdvisory => "unverified-local",
            IngestMode::RemoteAdvisory => "unverified-remote",
        },
        None => "synthetic-demo",
    }
}

pub(crate) fn annotate_export_trust(mut export: ExportBundle, cli: &Cli) -> ExportBundle {
    export.ingest_trust_mode = ingest_trust_mode_for_cli(cli).to_string();
    export
}

pub(crate) fn ingest_mode_for_export(export: &ExportBundle) -> &'static str {
    match export.ingest_trust_mode.as_str() {
        "synthetic-demo" => "demo",
        "unverified-local" => "local-advisory",
        "unverified-remote" => "remote-advisory",
        _ => "unknown",
    }
}

pub(crate) fn ingest_mode_note_for_export(export: &ExportBundle) -> &'static str {
    match ingest_mode_for_export(export) {
        "demo" => {
            "synthetic demo mode: useful for exercising flows and reports, not for real process attribution"
        }
        "local-advisory" => {
            "local advisory mode: facts come from a local socket source, but lineage is still unverified"
        }
        "remote-advisory" => {
            "remote advisory mode: facts come from an explicitly enabled remote socket source and should be treated as unverified"
        }
        _ => "ingest mode could not be classified; treat process-level conclusions conservatively",
    }
}

pub(crate) fn pid_attribution_status_for_export(export: &ExportBundle) -> &'static str {
    match export.ingest_trust_mode.as_str() {
        "synthetic-demo" => "synthetic",
        "unverified-local" | "unverified-remote" => "unverified",
        _ => "unknown",
    }
}

pub(crate) fn pid_attribution_note_for_export(export: &ExportBundle) -> &'static str {
    match export.ingest_trust_mode.as_str() {
        "synthetic-demo" => "pid-scoped conclusions come from synthetic demo lineage",
        "unverified-local" | "unverified-remote" => {
            "pid-scoped conclusions are advisory only because ingest lineage is unverified"
        }
        _ => "pid attribution status is unknown",
    }
}

pub(crate) fn export_has_operation(export: &ExportBundle, operation: &str) -> bool {
    export.program_flows.iter().any(|flow| {
        matches!(
            &flow.operation,
            gewyvern::flow::ProgramOperation::Custom(value) if value == operation
        )
    })
}

pub(crate) fn socket_target_is_local(target: &SocketTarget) -> bool {
    match target {
        SocketTarget::Unix(_) => true,
        SocketTarget::Tcp(addr) => tcp_bind_addr_is_local(addr),
    }
}

pub(crate) fn tcp_bind_addr_is_local(addr: &str) -> bool {
    addr.to_socket_addrs()
        .map(|resolved| resolved.into_iter().all(|addr| addr.ip().is_loopback()))
        .unwrap_or_else(|_| addr.starts_with("localhost:"))
}

pub(crate) fn api_socket_addr_is_local(addr: &str) -> bool {
    tcp_bind_addr_is_local(addr)
}

pub(crate) fn filter_export_by_pid(export: &ExportBundle, pid: u32) -> ExportBundle {
    let sessions = export
        .facts
        .iter()
        .filter_map(|fact| match &fact.kind {
            FactKind::SockLineage(lineage) if lineage.pid == pid => Some(fact.session),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let cookies = export
        .facts
        .iter()
        .filter_map(|fact| match &fact.kind {
            FactKind::SockLineage(lineage) if lineage.pid == pid => Some(lineage.sk_cookie),
            _ => None,
        })
        .collect::<HashSet<_>>();
    let flow_ids = export
        .flows
        .iter()
        .filter(|flow| process_matches_pid(flow.process.as_ref(), pid))
        .map(|flow| flow.id)
        .collect::<HashSet<FlowId>>();
    let program_flow_ids = export
        .program_flows
        .iter()
        .filter(|flow| process_matches_pid(flow.process.as_ref(), pid))
        .map(|flow| flow.id)
        .collect::<HashSet<ProgramFlowId>>();
    let fact_matches = |fact: &FactEnvelope| {
        if sessions.contains(&fact.session) {
            return true;
        }
        match &fact.kind {
            FactKind::SockLineage(lineage) => lineage.pid == pid,
            FactKind::TcpState(state) => cookies.contains(&state.sk_cookie),
            FactKind::PacketMeta(packet) => packet
                .sk_cookie
                .is_some_and(|cookie| cookies.contains(&cookie)),
            FactKind::QuicMeta(quic) => quic
                .sk_cookie
                .is_some_and(|cookie| cookies.contains(&cookie)),
            FactKind::RouteDecision(route) => route
                .sk_cookie
                .is_some_and(|cookie| cookies.contains(&cookie)),
            FactKind::DropAction(_) | FactKind::AttachScope(_) => false,
        }
    };

    let mut filtered = export.clone();
    filtered.facts = export
        .facts
        .iter()
        .filter(|fact| fact_matches(fact))
        .cloned()
        .collect();
    let accepted_fact_ids = filtered
        .facts
        .iter()
        .map(|fact| fact.id)
        .collect::<HashSet<_>>();
    filtered.rejected_facts = export
        .rejected_facts
        .iter()
        .filter(|fact| accepted_fact_ids.contains(&fact.id))
        .cloned()
        .collect();
    filtered.rejected_fact_summary =
        gewyvern::runtime::summarize_rejected_facts(&filtered.rejected_facts);
    filtered.flows = export
        .flows
        .iter()
        .filter(|flow| process_matches_pid(flow.process.as_ref(), pid))
        .cloned()
        .collect();
    filtered.program_flows = export
        .program_flows
        .iter()
        .filter(|flow| process_matches_pid(flow.process.as_ref(), pid))
        .cloned()
        .collect();
    filtered.program_findings = export
        .program_findings
        .iter()
        .filter(|finding| {
            process_matches_pid(finding.process.as_ref(), pid)
                || program_flow_ids.contains(&finding.program_flow)
        })
        .cloned()
        .collect();
    filtered.module_findings = export
        .module_findings
        .iter()
        .filter(|finding| process_matches_pid(finding.process.as_ref(), pid))
        .cloned()
        .collect();
    filtered.reasons = export
        .reasons
        .iter()
        .filter(|reason| flow_ids.contains(&reason.flow))
        .cloned()
        .collect();
    filtered.debug_summary.accepted_facts = filtered.facts.len() as u64;
    filtered.debug_summary.rejected_facts = filtered.rejected_facts.len() as u64;
    filtered.debug_summary.flows = filtered.flows.len() as u64;
    filtered.debug_summary.program_flows = filtered.program_flows.len() as u64;
    filtered.debug_summary.program_findings = filtered.program_findings.len() as u64;
    filtered.debug_summary.module_findings = filtered.module_findings.len() as u64;
    filtered.debug_summary.reasons = filtered.reasons.len() as u64;
    filtered
}

pub(crate) fn run_session(
    template: gewyvern::template::Template,
    facts: Vec<FactEnvelope>,
) -> ExportBundle {
    let config = SessionConfig::for_template(template).expect("builtin template should be valid");
    let mut session = RuntimeSession::start(config).expect("session startup should succeed");
    let window_end = facts
        .iter()
        .map(|fact| fact.ts)
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH);

    for fact in facts {
        session.ingest(fact);
    }
    session.freeze(window_end);

    let export = session.export_bundle();
    let replay = ExportBundle::from_json(&export.to_json())
        .expect("runtime should export replayable json")
        .replay()
        .expect("export should replay");

    assert_eq!(
        export.reasons, replay.reasons,
        "replay should stay deterministic"
    );
    export
}

pub(crate) fn run_binding_session(
    binding: TemplateBinding,
    facts: &[FactEnvelope],
) -> ExportBundle {
    let config = SessionConfig::for_binding(binding).expect("dsl binding should be valid");
    let mut session = RuntimeSession::start(config).expect("dsl session startup should succeed");
    let window_end = facts
        .iter()
        .map(|fact| fact.ts)
        .max()
        .unwrap_or(SystemTime::UNIX_EPOCH);

    for fact in facts {
        session.ingest(fact.clone());
    }
    session.freeze(window_end);
    session.export_bundle()
}

pub(crate) fn route_fact(
    id: u64,
    ts: SystemTime,
    cookie: u64,
    oif: u32,
    session: SessionId,
) -> FactEnvelope {
    FactEnvelope {
        id: FactId(id),
        ts,
        cpu: CpuId(1),
        ifindex: Some(oif),
        session,
        fragment_id: "route_meta_fragment".into(),
        kind: FactKind::RouteDecision(RouteDecisionFact {
            netns: 1,
            sk_cookie: Some(cookie),
            fib_table: Some(254),
            oif,
            gw: None,
        }),
    }
}

pub(crate) fn write_or_print(rendered: &str, out_path: Option<&str>, locale: UiLocale) {
    if let Some(path) = out_path {
        fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
            log_error_event(
                "output",
                EVENT_WRITE_FAILED,
                &[("path", path.to_string()), ("error", err.to_string())],
                "failed to write rendered output",
            );
            eprintln!(
                "{}",
                locale.msgf("write_failed", path, Some(&err.to_string()))
            );
            std::process::exit(1);
        });
    } else {
        println!("{rendered}");
    }
}

pub(crate) fn list_protocols_text() -> String {
    protocol_summaries()
        .into_iter()
        .map(|summary| {
            let alias_suffix = if summary.aliases.is_empty() {
                String::new()
            } else {
                format!(" aliases: {}", summary.aliases.join(", "))
            };
            format!(
                "{} (default: {}){}",
                summary.protocol, summary.default_entry, alias_suffix
            )
        })
        .collect::<Vec<String>>()
        .join("\n")
}

pub(crate) fn list_protocols_json() -> String {
    let items = protocol_summaries()
        .into_iter()
        .map(|summary| {
            let entries = summary
                .entries
                .iter()
                .map(|entry| format!("\"{}\"", entry.mode))
                .collect::<Vec<_>>()
                .join(",");
            format!(
                "{{\"protocol\":\"{}\",\"default_entry\":\"{}\",\"aliases\":{},\"entries\":[{}]}}",
                summary.protocol,
                summary.default_entry,
                json_string_array(&summary.aliases),
                entries
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    format!("[{items}]")
}

pub(crate) fn list_entries_text(protocol: &str) -> Option<String> {
    let lines = protocol_summary(protocol)?
        .entries
        .into_iter()
        .map(|entry| {
            let label = if entry.default {
                format!("{} (default)", entry.mode)
            } else {
                entry.mode
            };
            if entry.aliases.is_empty() {
                label
            } else {
                format!("{label} aliases: {}", entry.aliases.join(", "))
            }
        })
        .collect::<Vec<_>>();
    Some(lines.join("\n"))
}

pub(crate) fn list_entries_json(protocol: &str) -> Option<String> {
    let summary = protocol_summary(protocol)?;
    let entries = summary
        .entries
        .into_iter()
        .map(|entry| {
            format!(
                "{{\"mode\":\"{}\",\"default\":{},\"aliases\":{}}}",
                entry.mode,
                if entry.default { "true" } else { "false" },
                json_string_array(&entry.aliases)
            )
        })
        .collect::<Vec<_>>()
        .join(",");
    Some(format!(
        "{{\"protocol\":\"{}\",\"default_entry\":\"{}\",\"aliases\":{},\"entries\":[{entries}]}}",
        summary.protocol,
        summary.default_entry,
        json_string_array(&summary.aliases),
    ))
}

fn json_string_array(items: &[String]) -> String {
    let joined = items
        .iter()
        .map(|item| format!("\"{item}\""))
        .collect::<Vec<_>>()
        .join(",");
    format!("[{joined}]")
}

pub(crate) fn scan_targets_for_cli(cli: &Cli) -> Result<Vec<ScanTarget>, String> {
    if !cli.scan_all {
        return Ok(Vec::new());
    }
    match cli.protocol_set_path.as_deref() {
        Some(path) => scan_targets_from_set_file(path),
        None => Ok(default_protocol_scan_set()
            .into_iter()
            .map(ScanTarget::from_resolved)
            .collect()),
    }
}

pub(crate) fn scan_targets_from_set_file(path: &str) -> Result<Vec<ScanTarget>, String> {
    if Path::new(path).is_dir() {
        return default_protocol_scan_set_from_dir(path)
            .map(|targets| targets.into_iter().map(ScanTarget::from_resolved).collect())
            .ok_or_else(|| {
                format!(
                    "protocol registry directory '{}' did not resolve any scan targets",
                    path
                )
            });
    }
    let contents = fs::read_to_string(path)
        .map_err(|err| format!("failed to read protocol set '{path}': {err}"))?;
    let mut targets = Vec::new();
    let mut seen = HashSet::new();

    for (index, raw_line) in contents.lines().enumerate() {
        let line = raw_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let (protocol, entry) = parse_protocol_set_line(line)
            .map_err(|err| format!("invalid protocol set line {}: {err}", index + 1))?;
        let resolved = resolve_protocol_profile(protocol, entry).ok_or_else(|| {
            format!(
                "unsupported protocol target on line {}: {}",
                index + 1,
                line
            )
        })?;
        let key = format!("{}:{}", resolved.protocol, resolved.entry);
        if seen.insert(key) {
            targets.push(ScanTarget::from_resolved(resolved));
        }
    }

    if targets.is_empty() {
        return Err(format!(
            "protocol set '{}' did not resolve any scan targets",
            path
        ));
    }

    Ok(targets)
}

pub(crate) fn parse_protocol_set_line(line: &str) -> Result<(&str, Option<&str>), String> {
    if let Some((protocol, entry)) = line.split_once(':') {
        let protocol = protocol.trim();
        let entry = entry.trim();
        if protocol.is_empty() || entry.is_empty() {
            return Err(format!("expected '<protocol>:<entry>', got '{line}'"));
        }
        return Ok((protocol, Some(entry)));
    }

    let mut parts = line.split_whitespace();
    let protocol = parts
        .next()
        .ok_or_else(|| format!("expected '<protocol>' or '<protocol> <entry>', got '{line}'"))?;
    let entry = parts.next();
    if parts.next().is_some() {
        return Err(format!(
            "expected '<protocol>' or '<protocol> <entry>', got '{line}'"
        ));
    }
    Ok((protocol, entry))
}
