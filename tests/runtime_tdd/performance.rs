use super::*;
use gewyvern::program::build_program_flows;
use gewyvern::reason::build_reason_chains;
use std::hint::black_box;
use std::time::Instant;

#[test]
fn session_ingest_keeps_fact_ids_sorted_and_duplicate_arrival_order() {
    let config = SessionConfig::for_template(udp_debug_template()).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();

    for fact in [
        udp_packet_fact(3, 900, 73),
        udp_packet_fact(1, 900, 71),
        udp_packet_fact(3, 900, 75),
        udp_packet_fact(2, 900, 72),
    ] {
        session.ingest(fact);
    }

    let export = session.export_bundle();
    let facts = &export.facts;
    assert_eq!(
        facts.iter().map(|fact| fact.id.0).collect::<Vec<_>>(),
        vec![1, 2, 3, 3]
    );
    assert_eq!(
        facts
            .iter()
            .map(|fact| match &fact.kind {
                FactKind::PacketMeta(packet) => packet.tot_len,
                kind => panic!("unexpected fact kind: {kind:?}"),
            })
            .collect::<Vec<_>>(),
        vec![71, 72, 73, 75]
    );
    assert_eq!(
        export.reasons[0]
            .l0_facts
            .iter()
            .map(|id| id.0)
            .collect::<Vec<_>>(),
        vec![1, 2, 3, 3]
    );
}

#[test]
#[ignore = "local runtime-ingest performance baseline"]
fn benchmark_runtime_ingest_ordered_8192_facts() {
    const FACTS: u64 = 8_192;
    let input = (1..=FACTS)
        .map(|id| udp_packet_fact(id, (id % 256) + 1, 72))
        .collect::<Vec<_>>();
    let config = SessionConfig::for_template(udp_debug_template()).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();

    let started = Instant::now();
    for fact in input {
        session.ingest(black_box(fact));
    }
    black_box(&session);
    let elapsed = started.elapsed();

    println!(
        "benchmark_runtime_ingest_ordered_8192_facts: facts={FACTS} elapsed_ms={:.3}",
        elapsed.as_secs_f64() * 1_000.0
    );
}

#[test]
#[ignore = "local flow-reconstruction performance baseline"]
fn benchmark_flow_reconstruction_8192_facts() {
    const FACTS: u64 = 8_192;
    const ITERATIONS: usize = 50;
    let facts = (1..=FACTS)
        .map(|id| udp_packet_fact(id, (id % 256) + 1, 72))
        .collect::<Vec<_>>();

    let started = Instant::now();
    for _ in 0..ITERATIONS {
        let flows = build_flow_snapshots(black_box(&facts));
        assert_eq!(flows.len(), 256);
        black_box(flows);
    }
    let elapsed = started.elapsed();

    println!(
        "benchmark_flow_reconstruction_8192_facts: facts={FACTS} iterations={ITERATIONS} elapsed_ms={:.3}",
        elapsed.as_secs_f64() * 1_000.0
    );
}

#[test]
#[ignore = "local program-flow reconstruction performance baseline"]
fn benchmark_program_flow_reconstruction_8192_facts() {
    const ITERATIONS: usize = 10;
    let (facts, flows) = concurrent_udp_reconstruction_input();
    let template = udp_process_debug_template();
    let model = template.program_model.as_ref().unwrap();

    let started = Instant::now();
    for _ in 0..ITERATIONS {
        let program_flows = build_program_flows(model, &flows, black_box(&facts));
        assert_eq!(program_flows.len(), 256);
        black_box(program_flows);
    }
    let elapsed = started.elapsed();

    println!(
        "benchmark_program_flow_reconstruction_8192_facts: facts={} flows={} iterations={ITERATIONS} elapsed_ms={:.3}",
        facts.len(),
        flows.len(),
        elapsed.as_secs_f64() * 1_000.0
    );
}

#[test]
#[ignore = "local reason-chain reconstruction performance baseline"]
fn benchmark_reason_chain_reconstruction_8192_facts() {
    const ITERATIONS: usize = 10;
    let (facts, flows) = concurrent_udp_reconstruction_input();
    let template = udp_process_debug_template();
    let profile = template.reason_profile.as_ref().unwrap();

    let started = Instant::now();
    for _ in 0..ITERATIONS {
        let reasons = build_reason_chains(profile, &flows, black_box(&facts));
        assert_eq!(reasons.len(), 256);
        black_box(reasons);
    }
    let elapsed = started.elapsed();

    println!(
        "benchmark_reason_chain_reconstruction_8192_facts: facts={} flows={} iterations={ITERATIONS} elapsed_ms={:.3}",
        facts.len(),
        flows.len(),
        elapsed.as_secs_f64() * 1_000.0
    );
}

#[test]
#[ignore = "local missing-stage export performance baseline"]
fn benchmark_runtime_export_missing_route_7936_facts() {
    const FLOWS: u64 = 256;
    const PACKETS_PER_FLOW: u64 = 30;
    const ITERATIONS: usize = 10;
    let config = SessionConfig::for_template(udp_process_debug_template()).unwrap();
    let mut session = RuntimeSession::start(config).unwrap();
    let mut id = 1;

    for cookie in 1..=FLOWS {
        session.ingest(sock_lineage_fact(
            id,
            cookie,
            10_000 + cookie as u32,
            "bench",
        ));
        id += 1;
    }
    for _ in 0..PACKETS_PER_FLOW {
        for cookie in 1..=FLOWS {
            session.ingest(udp_packet_fact(id, cookie, 72));
            id += 1;
        }
    }
    assert_eq!(id - 1, 7_936);

    let started = Instant::now();
    for _ in 0..ITERATIONS {
        let export = session.export_bundle();
        assert_eq!(export.program_flows.len(), FLOWS as usize);
        assert_eq!(export.program_findings.len(), FLOWS as usize);
        assert_eq!(export.module_findings.len(), FLOWS as usize);
        assert_eq!(export.reasons.len(), FLOWS as usize);
        assert!(export.protocol_ir.is_empty());
        assert!(export.program_findings.iter().all(|finding| {
            finding.cause == gewyvern::flow::ProgramFindingCause::MissingCoreStage
        }));
        black_box(export);
    }
    let elapsed = started.elapsed();

    println!(
        "benchmark_runtime_export_missing_route_7936_facts: facts={} flows={FLOWS} iterations={ITERATIONS} elapsed_ms={:.3}",
        id - 1,
        elapsed.as_secs_f64() * 1_000.0
    );
}

fn concurrent_udp_reconstruction_input() -> (
    Vec<gewyvern::ledger::FactEnvelope>,
    Vec<gewyvern::flow::FlowSnapshot>,
) {
    const FLOWS: u64 = 256;
    const PACKETS_PER_FLOW: u64 = 30;
    let mut facts = Vec::with_capacity((FLOWS * (PACKETS_PER_FLOW + 2)) as usize);
    let mut id = 1;

    for cookie in 1..=FLOWS {
        facts.push(sock_lineage_fact(
            id,
            cookie,
            10_000 + cookie as u32,
            "bench",
        ));
        id += 1;
    }
    for _ in 0..PACKETS_PER_FLOW {
        for cookie in 1..=FLOWS {
            facts.push(udp_packet_fact(id, cookie, 72));
            id += 1;
        }
    }
    for cookie in 1..=FLOWS {
        facts.push(route_fact(id, cookie, (cookie % 16) as u32 + 1));
        id += 1;
    }

    let flows = build_flow_snapshots(&facts);
    assert_eq!(facts.len(), 8_192);
    assert_eq!(flows.len(), FLOWS as usize);
    (facts, flows)
}
