use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use super::{ExportBundle, ValidationError};
use crate::ledger::{FactKind, PacketDir};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum Outcome {
    Verified,
    MissingOrigin,
    ConflictingOrigin,
    ConflictingBytes,
    Incomplete,
    NotHttp11,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct ResponsePrefix {
    pub cookie: u64,
    pub netns: u32,
    pub outcome: Outcome,
    pub status: Option<u16>,
    pub stream_start_seq: Option<u32>,
    pub prefix: [Option<u8>; 13],
    pub anchor_fact_ids: Vec<u64>,
    pub payload_fact_ids: Vec<u64>,
}

// This is derived evidence, never a fabricated PacketMetaFact. It reconstructs
// only the first 13 bytes relative to a captured SYN-ACK, not an HTTP/TCP stream.
pub(super) fn response_prefixes(
    export: &ExportBundle,
) -> Result<Vec<ResponsePrefix>, ValidationError> {
    if export.facts.len() > 256 || !export.rejected_facts.is_empty() {
        return Err(ValidationError::new(
            "response prefix requires a bounded loss-free capture",
        ));
    }
    let mut groups = BTreeMap::new();
    for fact in &export.facts {
        if let FactKind::PacketMeta(packet) = &fact.kind {
            let cookie = packet
                .sk_cookie
                .filter(|c| *c != 0)
                .ok_or_else(|| ValidationError::new("response packet has no socket identity"))?;
            if packet.l4_proto != 6 || packet.dir != PacketDir::Ingress {
                return Err(ValidationError::new(
                    "response fixture requires ingress TCP packets",
                ));
            }
            groups
                .entry((cookie, packet.netns))
                .or_insert_with(Vec::new)
                .push((fact, packet));
        }
    }
    if groups.len() > 2 {
        return Err(ValidationError::new(
            "response fixture exceeded owned socket scope",
        ));
    }
    let mut results = Vec::new();
    for ((cookie, netns), packets) in groups {
        let mut result = ResponsePrefix {
            cookie,
            netns,
            outcome: Outcome::MissingOrigin,
            status: None,
            stream_start_seq: None,
            prefix: [None; 13],
            anchor_fact_ids: Vec::new(),
            payload_fact_ids: Vec::new(),
        };
        let mut origins = BTreeSet::new();
        for (fact, packet) in &packets {
            if packet.tcp_flags & 0x17 == 0x12 && packet.payload_bytes.is_empty() {
                let seq = packet
                    .seq
                    .ok_or_else(|| ValidationError::new("SYN-ACK missing sequence number"))?;
                origins.insert(seq.wrapping_add(1));
                result.anchor_fact_ids.push(fact.id.0);
            }
        }
        if origins.len() != 1 {
            if !origins.is_empty() {
                result.outcome = Outcome::ConflictingOrigin;
            }
            results.push(result);
            continue;
        }
        let start = *origins
            .first()
            .ok_or_else(|| ValidationError::new("missing prefix origin"))?;
        result.stream_start_seq = Some(start);
        let mut conflict = false;
        for (fact, packet) in packets {
            if packet.payload_bytes.is_empty() {
                continue;
            }
            let seq = packet
                .seq
                .ok_or_else(|| ValidationError::new("payload missing sequence number"))?;
            let offset = seq.wrapping_sub(start);
            if offset >= 13 {
                continue;
            }
            let mut contributed = false;
            for (&index, &byte) in &packet.payload_bytes {
                let at = offset + u32::from(index);
                if at >= 13 {
                    continue;
                }
                let slot = &mut result.prefix[at as usize];
                conflict |= slot.is_some_and(|previous| previous != byte);
                *slot = Some(byte);
                contributed = true;
            }
            if contributed {
                result.payload_fact_ids.push(fact.id.0);
            }
        }
        result.outcome = if conflict {
            Outcome::ConflictingBytes
        } else if result.prefix.iter().any(Option::is_none) {
            Outcome::Incomplete
        } else {
            let bytes = result.prefix.map(|b| b.unwrap_or_default());
            if &bytes[..9] == b"HTTP/1.1 "
                && bytes[12] == b' '
                && (b'1'..=b'5').contains(&bytes[9])
                && bytes[10..12].iter().all(u8::is_ascii_digit)
            {
                result.status = Some(
                    u16::from(bytes[9] - b'0') * 100
                        + u16::from(bytes[10] - b'0') * 10
                        + u16::from(bytes[11] - b'0'),
                );
                Outcome::Verified
            } else {
                Outcome::NotHttp11
            }
        };
        results.push(result);
    }
    Ok(results)
}

pub(super) fn verified_statuses(results: &[ResponsePrefix]) -> Vec<(u64, u16)> {
    results
        .iter()
        .filter_map(|p| {
            p.status
                .filter(|_| p.outcome == Outcome::Verified)
                .map(|s| (p.cookie, s))
        })
        .collect()
}

pub(super) fn matches_client_response(prefix: &ResponsePrefix, response: &[u8]) -> bool {
    prefix.outcome == Outcome::Verified
        && response.len() >= prefix.prefix.len()
        && prefix
            .prefix
            .iter()
            .zip(response)
            .all(|(captured, byte)| *captured == Some(*byte))
}

pub(super) fn verified_replay(
    export: &ExportBundle,
) -> Result<Vec<ResponsePrefix>, ValidationError> {
    let result = response_prefixes(export)?;
    let replay = ExportBundle::from_json(&export.to_json())
        .map_err(|e| ValidationError::new(format!("{e:?}")))?
        .replay()
        .map_err(|e| ValidationError::new(format!("{e:?}")))?;
    if response_prefixes(&replay)? != result {
        return Err(ValidationError::new(
            "derived response prefix replay diverged",
        ));
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ledger::{CpuId, FactEnvelope, FactId, PacketMetaFact, SessionId};
    use crate::runtime::{RuntimeSession, SessionConfig};
    use crate::template::handshake_debug_template;
    use std::time::{Duration, SystemTime};

    fn export(pieces: &[(u32, &[u8], u16)]) -> ExportBundle {
        let mut session =
            RuntimeSession::start(SessionConfig::for_template(handshake_debug_template()).unwrap())
                .unwrap();
        for (i, (seq, bytes, flags)) in pieces.iter().enumerate() {
            session.ingest(FactEnvelope {
                id: FactId(i as u64 + 1),
                ts: SystemTime::UNIX_EPOCH + Duration::from_millis(i as u64),
                cpu: CpuId(0),
                ifindex: Some(1),
                session: SessionId(1),
                fragment_id: "tcp_packet_meta_fragment".into(),
                kind: FactKind::PacketMeta(PacketMetaFact {
                    netns: 42,
                    sk_cookie: Some(1),
                    dir: PacketDir::Ingress,
                    local_port: Some(4000),
                    remote_port: Some(3000),
                    payload_byte0: bytes.first().copied(),
                    payload_byte1: None,
                    payload_prefix2: None,
                    payload_prefix4: None,
                    payload_byte4: None,
                    payload_byte5: None,
                    payload_byte9: None,
                    payload_byte10: None,
                    payload_byte13: None,
                    payload_bytes: bytes
                        .iter()
                        .enumerate()
                        .map(|(i, b)| (i as u16, *b))
                        .collect(),
                    l3_proto: 0x800,
                    l4_proto: 6,
                    tot_len: 40 + bytes.len() as u32,
                    tcp_flags: *flags,
                    seq: Some(*seq),
                    ack: Some(1),
                    window: Some(1024),
                }),
            });
        }
        session.into_export_bundle()
    }

    #[test]
    fn split_prefix_and_body_decoy_keep_origin_and_provenance() {
        let e = export(&[
            (99, b"", 0x12),
            (100, b"HTTP/1.", 0x18),
            (107, b"1 403 Forbidden", 0x18),
            (150, b"HTTP/1.1 500 fake body", 0x18),
        ]);
        let r = verified_replay(&e).unwrap();
        assert_eq!(verified_statuses(&r), vec![(1, 403)]);
        assert_eq!(r[0].anchor_fact_ids, vec![1]);
        assert_eq!(r[0].payload_fact_ids, vec![2, 3]);
        assert_eq!(e.facts.len(), 4);
    }

    #[test]
    fn reordered_duplicate_segments_and_sequence_wrap_are_safe() {
        let start = u32::MAX - 3;
        let e = export(&[
            (start - 1, b"", 0x12),
            (start.wrapping_add(7), b"1 500 Error", 0x18),
            (start, b"HTTP/1.", 0x18),
            (start, b"HTTP/1.", 0x18),
        ]);
        assert_eq!(
            verified_statuses(&verified_replay(&e).unwrap()),
            vec![(1, 500)]
        );
    }

    #[test]
    fn missing_gap_or_conflicting_evidence_never_invents_a_status() {
        for (pieces, outcome) in [
            (
                vec![(100, b"HTTP/1.1 500 ".as_slice(), 0x18)],
                Outcome::MissingOrigin,
            ),
            (
                vec![
                    (99, b"".as_slice(), 0x12),
                    (100, b"HTTP/1.", 0x18),
                    (108, b" 500 ", 0x18),
                ],
                Outcome::Incomplete,
            ),
            (
                vec![
                    (99, b"".as_slice(), 0x12),
                    (100, b"HTTP/1.1 500 ", 0x18),
                    (109, b"2", 0x18),
                ],
                Outcome::ConflictingBytes,
            ),
            (
                vec![
                    (99, b"".as_slice(), 0x12),
                    (100, b"HTTP/1.1 500 ", 0x18),
                    (98, b"", 0x12),
                ],
                Outcome::ConflictingOrigin,
            ),
            (
                vec![(99, b"".as_slice(), 0x12), (100, b"garbage  500 ", 0x18)],
                Outcome::NotHttp11,
            ),
        ] {
            let r = verified_replay(&export(&pieces)).unwrap();
            assert_eq!(r[0].outcome, outcome);
            assert!(r[0].status.is_none());
            assert!(verified_statuses(&r).is_empty());
        }
    }

    #[test]
    fn valid_200_does_not_become_500_when_body_starts_with_status_text() {
        let e = export(&[
            (99, b"", 0x12),
            (100, b"HTTP/1.1 200 OK\r\n\r\n", 0x18),
            (119, b"HTTP/1.1 500 ", 0x18),
        ]);
        assert_eq!(
            verified_statuses(&verified_replay(&e).unwrap()),
            vec![(1, 200)]
        );
    }

    #[test]
    fn empty_or_truncated_capture_cannot_vacuously_match_a_client_response() {
        let e = export(&[(99, b"", 0x12), (100, b"HTTP/1.1 200 OK", 0x18)]);
        let mut prefix = verified_replay(&e).unwrap().remove(0);
        assert!(matches_client_response(&prefix, b"HTTP/1.1 200 OK"));
        assert!(!matches_client_response(&prefix, b"HTTP/1.1 500 Error"));
        assert!(!matches_client_response(&prefix, b""));
        prefix.prefix[12] = None;
        assert!(!matches_client_response(&prefix, b"HTTP/1.1 200 OK"));
    }
}
