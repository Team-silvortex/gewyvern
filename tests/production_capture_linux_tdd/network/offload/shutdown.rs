//! Detach/drain an occupied ring before the admitted partial group can expire.
use super::*;
use std::os::unix::process::ExitStatusExt;

#[path = "status_failure.rs"]
mod status_failure;

fn client_packets(out: &Path, phase: usize, reply_only: bool) -> Vec<Packet> {
    let row: Value =
        serde_json::from_slice(&fs::read(out.join(format!("client/phase-{phase}.json"))).unwrap())
            .unwrap();
    row["expected"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| !reply_only || p["outbound"] == false)
        .map(|p| serde_json::from_value(p.clone()).unwrap())
        .collect()
}

#[test]
#[ignore = "root/BPF/iproute2; GEWY_OFFLOAD_EXPIRY_FAMILY=4|6 and GEWY_OFFLOAD_SHUTDOWN_OUT=new-directory"]
fn production_capture_offload_shutdown_drain_and_restart() {
    let mut f = start_partial("GEWY_OFFLOAD_SHUTDOWN_OUT");
    pause_main_loop(&mut f.capture, &f.capture_dir);
    phase(&mut f.server, &mut f.client, &f.out, 5);
    // The peer exchange is complete, but its records are queued. Deliver TERM
    // before CONT so the next loop boundary stops instead of publishing a window.
    signal(&f.capture, libc::SIGTERM);
    signal(&f.capture, libc::SIGCONT);
    assert_eq!(wait(&mut f.capture, 10).code(), Some(3));
    let elapsed = f.resumed.elapsed();
    assert!(
        elapsed < gewyvern::live_capture::UDP_OFFLOAD_WAIT,
        "test missed pre-expiry shutdown: {elapsed:?}"
    );
    let old = snapshot(&f.capture_dir);
    let partial = f.partial.capture.clone().unwrap();
    let mut expected = partial.clone();
    expected.state = CaptureState::Stopped;
    expected.final_accounted = true;
    expected.kernel.matched += 17;
    expected.received += 17;
    expected.retained += 1;
    expected.userspace_dropped += 16;
    expected.decode_rejected += 9;
    assert_eq!(old, expected);
    assert!(old.reconciles() && old.incomplete);
    let drained = export(&f.capture_dir);
    assert_eq!(drained.capture.as_ref(), Some(&old));
    let replay = drained.replay().unwrap();
    assert_eq!(replay.capture, drained.capture);
    assert_eq!(replay.facts, drained.facts);
    let baseline: Vec<_> = (0..4)
        .flat_map(|id| client_packets(&f.out, id, false))
        .collect();
    check_packets(&f.partial, &baseline);
    check_packets(&drained, &client_packets(&f.out, 5, true));
    assert_eq!(drained.facts.len(), 1);
    assert!(matches!(&drained.facts[0].kind, FactKind::PacketMeta(p) if p.udp_offload.is_none()));
    detached(&f.scope);

    let dir = f.out.join("restart");
    fs::create_dir(&dir).unwrap();
    let mut restarted = collector_with_options(&f.scope, PORT, &dir, OPTIONS);
    ready(&mut restarted, &dir);
    let run = snapshot(&dir).run_id;
    assert_ne!(run, old.run_id);
    for (id, received) in [(6, 17), (7, 22)] {
        phase(&mut f.server, &mut f.client, &f.out, id);
        let bundle = window(&mut restarted, &dir, &run, received);
        check_recovery(&bundle, &f.out, id, (id - 5) as u64);
    }
    let new = clean_stop(&mut restarted, &dir, 22, 0);
    assert_eq!(new.run_id, run);
    finish_peers(&mut f);
    fs::write(
        f.out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version":1, "family":if family()==0 {4} else {6},
            "pending_window":partial, "stopped":old, "restart":new,
            "resume_to_exit_ms":elapsed.as_millis() as u64,
            "final_window_facts":drained.facts.len(), "lossy_exit":3, "restart_exit":0,
            "binary_sha256":hash(&fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap())
        }))
        .unwrap(),
    )
    .unwrap();
}

fn check_recovery(bundle: &ExportBundle, out: &Path, id: usize, group: u64) {
    check_packets(bundle, &client_packets(out, id, false));
    let mut indices = BTreeSet::new();
    for fact in &bundle.facts {
        let FactKind::PacketMeta(p) = &fact.kind else {
            panic!("non-packet")
        };
        if let Some(meta) = p.udp_offload {
            assert!(meta.matches_packet(p));
            assert_eq!(meta.aggregate_id, group);
            assert_eq!(usize::from(meta.segment_count), phase_count(id));
            assert_eq!(p.dir, gewyvern::ledger::PacketDir::Egress);
            assert!(indices.insert(usize::from(meta.segment_index)));
        }
    }
    assert_eq!(indices, (0..phase_count(id)).collect());
}

fn finish_peers(f: &mut PartialRun) {
    for peer in [&mut f.client, &mut f.server] {
        writeln!(peer.0.stdin.as_mut().unwrap(), "finish").unwrap();
        assert!(wait(peer, 5).success());
    }
    for scope in [&f.scope, &f.server_scope] {
        detached(scope);
        fs::remove_dir(&scope.0).unwrap();
    }
    f.network.remove();
}

#[test]
#[ignore = "root/BPF/iproute2; GEWY_OFFLOAD_EXPIRY_FAMILY=4|6 and GEWY_OFFLOAD_CRASH_OUT=new-directory"]
fn production_capture_offload_kill_pending_and_restart_in_place() {
    let mut f = start_partial("GEWY_OFFLOAD_CRASH_OUT");
    pause_main_loop(&mut f.capture, &f.capture_dir);
    let files =
        || ["status.json", "export.json"].map(|file| fs::read(f.capture_dir.join(file)).unwrap());
    let before = files();
    let stale = snapshot(&f.capture_dir);
    assert_eq!(f.partial.capture.as_ref(), Some(&stale));
    assert!(!stale.final_accounted && !stale.reconciles() && stale.incomplete);
    let published = export(&f.capture_dir);
    assert_eq!(published.facts, f.partial.facts);
    assert_eq!(published.capture, f.partial.capture);
    phase(&mut f.server, &mut f.client, &f.out, 5);
    signal(&f.capture, libc::SIGKILL);
    assert_eq!(wait(&mut f.capture, 10).signal(), Some(libc::SIGKILL));
    detached(&f.scope);
    assert_eq!(files(), before, "SIGKILL must not publish a final window");
    phase(&mut f.server, &mut f.client, &f.out, 6);
    assert_eq!(
        files(),
        before,
        "traffic while detached changed persisted evidence"
    );

    // Keep copies outside the reused output directory for independent inspection.
    fs::write(f.out.join("stale-status.json"), &before[0]).unwrap();
    fs::write(f.out.join("stale-export.json"), &before[1]).unwrap();
    let mut restarted = collector_with_options(&f.scope, PORT, &f.capture_dir, OPTIONS);
    let id = wait_for_run(&mut restarted, &f.capture_dir, Some(&stale.run_id));
    let empty = window(&mut restarted, &f.capture_dir, &id, 0);
    assert!(empty.facts.is_empty());
    let fresh = empty.capture.unwrap();
    assert!(fresh.reconciles() && !fresh.has_loss() && !fresh.incomplete);
    assert_eq!(fresh.cgroup, stale.cgroup);
    phase(&mut f.server, &mut f.client, &f.out, 7);
    let recovery = window(&mut restarted, &f.capture_dir, &id, 5);
    check_recovery(&recovery, &f.out, 7, 1);
    let terminal = clean_stop(&mut restarted, &f.capture_dir, 5, 0);
    assert_eq!(terminal.run_id, id);
    finish_peers(&mut f);
    fs::write(
        f.out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version":1,"family":if family()==0 {4} else {6},"signal":libc::SIGKILL,
            "stale":stale,"fresh":fresh,"terminal":terminal,"stale_status_sha256":hash(&before[0]),
            "stale_export_sha256":hash(&before[1]),"stale_window_facts":published.facts.len(),
            "recovery_window_facts":recovery.facts.len(),"restart_exit":0,
            "binary_sha256":hash(&fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap())
        }))
        .unwrap(),
    )
    .unwrap();
}
