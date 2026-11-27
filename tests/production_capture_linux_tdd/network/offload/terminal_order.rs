use super::*;
#[path = "backfill_interruption.rs"]
mod backfill_interruption;
#[path = "expired_terminals.rs"]
mod expired_terminals;
use backfill_interruption::{Cut, Delayed, Fault};

fn api(addr: std::net::SocketAddr, path: &str) -> Value {
    let mut stream = std::net::TcpStream::connect_timeout(&addr, Duration::from_secs(2)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
    )
    .unwrap();
    let mut reply = String::new();
    stream.take(65536).read_to_string(&mut reply).unwrap();
    let (headers, body) = reply.split_once("\r\n\r\n").unwrap();
    assert!(headers.starts_with("HTTP/1.1 200 "), "{headers}");
    serde_json::from_str(body).unwrap()
}

fn json_file(p: &Path) -> Value {
    serde_json::from_slice(&fs::read(p).unwrap()).unwrap()
}

#[test]
#[ignore = "Linux root/BPF/fanotify; GEWY_ORDER_KEEP_OUT=new-directory"]
fn production_capture_late_terminal_fits() {
    exercise("GEWY_ORDER_KEEP_OUT", 1, 4, None);
}

#[test]
#[ignore = "Linux root/BPF/fanotify; GEWY_ORDER_FULL_OUT=new-directory"]
fn production_capture_late_terminal_outside_limit() {
    exercise("GEWY_ORDER_FULL_OUT", 4, 1, None);
}

#[test]
#[ignore = "Linux root/BPF/fanotify; GEWY_BACKFILL_PRE_OUT=new-directory"]
fn production_capture_backfill_before_index_restart() {
    exercise(
        "GEWY_BACKFILL_PRE_OUT",
        1,
        4,
        Some(Fault::Index(Cut::Before)),
    );
}

#[test]
#[ignore = "Linux root/BPF/fanotify; GEWY_BACKFILL_POST_OUT=new-directory"]
fn production_capture_backfill_after_index_restart() {
    exercise(
        "GEWY_BACKFILL_POST_OUT",
        1,
        4,
        Some(Fault::Index(Cut::After)),
    );
}

#[test]
#[ignore = "Linux root/BPF/fanotify; GEWY_REPLAY_RECLAIM_OUT=new-directory"]
fn production_capture_backfill_reclaim_replay_killed() {
    exercise(
        "GEWY_REPLAY_RECLAIM_OUT",
        1,
        4,
        Some(Fault::Replay(Cut::Before)),
    );
}

#[test]
#[ignore = "Linux root/BPF/fanotify; GEWY_REPLAY_PRUNE_OUT=new-directory"]
fn production_capture_backfill_prune_replay_killed() {
    exercise(
        "GEWY_REPLAY_PRUNE_OUT",
        1,
        4,
        Some(Fault::Replay(Cut::After)),
    );
}

#[test]
#[ignore = "Linux root/BPF/fanotify; GEWY_EXPIRED_RECLAIM_OUT=new-directory"]
fn production_capture_expired_only_reclaim() {
    exercise(
        "GEWY_EXPIRED_RECLAIM_OUT",
        1,
        4,
        Some(Fault::Expired(Cut::Before)),
    );
}

#[test]
#[ignore = "Linux root/BPF/fanotify; GEWY_EXPIRED_PRUNE_OUT=new-directory"]
fn production_capture_expired_only_prune() {
    exercise(
        "GEWY_EXPIRED_PRUNE_OUT",
        1,
        4,
        Some(Fault::Expired(Cut::After)),
    );
}

// Freeze a timestamped terminal before it acquires the shared shelf lock.
fn hold_terminal(p: &Process, history: &Path, addr: std::net::SocketAddr, id: &str) -> u64 {
    let mut ts = 0;
    shrink_interruption::at_writer_open(p, &history.join(".gewyvern-writer-v1.lock"), || {
        let health = api(addr, "/health");
        assert_eq!(health["capture"]["state"], "stopped");
        assert_eq!(health["capture"]["run_id"], id);
        ts = api(addr, "/v1/latest/meta")["updated_unix_ms"]
            .as_u64()
            .unwrap();
        signal(p, libc::SIGSTOP);
        true
    });
    let mut status = 0;
    let pid = p.0.id() as i32;
    assert_eq!(
        unsafe { libc::waitpid(pid, &mut status, libc::WUNTRACED) },
        pid
    );
    assert!(libc::WIFSTOPPED(status) && ts > 0);
    ts
}

fn exercise(out: &str, early_cap: usize, head_cap: usize, cut: Option<Fault>) {
    with_shelf(out, |dir, state, head_scope, start| {
        let history = state.join("history/api/v1");
        let latest = state.join("latest/api/v1/latest");
        let root = dir.parent().unwrap();
        let expired = if matches!(cut, Some(Fault::Expired(_))) {
            clean_stop(&mut start(4).0, dir, 4, 0); // Create the writer lock before parking.
            [1, 8, 4]
                .into_iter()
                .map(|cap| expired_terminals::Parked::new(state, root, head_scope, cap))
                .collect()
        } else {
            Vec::new()
        };
        let seeds: Vec<_> = (0..3)
            .map(|_| clean_stop(&mut start(4).0, dir, 4, 0))
            .collect();
        let (mut head, head_id) = start(head_cap);
        let early_scope = Scope(head_scope.0.with_file_name(format!(
            "gewyvern-production-test-order-early-{}",
            std::process::id()
        )));
        fs::create_dir(&early_scope.0).unwrap();
        let early_dir = root.join("early");
        fs::create_dir(&early_dir).unwrap();
        let peers = crate::peers();
        let addr = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let spawn = |cap: usize| {
            Process(
                collector_command(
                    &early_scope,
                    peers.port,
                    &early_dir,
                    &early_dir,
                    &[
                        "--serve",
                        "--capture-window-ms",
                        "200",
                        "--api-socket",
                        &addr.to_string(),
                    ],
                )
                .env("GEWY_STATE_HOME", state)
                .env("GEWY_HISTORY_RETENTION", cap.to_string())
                .spawn()
                .unwrap(),
            )
        };
        let mut early = spawn(early_cap);
        let early_id = wait_for_run(&mut early, &early_dir, None);
        assert_ne!(head_id, early_id);
        assert!(
            wait(
                &mut crate::worker(&early_scope, &peers, 2, 0, &early_dir),
                10
            )
            .success()
        );
        window(&mut early, &early_dir, &early_id, 8);
        let early_ts = hold_terminal(&early, &history, addr, &early_id);
        let newest = clean_stop(&mut head, dir, 4, 0);
        let before_index = fs::read(history.join("index.json")).unwrap();
        let head_ts = json_file(&latest.join("meta.json"))["updated_unix_ms"]
            .as_u64()
            .unwrap();
        assert!(0 < early_ts && early_ts < head_ts);
        assert_eq!(export(&latest).capture.as_ref(), Some(&newest));
        let tree = |p: &Path| api_archive::fingerprint_paths(p, &["."]);
        let head_path = history.join(head_ts.to_string());
        let head_hash = tree(&head_path);
        let latest_hash = tree(&latest);
        if let Some(cut) = cut {
            let delayed = Delayed {
                state,
                dir: &early_dir,
                early_ts,
                early_id: &early_id,
                head: &newest,
                seeds: &seeds,
                expired,
            };
            delayed.interrupt(cut, &mut early, |previous| {
                detached(&early_scope);
                let mut p = spawn(head_cap);
                let id = wait_for_run(&mut p, &early_dir, Some(previous));
                assert!(window(&mut p, &early_dir, &id, 0).facts.is_empty());
                assert!(
                    wait(
                        &mut crate::worker(&early_scope, &peers, 1, 0, &early_dir),
                        10
                    )
                    .success()
                );
                window(&mut p, &early_dir, &id, 4);
                (p, id)
            });
            detached(&early_scope);
            detached(head_scope);
            return;
        }
        signal(&early, libc::SIGCONT);
        assert_eq!(wait(&mut early, 10).code(), Some(0));
        let older = snapshot(&early_dir);
        assert_eq!(older.run_id, early_id);
        assert!(
            older.state == CaptureState::Stopped
                && older.final_accounted
                && older.reconciles()
                && older.retained == 8
        );
        assert_eq!(export(&early_dir).capture.as_ref(), Some(&older));
        detached(&early_scope);
        detached(head_scope);
        let final_index = json_file(&history.join("index.json"));
        let final_ids: Vec<_> = final_index["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| {
                export(&state.join(r["path"].as_str().unwrap()))
                    .capture
                    .unwrap()
                    .run_id
            })
            .collect();
        let published = export(&latest).capture.unwrap().run_id;
        let summary = json!({"early_run":early_id,"early_ms":early_ts,"early_limit":early_cap,
            "head_run":head_id,"head_ms":head_ts,"head_limit":head_cap,"latest_after":published,
            "head_tree_before":head_hash,"head_tree_after":tree(&head_path),
            "latest_tree_before":latest_hash,"latest_tree_after":tree(&latest),
            "seed_ids":seeds.iter().map(|s|&s.run_id).collect::<Vec<_>>(),
            "index_runs":final_ids,"index_limit":final_index["history_retention"],
            "physical_archives":numeric_dirs(&history).len()});
        fs::write(
            root.join("summary.json"),
            serde_json::to_vec_pretty(&summary).unwrap(),
        )
        .unwrap();
        assert_eq!(
            published, head_id,
            "latest regressed after the delayed commit"
        );
        assert_eq!(final_index["latest_updated_unix_ms"], head_ts);
        assert_eq!(final_index["history_retention"], head_cap);
        assert_eq!(tree(&latest), latest_hash);
        assert_eq!(tree(&head_path), head_hash);
        let mut wanted = vec![&newest, &older];
        wanted.extend(seeds.iter().rev());
        wanted.truncate(head_cap);
        api_archive::check(state, &wanted);
        if head_cap == 1 {
            assert_eq!(fs::read(history.join("index.json")).unwrap(), before_index);
        } else {
            assert_eq!(final_index["entries"][1]["updated_unix_ms"], early_ts);
            let delta = &final_index["latest_protocol_catalog_delta"];
            assert_eq!(delta["current_updated_unix_ms"], head_ts);
            assert_eq!(delta["previous_updated_unix_ms"], early_ts);
            assert_eq!(
                json_file(
                    &state.join(
                        final_index["latest_protocol_catalog_delta_path"]
                            .as_str()
                            .unwrap()
                    )
                ),
                *delta
            );
        }
        for name in [
            ".gewyvern-prune-v1.json",
            ".gewyvern-inflight-v1.json",
            ".gewyvern-pending-v1",
        ] {
            assert!(!history.join(name).try_exists().unwrap());
        }
    });
}
