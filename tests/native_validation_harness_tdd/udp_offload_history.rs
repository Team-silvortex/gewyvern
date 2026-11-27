use super::read_repo_file;
use super::udp_offload::{hash, packet};
use gewyvern::live_capture::{CaptureSnapshot, CaptureState};
use serde_json::{Value, json};
use std::collections::BTreeSet;

fn proof() -> Value {
    serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_offload_history_linux_x86_64_20261120.json",
    ))
    .unwrap()
}

fn data(block: usize, iteration: usize, family: usize, part: usize, length: usize) -> Vec<u8> {
    let mut data: Vec<u8> = (0..length)
        .map(|offset| (((block + iteration) * 19 + offset * 31) % 251) as u8)
        .collect();
    data[..2].copy_from_slice(&(block as u16).to_be_bytes());
    data[2..4].copy_from_slice(&(iteration as u16).to_be_bytes());
    data[4] = family as u8;
    data[5] = part as u8;
    data[6..8].copy_from_slice(&(length as u16).to_be_bytes());
    data
}

fn snapshot(value: &Value, count: u64, terminal: bool) -> CaptureSnapshot {
    let s: CaptureSnapshot = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(s.kernel.matched, count);
    assert_eq!(s.received, count);
    assert_eq!(s.retained, count);
    assert!(s.reconciles() && !s.incomplete && !s.has_loss());
    assert_eq!(s.final_accounted, terminal);
    assert_eq!(
        s.state,
        if terminal {
            CaptureState::Stopped
        } else {
            CaptureState::Running
        }
    );
    s
}

fn verify(proof: &Value) {
    assert_eq!(proof["release_artifact"], false);
    assert_eq!(proof["status_maturity_promoted"], false);
    let internal = &proof["internal_history_regression"];
    assert_eq!(internal["groups"], 12289);
    assert_eq!(internal["retired_pending_ids"], json!([1, 4097, 8193]));
    assert_eq!(internal["history_max_ids"], 4096);
    assert_eq!(internal["late_tails_rejected"], 3);
    assert_eq!(internal["received_partial_rejected"], 3);
    for flag in ["fixed_time", "reservations_released", "deadlines_removed"] {
        assert_eq!(internal[flag], true);
    }
    let run = &proof["run"];
    assert_eq!(run["binary_sha256"], proof["binary_sha256"]);
    for flag in [
        "ok",
        "production_cli",
        "packet_multiset_matches",
        "window_replay_identical",
        "unique_fact_ids",
        "contiguous_group_ids",
        "same_sockets_all_blocks",
        "post_rollover_16_segment_recovery",
        "hooks_detached",
        "owned_resources_removed",
    ] {
        assert_eq!(run[flag], true);
    }
    assert_eq!(run["host_network_changed"], false);
    for (key, count) in [
        ("history_id_window", 4096),
        ("history_groups", 10244),
        ("facts", 51268),
        ("publication_blocks", 129),
        ("native_requests", 20512),
        ("native_replies", 5122),
    ] {
        assert_eq!(run[key], count);
    }
    let terminal = snapshot(&run["terminal"], 51268, true);
    let windows = run["windows"].as_array().unwrap();
    assert_eq!(windows.len(), 129);
    let native = run["native"].as_array().unwrap();
    assert_eq!(native.len(), 2);
    assert_ne!(
        native[0]["identity"]["namespace"],
        native[1]["identity"]["namespace"]
    );
    let mut cookies = BTreeSet::new();
    for (peer, role) in native.iter().zip(["client", "server"]) {
        assert_eq!(peer["same_sockets_all_blocks"], true);
        assert_eq!(peer["identity"]["role"], role);
        assert!(peer["identity"]["ifindex"].as_u64().unwrap() > 0);
        let ids = peer["identity"]["cookies"].as_array().unwrap();
        assert_eq!(ids.len(), 2);
        for cookie in ids {
            let cookie = cookie.as_str().unwrap().parse::<u64>().unwrap();
            assert!(cookie > 0 && cookies.insert(cookie));
        }
        for key in [
            "IpFragCreates",
            "IpReasmOKs",
            "Ip6FragCreates",
            "Ip6ReasmOKs",
        ] {
            assert_eq!(peer["fragment_counter_deltas"][key], 0);
        }
        assert_eq!(peer["blocks"].as_array().unwrap().len(), 129);
    }
    for (block, window) in windows.iter().enumerate() {
        let (iterations, count) = if block == 128 { (1, 16) } else { (20, 4) };
        assert_eq!(window["block"], block);
        assert_eq!(window["facts"], iterations * (4 * count + 4));
        assert_eq!(window["groups"], iterations * 4);
        assert_eq!(window["first_group"], (block * 80 + 1).to_string());
        assert_eq!(
            window["last_group"],
            (block * 80 + iterations * 4).to_string()
        );
        assert_eq!(window["expected_sha256"].as_array().unwrap().len(), 2);
        for (peer_index, peer) in native.iter().enumerate() {
            let server = peer_index == 1;
            let mut requests = Vec::new();
            let mut replies = Vec::new();
            let mut expected = Vec::new();
            for iteration in 0..iterations {
                for family in 0..2 {
                    let cookie = &peer["identity"]["cookies"][family];
                    for part in 0..count {
                        let part = data(
                            block,
                            iteration,
                            family,
                            part,
                            if part + 1 == count { 17 } else { 1024 },
                        );
                        requests.extend_from_slice(&part);
                        expected.push(packet(server, family, cookie, !server, &part));
                    }
                    let reply = data(block, iteration, family, 255, 32);
                    replies.extend_from_slice(&reply);
                    expected.push(packet(server, family, cookie, server, &reply));
                }
            }
            let row = &peer["blocks"][block];
            assert_eq!(row["block"], block);
            assert_eq!(row["iterations"], iterations);
            assert_eq!(row["segment_count"], count);
            assert_eq!(row["logical_requests"], iterations * 2 * count);
            assert_eq!(row["native_replies"], iterations * 2);
            assert_eq!(row["request_sha256"], hash(&requests));
            assert_eq!(row["reply_sha256"], hash(&replies));
            assert_eq!(row["gro_buffers"], if server { iterations * 2 } else { 0 });
            assert_eq!(
                row["gro_segment"],
                if server { json!(1024) } else { Value::Null }
            );
            assert_eq!(row["extra_datagram_observed"], false);
            assert_eq!(
                window["expected_sha256"][peer_index],
                hash(&serde_json::to_vec(&expected).unwrap())
            );
        }
    }
    let checkpoints = run["checkpoints"].as_array().unwrap();
    assert_eq!(checkpoints.len(), 9);
    for (row, block) in checkpoints
        .iter()
        .zip([0, 31, 50, 51, 64, 101, 102, 127, 128])
    {
        assert_eq!(row["block"], block);
        let count = if block == 128 {
            51268
        } else {
            (block + 1) * 400
        };
        assert_eq!(
            snapshot(&row["capture"], count, false).run_id,
            terminal.run_id
        );
    }
    let resources = run["resources"].as_array().unwrap();
    assert_eq!(resources.len(), 129);
    let mut elapsed = 0.0;
    let mut user = 0;
    let mut system = 0;
    let mut hwm = 0;
    for (block, row) in resources.iter().enumerate() {
        assert_eq!(row["block"], block);
        assert!(row["elapsed_ms"].as_f64().unwrap() > elapsed);
        assert!(row["cpu_user_ticks"].as_u64().unwrap() >= user);
        assert!(row["cpu_system_ticks"].as_u64().unwrap() >= system);
        assert!(row["hwm_kib"].as_u64().unwrap() >= hwm);
        let rss = row["rss_kib"].as_u64().unwrap();
        assert!(rss > 0 && rss < 512 * 1024 && rss <= row["hwm_kib"].as_u64().unwrap());
        assert_eq!(
            row["clock_ticks_per_second"],
            resources[0]["clock_ticks_per_second"]
        );
        assert!(row["clock_ticks_per_second"].as_u64().unwrap() > 0);
        elapsed = row["elapsed_ms"].as_f64().unwrap();
        user = row["cpu_user_ticks"].as_u64().unwrap();
        system = row["cpu_system_ticks"].as_u64().unwrap();
        hwm = row["hwm_kib"].as_u64().unwrap();
    }
    let repetitions = proof["repetitions"].as_array().unwrap();
    assert_eq!(repetitions.len(), 3);
    let mut ids = BTreeSet::new();
    for (index, repeat) in repetitions.iter().enumerate() {
        assert_eq!(repeat["iteration"], index + 1);
        let s = snapshot(&repeat["terminal"], 51268, true);
        if index == 0 {
            assert_eq!(s.run_id, terminal.run_id);
        }
        assert!(ids.insert(s.run_id));
        let digest = repeat["summary_sha256"].as_str().unwrap();
        assert!(digest.len() == 64 && digest.bytes().all(|c| c.is_ascii_hexdigit()));
        let summary = &repeat["resource_summary"];
        let low = summary["rss_min_kib"].as_u64().unwrap();
        let high = summary["rss_max_kib"].as_u64().unwrap();
        assert!(low > 0 && low <= high && high < 512 * 1024);
        assert!(summary["hwm_max_kib"].as_u64().unwrap() >= high);
        assert!(summary["last_elapsed_ms"].as_f64().unwrap() > 0.0);
        assert!(summary["clock_ticks_per_second"].as_u64().unwrap() > 0);
        if index == 0 {
            assert_eq!(
                low,
                resources
                    .iter()
                    .map(|r| r["rss_kib"].as_u64().unwrap())
                    .min()
                    .unwrap()
            );
            assert_eq!(
                high,
                resources
                    .iter()
                    .map(|r| r["rss_kib"].as_u64().unwrap())
                    .max()
                    .unwrap()
            );
            assert_eq!(summary["hwm_max_kib"], hwm);
            assert_eq!(summary["last_elapsed_ms"], elapsed);
            assert_eq!(summary["cpu_user_ticks"], user);
            assert_eq!(summary["cpu_system_ticks"], system);
        }
    }
}

#[test]
fn product_history_proof_reconciles_every_native_block_and_rollover() {
    verify(&proof());
}

#[test]
fn product_history_proof_rejects_gaps_duplicates_and_fabricated_completion() {
    let original = proof();
    for case in 0..6 {
        let mut bad = original.clone();
        match case {
            0 => {
                bad["run"]["windows"].as_array_mut().unwrap().remove(51);
            }
            1 => bad["run"]["windows"][51]["first_group"] = json!("4001"),
            2 => bad["run"]["checkpoints"][3]["capture"]["final_accounted"] = json!(true),
            3 => bad["run"]["native"][1]["blocks"][102]["request_sha256"] = json!("0".repeat(64)),
            4 => bad["run"]["windows"][128]["facts"] = json!(64),
            5 => bad["run"]["terminal"]["decode_rejected"] = json!("1"),
            _ => unreachable!(),
        }
        assert!(
            std::panic::catch_unwind(|| verify(&bad)).is_err(),
            "case {case}"
        );
    }
}
