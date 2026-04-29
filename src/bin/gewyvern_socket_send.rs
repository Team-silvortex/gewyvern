use gewyvern::export::fact_to_json;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, RouteDecisionFact, SessionId,
    TcpStateFact,
};
use std::env;
use std::io::Write;
use std::net::TcpStream;
use std::time::{Duration, SystemTime};

fn main() {
    let cli = Cli::from_args(env::args().skip(1)).unwrap_or_else(|message| {
        eprintln!("{message}");
        std::process::exit(2);
    });

    match cli.socket_target {
        SocketTarget::Unix(path) => {
            #[cfg(target_family = "unix")]
            {
                use std::os::unix::net::UnixStream;

                let mut stream = UnixStream::connect(&path).unwrap_or_else(|err| {
                    eprintln!("failed to connect to {path}: {err}");
                    std::process::exit(1);
                });

                write_facts(&mut stream, cli.template_mode);
                return;
            }

            #[cfg(not(target_family = "unix"))]
            {
                let _ = path;
                eprintln!("unix socket sender is only supported on unix platforms");
                std::process::exit(1);
            }
        }
        SocketTarget::Tcp(addr) => {
            let mut stream = TcpStream::connect(&addr).unwrap_or_else(|err| {
                eprintln!("failed to connect to {addr}: {err}");
                std::process::exit(1);
            });

            write_facts(&mut stream, cli.template_mode);
            return;
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Cli {
    socket_target: SocketTarget,
    template_mode: TemplateMode,
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
}

impl Cli {
    fn from_args<I>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = String>,
    {
        let mut socket_target = None;
        let mut template_mode = TemplateMode::Udp;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--socket" | "--unix-socket" => {
                    socket_target = Some(SocketTarget::Unix(args.next().ok_or_else(|| {
                        "missing value for --socket, expected a unix socket path".to_string()
                    })?));
                }
                "--tcp-socket" => {
                    socket_target = Some(SocketTarget::Tcp(args.next().ok_or_else(|| {
                        "missing value for --tcp-socket, expected host:port".to_string()
                    })?));
                }
                "--template" => {
                    let value = args.next().ok_or_else(|| {
                        "missing value for --template, expected tcp or udp".to_string()
                    })?;
                    template_mode = TemplateMode::from_str(&value)?;
                }
                "--help" | "-h" => return Err(usage().into()),
                other => return Err(format!("unknown argument '{other}'\n{}", usage())),
            }
        }

        Ok(Self {
            socket_target: socket_target.ok_or_else(|| {
                "missing required --socket <path> or --tcp-socket <host:port>".to_string()
            })?,
            template_mode,
        })
    }
}

fn write_facts<W: Write>(stream: &mut W, template_mode: TemplateMode) {
    for fact in sample_facts(template_mode) {
        writeln!(stream, "{}", fact_to_json(&fact)).unwrap_or_else(|err| {
            eprintln!("failed to write fact to socket: {err}");
            std::process::exit(1);
        });
    }
}

fn sample_facts(template_mode: TemplateMode) -> Vec<FactEnvelope> {
    let base = SystemTime::UNIX_EPOCH + Duration::from_secs(1_710_000_000);
    match template_mode {
        TemplateMode::Tcp => vec![
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
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte13: None,
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
        TemplateMode::Udp => vec![
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
                    local_port: None,
                    remote_port: None,
                    payload_byte0: None,
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte13: None,
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
    }
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

fn usage() -> &'static str {
    "usage: gewyvern_socket_send (--socket path|--tcp-socket host:port) [--template tcp|udp]"
}
