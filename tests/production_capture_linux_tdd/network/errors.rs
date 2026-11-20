//! Native send/error-queue oracle, independent of the UDP-only product hooks.
use super::faults::{await_file, worker};
use super::*;
use std::io::BufRead;

const PHASES: [&str; 5] = [
    "baseline",
    "mtu-rejection",
    "mtu-recovery",
    "port-unreachable",
    "port-recovery",
];

fn option(socket: &UdpSocket, family: usize, name: libc::c_int, value: libc::c_int) {
    assert_eq!(
        unsafe {
            libc::setsockopt(
                socket.as_raw_fd(),
                if family == 0 {
                    libc::IPPROTO_IP
                } else {
                    libc::IPPROTO_IPV6
                },
                name,
                (&value as *const libc::c_int).cast(),
                size_of::<libc::c_int>() as libc::socklen_t,
            )
        },
        0,
        "setsockopt: {}",
        std::io::Error::last_os_error()
    );
}

#[derive(Debug, PartialEq, serde::Serialize)]
struct ExtendedError {
    errno: u32,
    origin: u8,
    icmp_type: u8,
    icmp_code: u8,
    info: u32,
    data: u32,
}

fn decode_error(bytes: &[u8]) -> Option<ExtendedError> {
    let bytes = bytes.get(..16)?;
    Some(ExtendedError {
        errno: u32::from_ne_bytes(bytes[..4].try_into().ok()?),
        origin: bytes[4],
        icmp_type: bytes[5],
        icmp_code: bytes[6],
        info: u32::from_ne_bytes(bytes[8..12].try_into().ok()?),
        data: u32::from_ne_bytes(bytes[12..16].try_into().ok()?),
    })
}

#[test]
fn extended_error_requires_complete_native_abi() {
    let mut bytes = [0; 16];
    bytes[..4].copy_from_slice(&(libc::EMSGSIZE as u32).to_ne_bytes());
    bytes[4] = libc::SO_EE_ORIGIN_LOCAL;
    bytes[8..12].copy_from_slice(&1500_u32.to_ne_bytes());
    for length in 0..16 {
        assert!(decode_error(&bytes[..length]).is_none());
    }
    assert_eq!(
        decode_error(&bytes),
        Some(ExtendedError {
            errno: libc::EMSGSIZE as u32,
            origin: libc::SO_EE_ORIGIN_LOCAL,
            icmp_type: 0,
            icmp_code: 0,
            info: 1500,
            data: 0,
        })
    );
}

fn error_queue(socket: &UdpSocket, family: usize, local: bool, sent: &[u8]) -> Value {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let mut payload = [0_u8; 8192];
        // usize alignment satisfies cmsghdr alignment, unlike a byte array.
        let mut control = [0_usize; 64];
        let mut address = socket2::SockAddrStorage::zeroed();
        let mut iov = libc::iovec {
            iov_base: payload.as_mut_ptr().cast(),
            iov_len: payload.len(),
        };
        let mut msg: libc::msghdr = unsafe { std::mem::zeroed() };
        msg.msg_name = (unsafe { address.view_as::<libc::sockaddr_storage>() }
            as *mut libc::sockaddr_storage)
            .cast();
        msg.msg_namelen = address.size_of();
        msg.msg_iov = &mut iov;
        msg.msg_iovlen = 1;
        msg.msg_control = control.as_mut_ptr().cast();
        msg.msg_controllen = size_of_val(&control);
        let received = unsafe {
            libc::recvmsg(
                socket.as_raw_fd(),
                &mut msg,
                libc::MSG_ERRQUEUE | libc::MSG_DONTWAIT,
            )
        };
        if received < 0 {
            let error = std::io::Error::last_os_error();
            assert_eq!(
                error.kind(),
                std::io::ErrorKind::WouldBlock,
                "error queue: {error}"
            );
            assert!(
                Instant::now() < deadline,
                "missing native error queue entry"
            );
            thread::sleep(Duration::from_millis(5));
            continue;
        }
        assert_eq!(msg.msg_flags & (libc::MSG_TRUNC | libc::MSG_CTRUNC), 0);
        assert_ne!(msg.msg_flags & libc::MSG_ERRQUEUE, 0);
        assert!(msg.msg_controllen <= size_of_val(&control));
        let mut errors = Vec::new();
        let mut header = unsafe { libc::CMSG_FIRSTHDR(&msg) };
        while !header.is_null() {
            let header_len = unsafe { libc::CMSG_LEN(0) } as usize;
            let length = unsafe { (*header).cmsg_len };
            let offset = header as usize - control.as_ptr() as usize;
            assert!(length >= header_len && length <= msg.msg_controllen - offset);
            if unsafe { (*header).cmsg_level }
                == if family == 0 {
                    libc::IPPROTO_IP
                } else {
                    libc::IPPROTO_IPV6
                }
                && unsafe { (*header).cmsg_type }
                    == if family == 0 {
                        libc::IP_RECVERR
                    } else {
                        libc::IPV6_RECVERR
                    }
            {
                let bytes = unsafe {
                    std::slice::from_raw_parts(libc::CMSG_DATA(header), length - header_len)
                };
                errors.push(decode_error(bytes).expect("complete sock_extended_err"));
            }
            header = unsafe { libc::CMSG_NXTHDR(&msg, header) };
        }
        assert_eq!(errors.len(), 1);
        let error = &errors[0];
        assert_eq!(
            error.errno,
            if local {
                libc::EMSGSIZE
            } else {
                libc::ECONNREFUSED
            } as u32
        );
        assert_eq!(
            error.origin,
            if local {
                libc::SO_EE_ORIGIN_LOCAL
            } else if family == 0 {
                libc::SO_EE_ORIGIN_ICMP
            } else {
                libc::SO_EE_ORIGIN_ICMP6
            }
        );
        assert_eq!(
            (error.icmp_type, error.icmp_code),
            if local {
                (0, 0)
            } else if family == 0 {
                (3, 3)
            } else {
                (1, 4)
            }
        );
        assert_eq!(error.info, if local { 1500 } else { 0 });
        let received = usize::try_from(received).unwrap();
        assert!(received <= payload.len());
        assert!(msg.msg_namelen <= address.size_of());
        let destination = unsafe { socket2::SockAddr::new(address, msg.msg_namelen) }.as_socket();
        if !local {
            assert_eq!(&payload[..received], sent);
            assert_eq!(destination, Some(endpoint(family, true)));
        }
        assert!(socket.take_error().unwrap().is_none());
        return json!({"extended_error":error,"quoted_length":received,
            "quoted_payload_sha256":hash(&payload[..received]),"destination":destination,
            "pending_socket_error_cleared":true});
    }
}

fn publish(out: &Path, phase: &str, result: &Value) {
    let pending = out.join(format!("{phase}.pending"));
    fs::write(&pending, serde_json::to_vec_pretty(result).unwrap()).unwrap();
    fs::rename(pending, out.join(format!("{phase}.json"))).unwrap();
}

#[test]
#[ignore = "internal persistent worker for isolated UDP native-error test"]
fn udp_error_worker() {
    let out = PathBuf::from(std::env::var_os("GW_NET_OUT").unwrap());
    let server = std::env::var("GW_NET_ROLE").unwrap() == "server";
    let ifindex = enter_worker_network();
    let before = fragment_stats();
    let (mut sockets, mut cookies) = network_sockets(server);
    let mut all_cookies = cookies.clone();
    if !server {
        for (family, socket) in sockets.iter().enumerate() {
            option(
                socket,
                family,
                if family == 0 {
                    libc::IP_MTU_DISCOVER
                } else {
                    libc::IPV6_MTU_DISCOVER
                },
                libc::IP_PMTUDISC_DO,
            );
            option(
                socket,
                family,
                if family == 0 {
                    libc::IP_RECVERR
                } else {
                    libc::IPV6_RECVERR
                },
                1,
            );
            socket.connect(endpoint(family, true)).unwrap();
        }
    }
    let namespace = fs::read_link("/proc/thread-self/ns/net").unwrap();
    fs::write(out.join("ready"), b"ready").unwrap();
    let mut expected = Vec::new();
    let mut phases = Vec::new();
    let input = std::io::stdin();
    let mut commands = input.lock().lines();
    for (index, phase) in PHASES.into_iter().enumerate() {
        assert_eq!(commands.next().unwrap().unwrap(), phase);
        if server {
            if phase == "port-unreachable" {
                sockets.clear();
            } else if phase == "port-recovery" {
                (sockets, cookies) = network_sockets(true);
                all_cookies.extend(cookies.clone());
            }
            // The controller must observe close/rebind before releasing the client.
            fs::write(out.join(format!("{phase}.ready")), b"ready").unwrap();
        }
        let mut observations = Vec::new();
        for (family, socket) in sockets.iter().enumerate() {
            let local = endpoint(family, server);
            let remote = endpoint(family, !server);
            let data = bytes(index, if phase == "mtu-rejection" { 4096 } else { 32 });
            let cookie = cookies[family].parse().unwrap();
            let mut row = json!({"family":if family==0 {4}else{6},"cookie":cookies[family],
                "length":data.len(),"payload_sha256":hash(&data),"sent":false,"received":false});
            if !server {
                if phase == "mtu-rejection" {
                    let error = socket.send(&data).unwrap_err();
                    assert_eq!(error.raw_os_error(), Some(libc::EMSGSIZE));
                    row["send_errno"] = json!(libc::EMSGSIZE);
                    row["error_queue"] = error_queue(socket, family, true, &data);
                    observations.push(row);
                    continue;
                }
                assert_eq!(socket.send(&data).unwrap(), data.len());
                row["sent"] = json!(true);
                expected.push(packet(cookie, local, remote, true, &data));
            }
            socket
                .set_read_timeout(Some(if phase == "mtu-rejection" {
                    Duration::from_millis(200)
                } else {
                    Duration::from_secs(3)
                }))
                .unwrap();
            let mut buffer = [0; 8192];
            let received = socket.recv_from(&mut buffer);
            if phase == "mtu-rejection" {
                let error = received.unwrap_err();
                assert!(matches!(
                    error.kind(),
                    std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                ));
                row["timeout"] = json!(true);
            } else if phase == "port-unreachable" {
                let error = received.unwrap_err();
                assert_eq!(error.raw_os_error(), Some(libc::ECONNREFUSED));
                row["receive_errno"] = json!(libc::ECONNREFUSED);
                row["error_queue"] = error_queue(socket, family, false, &data);
            } else {
                let (n, source) = received.unwrap();
                assert_eq!(source, remote);
                assert_eq!(&buffer[..n], &data);
                row["received"] = json!(true);
                expected.push(packet(cookie, local, remote, false, &buffer[..n]));
                if server {
                    assert_eq!(socket.send_to(&data, remote).unwrap(), data.len());
                    row["sent"] = json!(true);
                    expected.push(packet(cookie, local, remote, true, &data));
                }
            }
            observations.push(row);
        }
        let result = json!({"phase":phase,"observations":observations,
            "open_sockets":sockets.len(),"server_ready_before_client":true});
        publish(&out, phase, &result);
        phases.push(result);
    }
    let after = fragment_stats();
    let deltas: BTreeMap<_, _> = after
        .iter()
        .map(|(key, value)| (key.clone(), value - before[key]))
        .collect();
    for key in [
        "IpFragCreates",
        "IpReasmOKs",
        "Ip6FragCreates",
        "Ip6ReasmOKs",
    ] {
        assert_eq!(deltas[key], 0);
    }
    fs::write(
        out.join("oracle.json"),
        serde_json::to_vec_pretty(&json!({
            "role":if server {"server"}else{"client"},"namespace":namespace,"ifindex":ifindex,
            "cookies":all_cookies,"expected":expected,"phases":phases,
            "same_sockets_all_phases":!server,"fragment_counter_deltas":deltas
        }))
        .unwrap(),
    )
    .unwrap();
}

#[test]
#[ignore = "root, iproute2, BPF tools and GEWY_NETWORK_ERRORS_OUT=new-directory required"]
fn production_capture_network_errors() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let out =
        PathBuf::from(std::env::var_os("GEWY_NETWORK_ERRORS_OUT").expect("new evidence directory"));
    fs::create_dir(&out).unwrap();
    let mut network = Network::new();
    let names = network.0.clone();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scope = PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-netns-{}-{nonce}",
        std::process::id()
    ));
    fs::create_dir(&scope).unwrap();
    let scope = Scope(scope);
    let mut capture = collector(&scope, PORT, &out);
    ready(&mut capture, &out);
    let mut server = worker(
        &names[1],
        "server",
        &scope,
        &out.join("server"),
        "network::errors::udp_error_worker",
    );
    let mut client = worker(
        &names[0],
        "client",
        &scope,
        &out.join("client"),
        "network::errors::udp_error_worker",
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
            &mut server,
            &out.join("server").join(format!("{phase}.json")),
        );
        await_file(
            &mut client,
            &out.join("client").join(format!("{phase}.json")),
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
    assert_eq!(status.retained, 26);
    let bundle = export(&out);
    let oracles: Vec<Value> = ["client", "server"]
        .into_iter()
        .map(|role| {
            serde_json::from_slice(&fs::read(out.join(role).join("oracle.json")).unwrap()).unwrap()
        })
        .collect();
    assert_ne!(oracles[0]["namespace"], oracles[1]["namespace"]);
    let namespaces = check_namespaces(&bundle, &oracles);
    assert_eq!(namespaces.len(), 2);
    assert_eq!(bundle.facts.len(), 26);
    assert_eq!(bundle.capture.as_ref(), Some(&status));
    let replay = bundle.replay().unwrap();
    assert_eq!(replay.facts, bundle.facts);
    assert_eq!(replay.capture, bundle.capture);
    assert_eq!(replay.program_flows, bundle.program_flows);
    assert_eq!(replay.attach_report, bundle.attach_report);
    detached(&scope);
    fs::remove_dir(&scope.0).unwrap();
    network.remove();
    for name in names {
        assert!(!Path::new("/run/netns").join(name).exists());
    }
    fs::write(out.join("summary.json"), serde_json::to_vec_pretty(&json!({
        "schema_version":1,"ok":true,"production_cli":true,"validation_capture_used":false,
        "binary_sha256":hash(&fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap()),
        "capture":status,"oracles":oracles,"namespace_ids":namespaces,
        "send_attempts":10,"local_mtu_rejections":2,"remote_port_refusals":2,
        "successful_exchanges":6,"error_queue_entries":4,"facts":bundle.facts.len(),
        "same_client_socket_recovery":true,"server_socket_replacement":true,
        "packet_multiset_matches":true,"replay_identical":true,"hooks_detached":true,
        "owned_resources_removed":true,"host_network_changed":false,"product_icmp_decoder_claim":false,
        "limits":["single_host_private_veth","selected_udp_only","local_mtu_not_routed_pmtu",
            "application_failure_is_not_capture_loss","native_error_queue_is_not_product_icmp_analysis"]
    })).unwrap()).unwrap();
}
