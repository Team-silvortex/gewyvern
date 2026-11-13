use std::collections::BTreeSet;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use crate::export::ExportBundle;
use crate::fragment::{MapKind, MapSpec};
use crate::ledger::{CpuId, FactEnvelope, FactId, FactKind, SessionId, TcpStateFact};
use crate::runtime::{RuntimeSession, SessionConfig};
use crate::template::handshake_debug_template;

use super::command::{ValidationError, ValidationReport};

#[cfg(target_os = "linux")]
mod linux;

#[cfg(any(target_os = "linux", test))]
mod packet_evidence;
#[cfg(any(target_os = "linux", test))]
mod response_prefix;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct CaptureEvent {
    sequence: u64,
    monotonic_ns: u64,
    cookie: u64,
    netns: u32,
    cpu: u16,
    sport: u16,
    dport: u16,
    family: u8,
    old_state: u8,
    new_state: u8,
    saddr: [u8; 4],
    daddr: [u8; 4],
}

pub fn run_linux_tcp_capture_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    #[cfg(target_os = "linux")]
    return linux::run(out_dir);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = out_dir;
        Err(ValidationError::new(
            "linux-tcp-capture requires Linux and BPF privileges",
        ))
    }
}

pub fn run_linux_tcp_juice_shop_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    #[cfg(target_os = "linux")]
    return linux::run_juice_shop(out_dir);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = out_dir;
        Err(ValidationError::new(
            "linux-tcp-capture --juice-shop requires Linux and BPF privileges",
        ))
    }
}

pub fn run_linux_auth_breadth_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    #[cfg(target_os = "linux")]
    return linux::run_auth_breadth(out_dir);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = out_dir;
        Err(ValidationError::new(
            "linux-tcp-capture --auth-breadth requires Linux and BPF privileges",
        ))
    }
}

pub fn run_linux_http_boundaries_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    #[cfg(target_os = "linux")]
    return linux::run_http_boundaries(out_dir);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = out_dir;
        Err(ValidationError::new(
            "linux-tcp-capture --http-boundaries requires Linux and BPF privileges",
        ))
    }
}

pub fn run_linux_capture_recovery_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    #[cfg(target_os = "linux")]
    return linux::run_capture_recovery(out_dir);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = out_dir;
        Err(ValidationError::new(
            "linux-tcp-capture --capture-recovery requires Linux and BPF privileges",
        ))
    }
}

pub fn run_linux_capture_load_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    #[cfg(target_os = "linux")]
    return linux::run_capture_load(out_dir);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = out_dir;
        Err(ValidationError::new(
            "linux-tcp-capture --capture-load requires Linux, delegated cgroup limits and BPF privileges",
        ))
    }
}

pub fn run_linux_capture_stream_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    #[cfg(target_os = "linux")]
    return linux::run_capture_stream(out_dir);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = out_dir;
        Err(ValidationError::new(
            "linux-tcp-capture --capture-stream requires Linux, libbpf ring consumption and BPF privileges",
        ))
    }
}

pub fn run_linux_capture_runtime_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    #[cfg(target_os = "linux")]
    return linux::run_capture_runtime(out_dir);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = out_dir;
        Err(ValidationError::new(
            "linux-tcp-capture --capture-runtime requires Linux, libbpf and BPF privileges",
        ))
    }
}

pub fn run_linux_capture_mixed_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    #[cfg(target_os = "linux")]
    return linux::run_capture_mixed(out_dir);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = out_dir;
        Err(ValidationError::new(
            "linux-tcp-capture --capture-mixed requires Linux, libbpf and BPF privileges",
        ))
    }
}

pub fn run_linux_capture_soak_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    #[cfg(target_os = "linux")]
    return linux::run_capture_soak(out_dir);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = out_dir;
        Err(ValidationError::new(
            "linux-tcp-capture --capture-soak requires Linux, glibc and BPF privileges",
        ))
    }
}

pub fn run_linux_capture_soak_hour_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    #[cfg(target_os = "linux")]
    return linux::run_capture_soak_hour(out_dir);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = out_dir;
        Err(ValidationError::new(
            "linux-tcp-capture --capture-soak-hour requires Linux, glibc and BPF privileges",
        ))
    }
}

pub fn run_linux_capture_memory_control_validation(
    out_dir: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    #[cfg(target_os = "linux")]
    return linux::run_capture_memory_control(out_dir);
    #[cfg(not(target_os = "linux"))]
    {
        let _ = out_dir;
        Err(ValidationError::new(
            "capture-memory-control requires Linux and glibc",
        ))
    }
}

// The validation adapter consumes only kernel observations. It never invents
// packet bytes, process lineage, HTTP semantics or missing TCP transitions.
#[cfg(test)]
fn reconstruct(
    events: &[CaptureEvent],
    expected: &[(u64, u16, u16, u32)],
    total: u64,
    dropped: u64,
    clock: (u64, SystemTime),
) -> Result<ExportBundle, ValidationError> {
    reconstruct_at(
        events,
        expected,
        total,
        dropped,
        clock,
        ([127, 0, 0, 1], [127, 0, 0, 1]),
    )
}

#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
fn reconstruct_at(
    events: &[CaptureEvent],
    expected: &[(u64, u16, u16, u32)],
    total: u64,
    dropped: u64,
    clock: (u64, SystemTime),
    addresses: ([u8; 4], [u8; 4]),
) -> Result<ExportBundle, ValidationError> {
    if dropped != 0 || total == 0 || total > 128 || events.len() as u64 != total {
        return Err(ValidationError::new(
            "incomplete or overflowing TCP capture",
        ));
    }
    let mut sequences = BTreeSet::new();
    let mut previous_ns = 0;
    let mut template = handshake_debug_template();
    template.id = "linux_tcp_state_capture".into();
    template.fragment_set = vec!["tcp_state_fragment".into()];
    let mut config =
        SessionConfig::for_template(template).map_err(|e| ValidationError::new(e.to_string()))?;
    // This bounded test backend uses snapshot maps, not the production
    // fragment's planned ring buffer. Keep the exported map inventory honest.
    let mut fragment = config
        .registry
        .descriptor("tcp_state_fragment")
        .ok_or_else(|| ValidationError::new("missing TCP state fragment"))?
        .clone();
    fragment.maps = vec![
        MapSpec {
            name: "events".into(),
            kind: MapKind::Hash,
            max_entries: 128,
        },
        MapSpec {
            name: "counts".into(),
            kind: MapKind::Hash,
            max_entries: 1,
        },
    ];
    let mut registry = crate::fragment::FragmentRegistry::new();
    registry
        .register(fragment)
        .map_err(|e| ValidationError::new(format!("{e:?}")))?;
    config.registry = registry;
    let mut session =
        RuntimeSession::start(config).map_err(|e| ValidationError::new(e.to_string()))?;
    for (index, event) in events.iter().enumerate() {
        if event.sequence != index as u64 + 1
            || !sequences.insert(event.sequence)
            || event.monotonic_ns < previous_ns
            || event.monotonic_ns < clock.0
            || event.monotonic_ns - clock.0 > 30_000_000_000
            || !expected.contains(&(event.cookie, event.sport, event.dport, event.netns))
            || event.cookie == 0
            || event.netns == 0
            || event.family != 2
            || event.saddr != addresses.0
            || event.daddr != addresses.1
            || !(1..=12).contains(&event.old_state)
            || !(1..=12).contains(&event.new_state)
        {
            return Err(ValidationError::new(
                "TCP capture identity, order or scope mismatch",
            ));
        }
        previous_ns = event.monotonic_ns;
        let ts = clock
            .1
            .checked_add(Duration::from_nanos(event.monotonic_ns - clock.0))
            .ok_or_else(|| ValidationError::new("TCP capture clock overflow"))?;
        // The export schema stores wall time in milliseconds; raw monotonic
        // nanoseconds remain in the capture file instead of being fabricated on replay.
        let millis = ts
            .duration_since(SystemTime::UNIX_EPOCH)
            .ok()
            .and_then(|duration| u64::try_from(duration.as_millis()).ok())
            .ok_or_else(|| ValidationError::new("TCP capture timestamp out of range"))?;
        let ts = SystemTime::UNIX_EPOCH
            .checked_add(Duration::from_millis(millis))
            .ok_or_else(|| ValidationError::new("TCP capture timestamp overflow"))?;
        let mut saddr = [0; 16];
        let mut daddr = [0; 16];
        saddr[..4].copy_from_slice(&event.saddr);
        daddr[..4].copy_from_slice(&event.daddr);
        session.ingest(FactEnvelope {
            id: FactId(event.sequence),
            ts,
            cpu: CpuId(event.cpu),
            ifindex: None,
            session: SessionId(1),
            fragment_id: "tcp_state_fragment".into(),
            kind: FactKind::TcpState(TcpStateFact {
                netns: event.netns,
                sk_cookie: event.cookie,
                saddr,
                daddr,
                sport: event.sport,
                dport: event.dport,
                family: event.family,
                old: event.old_state,
                new: event.new_state,
            }),
        });
    }
    let mut export = session.into_export_bundle();
    export.ingest_trust_mode = "unverified-local".into();
    if !export.rejected_facts.is_empty() || export.facts.len() != events.len() {
        return Err(ValidationError::new("runtime rejected captured TCP facts"));
    }
    Ok(export)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn event() -> CaptureEvent {
        CaptureEvent {
            sequence: 1,
            monotonic_ns: 10,
            cookie: 42,
            netns: 1,
            cpu: 0,
            sport: 1234,
            dport: 4321,
            family: 2,
            old_state: 2,
            new_state: 1,
            saddr: [127, 0, 0, 1],
            daddr: [127, 0, 0, 1],
        }
    }
    fn decode(
        events: &[CaptureEvent],
        total: u64,
        dropped: u64,
    ) -> Result<ExportBundle, ValidationError> {
        reconstruct(
            events,
            &[(42, 1234, 4321, 1)],
            total,
            dropped,
            (0, SystemTime::UNIX_EPOCH),
        )
    }
    #[test]
    fn real_state_adapter_keeps_lineage_and_packet_content_absent() {
        let export = decode(&[event()], 1, 0).unwrap();
        assert_eq!(export.ingest_trust_mode, "unverified-local");
        assert_eq!(export.flows.len(), 1);
        assert!(export.flows[0].process.is_none());
        assert!(!export.flows[0].lifecycle.terminated);
        assert!(export.flows[0].evidence.packet_facts.is_empty());
        let replay = ExportBundle::from_json(&export.to_json())
            .unwrap()
            .replay()
            .unwrap();
        assert_eq!(export.facts, replay.facts);
        assert_eq!(export.flows, replay.flows);
        assert_eq!(export.reasons, replay.reasons);
    }
    #[test]
    fn recorded_linux_capture_replays_half_close_and_refusal_without_fabricating_packets() {
        let proof: serde_json::Value = serde_json::from_str(include_str!(
            "../../docs/fixtures/gewyvern_tcp_capture_linux_x86_64_20261104.json"
        ))
        .unwrap();
        let events: Vec<CaptureEvent> =
            serde_json::from_value(proof["capture_sample"]["events"].clone()).unwrap();
        let expected: Vec<(u64, u16, u16, u32)> =
            serde_json::from_value(proof["capture_sample"]["expected_sockets"].clone()).unwrap();
        // Replay the recorded relative timings; this is not the original wall-clock anchor.
        let clock = (events[0].monotonic_ns, SystemTime::UNIX_EPOCH);
        for (count, state, terminated) in [(2, 1, false), (3, 8, false), (7, 7, true)] {
            let export = reconstruct(&events[..count], &expected, count as u64, 0, clock).unwrap();
            assert_eq!(export.flows[0].lifecycle.tcp_state_now, Some(state));
            assert_eq!(export.flows[0].lifecycle.terminated, terminated);
            assert_eq!(export.flows.len(), if count == 7 { 2 } else { 1 });
            assert!(
                export
                    .flows
                    .iter()
                    .all(|flow| flow.process.is_none() && flow.evidence.packet_facts.is_empty())
            );
            let replay = ExportBundle::from_json(&export.to_json())
                .unwrap()
                .replay()
                .unwrap();
            assert_eq!(export.facts, replay.facts);
            assert_eq!(export.flows, replay.flows);
            assert_eq!(export.reasons, replay.reasons);
        }
    }
    #[test]
    fn private_container_addresses_require_an_explicit_exact_capture_scope() {
        let mut value = event();
        value.saddr = [172, 20, 0, 1];
        value.daddr = [172, 20, 0, 2];
        assert!(decode(&[value.clone()], 1, 0).is_err());
        let addresses = (value.saddr, value.daddr);
        let expected = [(42, 1234, 4321, 1)];
        let clock = (0, SystemTime::UNIX_EPOCH);
        let export = reconstruct_at(&[value.clone()], &expected, 1, 0, clock, addresses).unwrap();
        let replay = ExportBundle::from_json(&export.to_json())
            .unwrap()
            .replay()
            .unwrap();
        assert_eq!(export.facts, replay.facts);
        value.daddr[3] = 3;
        assert!(reconstruct_at(&[value], &expected, 1, 0, clock, addresses).is_err());
    }

    #[test]
    fn incomplete_overflowing_and_unscoped_captures_are_rejected() {
        for (total, dropped) in [(0, 0), (2, 0), (1, 1), (129, 0)] {
            assert!(decode(&[event()], total, dropped).is_err());
        }
        for mutate in [
            |e: &mut CaptureEvent| e.cookie = 43,
            |e: &mut CaptureEvent| e.sport = 1235,
            |e: &mut CaptureEvent| e.dport = 4322,
            |e: &mut CaptureEvent| e.saddr = [1, 2, 3, 4],
            |e: &mut CaptureEvent| e.family = 10,
            |e: &mut CaptureEvent| e.netns = 0,
            |e: &mut CaptureEvent| e.new_state = 13,
            |e: &mut CaptureEvent| e.sequence = 2,
            |e: &mut CaptureEvent| e.monotonic_ns = u64::MAX,
        ] {
            let mut value = event();
            mutate(&mut value);
            assert!(decode(&[value], 1, 0).is_err());
        }
        assert!(decode(&[event(), event()], 2, 0).is_err());
    }
}
