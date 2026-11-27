use super::*;
use std::os::unix::process::ExitStatusExt;

#[path = "retention_interruption.rs"]
mod retention_interruption;

fn history_view(state: &Path, json: bool) -> String {
    history_view_at(state, json, None)
}

fn history_view_at(state: &Path, json: bool, retention: Option<usize>) -> String {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_gewyvern"));
    cmd.env("GEWY_STATE_HOME", state).arg("--list-history");
    if let Some(cap) = retention {
        cmd.env("GEWY_HISTORY_RETENTION", cap.to_string());
    }
    if json {
        cmd.arg("--json");
    }
    let out = cmd.output().unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

#[test]
#[ignore = "Linux root/BPF; GEWY_ARCHIVE_INTERRUPT_OUT=new-directory"]
fn production_capture_interrupted_history_publication() {
    let root = PathBuf::from(std::env::var_os("GEWY_ARCHIVE_INTERRUPT_OUT").unwrap());
    fs::create_dir(&root).unwrap();
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-archive-{}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    let runtime = root.join("runtime");
    let dir = root.join("capture");
    for path in [&runtime, &dir] {
        fs::create_dir(path).unwrap();
    }
    let state = runtime.join("state");
    let history = state.join("history/api/v1");
    let peers = crate::peers();
    let addr = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let address = addr.to_string();
    let spawn = || {
        collector_with_output_dir(
            &scope,
            peers.port,
            &runtime,
            &dir,
            &[
                "--serve",
                "--capture-window-ms",
                "200",
                "--api-socket",
                &address,
            ],
        )
    };
    let traffic = || assert!(wait(&mut crate::worker(&scope, &peers, 1, 0, &root), 10).success());
    let mut seed = spawn();
    let seed_id = wait_for_run(&mut seed, &dir, None);
    window(&mut seed, &dir, &seed_id, 0);
    let old = clean_stop(&mut seed, &dir, 0, 0);
    let index = fs::read(history.join("index.json")).unwrap();
    let indexed: Value = serde_json::from_slice(&index).unwrap();
    let seed_dir = state.join(indexed["entries"][0]["path"].as_str().unwrap());
    let mut active = spawn();
    let id = wait_for_run(&mut active, &dir, Some(&seed_id));
    traffic();
    window(&mut active, &dir, &id, 4);
    signal(&active, libc::SIGTERM);
    // Observe the live product's writes, without a test hook or a timed kill.
    let deadline = Instant::now() + Duration::from_secs(10);
    let partial = loop {
        if let Some(path) = fs::read_dir(&history)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| p != &seed_dir && p.join("meta.json").is_file())
        {
            signal(&active, libc::SIGKILL);
            break path;
        }
        assert!(
            Instant::now() < deadline && active.0.try_wait().unwrap().is_none(),
            "missed archive write"
        );
        thread::sleep(Duration::from_millis(1));
    };
    assert_eq!(wait(&mut active, 10).signal(), Some(libc::SIGKILL));
    assert!(
        !partial.join("targets").exists(),
        "did not interrupt an incomplete archive"
    );
    detached(&scope);
    let terminal = snapshot(&dir);
    assert_eq!(terminal.run_id, id);
    assert!(terminal.final_accounted && terminal.reconciles() && !terminal.incomplete);
    assert_eq!(terminal.state, CaptureState::Stopped);
    assert_eq!(terminal.retained, 4);
    assert_eq!(
        export(&state.join("latest/api/v1/latest")).capture,
        Some(terminal)
    );
    assert_eq!(fs::read(history.join("index.json")).unwrap(), index);
    let disk_view = history_view(&state, false);
    fs::write(root.join("history-at-kill.txt"), &disk_view).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&history_view(&state, true)).unwrap(),
        indexed
    );
    assert!(disk_view.lines().any(|l| l == "entries: 1"), "{disk_view}");
    let mut next = spawn();
    let new_id = wait_for_run(&mut next, &dir, Some(&id));
    assert_ne!(new_id, seed_id);
    assert!(window(&mut next, &dir, &new_id, 0).facts.is_empty());
    let mut connection =
        std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
    connection
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    connection
        .write_all(b"GET /health HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    std::io::Read::take(connection, 16384)
        .read_to_string(&mut response)
        .unwrap();
    let (headers, body) = response.split_once("\r\n\r\n").unwrap();
    assert!(headers.starts_with("HTTP/1.1 200 "), "{headers}");
    let health: Value = serde_json::from_str(body).unwrap();
    assert_eq!(health["capture"]["run_id"], new_id);
    assert_eq!(health["capture"]["state"], "running");
    traffic();
    window(&mut next, &dir, &new_id, 4);
    let new = clean_stop(&mut next, &dir, 4, 0);
    api_archive::check(&state, &[&new, &old]);
    assert!(
        history_view(&state, false)
            .lines()
            .any(|l| l == "entries: 2")
    );
    assert!(!partial.exists());
    detached(&scope);
    fs::write(
        root.join("summary.json"),
        serde_json::to_vec_pretty(&json!({
            "seed_run":seed_id,"interrupted_run":id,"repaired_run":new_id,"signal":9,
            "latest_at_kill_retained":4,"history_at_kill":1,"history_after_restart":2,
            "unchanged_index_sha256":hash(&index),"repaired_retained":new.retained,
            "restart_health_run":health["capture"]["run_id"]
        }))
        .unwrap(),
    )
    .unwrap();
}
