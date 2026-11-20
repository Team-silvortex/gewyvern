use super::*;
use gewyvern::runtime::RuntimeExportContext;
use gewyvern::template::FragmentParamValue;

fn session() -> RuntimeSession {
    RuntimeSession::start(SessionConfig::for_template(udp_debug_template()).unwrap()).unwrap()
}
fn complete() -> ExportBundle {
    let mut runtime = session();
    for index in 0..4 {
        runtime.ingest(fact(false, index));
    }
    runtime.into_export_bundle()
}

#[test]
fn partial_snapshot_is_degraded_without_destroying_later_completion() {
    let context = RuntimeExportContext::discover();
    let mut runtime = session();
    for index in [3, 1] {
        runtime.ingest(fact(false, index));
    }
    assert!(runtime.flow_snapshots().is_empty());
    assert!(runtime.reasons().is_empty());
    let partial = runtime.export_bundle();
    assert!(partial.facts.is_empty());
    assert!(partial.debug_summary.degraded);
    assert_eq!(partial.rejected_facts.len(), 2);
    assert!(
        partial
            .rejected_facts
            .iter()
            .all(|r| r.reason == RejectedFactReason::IncompleteUdpOffload)
    );
    assert_eq!(
        runtime.export_bundle_with_context(&context).to_json(),
        partial.to_json()
    );
    assert_eq!(
        runtime
            .clone()
            .into_export_bundle_with_context(&context)
            .to_json(),
        partial.to_json()
    );
    assert!(ExportBundle::from_json(&partial.to_json()).is_ok());
    for index in [0, 2] {
        runtime.ingest(fact(false, index));
    }
    let full = runtime.into_export_bundle();
    assert_eq!(full.facts.len(), 4);
    assert!(!full.debug_summary.degraded);
    assert!(full.rejected_facts.is_empty());
    assert_eq!(full.replay().unwrap().facts, full.facts);
}

#[test]
fn duplicates_and_conflicts_reject_whole_group_not_ordinary_neighbors() {
    for conflict in [false, true] {
        let mut runtime = session();
        for index in 0..4 {
            let mut segment = fact(false, index);
            if conflict && index == 2 {
                segment.cpu = gewyvern::ledger::CpuId(4);
            }
            runtime.ingest(segment);
        }
        if !conflict {
            let mut duplicate = fact(false, 2);
            duplicate.id = FactId(50);
            runtime.ingest(duplicate);
        }
        let mut ordinary = fact(false, 0);
        ordinary.id = FactId(99);
        let FactKind::PacketMeta(packet) = &mut ordinary.kind else {
            panic!()
        };
        packet.udp_offload = None;
        runtime.ingest(ordinary.clone());
        let bundle = runtime.into_export_bundle();
        assert_eq!(bundle.facts, vec![ordinary]);
        assert_eq!(bundle.rejected_facts.len(), if conflict { 4 } else { 5 });
        let reason = if conflict {
            RejectedFactReason::ConflictingUdpOffload
        } else {
            RejectedFactReason::DuplicateUdpOffload
        };
        assert!(bundle.rejected_facts.iter().all(|r| r.reason == reason));
        assert_eq!(
            ExportBundle::from_json(&bundle.to_json())
                .unwrap()
                .rejected_facts,
            bundle.rejected_facts
        );
    }
}

#[test]
fn every_hook_identity_field_and_origin_shape_must_agree() {
    for case in 0..14 {
        let mut bundle = complete();
        let target = &mut bundle.facts[1];
        let FactKind::PacketMeta(packet) = &mut target.kind else {
            panic!()
        };
        match case {
            0 => target.ts += Duration::from_millis(1),
            1 => target.cpu = gewyvern::ledger::CpuId(8),
            2 => target.ifindex = Some(7),
            3 => target.fragment_id = "other".into(),
            4 => packet.netns = 2,
            5 => packet.sk_cookie = Some(43),
            6 => packet.dir = PacketDir::Ingress,
            7 => packet.local_port = Some(5000),
            8 => packet.remote_port = Some(54),
            9 => packet.udp_offload.as_mut().unwrap().aggregate_payload_len = 3090,
            10 => target.id = FactId(1),
            11 => target.session = SessionId(8),
            12 => {
                packet.udp_offload.as_mut().unwrap().ip_header_len = 24;
                packet.tot_len += 4;
            }
            13 => {
                packet.l3_proto = 0x86dd;
                packet.udp_offload.as_mut().unwrap().ip_header_len = 40;
                packet.tot_len += 20;
            }
            _ => unreachable!(),
        }
        assert!(bundle.replay().is_err(), "case {case}");
        assert!(
            ExportBundle::from_json(&bundle.to_json()).is_err(),
            "case {case}"
        );
    }
}

#[test]
fn removing_or_duplicating_records_cannot_keep_a_complete_bundle() {
    for duplicate in [false, true] {
        let mut bundle = complete();
        if duplicate {
            bundle.facts.push(bundle.facts[0].clone());
        } else {
            bundle.facts.pop();
        }
        assert!(bundle.replay().is_err());
        assert!(ExportBundle::from_json(&bundle.to_json()).is_err());
        bundle.debug_summary.degraded = true;
        assert!(ExportBundle::from_json(&bundle.to_json()).is_err());
    }
}

#[test]
fn aggregate_id_is_scoped_by_session_and_complete_groups_may_interleave() {
    let mut runtime = session();
    for index in [3, 0, 2, 1] {
        for session in [7, 8] {
            let mut segment = fact(false, index);
            segment.session = SessionId(session);
            segment.id = FactId(session * 10 + u64::from(index));
            runtime.ingest(segment);
        }
    }
    let bundle = runtime.into_export_bundle();
    assert_eq!(bundle.facts.len(), 8);
    assert!(!bundle.debug_summary.degraded);
    assert_eq!(bundle.replay().unwrap().facts, bundle.facts);
}

#[test]
fn per_record_filter_cannot_export_the_rest_of_a_partial_group() {
    let mut config = SessionConfig::for_template(udp_debug_template()).unwrap();
    config
        .fragment_params
        .entry("udp_packet_meta_fragment".into())
        .or_default()
        .insert("min_len".into(), FragmentParamValue::U64(100));
    let mut runtime = RuntimeSession::start(config).unwrap();
    for index in 0..4 {
        runtime.ingest(fact(false, index));
    }
    let bundle = runtime.into_export_bundle();
    assert!(bundle.facts.is_empty());
    assert_eq!(bundle.rejected_facts.len(), 4);
    assert_eq!(
        bundle
            .rejected_facts
            .iter()
            .filter(|r| r.reason == RejectedFactReason::FilteredByFragmentParam)
            .count(),
        1
    );
    assert_eq!(
        bundle
            .rejected_facts
            .iter()
            .filter(|r| r.reason == RejectedFactReason::IncompleteUdpOffload)
            .count(),
        3
    );
    let mut malformed = complete();
    malformed.fragment_params = bundle.fragment_params;
    assert!(malformed.replay().is_err());
}

#[test]
fn normalized_fact_ids_cannot_alias_another_group_or_an_ordinary_fact() {
    for another_group in [false, true] {
        for reversed in [false, true] {
            let mut bundle = complete();
            if another_group {
                for index in 0..4 {
                    let mut other = fact(false, index);
                    let FactKind::PacketMeta(packet) = &mut other.kind else {
                        panic!()
                    };
                    packet.udp_offload.as_mut().unwrap().aggregate_id = 2;
                    bundle.facts.push(other);
                }
            } else {
                let mut ordinary = fact(false, 0);
                let FactKind::PacketMeta(packet) = &mut ordinary.kind else {
                    panic!()
                };
                packet.udp_offload = None;
                bundle.facts.push(ordinary);
            }
            if reversed {
                bundle.facts.reverse();
            }
            assert!(bundle.replay().is_err());
            assert!(ExportBundle::from_json(&bundle.to_json()).is_err());
            let mut runtime = session();
            for fact in bundle.facts {
                runtime.ingest(fact);
            }
            let rejected = runtime.into_export_bundle();
            assert_eq!(rejected.facts.len(), usize::from(!another_group));
            assert_eq!(
                rejected.rejected_facts.len(),
                if another_group { 8 } else { 4 }
            );
            assert!(
                rejected
                    .rejected_facts
                    .iter()
                    .all(|r| r.reason == RejectedFactReason::DuplicateUdpOffload)
            );
        }
    }
}
