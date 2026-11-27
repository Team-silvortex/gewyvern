use super::http_status::{start_observer, until};
use super::*;
#[path = "mysql_boundaries.rs"]
mod boundaries;
#[path = "mysql_wire.rs"]
mod wire;

const IMAGE: &str = "mysql@sha256:7dcddc01f13bab2f15cde676d44d01f61fc9f99fe7785e86196dfc07d358ae2b";
fn out() -> PathBuf {
    PathBuf::from(std::env::var_os("GEWY_MY_OUT").unwrap())
}
fn net() {
    let actual = fs::read_link("/proc/self/ns/net").unwrap();
    assert_eq!(
        actual,
        PathBuf::from(std::env::var_os("GEWY_MY_NET").unwrap())
    );
    assert_ne!(actual, fs::read_link("/proc/1/ns/net").unwrap());
}
#[test]
#[ignore = "Linux root/BPF and local pinned MySQL image; GEWY_MY_OUT=fresh-directory"]
fn production_cli_mysql() {
    let dir = out();
    fs::create_dir(&dir).unwrap();
    let password = fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
    private_peer::run(
        &dir,
        &[
            "--user=999:999",
            "--memory=1g",
            "--pids-limit=256",
            "--tmpfs",
            "/var/lib/mysql:rw,nosuid,nodev,size=512m,uid=999,gid=999",
            "--tmpfs",
            "/var/run/mysqld:rw,nosuid,nodev,size=8m,uid=999,gid=999",
            "--tmpfs",
            "/tmp:rw,nosuid,nodev,size=32m,mode=1777",
            "--env",
            "MYSQL_USER=gewy",
            "--env",
            "MYSQL_DATABASE=gewy",
            "--env",
            "MYSQL_PASSWORD",
            "--env",
            "MYSQL_RANDOM_ROOT_PASSWORD=yes",
            "--env",
            "MYSQL_ROOT_HOST=localhost",
        ],
        &[
            IMAGE,
            "mysqld",
            "--bind-address=127.0.0.1",
            "--mysqlx=0",
            "--skip-log-bin",
            "--default-authentication-plugin=mysql_native_password",
            "--performance-schema=OFF",
            "--innodb-buffer-pool-size=64M",
            "--max-connections=20",
        ],
        &[("MYSQL_PASSWORD", password.trim())],
        "GEWY_MY_NET",
        "mysql::isolated",
    );
}
#[test]
#[ignore = "internal client in the private MySQL network namespace"]
fn client() {
    net();
    let cg = PathBuf::from(std::env::var_os("GEWY_MY_SCOPE").unwrap());
    assert_eq!(cg.parent(), Some(Path::new("/sys/fs/cgroup")));
    assert!(
        cg.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("gewyvern-production-test-mysql-")
    );
    fs::write(cg.join("cgroup.procs"), std::process::id().to_string()).unwrap();
    let dir = PathBuf::from(std::env::var_os("GEWY_MY_DIR").unwrap());
    let case = std::env::var("GEWY_MY_CASE").unwrap();
    let pre = (case == "preexisting").then(wire::Client::open); // consume the greeting before attachment
    fs::write(dir.join("ready"), []).unwrap();
    until(|| dir.join("go").is_file());
    if case.starts_with("control-") {
        boundaries::receive(
            std::env::var("GEWY_MY_PORT").unwrap().parse().unwrap(),
            &case,
        );
        return;
    }
    let mut c = pre.unwrap_or_else(wire::Client::open);
    if case == "denied" {
        c.login("wrong-fixture-password", false);
    } else {
        c.login(&std::env::var("MYSQL_PASSWORD").unwrap(), true);
        c.run(&case);
    }
}
#[test]
#[ignore = "internal coordinator in owned offline namespace"]
fn isolated() {
    net();
    let start = Instant::now();
    while TcpStream::connect(("127.0.0.1", 3306)).is_err() {
        assert!(
            start.elapsed() < Duration::from_secs(60),
            "temporary MySQL did not start"
        );
        thread::sleep(Duration::from_millis(100));
    }
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-mysql-{}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    for case in [
        "mixed",
        "denied",
        "columns",
        "limit32",
        "overflow33",
        "preexisting",
        "control-split",
        "control-sequence",
        "control-marker",
        "control-no-end",
        "control-row-length",
    ] {
        let dir = out().join(case);
        fs::create_dir(&dir).unwrap();
        let peer = case
            .starts_with("control-")
            .then(|| TcpListener::bind(("127.0.0.1", 0)).unwrap());
        let port = peer
            .as_ref()
            .map_or(3306, |p| p.local_addr().unwrap().port());
        let mut worker = Process(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "mysql::client", "--ignored", "--nocapture"])
                .env("GEWY_MY_SCOPE", &scope.0)
                .env("GEWY_MY_DIR", &dir)
                .env("GEWY_MY_CASE", case)
                .env("GEWY_MY_PORT", port.to_string())
                .spawn()
                .unwrap(),
        );
        until(|| dir.join("ready").is_file());
        let mut cap = start_observer("capture-mysql", &dir, &scope, port);
        if peer.is_none() {
            wire::Client::open().login("unselected-local-client", false);
        }
        fs::write(dir.join("go"), []).unwrap();
        if let Some(peer) = peer {
            boundaries::send(peer, case);
        }
        assert!(wait(&mut worker, 10).success());
        signal(&cap, libc::SIGTERM);
        let status = wait(&mut cap, 10);
        detached(&scope);
        let r: Value = serde_json::from_slice(&fs::read(dir.join("report.json")).unwrap()).unwrap();
        let complete = matches!(case, "mixed" | "denied" | "limit32" | "control-split");
        assert_eq!(r["incomplete"], !complete, "{r}");
        assert_eq!(status.code(), Some(if complete { 0 } else { 3 }));
        for key in ["invalid_packets", "missing_cookie", "admission_failures"] {
            assert_eq!(r["counters"][key], 0);
        }
        let unanchored = r["counters"]["unanchored_payload_packets"]
            .as_u64()
            .unwrap();
        if case == "preexisting" {
            assert!(unanchored > 0);
            assert_eq!(r["connections"], json!([]));
            continue;
        }
        assert_eq!(unanchored, 0);
        assert_eq!(r["connections"].as_array().unwrap().len(), 1);
        let row = &r["connections"][0];
        if complete {
            assert_eq!(row["outcome"], "framed");
            if case == "control-split" {
                assert!(row["server_payload_segments"].as_u64().unwrap() >= 6);
                assert_eq!(
                    row["events"],
                    json!([{"type":"greeting_v10"},{"type":"auth_ok"},
                    {"type":"error","phase":"command","errno":1064,"sqlstate_bucket":"42000"},{"type":"command_ok"}])
                );
            } else {
                assert_eq!(row["events"], json!(wire::expected(case)));
            }
        } else {
            assert_eq!(row["events"], json!([]));
            assert_eq!(
                row["outcome"],
                match case {
                    "columns" => "unsupported_type",
                    "overflow33" => "limit_reached",
                    "control-sequence" => "sequence_discontinuity",
                    "control-marker" | "control-row-length" => "malformed_frame",
                    "control-no-end" => "partial_frame",
                    _ => unreachable!(),
                }
            );
        }
    }
}
