//! Real isolated coturn Binding exchanges, without TURN relays or public ports.
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
    id: u8,
    kind: u8,
}
const CASES: [Case; 5] = [
    Case {
        label: "short-timeout",
        id: 3,
        kind: 2,
    },
    Case {
        label: "binding",
        id: 1,
        kind: 0,
    },
    Case {
        label: "unknown-required",
        id: 2,
        kind: 1,
    },
    Case {
        label: "recovery-binding",
        id: 4,
        kind: 0,
    },
    Case {
        label: "optional-padded",
        id: 5,
        kind: 3,
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
    local_address: SocketAddr,
}

fn exchange(socket: &UdpSocket, cookie: u64, case: Case) -> Result<Exchange, ValidationError> {
    let request = wire::query(case.id, case.kind);
    socket.set_read_timeout(Some(if case.kind == 2 {
        Duration::from_millis(350)
    } else {
        Duration::from_secs(2)
    }))?;
    let start_ns = clock_anchor()?.0;
    if socket.send(&request)? != request.len() {
        return Err(ValidationError::new("partial STUN datagram send"));
    }
    let mut bytes = [0; 129];
    let response = match socket.recv(&mut bytes) {
        Ok(n) => Some(bytes[..n].to_vec()),
        Err(e)
            if (case.kind == 2)
                && matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
        {
            None
        }
        Err(e) => return Err(e.into()),
    };
    let end_ns = clock_anchor()?.0;
    if response.is_none() != (case.kind == 2)
        || ((case.kind == 2) && end_ns.saturating_sub(start_ns) < 300_000_000)
    {
        return Err(ValidationError::new(
            "STUN timeout outcome differs from fixture",
        ));
    }
    let reply = response
        .as_ref()
        .map(|b| wire::reply(b, &request, socket.local_addr()?, case.kind == 1))
        .transpose()?;
    Ok(Exchange {
        label: case.label,
        cookie,
        start_ns,
        end_ns,
        request,
        response,
        reply,
        local_address: socket.local_addr()?,
    })
}

fn campaign(out: &Path) -> Result<Value, ValidationError> {
    fs::create_dir(out)?;
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ValidationError::new(e.to_string()))?
            .as_nanos()
    );
    let mut target = target::Target::start(&token, out)?;
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let probe: UdpSocket = socket(target.source, target.address)?.into();
        match exchange(&probe, 0, CASES[1]) {
            Ok(_) => break,
            Err(e) if Instant::now() >= deadline => {
                target.log(out)?;
                return Err(ValidationError::new(format!("STUN readiness: {e}")));
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
        _ => return Err(ValidationError::new("STUN fixture requires IPv4")),
    };
    let addresses = (target.source.octets(), destination.octets());
    let clock = clock_anchor()?;
    let in_dir = out.join("ingress");
    let out_dir = out.join("egress");
    fs::create_dir(&in_dir)?;
    fs::create_dir(&out_dir)?;
    let input = Capture::attach(&scope, &identities, addresses, 3478, false, &in_dir)?;
    let output = Capture::attach(&scope, &identities, addresses, 3478, true, &out_dir)?;
    let links_owned = [input.id()?, output.id()?];
    let [first, second] = sockets.map(UdpSocket::from);
    let mut exchanges = Vec::new();
    for case in CASES {
        eprintln!("UDP STUN case: {}", case.label);
        exchanges.push(exchange(&first, identities[0].0, case)?);
    }
    exchanges.push(exchange(&second, identities[1].0, CASES[1])?);
    let neighbor = socket(target.source, target.address)?;
    let neighbor_cookie = neighbor.cookie()?;
    let neighbor: UdpSocket = neighbor.into();
    exchange(&neighbor, neighbor_cookie, CASES[1])?;
    let end_ns = clock_anchor()?.0;
    let (ingress, in_counts) = input.finish(&in_dir)?;
    let (egress, out_counts) = output.finish(&out_dir)?;
    let context = Context {
        sockets: identities,
        remote_port: 3478,
        addresses: addresses.into(),
        netns: network_namespace()?,
        clock,
        end_ns,
    };
    let observations = evidence::reconcile(&exchanges, &ingress, &egress, &context)?;
    if in_counts.total != 5
        || out_counts.total != 6
        || ingress
            .iter()
            .chain(&egress)
            .any(|e| e.cookie == neighbor_cookie)
    {
        return Err(ValidationError::new(
            "STUN totals or neighbor isolation differs",
        ));
    }
    let packages = evidence::packages(&ingress, &egress, &context, out)?;
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
        return Err(ValidationError::new("owned STUN BPF link survived cleanup"));
    }
    drop((first, second, neighbor));
    scope.cleanup()?;
    target.cleanup()?;
    Ok(
        json!({"target":target.metadata,"selected_sockets":identities,"neighbor_cookie":neighbor_cookie,
        "ingress_counts":in_counts,"egress_counts":out_counts,"observations":observations,"packages":packages,"owned_resources_removed":true}),
    )
}

pub(super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out, "linux-udp-stun")?;
    let campaign = campaign(&out.join("binding"))?;
    super::http_boundaries::run(Some(out.join("after-stun-restart")))?;
    let checks = [
        "real_private_coturn_binding",
        "exact_transaction_and_mapped_address",
        "unknown_required_420",
        "optional_attribute_padding",
        "short_timeout_then_recovery",
        "same_transaction_distinct_sockets",
        "bidirectional_kernel_bytes_match_client",
        "neighbor_excluded",
        "two_packages_packet_only_replay",
        "owned_cleanup_and_fresh_HTTP",
    ];
    let summary = json!({"schema_version":1,"ok":true,"mode":"udp-stun","checks":checks,
        "ingest_trust_mode":"unverified-local","campaign":campaign,
        "not_proven":["TURN_allocation_or_relay","ICE_connectivity","NAT_traversal_or_public_mapping",
        "authentication_or_FINGERPRINT_verification","product_transaction_correlation",
        "RFC3489_legacy","IPv6_or_IP_reassembly","UDP_throughput","PID_attribution","multiple_hosts_or_kernels"]});
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    Ok(ValidationReport {
        name: "Linux real UDP/STUN Binding capture breadth".into(),
        checks: checks.into_iter().map(str::to_owned).collect(),
        out_dir: out,
    })
}
