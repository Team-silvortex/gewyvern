use std::fs;
use std::io::{Read, Write};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::super::packet_evidence::reconstruct_packets;
use super::packet_capture::{Capture, Scope};
use super::{
    Pins, ValidationError, ValidationReport, attach_capture, bpf_link, counts, export_stage,
    network_namespace, owned_socket, port, prepare_output, snapshot, tool, wait_state,
};

mod supervisor;
use supervisor::{Ready, verify_links};
const WORKER_PARENT: &str = "GEWY_CAPTURE_RECOVERY_PARENT";

struct Fixture {
    out: PathBuf,
    pins: Pins,
    capture: Capture,
    scope: Scope,
    client: TcpStream,
    peer: TcpStream,
    expected: [(u64, u16, u16, u32); 2],
    clock: (u64, SystemTime),
}

impl Fixture {
    fn new(out: &Path, capacity: u32) -> Result<Self, ValidationError> {
        let out = prepare_output(Some(out.to_owned()), "linux-capture-recovery")?;
        let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
        listener.set_nonblocking(true)?;
        let target = listener.local_addr()?;
        let token = format!(
            "{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|e| ValidationError::new(e.to_string()))?
                .as_nanos()
        );
        let scope = Scope::enter(&token)?;
        let first = owned_socket()?;
        let reserved = owned_socket()?;
        let cookies = [first.cookie()?, reserved.cookie()?];
        let ns = network_namespace()?;
        let expected = [
            (cookies[0], port(&first)?, target.port(), ns),
            (cookies[1], port(&reserved)?, target.port(), ns),
        ];
        let (pins, clock) = attach_capture(cookies, &out)?;
        let capture = Capture::attach_with_capacity(
            &scope,
            &expected,
            ([127, 0, 0, 1], [127, 0, 0, 1]),
            &out,
            capacity,
        )?;
        first.connect_timeout(&target.into(), Duration::from_secs(2))?;
        let client = first.into();
        let (peer, address) = listener.accept()?;
        if address.port() != expected[0].1 {
            return Err(ValidationError::new("recovery peer identity mismatch"));
        }
        peer.set_nodelay(true)?;
        peer.set_write_timeout(Some(Duration::from_secs(2)))?;
        Ok(Self {
            out,
            pins,
            capture,
            scope,
            client,
            peer,
            expected,
            clock,
        })
    }

    fn traffic(&mut self, pieces: usize) -> Result<(), ValidationError> {
        for _ in 0..pieces {
            self.peer.write_all(b"x")?;
            let mut byte = [0];
            self.client.read_exact(&mut byte)?;
            if byte != *b"x" {
                return Err(ValidationError::new("recovery traffic was modified"));
            }
        }
        Ok(())
    }

    fn ready(&self) -> Result<Ready, ValidationError> {
        Ready::capture(
            &self.pins.0,
            self.capture.path(),
            self.scope.path(),
            [
                bpf_link::id(
                    self.pins
                        .1
                        .as_ref()
                        .ok_or_else(|| ValidationError::new("tracepoint is detached"))?,
                )?,
                self.capture.link_id()?,
            ],
        )
    }

    fn finish(self, delete_entry: bool) -> Result<Value, ValidationError> {
        let Self {
            out,
            mut pins,
            mut capture,
            mut scope,
            client,
            peer,
            expected,
            clock,
        } = self;
        let ready = Ready::capture(
            &pins.0,
            capture.path(),
            scope.path(),
            [
                bpf_link::id(
                    pins.1
                        .as_ref()
                        .ok_or_else(|| ValidationError::new("missing tracepoint link"))?,
                )?,
                capture.link_id()?,
            ],
        )?;
        drop(peer);
        drop(client);
        wait_state(&pins, &out, "closed", expected[0].0, 7)?;
        pins.detach();
        let (states, stats) = snapshot(&pins, &out, "final")?;
        let tcp = export_stage(&states, stats, &expected, clock, &out, "tcp")?;
        if delete_entry {
            capture.remove_event_for_validation(&out)?;
        }
        let result = match capture.finish(&out) {
            Ok((packets, counts)) => {
                let export = reconstruct_packets(&tcp, &states, &packets, clock)?;
                fs::write(out.join("packet-export.json"), export.to_json())?;
                json!({"accepted":true,"counts":counts,"packet_facts":packets.len(),"replay_verified":true})
            }
            Err(error) => {
                let rejection: Value =
                    serde_json::from_slice(&fs::read(out.join("packet-rejection.json"))?)?;
                if rejection["accepted"] != false || out.join("packet-export.json").exists() {
                    return Err(ValidationError::new(
                        "rejected capture published a complete packet export",
                    ));
                }
                json!({"accepted":false,"error":error.to_string(),"rejection":rejection})
            }
        };
        pins.cleanup()?;
        scope.cleanup()?;
        ready.verify_removed()?;
        verify_links(&ready.link_ids, false, &out)?;
        fs::write(out.join("result.json"), serde_json::to_vec_pretty(&result)?)?;
        Ok(result)
    }
}

fn sample_case(
    out: &Path,
    capacity: u32,
    pieces: usize,
    delete_entry: bool,
    expected_code: Option<&str>,
) -> Result<Value, ValidationError> {
    let mut fixture = Fixture::new(out, capacity)?;
    fixture.traffic(pieces)?;
    let result = fixture.finish(delete_entry)?;
    if let Some(code) = expected_code {
        let rejection = &result["rejection"];
        if result["accepted"] != false
            || rejection["code"] != code
            || rejection["counts"]["total"] != (pieces + 1) as u64
            || rejection["counts"]["dropped"]
                != (pieces + 1).saturating_sub(capacity as usize) as u64
        {
            return Err(ValidationError::new(format!(
                "fault was not rejected for the expected reason: {result}"
            )));
        }
    } else if result["accepted"] != true || result["packet_facts"] != (pieces + 1) as u64 {
        return Err(ValidationError::new(
            "clean capture failed at its expected capacity",
        ));
    }
    Ok(result)
}

fn worker(out: PathBuf, parent: u32) -> Result<ValidationReport, ValidationError> {
    // SAFETY: this only inspects our parent and limits this worker's core files.
    let actual_parent = unsafe { libc::getppid() } as u32;
    if parent <= 1
        || parent != actual_parent
        || fs::read_link(format!("/proc/{parent}/exe"))? != std::env::current_exe()?
    {
        return Err(ValidationError::new(
            "capture recovery worker requires its native supervisor",
        ));
    }
    let limits = libc::rlimit {
        rlim_cur: 0,
        rlim_max: 0,
    };
    // SAFETY: valid rlimit pointer; PDEATHSIG confines this owned worker if its supervisor disappears.
    if unsafe { libc::setrlimit(libc::RLIMIT_CORE, &limits) } != 0
        || unsafe { libc::prctl(libc::PR_SET_DUMPABLE, 0) } != 0
        || unsafe { libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL) } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    // Close the parent-death race before allocating any capture resources.
    if unsafe { libc::getppid() } as u32 != parent {
        return Err(ValidationError::new("capture supervisor exited"));
    }
    let mut fixture = Fixture::new(&out, 128)?;
    fixture.traffic(3)?;
    let ready = fixture.ready()?;
    fs::write(out.join("ready.tmp"), serde_json::to_vec_pretty(&ready)?)?;
    fs::rename(out.join("ready.tmp"), out.join("ready.json"))?;
    let mut action = [0];
    std::io::stdin().read_exact(&mut action)?;
    match action[0] {
        b'e' => Err(ValidationError::new("injected capture worker error")),
        b'a' => std::process::abort(),
        _ => Err(ValidationError::new("invalid capture worker control byte")),
    }
}

fn tcp_overflow(out: &Path) -> Result<Value, ValidationError> {
    let out = prepare_output(Some(out.to_owned()), "tcp-capacity")?;
    let client = owned_socket()?;
    let reserved = owned_socket()?;
    let target = std::net::SocketAddr::from((Ipv4Addr::LOCALHOST, port(&reserved)?));
    let (mut pins, _) = attach_capture([client.cookie()?, reserved.cookie()?], &out)?;
    const ATTEMPTS: usize = 160;
    for _ in 0..ATTEMPTS {
        if !matches!(client.connect_timeout(&target.into(),Duration::from_secs(2)),Err(e) if matches!(e.kind(),std::io::ErrorKind::ConnectionRefused|std::io::ErrorKind::ConnectionAborted))
        {
            return Err(ValidationError::new(
                "owned refusal loop did not fail as expected",
            ));
        }
    }
    pins.detach();
    let stats = counts(&pins, &out)?;
    if stats.total <= 128
        || stats.dropped != stats.total - 128
        || snapshot(&pins, &out, "overflow").is_ok()
    {
        return Err(ValidationError::new(format!(
            "TCP event overflow was not rejected: {stats:?}"
        )));
    }
    let rejection: Value = serde_json::from_slice(&fs::read(out.join("tcp-rejection.json"))?)?;
    if rejection["code"] != "capacity_exceeded" {
        return Err(ValidationError::new("unexpected TCP rejection reason"));
    }
    pins.cleanup()?;
    let result = json!({"accepted":false,"test_passed":true,"rejection":rejection,"connect_attempts":ATTEMPTS,"tcp_export_produced":false});
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&result)?)?;
    Ok(result)
}

pub(super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    if let Some(parent) = std::env::var_os(WORKER_PARENT) {
        let parent = parent
            .to_str()
            .and_then(|p| p.parse().ok())
            .ok_or_else(|| ValidationError::new("invalid capture supervisor identity"))?;
        return worker(
            out.ok_or_else(|| ValidationError::new("capture worker requires a fresh output path"))?,
            parent,
        );
    }
    let out = prepare_output(out, "linux-capture-recovery")?;
    fs::set_permissions(&out, fs::Permissions::from_mode(0o700))?;
    let mut cases = serde_json::Map::new();
    cases.insert(
        "tcp-over-capacity".into(),
        tcp_overflow(&out.join("tcp-over-capacity"))?,
    );
    for (name, capacity, pieces, delete, code) in [
        ("exact-capacity", 128, 127, false, None),
        ("reported-loss", 2, 4, false, Some("reported_loss")),
        ("over-capacity", 128, 130, false, Some("capacity_exceeded")),
        ("snapshot-gap", 128, 4, true, Some("snapshot_incomplete")),
    ] {
        cases.insert(
            name.into(),
            sample_case(&out.join(name), capacity, pieces, delete, code)?,
        );
    }
    cases.insert(
        "after-loss-restart".into(),
        sample_case(&out.join("after-loss-restart"), 128, 4, false, None)?,
    );
    for name in ["error", "abort", "kill"] {
        cases.insert(name.into(), supervisor::exercise(&out, name)?);
        let restart = format!("after-{name}-restart");
        cases.insert(
            restart.clone(),
            sample_case(&out.join(restart), 128, 4, false, None)?,
        );
    }
    let checks = vec![
        "exact_128_event_acceptance",
        "tcp_state_budget_overflow_rejected",
        "real_map_write_loss_rejected",
        "event_budget_overflow_rejected",
        "unreported_snapshot_gap_rejected",
        "no_partial_packet_exports",
        "worker_error_unwinds",
        "abort_and_sigkill_detach_both_links",
        "owned_orphans_reaped",
        "fresh_capture_after_each_exit",
    ];
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({
            "schema_version":1,"ok":true,"mode":"capture-recovery","kernel":fs::read_to_string("/proc/sys/kernel/osrelease")?.trim(),
            "cases":cases,"checks":checks,"ingest_trust_mode":"unverified-local",
            "not_proven":["sustained-load-performance","production-ring-buffer-loss","supervisor-SIGKILL-or-host-reboot","kill-during-pre-ready-attachment","multi-host-kernel-matrix"]
        }))?,
    )?;
    Ok(ValidationReport {
        name: "Linux capture loss, limits and crash recovery".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}
