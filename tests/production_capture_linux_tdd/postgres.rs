use super::http_status::{start_observer, until};
use super::*;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt, chown};

fn bin(tool: &str) -> PathBuf {
    let version = std::env::var("GEWY_PG_MAJOR").unwrap_or_else(|_| "16".into());
    assert!(["16", "18"].contains(&version.as_str()));
    Path::new("/usr/lib/postgresql")
        .join(version)
        .join("bin")
        .join(tool)
}
fn unprivileged(tool: &str) -> Command {
    let mut cmd = Command::new("/usr/bin/setpriv");
    cmd.args([
        "--reuid=65534",
        "--regid=65534",
        "--clear-groups",
        "--no-new-privs",
    ])
    .arg(bin(tool))
    .env_clear()
    .env("LC_ALL", "C");
    cmd
}
struct ClusterDir(PathBuf);
impl Drop for ClusterDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn cluster(out: &Path, port: u16, password: &str) -> (ClusterDir, Process) {
    let path = PathBuf::from(format!(
        "/tmp/gewy-pg-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::DirBuilder::new().mode(0o700).create(&path).unwrap();
    let dir = ClusterDir(path);
    chown(&dir.0, Some(65534), Some(65534)).unwrap();
    let data = dir.0.join("data");
    let pw = dir.0.join("init-password");
    writeln!(
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .mode(0o600)
            .open(&pw)
            .unwrap(),
        "{password}"
    )
    .unwrap();
    chown(&pw, Some(65534), Some(65534)).unwrap();
    let mut init = Process(
        unprivileged("initdb")
            .current_dir(&dir.0)
            .arg("-D")
            .arg(&data)
            .arg("--pwfile")
            .arg(&pw)
            .args([
                "-U",
                "gewy_test",
                "--auth-host=scram-sha-256",
                "--auth-local=reject",
                "--no-sync",
                "--no-locale",
                "-E",
                "UTF8",
            ])
            .stdin(Stdio::null())
            .stdout(fs::File::create(out.join("init.log")).unwrap())
            .spawn()
            .unwrap(),
    );
    assert!(wait(&mut init, 30).success());
    fs::remove_file(pw).unwrap();
    let mut pg = Process(
        unprivileged("postgres")
            .current_dir(&dir.0)
            .arg("-D")
            .arg(&data)
            .args(["-h", "127.0.0.1", "-p", &port.to_string(), "-k"])
            .arg(&dir.0)
            .args(["-c", "ssl=off"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(out.join("postgres.log")).unwrap())
            .spawn()
            .unwrap(),
    );
    until(|| {
        assert!(
            pg.0.try_wait().unwrap().is_none(),
            "temporary PostgreSQL exited"
        );
        fs::read_to_string(data.join("postmaster.pid"))
            .is_ok_and(|s| s.lines().last().map(str::trim) == Some("ready"))
    });
    (dir, pg)
}

fn query(port: u16, pw: &str, sql: &str) -> std::process::Output {
    let mut child = unprivileged("psql")
        .current_dir("/tmp")
        .env("PGPASSWORD", pw)
        .env("PGSSLMODE", "disable")
        .env("PGGSSENCMODE", "disable")
        .env("PGCONNECT_TIMEOUT", "3")
        .args([
            "-XqAtw",
            "-h",
            "127.0.0.1",
            "-p",
            &port.to_string(),
            "-U",
            "gewy_test",
            "-d",
            "postgres",
            "-v",
            "VERBOSITY=verbose",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(sql.as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}
fn denied(port: u16) {
    let out = query(port, "not-the-fixture-password", "");
    assert!(!out.status.success());
    assert!(String::from_utf8_lossy(&out.stderr).contains("password authentication failed"));
}
const DECOY: &str = "E28P01Z25P02";
#[test]
#[ignore = "internal scoped psql worker"]
fn client() {
    let cg = PathBuf::from(std::env::var_os("GEWY_PG_SCOPE").unwrap());
    assert_eq!(cg.parent(), Some(Path::new("/sys/fs/cgroup")));
    assert!(
        cg.file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("gewyvern-production-test-pg-")
    );
    fs::write(cg.join("cgroup.procs"), std::process::id().to_string()).unwrap();
    let port = std::env::var("GEWY_PG_PORT").unwrap().parse().unwrap();
    let pw = std::env::var("GEWY_PG_PASSWORD").unwrap();
    match std::env::var("GEWY_PG_CASE").unwrap().as_str() {
        "denied" => denied(port),
        "transaction" => {
            let sql = format!(
                "BEGIN;\nSELECT repeat('{DECOY}',8192);\nSELECT 1/0;\nSELECT 7;\nROLLBACK;\nSELECT 45;\nSELECT '28P01'::integer;\n"
            );
            let out = query(port, &pw, &sql);
            let err = String::from_utf8(out.stderr).unwrap();
            assert!(out.status.success(), "{err}");
            assert_eq!(
                out.stdout,
                format!("{}\n45\n", DECOY.repeat(8192)).as_bytes()
            );
            for code in ["22012", "25P02", "22P02"] {
                assert!(err.contains(code), "{err}");
            }
        }
        "overflow" => {
            let out = query(port, &pw, "SELECT generate_series(1,33);\n");
            assert!(out.status.success());
            assert_eq!(
                String::from_utf8(out.stdout).unwrap(),
                (1..=33).map(|n| format!("{n}\n")).collect::<String>()
            );
        }
        "copy" => {
            let out = query(port, &pw, "COPY (SELECT 1) TO STDOUT;\n");
            assert!(out.status.success());
            assert_eq!(out.stdout, b"1\n");
        }
        _ => panic!("unknown case"),
    }
}

fn check(case: &str, r: &Value) {
    let rows = r["connections"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{r}");
    let row = &rows[0];
    let ready = |s: &str| json!({"type":"ready","state":s});
    let err = |phase: &str, code: &str| json!({"type":"error_response","phase":phase,"sqlstate_bucket":code});
    let d = json!({"type":"data_row"});
    let after = "after_first_ready";
    let (outcome, events) = match case {
        "transaction" => (
            "framed",
            json!([
                ready("I"),
                ready("T"),
                d,
                ready("T"),
                err(after, "22012"),
                ready("E"),
                err(after, "25P02"),
                ready("E"),
                ready("I"),
                d,
                ready("I"),
                err(after, "other"),
                ready("I")
            ]),
        ),
        "denied" => ("framed", json!([err("before_first_ready", "28P01")])),
        "overflow" => ("limit_reached", json!([])),
        "copy" => ("unsupported_type", json!([])),
        _ => unreachable!(),
    };
    assert_eq!(row["outcome"], outcome, "{r}");
    assert_eq!(row["events"], events);
    assert_eq!(r["incomplete"], outcome != "framed");
    assert!(
        r["counters"]
            .as_object()
            .unwrap()
            .values()
            .all(|v| v.as_u64() == Some(0))
    );
}

#[test]
#[ignore = "Linux root/BPF, installed PostgreSQL; GEWY_PG_OUT=fresh-directory"]
fn production_cli_postgres() {
    let out = PathBuf::from(std::env::var_os("GEWY_PG_OUT").unwrap());
    fs::create_dir(&out).unwrap();
    let port = TcpListener::bind(("127.0.0.1", 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let password = fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
    let (dir, mut pg) = cluster(&out, port, password.trim());
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-pg-{}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    for case in ["transaction", "denied", "overflow", "copy"] {
        let dst = out.join(case);
        fs::create_dir(&dst).unwrap();
        let mut capture = start_observer("capture-postgres", &dst, &scope, port);
        denied(port); // same-target control outside the client cgroup
        let mut worker = Process(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "postgres::client", "--ignored", "--nocapture"])
                .env("GEWY_PG_PORT", port.to_string())
                .env("GEWY_PG_SCOPE", &scope.0)
                .env("GEWY_PG_PASSWORD", password.trim())
                .env("GEWY_PG_CASE", case)
                .spawn()
                .unwrap(),
        );
        assert!(wait(&mut worker, 10).success());
        signal(&capture, libc::SIGTERM);
        let status = wait(&mut capture, 10);
        detached(&scope);
        let r: Value = serde_json::from_slice(&fs::read(dst.join("report.json")).unwrap()).unwrap();
        check(case, &r);
        assert_eq!(
            status.code(),
            Some(if matches!(case, "transaction" | "denied") {
                0
            } else {
                3
            })
        );
    }
    signal(&pg, libc::SIGINT);
    assert!(wait(&mut pg, 10).success());
    let path = dir.0.clone();
    drop(dir);
    assert!(!path.exists());
}
