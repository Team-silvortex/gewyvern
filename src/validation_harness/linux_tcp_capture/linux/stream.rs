use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::load::{self, Fixture, memory_summary, metrics, traffic::Traffic};
use super::packet_capture::Capture;
use super::{ValidationError, ValidationReport, prepare_output, tool};

mod consumer;
pub(super) mod mixed;
mod ring;
mod runtime_sink;
pub(super) mod soak;

#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
struct Counts {
    total: u64,
    dropped: u64,
    malformed: u64,
}

fn reconcile(counts: Counts, received: u64, gaps: u64) -> Result<bool, ValidationError> {
    if counts.total == 0
        || counts.malformed != 0
        || received.checked_add(counts.dropped) != Some(counts.total)
        || gaps > counts.dropped
    {
        return Err(ValidationError::new(
            "stream delivery counters or sequence gaps failed reconciliation",
        ));
    }
    Ok(counts.dropped == 0)
}

fn phase(out: &Path, pause: bool, runtime: bool) -> Result<Value, ValidationError> {
    phase_with_duration(out, pause, runtime, if pause { 10 } else { 60 }, false)
}

fn phase_with_duration(
    out: &Path,
    pause: bool,
    runtime: bool,
    seconds: u64,
    heap_diagnostics: bool,
) -> Result<Value, ValidationError> {
    let out = prepare_output(Some(out.to_owned()), "capture-stream")?;
    let Fixture {
        mut scope,
        pairs,
        expected,
    } = load::fixture()?;
    let mut capture = Capture::attach_stream(&scope, &expected, &out)?;
    let ownership = (capture.path().to_owned(), capture.link_id()?);
    let consumer = consumer::Consumer::start(capture.stream_map()?, expected, runtime)?;
    let traffic = Traffic::spawn(pairs)?;
    let warmup_requests = traffic.warmup()?;
    let warmup_received = consumer.pause_after_drain()?;
    let before: Counts = capture.stream_counts(&out)?;
    if warmup_received.checked_add(before.dropped) != Some(before.total) {
        return Err(ValidationError::new(
            "stream warmup checkpoint is incomplete",
        ));
    }
    let mut samples = Vec::new();
    let mut journal = heap_diagnostics
        .then(|| soak::Journal::new(&out))
        .transpose()?;
    let first_memory = metrics::memory(scope.path(), 0.0)?;
    if let Some(journal) = &mut journal {
        journal.record(&first_memory)?;
    } else {
        samples.push(first_memory);
    }
    let mut sample_count = 1usize;
    let cpu_before = metrics::Cpu::read(scope.path())?;
    if let Some(journal) = &mut journal {
        journal.record_heap(&soak::snapshot(&out, "heap-start")?)?;
    }
    consumer.begin_measurement()?;
    let started = Instant::now();
    if !pause {
        consumer.resume();
    }
    let deadline = started + Duration::from_secs(seconds);
    traffic.start(deadline)?;
    let mut resumed_after = None;
    while Instant::now() < deadline && !traffic.failed() && !consumer.failed() {
        std::thread::sleep(
            Duration::from_secs(1).min(deadline.saturating_duration_since(Instant::now())),
        );
        if pause && resumed_after.is_none() && started.elapsed() >= Duration::from_secs(2) {
            resumed_after = Some(started.elapsed().as_secs_f64());
            consumer.resume();
        }
        let sample = metrics::memory(scope.path(), started.elapsed().as_secs_f64())?;
        sample_count += 1;
        if let Some(journal) = &mut journal {
            journal.record(&sample)?;
        } else {
            samples.push(sample);
        }
        if let Some(journal) = &mut journal
            && sample_count.is_multiple_of(30)
        {
            let mut sample = soak::snapshot(&out, &format!("heap-{sample_count}"))?;
            sample["elapsed_seconds"] = json!(started.elapsed().as_secs_f64());
            journal.record_heap(&sample)?;
            if sample_count.is_multiple_of(60) {
                eprintln!(
                    "capture soak progress: {:.0}s rss={}KiB rust_live={}B",
                    started.elapsed().as_secs_f64(),
                    sample["rss_kib"],
                    sample["rust_allocations"]["live_requested_bytes"]
                );
            }
        }
    }
    if consumer.failed() {
        // Stop producers before joining a failed consumer, rather than waiting out the load window.
        drop(traffic);
        capture.detach();
        consumer.finish()?;
        return Err(ValidationError::new(
            "consumer stopped before the measurement deadline",
        ));
    }
    let rtt = traffic.finish()?;
    capture.detach();
    let stats = consumer.finish()?;
    let elapsed = started.elapsed().as_secs_f64();
    let cpu = cpu_before.delta(&metrics::Cpu::read(scope.path())?, elapsed)?;
    let after: Counts = capture.stream_counts(&out)?;
    let measured_total = after
        .total
        .checked_sub(before.total)
        .ok_or_else(|| ValidationError::new("stream counter reset"))?;
    let measured_dropped = after
        .dropped
        .checked_sub(before.dropped)
        .ok_or_else(|| ValidationError::new("stream drop counter reset"))?;
    let measured_received = stats
        .received
        .checked_sub(warmup_received)
        .ok_or_else(|| ValidationError::new("stream consumer count reset"))?;
    let complete = reconcile(after, stats.received, stats.sequence_gaps)?;
    if elapsed < seconds as f64
        || measured_total != rtt.count()
        || after.total != warmup_requests + rtt.count()
        || measured_received.checked_add(measured_dropped) != Some(measured_total)
        || measured_received <= 8192
    {
        return Err(ValidationError::new(
            "stream did not continuously deliver the owned response events",
        ));
    }
    if pause && (measured_dropped == 0 || resumed_after.is_none()) {
        return Err(ValidationError::new(
            "paused consumer did not demonstrate overflow and resumed delivery",
        ));
    }
    let delivery = stats.report()?;
    if delivery["measured_received_events"] != measured_received {
        return Err(ValidationError::new(
            "delivery latency window does not match measured events",
        ));
    }
    let events = fs::read_to_string(scope.path().join("memory.events"))?;
    if events
        .lines()
        .any(|line| line.split_whitespace().nth(1) != Some("0"))
    {
        return Err(ValidationError::new(
            "stream cgroup hit memory pressure or OOM",
        ));
    }
    let observation = journal.as_ref().map(soak::Journal::report);
    let rss = if let Some(report) = &observation {
        report["rss_kib"].clone()
    } else {
        memory_summary(&samples, "rss_kib")?
    };
    let cgroup_memory = if let Some(report) = &observation {
        report["cgroup_memory_bytes"].clone()
    } else {
        memory_summary(&samples, "cgroup_memory_bytes")?
    };
    let report = json!({"schema_version":1,"ok":true,"observation_complete":complete,"elapsed_seconds":elapsed,
        "ring_bytes":1048576,"record_bytes":128,"consumer_batch_limit":512,"loss_policy":"nonblocking-drop-newest-no-overwrite",
        "warmup_counts":before,"final_counts":after,"requests":rtt.count(),"requests_per_second":rtt.count() as f64/elapsed,
        "measured_events":measured_total,"measured_delivered_events":measured_received,"measured_output_failures":measured_dropped,
        "delivered_events_per_second":measured_received as f64/elapsed,"measured_loss_percent":measured_dropped as f64*100.0/measured_total as f64,
        "consumer_pause_seconds":resumed_after,"delivery":delivery,"request_rtt":rtt.report(),"cpu":cpu,
        "rss_kib":rss,"cgroup_memory_bytes":cgroup_memory,
        "memory_samples":samples,"observation_journal":observation,"memory_events":events,"transport_errors":0,"byte_integrity_errors":0,
        "full_runtime_export_produced":false,"loadavg_after":fs::read_to_string("/proc/loadavg")?.trim()});
    drop(capture);
    let bytes = tool(Command::new("bpftool").args(["-j", "link", "show"]), &out)?;
    let links: Vec<Value> = serde_json::from_slice(&bytes)?;
    if ownership.0.exists() || links.iter().any(|l| l["id"] == ownership.1) {
        return Err(ValidationError::new(
            "stream BPF resources survived cleanup",
        ));
    }
    scope.cleanup()?;
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&report)?)?;
    Ok(report)
}

pub(super) fn run(
    out: Option<PathBuf>,
    runtime: bool,
) -> Result<ValidationReport, ValidationError> {
    let out = prepare_output(
        out,
        if runtime {
            "linux-capture-runtime"
        } else {
            "linux-capture-stream"
        },
    )?;
    let mut phases = Vec::new();
    for name in [
        "baseline-before",
        "stream-1",
        "paused-consumer",
        "stream-2",
        "baseline-after",
    ] {
        eprintln!("capture stream: {name}");
        let result = if name.starts_with("baseline") {
            load::phase(&out.join(name), false)?
        } else {
            phase(&out.join(name), name == "paused-consumer", runtime)?
        };
        phases.push(json!({"name":name,"result":result}));
    }
    super::http_boundaries::run(Some(out.join("after-stream-restart")))?;
    let baseline = (phases[0]["result"]["requests_per_second"]
        .as_f64()
        .ok_or_else(|| ValidationError::new("missing baseline"))?
        + phases[4]["result"]["requests_per_second"]
            .as_f64()
            .ok_or_else(|| ValidationError::new("missing baseline"))?)
        / 2.0;
    let delivered = (phases[1]["result"]["delivered_events_per_second"]
        .as_f64()
        .ok_or_else(|| ValidationError::new("missing stream rate"))?
        + phases[3]["result"]["delivered_events_per_second"]
            .as_f64()
            .ok_or_else(|| ValidationError::new("missing stream rate"))?)
        / 2.0;
    let mut checks = vec![
        "continuous_delivery_beyond_fixed_slots",
        "bounded_ring_and_batches",
        "checked_kernel_payloads",
        "emitted_received_dropped_reconcile",
        "paused_consumer_overflow_and_resume",
        "no_incomplete_runtime_exports",
        "cpu_memory_and_delivery_latency",
        "owned_resource_cleanup",
        "fresh_capture_replay_after_stream",
    ];
    if runtime {
        checks.extend([
            "bounded_runtime_ingest_and_analysis",
            "same_adapter_json_reanalysis",
        ]);
    }
    let mut not_proven = vec![
        "cross-batch-continuous-session-semantics",
        "full-stream-export-and-serialization-throughput",
        "durable-storage",
        "general-network-retransmission-reassembly",
        "wire-packet-rate",
        "open-loop-tail-latency",
        "multi-host-kernel-matrix",
        "long-duration-leak-freedom",
        "supervisor-or-host-crash-recovery",
    ];
    if !runtime {
        not_proven.push("runtime-ledger-ingest-throughput");
    }
    let summary = json!({"schema_version":1,"ok":true,"mode":if runtime { "capture-runtime" } else { "capture-stream" },"kernel":fs::read_to_string("/proc/sys/kernel/osrelease")?.trim(),
        "limits":{"cpu_cores":2,"memory_bytes":536870912,"pids":32},"ring_bytes":1048576,"record_bytes":128,
        "normal_phase_seconds":60,"paused_phase_seconds":10,"warmup_seconds":2,"concurrency":2,
        "normal_windows_complete":phases[1]["result"]["observation_complete"]==true && phases[3]["result"]["observation_complete"]==true,
        "comparison":{"baseline_mean_requests_per_second":baseline,"normal_mean_delivered_events_per_second":delivered,
            "delivered_rate_change_from_baseline_percent":(delivered/baseline-1.0)*100.0},
        "phases":phases,"checks":checks,"ingest_trust_mode":"unverified-local",
        "runtime_batches_enabled":runtime,
        "metric_scope":{"cpu":"whole-cgroup-including-clients-servers-consumer-and-monitor-not-isolated-BPF",
            "delivery_latency":"kernel-BPF-monotonic-timestamp-to-Rust-sink-acceptance-including-ingest-when-enabled",
            "runtime_analysis_latency":"kernel-timestamp-to-independent-batch-analysis-completion-before-sampled-replay",
            "request_latency":"closed-loop-echo-RTT","loss":"ring-output-failures-not-network-packet-loss",
            "memory":"shared-process-allocator-and-cgroup-charges-not-isolated-collector"},
        "not_proven":not_proven});
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&summary)?,
    )?;
    Ok(ValidationReport {
        name: if runtime {
            "Linux continuous capture with bounded runtime batch analysis"
        } else {
            "Linux bounded continuous capture and consumer backpressure"
        }
        .into(),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reconciliation_never_promotes_loss_or_missing_delivery() {
        assert!(
            reconcile(
                Counts {
                    total: 200_000,
                    dropped: 0,
                    malformed: 0
                },
                200_000,
                0
            )
            .unwrap()
        );
        assert!(
            !reconcile(
                Counts {
                    total: 200_000,
                    dropped: 50_000,
                    malformed: 0
                },
                150_000,
                50_000
            )
            .unwrap()
        );
        assert!(
            reconcile(
                Counts {
                    total: 200_000,
                    dropped: 50_000,
                    malformed: 0
                },
                149_999,
                50_000
            )
            .is_err()
        );
        assert!(
            reconcile(
                Counts {
                    total: 200_000,
                    dropped: 50_000,
                    malformed: 0
                },
                150_001,
                50_000
            )
            .is_err()
        );
        assert!(
            reconcile(
                Counts {
                    total: 1,
                    dropped: 0,
                    malformed: 1
                },
                1,
                0
            )
            .is_err()
        );
        assert!(
            reconcile(
                Counts {
                    total: 0,
                    dropped: 0,
                    malformed: 0
                },
                0,
                0
            )
            .is_err()
        );
        assert!(
            reconcile(
                Counts {
                    total: u64::MAX,
                    dropped: 1,
                    malformed: 0
                },
                u64::MAX,
                0
            )
            .is_err()
        );
    }
}
