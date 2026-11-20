//! Run on the Linux host after a fresh `linux-udp-capture --ipv6-dns` campaign.
//! This consumes captured facts, not the harness's substituted fragment registry.
#![cfg(target_os = "linux")]

use std::fs;
use std::io::Write;
use std::net::{Shutdown, TcpListener, TcpStream};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

use gewyvern::export::{ExportBundle, fact_to_json};
use gewyvern::ledger::FactKind;
use gewyvern::runtime::SessionConfig;
use serde_json::{Value, json};

struct OwnedDirectory(PathBuf);
impl Drop for OwnedDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct OwnedChild(Child);
impl Drop for OwnedChild {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

enum Endpoint {
    Unix(PathBuf),
    Tcp(std::net::SocketAddr),
}

impl Endpoint {
    fn new(kind: &str, directory: &Path) -> Self {
        if kind == "unix" {
            Self::Unix(directory.join("facts.sock"))
        } else {
            let listener = TcpListener::bind(if kind == "tcp6" {
                "[::1]:0"
            } else {
                "127.0.0.1:0"
            })
            .unwrap();
            Self::Tcp(listener.local_addr().unwrap())
        }
    }

    fn args(&self) -> [&str; 1] {
        [match self {
            Self::Unix(_) => "--unix-socket",
            Self::Tcp(_) => "--tcp-socket",
        }]
    }

    fn address(&self) -> String {
        match self {
            Self::Unix(path) => path.display().to_string(),
            Self::Tcp(addr) => addr.to_string(),
        }
    }

    fn send(&self, bytes: &[u8]) -> std::io::Result<()> {
        match self {
            Self::Unix(path) => {
                let mut stream = UnixStream::connect(path)?;
                stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                stream.write_all(bytes)?;
                stream.shutdown(Shutdown::Write)
            }
            Self::Tcp(addr) => {
                let mut stream = TcpStream::connect_timeout(addr, Duration::from_millis(100))?;
                stream.set_write_timeout(Some(Duration::from_secs(2)))?;
                stream.write_all(bytes)?;
                stream.shutdown(Shutdown::Write)
            }
        }
    }
}

fn cli_session(
    endpoint: &Endpoint,
    rules: &Path,
    directory: &Path,
    bytes: &[u8],
    success: bool,
) -> Option<ExportBundle> {
    fs::create_dir(directory).unwrap();
    let output_path = directory.join("export.json");
    let stderr = directory.join("stderr.json");
    let mut command = Command::new(env!("CARGO_BIN_EXE_gewyvern"));
    command
        .env_clear()
        .env("HOME", directory.join("home"))
        .env("LANG", "C")
        .env("GEWY_CONFIG_HOME", directory.join("config"))
        .env("GEWY_DATA_HOME", directory.join("data"))
        .env("GEWY_STATE_HOME", directory.join("state"))
        .env("GEWY_CACHE_HOME", directory.join("cache"))
        .env(
            "GEWY_PROTOCOL_REGISTRY_ROOT",
            Path::new(env!("CARGO_MANIFEST_DIR")).join("protocols"),
        )
        .current_dir(directory)
        .args(["--dsl"])
        .arg(rules)
        .args(endpoint.args())
        .arg(endpoint.address())
        .args(["--ingest-mode", "local-advisory", "--json", "--out"])
        .arg(&output_path)
        .stdin(Stdio::null())
        .stdout(fs::File::create(directory.join("stdout.log")).unwrap())
        .stderr(fs::File::create(&stderr).unwrap());
    let mut child = OwnedChild(command.spawn().unwrap());
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        match endpoint.send(bytes) {
            Ok(()) => break,
            Err(error)
                if matches!(
                    error.kind(),
                    std::io::ErrorKind::ConnectionRefused | std::io::ErrorKind::NotFound
                ) => {}
            Err(error) => panic!("ingest write failed: {error}"),
        }
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "CLI exited before accepting: {}",
            fs::read_to_string(&stderr).unwrap()
        );
        assert!(
            Instant::now() < deadline,
            "CLI listener did not become ready"
        );
        thread::sleep(Duration::from_millis(10));
    }
    let status = loop {
        if let Some(status) = child.0.try_wait().unwrap() {
            break status;
        }
        assert!(Instant::now() < deadline, "CLI did not finish after EOF");
        thread::sleep(Duration::from_millis(10));
    };
    let error = fs::read_to_string(&stderr).unwrap();
    assert_eq!(status.success(), success, "{status}: {error}");
    if !success {
        let error: Value = serde_json::from_str(error.trim()).unwrap();
        assert!(error.to_string().contains("socket_session_run_failed"));
        assert!(!output_path.exists());
        if let Endpoint::Unix(path) = endpoint {
            assert!(!path.exists(), "failed ingest left a stale Unix socket");
        }
        return None;
    }
    assert!(error.trim().is_empty(), "{error}");
    Some(ExportBundle::from_json(&fs::read_to_string(output_path).unwrap()).unwrap())
}

#[test]
#[ignore = "requires fresh Linux IPv6 DNS capture; set GEWY_IPV6_DNS_CAPTURE_DIR and GEWY_CAPTURE_INGEST_OUT"]
fn linux_ipv6_capture_reaches_unmodified_cli_over_unix_ipv4_and_ipv6() {
    let capture = PathBuf::from(
        std::env::var_os("GEWY_IPV6_DNS_CAPTURE_DIR").expect("fresh capture directory"),
    );
    let evidence =
        PathBuf::from(std::env::var_os("GEWY_CAPTURE_INGEST_OUT").expect("new evidence directory"));
    fs::create_dir(&evidence).expect("evidence directory must not already exist");
    let summary: Value =
        serde_json::from_slice(&fs::read(capture.join("summary.json")).unwrap()).unwrap();
    assert_eq!(summary["ok"], true);
    assert_eq!(summary["ip_version"], 6);
    assert_eq!(summary["ipv6_only"], true);
    assert_eq!(summary["egress_counts"], json!({"total":9,"dropped":0}));
    assert_eq!(summary["ingress_counts"], json!({"total":10,"dropped":0}));
    assert_eq!(summary["owned_resources_removed"], true);
    let nonce = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let sockets =
        OwnedDirectory(std::env::temp_dir().join(format!("gw6-{}-{nonce}", std::process::id())));
    fs::create_dir(&sockets.0).unwrap();
    let mut results = Vec::new();
    for (index, package) in ["udp", "error"].into_iter().enumerate() {
        let captured = ExportBundle::from_json(
            &fs::read_to_string(capture.join(format!("package-{index}-export.json"))).unwrap(),
        )
        .unwrap();
        assert_eq!(captured.facts.len(), 19);
        assert!(captured.facts.iter().all(|fact| matches!(&fact.kind, FactKind::PacketMeta(p) if p.l3_proto == 0x86dd && p.l4_proto == 17)));
        let rules = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join(format!("protocols/dns/{package}/main.gewy"));
        let config = SessionConfig::for_binding(
            gewyvern::dsl::compile_str(&fs::read_to_string(&rules).unwrap()).unwrap(),
        )
        .unwrap();
        let default_plan = config
            .registry
            .plan(config.template.fragment_set.iter().map(String::as_str))
            .unwrap();
        let bytes: Vec<_> = captured
            .facts
            .iter()
            .flat_map(|fact| format!("{}\n", fact_to_json(fact)).into_bytes())
            .collect();
        for kind in ["unix", "tcp4", "tcp6"] {
            let endpoint = Endpoint::new(kind, &sockets.0);
            // A rejected input must not poison the next process using this endpoint.
            cli_session(
                &endpoint,
                &rules,
                &evidence.join(format!("{package}-{kind}-invalid")),
                b"{\"invalid\":true}\n",
                false,
            );
            let actual = cli_session(
                &endpoint,
                &rules,
                &evidence.join(format!("{package}-{kind}")),
                &bytes,
                true,
            )
            .unwrap();
            assert_eq!(actual.facts, captured.facts);
            assert_eq!(actual.attach_plan, default_plan);
            assert!(actual.rejected_facts.is_empty());
            assert_eq!(actual.flows.len(), 2);
            assert!(actual.flows.iter().all(|flow| flow.process.is_none()));
            assert_eq!(actual.ingest_trust_mode, "unverified-local");
            // Phase evidence must agree with the independently checked kernel campaign.
            let phases = |export: &ExportBundle| -> std::collections::BTreeSet<_> {
                export
                    .program_flows
                    .iter()
                    .flat_map(|flow| {
                        flow.stages
                            .iter()
                            .map(|stage| (stage.phase.clone(), stage.at))
                    })
                    .collect()
            };
            assert_eq!(phases(&actual), phases(&captured));
            let replay = actual.replay().unwrap();
            assert_eq!(replay.facts, actual.facts);
            assert_eq!(replay.flows, actual.flows);
            assert_eq!(replay.program_flows, actual.program_flows);
            assert_eq!(replay.program_findings, actual.program_findings);
            assert_eq!(replay.protocol_ir, actual.protocol_ir);
            assert_eq!(replay.ingest_trust_mode, "unverified-local");
            results.push(json!({"package":package,"transport":kind,"facts":actual.facts.len(),"flows":actual.flows.len(),
                "l3_proto":0x86dd,"default_fragment_plan":true,"phase_evidence_matches_kernel_campaign":true,
                "replay_identical":true,"malformed_input_rejected_then_recovered":true,"process_attribution":false,
                "ingest_trust_mode":actual.ingest_trust_mode}));
        }
    }
    let result = json!({"schema_version":1,"ok":true,"capture_directory":capture,"sessions":results,
        "scope":"real bounded IPv6 kernel capture -> product conversion -> advisory CLI ingest -> default registry -> replay",
        "automatic_production_probe_attachment":false,"continuous_production_collector":false});
    fs::write(
        evidence.join("summary.json"),
        serde_json::to_vec_pretty(&result).unwrap(),
    )
    .unwrap();
    println!("{}", result);
}
