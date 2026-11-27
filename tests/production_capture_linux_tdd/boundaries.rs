//! Real native UDP peers, not protocol-server conformance or fabricated facts.
use super::*;
use gewyvern::ledger::PacketDir;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const LENGTHS: [usize; 15] = [
    0, 1, 2, 3, 4, 127, 128, 129, 512, 1400, 4096, 60000, 1, 0, 32,
];

fn payload(index: usize) -> Vec<u8> {
    (0..LENGTHS[index])
        .map(|at| ((index * 17 + at * 29) % 251) as u8)
        .collect()
}
pub(super) fn hash(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
pub(super) struct Packet {
    family: u16,
    cookie: String,
    local_port: u16,
    remote_port: u16,
    outbound: bool,
    total_len: u32,
    prefix: Vec<u8>,
}
pub(super) fn packet(
    cookie: u64,
    local: SocketAddr,
    remote: SocketAddr,
    outbound: bool,
    bytes: &[u8],
) -> Packet {
    assert_eq!(local.is_ipv6(), remote.is_ipv6());
    Packet {
        family: if local.is_ipv6() { 0x86dd } else { 0x800 },
        cookie: cookie.to_string(),
        local_port: local.port(),
        remote_port: remote.port(),
        outbound,
        total_len: bytes.len() as u32 + if local.is_ipv6() { 48 } else { 28 },
        prefix: bytes[..bytes.len().min(128)].to_vec(),
    }
}

fn pair(port: u16) -> ([UdpSocket; 2], [u64; 2]) {
    let first = Socket::new(Domain::IPV4, Type::DGRAM, Some(Protocol::UDP)).unwrap();
    first.bind(&endpoint(0, port).into()).unwrap();
    let port = first.local_addr().unwrap().as_socket().unwrap().port();
    let second = Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP)).unwrap();
    second.set_only_v6(true).unwrap();
    second
        .bind(&SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, port)).into())
        .unwrap();
    let cookies = [first.cookie().unwrap(), second.cookie().unwrap()];
    let sockets = [UdpSocket::from(first), UdpSocket::from(second)];
    for socket in &sockets {
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
    }
    (sockets, cookies)
}
fn endpoint(family: usize, port: u16) -> SocketAddr {
    if family == 0 {
        SocketAddr::from((std::net::Ipv4Addr::LOCALHOST, port))
    } else {
        SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, port))
    }
}
fn echo() -> Peers {
    let (sockets, _) = pair(0);
    let port = sockets[0].local_addr().unwrap().port();
    let stop = Arc::new(AtomicBool::new(false));
    let threads = sockets
        .into_iter()
        .map(|socket| {
            let stop = stop.clone();
            socket
                .set_read_timeout(Some(Duration::from_millis(50)))
                .unwrap();
            thread::spawn(move || {
                let mut buffer = vec![0; 65536];
                while !stop.load(Ordering::Relaxed) {
                    match socket.recv_from(&mut buffer) {
                        Ok((n, remote)) => {
                            assert_eq!(socket.send_to(&buffer[..n], remote).unwrap(), n);
                        }
                        Err(e)
                            if matches!(
                                e.kind(),
                                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                            ) => {}
                        Err(e) => panic!("echo failed: {e}"),
                    }
                }
            })
        })
        .collect();
    Peers {
        stop,
        threads,
        port,
    }
}
fn exchange(socket: &UdpSocket, remote: SocketAddr, bytes: &[u8], connected: bool) -> Vec<u8> {
    let sent = if connected {
        socket.send(bytes)
    } else {
        socket.send_to(bytes, remote)
    }
    .unwrap();
    assert_eq!(sent, bytes.len());
    let mut buffer = vec![0; 65536];
    let (n, source) = socket.recv_from(&mut buffer).unwrap();
    assert_eq!(source, remote);
    assert_eq!(&buffer[..n], bytes);
    buffer.truncate(n);
    buffer
}
fn worker_start(mode: &str, scope: &Path, out: &Path, selected: u16, other: u16) -> Process {
    Process(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "boundaries::udp_boundary_worker",
                "--nocapture",
            ])
            .env("GW_BOUNDARY_MODE", mode)
            .env("GW_BOUNDARY_SCOPE", scope)
            .env("GW_BOUNDARY_OUT", out)
            .env("GW_BOUNDARY_PORT", selected.to_string())
            .env("GW_BOUNDARY_OTHER_PORT", other.to_string())
            .stdout(fs::File::create(out.join("worker.log")).unwrap())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}

#[test]
#[ignore = "internal native worker for the privileged UDP boundary test"]
fn udp_boundary_worker() {
    let mode = std::env::var("GW_BOUNDARY_MODE").unwrap();
    assert!(["connected", "unconnected", "descendant", "server"].contains(&mode.as_str()));
    let out = PathBuf::from(std::env::var_os("GW_BOUNDARY_OUT").unwrap());
    let scope = PathBuf::from(std::env::var_os("GW_BOUNDARY_SCOPE").unwrap());
    assert!(scope.starts_with("/sys/fs/cgroup"));
    assert!(
        scope
            .to_string_lossy()
            .contains("/gewyvern-production-test-boundaries-")
    );
    let selected: u16 = std::env::var("GW_BOUNDARY_PORT").unwrap().parse().unwrap();
    let other: u16 = std::env::var("GW_BOUNDARY_OTHER_PORT")
        .unwrap()
        .parse()
        .unwrap();
    // Socket membership precedes process migration: these must remain outside.
    let (neighbors, neighbor_cookies) = pair(0);
    fs::write(scope.join("cgroup.procs"), std::process::id().to_string()).unwrap();
    let (sockets, cookies) = pair(0);
    let mut expected = Vec::new();
    let mut observations = Vec::new();
    let mut excluded = Vec::new();
    if mode == "server" {
        for socket in &sockets {
            socket
                .set_read_timeout(Some(Duration::from_secs(45)))
                .unwrap();
        }
        fs::write(
            out.join("worker-ready.json"),
            serde_json::to_vec(&json!({"port":sockets[0].local_addr().unwrap().port()})).unwrap(),
        )
        .unwrap();
        for index in 0..LENGTHS.len() {
            for family in 0..2 {
                let mut bytes = vec![0; 65536];
                let (n, remote) = sockets[family].recv_from(&mut bytes).unwrap();
                bytes.truncate(n);
                assert_eq!(bytes, payload(index));
                let local = sockets[family].local_addr().unwrap();
                expected.push(packet(cookies[family], local, remote, false, &bytes));
                assert_eq!(sockets[family].send_to(&bytes, remote).unwrap(), n);
                expected.push(packet(cookies[family], local, remote, true, &bytes));
                observations.push(json!({"length":n,"family":if family==0 {4}else{6},
                    "cookie":cookies[family].to_string(),"remote_port":remote.port(),
                    "io_sha256":hash(&bytes),"captured_bytes":n.min(128),"full_payload_captured":n<=128}));
            }
        }
    } else {
        let connected = mode == "connected";
        let (wrong_port, wrong_cookies) = pair(0);
        for family in 0..2 {
            let remote = endpoint(family, selected);
            if connected {
                sockets[family].connect(remote).unwrap();
            }
            for (socket, cookie, port, kind) in [
                (
                    &neighbors[family],
                    neighbor_cookies[family],
                    selected,
                    "preexisting_socket_outside_scope",
                ),
                (
                    &wrong_port[family],
                    wrong_cookies[family],
                    other,
                    "different_udp_port",
                ),
            ] {
                let bytes = b"excluded but actually echoed";
                let reply = exchange(socket, endpoint(family, port), bytes, false);
                excluded.push(json!({"kind":kind,"cookie":cookie.to_string(),"reply_sha256":hash(&reply),"family":if family==0 {4}else{6}}));
            }
        }
        for index in 0..LENGTHS.len() {
            let bytes = payload(index);
            for family in 0..2 {
                let remote = endpoint(family, selected);
                let local = sockets[family].local_addr().unwrap();
                let reply = exchange(&sockets[family], remote, &bytes, connected);
                expected.push(packet(cookies[family], local, remote, true, &bytes));
                expected.push(packet(cookies[family], local, remote, false, &reply));
                observations.push(json!({"length":bytes.len(),"family":if family==0 {4}else{6},
                    "cookie":cookies[family].to_string(),"remote_port":remote.port(),
                    "io_sha256":hash(&reply),"captured_bytes":bytes.len().min(128),"full_payload_captured":bytes.len()<=128}));
            }
        }
    }
    fs::write(
        out.join("oracle.json"),
        serde_json::to_vec_pretty(&json!({
            "mode":mode,"expected":expected,"observations":observations,"excluded":excluded,
            "selected_cookies":cookies.map(|c|c.to_string()),"workload_scope":scope
        }))
        .unwrap(),
    )
    .unwrap();
}

pub(super) fn collector(scope: &Scope, port: u16, out: &Path) -> Process {
    collector_with_options(scope, port, out, &[])
}

pub(super) fn collector_with_options(
    scope: &Scope,
    port: u16,
    out: &Path,
    extra: &[&str],
) -> Process {
    collector_with_output_dir(scope, port, out, out, extra)
}

pub(super) fn collector_with_output_dir(
    scope: &Scope,
    port: u16,
    out: &Path,
    outputs: &Path,
    extra: &[&str],
) -> Process {
    Process(
        collector_command(scope, port, out, outputs, extra)
            .spawn()
            .unwrap(),
    )
}

pub(super) fn collector_command(
    scope: &Scope,
    port: u16,
    out: &Path,
    outputs: &Path,
    extra: &[&str],
) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_gewyvern"));
    command
        .env_clear()
        .env("HOME", out.join("home"))
        .env("GEWY_CONFIG_HOME", out.join("config"))
        .env("GEWY_DATA_HOME", out.join("data"))
        .env("GEWY_STATE_HOME", out.join("state"))
        .env("GEWY_CACHE_HOME", out.join("cache"))
        .current_dir(out)
        .args(["--json", "--capture-cgroup"])
        .arg(&scope.0)
        .args([
            "--capture-udp-port",
            &port.to_string(),
            "--capture-seconds",
            "40",
            "--capture-max-facts",
            "4096",
            "--out",
        ])
        .arg(outputs.join("export.json"))
        .arg("--capture-status")
        .arg(outputs.join("status.json"))
        .args(extra)
        .stdin(Stdio::null())
        .stdout(fs::File::create(out.join("stdout.log")).unwrap())
        .stderr(fs::File::create(out.join("stderr.log")).unwrap());
    command
}

pub(super) fn check_packets(bundle: &ExportBundle, expected: &[Packet]) {
    assert_eq!(bundle.ingest_trust_mode, "unverified-local");
    assert!(bundle.flows.iter().all(|f| f.process.is_none()));
    assert!(bundle.program_flows.iter().all(|f| f.process.is_none()));
    let mut namespaces = BTreeSet::new();
    let mut actual = Vec::new();
    for fact in &bundle.facts {
        let FactKind::PacketMeta(p) = &fact.kind else {
            panic!("fabricated non-packet fact")
        };
        assert_eq!(p.l4_proto, 17);
        assert_ne!(p.netns, 0);
        namespaces.insert(p.netns);
        let prefix: Vec<_> = (0..p.payload_bytes.len())
            .map(|at| p.payload_bytes[&(at as u16)])
            .collect();
        assert!(prefix.len() <= 128);
        for (at, value) in [
            (0, p.payload_byte0),
            (1, p.payload_byte1),
            (4, p.payload_byte4),
            (5, p.payload_byte5),
            (9, p.payload_byte9),
            (10, p.payload_byte10),
            (13, p.payload_byte13),
        ] {
            assert_eq!(value, prefix.get(at).copied());
        }
        assert_eq!(
            p.payload_prefix2,
            prefix
                .get(..2)
                .map(|b| u16::from_be_bytes(b.try_into().unwrap()))
        );
        assert_eq!(
            p.payload_prefix4,
            prefix
                .get(..4)
                .map(|b| u32::from_be_bytes(b.try_into().unwrap()))
        );
        actual.push(Packet {
            family: p.l3_proto,
            cookie: p.sk_cookie.unwrap().to_string(),
            local_port: p.local_port.unwrap(),
            remote_port: p.remote_port.unwrap(),
            outbound: p.dir == PacketDir::Egress,
            total_len: p.tot_len,
            prefix,
        });
    }
    assert_eq!(namespaces.len(), 1);
    let mut expected = expected.to_vec();
    actual.sort();
    expected.sort();
    assert_eq!(
        actual, expected,
        "product packet multiset differs from actual client/server I/O"
    );
}

#[test]
fn bounded_packet_oracle_checks_length_prefix_ports_and_absent_bytes() {
    use gewyvern::ledger::{FactId, SessionId};
    use gewyvern::packet_capture::{CapturedUdpDatagram, PacketObservation};
    use gewyvern::runtime::{RuntimeSession, SessionConfig};
    let mut bundle = RuntimeSession::start(
        SessionConfig::for_template(gewyvern::template::udp_debug_template()).unwrap(),
    )
    .unwrap()
    .into_export_bundle();
    bundle.ingest_trust_mode = "unverified-local".into();
    let mut expected = Vec::new();
    for index in 0..LENGTHS.len() {
        let bytes = payload(index);
        for family in 0..2 {
            let local = endpoint(family, 4000);
            let remote = endpoint(family, 9000);
            let p = packet(7, local, remote, true, &bytes);
            bundle.facts.push(
                CapturedUdpDatagram {
                    source: local,
                    destination: remote,
                    ip_header_len: if family == 0 { 20 } else { 40 },
                    total_len: p.total_len,
                    payload_len: bytes.len() as u16,
                    payload_prefix: &p.prefix,
                }
                .into_fact(PacketObservation {
                    id: FactId(bundle.facts.len() as u64 + 1),
                    session: SessionId(1),
                    ts: UNIX_EPOCH,
                    cpu: 0,
                    ifindex: 1,
                    netns: 1,
                    socket_cookie: 7,
                    direction: PacketDir::Egress,
                })
                .unwrap(),
            );
            expected.push(p);
        }
    }
    check_packets(&bundle, &expected);
    for variant in 0..4 {
        let mut bad = bundle.clone();
        let FactKind::PacketMeta(p) = &mut bad.facts[0].kind else {
            unreachable!()
        };
        match variant {
            0 => p.payload_byte0 = Some(0),
            1 => p.tot_len += 1,
            2 => p.remote_port = Some(9001),
            _ => {
                p.payload_bytes.insert(0, 1);
            }
        }
        assert!(std::panic::catch_unwind(|| check_packets(&bad, &expected)).is_err());
    }
}

fn campaign(mode: &str, out: &Path) -> Value {
    fs::create_dir(out).unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-boundaries-{}-{nonce}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    let child_scope = (mode == "descendant").then(|| {
        let child = Scope(scope.0.join("workload"));
        fs::create_dir(&child.0).unwrap();
        child
    });
    let workload = child_scope.as_ref().unwrap_or(&scope);
    let peers = echo();
    let other = echo();
    assert_ne!(peers.port, other.port);
    let (port, mut traffic) = if mode == "server" {
        let mut worker = worker_start(mode, &workload.0, out, 0, 0);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !out.join("worker-ready.json").exists() {
            assert!(worker.0.try_wait().unwrap().is_none());
            assert!(Instant::now() < deadline, "server worker not ready");
            thread::sleep(Duration::from_millis(10));
        }
        let meta: Value =
            serde_json::from_slice(&fs::read(out.join("worker-ready.json")).unwrap()).unwrap();
        (meta["port"].as_u64().unwrap() as u16, Some(worker))
    } else {
        (peers.port, None)
    };
    let mut capture = collector(&scope, port, out);
    ready(&mut capture, out);
    if mode == "server" {
        let clients = [pair(0).0, pair(0).0];
        for index in 0..LENGTHS.len() {
            for (family, socket) in clients[index % 2].iter().enumerate() {
                exchange(socket, endpoint(family, port), &payload(index), false);
            }
        }
    } else {
        traffic = Some(worker_start(mode, &workload.0, out, port, other.port));
    }
    assert!(wait(traffic.as_mut().unwrap(), 15).success());
    signal(&capture, libc::SIGTERM);
    assert!(
        wait(&mut capture, 10).success(),
        "capture failed: {}",
        fs::read_to_string(out.join("stderr.log")).unwrap()
    );
    let status = snapshot(out);
    assert!(
        status.final_accounted && status.reconciles() && !status.incomplete && !status.has_loss()
    );
    assert_eq!(status.state, CaptureState::Stopped);
    let bundle = export(out);
    assert_eq!(bundle.capture.as_ref(), Some(&status));
    let oracle: Value =
        serde_json::from_slice(&fs::read(out.join("oracle.json")).unwrap()).unwrap();
    let expected: Vec<Packet> = serde_json::from_value(oracle["expected"].clone()).unwrap();
    assert_eq!(expected.len(), LENGTHS.len() * 4);
    check_packets(&bundle, &expected);
    assert_eq!(status.retained, expected.len() as u64);
    let replay = bundle.replay().unwrap();
    assert_eq!(replay.facts, bundle.facts);
    assert_eq!(replay.capture, bundle.capture);
    assert_eq!(replay.program_flows, bundle.program_flows);
    assert_eq!(replay.attach_report, bundle.attach_report);
    detached(&scope);
    if let Some(child) = &child_scope {
        detached(child);
        fs::remove_dir(&child.0).unwrap();
    }
    fs::remove_dir(&scope.0).unwrap();
    json!({"mode":mode,"ok":true,"port":port,"capture":status,"observations":oracle["observations"],
        "excluded":oracle["excluded"],"selected_cookies":oracle["selected_cookies"],
        "workload_scope":oracle["workload_scope"],"facts":bundle.facts.len(),"prefix_and_lengths_match":true,
        "full_payload_claim_above_128":false,"replay_identical":true,"hooks_detached":true,"owned_cgroups_removed":true,
        "capture_role":if mode=="server" {"bound_unconnected_server"}else{"client"}})
}

#[test]
#[ignore = "requires root, cgroup v2, clang/libbpf/bpftool and GEWY_BOUNDARY_OUT=new-directory"]
fn production_capture_packet_boundaries() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let mtu: usize = fs::read_to_string("/sys/class/net/lo/mtu")
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    assert!(mtu >= 60048, "fixture must not require IP fragmentation");
    let out = PathBuf::from(std::env::var_os("GEWY_BOUNDARY_OUT").expect("new evidence directory"));
    fs::create_dir(&out).unwrap();
    let cases: Vec<_> = ["connected", "unconnected", "descendant", "server"]
        .into_iter()
        .map(|mode| campaign(mode, &out.join(mode)))
        .collect();
    fs::write(out.join("summary.json"),serde_json::to_vec_pretty(&json!({
        "schema_version":1,"ok":true,"production_cli":true,"validation_capture_used":false,
        "loopback_mtu":mtu,"payload_lengths":LENGTHS,"cases":cases,
        "binary_sha256":hash(&fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap()),
        "native_peers_not_protocol_servers":true,"owned_resources_removed":true,
        "not_proven":["full_payload_above_128","fragmentation","GSO_GRO","external_interfaces","protocol_conformance","high_load","multiple_hosts_or_kernels"]
    })).unwrap()).unwrap();
}
