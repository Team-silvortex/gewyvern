use super::{read_repo_file, udp_offload::hash};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn expired_writers_drain_recovery_debt_without_replacing_the_committed_view() {
    let proof: Value = serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_expired_only_linux_x86_64_20261127.json",
    ))
    .unwrap();
    let mut cases = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for row in proof["runs"].as_array().unwrap() {
        let s = &row["summary"];
        assert_eq!(
            row["summary_sha256"],
            hash(&serde_json::to_vec_pretty(s).unwrap())
        );
        let fixed = row["fixed"].as_bool().unwrap();
        let prune = match s["branch"].as_str().unwrap() {
            "reclaim" => false,
            "prune" => true,
            s => panic!("unknown branch {s}"),
        };
        *cases.entry((fixed, prune)).or_insert(0) += 1;
        let debt = &s["debt"];
        let view = &debt["view"];
        assert_eq!(debt["stage_present"], !prune);
        assert_eq!(debt["numeric_dirs"], if prune { 5 } else { 4 });
        let archives = view["runs"].as_array().unwrap();
        assert_eq!(archives.len(), 4);
        for id in archives.iter().chain([&s["interrupted_run"]]) {
            assert!(ids.insert(id.as_str().unwrap()));
        }
        let times = view["times"].as_array().unwrap();
        let (owner, file) = if prune {
            (1, "index-head-delta.json")
        } else {
            (0, "protocol-delta.json")
        };
        assert_eq!(
            view["delta_path"],
            format!("history/api/v1/{}/{file}", times[owner])
        );
        let rounds = s["rounds"].as_array().unwrap();
        assert_eq!(rounds.len(), 3);
        for (r, cap) in rounds.iter().zip([1, 8, 4]) {
            assert!(ids.insert(r["run"].as_str().unwrap()));
            assert!(r["ms"].as_u64().unwrap() < times[3].as_u64().unwrap());
            assert_eq!(r["limit"], cap);
            assert_eq!(r["exit"], 0);
            assert_eq!(r["identity"], s["identity_before"]);
            assert_eq!(r["index_sha256"], view["index_sha256"]);
            assert_eq!(r["latest_sha256"], view["latest_tree"]);
            assert_eq!(r["delta_sha256"], view["delta_sha256"]);
            assert_eq!(r["numeric_dirs"], 4);
            if fixed || prune {
                assert_eq!(r["extra"], json!([]));
                assert!(r["stage_sha256"].is_null());
            } else {
                assert_eq!(r["extra"], json!([".gewyvern-pending-v1"]));
                assert_eq!(r["stage_sha256"].as_str().unwrap().len(), 64);
                assert_eq!(r["stage_sha256"], rounds[0]["stage_sha256"]);
            }
        }
    }
    assert_eq!(
        cases,
        BTreeMap::from([
            ((false, false), 1),
            ((false, true), 1),
            ((true, false), 3),
            ((true, true), 3)
        ])
    );
}

#[test]
fn double_crash_replays_only_the_recorded_backfill_or_eviction() {
    let proof: Value = serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_backfill_replay_linux_x86_64_20261127.json",
    ))
    .unwrap();
    let mut cases = BTreeMap::new();
    let mut ids = BTreeSet::new();
    for row in proof["runs"].as_array().unwrap() {
        let mut s = row["summary"].clone();
        s["running"] = s["crash"].clone();
        s["interrupted_replay"]["second_crash"]["view"] = s["crash"].clone();
        s["interrupted_replay"]["second_running"] = s["interrupted_replay"]["second_crash"].clone();
        assert_eq!(
            row["summary_sha256"],
            hash(&serde_json::to_vec_pretty(&s).unwrap())
        );
        let replay = &s["interrupted_replay"];
        assert_eq!(replay["signal"], 9);
        let debt = &replay["second_crash"];
        let gap = &s["crash"];
        let orig = &s["before"];
        let final_view = &s["recovered"];
        let seeds = orig["runs"].as_array().unwrap();
        assert_eq!(seeds.len(), 4);
        let fresh = &final_view["runs"][0];
        for id in seeds
            .iter()
            .chain([&s["late_run"], &replay["killed_run"], fresh])
        {
            assert!(ids.insert(id.as_str().unwrap().to_owned()));
        }
        let post = match s["cut"].as_str().unwrap() {
            "before" => false,
            "after" => true,
            other => panic!("unexpected index boundary: {other}"),
        };
        *cases.entry(post).or_insert(0) += 1;
        assert_eq!(s["crash_dirs"], 5);
        assert_eq!(gap["latest_tree"], orig["latest_tree"]);
        assert_eq!(debt["indexed_tree_sha256"][0], s["head_archive_sha256"]);
        assert_eq!(debt["stage_present"], !post);
        assert_eq!(debt["late_numeric"], post);
        let left = debt["victim_summaries_left"].as_u64().unwrap();
        let total = debt["victim_summaries_total"].as_u64().unwrap();
        assert!(total > 1);
        if post {
            assert!((1..total).contains(&left));
            assert_eq!(debt["numeric_dirs"], 5);
            assert_eq!(debt["indexed_tree_sha256"][1], s["late_tree_sha256"]);
            assert_eq!(
                gap["runs"],
                json!([seeds[0], s["late_run"], seeds[1], seeds[2]])
            );
            assert_eq!(
                gap["delta_path"],
                format!("history/api/v1/{}/index-head-delta.json", s["late_ms"])
            );
            assert_eq!(gap["delta_sha256"], s["late_delta_sha256"]);
        } else {
            assert_eq!(left, total);
            assert_eq!(debt["numeric_dirs"], 4);
            assert_eq!(gap, orig);
        }
        assert_eq!(
            final_view["runs"],
            json!([fresh, gap["runs"][0], gap["runs"][1], gap["runs"][2]])
        );
        assert!(final_view["times"][0].as_u64().unwrap() > orig["times"][0].as_u64().unwrap());
        assert_eq!(
            final_view["delta_path"],
            format!(
                "history/api/v1/{}/protocol-delta.json",
                final_view["times"][0]
            )
        );
        assert_eq!(s["final_dirs"], 4);
        assert_eq!(s["residue"], json!([]));
    }
    assert_eq!(cases, BTreeMap::from([(false, 3), (true, 3)]));
}
