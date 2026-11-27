//! Native UDP_SEGMENT/UDP_GRO traffic must not become a fictitious datagram.
use super::faults::{await_file, worker};
use super::*;
use std::io::BufRead;

#[path = "offload/service.rs"]
mod service;

const SEGMENT: usize = 1024;
const LENGTHS: [usize; 4] = [SEGMENT, SEGMENT, SEGMENT, 17];
const PHASES: [&str; 4] = ["baseline", "offload", "unselected-offload", "recovery"];

fn lengths() -> Vec<usize> {
    let count: usize = std::env::var("GEWY_OFFLOAD_SEGMENTS")
        .unwrap_or_else(|_| "4".into())
        .parse()
        .unwrap();
    assert!([4, 16, 17].contains(&count));
    let mut result = vec![SEGMENT; count];
    *result.last_mut().unwrap() = 17;
    result
}

fn gro_enabled() -> bool {
    match std::env::var("GEWY_OFFLOAD_GRO").unwrap().as_str() {
        "true" => true,
        "false" => false,
        _ => panic!("explicit GRO mode required"),
    }
}

fn option(socket: &UdpSocket, name: libc::c_int, value: libc::c_int) {
    assert_eq!(
        unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                libc::IPPROTO_UDP,
                name,
                (&value as *const libc::c_int).cast(),
                size_of::<libc::c_int>() as libc::socklen_t,
            )
        },
        0,
        "UDP option: {}",
        std::io::Error::last_os_error()
    );
}

fn sockets(server: bool, shift: u16) -> (Vec<UdpSocket>, Vec<String>) {
    let mut sockets = Vec::new();
    let mut cookies = Vec::new();
    for family in 0..2 {
        let socket = Socket::new(
            if family == 0 {
                Domain::IPV4
            } else {
                Domain::IPV6
            },
            Type::DGRAM,
            Some(Protocol::UDP),
        )
        .unwrap();
        if family == 1 {
            socket.set_only_v6(true).unwrap();
        }
        let mut local = endpoint(family, server);
        let mut remote = endpoint(family, !server);
        local.set_port(local.port() + shift);
        remote.set_port(remote.port() + shift);
        socket.bind(&local.into()).unwrap();
        socket.connect(&remote.into()).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        cookies.push(socket.cookie().unwrap().to_string());
        sockets.push(UdpSocket::from(socket));
    }
    (sockets, cookies)
}

fn payload(phase: usize, family: usize, id: usize, length: usize) -> Vec<u8> {
    let mut data = bytes(phase, length);
    data[..3].copy_from_slice(&[phase as u8, family as u8, id as u8]);
    data
}

fn receive(socket: &UdpSocket) -> std::io::Result<(Vec<u8>, Option<u32>)> {
    let mut buffer = [0_u8; 65536];
    let mut control = [0_usize; 32];
    let mut iov = libc::iovec {
        iov_base: buffer.as_mut_ptr().cast(),
        iov_len: buffer.len(),
    };
    let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
    msg.msg_iov = &mut iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control.as_mut_ptr().cast();
    msg.msg_controllen = size_of_val(&control);
    let received = unsafe { libc::recvmsg(socket.as_raw_fd(), &mut msg, 0) };
    if received < 0 {
        return Err(std::io::Error::last_os_error());
    }
    assert_eq!(msg.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC), 0);
    assert!(msg.msg_controllen <= size_of_val(&control));
    let mut segment = None;
    let mut header = unsafe { libc::CMSG_FIRSTHDR(&msg) };
    while !header.is_null() {
        let offset = header as usize - control.as_ptr() as usize;
        let length = unsafe { (*header).cmsg_len };
        let base = unsafe { libc::CMSG_LEN(0) } as usize;
        assert!(
            offset <= msg.msg_controllen && length >= base && length <= msg.msg_controllen - offset
        );
        if unsafe {
            (*header).cmsg_level == libc::IPPROTO_UDP && (*header).cmsg_type == libc::UDP_GRO
        } {
            assert!(segment.is_none());
            assert_eq!(length - base, size_of::<u32>());
            segment = Some(unsafe { libc::CMSG_DATA(header).cast::<u32>().read_unaligned() });
        }
        header = unsafe { libc::CMSG_NXTHDR(&msg, header) };
    }
    let received = usize::try_from(received).unwrap();
    assert!(received <= buffer.len());
    Ok((buffer[..received].to_vec(), segment))
}

fn verify_receives(gro: bool, parts: &[Vec<u8>], received: &[(Vec<u8>, Option<u32>)]) {
    if gro {
        assert_eq!(
            received.len(),
            1,
            "fixture requires actual GRO, not only its option"
        );
        assert_eq!(received[0].1, Some(SEGMENT as u32));
        assert_eq!(received[0].0, parts.concat());
    } else {
        assert_eq!(received.len(), parts.len());
        for ((data, segment), expected) in received.iter().zip(parts) {
            assert!(segment.is_none());
            assert_eq!(data, expected);
        }
    }
}

#[test]
fn offload_oracle_requires_actual_segmentation_and_gro_metadata() {
    let parts: Vec<_> = LENGTHS
        .iter()
        .enumerate()
        .map(|(id, n)| payload(1, 0, id, *n))
        .collect();
    let split: Vec<_> = parts.iter().map(|data| (data.clone(), None)).collect();
    let joined = vec![(parts.concat(), Some(SEGMENT as u32))];
    verify_receives(false, &parts, &split);
    verify_receives(true, &parts, &joined);
    for (gro, bad) in [
        (true, split.clone()),
        (false, joined.clone()),
        (true, vec![(parts.concat(), None)]),
        (false, split[..3].to_vec()),
        (true, vec![(parts[0].clone(), Some(SEGMENT as u32))]),
    ] {
        assert!(std::panic::catch_unwind(|| verify_receives(gro, &parts, &bad)).is_err());
    }
}

#[test]
#[ignore = "internal native offload worker"]
fn udp_offload_worker() {
    let out = PathBuf::from(std::env::var_os("GW_NET_OUT").unwrap());
    let server = std::env::var("GW_NET_ROLE").unwrap() == "server";
    let gro = gro_enabled();
    let ifindex = enter_worker_network();
    let before = fragment_stats();
    let (selected, cookies) = sockets(server, 0);
    let (unselected, excluded_cookies) = sockets(server, 2);
    let namespace = fs::read_link("/proc/thread-self/ns/net").unwrap();
    fs::write(out.join("ready"), b"ready").unwrap();
    let input = std::io::stdin();
    let mut commands = input.lock().lines();
    let mut expected = Vec::new();
    let mut phases = Vec::new();
    for (index, phase) in PHASES.into_iter().enumerate() {
        assert_eq!(commands.next().unwrap().unwrap(), phase);
        let offload = index == 1 || index == 2;
        let excluded = index == 2;
        let sockets = if excluded { &unselected } else { &selected };
        for socket in sockets {
            option(
                socket,
                libc::UDP_SEGMENT,
                if !server && offload {
                    SEGMENT as i32
                } else {
                    0
                },
            );
            option(socket, libc::UDP_GRO, i32::from(server && offload && gro));
        }
        if server {
            fs::write(out.join(format!("{phase}.ready")), b"ready").unwrap();
        }
        let mut rows = Vec::new();
        for (family, socket) in sockets.iter().enumerate() {
            let cookie = &if excluded {
                &excluded_cookies
            } else {
                &cookies
            }[family];
            let local = socket.local_addr().unwrap();
            let remote = socket.peer_addr().unwrap();
            let parts: Vec<_> = if offload {
                lengths()
                    .iter()
                    .enumerate()
                    .map(|(id, n)| payload(index, family, id, *n))
                    .collect()
            } else {
                vec![payload(index, family, 0, 32)]
            };
            let request = parts.concat();
            let reply = payload(index, family, 0, 32);
            socket
                .set_read_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            let mut received = Vec::new();
            if !server {
                assert_eq!(socket.send(&request).unwrap(), request.len());
                if !excluded && (!offload || parts.len() <= 16) {
                    for part in &parts {
                        expected.push(packet(cookie.parse().unwrap(), local, remote, true, part));
                    }
                }
                let result = receive(socket).unwrap();
                assert_eq!(result, (reply.clone(), None));
                received.push(result);
                if !excluded {
                    expected.push(packet(
                        cookie.parse().unwrap(),
                        local,
                        remote,
                        false,
                        &reply,
                    ));
                }
            } else {
                for _ in 0..if offload && gro { 1 } else { parts.len() } {
                    received.push(receive(socket).unwrap());
                }
                verify_receives(offload && gro, &parts, &received);
                if !excluded && (!(offload && gro) || parts.len() <= 16) {
                    for part in &parts {
                        expected.push(packet(cookie.parse().unwrap(), local, remote, false, part));
                    }
                }
                assert_eq!(socket.send(&reply).unwrap(), reply.len());
                if !excluded {
                    expected.push(packet(cookie.parse().unwrap(), local, remote, true, &reply));
                }
            }
            socket
                .set_read_timeout(Some(Duration::from_millis(40)))
                .unwrap();
            let error = receive(socket).unwrap_err();
            assert!(matches!(
                error.kind(),
                std::io::ErrorKind::WouldBlock | std::io::ErrorKind::TimedOut
            ));
            rows.push(json!({"family":if family==0 {4} else {6},"cookie":cookie,
                "local_port":local.port(),"remote_port":remote.port(),"request_lengths":parts.iter().map(Vec::len).collect::<Vec<_>>(),
                "request_sha256":hash(&request),"reply_sha256":hash(&reply),"send_calls":1,
                "send_bytes":if server {32} else {request.len()},
                "receives":received.iter().map(|(data,segment)| json!({"length":data.len(),"sha256":hash(data),"gro_segment":segment})).collect::<Vec<_>>(),
                "extra_datagram_observed":false,"selected_port":!excluded}));
        }
        let value = json!({"phase":phase,"observations":rows});
        let pending = out.join(format!("{phase}.pending"));
        fs::write(&pending, serde_json::to_vec_pretty(&value).unwrap()).unwrap();
        fs::rename(pending, out.join(format!("{phase}.json"))).unwrap();
        phases.push(value);
    }
    let after = fragment_stats();
    let deltas: BTreeMap<_, _> = after
        .iter()
        .map(|(key, value)| (key.clone(), value - before[key]))
        .collect();
    assert!(
        deltas.values().all(|value| *value == 0),
        "segmentation is not IP fragmentation"
    );
    fs::write(out.join("oracle.json"),serde_json::to_vec_pretty(&json!({"role":if server {"server"} else {"client"},
        "namespace":namespace,"ifindex":ifindex,"cookies":cookies,"excluded_cookies":excluded_cookies,
        "expected":expected,"phases":phases,"same_sockets_all_phases":true,"fragment_counter_deltas":deltas})).unwrap()).unwrap();
}

#[test]
#[ignore = "root/BPF/iproute2; GEWY_OFFLOAD_GRO=true|false and GEWY_OFFLOAD_OUT=new-directory"]
fn production_capture_udp_offload_boundary() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let gro = gro_enabled();
    let count = lengths().len();
    let normalized = count <= 16;
    let fault = std::env::var("GEWY_OFFLOAD_FAULT").unwrap_or_else(|_| "none".into());
    assert!(["none", "ring"].contains(&fault.as_str()));
    let ring_loss = fault == "ring";
    if ring_loss {
        assert!(gro && count == 16);
    }
    let out = PathBuf::from(std::env::var_os("GEWY_OFFLOAD_OUT").unwrap());
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
    let mut capture = super::super::boundaries::collector_with_options(
        &scope,
        PORT,
        &out,
        if ring_loss {
            &["--capture-ring-kib", "4"]
        } else {
            &[]
        },
    );
    ready(&mut capture, &out);
    if ring_loss {
        signal(&capture, libc::SIGSTOP);
    }
    let mut server = worker(
        &names[1],
        "server",
        &scope,
        &out.join("server"),
        "network::offload::udp_offload_worker",
    );
    let mut client = worker(
        &names[0],
        "client",
        &scope,
        &out.join("client"),
        "network::offload::udp_offload_worker",
    );
    await_file(&mut server, &out.join("server/ready"));
    await_file(&mut client, &out.join("client/ready"));
    for phase in PHASES {
        if ring_loss && phase == "recovery" {
            signal(&capture, libc::SIGCONT);
            thread::sleep(Duration::from_millis(200));
        }
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
        await_file(
            &mut server,
            &out.join("server").join(format!("{phase}.json")),
        );
    }
    assert!(wait(&mut client, 5).success());
    assert!(wait(&mut server, 5).success());
    signal(&capture, libc::SIGTERM);
    let exit = wait(&mut capture, 10).code();
    let status = snapshot(&out);
    let bundle = export(&out);
    fs::write(out.join("diagnostic.json"),serde_json::to_vec_pretty(&json!({"exit":exit,"capture":status,
        "fact_ip_lengths":bundle.facts.iter().map(|fact|match &fact.kind {FactKind::PacketMeta(p)=>p.tot_len,_=>0}).collect::<Vec<_>>()})).unwrap()).unwrap();
    detached(&scope);
    fs::remove_dir(&scope.0).unwrap();
    network.remove();
    for name in names {
        assert!(!Path::new("/run/netns").join(name).exists());
    }
    assert_eq!(
        exit,
        Some(if normalized && !ring_loss { 0 } else { 3 }),
        "bounded UDP L4 groups normalize; over-limit aggregates stay unsupported"
    );
    assert!(status.final_accounted && status.reconciles());
    assert_eq!(status.incomplete, !normalized || ring_loss);
    assert_eq!(status.has_loss(), !normalized || ring_loss);
    assert_eq!(status.state, CaptureState::Stopped);
    assert_eq!(
        status.kernel.unsupported,
        if normalized {
            0
        } else if gro {
            4
        } else {
            2
        }
    );
    assert_eq!(status.kernel.malformed, 0);
    assert_eq!(status.kernel.no_cookie, 0);
    assert_eq!(status.userspace_dropped, 0);
    assert_eq!(status.kernel.ring_dropped, if ring_loss { 59 } else { 0 });
    assert_eq!(status.decode_rejected, if ring_loss { 9 } else { 0 });
    assert_eq!(
        status.retained,
        (if ring_loss {
            16
        } else if normalized {
            20 + 4 * count
        } else if gro {
            20
        } else {
            20 + 2 * count
        }) as u64
    );
    let groups = check_offload_groups(&bundle, gro, count, ring_loss);
    let mut oracles: Vec<Value> = ["client", "server"]
        .into_iter()
        .map(|role| {
            serde_json::from_slice(&fs::read(out.join(role).join("oracle.json")).unwrap()).unwrap()
        })
        .collect();
    if ring_loss {
        // A 4 KiB ring holds 17 ABI-v2 records: baseline's eight plus nine
        // members of the first 16-segment group. No partial group may escape.
        assert_eq!(status.received, 25);
        assert_eq!(status.kernel.matched, 84);
        for oracle in &mut oracles {
            oracle["expected"]
                .as_array_mut()
                .unwrap()
                .retain(|packet| packet["prefix"][0] == 0 || packet["prefix"][0] == 3);
        }
    }
    let namespaces = check_namespaces(&bundle, &oracles);
    assert_eq!(namespaces.len(), 2);
    assert_eq!(bundle.capture.as_ref(), Some(&status));
    let replay = bundle.replay().unwrap();
    assert_eq!(replay.facts, bundle.facts);
    assert_eq!(replay.capture, bundle.capture);
    assert_eq!(replay.program_flows, bundle.program_flows);
    assert_eq!(replay.attach_report, bundle.attach_report);
    for oracle in &mut oracles {
        let expected = oracle.as_object_mut().unwrap().remove("expected").unwrap();
        oracle["expected_count"] = json!(expected.as_array().unwrap().len());
        oracle["expected_sha256"] = json!(hash(&serde_json::to_vec(&expected).unwrap()));
    }
    fs::write(out.join("summary.json"),serde_json::to_vec_pretty(&json!({"schema_version":1,"ok":true,
        "gro_enabled":gro,"fault":fault,"production_cli":true,"validation_capture_used":false,"capture_exit":exit,
        "binary_sha256":hash(&fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap()),"capture":status,"oracles":oracles,
        "namespace_ids":namespaces,"facts":bundle.facts.len(),"native_requests":4*count+4,"native_replies":8,
        "segment_count":count,"normalized_groups":groups,"selected_offload_datagrams":2*count,"unselected_offload_datagrams":2*count,"same_socket_recovery":true,
        "packet_multiset_matches":true,"replay_identical":true,"hooks_detached":true,"owned_resources_removed":true,
        "host_network_changed":false,"product_offload_normalization_claim":normalized,
        "limits":["single_host_private_veth","udp_l4_only_max_16_segments","no_physical_nic_certification"]})).unwrap()).unwrap();
}

fn check_offload_groups(
    bundle: &ExportBundle,
    gro: bool,
    count: usize,
    ring_loss: bool,
) -> Vec<Value> {
    let mut groups = BTreeMap::<u64, Vec<_>>::new();
    for fact in &bundle.facts {
        let FactKind::PacketMeta(packet) = &fact.kind else {
            panic!("non-packet")
        };
        if let Some(meta) = packet.udp_offload {
            assert!(count <= 16);
            assert!(meta.matches_packet(packet));
            assert_eq!(
                meta.aggregate_payload_len as usize,
                lengths().iter().sum::<usize>()
            );
            assert_eq!(meta.segment_size, SEGMENT as u16);
            assert_eq!(meta.segment_count as usize, count);
            let family = usize::from(packet.l3_proto == 0x86dd);
            let expected = payload(
                1,
                family,
                meta.segment_index as usize,
                lengths()[meta.segment_index as usize],
            );
            for (offset, byte) in expected.iter().take(128).enumerate() {
                assert_eq!(packet.payload_bytes[&(offset as u16)], *byte);
            }
            groups.entry(meta.aggregate_id).or_default().push(fact);
        }
    }
    assert_eq!(
        groups.len(),
        if count > 16 || ring_loss {
            0
        } else if gro {
            4
        } else {
            2
        }
    );
    groups.into_iter().map(|(id, facts)| {
        assert_eq!(facts.len(), count);
        let mut indexes = BTreeSet::new();
        let mut identities = BTreeSet::new();
        for fact in &facts {
            let FactKind::PacketMeta(p) = &fact.kind else { unreachable!() };
            assert!(indexes.insert(p.udp_offload.unwrap().segment_index));
            assert_eq!(fact.ts, facts[0].ts);
            identities.insert(format!("{:?}/{}/{:?}/{:?}/{}/{:?}", p.dir, p.netns, p.sk_cookie, fact.ifindex, p.l3_proto, fact.cpu));
        }
        assert_eq!(identities.len(), 1);
        json!({"aggregate_id":id.to_string(),"segment_count":count,"indexes":indexes,"identity":identities})
    }).collect()
}
