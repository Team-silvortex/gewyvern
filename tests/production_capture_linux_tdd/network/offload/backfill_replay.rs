use super::*;

pub(super) struct Shelf<'a> {
    pub state: &'a Path,
    pub dir: &'a Path,
    pub early_ts: u64,
    pub committed: &'a [&'a CaptureSnapshot],
    pub gap: &'a Value,
    pub victim: &'a Path,
    pub candidate_hash: &'a str,
}

impl Shelf<'_> {
    pub(super) fn kill_and_restart(
        &self,
        cut: Cut,
        p: &mut Process,
        id: &str,
        restart: &mut impl FnMut(&str) -> (Process, String),
    ) -> (Process, String, Value) {
        let h = self.state.join("history/api/v1");
        let plan = h.join(".gewyvern-prune-v1.json");
        let intent = h.join(".gewyvern-inflight-v1.json");
        let stage = h.join(".gewyvern-pending-v1");
        let late = h.join(self.early_ts.to_string());
        let anchor = self.victim.join(".gewyvern-publication-v1.json");
        let original_anchor = fs::read(&anchor).unwrap();
        let original_plan = fs::read(&plan).unwrap();
        let tree = |p: &Path| api_archive::fingerprint_paths(p, &["."]);
        let indexed_hashes = || {
            self.gap["times"]
                .as_array()
                .unwrap()
                .iter()
                .map(|ts| tree(&h.join(ts.to_string())))
                .collect::<Vec<_>>()
        };
        let original_trees = indexed_hashes();
        let protocols = self.victim.join("protocols");
        let summaries: Vec<_> = fs::read_dir(&protocols)
            .unwrap()
            .map(|e| {
                let path = e.unwrap().path().join("summary.json");
                assert!(path.is_file());
                path
            })
            .collect();
        let remaining = || summaries.iter().filter(|p| p.is_file()).count();
        let total = summaries.len();
        assert!(total > 1);
        match cut {
            Cut::Before => {
                // The orphan has moved back to the reserved slot. Stop before
                // the stale eviction plan can be discarded against the old index.
                shrink_interruption::at_writer_open(p, &plan, || {
                    assert!(stage.is_dir() && !late.try_exists().unwrap());
                    signal(p, libc::SIGKILL);
                    true
                });
            }
            Cut::After => {
                // Only stat known paths while fanotify is armed; opening a
                // marked directory from the controller could block the observer.
                shrink_interruption::on_open(
                    p,
                    &protocols,
                    libc::FAN_ONDIR | libc::FAN_EVENT_ON_CHILD,
                    libc::SIGTERM,
                    |_| {
                        if !(1..total).contains(&remaining()) {
                            return false;
                        }
                        signal(p, libc::SIGKILL);
                        true
                    },
                );
            }
        }
        let sig = wait(p, 10).signal().unwrap();
        assert_eq!(sig, libc::SIGKILL);
        let stopped = snapshot(self.dir);
        assert_eq!(stopped.run_id, id);
        assert!(
            stopped.state == CaptureState::Stopped
                && stopped.final_accounted
                && stopped.reconciles()
                && stopped.retained == 4
        );
        assert_eq!(export(self.dir).capture.as_ref(), Some(&stopped));
        let inspect = || {
            let post = cut == Cut::After;
            let actual = view(self.state, self.committed, post.then_some(self.early_ts));
            assert_eq!(actual, *self.gap);
            assert_eq!(indexed_hashes(), original_trees);
            assert_eq!(fs::read(&plan).unwrap(), original_plan);
            assert_eq!(fs::read(&anchor).unwrap(), original_anchor);
            assert!(!intent.try_exists().unwrap());
            assert_eq!(late.try_exists().unwrap(), post);
            assert_eq!(stage.try_exists().unwrap(), !post);
            assert_eq!(tree(if post { &late } else { &stage }), self.candidate_hash);
            let mut expected: BTreeSet<_> = actual["times"]
                .as_array()
                .unwrap()
                .iter()
                .map(|ts| h.join(ts.to_string()))
                .collect();
            if post {
                expected.insert(self.victim.to_owned());
            }
            assert_eq!(numeric_dirs(&h), expected);
            let left = remaining();
            if post {
                assert!((1..total).contains(&left));
            } else {
                assert_eq!(left, total);
            }
            json!({"view":actual, "indexed_tree_sha256":original_trees, "numeric_dirs":expected.len(),
                "stage_present":stage.is_dir(), "late_numeric":late.is_dir(), "victim_summaries_left":left,
                "victim_summaries_total":total,"plan_sha256":hash(&original_plan),"anchor_sha256":hash(&original_anchor)})
        };
        let second_crash = inspect();
        let (replacement, new_id) = restart(id);
        assert_ne!(new_id, id);
        let second_running = inspect();
        assert_eq!(second_running, second_crash);
        let proof = json!({"killed_run":id,"signal":sig,"second_crash":second_crash,"second_running":second_running});
        (replacement, new_id, proof)
    }
}
