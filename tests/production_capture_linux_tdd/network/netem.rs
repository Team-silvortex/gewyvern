//! Real kernel queuing/duplication/reordering, with per-socket directional oracles.
use super::faults::{await_file, tc, worker};
use super::*;
use gewyvern::ledger::PacketDir;
use std::io::BufRead;

const PHASES: [&str; 6] = [
    "baseline",
    "delay",
    "request-duplicate",
    "response-duplicate",
    "reorder",
    "recovery",
];
const BURST: u8 = 4;

fn message(phase: usize, family: usize, id: u8) -> Vec<u8> {
    let mut data = bytes(phase, 32);
    data[..3].copy_from_slice(&[phase as u8, family as u8, id]);
    data
}

fn receive_count(phase: &str, server: bool) -> usize {
    usize::from(BURST)
        * if phase == "request-duplicate" || (!server && phase == "response-duplicate") {
            2
        } else {
            1
        }
}

fn verify_delivery(phase: &str, server: bool, ids: &[u8]) {
    let mut sorted = ids.to_vec();
    sorted.sort();
    let copies = receive_count(phase, server) / usize::from(BURST);
    let expected: Vec<_> = (0..BURST)
        .flat_map(|id| std::iter::repeat_n(id, copies))
        .collect();
    assert_eq!(
        sorted, expected,
        "missing, extra or deduplicated {phase} delivery"
    );
    if phase == "reorder" {
        assert!(
            ids.windows(2).any(|pair| pair[0] > pair[1]),
            "no actual reordered delivery"
        );
    } else if copies == 1 {
        assert_eq!(ids, expected);
    }
}

#[test]
fn netem_oracle_rejects_deduplication_and_unproven_reordering() {
    verify_delivery("baseline", false, &[0, 1, 2, 3]);
    verify_delivery("request-duplicate", true, &[0, 0, 1, 1, 2, 2, 3, 3]);
    verify_delivery("response-duplicate", true, &[0, 1, 2, 3]);
    verify_delivery("reorder", false, &[1, 3, 0, 2]);
    for (phase, server, ids) in [
        ("request-duplicate", true, vec![0, 1, 2, 3]),
        ("response-duplicate", false, vec![0, 1, 2, 3]),
        ("reorder", true, vec![0, 1, 2, 3]),
        ("baseline", false, vec![0, 1, 2, 4]),
    ] {
        assert!(std::panic::catch_unwind(|| verify_delivery(phase, server, &ids)).is_err());
    }
}

fn install(namespace: &str, port: u16, phase: &str) {
    tc(
        namespace,
        &[
            "qdisc", "add", "dev", "peer0", "root", "handle", "1:", "prio", "bands", "3",
            "priomap", "0", "0", "0", "0", "0", "0", "0", "0", "0", "0", "0", "0", "0", "0", "0",
            "0",
        ],
    );
    let mut args = vec![
        "qdisc", "add", "dev", "peer0", "parent", "1:3", "handle", "30:", "netem", "limit", "128",
    ];
    match phase {
        "delay" => args.extend(["delay", "120ms"]),
        "request-duplicate" | "response-duplicate" => args.extend(["duplicate", "100%"]),
        "reorder" => args.extend(["delay", "120ms", "reorder", "100%", "gap", "2"]),
        _ => panic!("unknown netem phase"),
    }
    tc(namespace, &args);
    for (protocol, priority) in [("ip", "10"), ("ipv6", "20")] {
        tc(
            namespace,
            &[
                "filter",
                "add",
                "dev",
                "peer0",
                "parent",
                "1:",
                "protocol",
                protocol,
                "pref",
                priority,
                "flower",
                "ip_proto",
                "udp",
                "dst_port",
                &port.to_string(),
                "classid",
                "1:3",
            ],
        );
    }
}

#[test]
#[ignore = "internal native burst worker for isolated netem test"]
fn udp_netem_worker() {
    let out = PathBuf::from(std::env::var_os("GW_NET_OUT").unwrap());
    let server = std::env::var("GW_NET_ROLE").unwrap() == "server";
    let ifindex = enter_worker_network();
    let (sockets, cookies) = network_sockets(server);
    let namespace = fs::read_link("/proc/thread-self/ns/net").unwrap();
    fs::write(out.join("ready"), b"ready").unwrap();
    let input = std::io::stdin();
    let mut commands = input.lock().lines();
    let mut expected = Vec::new();
    let mut phases = Vec::new();
    for (phase_index, phase) in PHASES.into_iter().enumerate() {
        assert_eq!(commands.next().unwrap().unwrap(), phase);
        let mut observations = Vec::new();
        for (family, socket) in sockets.iter().enumerate() {
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let local = endpoint(family, server);
            let remote = endpoint(family, !server);
            let cookie = cookies[family].parse().unwrap();
            let mut sends = Vec::new();
            let mut receives = Vec::new();
            let mut elapsed = Vec::new();
            let start = Instant::now();
            if !server {
                for id in 0..BURST {
                    let data = message(phase_index, family, id);
                    assert_eq!(socket.send_to(&data, remote).unwrap(), data.len());
                    expected.push(packet(cookie, local, remote, true, &data));
                    sends.push(id);
                }
            }
            for _ in 0..receive_count(phase, server) {
                let mut buffer = [0; 512];
                let (n, source) = socket.recv_from(&mut buffer).unwrap();
                assert_eq!(source, remote);
                assert_eq!(n, 32);
                let id = buffer[2];
                assert!(id < BURST);
                let data = message(phase_index, family, id);
                assert_eq!(&buffer[..n], &data);
                receives.push(id);
                elapsed.push(start.elapsed().as_secs_f64() * 1000.0);
                expected.push(packet(cookie, local, remote, false, &data));
                if server {
                    assert_eq!(socket.send_to(&data, remote).unwrap(), data.len());
                    expected.push(packet(cookie, local, remote, true, &data));
                    sends.push(id);
                }
            }
            verify_delivery(phase, server, &receives);
            if !server && phase == "delay" {
                assert!(elapsed[0] >= 100.0, "delay not observed: {elapsed:?}");
            }
            socket
                .set_read_timeout(Some(Duration::from_millis(40)))
                .unwrap();
            let error = socket
                .recv_from(&mut [0; 512])
                .expect_err("unexpected extra datagram");
            assert!(matches!(
                error.kind(),
                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
            ));
            observations.push(
                json!({"family":if family==0 {4}else{6},"cookie":cookies[family],
                "sent_ids":sends,"received_ids":receives,"receive_elapsed_ms":elapsed,
                "length":32,"extra_datagram_observed":false}),
            );
        }
        let result = json!({"phase":phase,"observations":observations});
        let pending = out.join(format!("{phase}.pending"));
        fs::write(&pending, serde_json::to_vec_pretty(&result).unwrap()).unwrap();
        fs::rename(&pending, out.join(format!("{phase}.json"))).unwrap();
        phases.push(result);
    }
    fs::write(
        out.join("oracle.json"),
        serde_json::to_vec_pretty(&json!({
            "role":if server {"server"}else{"client"},"namespace":namespace,"ifindex":ifindex,
            "cookies":cookies,"expected":expected,"phases":phases,"same_sockets_all_phases":true
        }))
        .unwrap(),
    )
    .unwrap();
}

fn check_stream_order(bundle: &ExportBundle, oracles: &[Value]) {
    for oracle in oracles {
        for (index, phase) in oracle["phases"].as_array().unwrap().iter().enumerate() {
            for (family, o) in phase["observations"].as_array().unwrap().iter().enumerate() {
                let cookie = o["cookie"].as_str().unwrap().parse().unwrap();
                for (direction, field) in [
                    (PacketDir::Ingress, "received_ids"),
                    (PacketDir::Egress, "sent_ids"),
                ] {
                    let actual: Vec<_> = bundle
                        .facts
                        .iter()
                        .filter_map(|fact| {
                            let FactKind::PacketMeta(p) = &fact.kind else {
                                return None;
                            };
                            if p.sk_cookie == Some(cookie)
                                && p.dir == direction
                                && p.payload_bytes.get(&0) == Some(&(index as u8))
                                && p.payload_bytes.get(&1) == Some(&(family as u8))
                            {
                                Some(p.payload_bytes[&2])
                            } else {
                                None
                            }
                        })
                        .collect();
                    let expected: Vec<u8> = serde_json::from_value(o[field].clone()).unwrap();
                    assert_eq!(
                        actual, expected,
                        "per-socket directional order differs from native I/O"
                    );
                }
            }
        }
    }
}

#[test]
#[ignore = "root, iproute2/netem, BPF tools and GEWY_NETEM_OUT=new-directory required"]
fn production_capture_network_netem() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let out = PathBuf::from(std::env::var_os("GEWY_NETEM_OUT").expect("new evidence directory"));
    fs::create_dir(&out).unwrap();
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
    let mut capture = collector(&scope, PORT, &out);
    ready(&mut capture, &out);
    let selector = "network::netem::udp_netem_worker";
    let mut server = worker(&names[1], "server", &scope, &out.join("server"), selector);
    let mut client = worker(&names[0], "client", &scope, &out.join("client"), selector);
    await_file(&mut server, &out.join("server/ready"));
    await_file(&mut client, &out.join("client/ready"));
    let mut injections = Vec::new();
    for phase in PHASES {
        let injection = match phase {
            "delay" | "response-duplicate" => Some((&names[1], PORT - 1)),
            "request-duplicate" | "reorder" => Some((&names[0], PORT)),
            _ => None,
        };
        if let Some((namespace, port)) = injection {
            install(namespace, port, phase);
        }
        for process in [&mut server, &mut client] {
            writeln!(process.0.stdin.as_mut().unwrap(), "{phase}").unwrap();
        }
        await_file(
            &mut server,
            &out.join("server").join(format!("{phase}.json")),
        );
        await_file(
            &mut client,
            &out.join("client").join(format!("{phase}.json")),
        );
        if let Some((namespace, port)) = injection {
            let queues = tc(namespace, &["-s", "qdisc", "show", "dev", "peer0"]);
            let filters = tc(
                namespace,
                &["filter", "show", "dev", "peer0", "parent", "1:"],
            );
            fs::write(
                out.join(format!("{phase}-tc.json")),
                serde_json::to_vec_pretty(&json!({"queues":queues,"filters":filters})).unwrap(),
            )
            .unwrap();
            let netem: Vec<_> = queues
                .as_array()
                .unwrap()
                .iter()
                .filter(|q| q["kind"] == "netem")
                .collect();
            assert_eq!(netem.len(), 1);
            assert_eq!(netem[0]["handle"], "30:");
            assert_eq!(netem[0]["parent"], "1:3");
            assert_eq!(netem[0]["drops"], 0);
            assert_eq!(netem[0]["qlen"], 0);
            assert_eq!(netem[0]["options"]["limit"], 128);
            if phase.contains("duplicate") {
                assert_eq!(netem[0]["options"]["duplicate"]["duplicate"], 1);
            } else {
                assert_eq!(netem[0]["options"]["delay"]["delay"], 0.12);
                assert_eq!(netem[0]["options"]["delay"]["jitter"], 0);
            }
            if phase == "reorder" {
                assert_eq!(netem[0]["options"]["gap"], 2);
                assert_eq!(netem[0]["options"]["reorder"]["reorder"], 1);
            }
            let mut protocols = BTreeSet::new();
            for filter in filters.as_array().unwrap() {
                if filter.get("options").is_none() {
                    continue;
                }
                assert_eq!(filter["kind"], "flower");
                assert!(protocols.insert(filter["protocol"].as_str().unwrap()));
                assert_eq!(filter["options"]["classid"], "1:3");
                assert_eq!(filter["options"]["keys"]["ip_proto"], "udp");
                assert_eq!(filter["options"]["keys"]["dst_port"], port);
            }
            assert_eq!(protocols, BTreeSet::from(["ip", "ipv6"]));
            assert_eq!(
                netem[0]["packets"],
                if phase.contains("duplicate") { 16 } else { 8 }
            );
            tc(namespace, &["qdisc", "del", "dev", "peer0", "root"]);
            assert!(
                tc(namespace, &["qdisc", "show", "dev", "peer0"])
                    .as_array()
                    .unwrap()
                    .iter()
                    .all(|q| q["kind"] != "netem" && q["kind"] != "prio")
            );
            injections.push(json!({"phase":phase,"destination_port":port,"queues":queues,"filters":filters,"rules_removed":true}));
        }
    }
    assert!(wait(&mut client, 5).success());
    assert!(wait(&mut server, 5).success());
    signal(&capture, libc::SIGTERM);
    assert!(wait(&mut capture, 10).success());
    let status = snapshot(&out);
    assert!(
        status.final_accounted && status.reconciles() && !status.incomplete && !status.has_loss()
    );
    assert_eq!(status.state, CaptureState::Stopped);
    assert_eq!(status.retained, 224);
    let bundle = export(&out);
    let oracles: Vec<Value> = ["client", "server"]
        .into_iter()
        .map(|role| {
            serde_json::from_slice(&fs::read(out.join(role).join("oracle.json")).unwrap()).unwrap()
        })
        .collect();
    assert_ne!(oracles[0]["namespace"], oracles[1]["namespace"]);
    let namespaces = check_namespaces(&bundle, &oracles);
    check_stream_order(&bundle, &oracles);
    assert_eq!(bundle.facts.len(), 224);
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
    fs::write(out.join("summary.json"), serde_json::to_vec_pretty(&json!({
        "schema_version":1,"ok":true,"production_cli":true,"validation_capture_used":false,
        "binary_sha256":hash(&fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap()),
        "capture":status,"oracles":oracles,"injections":injections,"namespace_ids":namespaces,
        "selected_requests":48,"client_replies":64,"facts":bundle.facts.len(),
        "same_socket_recovery":true,"packet_multiset_matches":true,"directional_order_matches":true,
        "replay_identical":true,"hooks_detached":true,"owned_resources_removed":true,
        "host_network_changed":false,"product_cause_classifier_claim":false,
        "limits":["single_host_private_veth","small_bounded_bursts","not_global_wire_order",
            "not_rtt_benchmark_or_automatic_root_cause","not_random_jitter_or_high_load"]
    })).unwrap()).unwrap();
}
