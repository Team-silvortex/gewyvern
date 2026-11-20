use super::read_repo_file;
use gewyvern::live_capture::{CaptureSnapshot, CaptureState};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn proof() -> Value {
    serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_pressure_linux_x86_64_20261120.json",
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

fn message(phase: usize, family: usize, id: u8, length: usize) -> Vec<u8> {
    let mut bytes: Vec<u8> = (0..length)
        .map(|at| ((phase * 19 + at * 31) % 251) as u8)
        .collect();
    bytes[..3].copy_from_slice(&[phase as u8, family as u8, id]);
    bytes
}

fn packet(server: bool, family: usize, cookie: &Value, outbound: bool, bytes: &[u8]) -> Value {
    json!({"cookie":cookie,"family":if family==0 {0x800} else {0x86dd},
        "local_port":if server {41002} else {41001},"remote_port":if server {41001} else {41002},
        "outbound":outbound,"total_len":bytes.len()+if family==0 {28} else {48},
        "prefix":bytes[..bytes.len().min(128)]})
}

fn verify(proof: &Value) {
    assert_eq!(proof["release_artifact"], false);
    assert_eq!(proof["status_maturity_promoted"], false);
    let runs = proof["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 3);
    let mut run_ids = BTreeSet::new();
    let mut total_drops = 0;
    for run in runs {
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
        for flag in [
            "validation_capture_used",
            "host_network_changed",
            "product_socket_drop_classifier_claim",
        ] {
            assert_eq!(run[flag], false);
        }
        for (field, count) in [
            ("requests_sent", 136),
            ("request_replies", 8),
            ("application_truncations", 2),
            ("overflow_sent", 128),
            ("facts", 288),
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
        assert_eq!(status.retained, 288);
        assert!(run_ids.insert(status.run_id));
        let oracles = run["oracles"].as_array().unwrap();
        assert_eq!(oracles.len(), 2);
        assert_ne!(oracles[0]["namespace"], oracles[1]["namespace"]);
        let mut cookies = BTreeSet::new();
        let mut run_drops = 0;
        for (oracle, server) in oracles.iter().zip([false, true]) {
            assert_eq!(oracle["role"], if server { "server" } else { "client" });
            assert_eq!(oracle["same_sockets_all_phases"], true);
            assert_eq!(
                oracle["overflow_expected_basis"],
                "sender_bytes_and_native_rcvbuf_drop_accounting_not_application_delivery"
            );
            let own_cookies = oracle["cookies"].as_array().unwrap();
            assert_eq!(own_cookies.len(), 2);
            for cookie in own_cookies {
                let cookie: u64 = cookie.as_str().unwrap().parse().unwrap();
                assert_ne!(cookie, 0);
                assert!(cookies.insert(cookie));
            }
            let phases = oracle["phases"].as_array().unwrap();
            assert_eq!(phases.len(), 5);
            let mut expected = Vec::new();
            let mut prior_drops = [0_u64; 2];
            for (index, (phase, name)) in phases
                .iter()
                .zip([
                    "baseline",
                    "truncate",
                    "truncate-recovery",
                    "overflow",
                    "overflow-recovery",
                ])
                .enumerate()
            {
                let overflow = name == "overflow";
                assert_eq!(phase["phase"], name);
                assert_eq!(phase["receiver_paused_until_all_sends"], server && overflow);
                let rows = phase["observations"].as_array().unwrap();
                assert_eq!(rows.len(), 2);
                for (family, (row, prefix)) in rows.iter().zip(["Udp", "Udp6"]).enumerate() {
                    assert_eq!(row["family"], if family == 0 { 4 } else { 6 });
                    assert_eq!(row["cookie"], own_cookies[family]);
                    let request_length = if overflow || name == "truncate" {
                        1024
                    } else {
                        32
                    };
                    assert_eq!(row["request_length"], request_length);
                    assert!(row["receive_buffer_bytes"].as_u64().unwrap() > 0);
                    let sent: Vec<u8> = serde_json::from_value(row["sent_ids"].clone()).unwrap();
                    assert_eq!(
                        sent,
                        if !server && overflow {
                            (0..64).collect::<Vec<_>>()
                        } else if server && overflow {
                            vec![]
                        } else {
                            vec![0]
                        }
                    );
                    let receipts = row["receipts"].as_array().unwrap();
                    let stats = &phase["udp_counter_deltas"];
                    let drops = stats[format!("{prefix}RcvbufErrors")].as_u64().unwrap();
                    assert_eq!(stats[format!("{prefix}InDatagrams")], receipts.len());
                    assert_eq!(stats[format!("{prefix}OutDatagrams")], sent.len());
                    assert_eq!(stats[format!("{prefix}InErrors")], drops);
                    for key in ["InCsumErrors", "NoPorts", "SndbufErrors"] {
                        assert_eq!(stats[format!("{prefix}{key}")], 0);
                    }
                    if server && overflow {
                        assert!(!receipts.is_empty() && receipts.len() < 64 && drops > 0);
                        assert_eq!(receipts.len() as u64 + drops, 64);
                        run_drops += drops;
                        prior_drops[family] = drops;
                    } else {
                        assert_eq!(drops, 0);
                        assert_eq!(receipts.len(), if overflow { 0 } else { 1 });
                    }
                    let mut received_ids = BTreeSet::new();
                    for receipt in receipts {
                        let id = receipt["id"].as_u64().unwrap();
                        assert!(id < if overflow { 64 } else { 1 });
                        assert!(received_ids.insert(id));
                        let length = if server { request_length } else { 32 };
                        let truncated = server && name == "truncate";
                        let copied = if truncated { 64 } else { length };
                        let bytes = message(index, family, id as u8, length);
                        assert_eq!(receipt["wire_len"], length);
                        assert_eq!(receipt["copied_len"], copied);
                        assert_eq!(receipt["truncated"], truncated);
                        assert_eq!(receipt["copied_sha256"], hash(&bytes[..copied]));
                        if server && index == 4 {
                            assert_eq!(receipt["rxq_drops"], prior_drops[family]);
                        } else {
                            assert_eq!(receipt["rxq_drops"].as_u64().unwrap_or(0), 0);
                        }
                    }
                    // Reconstruct the exact compact-oracle digest, including pre-queue
                    // ingress for packets whose native application read never happened.
                    if !server {
                        for id in &sent {
                            expected.push(packet(
                                false,
                                family,
                                &row["cookie"],
                                true,
                                &message(index, family, *id, request_length),
                            ));
                        }
                    }
                    if overflow && server {
                        for id in 0..64 {
                            expected.push(packet(
                                true,
                                family,
                                &row["cookie"],
                                false,
                                &message(index, family, id, 1024),
                            ));
                        }
                    } else if !overflow {
                        expected.push(packet(
                            server,
                            family,
                            &row["cookie"],
                            false,
                            &message(index, family, 0, if server { request_length } else { 32 }),
                        ));
                        if server {
                            expected.push(packet(
                                true,
                                family,
                                &row["cookie"],
                                true,
                                &message(index, family, 0, 32),
                            ));
                        }
                    }
                }
            }
            assert_eq!(expected.len(), 144);
            assert_eq!(oracle["expected_count"], expected.len());
            assert_eq!(
                oracle["expected_sha256"],
                hash(&serde_json::to_vec(&expected).unwrap())
            );
        }
        assert_eq!(run["socket_queue_drops"], run_drops);
        assert_eq!(run["overflow_delivered"], 128 - run_drops);
        total_drops += run_drops;
    }
    for (field, count) in [
        ("accepted_runs", 3),
        ("requests_sent", 408),
        ("request_replies", 24),
        ("application_truncations", 6),
        ("overflow_sent", 384),
        ("retained_packet_facts", 864),
        ("recorded_collector_loss", 0),
    ] {
        assert_eq!(proof["results"][field], count);
    }
    assert_eq!(proof["results"]["socket_queue_drops"], total_drops);
    assert_eq!(proof["results"]["overflow_delivered"], 384 - total_drops);
}

#[test]
fn native_udp_pressure_keeps_capture_queue_and_application_accounting_separate() {
    verify(&proof());
}

#[test]
fn pressure_proof_rejects_false_delivery_truncation_and_recovery() {
    for mutation in 0..7 {
        let mut bad = proof();
        let run = &mut bad["runs"][0];
        match mutation {
            0 => {
                run["oracles"][1]["phases"][1]["observations"][0]["receipts"][0]["truncated"] =
                    json!(false)
            }
            1 => run["oracles"][1]["phases"][3]["udp_counter_deltas"]["UdpRcvbufErrors"] = json!(0),
            2 => {
                run["oracles"][1]["phases"][4]["observations"][1]["receipts"][0]["rxq_drops"] =
                    json!(0)
            }
            3 => run["oracles"][1]["expected_sha256"] = json!("not-observed"),
            4 => run["capture"]["incomplete"] = json!(true),
            5 => {
                let receipts = run["oracles"][1]["phases"][3]["observations"][0]["receipts"]
                    .as_array_mut()
                    .unwrap();
                receipts.push(receipts[0].clone());
            }
            _ => run["oracles"][1]["phases"][0]["udp_counter_deltas"]["UdpInDatagrams"] = json!(99),
        }
        assert!(std::panic::catch_unwind(|| verify(&bad)).is_err());
    }
}
