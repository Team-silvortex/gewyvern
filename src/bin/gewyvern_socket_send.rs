use gewyvern::export::fact_to_json;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, RouteDecisionFact, SessionId,
    TcpStateFact,
};
use gewyvern::socket_input::MAX_FACT_LINE_BYTES;
use gewyvern::transport_safety::connect_with_deadline;
use std::env;
use std::io::{self, Write};
use std::net::TcpStream;
use std::time::{Duration, SystemTime};

const SOCKET_IO_TIMEOUT: Duration = Duration::from_secs(3);

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
                stream
                    .set_write_timeout(Some(SOCKET_IO_TIMEOUT))
                    .unwrap_or_else(|err| {
                        eprintln!("failed to configure socket timeout for {path}: {err}");
                        std::process::exit(1);
                    });

                write_payload(&mut stream, &cli.payload_mode);
            }

            #[cfg(not(target_family = "unix"))]
            {
                let _ = path;
                eprintln!("unix socket sender is only supported on unix platforms");
                std::process::exit(1);
            }
        }
        SocketTarget::Tcp(addr) => {
            let mut stream = connect_tcp(&addr).unwrap_or_else(|err| {
                eprintln!("failed to connect to {addr}: {err}");
                std::process::exit(1);
            });

            write_payload(&mut stream, &cli.payload_mode);
        }
    }
}

fn connect_tcp(addr: &str) -> io::Result<TcpStream> {
    let stream = connect_with_deadline(addr, SOCKET_IO_TIMEOUT)?;
    stream.set_write_timeout(Some(SOCKET_IO_TIMEOUT))?;
    Ok(stream)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct Cli {
    socket_target: SocketTarget,
    payload_mode: PayloadMode,
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

#[derive(Clone, Debug, Eq, PartialEq)]
enum PayloadMode {
    Template(TemplateMode),
    RawLine(String),
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
        let mut payload_mode = None;
        let mut args = args.into_iter();

        while let Some(arg) = args.next() {
            match arg.as_str() {
                "--socket" | "--unix-socket" => {
                    let target = SocketTarget::Unix(args.next().ok_or_else(|| {
                        "missing value for --socket, expected a unix socket path".to_string()
                    })?);
                    select_once(
                        &mut socket_target,
                        target,
                        "socket target may be specified only once",
                    )?;
                }
                "--tcp-socket" => {
                    let target = SocketTarget::Tcp(args.next().ok_or_else(|| {
                        "missing value for --tcp-socket, expected host:port".to_string()
                    })?);
                    select_once(
                        &mut socket_target,
                        target,
                        "socket target may be specified only once",
                    )?;
                }
                "--template" => {
                    let value = args.next().ok_or_else(|| {
                        "missing value for --template, expected tcp or udp".to_string()
                    })?;
                    select_once(
                        &mut payload_mode,
                        PayloadMode::Template(TemplateMode::from_str(&value)?),
                        "payload mode may be specified only once",
                    )?;
                }
                "--raw-line" => {
                    let line = args.next().ok_or_else(|| {
                        "missing value for --raw-line, expected one literal line".to_string()
                    })?;
                    select_once(
                        &mut payload_mode,
                        PayloadMode::RawLine(validate_raw_line(line)?),
                        "payload mode may be specified only once",
                    )?;
                }
                "--help" | "-h" => return Err(usage().into()),
                other => return Err(format!("unknown argument '{other}'\n{}", usage())),
            }
        }

        Ok(Self {
            socket_target: socket_target.ok_or_else(|| {
                "missing required --socket <path> or --tcp-socket <host:port>".to_string()
            })?,
            payload_mode: payload_mode.unwrap_or(PayloadMode::Template(TemplateMode::Udp)),
        })
    }
}

fn select_once<T>(slot: &mut Option<T>, value: T, error: &str) -> Result<(), String> {
    if slot.is_some() {
        return Err(error.into());
    }
    *slot = Some(value);
    Ok(())
}

fn validate_raw_line(line: String) -> Result<String, String> {
    if line.trim().is_empty() {
        return Err("--raw-line must not be empty or whitespace-only".into());
    }
    if line.contains(['\r', '\n']) {
        return Err("--raw-line must contain exactly one line without CR or LF".into());
    }
    if line.len().saturating_add(1) > MAX_FACT_LINE_BYTES {
        return Err(format!(
            "--raw-line exceeds the {} byte socket-ingest line limit",
            MAX_FACT_LINE_BYTES
        ));
    }
    Ok(line)
}

fn write_payload<W: Write>(stream: &mut W, payload_mode: &PayloadMode) {
    match payload_mode {
        PayloadMode::Template(template_mode) => {
            for fact in sample_facts(*template_mode) {
                writeln!(stream, "{}", fact_to_json(&fact)).unwrap_or_else(|err| {
                    eprintln!("failed to write fact to socket: {err}");
                    std::process::exit(1);
                });
            }
        }
        PayloadMode::RawLine(line) => {
            writeln!(stream, "{line}").unwrap_or_else(|err| {
                eprintln!("failed to write raw line to socket: {err}");
                std::process::exit(1);
            });
        }
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
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
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
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: std::collections::BTreeMap::new(),
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
    "usage: gewyvern_socket_send (--socket path|--tcp-socket host:port) [--template tcp|udp|--raw-line literal]"
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;

    use super::{Cli, MAX_FACT_LINE_BYTES, PayloadMode, SocketTarget, TemplateMode, connect_tcp};

    #[test]
    fn parse_cli_accepts_raw_line_payload() {
        let cli = Cli::from_args([
            "--tcp-socket".to_string(),
            "127.0.0.1:9000".to_string(),
            "--raw-line".to_string(),
            "{\"broken\":true".to_string(),
        ])
        .unwrap();

        assert_eq!(
            cli.socket_target,
            SocketTarget::Tcp("127.0.0.1:9000".into())
        );
        assert_eq!(
            cli.payload_mode,
            PayloadMode::RawLine("{\"broken\":true".into())
        );
    }

    #[test]
    fn parse_cli_defaults_to_udp_template_mode() {
        let cli =
            Cli::from_args(["--tcp-socket".to_string(), "127.0.0.1:9000".to_string()]).unwrap();

        assert_eq!(cli.payload_mode, PayloadMode::Template(TemplateMode::Udp));
    }

    #[test]
    fn parse_cli_rejects_duplicate_or_conflicting_selections_in_any_order() {
        for args in [
            vec![
                "--socket",
                "/tmp/gewyvern.sock",
                "--tcp-socket",
                "127.0.0.1:9",
            ],
            vec![
                "--tcp-socket",
                "127.0.0.1:9",
                "--socket",
                "/tmp/gewyvern.sock",
            ],
            vec![
                "--tcp-socket",
                "127.0.0.1:9",
                "--template",
                "udp",
                "--raw-line",
                "{}",
            ],
            vec![
                "--tcp-socket",
                "127.0.0.1:9",
                "--raw-line",
                "{}",
                "--template",
                "udp",
            ],
            vec![
                "--tcp-socket",
                "127.0.0.1:9",
                "--template",
                "udp",
                "--template",
                "tcp",
            ],
        ] {
            assert!(
                Cli::from_args(args.iter().map(|value| (*value).to_string())).is_err(),
                "{args:?}"
            );
        }
    }

    #[test]
    fn parse_cli_rejects_multiline_empty_and_oversized_raw_payloads() {
        for line in ["", "   ", "{}\n{}", "{}\r{}"] {
            assert!(
                Cli::from_args([
                    "--tcp-socket".to_string(),
                    "127.0.0.1:9000".to_string(),
                    "--raw-line".to_string(),
                    line.to_string(),
                ])
                .is_err(),
                "{line:?}"
            );
        }

        let oversized = "x".repeat(MAX_FACT_LINE_BYTES);
        assert!(
            Cli::from_args([
                "--tcp-socket".to_string(),
                "127.0.0.1:9000".to_string(),
                "--raw-line".to_string(),
                oversized,
            ])
            .is_err()
        );

        let largest_valid = "x".repeat(MAX_FACT_LINE_BYTES - 1);
        assert!(
            Cli::from_args([
                "--tcp-socket".to_string(),
                "127.0.0.1:9000".to_string(),
                "--raw-line".to_string(),
                largest_valid,
            ])
            .is_ok()
        );
    }

    #[test]
    fn tcp_connector_uses_resolved_loopback_candidates() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        assert!(connect_tcp(&listener.local_addr().unwrap().to_string()).is_ok());
    }
}
