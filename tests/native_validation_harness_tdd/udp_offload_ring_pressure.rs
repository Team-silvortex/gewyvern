use super::read_repo_file;
use super::udp_offload::{hash, packet};
use super::udp_offload_concurrent::data;
use gewyvern::live_capture::{CaptureSnapshot, CaptureState};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet};

fn proof() -> Value {
    serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_multicpu_pressure_linux_x86_64_20261120.json",
    ))
    .unwrap()
}

fn shape(phase: usize, lane: usize, stride: usize) -> (usize, usize) {
    assert!(phase < 3 * stride && lane < 4);
    match phase % stride {
        0 => (usize::from(lane == 0), 1),
        1 => (2, 16),
        2 if stride == 7 => (usize::from(lane == phase / stride), 16),
        step => (usize::from(lane == step - (stride - 4)), 2),
    }
}

fn snapshot(value: &Value, counts: [u64; 6], terminal: bool) -> CaptureSnapshot {
    let s: CaptureSnapshot = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(
        [
            s.kernel.matched,
            s.kernel.ring_dropped,
            s.received,
            s.retained,
            s.decode_rejected,
            s.userspace_dropped,
        ],
        counts
    );
    assert_eq!(
        s.kernel.malformed + s.kernel.unsupported + s.kernel.no_cookie,
        0
    );
    assert!(s.incomplete && s.has_loss());
    assert_eq!(s.final_accounted, terminal);
    assert_eq!(
        s.state,
        if terminal {
            CaptureState::Stopped
        } else {
            CaptureState::Running
        }
    );
    assert_eq!(
        s.reconciles(),
        s.received == s.retained + s.decode_rejected + s.userspace_dropped
    );
    assert_eq!(s.kernel.matched, s.received + s.kernel.ring_dropped);
    s
}

fn verify(proof: &Value, mixed: bool) {
    let extra = usize::from(mixed);
    let stride = 6 + extra;
    let (matched, lost, received) = if mixed {
        (668, 586, 82)
    } else {
        (600, 535, 65)
    };
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
        "idle_expiry_without_traffic",
        "no_partial_groups_published",
        "hooks_detached",
        "owned_resources_removed",
    ] {
        assert_eq!(run[flag], true);
    }
    assert_eq!(run["host_network_changed"], false);
    for (key, n) in [
        ("cycles", 3),
        ("ring_kib", 4),
        ("ring_records", 17),
        ("max_facts", if mixed { 24 } else { 4096 }),
        ("window_ms", 600),
        ("native_requests", 822 + 96 * extra),
        ("native_replies", 78 + 6 * extra),
        ("capture_exit", 3),
    ] {
        assert_eq!(run[key], n);
    }
    let cpus: Vec<u64> = serde_json::from_value(run["cpus"].clone()).unwrap();
    assert_eq!(cpus.len(), 8);
    assert_eq!(cpus.iter().collect::<BTreeSet<_>>().len(), 8);
    let peers = run["native"].as_array().unwrap();
    assert_eq!(peers.len(), 8);
    let mut cookies = BTreeSet::new();
    let mut native_namespaces = BTreeSet::new();
    for (index, peer) in peers.iter().enumerate() {
        let (lane, server) = (index / 2, index % 2 == 1);
        let identity = &peer["identity"];
        assert_eq!(identity["lane"], lane);
        assert_eq!(identity["role"], if server { "server" } else { "client" });
        assert_eq!(identity["cpu"], cpus[lane + if server { 4 } else { 0 }]);
        assert!(identity["ifindex"].as_u64().unwrap() > 0);
        assert!(native_namespaces.insert(identity["namespace"].as_str().unwrap()));
        assert_eq!(identity["cookies"].as_array().unwrap().len(), 2);
        for cookie in identity["cookies"].as_array().unwrap() {
            let id = cookie.as_str().unwrap().parse::<u64>().unwrap();
            assert!(id > 0 && cookies.insert(id));
        }
        assert_eq!(peer["same_sockets_all_blocks"], true);
        for key in [
            "IpFragCreates",
            "IpReasmOKs",
            "Ip6FragCreates",
            "Ip6ReasmOKs",
        ] {
            assert_eq!(peer["fragment_counter_deltas"][key], 0);
        }
        let blocks = peer["blocks"].as_array().unwrap();
        assert_eq!(blocks.len(), 3 * stride);
        for (phase, row) in blocks.iter().enumerate() {
            let (iterations, count) = shape(phase, lane, stride);
            assert_eq!(row["block"], phase);
            assert_eq!(row["iterations"], iterations);
            assert_eq!(row["segments"], count);
            let mut requests = Vec::new();
            let mut replies = Vec::new();
            for iteration in 0..iterations {
                for family in 0..2 {
                    for part in 0..count {
                        requests.extend(data(
                            phase,
                            lane,
                            iteration,
                            family,
                            part,
                            if part + 1 == count { 17 } else { 1024 },
                        ));
                    }
                    replies.extend(data(phase, lane, iteration, family, 255, 32));
                }
            }
            assert_eq!(row["request_sha256"], hash(&requests));
            assert_eq!(row["reply_sha256"], hash(&replies));
            assert_eq!(
                row["gro_buffers"],
                if server && count > 1 {
                    iterations * 2
                } else {
                    0
                }
            );
            assert_eq!(
                row["gro_segment"],
                if server && count > 1 && iterations > 0 {
                    json!(1024)
                } else {
                    Value::Null
                }
            );
            assert_eq!(row["extra_datagram_observed"], false);
        }
    }
    let stages = run["stages"].as_array().unwrap();
    assert_eq!(stages.len(), 15);
    let expirations = run["expirations"].as_array().unwrap();
    assert_eq!(expirations.len(), 3);
    let mut retained = 0;
    let mut rejected = 0;
    let mut budget = 0;
    if mixed {
        assert_eq!(run["budget_probes"].as_array().unwrap().len(), 3);
    }
    let mut ids = BTreeSet::new();
    let mut namespaces = BTreeMap::new();
    let run_id = run["terminal"]["run_id"].as_str().unwrap();
    for cycle in 0..3 {
        let starts: Vec<u64> = peers
            .iter()
            .step_by(2)
            .map(|p| {
                p["blocks"][cycle * stride + 1]["start_ns"]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap()
            })
            .collect();
        let ends: Vec<u64> = peers
            .iter()
            .step_by(2)
            .map(|p| {
                p["blocks"][cycle * stride + 1]["end_ns"]
                    .as_str()
                    .unwrap()
                    .parse()
                    .unwrap()
            })
            .collect();
        assert!(starts.iter().max().unwrap() < ends.iter().min().unwrap());
        for step in 0..5 {
            let after_probe = usize::from(mixed && step > 0);
            let phase = cycle * stride + step + 1 + after_probe;
            let stage = &stages[cycle * 5 + step];
            assert_eq!(stage["phase"], phase);
            let partial = step == 0;
            let rows = stage["records"].as_array().unwrap();
            if partial {
                assert!((8..=16).contains(&rows.len()));
                if mixed {
                    assert_eq!(rows.len(), 8);
                }
            } else {
                assert_eq!(rows.len(), 12);
            }
            let low = if partial {
                cycle * received
            } else {
                cycle * received + 17 + 17 * after_probe + (step - 1) * 12
            };
            let high = if partial { low + 17 } else { low + 12 };
            let mut expected = Vec::new();
            let mut observations = BTreeSet::new();
            let mut baseline = BTreeSet::new();
            let mut group_members = BTreeMap::new();
            for row in rows {
                let id = row["id"].as_u64().unwrap();
                assert!(id > low as u64 && id <= high as u64 && ids.insert(id));
                let block = row["block"].as_u64().unwrap() as usize;
                let lane = row["lane"].as_u64().unwrap() as usize;
                let server = row["server"].as_bool().unwrap();
                let family = row["family"].as_u64().unwrap() as usize;
                let iteration = row["iteration"].as_u64().unwrap() as usize;
                let part = row["part"].as_u64().unwrap() as usize;
                assert!(lane < 4 && family < 2);
                assert!(if partial {
                    block == phase || block == phase - 1
                } else {
                    block == phase
                });
                let (iterations, count) = shape(block, lane, stride);
                assert!(iteration < iterations && (part == 255 || part < count));
                assert!(observations.insert((lane, server, block, iteration, family, part)));
                let owner = (lane, server);
                let ns = row["netns"].as_u64().unwrap();
                assert!(ns > 0);
                assert_eq!(*namespaces.entry(owner).or_insert(ns), ns);
                let outbound = if part == 255 { server } else { !server };
                if outbound {
                    assert_eq!(row["cpu"], cpus[lane + if server { 4 } else { 0 }]);
                }
                let identity = &peers[lane * 2 + usize::from(server)]["identity"];
                let bytes = data(
                    block,
                    lane,
                    iteration,
                    family,
                    part,
                    if part == 255 {
                        32
                    } else if part + 1 == count {
                        17
                    } else {
                        1024
                    },
                );
                expected.push(packet(
                    server,
                    family,
                    &identity["cookies"][family],
                    outbound,
                    &bytes,
                ));
                if partial {
                    assert!(row["group"].is_null());
                    if block == phase - 1 {
                        baseline.insert((server, family, part));
                        assert!(id <= (cycle * received + 8) as u64);
                    } else {
                        assert_eq!(part, 255);
                    }
                } else if part == 255 {
                    assert!(row["group"].is_null());
                } else {
                    let group = row["group"].as_u64().unwrap();
                    let first = (cycle * (48 + 4 * extra) + (step - 1) * 4 + 33 + 4 * extra) as u64;
                    assert!((first..first + 4).contains(&group));
                    let entry = group_members
                        .entry(group)
                        .or_insert(((lane, server, family), BTreeSet::new()));
                    assert_eq!(entry.0, (lane, server, family));
                    assert!(entry.1.insert(part));
                }
            }
            if partial {
                assert_eq!(baseline.len(), 8);
            } else {
                assert_eq!(group_members.len(), 4);
                assert!(
                    group_members
                        .values()
                        .all(|(_, parts)| *parts == BTreeSet::from([0, 1]))
                );
            }
            assert_eq!(
                stage["expected_sha256"],
                hash(&serde_json::to_vec(&expected).unwrap())
            );
            retained += rows.len() as u64;
            let pending = expirations[cycle]["pending_members"].as_u64().unwrap();
            assert!((1..=9).contains(&pending));
            if mixed && partial {
                budget += 9 - pending;
            }
            let counts = [
                (cycle * matched + 552 + step * 12 + 68 * after_probe) as u64,
                (cycle * lost + 535 + 51 * after_probe) as u64,
                high as u64,
                retained,
                rejected,
                budget,
            ];
            let s = snapshot(&stage["capture"], counts, false);
            assert_eq!(s.run_id, run_id);
            if partial {
                assert_eq!(
                    s.received - s.retained - s.decode_rejected - s.userspace_dropped,
                    pending
                );
                let mut done = counts;
                if mixed {
                    let probe = &run["budget_probes"][cycle];
                    assert_eq!(probe["cycle"], cycle);
                    assert_eq!(probe["phase"], phase + 1);
                    assert_eq!(probe["facts"], 0);
                    done[0] += 68;
                    done[1] += 51;
                    done[2] += 17;
                    budget += 17;
                    done[5] = budget;
                    let waiting = snapshot(&probe["capture"], done, false);
                    assert_eq!(waiting.run_id, run_id);
                    assert!(!waiting.reconciles());
                }
                rejected += pending;
                let expiry = &expirations[cycle];
                assert_eq!(expiry["cycle"], cycle);
                assert_eq!(expiry["pending_members"], pending);
                assert_eq!(expiry["facts"], 0);
                assert!((1000.0..10000.0).contains(&expiry["elapsed_ms"].as_f64().unwrap()));
                done[4] = rejected;
                let s = snapshot(&expiry["capture"], done, false);
                assert!(s.reconciles());
                assert_eq!(s.run_id, run_id);
            } else {
                assert!(s.reconciles());
            }
        }
    }
    assert_eq!(namespaces.len(), 8);
    assert_eq!(namespaces.values().collect::<BTreeSet<_>>().len(), 8);
    assert_eq!(ids.len() as u64, retained);
    let totals = [
        (3 * matched) as u64,
        (3 * lost) as u64,
        (3 * received) as u64,
    ];
    assert_eq!(retained + rejected + budget, totals[2]);
    snapshot(
        &run["terminal"],
        [totals[0], totals[1], totals[2], retained, rejected, budget],
        true,
    );
    let repeats = proof["repetitions"].as_array().unwrap();
    assert_eq!(repeats.len(), 3);
    let mut runs = BTreeSet::new();
    for (index, repeat) in repeats.iter().enumerate() {
        assert_eq!(repeat["iteration"], index + 1);
        assert_eq!(repeat["capture_exit"], 3);
        let pending: Vec<u64> = serde_json::from_value(repeat["pending_members"].clone()).unwrap();
        assert_eq!(pending.len(), 3);
        assert!(pending.iter().all(|n| (1..=9).contains(n)));
        let rejected = pending.iter().sum::<u64>();
        let budget = if mixed { 78 - rejected } else { 0 };
        let s = snapshot(
            &repeat["terminal"],
            [
                totals[0],
                totals[1],
                totals[2],
                totals[2] - rejected - budget,
                rejected,
                budget,
            ],
            true,
        );
        assert!(runs.insert(s.run_id.clone()));
        if index == 0 {
            assert_eq!(s.run_id, run_id);
            assert_eq!(s.retained, retained);
            assert_eq!(repeat["terminal"], run["terminal"]);
            for (expiry, count) in expirations.iter().zip(pending) {
                assert_eq!(expiry["pending_members"], count);
            }
        }
        let sha = repeat["summary_sha256"].as_str().unwrap();
        assert!(sha.len() == 64 && sha.bytes().all(|c| c.is_ascii_hexdigit()));
    }
}

#[test]
fn multi_cpu_ring_loss_proof_reconciles_native_io_partial_expiry_and_recovery() {
    verify(&proof(), false);
}

fn mixed_proof() -> Value {
    serde_json::from_str(&read_repo_file(
        "docs/fixtures/gewyvern_product_udp_mixed_pressure_linux_x86_64_20261127.json",
    ))
    .unwrap()
}

#[test]
fn mixed_pressure_proof_separates_ring_budget_and_idle_expiry() {
    verify(&mixed_proof(), true);
}

#[test]
fn mixed_pressure_proof_rejects_reclassification_and_false_recovery() {
    let original = mixed_proof();
    for case in 0..4 {
        let mut bad = original.clone();
        let run = &mut bad["run"];
        match case {
            0 => {
                let s = &mut run["budget_probes"][0]["capture"];
                let dropped = s["userspace_dropped"]
                    .as_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap();
                s["userspace_dropped"] = json!((dropped - 1).to_string());
                s["decode_rejected"] = json!("1");
            }
            1 => run["budget_probes"][0]["facts"] = json!(1),
            2 => {
                let records = run["stages"][1]["records"].as_array_mut().unwrap();
                let row = records
                    .iter_mut()
                    .find(|row| !row["group"].is_null())
                    .unwrap();
                row["group"] = json!(33);
            }
            3 => run["capture_exit"] = json!(0),
            _ => unreachable!(),
        }
        assert!(
            std::panic::catch_unwind(|| verify(&bad, true)).is_err(),
            "case {case}"
        );
    }
}

#[test]
fn multi_cpu_ring_loss_proof_refuses_partial_publication_and_erased_loss() {
    let original = proof();
    for case in 0..6 {
        let mut bad = original.clone();
        match case {
            0 => bad["run"]["stages"][0]["records"][0]["group"] = json!(1),
            1 => bad["run"]["expirations"][0]["capture"]["final_accounted"] = json!(true),
            2 => bad["run"]["terminal"]["kernel"]["ring_dropped"] = json!("0"),
            3 => {
                bad["run"]["stages"][1]["records"]
                    .as_array_mut()
                    .unwrap()
                    .pop();
            }
            4 => bad["run"]["native"][1]["blocks"][2]["gro_segment"] = Value::Null,
            5 => bad["run"]["capture_exit"] = json!(0),
            _ => unreachable!(),
        }
        assert!(
            std::panic::catch_unwind(|| verify(&bad, false)).is_err(),
            "case {case}"
        );
    }
}
