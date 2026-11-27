//! A live, incomplete group must expire without more traffic or process exit.
use super::*;

#[path = "shutdown.rs"]
mod shutdown;

fn phase_count(phase: usize) -> usize {
    match phase {
        0..=3 => 1,
        4..=6 => 16,
        7 => 4,
        _ => panic!("unexpected phase"),
    }
}

fn family() -> usize {
    match std::env::var("GEWY_OFFLOAD_EXPIRY_FAMILY")
        .unwrap()
        .as_str()
    {
        "4" => 0,
        "6" => 1,
        _ => panic!("explicit IPv4/IPv6 family required"),
    }
}

fn phase_parts(phase: usize, family: usize) -> Vec<Vec<u8>> {
    let count = phase_count(phase);
    (0..count)
        .map(|index| {
            payload(
                phase,
                family,
                index,
                if count == 1 {
                    32
                } else if index + 1 == count {
                    17
                } else {
                    SEGMENT
                },
            )
        })
        .collect()
}

#[test]
#[ignore = "internal native offload expiry worker"]
fn udp_offload_expiry_worker() {
    let out = PathBuf::from(std::env::var_os("GW_NET_OUT").unwrap());
    let server = std::env::var("GW_NET_ROLE").unwrap() == "server";
    let family = family();
    let ifindex = enter_worker_network();
    let before = fragment_stats();
    let (sockets, cookies) = sockets(server, 0);
    let socket = &sockets[family];
    let cookie: u64 = cookies[family].parse().unwrap();
    let identity = json!({"role":if server {"server"} else {"client"},
        "family":if family==0 {4} else {6},"cookie":cookie.to_string(),"ifindex":ifindex,
        "namespace":fs::read_link("/proc/thread-self/ns/net").unwrap()});
    fs::write(out.join("ready"), b"ready").unwrap();
    let mut phases = Vec::new();
    for command in std::io::stdin().lock().lines() {
        let command = command.unwrap();
        if command == "finish" {
            break;
        }
        let phase: usize = command.parse().unwrap();
        assert_eq!(phase, phases.len());
        let offload = phase_count(phase) > 1;
        option(
            socket,
            libc::UDP_SEGMENT,
            if !server && offload {
                SEGMENT as i32
            } else {
                0
            },
        );
        option(socket, libc::UDP_GRO, i32::from(server && offload));
        if server {
            fs::write(out.join(format!("phase-{phase}.ready")), b"ready").unwrap();
        }
        let parts = phase_parts(phase, family);
        let request = parts.concat();
        let reply = payload(phase, family, 0, 32);
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let received = if server {
            let received = receive(socket).unwrap();
            verify_receives(offload, &parts, std::slice::from_ref(&received));
            assert_eq!(socket.send(&reply).unwrap(), reply.len());
            received
        } else {
            assert_eq!(socket.send(&request).unwrap(), request.len());
            let received = receive(socket).unwrap();
            assert_eq!(received, (reply.clone(), None));
            received
        };
        let local = socket.local_addr().unwrap();
        let remote = socket.peer_addr().unwrap();
        let mut expected: Vec<_> = parts
            .iter()
            .map(|part| packet(cookie, local, remote, !server, part))
            .collect();
        expected.push(packet(cookie, local, remote, server, &reply));
        socket
            .set_read_timeout(Some(Duration::from_millis(10)))
            .unwrap();
        assert!(matches!(
            receive(socket).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
        ));
        let row = json!({"phase":phase,"segment_count":parts.len(),
            "request_lengths":parts.iter().map(Vec::len).collect::<Vec<_>>(),
            "request_sha256":hash(&request),"reply_sha256":hash(&reply),
            "received_length":received.0.len(),"received_sha256":hash(&received.0),
            "gro_segment":received.1,"extra_datagram_observed":false});
        let mut phase_oracle = row.clone();
        phase_oracle["expected"] = json!(expected);
        let pending = out.join("pending.json");
        fs::write(&pending, serde_json::to_vec_pretty(&phase_oracle).unwrap()).unwrap();
        fs::rename(pending, out.join(format!("phase-{phase}.json"))).unwrap();
        phases.push(row);
    }
    assert_eq!(phases.len(), 8);
    let after = fragment_stats();
    let deltas: BTreeMap<_, _> = after
        .iter()
        .map(|(key, value)| (key.clone(), value - before[key]))
        .collect();
    assert!(deltas.values().all(|n| *n == 0));
    fs::write(
        out.join("oracle.json"),
        serde_json::to_vec_pretty(&json!({"identity":identity,
        "phases":phases,"fragment_counter_deltas":deltas,"same_socket_all_phases":true}))
        .unwrap(),
    )
    .unwrap();
}

fn phase(server: &mut Process, client: &mut Process, out: &Path, phase: usize) {
    writeln!(server.0.stdin.as_mut().unwrap(), "{phase}").unwrap();
    await_file(server, &out.join(format!("server/phase-{phase}.ready")));
    writeln!(client.0.stdin.as_mut().unwrap(), "{phase}").unwrap();
    await_file(client, &out.join(format!("client/phase-{phase}.json")));
    await_file(server, &out.join(format!("server/phase-{phase}.json")));
}

fn stage(
    capture: &mut Process,
    out: &Path,
    run: &str,
    label: &str,
    counts: [u64; 5],
) -> ExportBundle {
    let [matched, received, retained, dropped, rejected] = counts;
    let deadline = Instant::now() + Duration::from_secs(8);
    loop {
        assert!(capture.0.try_wait().unwrap().is_none(), "collector exited");
        let text = fs::read_to_string(out.join("export.json")).unwrap();
        let bundle = ExportBundle::from_json(&text).unwrap();
        let s = bundle.capture.as_ref().unwrap();
        if s.run_id == run && s.received == received && s.decode_rejected == rejected {
            assert_eq!(s.state, CaptureState::Running);
            assert!(!s.final_accounted && s.incomplete);
            assert_eq!(s.kernel.matched, matched);
            assert_eq!(s.retained, retained);
            assert_eq!(s.userspace_dropped, dropped);
            assert_eq!(s.kernel.ring_dropped, 8);
            assert_eq!(
                s.kernel.unsupported + s.kernel.malformed + s.kernel.no_cookie,
                0
            );
            let replay = bundle.replay().unwrap();
            assert_eq!(replay.facts, bundle.facts);
            assert_eq!(replay.capture, bundle.capture);
            assert_eq!(replay.program_flows, bundle.program_flows);
            fs::write(out.join(format!("stage-{label}.json")), text).unwrap();
            return bundle;
        }
        assert!(Instant::now() < deadline, "missing stage {label}");
        thread::sleep(Duration::from_millis(2));
    }
}

struct PartialRun {
    capture: Process,
    server: Process,
    client: Process,
    scope: Scope,
    server_scope: Scope,
    network: Network,
    out: PathBuf,
    capture_dir: PathBuf,
    partial: ExportBundle,
    resumed: Instant,
}

const OPTIONS: &[&str] = &[
    "--serve",
    "--capture-window-ms",
    "200",
    "--capture-max-facts",
    "24",
    "--capture-ring-kib",
    "4",
];

fn start_partial(output_var: &str) -> PartialRun {
    start_partial_with_options(output_var, OPTIONS)
}

fn start_partial_with_options(output_var: &str, options: &[&str]) -> PartialRun {
    start_partial_using(output_var, |scope, dir| {
        collector_with_options(scope, PORT, dir, options)
    })
}

fn start_partial_using(
    output_var: &str,
    spawn: impl FnOnce(&Scope, &Path) -> Process,
) -> PartialRun {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let out = PathBuf::from(std::env::var_os(output_var).unwrap());
    fs::create_dir(&out).unwrap();
    let network = Network::new();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let prefix = format!(
        "/sys/fs/cgroup/gewyvern-production-test-netns-expiry-{}-{nonce}",
        std::process::id()
    );
    let scope = Scope(PathBuf::from(format!("{prefix}-client")));
    let server_scope = Scope(PathBuf::from(format!("{prefix}-server")));
    for scope in [&scope, &server_scope] {
        fs::create_dir(&scope.0).unwrap();
    }
    let capture_dir = out.join("capture");
    fs::create_dir(&capture_dir).unwrap();
    let mut capture = spawn(&scope, &capture_dir);
    ready(&mut capture, &capture_dir);
    let run_id = snapshot(&capture_dir).run_id;
    pause_idle(&mut capture, &capture_dir, &run_id);
    let selector = "network::offload::service::expiry::udp_offload_expiry_worker";
    let mut server = worker(
        &network.0[1],
        "server",
        &server_scope,
        &out.join("server"),
        selector,
    );
    let mut client = worker(
        &network.0[0],
        "client",
        &scope,
        &out.join("client"),
        selector,
    );
    await_file(&mut server, &out.join("server/ready"));
    await_file(&mut client, &out.join("client/ready"));
    for id in 0..5 {
        phase(&mut server, &mut client, &out, id);
    }
    thread::sleep(Duration::from_millis(250));
    let resumed = Instant::now();
    signal(&capture, libc::SIGCONT);
    let partial = stage(
        &mut capture,
        &capture_dir,
        &run_id,
        "partial",
        [25, 17, 8, 0, 0],
    );
    assert_eq!(partial.facts.len(), 8);
    PartialRun {
        capture,
        server,
        client,
        scope,
        server_scope,
        network,
        out,
        capture_dir,
        partial,
        resumed,
    }
}

#[test]
#[ignore = "root/BPF/iproute2; GEWY_OFFLOAD_EXPIRY_FAMILY=4|6 and GEWY_OFFLOAD_EXPIRY_OUT=new-directory"]
fn production_capture_offload_idle_expiry_and_capacity_recovery() {
    let family = family();
    let PartialRun {
        mut capture,
        mut server,
        mut client,
        scope,
        server_scope,
        mut network,
        out,
        capture_dir,
        partial,
        ..
    } = start_partial("GEWY_OFFLOAD_EXPIRY_OUT");
    let observed_partial = Instant::now();
    let run_id = partial.capture.as_ref().unwrap().run_id.clone();
    phase(&mut server, &mut client, &out, 5);
    let blocked = stage(
        &mut capture,
        &capture_dir,
        &run_id,
        "reserved",
        [42, 34, 9, 16, 0],
    );
    assert_eq!(blocked.facts.len(), 1);
    // No traffic or stop signal here: the active service must expire on idle polls.
    let expired = stage(
        &mut capture,
        &capture_dir,
        &run_id,
        "expired",
        [42, 34, 9, 16, 9],
    );
    let expiry_elapsed = observed_partial.elapsed();
    assert!(expiry_elapsed >= Duration::from_secs(1) && expiry_elapsed < Duration::from_secs(8));
    assert!(expired.facts.is_empty());
    phase(&mut server, &mut client, &out, 6);
    let admitted = stage(
        &mut capture,
        &capture_dir,
        &run_id,
        "admitted",
        [59, 51, 26, 16, 9],
    );
    assert_eq!(admitted.facts.len(), 17);
    phase(&mut server, &mut client, &out, 7);
    let recovery = stage(
        &mut capture,
        &capture_dir,
        &run_id,
        "recovery",
        [64, 56, 31, 16, 9],
    );
    assert_eq!(recovery.facts.len(), 5);
    signal(&capture, libc::SIGTERM);
    assert_eq!(wait(&mut capture, 10).code(), Some(3));
    let terminal = snapshot(&capture_dir);
    assert!(terminal.final_accounted && terminal.reconciles() && terminal.incomplete);
    assert_eq!(terminal.state, CaptureState::Stopped);
    let mut expected_terminal = recovery.capture.clone().unwrap();
    expected_terminal.state = CaptureState::Stopped;
    expected_terminal.final_accounted = true;
    assert_eq!(terminal, expected_terminal);
    assert!(export(&capture_dir).facts.is_empty());
    detached(&scope);
    detached(&server_scope);
    for process in [&mut server, &mut client] {
        writeln!(process.0.stdin.as_mut().unwrap(), "finish").unwrap();
        assert!(wait(process, 5).success());
    }
    let native: Vec<Value> = ["client", "server"]
        .iter()
        .map(|role| {
            serde_json::from_slice(&fs::read(out.join(role).join("oracle.json")).unwrap()).unwrap()
        })
        .collect();
    let mut expected = Vec::<Value>::new();
    for phase in [0, 1, 2, 3, 5, 6, 7] {
        let row: Value = serde_json::from_slice(
            &fs::read(out.join(format!("client/phase-{phase}.json"))).unwrap(),
        )
        .unwrap();
        for packet in row["expected"].as_array().unwrap() {
            if phase != 5 || packet["outbound"] == false {
                expected.push(packet.clone());
            }
        }
    }
    let mut combined = partial.clone();
    combined.facts = [&partial, &blocked, &expired, &admitted, &recovery]
        .iter()
        .flat_map(|b| b.facts.iter().cloned())
        .collect();
    let expected_packets: Vec<Packet> = serde_json::from_value(json!(expected)).unwrap();
    check_packets(&combined, &expected_packets);
    let mut ids = BTreeSet::new();
    let mut groups = BTreeMap::<u64, Vec<u16>>::new();
    for fact in &combined.facts {
        assert!(ids.insert(fact.id));
        assert_eq!(
            json!(fact.ifindex.unwrap()),
            native[0]["identity"]["ifindex"]
        );
        let FactKind::PacketMeta(p) = &fact.kind else {
            panic!("non-packet")
        };
        assert_eq!(
            json!(p.sk_cookie.unwrap().to_string()),
            native[0]["identity"]["cookie"]
        );
        if let Some(meta) = p.udp_offload {
            assert!(meta.matches_packet(p));
            assert!([3, 4].contains(&meta.aggregate_id));
            let phase = if meta.aggregate_id == 3 { 6 } else { 7 };
            assert_eq!(usize::from(meta.segment_count), phase_count(phase));
            assert_eq!(
                meta.aggregate_payload_len as usize,
                phase_parts(phase, family).concat().len()
            );
            assert_eq!(meta.segment_size, SEGMENT as u16);
            let data = &phase_parts(phase, family)[usize::from(meta.segment_index)];
            for (offset, byte) in data.iter().take(128).enumerate() {
                assert_eq!(p.payload_bytes[&(offset as u16)], *byte);
            }
            groups
                .entry(meta.aggregate_id)
                .or_default()
                .push(meta.segment_index);
        }
    }
    assert_eq!(groups.len(), 2);
    for (id, n) in [(3, 16), (4, 4)] {
        groups.get_mut(&id).unwrap().sort_unstable();
        assert_eq!(groups[&id], (0..n).collect::<Vec<_>>());
    }
    for scope in [&scope, &server_scope] {
        fs::remove_dir(&scope.0).unwrap();
    }
    network.remove();
    let stages: Vec<_> = [
        (&partial, "partial"),
        (&blocked, "reserved"),
        (&expired, "expired"),
        (&admitted, "admitted"),
        (&recovery, "recovery"),
    ]
    .into_iter()
    .map(|(bundle, name)| json!({"stage":name,"capture":bundle.capture,"facts":bundle.facts.len()}))
    .collect();
    fs::write(out.join("summary.json"), serde_json::to_vec_pretty(&json!({
        "schema_version":1,"ok":true,"production_cli":true,"family":if family==0 {4} else {6},
        "stages":stages,"terminal":terminal,"native":native,
        "expiry_elapsed_ms":expiry_elapsed.as_secs_f64()*1000.0,
        "expected_count":expected.len(),"expected_sha256":hash(&serde_json::to_vec(&expected).unwrap()),
        "group_indexes":groups,"native_requests":56,"native_replies":8,
        "expired_without_new_traffic":true,"capacity_recovered":true,"same_socket_recovery":true,
        "packet_multiset_matches":true,"replay_identical":true,"hooks_detached":true,"owned_resources_removed":true,
        "host_network_changed":false,"binary_sha256":hash(&fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap()),
        "limits":["single_host_private_veth","client_scope_only","bounded_idle_expiry_not_soak","no_physical_nic_certification"]
    })).unwrap()).unwrap();
}
