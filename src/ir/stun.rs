//! Bounded modern STUN Binding candidates, not ICE, authentication or NAT proof.
use super::StunBindingKind;
use crate::ledger::PacketMetaFact;

pub(crate) const MAX_BYTES: u16 = 128;

fn envelope(bytes: &[u8], kind: StunBindingKind) -> Option<()> {
    if bytes.len() < 20 || bytes[4..8] != [0x21, 0x12, 0xa4, 0x42] {
        return None;
    }
    let expected = match kind {
        StunBindingKind::Request => 0x0001,
        StunBindingKind::Response => 0x0101,
        StunBindingKind::Error => 0x0111,
    };
    let length = usize::from(u16::from_be_bytes([bytes[2], bytes[3]]));
    if u16::from_be_bytes([bytes[0], bytes[1]]) != expected
        || length % 4 != 0
        || length + 20 != bytes.len()
    {
        return None;
    }
    let mut attrs = &bytes[20..];
    let (mut address, mut error) = (None, None);
    while !attrs.is_empty() {
        let header = attrs.get(..4)?;
        let tag = u16::from_be_bytes([header[0], header[1]]);
        let size = usize::from(u16::from_be_bytes([header[2], header[3]]));
        let padded = (size + 3) & !3;
        let body = attrs.get(4..4 + size)?;
        attrs = attrs.get(4 + padded..)?;
        // RFC 8489 uses the first occurrence; unknown attributes remain opaque.
        if tag == 0x0020 && address.is_none() {
            address = Some(matches!(
                (body.get(1), body.len()),
                (Some(1), 8) | (Some(2), 20)
            ));
        }
        if tag == 0x0009 && error.is_none() {
            error = Some(body.len() >= 4 && (3..=6).contains(&(body[2] & 7)) && body[3] <= 99);
        }
    }
    match kind {
        StunBindingKind::Request => Some(()),
        StunBindingKind::Response if address == Some(true) => Some(()),
        StunBindingKind::Error if error == Some(true) => Some(()),
        _ => None,
    }
}

pub(super) fn matches(packet: &PacketMetaFact, kind: StunBindingKind) -> bool {
    let count = packet.payload_bytes.len();
    if !(20..=usize::from(MAX_BYTES)).contains(&count) {
        return false;
    }
    let mut bytes = [0; MAX_BYTES as usize];
    for (offset, slot) in bytes[..count].iter_mut().enumerate() {
        let Some(byte) = packet.payload_bytes.get(&(offset as u16)) else {
            return false;
        };
        *slot = *byte;
    }
    envelope(&bytes[..count], kind).is_some()
}
