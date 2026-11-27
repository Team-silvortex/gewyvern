use super::http_status::{start_observer, until};
use super::*;
#[path = "mqtt_boundaries.rs"]
mod boundaries;
#[path = "mqtt_wire.rs"]
mod wire;
const IMAGE: &str =
    "eclipse-mosquitto@sha256:75d6662dee52c947dc9dd9c29f1d8d7ffef78ddaf38001caa09755681e8bbc98";
fn out() -> PathBuf {
    PathBuf::from(std::env::var_os("GEWY_MQ_OUT").unwrap())
}
fn net() {
    let actual = fs::read_link("/proc/self/ns/net").unwrap();
    assert_eq!(
        actual,
        PathBuf::from(std::env::var_os("GEWY_MQ_NET").unwrap())
    );
    assert_ne!(actual, fs::read_link("/proc/1/ns/net").unwrap());
}
#[test]
#[ignore = "Linux root/BPF, local pinned Mosquitto image; GEWY_MQ_OUT=fresh-directory"]
fn production_cli_mqtt() {
    let dir = out();
    fs::create_dir(&dir).unwrap();
    private_peer::run(
        &dir,
        &[
            "--user=1883:1883",
            "--memory=64m",
            "--pids-limit=32",
            "--entrypoint",
            "/usr/sbin/mosquitto",
        ],
        &[IMAGE, "-p", "1883"],
        &[],
        "GEWY_MQ_NET",
        "mqtt::isolated",
    );
}
#[test]
#[ignore = "internal scoped MQTT client"]
fn client() {
    net();
    let cg = PathBuf::from(std::env::var_os("GEWY_MQ_SCOPE").unwrap());
    assert_eq!(cg.parent(), Some(Path::new("/sys/fs/cgroup")));
    assert!(
        cg.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("gewyvern-production-test-mqtt-")
    );
    fs::write(cg.join("cgroup.procs"), std::process::id().to_string()).unwrap();
    let case = std::env::var("GEWY_MQ_CASE").unwrap();
    let dir = out().join(&case);
    let pre = (case == "preexisting").then(|| {
        let mut w = wire::Wire::open();
        w.connect(b"gewy-old", true, false, 0);
        w
    });
    fs::write(dir.join("ready"), []).unwrap();
    until(|| dir.join("go").is_file());
    if case.starts_with("control-") {
        boundaries::receive(
            std::env::var("GEWY_MQ_PORT").unwrap().parse().unwrap(),
            &case,
        );
    } else {
        fs::write(
            dir.join("client.json"),
            serde_json::to_vec(&wire::scenario(&case, pre)).unwrap(),
        )
        .unwrap();
    }
}
#[test]
#[ignore = "internal coordinator in offline broker netns"]
fn isolated() {
    net();
    until(|| TcpStream::connect(("127.0.0.1", 1883)).is_ok());
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-mqtt-{}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    for case in [
        "resume",
        "denied",
        "limit32",
        "overflow33",
        "preexisting",
        "control-split",
        "control-truncated",
        "control-zero-id",
        "control-length",
        "control-session",
        "control-flags",
        "control-too-large",
    ] {
        let dir = out().join(case);
        fs::create_dir(&dir).unwrap();
        let peer = case
            .starts_with("control-")
            .then(|| TcpListener::bind(("127.0.0.1", 0)).unwrap());
        let port = peer
            .as_ref()
            .map_or(1883, |p| p.local_addr().unwrap().port());
        let mut worker = Process(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "mqtt::client", "--ignored", "--nocapture"])
                .env("GEWY_MQ_CASE", case)
                .env("GEWY_MQ_PORT", port.to_string())
                .env("GEWY_MQ_SCOPE", &scope.0)
                .spawn()
                .unwrap(),
        );
        until(|| dir.join("ready").is_file());
        let mut capture = start_observer("capture-mqtt", &dir, &scope, port);
        if peer.is_none() {
            wire::neighbor();
        }
        fs::write(dir.join("go"), []).unwrap();
        if let Some(l) = peer {
            boundaries::send(l, case);
        }
        assert!(wait(&mut worker, 12).success());
        signal(&capture, libc::SIGTERM);
        let status = wait(&mut capture, 10);
        detached(&scope);
        let r: Value = serde_json::from_slice(&fs::read(dir.join("report.json")).unwrap()).unwrap();
        let complete = matches!(case, "resume" | "denied" | "limit32" | "control-split");
        assert_eq!(r["incomplete"], !complete, "{case}: {r}");
        assert_eq!(status.code(), Some(if complete { 0 } else { 3 }));
        assert_eq!(
            r["counters"],
            json!({"invalid_packets":0,"missing_cookie":0,"admission_failures":0,"unanchored_payload_packets":u32::from(case=="preexisting")})
        );
        let rows = r["connections"].as_array().unwrap();
        if case == "preexisting" {
            assert!(rows.is_empty());
        } else if case.starts_with("control-") {
            assert_eq!(rows.len(), 1);
            boundaries::check(&rows[0], case);
        } else {
            let oracle: Vec<Value> =
                serde_json::from_slice(&fs::read(dir.join("client.json")).unwrap()).unwrap();
            assert_eq!(rows.len(), if case == "resume" { 5 } else { 1 });
            assert_eq!(rows.len(), oracle.len());
            for row in rows {
                let client = oracle
                    .iter()
                    .find(|c| c["port"] == row["local_port"])
                    .unwrap();
                if complete {
                    assert_eq!(row["outcome"], "framed");
                    assert_eq!(row["events"], client["events"]);
                } else {
                    assert_eq!(row["outcome"], "limit_reached");
                    assert_eq!(row["events"], json!([]));
                }
            }
        }
    }
}
