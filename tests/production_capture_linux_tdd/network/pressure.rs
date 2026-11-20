//! Separate hook observation, UDP queue admission and native application reads.
use super::faults::{await_file, worker};
use super::*;
use std::io::BufRead;

const BURST: u8 = 64;
const PHASES: [&str; 5] = [
    "baseline",
    "truncate",
    "truncate-recovery",
    "overflow",
    "overflow-recovery",
];

fn message(phase: usize, family: usize, id: u8, length: usize) -> Vec<u8> {
    let mut data = bytes(phase, length);
    data[..3].copy_from_slice(&[phase as u8, family as u8, id]);
    data
}

fn udp_stats() -> BTreeMap<String, u64> {
    let mut result = BTreeMap::new();
    let text = fs::read_to_string("/proc/thread-self/net/snmp").unwrap();
    let mut lines = text.lines();
    while let (Some(names), Some(values)) = (lines.next(), lines.next()) {
        if names.starts_with("Udp:") {
            for (name, value) in names
                .split_whitespace()
                .zip(values.split_whitespace())
                .skip(1)
            {
                result.insert(format!("Udp{name}"), value.parse().unwrap());
            }
        }
    }
    for line in fs::read_to_string("/proc/thread-self/net/snmp6")
        .unwrap()
        .lines()
    {
        let mut fields = line.split_whitespace();
        let name = fields.next().unwrap();
        if name.starts_with("Udp6") {
            result.insert(name.to_owned(), fields.next().unwrap().parse().unwrap());
        }
    }
    for prefix in ["Udp", "Udp6"] {
        assert!(result.contains_key(&format!("{prefix}RcvbufErrors")));
    }
    result
}

struct Received {
    wire_len: usize,
    bytes: Vec<u8>,
    truncated: bool,
    drops: Option<u32>,
}

fn receive(socket: &UdpSocket, capacity: usize) -> std::io::Result<Received> {
    let mut bytes = vec![0; capacity];
    let mut control = [0_usize; 32];
    let mut iov = libc::iovec {
        iov_base: bytes.as_mut_ptr().cast(),
        iov_len: bytes.len(),
    };
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr().cast();
    msg.msg_controllen = size_of_val(&control);
    // MSG_TRUNC returns the datagram length even when the application copies less.
    let received = unsafe { libc::recvmsg(socket.as_raw_fd(), &mut msg, libc::MSG_TRUNC) };
    if received < 0 {
        return Err(std::io::Error::last_os_error());
    }
    assert_eq!(msg.msg_flags & libc::MSG_CTRUNC, 0);
    assert!(msg.msg_controllen <= size_of_val(&control));
    let mut drops = None;
    let mut header = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    while !header.is_null() {
        let offset = header as usize - control.as_ptr() as usize;
        let length = unsafe { (*header).cmsg_len };
        let base = unsafe { libc::CMSG_LEN(0) } as usize;
        assert!(
            offset <= msg.msg_controllen && length >= base && length <= msg.msg_controllen - offset
        );
        if unsafe {
            (*header).cmsg_level == libc::SOL_SOCKET && (*header).cmsg_type == libc::SO_RXQ_OVFL
        } {
            assert_eq!(length - base, size_of::<u32>());
            assert!(drops.is_none());
            drops = Some(unsafe { libc::CMSG_DATA(header).cast::<u32>().read_unaligned() });
        }
        header = unsafe { libc::CMSG_NXTHDR(&msg, header) };
    }
    let wire_len = usize::try_from(received).unwrap();
    bytes.truncate(capacity.min(wire_len));
    Ok(Received {
        wire_len,
        bytes,
        truncated: msg.msg_flags & libc::MSG_TRUNC != 0,
        drops,
    })
}

fn receipt(received: &Received, data: &[u8], capacity: usize) -> Value {
    assert_eq!(received.wire_len, data.len());
    assert_eq!(received.bytes, data[..data.len().min(capacity)]);
    assert_eq!(received.truncated, data.len() > capacity);
    json!({"id":data[2],"wire_len":received.wire_len,"copied_len":received.bytes.len(),
        "copied_sha256":hash(&received.bytes),"truncated":received.truncated,"rxq_drops":received.drops})
}

fn verify_overflow(ids: &[u8], drops: u64) {
    assert!(!ids.is_empty() && ids.len() < usize::from(BURST));
    let unique: BTreeSet<_> = ids.iter().copied().collect();
    assert_eq!(unique.len(), ids.len());
    assert!(unique.iter().all(|id| *id < BURST));
    assert!(drops > 0);
    assert_eq!(ids.len() as u64 + drops, u64::from(BURST));
}

#[test]
fn overflow_oracle_requires_actual_drop_and_exact_accounting() {
    verify_overflow(&[0, 1, 2, 3], 60);
    for (ids, drops) in [
        (vec![], 64),
        (vec![0, 0], 62),
        (vec![64], 63),
        (vec![0, 1], 61),
        ((0..64).collect(), 0),
    ] {
        assert!(std::panic::catch_unwind(|| verify_overflow(&ids, drops)).is_err());
    }
}

#[test]
fn truncation_oracle_requires_original_length_prefix_and_flag() {
    let data = message(1, 0, 0, 1024);
    let received = Received {
        wire_len: 1024,
        bytes: data[..64].to_vec(),
        truncated: true,
        drops: None,
    };
    assert_eq!(receipt(&received, &data, 64)["copied_len"], 64);
    for mutation in 0..3 {
        let mut bad = Received {
            wire_len: 1024,
            bytes: data[..64].to_vec(),
            truncated: true,
            drops: None,
        };
        match mutation {
            0 => bad.wire_len = 64,
            1 => bad.bytes[10] ^= 1,
            _ => bad.truncated = false,
        }
        assert!(std::panic::catch_unwind(|| receipt(&bad, &data, 64)).is_err());
    }
}

fn publish(out: &Path, phase: &str, value: &Value) {
    let pending = out.join(format!("{phase}.pending"));
    fs::write(&pending, serde_json::to_vec_pretty(value).unwrap()).unwrap();
    fs::rename(pending, out.join(format!("{phase}.json"))).unwrap();
}

#[test]
#[ignore = "internal native worker for isolated UDP receive-pressure test"]
fn udp_pressure_worker() {
    let out = PathBuf::from(std::env::var_os("GW_NET_OUT").unwrap());
    let server = std::env::var("GW_NET_ROLE").unwrap() == "server";
    let ifindex = enter_worker_network();
    let (sockets, cookies) = network_sockets(server);
    for (family, socket) in sockets.iter().enumerate() {
        socket.connect(endpoint(family, !server)).unwrap();
        let enabled: libc::c_int = 1;
        assert_eq!(
            unsafe {
                libc::setsockopt(
                    socket.as_raw_fd(),
                    libc::SOL_SOCKET,
                    libc::SO_RXQ_OVFL,
                    (&enabled as *const libc::c_int).cast(),
                    size_of::<libc::c_int>() as libc::socklen_t,
                )
            },
            0
        );
    }
    let namespace = fs::read_link("/proc/thread-self/ns/net").unwrap();
    fs::write(out.join("ready"), b"ready").unwrap();
    let input = std::io::stdin();
    let mut commands = input.lock().lines();
    let mut expected = Vec::new();
    let mut phases = Vec::new();
    let mut overflow_drops = [0_u64; 2];
    for (index, phase) in PHASES.into_iter().enumerate() {
        assert_eq!(commands.next().unwrap().unwrap(), phase);
        let overflow = phase == "overflow";
        let before = udp_stats();
        if server {
            if overflow {
                for socket in &sockets {
                    socket2::SockRef::from(socket)
                        .set_recv_buffer_size(4096)
                        .unwrap();
                }
            }
            fs::write(out.join(format!("{phase}.ready")), b"ready").unwrap();
            if overflow {
                // No reads until BOTH families have completed the whole burst.
                assert_eq!(commands.next().unwrap().unwrap(), "drain");
            }
        }
        let mut rows = Vec::new();
        for (family, socket) in sockets.iter().enumerate() {
            let local = endpoint(family, server);
            let remote = endpoint(family, !server);
            let cookie = cookies[family].parse().unwrap();
            let length = if overflow || phase == "truncate" {
                1024
            } else {
                32
            };
            let count = if overflow { BURST } else { 1 };
            let mut sent = Vec::new();
            let mut receipts = Vec::new();
            socket
                .set_read_timeout(Some(if overflow {
                    Duration::from_millis(200)
                } else {
                    Duration::from_secs(3)
                }))
                .unwrap();
            if !server {
                for id in 0..count {
                    let data = message(index, family, id, length);
                    assert_eq!(socket.send(&data).unwrap(), data.len());
                    expected.push(packet(cookie, local, remote, true, &data));
                    sent.push(id);
                }
            }
            if overflow {
                if server {
                    loop {
                        let received = match receive(socket, 2048) {
                            Ok(received) => received,
                            Err(error)
                                if matches!(
                                    error.kind(),
                                    std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
                                ) =>
                            {
                                break;
                            }
                            Err(error) => panic!("overflow drain: {error}"),
                        };
                        assert!(receipts.len() < usize::from(BURST));
                        let id = received.bytes[2];
                        assert!(id < BURST);
                        receipts.push(receipt(
                            &received,
                            &message(index, family, id, length),
                            2048,
                        ));
                    }
                    // The hypothesis under test is hook visibility before queue rejection,
                    // NOT a claim that the application received every sent datagram.
                    for id in 0..count {
                        expected.push(packet(
                            cookie,
                            local,
                            remote,
                            false,
                            &message(index, family, id, length),
                        ));
                    }
                }
            } else {
                let capacity = if server && phase == "truncate" {
                    64
                } else {
                    2048
                };
                let data = message(index, family, 0, if server { length } else { 32 });
                let received = receive(socket, capacity).unwrap();
                if server && phase == "overflow-recovery" {
                    assert_eq!(received.drops.map(u64::from), Some(overflow_drops[family]));
                } else {
                    assert_eq!(received.drops.unwrap_or(0), 0);
                }
                receipts.push(receipt(&received, &data, capacity));
                expected.push(packet(cookie, local, remote, false, &data));
                if server {
                    let reply = message(index, family, 0, 32);
                    assert_eq!(socket.send(&reply).unwrap(), reply.len());
                    expected.push(packet(cookie, local, remote, true, &reply));
                    sent.push(0);
                }
            }
            rows.push(
                json!({"family":if family==0 {4} else {6},"cookie":cookies[family],
                "request_length":length,"sent_ids":sent,"receipts":receipts,
                "receive_buffer_bytes":socket2::SockRef::from(socket).recv_buffer_size().unwrap()}),
            );
        }
        let after = udp_stats();
        let deltas: BTreeMap<_, _> = after
            .iter()
            .map(|(key, value)| (key.clone(), value - before[key]))
            .collect();
        for (family, prefix) in ["Udp", "Udp6"].into_iter().enumerate() {
            let drops = deltas[&format!("{prefix}RcvbufErrors")];
            assert_eq!(
                deltas[&format!("{prefix}InDatagrams")],
                rows[family]["receipts"].as_array().unwrap().len() as u64
            );
            assert_eq!(
                deltas[&format!("{prefix}OutDatagrams")],
                rows[family]["sent_ids"].as_array().unwrap().len() as u64
            );
            assert_eq!(deltas[&format!("{prefix}InErrors")], drops);
            for name in ["InCsumErrors", "NoPorts", "SndbufErrors"] {
                assert_eq!(deltas[&format!("{prefix}{name}")], 0);
            }
            if server && overflow {
                let ids: Vec<_> = rows[family]["receipts"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|row| row["id"].as_u64().unwrap() as u8)
                    .collect();
                verify_overflow(&ids, drops);
                overflow_drops[family] = drops;
            } else {
                assert_eq!(drops, 0);
            }
        }
        let value = json!({"phase":phase,"observations":rows,"udp_counter_deltas":deltas,
            "receiver_paused_until_all_sends":server && overflow});
        publish(&out, phase, &value);
        phases.push(value);
    }
    fs::write(out.join("oracle.json"),serde_json::to_vec_pretty(&json!({
        "role":if server {"server"} else {"client"},"namespace":namespace,"ifindex":ifindex,
        "cookies":cookies,"expected":expected,"phases":phases,"same_sockets_all_phases":true,
        "overflow_expected_basis":"sender_bytes_and_native_rcvbuf_drop_accounting_not_application_delivery"
    })).unwrap()).unwrap();
}

#[test]
#[ignore = "root, iproute2, BPF tools and GEWY_NETWORK_PRESSURE_OUT=new-directory required"]
fn production_capture_receive_pressure() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let out = PathBuf::from(
        std::env::var_os("GEWY_NETWORK_PRESSURE_OUT").expect("new evidence directory"),
    );
    fs::create_dir(&out).unwrap();
    let mut network = Network::new();
    let names = network.0.clone();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-netns-{}-{nonce}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    let mut capture = collector(&scope, PORT, &out);
    ready(&mut capture, &out);
    let mut server = worker(
        &names[1],
        "server",
        &scope,
        &out.join("server"),
        "network::pressure::udp_pressure_worker",
    );
    let mut client = worker(
        &names[0],
        "client",
        &scope,
        &out.join("client"),
        "network::pressure::udp_pressure_worker",
    );
    await_file(&mut server, &out.join("server/ready"));
    await_file(&mut client, &out.join("client/ready"));
    for phase in PHASES {
        writeln!(server.0.stdin.as_mut().unwrap(), "{phase}").unwrap();
        await_file(
            &mut server,
            &out.join("server").join(format!("{phase}.ready")),
        );
        writeln!(client.0.stdin.as_mut().unwrap(), "{phase}").unwrap();
        await_file(
            &mut client,
            &out.join("client").join(format!("{phase}.json")),
        );
        if phase == "overflow" {
            writeln!(server.0.stdin.as_mut().unwrap(), "drain").unwrap();
        }
        await_file(
            &mut server,
            &out.join("server").join(format!("{phase}.json")),
        );
    }
    assert!(wait(&mut client, 5).success());
    assert!(wait(&mut server, 5).success());
    signal(&capture, libc::SIGTERM);
    assert!(wait(&mut capture, 10).success());
    let status = snapshot(&out);
    assert!(
        status.final_accounted && status.reconciles() && !status.incomplete && !status.has_loss()
    );
    assert_eq!(status.state, CaptureState::Stopped);
    assert_eq!(status.retained, 288);
    let bundle = export(&out);
    let mut oracles: Vec<Value> = ["client", "server"]
        .into_iter()
        .map(|role| {
            serde_json::from_slice(&fs::read(out.join(role).join("oracle.json")).unwrap()).unwrap()
        })
        .collect();
    assert_ne!(oracles[0]["namespace"], oracles[1]["namespace"]);
    let namespaces = check_namespaces(&bundle, &oracles);
    assert_eq!(namespaces.len(), 2);
    assert_eq!(bundle.facts.len(), 288);
    assert_eq!(bundle.capture.as_ref(), Some(&status));
    let replay = bundle.replay().unwrap();
    assert_eq!(replay.facts, bundle.facts);
    assert_eq!(replay.capture, bundle.capture);
    assert_eq!(replay.program_flows, bundle.program_flows);
    assert_eq!(replay.attach_report, bundle.attach_report);
    let drops: u64 = ["UdpRcvbufErrors", "Udp6RcvbufErrors"]
        .iter()
        .map(|key| {
            oracles[1]["phases"][3]["udp_counter_deltas"][key]
                .as_u64()
                .unwrap()
        })
        .sum();
    for oracle in &mut oracles {
        let expected = oracle.as_object_mut().unwrap().remove("expected").unwrap();
        oracle["expected_count"] = json!(expected.as_array().unwrap().len());
        oracle["expected_sha256"] = json!(hash(&serde_json::to_vec(&expected).unwrap()));
    }
    detached(&scope);
    fs::remove_dir(&scope.0).unwrap();
    network.remove();
    for name in names {
        assert!(!Path::new("/run/netns").join(name).exists());
    }
    fs::write(out.join("summary.json"),serde_json::to_vec_pretty(&json!({
        "schema_version":1,"ok":true,"production_cli":true,"validation_capture_used":false,
        "binary_sha256":hash(&fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap()),
        "capture":status,"oracles":oracles,"namespace_ids":namespaces,
        "requests_sent":136,"request_replies":8,"application_truncations":2,
        "overflow_sent":128,"overflow_delivered":128-drops,"socket_queue_drops":drops,
        "facts":bundle.facts.len(),"same_socket_recovery":true,"packet_multiset_matches":true,
        "replay_identical":true,"hooks_detached":true,"owned_resources_removed":true,
        "host_network_changed":false,"product_socket_drop_classifier_claim":false,
        "limits":["single_host_private_veth","selected_udp_only","bounded_burst_not_sustained_load",
            "ingress_observation_is_not_application_delivery","native_drop_counters_are_independent_oracles"]
    })).unwrap()).unwrap();
}
