use super::*;

fn file_count(dir: &Path) -> usize {
    fs::read_dir(dir)
        .unwrap()
        .map(Result::unwrap)
        .map(|e| {
            if e.file_type().unwrap().is_dir() {
                file_count(&e.path())
            } else {
                1
            }
        })
        .sum()
}

fn rows(state: &Path) -> Vec<PathBuf> {
    let index: Value = serde_json::from_str(&history_view(state, true)).unwrap();
    assert_eq!(index["history_retention"], 2);
    let rows = index["entries"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    rows.iter()
        .map(|r| state.join(r["path"].as_str().unwrap()))
        .collect()
}

pub(super) fn exercise(
    state: &Path,
    dir: &Path,
    scope: &Scope,
    mut start: impl FnMut() -> (Process, String),
    oldest: CaptureSnapshot,
    newer: CaptureSnapshot,
    retries: usize,
) {
    let history = state.join("history/api/v1");
    let mut expected = newer.clone();
    let fingerprint = |p: &Path| api_archive::fingerprint_paths(p, &["."]);
    let mut kept_hash = fingerprint(&rows(state)[0]);
    let mut rounds = Vec::new();
    for _ in 0..3 {
        let before = rows(state);
        let victim = &before[1];
        let before_files = file_count(victim);
        let (mut p, id) = start();
        let deleted = kill_on_event(&mut p, victim, libc::IN_DELETE, |_, name| !name.is_empty());
        assert!(!deleted.exists());
        detached(scope);
        let remaining = file_count(victim);
        assert!(
            (1..before_files).contains(&remaining),
            "missed partial prune: {remaining}/{before_files}"
        );
        let after = rows(state);
        assert_eq!(after[1], before[0]);
        assert_eq!(fingerprint(&after[1]), kept_hash);
        assert_eq!(export(&after[1]).capture, Some(expected));
        let terminal = snapshot(dir);
        assert_eq!(terminal.run_id, id);
        assert!(
            terminal.state == CaptureState::Stopped
                && terminal.final_accounted
                && terminal.reconciles()
                && terminal.retained == 4
        );
        assert_eq!(export(&after[0]).capture.as_ref(), Some(&terminal));
        assert_eq!(
            export(&state.join("latest/api/v1/latest")).capture.as_ref(),
            Some(&terminal)
        );
        assert!(after[0].join("protocol-evolution.md").is_file());
        assert!(
            history_view(state, false)
                .lines()
                .any(|l| l == "entries: 2")
        );
        kept_hash = fingerprint(&after[0]);
        expected = terminal;
        rounds.push(
            json!({"committed":id,"survivor_sha256":fingerprint(&after[1]),
            "retired_files_before":before_files,"retired_files_remaining":remaining,
            "retired_dirs_remaining":numeric_dirs(&history).len()-2}),
        );
    }
    let mut resume = Vec::new();
    if retries > 0 {
        let live: BTreeSet<_> = rows(state).into_iter().collect();
        let debt: Vec<_> = numeric_dirs(&history).difference(&live).cloned().collect();
        assert_eq!(debt.len(), 1);
        let index = fs::read(history.join("index.json")).unwrap();
        let committed_trees = || live.iter().map(|p| fingerprint(p)).collect::<Vec<_>>();
        let hashes = committed_trees();
        for _ in 0..retries {
            let before = file_count(&debt[0]);
            let (mut p, id) = start();
            kill_on_event(&mut p, &debt[0], libc::IN_DELETE, |_, name| {
                !name.is_empty()
            });
            let after = file_count(&debt[0]);
            assert!((1..before).contains(&after));
            assert_eq!(fs::read(history.join("index.json")).unwrap(), index);
            assert_eq!(committed_trees(), hashes);
            assert_eq!(numeric_dirs(&history).len(), 3);
            detached(scope);
            resume.push(json!({"run":id,"files_before":before,"files_remaining":after}));
        }
    }
    let repaired = clean_stop(&mut start().0, dir, 4, 0);
    let final_rows = rows(state);
    assert_eq!(fingerprint(&final_rows[1]), kept_hash);
    assert_eq!(export(&final_rows[0]).capture.as_ref(), Some(&repaired));
    assert_eq!(export(&final_rows[1]).capture.as_ref(), Some(&expected));
    detached(scope);
    let mut summary = json!({
        "retention":2,"oldest":oldest.run_id,"newer":newer.run_id,"rounds":rounds,
        "final_index":[repaired.run_id,expected.run_id],"final_numeric_dirs":numeric_dirs(&history).len()
    });
    if retries > 0 {
        summary["resume"] = json!(resume);
    }
    fs::write(
        dir.parent().unwrap().join("summary.json"),
        serde_json::to_vec_pretty(&summary).unwrap(),
    )
    .unwrap();
    assert!(
        rounds.iter().all(|r| r["retired_dirs_remaining"] == 1),
        "{rounds:?}"
    );
    api_archive::check(state, &[&repaired, &expected]);
}
