//! Complete, bounded community-message candidates, not transaction outcomes.
use super::SnmpPduKind;
use crate::ledger::PacketMetaFact;

pub(crate) const MAX_BYTES: u16 = 128;

// SNMP uses definite BER lengths, including non-minimal long-form lengths.
fn tlv<'a>(input: &mut &'a [u8]) -> Option<(u8, &'a [u8])> {
    let (&tag, rest) = input.split_first()?;
    let (&first, mut rest) = rest.split_first()?;
    let length = if first < 128 {
        usize::from(first)
    } else {
        let width = usize::from(first & 127);
        if !(1..=2).contains(&width) {
            return None;
        }
        let mut length = 0usize;
        for _ in 0..width {
            let (&byte, tail) = rest.split_first()?;
            length = (length << 8) | usize::from(byte);
            rest = tail;
        }
        length
    };
    let (body, tail) = rest.split_at_checked(length)?;
    *input = tail;
    Some((tag, body))
}

fn tagged<'a>(input: &mut &'a [u8], expected: u8) -> Option<&'a [u8]> {
    let (tag, bytes) = tlv(input)?;
    (tag == expected).then_some(bytes)
}

fn integer(input: &mut &[u8]) -> Option<i32> {
    let bytes = tagged(input, 2)?;
    if bytes.is_empty() || bytes.len() > 4 {
        return None;
    }
    let mut value = if bytes[0] & 128 == 0 { 0i32 } else { -1 };
    for &byte in bytes {
        value = (value << 8) | i32::from(byte);
    }
    Some(value)
}

fn oid(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && bytes.last().is_some_and(|b| b & 128 == 0)
        && bytes
            .split_inclusive(|b| b & 128 == 0)
            .all(|part| part.len() <= 5 && part[0] != 0x80)
}

fn envelope(bytes: &[u8], kind: SnmpPduKind) -> Option<()> {
    let mut input = bytes;
    let mut message = tagged(&mut input, 0x30)?;
    if !input.is_empty() {
        return None;
    }
    let version = integer(&mut message)?;
    if !matches!(version, 0 | 1) || (kind == SnmpPduKind::GetBulk && version != 1) {
        return None;
    }
    tagged(&mut message, 4)?;
    let tag = match kind {
        SnmpPduKind::Get => 0xa0,
        SnmpPduKind::GetNext => 0xa1,
        SnmpPduKind::GetBulk => 0xa5,
        SnmpPduKind::Response => 0xa2,
    };
    let mut pdu = tagged(&mut message, tag)?;
    if !message.is_empty() {
        return None;
    }
    integer(&mut pdu)?;
    let status = integer(&mut pdu)?;
    let index = integer(&mut pdu)?;
    if status < 0
        || index < 0
        || match kind {
            SnmpPduKind::Get | SnmpPduKind::GetNext => status != 0 || index != 0,
            SnmpPduKind::Response => status > if version == 0 { 5 } else { 18 },
            SnmpPduKind::GetBulk => false,
        }
    {
        return None;
    }
    let mut bindings = tagged(&mut pdu, 0x30)?;
    if !pdu.is_empty() {
        return None;
    }
    let mut count = 0;
    while !bindings.is_empty() {
        count += 1;
        if count > 16 {
            return None;
        }
        let mut binding = tagged(&mut bindings, 0x30)?;
        if !oid(tagged(&mut binding, 6)?) {
            return None;
        }
        let (tag, value) = tlv(&mut binding)?;
        if !binding.is_empty()
            || !match tag {
                2 => !value.is_empty() && value.len() <= 4,
                4 | 0x44 => true,
                5 => value.is_empty(),
                6 => oid(value),
                0x40 => value.len() == 4,
                0x41..=0x43 => !value.is_empty() && value.len() <= 5 && value[0] & 128 == 0,
                0x46 => {
                    version == 1 && !value.is_empty() && value.len() <= 9 && value[0] & 128 == 0
                }
                0x80..=0x82 => version == 1 && value.is_empty(),
                _ => false,
            }
        {
            return None;
        }
        if kind != SnmpPduKind::Response && tag != 5 {
            return None;
        }
    }
    Some(())
}

pub(super) fn matches(packet: &PacketMetaFact, kind: SnmpPduKind) -> bool {
    let mut bytes = [0; MAX_BYTES as usize];
    let mut count = 0;
    for offset in 0..MAX_BYTES {
        let Some(&byte) = packet.payload_bytes.get(&offset) else {
            break;
        };
        bytes[usize::from(offset)] = byte;
        count += 1;
    }
    // Never complete a missing byte with zero or join sparse samples.
    if packet
        .payload_bytes
        .keys()
        .any(|offset| usize::from(*offset) >= count)
    {
        return false;
    }
    envelope(&bytes[..count], kind).is_some()
}
