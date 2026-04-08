use gewyvern::dsl::compile_file;
use gewyvern::export::ExportBundle;
use gewyvern::fragment::{builtin_registry, BindingDiagnostics};
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, RouteDecisionFact,
    SessionId, SockLineageFact, TcpStateFact,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::socket_input::{
    bind_unix_socket_listener, run_tcp_socket_session, run_tcp_socket_session_on_listener,
    run_tcp_socket_session_on_listener_with_binding, run_tcp_socket_session_with_binding,
    run_unix_socket_session, run_unix_socket_session_on_listener,
    run_unix_socket_session_on_listener_with_binding, run_unix_socket_session_with_binding,
};
use gewyvern::template::{handshake_debug_template, udp_debug_template, TemplateBinding};
use std::env;
use std::fs;
use std::net::TcpListener;
use std::time::{Duration, SystemTime};

fn main() {
    let cli = Cli::from_args(env::args().skip(1)).unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2);
    });
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_000_000);
    let mut outputs = Vec::new();

    if cli.diagnostics {
        let binding = cli.dsl_binding().unwrap_or_else(|| {
            eprintln!("--diagnostics requires --dsl");
            std::process::exit(2);
        });
        let diagnostics = builtin_registry()
            .binding_diagnostics(&binding)
            .unwrap_or_else(|err| {
                eprintln!("binding diagnostics failed: {err:?}");
                std::process::exit(2);
            });
        let rendered = if cli.json {
            diagnostics_json(&binding, &diagnostics)
        } else {
            diagnostics_text(&binding, &diagnostics)
        };
        if let Some(path) = cli.out_path.as_deref() {
            fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
                eprintln!("failed to write output to {path}: {err}");
                std::process::exit(1);
            });
        } else {
            println!("{rendered}");
        }
        return;
    }

    if let Some(socket_target) = cli.socket_target.as_ref() {
        if cli.serve {
            serve_socket_sessions(&cli, socket_target);
            return;
        }

        let export = match (socket_target, cli.dsl_binding()) {
            (SocketTarget::Unix(path), Some(binding)) => run_unix_socket_session_with_binding(path, binding),
            (SocketTarget::Tcp(addr), Some(binding)) => run_tcp_socket_session_with_binding(addr, binding),
            (SocketTarget::Unix(path), None) => run_unix_socket_session(path, cli.template_mode.template()),
            (SocketTarget::Tcp(addr), None) => run_tcp_socket_session(addr, cli.template_mode.template()),
        }
        .unwrap_or_else(|err| {
            eprintln!("socket session failed: {err:?}");
            std::process::exit(1);
        });
        outputs.push(("socket_session", export));
    } else {
        if let Some(binding) = cli.dsl_binding() {
            outputs.push(("dsl_demo", run_binding_demo(binding)));
        } else {
            if cli.demo_mode.includes_tcp() {
                let tcp_export = run_session(
                    handshake_debug_template(),
                    vec![
                        FactEnvelope {
                            id: FactId(1),
                            ts: base,
                            cpu: CpuId(0),
                            ifindex: Some(2),
                            session: SessionId(1),
                            fragment_id: "tcp_state_fragment".into(),
                            kind: FactKind::TcpState(TcpStateFact {
                                netns: 1,
                                sk_cookie: 42,
                                saddr: [0; 16],
                                daddr: [0; 16],
                                sport: 42310,
                                dport: 443,
                                family: 2,
                                old: 1,
                                new: 2,
                            }),
                        },
                        FactEnvelope {
                            id: FactId(2),
                            ts: base + Duration::from_millis(10),
                            cpu: CpuId(0),
                            ifindex: Some(2),
                            session: SessionId(1),
                            fragment_id: "tcp_packet_meta_fragment".into(),
                            kind: FactKind::PacketMeta(PacketMetaFact {
                                netns: 1,
                                sk_cookie: Some(42),
                                dir: PacketDir::Egress,
                                l3_proto: 0x0800,
                                l4_proto: 6,
                                tot_len: 60,
                                tcp_flags: 0x02,
                                seq: Some(1),
                                ack: None,
                                window: Some(65535),
                            }),
                        },
                        route_fact(3, base + Duration::from_millis(20), 42, 2, SessionId(1)),
                    ],
                );

                outputs.push(("tcp_demo", tcp_export));
            }

            if cli.demo_mode.includes_udp() {
                let udp_export = run_session(
                    udp_debug_template(),
                    vec![
                        FactEnvelope {
                            id: FactId(1),
                            ts: base,
                            cpu: CpuId(0),
                            ifindex: Some(3),
                            session: SessionId(2),
                            fragment_id: "udp_packet_meta_fragment".into(),
                            kind: FactKind::PacketMeta(PacketMetaFact {
                                netns: 1,
                                sk_cookie: Some(99),
                                dir: PacketDir::Egress,
                                l3_proto: 0x0800,
                                l4_proto: 17,
                                tot_len: 72,
                                tcp_flags: 0,
                                seq: None,
                                ack: None,
                                window: None,
                            }),
                        },
                        route_fact(2, base + Duration::from_millis(10), 99, 3, SessionId(2)),
                    ],
                );

                outputs.push(("udp_demo", udp_export));
            }
        }
    }

    let rendered = if cli.json {
        outputs
            .into_iter()
            .map(|(name, export)| {
                if cli.summary_only {
                    summary_json(name, &export)
                } else {
                    export.to_json()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        outputs
            .into_iter()
            .map(|(name, export)| summary_line(name, &export))
            .collect::<Vec<_>>()
            .join("\n")
    };

    if let Some(path) = cli.out_path.as_deref() {
        fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
            eprintln!("failed to write output to {path}: {err}");
            std::process::exit(1);
        });
    } else {
        println!("{rendered}");
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Cli {
    demo_mode: DemoMode,
    template_mode: TemplateMode,
    dsl_path: Option<String>,
    diagnostics: bool,
    serve: bool,
    max_sessions: Option<usize>,
    json: bool,
    summary_only: bool,
    out_path: Option<String>,
    socket_target: Option<SocketTarget>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DemoMode {
    Tcp,
    Udp,
    Both,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum TemplateMode {
    Tcp,
    Udp,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum SocketTarget {
    Unix(String),
    Tcp(String),
}

impl DemoMode {
    fn from_str(value: &str) -> Result<Self, String>
    {
        match value {
            "tcp" => Ok(Self::Tcp),
            "udp" => Ok(Self::Udp),
            "both" => Ok(Self::Both),
            other => Err(format!(
                "unsupported demo mode '{other}', expected tcp, udp, or both"
            )),
        }
    }

    fn includes_tcp(self) -> bool {
        matches!(self, Self::Tcp | Self::Both)
    }

    fn includes_udp(self) -> bool {
        matches!(self, Self::Udp | Self::Both)
    }
}

impl TemplateMode {
    fn from_str(value: &str) -> Result<Self, String> {
        match value {
            "tcp" => Ok(Self::Tcp),
            "udp" => Ok(Self::Udp),
            other => Err(format!(
                "unsupported template '{other}', expected tcp or udp"
            )),
        }
    }

    fn template(self) -> gewyvern::template::Template {
        match self {
            Self::Tcp => handshake_debug_template(),
            Self::Udp => udp_debug_template(),
        }
    }
}

impl Cli {
    fn dsl_binding(&self) -> Option<TemplateBinding> {
        self.dsl_path
            .as_deref()
            .map(|path| compile_file(path).unwrap_or_else(|err| {
                eprintln!("dsl compile failed: {err:?}");
                std::process::exit(2);
            }))
    }

    fn from_args<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut demo_mode = DemoMode::Both;
        let mut template_mode = TemplateMode::Tcp;
        let mut dsl_path = None;
        let mut diagnostics = false;
        let mut serve = false;
        let mut max_sessions = None;
        let mut json = false;
        let mut summary_only = false;
        let mut out_path = None;
        let mut socket_target = None;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--demo" => {
                    let value = args.next().ok_or_else(|| {
                        "missing value for --demo, expected tcp, udp, or both".to_string()
                    })?;
                    demo_mode = DemoMode::from_str(&value)?;
                }
                "--json" => json = true,
                "--serve" => serve = true,
                "--max-sessions" => {
                    let value = args.next().ok_or_else(|| {
                        "missing value for --max-sessions, expected a positive integer".to_string()
                    })?;
                    max_sessions = Some(
                        value
                            .parse::<usize>()
                            .map_err(|_| "--max-sessions must be a positive integer".to_string())?,
                    );
                }
                "--summary-only" => summary_only = true,
                "--template" => {
                    let value = args.next().ok_or_else(|| {
                        "missing value for --template, expected tcp or udp".to_string()
                    })?;
                    template_mode = TemplateMode::from_str(&value)?;
                }
                "--dsl" => {
                    dsl_path = Some(args.next().ok_or_else(|| {
                        "missing value for --dsl, expected a DSL file path".to_string()
                    })?);
                }
                "--diagnostics" => diagnostics = true,
                "--unix-socket" => {
                    socket_target = Some(SocketTarget::Unix(args.next().ok_or_else(|| {
                        "missing value for --unix-socket, expected a filesystem path".to_string()
                    })?));
                }
                "--tcp-socket" => {
                    socket_target = Some(SocketTarget::Tcp(args.next().ok_or_else(|| {
                        "missing value for --tcp-socket, expected host:port".to_string()
                    })?));
                }
                "--out" => {
                    out_path = Some(args.next().ok_or_else(|| {
                        "missing value for --out, expected a writable file path".to_string()
                    })?);
                }
                "--help" | "-h" => return Err(usage().into()),
                other => return Err(format!("unknown argument '{other}'\n{}", usage())),
            }
        }

        if summary_only && !json {
            return Err("--summary-only requires --json".into());
        }
        if diagnostics && dsl_path.is_none() {
            return Err("--diagnostics requires --dsl".into());
        }
        if diagnostics && socket_target.is_some() {
            return Err("--diagnostics cannot be combined with socket listener mode".into());
        }
        if diagnostics && serve {
            return Err("--diagnostics cannot be combined with --serve".into());
        }
        if dsl_path.is_some() && demo_mode != DemoMode::Both {
            return Err("--dsl cannot be combined with --demo".into());
        }
        if socket_target.is_some() && demo_mode != DemoMode::Both {
            return Err("--demo cannot be combined with socket listener mode".into());
        }
        if serve && socket_target.is_none() {
            return Err("--serve requires --unix-socket or --tcp-socket".into());
        }

        Ok(Self {
            demo_mode,
            template_mode,
            dsl_path,
            diagnostics,
            serve,
            max_sessions,
            json,
            summary_only,
            out_path,
            socket_target,
        })
    }
}

fn run_session(
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

    assert_eq!(export.reasons, replay.reasons, "replay should stay deterministic");
    export
}

fn run_binding_demo(binding: TemplateBinding) -> ExportBundle {
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_000_000);
    let fragments = &binding.template.fragment_set;
    let facts = if fragments.contains(&"tcp_state_fragment") && fragments.contains(&"tcp_packet_meta_fragment") {
        vec![
            FactEnvelope {
                id: FactId(1),
                ts: base,
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(1),
                fragment_id: "tcp_state_fragment".into(),
                kind: FactKind::TcpState(TcpStateFact {
                    netns: 1,
                    sk_cookie: 42,
                    saddr: [0; 16],
                    daddr: [0; 16],
                    sport: 42310,
                    dport: 443,
                    family: 2,
                    old: 1,
                    new: 2,
                }),
            },
            FactEnvelope {
                id: FactId(2),
                ts: base + Duration::from_millis(10),
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(1),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(42),
                    dir: PacketDir::Egress,
                    l3_proto: 0x0800,
                    l4_proto: 6,
                    tot_len: 60,
                    tcp_flags: 0x02,
                    seq: Some(1),
                    ack: None,
                    window: Some(65535),
                }),
            },
            route_fact(3, base + Duration::from_millis(20), 42, 2, SessionId(1)),
        ]
    } else if fragments.contains(&"udp_packet_meta_fragment") && fragments.contains(&"sock_lineage_fragment") {
        vec![
            FactEnvelope {
                id: FactId(1),
                ts: base,
                cpu: CpuId(0),
                ifindex: Some(2),
                session: SessionId(2),
                fragment_id: "sock_lineage_fragment".into(),
                kind: FactKind::SockLineage(SockLineageFact {
                    netns: 1,
                    sk_cookie: 99,
                    pid: 4242,
                    tid: 4242,
                    cgroup_id: 4242,
                    comm: {
                        let mut comm = [0u8; 16];
                        comm[..4].copy_from_slice(b"curl");
                        comm
                    },
                }),
            },
            FactEnvelope {
                id: FactId(2),
                ts: base + Duration::from_millis(10),
                cpu: CpuId(0),
                ifindex: Some(3),
                session: SessionId(2),
                fragment_id: "udp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(99),
                    dir: PacketDir::Egress,
                    l3_proto: 0x0800,
                    l4_proto: 17,
                    tot_len: 72,
                    tcp_flags: 0,
                    seq: None,
                    ack: None,
                    window: None,
                }),
            },
            route_fact(3, base + Duration::from_millis(20), 99, 3, SessionId(2)),
        ]
    } else if fragments.contains(&"udp_packet_meta_fragment") {
        vec![
            FactEnvelope {
                id: FactId(1),
                ts: base,
                cpu: CpuId(0),
                ifindex: Some(3),
                session: SessionId(2),
                fragment_id: "udp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 1,
                    sk_cookie: Some(99),
                    dir: PacketDir::Egress,
                    l3_proto: 0x0800,
                    l4_proto: 17,
                    tot_len: 72,
                    tcp_flags: 0,
                    seq: None,
                    ack: None,
                    window: None,
                }),
            },
            route_fact(2, base + Duration::from_millis(10), 99, 3, SessionId(2)),
        ]
    } else {
        eprintln!("dsl demo failed: unsupported fragment combination");
        std::process::exit(2);
    };

    let config = SessionConfig::for_binding(binding).expect("dsl binding should validate");
    let mut session = RuntimeSession::start(config).expect("dsl session startup should succeed");
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

    assert_eq!(export.reasons, replay.reasons, "replay should stay deterministic");
    export
}

fn route_fact(id: u64, ts: SystemTime, cookie: u64, oif: u32, session: SessionId) -> FactEnvelope {
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

fn summary_line(name: &str, export: &ExportBundle) -> String {
    format!(
        "{name}: template={} fragments_loaded={} hookpoints_failed={} accepted_facts={} rejected_facts={} flows={} reasons={} degraded={}",
        export.template_id,
        export.debug_summary.fragments_loaded,
        export.debug_summary.hookpoints_failed,
        export.debug_summary.accepted_facts,
        export.debug_summary.rejected_facts,
        export.debug_summary.flows,
        export.debug_summary.reasons,
        export.debug_summary.degraded
    )
}

fn usage() -> &'static str {
    "usage: gewyvern [--demo tcp|udp|both] [--dsl path] [--diagnostics] [--template tcp|udp] [--unix-socket path|--tcp-socket host:port] [--serve] [--max-sessions n] [--json] [--summary-only] [--out path]"
}

fn summary_json(name: &str, export: &ExportBundle) -> String {
    format!(
        "{{\"demo\":\"{name}\",\"template_id\":\"{}\",\"fragments_loaded\":{},\"hookpoints_failed\":{},\"accepted_facts\":{},\"rejected_facts\":{},\"flows\":{},\"reasons\":{},\"degraded\":{}}}",
        export.template_id,
        export.debug_summary.fragments_loaded,
        export.debug_summary.hookpoints_failed,
        export.debug_summary.accepted_facts,
        export.debug_summary.rejected_facts,
        export.debug_summary.flows,
        export.debug_summary.reasons,
        export.debug_summary.degraded
    )
}

fn diagnostics_text(binding: &TemplateBinding, diagnostics: &BindingDiagnostics) -> String {
    fn tier_label(tier: &gewyvern::fragment::RuleTier) -> &'static str {
        match tier {
            gewyvern::fragment::RuleTier::CoreRequirement => "core_requirement",
            gewyvern::fragment::RuleTier::OptionalEnhancement => "optional_enhancement",
            gewyvern::fragment::RuleTier::Unsupported => "unsupported",
        }
    }

    let mut lines = vec![format!(
        "template={} fragments={}",
        binding.template.id,
        binding.template.fragment_set.join(",")
    )];

    if let Some(model) = &diagnostics.program_model {
        lines.push(format!("program_model={}", model.model));
        for rule in &model.rules {
            lines.push(format!(
                "  program_rule[{}]: tier={} supported={} required={:?} supporting={:?} missing={:?}",
                rule.rule_index,
                tier_label(&rule.tier),
                rule.supported,
                rule.required_facts,
                rule.supporting_fragments,
                rule.missing_facts
            ));
        }
    }

    if let Some(model) = &diagnostics.reason_model {
        lines.push(format!("reason_model={}", model.model));
        for rule in &model.rules {
            lines.push(format!(
                "  reason_rule[{}]: tier={} supported={} required={:?} supporting={:?} missing={:?}",
                rule.rule_index,
                tier_label(&rule.tier),
                rule.supported,
                rule.required_facts,
                rule.supporting_fragments,
                rule.missing_facts
            ));
        }
    }

    lines.join("\n")
}

fn diagnostics_json(binding: &TemplateBinding, diagnostics: &BindingDiagnostics) -> String {
    fn fact_list(items: &[gewyvern::ledger::FactKindTag]) -> String {
        format!(
            "[{}]",
            items.iter()
                .map(|item| format!("\"{}\"", item))
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn string_list(items: &[String]) -> String {
        format!(
            "[{}]",
            items.iter()
                .map(|item| format!("\"{}\"", item))
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    fn model_json(name: &str, model: &gewyvern::fragment::ModelDiagnostics) -> String {
        format!(
            "\"{name}\":{{\"model\":\"{}\",\"rules\":[{}]}}",
            model.model,
            model.rules
                .iter()
                .map(|rule| format!(
                    "{{\"rule_index\":{},\"tier\":\"{}\",\"supported\":{},\"required_facts\":{},\"supporting_fragments\":{},\"missing_facts\":{}}}",
                    rule.rule_index,
                    match rule.tier {
                        gewyvern::fragment::RuleTier::CoreRequirement => "core_requirement",
                        gewyvern::fragment::RuleTier::OptionalEnhancement => "optional_enhancement",
                        gewyvern::fragment::RuleTier::Unsupported => "unsupported",
                    },
                    rule.supported,
                    fact_list(&rule.required_facts),
                    string_list(&rule.supporting_fragments),
                    fact_list(&rule.missing_facts),
                ))
                .collect::<Vec<_>>()
                .join(",")
        )
    }

    let mut fields = vec![
        format!("\"template_id\":\"{}\"", binding.template.id),
        format!(
            "\"fragments\":[{}]",
            binding
                .template
                .fragment_set
                .iter()
                .map(|fragment| format!("\"{}\"", fragment))
                .collect::<Vec<_>>()
                .join(",")
        ),
    ];
    if let Some(model) = &diagnostics.program_model {
        fields.push(model_json("program_model", model));
    } else {
        fields.push("\"program_model\":null".into());
    }
    if let Some(model) = &diagnostics.reason_model {
        fields.push(model_json("reason_model", model));
    } else {
        fields.push("\"reason_model\":null".into());
    }
    format!("{{{}}}", fields.join(","))
}

fn serve_socket_sessions(cli: &Cli, socket_target: &SocketTarget) {
    match socket_target {
        SocketTarget::Unix(path) => serve_unix_socket_sessions(cli, path),
        SocketTarget::Tcp(addr) => serve_tcp_socket_sessions(cli, addr),
    }
}

fn serve_unix_socket_sessions(cli: &Cli, path: &str) {
    #[cfg(target_family = "unix")]
    {
        let _ = fs::remove_file(path);
        let listener = bind_unix_socket_listener(path).unwrap_or_else(|err| {
            eprintln!("socket service failed: {err:?}");
            std::process::exit(1);
        });
        let max_sessions = cli.max_sessions.unwrap_or(usize::MAX);

        for _ in 0..max_sessions {
            let export = if let Some(binding) = cli.dsl_binding() {
                run_unix_socket_session_on_listener_with_binding(&listener, binding)
            } else {
                run_unix_socket_session_on_listener(&listener, cli.template_mode.template())
            }
                .unwrap_or_else(|err| {
                    eprintln!("socket service failed: {err:?}");
                    std::process::exit(1);
                });
            emit_rendered(cli, "socket_session", &export, true);
        }

        let _ = fs::remove_file(path);
        return;
    }

    #[cfg(not(target_family = "unix"))]
    {
        let _ = path;
        eprintln!("unix socket service is only supported on unix platforms");
        std::process::exit(1);
    }
}

fn serve_tcp_socket_sessions(cli: &Cli, addr: &str) {
    let listener = TcpListener::bind(addr).unwrap_or_else(|err| {
        eprintln!("socket service failed: {err}");
        std::process::exit(1);
    });
    let max_sessions = cli.max_sessions.unwrap_or(usize::MAX);

    for _ in 0..max_sessions {
        let export = if let Some(binding) = cli.dsl_binding() {
            run_tcp_socket_session_on_listener_with_binding(&listener, binding)
        } else {
            run_tcp_socket_session_on_listener(&listener, cli.template_mode.template())
        }
            .unwrap_or_else(|err| {
                eprintln!("socket service failed: {err:?}");
                std::process::exit(1);
            });
        emit_rendered(cli, "socket_session", &export, true);
    }
}

fn emit_rendered(cli: &Cli, name: &str, export: &ExportBundle, append: bool) {
    let rendered = if cli.json {
        if cli.summary_only {
            summary_json(name, export)
        } else {
            export.to_json()
        }
    } else {
        summary_line(name, export)
    };

    if let Some(path) = cli.out_path.as_deref() {
        if append {
            let mut existing = fs::read_to_string(path).unwrap_or_default();
            existing.push_str(&rendered);
            existing.push('\n');
            fs::write(path, existing).unwrap_or_else(|err| {
                eprintln!("failed to write output to {path}: {err}");
                std::process::exit(1);
            });
        } else {
            fs::write(path, format!("{rendered}\n")).unwrap_or_else(|err| {
                eprintln!("failed to write output to {path}: {err}");
                std::process::exit(1);
            });
        }
    } else {
        println!("{rendered}");
    }
}
