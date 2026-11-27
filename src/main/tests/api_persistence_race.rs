use super::{ApiSnapshot, Arc, Mutex, fs, persist_api_snapshot, temp_dir};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::{Child, Command};
use std::time::{Duration, Instant};

const TEST: &str =
    "tests::api_persistence::race::same_millisecond_writers_leave_only_the_winner_visible";
const WORKER: &str = "GEWY_TEST_HISTORY_RACER";

struct Shelf(PathBuf);

impl Drop for Shelf {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn wait_for(path: &Path) {
    let deadline = Instant::now() + Duration::from_secs(15);
    while !path.try_exists().unwrap() {
        assert!(Instant::now() < deadline, "missing {}", path.display());
        std::thread::sleep(Duration::from_millis(5));
    }
}

fn worker(root: &Path, label: &str) {
    let ts = std::env::var("GEWY_TEST_HISTORY_MS")
        .unwrap()
        .parse()
        .unwrap();
    fs::write(root.join(format!("ready-{label}")), []).unwrap();
    if matches!(label, "left" | "right") {
        wait_for(&root.join("race"));
    }
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot {
        updated_unix_ms: ts,
        kind: "single".into(),
        name: Some(label.into()),
        summary_text: Some(label.into()),
        export_json: Some(serde_json::json!({"writer":label}).to_string()),
        ..Default::default()
    })));
    let receipt = persist_api_snapshot(&state)
        .err()
        .unwrap_or_else(|| "ok".into());
    fs::write(root.join(format!("receipt-{label}")), receipt).unwrap();
}

fn start(root: &Path, label: &str, ts: u128, retention: usize) -> Child {
    Command::new(std::env::current_exe().unwrap())
        .args(["--exact", TEST])
        .current_dir(root)
        .env(WORKER, label)
        .env("GEWY_TEST_HISTORY_MS", ts.to_string())
        .env("GEWY_STATE_HOME", root.join("state"))
        .env("GEWY_HISTORY_RETENTION", retention.to_string())
        .stdout(std::process::Stdio::null())
        .spawn()
        .unwrap()
}

fn index(history: &Path, retention: usize, times: &[u64]) {
    let index: Value =
        serde_json::from_slice(&fs::read(history.join("index.json")).unwrap()).unwrap();
    assert_eq!(index["history_retention"], retention);
    let indexed: Vec<_> = index["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["updated_unix_ms"].as_u64().unwrap())
        .collect();
    assert_eq!(indexed, times);
    let mut disk: Vec<_> = fs::read_dir(history)
        .unwrap()
        .map(Result::unwrap)
        .filter(|e| e.file_type().unwrap().is_dir())
        .map(|e| e.file_name().to_str().unwrap().parse::<u64>().unwrap())
        .collect();
    disk.sort_unstable_by(|a, b| b.cmp(a));
    assert_eq!(disk, indexed);
}

#[test]
fn same_millisecond_writers_leave_only_the_winner_visible() {
    if let Ok(label) = std::env::var(WORKER) {
        return worker(&std::env::current_dir().unwrap(), &label);
    }
    let root = temp_dir("writer-race");
    fs::create_dir(&root).unwrap();
    let _cleanup = Shelf(root.clone());
    let receipt = |label| fs::read_to_string(root.join(format!("receipt-{label}"))).unwrap();
    assert!(start(&root, "seed", 100, 4).wait().unwrap().success());
    assert_eq!(receipt("seed"), "ok");

    let mut left = start(&root, "left", 101, 1);
    let mut right = start(&root, "right", 101, 4);
    for label in ["left", "right"] {
        wait_for(&root.join(format!("ready-{label}")));
    }
    fs::write(root.join("race"), []).unwrap();
    assert!(left.wait().unwrap().success());
    assert!(right.wait().unwrap().success());
    let (winner, loser, cap) = if receipt("left") == "ok" {
        ("left", "right", 1)
    } else {
        ("right", "left", 4)
    };
    assert_eq!(receipt(winner), "ok");
    let initial = receipt(loser);
    if initial.ends_with(": timed out waiting for the file lock") {
        // A slow filesystem can spend the entire lock budget publishing the
        // winner. Retry the very same proposal after contention has ended.
        assert!(
            start(&root, loser, 101, if loser == "left" { 1 } else { 4 })
                .wait()
                .unwrap()
                .success()
        );
    }
    println!("winner={winner}, retention={cap}, contender={initial}");
    assert_eq!(receipt(loser), "history timestamp already exists: 101");
    let history = root.join("state/history/api/v1");
    let latest = root.join("state/latest/api/v1/latest");
    let archive = history.join("101");
    index(&history, cap, if cap == 1 { &[101] } else { &[101, 100] });
    assert_eq!(
        fs::read_to_string(archive.join("summary.txt")).unwrap(),
        winner
    );
    for name in [
        "summary.txt",
        "export.json",
        "meta.json",
        "protocol-delta.json",
    ] {
        assert_eq!(
            fs::read(latest.join(name)).unwrap(),
            fs::read(archive.join(name)).unwrap(),
            "{name}"
        );
    }

    assert!(start(&root, "fresh", 102, 2).wait().unwrap().success());
    assert_eq!(receipt("fresh"), "ok");
    index(&history, 2, &[102, 101]);
    assert_eq!(
        fs::read_to_string(latest.join("summary.txt")).unwrap(),
        "fresh"
    );
    assert_eq!(
        fs::read_to_string(archive.join("summary.txt")).unwrap(),
        winner
    );
}
