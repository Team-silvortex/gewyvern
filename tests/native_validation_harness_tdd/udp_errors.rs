use super::read_repo_file;
use gewyvern::live_capture::{CaptureSnapshot, CaptureState};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn proof() -> Value {
    serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_errors_linux_x86_64_20261120.json",
    ))
    .unwrap()
}

fn hash(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn verify(proof: &Value) {
    assert_eq!(proof["release_artifact"], false);
    assert_eq!(proof["status_maturity_promoted"], false);
    let runs = proof["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 3);
    let mut ids = BTreeSet::new();
    for run in runs {
        for flag in [
            "ok",
            "production_cli",
            "same_client_socket_recovery",
            "server_socket_replacement",
            "packet_multiset_matches",
            "replay_identical",
            "hooks_detached",
            "owned_resources_removed",
        ] {
            assert_eq!(run[flag], true);
        }
        for flag in [
            "validation_capture_used",
            "host_network_changed",
            "product_icmp_decoder_claim",
        ] {
            assert_eq!(run[flag], false);
        }
        for (field, count) in [
            ("send_attempts", 10),
            ("local_mtu_rejections", 2),
            ("remote_port_refusals", 2),
            ("successful_exchanges", 6),
            ("error_queue_entries", 4),
            ("facts", 26),
        ] {
            assert_eq!(run[field], count);
        }
        let status: CaptureSnapshot = serde_json::from_value(run["capture"].clone()).unwrap();
        assert!(
            status.final_accounted
                && status.reconciles()
                && !status.incomplete
                && !status.has_loss()
        );
        assert_eq!(status.state, CaptureState::Stopped);
        assert_eq!(status.retained, 26);
        assert!(ids.insert(status.run_id));
        let oracles = run["oracles"].as_array().unwrap();
        assert_eq!(oracles.len(), 2);
        assert_ne!(oracles[0]["namespace"], oracles[1]["namespace"]);
        let mut all_cookies = BTreeSet::new();
        for (oracle, server) in oracles.iter().zip([false, true]) {
            assert_eq!(oracle["role"], if server { "server" } else { "client" });
            assert_eq!(oracle["same_sockets_all_phases"], !server);
            let cookies = oracle["cookies"].as_array().unwrap();
            assert_eq!(cookies.len(), if server { 4 } else { 2 });
            for cookie in cookies {
                let cookie: u64 = cookie.as_str().unwrap().parse().unwrap();
                assert_ne!(cookie, 0);
                assert!(all_cookies.insert(cookie));
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
            assert_eq!(phases.len(), 5);
            let mut expected = Vec::new();
            for (index, (phase, name)) in phases
                .iter()
                .zip([
                    "baseline",
                    "mtu-rejection",
                    "mtu-recovery",
                    "port-unreachable",
                    "port-recovery",
                ])
                .enumerate()
            {
                assert_eq!(phase["phase"], name);
                assert_eq!(phase["server_ready_before_client"], true);
                let closed = server && name == "port-unreachable";
                assert_eq!(phase["open_sockets"], if closed { 0 } else { 2 });
                let rows = phase["observations"].as_array().unwrap();
                assert_eq!(rows.len(), if closed { 0 } else { 2 });
                for (family, row) in rows.iter().enumerate() {
                    let mtu = name == "mtu-rejection";
                    let refused = name == "port-unreachable";
                    let length = if mtu { 4096 } else { 32 };
                    let data: Vec<u8> = (0..length)
                        .map(|at| ((index * 19 + at * 31) % 251) as u8)
                        .collect();
                    assert_eq!(row["family"], if family == 0 { 4 } else { 6 });
                    assert_eq!(row["length"], length);
                    assert_eq!(row["payload_sha256"], hash(&data));
                    assert_eq!(
                        row["cookie"],
                        cookies[family + if server && index == 4 { 2 } else { 0 }]
                    );
                    assert_eq!(row["sent"], !mtu);
                    assert_eq!(row["received"], !mtu && !refused);
                    if !server && (mtu || refused) {
                        // Fixed Linux x86_64 evidence values, not this test host's errno ABI.
                        let errno = if mtu { 90 } else { 111 };
                        assert_eq!(row[if mtu { "send_errno" } else { "receive_errno" }], errno);
                        let queue = &row["error_queue"];
                        let error = &queue["extended_error"];
                        assert_eq!(error["errno"], errno);
                        assert_eq!(
                            error["origin"],
                            if mtu {
                                1
                            } else if family == 0 {
                                2
                            } else {
                                3
                            }
                        );
                        assert_eq!(
                            error["icmp_type"],
                            if mtu {
                                0
                            } else if family == 0 {
                                3
                            } else {
                                1
                            }
                        );
                        assert_eq!(
                            error["icmp_code"],
                            if mtu {
                                0
                            } else if family == 0 {
                                3
                            } else {
                                4
                            }
                        );
                        assert_eq!(error["info"], if mtu { 1500 } else { 0 });
                        assert_eq!(queue["pending_socket_error_cleared"], true);
                        assert_eq!(queue["quoted_length"], if mtu { 0 } else { 32 });
                        assert_eq!(
                            queue["quoted_payload_sha256"],
                            hash(if mtu { &[] } else { &data })
                        );
                        assert_eq!(
                            queue["destination"],
                            if family == 0 {
                                "192.0.2.2:41002"
                            } else {
                                "[fd00:6765:7779::2]:41002"
                            }
                        );
                    } else {
                        assert!(row["error_queue"].is_null());
                    }
                    if server && mtu {
                        assert_eq!(row["timeout"], true);
                    }
                    for (outbound, present) in [(true, !mtu), (false, !mtu && !refused)] {
                        if present {
                            expected.push(json!({"cookie":row["cookie"],"family":if family==0 {0x800} else {0x86dd},
                                "local_port":if server {41002} else {41001},"remote_port":if server {41001} else {41002},
                                "outbound":outbound,"total_len":length+if family==0 {28} else {48},"prefix":data}));
                        }
                    }
                }
            }
            let mut actual: Vec<_> = oracle["expected"]
                .as_array()
                .unwrap()
                .iter()
                .map(Value::to_string)
                .collect();
            let mut expected: Vec<_> = expected.iter().map(Value::to_string).collect();
            actual.sort();
            expected.sort();
            assert_eq!(
                actual, expected,
                "packet oracle must match each native phase"
            );
        }
    }
    for (field, count) in [
        ("accepted_runs", 3),
        ("send_attempts", 30),
        ("local_mtu_rejections", 6),
        ("remote_port_refusals", 6),
        ("successful_exchanges", 18),
        ("error_queue_entries", 12),
        ("retained_packet_facts", 78),
        ("recorded_collector_loss", 0),
    ] {
        assert_eq!(proof["results"][field], count);
    }
}

#[test]
fn native_udp_errors_do_not_invent_delivery_or_collector_loss() {
    verify(&proof());
}

#[test]
fn error_proof_rejects_fabricated_delivery_or_stale_socket_identity() {
    for mutation in 0..6 {
        let mut bad = proof();
        match mutation {
            0 => bad["runs"][0]["oracles"][0]["phases"][1]["observations"][0]["sent"] = json!(true),
            1 => {
                bad["runs"][0]["oracles"][0]["phases"][3]["observations"][1]["error_queue"]["extended_error"]
                    ["origin"] = json!(1)
            }
            2 => {
                bad["runs"][0]["oracles"][1]["cookies"][2] =
                    bad["runs"][0]["oracles"][1]["cookies"][0].clone()
            }
            3 => {
                bad["runs"][0]["oracles"][0]["expected"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            4 => bad["runs"][0]["capture"]["incomplete"] = json!(true),
            _ => {
                bad["runs"][0]["oracles"][1]["phases"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
        }
        assert!(std::panic::catch_unwind(|| verify(&bad)).is_err());
    }
}
