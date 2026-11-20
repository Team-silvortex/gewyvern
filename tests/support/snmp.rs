use super::*;

pub fn snmp_message(pdu: u8, community: &[u8], long_lengths: bool) -> Vec<u8> {
    fn wrap(tag: u8, bytes: &[u8], long: bool) -> Vec<u8> {
        let mut out = if long {
            vec![tag, 0x82, 0, bytes.len() as u8]
        } else {
            vec![tag, bytes.len() as u8]
        };
        out.extend_from_slice(bytes);
        out
    }
    let binding = [6, 8, 43, 6, 1, 2, 1, 1, 6, 0, 5, 0];
    let mut body = vec![2, 1, 1, 2, 1, 0, 2, 1, u8::from(pdu == 0xa5)];
    body.extend(wrap(
        0x30,
        &wrap(0x30, &binding, long_lengths),
        long_lengths,
    ));
    let mut message = vec![2, 1, 1];
    message.extend(wrap(4, community, long_lengths));
    message.extend(wrap(pdu, &body, long_lengths));
    wrap(0x30, &message, long_lengths)
}

pub fn snmp_packet(
    id: u64,
    cookie: u64,
    dir: PacketDir,
    ports: (u16, u16),
    bytes: &[u8],
) -> FactEnvelope {
    let mut fact = udp_packet_fact_with_dir_and_ports_and_payload_prefix4_and_byte13(
        id,
        cookie,
        28 + bytes.len() as u32,
        dir,
        Some(ports.0),
        Some(ports.1),
        bytes.first().copied(),
        None,
        None,
        bytes.get(13).copied(),
    );
    if let FactKind::PacketMeta(p) = &mut fact.kind {
        p.payload_byte1 = bytes.get(1).copied();
        p.payload_prefix2 = bytes.get(..2).map(|b| u16::from_be_bytes([b[0], b[1]]));
        p.payload_prefix4 = bytes
            .get(..4)
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]));
        p.payload_bytes = bytes
            .iter()
            .enumerate()
            .map(|(i, b)| (i as u16, *b))
            .collect();
    }
    fact
}

// Preserve the old fixture call shape while replacing sparse pseudo-PDUs with BER.
#[allow(clippy::too_many_arguments)]
pub fn complete_snmp_packet(
    id: u64,
    cookie: u64,
    _len: u32,
    dir: PacketDir,
    local: Option<u16>,
    remote: Option<u16>,
    _first: Option<u8>,
    _prefix2: Option<u16>,
    _prefix4: Option<u32>,
    pdu: Option<u8>,
) -> FactEnvelope {
    snmp_packet(
        id,
        cookie,
        dir,
        (local.unwrap(), remote.unwrap()),
        &snmp_message(pdu.unwrap(), b"public", false),
    )
}
