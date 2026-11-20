//! Packet-local NTPv3/v4 base headers; never evidence of clock discipline.
use super::NtpMessageKind;
use crate::ledger::PacketMetaFact;

pub(crate) const HEADER_BYTES: u16 = 48;

pub(super) fn matches(packet: &PacketMetaFact, kind: NtpMessageKind) -> bool {
    let mut header = [0; HEADER_BYTES as usize];
    for offset in 0..HEADER_BYTES {
        let Some(&byte) = packet.payload_bytes.get(&offset) else {
            return false;
        };
        header[usize::from(offset)] = byte;
    }
    let version = (header[0] >> 3) & 7;
    let mode = header[0] & 7;
    if !matches!(version, 3 | 4) {
        return false;
    }
    match kind {
        NtpMessageKind::Request => mode == 3,
        NtpMessageKind::Response => mode == 4,
        NtpMessageKind::TimeReply => {
            mode == 4
                && header[0] >> 6 != 3
                && (1..=15).contains(&header[1])
                && header[24..32].iter().any(|b| *b != 0)
                && header[32..40].iter().any(|b| *b != 0)
                && header[40..48].iter().any(|b| *b != 0)
        }
    }
}
