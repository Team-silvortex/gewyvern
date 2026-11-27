use super::http_status::{start_observer, until};
use super::*;
#[path = "memcached_boundaries.rs"]
mod boundaries;
#[path = "memcached_wire.rs"]
mod wire;
const IMAGE: &str =
    "memcached@sha256:084fb554292d1447bc2c68b80c2331b98834514f56c866bc21a61be3c2ee376d";
fn out_dir() -> PathBuf {
    PathBuf::from(std::env::var_os("GEWY_MC_OUT").unwrap())
}
#[test]
#[ignore = "Linux root/BPF and local pinned Memcached image; GEWY_MC_OUT=fresh-directory"]
fn production_cli_memcached() {
    let dir = out_dir();
    fs::create_dir(&dir).unwrap();
    private_peer::run(
        &dir,
        &["--memory=64m", "--pids-limit=32"],
        &[
            IMAGE,
            "memcached",
            "-l",
            "127.0.0.1",
            "-p",
            "11211",
            "-U",
            "0",
            "-B",
            "binary",
            "-t",
            "1",
            "-m",
            "16",
        ],
        &[],
        "GEWY_MC_NET",
        "memcached::isolated",
    );
}
fn connect(port: u16) -> TcpStream {
    let s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    s.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
    s
}
fn check_net() {
    let expected = PathBuf::from(std::env::var_os("GEWY_MC_NET").unwrap());
    assert_ne!(expected, fs::read_link("/proc/1/ns/net").unwrap());
    assert_eq!(expected, fs::read_link("/proc/self/ns/net").unwrap());
}
#[test]
#[ignore = "internal test peer in owned offline netns"]
fn client() {
    check_net();
    let cg = PathBuf::from(std::env::var_os("GEWY_MC_SCOPE").unwrap());
    assert_eq!(cg.parent(), Some(Path::new("/sys/fs/cgroup")));
    assert!(
        cg.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("gewyvern-production-test-mc-")
    );
    fs::write(cg.join("cgroup.procs"), std::process::id().to_string()).unwrap();
    let dir = PathBuf::from(std::env::var_os("GEWY_MC_DIR").unwrap());
    let case = std::env::var("GEWY_MC_CASE").unwrap();
    let port = std::env::var("GEWY_MC_PORT").unwrap().parse().unwrap();
    let pre = (case == "preexisting").then(|| connect(port));
    fs::write(dir.join("ready"), []).unwrap();
    until(|| dir.join("go").is_file());
    let mut s = pre.unwrap_or_else(|| connect(port));
    if case.starts_with("control-") {
        boundaries::receive(&mut s, &dir, &case);
    } else {
        wire::traffic(&mut s, &case);
    }
}
#[test]
#[ignore = "internal coordinator in owned offline netns"]
fn isolated() {
    check_net();
    until(|| TcpStream::connect(("127.0.0.1", 11211)).is_ok());
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-mc-{}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    for case in [
        "mixed",
        "limit32",
        "overflow33",
        "version",
        "preexisting",
        "control-split",
        "control-header",
        "control-body",
        "control-flags",
    ] {
        let dir = out_dir().join(case);
        fs::create_dir(&dir).unwrap();
        let listener = case
            .starts_with("control-")
            .then(|| TcpListener::bind(("127.0.0.1", 0)).unwrap());
        let port = listener
            .as_ref()
            .map_or(11211, |l| l.local_addr().unwrap().port());
        let mut worker = Process(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "memcached::client", "--ignored", "--nocapture"])
                .env("GEWY_MC_SCOPE", &scope.0)
                .env("GEWY_MC_CASE", case)
                .env("GEWY_MC_PORT", port.to_string())
                .env("GEWY_MC_DIR", &dir)
                .spawn()
                .unwrap(),
        );
        until(|| dir.join("ready").is_file());
        let mut capture = start_observer("capture-memcached", &dir, &scope, port);
        if listener.is_none() {
            wire::noop(&mut connect(port));
        } // same service, outside the client cgroup
        fs::write(dir.join("go"), []).unwrap();
        if let Some(l) = listener {
            boundaries::send(l, &dir, case);
        }
        assert!(wait(&mut worker, 10).success());
        signal(&capture, libc::SIGTERM);
        let status = wait(&mut capture, 10);
        detached(&scope);
        let r: Value = serde_json::from_slice(&fs::read(dir.join("report.json")).unwrap()).unwrap();
        let complete = matches!(case, "mixed" | "limit32" | "control-split");
        assert_eq!(r["incomplete"], !complete, "{r}");
        assert_eq!(status.code(), Some(if complete { 0 } else { 3 }));
        assert_eq!(
            r["counters"],
            json!({"admission_failures":0,"invalid_packets":0,"missing_cookie":0,"unanchored_payload_packets":u32::from(case=="preexisting")})
        );
        if case == "preexisting" {
            assert_eq!(r["connections"], json!([]));
            continue;
        }
        assert_eq!(r["connections"].as_array().unwrap().len(), 1);
        let row = &r["connections"][0];
        let (outcome, events) = match case {
            "mixed" => {
                assert!(row["server_payload_segments"].as_u64().unwrap() > 1);
                ("framed", wire::expected_mixed())
            }
            "limit32" => (
                "framed",
                vec![json!({"operation":"noop","status":0,"result":"ok"}); 32],
            ),
            "overflow33" => ("limit_reached", vec![]),
            "version" => ("unsupported_type", vec![]),
            "control-split" => {
                assert!(row["server_payload_segments"].as_u64().unwrap() >= 5);
                let noop = json!({"operation":"noop","status":0,"result":"ok"});
                (
                    "framed",
                    vec![
                        noop.clone(),
                        json!({"operation":"get","status":0,"result":"hit","value_bytes":24}),
                        noop,
                    ],
                )
            }
            "control-header" | "control-body" => ("partial_frame", vec![]),
            "control-flags" => ("malformed_frame", vec![]),
            _ => unreachable!(),
        };
        assert_eq!(row["outcome"], outcome, "{r}");
        assert_eq!(row["events"], json!(events));
    }
}
