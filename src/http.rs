use crate::export::ExportBundle;
use crate::flow::{ModuleSeverity, ProcessView, ProgramOperation};
use std::time::SystemTime;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct HttpTransactionId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HttpComponentKind {
    DnsLookup,
    ClientRequest,
    ServerResponse,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpComponentRef {
    pub template_id: String,
    pub kind: HttpComponentKind,
    pub operation: ProgramOperation,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HttpTransactionView {
    pub id: HttpTransactionId,
    pub client_process: Option<ProcessView>,
    pub server_process: Option<ProcessView>,
    pub components: Vec<HttpComponentRef>,
    pub phases: Vec<String>,
    pub phase_kinds: Vec<String>,
    pub verdict: HttpTransactionVerdict,
    pub severity: Option<ModuleSeverity>,
    pub degraded: bool,
    pub suspect_sides: Vec<HttpSuspectSide>,
    pub finding_summaries: Vec<String>,
    pub summaries: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub enum HttpSuspectSide {
    Dns,
    Client,
    Server,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HttpTransactionVerdict {
    HealthyRequestResponsePath,
    SuspectDnsResolutionGap,
    SuspectClientResponseGap,
    SuspectServerResponseGap,
    SuspectMultiSidedGap,
}

#[derive(Clone, Debug)]
struct BundleView<'a> {
    bundle: &'a ExportBundle,
    operation: ProgramOperation,
    process: Option<ProcessView>,
    kind: HttpComponentKind,
    start: SystemTime,
    end: SystemTime,
}

pub fn compose_http_transactions(exports: &[ExportBundle]) -> Vec<HttpTransactionView> {
    let mut views = exports
        .iter()
        .filter_map(bundle_view)
        .collect::<Vec<_>>();
    views.sort_by_key(|view| (view.start, view.end));

    views.into_iter()
        .filter(|view| view.kind == HttpComponentKind::ClientRequest)
        .enumerate()
        .map(|(idx, request)| {
            let mut components = vec![HttpComponentRef {
                template_id: request.bundle.template_id.clone(),
                kind: request.kind.clone(),
                operation: request.operation.clone(),
            }];
            let mut phases = request
                .bundle
                .program_flows
                .iter()
                .flat_map(|flow| flow.stages.iter().filter_map(|stage| stage.phase.clone()))
                .collect::<Vec<_>>();
            let mut phase_kinds = request
                .bundle
                .program_flows
                .iter()
                .flat_map(|flow| flow.stages.iter().filter_map(|stage| stage.phase_kind.clone()))
                .collect::<Vec<_>>();
            phases.sort();
            phases.dedup();
            phase_kinds.sort();
            phase_kinds.dedup();
            let mut summaries = request
                .bundle
                .module_findings
                .iter()
                .flat_map(|finding| finding.summaries.clone())
                .collect::<Vec<_>>();
            if summaries.is_empty() {
                summaries.push(format!(
                    "http client transaction for {}",
                    describe_process(request.process.as_ref())
                ));
            }

            let mut degraded = request.bundle.debug_summary.degraded;
            let mut severity = request
                .bundle
                .module_findings
                .iter()
                .map(|finding| finding.severity.clone())
                .max();
            let mut suspect_sides = if request.bundle.module_findings.is_empty() {
                Vec::new()
            } else {
                vec![HttpSuspectSide::Client]
            };
            let mut finding_summaries = request
                .bundle
                .module_findings
                .iter()
                .flat_map(|finding| finding.summaries.clone())
                .collect::<Vec<_>>();

            if let Some(dns) = find_related_dns(&views_for_dns(exports), &request) {
                components.push(HttpComponentRef {
                    template_id: dns.bundle.template_id.clone(),
                    kind: dns.kind.clone(),
                    operation: dns.operation.clone(),
                });
                extend_unique(
                    &mut phases,
                    dns.bundle
                        .program_flows
                        .iter()
                        .flat_map(|flow| flow.stages.iter().filter_map(|stage| stage.phase.clone())),
                );
                extend_unique(
                    &mut phase_kinds,
                    dns.bundle.program_flows.iter().flat_map(|flow| {
                        flow.stages.iter().filter_map(|stage| stage.phase_kind.clone())
                    }),
                );
                degraded |= dns.bundle.debug_summary.degraded;
                severity = max_severity(
                    severity,
                    dns.bundle
                        .module_findings
                        .iter()
                        .map(|finding| finding.severity.clone())
                        .max(),
                );
                if !dns.bundle.module_findings.is_empty() {
                    suspect_sides.push(HttpSuspectSide::Dns);
                    finding_summaries.extend(
                        dns.bundle
                            .module_findings
                            .iter()
                            .flat_map(|finding| finding.summaries.clone()),
                    );
                }
            }

            let server_views = views_for_server(exports);
            let server_process = if let Some(server) = find_related_server(&server_views, &request) {
                components.push(HttpComponentRef {
                    template_id: server.bundle.template_id.clone(),
                    kind: server.kind.clone(),
                    operation: server.operation.clone(),
                });
                extend_unique(
                    &mut phases,
                    server
                        .bundle
                        .program_flows
                        .iter()
                        .flat_map(|flow| flow.stages.iter().filter_map(|stage| stage.phase.clone())),
                );
                extend_unique(
                    &mut phase_kinds,
                    server.bundle.program_flows.iter().flat_map(|flow| {
                        flow.stages.iter().filter_map(|stage| stage.phase_kind.clone())
                    }),
                );
                degraded |= server.bundle.debug_summary.degraded;
                severity = max_severity(
                    severity,
                    server
                        .bundle
                        .module_findings
                        .iter()
                        .map(|finding| finding.severity.clone())
                        .max(),
                );
                if !server.bundle.module_findings.is_empty() {
                    suspect_sides.push(HttpSuspectSide::Server);
                    finding_summaries.extend(
                        server
                            .bundle
                            .module_findings
                            .iter()
                            .flat_map(|finding| finding.summaries.clone()),
                    );
                }
                server.process.clone()
            } else {
                None
            };
            suspect_sides.sort();
            suspect_sides.dedup();
            finding_summaries.sort();
            finding_summaries.dedup();
            let verdict = verdict_for_transaction(&suspect_sides);

            HttpTransactionView {
                id: HttpTransactionId((idx + 1) as u64),
                client_process: request.process.clone(),
                server_process,
                components,
                phases,
                phase_kinds,
                verdict,
                severity,
                degraded,
                suspect_sides,
                finding_summaries,
                summaries,
            }
        })
        .collect()
}

fn views_for_dns(exports: &[ExportBundle]) -> Vec<BundleView<'_>> {
    exports
        .iter()
        .filter_map(bundle_view)
        .filter(|view| view.kind == HttpComponentKind::DnsLookup)
        .collect()
}

fn views_for_server(exports: &[ExportBundle]) -> Vec<BundleView<'_>> {
    exports
        .iter()
        .filter_map(bundle_view)
        .filter(|view| view.kind == HttpComponentKind::ServerResponse)
        .collect()
}

fn find_related_dns<'a>(dns_views: &'a [BundleView<'a>], request: &BundleView<'a>) -> Option<&'a BundleView<'a>> {
    dns_views.iter().find(|dns| {
        same_process(dns.process.as_ref(), request.process.as_ref()) && near_precedes(dns, request)
    })
}

fn find_related_server<'a>(
    server_views: &'a [BundleView<'a>],
    request: &BundleView<'a>,
) -> Option<&'a BundleView<'a>> {
    server_views.iter().find(|server| near_precedes(request, server) || overlaps(server, request))
}

fn overlaps(lhs: &BundleView<'_>, rhs: &BundleView<'_>) -> bool {
    lhs.start <= rhs.end && rhs.start <= lhs.end
}

fn near_precedes(lhs: &BundleView<'_>, rhs: &BundleView<'_>) -> bool {
    if lhs.end > rhs.start {
        return false;
    }
    rhs.start
        .duration_since(lhs.end)
        .map(|delta| delta.as_secs() <= 5)
        .unwrap_or(false)
}

fn same_process(lhs: Option<&ProcessView>, rhs: Option<&ProcessView>) -> bool {
    matches!((lhs, rhs), (Some(lhs), Some(rhs)) if lhs.pid == rhs.pid && lhs.comm == rhs.comm)
}

fn bundle_view(bundle: &ExportBundle) -> Option<BundleView<'_>> {
    let flow = bundle.program_flows.first()?;
    let kind = match &flow.operation {
        ProgramOperation::Custom(value) if value == "dns_lookup" => HttpComponentKind::DnsLookup,
        ProgramOperation::Custom(value) if value == "http_request" => HttpComponentKind::ClientRequest,
        ProgramOperation::Custom(value) if value == "http_server_response" => {
            HttpComponentKind::ServerResponse
        }
        _ => return None,
    };
    let start = bundle.facts.iter().map(|fact| fact.ts).min()?;
    let end = bundle.facts.iter().map(|fact| fact.ts).max()?;
    Some(BundleView {
        bundle,
        operation: flow.operation.clone(),
        process: flow.process.clone(),
        kind,
        start,
        end,
    })
}

fn extend_unique<I>(values: &mut Vec<String>, incoming: I)
where
    I: IntoIterator<Item = String>,
{
    for value in incoming {
        if !values.contains(&value) {
            values.push(value);
        }
    }
}

fn max_severity(
    lhs: Option<ModuleSeverity>,
    rhs: Option<ModuleSeverity>,
) -> Option<ModuleSeverity> {
    match (lhs, rhs) {
        (Some(lhs), Some(rhs)) => Some(lhs.max(rhs)),
        (Some(lhs), None) => Some(lhs),
        (None, Some(rhs)) => Some(rhs),
        (None, None) => None,
    }
}

fn describe_process(process: Option<&ProcessView>) -> String {
    process
        .map(|process| format!("{} (pid={})", process.comm, process.pid))
        .unwrap_or_else(|| "unknown process".into())
}

fn verdict_for_transaction(suspect_sides: &[HttpSuspectSide]) -> HttpTransactionVerdict {
    match suspect_sides {
        [] => HttpTransactionVerdict::HealthyRequestResponsePath,
        [HttpSuspectSide::Dns] => HttpTransactionVerdict::SuspectDnsResolutionGap,
        [HttpSuspectSide::Client] => HttpTransactionVerdict::SuspectClientResponseGap,
        [HttpSuspectSide::Server] => HttpTransactionVerdict::SuspectServerResponseGap,
        _ => HttpTransactionVerdict::SuspectMultiSidedGap,
    }
}
