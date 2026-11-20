//! Runs the generated unit unchanged under the host manager, never enabled at boot.
use super::*;
use std::collections::BTreeMap;
use std::os::unix::fs::OpenOptionsExt;

#[path = "systemd_reference.rs"]
mod reference;

fn command(program: &str, args: &[&str]) -> std::process::Output {
    silvortex_bounded_io::process::run_command_output(
        Command::new(program).args(args).env("SYSTEMD_PAGER", ""),
        None,
        Duration::from_secs(20),
        silvortex_bounded_io::process::OutputLimits::new(1024 * 1024, 65536),
        "owned systemd field service",
    )
    .unwrap()
}

struct Unit {
    name: String,
    path: PathBuf,
    evidence: PathBuf,
    _reference: Option<reference::UnitReference>,
}

impl Unit {
    fn publish(installed: &Installed, out: &Path, label: &str) -> Self {
        let name = format!(
            "gewyvern-field-systemd-{}-{}-{label}.service",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        );
        let path = Path::new("/run/systemd/system").join(&name);
        let bytes = fs::read(installed.generation.join("service.service")).unwrap();
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o644)
            .open(&path)
            .unwrap();
        let mut unit = Self {
            name,
            path,
            evidence: out.join(format!("{label}.journal")),
            _reference: None,
        };
        file.write_all(&bytes).unwrap();
        fs::write(out.join(format!("{label}.service")), &bytes).unwrap();
        unit.ctl(&["daemon-reload"], false);
        unit._reference = Some(reference::UnitReference::new(&unit.name));
        assert_eq!(
            unit.properties()["FragmentPath"],
            unit.path.to_str().unwrap()
        );
        unit
    }

    fn ctl(&self, args: &[&str], include_unit: bool) {
        let mut args = args.to_vec();
        if include_unit {
            args.push(&self.name);
        }
        let output = command("/usr/bin/systemctl", &args);
        assert!(
            output.status.success(),
            "systemctl {args:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn properties(&self) -> BTreeMap<String, String> {
        let output = command(
            "/usr/bin/systemctl",
            &[
                "show",
                &self.name,
                "--property=ActiveState,SubState,MainPID,NRestarts,Result,ExecMainCode,ExecMainStatus,InvocationID,FragmentPath,RestartUSec,StartLimitIntervalUSec,StartLimitBurst,PrivateTmp,ProtectSystem,NoNewPrivileges",
            ],
        );
        assert!(output.status.success());
        String::from_utf8(output.stdout)
            .unwrap()
            .lines()
            .filter_map(|l| {
                l.split_once('=')
                    .map(|(k, v)| (k.to_string(), v.to_string()))
            })
            .collect()
    }

    fn until(
        &self,
        label: &str,
        accept: impl Fn(&BTreeMap<String, String>) -> bool,
    ) -> BTreeMap<String, String> {
        let deadline = Instant::now() + Duration::from_secs(22);
        loop {
            let properties = self.properties();
            if accept(&properties) {
                return properties;
            }
            assert!(Instant::now() < deadline, "{label}: {properties:?}");
            thread::sleep(Duration::from_millis(80));
        }
    }

    fn ready(&self, installed: &Installed, previous: Option<&str>) -> CaptureSnapshot {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok((200, health)) = installed.request("/health", Some(TOKEN), &installed.ca)
                && let Some(value) = health.get("capture")
            {
                let snapshot: CaptureSnapshot = serde_json::from_value(value.clone()).unwrap();
                if snapshot.state == CaptureState::Running
                    && previous != Some(snapshot.run_id.as_str())
                {
                    let properties = self.properties();
                    assert_eq!(properties["ActiveState"], "active");
                    assert_ne!(properties["MainPID"], "0");
                    assert!(!snapshot.incomplete);
                    return snapshot;
                }
            }
            assert!(
                Instant::now() < deadline,
                "systemd TLS readiness: {:?}",
                self.properties()
            );
            thread::sleep(Duration::from_millis(80));
        }
    }

    fn start(&self) {
        let properties = self.properties();
        // A never-started inactive unit may already have been garbage-collected.
        if properties["ActiveState"] == "failed" || properties["NRestarts"] != "0" {
            self.ctl(&["reset-failed"], true);
        }
        self.ctl(&["start"], true);
    }

    fn backoff(&self, installed: &Installed, scope: &Scope) -> BTreeMap<String, String> {
        let properties = self.until("no failure/backoff observed", |p| {
            p["SubState"] == "auto-restart"
        });
        assert!(TcpStream::connect(("127.0.0.1", installed.port)).is_err());
        if scope.0.exists() {
            detached(scope);
        }
        properties
    }

    fn stop(&self, installed: &Installed, scope: &Scope, expected: u64) -> CaptureSnapshot {
        self.ctl(&["stop"], true);
        let p = self.properties();
        assert_eq!(p["ActiveState"], "inactive");
        assert_eq!(p["ExecMainStatus"], "0");
        assert_eq!(p["Result"], "success");
        detached(scope);
        let snapshot = installed.status();
        assert_eq!(snapshot.kernel.matched, expected);
        assert_eq!(snapshot.received, expected);
        assert_eq!(snapshot.retained, expected);
        assert!(snapshot.final_accounted && snapshot.reconciles() && !snapshot.incomplete);
        snapshot
    }

    fn cleanup(&self) {
        self.ctl(&["stop"], true);
        let output = command(
            "/usr/bin/journalctl",
            &[
                "-u",
                &self.name,
                "--no-pager",
                "-n",
                "120",
                "-o",
                "short-monotonic",
            ],
        );
        fs::write(&self.evidence, output.stdout).unwrap();
        let _ = command("/usr/bin/systemctl", &["reset-failed", &self.name]);
        fs::remove_file(&self.path).unwrap();
        self.ctl(&["daemon-reload"], false);
        assert!(!self.path.exists());
    }
}

impl Drop for Unit {
    fn drop(&mut self) {
        if self.path.exists() {
            // Best effort on assertion failure; never touch any other unit.
            let _ = std::panic::catch_unwind(|| self.cleanup());
        }
    }
}

fn traffic(installed: &Installed, scope: &Scope, peers: &Peers, count: u16) {
    let mut process = worker(scope, peers, count, 20, &installed.root);
    assert!(wait(&mut process, 10).success());
}

#[test]
#[ignore = "requires host systemd/root, cgroup v2 and GEWY_SYSTEMD_CAPTURE_OUT=new-directory"]
fn generated_unit_restart_and_failure_recovery() {
    assert_eq!(unsafe { libc::geteuid() }, 0);
    assert_eq!(
        fs::read_to_string("/proc/1/comm").unwrap().trim(),
        "systemd"
    );
    let out = PathBuf::from(
        std::env::var_os("GEWY_SYSTEMD_CAPTURE_OUT").expect("new evidence directory"),
    );
    fs::create_dir(&out).unwrap();
    let scope = Scope(PathBuf::from(format!(
        "/sys/fs/cgroup/gewyvern-production-test-systemd-{}-{}",
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
    let (installed, _) = install(&out, "systemd-main", plan.clone());
    let unit = Unit::publish(&installed, &out, "main");
    let mut cases = Vec::new();
    let record = |cases: &[Value]| {
        fs::write(
            out.join("progress.json"),
            serde_json::to_vec_pretty(cases).unwrap(),
        )
        .unwrap()
    };

    unit.start();
    unit.ready(&installed, None);
    let initial = unit.properties();
    assert_eq!(initial["PrivateTmp"], "yes");
    assert_eq!(initial["ProtectSystem"], "strict");
    assert_eq!(initial["NoNewPrivileges"], "yes");
    traffic(&installed, &scope, &peers, 16);
    let deadline = Instant::now() + Duration::from_secs(3);
    let tls_facts = loop {
        let (status, value) = installed
            .request("/v1/latest/export.json", Some(TOKEN), &installed.ca)
            .unwrap();
        assert_eq!(status, 200);
        let export = ExportBundle::from_json(&value.to_string()).unwrap();
        check_export(&export);
        if [0x800, 0x86dd].into_iter().all(|family| {
            export.facts.iter().any(|fact|
            matches!(&fact.kind, FactKind::PacketMeta(packet) if packet.l3_proto == family))
        }) {
            fs::write(out.join("tls-export.json"), export.to_json()).unwrap();
            break export.facts.len();
        }
        assert!(
            Instant::now() < deadline,
            "missing real dual-stack TLS facts"
        );
        thread::sleep(Duration::from_millis(20));
    };
    let snapshot = unit.stop(&installed, &scope, 64);
    let export = ExportBundle::from_json(
        &fs::read_to_string(installed.capture_root().join("export.json")).unwrap(),
    )
    .unwrap();
    check_export(&export);
    thread::sleep(Duration::from_millis(2300));
    assert_eq!(unit.properties()["ActiveState"], "inactive");
    assert_eq!(unit.properties()["NRestarts"], "0");
    cases.push(json!({"case":"systemd_start_and_graceful_stop","snapshot":snapshot,"properties":initial,"tls_observed_facts":tls_facts,"no_restart_after_stop":true}));
    record(&cases);

    unit.start();
    let before = unit.ready(&installed, None);
    let previous = unit.properties();
    unit.ctl(&["kill", "--kill-whom=main", "--signal=SIGKILL"], true);
    let recovered = unit.ready(&installed, Some(&before.run_id));
    let properties = unit.properties();
    assert_ne!(previous["MainPID"], properties["MainPID"]);
    assert_ne!(previous["InvocationID"], properties["InvocationID"]);
    assert_eq!(properties["NRestarts"], "1");
    traffic(&installed, &scope, &peers, 8);
    cases.push(json!({"case":"sigkill_automatic_restart","snapshot":unit.stop(&installed,&scope,32),
        "old_run_id":before.run_id,"new_run_id":recovered.run_id,"old_final_counters_available":false,"properties":properties}));
    record(&cases);

    let certificate_path = installed.generation.join("server.crt");
    let certificate = fs::read(&certificate_path).unwrap();
    fs::write(&certificate_path, b"invalid-test-certificate").unwrap();
    unit.start();
    let failure = unit.backoff(&installed, &scope);
    let failed = installed.status();
    assert_eq!(failed.state, CaptureState::Failed);
    fs::write(&certificate_path, &certificate).unwrap();
    unit.ready(&installed, Some(&failed.run_id));
    assert_eq!(unit.properties()["NRestarts"], "1");
    traffic(&installed, &scope, &peers, 8);
    cases.push(
        json!({"case":"tls_repair_automatic_recovery","failure":failure,"failed_snapshot":failed,
        "snapshot":unit.stop(&installed,&scope,32),"manual_restart_after_repair":false}),
    );
    record(&cases);

    unit.start();
    let before = unit.ready(&installed, None);
    let output = installed.capture_root().join("export.json");
    fs::remove_file(&output).unwrap();
    fs::create_dir(&output).unwrap();
    let failure = unit.backoff(&installed, &scope);
    let failed = installed.status();
    assert_eq!(failed.state, CaptureState::Failed);
    fs::remove_dir(&output).unwrap();
    unit.ready(&installed, Some(&before.run_id));
    assert_eq!(unit.properties()["NRestarts"], "1");
    traffic(&installed, &scope, &peers, 8);
    cases.push(
        json!({"case":"output_repair_automatic_recovery","failure":failure,"failed_snapshot":failed,
        "snapshot":unit.stop(&installed,&scope,32),"manual_restart_after_repair":false}),
    );
    record(&cases);

    unit.start();
    let before = unit.ready(&installed, None);
    fs::remove_dir(&scope.0).unwrap();
    let failure = unit.backoff(&installed, &scope);
    let failed = installed.status();
    assert_eq!(failed.state, CaptureState::Failed);
    fs::create_dir(&scope.0).unwrap();
    unit.ready(&installed, Some(&before.run_id));
    assert_eq!(unit.properties()["NRestarts"], "1");
    traffic(&installed, &scope, &peers, 8);
    cases.push(json!({"case":"scope_recreation_automatic_recovery","failure":failure,"failed_snapshot":failed,
        "snapshot":unit.stop(&installed,&scope,32),"manual_restart_after_repair":false}));
    record(&cases);

    fs::write(&certificate_path, b"persistent-invalid-certificate").unwrap();
    unit.start();
    let limited = unit.until(
        "persistent failure must stop at a bounded start limit",
        |p| p["ActiveState"] == "failed" && p["SubState"] == "failed" && p["NRestarts"] == "5",
    );
    // Some systemd versions retain the original exit-code Result on retry refusal.
    let journal = command(
        "/usr/bin/journalctl",
        &["-u", &unit.name, "--no-pager", "-n", "100"],
    );
    let start_limit_logged =
        String::from_utf8_lossy(&journal.stdout).contains("Start request repeated too quickly");
    assert!(start_limit_logged, "missing manager start-limit evidence");
    detached(&scope);
    assert!(TcpStream::connect(("127.0.0.1", installed.port)).is_err());
    assert_eq!(limited["StartLimitBurst"], "5");
    thread::sleep(Duration::from_millis(2300));
    assert_eq!(unit.properties()["NRestarts"], limited["NRestarts"]);
    fs::write(&certificate_path, &certificate).unwrap();
    unit.start();
    unit.ready(&installed, None);
    traffic(&installed, &scope, &peers, 8);
    cases.push(
        json!({"case":"persistent_failure_bounded_and_operator_reset","limit":limited,
        "snapshot":unit.stop(&installed,&scope,32),"manual_reset_required":true,"start_limit_logged":start_limit_logged}),
    );
    record(&cases);

    plan.max_facts = 1;
    plan.window_ms = 60000;
    let (limited, _) = install(&out, "systemd-loss", plan);
    let loss_unit = Unit::publish(&limited, &out, "loss");
    loss_unit.start();
    loss_unit.ready(&limited, None);
    traffic(&limited, &scope, &peers, 16);
    // Unlike systemctl stop, a signal alone does not suppress Restart=on-failure.
    loss_unit.ctl(&["kill", "--kill-whom=main", "--signal=SIGTERM"], true);
    let properties = loss_unit.until("accounted loss should be a clean service stop", |p| {
        p["ActiveState"] == "inactive"
    });
    assert_eq!(properties["ExecMainStatus"], "3");
    assert_eq!(properties["Result"], "success");
    thread::sleep(Duration::from_millis(2300));
    assert_eq!(loss_unit.properties()["NRestarts"], "0");
    assert_eq!(loss_unit.properties()["ActiveState"], "inactive");
    let snapshot = limited.status();
    assert!(snapshot.final_accounted && snapshot.incomplete && snapshot.reconciles());
    assert_eq!(snapshot.userspace_dropped, 63);
    detached(&scope);
    cases.push(json!({"case":"accounted_loss_no_restart_loop","snapshot":snapshot,"properties":properties,"no_restart":true}));
    record(&cases);
    loss_unit.cleanup();
    unit.cleanup();
    fs::remove_dir(&scope.0).unwrap();
    fs::write(out.join("summary.json"), serde_json::to_vec_pretty(&json!({
        "schema_version":1,"ok":true,"systemd_unit_executed":true,"generated_unit_unchanged":true,
        "operator_profile":"test","native_activation_entrypoint_executed":false,"boot_enablement":false,
        "host_reboot":false,"automatic_restart_verified":true,"tls_pinned_ca":true,
        "owned_units_removed":true,"owned_cgroup_removed":true,"cases":cases
    })).unwrap()).unwrap();
}
