use super::*;
use std::mem::{size_of, size_of_val};

pub(super) fn at_writer_open(active: &Process, file: &Path, mut observe: impl FnMut() -> bool) {
    on_open(active, file, 0, libc::SIGTERM, |_| observe());
}

// Dropping the group releases the held open; no sleeps or product failpoints.
pub(super) fn on_open(
    active: &Process,
    file: &Path,
    modifiers: u64,
    start_signal: i32,
    mut observe: impl FnMut(&fs::File) -> bool,
) {
    let path = CString::new(file.as_os_str().as_bytes()).unwrap();
    let raw = unsafe {
        libc::fanotify_init(
            libc::FAN_CLASS_CONTENT | libc::FAN_CLOEXEC,
            libc::O_RDONLY as _,
        )
    };
    assert!(
        raw >= 0,
        "fanotify_init: {}",
        std::io::Error::last_os_error()
    );
    let mut gate = unsafe { fs::File::from_raw_fd(raw) };
    let rc = unsafe {
        libc::fanotify_mark(
            raw,
            libc::FAN_MARK_ADD,
            libc::FAN_OPEN_PERM | modifiers,
            libc::AT_FDCWD,
            path.as_ptr(),
        )
    };
    assert_eq!(rc, 0, "fanotify_mark: {}", std::io::Error::last_os_error());
    signal(active, start_signal);
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        let mut poll = libc::pollfd {
            fd: raw,
            events: libc::POLLIN,
            revents: 0,
        };
        let ready = unsafe { libc::poll(&mut poll, 1, 100) };
        assert!(ready >= 0, "poll: {}", std::io::Error::last_os_error());
        if ready == 0 {
            continue;
        }
        // No variable-length information records were requested.
        let mut bytes = [0; size_of::<libc::fanotify_event_metadata>()];
        gate.read_exact(&mut bytes).unwrap();
        let event = unsafe {
            bytes
                .as_ptr()
                .cast::<libc::fanotify_event_metadata>()
                .read_unaligned()
        };
        assert_eq!(event.event_len as usize, bytes.len());
        assert_ne!(event.mask & libc::FAN_OPEN_PERM, 0);
        assert!(event.fd >= 0);
        let file = unsafe { fs::File::from_raw_fd(event.fd) };
        if event.pid == active.0.id() as i32 && observe(&file) {
            return;
        }
        let reply = libc::fanotify_response {
            fd: event.fd,
            response: libc::FAN_ALLOW,
        };
        let n = unsafe { libc::write(raw, (&raw const reply).cast(), size_of_val(&reply)) };
        assert_eq!(n, size_of_val(&reply) as isize);
    }
    panic!(
        "writer never reached the expected open of {}",
        file.display()
    );
}

fn paths(state: &Path, cap: usize) -> Vec<PathBuf> {
    let index: Value = serde_json::from_str(&history_view(state, true)).unwrap();
    assert_eq!(index["history_retention"], cap);
    index["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| state.join(r["path"].as_str().unwrap()))
        .collect()
}

#[test]
#[ignore = "Linux root/BPF/fanotify; GEWY_SHRINK_RAISE_OUT=new-directory"]
fn production_capture_raise_after_partial_batch() {
    reconfigure("GEWY_SHRINK_RAISE_OUT", 4);
}

#[test]
#[ignore = "Linux root/BPF/fanotify; GEWY_SHRINK_LOWER_OUT=new-directory"]
fn production_capture_lower_after_partial_batch() {
    reconfigure("GEWY_SHRINK_LOWER_OUT", 1);
}

fn reconfigure(out: &str, restart: usize) {
    with_shelf(out, |dir, state, scope, start| {
        let history = state.join("history/api/v1");
        let index = history.join("index.json");
        let journal = history.join(".gewyvern-prune-v1.json");
        let fingerprint = |p: &Path| api_archive::fingerprint_paths(p, &["."]);
        let seeds: Vec<_> = (0..5)
            .map(|_| clean_stop(&mut start(5).0, dir, 4, 0))
            .collect();
        api_archive::check(state, &seeds.iter().rev().collect::<Vec<_>>());
        let before = paths(state, 5);
        let hashes: Vec<_> = before.iter().map(|p| fingerprint(p)).collect();
        let (mut p, shrink_id) = start(2);
        let stamp = ".gewyvern-publication-v1.json";
        at_writer_open(&p, &before[2].join(stamp), || {
            if before[1].try_exists().unwrap() {
                return false; // Identifying victims before the index switch.
            }
            signal(&p, libc::SIGKILL);
            true
        });
        assert_eq!(wait(&mut p, 10).signal(), Some(libc::SIGKILL));
        detached(scope);
        let committed = paths(state, 2);
        assert_eq!(committed.len(), 2);
        assert_eq!(committed[1], before[0]);
        let stopped = snapshot(dir);
        assert_eq!(stopped.run_id, shrink_id);
        assert!(stopped.final_accounted && stopped.reconciles() && stopped.retained == 4);
        assert_eq!(stopped.state, CaptureState::Stopped);
        assert_eq!(export(&committed[0]).capture.as_ref(), Some(&stopped));
        let committed_hash = fingerprint(&committed[0]);
        let index_bytes = fs::read(&index).unwrap();
        let plan = fs::read(&journal).unwrap();
        let retired: Value = serde_json::from_slice(&plan).unwrap();
        let ts: Vec<_> = retired["retired"]
            .as_array()
            .unwrap()
            .iter()
            .map(|r| history.join(r["timestamp"].to_string()))
            .collect();
        assert_eq!(ts, before[1..]);
        let check_gap = || {
            assert_eq!(fs::read(&index).unwrap(), index_bytes);
            assert_eq!(fs::read(&journal).unwrap(), plan);
            let disk = numeric_dirs(&history);
            assert_eq!(
                disk,
                committed.iter().chain(&before[2..]).cloned().collect()
            );
            assert_eq!(fingerprint(&committed[0]), committed_hash);
            for (p, expected) in before.iter().zip(&hashes).filter(|(p, _)| *p != &before[1]) {
                assert_eq!(&fingerprint(p), expected);
            }
        };
        check_gap();
        let (mut p, repair_id) = start(restart);
        check_gap();
        let text = history_view_at(state, false, Some(restart));
        let reported = text
            .lines()
            .find_map(|l| l.strip_prefix("retention: "))
            .unwrap();
        // Both opens occur after old-plan recovery and before the next index
        // switch: catalog delta for a survivor, or identity for a new eviction.
        let watch = if restart == 1 {
            committed[0].join(stamp)
        } else {
            fs::read_dir(committed[0].join("protocols"))
                .unwrap()
                .next()
                .unwrap()
                .unwrap()
                .path()
                .join("summary.json")
        };
        let mut replay_ids = Vec::new();
        at_writer_open(&p, &watch, || {
            assert!(!journal.try_exists().unwrap());
            assert_eq!(fs::read(&index).unwrap(), index_bytes);
            assert!(before[1..].iter().all(|p| !p.try_exists().unwrap()));
            for (p, expected) in committed.iter().zip([&stopped, seeds.last().unwrap()]) {
                let actual = export(p).capture.unwrap();
                assert_eq!(&actual, expected);
                replay_ids.push(actual.run_id);
            }
            true
        });
        assert_eq!(wait(&mut p, 10).code(), Some(0));
        let repaired = snapshot(dir);
        assert_eq!(repaired.state, CaptureState::Stopped);
        assert!(repaired.final_accounted && repaired.reconciles() && repaired.retained == 4);
        assert_eq!(export(dir).capture.as_ref(), Some(&repaired));
        assert_eq!(repaired.run_id, repair_id);
        let mut expected = vec![&repaired, &stopped, seeds.last().unwrap()];
        expected.truncate(restart);
        api_archive::check(state, &expected);
        let final_paths = paths(state, restart);
        assert!(before[1..].iter().all(|p| !p.try_exists().unwrap()));
        assert!(!journal.try_exists().unwrap());
        assert!(
            !history
                .join(".gewyvern-inflight-v1.json")
                .try_exists()
                .unwrap()
        );
        if restart > 2 {
            assert_eq!(final_paths[1..], committed);
            assert_eq!(fingerprint(&final_paths[1]), committed_hash);
            assert_eq!(fingerprint(&final_paths[2]), hashes[0]);
        }
        detached(scope);
        let summary = json!({"seed_cap":5,"shrink_cap":2,"restart_cap":restart,
            "seed_ids_newest_first":seeds.iter().rev().map(|s| &s.run_id).collect::<Vec<_>>(),
            "shrink_id":shrink_id,"repair_id":repair_id,"gap_index_sha256":hash(&index_bytes),
            "gap_old_victims_remaining":before[2..].len(),"gap_victim_sha256":&hashes[2..],
            "text_retention_during_restart":reported,"committed_tree_sha256":committed_hash,
            "post_replay_pre_commit_ids":replay_ids,
            "final_ids":expected.iter().map(|s| &s.run_id).collect::<Vec<_>>(),
            "final_directories":final_paths.len(),"pending_prune":journal.exists()});
        fs::write(
            dir.parent().unwrap().join("summary.json"),
            serde_json::to_vec_pretty(&summary).unwrap(),
        )
        .unwrap();
        assert_eq!(
            reported, "2",
            "human history view must report the committed retention"
        );
    });
}
