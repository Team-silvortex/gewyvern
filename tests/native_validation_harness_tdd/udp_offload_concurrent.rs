use super::read_repo_file;
use super::udp_offload::{hash, packet};
use gewyvern::live_capture::{CaptureSnapshot, CaptureState};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn proof() -> Value {
    serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_offload_concurrent_linux_x86_64_20261120.json",
    ))
    .unwrap()
}

pub(super) fn data(
    block: usize,
    lane: usize,
    iteration: usize,
    family: usize,
    part: usize,
    len: usize,
) -> Vec<u8> {
    let mut bytes: Vec<u8> = (0..len)
        .map(|at| (((block + iteration) * 19 + at * 31) % 251) as u8)
        .collect();
    bytes[..2].copy_from_slice(&(block as u16).to_be_bytes());
    bytes[2..4].copy_from_slice(&(iteration as u16).to_be_bytes());
    bytes[4] = family as u8;
    bytes[5] = part as u8;
    bytes[6..8].copy_from_slice(&(len as u16).to_be_bytes());
    bytes[8] = lane as u8;
    bytes
}

fn snapshot(
    value: &Value,
    matched: u64,
    received: u64,
    retained: u64,
    terminal: bool,
) -> CaptureSnapshot {
    let s: CaptureSnapshot = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        (s.kernel.matched, s.received, s.retained),
        (matched, received, retained)
    );
    assert!(!s.incomplete && !s.has_loss());
    assert_eq!(s.final_accounted, terminal);
    assert_eq!(
        s.state,
        if terminal {
            CaptureState::Stopped
        } else {
            CaptureState::Running
        }
    );
    assert_eq!(s.reconciles(), matched == received && received == retained);
    s
}

fn verify(proof: &Value) {
    assert_eq!(proof["release_artifact"], false);
    assert_eq!(proof["status_maturity_promoted"], false);
    let run = &proof["run"];
    assert_eq!(run["binary_sha256"], proof["binary_sha256"]);
    for flag in [
        "ok",
        "production_cli",
        "packet_multiset_matches",
        "replay_identical",
        "same_sockets_recovery",
        "hooks_detached",
        "owned_resources_removed",
    ] {
        assert_eq!(run[flag], true);
    }
    assert_eq!(run["host_network_changed"], false);
    assert_eq!(run["window_ms"], 600);
    assert_eq!(run["freeze_before_replay"], true);
    for (key, n) in [
        ("facts", 8784),
        ("groups", 528),
        ("lanes", 4),
        ("native_requests", 4128),
        ("native_replies", 264),
    ] {
        assert_eq!(run[key], n);
    }
    let cpus: Vec<u64> = serde_json::from_value(run["cpus"].clone()).unwrap();
    assert_eq!(cpus.len(), 8);
    assert_eq!(cpus.iter().collect::<BTreeSet<_>>().len(), 8);
    let terminal = snapshot(&run["terminal"], 8784, 8784, 8784, true);
    let peers = run["native"].as_array().unwrap();
    assert_eq!(peers.len(), 8);
    let mut cookies = BTreeSet::new();
    let mut namespaces = BTreeSet::new();
    for (index, peer) in peers.iter().enumerate() {
        let (lane, server) = (index / 2, index % 2 == 1);
        let identity = &peer["identity"];
        assert_eq!(identity["lane"], lane);
        assert_eq!(identity["role"], if server { "server" } else { "client" });
        assert_eq!(identity["cpu"], cpus[lane + if server { 4 } else { 0 }]);
        assert!(identity["ifindex"].as_u64().unwrap() > 0);
        assert!(namespaces.insert(identity["namespace"].as_str().unwrap()));
        assert_eq!(identity["cookies"].as_array().unwrap().len(), 2);
        for cookie in identity["cookies"].as_array().unwrap() {
            let id = cookie.as_str().unwrap().parse::<u64>().unwrap();
            assert!(id > 0 && cookies.insert(id));
        }
        assert_eq!(peer["same_sockets_all_blocks"], true);
        assert_eq!(peer["blocks"].as_array().unwrap().len(), 17);
        for key in [
            "IpFragCreates",
            "IpReasmOKs",
            "Ip6FragCreates",
            "Ip6ReasmOKs",
        ] {
            assert_eq!(peer["fragment_counter_deltas"][key], 0);
        }
    }
    let blocks = run["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 17);
    let mut all_ids = BTreeSet::new();
    let mut total = 0u64;
    let mut all_interleaved = 0;
    let mut pending_windows = 0;
    for (block, b) in blocks.iter().enumerate() {
        let (iterations, count) = if block == 16 { (1, 4) } else { (2, 16) };
        let previous = total;
        let facts = 4 * iterations * (4 * count + 4);
        total += facts as u64;
        assert_eq!(b["block"], block);
        assert_eq!(b["facts"], facts);
        assert_eq!(b["expected_sha256"].as_array().unwrap().len(), 8);
        let mut starts = Vec::new();
        let mut ends = Vec::new();
        for (index, peer) in peers.iter().enumerate() {
            let (lane, server) = (index / 2, index % 2 == 1);
            let mut requests = Vec::new();
            let mut replies = Vec::new();
            let mut expected = Vec::new();
            for iteration in 0..iterations {
                for family in 0..2 {
                    let cookie = &peer["identity"]["cookies"][family];
                    for part in 0..count {
                        let bytes = data(
                            block,
                            lane,
                            iteration,
                            family,
                            part,
                            if part + 1 == count { 17 } else { 1024 },
                        );
                        requests.extend_from_slice(&bytes);
                        expected.push(packet(server, family, cookie, !server, &bytes));
                    }
                    let reply = data(block, lane, iteration, family, 255, 32);
                    replies.extend_from_slice(&reply);
                    expected.push(packet(server, family, cookie, server, &reply));
                }
            }
            let row = &peer["blocks"][block];
            assert_eq!(row["block"], block);
            assert_eq!(row["iterations"], iterations);
            assert_eq!(row["segments"], count);
            assert_eq!(row["request_sha256"], hash(&requests));
            assert_eq!(row["reply_sha256"], hash(&replies));
            assert_eq!(row["gro_buffers"], if server { iterations * 2 } else { 0 });
            assert_eq!(
                row["gro_segment"],
                if server { json!(1024) } else { Value::Null }
            );
            assert_eq!(row["extra_datagram_observed"], false);
            assert_eq!(
                b["expected_sha256"][index],
                hash(&serde_json::to_vec(&expected).unwrap())
            );
            let start = row["start_ns"].as_str().unwrap().parse::<u64>().unwrap();
            let end = row["end_ns"].as_str().unwrap().parse::<u64>().unwrap();
            assert!(start < end);
            if !server {
                starts.push(start);
                ends.push(end);
            }
        }
        assert!(starts.iter().max().unwrap() < ends.iter().min().unwrap());
        let windows = b["windows"].as_array().unwrap();
        assert_eq!(windows.len(), if block == 16 { 1 } else { 2 });
        let mut retained = previous;
        let mut window_counts = vec![0u64; windows.len()];
        let groups = b["groups"].as_array().unwrap();
        assert_eq!(groups.len(), 4 * iterations * 4);
        let mut group_ids = BTreeSet::new();
        let mut owners = BTreeMap::new();
        let mut arrivals = Vec::new();
        let mut interleaved = 0;
        let mut pending = 0;
        for g in groups {
            let id = g["id"].as_u64().unwrap();
            assert!(group_ids.insert(id));
            let lane = g["lane"].as_u64().unwrap() as usize;
            let family = g["family"].as_u64().unwrap();
            let server = g["server"].as_bool().unwrap();
            assert!(lane < 4 && family < 2);
            *owners.entry((lane, server, family)).or_insert(0) += 1;
            if !server {
                assert_eq!(g["cpu"], cpus[lane]);
            }
            let ids: Vec<u64> = serde_json::from_value(g["fact_ids"].clone()).unwrap();
            assert_eq!(ids.len(), count);
            assert!(ids.windows(2).all(|p| p[0] < p[1]));
            assert!(
                ids.iter()
                    .all(|id| *id > previous && *id <= total && all_ids.insert(*id))
            );
            arrivals.push((ids[0], id));
            if ids[count - 1] - ids[0] + 1 > count as u64 {
                interleaved += 1;
            }
            let window = usize::from(block < 16 && ids[count - 1] > previous + 512);
            assert_eq!(g["window"], window);
            window_counts[window] += count as u64;
            if window == 1 {
                pending += ids.iter().filter(|id| **id <= previous + 512).count() as u64;
            }
        }
        assert_eq!(owners.len(), 16);
        assert!(owners.values().all(|n| *n == iterations));
        assert_eq!(
            group_ids.into_iter().collect::<Vec<_>>(),
            (block as u64 * 32 + 1..block as u64 * 32 + 1 + (4 * iterations * 4) as u64)
                .collect::<Vec<_>>()
        );
        let ordinary: Vec<u64> = serde_json::from_value(b["ordinary_ids"].clone()).unwrap();
        assert_eq!(ordinary.len(), 4 * iterations * 4);
        for id in ordinary {
            assert!(id > previous && id <= total && all_ids.insert(id));
            window_counts[usize::from(block < 16 && id > previous + 512)] += 1;
        }
        arrivals.sort_unstable();
        let descents = arrivals.windows(2).filter(|p| p[0].1 > p[1].1).count();
        assert_eq!(b["first_arrival_id_descents"], descents);
        assert_eq!(b["interleaved_groups"], interleaved);
        all_interleaved += interleaved;
        for (index, w) in windows.iter().enumerate() {
            assert_eq!(w["facts"], window_counts[index]);
            retained += window_counts[index];
            let received = if block < 16 && index == 0 {
                previous + 512
            } else {
                total
            };
            let s = snapshot(&w["capture"], total, received, retained, false);
            assert_eq!(s.run_id, terminal.run_id);
            if block < 16 && index == 0 {
                assert_eq!(s.received - s.retained, pending);
                pending_windows += usize::from(pending > 0);
            }
        }
        assert_eq!(retained, total);
    }
    assert!(all_interleaved > 0 && pending_windows > 0);
    assert_eq!(
        all_ids.into_iter().collect::<Vec<_>>(),
        (1..=8784).collect::<Vec<_>>()
    );
    let repeats = proof["repetitions"].as_array().unwrap();
    assert_eq!(repeats.len(), 3);
    let mut run_ids = BTreeSet::new();
    for (index, repeat) in repeats.iter().enumerate() {
        assert_eq!(repeat["iteration"], index + 1);
        let s = snapshot(&repeat["terminal"], 8784, 8784, 8784, true);
        if index == 0 {
            assert_eq!(s.run_id, terminal.run_id);
        }
        assert!(run_ids.insert(s.run_id));
        assert!(repeat["interleaved_groups"].as_u64().unwrap() > 0);
        assert!(repeat["pending_windows"].as_u64().unwrap() > 0);
        let sha = repeat["summary_sha256"].as_str().unwrap();
        assert!(sha.len() == 64 && sha.bytes().all(|c| c.is_ascii_hexdigit()));
    }
}

#[test]
fn product_multi_cpu_proof_reconciles_native_io_group_interleaving_and_windows() {
    verify(&proof());
}

#[test]
fn product_multi_cpu_proof_rejects_duplicate_ids_split_groups_and_fabricated_recovery() {
    let original = proof();
    for case in 0..6 {
        let mut bad = original.clone();
        match case {
            0 => {
                bad["run"]["blocks"][0]["groups"][1]["fact_ids"][0] =
                    bad["run"]["blocks"][0]["groups"][0]["fact_ids"][0].clone()
            }
            1 => bad["run"]["blocks"][0]["groups"][0]["window"] = json!(1),
            2 => bad["run"]["blocks"][0]["windows"][0]["capture"]["final_accounted"] = json!(true),
            3 => bad["run"]["native"][1]["blocks"][16]["gro_segment"] = Value::Null,
            4 => bad["run"]["blocks"][16]["facts"] = json!(64),
            5 => bad["run"]["terminal"]["decode_rejected"] = json!("1"),
            _ => unreachable!(),
        }
        assert!(
            std::panic::catch_unwind(|| verify(&bad)).is_err(),
            "case {case}"
        );
    }
}
