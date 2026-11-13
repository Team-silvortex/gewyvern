use gewyvern::export::fact_to_json;
use gewyvern::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId,
};
use serde_json::Value;
use silvortex_bounded_io::process::{OutputLimits, run_command_output};
use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime};

struct Probe {
    root: PathBuf,
}

impl Probe {
    fn new() -> Self {
        let nonce = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root =
            std::env::temp_dir().join(format!("gewy-recovery-{}-{nonce}", std::process::id()));
        fs::create_dir_all(&root).unwrap();
        let probe = Self { root };
        let output = run_command_output(
            Command::new(std::env::var_os("RUSTC").unwrap_or_else(|| "rustc".into()))
                .args(["--edition=2024", "--crate-name", "external_recovery_probe"])
                .arg(
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("tests/fixtures/external_recovery_engine.rs"),
                )
                .arg("-o")
                .arg(probe.binary()),
            None,
            Duration::from_secs(60),
            OutputLimits::new(65536, 65536),
            "compile recovery probe",
        )
        .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        probe
    }

    fn binary(&self) -> PathBuf {
        self.root
            .join(format!("probe{}", std::env::consts::EXE_SUFFIX))
    }
}

impl Drop for Probe {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

struct Service {
    child: Child,
    root: PathBuf,
    ingest: SocketAddr,
    api: SocketAddr,
}

impl Service {
    fn start(probe: &Probe, mode: &str) -> Self {
        let root = probe.root.join(mode);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("mode"), mode).unwrap();
        let ingest = TcpListener::bind("127.0.0.1:0").unwrap();
        let api = TcpListener::bind("127.0.0.1:0").unwrap();
        let ingest_addr = ingest.local_addr().unwrap();
        let api_addr = api.local_addr().unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_gewyvern"));
        // Do not inherit credentials, runtime paths, or optional providers from the caller.
        command
            .env_clear()
            .env("LANG", "C")
            .env("HOME", root.join("home"))
            .env("USERPROFILE", root.join("home"));
        for key in ["PATH", "SYSTEMROOT", "WINDIR"] {
            if let Some(value) = std::env::var_os(key) {
                command.env(key, value);
            }
        }
        for (key, dir) in [
            ("GEWY_CONFIG_HOME", "config"),
            ("GEWY_DATA_HOME", "data"),
            ("GEWY_STATE_HOME", "state"),
            ("GEWY_CACHE_HOME", "cache"),
        ] {
            command.env(key, root.join(dir));
        }
        command
            .current_dir(&root)
            .env("GEWY_RECOVERY_PROBE_ROOT", &root)
            .env("GEWY_EXTERNAL_FAILURE_CIRCUIT_THRESHOLD", "2")
            .env("GEWY_EXTERNAL_FAILURE_CIRCUIT_COOLDOWN_SECONDS", "5")
            .args([
                "--tcp-socket",
                &ingest_addr.to_string(),
                "--api-socket",
                &api_addr.to_string(),
                "--serve",
                "--ingest-mode",
                "local-advisory",
                "--template",
                "udp",
                "--json",
                "--summary-only",
                "--external-engine-bin",
            ])
            .arg(probe.binary())
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(fs::File::create(root.join("stderr.log")).unwrap());
        drop((ingest, api));
        let child = command.spawn().unwrap();
        let mut service = Self {
            child,
            root,
            ingest: ingest_addr,
            api: api_addr,
        };
        service.wait_for("/health", |_| true);
        service
    }

    fn mode(&self, mode: &str) {
        fs::write(self.root.join("mode"), mode).unwrap();
    }

    fn calls(&self, command: &str) -> usize {
        fs::read_to_string(self.root.join("calls"))
            .unwrap_or_default()
            .lines()
            .filter(|line| *line == command)
            .count()
    }

    fn get(&self, path: &str) -> Option<Value> {
        let timeout = Duration::from_secs(1);
        let mut stream = TcpStream::connect_timeout(&self.api, timeout).ok()?;
        stream.set_read_timeout(Some(timeout)).ok()?;
        stream.set_write_timeout(Some(timeout)).ok()?;
        write!(
            stream,
            "GET {path} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n\r\n"
        )
        .ok()?;
        let mut response = String::new();
        stream
            .take(4 * 1024 * 1024)
            .read_to_string(&mut response)
            .ok()?;
        let (head, body) = response.split_once("\r\n\r\n")?;
        if !head.starts_with("HTTP/1.1 200") {
            return None;
        }
        serde_json::from_str(body).ok()
    }

    fn wait_for(&mut self, path: &str, ready: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(15);
        loop {
            assert!(
                self.child.try_wait().unwrap().is_none(),
                "service exited: {}",
                fs::read_to_string(self.root.join("stderr.log")).unwrap()
            );
            if let Some(value) = self.get(path).filter(&ready) {
                return value;
            }
            assert!(Instant::now() < deadline, "timed out waiting for {path}");
            thread::sleep(Duration::from_millis(20));
        }
    }

    fn submit(&mut self) -> Value {
        let previous = self.get("/v1/latest/meta").unwrap()["updated_unix_ms"]
            .as_u64()
            .unwrap();
        thread::sleep(Duration::from_millis(2));
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut stream = loop {
            if let Ok(stream) = TcpStream::connect_timeout(&self.ingest, Duration::from_millis(100))
            {
                break stream;
            }
            assert!(Instant::now() < deadline, "ingest listener did not start");
            thread::sleep(Duration::from_millis(20));
        };
        stream
            .set_write_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        writeln!(stream, "{}", fact_to_json(&packet())).unwrap();
        drop(stream);
        self.wait_for("/v1/latest/meta", |value| {
            value["updated_unix_ms"].as_u64().unwrap() > previous
        });
        self.get("/v1/latest/analysis.json").unwrap()
    }

    fn resilience(&self) -> Value {
        self.get("/v1/runtime/resilience.json").unwrap()["external_analysis"].clone()
    }
}

impl Drop for Service {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn packet() -> FactEnvelope {
    FactEnvelope {
        id: FactId(1),
        ts: SystemTime::UNIX_EPOCH + Duration::from_millis(10),
        cpu: CpuId(0),
        ifindex: Some(2),
        session: SessionId(1),
        fragment_id: "udp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns: 1,
            sk_cookie: Some(123),
            dir: PacketDir::Egress,
            local_port: None,
            remote_port: None,
            payload_byte0: None,
            payload_byte1: None,
            payload_prefix2: None,
            payload_prefix4: None,
            payload_byte4: None,
            payload_byte5: None,
            payload_byte9: None,
            payload_byte10: None,
            payload_byte13: None,
            payload_bytes: Default::default(),
            l3_proto: 0x0800,
            l4_proto: 17,
            tot_len: 88,
            tcp_flags: 0,
            seq: None,
            ack: None,
            window: None,
        }),
    }
}

fn augmentation<'a>(analysis: &'a Value, name: &str) -> Option<&'a Value> {
    analysis["augmentations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["name"] == name)
}

#[test]
fn production_service_recovers_identical_input_after_external_faults() {
    let probe = Probe::new();
    for mode in ["exit", "malformed", "overflow", "timeout"] {
        let mut service = Service::start(&probe, mode);
        let mut original = service.submit();
        assert!(
            augmentation(&original, "external_engine_failed").is_some(),
            "{mode}"
        );
        assert_eq!(service.resilience()["total_failures"], 1, "{mode}");
        let failed_again = service.submit();
        assert!(augmentation(&failed_again, "external_engine_failed").is_some());
        assert_eq!(
            service.calls("analyze-json"),
            2,
            "{mode}: failure must not poison the snapshot cache"
        );
        assert_eq!(service.resilience()["circuit_open"], true, "{mode}");
        service.mode("healthy");
        assert!(augmentation(&service.submit(), "external_engine_failed").is_some());
        assert_eq!(
            service.calls("analyze-json"),
            2,
            "{mode}: open circuit must bypass the child"
        );
        let remaining = service.resilience()["cooldown_remaining_ms"]
            .as_u64()
            .unwrap();
        thread::sleep(Duration::from_millis(remaining + 100));
        let mut recovered = service.submit();
        assert!(
            augmentation(&recovered, "recovery_probe").is_some(),
            "{mode}"
        );
        assert!(augmentation(&recovered, "external_engine_failed").is_none());
        assert_eq!(service.resilience()["consecutive_failures"], 0);
        assert_eq!(service.resilience()["total_failures"], 2);
        assert_eq!(service.resilience()["circuit_open"], false);
        assert!(augmentation(&service.submit(), "recovery_probe").is_some());
        assert_eq!(
            service.calls("analyze-json"),
            3,
            "{mode}: successful snapshots should remain cached"
        );
        assert_eq!(original["protocol_flows"].as_array().unwrap().len(), 1);
        for built_in in original["augmentations"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["producer_stage"] != "external")
        {
            assert!(
                recovered["augmentations"]
                    .as_array()
                    .unwrap()
                    .contains(built_in)
            );
        }
        // The renderer derives sidecar status from augmentations; compare only
        // the built-in fields, which must be stable across external recovery.
        for analysis in [&mut original, &mut recovered] {
            analysis.as_object_mut().unwrap().retain(|key, _| {
                key != "augmentations"
                    && key != "has_external_capability_profile"
                    && !key.starts_with("external_")
            });
        }
        assert_eq!(
            original, recovered,
            "{mode}: built-in evidence must not change"
        );
        println!(
            "{mode}: fallback -> circuit open -> cooldown -> recovered -> successful cache hit"
        );
    }
}

#[test]
fn production_service_retries_failed_capabilities_and_refreshes_downgraded_cache() {
    let probe = Probe::new();
    let mut service = Service::start(&probe, "capability-failure");
    let first = service.submit();
    let first_context = augmentation(&first, "external_evidence_chain_enrichment").unwrap();
    assert_eq!(
        first_context["data"]["external_handoff_readiness"],
        "advisory_only"
    );
    service.mode("healthy");
    assert_eq!(service.submit(), first);
    assert_eq!(
        service.calls("protocol-capabilities"),
        1,
        "failed handshake must be rate limited"
    );
    assert_eq!(service.calls("analyze-json"), 1);
    thread::sleep(Duration::from_millis(5200));
    let recovered = service.submit();
    assert_eq!(
        service.calls("protocol-capabilities"),
        2,
        "unavailable capabilities must not be cached forever"
    );
    assert_eq!(
        service.calls("analyze-json"),
        2,
        "downgraded snapshot must be refreshed"
    );
    assert_eq!(
        augmentation(&recovered, "external_evidence_chain_enrichment").unwrap()["data"]["external_handoff_readiness"],
        "mergeable"
    );
    assert_eq!(service.submit(), recovered);
    assert_eq!(service.calls("protocol-capabilities"), 2);
    assert_eq!(service.calls("analyze-json"), 2);
    println!(
        "capability handshake failure: bounded retry -> capability recovery -> refreshed cached result"
    );
}
