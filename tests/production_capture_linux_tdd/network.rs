//! Isolated, routed-kernel UDP I/O over a private veth pair, not host networking.
use super::boundaries::{Packet, check_packets, collector, hash, packet};
use super::*;
use std::collections::{BTreeMap, BTreeSet};
use std::os::fd::AsRawFd;

#[path = "network/errors.rs"]
mod errors;
#[path = "network/faults.rs"]
mod faults;
#[path = "network/netem.rs"]
mod netem;
#[path = "network/offload.rs"]
mod offload;
#[path = "network/pressure.rs"]
mod pressure;

const FIT: &[usize] = &[0, 1, 128, 129, 1400, 1452, 1, 0, 32];
const FRAGMENTED: &[usize] = &[1473, 4096, 60000, 1, 0, 32];
const PORT: u16 = 41002;

fn ip(args: &[&str]) {
    let output = Command::new("ip").args(args).output().unwrap();
    assert!(
        output.status.success(),
        "ip {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

struct Network(Vec<String>);
impl Network {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let mut network = Self(Vec::new());
        for role in ["client", "server"] {
            let name = format!("gewyvern-udp-{role}-{}-{nonce}", std::process::id());
            ip(&["netns", "add", &name]);
            network.0.push(name);
        }
        let [left, right] = network.0.as_slice() else {
            unreachable!()
        };
        // Both ends are born inside owned namespaces; no host interface or route is changed.
        ip(&[
            "-n", left, "link", "add", "peer0", "type", "veth", "peer", "name", "peer0", "netns",
            right,
        ]);
        for (name, host) in [(left, 1), (right, 2)] {
            ip(&["-n", name, "link", "set", "lo", "up"]);
            ip(&[
                "-n",
                name,
                "addr",
                "add",
                &format!("192.0.2.{host}/24"),
                "dev",
                "peer0",
            ]);
            ip(&[
                "-n",
                name,
                "-6",
                "addr",
                "add",
                &format!("fd00:6765:7779::{host}/64"),
                "dev",
                "peer0",
                "nodad",
            ]);
            ip(&["-n", name, "link", "set", "peer0", "mtu", "1500", "up"]);
        }
        network
    }
    fn remove(&mut self) {
        while let Some(name) = self.0.last() {
            ip(&["netns", "del", name]);
            self.0.pop();
        }
    }
}
impl Drop for Network {
    fn drop(&mut self) {
        for name in self.0.iter().rev() {
            let _ = Command::new("ip").args(["netns", "del", name]).status();
        }
    }
}

fn endpoint(family: usize, server: bool) -> SocketAddr {
    let host = if server { 2 } else { 1 };
    let port = if server { PORT } else { PORT - 1 };
    if family == 0 {
        format!("192.0.2.{host}:{port}").parse().unwrap()
    } else {
        format!("[fd00:6765:7779::{host}]:{port}").parse().unwrap()
    }
}
fn bytes(index: usize, length: usize) -> Vec<u8> {
    (0..length)
        .map(|at| ((index * 19 + at * 31) % 251) as u8)
        .collect()
}
fn lengths(fragmented: bool) -> &'static [usize] {
    if fragmented { FRAGMENTED } else { FIT }
}
fn fragment_stats() -> BTreeMap<String, u64> {
    let mut result = BTreeMap::new();
    let text = fs::read_to_string("/proc/thread-self/net/snmp").unwrap();
    let mut lines = text.lines();
    while let (Some(names), Some(values)) = (lines.next(), lines.next()) {
        if names.starts_with("Ip:") {
            for (name, value) in names
                .split_whitespace()
                .zip(values.split_whitespace())
                .skip(1)
            {
                if name.starts_with("Frag") || name.starts_with("Reasm") {
                    result.insert(format!("Ip{name}"), value.parse().unwrap());
                }
            }
        }
    }
    for line in fs::read_to_string("/proc/thread-self/net/snmp6")
        .unwrap()
        .lines()
    {
        let mut fields = line.split_whitespace();
        let name = fields.next().unwrap();
        if name.starts_with("Ip6Frag") || name.starts_with("Ip6Reasm") {
            result.insert(name.to_owned(), fields.next().unwrap().parse().unwrap());
        }
    }
    result
}

fn enter_worker_network() -> u32 {
    let scope = PathBuf::from(std::env::var_os("GW_NET_SCOPE").unwrap());
    assert!(scope.starts_with("/sys/fs/cgroup"));
    assert!(
        scope
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("gewyvern-production-test-netns-")
    );
    fs::write(scope.join("cgroup.procs"), std::process::id().to_string()).unwrap();
    let namespace = std::env::var("GW_NET_NAMESPACE").unwrap();
    assert!(namespace.starts_with("gewyvern-udp-") && !namespace.contains('/'));
    let namespace = fs::File::open(Path::new("/run/netns").join(namespace)).unwrap();
    // Enter only the network namespace on this worker thread. ip netns exec also
    // remounts sysfs and can hide the host's selected cgroup mount.
    assert_eq!(
        unsafe { libc::setns(namespace.as_raw_fd(), libc::CLONE_NEWNET) },
        0
    );
    let ifindex = unsafe { libc::if_nametoindex(c"peer0".as_ptr()) };
    assert_ne!(ifindex, 0);
    ifindex
}

fn network_sockets(server: bool) -> (Vec<UdpSocket>, Vec<String>) {
    let mut cookies = Vec::new();
    let mut sockets = Vec::new();
    for family in 0..2 {
        let socket = Socket::new(
            if family == 0 {
                Domain::IPV4
            } else {
                Domain::IPV6
            },
            Type::DGRAM,
            Some(Protocol::UDP),
        )
        .unwrap();
        if family == 1 {
            socket.set_only_v6(true).unwrap();
        }
        socket.bind(&endpoint(family, server).into()).unwrap();
        let option: libc::c_int = libc::IP_PMTUDISC_DONT;
        // Allow native IP fragmentation only on these disposable sockets.
        assert_eq!(
            unsafe {
                libc::setsockopt(
                    socket.as_raw_fd(),
                    if family == 0 {
                        libc::IPPROTO_IP
                    } else {
                        libc::IPPROTO_IPV6
                    },
                    if family == 0 {
                        libc::IP_MTU_DISCOVER
                    } else {
                        libc::IPV6_MTU_DISCOVER
                    },
                    (&option as *const libc::c_int).cast(),
                    size_of::<libc::c_int>() as libc::socklen_t,
                )
            },
            0
        );
        socket
            .set_read_timeout(Some(Duration::from_secs(40)))
            .unwrap();
        cookies.push(socket.cookie().unwrap().to_string());
        sockets.push(UdpSocket::from(socket));
    }
    (sockets, cookies)
}

#[test]
#[ignore = "internal worker, spawned inside an owned network namespace"]
fn udp_network_worker() {
    let out = PathBuf::from(std::env::var_os("GW_NET_OUT").unwrap());
    let server = std::env::var("GW_NET_ROLE").unwrap() == "server";
    let fragmented = std::env::var("GW_NET_FRAGMENTED").unwrap() == "true";
    let ifindex = enter_worker_network();
    let before = fragment_stats();
    let (sockets, cookies) = network_sockets(server);
    let netns = fs::read_link("/proc/thread-self/ns/net").unwrap();
    fs::write(out.join("ready"), b"ready").unwrap();
    let mut expected = Vec::new();
    let mut observations = Vec::new();
    for (index, &length) in lengths(fragmented).iter().enumerate() {
        for (family, socket) in sockets.iter().enumerate() {
            let local = endpoint(family, server);
            let remote = endpoint(family, !server);
            let sent = bytes(index, length);
            let mut buffer = vec![0; 65536];
            if !server {
                assert_eq!(socket.send_to(&sent, remote).unwrap(), length);
            }
            let (n, source) = socket.recv_from(&mut buffer).unwrap();
            assert_eq!(source, remote);
            assert_eq!(&buffer[..n], &sent);
            if server {
                assert_eq!(socket.send_to(&sent, remote).unwrap(), length);
            }
            for outbound in [false, true] {
                expected.push(packet(
                    cookies[family].parse().unwrap(),
                    local,
                    remote,
                    outbound,
                    &sent,
                ));
            }
            observations.push(json!({"family":if family==0 {4}else{6}, "length":length,
                "sha256":hash(&buffer[..n]), "full_native_io":true, "captured_prefix_limit":128}));
        }
    }
    let after = fragment_stats();
    let delta: BTreeMap<_, _> = after
        .iter()
        .map(|(name, value)| (name, value - before[name]))
        .collect();
    fs::write(
        out.join("oracle.json"),
        serde_json::to_vec_pretty(&json!({
            "role":if server {"server"}else{"client"}, "namespace":netns, "ifindex":ifindex,
            "cookies":cookies, "expected":expected, "observations":observations,
            "fragment_counters_delta":delta, "native_io_verified":true
        }))
        .unwrap(),
    )
    .unwrap();
}

fn worker(network: &str, role: &str, scope: &Scope, out: &Path, fragmented: bool) -> Process {
    fs::create_dir(out).unwrap();
    Process(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "network::udp_network_worker",
                "--nocapture",
            ])
            .env("GW_NET_ROLE", role)
            .env("GW_NET_NAMESPACE", network)
            .env("GW_NET_SCOPE", &scope.0)
            .env("GW_NET_OUT", out)
            .env("GW_NET_FRAGMENTED", fragmented.to_string())
            .stdout(fs::File::create(out.join("worker.log")).unwrap())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}

fn check_namespaces(bundle: &ExportBundle, oracles: &[Value]) -> Vec<u32> {
    let mut namespaces = BTreeSet::new();
    let mut all_cookies = BTreeSet::new();
    let mut facts = 0;
    for oracle in oracles {
        let cookies: BTreeSet<u64> = oracle["cookies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|cookie| cookie.as_str().unwrap().parse().unwrap())
            .collect();
        assert!(cookies.len() >= 2 && cookies.len().is_multiple_of(2));
        assert_eq!(cookies.len(), oracle["cookies"].as_array().unwrap().len());
        assert!(!cookies.contains(&0));
        for cookie in &cookies {
            assert!(all_cookies.insert(*cookie));
        }
        let expected: Vec<Packet> = serde_json::from_value(oracle["expected"].clone()).unwrap();
        let mut selected = bundle.clone();
        selected.facts.retain(|fact| matches!(&fact.kind, FactKind::PacketMeta(p) if p.sk_cookie.is_some_and(|c| cookies.contains(&c))));
        check_packets(&selected, &expected);
        let observed: BTreeSet<_> = selected
            .facts
            .iter()
            .filter_map(|fact| match &fact.kind {
                FactKind::PacketMeta(p) => p.sk_cookie,
                _ => None,
            })
            .collect();
        assert_eq!(observed, cookies, "unobserved socket generation");
        let FactKind::PacketMeta(first) = &selected.facts[0].kind else {
            unreachable!()
        };
        assert!(
            namespaces.insert(first.netns),
            "distinct namespaces were merged"
        );
        assert!(selected.facts.iter().all(|f| {
            f.ifindex
                .is_some_and(|index| index != 0 && oracle["ifindex"] == index)
                && matches!(&f.kind, FactKind::PacketMeta(p) if p.netns == first.netns)
        }));
        facts += selected.facts.len();
    }
    assert_eq!(facts, bundle.facts.len(), "unowned or non-packet facts");
    namespaces.into_iter().collect()
}

#[test]
fn namespace_oracle_rejects_merged_identity_and_foreign_packets() {
    use gewyvern::ledger::{FactId, PacketDir, SessionId};
    use gewyvern::packet_capture::{CapturedUdpDatagram, PacketObservation};
    use gewyvern::runtime::{RuntimeSession, SessionConfig};
    let mut bundle = RuntimeSession::start(
        SessionConfig::for_template(gewyvern::template::udp_debug_template()).unwrap(),
    )
    .unwrap()
    .into_export_bundle();
    bundle.ingest_trust_mode = "unverified-local".into();
    let mut oracles = Vec::new();
    for (role, server) in [false, true].into_iter().enumerate() {
        let mut expected = Vec::new();
        let mut cookies = Vec::new();
        for generation in 0..if server { 4 } else { 2 } {
            let family = generation % 2;
            let cookie = (role * 2 + generation + 1) as u64;
            cookies.push(cookie.to_string());
            let local = endpoint(family, server);
            let remote = endpoint(family, !server);
            for outbound in [false, true] {
                let payload = b"namespace-test";
                expected.push(packet(cookie, local, remote, outbound, payload));
                bundle.facts.push(
                    CapturedUdpDatagram {
                        source: if outbound { local } else { remote },
                        destination: if outbound { remote } else { local },
                        ip_header_len: if family == 0 { 20 } else { 40 },
                        total_len: payload.len() as u32 + if family == 0 { 28 } else { 48 },
                        payload_len: payload.len() as u16,
                        payload_prefix: payload,
                    }
                    .into_fact(PacketObservation {
                        id: FactId(bundle.facts.len() as u64 + 1),
                        session: SessionId(1),
                        ts: UNIX_EPOCH,
                        cpu: 0,
                        ifindex: 2,
                        netns: role as u32 + 1,
                        socket_cookie: cookie,
                        direction: if outbound {
                            PacketDir::Egress
                        } else {
                            PacketDir::Ingress
                        },
                    })
                    .unwrap(),
                );
            }
        }
        oracles.push(json!({"cookies":cookies,"expected":expected,"ifindex":2}));
    }
    assert_eq!(check_namespaces(&bundle, &oracles), vec![1, 2]);
    for mutation in 0..3 {
        let mut bad = bundle.clone();
        for fact in &mut bad.facts {
            let FactKind::PacketMeta(p) = &mut fact.kind else {
                unreachable!()
            };
            match mutation {
                0 => p.netns = 1,
                1 => p.sk_cookie = Some(99),
                _ => fact.ifindex = None,
            }
        }
        assert!(std::panic::catch_unwind(|| check_namespaces(&bad, &oracles)).is_err());
    }
    for mutation in 0..4 {
        let mut bad = oracles.clone();
        match mutation {
            0 => bad[0]["cookies"][1] = bad[0]["cookies"][0].clone(),
            1 => bad[0]["cookies"][0] = json!("0"),
            2 => bad[1]["cookies"]
                .as_array_mut()
                .unwrap()
                .extend([json!("98"), json!("99")]),
            _ => {
                bad[1]["cookies"].as_array_mut().unwrap().pop();
            }
        }
        assert!(std::panic::catch_unwind(|| check_namespaces(&bundle, &bad)).is_err());
    }
}

fn campaign(fragmented: bool, out: &Path) -> Value {
    fs::create_dir(out).unwrap();
    let mut network = Network::new();
    let names = network.0.clone();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scope = PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-netns-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&scope).unwrap();
    let scope = Scope(scope);
    let mut capture = collector(&scope, PORT, out);
    ready(&mut capture, out);
    let mut server = worker(&names[1], "server", &scope, &out.join("server"), fragmented);
    let deadline = Instant::now() + Duration::from_secs(10);
    while !out.join("server/ready").exists() {
        assert!(server.0.try_wait().unwrap().is_none());
        assert!(Instant::now() < deadline);
        thread::sleep(Duration::from_millis(10));
    }
    let mut client = worker(&names[0], "client", &scope, &out.join("client"), fragmented);
    assert!(wait(&mut client, 20).success());
    assert!(wait(&mut server, 5).success());
    signal(&capture, libc::SIGTERM);
    let exit = wait(&mut capture, 10);
    let status = snapshot(out);
    let bundle = export(out);
    let oracles: Vec<Value> = ["client", "server"]
        .into_iter()
        .map(|role| {
            serde_json::from_slice(&fs::read(out.join(role).join("oracle.json")).unwrap()).unwrap()
        })
        .collect();
    assert_ne!(oracles[0]["namespace"], oracles[1]["namespace"]);
    for oracle in &oracles {
        for key in [
            "IpFragCreates",
            "IpReasmOKs",
            "Ip6FragCreates",
            "Ip6ReasmOKs",
        ] {
            let count = oracle["fragment_counters_delta"][key].as_u64().unwrap();
            assert_eq!(count > 0, fragmented, "{key}: {oracle}");
        }
        for key in [
            "IpFragFails",
            "IpReasmFails",
            "Ip6FragFails",
            "Ip6ReasmFails",
        ] {
            assert_eq!(oracle["fragment_counters_delta"][key], 0);
        }
    }
    fs::write(
        out.join("observed.json"),
        serde_json::to_vec_pretty(&json!({
            "capture":status, "exit":exit.code(), "oracles":oracles, "facts":bundle.facts.len()
        }))
        .unwrap(),
    )
    .unwrap();
    assert!(exit.success(), "capture incomplete: {status:?}");
    assert!(
        status.final_accounted && status.reconciles() && !status.incomplete && !status.has_loss()
    );
    assert_eq!(status.state, CaptureState::Stopped);
    let namespaces = check_namespaces(&bundle, &oracles);
    assert_eq!(namespaces.len(), 2);
    assert_eq!(bundle.facts.len(), lengths(fragmented).len() * 8);
    assert_eq!(status.retained as usize, bundle.facts.len());
    assert_eq!(bundle.capture.as_ref(), Some(&status));
    let replay = bundle.replay().unwrap();
    assert_eq!(replay.facts, bundle.facts);
    assert_eq!(replay.capture, bundle.capture);
    assert_eq!(replay.program_flows, bundle.program_flows);
    assert_eq!(replay.attach_report, bundle.attach_report);
    detached(&scope);
    fs::remove_dir(&scope.0).unwrap();
    network.remove();
    for name in names {
        assert!(!Path::new("/run/netns").join(name).exists());
    }
    json!({"ok":true,"fragmentation":fragmented,"mtu":1500,"payload_lengths":lengths(fragmented),
        "capture":status,"facts":bundle.facts.len(),"namespace_ids":namespaces,"oracles":oracles,
        "prefix_and_lengths_match":true,"replay_identical":true,"hooks_detached":true,
        "owned_namespaces_removed":true,"owned_cgroups_removed":true})
}

#[test]
#[ignore = "root, iproute2, BPF tools and GEWY_NETWORK_OUT=new-directory required"]
fn production_capture_network_namespaces() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let out = PathBuf::from(std::env::var_os("GEWY_NETWORK_OUT").expect("new evidence directory"));
    fs::create_dir(&out).unwrap();
    let cases = [
        campaign(false, &out.join("mtu-fit")),
        campaign(true, &out.join("fragmented")),
    ];
    fs::write(out.join("summary.json"), serde_json::to_vec_pretty(&json!({
        "schema_version":1,"ok":true,"production_cli":true,"validation_capture_used":false,
        "binary_sha256":hash(&fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap()),
        "cases":cases,"host_network_changed":false,"owned_resources_removed":true,
        "limits":["single_host_private_veth","prefix_only_above_128","no_raw_fragment_archival", "no_high_load_or_physical_link_claim"]
    })).unwrap()).unwrap();
}
