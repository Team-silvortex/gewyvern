use std::fs::{self, OpenOptions};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, TcpListener, TcpStream};
use std::os::fd::OwnedFd;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use socket2::{Domain, Protocol, Socket, Type};

use super::{CaptureEvent, ValidationError, ValidationReport, reconstruct_at};
use crate::bounded_process::{OutputLimits, run_command_output};
use crate::export::ExportBundle;
use crate::validation_harness::command::{default_out_dir, repo_root};

const TOOL_TIMEOUT: Duration = Duration::from_secs(20);

mod bpf_link;
mod docker_target;
mod http_boundaries;
mod http_evidence;
mod juice_shop;
mod load;
mod packet_capture;
mod recovery;
mod stream;

pub(super) fn run_juice_shop(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    juice_shop::run(out)
}

pub(super) fn run_http_boundaries(
    out: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    http_boundaries::run(out)
}

pub(super) fn run_capture_recovery(
    out: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    recovery::run(out)
}

pub(super) fn run_capture_load(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    load::run(out)
}

pub(super) fn run_capture_stream(
    out: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    stream::run(out, false)
}

pub(super) fn run_capture_runtime(
    out: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    stream::run(out, true)
}

pub(super) fn run_capture_mixed(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    stream::mixed::run(out)
}

struct Pins(PathBuf, Option<OwnedFd>);
impl Pins {
    fn new() -> Result<Self, ValidationError> {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ValidationError::new(e.to_string()))?
            .as_nanos();
        let path = Path::new("/sys/fs/bpf")
            .join(format!("gewyvern-capture-{}-{nonce}", std::process::id()));
        fs::create_dir(&path)?;
        Ok(Self(path, None))
    }
    fn own_trace_link(&mut self) -> Result<(), ValidationError> {
        let path = self.0.join("link");
        self.1 = Some(bpf_link::open_pinned(&path)?);
        // No persistent link remains once setup finishes: even SIGKILL closes
        // the process-owned descriptor and detaches the tracepoint.
        fs::remove_file(path)?;
        Ok(())
    }
    fn detach(&mut self) {
        self.1.take();
    }
    fn cleanup(&self) -> Result<(), ValidationError> {
        fs::remove_dir_all(&self.0)?;
        Ok(())
    }
}
impl Drop for Pins {
    fn drop(&mut self) {
        self.detach();
        if self.0.exists()
            && let Err(error) = fs::remove_dir_all(&self.0)
        {
            eprintln!(
                "capture pin cleanup failed for {}: {error}",
                self.0.display()
            );
        }
    }
}

fn tool(command: &mut Command, out: &Path) -> Result<Vec<u8>, ValidationError> {
    let mut log = OpenOptions::new()
        .create(true)
        .append(true)
        .open(out.join("tools.log"))?;
    writeln!(log, "{command:?}")?;
    let output = run_command_output(
        command,
        TOOL_TIMEOUT,
        OutputLimits::new(256 * 1024, 64 * 1024),
        "TCP capture tool",
    )
    .map_err(|e| ValidationError::new(e.to_string()))?;
    log.write_all(&output.stderr)?;
    if !output.status.success() {
        return Err(ValidationError::new(format!(
            "TCP capture tool failed: {}",
            String::from_utf8_lossy(&output.stderr)
        )));
    }
    Ok(output.stdout)
}

#[derive(Deserialize)]
struct MapEntry<T> {
    key: u32,
    value: T,
}
#[derive(Deserialize)]
#[serde(untagged)]
enum MapRecord<T> {
    Typed(MapEntry<T>),
    Btf { formatted: MapEntry<T> },
}
fn map_entries<T: serde::de::DeserializeOwned>(
    bytes: &[u8],
) -> Result<Vec<MapEntry<T>>, ValidationError> {
    let records: Vec<MapRecord<T>> = serde_json::from_slice(bytes)?;
    Ok(records
        .into_iter()
        .map(|record| match record {
            MapRecord::Typed(entry) | MapRecord::Btf { formatted: entry } => entry,
        })
        .collect())
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
struct Counts {
    total: u64,
    dropped: u64,
}

fn map_dump(pins: &Pins, name: &str, out: &Path) -> Result<Vec<u8>, ValidationError> {
    tool(
        Command::new("bpftool")
            .args(["-j", "map", "dump", "pinned"])
            .arg(pins.0.join("maps").join(name)),
        out,
    )
}

fn counts(pins: &Pins, out: &Path) -> Result<Counts, ValidationError> {
    let bytes = map_dump(pins, "counts", out)?;
    fs::write(out.join("counts-raw.json"), &bytes)?;
    let entries: Vec<MapEntry<Counts>> = map_entries(&bytes)?;
    match entries.as_slice() {
        [entry] if entry.key == 0 => Ok(entry.value),
        _ => Err(ValidationError::new("invalid TCP capture counters")),
    }
}

fn snapshot(
    pins: &Pins,
    out: &Path,
    name: &str,
) -> Result<(Vec<CaptureEvent>, Counts), ValidationError> {
    let before = counts(pins, out)?;
    if before.total > 128 || before.dropped > 0 {
        let code = if before.total > 128 {
            "capacity_exceeded"
        } else {
            "reported_loss"
        };
        fs::write(
            out.join("tcp-rejection.json"),
            serde_json::to_vec_pretty(&json!({
                "schema_version":1,"accepted":false,"code":code,"counts":before,"tcp_export_produced":false
            }))?,
        )?;
        return Err(ValidationError::new(format!(
            "TCP capture rejected: {code}"
        )));
    }
    let bytes = map_dump(pins, "events", out)?;
    fs::write(out.join(format!("{name}-raw.json")), &bytes)?;
    let after = counts(pins, out)?;
    if before != after || after.dropped != 0 || after.total > 128 {
        return Err(ValidationError::new(
            "TCP capture changed during snapshot or lost events",
        ));
    }
    let entries: Vec<MapEntry<CaptureEvent>> = map_entries(&bytes)?;
    let mut events = Vec::with_capacity(entries.len());
    for entry in entries {
        if entry.key >= 128 || entry.value.sequence != u64::from(entry.key) + 1 {
            return Err(ValidationError::new("TCP capture slot/sequence mismatch"));
        }
        events.push(entry.value);
    }
    events.sort_by_key(|event| event.sequence);
    if events.len() as u64 != after.total {
        return Err(ValidationError::new("incomplete TCP capture snapshot"));
    }
    Ok((events, after))
}

fn wait_state(
    pins: &Pins,
    out: &Path,
    name: &str,
    cookie: u64,
    state: u8,
) -> Result<(Vec<CaptureEvent>, Counts), ValidationError> {
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        let result = snapshot(pins, out, name)?;
        if result
            .0
            .iter()
            .any(|event| event.cookie == cookie && event.new_state == state)
        {
            return Ok(result);
        }
        if Instant::now() >= deadline {
            return Err(ValidationError::new(format!(
                "missing real TCP state {state} for owned socket"
            )));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn owned_socket() -> Result<Socket, ValidationError> {
    owned_socket_on(Ipv4Addr::LOCALHOST)
}

fn owned_socket_on(source: Ipv4Addr) -> Result<Socket, ValidationError> {
    let socket = Socket::new(Domain::IPV4, Type::STREAM, Some(Protocol::TCP))?;
    socket.bind(&SocketAddr::from((source, 0)).into())?;
    socket.set_read_timeout(Some(Duration::from_secs(2)))?;
    socket.set_write_timeout(Some(Duration::from_secs(2)))?;
    Ok(socket)
}
fn port(socket: &Socket) -> Result<u16, ValidationError> {
    socket
        .local_addr()?
        .as_socket_ipv4()
        .map(|addr| addr.port())
        .ok_or_else(|| ValidationError::new("owned socket has no IPv4 address"))
}

fn clock_anchor() -> Result<(u64, SystemTime), ValidationError> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    // clock_gettime writes exactly one initialized timespec through this valid pointer.
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    let seconds =
        u64::try_from(time.tv_sec).map_err(|_| ValidationError::new("invalid monotonic clock"))?;
    let nanos = u64::try_from(time.tv_nsec)
        .map_err(|_| ValidationError::new("invalid clock nanoseconds"))?;
    let total = seconds
        .checked_mul(1_000_000_000)
        .and_then(|s| s.checked_add(nanos))
        .filter(|_| nanos < 1_000_000_000)
        .ok_or_else(|| ValidationError::new("clock overflow"))?;
    Ok((total, SystemTime::now()))
}

fn export_stage(
    events: &[CaptureEvent],
    stats: Counts,
    expected: &[(u64, u16, u16, u32)],
    clock: (u64, SystemTime),
    out: &Path,
    name: &str,
) -> Result<ExportBundle, ValidationError> {
    export_stage_at(
        events,
        stats,
        expected,
        clock,
        out,
        name,
        ([127, 0, 0, 1], [127, 0, 0, 1]),
    )
}

fn export_stage_at(
    events: &[CaptureEvent],
    stats: Counts,
    expected: &[(u64, u16, u16, u32)],
    clock: (u64, SystemTime),
    out: &Path,
    name: &str,
    addresses: ([u8; 4], [u8; 4]),
) -> Result<ExportBundle, ValidationError> {
    let export = reconstruct_at(
        events,
        expected,
        stats.total,
        stats.dropped,
        clock,
        addresses,
    )?;
    let encoded = export.to_json();
    fs::write(out.join(format!("{name}-export.json")), &encoded)?;
    let decoded =
        ExportBundle::from_json(&encoded).map_err(|e| ValidationError::new(format!("{e:?}")))?;
    let replay = decoded
        .replay()
        .map_err(|e| ValidationError::new(format!("{e:?}")))?;
    if replay.facts != export.facts
        || replay.flows != export.flows
        || replay.reasons != export.reasons
    {
        return Err(ValidationError::new(
            "captured TCP evidence did not replay identically",
        ));
    }
    Ok(export)
}

fn expect_lifecycle(
    export: &ExportBundle,
    cookie: u64,
    state: u8,
    terminated: bool,
) -> Result<(), ValidationError> {
    let fact = export
        .facts
        .iter()
        .find(|fact| {
            matches!(&fact.kind,
        crate::ledger::FactKind::TcpState(value) if value.sk_cookie == cookie)
        })
        .ok_or_else(|| ValidationError::new("missing captured socket flow"))?;
    let flow = export
        .flows
        .iter()
        .find(|flow| flow.evidence.tcp_state_facts.contains(&fact.id))
        .ok_or_else(|| ValidationError::new("missing reconstructed TCP flow"))?;
    if flow.lifecycle.tcp_state_now != Some(state)
        || flow.lifecycle.terminated != terminated
        || flow.process.is_some()
        || !flow.evidence.packet_facts.is_empty()
    {
        return Err(ValidationError::new(format!(
            "incorrect lifecycle or invented evidence for TCP state {state}"
        )));
    }
    Ok(())
}

fn prepare_output(out_dir: Option<PathBuf>, name: &str) -> Result<PathBuf, ValidationError> {
    let out = out_dir.unwrap_or_else(|| default_out_dir(name));
    if let Some(parent) = out.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::create_dir(&out)?;
    if !Path::new("/sys/kernel/btf/vmlinux").exists() {
        return Err(ValidationError::new("TCP capture requires kernel BTF"));
    }
    Ok(out)
}

fn network_namespace() -> Result<u32, ValidationError> {
    let netns = fs::read_link("/proc/self/ns/net")?
        .to_string_lossy()
        .into_owned();
    netns
        .strip_prefix("net:[")
        .and_then(|s| s.strip_suffix(']'))
        .ok_or_else(|| ValidationError::new("invalid network namespace identity"))?
        .parse()
        .map_err(|_| ValidationError::new("invalid network namespace number"))
}

fn attach_capture(
    cookies: [u64; 2],
    out: &Path,
) -> Result<(Pins, (u64, SystemTime)), ValidationError> {
    attach_capture_at(cookies, out, ([127, 0, 0, 1], [127, 0, 0, 1]))
}

fn attach_capture_at(
    cookies: [u64; 2],
    out: &Path,
    addresses: ([u8; 4], [u8; 4]),
) -> Result<(Pins, (u64, SystemTime)), ValidationError> {
    if cookies[0] == 0 || cookies[1] == 0 || cookies[0] == cookies[1] {
        return Err(ValidationError::new("invalid owned SO_COOKIE identities"));
    }
    let object = out.join("tcp-state.bpf.o");
    tool(
        Command::new("clang")
            .args(["-O2", "-g", "-target", "bpf", "-I/usr/include"])
            .arg(format!(
                "-I/usr/include/{}-linux-gnu",
                std::env::consts::ARCH
            ))
            .arg(format!("-DCOOKIE_A={}ULL", cookies[0]))
            .arg(format!("-DCOOKIE_B={}ULL", cookies[1]))
            .arg(format!(
                "-DSOURCE_IPV4={}U",
                u32::from_be_bytes(addresses.0)
            ))
            .arg(format!(
                "-DDESTINATION_IPV4={}U",
                u32::from_be_bytes(addresses.1)
            ))
            .arg("-c")
            .arg(repo_root().join("ebpf/validation/tcp_state.bpf.c"))
            .arg("-o")
            .arg(&object),
        out,
    )?;
    let mut pins = Pins::new()?;
    fs::create_dir(pins.0.join("maps"))?;
    tool(
        Command::new("bpftool")
            .args(["prog", "load"])
            .arg(&object)
            .arg(pins.0.join("link"))
            .arg("pinmaps")
            .arg(pins.0.join("maps"))
            .arg("autoattach"),
        out,
    )?;
    pins.own_trace_link()?;
    tool(
        Command::new("bpftool")
            .args(["map", "update", "pinned"])
            .arg(pins.0.join("maps/counts"))
            .args(["key", "hex", "00", "00", "00", "00", "value", "hex"])
            .args(["00"; 16]),
        out,
    )?;
    let clock = clock_anchor()?;
    let (idle, idle_counts) = snapshot(&pins, out, "idle")?;
    if !idle.is_empty() || idle_counts.total != 0 {
        return Err(ValidationError::new("capture was not initially empty"));
    }
    Ok((pins, clock))
}

pub(super) fn run(out_dir: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out_dir, "linux-tcp-capture")?;
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0))?;
    listener.set_nonblocking(true)?;
    let target = listener.local_addr()?;
    let client = owned_socket()?;
    let refused = owned_socket()?;
    let reserved = owned_socket()?; // Bound but never listening; no port-allocation race.
    let cookies = [client.cookie()?, refused.cookie()?];
    let netns_id = network_namespace()?;
    let expected = [
        (cookies[0], port(&client)?, target.port(), netns_id),
        (cookies[1], port(&refused)?, port(&reserved)?, netns_id),
    ];
    let (mut pins, clock) = attach_capture(cookies, &out)?;

    client.connect_timeout(&target.into(), Duration::from_secs(2))?;
    let mut stream: TcpStream = client.into();
    let (mut peer, peer_address) = listener.accept()?;
    peer.set_read_timeout(Some(Duration::from_secs(2)))?;
    peer.set_write_timeout(Some(Duration::from_secs(2)))?;
    if peer_address.port() != expected[0].1 {
        return Err(ValidationError::new("accepted peer identity mismatch"));
    }
    let request = b"GET /gewyvern-capture HTTP/1.1\r\nHost: localhost\r\n\r\n";
    let response = b"HTTP/1.1 204 No Content\r\nContent-Length: 0\r\n\r\n";
    stream.write_all(request)?;
    let mut bytes = vec![0; request.len()];
    peer.read_exact(&mut bytes)?;
    if bytes != request {
        return Err(ValidationError::new("loopback request mismatch"));
    }
    peer.write_all(response)?;
    bytes.resize(response.len(), 0);
    stream.read_exact(&mut bytes)?;
    if bytes != response {
        return Err(ValidationError::new("loopback response mismatch"));
    }
    let (events, stats) = wait_state(&pins, &out, "established", cookies[0], 1)?;
    let export = export_stage(&events, stats, &expected, clock, &out, "established")?;
    expect_lifecycle(&export, cookies[0], 1, false)?;

    drop(peer);
    let (events, stats) = wait_state(&pins, &out, "half-closed", cookies[0], 8)?;
    let export = export_stage(&events, stats, &expected, clock, &out, "half-closed")?;
    expect_lifecycle(&export, cookies[0], 8, false)?;

    let refusal = refused.connect_timeout(
        &SocketAddr::from((Ipv4Addr::LOCALHOST, port(&reserved)?)).into(),
        Duration::from_secs(2),
    );
    if !matches!(refusal, Err(ref e) if e.kind() == std::io::ErrorKind::ConnectionRefused) {
        return Err(ValidationError::new(
            "owned non-listening port did not refuse connect",
        ));
    }
    let decoy = owned_socket()?;
    let decoy_cookie = decoy.cookie()?;
    decoy.connect_timeout(&target.into(), Duration::from_secs(2))?;
    let (decoy_peer, _) = listener.accept()?;
    drop(decoy_peer);
    drop(decoy);
    drop(stream);
    let (events, _) = wait_state(&pins, &out, "closed", cookies[0], 7)?;
    if !events
        .iter()
        .any(|e| e.cookie == cookies[1] && e.old_state == 2 && e.new_state == 7)
    {
        return Err(ValidationError::new(
            "refused connect missing kernel SYN_SENT -> CLOSE evidence",
        ));
    }
    pins.detach();
    let (events, stats) = snapshot(&pins, &out, "final")?;
    let export = export_stage(&events, stats, &expected, clock, &out, "final")?;
    expect_lifecycle(&export, cookies[0], 7, true)?;
    expect_lifecycle(&export, cookies[1], 7, true)?;
    if export.flows.len() != 2 || events.iter().any(|e| e.cookie == decoy_cookie) {
        return Err(ValidationError::new(
            "unselected socket leaked into capture",
        ));
    }
    pins.cleanup()?;
    let summary: Value = json!({
        "schema_version":1, "scope":"owned-ipv4-loopback-tcp-state-only", "ok":true,
        "kernel":fs::read_to_string("/proc/sys/kernel/osrelease")?.trim(),
        "selected_sockets":expected, "decoy_cookie":decoy_cookie, "decoy_events":0,
        "kernel_events":stats.total, "dropped_events":stats.dropped, "flows":export.flows.len(),
        "http_status_observed_by_client":204, "http_status_inferred_by_gewyvern":false,
        "pid_attribution":false, "ingest_trust_mode":"unverified-local",
        "timestamps":"CLOCK_MONOTONIC mapped to wall-clock anchor; original nanoseconds retained",
        "pin_directory":pins.0, "pins_removed":true,
        "checks":["real_kernel_tcp_events","exact_socket_cookie_and_tuple_correlation",
          "live_http_exchange","close_wait_is_not_terminated","refused_connect",
          "decoy_exclusion","zero_capture_loss","deterministic_replay","owned_pin_cleanup"],
        "not_proven":["payload-classification","general-live-cli-capture","pid-attribution","sustained-load"]
    });
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    Ok(ValidationReport {
        name: "Linux TCP state capture and reconstruction".into(),
        out_dir: out,
        checks: summary["checks"]
            .as_array()
            .ok_or_else(|| ValidationError::new("missing capture checks"))?
            .iter()
            .filter_map(|v| v.as_str().map(str::to_owned))
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bpftool_typed_and_btf_wrapped_counts_are_decoded_without_guessing_raw_bytes() {
        for input in [
            r#"[{"key":0,"value":{"total":2,"dropped":0}}]"#,
            r#"[{"key":["0x00","0x00","0x00","0x00"],"value":["0x02"],"formatted":{"key":0,"value":{"total":2,"dropped":0}}}]"#,
        ] {
            let entries = map_entries::<Counts>(input.as_bytes()).unwrap();
            assert_eq!(entries.len(), 1);
            assert_eq!(entries[0].key, 0);
            assert_eq!(entries[0].value.total, 2);
            assert_eq!(entries[0].value.dropped, 0);
        }
        for input in [
            r#"[{"key":["0x00"],"value":["0x02"]}]"#,
            r#"[{"formatted":{"key":0,"value":{"total":2}}}]"#,
            r#"[{"formatted":{"key":-1,"value":{"total":2,"dropped":0}}}]"#,
            r#"[{"key":0,"value":{"total":-1,"dropped":0}}]"#,
        ] {
            assert!(map_entries::<Counts>(input.as_bytes()).is_err());
        }
    }
}
