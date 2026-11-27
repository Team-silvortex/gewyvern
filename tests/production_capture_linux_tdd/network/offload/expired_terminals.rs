use super::*;
use std::os::unix::fs::MetadataExt;

pub(super) struct Parked {
    process: Process,
    scope: Scope,
    dir: PathBuf,
    id: String,
    ts: u64,
    cap: usize,
}

impl Parked {
    pub(super) fn new(state: &Path, root: &Path, parent: &Scope, cap: usize) -> Self {
        let scope = Scope(parent.0.with_file_name(format!(
            "gewyvern-production-test-expired-{}-{cap}",
            std::process::id()
        )));
        let dir = root.join(format!("expired-{cap}"));
        fs::create_dir(&scope.0).unwrap();
        fs::create_dir(&dir).unwrap();
        let peers = crate::peers();
        let addr = std::net::TcpListener::bind("127.0.0.1:0")
            .unwrap()
            .local_addr()
            .unwrap();
        let mut process = Process(
            collector_command(
                &scope,
                peers.port,
                &dir,
                &dir,
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
        );
        let id = wait_for_run(&mut process, &dir, None);
        assert!(wait(&mut crate::worker(&scope, &peers, 1, 0, &dir), 10).success());
        window(&mut process, &dir, &id, 4);
        let ts = hold_terminal(&process, &state.join("history/api/v1"), addr, &id);
        Self {
            process,
            scope,
            dir,
            id,
            ts,
            cap,
        }
    }
}

pub(super) fn drain(
    writers: Vec<Parked>,
    state: &Path,
    committed: &[&CaptureSnapshot],
    sidecar: Option<u64>,
    proof: &Value,
    root: &Path,
) {
    let debt = &proof["second_crash"];
    let baseline = &debt["view"];
    let h = state.join("history/api/v1");
    let times = baseline["times"].as_array().unwrap();
    let oldest = times.last().unwrap().as_u64().unwrap();
    let tree = |p: &Path| api_archive::fingerprint_paths(p, &["."]);
    // Equal bytes alone would miss a redundant atomic file replacement.
    let published_identity = || {
        let files = [
            h.join("index.json"),
            state.join(baseline["delta_path"].as_str().unwrap()),
            state.join("latest/api/v1/latest/meta.json"),
            state.join("latest/api/v1/latest/export.json"),
        ];
        hash(
            &serde_json::to_vec(&files.map(|p| {
                let m = fs::metadata(p).unwrap();
                (m.dev(), m.ino(), m.mtime(), m.mtime_nsec())
            }))
            .unwrap(),
        )
    };
    let identity = published_identity();
    let mut rounds = Vec::new();
    let mut ids: BTreeSet<_> = committed.iter().map(|s| s.run_id.clone()).collect();
    ids.insert(proof["killed_run"].as_str().unwrap().to_owned());
    for mut writer in writers {
        assert!(writer.ts < oldest && ids.insert(writer.id.clone()));
        signal(&writer.process, libc::SIGCONT);
        let exit = wait(&mut writer.process, 10).code();
        assert_eq!(exit, Some(0));
        detached(&writer.scope);
        let s = snapshot(&writer.dir);
        assert_eq!(s.run_id, writer.id);
        assert!(
            s.state == CaptureState::Stopped
                && s.final_accounted
                && s.reconciles()
                && s.retained == 4
        );
        assert_eq!(export(&writer.dir).capture.as_ref(), Some(&s));
        let v = backfill_interruption::view(state, committed, sidecar);
        assert_eq!(&v, baseline);
        let indexes = times
            .iter()
            .map(|ts| tree(&h.join(ts.to_string())))
            .collect::<Vec<_>>();
        assert_eq!(json!(indexes), debt["indexed_tree_sha256"]);
        let observed_identity = published_identity();
        assert_eq!(observed_identity, identity);
        let stage = h.join(".gewyvern-pending-v1");
        let staged = stage.try_exists().unwrap().then(|| tree(&stage));
        assert!(!h.join(writer.ts.to_string()).try_exists().unwrap());
        let names: BTreeSet<_> = times
            .iter()
            .map(|t| t.to_string())
            .chain(["index.json", ".gewyvern-writer-v1.lock"].map(str::to_owned))
            .collect();
        let extra: BTreeSet<_> = fs::read_dir(&h)
            .unwrap()
            .map(|e| e.unwrap().file_name().into_string().unwrap())
            .filter(|n| !names.contains(n))
            .collect();
        rounds.push(json!({"run":writer.id, "ms":writer.ts, "limit":writer.cap, "exit":exit,
            "index_sha256":v["index_sha256"], "latest_sha256":v["latest_tree"], "delta_sha256":v["delta_sha256"],
            "identity":observed_identity, "stage_sha256":staged, "extra":extra,"numeric_dirs":numeric_dirs(&h).len()}));
    }
    assert_eq!(rounds.len(), 3);
    let report = json!({"branch":if sidecar.is_some() {"prune"} else {"reclaim"},
        "interrupted_run":proof["killed_run"], "debt":debt,"identity_before":identity,"rounds":rounds});
    fs::write(
        root.join("summary.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    assert!(
        rounds.iter().all(|r| r["extra"] == json!([])),
        "cleanup debt survived an expired terminal: {rounds:?}"
    );
    api_archive::check(state, committed);
}
