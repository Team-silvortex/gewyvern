//! Small-ring loss during pinned multi-CPU production, followed by idle recovery.
use super::*;

const CYCLES: usize = 3;

#[derive(Clone, Copy)]
enum Profile {
    RingOnly,
    Mixed,
}

impl Profile {
    fn mixed(self) -> bool {
        matches!(self, Self::Mixed)
    }
    fn stride(self) -> usize {
        6 + usize::from(self.mixed())
    }
    fn received(self) -> usize {
        65 + 17 * usize::from(self.mixed())
    }
    fn matched(self) -> usize {
        600 + 68 * usize::from(self.mixed())
    }
    fn lost(self) -> usize {
        535 + 51 * usize::from(self.mixed())
    }
    fn groups(self) -> usize {
        48 + 4 * usize::from(self.mixed())
    }
    fn limit(self) -> usize {
        if self.mixed() { 24 } else { 4096 }
    }
    fn shape(self, block: usize, lane: usize) -> (usize, usize) {
        assert!(block < CYCLES * self.stride() && lane < LANES);
        match block % self.stride() {
            0 => (usize::from(lane == 0), 1),
            1 => (2, 16),
            2 if self.mixed() => (usize::from(lane == (block / self.stride()) % LANES), 16),
            recovery => (usize::from(lane == recovery - (self.stride() - 4)), 2),
        }
    }
}

fn shape(block: usize, lane: usize) -> (usize, usize) {
    Profile::RingOnly.shape(block, lane)
}

#[test]
#[ignore = "internal pinned ring-pressure peer"]
fn udp_offload_pressure_worker() {
    run_worker(CYCLES * 6, shape);
}

#[test]
#[ignore = "internal pinned mixed-pressure peer"]
fn udp_offload_mixed_pressure_worker() {
    run_worker(CYCLES * 7, |block, lane| Profile::Mixed.shape(block, lane));
}

fn dispatch(peers: &mut [(Process, PathBuf, bool)], block: usize, profile: Profile) {
    let deadline = clock_ns() + 100_000_000;
    for (peer, _, _) in peers.iter_mut() {
        writeln!(peer.0.stdin.as_mut().unwrap(), "{block} {deadline}").unwrap();
    }
    let mut starts = Vec::new();
    let mut ends = Vec::new();
    for (peer, dir, server) in peers {
        let file = dir.join(format!("block-{block}.json"));
        await_file(peer, &file);
        let row: Value = serde_json::from_slice(&fs::read(file).unwrap()).unwrap();
        if !*server {
            starts.push(row["start_ns"].as_str().unwrap().parse::<u64>().unwrap());
            ends.push(row["end_ns"].as_str().unwrap().parse::<u64>().unwrap());
        }
    }
    if block % profile.stride() == 1 {
        assert!(
            starts.iter().max().unwrap() < ends.iter().min().unwrap(),
            "native pressure clients did not overlap"
        );
    }
}

fn expired(capture: &mut Process, dir: &Path, partial: &CaptureSnapshot) -> ExportBundle {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        assert!(capture.0.try_wait().unwrap().is_none());
        let bundle = export(dir);
        let s = bundle.capture.as_ref().unwrap();
        assert_eq!(s.run_id, partial.run_id);
        assert_eq!(s.received, partial.received);
        assert_eq!(s.retained, partial.retained);
        assert_eq!(s.kernel, partial.kernel);
        assert_eq!(s.userspace_dropped, partial.userspace_dropped);
        if s.reconciles() {
            pause_main_loop(capture, dir);
            assert_eq!(
                s.decode_rejected,
                s.received - s.retained - s.userspace_dropped
            );
            assert!(s.decode_rejected > partial.decode_rejected);
            assert!(bundle.facts.is_empty());
            assert!(s.incomplete && s.has_loss() && !s.final_accounted);
            assert_eq!(s.state, CaptureState::Running);
            assert_eq!(bundle.replay().unwrap().capture, bundle.capture);
            return bundle;
        }
        assert!(
            Instant::now() < deadline,
            "idle partial groups did not expire"
        );
        thread::sleep(Duration::from_millis(5));
    }
}

fn audit(
    bundle: &ExportBundle,
    identities: &[Value],
    phase: usize,
    seen: &mut BTreeSet<u64>,
    namespaces: &mut BTreeMap<(usize, bool), u32>,
    profile: Profile,
) -> Value {
    let partial = phase % profile.stride() == 1;
    let mut expected = Vec::new();
    let mut per_peer = BTreeMap::<usize, Vec<Packet>>::new();
    let mut records = Vec::new();
    let mut baseline = BTreeSet::new();
    let mut observations = BTreeSet::new();
    let mut groups = BTreeMap::<u64, Vec<u16>>::new();
    for fact in &bundle.facts {
        assert!(seen.insert(fact.id.0), "duplicate retained fact");
        let FactKind::PacketMeta(p) = &fact.kind else {
            panic!("non-packet")
        };
        let cookie = p.sk_cookie.unwrap().to_string();
        let (peer_index, identity) = identities
            .iter()
            .enumerate()
            .find(|(_, i)| {
                i["cookies"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|id| id == &cookie)
            })
            .unwrap();
        let (lane, server) = (peer_index / 2, peer_index % 2 == 1);
        let family = identity["cookies"]
            .as_array()
            .unwrap()
            .iter()
            .position(|id| id == &cookie)
            .unwrap();
        assert_eq!(
            fact.ifindex,
            Some(identity["ifindex"].as_u64().unwrap() as u32)
        );
        let namespace = namespaces.entry((lane, server)).or_insert(p.netns);
        assert_eq!(*namespace, p.netns);
        let block = u16::from_be_bytes([p.payload_bytes[&0], p.payload_bytes[&1]]) as usize;
        let iteration = u16::from_be_bytes([p.payload_bytes[&2], p.payload_bytes[&3]]) as usize;
        let part = usize::from(p.payload_bytes[&5]);
        assert_eq!(usize::from(p.payload_bytes[&4]), family);
        assert_eq!(usize::from(p.payload_bytes[&8]), lane);
        if partial {
            assert!(block == phase - 1 || block == phase);
        } else {
            assert_eq!(block, phase);
        }
        let (iterations, count) = profile.shape(block, lane);
        assert!(iteration < iterations && (part == 255 || part < count));
        let outbound = if part == 255 { server } else { !server };
        assert_eq!(
            p.dir,
            if outbound {
                gewyvern::ledger::PacketDir::Egress
            } else {
                gewyvern::ledger::PacketDir::Ingress
            }
        );
        if outbound {
            assert_eq!(json!(fact.cpu.0), identity["cpu"]);
        }
        assert!(
            observations.insert((peer_index, block, iteration, family, part)),
            "duplicate native observation"
        );
        let length = if part == 255 {
            32
        } else if part + 1 == count {
            17
        } else {
            SEGMENT
        };
        expected.push(packet(
            cookie.parse().unwrap(),
            endpoint(family, server),
            endpoint(family, !server),
            outbound,
            &data(block, lane, iteration, family, part, length),
        ));
        per_peer
            .entry(peer_index)
            .or_default()
            .push(expected.last().unwrap().clone());
        if partial {
            assert!(
                p.udp_offload.is_none(),
                "partial pressure group leaked to export"
            );
            if block == phase - 1 {
                baseline.insert((server, family, part));
            } else {
                assert_eq!(part, 255, "pressure request unexpectedly retained");
            }
        } else if part != 255 {
            let meta = p.udp_offload.unwrap();
            assert!(meta.matches_packet(p));
            assert_eq!(
                (meta.segment_size, meta.segment_count, meta.segment_index),
                (1024, 2, part as u16)
            );
            assert_eq!(meta.aggregate_payload_len, 1041);
            let lane = phase % profile.stride() - (profile.stride() - 4);
            let start = (phase / profile.stride() * profile.groups()
                + 32
                + 4 * usize::from(profile.mixed())
                + lane * 4
                + 1) as u64;
            assert!((start..start + 4).contains(&meta.aggregate_id));
            groups
                .entry(meta.aggregate_id)
                .or_default()
                .push(meta.segment_index);
        } else {
            assert!(p.udp_offload.is_none());
        }
        records.push(
            json!({"id":fact.id.0,"block":block,"lane":lane,"server":server,
            "family":family,"iteration":iteration,"part":part,"cpu":fact.cpu.0,"netns":p.netns,
            "group":p.udp_offload.map(|m| m.aggregate_id)}),
        );
    }
    for (peer, expected) in per_peer {
        let mut selected = bundle.clone();
        selected.facts.retain(|fact| matches!(&fact.kind,FactKind::PacketMeta(p)
            if p.sk_cookie.is_some_and(|cookie| identities[peer]["cookies"].as_array().unwrap().iter().any(|id| id==&cookie.to_string()))));
        check_packets(&selected, &expected);
    }
    if partial {
        assert_eq!(baseline.len(), 8, "ordinary prelude did not survive");
        assert!((8..=16).contains(&bundle.facts.len()));
        if profile.mixed() {
            assert_eq!(bundle.facts.len(), 8);
        }
    } else {
        assert_eq!(bundle.facts.len(), 12);
        assert_eq!(groups.len(), 4);
        for indexes in groups.values_mut() {
            indexes.sort_unstable();
            assert_eq!(*indexes, [0, 1]);
        }
    }
    json!({"phase":phase,"capture":bundle.capture,"records":records,
        "expected_sha256":hash(&serde_json::to_vec(&json!(expected)).unwrap())})
}

#[test]
#[ignore = "root/BPF/8 CPUs; GEWY_OFFLOAD_CONCURRENT_CPUS=list and GEWY_OFFLOAD_PRESSURE_OUT=new-directory"]
fn production_capture_multi_cpu_ring_loss_and_recovery() {
    run_pressure(Profile::RingOnly, "GEWY_OFFLOAD_PRESSURE_OUT");
}

#[test]
#[ignore = "root/BPF/8 CPUs; GEWY_OFFLOAD_CONCURRENT_CPUS=list and GEWY_OFFLOAD_MIXED_OUT=new-directory"]
fn production_capture_multi_cpu_mixed_limits_and_recovery() {
    run_pressure(Profile::Mixed, "GEWY_OFFLOAD_MIXED_OUT");
}

fn run_pressure(profile: Profile, output_variable: &str) {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let cpus = cpus();
    let out = PathBuf::from(std::env::var_os(output_variable).unwrap());
    fs::create_dir(&out).unwrap();
    let mut networks: Vec<_> = (0..LANES).map(|_| Network::new()).collect();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-netns-pressure-{}-{nonce}",
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
            "120",
            "--capture-window-ms",
            "600",
            "--capture-max-facts",
            &profile.limit().to_string(),
            "--capture-ring-kib",
            "4",
        ],
    );
    ready(&mut capture, &dir);
    let run = snapshot(&dir).run_id;
    pause_idle(&mut capture, &dir, &run);
    let mut peers = Vec::new();
    for (lane, network) in networks.iter().enumerate() {
        let root = out.join(lane.to_string());
        fs::create_dir(&root).unwrap();
        for (role, server) in [("client", false), ("server", true)] {
            let path = root.join(role);
            let peer = worker(
                &network.0[usize::from(server)],
                role,
                &scope,
                &path,
                if profile.mixed() {
                    "network::offload::service::concurrent::ring_pressure::udp_offload_mixed_pressure_worker"
                } else {
                    "network::offload::service::concurrent::ring_pressure::udp_offload_pressure_worker"
                },
            );
            peers.push((peer, path, server));
        }
    }
    let mut identities = Vec::new();
    for (peer, path, _) in &mut peers {
        await_file(peer, &path.join("ready"));
        identities.push(
            serde_json::from_slice::<Value>(&fs::read(path.join("identity.json")).unwrap())
                .unwrap(),
        );
    }
    let mut stages = Vec::new();
    let mut expirations = Vec::new();
    let mut budget_probes = Vec::new();
    let mut seen = BTreeSet::new();
    let mut namespaces = BTreeMap::new();
    let mut rejected = 0;
    let mut retained = 0;
    let mut budget = 0;
    for cycle in 0..CYCLES {
        let base = cycle * profile.stride();
        dispatch(&mut peers, base, profile);
        dispatch(&mut peers, base + 1, profile);
        thread::sleep(Duration::from_millis(650));
        signal(&capture, libc::SIGCONT);
        let partial = frozen_window(
            &mut capture,
            &dir,
            &run,
            (cycle * profile.received() + 17) as u64,
        );
        let s = partial.capture.as_ref().unwrap();
        assert_eq!(s.kernel.matched, (cycle * profile.matched() + 552) as u64);
        assert_eq!(s.kernel.ring_dropped, (cycle * profile.lost() + 535) as u64);
        assert_eq!(
            s.kernel.unsupported + s.kernel.malformed + s.kernel.no_cookie,
            0
        );
        assert_eq!(s.decode_rejected, rejected);
        retained += partial.facts.len() as u64;
        assert_eq!(s.retained, retained);
        let pending = s.received - s.retained - s.decode_rejected - s.userspace_dropped;
        assert!((1..=9).contains(&pending));
        if profile.mixed() {
            budget += 9 - pending;
        }
        assert_eq!(s.userspace_dropped, budget);
        assert!(!s.reconciles() && s.incomplete && !s.final_accounted);
        stages.push(audit(
            &partial,
            &identities,
            base + 1,
            &mut seen,
            &mut namespaces,
            profile,
        ));
        let mut waiting = s.clone();
        if profile.mixed() {
            dispatch(&mut peers, base + 2, profile);
            thread::sleep(Duration::from_millis(650));
            signal(&capture, libc::SIGCONT);
            let probe = freeze_publication(
                &mut capture,
                &dir,
                &run,
                (cycle * profile.received() + 34) as u64,
            );
            assert!(probe.facts.is_empty(), "refused probe produced facts");
            waiting = probe.capture.as_ref().unwrap().clone();
            budget += 17;
            assert_eq!(waiting.userspace_dropped, budget);
            assert_eq!(waiting.retained, retained);
            assert_eq!(
                waiting.decode_rejected, rejected,
                "original reservation expired before the probe"
            );
            assert_eq!(
                waiting.kernel.matched,
                (cycle * profile.matched() + 620) as u64
            );
            assert_eq!(
                waiting.kernel.ring_dropped,
                (cycle * profile.lost() + 586) as u64
            );
            assert_eq!(
                waiting.kernel.unsupported + waiting.kernel.malformed + waiting.kernel.no_cookie,
                0
            );
            assert_eq!(
                waiting.received
                    - waiting.retained
                    - waiting.userspace_dropped
                    - waiting.decode_rejected,
                pending
            );
            budget_probes.push(json!({"cycle":cycle,"phase":base+2,"capture":waiting,"facts":0}));
        }
        let since = Instant::now();
        signal(&capture, libc::SIGCONT);
        let expired = expired(&mut capture, &dir, &waiting);
        assert!(since.elapsed() >= Duration::from_secs(1));
        rejected += pending;
        assert_eq!(expired.capture.as_ref().unwrap().decode_rejected, rejected);
        expirations.push(json!({"cycle":cycle,"pending_members":pending,"elapsed_ms":since.elapsed().as_secs_f64()*1000.0,
            "capture":expired.capture,"facts":expired.facts.len()}));
        for lane in 0..LANES {
            let phase = base + profile.stride() - 4 + lane;
            dispatch(&mut peers, phase, profile);
            signal(&capture, libc::SIGCONT);
            let recovery = frozen_window(
                &mut capture,
                &dir,
                &run,
                (cycle * profile.received()
                    + 17
                    + 17 * usize::from(profile.mixed())
                    + (lane + 1) * 12) as u64,
            );
            let s = recovery.capture.as_ref().unwrap();
            retained += 12;
            assert_eq!(s.retained, retained);
            assert_eq!(s.decode_rejected, rejected);
            assert_eq!(
                s.kernel.matched,
                (cycle * profile.matched()
                    + 552
                    + 68 * usize::from(profile.mixed())
                    + (lane + 1) * 12) as u64
            );
            assert_eq!(s.kernel.ring_dropped, ((cycle + 1) * profile.lost()) as u64);
            assert!(s.reconciles() && s.incomplete && !s.final_accounted);
            assert_eq!(
                s.kernel.unsupported + s.kernel.malformed + s.kernel.no_cookie,
                0
            );
            assert_eq!(s.userspace_dropped, budget);
            stages.push(audit(
                &recovery,
                &identities,
                phase,
                &mut seen,
                &mut namespaces,
                profile,
            ));
        }
    }
    assert_eq!(namespaces.len(), 8);
    assert_eq!(namespaces.values().collect::<BTreeSet<_>>().len(), 8);
    assert_eq!(seen.len() as u64, retained);
    signal(&capture, libc::SIGCONT);
    signal(&capture, libc::SIGTERM);
    assert_eq!(wait(&mut capture, 10).code(), Some(3));
    let terminal = snapshot(&dir);
    assert!(terminal.final_accounted && terminal.reconciles() && terminal.incomplete);
    assert_eq!(terminal.state, CaptureState::Stopped);
    let mut expected: CaptureSnapshot =
        serde_json::from_value(stages.last().unwrap()["capture"].clone()).unwrap();
    expected.state = CaptureState::Stopped;
    expected.final_accounted = true;
    assert_eq!(terminal, expected);
    assert!(export(&dir).facts.is_empty());
    detached(&scope);
    let mut native = Vec::new();
    for (peer, path, _) in &mut peers {
        writeln!(peer.0.stdin.as_mut().unwrap(), "finish").unwrap();
        assert!(wait(peer, 5).success());
        native.push(
            serde_json::from_slice::<Value>(&fs::read(path.join("oracle.json")).unwrap()).unwrap(),
        );
    }
    fs::remove_dir(&scope.0).unwrap();
    for network in &mut networks {
        network.remove();
    }
    fs::write(out.join("summary.json"),serde_json::to_vec_pretty(&json!({"schema_version":1,"ok":true,
        "production_cli":true,"cpus":cpus,"cycles":CYCLES,"ring_kib":4,"ring_records":17,"max_facts":profile.limit(),
        "window_ms":600,"stages":stages,"expirations":expirations,"budget_probes":budget_probes,"terminal":terminal,"native":native,
        "native_requests":822+96*usize::from(profile.mixed()),"native_replies":78+6*usize::from(profile.mixed()),"same_sockets_recovery":true,"capture_exit":3,
        "packet_multiset_matches":true,"replay_identical":true,"idle_expiry_without_traffic":true,
        "no_partial_groups_published":true,"hooks_detached":true,"owned_resources_removed":true,
        "host_network_changed":false,"binary_sha256":hash(&fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap()),
        "limits":["single_host_private_veth","induced_ring_loss_not_network_loss","paced_not_throughput_benchmark",
            "no_late_kernel_tail_recovery_claim","no_durable_archive_claim"]})).unwrap()).unwrap();
}
