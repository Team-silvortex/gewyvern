//! Pinned independent peers exercise real multi-CPU aggregate interleaving.
use super::*;

#[path = "ring_pressure.rs"]
mod ring_pressure;

const LANES: usize = 4;
const BLOCKS: usize = 16;

fn shape(block: usize) -> (usize, usize) {
    assert!(block <= BLOCKS);
    if block == BLOCKS { (1, 4) } else { (2, 16) }
}

fn data(
    block: usize,
    lane: usize,
    iteration: usize,
    family: usize,
    part: usize,
    len: usize,
) -> Vec<u8> {
    let mut data = bytes(block + iteration, len);
    data[..9].copy_from_slice(&[
        (block >> 8) as u8,
        block as u8,
        (iteration >> 8) as u8,
        iteration as u8,
        family as u8,
        part as u8,
        (len >> 8) as u8,
        len as u8,
        lane as u8,
    ]);
    data
}

fn clock_ns() -> u64 {
    let mut value: libc::timespec = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut value) },
        0
    );
    value.tv_sec as u64 * 1_000_000_000 + value.tv_nsec as u64
}

fn wait_until(deadline: u64) {
    while let Some(left) = deadline.checked_sub(clock_ns()) {
        if left > 200_000 {
            thread::sleep(Duration::from_nanos(left - 100_000));
        } else {
            std::hint::spin_loop();
        }
    }
}

fn cpus() -> Vec<usize> {
    let cpus: Vec<usize> = std::env::var("GEWY_OFFLOAD_CONCURRENT_CPUS")
        .unwrap()
        .split(',')
        .map(|n| n.parse().unwrap())
        .collect();
    assert_eq!(cpus.len(), LANES * 2);
    assert_eq!(cpus.iter().collect::<BTreeSet<_>>().len(), cpus.len());
    let mut allowed: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    assert_eq!(
        unsafe { libc::sched_getaffinity(0, size_of_val(&allowed), &mut allowed) },
        0
    );
    assert!(
        cpus.iter()
            .all(|cpu| *cpu < libc::CPU_SETSIZE as usize
                && unsafe { libc::CPU_ISSET(*cpu, &allowed) })
    );
    cpus
}

#[test]
#[ignore = "internal pinned concurrent offload peer"]
fn udp_offload_concurrent_worker() {
    run_worker(BLOCKS + 1, |block, _| shape(block));
}

fn run_worker(total_blocks: usize, shape: fn(usize, usize) -> (usize, usize)) {
    let out = PathBuf::from(std::env::var_os("GW_NET_OUT").unwrap());
    let server = std::env::var("GW_NET_ROLE").unwrap() == "server";
    let lane: usize = out
        .parent()
        .unwrap()
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(lane < LANES);
    let cpu = cpus()[lane + if server { LANES } else { 0 }];
    let mut mask: libc::cpu_set_t = unsafe { std::mem::zeroed() };
    unsafe { libc::CPU_SET(cpu, &mut mask) };
    assert_eq!(
        unsafe { libc::sched_setaffinity(0, size_of_val(&mask), &mask) },
        0
    );
    assert_eq!(unsafe { libc::sched_getcpu() }, cpu as i32);
    let ifindex = enter_worker_network();
    let before = fragment_stats();
    let (sockets, cookies) = sockets(server, 0);
    for socket in &sockets {
        option(
            socket,
            libc::UDP_SEGMENT,
            if server { 0 } else { SEGMENT as i32 },
        );
        option(socket, libc::UDP_GRO, i32::from(server));
    }
    let identity = json!({"role":if server {"server"} else {"client"},"lane":lane,
        "cpu":cpu,"ifindex":ifindex,"cookies":cookies,
        "namespace":fs::read_link("/proc/thread-self/ns/net").unwrap()});
    fs::write(
        out.join("identity.json"),
        serde_json::to_vec_pretty(&identity).unwrap(),
    )
    .unwrap();
    fs::write(out.join("ready"), b"ready").unwrap();
    let mut blocks = Vec::new();
    for command in std::io::stdin().lock().lines() {
        let command = command.unwrap();
        if command == "finish" {
            break;
        }
        let (block, deadline) = command.split_once(' ').unwrap();
        let block: usize = block.parse().unwrap();
        let deadline: u64 = deadline.parse().unwrap();
        assert_eq!(block, blocks.len());
        let (iterations, count) = shape(block, lane);
        for socket in &sockets {
            option(
                socket,
                libc::UDP_SEGMENT,
                if !server && count > 1 {
                    SEGMENT as i32
                } else {
                    0
                },
            );
        }
        let mut requests = Vec::new();
        let mut replies = Vec::new();
        wait_until(deadline);
        let started = clock_ns();
        for iteration in 0..iterations {
            for (family, socket) in sockets.iter().enumerate() {
                wait_until(deadline + (iteration * 2 + family) as u64 * 2_000_000);
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                assert_eq!(unsafe { libc::sched_getcpu() }, cpu as i32);
                let parts: Vec<_> = (0..count)
                    .map(|part| {
                        data(
                            block,
                            lane,
                            iteration,
                            family,
                            part,
                            if part + 1 == count { 17 } else { SEGMENT },
                        )
                    })
                    .collect();
                let request = parts.concat();
                let reply = data(block, lane, iteration, family, 255, 32);
                if server {
                    let received = receive(socket).unwrap();
                    verify_receives(count > 1, &parts, std::slice::from_ref(&received));
                    requests.extend_from_slice(&received.0);
                    assert_eq!(socket.send(&reply).unwrap(), reply.len());
                    replies.extend_from_slice(&reply);
                } else {
                    assert_eq!(socket.send(&request).unwrap(), request.len());
                    requests.extend_from_slice(&request);
                    let received = receive(socket).unwrap();
                    assert_eq!(received, (reply, None));
                    replies.extend_from_slice(&received.0);
                }
            }
        }
        let ended = clock_ns();
        for socket in &sockets {
            socket
                .set_read_timeout(Some(Duration::from_millis(5)))
                .unwrap();
            assert!(matches!(
                receive(socket).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ));
        }
        let row = json!({"block":block,"iterations":iterations,"segments":count,
            "start_ns":started.to_string(),"end_ns":ended.to_string(),
            "request_sha256":hash(&requests),"reply_sha256":hash(&replies),
            "gro_buffers":if server && count>1 {iterations*2} else {0},
            "gro_segment":if server && count>1 && iterations>0 {Some(SEGMENT)} else {None},"extra_datagram_observed":false});
        fs::write(
            out.join("pending.json"),
            serde_json::to_vec_pretty(&row).unwrap(),
        )
        .unwrap();
        fs::rename(
            out.join("pending.json"),
            out.join(format!("block-{block}.json")),
        )
        .unwrap();
        blocks.push(row);
    }
    assert_eq!(blocks.len(), total_blocks);
    let deltas: BTreeMap<_, _> = fragment_stats()
        .into_iter()
        .map(|(key, value)| {
            let delta = value - before[&key];
            (key, delta)
        })
        .collect();
    assert!(deltas.values().all(|n| *n == 0));
    fs::write(
        out.join("oracle.json"),
        serde_json::to_vec_pretty(&json!({"identity":identity,
        "blocks":blocks,"fragment_counter_deltas":deltas,"same_sockets_all_blocks":true}))
        .unwrap(),
    )
    .unwrap();
}

fn audit(windows: &[ExportBundle], out: &Path, block: usize, cpus: &[usize]) -> Value {
    let (iterations, count) = shape(block);
    let mut combined = windows[0].clone();
    combined.facts = windows.iter().flat_map(|w| w.facts.clone()).collect();
    assert_eq!(combined.facts.len(), LANES * iterations * (4 * count + 4));
    let mut oracles = Vec::new();
    let mut socket_owners = BTreeMap::new();
    for lane in 0..LANES {
        for (role, server) in [("client", false), ("server", true)] {
            let mut identity: Value = serde_json::from_slice(
                &fs::read(out.join(lane.to_string()).join(role).join("identity.json")).unwrap(),
            )
            .unwrap();
            let mut expected = Vec::new();
            for iteration in 0..iterations {
                for family in 0..2 {
                    let cookie = identity["cookies"][family]
                        .as_str()
                        .unwrap()
                        .parse()
                        .unwrap();
                    socket_owners.insert(cookie, (lane, server, family));
                    for part in 0..count {
                        expected.push(packet(
                            cookie,
                            endpoint(family, server),
                            endpoint(family, !server),
                            !server,
                            &data(
                                block,
                                lane,
                                iteration,
                                family,
                                part,
                                if part + 1 == count { 17 } else { SEGMENT },
                            ),
                        ));
                    }
                    expected.push(packet(
                        cookie,
                        endpoint(family, server),
                        endpoint(family, !server),
                        server,
                        &data(block, lane, iteration, family, 255, 32),
                    ));
                }
            }
            identity["expected"] = json!(expected);
            oracles.push(identity);
        }
    }
    assert_eq!(check_namespaces(&combined, &oracles).len(), LANES * 2);
    let mut groups = Vec::new();
    let mut ordinary_ids = Vec::new();
    let mut seen = BTreeSet::new();
    let mut egress_cpus = BTreeSet::new();
    for (window, bundle) in windows.iter().enumerate() {
        let mut members: BTreeMap<u64, Vec<_>> = BTreeMap::new();
        for fact in &bundle.facts {
            let FactKind::PacketMeta(p) = &fact.kind else {
                panic!("non-packet")
            };
            let (lane, server, family) = socket_owners[&p.sk_cookie.unwrap()];
            if p.dir == gewyvern::ledger::PacketDir::Egress {
                let cpu = cpus[lane + if server { LANES } else { 0 }];
                assert_eq!(usize::from(fact.cpu.0), cpu);
                egress_cpus.insert(cpu);
            }
            if let Some(meta) = p.udp_offload {
                assert!(meta.matches_packet(p));
                assert_eq!(usize::from(meta.segment_count), count);
                assert_eq!(usize::from(meta.segment_size), SEGMENT);
                assert_eq!(
                    meta.aggregate_payload_len as usize,
                    (count - 1) * SEGMENT + 17
                );
                assert_eq!(p.payload_bytes[&5], meta.segment_index as u8);
                assert_eq!(p.payload_bytes[&8], lane as u8);
                members.entry(meta.aggregate_id).or_default().push((
                    meta.segment_index,
                    fact,
                    lane,
                    server,
                    family,
                ));
            } else {
                assert_eq!(p.payload_bytes[&5], 255);
                ordinary_ids.push(fact.id.0);
            }
        }
        for (id, mut members) in members {
            assert!(seen.insert(id), "group split/repeated across windows");
            members.sort_by_key(|m| m.0);
            assert_eq!(
                members.iter().map(|m| m.0).collect::<Vec<_>>(),
                (0..count as u16).collect::<Vec<_>>()
            );
            let (_, first, lane, server, family) = members[0];
            let FactKind::PacketMeta(first_packet) = &first.kind else {
                unreachable!()
            };
            for (_, member, member_lane, member_server, member_family) in &members {
                assert_eq!(
                    (member.cpu, member.ts, member.ifindex, member.session),
                    (first.cpu, first.ts, first.ifindex, first.session)
                );
                assert_eq!(
                    (*member_lane, *member_server, *member_family),
                    (lane, server, family)
                );
                let FactKind::PacketMeta(packet) = &member.kind else {
                    unreachable!()
                };
                for offset in 0..5 {
                    assert_eq!(
                        packet.payload_bytes[&offset],
                        first_packet.payload_bytes[&offset]
                    );
                }
            }
            let ids: Vec<_> = members.iter().map(|m| m.1.id.0).collect();
            assert!(ids.windows(2).all(|pair| pair[0] < pair[1]));
            groups.push(
                json!({"id":id,"cpu":first.cpu.0,"lane":lane,"server":server,
                "family":family,"window":window,"fact_ids":ids}),
            );
        }
    }
    assert_eq!(
        egress_cpus.into_iter().collect::<Vec<_>>(),
        cpus.iter()
            .copied()
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>()
    );
    let first = block as u64 * 32 + 1;
    assert_eq!(
        seen.into_iter().collect::<Vec<_>>(),
        (first..first + (LANES * iterations * 4) as u64).collect::<Vec<_>>()
    );
    let hashes: Vec<_> = oracles
        .iter()
        .map(|o| hash(&serde_json::to_vec(&o["expected"]).unwrap()))
        .collect();
    let interleaved = groups
        .iter()
        .filter(|g| {
            let ids = g["fact_ids"].as_array().unwrap();
            ids.last().unwrap().as_u64().unwrap() - ids[0].as_u64().unwrap() + 1 > count as u64
        })
        .count();
    let mut first_arrivals: Vec<_> = groups
        .iter()
        .map(|g| {
            (
                g["fact_ids"][0].as_u64().unwrap(),
                g["id"].as_u64().unwrap(),
            )
        })
        .collect();
    first_arrivals.sort_unstable();
    let inversions = first_arrivals
        .windows(2)
        .filter(|pair| pair[0].1 > pair[1].1)
        .count();
    json!({"block":block,"facts":combined.facts.len(),"groups":groups,"ordinary_ids":ordinary_ids,
        "expected_sha256":hashes,"interleaved_groups":interleaved,"first_arrival_id_descents":inversions,
        "windows":windows.iter().map(|w| json!({"facts":w.facts.len(),"capture":w.capture})).collect::<Vec<_>>()})
}

fn frozen_window(capture: &mut Process, out: &Path, run: &str, received: u64) -> ExportBundle {
    let bundle = freeze_publication(capture, out, run, received);
    assert!(
        !bundle.facts.is_empty(),
        "test read an overwritten empty window"
    );
    bundle
}

fn freeze_publication(capture: &mut Process, out: &Path, run: &str, received: u64) -> ExportBundle {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(capture.0.try_wait().unwrap().is_none(), "collector exited");
        if let Ok(text) = fs::read_to_string(out.join("export.json")) {
            let bundle = ExportBundle::from_json(&text).unwrap();
            let status = bundle.capture.as_ref().unwrap();
            if status.run_id == run && status.received >= received {
                assert_eq!(status.received, received, "test skipped a publication");
                // Latest-window output is not a queue. Freeze before expensive
                // replay/checks can allow a later empty publication to replace it.
                pause_main_loop(capture, out);
                assert_eq!(
                    export(out).capture.as_ref().unwrap().received,
                    received,
                    "test missed its freeze boundary"
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
        assert!(Instant::now() < deadline, "publication timeout");
        thread::sleep(Duration::from_millis(2));
    }
}

#[test]
#[ignore = "root/BPF/8 distinct CPUs; GEWY_OFFLOAD_CONCURRENT_CPUS=list and GEWY_OFFLOAD_CONCURRENT_OUT=new-directory"]
fn production_capture_offload_multi_cpu_windows() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let cpus = cpus();
    let out = PathBuf::from(std::env::var_os("GEWY_OFFLOAD_CONCURRENT_OUT").unwrap());
    fs::create_dir(&out).unwrap();
    let mut networks: Vec<_> = (0..LANES).map(|_| Network::new()).collect();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-netns-concurrent-{}-{nonce}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    let capture_dir = out.join("capture");
    fs::create_dir(&capture_dir).unwrap();
    let mut capture = collector_with_options(
        &scope,
        PORT,
        &capture_dir,
        &[
            "--serve",
            "--capture-seconds",
            "120",
            "--capture-window-ms",
            "600",
            "--capture-max-facts",
            "4096",
        ],
    );
    ready(&mut capture, &capture_dir);
    let run_id = snapshot(&capture_dir).run_id;
    pause_idle(&mut capture, &capture_dir, &run_id);
    let mut peers = Vec::new();
    for (lane, network) in networks.iter().enumerate() {
        let root = out.join(lane.to_string());
        fs::create_dir(&root).unwrap();
        for (role, server) in [("client", false), ("server", true)] {
            let dir = root.join(role);
            let process = worker(
                &network.0[usize::from(server)],
                role,
                &scope,
                &dir,
                "network::offload::service::concurrent::udp_offload_concurrent_worker",
            );
            peers.push((process, dir, server));
        }
    }
    for (peer, dir, _) in &mut peers {
        await_file(peer, &dir.join("ready"));
    }
    let mut blocks = Vec::new();
    let mut total = 0;
    let mut ids = BTreeSet::new();
    for block in 0..=BLOCKS {
        let deadline = clock_ns() + 100_000_000;
        for (peer, _, _) in &mut peers {
            writeln!(peer.0.stdin.as_mut().unwrap(), "{block} {deadline}").unwrap();
        }
        let mut starts = Vec::new();
        let mut ends = Vec::new();
        for (peer, dir, server) in &mut peers {
            let file = dir.join(format!("block-{block}.json"));
            await_file(peer, &file);
            let row: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
            if !*server {
                starts.push(row["start_ns"].as_str().unwrap().parse::<u64>().unwrap());
                ends.push(row["end_ns"].as_str().unwrap().parse::<u64>().unwrap());
            }
        }
        assert!(
            starts.iter().max().unwrap() < ends.iter().min().unwrap(),
            "native client intervals did not overlap"
        );
        thread::sleep(Duration::from_millis(650));
        signal(&capture, libc::SIGCONT);
        let mut windows = Vec::new();
        if block < BLOCKS {
            windows.push(frozen_window(
                &mut capture,
                &capture_dir,
                &run_id,
                total + 512,
            ));
            signal(&capture, libc::SIGCONT);
        }
        total += if block < BLOCKS { 544 } else { 80 };
        windows.push(frozen_window(&mut capture, &capture_dir, &run_id, total));
        for w in &windows {
            let s = w.capture.as_ref().unwrap();
            assert!(!s.incomplete && !s.has_loss());
            assert_eq!(s.kernel.matched, total);
            assert!(w.facts.iter().all(|f| ids.insert(f.id.0)));
        }
        let last = windows.last().unwrap().capture.as_ref().unwrap();
        assert!(last.reconciles());
        assert_eq!(last.retained, total);
        blocks.push(audit(&windows, &out, block, &cpus));
    }
    assert_eq!(total, 8784);
    assert_eq!(
        ids.into_iter().collect::<Vec<_>>(),
        (1..=total).collect::<Vec<_>>()
    );
    assert!(
        blocks
            .iter()
            .any(|b| b["interleaved_groups"].as_u64().unwrap() > 0),
        "no observed producer interleaving"
    );
    assert!(
        blocks.iter().take(BLOCKS).any(|b| {
            let s: CaptureSnapshot =
                serde_json::from_value(b["windows"][0]["capture"].clone()).unwrap();
            s.received > s.retained
        }),
        "no observed pending group across publication"
    );
    signal(&capture, libc::SIGCONT);
    let terminal = clean_stop(&mut capture, &capture_dir, total, 0);
    detached(&scope);
    let mut native = Vec::new();
    for (peer, dir, _) in &mut peers {
        writeln!(peer.0.stdin.as_mut().unwrap(), "finish").unwrap();
        assert!(wait(peer, 5).success());
        native.push(
            serde_json::from_slice::<Value>(&fs::read(dir.join("oracle.json")).unwrap()).unwrap(),
        );
    }
    fs::remove_dir(&scope.0).unwrap();
    for network in &mut networks {
        network.remove();
    }
    fs::write(out.join("summary.json"),serde_json::to_vec_pretty(&json!({"schema_version":1,"ok":true,
        "production_cli":true,"cpus":cpus,"lanes":LANES,"window_ms":600,"freeze_before_replay":true,
        "blocks":blocks,"native":native,
        "terminal":terminal,"facts":total,"groups":528,"native_requests":4128,"native_replies":264,
        "packet_multiset_matches":true,"replay_identical":true,"same_sockets_recovery":true,
        "hooks_detached":true,"owned_resources_removed":true,"host_network_changed":false,
        "binary_sha256":hash(&fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap()),
        "limits":["single_host_private_veth","paced_not_throughput_benchmark","no_forced_kernel_order",
            "no_exhaustive_contention_claim","no_durable_archive_claim"]})).unwrap()).unwrap();
}
