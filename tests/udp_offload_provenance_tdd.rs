use gewyvern::export::{ExportBundle, fact_from_json, fact_to_json};
use gewyvern::ledger::{FactEnvelope, FactId, FactKind, PacketDir, SessionId, UdpOffloadSegment};
use gewyvern::packet_capture::{CaptureDecodeError, CapturedUdpDatagram, PacketObservation};
use gewyvern::runtime::{RejectedFactReason, RuntimeSession, SessionConfig};
use gewyvern::template::udp_debug_template;
use serde_json::{Value, json};
use std::time::{Duration, SystemTime};

#[path = "udp_offload_provenance_tdd/groups.rs"]
mod groups;

fn observation(index: u16) -> PacketObservation {
    PacketObservation {
        id: FactId(u64::from(index) + 1),
        session: SessionId(7),
        ts: SystemTime::UNIX_EPOCH + Duration::from_secs(1),
        cpu: 3,
        ifindex: 2,
        netns: 1,
        socket_cookie: 42,
        direction: PacketDir::Egress,
    }
}

fn provenance(ipv6: bool, index: u16) -> UdpOffloadSegment {
    UdpOffloadSegment {
        aggregate_id: u64::MAX,
        aggregate_payload_len: 3089,
        segment_size: 1024,
        segment_index: index,
        segment_count: 4,
        ip_header_len: if ipv6 { 40 } else { 20 },
    }
}

fn segment(ipv6: bool, metadata: UdpOffloadSegment) -> Result<FactEnvelope, CaptureDecodeError> {
    let payload_len = (metadata.aggregate_payload_len
        - u32::from(metadata.segment_index) * u32::from(metadata.segment_size))
    .min(u32::from(metadata.segment_size)) as u16;
    let bytes: Vec<_> = (0..payload_len.min(128)).map(|n| (n % 251) as u8).collect();
    CapturedUdpDatagram {
        source: if ipv6 {
            "[fd00::1]:4000"
        } else {
            "192.0.2.1:4000"
        }
        .parse()
        .unwrap(),
        destination: if ipv6 { "[fd00::2]:53" } else { "192.0.2.2:53" }
            .parse()
            .unwrap(),
        ip_header_len: metadata.ip_header_len,
        total_len: u32::from(metadata.ip_header_len) + 8 + u32::from(payload_len),
        payload_len,
        payload_prefix: &bytes,
    }
    .into_offload_segment_fact(observation(metadata.segment_index), metadata)
}

fn fact(ipv6: bool, index: u16) -> FactEnvelope {
    segment(ipv6, provenance(ipv6, index)).unwrap()
}

#[test]
fn both_families_and_short_tail_round_trip_without_erasing_origin() {
    for ipv6 in [false, true] {
        let mut payload_total = 0;
        for index in 0..4 {
            let fact = fact(ipv6, index);
            let FactKind::PacketMeta(packet) = &fact.kind else {
                panic!()
            };
            assert_eq!(packet.udp_offload, Some(provenance(ipv6, index)));
            payload_total += packet.tot_len - if ipv6 { 48 } else { 28 };
            assert_eq!(
                packet.payload_bytes.len(),
                if index == 3 { 17 } else { 128 }
            );
            let encoded = fact_to_json(&fact);
            let wire: Value = serde_json::from_str(&encoded).unwrap();
            assert_eq!(wire["kind"]["tag"], "packet_meta_udp_segment_v1");
            assert_eq!(
                wire["kind"]["udp_offload"]["aggregate_id"],
                u64::MAX.to_string()
            );
            assert_eq!(fact_from_json(&encoded).unwrap(), fact);
        }
        assert_eq!(payload_total, 3089);
    }
}

#[test]
fn legal_header_and_segment_limits_include_single_byte_tails() {
    for (ipv6, header) in [(false, 20), (false, 24), (false, 60), (true, 40)] {
        for count in [2, 16] {
            for size in [1, 7, 128, 1024, 4096] {
                for index in 0..count {
                    let metadata = UdpOffloadSegment {
                        ip_header_len: header,
                        segment_count: count,
                        segment_index: index,
                        segment_size: size,
                        aggregate_payload_len: u32::from(count - 1) * u32::from(size) + 1,
                        ..provenance(ipv6, index)
                    };
                    let mut fact = segment(ipv6, metadata).unwrap();
                    let FactKind::PacketMeta(packet) = &mut fact.kind else {
                        panic!()
                    };
                    packet.dir = PacketDir::Ingress;
                    assert!(metadata.matches_packet(packet));
                    assert_eq!(fact_from_json(&fact_to_json(&fact)).unwrap(), fact);
                }
            }
        }
    }
}

#[test]
fn ordinary_packet_wire_shape_remains_legacy_compatible() {
    let mut fact = fact(false, 0);
    let FactKind::PacketMeta(packet) = &mut fact.kind else {
        panic!()
    };
    packet.udp_offload = None;
    let encoded = fact_to_json(&fact);
    let wire: Value = serde_json::from_str(&encoded).unwrap();
    assert_eq!(wire["kind"]["tag"], "packet_meta");
    assert!(wire["kind"].get("udp_offload").is_none());
    assert_eq!(fact_from_json(&encoded).unwrap(), fact);
}

#[test]
fn aggregate_ip_length_limit_is_checked_independently_of_segment_length() {
    for (ipv6, header, limit) in [(false, 20, 65507), (false, 60, 65467), (true, 40, 65527)] {
        let metadata = UdpOffloadSegment {
            aggregate_payload_len: limit,
            segment_size: 4096,
            segment_count: 16,
            ip_header_len: header,
            ..provenance(ipv6, 0)
        };
        assert!(segment(ipv6, metadata).is_ok());
        assert_eq!(
            segment(
                ipv6,
                UdpOffloadSegment {
                    aggregate_payload_len: limit + 1,
                    ..metadata
                }
            ),
            Err(CaptureDecodeError::OffloadMetadata)
        );
    }
}

#[test]
fn malformed_segment_metadata_is_rejected_without_arithmetic_panics() {
    let valid = fact(false, 0);
    let FactKind::PacketMeta(packet) = &valid.kind else {
        panic!()
    };
    let p = provenance(false, 0);
    let bad = [
        UdpOffloadSegment {
            aggregate_id: 0,
            ..p
        },
        UdpOffloadSegment {
            aggregate_payload_len: 0,
            ..p
        },
        UdpOffloadSegment {
            aggregate_payload_len: 1024,
            ..p
        },
        UdpOffloadSegment {
            aggregate_payload_len: u32::MAX,
            ..p
        },
        UdpOffloadSegment {
            aggregate_payload_len: 65508,
            segment_size: 4096,
            segment_count: 16,
            ..p
        },
        UdpOffloadSegment {
            segment_size: 0,
            ..p
        },
        UdpOffloadSegment {
            segment_size: u16::MAX,
            ..p
        },
        UdpOffloadSegment {
            segment_index: 4,
            ..p
        },
        UdpOffloadSegment {
            segment_index: u16::MAX,
            ..p
        },
        UdpOffloadSegment {
            segment_count: 0,
            ..p
        },
        UdpOffloadSegment {
            segment_count: 1,
            ..p
        },
        UdpOffloadSegment {
            segment_count: 3,
            ..p
        },
        UdpOffloadSegment {
            segment_count: 17,
            ..p
        },
        UdpOffloadSegment {
            segment_count: u16::MAX,
            ..p
        },
        UdpOffloadSegment {
            ip_header_len: 19,
            ..p
        },
        UdpOffloadSegment {
            ip_header_len: 21,
            ..p
        },
        UdpOffloadSegment {
            ip_header_len: 64,
            ..p
        },
    ];
    for metadata in bad {
        assert!(!metadata.matches_packet(packet), "{metadata:?}");
    }
}

#[test]
fn adapter_rejects_partial_prefix_and_header_disagreement() {
    let bytes = [0; 128];
    let packet = CapturedUdpDatagram {
        source: "192.0.2.1:4000".parse().unwrap(),
        destination: "192.0.2.2:53".parse().unwrap(),
        ip_header_len: 20,
        total_len: 1052,
        payload_len: 1024,
        payload_prefix: &bytes,
    };
    let mut metadata = provenance(false, 0);
    metadata.ip_header_len = 24;
    assert_eq!(
        packet.into_offload_segment_fact(observation(0), metadata),
        Err(CaptureDecodeError::OffloadMetadata)
    );
    for len in [0, 1, 127] {
        let partial = CapturedUdpDatagram {
            payload_prefix: &bytes[..len],
            ..packet
        };
        assert!(partial.into_fact(observation(0)).is_ok());
        assert_eq!(
            partial.into_offload_segment_fact(observation(0), provenance(false, 0)),
            Err(CaptureDecodeError::OffloadMetadata)
        );
    }
}

#[test]
fn packet_identity_framing_and_cached_bytes_cannot_disagree() {
    let valid = fact(false, 0);
    let FactKind::PacketMeta(packet) = &valid.kind else {
        panic!()
    };
    for case in 0..24 {
        let mut bad = packet.clone();
        match case {
            0 => bad.netns = 0,
            1 => bad.sk_cookie = None,
            2 => bad.sk_cookie = Some(0),
            3 => bad.local_port = Some(0),
            4 => bad.remote_port = None,
            5 => bad.l4_proto = 6,
            6 => bad.l3_proto = 0x86dd,
            7 => bad.tot_len += 1,
            8 => bad.tcp_flags = 1,
            9 => bad.seq = Some(0),
            10 => bad.ack = Some(0),
            11 => bad.window = Some(0),
            12 => {
                bad.payload_bytes.remove(&64);
            }
            13 => {
                bad.payload_bytes.insert(128, 0);
            }
            14 => {
                bad.payload_bytes.remove(&0);
                bad.payload_bytes.insert(129, 0);
            }
            15 => bad.payload_byte0 = Some(255),
            16 => bad.payload_byte1 = None,
            17 => bad.payload_prefix2 = None,
            18 => bad.payload_prefix4 = Some(0),
            19 => bad.payload_byte4 = None,
            20 => bad.payload_byte5 = None,
            21 => bad.payload_byte9 = None,
            22 => bad.payload_byte10 = None,
            23 => bad.payload_byte13 = None,
            _ => unreachable!(),
        }
        assert!(
            !bad.udp_offload.unwrap().matches_packet(&bad),
            "case {case}"
        );
        let mut fact = valid.clone();
        fact.kind = FactKind::PacketMeta(bad);
        assert!(fact_from_json(&fact_to_json(&fact)).is_err(), "case {case}");
    }
}

#[test]
fn wire_provenance_cannot_be_downgraded_or_silently_narrowed() {
    let valid: Value = serde_json::from_str(&fact_to_json(&fact(false, 0))).unwrap();
    let cases = [
        ("/kind/tag", json!("packet_meta")),
        ("/kind/tag", json!("packet_meta_udp_segment_v2")),
        ("/kind/udp_offload", Value::Null),
        ("/kind/udp_offload/aggregate_id", json!(1)),
        ("/kind/udp_offload/aggregate_id", json!("01")),
        ("/kind/udp_offload/aggregate_id", json!("+1")),
        ("/kind/udp_offload/aggregate_id", json!("0")),
        (
            "/kind/udp_offload/aggregate_id",
            json!("18446744073709551616"),
        ),
        ("/kind/udp_offload/aggregate_payload_len", json!(-1)),
        ("/kind/udp_offload/segment_size", json!(66560)),
        ("/kind/udp_offload/segment_index", json!(-1)),
        ("/kind/udp_offload/segment_count", json!(65540)),
        ("/kind/udp_offload/ip_header_len", json!(276)),
        ("/kind/l4_proto", json!(273)),
        ("/kind/l3_proto", json!(67584)),
        ("/kind/tot_len", json!(4294968348_i64)),
        ("/kind/local_port", json!(69536)),
        ("/kind/payload_byte0", json!(256)),
        ("/kind/payload_prefix2", json!(65537)),
        ("/kind/payload_bytes/0/offset", json!(65536)),
        ("/kind/payload_bytes/0/value", json!(256)),
        ("/id", json!(-1)),
        ("/session", json!(-1)),
        ("/ts_ms", json!(-1)),
        ("/cpu", json!(65539)),
    ];
    for (path, value) in cases {
        let mut bad = valid.clone();
        *bad.pointer_mut(path).unwrap() = value;
        assert!(fact_from_json(&bad.to_string()).is_err(), "{path}: {bad}");
    }
    for path in ["", "/kind", "/kind/udp_offload"] {
        let mut bad = valid.clone();
        bad.pointer_mut(path)
            .unwrap()
            .as_object_mut()
            .unwrap()
            .insert("unknown".into(), json!(1));
        assert!(fact_from_json(&bad.to_string()).is_err());
    }
    for key in ["udp_offload", "payload_byte0", "local_port"] {
        let mut bad = valid.clone();
        bad["kind"].as_object_mut().unwrap().remove(key);
        assert!(fact_from_json(&bad.to_string()).is_err());
    }
    let mut duplicate = valid;
    let bytes = duplicate["kind"]["payload_bytes"].as_array_mut().unwrap();
    bytes.push(bytes[0].clone());
    assert!(fact_from_json(&duplicate.to_string()).is_err());
}

#[test]
fn runtime_and_export_keep_provenance_and_structured_rejections() {
    let mut session =
        RuntimeSession::start(SessionConfig::for_template(udp_debug_template()).unwrap()).unwrap();
    for index in 0..4 {
        session.ingest(fact(false, index));
    }
    let mut invalid = fact(false, 0);
    invalid.id = FactId(5);
    let FactKind::PacketMeta(packet) = &mut invalid.kind else {
        panic!()
    };
    packet.udp_offload.as_mut().unwrap().segment_count = 17;
    session.ingest(invalid);
    let bundle = session.into_export_bundle();
    assert_eq!(bundle.facts.len(), 4);
    assert_eq!(bundle.rejected_facts.len(), 1);
    assert_eq!(
        bundle.rejected_facts[0].reason,
        RejectedFactReason::InvalidUdpOffload
    );
    assert_eq!(
        bundle.rejected_facts[0].reason.label(),
        "invalid_udp_offload"
    );
    assert!(bundle.debug_summary.degraded);
    let replay = ExportBundle::from_json(&bundle.to_json()).unwrap();
    assert_eq!(replay.facts, bundle.facts);
    assert_eq!(replay.rejected_facts, bundle.rejected_facts);
    let reanalyzed = replay.replay().unwrap();
    assert_eq!(reanalyzed.facts, bundle.facts);
    assert_eq!(reanalyzed.program_flows, bundle.program_flows);
    assert_eq!(reanalyzed.rejected_facts, bundle.rejected_facts);
    let mut fresh =
        RuntimeSession::start(SessionConfig::for_template(udp_debug_template()).unwrap()).unwrap();
    fresh.seed_rejected_facts(replay.rejected_facts);
    for fact in replay.facts {
        fresh.ingest(fact);
    }
    let repeated = fresh.into_export_bundle();
    assert_eq!(repeated.facts, bundle.facts);
    assert_eq!(repeated.program_flows, bundle.program_flows);
    assert_eq!(repeated.rejected_fact_summary, bundle.rejected_fact_summary);
}

#[test]
#[ignore = "requires loopback socket bind permissions"]
fn socket_ingest_preserves_segment_origin_and_rejects_downgrade() {
    use gewyvern::socket_input::{SocketInputError, collect_tcp_socket_facts_on_listener};
    use std::io::Write;
    use std::net::{TcpListener, TcpStream};

    let expected: Vec<_> = (0..4).map(|index| fact(true, index)).collect();
    for downgrade in [false, true] {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let mut lines = expected.iter().map(fact_to_json).collect::<Vec<_>>();
        if downgrade {
            let mut bad: Value = serde_json::from_str(&lines[1]).unwrap();
            bad["kind"]["tag"] = json!("packet_meta");
            lines[1] = bad.to_string();
        }
        let sender = std::thread::spawn(move || {
            let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(3)).unwrap();
            stream
                .set_write_timeout(Some(Duration::from_secs(3)))
                .unwrap();
            stream
                .write_all(format!("{}\n", lines.join("\n")).as_bytes())
                .unwrap();
        });
        let result = collect_tcp_socket_facts_on_listener(&listener);
        sender.join().unwrap();
        if downgrade {
            assert!(matches!(result, Err(SocketInputError::ParseFailed(_))));
        } else {
            assert_eq!(result.unwrap(), expected);
        }
    }
}

#[test]
fn direct_bundle_replay_cannot_hide_invalid_segment_rejection() {
    let mut session =
        RuntimeSession::start(SessionConfig::for_template(udp_debug_template()).unwrap()).unwrap();
    for index in 0..4 {
        session.ingest(fact(false, index));
    }
    let mut bundle = session.into_export_bundle();
    assert!(!bundle.debug_summary.degraded);
    let FactKind::PacketMeta(packet) = &mut bundle.facts[0].kind else {
        panic!()
    };
    packet.udp_offload.as_mut().unwrap().segment_index = 16;
    assert!(bundle.replay().is_err());
}
