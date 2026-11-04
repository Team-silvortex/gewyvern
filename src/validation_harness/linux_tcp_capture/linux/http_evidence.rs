use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use socket2::Socket;

use super::{CaptureEvent, ExportBundle, ValidationError};
use crate::ledger::FactKind;

const MAX_RESPONSE: usize = 256 * 1024;

pub(super) fn sha256(bytes: &[u8]) -> String {
    ring::digest::digest(&ring::digest::SHA256, bytes)
        .as_ref()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

pub(super) fn exchange(
    socket: Socket,
    target: SocketAddr,
    path: &str,
    request_id: &str,
) -> Result<(Vec<u8>, Vec<u8>), ValidationError> {
    if !path.starts_with('/')
        || path.bytes().any(|c| !c.is_ascii_graphic())
        || request_id.is_empty()
        || !request_id
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
        || !matches!(target.ip(), std::net::IpAddr::V4(ip) if ip.is_loopback() || ip.is_private())
    {
        return Err(ValidationError::new("invalid bounded lab HTTP request"));
    }
    socket.connect_timeout(&target.into(), Duration::from_secs(2))?;
    let mut stream: TcpStream = socket.into();
    let request = format!("GET {path} HTTP/1.1\r\nHost: {target}\r\nConnection: close\r\nAccept-Encoding: identity\r\nX-Gewyvern-Test: {request_id}\r\n\r\n").into_bytes();
    stream.write_all(&request)?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut response = Vec::new();
    let mut buffer = [0; 8192];
    loop {
        let remaining = deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| ValidationError::new("lab HTTP response deadline exceeded"))?;
        stream.set_read_timeout(Some(remaining))?;
        let read = stream.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        if response.len() + read > MAX_RESPONSE {
            return Err(ValidationError::new(
                "lab HTTP response exceeds bounded evidence size",
            ));
        }
        response.extend_from_slice(&buffer[..read]);
    }
    Ok((request, response))
}

pub(super) fn parse_response(raw: &[u8]) -> Result<(u16, Vec<u8>), ValidationError> {
    let boundary = raw
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .filter(|boundary| *boundary <= 16 * 1024 && raw.len() <= MAX_RESPONSE)
        .ok_or_else(|| ValidationError::new("invalid bounded HTTP headers"))?;
    let headers = std::str::from_utf8(&raw[..boundary])
        .map_err(|_| ValidationError::new("invalid HTTP header encoding"))?;
    let mut lines = headers.split("\r\n");
    let mut status = lines.next().unwrap_or_default().split_ascii_whitespace();
    if !matches!(status.next(), Some("HTTP/1.0" | "HTTP/1.1")) {
        return Err(ValidationError::new("unsupported HTTP version"));
    }
    let code = status
        .next()
        .filter(|code| code.len() == 3 && code.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|code| code.parse::<u16>().ok())
        .filter(|code| (100..=599).contains(code))
        .ok_or_else(|| ValidationError::new("invalid HTTP status"))?;
    let mut length = None;
    let mut chunked = false;
    for line in lines {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| ValidationError::new("malformed HTTP header"))?;
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || value.bytes().any(|b| b.is_ascii_control() && b != b'\t')
        {
            return Err(ValidationError::new("invalid HTTP header name"));
        }
        if name.eq_ignore_ascii_case("content-encoding")
            && !value.trim().eq_ignore_ascii_case("identity")
        {
            return Err(ValidationError::new(
                "lab fixture does not accept compressed responses",
            ));
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            if chunked || !value.trim().eq_ignore_ascii_case("chunked") {
                return Err(ValidationError::new(
                    "ambiguous or unsupported Transfer-Encoding",
                ));
            }
            chunked = true;
        }
        if name.eq_ignore_ascii_case("content-length") {
            let value = value.trim();
            let parsed = (!value.is_empty() && value.bytes().all(|b| b.is_ascii_digit()))
                .then(|| value.parse::<usize>().ok())
                .flatten()
                .ok_or_else(|| ValidationError::new("invalid Content-Length"))?;
            if length.replace(parsed).is_some() {
                return Err(ValidationError::new("duplicate Content-Length"));
            }
        }
    }
    let body = &raw[boundary + 4..];
    if chunked {
        if length.is_some() {
            return Err(ValidationError::new("conflicting HTTP response framing"));
        }
        return Ok((code, decode_chunks(body)?));
    }
    if length != Some(body.len()) {
        return Err(ValidationError::new(
            "incomplete or ambiguously framed HTTP response",
        ));
    }
    Ok((code, body.to_vec()))
}

fn decode_chunks(mut remaining: &[u8]) -> Result<Vec<u8>, ValidationError> {
    let mut decoded = Vec::new();
    loop {
        let end = remaining
            .windows(2)
            .position(|w| w == b"\r\n")
            .filter(|end| (1..=16).contains(end))
            .ok_or_else(|| ValidationError::new("invalid bounded chunk size"))?;
        let size_text = std::str::from_utf8(&remaining[..end])
            .ok()
            .filter(|text| text.bytes().all(|b| b.is_ascii_hexdigit()))
            .ok_or_else(|| ValidationError::new("unsupported chunk size or extension"))?;
        let size = usize::from_str_radix(size_text, 16)
            .map_err(|_| ValidationError::new("chunk size overflow"))?;
        remaining = &remaining[end + 2..];
        if size == 0 {
            if remaining != b"\r\n" {
                return Err(ValidationError::new(
                    "unsupported trailers or trailing chunk data",
                ));
            }
            return Ok(decoded);
        }
        let framed = size
            .checked_add(2)
            .filter(|len| *len <= remaining.len())
            .ok_or_else(|| ValidationError::new("truncated chunk body"))?;
        if remaining.get(size..framed) != Some(b"\r\n")
            || size > MAX_RESPONSE.saturating_sub(decoded.len())
        {
            return Err(ValidationError::new(
                "invalid chunk terminator or response size",
            ));
        }
        decoded.extend_from_slice(&remaining[..size]);
        remaining = &remaining[framed..];
    }
}

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Observation {
    pub request_id: String,
    pub path: String,
    pub status: u16,
    pub cookie: u64,
    pub sport: u16,
    pub dport: u16,
    pub netns: u32,
    pub started_ns: u64,
    pub completed_ns: u64,
    pub request_sha256: String,
    pub response_sha256: String,
}

pub(super) fn correlate(
    events: &[CaptureEvent],
    export: &ExportBundle,
    observations: &[Observation],
) -> Result<Value, ValidationError> {
    let mut cookies = BTreeSet::new();
    let mut requests = BTreeSet::new();
    let mut linked = BTreeSet::new();
    let mut rows = Vec::new();
    if observations.is_empty()
        || export.facts.len() != events.len()
        || export.flows.len() != observations.len()
    {
        return Err(ValidationError::new("incomplete request/flow correlation"));
    }
    for observation in observations {
        if !cookies.insert(observation.cookie) || !requests.insert(&observation.request_id) {
            return Err(ValidationError::new("duplicate request/socket identity"));
        }
        let selected: Vec<_> = events
            .iter()
            .filter(|e| e.cookie == observation.cookie)
            .collect();
        let mut state = 7;
        let mut established = false;
        let mut ids = Vec::new();
        for event in selected {
            if (event.sport, event.dport, event.netns)
                != (observation.sport, observation.dport, observation.netns)
                || event.monotonic_ns < observation.started_ns
                || event.monotonic_ns > observation.completed_ns
                || event.old_state != state
                || !linked.insert(event.sequence)
            {
                return Err(ValidationError::new(
                    "request identity, time or TCP continuity mismatch",
                ));
            }
            state = event.new_state;
            established |= state == 1;
            let matching_fact = export.facts.iter().any(|fact| fact.id.0 == event.sequence && matches!(&fact.kind,
                FactKind::TcpState(value) if value.sk_cookie == event.cookie && value.netns == event.netns
                    && value.sport == event.sport && value.dport == event.dport && value.old == event.old_state && value.new == event.new_state
                    && value.saddr[..4] == event.saddr && value.daddr[..4] == event.daddr));
            if !matching_fact {
                return Err(ValidationError::new("kernel event/export fact mismatch"));
            }
            ids.push(event.sequence);
        }
        let flows: Vec<_> = export
            .flows
            .iter()
            .filter(|flow| {
                flow.evidence
                    .tcp_state_facts
                    .iter()
                    .map(|id| id.0)
                    .collect::<Vec<_>>()
                    == ids
            })
            .collect();
        let [flow] = flows.as_slice() else {
            return Err(ValidationError::new("ambiguous request flow"));
        };
        if !established
            || state != 7
            || !flow.lifecycle.terminated
            || flow.process.is_some()
            || !flow.evidence.packet_facts.is_empty()
        {
            return Err(ValidationError::new(
                "incomplete TCP lifecycle or invented payload/lineage evidence",
            ));
        }
        rows.push(
            json!({"client_observation":observation, "flow_id":flow.id.0, "tcp_fact_ids":ids,
            "http_evidence_source":"owned-native-client-io", "kernel_evidence":"tcp-state-only"}),
        );
    }
    if linked.len() != events.len() {
        return Err(ValidationError::new(
            "unmatched kernel events in request correlation",
        ));
    }
    Ok(
        json!({"schema_version":1, "scope":"client-socket-to-owned-private-container-endpoint",
        "ingest_trust_mode":"unverified-local", "payload_inferred_by_gewyvern":false,
        "container_pid_or_post_nat_socket_attribution":false, "requests":rows}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn real_container_sample_replays_and_correlates_without_http_packet_facts() {
        let proof: Value = serde_json::from_str(include_str!(
            "../../../../docs/fixtures/gewyvern_container_capture_linux_x86_64_20261104.json"
        ))
        .unwrap();
        let sample = &proof["capture_sample"];
        let events: Vec<CaptureEvent> = serde_json::from_value(sample["events"].clone()).unwrap();
        let expected: Vec<(u64, u16, u16, u32)> =
            serde_json::from_value(sample["expected_sockets"].clone()).unwrap();
        let addresses = serde_json::from_value(sample["addresses"].clone()).unwrap();
        let observations: Vec<Observation> =
            serde_json::from_value(sample["observations"].clone()).unwrap();
        // Preserve relative capture time; do not pretend this is the original wall-clock anchor.
        let clock = (
            sample["clock_anchor_monotonic_ns"].as_u64().unwrap(),
            std::time::SystemTime::UNIX_EPOCH,
        );
        let export =
            super::super::super::reconstruct_at(&events, &expected, 10, 0, clock, addresses)
                .unwrap();
        let replay = ExportBundle::from_json(&export.to_json())
            .unwrap()
            .replay()
            .unwrap();
        assert_eq!(export.facts, replay.facts);
        assert_eq!(export.flows, replay.flows);
        assert_eq!(export.reasons, replay.reasons);
        let correlation = correlate(&events, &replay, &observations).unwrap();
        assert_eq!(
            correlation["requests"][0]["tcp_fact_ids"],
            json!([1, 2, 3, 4, 5])
        );
        assert_eq!(
            correlation["requests"][1]["tcp_fact_ids"],
            json!([6, 7, 8, 9, 10])
        );
        assert_eq!(correlation["payload_inferred_by_gewyvern"], false);
    }

    fn sample() -> (Vec<CaptureEvent>, ExportBundle, Vec<Observation>) {
        let mut events = Vec::new();
        let mut observations = Vec::new();
        let mut expected = Vec::new();
        for index in 0..2u16 {
            let cookie = 42 + u64::from(index);
            let sport = 1000 + index;
            expected.push((cookie, sport, 3000, 1));
            for (old_state, new_state) in [(7, 2), (2, 1), (1, 8), (8, 9), (9, 7)] {
                let sequence = events.len() as u64 + 1;
                events.push(CaptureEvent {
                    sequence,
                    monotonic_ns: sequence * 10,
                    cookie,
                    netns: 1,
                    cpu: 0,
                    sport,
                    dport: 3000,
                    family: 2,
                    old_state,
                    new_state,
                    saddr: [127, 0, 0, 1],
                    daddr: [127, 0, 0, 1],
                });
            }
            observations.push(Observation {
                request_id: format!("request-{index}"),
                path: "/".into(),
                status: 200,
                cookie,
                sport,
                dport: 3000,
                netns: 1,
                started_ns: u64::from(index) * 50,
                completed_ns: u64::from(index + 1) * 50,
                request_sha256: sha256(b"request"),
                response_sha256: sha256(b"response"),
            });
        }
        let export = super::super::super::reconstruct(
            &events,
            &expected,
            events.len() as u64,
            0,
            (0, std::time::SystemTime::UNIX_EPOCH),
        )
        .unwrap();
        (events, export, observations)
    }

    #[test]
    fn correlation_does_not_join_on_shared_destination_port_alone() {
        let (events, export, observations) = sample();
        let value = correlate(&events, &export, &observations).unwrap();
        assert_eq!(value["requests"][0]["flow_id"], 1);
        assert_eq!(value["requests"][1]["flow_id"], 2);
        assert_eq!(value["requests"][0]["tcp_fact_ids"], json!([1, 2, 3, 4, 5]));
        for mutate in [
            |o: &mut Observation| o.cookie = 43,
            |o: &mut Observation| o.sport = 1001,
            |o: &mut Observation| o.netns = 2,
            |o: &mut Observation| o.started_ns = 20,
            |o: &mut Observation| o.completed_ns = 40,
            |o: &mut Observation| o.request_id = "request-1".into(),
        ] {
            let mut wrong = observations.clone();
            mutate(&mut wrong[0]);
            assert!(correlate(&events, &export, &wrong).is_err());
        }
        let mut wrong = observations.clone();
        wrong[0].cookie = 43;
        wrong[0].sport = 1001;
        wrong[1].cookie = 42;
        wrong[1].sport = 1000;
        assert!(
            correlate(&events, &export, &wrong).is_err(),
            "swapped sockets must fail the request time fence"
        );
    }

    #[test]
    fn correlation_rejects_missing_transitions_and_mismatched_export_facts() {
        let (mut events, mut export, observations) = sample();
        events[1].old_state = 7;
        assert!(correlate(&events, &export, &observations).is_err());
        events[1].old_state = 2;
        let FactKind::TcpState(value) = &mut export.facts[0].kind else {
            unreachable!()
        };
        value.saddr[0] = 192;
        assert!(correlate(&events, &export, &observations).is_err());
        assert!(correlate(&events, &export, &[]).is_err());
    }

    #[test]
    fn bounded_http_evidence_requires_complete_unambiguous_framing() {
        assert_eq!(
            parse_response(b"HTTP/1.1 403 Forbidden\r\nContent-Length: 2\r\n\r\nno").unwrap(),
            (403, b"no".to_vec())
        );
        for raw in [
            "HTTP/1.1 200evil OK\r\nContent-Length: 0\r\n\r\n",
            "HTTP/1.1 200 OK\r\nContent-Length: 3\r\n\r\nno",
            "HTTP/1.1 200 OK\r\nContent-Length: 1\r\n\r\nno",
            "HTTP/1.1 200 OK\r\n\r\nno",
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\ncontent-length: 2\r\n\r\nno",
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nTransfer-Encoding: chunked\r\n\r\nno",
            "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nContent-Encoding: gzip\r\n\r\nno",
        ] {
            assert!(parse_response(raw.as_bytes()).is_err());
        }
    }

    #[test]
    fn chunked_evidence_rejects_truncation_overflow_and_trailing_data() {
        assert_eq!(
            decode_chunks(b"1\r\nn\r\n1\r\no\r\n0\r\n\r\n").unwrap(),
            b"no"
        );
        assert_eq!(
            parse_response(
                b"HTTP/1.1 500 Error\r\nTransfer-Encoding: chunked\r\n\r\n2\r\nno\r\n0\r\n\r\n"
            )
            .unwrap(),
            (500, b"no".to_vec())
        );
        for body in [
            b"2\r\nn\r\n0\r\n\r\n".as_slice(),
            b"ffffffffffffffff\r\n",
            b"0\r\n",
            b"0\r\n\r\nextra",
            b"1\r\naXX0\r\n\r\n",
        ] {
            assert!(decode_chunks(body).is_err());
        }
    }
}
