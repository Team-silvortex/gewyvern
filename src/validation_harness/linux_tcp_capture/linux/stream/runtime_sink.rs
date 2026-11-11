//! Bounded independent runtime batches, not a rolling connection/session window.
use std::time::{Duration, SystemTime};

use serde_json::{Value, json};

use crate::export::ExportBundle;
use crate::fragment::{FragmentRegistry, HookPoint, MapKind, MapSpec};
use crate::ledger::{
    CpuId, FactEnvelope, FactId, FactKind, FactKindTag, PacketDir, PacketMetaFact, SessionId,
};
use crate::runtime::{RuntimeExportContext, RuntimeSession, SessionConfig};

use super::super::load::metrics::Latency;
use super::ValidationError;
use super::consumer::monotonic_ns;
use super::ring::Record;

mod payload_pool;
mod timing;

const BATCH_LIMIT: usize = 256;
const RULES: &str = r#"
template :linux_stream_runtime_batch
|> fragment :tcp_packet_meta_fragment
|> window(duration_ms: 30000, lateness_ms: 200)
|> operation :captured_echo_batch
|> reason_model :captured_echo_batch_reason
|> program_rule pred: "packet_observed:tcp:remote_to_local", stage: :packet_observed, narr: "static:captured echo packet", dedupe: false
|> reason_rule pred: "packet_observed:tcp:remote_to_local", event: :packet_observed, narr: :transport_payload_received, dedupe: true
"#;

pub(super) struct RuntimeSink {
    empty: RuntimeSession,
    session: RuntimeSession,
    clock: (u64, SystemTime),
    pending: Vec<(u64, bool)>,
    latency: Latency,
    ingested: u64,
    analyzed: u64,
    batches: u64,
    replayed: u64,
    max_pending: usize,
    profile: timing::Profile,
    export_context: RuntimeExportContext,
    payload_pool: payload_pool::PayloadPool,
}

impl RuntimeSink {
    pub(super) fn new() -> Result<Self, ValidationError> {
        let binding =
            crate::dsl::compile_str(RULES).map_err(|e| ValidationError::new(format!("{e:?}")))?;
        let mut config = SessionConfig::for_template(binding.template)
            .map_err(|e| ValidationError::new(e.to_string()))?;
        let mut descriptor = config
            .registry
            .descriptor("tcp_packet_meta_fragment")
            .ok_or_else(|| ValidationError::new("missing packet descriptor"))?
            .clone();
        // This standalone ingress probe does not capture or synthesize TCP transitions.
        descriptor.requires.clear();
        descriptor.hookpoints = vec![HookPoint::CgroupSkbIngress];
        descriptor.emits = vec![FactKindTag::PacketMeta];
        descriptor
            .evidence_classes
            .retain(|c| c.fact_kind == FactKindTag::PacketMeta);
        descriptor.sampled_payload_offsets = (0..64).collect();
        descriptor.maps = vec![
            MapSpec {
                name: "events".into(),
                kind: MapKind::RingBuf,
                max_entries: 1_048_576,
            },
            MapSpec {
                name: "counts".into(),
                kind: MapKind::Hash,
                max_entries: 1,
            },
        ];
        let mut registry = FragmentRegistry::new();
        registry
            .register(descriptor)
            .map_err(|e| ValidationError::new(format!("{e:?}")))?;
        config.registry = registry;
        let empty =
            RuntimeSession::start(config).map_err(|e| ValidationError::new(e.to_string()))?;
        let mut profile = timing::Profile::default();
        let started = profile.catalog_discovery.start(1)?;
        let export_context = RuntimeExportContext::discover();
        profile.catalog_discovery.record(started)?;
        Ok(Self {
            session: empty.clone(),
            empty,
            clock: (monotonic_ns()?, SystemTime::now()),
            pending: Vec::with_capacity(BATCH_LIMIT),
            latency: Latency::new(),
            ingested: 0,
            analyzed: 0,
            batches: 0,
            replayed: 0,
            max_pending: 0,
            profile,
            export_context,
            payload_pool: payload_pool::PayloadPool::default(),
        })
    }

    pub(super) fn accept(
        &mut self,
        record: &Record,
        netns: u32,
        measured: bool,
    ) -> Result<(), ValidationError> {
        let id = self
            .ingested
            .checked_add(1)
            .ok_or_else(|| ValidationError::new("runtime fact id overflow"))?;
        let started = self.profile.conversion.start(257)?;
        let fact = packet_fact(
            record,
            netns,
            self.clock,
            id,
            self.batches + 1,
            &mut self.payload_pool,
        )?;
        self.profile.conversion.record(started)?;
        let started = self.profile.ingest.start(257)?;
        self.session.ingest(fact);
        self.profile.ingest.record(started)?;
        self.pending.push((record.monotonic_ns, measured));
        self.ingested = id;
        self.max_pending = self.max_pending.max(self.pending.len());
        if self.pending.len() == BATCH_LIMIT {
            self.flush(false)?;
        }
        Ok(())
    }

    pub(super) fn flush(&mut self, replay: bool) -> Result<(), ValidationError> {
        if self.pending.is_empty() {
            return Ok(());
        }
        let started = self.profile.rotation.start(1)?;
        let session = std::mem::replace(&mut self.session, self.empty.clone());
        self.profile.rotation.record(started)?;
        let started = self.profile.analysis.start(1)?;
        let mut export = session.into_export_bundle_with_context(&self.export_context);
        self.profile.analysis.record(started)?;
        let started = self.profile.validation.start(1)?;
        export.ingest_trust_mode = "unverified-local".into();
        let count = self.pending.len();
        if export.facts.len() != count
            || !export.rejected_facts.is_empty()
            || export
                .flows
                .iter()
                .map(|f| f.evidence.packet_facts.len())
                .sum::<usize>()
                != count
            || export
                .program_flows
                .iter()
                .map(|f| f.stages.len())
                .sum::<usize>()
                != count
        {
            return Err(ValidationError::new(
                "runtime batch rejected, lost or misanalyzed captured facts",
            ));
        }
        let now = monotonic_ns()?;
        for (timestamp, measured) in &self.pending {
            if *measured {
                self.latency.record(Duration::from_nanos(
                    now.checked_sub(*timestamp).ok_or_else(|| {
                        ValidationError::new("runtime completion precedes capture")
                    })?,
                ));
            }
        }
        self.profile.validation.record(started)?;
        // Roundtrip representative batches, not an unbounded on-disk export archive.
        if replay || self.batches == 0 {
            let started = self.profile.replay.start(1)?;
            verify_replay(&export, &self.empty, &self.export_context)?;
            self.profile.replay.record(started)?;
            self.replayed += 1;
        }
        let started = self.profile.release.start(1)?;
        for fact in &mut export.facts {
            if let FactKind::PacketMeta(packet) = &mut fact.kind {
                self.payload_pool
                    .recycle(std::mem::take(&mut packet.payload_bytes));
            }
        }
        drop(export);
        self.profile.release.record(started)?;
        self.analyzed += count as u64;
        self.batches += 1;
        self.pending.clear();
        Ok(())
    }

    pub(super) fn report(&self) -> Result<Value, ValidationError> {
        if !self.pending.is_empty() || self.ingested != self.analyzed || self.replayed == 0 {
            return Err(ValidationError::new(
                "runtime sink has unfinished or unchecked batches",
            ));
        }
        Ok(
            json!({"ingested_facts":self.ingested,"analyzed_facts":self.analyzed,
            "analyzed_batches":self.batches,"replayed_sample_batches":self.replayed,
            "batch_limit":BATCH_LIMIT,"max_pending_facts":self.max_pending,
            "measured_analyzed_facts":self.latency.count(),"analysis_completion_latency":self.latency.report(),
            "stage_profile":self.profile.report(),
            "payload_buffer_pool":self.payload_pool.report(),
            "scope":"independent-packet-only-batches-no-cross-batch-session-context",
            "analysis":"RuntimeSession-ingest-flow-program-reason-and-export-analysis",
            "replay_scope":"JSON-roundtrip-and-fresh-session-with-same-capture-adapter-and-rules",
            "retains_full_stream":false,"durable_storage":false,"packet_exports_published":false}),
        )
    }
}

fn verify_replay(
    export: &ExportBundle,
    empty: &RuntimeSession,
    context: &RuntimeExportContext,
) -> Result<(), ValidationError> {
    let decoded = ExportBundle::from_json(&export.to_json())
        .map_err(|e| ValidationError::new(format!("{e:?}")))?;
    // Generic ExportBundle::replay assumes builtin probe dependencies and copies program output.
    // Re-run this standalone probe's exact rules instead, without fabricating TCP-state evidence.
    let mut session = empty.clone();
    for fact in decoded.facts {
        session.ingest(fact);
    }
    let replay = session.into_export_bundle_with_context(context);
    if replay.facts != export.facts
        || replay.flows != export.flows
        || replay.reasons != export.reasons
        || replay.program_flows != export.program_flows
        || replay.protocol_ir != export.protocol_ir
        || replay.program_findings != export.program_findings
        || replay.module_findings != export.module_findings
    {
        return Err(ValidationError::new("stream runtime batch replay diverged"));
    }
    Ok(())
}

fn packet_fact(
    record: &Record,
    netns: u32,
    clock: (u64, SystemTime),
    id: u64,
    session: u64,
    payload_pool: &mut payload_pool::PayloadPool,
) -> Result<FactEnvelope, ValidationError> {
    let header = u16::from(record.ip_header_len) + u16::from(record.tcp_header_len);
    if !(20..=60).contains(&record.ip_header_len)
        || !record.ip_header_len.is_multiple_of(4)
        || !(20..=60).contains(&record.tcp_header_len)
        || !record.tcp_header_len.is_multiple_of(4)
        || record.total_len.checked_sub(header) != Some(record.payload_len)
        || record.prefix_len != record.payload_len.min(64)
    {
        return Err(ValidationError::new("invalid runtime packet framing"));
    }
    let ts = record
        .monotonic_ns
        .checked_sub(clock.0)
        .and_then(|ns| clock.1.checked_add(Duration::from_nanos(ns)))
        .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
        .and_then(|d| u64::try_from(d.as_millis()).ok())
        .and_then(|ms| SystemTime::UNIX_EPOCH.checked_add(Duration::from_millis(ms)))
        .ok_or_else(|| ValidationError::new("runtime capture timestamp overflow"))?;
    let bytes = &record.prefix[..usize::from(record.prefix_len)];
    let cpu = CpuId(
        u16::try_from(record.cpu)
            .map_err(|_| ValidationError::new("capture CPU exceeds fact ABI"))?,
    );
    // Only take a reusable buffer after every fallible record check has passed.
    let payload_bytes = payload_pool.take(bytes);
    let at = |n| bytes.get(n).copied();
    Ok(FactEnvelope {
        id: FactId(id),
        ts,
        cpu,
        ifindex: Some(record.ifindex),
        session: SessionId(session),
        fragment_id: "tcp_packet_meta_fragment".into(),
        kind: FactKind::PacketMeta(PacketMetaFact {
            netns,
            sk_cookie: Some(record.cookie),
            dir: PacketDir::Ingress,
            local_port: Some(record.dport),
            remote_port: Some(record.sport),
            payload_byte0: at(0),
            payload_byte1: at(1),
            payload_prefix2: bytes.get(..2).map(|b| u16::from_be_bytes([b[0], b[1]])),
            payload_prefix4: bytes
                .get(..4)
                .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]])),
            payload_byte4: at(4),
            payload_byte5: at(5),
            payload_byte9: at(9),
            payload_byte10: at(10),
            payload_byte13: at(13),
            payload_bytes,
            l3_proto: 0x0800,
            l4_proto: 6,
            tot_len: u32::from(record.total_len),
            tcp_flags: u16::from(record.flags),
            seq: Some(record.seq),
            ack: Some(record.ack),
            window: Some(record.window),
        }),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Instant;
    fn record(ns: u64) -> Record {
        Record {
            sequence: 1,
            monotonic_ns: ns,
            cookie: 123,
            cpu: 0,
            ifindex: 1,
            source: 0x7f000001,
            destination: 0x7f000001,
            seq: 1,
            ack: 1,
            sport: 3000,
            dport: 1234,
            total_len: 104,
            window: 100,
            payload_len: 64,
            prefix_len: 64,
            flags: 0x18,
            ip_header_len: 20,
            tcp_header_len: 20,
            prefix: [0; 64],
        }
    }

    #[test]
    fn runtime_batches_bound_retention_and_replay_real_analysis() {
        let mut sink = RuntimeSink::new().unwrap();
        for index in 0..(BATCH_LIMIT * 3 + 7) {
            sink.accept(&record(monotonic_ns().unwrap()), 42, index >= 3)
                .unwrap();
            assert!(sink.pending.len() < BATCH_LIMIT);
        }
        assert!(sink.report().is_err());
        sink.flush(true).unwrap();
        let report = sink.report().unwrap();
        assert_eq!(report["analyzed_facts"], 775);
        assert_eq!(report["measured_analyzed_facts"], 772);
        assert_eq!(report["analyzed_batches"], 4);
        assert_eq!(report["replayed_sample_batches"], 2);
        assert_eq!(report["max_pending_facts"], BATCH_LIMIT);
        assert_eq!(report["stage_profile"]["conversion"]["calls"], 775);
        assert_eq!(report["stage_profile"]["ingest"]["calls"], 775);
        for stage in ["rotation", "analysis", "validation", "release"] {
            assert_eq!(report["stage_profile"][stage]["calls"], 4);
        }
        assert_eq!(report["stage_profile"]["replay"]["calls"], 2);
        sink.flush(true).unwrap();
        assert_eq!(sink.report().unwrap(), report);
    }

    #[test]
    fn reanalysis_detects_changed_program_results_instead_of_copying_them() {
        let mut sink = RuntimeSink::new().unwrap();
        sink.accept(&record(monotonic_ns().unwrap()), 42, true)
            .unwrap();
        let mut export = sink.session.clone().into_export_bundle();
        verify_replay(&export, &sink.empty, &sink.export_context).unwrap();
        export.program_flows[0].stages.clear();
        assert!(verify_replay(&export, &sink.empty, &sink.export_context).is_err());
    }

    #[test]
    fn invalid_record_does_not_mutate_runtime_batch() {
        let mut sink = RuntimeSink::new().unwrap();
        let mut bad = record(monotonic_ns().unwrap());
        bad.prefix_len = u16::MAX;
        assert!(sink.accept(&bad, 42, true).is_err());
        assert_eq!(sink.ingested, 0);
        assert!(sink.pending.is_empty());
        assert!(sink.session.into_export_bundle().facts.is_empty());
    }

    #[test]
    fn recycled_payloads_preserve_batch_evidence_and_invalid_inputs_leave_pool_unchanged() {
        let mut sink = RuntimeSink::new().unwrap();
        for value in [1, 77, 255] {
            for _ in 0..BATCH_LIMIT {
                let mut input = record(monotonic_ns().unwrap());
                input.prefix.fill(value);
                sink.accept(&input, 42, false).unwrap();
            }
            assert_eq!(sink.payload_pool.report()["retained_buffers"], BATCH_LIMIT);
        }
        let pool = sink.payload_pool.report();
        let mut bad = record(monotonic_ns().unwrap());
        bad.cpu = u32::MAX;
        assert!(sink.accept(&bad, 42, false).is_err());
        assert_eq!(sink.payload_pool.report(), pool);
        let mut input = record(monotonic_ns().unwrap());
        input.prefix.fill(99);
        sink.accept(&input, 43, false).unwrap();
        let export = sink
            .session
            .clone()
            .into_export_bundle_with_context(&sink.export_context);
        let FactKind::PacketMeta(packet) = &export.facts[0].kind else {
            panic!("expected packet")
        };
        assert_eq!(packet.netns, 43);
        assert_eq!(packet.payload_byte0, Some(99));
        assert!(packet.payload_bytes.values().all(|b| *b == 99));
        sink.flush(true).unwrap();
        assert_eq!(
            sink.report().unwrap()["payload_buffer_pool"]["reused_buffers"],
            BATCH_LIMIT * 2 + 1
        );
    }

    #[test]
    #[ignore = "fixed synthetic input stage profile, not live capture throughput"]
    fn profile_runtime_sink_stages() {
        let _allocations = timing::allocations::enable();
        let mut sink = RuntimeSink::new().unwrap();
        let cgroup_before = std::fs::read_to_string("/sys/fs/cgroup/cpu.stat").unwrap();
        let start = Instant::now();
        let cpu_start = timing::thread_cpu_ns().unwrap();
        for index in 0..(BATCH_LIMIT * 128) {
            let mut input = record(monotonic_ns().unwrap());
            input.cookie = 123 + (index % 2) as u64;
            input.prefix[..8].copy_from_slice(&(index as u64).to_le_bytes());
            sink.accept(std::hint::black_box(&input), 42, true).unwrap();
        }
        sink.flush(true).unwrap();
        let cpu_ns = timing::thread_cpu_ns().unwrap() - cpu_start;
        let elapsed = start.elapsed();
        let cgroup_after = std::fs::read_to_string("/sys/fs/cgroup/cpu.stat").unwrap();
        let mut report = sink.report().unwrap();
        report["fixture_elapsed_seconds"] = json!(elapsed.as_secs_f64());
        report["fixture_thread_cpu_seconds"] = json!(cpu_ns as f64 / 1e9);
        report["fixture_cgroup_cpu_stat_before"] = json!(cgroup_before);
        report["fixture_cgroup_cpu_stat_after"] = json!(cgroup_after);
        report["allocation_scope"] = json!(
            "test-only-thread-local-System-calls;requested-bytes-not-RSS;includes-counter-overhead"
        );
        let mut teardown = timing::Timing::default();
        let started = teardown.start(1).unwrap();
        drop(sink);
        teardown.record(started).unwrap();
        report["fixture_sink_teardown"] = teardown.report();
        report["fixture_sink_teardown_excluded_from_timed_region"] = json!(true);
        let mut empty_timer = timing::Timing::default();
        for _ in 0..1000 {
            let start = empty_timer.start(1).unwrap();
            empty_timer.record(start).unwrap();
        }
        report["empty_timer_calibration"] = empty_timer.report();
        report["fixture_source"] =
            json!("synthetic-two-cookie-64-byte-packets-not-kernel-evidence");
        report["fixture_elapsed_excludes_setup"] = json!(true);
        println!("runtime_sink_profile={report}");
    }

    #[test]
    fn conversion_preserves_evidence_and_rejects_invalid_abi() {
        let clock = (100, SystemTime::UNIX_EPOCH + Duration::from_secs(1));
        let mut value = record(101);
        value.prefix[0] = 42;
        let mut pool = payload_pool::PayloadPool::default();
        let fact = packet_fact(&value, 321, clock, 7, 2, &mut pool).unwrap();
        assert_eq!(fact.id, FactId(7));
        assert_eq!(fact.session, SessionId(2));
        let FactKind::PacketMeta(packet) = fact.kind else {
            panic!("expected packet")
        };
        assert_eq!(packet.netns, 321);
        assert_eq!(packet.sk_cookie, Some(123));
        assert_eq!(packet.payload_bytes.len(), 64);
        assert_eq!(packet.payload_byte0, Some(42));
        value.cpu = u32::MAX;
        assert!(packet_fact(&value, 321, clock, 7, 2, &mut pool).is_err());
        value.cpu = 0;
        value.monotonic_ns = 99;
        assert!(packet_fact(&value, 321, clock, 7, 2, &mut pool).is_err());
        value.monotonic_ns = 101;
        value.prefix_len = 65;
        assert!(packet_fact(&value, 321, clock, 7, 2, &mut pool).is_err());
        value.prefix_len = 64;
        value.total_len = 40;
        assert!(packet_fact(&value, 321, clock, 7, 2, &mut pool).is_err());
    }
}
