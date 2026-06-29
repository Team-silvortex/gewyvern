mod support;

use gewyvern::dsl::compile_file;
use gewyvern::flow::ProgramOperation;
use gewyvern::ledger::PacketDir;
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use std::time::{Duration, SystemTime};
use support::{
    route_fact, sock_lineage_fact, udp_packet_fact_with_dir_and_ports_and_payload,
    udp_quic_meta_fact, udp_quic_meta_fact_with_payload_bytes,
};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[path = "http3_hy2_protocol_registry_tdd/part_001.rs"]
mod part_001;
#[path = "http3_hy2_protocol_registry_tdd/part_002.rs"]
mod part_002;
