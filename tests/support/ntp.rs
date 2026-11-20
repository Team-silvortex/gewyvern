use super::*;

pub fn ntp_header(flags: u8) -> Vec<u8> {
    let mut bytes = vec![0; 48];
    bytes[0] = flags;
    bytes[1] = if flags >> 6 == 3 { 0 } else { 10 };
    bytes[31] = 1;
    bytes[39] = 2;
    bytes[47] = 3;
    bytes
}

pub fn ntp_packet(
    id: u64,
    cookie: u64,
    dir: PacketDir,
    ports: (u16, u16),
    bytes: &[u8],
) -> FactEnvelope {
    let mut fact = udp_packet_fact_with_dir_and_ports_and_payload(
        id,
        cookie,
        28 + bytes.len() as u32,
        dir,
        Some(ports.0),
        Some(ports.1),
        bytes.first().copied(),
        bytes.get(..2).map(|b| u16::from_be_bytes([b[0], b[1]])),
    );
    if let FactKind::PacketMeta(p) = &mut fact.kind {
        p.payload_bytes = bytes
            .iter()
            .enumerate()
            .map(|(i, b)| (i as u16, *b))
            .collect();
    }
    fact
}

// Old transport-fixture call shape, now backed by the complete NTP base header.
#[allow(clippy::too_many_arguments)]
pub fn complete_ntp_packet(
    id: u64,
    cookie: u64,
    _len: u32,
    dir: PacketDir,
    local: Option<u16>,
    remote: Option<u16>,
    flags: Option<u8>,
    _prefix: Option<u16>,
) -> FactEnvelope {
    ntp_packet(
        id,
        cookie,
        dir,
        (local.unwrap(), remote.unwrap()),
        &ntp_header(flags.unwrap()),
    )
}
