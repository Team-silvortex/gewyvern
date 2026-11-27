//! Privileged, opt-in Linux lifecycle test. The target peers are real UDP
//! sockets; only the separate worker enters the owned cgroup before socket creation.
#![cfg(target_os = "linux")]
#[path = "production_capture_linux_tdd/boundaries.rs"]
mod boundaries;
#[path = "production_capture_linux_tdd/http_status.rs"]
mod http_status;
#[path = "production_capture_linux_tdd/installed.rs"]
mod installed;
#[path = "production_capture_linux_tdd/memcached.rs"]
mod memcached;
#[path = "production_capture_linux_tdd/mqtt.rs"]
mod mqtt;
#[path = "production_capture_linux_tdd/mysql.rs"]
mod mysql;
#[path = "production_capture_linux_tdd/network.rs"]
mod network;
#[path = "production_capture_linux_tdd/postgres.rs"]
mod postgres;
#[path = "production_capture_linux_tdd/private_peer.rs"]
mod private_peer;
#[path = "production_capture_linux_tdd/redis_resp.rs"]
mod redis_resp;
use gewyvern::export::ExportBundle;
use gewyvern::ledger::FactKind;
use gewyvern::live_capture::{CaptureSnapshot, CaptureState};
use serde_json::{Value, json};
use socket2::{Domain, Protocol, Socket, Type};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

struct Process(Child);
impl Drop for Process {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
struct Scope(PathBuf);
impl Drop for Scope {
    fn drop(&mut self) {
        let _ = fs::remove_dir(&self.0);
    }
}
struct Peers {
    stop: Arc<AtomicBool>,
    threads: Vec<thread::JoinHandle<()>>,
    port: u16,
}
impl Drop for Peers {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for thread in self.threads.drain(..) {
            let _ = thread.join();
        }
    }
}

fn peers() -> Peers {
    let v4 = UdpSocket::bind("127.0.0.1:0").unwrap();
    let port = v4.local_addr().unwrap().port();
    let v6 = Socket::new(Domain::IPV6, Type::DGRAM, Some(Protocol::UDP)).unwrap();
    v6.set_only_v6(true).unwrap();
    v6.bind(&SocketAddr::from((std::net::Ipv6Addr::LOCALHOST, port)).into())
        .unwrap();
    let stop = Arc::new(AtomicBool::new(false));
    let threads = [v4, UdpSocket::from(v6)]
        .into_iter()
        .map(|socket| {
            let stop = stop.clone();
            socket
                .set_read_timeout(Some(Duration::from_millis(50)))
                .unwrap();
            thread::spawn(move || {
                let mut bytes = [0u8; 512];
                while !stop.load(Ordering::Relaxed) {
                    match socket.recv_from(&mut bytes) {
                        Ok((length, source)) => {
                            assert_eq!(length, 22);
                            assert_eq!(&bytes[2..12], &[1, 0, 0, 1, 0, 0, 0, 0, 0, 0]);
                            bytes[2] = 0x81;
                            bytes[3] = 0x83;
                            socket.send_to(&bytes[..length], source).unwrap();
                        }
                        Err(error)
                            if matches!(
                                error.kind(),
                                std::io::ErrorKind::TimedOut | std::io::ErrorKind::WouldBlock
                            ) => {}
                        Err(error) => panic!("peer read: {error}"),
                    }
                }
            })
        })
        .collect();
    Peers {
        stop,
        threads,
        port,
    }
}

#[test]
#[ignore = "internal worker for the privileged production capture test"]
fn production_udp_traffic_worker() {
    let scope = PathBuf::from(std::env::var_os("GW_CAPTURE_WORKER_SCOPE").expect("worker scope"));
    assert!(
        scope.starts_with("/sys/fs/cgroup/gewyvern-production-test")
            || scope
                .file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with("gewyvern-production-test-")
    );
    fs::write(scope.join("cgroup.procs"), std::process::id().to_string()).unwrap();
    let port: u16 = std::env::var("GW_CAPTURE_WORKER_PORT")
        .unwrap()
        .parse()
        .unwrap();
    let count: u16 = std::env::var("GW_CAPTURE_WORKER_COUNT")
        .unwrap()
        .parse()
        .unwrap();
    let pause: u64 = std::env::var("GW_CAPTURE_WORKER_PAUSE")
        .unwrap()
        .parse()
        .unwrap();
    let v4 = UdpSocket::bind("127.0.0.1:0").unwrap();
    let v6 = UdpSocket::bind("[::1]:0").unwrap();
    v4.connect((std::net::Ipv4Addr::LOCALHOST, port)).unwrap();
    v6.connect((std::net::Ipv6Addr::LOCALHOST, port)).unwrap();
    for socket in [&v4, &v6] {
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
    }
    let neighbor = UdpSocket::bind("127.0.0.1:0").unwrap();
    assert_ne!(neighbor.local_addr().unwrap().port(), port);
    neighbor.send_to(b"not selected", ("127.0.0.1", 9)).unwrap();
    for id in 0..count {
        let [hi, lo] = id.to_be_bytes();
        let query = [
            hi, lo, 1, 0, 0, 1, 0, 0, 0, 0, 0, 0, 4, b't', b'e', b's', b't', 0, 0, 1, 0, 1,
        ];
        for socket in [&v4, &v6] {
            assert_eq!(socket.send(&query).unwrap(), query.len());
            let mut reply = [0; 64];
            let length = socket.recv(&mut reply).unwrap();
            let mut expected = query;
            expected[2] = 0x81;
            expected[3] = 0x83;
            assert_eq!(&reply[..length], &expected);
        }
        thread::sleep(Duration::from_millis(pause));
    }
}

fn worker(scope: &Scope, peers: &Peers, count: u16, pause: u64, directory: &Path) -> Process {
    Process(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "production_udp_traffic_worker",
                "--nocapture",
            ])
            .env("GW_CAPTURE_WORKER_SCOPE", &scope.0)
            .env("GW_CAPTURE_WORKER_PORT", peers.port.to_string())
            .env("GW_CAPTURE_WORKER_COUNT", count.to_string())
            .env("GW_CAPTURE_WORKER_PAUSE", pause.to_string())
            .stdout(fs::File::create(directory.join("worker.log")).unwrap())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    )
}
fn wait(process: &mut Process, seconds: u64) -> std::process::ExitStatus {
    let deadline = Instant::now() + Duration::from_secs(seconds);
    loop {
        if let Some(status) = process.0.try_wait().unwrap() {
            return status;
        }
        assert!(
            Instant::now() < deadline,
            "child did not exit within deadline"
        );
        thread::sleep(Duration::from_millis(10));
    }
}
fn signal(process: &Process, signal: i32) {
    assert_eq!(unsafe { libc::kill(process.0.id() as i32, signal) }, 0);
}
fn snapshot(directory: &Path) -> CaptureSnapshot {
    serde_json::from_slice(&fs::read(directory.join("status.json")).unwrap()).unwrap()
}
fn detached(scope: &Scope) {
    let output = Command::new("bpftool")
        .args(["-j", "cgroup", "show"])
        .arg(&scope.0)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    // bpftool prints an empty line rather than [] for an unattached cgroup.
    let entries: Vec<Value> = if text.trim().is_empty() {
        Vec::new()
    } else {
        serde_json::from_str(&text).unwrap()
    };
    assert!(
        entries.is_empty(),
        "owned cgroup still has BPF attachments: {entries:?}"
    );
}
fn start(scope: &Scope, peers: &Peers, directory: &Path, args: &[&str]) -> Process {
    fs::create_dir(directory).unwrap();
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    Process(
        Command::new(env!("CARGO_BIN_EXE_gewyvern"))
            .env_clear()
            .env("HOME", directory.join("home"))
            .env("GEWY_CONFIG_HOME", directory.join("config"))
            .env("GEWY_DATA_HOME", directory.join("data"))
            .env("GEWY_STATE_HOME", directory.join("state"))
            .env("GEWY_CACHE_HOME", directory.join("cache"))
            .env("GEWY_PROTOCOL_REGISTRY_ROOT", root.join("protocols"))
            .current_dir(directory)
            .args([
                "--protocol",
                "dns",
                "--entry",
                "udp",
                "--json",
                "--capture-cgroup",
            ])
            .arg(&scope.0)
            .args(["--capture-udp-port"])
            .arg(peers.port.to_string())
            .args(["--capture-status"])
            .arg(directory.join("status.json"))
            .args(["--out"])
            .arg(directory.join("export.json"))
            .args(args)
            .stdin(Stdio::null())
            .stdout(fs::File::create(directory.join("stdout.log")).unwrap())
            .stderr(fs::File::create(directory.join("stderr.log")).unwrap())
            .spawn()
            .unwrap(),
    )
}
fn ready(process: &mut Process, directory: &Path) {
    let deadline = Instant::now() + Duration::from_secs(40);
    while !directory.join("status.json").exists() {
        assert!(
            process.0.try_wait().unwrap().is_none(),
            "capture start failed: {}",
            fs::read_to_string(directory.join("stderr.log")).unwrap()
        );
        assert!(Instant::now() < deadline, "capture not ready");
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(snapshot(directory).state, CaptureState::Running);
}
fn export(directory: &Path) -> ExportBundle {
    ExportBundle::from_json(&fs::read_to_string(directory.join("export.json")).unwrap()).unwrap()
}
fn check_export(export: &ExportBundle) {
    assert_eq!(export.ingest_trust_mode, "unverified-local");
    assert!(export.flows.iter().all(|flow| flow.process.is_none()));
    for fact in &export.facts {
        let FactKind::PacketMeta(packet) = &fact.kind else {
            panic!("non-packet evidence fabricated")
        };
        assert!([0x800, 0x86dd].contains(&packet.l3_proto));
        assert_eq!(packet.l4_proto, 17);
        assert_eq!(packet.payload_bytes.len(), 22);
        assert_eq!(
            packet.tot_len,
            if packet.l3_proto == 0x86dd { 70 } else { 50 }
        );
        assert_eq!(packet.payload_bytes.get(&12), Some(&4));
        assert!([Some(&1), Some(&0x81)].contains(&packet.payload_bytes.get(&2)));
    }
    let replay = export.replay().unwrap();
    assert_eq!(replay.facts, export.facts);
    assert_eq!(replay.capture, export.capture);
    assert_eq!(replay.attach_plan, export.attach_plan);
    assert_eq!(replay.attach_report, export.attach_report);
    assert_eq!(replay.program_flows, export.program_flows);
}

#[test]
#[ignore = "requires root, cgroup v2, clang/libbpf/bpftool and GEWY_PRODUCTION_CAPTURE_OUT=new-directory"]
fn production_capture_cli_lifecycle_and_loss() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let out = PathBuf::from(
        std::env::var_os("GEWY_PRODUCTION_CAPTURE_OUT").expect("new evidence directory"),
    );
    fs::create_dir(&out).unwrap();
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let scope = Scope(Path::new("/sys/fs/cgroup").join(format!(
        "gewyvern-production-test-{}-{nonce}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    detached(&scope);
    let peers = peers();
    let mut results = Vec::new();

    let dir = out.join("oneshot");
    let mut collector = start(&scope, &peers, &dir, &["--capture-seconds", "2"]);
    ready(&mut collector, &dir);
    let mut traffic = worker(&scope, &peers, 16, 0, &dir);
    assert!(wait(&mut traffic, 10).success());
    assert!(wait(&mut collector, 10).success());
    let s = snapshot(&dir);
    assert_eq!(s.kernel.matched, 64);
    assert_eq!(s.retained, 64);
    assert!(s.final_accounted && !s.incomplete);
    let e = export(&dir);
    assert_eq!(e.facts.len(), 64);
    check_export(&e);
    assert!(
        e.facts
            .iter()
            .any(|f| matches!(&f.kind,FactKind::PacketMeta(p) if p.l3_proto==0x86dd))
    );
    results.push(json!({"case":"oneshot_ipv4_ipv6","snapshot":s}));
    detached(&scope);

    let dir = out.join("service");
    let api = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();
    let api_text = api.to_string();
    let mut collector = start(
        &scope,
        &peers,
        &dir,
        &[
            "--serve",
            "--capture-window-ms",
            "200",
            "--api-socket",
            &api_text,
        ],
    );
    ready(&mut collector, &dir);
    let service_count: u16 = std::env::var("GEWY_CAPTURE_SERVICE_COUNT")
        .unwrap_or_else(|_| "200".into())
        .parse()
        .unwrap();
    assert!((200..=5000).contains(&service_count));
    let mut traffic = worker(&scope, &peers, service_count, 20, &dir);
    let deadline = Instant::now() + Duration::from_millis(u64::from(service_count) * 20);
    let mut windows = 0;
    while Instant::now() < deadline {
        thread::sleep(Duration::from_millis(250));
        if dir.join("export.json").exists() {
            let e = export(&dir);
            check_export(&e);
            if !e.facts.is_empty() {
                windows += 1;
            }
        }
    }
    assert!(wait(&mut traffic, 10).success());
    let mut client = TcpStream::connect(api).unwrap();
    client
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    client
        .write_all(
            b"GET /v1/latest/export.json HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n",
        )
        .unwrap();
    let mut response = String::new();
    client
        .take(4 * 1024 * 1024)
        .read_to_string(&mut response)
        .unwrap();
    assert!(response.starts_with("HTTP/1.1 200"));
    assert!(response.contains("\"capture\""));
    assert!(windows >= 3);
    let history = dir.join("state/history/api/v1");
    assert!(
        !history.exists(),
        "running windows must not archive whole API trees"
    );
    let busy = out.join("busy");
    let mut second = start(&scope, &peers, &busy, &["--capture-seconds", "1"]);
    assert!(!wait(&mut second, 10).success());
    assert!(
        fs::read_to_string(busy.join("stderr.log"))
            .unwrap()
            .contains("capture_scope_busy")
    );
    signal(&collector, libc::SIGTERM);
    assert!(wait(&mut collector, 10).success());
    let s = snapshot(&dir);
    assert_eq!(s.kernel.matched, u64::from(service_count) * 4);
    assert!(s.final_accounted && !s.incomplete);
    let history_snapshots = fs::read_dir(&history)
        .unwrap()
        .map(Result::unwrap)
        .filter(|entry| entry.file_type().unwrap().is_dir())
        .count();
    assert_eq!(history_snapshots, 1);
    results.push(
        json!({"case":"service_api_sigterm_and_exclusion","nonempty_windows":windows,"snapshot":s,
            "running_history_snapshots":0,"terminal_history_snapshots":history_snapshots}),
    );
    detached(&scope);

    let dir = out.join("kernel-loss");
    let mut collector = start(
        &scope,
        &peers,
        &dir,
        &["--serve", "--capture-ring-kib", "4"],
    );
    ready(&mut collector, &dir);
    signal(&collector, libc::SIGSTOP);
    let mut traffic = worker(&scope, &peers, 2500, 0, &dir);
    assert!(wait(&mut traffic, 20).success());
    signal(&collector, libc::SIGCONT);
    thread::sleep(Duration::from_millis(200));
    let mut traffic = worker(&scope, &peers, 4, 0, &dir);
    assert!(wait(&mut traffic, 10).success());
    signal(&collector, libc::SIGTERM);
    assert_eq!(wait(&mut collector, 10).code(), Some(3));
    let s = snapshot(&dir);
    assert_eq!(s.kernel.matched, 10016);
    assert!(s.kernel.ring_dropped > 0 && s.final_accounted && s.incomplete);
    assert_eq!(s.decode_rejected, 0);
    results.push(json!({"case":"kernel_overflow_and_resume","snapshot":s}));
    detached(&scope);

    let dir = out.join("user-loss");
    let mut collector = start(
        &scope,
        &peers,
        &dir,
        &["--capture-seconds", "2", "--capture-max-facts", "1"],
    );
    ready(&mut collector, &dir);
    let mut traffic = worker(&scope, &peers, 16, 0, &dir);
    assert!(wait(&mut traffic, 10).success());
    assert_eq!(wait(&mut collector, 10).code(), Some(3));
    let s = snapshot(&dir);
    assert_eq!(s.userspace_dropped, 63);
    assert_eq!(s.retained, 1);
    assert_eq!(s.kernel.ring_dropped, 0);
    assert!(s.final_accounted);
    results.push(json!({"case":"userspace_budget","snapshot":s}));
    detached(&scope);

    let dir = out.join("killed");
    let mut collector = start(&scope, &peers, &dir, &["--serve"]);
    ready(&mut collector, &dir);
    signal(&collector, libc::SIGKILL);
    assert!(!wait(&mut collector, 10).success());
    detached(&scope);
    results.push(json!({"case":"sigkill_detaches_unpinned_links","ok":true,"final_counters_available":false}));

    let dir = out.join("output-failure");
    let mut collector = start(
        &scope,
        &peers,
        &dir,
        &["--serve", "--capture-window-ms", "500"],
    );
    ready(&mut collector, &dir);
    fs::create_dir(dir.join("export.json")).unwrap();
    assert!(!wait(&mut collector, 10).success());
    let s = snapshot(&dir);
    assert_eq!(s.state, CaptureState::Failed);
    assert!(s.incomplete);
    results.push(json!({"case":"output_failure_detaches","snapshot":s}));
    detached(&scope);

    let dir = out.join("removed-scope");
    let mut collector = start(&scope, &peers, &dir, &["--serve"]);
    ready(&mut collector, &dir);
    fs::remove_dir(&scope.0).unwrap();
    assert!(!wait(&mut collector, 10).success());
    let s = snapshot(&dir);
    assert_eq!(s.state, CaptureState::Failed);
    assert!(s.incomplete);
    results.push(json!({"case":"scope_removal_stops_capture","snapshot":s}));
    fs::create_dir(&scope.0).unwrap();
    detached(&scope);

    let dir = out.join("bounded-service");
    let mut collector = start(
        &scope,
        &peers,
        &dir,
        &[
            "--serve",
            "--max-sessions",
            "2",
            "--capture-window-ms",
            "50",
        ],
    );
    ready(&mut collector, &dir);
    assert!(wait(&mut collector, 10).success());
    let s = snapshot(&dir);
    assert_eq!(s.state, CaptureState::Stopped);
    assert!(s.final_accounted);
    results.push(json!({"case":"bounded_service_idle_shutdown","snapshot":s}));
    detached(&scope);

    let dir = out.join("restart");
    let mut collector = start(&scope, &peers, &dir, &["--capture-seconds", "2"]);
    ready(&mut collector, &dir);
    let mut traffic = worker(&scope, &peers, 8, 0, &dir);
    assert!(wait(&mut traffic, 10).success());
    assert!(wait(&mut collector, 10).success());
    let s = snapshot(&dir);
    assert_eq!(s.kernel.matched, 32);
    assert!(s.final_accounted && !s.incomplete);
    check_export(&export(&dir));
    results.push(json!({"case":"restart_after_kill_and_output_failure","snapshot":s}));
    detached(&scope);
    fs::remove_dir(&scope.0).unwrap();
    fs::write(out.join("summary.json"),serde_json::to_vec_pretty(&json!({"schema_version":1,"ok":true,"cases":results,
        "real_production_binary":true,"validation_capture_used":false,"traffic":"native IPv4/IPv6 loopback DNS-shaped peers", "udp_port":peers.port,
        "owned_cgroup_removed":true,"no_pinned_objects":true,"docker_targets_used":false,"pid_attribution":false})).unwrap()).unwrap();
}
