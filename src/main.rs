use gewyvern::export::ExportBundle;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, RouteDecisionFact, SessionId,
    TcpStateFact,
};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::template::{handshake_debug_template, udp_debug_template};
use std::env;
use std::fs;
use std::time::{Duration, SystemTime};

fn main() {
    let cli = Cli::from_args(env::args().skip(1)).unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2);
    });
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_000_000);
    let mut outputs = Vec::new();

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
    json: bool,
    summary_only: bool,
    out_path: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DemoMode {
    Tcp,
    Udp,
    Both,
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

impl Cli {
    fn from_args<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut demo_mode = DemoMode::Both;
        let mut json = false;
        let mut summary_only = false;
        let mut out_path = None;
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
                "--summary-only" => summary_only = true,
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

        Ok(Self {
            demo_mode,
            json,
            summary_only,
            out_path,
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
    "usage: gewyvern [--demo tcp|udp|both] [--json] [--summary-only] [--out path]"
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
