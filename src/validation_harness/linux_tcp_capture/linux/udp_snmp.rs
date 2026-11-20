//! Real Net-SNMP, isolated read-only MIB, exact bidirectional kernel evidence.
use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::super::udp_evidence::{Context, Datagram, reconstruct, validate};
use super::docker_target::{OwnedResource, docker, target_addresses};
use super::http_evidence::sha256;
use super::packet_capture::Scope;
use super::udp_capture::Capture;
use super::{
    ValidationError, ValidationReport, clock_anchor, network_namespace, port, prepare_output,
    repo_root, tool,
};
use crate::ledger::{FactKind, PacketDir};
use serde_json::{Value, json};
use socket2::{Domain, Protocol, Socket, Type};

mod production;
mod wire;

pub(super) fn run_production(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    production::run(out)
}

const IMAGE: &str = "gewyvern-field-snmp:5.9.5.2";
const CONFIG: &str = include_str!("../../../../ebpf/validation/snmp/snmpd.conf");

fn socket(source: Ipv4Addr, target: SocketAddr) -> Result<Socket, ValidationError> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.bind(&SocketAddr::from((source, 0)).into())?;
    socket.connect(&target.into())?;
    Ok(socket)
}

struct Exchange {
    label: &'static str,
    cookie: u64,
    id: u32,
    start_ns: u64,
    end_ns: u64,
    request: Vec<u8>,
    response: Option<Vec<u8>>,
    unknown: bool,
}

#[derive(Clone, Copy)]
struct Case {
    label: &'static str,
    pdu: u8,
    id: u32,
    unknown: bool,
    timeout: bool,
}
const CASES: [Case; 6] = [
    Case {
        label: "get",
        pdu: 0xa0,
        id: 1,
        unknown: false,
        timeout: false,
    },
    Case {
        label: "get-next",
        pdu: 0xa1,
        id: 128,
        unknown: false,
        timeout: false,
    },
    Case {
        label: "get-bulk",
        pdu: 0xa5,
        id: 32768,
        unknown: false,
        timeout: false,
    },
    Case {
        label: "unknown-oid",
        pdu: 0xa0,
        id: 2,
        unknown: true,
        timeout: false,
    },
    Case {
        label: "wrong-community",
        pdu: 0xa0,
        id: 3,
        unknown: false,
        timeout: true,
    },
    Case {
        label: "recovery-get",
        pdu: 0xa0,
        id: 4,
        unknown: false,
        timeout: false,
    },
];

fn exchange(socket: &UdpSocket, cookie: u64, case: Case) -> Result<Exchange, ValidationError> {
    let community = if case.timeout {
        b"not-authorized".as_slice()
    } else {
        wire::COMMUNITY
    };
    let oid = if case.unknown {
        wire::UNKNOWN
    } else if case.pdu == 0xa0 {
        wire::LOCATION
    } else {
        wire::PREVIOUS
    };
    let request = wire::query(case.pdu, case.id, community, oid);
    socket.set_read_timeout(Some(if case.timeout {
        Duration::from_millis(350)
    } else {
        Duration::from_secs(2)
    }))?;
    let start_ns = clock_anchor()?.0;
    if socket.send(&request)? != request.len() {
        return Err(ValidationError::new("partial SNMP datagram send"));
    }
    let mut bytes = [0; 129];
    let response = match socket.recv(&mut bytes) {
        Ok(count) => Some(bytes[..count].to_vec()),
        Err(e)
            if case.timeout && matches!(e.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
        {
            None
        }
        Err(e) => return Err(e.into()),
    };
    let end_ns = clock_anchor()?.0;
    if response.is_none() != case.timeout || (case.timeout && end_ns - start_ns < 300_000_000) {
        return Err(ValidationError::new(
            "SNMP timeout outcome differs from fixture",
        ));
    }
    if let Some(bytes) = &response {
        wire::check_reply(bytes, case.id, case.unknown)?;
    }
    Ok(Exchange {
        label: case.label,
        cookie,
        id: case.id,
        start_ns,
        end_ns,
        request,
        response,
        unknown: case.unknown,
    })
}

fn reconcile(
    exchanges: &[Exchange],
    ingress: &[Datagram],
    egress: &[Datagram],
    context: &Context,
) -> Result<Vec<Value>, ValidationError> {
    let mut seen = [BTreeSet::new(), BTreeSet::new()];
    let mut observations = Vec::new();
    for exchange in exchanges {
        let mut captured = [Vec::new(), Vec::new()];
        for (index, events) in [egress, ingress].into_iter().enumerate() {
            for event in events {
                let bytes = validate(event, index == 0, context)?;
                if event.cookie == exchange.cookie
                    && (exchange.start_ns..=exchange.end_ns).contains(&event.monotonic_ns)
                {
                    if !seen[index].insert(event.sequence) {
                        return Err(ValidationError::new("SNMP datagram reused"));
                    }
                    captured[index].push(bytes.to_vec());
                }
            }
        }
        if captured[0] != [exchange.request.clone()]
            || captured[1] != exchange.response.iter().cloned().collect::<Vec<_>>()
        {
            return Err(ValidationError::new(
                "kernel SNMP datagrams differ from independent client I/O",
            ));
        }
        if let Some(bytes) = captured[1].first() {
            wire::check_reply(bytes, exchange.id, exchange.unknown)?;
        }
        observations.push(json!({"case":exchange.label,"cookie":exchange.cookie,"request_id":exchange.id,
            "start_ns":exchange.start_ns,"end_ns":exchange.end_ns,"timeout":exchange.response.is_none(),
            "no_such_object":exchange.unknown,"request_bytes":exchange.request.len(),
            "response_bytes":exchange.response.as_ref().map(Vec::len),
            "client_request_sha256":sha256(&exchange.request),"captured_request_sha256":sha256(&captured[0][0]),
            "client_response_sha256":exchange.response.as_ref().map(|b|sha256(b)),
            "captured_response_sha256":captured[1].first().map(|b|sha256(b)),
            "legacy_request_byte13":exchange.request.get(13),"legacy_response_byte13":exchange.response.as_ref().and_then(|b|b.get(13))}));
    }
    if seen[0].len() != egress.len() || seen[1].len() != ingress.len() {
        return Err(ValidationError::new("unaccounted SNMP datagrams"));
    }
    Ok(observations)
}

struct Target {
    container: OwnedResource,
    network: OwnedResource,
    token: String,
    source: Ipv4Addr,
    address: SocketAddr,
    metadata: Value,
}
fn prepare_target(out: &std::path::Path) -> Result<Target, ValidationError> {
    let image_id = String::from_utf8(tool(
        docker().args(["image", "inspect", IMAGE, "--format", "{{.Id}}"]),
        out,
    )?)
    .map_err(|e| ValidationError::new(e.to_string()))?;
    if !image_id.trim().starts_with("sha256:") {
        return Err(ValidationError::new("invalid SNMP image identity"));
    }
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ValidationError::new(e.to_string()))?
            .as_nanos()
    );
    let network = OwnedResource::create(
        "network",
        &token,
        &["--driver", "bridge", "--internal"],
        out,
    )?;
    let network_info = network.inspect()?;
    let container = OwnedResource::create(
        "container",
        &token,
        &[
            "--pull=never",
            "--network",
            &network.name,
            "--cpus=1",
            "--memory=128m",
            "--pids-limit=64",
            "--read-only",
            "--tmpfs",
            "/tmp:rw,noexec,nosuid,size=4m",
            "--cap-drop=ALL",
            "--cap-add=NET_BIND_SERVICE",
            "--security-opt=no-new-privileges",
            image_id.trim(),
        ],
        out,
    )?;
    // Inspect the image's actual configuration, not just the mutable tag.
    tool(
        docker()
            .args(["cp", &format!("{}:/snmpd.conf", container.name)])
            .arg(out.join("snmpd.conf")),
        out,
    )?;
    if fs::read(out.join("snmpd.conf"))? != CONFIG.as_bytes()
        || fs::read(repo_root().join("ebpf/validation/snmp/snmpd.conf"))? != CONFIG.as_bytes()
    {
        return Err(ValidationError::new(
            "SNMP image configuration differs from compiled fixture",
        ));
    }
    tool(docker().args(["start", &container.name]), out)?;
    let info = container.inspect()?;
    if info["Mounts"]
        .as_array()
        .is_none_or(|mounts| !mounts.is_empty())
    {
        return Err(ValidationError::new("SNMP target has unexpected mounts"));
    }
    let (source, destination) = target_addresses(&info, &network_info, &network.name)?;
    let target = SocketAddr::from((destination, 161));
    let deadline = Instant::now() + Duration::from_secs(20);
    loop {
        let probe: UdpSocket = socket(source, target)?.into();
        match exchange(&probe, 0, CASES[0]) {
            Ok(_) => break,
            Err(error) if Instant::now() >= deadline => {
                fs::write(
                    out.join("startup.log"),
                    tool(
                        docker().args(["logs", "--tail", "100", &container.name]),
                        out,
                    )?,
                )?;
                return Err(ValidationError::new(format!("SNMP readiness: {error}")));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    Ok(Target {
        metadata: json!({"image_id":image_id.trim(),"config_sha256":sha256(CONFIG.as_bytes()),
            "container_id":info["Id"],"network_id":network_info["Id"],"internal_network":true,
            "published_ports":false,"host_mounts":false}),
        container,
        network,
        token,
        source,
        address: target,
    })
}

pub(super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out, "linux-udp-snmp")?;
    let Target {
        mut network,
        mut container,
        token,
        source,
        address: target,
        metadata,
    } = prepare_target(&out)?;
    let mut scope = Scope::enter(&token)?;
    let sockets = [socket(source, target)?, socket(source, target)?];
    let identities = [
        (sockets[0].cookie()?, port(&sockets[0])?),
        (sockets[1].cookie()?, port(&sockets[1])?),
    ];
    let std::net::IpAddr::V4(destination) = target.ip() else {
        return Err(ValidationError::new("SNMP fixture requires IPv4"));
    };
    let addresses = (source.octets(), destination.octets());
    let clock = clock_anchor()?;
    let in_dir = out.join("ingress");
    let out_dir = out.join("egress");
    fs::create_dir(&in_dir)?;
    fs::create_dir(&out_dir)?;
    let input = Capture::attach(&scope, &identities, addresses, 161, false, &in_dir)?;
    let output = Capture::attach(&scope, &identities, addresses, 161, true, &out_dir)?;
    let link_ids = [input.id()?, output.id()?];
    let [first, second] = sockets.map(UdpSocket::from);
    let mut exchanges = Vec::new();
    for case in CASES {
        eprintln!("UDP SNMP case: {}", case.label);
        exchanges.push(exchange(&first, identities[0].0, case)?);
    }
    exchanges.push(exchange(&second, identities[1].0, CASES[0])?);
    let neighbor = socket(source, target)?;
    let neighbor_cookie = neighbor.cookie()?;
    let neighbor: UdpSocket = neighbor.into();
    exchange(&neighbor, neighbor_cookie, CASES[0])?;
    let end_ns = clock_anchor()?.0;
    let (ingress, in_counts) = input.finish(&in_dir)?;
    let (egress, out_counts) = output.finish(&out_dir)?;
    let context = Context {
        sockets: identities,
        remote_port: 161,
        addresses: addresses.into(),
        netns: network_namespace()?,
        clock,
        end_ns,
    };
    let observations = reconcile(&exchanges, &ingress, &egress, &context)?;
    if in_counts.total != 6
        || out_counts.total != 7
        || ingress
            .iter()
            .chain(&egress)
            .any(|e| e.cookie == neighbor_cookie)
    {
        return Err(ValidationError::new(
            "SNMP counts or neighbor isolation differs",
        ));
    }
    let mut packages = Vec::new();
    for (index, (path, rules, request_phase, response_phase)) in [
        (
            "protocols/snmp/get/main.gewy",
            include_str!("../../../../protocols/snmp/get/main.gewy"),
            "send_get_request",
            "receive_get_response",
        ),
        (
            "protocols/snmp/get-next/main.gewy",
            include_str!("../../../../protocols/snmp/get-next/main.gewy"),
            "send_get_next_request",
            "receive_get_next_response",
        ),
        (
            "protocols/snmp/bulk/main.gewy",
            include_str!("../../../../protocols/snmp/bulk/main.gewy"),
            "send_bulk_request",
            "receive_bulk_response",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if fs::read(repo_root().join(path))? != rules.as_bytes() {
            return Err(ValidationError::new(
                "compiled SNMP package differs from workspace",
            ));
        }
        let request_kind = ["get", "get_next", "get_bulk"][index];
        let request_tag = ["a0", "a1", "a5"][index];
        let legacy_rules = rules.replace(
            &format!("snmp_pdu_observed:remote:snmp:{request_kind}"),
            &format!("datagram_observed:udp:remote:snmp:local_to_remote:byte0_mask:0xff:0x30:byte_at:13:0xff:0x{request_tag}"),
        ).replace("snmp_pdu_observed:remote:snmp:response", "datagram_observed:udp:remote:snmp:remote_to_local:byte0_mask:0xff:0x30:byte_at:13:0xff:0xa2");
        let legacy = reconstruct(&ingress, &egress, &context, &legacy_rules)?;
        let legacy_matches = legacy.program_flows.iter().flat_map(|f| &f.stages)
            .filter(|s| matches!(s.phase.as_deref(), Some(p) if p == request_phase || p == response_phase)).count();
        if legacy_matches != 0 {
            return Err(ValidationError::new(
                "SNMP fixed-offset regression was not reproduced",
            ));
        }
        let export = reconstruct(&ingress, &egress, &context, rules)?;
        for (phase, dir) in [
            (request_phase, PacketDir::Egress),
            (response_phase, PacketDir::Ingress),
        ] {
            let cookies: BTreeSet<_> = export
                .program_flows
                .iter()
                .flat_map(|f| &f.stages)
                .filter(|s| s.phase.as_deref() == Some(phase))
                .map(|stage| {
                    export
                        .facts
                        .iter()
                        .find_map(|fact| match &fact.kind {
                            FactKind::PacketMeta(p) if fact.id == stage.at && p.dir == dir => {
                                p.sk_cookie
                            }
                            _ => None,
                        })
                        .ok_or_else(|| ValidationError::new("SNMP stage lacks packet provenance"))
                })
                .collect::<Result<_, _>>()?;
            let expected = if index == 0 || dir == PacketDir::Ingress {
                BTreeSet::from([identities[0].0, identities[1].0])
            } else {
                BTreeSet::from([identities[0].0])
            };
            if cookies != expected {
                return Err(ValidationError::new(format!(
                    "SNMP package phase {phase} differs from real datagrams: {cookies:?}"
                )));
            }
        }
        fs::write(
            out.join(format!("package-{index}-export.json")),
            export.to_json(),
        )?;
        packages.push(json!({"path":path,"source_sha256":sha256(rules.as_bytes()),"request_phase":request_phase,"response_phase":response_phase,
            "replay_identical":true,"packet_only":true,"response_is_generic_candidate":true,"transaction_correlation":false,
            "legacy_fixed_offset_matching_stages":legacy_matches}));
    }
    fs::write(
        out.join("datagrams.json"),
        serde_json::to_vec_pretty(&json!({"ingress":ingress,"egress":egress}))?,
    )?;
    let links: Vec<Value> = serde_json::from_slice(&tool(
        Command::new("bpftool").args(["-j", "link", "show"]),
        &out,
    )?)?;
    if links
        .iter()
        .any(|link| link_ids.iter().any(|id| link["id"] == *id))
    {
        return Err(ValidationError::new("SNMP BPF link survived cleanup"));
    }
    drop((first, second, neighbor));
    scope.cleanup()?;
    container.cleanup()?;
    network.cleanup()?;
    super::http_boundaries::run(Some(out.join("after-snmp-restart")))?;
    let checks = [
        "real_private_Net_SNMP",
        "GET_GETNEXT_GETBULK",
        "noSuchObject_not_success",
        "wrong_community_timeout_then_recovery",
        "same_request_id_distinct_sockets",
        "bidirectional_kernel_bytes_match_client",
        "neighbor_excluded",
        "packet_only_replay",
        "owned_cleanup_and_fresh_HTTP",
    ];
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({"schema_version":1,"ok":true,"mode":"udp-snmp",
        "image_id":metadata["image_id"],"config_sha256":metadata["config_sha256"],"container_id":metadata["container_id"],"network_id":metadata["network_id"],
        "internal_network":true,"published_ports":false,"selected_sockets":identities,"neighbor_cookie":neighbor_cookie,
        "observations":observations,"ingress_counts":in_counts,"egress_counts":out_counts,"packages":packages,"checks":checks,"owned_resources_removed":true,
        "not_proven":["SNMPv3_auth_or_privacy","SET_trap_inform","full_MIB_walk","product_request_response_correlation","IPv6_or_IP_reassembly","UDP_throughput","PID_attribution","multiple_hosts_or_kernels"]}))?,
    )?;
    Ok(ValidationReport {
        name: "Linux real UDP/SNMP capture breadth".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reconciliation_never_substitutes_client_bytes_or_request_id_for_kernel_identity() {
        let context = Context {
            sockets: [(1, 4000), (2, 4001)],
            remote_port: 161,
            addresses: ([127, 0, 0, 1], [127, 0, 0, 2]).into(),
            netns: 1,
            clock: (1, SystemTime::UNIX_EPOCH),
            end_ns: 10,
        };
        let request = wire::query(0xa0, 1, b"denied", wire::LOCATION);
        let mut prefix = vec![0; 128];
        prefix[..request.len()].copy_from_slice(&request);
        let event = Datagram {
            sequence: 1,
            monotonic_ns: 3,
            cookie: 1,
            cpu: 0,
            ifindex: 1,
            source: [127, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            destination: [127, 0, 0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
            sport: 4000,
            dport: 161,
            total_len: 28 + request.len() as u32,
            payload_len: request.len() as u16,
            prefix_len: request.len() as u16,
            ip_header_len: 20,
            ip_version: 4,
            prefix,
        };
        let exchange = Exchange {
            label: "timeout",
            cookie: 1,
            id: 1,
            start_ns: 2,
            end_ns: 4,
            request,
            response: None,
            unknown: false,
        };
        let exchanges = [exchange];
        assert!(reconcile(&exchanges, &[], std::slice::from_ref(&event), &context).is_ok());
        assert!(reconcile(&exchanges, &[], &[], &context).is_err());
        for kind in 0..4 {
            let mut bad = event.clone();
            match kind {
                0 => {
                    bad.cookie = 2;
                    bad.sport = 4001;
                }
                1 => bad.monotonic_ns = 5,
                2 => bad.prefix[13] ^= 1,
                _ => bad.dport = 53,
            }
            assert!(reconcile(&exchanges, &[], &[bad], &context).is_err());
        }
        let mut duplicate = event.clone();
        duplicate.sequence = 2;
        assert!(reconcile(&exchanges, &[], &[event, duplicate], &context).is_err());
    }
}
