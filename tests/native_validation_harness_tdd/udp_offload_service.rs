use super::read_repo_file;
use super::udp_offload::{hash, packet, payload};
use gewyvern::live_capture::{CaptureSnapshot, CaptureState};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn proof() -> Value {
    serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_offload_service_linux_x86_64_20261120.json",
    ))
    .unwrap()
}

fn snapshot(
    value: &Value,
    matched: u64,
    received: u64,
    retained: u64,
    dropped: u64,
    terminal: bool,
) -> CaptureSnapshot {
    let s: CaptureSnapshot = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(s.kernel.matched, matched);
    assert_eq!(s.received, received);
    assert_eq!(s.retained, retained);
    assert_eq!(s.userspace_dropped, dropped);
    assert_eq!(
        s.kernel.ring_dropped
            + s.kernel.unsupported
            + s.kernel.malformed
            + s.kernel.no_cookie
            + s.decode_rejected,
        0
    );
    assert_eq!(s.incomplete, dropped != 0);
    assert_eq!(s.final_accounted, terminal);
    assert_eq!(
        s.state,
        if terminal {
            CaptureState::Stopped
        } else {
            CaptureState::Running
        }
    );
    if terminal {
        assert!(s.reconciles());
    }
    s
}

fn parts(round: usize, family: usize) -> Vec<Vec<u8>> {
    let count = if round < 8 { 16 } else { 4 };
    (0..count)
        .map(|id| payload(round, family, id, if id + 1 == count { 17 } else { 1024 }))
        .collect()
}

fn verify(proof: &Value) {
    assert_eq!(proof["release_artifact"], false);
    assert_eq!(proof["status_maturity_promoted"], false);
    let runs = proof["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 2);
    let mut limits = BTreeSet::new();
    for run in runs {
        let limit = run["limit"].as_u64().unwrap();
        assert!([512, 528].contains(&limit) && limits.insert(limit));
        let budget = limit == 512;
        let dropped = if budget { 16 } else { 0 };
        for flag in [
            "ok",
            "production_cli",
            "same_socket_restart",
            "downtime_not_backfilled",
            "run_id_changed",
            "group_ids_restart_at_one",
            "whole_groups_per_window",
            "window_replay_identical",
            "hooks_detached",
            "owned_resources_removed",
        ] {
            assert_eq!(run[flag], true);
        }
        assert_eq!(run["host_network_changed"], false);
        assert_eq!(run["binary_sha256"], proof["binary_sha256"]);
        assert_eq!(run["kernel_batch_limit"], 512);
        assert_eq!(run["native_requests"], 288);
        assert_eq!(run["native_replies"], 24);
        let first = snapshot(
            &run["first_window"],
            544,
            512,
            510,
            if budget { 2 } else { 0 },
            false,
        );
        assert!(!first.reconciles());
        snapshot(
            &run["second_window"],
            544,
            544,
            544 - dropped,
            dropped,
            false,
        );
        snapshot(
            &run["recovery_window"],
            564,
            564,
            564 - dropped,
            dropped,
            false,
        );
        let service = snapshot(
            &run["service_terminal"],
            564,
            564,
            564 - dropped,
            dropped,
            true,
        );
        let crash = snapshot(&run["crash_snapshot"], 20, 20, 20, 0, false);
        let restart = snapshot(&run["restart_terminal"], 20, 20, 20, 0, true);
        assert_eq!(first.run_id, service.run_id);
        assert_ne!(service.run_id, crash.run_id);
        assert_ne!(crash.run_id, restart.run_id);
        assert_ne!(service.run_id, restart.run_id);
        assert_eq!(run["first_window_facts"], 510);
        assert_eq!(run["second_window_facts"], if budget { 18 } else { 34 });
        assert_eq!(run["recovery_window_facts"], 20);
        assert_eq!(
            run["first_window_groups"],
            json!((1..=30).collect::<Vec<_>>())
        );
        assert_eq!(
            run["second_window_groups"],
            if budget { json!([32]) } else { json!([31, 32]) }
        );
        assert_eq!(run["recovery_window_groups"], json!([33, 34, 35, 36]));
        let native = run["native"].as_array().unwrap();
        assert_eq!(native.len(), 2);
        assert_ne!(
            native[0]["identity"]["namespace"],
            native[1]["identity"]["namespace"]
        );
        let mut cookies = BTreeSet::new();
        for (peer, server) in native.iter().zip([false, true]) {
            assert_eq!(peer["same_sockets_all_rounds"], true);
            assert_eq!(
                peer["identity"]["role"],
                if server { "server" } else { "client" }
            );
            assert!(peer["identity"]["ifindex"].as_u64().unwrap() > 0);
            let ids = peer["identity"]["cookies"].as_array().unwrap();
            assert_eq!(ids.len(), 2);
            for cookie in ids {
                let cookie: u64 = cookie.as_str().unwrap().parse().unwrap();
                assert!(cookie > 0 && cookies.insert(cookie));
            }
            for name in [
                "IpFragCreates",
                "IpReasmOKs",
                "Ip6FragCreates",
                "Ip6ReasmOKs",
            ] {
                assert_eq!(peer["fragment_counter_deltas"][name], 0);
            }
            let rounds = peer["rounds"].as_array().unwrap();
            assert_eq!(rounds.len(), 12);
            for (round, row) in rounds.iter().enumerate() {
                assert_eq!(row["round"], round);
                assert_eq!(row["segment_count"], if round < 8 { 16 } else { 4 });
                let observations = row["observations"].as_array().unwrap();
                assert_eq!(observations.len(), 2);
                for (family, observation) in observations.iter().enumerate() {
                    let parts = parts(round, family);
                    let request = parts.concat();
                    let reply = payload(round, family, 0, 32);
                    assert_eq!(observation["cookie"], ids[family]);
                    assert_eq!(observation["family"], if family == 0 { 4 } else { 6 });
                    assert_eq!(
                        observation["request_lengths"],
                        json!(parts.iter().map(Vec::len).collect::<Vec<_>>())
                    );
                    assert_eq!(observation["request_sha256"], hash(&request));
                    assert_eq!(observation["reply_sha256"], hash(&reply));
                    let received = if server { request } else { reply };
                    assert_eq!(observation["received_length"], received.len());
                    assert_eq!(observation["received_sha256"], hash(&received));
                    assert_eq!(
                        observation["gro_segment"],
                        if server { json!(1024) } else { Value::Null }
                    );
                    assert_eq!(observation["extra_datagram_observed"], false);
                }
            }
        }
        for (name, rounds, drop_group) in [
            ("service_oracle", (0..9).collect::<Vec<_>>(), budget),
            ("crash_oracle", vec![9], false),
            ("restart_oracle", vec![11], false),
        ] {
            let value = &run[name];
            let group_ids: Vec<_> = if name == "service_oracle" {
                (1..=36).filter(|id| !budget || *id != 31).collect()
            } else {
                vec![1, 2, 3, 4]
            };
            assert_eq!(value["group_ids"], json!(group_ids));
            assert_eq!(
                value["facts"],
                if name == "service_oracle" {
                    564 - dropped
                } else {
                    20
                }
            );
            let oracles = value["oracles"].as_array().unwrap();
            assert_eq!(oracles.len(), 2);
            for (index, oracle) in oracles.iter().enumerate() {
                let server = index == 1;
                for field in ["role", "cookies", "namespace", "ifindex"] {
                    assert_eq!(oracle[field], native[index]["identity"][field]);
                }
                let mut expected = Vec::new();
                for round in &rounds {
                    for family in 0..2 {
                        let cookie = &oracle["cookies"][family];
                        if !(drop_group && *round == 7 && !server && family == 1) {
                            for part in parts(*round, family) {
                                expected.push(packet(server, family, cookie, !server, &part));
                            }
                        }
                        expected.push(packet(
                            server,
                            family,
                            cookie,
                            server,
                            &payload(*round, family, 0, 32),
                        ));
                    }
                }
                assert_eq!(oracle["expected_count"], expected.len());
                assert_eq!(
                    oracle["expected_sha256"],
                    hash(&serde_json::to_vec(&expected).unwrap())
                );
            }
        }
    }
    let repeats = proof["repetitions"].as_array().unwrap();
    assert_eq!(repeats.len(), 6);
    let mut cases = BTreeSet::new();
    let mut ids = BTreeSet::new();
    for repeat in repeats {
        let limit = repeat["limit"].as_u64().unwrap();
        let iteration = repeat["iteration"].as_u64().unwrap();
        assert!([512, 528].contains(&limit) && (1..=3).contains(&iteration));
        assert!(cases.insert((limit, iteration)));
        assert_eq!(repeat["summary_sha256"].as_str().unwrap().len(), 64);
        let dropped = if limit == 512 { 16 } else { 0 };
        let service = snapshot(
            &repeat["service_terminal"],
            564,
            564,
            564 - dropped,
            dropped,
            true,
        );
        let restart = snapshot(&repeat["restart_terminal"], 20, 20, 20, 0, true);
        assert!(ids.insert(service.run_id) && ids.insert(restart.run_id));
    }
}

#[test]
fn product_offload_service_proof_checks_window_budgets_native_io_and_restart() {
    verify(&proof());
}

#[test]
fn product_offload_service_proof_rejects_partial_resurrection_and_crash_completion() {
    let original = proof();
    for case in 0..5 {
        let mut bad = original.clone();
        match case {
            0 => bad["runs"][0]["first_window_groups"] = json!((1..=31).collect::<Vec<_>>()),
            1 => bad["runs"][1]["second_window_groups"] = json!([31, 32]),
            2 => bad["runs"][0]["crash_snapshot"]["final_accounted"] = json!(true),
            3 => {
                bad["runs"][0]["restart_terminal"]["run_id"] =
                    bad["runs"][0]["crash_snapshot"]["run_id"].clone()
            }
            _ => {
                bad["runs"][0]["native"][1]["rounds"][7]["observations"][0]["gro_segment"] =
                    Value::Null
            }
        }
        assert!(
            std::panic::catch_unwind(|| verify(&bad)).is_err(),
            "case {case}"
        );
    }
}
