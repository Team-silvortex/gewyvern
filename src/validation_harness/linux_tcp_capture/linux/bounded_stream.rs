use std::collections::BTreeSet;

use super::super::packet_evidence::PacketEvent;
use super::ValidationError;

// Only a complete, small server stream is accepted. Missing/truncated/conflicting
// bytes stay an error, rather than being filled from the independently read client.
pub(super) fn captured_response(
    packets: &[PacketEvent],
    cookie: u64,
) -> Result<Vec<u8>, ValidationError> {
    let selected: Vec<_> = packets.iter().filter(|p| p.cookie == cookie).collect();
    let origins: BTreeSet<_> = selected
        .iter()
        .filter(|p| p.flags & 0x17 == 0x12 && p.payload_len == 0)
        .map(|p| p.seq.wrapping_add(1))
        .collect();
    if origins.len() != 1 {
        return Err(ValidationError::new(
            "bounded server stream requires one captured SYN-ACK origin",
        ));
    }
    let origin = *origins
        .first()
        .ok_or_else(|| ValidationError::new("missing stream origin"))?;
    let mut bytes = vec![None; 4096];
    let mut extent = 0;
    for packet in selected.iter().filter(|p| p.payload_len > 0) {
        let start = packet.seq.wrapping_sub(origin) as usize;
        let length = usize::from(packet.payload_len);
        if length != usize::from(packet.prefix_len)
            || length > packet.prefix.len()
            || start > bytes.len()
            || length > bytes.len() - start
        {
            return Err(ValidationError::new(
                "bounded server stream truncated or outside 4-KiB bound",
            ));
        }
        for (slot, byte) in bytes[start..start + length].iter_mut().zip(&packet.prefix) {
            if slot.is_some_and(|previous| previous != *byte) {
                return Err(ValidationError::new(
                    "conflicting bounded server stream bytes",
                ));
            }
            *slot = Some(*byte);
        }
        extent = extent.max(start + length);
    }
    if extent == 0 {
        return Err(ValidationError::new("empty bounded server stream"));
    }
    bytes[..extent]
        .iter()
        .map(|b| b.ok_or_else(|| ValidationError::new("gap in bounded server stream")))
        .collect()
}
