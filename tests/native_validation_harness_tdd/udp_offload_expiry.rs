use super::read_repo_file;
use super::udp_offload::{hash, packet, payload};
use gewyvern::live_capture::{CaptureSnapshot, CaptureState};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn proof() -> Value {
    serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_offload_expiry_linux_x86_64_20261120.json",
    ))
    .unwrap()
}

fn parts(phase: usize, family: usize) -> Vec<Vec<u8>> {
    let n = if phase < 4 {
        1
    } else if phase == 7 {
        4
    } else {
        16
    };
    (0..n)
        .map(|id| {
            payload(
                phase,
                family,
                id,
                if n == 1 {
                    32
                } else if id + 1 == n {
                    17
                } else {
                    1024
                },
            )
        })
        .collect()
}

fn snapshot(value: &Value, counts: [u64; 5], terminal: bool) -> CaptureSnapshot {
    let s: CaptureSnapshot = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        [
            s.kernel.matched,
            s.received,
            s.retained,
            s.userspace_dropped,
            s.decode_rejected
        ],
        counts
    );
    assert_eq!(s.kernel.ring_dropped, 8);
    assert_eq!(
        s.kernel.unsupported + s.kernel.malformed + s.kernel.no_cookie,
        0
    );
    assert!(s.incomplete);
    assert_eq!(s.final_accounted, terminal);
    assert_eq!(
        s.state,
        if terminal {
            CaptureState::Stopped
        } else {
            CaptureState::Running
        }
    );
    assert_eq!(s.reconciles(), s.decode_rejected == 9);
    s
}

fn verify(proof: &Value) {
    assert_eq!(proof["release_artifact"], false);
    assert_eq!(proof["status_maturity_promoted"], false);
    let allocation = &proof["allocation_regression"];
    assert_eq!(allocation["before_failed_tests"], 2);
    assert_eq!(allocation["after_passed_tests"], 2);
    assert_eq!(allocation["capacities"], json!([1, 24, 65536]));
    assert_eq!(allocation["empty_calls_per_capacity"], 1000);
    assert_eq!(allocation["whole_service_allocation_free_claim"], false);
    let runs = proof["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2);
    let mut families = BTreeSet::new();
    for run in runs {
        let family = run["family"].as_u64().unwrap();
        assert!([4, 6].contains(&family) && families.insert(family));
        let ip_index = usize::from(family == 6);
        for flag in [
            "ok",
            "production_cli",
            "expired_without_new_traffic",
            "capacity_recovered",
            "same_socket_recovery",
            "packet_multiset_matches",
            "replay_identical",
            "hooks_detached",
            "owned_resources_removed",
        ] {
            assert_eq!(run[flag], true);
        }
        assert_eq!(run["host_network_changed"], false);
        assert_eq!(run["binary_sha256"], proof["binary_sha256"]);
        assert_eq!(run["native_requests"], 56);
        assert_eq!(run["native_replies"], 8);
        let elapsed = run["expiry_elapsed_ms"].as_f64().unwrap();
        assert!((1000.0..8000.0).contains(&elapsed));
        let terminal = snapshot(&run["terminal"], [64, 56, 31, 16, 9], true);
        let stages = run["stages"].as_array().unwrap();
        assert_eq!(stages.len(), 5);
        for (stage, (name, counts, facts)) in stages.iter().zip([
            ("partial", [25, 17, 8, 0, 0], 8),
            ("reserved", [42, 34, 9, 16, 0], 1),
            ("expired", [42, 34, 9, 16, 9], 0),
            ("admitted", [59, 51, 26, 16, 9], 17),
            ("recovery", [64, 56, 31, 16, 9], 5),
        ]) {
            assert_eq!(stage["stage"], name);
            assert_eq!(stage["facts"], facts);
            assert_eq!(
                snapshot(&stage["capture"], counts, false).run_id,
                terminal.run_id
            );
        }
        assert_eq!(
            run["group_indexes"],
            json!({"3":(0..16).collect::<Vec<_>>(),"4":(0..4).collect::<Vec<_>>()})
        );
        let native = run["native"].as_array().unwrap();
        assert_eq!(native.len(), 2);
        assert_ne!(
            native[0]["identity"]["namespace"],
            native[1]["identity"]["namespace"]
        );
        assert_ne!(
            native[0]["identity"]["cookie"],
            native[1]["identity"]["cookie"]
        );
        for (peer, server) in native.iter().zip([false, true]) {
            assert_eq!(peer["same_socket_all_phases"], true);
            assert_eq!(
                peer["identity"]["role"],
                if server { "server" } else { "client" }
            );
            assert_eq!(peer["identity"]["family"], family);
            assert!(
                peer["identity"]["cookie"]
                    .as_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
                    > 0
            );
            assert!(peer["identity"]["ifindex"].as_u64().unwrap() > 0);
            for counter in [
                "IpFragCreates",
                "IpReasmOKs",
                "Ip6FragCreates",
                "Ip6ReasmOKs",
            ] {
                assert_eq!(peer["fragment_counter_deltas"][counter], 0);
            }
            let phases = peer["phases"].as_array().unwrap();
            assert_eq!(phases.len(), 8);
            for (phase, row) in phases.iter().enumerate() {
                let parts = parts(phase, ip_index);
                let request = parts.concat();
                let reply = payload(phase, ip_index, 0, 32);
                assert_eq!(row["phase"], phase);
                assert_eq!(row["segment_count"], parts.len());
                assert_eq!(
                    row["request_lengths"],
                    json!(parts.iter().map(Vec::len).collect::<Vec<_>>())
                );
                assert_eq!(row["request_sha256"], hash(&request));
                assert_eq!(row["reply_sha256"], hash(&reply));
                let received = if server { request } else { reply };
                assert_eq!(row["received_length"], received.len());
                assert_eq!(row["received_sha256"], hash(&received));
                assert_eq!(
                    row["gro_segment"],
                    if server && phase >= 4 {
                        json!(1024)
                    } else {
                        Value::Null
                    }
                );
                assert_eq!(row["extra_datagram_observed"], false);
            }
        }
        let cookie = &native[0]["identity"]["cookie"];
        let mut expected = Vec::new();
        for phase in [0, 1, 2, 3, 5, 6, 7] {
            if phase != 5 {
                for part in parts(phase, ip_index) {
                    expected.push(packet(false, ip_index, cookie, true, &part));
                }
            }
            expected.push(packet(
                false,
                ip_index,
                cookie,
                false,
                &payload(phase, ip_index, 0, 32),
            ));
        }
        assert_eq!(run["expected_count"], expected.len());
        assert_eq!(expected.len(), 31);
        assert_eq!(
            run["expected_sha256"],
            hash(&serde_json::to_vec(&expected).unwrap())
        );
    }
    let repeats = proof["repetitions"].as_array().unwrap();
    assert_eq!(repeats.len(), 6);
    let mut cases = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for repeat in repeats {
        let family = repeat["family"].as_u64().unwrap();
        let iteration = repeat["iteration"].as_u64().unwrap();
        assert!([4, 6].contains(&family) && (1..=3).contains(&iteration));
        assert!(cases.insert((family, iteration)));
        assert!(ids.insert(snapshot(&repeat["terminal"], [64, 56, 31, 16, 9], true).run_id));
        assert_eq!(repeat["summary_sha256"].as_str().unwrap().len(), 64);
    }
}

#[test]
fn product_idle_expiry_proof_reconciles_native_io_reservations_and_timeout() {
    verify(&proof());
}

#[test]
fn product_shutdown_drain_finishes_pending_group_without_carrying_loss_into_restart() {
    let proof: Value = serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_offload_shutdown_linux_x86_64_20261127.json",
    ))
    .unwrap();
    let mut ids = BTreeSet::new();
    for case in dual_stack_runs(&proof) {
        let run = &case["summary"];
        assert_eq!(run["lossy_exit"], 3);
        assert_eq!(run["restart_exit"], 0);
        assert_eq!(run["final_window_facts"], 1);
        assert!(run["resume_to_exit_ms"].as_u64().unwrap() < 2000);
        let partial = snapshot(&run["pending_window"], [25, 17, 8, 0, 0], false);
        let stop = snapshot(&run["stopped"], [42, 34, 9, 16, 9], true);
        assert_eq!(stop.run_id, partial.run_id);
        assert!(ids.insert(stop.run_id));
        let new: CaptureSnapshot = serde_json::from_value(run["restart"].clone()).unwrap();
        assert!(ids.insert(new.run_id.clone()));
        assert_eq!(new.cgroup, partial.cgroup);
        assert_eq!(stop.cgroup, new.cgroup);
        assert_eq!(new.udp_port, partial.udp_port);
        assert_eq!(new.state, CaptureState::Stopped);
        assert!(new.final_accounted && new.reconciles() && !new.incomplete && !new.has_loss());
        assert_eq!(new.retained, 22);
    }
}

fn dual_stack_runs(proof: &Value) -> &[Value] {
    let runs = proof["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 6);
    let mut cases = BTreeSet::new();
    for case in runs {
        let run = &case["summary"];
        assert_eq!(
            case["summary_sha256"],
            hash(&serde_json::to_vec_pretty(run).unwrap())
        );
        assert_eq!(run["binary_sha256"], proof["binary_sha256"]);
        let family = run["family"].as_u64().unwrap();
        assert!([4, 6].contains(&family));
        let name = case["summary_file"].as_str().unwrap();
        let iteration = (1..=3)
            .find(|n| name == format!("v{family}-{n}/summary.json"))
            .unwrap();
        assert!(cases.insert((family, iteration)));
    }
    runs
}

#[test]
fn pending_crash_keeps_stale_evidence_and_excludes_downtime_from_restart() {
    let proof: Value = serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_offload_pending_crash_linux_x86_64_20261127.json",
    ))
    .unwrap();
    let mut ids = BTreeSet::new();
    for case in dual_stack_runs(&proof) {
        let run = &case["summary"];
        assert_eq!(run["signal"], 9);
        assert_eq!(run["stale_window_facts"], 8);
        assert_eq!(run["recovery_window_facts"], 5);
        assert_eq!(run["restart_exit"], 0);
        let stale = snapshot(&run["stale"], [25, 17, 8, 0, 0], false);
        assert_eq!(
            run["stale_status_sha256"],
            hash(&serde_json::to_vec(&stale).unwrap())
        );
        let mut fresh: CaptureSnapshot = serde_json::from_value(run["fresh"].clone()).unwrap();
        assert!(ids.insert(stale.run_id) && ids.insert(fresh.run_id.clone()));
        assert_eq!(fresh.cgroup, stale.cgroup);
        assert_eq!(fresh.udp_port, stale.udp_port);
        assert_eq!(fresh.state, CaptureState::Running);
        assert!(
            fresh.reconciles() && !fresh.has_loss() && !fresh.incomplete && !fresh.final_accounted
        );
        assert_eq!(fresh.received, 0);
        fresh.kernel.matched = 5;
        fresh.received = 5;
        fresh.retained = 5;
        fresh.state = CaptureState::Stopped;
        fresh.final_accounted = true;
        assert_eq!(run["terminal"], json!(fresh));

        let peers = case["peers"].as_array().unwrap();
        assert_eq!(peers.len(), 2);
        assert_ne!(
            peers[0]["identity"]["cookie"],
            peers[1]["identity"]["cookie"]
        );
        for (peer, server) in peers.iter().zip([false, true]) {
            assert_eq!(peer["same_socket_all_phases"], true);
            assert_eq!(peer["identity"]["family"], run["family"]);
            assert_eq!(
                peer["identity"]["role"],
                if server { "server" } else { "client" }
            );
            let family = usize::from(run["family"] == 6);
            let rows = peer["receives"].as_array().unwrap();
            assert_eq!(rows.len(), 3);
            for (row, phase) in rows.iter().zip(5..=7) {
                let bytes = if server {
                    parts(phase, family).concat()
                } else {
                    payload(phase, family, 0, 32)
                };
                assert_eq!(row["phase"], phase);
                assert_eq!(row["received_sha256"], hash(&bytes));
                assert_eq!(
                    row["gro_segment"],
                    if server { json!(1024) } else { Value::Null }
                );
            }
        }
    }
}

#[test]
fn read_only_cli_outputs_preserve_last_window_and_restart_in_place() {
    verify_read_only_outputs(
        "docs/fixtures/gewyvern_product_udp_readonly_outputs_linux_x86_64_20261127.json",
        false,
    );
}

#[test]
fn reverse_order_terminals_follow_the_head_not_the_last_writer() {
    let proof: Value = serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_terminal_order_linux_x86_64_20261127.json",
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
        let head_cap = s["head_limit"].as_u64().unwrap();
        *cases.entry((fixed, head_cap)).or_insert(0) += 1;
        assert!(s["early_ms"].as_u64().unwrap() < s["head_ms"].as_u64().unwrap());
        let seeds = s["seed_ids"].as_array().unwrap();
        assert_eq!(seeds.len(), 3);
        for id in seeds.iter().chain([&s["early_run"], &s["head_run"]]) {
            assert!(ids.insert(id.as_str().unwrap()));
        }
        assert_eq!(s["head_tree_after"], s["head_tree_before"]);
        assert_eq!(s["latest_tree_after"] == s["latest_tree_before"], fixed);
        assert_eq!(
            s["latest_after"],
            s[if fixed { "head_run" } else { "early_run" }]
        );
        assert_eq!(s["early_limit"], if head_cap == 4 { 1 } else { 4 });
        assert_eq!(
            s["index_limit"],
            s[if fixed { "head_limit" } else { "early_limit" }]
        );
        let expected = match (fixed, head_cap) {
            (true, 4) => json!([s["head_run"], s["early_run"], seeds[2], seeds[1]]),
            (false, 1) => json!([s["head_run"], s["early_run"]]),
            _ => json!([s["head_run"]]),
        };
        assert_eq!(s["index_runs"], expected);
        assert_eq!(s["physical_archives"], expected.as_array().unwrap().len());
    }
    assert_eq!(
        cases,
        BTreeMap::from([
            ((false, 1), 1),
            ((false, 4), 1),
            ((true, 1), 3),
            ((true, 4), 3)
        ])
    );
}

#[test]
fn interrupted_backfill_keeps_the_committed_view_until_a_new_terminal() {
    let proof: Value = serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_backfill_interruption_linux_x86_64_20261127.json",
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
        let cut = s["cut"].as_str().unwrap();
        *cases.entry(cut).or_insert(0) += 1;
        let (before, gap, final_view) = (&s["before"], &s["crash"], &s["recovered"]);
        let seeds = before["runs"].as_array().unwrap();
        assert_eq!(seeds.len(), 4);
        let fresh = &final_view["runs"][0];
        let late = &s["late_run"];
        for id in seeds.iter().chain([fresh, late]) {
            assert!(ids.insert(id.as_str().unwrap()));
        }
        assert_eq!(s["running"], *gap);
        assert_eq!(gap["latest_tree"], before["latest_tree"]);
        assert_eq!(s["crash_dirs"], 5);
        assert_eq!(s["final_dirs"], 4);
        assert_eq!(s["residue"], json!([]));
        let before_rows = &before["times"];
        let late_ms = s["late_ms"].as_u64().unwrap();
        assert!(
            before_rows[0].as_u64().unwrap() > late_ms
                && late_ms > before_rows[1].as_u64().unwrap()
        );
        if cut == "before" {
            assert_eq!(gap, before);
        } else {
            assert_eq!(cut, "after");
            assert_eq!(gap["runs"], json!([seeds[0], late, seeds[1], seeds[2]]));
            assert_eq!(
                gap["times"],
                json!([before_rows[0], late_ms, before_rows[1], before_rows[2]])
            );
            assert_eq!(
                gap["delta_path"],
                format!("history/api/v1/{late_ms}/index-head-delta.json")
            );
            assert_eq!(gap["delta_sha256"], s["late_delta_sha256"]);
        }
        assert_eq!(
            final_view["runs"],
            json!([fresh, gap["runs"][0], gap["runs"][1], gap["runs"][2]])
        );
        let t = final_view["times"][0].as_u64().unwrap();
        assert!(t > before_rows[0].as_u64().unwrap());
        assert_eq!(
            final_view["delta_path"],
            format!("history/api/v1/{t}/protocol-delta.json")
        );
    }
    assert_eq!(cases, BTreeMap::from([("before", 3), ("after", 3)]));
}

#[test]
fn shrink_replay_keeps_the_committed_pair_until_the_new_policy_commits() {
    let proof: Value = serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_history_shrink_linux_x86_64_20261127.json",
    ))
    .unwrap();
    let mut ids = BTreeSet::new();
    let mut cases = BTreeMap::new();
    for row in proof["runs"].as_array().unwrap() {
        let s = &row["summary"];
        assert_eq!(
            row["summary_sha256"],
            hash(&serde_json::to_vec_pretty(s).unwrap())
        );
        let fixed = row["fixed"].as_bool().unwrap();
        let cap = s["restart_cap"].as_u64().unwrap();
        *cases.entry((fixed, cap)).or_insert(0) += 1;
        assert_eq!((&s["seed_cap"], &s["shrink_cap"]), (&json!(5), &json!(2)));
        let seeds = s["seed_ids_newest_first"].as_array().unwrap();
        assert_eq!(seeds.len(), 5);
        for id in seeds.iter().chain([&s["shrink_id"], &s["repair_id"]]) {
            assert!(ids.insert(id.as_str().unwrap()));
        }
        let survivors = json!([s["shrink_id"], seeds[0]]);
        if fixed {
            assert_eq!(s["post_replay_pre_commit_ids"], survivors);
        }
        let shown = if fixed { 2 } else { cap };
        assert_eq!(s["text_retention_during_restart"], shown.to_string());
        assert_eq!(s["gap_old_victims_remaining"], 3);
        assert_eq!(s["gap_victim_sha256"].as_array().unwrap().len(), 3);
        let mut expected = vec![&s["repair_id"], &s["shrink_id"], &seeds[0]];
        expected.truncate(cap as usize);
        assert_eq!(s["final_ids"], json!(expected));
        assert_eq!(s["final_directories"], expected.len());
        assert_eq!(s["pending_prune"], false);
    }
    assert_eq!(
        cases,
        BTreeMap::from([
            ((false, 1), 1),
            ((false, 4), 1),
            ((true, 1), 3),
            ((true, 4), 3)
        ])
    );
}

#[test]
fn committed_history_prune_proofs_cover_partial_deletion_and_interrupted_recovery() {
    let proof: Value = serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_history_prune_linux_x86_64_20261127.json",
    ))
    .unwrap();
    let fixed = proof["fixed_runs"].as_array().unwrap();
    assert_eq!(fixed.len(), 3);
    let mut ids = BTreeSet::new();
    for (i, row) in std::iter::once(&proof["before_fix"])
        .chain(fixed)
        .chain([&proof["resume_run"]])
        .enumerate()
    {
        let s = &row["summary"];
        assert_eq!(
            row["summary_sha256"],
            hash(&serde_json::to_vec_pretty(s).unwrap())
        );
        assert_eq!(s["retention"], 2);
        let final_index = s["final_index"].as_array().unwrap();
        assert_eq!(final_index.len(), 2);
        for id in [&s["oldest"], &s["newer"], &final_index[0]] {
            assert!(ids.insert(id.as_str().unwrap()));
        }
        let rounds = s["rounds"].as_array().unwrap();
        assert_eq!(rounds.len(), 3);
        for (n, r) in rounds.iter().enumerate() {
            assert!(ids.insert(r["committed"].as_str().unwrap()));
            let remaining = r["retired_files_remaining"].as_u64().unwrap();
            assert!((1..r["retired_files_before"].as_u64().unwrap()).contains(&remaining));
            assert_eq!(r["retired_dirs_remaining"], if i == 0 { n + 1 } else { 1 });
        }
        assert_eq!(final_index[1], rounds[2]["committed"]);
        assert_eq!(s["final_numeric_dirs"], if i == 0 { 5 } else { 2 });
    }
    let s = &proof["resume_run"]["summary"];
    let retries = s["resume"].as_array().unwrap();
    assert_eq!(retries.len(), 2);
    let mut left = s["rounds"][2]["retired_files_remaining"].as_u64().unwrap();
    for r in retries {
        assert!(ids.insert(r["run"].as_str().unwrap()));
        assert_eq!(r["files_before"], left);
        let next = r["files_remaining"].as_u64().unwrap();
        assert!((1..left).contains(&next));
        left = next;
    }
}

#[test]
fn interrupted_rename_proofs_bound_orphans_without_evicting_committed_history() {
    let proof: Value = serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_history_retention_rename_linux_x86_64_20261127.json",
    ))
    .unwrap();
    let fixed = proof["fixed_runs"].as_array().unwrap();
    assert_eq!(fixed.len(), 3);
    let mut ids = BTreeSet::new();
    for (i, row) in std::iter::once(&proof["before_fix"])
        .chain(fixed)
        .enumerate()
    {
        let s = &row["summary"];
        assert_eq!(
            row["summary_sha256"],
            hash(&serde_json::to_vec_pretty(s).unwrap())
        );
        assert_eq!(s["retention"], 2);
        assert_eq!(s["old_tree_sha256"].as_array().unwrap().len(), 2);
        let commits = s["final_index"].as_array().unwrap();
        assert_eq!(commits.len(), 2);
        assert_eq!(commits[1], s["newer"]);
        for id in [&commits[0], &s["newer"], &s["oldest"]] {
            assert!(ids.insert(id.as_str().unwrap()));
        }
        let rounds = s["rounds"].as_array().unwrap();
        assert_eq!(rounds.len(), 3);
        for (n, round) in rounds.iter().enumerate() {
            assert!(ids.insert(round["interrupted_run"].as_str().unwrap()));
            assert_eq!(round["unindexed"], if i == 0 { n + 1 } else { 1 });
        }
        assert_eq!(s["final_numeric_dirs"], if i == 0 { 5 } else { 2 });
    }
}

#[test]
fn interrupted_history_receipts_keep_live_identity_separate_from_committed_history() {
    let proof: Value = serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_interrupted_history_linux_x86_64_20261127.json",
    ))
    .unwrap();
    let runs = proof["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 3);
    let mut ids = BTreeSet::new();
    for row in runs {
        let s = &row["summary"];
        assert_eq!(
            row["summary_sha256"],
            hash(&serde_json::to_vec_pretty(s).unwrap())
        );
        for key in ["seed_run", "interrupted_run", "repaired_run"] {
            assert!(ids.insert(s[key].as_str().unwrap()));
        }
        assert_eq!(s["restart_health_run"], s["repaired_run"]);
        assert_eq!(s["signal"], 9);
        assert_eq!(s["latest_at_kill_retained"], 4);
        assert_eq!(s["history_at_kill"], 1);
        assert_eq!(s["history_after_restart"], 2);
        assert_eq!(s["repaired_retained"], 4);
    }
}

#[test]
fn read_only_archive_preserves_prior_history_and_only_archives_the_repaired_run() {
    verify_read_only_outputs(
        "docs/fixtures/gewyvern_product_udp_readonly_archive_linux_x86_64_20261127.json",
        true,
    );
}

fn verify_read_only_outputs(path: &str, block_api: bool) {
    let proof: Value = serde_json::from_str(&read_repo_file(path)).unwrap();
    let mut cases = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for row in proof["runs"].as_array().unwrap() {
        let s = &row["summary"];
        assert_eq!(
            row["summary_sha256"],
            hash(&serde_json::to_vec_pretty(s).unwrap())
        );
        let family = s["family"].as_u64().unwrap();
        let case = s["case"].as_str().unwrap();
        assert!(
            [4, 6].contains(&family)
                && matches!(
                    case,
                    "running" | "terminal" | "running-api" | "terminal-api"
                )
        );
        assert!(cases.insert((family, case)));
        assert_eq!(s["exit"], 1);
        assert_eq!(s["error"], "output_write_failed");
        assert_eq!(s["status_error"], "capture_status_write_failed");
        let stale = snapshot(&s["stale"], [25, 17, 8, 0, 0], false);
        assert_eq!(
            s["stale_status_sha256"],
            hash(&serde_json::to_vec(&stale).unwrap())
        );
        if block_api {
            assert!(case.ends_with("-api"));
            let archive = &s["archive"];
            assert!(ids.insert(archive["seed_run_id"].as_str().unwrap().to_owned()));
            assert_eq!(archive["history_before"], 1);
            assert_eq!(archive["history_after_fault"], 1);
            assert_eq!(archive["history_after_repair"], 2);
            assert_eq!(archive["persistence_error_reported"], true);
            assert_eq!(
                archive["tree_sha256_before_and_after"]
                    .as_str()
                    .unwrap()
                    .len(),
                64
            );
            assert!(s["api_failure"].is_null());
            let error = &row["stderr"]["error"];
            assert_eq!(error["code"], s["error"]);
            assert_eq!(error["exit_code"], s["exit"]);
            assert_eq!(error["retryable"], true);
            for token in [
                "export.json",
                "[capture_status_write_failed]",
                "[snapshot_persist_failed]",
                "$CASE/runtime/state/",
                "os error 30",
            ] {
                assert!(
                    error["message"].as_str().unwrap().contains(token),
                    "{token}"
                );
            }
        } else if case.ends_with("-api") {
            let failure: CaptureSnapshot =
                serde_json::from_value(s["api_failure"].clone()).unwrap();
            assert_eq!(failure.run_id, stale.run_id);
            assert_eq!(failure.state, CaptureState::Failed);
            assert!(failure.final_accounted && failure.incomplete && failure.reconciles());
        } else {
            assert!(s["api_failure"].is_null());
        }
        let mut fresh: CaptureSnapshot = serde_json::from_value(s["fresh"].clone()).unwrap();
        assert!(ids.insert(stale.run_id) && ids.insert(fresh.run_id.clone()));
        assert_eq!(fresh.cgroup, stale.cgroup);
        assert_eq!(fresh.udp_port, stale.udp_port);
        assert_eq!(fresh.state, CaptureState::Running);
        assert!(
            !fresh.final_accounted && !fresh.incomplete && !fresh.has_loss() && fresh.reconciles()
        );
        assert_eq!(fresh.kernel.matched, 0);
        fresh.kernel.matched = 5;
        fresh.received = 5;
        fresh.retained = 5;
        fresh.state = CaptureState::Stopped;
        fresh.final_accounted = true;
        assert_eq!(s["repaired"], json!(fresh));
    }
    assert_eq!(cases.len(), if block_api { 4 } else { 8 });
}

#[test]
fn product_idle_expiry_proof_rejects_fabricated_completion_and_missing_gro() {
    let original = proof();
    for case in 0..5 {
        let mut bad = original.clone();
        match case {
            0 => bad["runs"][0]["stages"][2]["capture"]["decode_rejected"] = json!("0"),
            1 => bad["runs"][0]["stages"][2]["facts"] = json!(9),
            2 => bad["runs"][0]["group_indexes"]["1"] = json!((0..16).collect::<Vec<_>>()),
            3 => bad["runs"][0]["stages"][2]["capture"]["final_accounted"] = json!(true),
            _ => bad["runs"][0]["native"][1]["phases"][6]["gro_segment"] = Value::Null,
        }
        assert!(
            std::panic::catch_unwind(|| verify(&bad)).is_err(),
            "case {case}"
        );
    }
}
