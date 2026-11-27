use super::http_status::{start_observer, until};
use super::*;
use std::net::Ipv4Addr;
#[path = "redis_transactions.rs"]
mod tx;

fn bulk(body: &[u8]) -> Vec<u8> {
    let mut v = format!("${}\r\n", body.len()).into_bytes();
    v.extend_from_slice(body);
    v.extend_from_slice(b"\r\n");
    v
}
fn command(args: &[&[u8]]) -> Vec<u8> {
    let mut v = format!("*{}\r\n", args.len()).into_bytes();
    for arg in args {
        v.extend(bulk(arg));
    }
    v
}
const DECOY: &[u8] = b"-ERR not an error";
const BINARY: &[u8] = b"\x00\xff\r\n-ERR decoy\r\n$-1\r\n";
const LARGE: usize = 96 * 1024;

#[test]
#[ignore = "internal client for scoped Redis RESP qualification"]
fn client() {
    let scope = PathBuf::from(std::env::var_os("GEWY_REDIS_SCOPE").unwrap());
    assert_eq!(scope.parent(), Some(Path::new("/sys/fs/cgroup")));
    assert!(
        scope
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .starts_with("gewyvern-production-test-redis-")
    );
    fs::write(scope.join("cgroup.procs"), std::process::id().to_string()).unwrap();
    let port: u16 = std::env::var("GEWY_REDIS_PORT").unwrap().parse().unwrap();
    let token = std::env::var("GEWY_REDIS_TOKEN").unwrap();
    let case = std::env::var("GEWY_REDIS_CASE").unwrap();
    let dir = PathBuf::from(std::env::var_os("GEWY_REDIS_DIR").unwrap());
    let connect = || TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    let pre = (case == "preexisting").then(connect);
    fs::write(dir.join("ready"), []).unwrap();
    until(|| dir.join("go").is_file());
    let mut sock = pre.unwrap_or_else(connect);
    sock.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
    sock.set_write_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut req = command(&[b"AUTH", token.as_bytes()]);
    let mut expected = b"+OK\r\n".to_vec();
    match case.as_str() {
        "pipeline" => {
            let large: Vec<u8> = BINARY.iter().copied().cycle().take(LARGE).collect();
            let mut array = b"*3\r\n".to_vec();
            array.extend(bulk(DECOY));
            array.extend(b"$-1\r\n");
            array.extend(bulk(BINARY));
            let cases: &[(&[&[u8]], Vec<u8>)] = &[
                (&[b"SET", b"k", DECOY], b"+OK\r\n".to_vec()),
                (&[b"GET", b"k"], bulk(DECOY)),
                (&[b"SET", b"bin", BINARY], b"+OK\r\n".to_vec()),
                (&[b"GET", b"bin"], bulk(BINARY)),
                (&[b"GET", b"missing"], b"$-1\r\n".to_vec()),
                (&[b"ECHO", b""], bulk(b"")),
                (
                    &[b"LPUSH", b"k", b"x"],
                    b"-WRONGTYPE Operation against a key holding the wrong kind of value\r\n"
                        .to_vec(),
                ),
                (
                    &[b"ECHO"],
                    b"-ERR wrong number of arguments for 'echo' command\r\n".to_vec(),
                ),
                (&[b"EXISTS", b"k"], b":1\r\n".to_vec()),
                (&[b"MGET", b"k", b"missing", b"bin"], array),
                (&[b"ECHO", &large], bulk(&large)),
                (&[b"PING"], b"+PONG\r\n".to_vec()),
            ];
            for (args, reply) in cases {
                req.extend(command(args));
                expected.extend(reply);
            }
        }
        "overflow" => {
            for _ in 0..32 {
                req.extend(command(&[b"PING"]));
                expected.extend(b"+PONG\r\n");
            }
        }
        "transaction" => tx::mixed(&mut req, &mut expected),
        "watch" => tx::watch(&mut sock, &dir, &mut req, &mut expected),
        "depth" => tx::too_deep(&mut req, &mut expected),
        "preexisting" => {
            req = command(&[b"PING"]);
            expected = b"-NOAUTH Authentication required.\r\n".to_vec();
        }
        _ => panic!("unknown scenario"),
    }
    tx::exchange(&mut sock, &req, &expected);
}

#[test]
#[ignore = "Linux root/BPF and redis-server; GEWY_REDIS_RESP_OUT=new-directory"]
fn production_cli_redis_resp() {
    let out = PathBuf::from(std::env::var_os("GEWY_REDIS_RESP_OUT").unwrap());
    fs::create_dir(&out).unwrap();
    let addr = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap();
    let token = fs::read_to_string("/proc/sys/kernel/random/uuid").unwrap();
    let token = token.trim();
    let mut server = Process(
        Command::new("/usr/bin/setpriv")
            .args([
                "--reuid=65534",
                "--regid=65534",
                "--clear-groups",
                "--no-new-privs",
                "/usr/bin/redis-server",
                "-",
            ])
            .env_clear()
            .current_dir("/tmp")
            .stdin(Stdio::piped())
            .stdout(fs::File::create(out.join("redis.log")).unwrap())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap(),
    );
    writeln!(server.0.stdin.take().unwrap(),"bind 127.0.0.1\nprotected-mode yes\nport {}\nsave \"\"\nappendonly no\nrequirepass {token}",addr.port()).unwrap();
    until(|| {
        assert!(server.0.try_wait().unwrap().is_none());
        TcpStream::connect(addr).is_ok()
    });
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-redis-{}",
        std::process::id()
    )));
    fs::create_dir(&scope.0).unwrap();
    for case in [
        "pipeline",
        "overflow",
        "transaction",
        "watch",
        "depth",
        "preexisting",
    ] {
        let dir = out.join(case);
        fs::create_dir(&dir).unwrap();
        let mut worker = Process(
            Command::new(std::env::current_exe().unwrap())
                .args(["--exact", "redis_resp::client", "--ignored", "--nocapture"])
                .env("GEWY_REDIS_SCOPE", &scope.0)
                .env("GEWY_REDIS_PORT", addr.port().to_string())
                .env("GEWY_REDIS_TOKEN", token)
                .env("GEWY_REDIS_CASE", case)
                .env("GEWY_REDIS_DIR", &dir)
                .spawn()
                .unwrap(),
        );
        until(|| dir.join("ready").is_file());
        let mut capture = start_observer("capture-redis-resp", &dir, &scope, addr.port());
        // A real refusal outside the selected cgroup must not add a row or an error.
        let mut outside = TcpStream::connect(addr).unwrap();
        outside
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        outside.write_all(&command(&[b"PING"])).unwrap();
        let refusal = b"-NOAUTH Authentication required.\r\n";
        let mut got = vec![0; refusal.len()];
        outside.read_exact(&mut got).unwrap();
        assert_eq!(got, refusal);
        fs::write(dir.join("go"), []).unwrap();
        if case == "watch" {
            until(|| dir.join("watch-active").is_file());
            tx::invalidate_watch(&mut outside, token);
            fs::write(dir.join("watch-invalidated"), []).unwrap();
        }
        assert!(wait(&mut worker, 10).success());
        signal(&capture, libc::SIGTERM);
        let status = wait(&mut capture, 10);
        detached(&scope);
        let r: Value = serde_json::from_slice(&fs::read(dir.join("report.json")).unwrap()).unwrap();
        let complete = matches!(case, "pipeline" | "transaction" | "watch");
        assert_eq!(status.code(), Some(if complete { 0 } else { 3 }), "{r}");
        assert_eq!(r["incomplete"], !complete);
        if case == "preexisting" {
            assert_eq!(r["connections"], json!([]));
            assert_eq!(
                r["counters"],
                json!({"unanchored_payload_packets":1,"missing_cookie":0,"admission_failures":0,"invalid_packets":0})
            );
            continue;
        }
        assert!(
            r["counters"]
                .as_object()
                .unwrap()
                .values()
                .all(|v| v.as_u64() == Some(0))
        );
        assert_eq!(r["connections"].as_array().unwrap().len(), 1);
        let row = &r["connections"][0];
        if case == "pipeline" {
            assert_eq!(row["outcome"], "framed");
            let simple = json!({"type":"simple_string"});
            let error = json!({"type":"error"});
            let bulk = |n| json!({"type":"bulk_string","bytes":n});
            let expected = json!([simple,simple,bulk(DECOY.len()),simple,bulk(BINARY.len()),{"type":"null_bulk"},bulk(0),error,error,{"type":"integer"},{"type":"array","elements":3,"error_frames":0},bulk(LARGE),simple]);
            assert_eq!(row["replies"], expected);
            assert!(row["max_replies_per_segment"].as_u64().unwrap() > 1);
            assert!(row["server_payload_segments"].as_u64().unwrap() > 1);
        } else if complete {
            tx::check(case, row);
        } else {
            assert_eq!(
                row["outcome"],
                if case == "overflow" {
                    "limit_reached"
                } else {
                    "array_depth_exceeded"
                }
            );
            assert_eq!(row["replies"], json!([]));
        }
    }
}
