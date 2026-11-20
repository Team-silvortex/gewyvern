//! Real isolated chrony campaigns with clock control disabled at two layers.
use std::fs;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::super::udp_evidence::{Context, Datagram, validate};
use super::http_evidence::sha256;
use super::packet_capture::Scope;
use super::udp_capture::Capture;
use super::{
    ValidationError, ValidationReport, clock_anchor, network_namespace, port, prepare_output, tool,
};
use serde_json::{Value, json};
use socket2::{Domain, Protocol, Socket, Type};

mod evidence;
mod production;
mod target;
mod wire;

pub(super) fn run_production(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    production::run(out)
}

fn socket(source: Ipv4Addr, target: SocketAddr) -> Result<Socket, ValidationError> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.bind(&SocketAddr::from((source, 0)).into())?;
    socket.connect(&target.into())?;
    Ok(socket)
}

#[derive(Clone, Copy)]
struct Case {
    label: &'static str,
    version: u8,
    stamp: u64,
    short: bool,
}
const CASES: [Case; 4] = [
    Case {
        label: "v4",
        version: 4,
        stamp: 0x1234567800000001,
        short: false,
    },
    Case {
        label: "v3",
        version: 3,
        stamp: 0x1234567800000002,
        short: false,
    },
    Case {
        label: "short-timeout",
        version: 4,
        stamp: 0,
        short: true,
    },
    Case {
        label: "recovery-v4",
        version: 4,
        stamp: 0x1234567800000003,
        short: false,
    },
];

struct Exchange {
    label: &'static str,
    cookie: u64,
    start_ns: u64,
    end_ns: u64,
    request: Vec<u8>,
    response: Option<Vec<u8>>,
    reply: Option<wire::Reply>,
}

fn exchange(
    socket: &UdpSocket,
    cookie: u64,
    case: Case,
    local: bool,
) -> Result<Exchange, ValidationError> {
    let request = wire::query(case.version, case.stamp, case.short);
    socket.set_read_timeout(Some(if case.short {
        Duration::from_millis(350)
    } else {
        Duration::from_secs(2)
    }))?;
    let start_ns = clock_anchor()?.0;
    if socket.send(&request)? != request.len() {
        return Err(ValidationError::new("partial NTP datagram send"));
    }
    let mut bytes = [0; 129];
    let response = match socket.recv(&mut bytes) {
        Ok(n) => Some(bytes[..n].to_vec()),
        Err(e) if case.short && matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) => {
            None
        }
        Err(e) => return Err(e.into()),
    };
    let end_ns = clock_anchor()?.0;
    if response.is_none() != case.short
        || (case.short && end_ns.saturating_sub(start_ns) < 300_000_000)
    {
        return Err(ValidationError::new(
            "NTP timeout outcome differs from fixture",
        ));
    }
    let reply = response
        .as_ref()
        .map(|b| wire::reply(b, &request, local))
        .transpose()?;
    Ok(Exchange {
        label: case.label,
        cookie,
        start_ns,
        end_ns,
        request,
        response,
        reply,
    })
}

fn phase(local: bool, out: &Path) -> Result<Value, ValidationError> {
    fs::create_dir(out)?;
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ValidationError::new(e.to_string()))?
            .as_nanos()
    );
    let mut target = target::Target::start(&token, local, out)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let probe: UdpSocket = socket(target.source, target.address)?.into();
        match exchange(&probe, 0, CASES[0], local) {
            Ok(_) => break,
            Err(e) if Instant::now() >= deadline => {
                target.log(out)?;
                return Err(ValidationError::new(format!("NTP readiness: {e}")));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let mut scope = Scope::enter(&token)?;
    let sockets = [
        socket(target.source, target.address)?,
        socket(target.source, target.address)?,
    ];
    let identities = [
        (sockets[0].cookie()?, port(&sockets[0])?),
        (sockets[1].cookie()?, port(&sockets[1])?),
    ];
    let destination = match target.address.ip() {
        std::net::IpAddr::V4(ip) => ip,
        _ => return Err(ValidationError::new("NTP fixture requires IPv4")),
    };
    let addresses = (target.source.octets(), destination.octets());
    let clock = clock_anchor()?;
    let in_dir = out.join("ingress");
    let out_dir = out.join("egress");
    fs::create_dir(&in_dir)?;
    fs::create_dir(&out_dir)?;
    let input = Capture::attach(&scope, &identities, addresses, 123, false, &in_dir)?;
    let output = Capture::attach(&scope, &identities, addresses, 123, true, &out_dir)?;
    let links_owned = [input.id()?, output.id()?];
    let [first, second] = sockets.map(UdpSocket::from);
    let mut exchanges = Vec::new();
    for case in CASES {
        eprintln!("UDP NTP local_reference={local}: {}", case.label);
        exchanges.push(exchange(&first, identities[0].0, case, local)?);
    }
    exchanges.push(exchange(&second, identities[1].0, CASES[1], local)?);
    let neighbor = socket(target.source, target.address)?;
    let neighbor_cookie = neighbor.cookie()?;
    let neighbor: UdpSocket = neighbor.into();
    exchange(&neighbor, neighbor_cookie, CASES[1], local)?;
    let end_ns = clock_anchor()?.0;
    let (ingress, in_counts) = input.finish(&in_dir)?;
    let (egress, out_counts) = output.finish(&out_dir)?;
    let context = Context {
        sockets: identities,
        remote_port: 123,
        addresses: addresses.into(),
        netns: network_namespace()?,
        clock,
        end_ns,
    };
    let observations = evidence::reconcile(&exchanges, &ingress, &egress, &context, local)?;
    if in_counts.total != 4
        || out_counts.total != 5
        || ingress
            .iter()
            .chain(&egress)
            .any(|e| e.cookie == neighbor_cookie)
    {
        return Err(ValidationError::new(
            "NTP totals or neighbor isolation differs",
        ));
    }
    let packages = evidence::packages(&ingress, &egress, &context, local, out)?;
    fs::write(
        out.join("datagrams.json"),
        serde_json::to_vec_pretty(&json!({"ingress":ingress,"egress":egress}))?,
    )?;
    let links: Vec<Value> = serde_json::from_slice(&tool(
        Command::new("bpftool").args(["-j", "link", "show"]),
        out,
    )?)?;
    if links
        .iter()
        .any(|link| links_owned.iter().any(|id| link["id"] == *id))
    {
        return Err(ValidationError::new("owned NTP BPF link survived cleanup"));
    }
    drop((first, second, neighbor));
    scope.cleanup()?;
    target.cleanup()?;
    Ok(
        json!({"local_reference":local,"target":target.metadata,"selected_sockets":identities,"neighbor_cookie":neighbor_cookie,
        "ingress_counts":in_counts,"egress_counts":out_counts,"observations":observations,"packages":packages,"owned_resources_removed":true}),
    )
}

pub(super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out, "linux-udp-ntp")?;
    let mut phases = Vec::new();
    for (name, local) in [("local-reference", true), ("unsynchronised", false)] {
        phases.push(phase(local, &out.join(name))?);
    }
    super::http_boundaries::run(Some(out.join("after-ntp-restart")))?;
    let checks = [
        "real_private_chrony_no_clock_control",
        "NTPv3_and_v4",
        "unsynchronised_reply_not_time_success",
        "short_datagram_timeout_then_recovery",
        "same_origin_distinct_sockets",
        "bidirectional_kernel_bytes_match_client",
        "neighbor_excluded",
        "three_packages_both_server_states",
        "owned_cleanup_and_fresh_HTTP",
    ];
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(
            &json!({"schema_version":1,"ok":true,"mode":"udp-ntp","phases":phases,"checks":checks,
        "host_clock_adjusted":false,"external_time_sources":false,"ingest_trust_mode":"unverified-local",
        "not_proven":["clock_accuracy_or_clock_discipline","real_KoD_rate_limit","NTS_MAC_or_extension_authentication","product_origin_correlation","IPv6_or_IP_reassembly","UDP_throughput","PID_attribution","multiple_hosts_or_kernels"]}),
        )?,
    )?;
    Ok(ValidationReport {
        name: "Linux real UDP/NTP capture breadth".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}
