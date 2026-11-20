//! Deterministic, selected-port TC loss outside the product capture hooks.
use super::*;
use std::io::BufRead;

const PHASES: [&str; 5] = [
    "baseline",
    "request-loss",
    "request-recovery",
    "response-loss",
    "response-recovery",
];

pub(super) fn tc(namespace: &str, args: &[&str]) -> Value {
    let output = Command::new("ip")
        .args(["netns", "exec", namespace, "tc", "-j"])
        .args(args)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "tc {args:?}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    if output.stdout.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&output.stdout).unwrap()
    }
}

fn install_loss(namespace: &str, destination: u16) {
    tc(namespace, &["qdisc", "add", "dev", "peer0", "clsact"]);
    for (protocol, priority) in [("ip", "10"), ("ipv6", "20")] {
        tc(
            namespace,
            &[
                "filter",
                "add",
                "dev",
                "peer0",
                "egress",
                "protocol",
                protocol,
                "pref",
                priority,
                "flower",
                "ip_proto",
                "udp",
                "dst_port",
                &destination.to_string(),
                "action",
                "drop",
            ],
        );
    }
}

fn verify_drop_counters(filters: &Value, destination: u16) {
    let mut protocols = BTreeSet::new();
    let mut actions = 0;
    for filter in filters.as_array().unwrap() {
        let Some(entries) = filter.pointer("/options/actions").and_then(Value::as_array) else {
            continue;
        };
        assert_eq!(filter["kind"], "flower");
        assert!(protocols.insert(filter["protocol"].as_str().unwrap()));
        assert_eq!(filter["options"]["keys"]["ip_proto"], "udp");
        assert_eq!(filter["options"]["keys"]["dst_port"], destination);
        assert_eq!(entries.len(), 1);
        for action in entries {
            assert_eq!(action["kind"], "gact");
            assert_eq!(action["control_action"]["type"], "drop");
            assert_eq!(action["stats"]["packets"], 1);
            assert_eq!(action["stats"]["drops"], 1);
            actions += 1;
        }
    }
    assert_eq!(protocols, BTreeSet::from(["ip", "ipv6"]));
    assert_eq!(actions, 2);
}

#[test]
fn drop_oracle_rejects_missing_or_unhit_filters() {
    let filters = json!(["ip", "ipv6"].map(
        |protocol| json!({"kind":"flower", "protocol":protocol,
        "options":{"keys":{"ip_proto":"udp","dst_port":PORT},
            "actions":[{"kind":"gact", "control_action":{"type":"drop"},
            "stats":{"packets":1,"drops":1}}]}})
    ));
    verify_drop_counters(&filters, PORT);
    for variant in 0..5 {
        let mut bad = filters.clone();
        match variant {
            0 => bad[0]["options"]["actions"][0]["stats"]["packets"] = json!(0),
            1 => bad[1]["protocol"] = json!("ip"),
            2 => bad[0]["options"]["actions"][0]["control_action"]["type"] = json!("pass"),
            3 => bad[0]["options"]["keys"]["dst_port"] = json!(PORT + 1),
            _ => {
                bad.as_array_mut().unwrap().pop();
            }
        }
        assert!(std::panic::catch_unwind(|| verify_drop_counters(&bad, PORT)).is_err());
    }
}

#[test]
#[ignore = "internal persistent UDP worker for isolated network-loss test"]
fn udp_loss_worker() {
    let out = PathBuf::from(std::env::var_os("GW_NET_OUT").unwrap());
    let server = std::env::var("GW_NET_ROLE").unwrap() == "server";
    let ifindex = enter_worker_network();
    let (sockets, cookies) = network_sockets(server);
    let namespace = fs::read_link("/proc/thread-self/ns/net").unwrap();
    fs::write(out.join("ready"), b"ready").unwrap();
    let mut expected = Vec::new();
    let mut phases = Vec::new();
    let input = std::io::stdin();
    let mut commands = input.lock().lines();
    for (index, phase) in PHASES.into_iter().enumerate() {
        assert_eq!(commands.next().unwrap().unwrap(), phase);
        let mut observations = Vec::new();
        for (family, socket) in sockets.iter().enumerate() {
            let local = endpoint(family, server);
            let remote = endpoint(family, !server);
            let data = bytes(index, 32);
            let cookie = cookies[family].parse().unwrap();
            if !server {
                assert_eq!(socket.send_to(&data, remote).unwrap(), data.len());
                expected.push(packet(cookie, local, remote, true, &data));
            }
            let timeout = phase == "request-loss" || (!server && phase == "response-loss");
            // Fresh neighbors may need ARP/NDP resolution before baseline I/O.
            // Keep the short deadline only for deliberately dropped deliveries.
            socket
                .set_read_timeout(Some(if timeout {
                    Duration::from_millis(350)
                } else {
                    Duration::from_secs(3)
                }))
                .unwrap();
            let mut buffer = [0; 512];
            let start = Instant::now();
            let received = match socket.recv_from(&mut buffer) {
                Ok((n, source)) => {
                    assert!(!timeout, "unexpected delivery in {phase}");
                    assert_eq!(source, remote);
                    assert_eq!(&buffer[..n], &data);
                    expected.push(packet(cookie, local, remote, false, &buffer[..n]));
                    if server {
                        assert_eq!(socket.send_to(&data, remote).unwrap(), data.len());
                        expected.push(packet(cookie, local, remote, true, &data));
                    }
                    true
                }
                Err(error) => {
                    assert!(
                        timeout
                            && matches!(
                                error.kind(),
                                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                            ),
                        "unexpected receive failure in {phase}: {error}"
                    );
                    false
                }
            };
            observations.push(
                json!({"family":if family==0 {4}else{6},"cookie":cookies[family],
                "local_port":local.port(),"remote_port":remote.port(),"length":data.len(),
                "payload_sha256":hash(&data),"received":received,"timeout":timeout,
                "sent":!server || received,"elapsed_ms":start.elapsed().as_secs_f64()*1000.0}),
            );
        }
        let result = json!({"phase":phase,"observations":observations});
        // Publish completion only after the whole phase; the controller removes
        // the loss rule before releasing either worker into the next phase.
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

pub(super) fn worker(
    namespace: &str,
    role: &str,
    scope: &Scope,
    out: &Path,
    selector: &str,
) -> Process {
    fs::create_dir(out).unwrap();
    Process(
        Command::new(std::env::current_exe().unwrap())
            .args(["--ignored", "--exact", selector, "--nocapture"])
            .env("GW_NET_NAMESPACE", namespace)
            .env("GW_NET_ROLE", role)
            .env("GW_NET_SCOPE", &scope.0)
            .env("GW_NET_OUT", out)
            .stdin(Stdio::piped())
            .stdout(fs::File::create(out.join("worker.log")).unwrap())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}

pub(super) fn await_file(process: &mut Process, path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !path.exists() {
        assert!(
            process.0.try_wait().unwrap().is_none(),
            "worker exited before {}",
            path.display()
        );
        assert!(
            Instant::now() < deadline,
            "worker phase timeout: {}",
            path.display()
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
#[ignore = "root, iproute2/tc, BPF tools and GEWY_NETWORK_LOSS_OUT=new-directory required"]
fn production_capture_network_loss() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let out =
        PathBuf::from(std::env::var_os("GEWY_NETWORK_LOSS_OUT").expect("new evidence directory"));
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
    let mut server = worker(
        &names[1],
        "server",
        &scope,
        &out.join("server"),
        "network::faults::udp_loss_worker",
    );
    let mut client = worker(
        &names[0],
        "client",
        &scope,
        &out.join("client"),
        "network::faults::udp_loss_worker",
    );
    await_file(&mut server, &out.join("server/ready"));
    await_file(&mut client, &out.join("client/ready"));
    let mut injections = Vec::new();
    for phase in PHASES {
        let injection = match phase {
            "request-loss" => Some((&names[0], PORT)),
            "response-loss" => Some((&names[1], PORT - 1)),
            _ => None,
        };
        if let Some((namespace, port)) = injection {
            install_loss(namespace, port);
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
            let filters = tc(
                namespace,
                &["-s", "filter", "show", "dev", "peer0", "egress"],
            );
            fs::write(
                out.join(format!("{phase}-tc.json")),
                serde_json::to_vec_pretty(&filters).unwrap(),
            )
            .unwrap();
            verify_drop_counters(&filters, port);
            tc(namespace, &["qdisc", "del", "dev", "peer0", "clsact"]);
            assert!(
                tc(namespace, &["filter", "show", "dev", "peer0", "egress"])
                    .as_array()
                    .unwrap()
                    .is_empty()
            );
            injections.push(
                json!({"phase":phase,"namespace":namespace,"destination_port":port,
                "filters":filters,"rules_removed":true}),
            );
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
    assert_eq!(status.retained, 32);
    let bundle = export(&out);
    let oracles: Vec<Value> = ["client", "server"]
        .into_iter()
        .map(|role| {
            serde_json::from_slice(&fs::read(out.join(role).join("oracle.json")).unwrap()).unwrap()
        })
        .collect();
    assert_ne!(oracles[0]["namespace"], oracles[1]["namespace"]);
    let namespaces = check_namespaces(&bundle, &oracles);
    assert_eq!(namespaces.len(), 2);
    assert_eq!(bundle.facts.len(), 32);
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
        "selected_requests":10,"successful_exchanges":6,"client_timeouts":4,"tc_drops":4,
        "facts":bundle.facts.len(),"same_socket_recovery":true,"packet_multiset_matches":true,
        "replay_identical":true,"hooks_detached":true,"owned_resources_removed":true,
        "host_network_changed":false,"product_cause_classifier_claim":false,
        "limits":["single_host_private_veth","selected_udp_only","not_random_loss_or_reordering",
            "network_loss_is_not_capture_loss","no_automatic_root_cause_or_remote_delivery_proof"]
    })).unwrap()).unwrap();
}
