use std::fs;
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

use super::packet_capture::{Capture, Scope};
use super::{
    ValidationError, ValidationReport, network_namespace, owned_socket, port, prepare_output, tool,
};

pub(super) mod metrics;
pub(super) mod traffic;

const SECONDS: u64 = 60;

pub(super) fn memory_summary(samples: &[Value], key: &str) -> Result<Value, ValidationError> {
    let values: Vec<u64> = samples
        .iter()
        .map(|s| {
            s[key]
                .as_u64()
                .ok_or_else(|| ValidationError::new("missing memory observation"))
        })
        .collect::<Result<_, _>>()?;
    let first = *values
        .first()
        .ok_or_else(|| ValidationError::new("no memory observations"))?;
    let last = *values
        .last()
        .ok_or_else(|| ValidationError::new("no memory observations"))?;
    Ok(
        json!({"first":first,"last":last,"peak":values.iter().max(),"last_minus_first":i128::from(last)-i128::from(first)}),
    )
}

pub(super) struct Fixture {
    pub(super) scope: Scope,
    pub(super) pairs: Vec<(TcpStream, TcpStream)>,
    pub(super) expected: [(u64, u16, u16, u32); 2],
}

pub(super) fn fixture() -> Result<Fixture, ValidationError> {
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
    // Only this fresh cgroup is modified; never enable controllers globally.
    for (file, value) in [
        ("cpu.max", "200000 100000"),
        ("memory.max", "536870912"),
        ("pids.max", "32"),
    ] {
        let path = scope.path().join(file);
        if !path.is_file() {
            return Err(ValidationError::new(format!(
                "load test requires delegated {file}"
            )));
        }
        fs::write(path, value)?;
    }
    let mut expected = [(0, 0, 0, network_namespace()?); 2];
    let mut pairs = Vec::new();
    for entry in &mut expected {
        let socket = owned_socket()?;
        *entry = (socket.cookie()?, port(&socket)?, target.port(), entry.3);
        socket.connect_timeout(&target.into(), Duration::from_secs(2))?;
        let client: TcpStream = socket.into();
        let (peer, address) = listener.accept()?;
        if address.port() != entry.1 {
            return Err(ValidationError::new("load peer identity mismatch"));
        }
        pairs.push((client, peer));
    }
    Ok(Fixture {
        scope,
        pairs,
        expected,
    })
}

pub(super) fn phase(out: &Path, enabled: bool) -> Result<Value, ValidationError> {
    let out = prepare_output(Some(out.to_owned()), "capture-load")?;
    let Fixture {
        mut scope,
        pairs,
        expected,
    } = fixture()?;
    // Attach after the handshake: observed events are response payloads only.
    let capture = enabled
        .then(|| Capture::attach(&scope, &expected, ([127, 0, 0, 1], [127, 0, 0, 1]), &out))
        .transpose()?;
    let ownership = capture
        .as_ref()
        .map(|c| Ok::<_, ValidationError>((c.path().to_owned(), c.link_id()?)))
        .transpose()?;
    let traffic = traffic::Traffic::spawn(pairs)?;
    let warmup_requests = traffic.warmup()?;
    let before = capture
        .as_ref()
        .map(|c| c.sample_counts(&out))
        .transpose()?;
    if before.is_some_and(|c| c.total <= 128 || c.dropped != c.total - 128) {
        return Err(ValidationError::new(
            "capture did not reach its expected capacity during warmup",
        ));
    }
    let cpu_before = metrics::Cpu::read(scope.path())?;
    let mut samples = vec![metrics::memory(scope.path(), 0.0)?];
    let started = Instant::now();
    let deadline = started + Duration::from_secs(SECONDS);
    traffic.start(deadline)?;
    while Instant::now() < deadline && !traffic.failed() {
        std::thread::sleep(
            Duration::from_secs(1).min(deadline.saturating_duration_since(Instant::now())),
        );
        samples.push(metrics::memory(
            scope.path(),
            started.elapsed().as_secs_f64(),
        )?);
    }
    let latency = traffic.finish()?;
    let elapsed = started.elapsed().as_secs_f64();
    let cpu = cpu_before.delta(&metrics::Cpu::read(scope.path())?, elapsed)?;
    if latency.count() == 0 || elapsed < SECONDS as f64 {
        return Err(ValidationError::new(
            "load phase ended without a full nonempty observation window",
        ));
    }
    let capture_report = if let Some(capture) = capture {
        let after = capture.sample_counts(&out)?;
        let before =
            before.ok_or_else(|| ValidationError::new("missing pre-load capture counters"))?;
        if after.total < warmup_requests + latency.count() || after.dropped != after.total - 128 {
            return Err(ValidationError::new(
                "capture counters diverged from bounded overload accounting",
            ));
        }
        let measured_total = after
            .total
            .checked_sub(before.total)
            .ok_or_else(|| ValidationError::new("capture counter reset"))?;
        let measured_dropped = after
            .dropped
            .checked_sub(before.dropped)
            .ok_or_else(|| ValidationError::new("drop counter reset"))?;
        if measured_total < latency.count() || measured_total != measured_dropped {
            return Err(ValidationError::new(
                "saturated capture accounting mismatch",
            ));
        }
        if capture.finish(&out).is_ok() {
            return Err(ValidationError::new(
                "overloaded capture was incorrectly accepted",
            ));
        }
        let rejection: Value =
            serde_json::from_slice(&fs::read(out.join("packet-rejection.json"))?)?;
        if rejection["code"] != "capacity_exceeded" || out.join("packet-export.json").exists() {
            return Err(ValidationError::new(
                "overloaded capture published or misclassified evidence",
            ));
        }
        json!({"enabled":true,"state_during_measurement":"capacity-exhausted-discard-path",
            "warmup_counts":before,"final_counts":after,"retained_event_slots":128,
            "measured_events":measured_total,"measured_rejected_events":measured_dropped,
            "measured_rejection_percent":100.0 * measured_dropped as f64 / measured_total as f64,
            "whole_run_rejection_percent":100.0 * after.dropped as f64 / after.total as f64,
            "complete_export_produced":false,"rejection":rejection,
            "unexplained_counter_loss":0,"loss_interpretation":"fixed-budget-rejection-not-streaming-throughput-loss"})
    } else {
        json!({"enabled":false})
    };
    if let Some((path, id)) = ownership {
        let bytes = tool(Command::new("bpftool").args(["-j", "link", "show"]), &out)?;
        let links: Vec<Value> = serde_json::from_slice(&bytes)?;
        if path.exists() || links.iter().any(|l| l["id"] == id) {
            return Err(ValidationError::new(
                "load capture resources remained attached",
            ));
        }
    }
    let memory_events = fs::read_to_string(scope.path().join("memory.events"))?;
    if memory_events
        .lines()
        .any(|line| line.split_whitespace().nth(1) != Some("0"))
    {
        return Err(ValidationError::new(
            "load cgroup reached a memory pressure/OOM limit",
        ));
    }
    let report = json!({"schema_version":1,"ok":true,"capture":capture_report,"elapsed_seconds":elapsed,
        "warmup_requests":warmup_requests,"requests":latency.count(),"requests_per_second":latency.count() as f64 / elapsed,
        "latency":latency.report(),"transport_errors":0,"byte_integrity_errors":0,"cpu":cpu,
        "rss_kib":memory_summary(&samples,"rss_kib")?,"anonymous_rss_kib":memory_summary(&samples,"anonymous_rss_kib")?,
        "cgroup_memory_bytes":memory_summary(&samples,"cgroup_memory_bytes")?,"memory_samples":samples,
        "memory_events":memory_events,"loadavg_after":fs::read_to_string("/proc/loadavg")?.trim()});
    scope.cleanup()?;
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}

pub(super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out, "linux-capture-load")?;
    let mut phases = Vec::new();
    for (name, enabled) in [
        ("baseline-1", false),
        ("capture-1", true),
        ("capture-2", true),
        ("baseline-2", false),
    ] {
        eprintln!("capture load: {name}, {SECONDS}s measurement after 2s warmup");
        let result = phase(&out.join(name), enabled)?;
        phases.push(json!({"name":name,"result":result}));
    }
    super::http_boundaries::run(Some(out.join("after-pressure-restart")))?;
    let rates: Vec<f64> = phases
        .iter()
        .map(|p| {
            p["result"]["requests_per_second"]
                .as_f64()
                .ok_or_else(|| ValidationError::new("missing phase throughput"))
        })
        .collect::<Result<_, _>>()?;
    let baseline = (rates[0] + rates[3]) / 2.0;
    let captured = (rates[1] + rates[2]) / 2.0;
    let checks = vec![
        "four_sustained_phases",
        "owned_two_socket_traffic",
        "byte_integrity",
        "bounded_cpu_memory_pids",
        "capacity_loss_accounted_not_hidden",
        "no_incomplete_exports",
        "per_phase_resources_removed",
        "fresh_capture_and_replay_after_pressure",
    ];
    let summary = json!({"schema_version":1,"ok":true,"mode":"capture-load","kernel":fs::read_to_string("/proc/sys/kernel/osrelease")?.trim(),
        "measurement_seconds_per_phase":SECONDS,"warmup_seconds":2,"concurrency":2,"request_bytes":64,"response_bytes":64,
        "limits":{"cpu_cores":2,"memory_bytes":536870912,"pids":32},
        "driver":"native-Rust-closed-loop-one-inflight-per-client","capture_backend":"128-slot-validation-hash-map",
        "phases":phases,"comparison":{"baseline_mean_requests_per_second":baseline,"saturated_capture_mean_requests_per_second":captured,
            "saturated_capture_throughput_change_percent":(captured/baseline-1.0)*100.0,
            "baseline_repeat_change_percent":(rates[3]/rates[0]-1.0)*100.0,"capture_repeat_change_percent":(rates[2]/rates[1]-1.0)*100.0},
        "checks":checks,"ingest_trust_mode":"unverified-local","post_pressure_restart":"passed",
        "metric_scope":{"cpu":"whole-test-cgroup-including-clients-servers-sampler-not-isolated-BPF-cost",
            "memory":"process-RSS-and-whole-cgroup-including-fixed-histograms-map-and-kernel-charges",
            "latency":"closed-loop-application-request-response-RTT-not-capture-delivery-latency",
            "cpu_attribution_limit":"kernel-work-outside-test-cgroup-may-not-be-charged",
            "steady_state":"map-already-full-after-warmup-no-continuous-drain"},
        "not_proven":["lossless-sustained-capture","production-ring-buffer-throughput","full-runtime-ingest-performance",
            "open-loop-offered-load-latency","wire-packet-rate","isolated-BPF-CPU-overhead","multi-host-kernel-matrix","long-duration-memory-leak-freedom"]});
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    Ok(ValidationReport {
        name: "Linux sustained bounded-capture pressure and baseline".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}
