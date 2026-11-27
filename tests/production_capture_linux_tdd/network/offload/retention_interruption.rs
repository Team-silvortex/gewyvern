use super::*;
use crate::boundaries::collector_command;
use std::ffi::CString;
use std::os::fd::{AsRawFd, FromRawFd};
use std::os::unix::ffi::OsStrExt;

#[path = "prune_interruption.rs"]
mod prune_interruption;
#[path = "shrink_interruption.rs"]
mod shrink_interruption;
#[path = "terminal_order.rs"]
mod terminal_order;

fn kill_on_event(
    active: &mut Process,
    directory: &Path,
    mask: u32,
    matches: fn(u32, &[u8]) -> bool,
) -> PathBuf {
    let fd = unsafe { libc::inotify_init1(libc::IN_CLOEXEC | libc::IN_NONBLOCK) };
    assert!(fd >= 0);
    let mut events = unsafe { fs::File::from_raw_fd(fd) };
    let path = CString::new(directory.as_os_str().as_bytes()).unwrap();
    assert!(unsafe { libc::inotify_add_watch(fd, path.as_ptr(), mask) } >= 0);
    let mut poll = libc::pollfd {
        fd: events.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };
    signal(active, libc::SIGTERM);
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if unsafe { libc::poll(&mut poll, 1, 100) } == 0 {
            continue;
        }
        let mut buf = [0u8; 4096];
        let len = events.read(&mut buf).unwrap();
        let mut record = &buf[..len];
        while !record.is_empty() {
            let size = std::mem::size_of::<libc::inotify_event>();
            assert!(record.len() >= size);
            let event = unsafe {
                record
                    .as_ptr()
                    .cast::<libc::inotify_event>()
                    .read_unaligned()
            };
            let end = size + event.len as usize;
            let name = record[size..end].split(|&b| b == 0).next().unwrap();
            if event.mask & mask != 0 && matches(event.mask, name) {
                signal(active, libc::SIGKILL);
                assert_eq!(wait(active, 10).signal(), Some(libc::SIGKILL));
                return directory.join(std::str::from_utf8(name).unwrap());
            }
            record = &record[end..];
        }
    }
    panic!("never observed expected archive event");
}

fn numeric_dirs(history: &Path) -> BTreeSet<PathBuf> {
    fs::read_dir(history)
        .unwrap()
        .map(Result::unwrap)
        .filter(|e| {
            e.file_type().unwrap().is_dir()
                && e.file_name()
                    .to_str()
                    .is_some_and(|s| s.parse::<u128>().is_ok())
        })
        .map(|e| e.path())
        .collect()
}

#[test]
#[ignore = "Linux root/BPF/inotify; GEWY_RETENTION_INTERRUPT_OUT=new-directory"]
fn production_capture_repeated_rename_at_retention_limit() {
    exercise_full_shelf("GEWY_RETENTION_INTERRUPT_OUT", None);
}

#[test]
#[ignore = "Linux root/BPF/inotify; GEWY_PRUNE_INTERRUPT_OUT=new-directory"]
fn production_capture_repeated_committed_prune() {
    exercise_full_shelf("GEWY_PRUNE_INTERRUPT_OUT", Some(0));
}

#[test]
#[ignore = "Linux root/BPF/inotify; GEWY_PRUNE_RETRY_OUT=new-directory"]
fn production_capture_prune_recovery_is_interruptible() {
    exercise_full_shelf("GEWY_PRUNE_RETRY_OUT", Some(2));
}

fn exercise_full_shelf(out: &str, prune_retries: Option<usize>) {
    with_shelf(out, |dir, state, scope, launch| {
        exercise_fixed(dir, state, scope, || launch(2), prune_retries)
    });
}

fn with_shelf(
    out: &str,
    test: impl FnOnce(&Path, &Path, &Scope, &mut dyn FnMut(usize) -> (Process, String)),
) {
    let root = PathBuf::from(std::env::var_os(out).unwrap());
    fs::create_dir(&root).unwrap();
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-retention-{}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    let runtime = root.join("runtime");
    let dir = root.join("capture");
    for p in [&runtime, &dir] {
        fs::create_dir(p).unwrap();
    }
    let state = runtime.join("state");
    let peers = crate::peers();
    let spawn = |retention: usize| {
        Process(
            collector_command(
                &scope,
                peers.port,
                &runtime,
                &dir,
                &[
                    "--serve",
                    "--capture-window-ms",
                    "200",
                    "--api-socket",
                    "127.0.0.1:0",
                ],
            )
            .env("GEWY_HISTORY_RETENTION", retention.to_string())
            .spawn()
            .unwrap(),
        )
    };
    let mut ids = BTreeSet::new();
    let mut previous = None;
    let mut start = |retention| {
        let mut p = spawn(retention);
        let id = wait_for_run(&mut p, &dir, previous.as_deref());
        assert!(ids.insert(id.clone()));
        assert!(window(&mut p, &dir, &id, 0).facts.is_empty());
        previous = Some(id.clone());
        assert!(wait(&mut crate::worker(&scope, &peers, 1, 0, &root), 10).success());
        window(&mut p, &dir, &id, 4);
        (p, id)
    };
    test(&dir, &state, &scope, &mut start);
}

fn exercise_fixed(
    dir: &Path,
    state: &Path,
    scope: &Scope,
    mut start: impl FnMut() -> (Process, String),
    prune_retries: Option<usize>,
) {
    let root = dir.parent().unwrap();
    let history = state.join("history/api/v1");
    let oldest = clean_stop(&mut start().0, dir, 4, 0);
    let newer = clean_stop(&mut start().0, dir, 4, 0);
    api_archive::check(state, &[&newer, &oldest]);
    if let Some(retries) = prune_retries {
        return prune_interruption::exercise(state, dir, scope, start, oldest, newer, retries);
    }
    let index = fs::read(history.join("index.json")).unwrap();
    let indexed: Value = serde_json::from_slice(&index).unwrap();
    assert_eq!(indexed["history_retention"], 2);
    let old_dirs = numeric_dirs(&history);
    let old_trees = || {
        old_dirs
            .iter()
            .map(|p| api_archive::fingerprint_paths(p, &["."]))
            .collect::<Vec<_>>()
    };
    let old_hashes = old_trees();
    let mut rounds = Vec::new();
    for _ in 0..3 {
        let (mut p, id) = start();
        let dest = kill_on_event(&mut p, &history, libc::IN_MOVED_TO, |mask, name| {
            mask & libc::IN_ISDIR != 0 && !name.is_empty() && name.iter().all(u8::is_ascii_digit)
        });
        detached(scope);
        let s = export(&dest).capture.unwrap();
        assert_eq!(s.run_id, id);
        assert!(
            s.state == CaptureState::Stopped
                && s.final_accounted
                && s.reconciles()
                && s.retained == 4
        );
        assert!(dest.join("protocol-evolution.md").is_file());
        assert_eq!(
            fs::read(history.join("index.json")).unwrap(),
            index,
            "missed the pre-index window"
        );
        assert_eq!(old_trees(), old_hashes);
        assert_eq!(
            serde_json::from_str::<Value>(&history_view(state, true)).unwrap(),
            indexed
        );
        assert!(
            history_view(state, false)
                .lines()
                .any(|l| l == "entries: 2")
        );
        let unindexed = numeric_dirs(&history).difference(&old_dirs).count();
        rounds.push(json!({"interrupted_run":id,"unindexed":unindexed}));
    }
    let repaired = clean_stop(&mut start().0, dir, 4, 0);
    let actual = numeric_dirs(&history).len();
    let committed: Value =
        serde_json::from_slice(&fs::read(history.join("index.json")).unwrap()).unwrap();
    let commits: Vec<_> = committed["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            export(&state.join(row["path"].as_str().unwrap()))
                .capture
                .unwrap()
                .run_id
        })
        .collect();
    assert_eq!(commits, [repaired.run_id.clone(), newer.run_id.clone()]);
    detached(scope);
    fs::write(root.join("summary.json"), serde_json::to_vec_pretty(&json!({
        "retention":2,"oldest":oldest.run_id,"newer":newer.run_id,"index_sha256":hash(&index),
        "old_tree_sha256":old_hashes,"rounds":rounds,"final_index":commits,"final_numeric_dirs":actual
    })).unwrap()).unwrap();
    assert!(rounds.iter().all(|r| r["unindexed"] == 1), "{rounds:?}");
    api_archive::check(state, &[&repaired, &newer]);
}
