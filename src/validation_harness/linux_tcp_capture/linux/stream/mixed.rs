//! Bounded packetization/identity correctness, not a throughput benchmark.
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

use super::super::load::{self, memory_summary, metrics};
use super::super::packet_capture::Capture;
use super::super::{ValidationError, ValidationReport, prepare_output, tool};
use super::ring::Ring;
use super::runtime_sink::RuntimeSink;
use super::{Counts, reconcile};

mod ledger;
mod traffic;

fn phase(out: &Path, epoch: usize, mode: traffic::Mode) -> Result<Value, ValidationError> {
    let out = prepare_output(Some(out.to_owned()), "capture-mixed")?;
    let (fixture, (neighbor_client, neighbor_peer, neighbor_cookie)) =
        load::fixture_with_neighbor()?;
    let load::Fixture {
        mut scope,
        mut pairs,
        expected,
    } = fixture;
    let mut capture = Capture::attach_stream(&scope, &expected, &out)?;
    let ownership = (capture.path().to_owned(), capture.link_id()?);
    let mut ledger = ledger::Ledger::new(expected, [mode.bytes(0), mode.bytes(1)], epoch);
    let mut runtime = RuntimeSink::new()?;
    let mut ring = Ring::new(capture.stream_map()?, |record| {
        let netns = ledger.observe(record)?;
        runtime.accept(record, netns, true)
    })?;
    pairs.push((neighbor_client, neighbor_peer));
    let cpu_start = metrics::Cpu::read(scope.path())?;
    let mut samples = vec![metrics::memory(scope.path(), 0.0)?];
    let start = Instant::now();
    let traffic = traffic::Traffic::spawn(pairs, epoch, mode)?;
    let mut next_sample = start + Duration::from_millis(200);
    while !traffic.finished() {
        if traffic.failed() || start.elapsed() > Duration::from_secs(45) {
            return Err(ValidationError::new(
                "mixed traffic failed or exceeded bounded deadline",
            ));
        }
        if ring.consume()? == 0 {
            std::thread::sleep(Duration::from_millis(1));
        }
        if Instant::now() >= next_sample {
            samples.push(metrics::memory(
                scope.path(),
                start.elapsed().as_secs_f64(),
            )?);
            next_sample = Instant::now() + Duration::from_millis(200);
        }
    }
    let applications = traffic.finish()?;
    capture.detach();
    let drain_deadline = Instant::now() + Duration::from_secs(3);
    while ring.consume()? > 0 {
        if Instant::now() > drain_deadline {
            return Err(ValidationError::new("mixed final drain timed out"));
        }
    }
    drop(ring);
    runtime.flush(true)?;
    let observation = ledger.finish()?;
    let analysis = runtime.report()?;
    let counts: Counts = capture.stream_counts(&out)?;
    let received = observation["received_events"]
        .as_u64()
        .ok_or_else(|| ValidationError::new("missing mixed event count"))?;
    if !reconcile(counts, received, 0)?
        || analysis["analyzed_facts"] != received
        || analysis["measured_analyzed_facts"] != received
    {
        return Err(ValidationError::new(
            "mixed capture was lossy or runtime analysis diverged",
        ));
    }
    if applications.len() != 3
        || applications
            .iter()
            .enumerate()
            .any(|(id, report)| report["response_bytes"] != mode.bytes(id))
    {
        return Err(ValidationError::new(
            "mixed application totals diverged from capture oracle",
        ));
    }
    samples.push(metrics::memory(
        scope.path(),
        start.elapsed().as_secs_f64(),
    )?);
    let cpu = cpu_start.delta(
        &metrics::Cpu::read(scope.path())?,
        start.elapsed().as_secs_f64(),
    )?;
    let events = fs::read_to_string(scope.path().join("memory.events"))?;
    if events
        .lines()
        .any(|line| line.split_whitespace().nth(1) != Some("0"))
    {
        return Err(ValidationError::new("mixed cgroup memory pressure or OOM"));
    }
    let report = json!({"schema_version":1,"ok":true,"epoch":epoch,"mode":format!("{mode:?}"),
        "elapsed_seconds":start.elapsed().as_secs_f64(),"selected_cookies":expected.map(|e| e.0),
        "unselected_neighbor_cookie":neighbor_cookie,"negative_control":"same-cgroup-same-listener-unselected-client-with-completed-full-byte-echo",
        "applications":applications,"capture":observation,"runtime":analysis,"kernel_counts":counts,
        "cpu":cpu,"rss_kib":memory_summary(&samples,"rss_kib")?,"memory_samples":samples,
        "memory_events":events,"full_payload_capture":false,"full_byte_application_integrity":true});
    drop(capture);
    let links: Vec<Value> = serde_json::from_slice(&tool(
        Command::new("bpftool").args(["-j", "link", "show"]),
        &out,
    )?)?;
    if ownership.0.exists() || links.iter().any(|link| link["id"] == ownership.1) {
        return Err(ValidationError::new(
            "mixed capture resources survived cleanup",
        ));
    }
    scope.cleanup()?;
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}

pub(in super::super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(out, "linux-capture-mixed")?;
    let mut phases = Vec::new();
    let mut cookies = BTreeSet::new();
    for (epoch, mode) in [
        traffic::Mode::Variable,
        traffic::Mode::Split,
        traffic::Mode::Coalesced,
        traffic::Mode::Variable,
    ]
    .into_iter()
    .enumerate()
    {
        eprintln!("capture mixed: epoch {epoch}, {mode:?}");
        let report = phase(&out.join(format!("epoch-{epoch}")), epoch, mode)?;
        for cookie in report["selected_cookies"]
            .as_array()
            .into_iter()
            .flatten()
            .chain(std::iter::once(&report["unselected_neighbor_cookie"]))
        {
            let cookie = cookie
                .as_u64()
                .ok_or_else(|| ValidationError::new("missing mixed socket identity"))?;
            if !cookies.insert(cookie) {
                return Err(ValidationError::new(
                    "mixed epochs reused a socket identity",
                ));
            }
        }
        phases.push(report);
    }
    if !phases
        .iter()
        .any(|p| p["capture"]["short_payload_events"].as_u64().unwrap_or(0) > 0)
        || !phases.iter().any(|p| {
            p["capture"]["truncated_payload_events"]
                .as_u64()
                .unwrap_or(0)
                > 0
        })
        || phases
            .iter()
            .all(|p| p["capture"]["received_events"] == (traffic::MESSAGES * 2) as u64)
    {
        return Err(ValidationError::new(
            "mixed campaign did not exercise packetization boundaries",
        ));
    }
    super::super::http_boundaries::run(Some(out.join("after-mixed-restart")))?;
    let checks = [
        "variable_and_truncated_prefixes",
        "split_and_coalesced_writes",
        "TCP_byte_ranges_not_message_counts",
        "same_endpoint_unselected_neighbor_excluded",
        "half_close_preserves_last_response",
        "fresh_socket_identities_across_epochs",
        "bounded_runtime_analysis_and_replay",
        "owned_resource_cleanup",
        "fresh_HTTP_capture_replay_after_mixed",
    ];
    let summary = json!({"schema_version":1,"ok":true,"mode":"capture-mixed","kernel":fs::read_to_string("/proc/sys/kernel/osrelease")?.trim(),
        "limits":{"cpu_cores":2,"memory_bytes":536870912,"pids":32,"phase_deadline_seconds":45},
        "ring_bytes":1048576,"batch_limit":256,"logical_sizes":traffic::SIZES,"logical_messages_per_client_per_epoch":traffic::MESSAGES,
        "unique_sockets":cookies.len(),"phases":phases,"checks":checks,"ingest_trust_mode":"unverified-local",
        "metric_scope":"whole-test-cgroup-CPU-and-process-RSS;finite-correctness-campaign-not-throughput-benchmark",
        "not_proven":["general-TCP-retransmission-or-reordering-reassembly","full-payload-capture","protocol-or-vulnerability-detection",
            "cross-batch-session-state","open-loop-tail-latency","long-duration-memory-stability","multi-host-or-kernel-matrix"]});
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    Ok(ValidationReport {
        name: "Linux mixed packetization and bounded runtime validation".into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}
