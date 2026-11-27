use super::read_repo_file;
use super::udp_offload::{hash, packet, payload};
use gewyvern::live_capture::{CaptureSnapshot, CaptureState};
use serde_json::{Value, json};
use std::collections::BTreeSet;

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
