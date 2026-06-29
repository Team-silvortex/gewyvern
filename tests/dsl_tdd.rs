use gewyvern::dsl::{DslError, compile_file, compile_str, parse_str_unvalidated};
use gewyvern::flow::ProgramOperation;
use gewyvern::fragment::{RegistryError, RuleTier, builtin_registry};
use gewyvern::gewyc::collect_binding_diagnostics;
use gewyvern::ledger::PacketDir;
use gewyvern::reason::{KeyEventKind, ReasonProfile};
use gewyvern::runtime::{RuntimeSession, SessionConfig};
use gewyvern::template::FragmentParamValue;

mod support;

use std::fs;
use std::time::{Duration, SystemTime};
use support::{
    packet_fact, packet_fact_with_dir, packet_fact_with_dir_and_payload,
    packet_fact_with_dir_and_payload_and_byte1, packet_fact_with_dir_and_payload_and_byte4,
    packet_fact_with_dir_and_payload_and_byte10,
    packet_fact_with_dir_and_payload_and_bytes4_5_and9,
    packet_fact_with_dir_and_payload_and_bytes4_and5, packet_fact_with_dir_and_payload_bytes,
    route_fact, sock_lineage_fact, tcp_state_fact, tcp_state_fact_with_ports, udp_packet_fact,
    udp_packet_fact_with_dir, udp_packet_fact_with_dir_and_ports_and_payload,
    udp_packet_fact_with_dir_and_ports_and_payload_prefix4,
    udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13, udp_quic_meta_fact,
    udp_quic_meta_fact_with_payload_bytes,
};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

#[path = "dsl_tdd/part_001.rs"]
mod part_001;
#[path = "dsl_tdd/part_002.rs"]
mod part_002;
#[path = "dsl_tdd/part_003.rs"]
mod part_003;
#[path = "dsl_tdd/part_004.rs"]
mod part_004;
#[path = "dsl_tdd/part_005.rs"]
mod part_005;
#[path = "dsl_tdd/part_006.rs"]
mod part_006;
#[path = "dsl_tdd/part_007.rs"]
mod part_007;
#[path = "dsl_tdd/part_008.rs"]
mod part_008;
#[path = "dsl_tdd/part_009.rs"]
mod part_009;
#[path = "dsl_tdd/part_010.rs"]
mod part_010;
#[path = "dsl_tdd/part_011.rs"]
mod part_011;
#[path = "dsl_tdd/part_012.rs"]
mod part_012;
#[path = "dsl_tdd/part_013.rs"]
mod part_013;
#[path = "dsl_tdd/part_014.rs"]
mod part_014;
#[path = "dsl_tdd/part_015.rs"]
mod part_015;
#[path = "dsl_tdd/part_016.rs"]
mod part_016;
#[path = "dsl_tdd/part_017.rs"]
mod part_017;
#[path = "dsl_tdd/part_018.rs"]
mod part_018;
#[path = "dsl_tdd/part_019.rs"]
mod part_019;
#[path = "dsl_tdd/part_020.rs"]
mod part_020;
#[path = "dsl_tdd/part_021.rs"]
mod part_021;
#[path = "dsl_tdd/part_022.rs"]
mod part_022;
#[path = "dsl_tdd/part_023.rs"]
mod part_023;
#[path = "dsl_tdd/part_024.rs"]
mod part_024;
#[path = "dsl_tdd/part_025.rs"]
mod part_025;
#[path = "dsl_tdd/part_026.rs"]
mod part_026;
#[path = "dsl_tdd/part_027.rs"]
mod part_027;
#[path = "dsl_tdd/part_028.rs"]
mod part_028;
#[path = "dsl_tdd/part_029.rs"]
mod part_029;
#[path = "dsl_tdd/part_030.rs"]
mod part_030;
#[path = "dsl_tdd/part_031.rs"]
mod part_031;
#[path = "dsl_tdd/part_032.rs"]
mod part_032;
#[path = "dsl_tdd/part_033.rs"]
mod part_033;
#[path = "dsl_tdd/part_034.rs"]
mod part_034;
