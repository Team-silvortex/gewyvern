use super::read_repo_file;
use super::udp_offload::{hash, packet, parts_with_count, payload, verify_row_with_count};
use gewyvern::live_capture::{CaptureSnapshot, CaptureState};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn proof() -> Value {
    serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_normalization_linux_x86_64_20261120.json",
    ))
    .unwrap()
}

fn verify(proof: &Value) {
    assert_eq!(proof["private_event_abi"], 2);
    assert_eq!(proof["release_artifact"], false);
    assert_eq!(proof["status_maturity_promoted"], false);
    let runs = proof["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 7);
    let mut cases = BTreeSet::new();
    let mut runs_seen = BTreeSet::new();
    for run in runs {
        let count = run["segment_count"].as_u64().unwrap() as usize;
        let gro = run["gro_enabled"].as_bool().unwrap();
        let fault = run["fault"].as_str().unwrap();
        assert!([4, 16, 17].contains(&count));
        assert!(["none", "ring"].contains(&fault));
        let ring = fault == "ring";
        if ring {
            assert!(gro && count == 16);
        }
        assert!(cases.insert((count, gro, fault)));
        for flag in [
            "ok",
            "production_cli",
            "same_socket_recovery",
            "packet_multiset_matches",
            "replay_identical",
            "hooks_detached",
            "owned_resources_removed",
        ] {
            assert_eq!(run[flag], true);
        }
        assert_eq!(run["validation_capture_used"], false);
        assert_eq!(run["host_network_changed"], false);
        assert_eq!(run["product_offload_normalization_claim"], count <= 16);
        assert_eq!(run["binary_sha256"], proof["binary_sha256"]);
        assert_eq!(run["native_requests"], 4 * count + 4);
        assert_eq!(run["native_replies"], 8);
        assert_eq!(run["selected_offload_datagrams"], 2 * count);
        assert_eq!(run["unselected_offload_datagrams"], 2 * count);
        let s: CaptureSnapshot = serde_json::from_value(run["capture"].clone()).unwrap();
        assert!(s.final_accounted && s.reconciles());
        assert!(runs_seen.insert(s.run_id.clone()));
        assert_eq!(s.state, CaptureState::Stopped);
        let incomplete = ring || count > 16;
        assert_eq!(s.incomplete, incomplete);
        assert_eq!(s.has_loss(), incomplete);
        assert_eq!(run["capture_exit"], if incomplete { 3 } else { 0 });
        assert_eq!(
            s.kernel.unsupported,
            if count <= 16 {
                0
            } else if gro {
                4
            } else {
                2
            }
        );
        assert_eq!(
            s.kernel.malformed + s.kernel.no_cookie + s.userspace_dropped,
            0
        );
        assert_eq!(s.kernel.ring_dropped, if ring { 59 } else { 0 });
        assert_eq!(s.decode_rejected, if ring { 9 } else { 0 });
        let retained = if ring {
            16
        } else if count <= 16 {
            20 + 4 * count
        } else if gro {
            20
        } else {
            20 + 2 * count
        };
        assert_eq!(s.retained, retained as u64);
        assert_eq!(run["facts"], retained);
        let groups = run["normalized_groups"].as_array().unwrap();
        assert_eq!(
            groups.len(),
            if incomplete {
                0
            } else if gro {
                4
            } else {
                2
            }
        );
        let mut ids = BTreeSet::new();
        for group in groups {
            let id = group["aggregate_id"]
                .as_str()
                .unwrap()
                .parse::<u64>()
                .unwrap();
            assert!(id > 0 && ids.insert(id));
            assert_eq!(group["segment_count"], count);
            assert_eq!(group["indexes"], json!((0..count).collect::<Vec<_>>()));
            assert_eq!(group["identity"].as_array().unwrap().len(), 1);
        }
        let oracles = run["oracles"].as_array().unwrap();
        assert_eq!(oracles.len(), 2);
        assert_ne!(oracles[0]["namespace"], oracles[1]["namespace"]);
        let mut cookies = BTreeSet::new();
        for (oracle, server) in oracles.iter().zip([false, true]) {
            assert_eq!(oracle["role"], if server { "server" } else { "client" });
            assert_eq!(oracle["same_sockets_all_phases"], true);
            for field in ["cookies", "excluded_cookies"] {
                let entries = oracle[field].as_array().unwrap();
                assert_eq!(entries.len(), 2);
                for value in entries {
                    let cookie: u64 = value.as_str().unwrap().parse().unwrap();
                    assert!(cookie != 0 && cookies.insert(cookie));
                }
            }
            for field in [
                "IpFragCreates",
                "IpReasmOKs",
                "Ip6FragCreates",
                "Ip6ReasmOKs",
            ] {
                assert_eq!(oracle["fragment_counter_deltas"][field], 0);
            }
            let phases = oracle["phases"].as_array().unwrap();
            assert_eq!(phases.len(), 4);
            let mut expected = Vec::new();
            for (phase, (rows, name)) in phases
                .iter()
                .zip(["baseline", "offload", "unselected-offload", "recovery"])
                .enumerate()
            {
                assert_eq!(rows["phase"], name);
                let rows = rows["observations"].as_array().unwrap();
                assert_eq!(rows.len(), 2);
                for (family, row) in rows.iter().enumerate() {
                    verify_row_with_count(row, phase, family, server, gro, count);
                    if phase == 2 || (ring && phase == 1) {
                        continue;
                    }
                    let cookie = &oracle["cookies"][family];
                    if phase != 1 || count <= 16 || (server && !gro) {
                        for part in parts_with_count(phase, family, count) {
                            expected.push(packet(server, family, cookie, !server, &part));
                        }
                    }
                    expected.push(packet(
                        server,
                        family,
                        cookie,
                        server,
                        &payload(phase, family, 0, 32),
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
    for count in [4, 16, 17] {
        for gro in [false, true] {
            assert!(cases.contains(&(count, gro, "none")));
        }
    }
    assert!(cases.contains(&(16, true, "ring")));
}

#[test]
fn product_udp_normalization_proof_reconciles_native_segments_and_partial_loss() {
    verify(&proof());
}

#[test]
fn product_udp_normalization_proof_rejects_false_gro_groups_and_loss() {
    let original = proof();
    for case in 0..5 {
        let mut bad = original.clone();
        match case {
            0 => bad["runs"][0]["normalized_groups"][0]["indexes"] = json!([0, 1, 2]),
            1 => {
                bad["runs"][1]["oracles"][1]["phases"][1]["observations"][0]["receives"][0]["gro_segment"] =
                    Value::Null
            }
            2 => bad["runs"][6]["capture"]["decode_rejected"] = json!("0"),
            3 => bad["runs"][4]["product_offload_normalization_claim"] = json!(true),
            _ => {
                bad["runs"][0]["oracles"][1]["phases"][1]["observations"][0]["receives"][3]["length"] =
                    json!(1024)
            }
        }
        assert!(
            std::panic::catch_unwind(|| verify(&bad)).is_err(),
            "case {case}"
        );
    }
}
