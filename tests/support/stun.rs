use super::*;

pub fn stun_message(kind: u16) -> Vec<u8> {
    let mut bytes = kind.to_be_bytes().to_vec();
    bytes.extend([0, 0, 0x21, 0x12, 0xa4, 0x42]);
    bytes.extend([0x5a; 12]);
    match kind {
        0x0101 => bytes.extend([0, 0x20, 0, 8, 0, 1, 0x32, 0x9a, 0x5e, 0x12, 0xa4, 0x43]),
        0x0111 => bytes.extend([0, 9, 0, 4, 0, 0, 4, 20]),
        _ => {}
    }
    let len = (bytes.len() - 20) as u16;
    bytes[2..4].copy_from_slice(&len.to_be_bytes());
    bytes
}

pub fn stun_packet(
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

#[allow(clippy::too_many_arguments)]
pub fn complete_stun_packet(
    id: u64,
    cookie: u64,
    _len: u32,
    dir: PacketDir,
    local: Option<u16>,
    remote: Option<u16>,
    _byte: Option<u8>,
    prefix: Option<u16>,
) -> FactEnvelope {
    stun_packet(
        id,
        cookie,
        dir,
        (local.unwrap(), remote.unwrap()),
        &stun_message(prefix.unwrap()),
    )
}
