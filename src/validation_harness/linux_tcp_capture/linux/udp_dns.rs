//! Bounded real DNS server campaign; client bytes never fill capture gaps.
use std::collections::BTreeSet;
use std::fs;
use std::io::ErrorKind;
use std::net::{Ipv4Addr, SocketAddr, UdpSocket};
use std::path::PathBuf;
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};
use socket2::{Domain, Protocol, Socket, Type};

use super::super::udp_evidence::{Context, Datagram, reconstruct, validate};
use super::docker_target::{OwnedResource, docker, target_addresses};
use super::http_evidence::sha256;
use super::packet_capture::Scope;
use super::{
    Counts, ValidationError, ValidationReport, clock_anchor, network_namespace, port,
    prepare_output, repo_root, tool,
};
use crate::export::ExportBundle;
use crate::ledger::{FactKind, PacketDir};

mod capture;
mod wire;

const IMAGE: &str =
    "coredns/coredns@sha256:0a9d8c8ddf34347eb59aad9a8784f0710de88d6d112726d11d271f9bc6daca26";
const CONFIG: &str = include_str!("../../../../ebpf/validation/dns.Corefile");
const ID: u16 = 0x4411;

fn socket(source: Ipv4Addr, target: SocketAddr) -> Result<Socket, ValidationError> {
    let socket = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP))?;
    socket.bind(&SocketAddr::from((source, 0)).into())?;
    socket.connect(&target.into())?;
    Ok(socket)
}

struct Exchange {
    cookie: u64,
    start_ns: u64,
    end_ns: u64,
    query: Vec<u8>,
    responses: Vec<Vec<u8>>,
    reply: Option<wire::Reply>,
}

fn exchange(
    socket: &UdpSocket,
    cookie: u64,
    question: &str,
    expected: Option<(u8, bool)>,
) -> Result<Exchange, ValidationError> {
    let query = wire::query(question, ID)?;
    socket.set_read_timeout(Some(if expected.is_none() {
        Duration::from_millis(350)
    } else {
        Duration::from_secs(2)
    }))?;
    let start_ns = clock_anchor()?.0;
    if socket.send(&query)? != query.len() {
        return Err(ValidationError::new("partial DNS datagram send"));
    }
    let mut buffer = [0; 513];
    let response = match socket.recv(&mut buffer) {
        Ok(count) => Some(buffer[..count].to_vec()),
        Err(error)
            if expected.is_none()
                && matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
        {
            None
        }
        Err(error) => return Err(error.into()),
    };
    let mut responses: Vec<_> = response.into_iter().collect();
    // CoreDNS template errors can produce multiple wire responses. Retain every
    // observed datagram in a fixed window rather than shifting it to the next query.
    if !responses.is_empty() {
        let deadline = Instant::now() + Duration::from_millis(50);
        while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
            if remaining.is_zero() {
                break;
            }
            socket.set_read_timeout(Some(remaining))?;
            match socket.recv(&mut buffer) {
                Ok(count) if responses.len() < 4 => responses.push(buffer[..count].to_vec()),
                Ok(_) => {
                    return Err(ValidationError::new(
                        "DNS response count exceeds bounded exchange",
                    ));
                }
                Err(error)
                    if matches!(error.kind(), ErrorKind::TimedOut | ErrorKind::WouldBlock) =>
                {
                    break;
                }
                Err(error) => return Err(error.into()),
            }
        }
    }
    let end_ns = clock_anchor()?.0;
    let reply = responses
        .first()
        .map(|bytes| {
            wire::parse_reply(bytes, &query).map_err(|error| {
                ValidationError::new(format!(
                    "{question}: {error}; bounded server datagram: {bytes:02x?}"
                ))
            })
        })
        .transpose()?;
    for response in &responses {
        if Some(wire::parse_reply(response, &query)?) != reply {
            return Err(ValidationError::new(
                "DNS replies disagree within bounded exchange",
            ));
        }
    }
    if reply.as_ref().map(|reply| (reply.rcode, reply.truncated)) != expected
        || reply.as_ref().is_some_and(|reply| {
            !reply.truncated && reply.rcode == 0 && reply.addresses != [[192, 0, 2, 7]]
        })
        || (expected.is_none() && end_ns.saturating_sub(start_ns) < 300_000_000)
    {
        return Err(ValidationError::new(
            "DNS client outcome differs from controlled server case",
        ));
    }
    Ok(Exchange {
        cookie,
        start_ns,
        end_ns,
        query,
        responses,
        reply,
    })
}

fn reconcile(
    exchanges: &[Exchange],
    ingress: &[Datagram],
    egress: &[Datagram],
    context: &Context,
) -> Result<Vec<Value>, ValidationError> {
    let mut used_in = BTreeSet::new();
    let mut used_out = BTreeSet::new();
    let mut observations = Vec::new();
    for exchange in exchanges {
        let matching = |events: &[Datagram],
                        outbound: bool,
                        used: &mut BTreeSet<u64>|
         -> Result<Vec<Vec<u8>>, ValidationError> {
            let mut bytes = Vec::new();
            for event in events {
                let payload = validate(event, outbound, context)?;
                if event.cookie == exchange.cookie
                    && event.monotonic_ns >= exchange.start_ns
                    && event.monotonic_ns <= exchange.end_ns
                {
                    if !used.insert(event.sequence) {
                        return Err(ValidationError::new(
                            "DNS datagram reused by multiple exchanges",
                        ));
                    }
                    bytes.push(payload.to_vec());
                }
            }
            Ok(bytes)
        };
        let requests = matching(egress, true, &mut used_out)?;
        let responses = matching(ingress, false, &mut used_in)?;
        if requests != [exchange.query.clone()] || responses != exchange.responses {
            return Err(ValidationError::new(
                "kernel DNS datagrams differ from independent client I/O",
            ));
        }
        let captured_reply = responses
            .first()
            .map(|bytes| wire::parse_reply(bytes, &requests[0]))
            .transpose()?;
        if captured_reply != exchange.reply {
            return Err(ValidationError::new(
                "captured DNS semantics disagree with client",
            ));
        }
        observations.push(json!({"cookie":exchange.cookie,"start_ns":exchange.start_ns,"end_ns":exchange.end_ns,
            "elapsed_us":(exchange.end_ns-exchange.start_ns)/1000,"client_reply":exchange.reply,"captured_reply":captured_reply,
            "timeout":exchange.responses.is_empty(),"response_count":responses.len(),"query_sha256":sha256(&exchange.query),"captured_query_sha256":sha256(&requests[0]),
            "client_response_sha256":exchange.responses.iter().map(|b|sha256(b)).collect::<Vec<_>>(),"captured_response_sha256":responses.iter().map(|b|sha256(b)).collect::<Vec<_>>()}));
    }
    if used_in.len() != ingress.len() || used_out.len() != egress.len() {
        return Err(ValidationError::new(
            "unaccounted UDP datagrams in selected capture",
        ));
    }
    Ok(observations)
}

fn stage_cookies(
    export: &ExportBundle,
    phase: &str,
    direction: PacketDir,
) -> Result<BTreeSet<u64>, ValidationError> {
    export
        .program_flows
        .iter()
        .flat_map(|flow| &flow.stages)
        .filter(|stage| stage.phase.as_deref() == Some(phase))
        .map(|stage| {
            export
                .facts
                .iter()
                .find_map(|fact| match &fact.kind {
                    FactKind::PacketMeta(packet)
                        if fact.id == stage.at && packet.dir == direction =>
                    {
                        packet.sk_cookie
                    }
                    _ => None,
                })
                .ok_or_else(|| ValidationError::new("DNS stage lacks matching packet provenance"))
        })
        .collect()
}

pub(super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out, "linux-udp-dns")?;
    let image_id = tool(
        docker().args(["image", "inspect", IMAGE, "--format", "{{.Id}}"]),
        &out,
    )?;
    if fs::read(repo_root().join("ebpf/validation/dns.Corefile"))? != CONFIG.as_bytes() {
        return Err(ValidationError::new(
            "compiled DNS fixture differs from workspace",
        ));
    }
    fs::write(out.join("Corefile"), CONFIG)?;
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ValidationError::new(e.to_string()))?
            .as_nanos()
    );
    let mut network = OwnedResource::create(
        "network",
        &token,
        &["--driver", "bridge", "--internal"],
        &out,
    )?;
    let network_info = network.inspect()?;
    let mut container = OwnedResource::create(
        "container",
        &token,
        &[
            "--pull=never",
            "--network",
            &network.name,
            "--cpus=1",
            "--memory=256m",
            "--pids-limit=128",
            "--cap-drop=ALL",
            "--cap-add=NET_BIND_SERVICE",
            "--security-opt=no-new-privileges",
            IMAGE,
            "-conf",
            "/Corefile",
        ],
        &out,
    )?;
    tool(
        docker()
            .arg("cp")
            .arg(out.join("Corefile"))
            .arg(format!("{}:/Corefile", container.name)),
        &out,
    )?;
    tool(docker().args(["start", &container.name]), &out)?;
    let container_info = container.inspect()?;
    if container_info["Mounts"]
        .as_array()
        .is_none_or(|mounts| !mounts.is_empty())
    {
        return Err(ValidationError::new("DNS target has unexpected mounts"));
    }
    let (source, destination) = target_addresses(&container_info, &network_info, &network.name)?;
    let target = SocketAddr::from((destination, 53));
    let deadline = Instant::now() + Duration::from_secs(30);
    loop {
        let probe: UdpSocket = socket(source, target)?.into();
        match exchange(&probe, 0, "ok.test.", Some((0, false))) {
            Ok(_) => break,
            Err(error) if Instant::now() >= deadline => {
                fs::write(
                    out.join("target-startup.log"),
                    tool(
                        docker().args(["logs", "--tail", "100", &container.name]),
                        &out,
                    )?,
                )?;
                return Err(ValidationError::new(format!(
                    "DNS readiness failed: {error}"
                )));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
    let mut scope = Scope::enter(&token)?;
    let sockets = [socket(source, target)?, socket(source, target)?];
    let identities = [
        (sockets[0].cookie()?, port(&sockets[0])?),
        (sockets[1].cookie()?, port(&sockets[1])?),
    ];
    if identities
        .iter()
        .any(|(cookie, port)| *cookie == 0 || *port == 0)
        || identities[0].0 == identities[1].0
        || identities[0].1 == identities[1].1
    {
        return Err(ValidationError::new(
            "selected UDP identities are not unique",
        ));
    }
    let addresses = (source.octets(), destination.octets());
    let clock = clock_anchor()?;
    let in_dir = out.join("ingress");
    let out_dir = out.join("egress");
    fs::create_dir(&in_dir)?;
    fs::create_dir(&out_dir)?;
    let input = capture::Capture::attach(&scope, &identities, addresses, false, &in_dir)?;
    let output = capture::Capture::attach(&scope, &identities, addresses, true, &out_dir)?;
    let links_owned = [input.id()?, output.id()?];
    let [first, second] = sockets.map(UdpSocket::from);
    let mut exchanges = Vec::new();
    for (question, expected) in [
        ("ok.test.", Some((0, false))),
        ("form.test.", Some((1, false))),
        ("fail.test.", Some((2, false))),
        ("nx.test.", Some((3, false))),
        ("deny.test.", Some((5, false))),
        ("cut.test.", Some((0, true))),
        ("drop.test.", None),
        ("ok.test.", Some((0, false))),
    ] {
        eprintln!("UDP DNS case: {question}");
        exchanges.push(exchange(&first, identities[0].0, question, expected)?);
    }
    exchanges.push(exchange(
        &second,
        identities[1].0,
        "ok.test.",
        Some((0, false)),
    )?);
    let neighbor = socket(source, target)?;
    let neighbor_cookie = neighbor.cookie()?;
    let neighbor: UdpSocket = neighbor.into();
    let neighbor_exchange = exchange(&neighbor, neighbor_cookie, "nx.test.", Some((3, false)))?;
    let end_ns = clock_anchor()?.0;
    let (ingress, in_counts) = input.finish(&in_dir)?;
    let (egress, out_counts) = output.finish(&out_dir)?;
    let context = Context {
        sockets: identities,
        addresses,
        netns: network_namespace()?,
        clock,
        end_ns,
    };
    let observations = reconcile(&exchanges, &ingress, &egress, &context)?;
    if in_counts.total
        != exchanges
            .iter()
            .map(|exchange| exchange.responses.len() as u64)
            .sum::<u64>()
        || out_counts.total != 9
        || ingress
            .iter()
            .chain(&egress)
            .any(|event| event.cookie == neighbor_cookie)
    {
        return Err(ValidationError::new(
            "DNS event totals or neighbor isolation differ",
        ));
    }
    let mut packages = Vec::new();
    for (index, (path, rules)) in [
        (
            "protocols/dns/udp/main.gewy",
            include_str!("../../../../protocols/dns/udp/main.gewy"),
        ),
        (
            "protocols/dns/error/main.gewy",
            include_str!("../../../../protocols/dns/error/main.gewy"),
        ),
    ]
    .into_iter()
    .enumerate()
    {
        if fs::read(repo_root().join(path))? != rules.as_bytes() {
            return Err(ValidationError::new(
                "compiled DNS package differs from workspace",
            ));
        }
        let export = reconstruct(&ingress, &egress, &context, rules)?;
        let wanted = if index == 0 {
            identities.iter().map(|(cookie, _)| *cookie).collect()
        } else {
            BTreeSet::from([identities[0].0])
        };
        let phases: &[&str] = if index == 0 {
            &["send_request", "receive_reply"]
        } else {
            &[
                "receive_formerr",
                "receive_servfail",
                "receive_nxdomain",
                "receive_refused",
            ]
        };
        for phase in phases {
            if stage_cookies(
                &export,
                phase,
                if *phase == "send_request" {
                    PacketDir::Egress
                } else {
                    PacketDir::Ingress
                },
            )? != wanted
            {
                return Err(ValidationError::new(format!(
                    "DNS package phase {phase} differs from real datagrams"
                )));
            }
        }
        fs::write(
            out.join(format!("package-{index}-export.json")),
            export.to_json(),
        )?;
        packages.push(json!({"path":path,"source_sha256":sha256(rules.as_bytes()),"phases":phases,"matched_cookies":wanted,"replay_identical":true,
            "facts":export.facts.len(),"flows":export.flows.len(),"process_attribution":false,"packet_only":true}));
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
        .any(|link| links_owned.iter().any(|id| link["id"] == *id))
    {
        return Err(ValidationError::new("owned UDP BPF link survived cleanup"));
    }
    drop((first, second, neighbor));
    scope.cleanup()?;
    container.cleanup()?;
    network.cleanup()?;
    super::http_boundaries::run(Some(out.join("after-dns-restart")))?;
    let checks = [
        "real_private_DNS_server",
        "two_selected_sockets_same_DNS_id",
        "bidirectional_kernel_bytes_match_client",
        "four_RCODEs_and_truncation",
        "bounded_timeout_then_success",
        "unselected_neighbor_excluded",
        "packet_only_package_replay",
        "owned_resources_removed",
        "fresh_HTTP_capture_after_DNS",
    ];
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({"schema_version":1,"ok":true,"mode":"udp-dns",
        "image":IMAGE,"image_id":String::from_utf8_lossy(&image_id).trim(),"config_sha256":sha256(CONFIG.as_bytes()),
        "container_id":container_info["Id"],"network_id":network_info["Id"],"internal_network":true,"published_ports":false,
        "selected_sockets":identities,"unselected_neighbor_cookie":neighbor_cookie,"unselected_neighbor_reply":neighbor_exchange.reply,
        "ingress_counts":in_counts,"egress_counts":out_counts,"observations":observations,"packages":packages,"checks":checks,
        "owned_resources_removed":true,"ingest_trust_mode":"unverified-local","not_proven":["IPv6","IP_fragment_reassembly",
        "general_DNS_parser","EDNS_DNSSEC_DoH_DoT","TCP_fallback_for_TC","recursive_resolution_or_cache",
        "general_UDP_throughput","PID_or_container_process_attribution","multiple_hosts_or_kernels"]}))?,
    )?;
    Ok(ValidationReport {
        name: "Linux real UDP/DNS capture breadth".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dns_reconciliation_requires_socket_time_and_exact_bytes_not_only_id() {
        let context = Context {
            sockets: [(1, 4000), (2, 4001)],
            addresses: ([127, 0, 0, 1], [127, 0, 0, 2]),
            netns: 1,
            clock: (1, SystemTime::UNIX_EPOCH),
            end_ns: 10,
        };
        let query = wire::query("ok.test.", ID).unwrap();
        let mut response = query.clone();
        response[2] |= 0x80;
        let make = |cookie: u64, sequence: u64, outbound: bool| {
            let payload = if outbound { &query } else { &response };
            let mut prefix = vec![0; 128];
            prefix[..payload.len()].copy_from_slice(payload);
            Datagram {
                sequence,
                monotonic_ns: if outbound { 2 } else { 3 },
                cookie,
                cpu: 0,
                ifindex: 1,
                source: if outbound { 0x7f000001 } else { 0x7f000002 },
                destination: if outbound { 0x7f000002 } else { 0x7f000001 },
                sport: if outbound { 3999 + cookie as u16 } else { 53 },
                dport: if outbound { 53 } else { 3999 + cookie as u16 },
                total_len: 28 + payload.len() as u16,
                payload_len: payload.len() as u16,
                prefix_len: payload.len() as u16,
                ip_header_len: 20,
                prefix,
            }
        };
        let exchanges: Vec<_> = [1, 2]
            .into_iter()
            .map(|cookie| Exchange {
                cookie,
                start_ns: 2,
                end_ns: 4,
                query: query.clone(),
                responses: vec![response.clone()],
                reply: Some(wire::parse_reply(&response, &query).unwrap()),
            })
            .collect();
        let input = [make(1, 1, false), make(2, 2, false)];
        let output = [make(1, 1, true), make(2, 2, true)];
        assert_eq!(
            reconcile(&exchanges, &input, &output, &context)
                .unwrap()
                .len(),
            2
        );
        let mut wrong = input.clone();
        wrong[1].cookie = 1;
        wrong[1].dport = 4000;
        assert!(reconcile(&exchanges, &wrong, &output, &context).is_err());
        let mut wrong = input.clone();
        wrong[0].prefix[13] = b'x';
        assert!(reconcile(&exchanges, &wrong, &output, &context).is_err());
        let mut wrong = input.clone();
        wrong[0].monotonic_ns = 5;
        assert!(reconcile(&exchanges, &wrong, &output, &context).is_err());
        assert!(reconcile(&exchanges, &input[..1], &output, &context).is_err());
        assert!(reconcile(&exchanges, &input, &output[..1], &context).is_err());
        let mut duplicate = input[0].clone();
        duplicate.sequence = 3;
        assert!(
            reconcile(
                &exchanges,
                &[input[0].clone(), input[1].clone(), duplicate.clone()],
                &output,
                &context
            )
            .is_err()
        );
        let mut exchanges = exchanges;
        exchanges[0].responses.push(response);
        assert!(
            reconcile(
                &exchanges,
                &[input[0].clone(), input[1].clone(), duplicate],
                &output,
                &context
            )
            .is_ok()
        );
    }
}
