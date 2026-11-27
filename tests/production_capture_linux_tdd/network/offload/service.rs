//! Real groups straddling consume_n(512), bounded windows and process lifetimes.
use super::*;
use crate::boundaries::collector_with_options;

#[path = "concurrent.rs"]
mod concurrent;
#[path = "expiry.rs"]
mod expiry;
#[path = "history.rs"]
mod history;

fn count(round: usize) -> usize {
    assert!(round < 12);
    if round < 8 { 16 } else { 4 }
}

#[test]
#[ignore = "internal persistent offload service worker"]
fn udp_offload_service_worker() {
    let out = PathBuf::from(std::env::var_os("GW_NET_OUT").unwrap());
    let server = std::env::var("GW_NET_ROLE").unwrap() == "server";
    let ifindex = enter_worker_network();
    let before = fragment_stats();
    let (sockets, cookies) = sockets(server, 0);
    let identity = json!({"role":if server {"server"} else {"client"},
        "namespace":fs::read_link("/proc/thread-self/ns/net").unwrap(),
        "ifindex":ifindex,"cookies":cookies});
    fs::write(
        out.join("identity.json"),
        serde_json::to_vec_pretty(&identity).unwrap(),
    )
    .unwrap();
    for socket in &sockets {
        option(
            socket,
            libc::UDP_SEGMENT,
            if server { 0 } else { SEGMENT as i32 },
        );
        option(socket, libc::UDP_GRO, i32::from(server));
    }
    fs::write(out.join("ready"), b"ready").unwrap();
    let mut rounds = Vec::new();
    for command in std::io::stdin().lock().lines() {
        let command = command.unwrap();
        if command == "finish" {
            break;
        }
        let round: usize = command.parse().unwrap();
        assert_eq!(round, rounds.len());
        let n = count(round);
        if server {
            fs::write(out.join(format!("round-{round}.ready")), b"ready").unwrap();
        }
        let mut rows = Vec::new();
        let mut expected = Vec::new();
        for (family, socket) in sockets.iter().enumerate() {
            let parts: Vec<_> = (0..n)
                .map(|index| {
                    payload(
                        round,
                        family,
                        index,
                        if index + 1 == n { 17 } else { SEGMENT },
                    )
                })
                .collect();
            let request = parts.concat();
            let reply = payload(round, family, 0, 32);
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let received = if server {
                let received = receive(socket).unwrap();
                verify_receives(true, &parts, std::slice::from_ref(&received));
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
            let cookie = cookies[family].parse().unwrap();
            for part in &parts {
                expected.push(packet(cookie, local, remote, !server, part));
            }
            expected.push(packet(cookie, local, remote, server, &reply));
            socket
                .set_read_timeout(Some(Duration::from_millis(10)))
                .unwrap();
            assert!(matches!(
                receive(socket).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ));
            rows.push(
                json!({"family":if family == 0 {4} else {6},"cookie":cookies[family],
                "request_lengths":parts.iter().map(Vec::len).collect::<Vec<_>>(),
                "request_sha256":hash(&request),"reply_sha256":hash(&reply),
                "received_length":received.0.len(),"received_sha256":hash(&received.0),
                "gro_segment":received.1,"extra_datagram_observed":false}),
            );
        }
        let row = json!({"round":round,"segment_count":n,"observations":rows});
        let mut oracle = row.clone();
        oracle["expected"] = serde_json::to_value(expected).unwrap();
        let pending = out.join("pending.json");
        fs::write(&pending, serde_json::to_vec_pretty(&oracle).unwrap()).unwrap();
        fs::rename(pending, out.join(format!("round-{round}.json"))).unwrap();
        rounds.push(row);
    }
    assert_eq!(rounds.len(), 12);
    let after = fragment_stats();
    let deltas: BTreeMap<_, _> = after
        .iter()
        .map(|(key, value)| (key.clone(), value - before[key]))
        .collect();
    assert!(deltas.values().all(|n| *n == 0));
    fs::write(
        out.join("oracle.json"),
        serde_json::to_vec_pretty(&json!({
        "identity":identity,"rounds":rounds,"fragment_counter_deltas":deltas,
        "same_sockets_all_rounds":true}))
        .unwrap(),
    )
    .unwrap();
}

fn round(server: &mut Process, client: &mut Process, out: &Path, round: usize) {
    writeln!(server.0.stdin.as_mut().unwrap(), "{round}").unwrap();
    await_file(server, &out.join(format!("server/round-{round}.ready")));
    writeln!(client.0.stdin.as_mut().unwrap(), "{round}").unwrap();
    await_file(client, &out.join(format!("client/round-{round}.json")));
    await_file(server, &out.join(format!("server/round-{round}.json")));
}

fn start(scope: &Scope, out: &Path, limit: usize, previous: Option<&str>) -> (Process, String) {
    fs::create_dir_all(out).unwrap();
    let mut capture = collector_with_options(
        scope,
        PORT,
        out,
        &[
            "--serve",
            "--capture-seconds",
            "120",
            "--capture-window-ms",
            "200",
            "--capture-max-facts",
            &limit.to_string(),
        ],
    );
    let id = wait_for_run(&mut capture, out, previous);
    (capture, id)
}

fn wait_for_run(capture: &mut Process, out: &Path, previous: Option<&str>) -> String {
    let deadline = Instant::now() + Duration::from_secs(40);
    loop {
        assert!(
            capture.0.try_wait().unwrap().is_none(),
            "{}",
            fs::read_to_string(out.join("stderr.log")).unwrap()
        );
        if let Ok(bytes) = fs::read(out.join("status.json")) {
            let status: CaptureSnapshot = serde_json::from_slice(&bytes).unwrap();
            if previous != Some(status.run_id.as_str()) {
                assert_eq!(status.state, CaptureState::Running);
                assert!(!status.final_accounted);
                return status.run_id;
            }
        }
        assert!(Instant::now() < deadline, "fresh run did not become ready");
        thread::sleep(Duration::from_millis(5));
    }
}

fn window(capture: &mut Process, out: &Path, run: &str, received: u64) -> ExportBundle {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(capture.0.try_wait().unwrap().is_none(), "collector exited");
        if let Ok(text) = fs::read_to_string(out.join("export.json")) {
            let bundle = ExportBundle::from_json(&text).unwrap();
            let status = bundle.capture.as_ref().unwrap();
            if status.run_id == run && status.received >= received {
                assert_eq!(
                    status.received, received,
                    "unexpected traffic in private scope"
                );
                assert_eq!(status.state, CaptureState::Running);
                assert!(!status.final_accounted);
                let replay = bundle.replay().unwrap();
                assert_eq!(replay.facts, bundle.facts);
                assert_eq!(replay.capture, bundle.capture);
                assert_eq!(replay.program_flows, bundle.program_flows);
                fs::write(out.join(format!("observed-{run}-{received}.json")), text).unwrap();
                return bundle;
            }
        }
        assert!(Instant::now() < deadline, "missing publication {received}");
        thread::sleep(Duration::from_millis(2));
    }
}

fn pause_idle(capture: &mut Process, out: &Path, run: &str) {
    // Startup status proves attachment, not that the window clock has started.
    // After one real publication, stop only inside the main loop's idle sleep,
    // not between publish() and resetting its clock. No traffic exists yet.
    assert!(window(capture, out, run, 0).facts.is_empty());
    pause_main_loop(capture, out);
}

fn pause_main_loop(capture: &mut Process, out: &Path) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let process = PathBuf::from(format!("/proc/{}", capture.0.id()));
    loop {
        signal(capture, libc::SIGSTOP);
        while !fs::read_to_string(process.join("status"))
            .unwrap()
            .lines()
            .any(|line| line.starts_with("State:\tT"))
        {
            assert!(capture.0.try_wait().unwrap().is_none());
            assert!(Instant::now() < deadline, "collector did not stop");
            thread::sleep(Duration::from_millis(1));
        }
        let syscall = fs::read_to_string(process.join("syscall")).unwrap();
        let number = syscall
            .split_whitespace()
            .next()
            .unwrap()
            .parse::<libc::c_long>();
        if number.is_ok_and(|n| n == libc::SYS_clock_nanosleep || n == libc::SYS_nanosleep) {
            fs::write(out.join("paused-idle-syscall.txt"), syscall).unwrap();
            return;
        }
        signal(capture, libc::SIGCONT);
        assert!(
            Instant::now() < deadline,
            "collector did not reach idle sleep"
        );
        thread::sleep(Duration::from_millis(1));
    }
}

fn groups(bundle: &ExportBundle) -> BTreeMap<u64, Vec<usize>> {
    let mut groups: BTreeMap<u64, Vec<_>> = BTreeMap::new();
    for fact in &bundle.facts {
        let FactKind::PacketMeta(p) = &fact.kind else {
            panic!("non-packet")
        };
        if let Some(meta) = p.udp_offload {
            assert!(meta.matches_packet(p));
            let round = usize::from(p.payload_bytes[&0]);
            assert_eq!(usize::from(meta.segment_count), count(round));
            let family = usize::from(p.l3_proto == 0x86dd);
            let data = payload(
                round,
                family,
                usize::from(meta.segment_index),
                if meta.segment_index + 1 == meta.segment_count {
                    17
                } else {
                    SEGMENT
                },
            );
            assert_eq!(
                meta.aggregate_payload_len as usize,
                (count(round) - 1) * SEGMENT + 17
            );
            assert_eq!(meta.segment_size, SEGMENT as u16);
            for (offset, byte) in data.iter().take(128).enumerate() {
                assert_eq!(p.payload_bytes[&(offset as u16)], *byte);
            }
            groups
                .entry(meta.aggregate_id)
                .or_default()
                .push(usize::from(meta.segment_index));
        }
    }
    // Import/replay above independently enforces complete groups and matching
    // timestamps, socket identity and origin; indexes must also be contiguous.
    for indexes in groups.values_mut() {
        indexes.sort_unstable();
        assert_eq!(*indexes, (0..indexes.len()).collect::<Vec<_>>());
    }
    groups
}

fn check_rounds(out: &Path, windows: &[&ExportBundle], rounds: &[usize], budget: bool) -> Value {
    let mut combined = (*windows[0]).clone();
    combined.facts = windows
        .iter()
        .flat_map(|w| w.facts.iter().cloned())
        .collect();
    let mut ids = BTreeSet::new();
    assert!(combined.facts.iter().all(|fact| ids.insert(fact.id)));
    let mut oracles = Vec::new();
    for role in ["client", "server"] {
        let mut identity: Value =
            serde_json::from_slice(&fs::read(out.join(role).join("identity.json")).unwrap())
                .unwrap();
        let mut expected = Vec::<Value>::new();
        for round in rounds {
            let row: Value = serde_json::from_slice(
                &fs::read(out.join(role).join(format!("round-{round}.json"))).unwrap(),
            )
            .unwrap();
            for packet in row["expected"].as_array().unwrap() {
                // Group 31 is IPv6 client egress in round 7: its first two
                // records hit a full 512-slot window, so all 16 stay rejected.
                if budget
                    && *round == 7
                    && role == "client"
                    && packet["family"] == 0x86dd
                    && packet["outbound"] == true
                {
                    continue;
                }
                expected.push(packet.clone());
            }
        }
        identity["expected"] = json!(expected);
        oracles.push(identity);
    }
    assert_eq!(check_namespaces(&combined, &oracles).len(), 2);
    let all_groups = groups(&combined);
    let mut seen = BTreeSet::new();
    for window in windows {
        for id in groups(window).keys() {
            assert!(seen.insert(*id), "group repeated across windows");
        }
    }
    for oracle in &mut oracles {
        let expected = oracle.as_object_mut().unwrap().remove("expected").unwrap();
        oracle["expected_count"] = json!(expected.as_array().unwrap().len());
        oracle["expected_sha256"] = json!(hash(&serde_json::to_vec(&expected).unwrap()));
    }
    json!({"facts":combined.facts.len(),"group_ids":all_groups.keys().collect::<Vec<_>>(),"oracles":oracles})
}

fn clean_stop(capture: &mut Process, out: &Path, received: u64, dropped: u64) -> CaptureSnapshot {
    signal(capture, libc::SIGTERM);
    assert_eq!(
        wait(capture, 10).code(),
        Some(if dropped == 0 { 0 } else { 3 })
    );
    let status = snapshot(out);
    assert!(status.final_accounted && status.reconciles());
    assert_eq!(status.state, CaptureState::Stopped);
    assert_eq!(status.kernel.matched, received);
    assert_eq!(status.received, received);
    assert_eq!(status.retained, received - dropped);
    assert_eq!(status.userspace_dropped, dropped);
    assert_eq!(
        status.kernel.ring_dropped
            + status.decode_rejected
            + status.kernel.unsupported
            + status.kernel.malformed
            + status.kernel.no_cookie,
        0
    );
    assert_eq!(status.incomplete, dropped != 0);
    assert_eq!(export(out).capture.as_ref(), Some(&status));
    assert!(
        export(out).facts.is_empty(),
        "previous windows must not be republished at stop"
    );
    status
}

#[test]
#[ignore = "root/BPF/iproute2; GEWY_OFFLOAD_SERVICE_LIMIT=512|528 and GEWY_OFFLOAD_SERVICE_OUT=new-directory"]
fn production_capture_offload_service_windows_and_restart() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let limit: usize = std::env::var("GEWY_OFFLOAD_SERVICE_LIMIT")
        .unwrap()
        .parse()
        .unwrap();
    assert!([512, 528].contains(&limit));
    let budget = limit == 512;
    let out = PathBuf::from(std::env::var_os("GEWY_OFFLOAD_SERVICE_OUT").unwrap());
    fs::create_dir(&out).unwrap();
    let mut network = Network::new();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-netns-offload-service-{}-{nonce}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    let service_dir = out.join("service");
    let (mut capture, run_id) = start(&scope, &service_dir, limit, None);
    pause_idle(&mut capture, &service_dir, &run_id);
    let mut server = worker(
        &network.0[1],
        "server",
        &scope,
        &out.join("server"),
        "network::offload::service::udp_offload_service_worker",
    );
    let mut client = worker(
        &network.0[0],
        "client",
        &scope,
        &out.join("client"),
        "network::offload::service::udp_offload_service_worker",
    );
    await_file(&mut server, &out.join("server/ready"));
    await_file(&mut client, &out.join("client/ready"));
    for id in 0..8 {
        round(&mut server, &mut client, &out, id);
    }
    thread::sleep(Duration::from_millis(250));
    signal(&capture, libc::SIGCONT);
    let first = window(&mut capture, &service_dir, &run_id, 512);
    assert_eq!(first.facts.len(), 510);
    let first_status = first.capture.as_ref().unwrap();
    assert_eq!(first_status.retained, 510);
    assert_eq!(first_status.userspace_dropped, if budget { 2 } else { 0 });
    assert!(
        !first_status.reconciles(),
        "32 records are still in the kernel ring"
    );
    assert_eq!(
        first_status.received - first_status.retained - first_status.userspace_dropped,
        if budget { 0 } else { 2 }
    );
    assert!(!groups(&first).contains_key(&31));
    let second = window(&mut capture, &service_dir, &run_id, 544);
    assert_eq!(second.facts.len(), if budget { 18 } else { 34 });
    assert_eq!(groups(&second).contains_key(&31), !budget);
    round(&mut server, &mut client, &out, 8);
    let recovery = window(&mut capture, &service_dir, &run_id, 564);
    assert_eq!(recovery.facts.len(), 20);
    let service_oracle = check_rounds(
        &out,
        &[&first, &second, &recovery],
        &(0..9).collect::<Vec<_>>(),
        budget,
    );
    let final_status = clean_stop(&mut capture, &service_dir, 564, if budget { 16 } else { 0 });
    detached(&scope);

    let restart_dir = out.join("restart");
    let (mut crash, crash_id) = start(&scope, &restart_dir, 528, None);
    round(&mut server, &mut client, &out, 9);
    let before_crash = window(&mut crash, &restart_dir, &crash_id, 20);
    let crash_oracle = check_rounds(&out, &[&before_crash], &[9], false);
    let deadline = Instant::now() + Duration::from_secs(3);
    while snapshot(&restart_dir).received != 20 {
        assert!(crash.0.try_wait().unwrap().is_none());
        assert!(
            Instant::now() < deadline,
            "running status was not published"
        );
        thread::sleep(Duration::from_millis(2));
    }
    signal(&crash, libc::SIGKILL);
    assert!(!wait(&mut crash, 10).success());
    detached(&scope);
    let stale = snapshot(&restart_dir);
    assert_eq!(stale.run_id, crash_id);
    assert_eq!(stale.state, CaptureState::Running);
    assert!(!stale.final_accounted);
    fs::write(
        out.join("stale-after-kill.json"),
        serde_json::to_vec_pretty(&stale).unwrap(),
    )
    .unwrap();
    round(&mut server, &mut client, &out, 10);
    let (mut restarted, restarted_id) = start(&scope, &restart_dir, 528, Some(&crash_id));
    assert_ne!(restarted_id, run_id);
    round(&mut server, &mut client, &out, 11);
    let after_restart = window(&mut restarted, &restart_dir, &restarted_id, 20);
    let restarted_oracle = check_rounds(&out, &[&after_restart], &[11], false);
    assert_eq!(
        groups(&before_crash).keys().collect::<Vec<_>>(),
        groups(&after_restart).keys().collect::<Vec<_>>()
    );
    assert_eq!(
        groups(&after_restart).into_keys().collect::<Vec<_>>(),
        [1, 2, 3, 4]
    );
    let restarted_status = clean_stop(&mut restarted, &restart_dir, 20, 0);
    detached(&scope);
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
    fs::remove_dir(&scope.0).unwrap();
    network.remove();
    fs::write(out.join("summary.json"), serde_json::to_vec_pretty(&json!({
        "schema_version":1,"ok":true,"production_cli":true,"limit":limit,"kernel_batch_limit":512,
        "first_window":first.capture,"first_window_facts":first.facts.len(),
        "first_window_groups":groups(&first).into_keys().collect::<Vec<_>>(),
        "second_window":second.capture,"second_window_facts":second.facts.len(),
        "second_window_groups":groups(&second).into_keys().collect::<Vec<_>>(),
        "recovery_window":recovery.capture,"recovery_window_facts":recovery.facts.len(),
        "recovery_window_groups":groups(&recovery).into_keys().collect::<Vec<_>>(),
        "service_terminal":final_status,"service_oracle":service_oracle,
        "crash_snapshot":stale,"crash_oracle":crash_oracle,
        "restart_terminal":restarted_status,"restart_oracle":restarted_oracle,
        "native":native,"native_requests":288,"native_replies":24,
        "same_socket_restart":true,"downtime_not_backfilled":true,"run_id_changed":true,
        "group_ids_restart_at_one":true,"whole_groups_per_window":true,"window_replay_identical":true,
        "hooks_detached":true,"owned_resources_removed":true,"host_network_changed":false,
        "binary_sha256":hash(&fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap()),
        "limits":["single_host_private_veth","bounded_12_round_scenario","no_durable_window_archive","no_crash_flush_claim"]
    })).unwrap()).unwrap();
}
