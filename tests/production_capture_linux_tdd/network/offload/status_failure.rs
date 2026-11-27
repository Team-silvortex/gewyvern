//! Failure of the status destination after capture is ready, with --out still writable.
use super::*;

#[path = "read_only_outputs.rs"]
mod read_only_outputs;

#[test]
#[ignore = "root/BPF; GEWY_OFFLOAD_EXPIRY_FAMILY=4|6, GEWY_STATUS_CASE=running|terminal[-api], GEWY_STATUS_OUT=new-directory"]
fn production_capture_status_write_failure_and_repair() {
    let case = std::env::var("GEWY_STATUS_CASE").unwrap();
    assert!(matches!(
        case.as_str(),
        "running" | "terminal" | "running-api" | "terminal-api"
    ));
    let api = case.ends_with("-api");
    let mut options = OPTIONS.to_vec();
    if api {
        options.extend(["--api-socket", "127.0.0.1:0"]);
    }
    let mut f = start_partial_with_options("GEWY_STATUS_OUT", &options);
    pause_main_loop(&mut f.capture, &f.capture_dir);
    let path = f.capture_dir.join("status.json");
    let backup = f.out.join("before-failure.json");
    fs::rename(&path, &backup).unwrap();
    fs::create_dir(&path).unwrap();
    phase(&mut f.server, &mut f.client, &f.out, 5);
    if case.starts_with("terminal") {
        signal(&f.capture, libc::SIGTERM);
    }
    signal(&f.capture, libc::SIGCONT);
    assert_eq!(wait(&mut f.capture, 10).code(), Some(1));
    detached(&f.scope);
    let log = fs::read_to_string(f.capture_dir.join("stderr.log")).unwrap();
    let error: Value = serde_json::from_str(log.lines().last().unwrap()).unwrap();
    assert_eq!(error["error"]["code"], "capture_status_write_failed");
    assert!(path.is_dir(), "collector removed the obstructing directory");
    let failed = export(&f.capture_dir);
    let s = failed.capture.as_ref().unwrap();
    assert_eq!(s.state, CaptureState::Failed);
    assert!(s.final_accounted && s.reconciles() && s.incomplete);
    assert_eq!(s.run_id, f.partial.capture.as_ref().unwrap().run_id);
    assert_eq!(
        [
            s.kernel.matched,
            s.kernel.ring_dropped,
            s.received,
            s.retained,
            s.userspace_dropped,
            s.decode_rejected
        ],
        [42, 8, 34, 9, 16, 9]
    );
    check_packets(&failed, &client_packets(&f.out, 5, true));
    let replay = failed.replay().unwrap();
    assert_eq!(replay.facts, failed.facts);
    assert_eq!(replay.capture, failed.capture);
    if api {
        let history = fs::read_dir(f.capture_dir.join("state/history/api/v1"))
            .unwrap()
            .map(Result::unwrap)
            .filter(|e| e.file_type().unwrap().is_dir())
            .collect::<Vec<_>>();
        assert_eq!(
            history.len(),
            1,
            "one failure, not a successful stop followed by a failure"
        );
        for dir in [
            f.capture_dir.join("state/latest/api/v1/latest"),
            history[0].path(),
        ] {
            let persisted = export(&dir);
            assert_eq!(persisted.capture, failed.capture);
            assert_eq!(persisted.facts, failed.facts);
        }
    }
    fs::copy(
        f.capture_dir.join("export.json"),
        f.out.join("failed-export.json"),
    )
    .unwrap();
    fs::copy(f.capture_dir.join("stderr.log"), f.out.join("failure.log")).unwrap();
    fs::remove_dir(&path).unwrap();
    fs::copy(&backup, &path).unwrap();
    phase(&mut f.server, &mut f.client, &f.out, 6);
    let mut restarted = collector_with_options(&f.scope, PORT, &f.capture_dir, &options);
    let id = wait_for_run(&mut restarted, &f.capture_dir, Some(&s.run_id));
    assert!(
        window(&mut restarted, &f.capture_dir, &id, 0)
            .facts
            .is_empty()
    );
    phase(&mut f.server, &mut f.client, &f.out, 7);
    let recovery = window(&mut restarted, &f.capture_dir, &id, 5);
    check_recovery(&recovery, &f.out, 7, 1);
    let terminal = clean_stop(&mut restarted, &f.capture_dir, 5, 0);
    assert_eq!(terminal.run_id, id);
    finish_peers(&mut f);
    fs::write(
        f.out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({
            "case":case,"family":if family()==0 {4} else {6},"error":error["error"]["code"],
            "failed":s,"failed_window_facts":failed.facts.len(),"repaired":terminal
        }))
        .unwrap(),
    )
    .unwrap();
}
