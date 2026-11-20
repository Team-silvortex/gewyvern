use super::*;
use gewyvern_install_contract::capture::UdpCapturePlan;
use gewyvern_install_contract::installer::{
    GewyvernInstallerRequest, decode_gewyvern_installer_response, encode_gewyvern_installer_request,
};
use gewyvern_install_contract::{CredentialHandle, ProvisioningId, RuntimeId};
use rustls::pki_types::{CertificateDer, ServerName, pem::PemObject};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

#[path = "systemd.rs"]
mod systemd;

const TOKEN: &str = "installed-capture-test-token-0123456789";

struct Installed {
    generation: PathBuf,
    root: PathBuf,
    port: u16,
    ca: String,
}
impl Installed {
    fn start(&self, label: &str, wrong_environment: bool) -> Process {
        let mut command = Command::new(self.generation.join("gewyvern"));
        command
            .env_clear()
            .env("HOME", &self.root)
            .arg("gewyvern-service-v1");
        for (key, directory) in [
            ("GEWY_CONFIG_HOME", "config"),
            ("GEWY_DATA_HOME", "data"),
            ("GEWY_STATE_HOME", "state"),
            ("GEWY_CACHE_HOME", "cache"),
        ] {
            command.env(key, self.root.join(directory));
        }
        if wrong_environment {
            command.env("GEWY_STATE_HOME", self.root.join("wrong-state"));
        }
        Process(
            command
                .current_dir(&self.root)
                .stdin(Stdio::null())
                .stdout(fs::File::create(self.root.join(format!("{label}.stdout"))).unwrap())
                .stderr(fs::File::create(self.root.join(format!("{label}.stderr"))).unwrap())
                .spawn()
                .unwrap(),
        )
    }
    fn capture_root(&self) -> PathBuf {
        self.root.join("state/capture")
    }
    fn status(&self) -> CaptureSnapshot {
        snapshot(&self.capture_root())
    }
    fn request(&self, path: &str, token: Option<&str>, ca: &str) -> Result<(u16, Value), String> {
        let mut roots = RootCertStore::empty();
        for certificate in CertificateDer::pem_slice_iter(ca.as_bytes()) {
            roots
                .add(certificate.map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
        }
        let config =
            ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
                .with_safe_default_protocol_versions()
                .map_err(|e| e.to_string())?
                .with_root_certificates(roots)
                .with_no_client_auth();
        let socket = TcpStream::connect_timeout(
            &SocketAddr::from(([127, 0, 0, 1], self.port)),
            Duration::from_secs(1),
        )
        .map_err(|e| e.to_string())?;
        socket
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|e| e.to_string())?;
        socket
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|e| e.to_string())?;
        let client =
            ClientConnection::new(Arc::new(config), ServerName::try_from("127.0.0.1").unwrap())
                .map_err(|e| e.to_string())?;
        let mut stream = StreamOwned::new(client, socket);
        let auth = token
            .map(|t| format!("X-Gewyvern-Admin-Token: {t}\r\n"))
            .unwrap_or_default();
        stream
            .write_all(
                format!(
                    "GET {path} HTTP/1.1\r\nHost: localhost\r\n{auth}Connection: close\r\n\r\n"
                )
                .as_bytes(),
            )
            .map_err(|e| e.to_string())?;
        stream.flush().map_err(|e| e.to_string())?;
        let mut header = Vec::new();
        while !header.ends_with(b"\r\n\r\n") {
            if header.len() >= 16384 {
                return Err("header limit".into());
            }
            let mut byte = [0];
            stream.read_exact(&mut byte).map_err(|e| e.to_string())?;
            header.push(byte[0]);
        }
        let header = String::from_utf8(header).map_err(|e| e.to_string())?;
        let status = header
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .parse()
            .unwrap();
        let length: usize = header
            .lines()
            .find_map(|line| {
                line.split_once(':')
                    .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
                    .map(|(_, value)| value.trim())
            })
            .ok_or("missing length")?
            .parse()
            .map_err(|_| "invalid length")?;
        if length > 4 * 1024 * 1024 {
            return Err("body limit".into());
        }
        let mut body = vec![0; length];
        stream.read_exact(&mut body).map_err(|e| e.to_string())?;
        Ok((
            status,
            serde_json::from_slice(&body).map_err(|e| e.to_string())?,
        ))
    }
    fn ready(&self, process: &mut Process) -> CaptureSnapshot {
        let deadline = Instant::now() + Duration::from_secs(40);
        loop {
            assert!(
                process.0.try_wait().unwrap().is_none(),
                "installed capture exited before TLS readiness"
            );
            if let Ok((200, health)) = self.request("/health", Some(TOKEN), &self.ca)
                && let Some(capture) = health.get("capture")
            {
                let capture: CaptureSnapshot = serde_json::from_value(capture.clone()).unwrap();
                assert_eq!(capture.state, CaptureState::Running);
                return capture;
            }
            assert!(
                Instant::now() < deadline,
                "installed TLS capture never ready"
            );
            thread::sleep(Duration::from_millis(20));
        }
    }
}

fn invoke(root: &Path, request: &GewyvernInstallerRequest) -> std::process::Output {
    let payload = encode_gewyvern_installer_request(request).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_gewyvern"))
        .arg("gewyvern-install-v1")
        .env("GEWYVERN_INSTALL_ROOT", root)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(&payload).unwrap();
    child.wait_with_output().unwrap()
}

fn install(
    out: &Path,
    name: &str,
    capture: UdpCapturePlan,
) -> (Installed, GewyvernInstallerRequest) {
    let artifact = fs::read(env!("CARGO_BIN_EXE_gewyvern")).unwrap();
    let digest = ring::digest::digest(&ring::digest::SHA256, &artifact)
        .as_ref()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect::<String>();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let mut request = GewyvernInstallerRequest::new(
        ProvisioningId::new(format!("capture-{name}")).unwrap(),
        RuntimeId::new(format!("capture-{name}")).unwrap(),
        format!("https://127.0.0.1:{port}"),
        "test",
        digest,
        CredentialHandle::new("vault:gewyvern:capture-test").unwrap(),
        CredentialHandle::new("vault:gewyvern-ca:capture-test").unwrap(),
        TOKEN,
    )
    .unwrap();
    request.capture = Some(capture);
    let installation = out.join("install");
    let first = invoke(&installation, &request);
    assert!(
        first.status.success(),
        "{}",
        String::from_utf8_lossy(&first.stderr)
    );
    let first = decode_gewyvern_installer_response(&first.stdout).unwrap();
    let root = installation
        .join("runtimes")
        .join(request.runtime_id.as_str());
    let generation = root.join("generations").join(&first.generation);
    let unit = fs::read_to_string(generation.join("service.service")).unwrap();
    assert!(
        unit.contains("GEWY_STATE_HOME=")
            && unit.contains("SuccessExitStatus=3")
            && unit.contains("NoNewPrivileges=true")
    );
    let replay = invoke(&installation, &request);
    assert!(replay.status.success());
    assert!(
        decode_gewyvern_installer_response(&replay.stdout)
            .unwrap()
            .replayed
    );
    (
        Installed {
            generation,
            root,
            port,
            ca: first.tls_ca_pem,
        },
        request,
    )
}

#[test]
#[ignore = "requires Linux root, cgroup v2, clang/libbpf/bpftool and GEWY_INSTALLED_CAPTURE_OUT=new-directory"]
fn installed_capture_service_tls_lifecycle() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    let out = PathBuf::from(
        std::env::var_os("GEWY_INSTALLED_CAPTURE_OUT").expect("new evidence directory"),
    );
    fs::create_dir(&out).unwrap();
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-installed-{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    )));
    fs::create_dir(&scope.0).unwrap();
    let peers = peers();
    let mut plan = UdpCapturePlan::new(scope.0.to_str().unwrap(), peers.port);
    plan.window_ms = 200;
    let (installed, mut request) = install(&out, "main", plan.clone());
    let mut results = Vec::new();

    request.capture.as_mut().unwrap().max_facts = 1;
    assert!(!invoke(&out.join("install"), &request).status.success());
    results.push(json!({"case":"changed_plan_same_generation_rejected","ok":true}));
    let plan_path = installed.generation.join("service-plan.json");
    let original = fs::read(&plan_path).unwrap();
    let mut tampered: Value = serde_json::from_slice(&original).unwrap();
    tampered["capture"]["cgroup"] = json!("/sys/fs/cgroup/other");
    fs::write(&plan_path, serde_json::to_vec(&tampered).unwrap()).unwrap();
    let mut invalid = installed.start("tampered", false);
    assert!(!wait(&mut invalid, 10).success());
    fs::write(&plan_path, &original).unwrap();
    let mut invalid = installed.start("foreign-state", true);
    assert!(!wait(&mut invalid, 10).success());
    assert!(!installed.root.join("wrong-state").exists());
    assert!(TcpStream::connect(("127.0.0.1", installed.port)).is_err());
    detached(&scope);
    results.push(json!({"case":"plan_binding_and_private_environment_fail_closed","ok":true}));

    let certificate_path = installed.generation.join("server.crt");
    let certificate = fs::read(&certificate_path).unwrap();
    fs::write(&certificate_path, b"invalid-test-certificate").unwrap();
    let mut invalid_tls = installed.start("invalid-tls", false);
    assert!(!wait(&mut invalid_tls, 10).success());
    assert_eq!(installed.status().state, CaptureState::Failed);
    assert!(installed.status().incomplete);
    assert!(TcpStream::connect(("127.0.0.1", installed.port)).is_err());
    detached(&scope);
    fs::write(&certificate_path, certificate).unwrap();
    results
        .push(json!({"case":"tls_start_failure_detaches_capture","snapshot":installed.status()}));

    let mut process = installed.start("normal", false);
    let first = installed.ready(&mut process);
    assert_eq!(first.cgroup, plan.cgroup);
    assert_eq!(first.udp_port, plan.udp_port);
    let missing = installed.request("/health", None, &installed.ca).unwrap().0;
    let wrong = installed
        .request("/v1/latest/export.json", Some("wrong-token"), &installed.ca)
        .unwrap()
        .0;
    assert_ne!(missing, 200);
    assert_ne!(wrong, 200);
    let wrong_ca = rcgen::generate_simple_self_signed(vec!["127.0.0.1".into()])
        .unwrap()
        .cert
        .pem();
    assert!(
        installed
            .request("/health", Some(TOKEN), &wrong_ca)
            .is_err()
    );
    let mut traffic = worker(&scope, &peers, 16, 20, &installed.root);
    assert!(wait(&mut traffic, 10).success());
    let mut observed = false;
    for _ in 0..40 {
        let (status, value) = installed
            .request("/v1/latest/export.json", Some(TOKEN), &installed.ca)
            .unwrap();
        assert_eq!(status, 200);
        let export = ExportBundle::from_json(&value.to_string()).unwrap();
        check_export(&export);
        if export
            .facts
            .iter()
            .any(|f| matches!(&f.kind,FactKind::PacketMeta(p) if p.l3_proto==0x86dd))
        {
            fs::write(out.join("tls-export.json"), export.to_json()).unwrap();
            observed = true;
            break;
        }
        thread::sleep(Duration::from_millis(20));
    }
    assert!(observed, "TLS export did not expose real IPv6 facts");
    signal(&process, libc::SIGTERM);
    assert_eq!(wait(&mut process, 10).code(), Some(0));
    let status = installed.status();
    assert_eq!(status.kernel.matched, 64);
    assert!(status.final_accounted && !status.incomplete);
    detached(&scope);
    results.push(
        json!({"case":"tls_authenticated_dual_stack_capture","snapshot":status,
        "missing_token_status":missing,"wrong_token_status":wrong,"wrong_ca_rejected":true}),
    );

    let mut killed = installed.start("kill", false);
    let killed_id = installed.ready(&mut killed).run_id;
    signal(&killed, libc::SIGKILL);
    assert!(!wait(&mut killed, 10).success());
    detached(&scope);
    let mut recovered = installed.start("restart", false);
    assert_ne!(installed.ready(&mut recovered).run_id, killed_id);
    let mut traffic = worker(&scope, &peers, 8, 0, &installed.root);
    assert!(wait(&mut traffic, 10).success());
    signal(&recovered, libc::SIGTERM);
    assert_eq!(wait(&mut recovered, 10).code(), Some(0));
    assert_eq!(installed.status().kernel.matched, 32);
    detached(&scope);
    results.push(json!({"case":"installed_sigkill_restart","snapshot":installed.status(),"old_final_counters_available":false}));

    let mut failed = installed.start("output-failure", false);
    installed.ready(&mut failed);
    let output = installed.capture_root().join("export.json");
    fs::remove_file(&output).unwrap();
    fs::create_dir(&output).unwrap();
    assert!(!wait(&mut failed, 10).success());
    assert_eq!(installed.status().state, CaptureState::Failed);
    detached(&scope);
    fs::remove_dir(&output).unwrap();
    results.push(json!({"case":"installed_output_failure_detaches","snapshot":installed.status()}));

    let mut missing_plan = plan.clone();
    missing_plan.cgroup.push_str("-missing");
    let (missing, _) = install(&out, "missing", missing_plan);
    let mut failed = missing.start("missing-cgroup", false);
    assert!(!wait(&mut failed, 10).success());
    assert!(TcpStream::connect(("127.0.0.1", missing.port)).is_err());
    results.push(json!({"case":"capture_start_failure_never_opens_tls","ok":true}));

    plan.max_facts = 1;
    plan.window_ms = 60000;
    let (limited, _) = install(&out, "limited", plan);
    let mut process = limited.start("limited", false);
    limited.ready(&mut process);
    let mut traffic = worker(&scope, &peers, 16, 0, &limited.root);
    assert!(wait(&mut traffic, 10).success());
    signal(&process, libc::SIGTERM);
    assert_eq!(wait(&mut process, 10).code(), Some(3));
    let status = limited.status();
    assert_eq!(status.userspace_dropped, 63);
    assert!(status.final_accounted && status.incomplete);
    detached(&scope);
    results
        .push(json!({"case":"installed_loss_exit_code_preserved","snapshot":status,"exit_code":3}));
    fs::remove_dir(&scope.0).unwrap();
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({"schema_version":1,"ok":true,
        "real_installed_binary":true,"real_tls":true,"systemd_unit_executed":false,
        "operator_profile":"test","api_uses_same_capture_loop":true,"owned_cgroup_removed":true,
        "cases":results}))
        .unwrap(),
    )
    .unwrap();
}
