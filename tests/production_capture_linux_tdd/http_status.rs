use super::*;
use std::net::Ipv4Addr;
#[path = "http_status_caddy.rs"]
mod caddy;

const CASES: [(&[u8], &[u8]); 3] = [
    (b"HTTP/1.", b"1 403 Forbidden\r\nContent-Length: 0\r\n\r\n"),
    (
        b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\n",
        b"HTTP/1.1 500 ",
    ),
    (
        b"HTTP/1.0 502 Bad Gateway\r\nContent-Length: 0\r\n\r\n",
        b"",
    ),
];

pub(super) fn until(mut predicate: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(10);
    while !predicate() {
        assert!(Instant::now() < deadline, "timed out awaiting peer");
        thread::sleep(Duration::from_millis(5));
    }
}

#[test]
#[ignore = "internal HTTP status client; started by the host test"]
fn client() {
    let (dir, port) = client_scope();
    let mut ports = Vec::new();
    for (i, (first, tail)) in CASES.iter().enumerate() {
        let mut sock = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        ports.push(sock.local_addr().unwrap().port());
        sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        sock.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut chunk = vec![0; first.len()];
        sock.read_exact(&mut chunk).unwrap();
        assert_eq!(&chunk, first);
        fs::write(dir.join(format!("read-{i}")), []).unwrap();
        chunk.clear();
        sock.read_to_end(&mut chunk).unwrap();
        assert_eq!(&chunk, tail);
    }
    fs::write(dir.join("ports.json"), serde_json::to_vec(&ports).unwrap()).unwrap();
}

fn client_scope() -> (PathBuf, u16) {
    let dir = PathBuf::from(std::env::var_os("GEWY_HTTP_DIR").unwrap());
    let cg = PathBuf::from(std::env::var_os("GEWY_HTTP_SCOPE").unwrap());
    assert_eq!(cg.parent().unwrap(), Path::new("/sys/fs/cgroup"));
    assert!(
        cg.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("gewyvern-production-test-http-")
    );
    fs::write(cg.join("cgroup.procs"), std::process::id().to_string()).unwrap();
    let port: u16 = std::env::var("GEWY_HTTP_PORT").unwrap().parse().unwrap();
    (dir, port)
}

#[test]
#[ignore = "Linux root/BPF; GEWY_HTTP_STATUS_OUT=new-directory"]
fn production_cli_http_status() {
    let dir = PathBuf::from(std::env::var_os("GEWY_HTTP_STATUS_OUT").unwrap());
    fs::create_dir(&dir).unwrap();
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-http-{}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let mut capture = start_capture(&dir, &scope, port);
    let mut outside = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
    listener
        .accept()
        .unwrap()
        .0
        .write_all(b"HTTP/1.1 503 Unavailable\r\n\r\n")
        .unwrap();
    let mut response = String::new();
    outside.read_to_string(&mut response).unwrap();
    assert!(response.contains("503"));
    let mut worker = Process(
        Command::new(std::env::current_exe().unwrap())
            .args(["--exact", "http_status::client", "--ignored", "--nocapture"])
            .env("GEWY_HTTP_DIR", &dir)
            .env("GEWY_HTTP_SCOPE", &scope.0)
            .env("GEWY_HTTP_PORT", port.to_string())
            .spawn()
            .unwrap(),
    );
    listener.set_nonblocking(true).unwrap();
    for (i, (first, tail)) in CASES.iter().enumerate() {
        let mut accepted = None;
        until(|| {
            accepted = listener.accept().ok();
            accepted.is_some()
        });
        let mut s = accepted.unwrap().0;
        s.set_nodelay(true).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        let req = b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n";
        let mut buf = vec![0; req.len()];
        s.read_exact(&mut buf).unwrap();
        assert_eq!(&buf, req);
        s.write_all(first).unwrap();
        until(|| dir.join(format!("read-{i}")).is_file());
        s.write_all(tail).unwrap();
    }
    assert!(wait(&mut worker, 10).success());
    signal(&capture, libc::SIGTERM);
    assert_eq!(wait(&mut capture, 10).code(), Some(0));
    detached(&scope);
    let r: Value = serde_json::from_slice(&fs::read(dir.join("report.json")).unwrap()).unwrap();
    let ports: Vec<u16> =
        serde_json::from_slice(&fs::read(dir.join("ports.json")).unwrap()).unwrap();
    let rows = r["connections"].as_array().unwrap();
    assert_eq!(rows.len(), 3);
    assert_eq!(ports.len(), 3);
    assert_eq!(r["incomplete"], false);
    assert!(
        r["counters"]
            .as_object()
            .unwrap()
            .values()
            .all(|v| v.as_u64() == Some(0))
    );
    for (i, (p, code)) in ports.iter().zip([Some(403), Some(200), None]).enumerate() {
        let row = rows.iter().find(|r| r["local_port"] == *p).unwrap();
        assert_eq!(row["status"], json!(code));
        assert_eq!(
            row["outcome"],
            if code.is_some() {
                "verified_prefix"
            } else {
                "not_http11"
            }
        );
        if i == 0 {
            assert!(row["prefix_segments"].as_u64().unwrap() >= 2);
        }
        if i == 1 {
            assert_eq!(row["prefix_segments"], 1);
        }
    }
}

fn start_capture(dir: &Path, scope: &Scope, port: u16) -> Process {
    start_observer("capture-http-status", dir, scope, port)
}

pub(super) fn start_observer(command: &str, dir: &Path, scope: &Scope, port: u16) -> Process {
    let log = dir.join("stderr");
    let mut capture = Process(
        Command::new(env!("CARGO_BIN_EXE_gewyvern"))
            .args([command, "--capture-cgroup"])
            .arg(&scope.0)
            .args([
                "--capture-tcp-port",
                &port.to_string(),
                "--capture-seconds",
                "30",
                "--json",
            ])
            .stderr(fs::File::create(&log).unwrap())
            .stdout(fs::File::create(dir.join("report.json")).unwrap())
            .spawn()
            .unwrap(),
    );
    until(|| {
        let msg = fs::read_to_string(&log).unwrap();
        assert!(capture.0.try_wait().unwrap().is_none(), "{msg}");
        msg.contains(&format!("{command}: ready"))
    });
    capture
}
