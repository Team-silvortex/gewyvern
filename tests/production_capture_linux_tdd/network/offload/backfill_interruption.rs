use super::*;
#[path = "backfill_replay.rs"]
mod backfill_replay;

#[derive(Clone, Copy, PartialEq)]
pub(super) enum Cut {
    Before,
    After,
}

pub(super) enum Fault {
    Index(Cut),
    Replay(Cut),
}

pub(super) struct Delayed<'a> {
    pub state: &'a Path,
    pub dir: &'a Path,
    pub early_ts: u64,
    pub early_id: &'a str,
    pub head: &'a CaptureSnapshot,
    pub seeds: &'a [CaptureSnapshot],
}

// Verify the published pointers by opening their actual payloads, not by
// reconstructing a delta from a directory scan (which can include an orphan).
fn view(state: &Path, wanted: &[&CaptureSnapshot], sidecar: Option<u64>) -> Value {
    let bytes = fs::read(state.join("history/api/v1/index.json")).unwrap();
    let index: Value = serde_json::from_slice(&bytes).unwrap();
    let entries = index["entries"].as_array().unwrap();
    assert_eq!(index["history_retention"], 4);
    assert_eq!(entries.len(), wanted.len());
    let times: Vec<_> = entries
        .iter()
        .zip(wanted)
        .map(|(entry, expected)| {
            let ts = entry["updated_unix_ms"].as_u64().unwrap();
            assert_eq!(entry["path"], format!("history/api/v1/{ts}"));
            assert_eq!(
                export(&state.join(entry["path"].as_str().unwrap()))
                    .capture
                    .as_ref(),
                Some(*expected)
            );
            ts
        })
        .collect();
    assert!(times.windows(2).all(|w| w[0] > w[1]));
    let path = match sidecar {
        Some(ts) => format!("history/api/v1/{ts}/index-head-delta.json"),
        None => format!("history/api/v1/{}/protocol-delta.json", times[0]),
    };
    assert_eq!(index["latest_protocol_catalog_delta_path"], path);
    let delta_bytes = fs::read(state.join(&path)).unwrap();
    let delta: Value = serde_json::from_slice(&delta_bytes).unwrap();
    assert_eq!(index["latest_protocol_catalog_delta"], delta);
    assert_eq!(delta["current_updated_unix_ms"], times[0]);
    assert_eq!(delta["previous_updated_unix_ms"], times[1]);
    assert_eq!(index["latest_updated_unix_ms"], times[0]);
    let latest = state.join("latest/api/v1/latest");
    assert_eq!(
        json_file(&latest.join("meta.json"))["updated_unix_ms"],
        times[0]
    );
    assert_eq!(export(&latest).capture.as_ref(), Some(wanted[0]));
    json!({"runs":wanted.iter().map(|s| &s.run_id).collect::<Vec<_>>(),"times":times,
        "index_sha256":hash(&bytes),"delta_path":path,"delta_sha256":hash(&delta_bytes),
        "latest_tree":api_archive::fingerprint_paths(&latest, &["."])})
}

impl Delayed<'_> {
    pub(super) fn interrupt(
        &self,
        fault: Fault,
        p: &mut Process,
        mut restart: impl FnMut(&str) -> (Process, String),
    ) {
        let cut = match fault {
            Fault::Index(c) | Fault::Replay(c) => c,
        };
        let Self {
            state,
            dir,
            early_ts,
            early_id,
            head,
            seeds,
        } = *self;
        let h = state.join("history/api/v1");
        let index = h.join("index.json");
        let seed_view = [head, &seeds[2], &seeds[1], &seeds[0]];
        let before = view(state, &seed_view, None);
        let initial_bytes = fs::read(&index).unwrap();
        let head_dir = h.join(before["times"][0].to_string());
        let victim = h.join(before["times"][3].to_string());
        let candidate = h.join(early_ts.to_string());
        let sidecar = candidate.join("index-head-delta.json");
        let tree = |p: &Path| api_archive::fingerprint_paths(p, &["."]);
        let head_hash = tree(&head_dir);
        let old_dirs = numeric_dirs(&h);
        match cut {
            Cut::Before => {
                // The atomic sidecar writer opens its parent for fsync after rename.
                // Only stat the marked tree here: opening a child would gate ourselves.
                shrink_interruption::on_open(
                    p,
                    &h,
                    libc::FAN_EVENT_ON_CHILD | libc::FAN_ONDIR,
                    libc::SIGCONT,
                    |fd| {
                        if fs::read_link(format!("/proc/self/fd/{}", fd.as_raw_fd())).unwrap()
                            == candidate
                            && sidecar.is_file()
                        {
                            signal(p, libc::SIGKILL);
                            true
                        } else {
                            false
                        }
                    },
                );
            }
            Cut::After => {
                shrink_interruption::on_open(
                    p,
                    &victim.join(".gewyvern-publication-v1.json"),
                    0,
                    libc::SIGCONT,
                    |_| {
                        if fs::read(&index).unwrap() == initial_bytes {
                            return false;
                        }
                        signal(p, libc::SIGKILL);
                        true
                    },
                );
            }
        }
        assert_eq!(wait(p, 10).signal(), Some(libc::SIGKILL));
        let older = snapshot(dir);
        assert_eq!(older.run_id, early_id);
        assert!(
            older.state == CaptureState::Stopped
                && older.final_accounted
                && older.reconciles()
                && older.retained == 8
        );
        assert_eq!(export(&candidate).capture.as_ref(), Some(&older));
        let candidate_hash = tree(&candidate);
        let staged_delta = fs::read(&sidecar).unwrap();
        let staged: Value = serde_json::from_slice(&staged_delta).unwrap();
        assert_eq!(staged["current_updated_unix_ms"], before["times"][0]);
        assert_eq!(staged["previous_updated_unix_ms"], early_ts);
        let post = cut == Cut::After;
        let new_view = [head, &older, &seeds[2], &seeds[1]];
        let check_gap = || {
            let v = view(
                state,
                if post { &new_view } else { &seed_view },
                post.then_some(early_ts),
            );
            assert_eq!(v["latest_tree"], before["latest_tree"]);
            assert_eq!(tree(&head_dir), head_hash);
            if !post {
                assert_eq!(fs::read(&index).unwrap(), initial_bytes);
            }
            assert_eq!(tree(&candidate), candidate_hash);
            let mut expected = old_dirs.clone();
            expected.insert(candidate.clone());
            assert_eq!(numeric_dirs(&h), expected);
            for f in [".gewyvern-inflight-v1.json", ".gewyvern-prune-v1.json"] {
                assert!(h.join(f).is_file());
            }
            v
        };
        let gap = check_gap();
        let (mut fresh, mut fresh_id) = restart(early_id);
        assert_ne!(fresh_id, early_id);
        assert!(seed_view.iter().all(|s| s.run_id != fresh_id));
        let running = check_gap();
        assert_eq!(running, gap);
        let replay = if let Fault::Replay(_) = fault {
            let committed = if post { &new_view } else { &seed_view };
            let shelf = backfill_replay::Shelf {
                state,
                dir,
                early_ts,
                committed,
                gap: &gap,
                victim: &victim,
                candidate_hash: &candidate_hash,
            };
            let (replacement, id, proof) =
                shelf.kill_and_restart(cut, &mut fresh, &fresh_id, &mut restart);
            assert!(id != early_id && seed_view.iter().all(|s| s.run_id != id));
            fresh = replacement;
            fresh_id = id;
            Some(proof)
        } else {
            None
        };
        let repaired = clean_stop(&mut fresh, dir, 4, 0);
        assert_eq!(repaired.run_id, fresh_id);
        let final_rows = if post {
            [&repaired, head, &older, &seeds[2]]
        } else {
            [&repaired, head, &seeds[2], &seeds[1]]
        };
        let recovered = view(state, &final_rows, None);
        api_archive::check(state, &final_rows);
        assert_eq!(tree(&head_dir), head_hash);
        assert!(!victim.try_exists().unwrap());
        if post {
            assert_eq!(tree(&candidate), candidate_hash);
            assert_eq!(fs::read(&sidecar).unwrap(), staged_delta);
        } else {
            assert!(!candidate.try_exists().unwrap());
        }
        let residue: Vec<_> = fs::read_dir(&h)
            .unwrap()
            .map(Result::unwrap)
            .filter(|e| {
                let n = e.file_name();
                let n = n.to_str().unwrap();
                n != "index.json" && n != ".gewyvern-writer-v1.lock" && n.parse::<u64>().is_err()
            })
            .map(|e| e.file_name())
            .collect();
        assert!(residue.is_empty(), "{residue:?}");
        let mut report = json!({"cut":if post {"after"} else {"before"}, "late_run":early_id,"late_ms":early_ts,
            "before":before, "crash":gap, "running":running, "recovered":recovered,
            "head_archive_sha256":head_hash, "late_tree_sha256":candidate_hash,"late_delta_sha256":hash(&staged_delta),
            "crash_dirs":old_dirs.len()+1,"final_dirs":numeric_dirs(&h).len(),"residue":residue});
        if let Some(proof) = replay {
            report["interrupted_replay"] = proof;
        }
        fs::write(
            dir.parent().unwrap().join("summary.json"),
            serde_json::to_vec_pretty(&report).unwrap(),
        )
        .unwrap();
    }
}
