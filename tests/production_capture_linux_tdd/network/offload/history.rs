//! Paced real CLI traffic crossing multiple bounded aggregate-ID histories.
use super::*;

const BLOCKS: usize = 128;
const CHECKPOINTS: &[usize] = &[0, 31, 50, 51, 64, 101, 102, 127, 128];

fn shape(block: usize) -> (usize, usize) {
    assert!(block <= BLOCKS);
    if block == BLOCKS { (1, 16) } else { (20, 4) }
}

fn data(block: usize, iteration: usize, family: usize, part: usize, length: usize) -> Vec<u8> {
    let mut value = bytes(block + iteration, length);
    value[..2].copy_from_slice(&(block as u16).to_be_bytes());
    value[2..4].copy_from_slice(&(iteration as u16).to_be_bytes());
    value[4] = family as u8;
    value[5] = part as u8;
    value[6..8].copy_from_slice(&(length as u16).to_be_bytes());
    value
}

fn parts(block: usize, iteration: usize, family: usize) -> Vec<Vec<u8>> {
    let (_, count) = shape(block);
    (0..count)
        .map(|part| {
            data(
                block,
                iteration,
                family,
                part,
                if part + 1 == count { 17 } else { SEGMENT },
            )
        })
        .collect()
}

fn digest(context: ring::digest::Context) -> String {
    context
        .finish()
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

#[test]
#[ignore = "internal native offload history worker"]
fn udp_offload_history_worker() {
    let out = PathBuf::from(std::env::var_os("GW_NET_OUT").unwrap());
    let server = std::env::var("GW_NET_ROLE").unwrap() == "server";
    let ifindex = enter_worker_network();
    let before = fragment_stats();
    let (sockets, cookies) = sockets(server, 0);
    let identity = json!({"role":if server {"server"} else {"client"},"ifindex":ifindex,
        "cookies":cookies,"namespace":fs::read_link("/proc/thread-self/ns/net").unwrap()});
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
    let mut blocks = Vec::new();
    for command in std::io::stdin().lock().lines() {
        let command = command.unwrap();
        if command == "finish" {
            break;
        }
        let block: usize = command.parse().unwrap();
        assert_eq!(block, blocks.len());
        let (iterations, count) = shape(block);
        if server {
            fs::write(out.join(format!("block-{block}.ready")), b"ready").unwrap();
        }
        let mut requests = ring::digest::Context::new(&ring::digest::SHA256);
        let mut replies = ring::digest::Context::new(&ring::digest::SHA256);
        for iteration in 0..iterations {
            for (family, socket) in sockets.iter().enumerate() {
                socket
                    .set_read_timeout(Some(Duration::from_secs(3)))
                    .unwrap();
                let parts = parts(block, iteration, family);
                let request = parts.concat();
                let reply = data(block, iteration, family, 255, 32);
                if server {
                    let received = receive(socket).unwrap();
                    verify_receives(true, &parts, std::slice::from_ref(&received));
                    requests.update(&received.0);
                    assert_eq!(socket.send(&reply).unwrap(), reply.len());
                    replies.update(&reply);
                } else {
                    assert_eq!(socket.send(&request).unwrap(), request.len());
                    requests.update(&request);
                    let received = receive(socket).unwrap();
                    assert_eq!(received, (reply.clone(), None));
                    replies.update(&received.0);
                }
            }
        }
        for socket in &sockets {
            socket
                .set_read_timeout(Some(Duration::from_millis(5)))
                .unwrap();
            assert!(matches!(
                receive(socket).unwrap_err().kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ));
        }
        let value = json!({"block":block,"iterations":iterations,"segment_count":count,
            "logical_requests":iterations*2*count,"native_replies":iterations*2,
            "request_sha256":digest(requests),"reply_sha256":digest(replies),
            "gro_buffers":if server {iterations*2} else {0},"gro_segment":if server {Some(SEGMENT)} else {None},
            "extra_datagram_observed":false});
        fs::write(
            out.join("pending.json"),
            serde_json::to_vec_pretty(&value).unwrap(),
        )
        .unwrap();
        fs::rename(
            out.join("pending.json"),
            out.join(format!("block-{block}.json")),
        )
        .unwrap();
        blocks.push(value);
    }
    assert_eq!(blocks.len(), BLOCKS + 1);
    let after = fragment_stats();
    let deltas: BTreeMap<_, _> = after
        .iter()
        .map(|(key, value)| (key.clone(), value - before[key]))
        .collect();
    assert!(deltas.values().all(|n| *n == 0));
    fs::write(
        out.join("oracle.json"),
        serde_json::to_vec_pretty(&json!({"identity":identity,"blocks":blocks,
        "fragment_counter_deltas":deltas,"same_sockets_all_blocks":true}))
        .unwrap(),
    )
    .unwrap();
}

fn audit(bundle: &ExportBundle, out: &Path, block: usize) -> Value {
    let (iterations, count) = shape(block);
    assert_eq!(bundle.facts.len(), iterations * (4 * count + 4));
    let mut oracles = Vec::new();
    for (role, server) in [("client", false), ("server", true)] {
        let mut identity: Value =
            serde_json::from_slice(&fs::read(out.join(role).join("identity.json")).unwrap())
                .unwrap();
        let mut expected = Vec::new();
        for iteration in 0..iterations {
            for family in 0..2 {
                let cookie = identity["cookies"][family]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap();
                for part in parts(block, iteration, family) {
                    expected.push(packet(
                        cookie,
                        endpoint(family, server),
                        endpoint(family, !server),
                        !server,
                        &part,
                    ));
                }
                expected.push(packet(
                    cookie,
                    endpoint(family, server),
                    endpoint(family, !server),
                    server,
                    &data(block, iteration, family, 255, 32),
                ));
            }
        }
        identity["expected"] = json!(expected);
        oracles.push(identity);
    }
    assert_eq!(check_namespaces(bundle, &oracles).len(), 2);
    let mut groups: BTreeMap<u64, Vec<_>> = BTreeMap::new();
    for fact in &bundle.facts {
        let FactKind::PacketMeta(p) = &fact.kind else {
            panic!("non-packet")
        };
        if let Some(meta) = p.udp_offload {
            assert!(meta.matches_packet(p));
            assert_eq!(usize::from(meta.segment_count), count);
            assert_eq!(meta.segment_size, SEGMENT as u16);
            assert_eq!(
                meta.aggregate_payload_len as usize,
                (count - 1) * SEGMENT + 17
            );
            assert_eq!(p.payload_bytes[&5], meta.segment_index as u8);
            groups
                .entry(meta.aggregate_id)
                .or_default()
                .push(meta.segment_index);
        } else {
            assert_eq!(p.payload_bytes[&5], 255);
        }
    }
    let first = block as u64 * 80 + 1;
    let last = first + (iterations * 4) as u64 - 1;
    assert_eq!(
        groups.keys().copied().collect::<Vec<_>>(),
        (first..=last).collect::<Vec<_>>()
    );
    for members in groups.values_mut() {
        members.sort_unstable();
        assert_eq!(*members, (0..count as u16).collect::<Vec<_>>());
    }
    let hashes: Vec<_> = oracles
        .iter()
        .map(|o| hash(&serde_json::to_vec(&o["expected"]).unwrap()))
        .collect();
    json!({"block":block,"facts":bundle.facts.len(),"groups":groups.len(),
        "first_group":first.to_string(),"last_group":last.to_string(),"expected_sha256":hashes})
}

fn resource(capture: &Process, block: usize, start: Instant) -> Value {
    let root = PathBuf::from(format!("/proc/{}", capture.0.id()));
    let status = fs::read_to_string(root.join("status")).unwrap();
    let kib = |name: &str| -> u64 {
        let value = status
            .lines()
            .find_map(|line| line.strip_prefix(name))
            .unwrap();
        assert_eq!(value.split_whitespace().nth(1), Some("kB"));
        value.split_whitespace().next().unwrap().parse().unwrap()
    };
    let stat = fs::read_to_string(root.join("stat")).unwrap();
    let fields: Vec<_> = stat
        .rsplit_once(") ")
        .unwrap()
        .1
        .split_whitespace()
        .collect();
    let user: u64 = fields[11].parse().unwrap();
    let system: u64 = fields[12].parse().unwrap();
    let ticks = unsafe { libc::sysconf(libc::_SC_CLK_TCK) };
    assert!(ticks > 0);
    assert!(
        kib("VmRSS:") < 512 * 1024,
        "bounded-test RSS guard, not a leak detector"
    );
    json!({"block":block,"elapsed_ms":start.elapsed().as_secs_f64()*1000.0,
        "rss_kib":kib("VmRSS:"),"hwm_kib":kib("VmHWM:"),"cpu_user_ticks":user,
        "cpu_system_ticks":system,"clock_ticks_per_second":ticks})
}

#[test]
#[ignore = "root/BPF/iproute2; GEWY_OFFLOAD_HISTORY_OUT=new-directory; paced ~1-minute dual-stack run"]
fn production_capture_offload_history_rollover() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let out = PathBuf::from(std::env::var_os("GEWY_OFFLOAD_HISTORY_OUT").unwrap());
    fs::create_dir(&out).unwrap();
    let mut network = Network::new();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-netns-history-{}-{nonce}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    let dir = out.join("capture");
    fs::create_dir(&dir).unwrap();
    let mut capture = collector_with_options(
        &scope,
        PORT,
        &dir,
        &[
            "--serve",
            "--capture-seconds",
            "180",
            "--capture-window-ms",
            "300",
            "--capture-max-facts",
            "4096",
        ],
    );
    ready(&mut capture, &dir);
    let run_id = snapshot(&dir).run_id;
    pause_idle(&mut capture, &dir, &run_id);
    let selector = "network::offload::service::history::udp_offload_history_worker";
    let mut server = worker(
        &network.0[1],
        "server",
        &scope,
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
    let start = Instant::now();
    let mut total = 0u64;
    let mut ids = BTreeSet::new();
    let mut windows = Vec::new();
    let mut resources = Vec::new();
    let mut checkpoints = Vec::new();
    for block in 0..=BLOCKS {
        writeln!(server.0.stdin.as_mut().unwrap(), "{block}").unwrap();
        await_file(
            &mut server,
            &out.join(format!("server/block-{block}.ready")),
        );
        writeln!(client.0.stdin.as_mut().unwrap(), "{block}").unwrap();
        await_file(&mut client, &out.join(format!("client/block-{block}.json")));
        await_file(&mut server, &out.join(format!("server/block-{block}.json")));
        signal(&capture, libc::SIGCONT);
        let (iterations, count) = shape(block);
        total += (iterations * (4 * count + 4)) as u64;
        let bundle = window(&mut capture, &dir, &run_id, total);
        pause_main_loop(&mut capture, &dir);
        let s = bundle.capture.as_ref().unwrap();
        assert!(s.reconciles() && !s.incomplete && !s.has_loss());
        assert_eq!(s.kernel.matched, total);
        assert_eq!(s.retained, total);
        assert!(
            bundle.facts.iter().all(|f| ids.insert(f.id.0)),
            "fact ID repeated across windows"
        );
        windows.push(audit(&bundle, &out, block));
        resources.push(resource(&capture, block, start));
        if CHECKPOINTS.contains(&block) {
            checkpoints.push(json!({"block":block,"capture":s}));
            eprintln!(
                "history block {block}: {total} facts, last group {}",
                windows.last().unwrap()["last_group"]
            );
        }
    }
    assert_eq!(total, 51268);
    assert_eq!(ids.len(), total as usize);
    assert_eq!(*ids.first().unwrap(), 1);
    assert_eq!(*ids.last().unwrap(), total);
    signal(&capture, libc::SIGCONT);
    let terminal = clean_stop(&mut capture, &dir, total, 0);
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
    fs::write(out.join("summary.json"),serde_json::to_vec_pretty(&json!({
        "schema_version":1,"ok":true,"production_cli":true,"terminal":terminal,
        "history_id_window":4096,"history_groups":10244,"facts":total,"publication_blocks":129,
        "native_requests":20512,"native_replies":5122,"windows":windows,"checkpoints":checkpoints,
        "resources":resources,"native":native,"packet_multiset_matches":true,"window_replay_identical":true,
        "unique_fact_ids":true,"contiguous_group_ids":true,"same_sockets_all_blocks":true,
        "post_rollover_16_segment_recovery":true,"hooks_detached":true,"owned_resources_removed":true,
        "host_network_changed":false,"binary_sha256":hash(&fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap()),
        "limits":["single_host_private_veth","paced_not_throughput_benchmark","collector_cpu_excludes_workload_bpf_cost",
            "rss_observation_not_leak_proof","internal_history_bound_separately_unit_tested","no_durable_archive_claim"]
    })).unwrap()).unwrap();
}
