use super::*;

const BODY: &str = "HTTP/1.1 500 body, not a status";

#[test]
#[ignore = "internal client for real Caddy loopback checks"]
fn client() {
    let (dir, port) = client_scope();
    let bounded = std::env::var_os("GEWY_HTTP_BOUNDED").is_some();
    let connect = || TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    let pre = bounded.then(connect);
    fs::write(dir.join("client-ready"), []).unwrap();
    until(|| dir.join("go").is_file());
    let mut ports = Vec::new();
    for mut sock in pre
        .into_iter()
        .chain((0..if bounded { 65 } else { 2 }).map(|_| connect()))
    {
        ports.push(sock.local_addr().unwrap().port());
        sock.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
        sock.write_all(b"GET / HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n")
            .unwrap();
        let mut reply = String::new();
        sock.take(65536).read_to_string(&mut reply).unwrap();
        let (headers, body) = reply.split_once("\r\n\r\n").unwrap();
        assert!(headers.starts_with("HTTP/1.1 201 "));
        assert_eq!(body, BODY);
    }
    fs::write(dir.join("ports.json"), serde_json::to_vec(&ports).unwrap()).unwrap();
}

#[test]
#[ignore = "Linux root/BPF and Caddy; GEWY_HTTP_CADDY_OUT=new-directory"]
fn production_cli_caddy() {
    let out = PathBuf::from(std::env::var_os("GEWY_HTTP_CADDY_OUT").unwrap());
    fs::create_dir(&out).unwrap();
    let addr = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap();
    let mut server = Process(
        Command::new("/usr/bin/setpriv")
            .args([
                "--reuid=65534",
                "--regid=65534",
                "--clear-groups",
                "--no-new-privs",
                "/usr/bin/caddy",
                "respond",
                "--listen",
                &addr.to_string(),
                "--status",
                "201",
                "--body",
                BODY,
            ])
            .env_clear()
            .current_dir("/tmp")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(out.join("server.log")).unwrap())
            .spawn()
            .unwrap(),
    );
    until(|| {
        assert!(server.0.try_wait().unwrap().is_none());
        TcpStream::connect(addr).is_ok()
    });
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-http-caddy-{}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    for bounded in [false, true] {
        let dir = out.join(if bounded { "bounded" } else { "complete" });
        fs::create_dir(&dir).unwrap();
        let mut cmd = Command::new(std::env::current_exe().unwrap());
        cmd.args([
            "--exact",
            "http_status::caddy::client",
            "--ignored",
            "--nocapture",
        ])
        .env("GEWY_HTTP_DIR", &dir)
        .env("GEWY_HTTP_SCOPE", &scope.0)
        .env("GEWY_HTTP_PORT", addr.port().to_string())
        .env_remove("GEWY_HTTP_BOUNDED");
        if bounded {
            cmd.env("GEWY_HTTP_BOUNDED", "1");
        }
        let mut worker = Process(cmd.spawn().unwrap());
        until(|| dir.join("client-ready").is_file());
        let mut capture = start_capture(&dir, &scope, addr.port());
        fs::write(dir.join("go"), []).unwrap();
        assert!(wait(&mut worker, 10).success());
        signal(&capture, libc::SIGTERM);
        assert_eq!(
            wait(&mut capture, 10).code(),
            Some(if bounded { 3 } else { 0 })
        );
        detached(&scope);
        let r: Value = serde_json::from_slice(&fs::read(dir.join("report.json")).unwrap()).unwrap();
        let ports: Vec<u16> =
            serde_json::from_slice(&fs::read(dir.join("ports.json")).unwrap()).unwrap();
        let rows = r["connections"].as_array().unwrap();
        assert_eq!(r["incomplete"], bounded);
        assert_eq!(ports.len(), if bounded { 66 } else { 2 });
        assert_eq!(rows.len(), if bounded { 64 } else { 2 });
        let counters = &r["counters"];
        assert_eq!(counters["admission_failures"], u64::from(bounded));
        assert_eq!(counters["invalid_packets"], 0);
        assert_eq!(counters["missing_cookie"], 0);
        assert_eq!(
            counters["unanchored_payload_packets"].as_u64().unwrap() >= 2,
            bounded
        );
        let accepted = if bounded { &ports[1..65] } else { &ports[..] };
        let mut got: Vec<_> = rows
            .iter()
            .map(|row| {
                assert_eq!(row["status"], 201);
                assert_eq!(row["outcome"], "verified_prefix");
                row["local_port"].as_u64().unwrap() as u16
            })
            .collect();
        got.sort_unstable();
        let mut expected = accepted.to_vec();
        expected.sort_unstable();
        assert_eq!(got, expected);
    }
}
