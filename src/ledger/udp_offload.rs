use super::PacketMetaFact;

pub(super) mod groups;

pub const MAX_UDP_OFFLOAD_SEGMENTS: u16 = 16;

/// Provenance of one logical UDP datagram reconstructed from an aggregate SKB.
/// The ID identifies one hook observation within a capture run, not a wire
/// packet or a cross-host batch. All its segments share the observation time;
/// their lengths and boundaries are derived, not individual wire measurements.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct UdpOffloadSegment {
    pub aggregate_id: u64,
    pub aggregate_payload_len: u32,
    pub segment_size: u16,
    pub segment_index: u16,
    pub segment_count: u16,
    pub ip_header_len: u8,
}

impl UdpOffloadSegment {
    /// Checks framing, resource limits and the complete bounded prefix. This
    /// validates a record's shape, not the producer's trust or group completeness.
    pub fn matches_packet(self, packet: &PacketMetaFact) -> bool {
        let header = u32::from(self.ip_header_len);
        let limit = match packet.l3_proto {
            0x0800 if (20..=60).contains(&header) && header.is_multiple_of(4) => 65535 - header - 8,
            0x86dd if header == 40 => 65535 - 8,
            _ => return false,
        };
        let size = u32::from(self.segment_size);
        if self.aggregate_id == 0
            || size == 0
            || self.aggregate_payload_len <= size
            || self.aggregate_payload_len > limit
            || !(2..=MAX_UDP_OFFLOAD_SEGMENTS).contains(&self.segment_count)
            || self.segment_index >= self.segment_count
            || self.aggregate_payload_len.div_ceil(size) != u32::from(self.segment_count)
        {
            return false;
        }
        let length = (self.aggregate_payload_len - u32::from(self.segment_index) * size).min(size);
        let prefix_len = length.min(128) as usize;
        if packet.l4_proto != 17
            || packet.netns == 0
            || packet.sk_cookie.is_none_or(|cookie| cookie == 0)
            || packet.local_port.is_none_or(|port| port == 0)
            || packet.remote_port.is_none_or(|port| port == 0)
            || packet.tot_len != header + 8 + length
            || packet.tcp_flags != 0
            || packet.seq.is_some()
            || packet.ack.is_some()
            || packet.window.is_some()
            || packet.payload_bytes.len() != prefix_len
            || packet
                .payload_bytes
                .keys()
                .copied()
                .ne(0..prefix_len as u16)
        {
            return false;
        }
        let at = |offset| packet.payload_bytes.get(&offset).copied();
        let prefix2 = at(0).zip(at(1)).map(|(a, b)| u16::from_be_bytes([a, b]));
        let prefix4 = at(0)
            .zip(at(1))
            .zip(at(2))
            .zip(at(3))
            .map(|(((a, b), c), d)| u32::from_be_bytes([a, b, c, d]));
        packet.payload_byte0 == at(0)
            && packet.payload_byte1 == at(1)
            && packet.payload_prefix2 == prefix2
            && packet.payload_prefix4 == prefix4
            && packet.payload_byte4 == at(4)
            && packet.payload_byte5 == at(5)
            && packet.payload_byte9 == at(9)
            && packet.payload_byte10 == at(10)
            && packet.payload_byte13 == at(13)
    }
}
