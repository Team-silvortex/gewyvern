use super::read_repo_file;
use gewyvern::live_capture::{CaptureSnapshot, CaptureState};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn proof() -> Value {
    serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_offload_linux_x86_64_20261120.json",
    ))
    .unwrap()
}
pub(super) fn hash(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
pub(super) fn payload(phase: usize, family: usize, id: usize, length: usize) -> Vec<u8> {
    let mut data: Vec<u8> = (0..length)
        .map(|at| ((phase * 19 + at * 31) % 251) as u8)
        .collect();
    data[..3].copy_from_slice(&[phase as u8, family as u8, id as u8]);
    data
}
fn parts(phase: usize, family: usize) -> Vec<Vec<u8>> {
    parts_with_count(phase, family, 4)
}
pub(super) fn parts_with_count(phase: usize, family: usize, count: usize) -> Vec<Vec<u8>> {
    let lengths = if phase == 1 || phase == 2 {
        let mut lengths = vec![1024; count];
        *lengths.last_mut().unwrap() = 17;
        lengths
    } else {
        vec![32]
    };
    lengths
        .into_iter()
        .enumerate()
        .map(|(id, length)| payload(phase, family, id, length))
        .collect()
}
fn verify_row(row: &Value, phase: usize, family: usize, server: bool, gro: bool) {
    verify_row_with_count(row, phase, family, server, gro, 4);
}
pub(super) fn verify_row_with_count(
    row: &Value,
    phase: usize,
    family: usize,
    server: bool,
    gro: bool,
    count: usize,
) {
    let parts = parts_with_count(phase, family, count);
    let request = parts.concat();
    let reply = payload(phase, family, 0, 32);
    assert_eq!(row["family"], if family == 0 { 4 } else { 6 });
    assert_eq!(
        row["request_lengths"],
        json!(parts.iter().map(Vec::len).collect::<Vec<_>>())
    );
    assert_eq!(row["request_sha256"], hash(&request));
    assert_eq!(row["reply_sha256"], hash(&reply));
    assert_eq!(row["send_calls"], 1);
    assert_eq!(row["send_bytes"], if server { 32 } else { request.len() });
    assert_eq!(row["extra_datagram_observed"], false);
    assert_eq!(row["selected_port"], phase != 2);
    let aggregated = server && gro && (phase == 1 || phase == 2);
    let received = if !server {
        vec![reply]
    } else if aggregated {
        vec![request]
    } else {
        parts
    };
    let rows = row["receives"].as_array().unwrap();
    assert_eq!(rows.len(), received.len());
    for (row, data) in rows.iter().zip(received) {
        assert_eq!(row["length"], data.len());
        assert_eq!(row["sha256"], hash(&data));
        assert_eq!(
            row["gro_segment"],
            if aggregated { json!(1024) } else { Value::Null }
        );
    }
}
pub(super) fn packet(
    server: bool,
    family: usize,
    cookie: &Value,
    outbound: bool,
    data: &[u8],
) -> Value {
    json!({"cookie":cookie,"family":if family==0 {0x800} else {0x86dd},
        "local_port":if server {41002} else {41001},"remote_port":if server {41001} else {41002},
        "outbound":outbound,"total_len":data.len()+if family==0 {28} else {48},"prefix":data[..data.len().min(128)]})
}
fn verify(proof: &Value) {
    assert_eq!(proof["release_artifact"], false);
    assert_eq!(proof["status_maturity_promoted"], false);
    let before = proof["before"].as_array().unwrap();
    assert_eq!(before.len(), 2);
    for (before, gro) in before.iter().zip([false, true]) {
        assert_eq!(before["gro_enabled"], gro);
        let diagnostic = &before["diagnostic"];
        assert_eq!(diagnostic["exit"], 0);
        let snapshot: CaptureSnapshot =
            serde_json::from_value(diagnostic["capture"].clone()).unwrap();
        assert!(
            snapshot.final_accounted
                && snapshot.reconciles()
                && !snapshot.incomplete
                && !snapshot.has_loss()
        );
        assert_eq!(snapshot.retained, if gro { 24 } else { 30 });
        let lengths = diagnostic["fact_ip_lengths"].as_array().unwrap();
        for length in [3117, 3137] {
            assert_eq!(
                lengths.iter().filter(|n| **n == length).count(),
                if gro { 2 } else { 1 }
            );
        }
        let phase = &before["native_server_offload"];
        assert_eq!(phase["phase"], "offload");
        let rows = phase["observations"].as_array().unwrap();
        assert_eq!(rows.len(), 2);
        for (family, row) in rows.iter().enumerate() {
            verify_row(row, 1, family, true, gro);
        }
    }
    let runs = proof["runs"].as_array().unwrap();
    assert_eq!(runs.len(), 6);
    let mut ids = BTreeSet::new();
    let mut modes = BTreeMap::new();
    for run in runs {
        let gro = run["gro_enabled"].as_bool().unwrap();
        *modes.entry(gro).or_insert(0) += 1;
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
            "product_offload_normalization_claim",
        ] {
            assert_eq!(run[flag], false);
        }
        assert_eq!(run["capture_exit"], 3);
        assert_eq!(run["facts"], if gro { 20 } else { 28 });
        for (field, n) in [
            ("native_requests", 20),
            ("native_replies", 8),
            ("selected_offload_datagrams", 8),
            ("unselected_offload_datagrams", 8),
        ] {
            assert_eq!(run[field], n);
        }
        let s: CaptureSnapshot = serde_json::from_value(run["capture"].clone()).unwrap();
        assert!(s.final_accounted && s.reconciles() && s.incomplete && s.has_loss());
        assert_eq!(s.state, CaptureState::Stopped);
        assert!(ids.insert(s.run_id));
        assert_eq!(s.kernel.unsupported, if gro { 4 } else { 2 });
        assert_eq!(s.retained, if gro { 20 } else { 28 });
        assert_eq!(
            s.kernel.malformed
                + s.kernel.no_cookie
                + s.kernel.ring_dropped
                + s.userspace_dropped
                + s.decode_rejected,
            0
        );
        let oracles = run["oracles"].as_array().unwrap();
        assert_eq!(oracles.len(), 2);
        assert_ne!(oracles[0]["namespace"], oracles[1]["namespace"]);
        let mut cookies = BTreeSet::new();
        for (oracle, server) in oracles.iter().zip([false, true]) {
            assert_eq!(oracle["role"], if server { "server" } else { "client" });
            assert_eq!(oracle["same_sockets_all_phases"], true);
            for field in ["cookies", "excluded_cookies"] {
                let items = oracle[field].as_array().unwrap();
                assert_eq!(items.len(), 2);
                for item in items {
                    let cookie: u64 = item.as_str().unwrap().parse().unwrap();
                    assert_ne!(cookie, 0);
                    assert!(cookies.insert(cookie));
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
            for (index, (phase, name)) in phases
                .iter()
                .zip(["baseline", "offload", "unselected-offload", "recovery"])
                .enumerate()
            {
                assert_eq!(phase["phase"], name);
                let rows = phase["observations"].as_array().unwrap();
                assert_eq!(rows.len(), 2);
                for (family, row) in rows.iter().enumerate() {
                    verify_row(row, index, family, server, gro);
                    assert_eq!(
                        row["cookie"],
                        oracle[if index == 2 {
                            "excluded_cookies"
                        } else {
                            "cookies"
                        }][family]
                    );
                    assert_eq!(
                        row["local_port"],
                        if server { 41002 } else { 41001 } + if index == 2 { 2 } else { 0 }
                    );
                    assert_eq!(
                        row["remote_port"],
                        if server { 41001 } else { 41002 } + if index == 2 { 2 } else { 0 }
                    );
                    if index == 2 {
                        continue;
                    }
                    let parts = parts(index, family);
                    let reply = payload(index, family, 0, 32);
                    if server {
                        if !(gro && index == 1) {
                            for part in &parts {
                                expected.push(packet(true, family, &row["cookie"], false, part));
                            }
                        }
                        expected.push(packet(true, family, &row["cookie"], true, &reply));
                    } else {
                        if index != 1 {
                            expected.push(packet(
                                false,
                                family,
                                &row["cookie"],
                                true,
                                &parts.concat(),
                            ));
                        }
                        expected.push(packet(false, family, &row["cookie"], false, &reply));
                    }
                }
            }
            assert_eq!(oracle["expected_count"], expected.len());
            assert_eq!(
                oracle["expected_sha256"],
                hash(&serde_json::to_vec(&expected).unwrap())
            );
        }
    }
    assert_eq!(modes, BTreeMap::from([(false, 3), (true, 3)]));
    for (field, n) in [
        ("accepted_runs", 6),
        ("native_requests", 120),
        ("native_replies", 48),
        ("selected_offload_datagrams", 48),
        ("unselected_offload_datagrams", 48),
        ("retained_packet_facts", 144),
        ("unsupported_aggregate_observations", 18),
        ("ring_or_userspace_or_decode_drops", 0),
    ] {
        assert_eq!(proof["results"][field], n);
    }
}

#[test]
fn native_offload_proof_rejects_false_complete_single_datagrams() {
    verify(&proof());
}

#[test]
fn offload_proof_rejects_missing_gro_false_completeness_and_foreign_port() {
    for mutation in 0..6 {
        let mut bad = proof();
        match mutation {
            0 => bad["runs"][0]["capture"]["incomplete"] = json!(false),
            1 => bad["runs"][0]["capture"]["kernel"]["unsupported"] = json!("0"),
            2 => {
                bad["runs"][3]["oracles"][1]["phases"][1]["observations"][0]["receives"][0]["gro_segment"] =
                    Value::Null
            }
            3 => {
                bad["runs"][0]["oracles"][0]["phases"][2]["observations"][0]["selected_port"] =
                    json!(true)
            }
            4 => bad["runs"][0]["oracles"][1]["expected_sha256"] = json!("fabricated"),
            _ => bad["before"][0]["diagnostic"]["exit"] = json!(3),
        }
        assert!(std::panic::catch_unwind(|| verify(&bad)).is_err());
    }
}

#[test]
fn production_bpf_offload_boundary_precedes_admission_but_follows_port_filter() {
    let source = read_repo_file("ebpf/production/udp.bpf.c");
    let filter = source
        .find("if (be16(udp) != UDP_PORT && be16(udp + 2) != UDP_PORT) return 1;")
        .unwrap();
    let reject = source
        .find("if (!udp_l4_offload(ctx)) goto unsupported;")
        .unwrap();
    let admit = source
        .find("__sync_fetch_and_add(&stats->matched, segments);")
        .unwrap();
    assert!(filter < reject && reject < admit);
    assert!(source.contains("segments > MAX_SEGMENTS"));
    assert!(source.contains("info->gso_type == (1U << 17)"));
    assert!(source.contains("__sync_fetch_and_add(&stats->ring_dropped, segments - index)"));
}
