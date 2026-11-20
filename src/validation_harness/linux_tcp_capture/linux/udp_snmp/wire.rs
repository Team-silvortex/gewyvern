//! Independent fixture client: one read-only varbind, no product IR parser.
use super::ValidationError;

pub(super) const COMMUNITY: &[u8] = b"gewyvern-lab";
pub(super) const LOCATION: &[u8] = &[43, 6, 1, 2, 1, 1, 6, 0];
pub(super) const PREVIOUS: &[u8] = &[43, 6, 1, 2, 1, 1, 5, 0];
pub(super) const UNKNOWN: &[u8] = &[43, 6, 1, 2, 1, 1, 0x87, 0x67, 0];

fn wrap(tag: u8, bytes: &[u8]) -> Vec<u8> {
    let mut result = vec![tag, bytes.len() as u8];
    result.extend_from_slice(bytes);
    result
}

pub(super) fn query(pdu: u8, id: u32, community: &[u8], oid: &[u8]) -> Vec<u8> {
    let mut binding = wrap(6, oid);
    binding.extend([5, 0]);
    // A fixed positive four-byte request-id intentionally moves later fields.
    let mut body = wrap(2, &id.to_be_bytes());
    body.extend([2, 1, 0, 2, 1, u8::from(pdu == 0xa5)]);
    body.extend(wrap(0x30, &wrap(0x30, &binding)));
    let mut message = vec![2, 1, 1];
    message.extend(wrap(4, community));
    message.extend(wrap(pdu, &body));
    wrap(0x30, &message)
}

struct Reader<'a>(&'a [u8]);
impl<'a> Reader<'a> {
    fn take(&mut self, tag: u8) -> Option<&'a [u8]> {
        if self.0.first() != Some(&tag) {
            return None;
        }
        let first = *self.0.get(1)?;
        let (header, count) = if first < 128 {
            (2, usize::from(first))
        } else {
            match first {
                0x81 => (3, usize::from(*self.0.get(2)?)),
                0x82 => (
                    4,
                    usize::from(u16::from_be_bytes([*self.0.get(2)?, *self.0.get(3)?])),
                ),
                _ => return None,
            }
        };
        let bytes = self.0.get(header..header + count)?;
        self.0 = self.0.get(header + count..)?;
        Some(bytes)
    }
    fn integer(&mut self) -> Option<u32> {
        let bytes = self.take(2)?;
        if bytes.is_empty() || bytes.len() > 4 || bytes[0] & 128 != 0 {
            return None;
        }
        Some(
            bytes
                .iter()
                .fold(0, |value, byte| (value << 8) | u32::from(*byte)),
        )
    }
}

pub(super) fn check_reply(bytes: &[u8], id: u32, unknown: bool) -> Result<(), ValidationError> {
    let check = || -> Option<()> {
        if bytes.len() > 128 {
            return None;
        }
        let mut outer = Reader(bytes);
        let mut message = Reader(outer.take(0x30)?);
        if !outer.0.is_empty() || message.integer()? != 1 || message.take(4)? != COMMUNITY {
            return None;
        }
        let mut pdu = Reader(message.take(0xa2)?);
        if !message.0.is_empty()
            || pdu.integer()? != id
            || pdu.integer()? != 0
            || pdu.integer()? != 0
        {
            return None;
        }
        let mut list = Reader(pdu.take(0x30)?);
        let mut binding = Reader(list.take(0x30)?);
        if !pdu.0.is_empty()
            || !list.0.is_empty()
            || binding.take(6)? != if unknown { UNKNOWN } else { LOCATION }
        {
            return None;
        }
        if unknown {
            if !binding.take(0x80)?.is_empty() {
                return None;
            }
        } else if binding.take(4)? != b"gewyvern-fixture" {
            return None;
        }
        binding.0.is_empty().then_some(())
    };
    check().ok_or_else(|| {
        ValidationError::new(format!(
            "SNMP client reply differs from owned fixture: {bytes:02x?}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn reply_oracle_rejects_request_and_wrong_identity() {
        let request = query(0xa0, 1, COMMUNITY, LOCATION);
        assert!(check_reply(&request, 1, false).is_err());
        let mut binding = wrap(6, LOCATION);
        binding.extend(wrap(4, b"gewyvern-fixture"));
        let mut pdu = vec![2, 1, 1, 2, 1, 0, 2, 1, 0];
        pdu.extend(wrap(0x30, &wrap(0x30, &binding)));
        let mut message = vec![2, 1, 1];
        message.extend(wrap(4, COMMUNITY));
        message.extend(wrap(0xa2, &pdu));
        let good = wrap(0x30, &message);
        assert!(check_reply(&good, 1, false).is_ok());
        assert!(check_reply(&good, 2, false).is_err());
        assert!(check_reply(&good, 1, true).is_err());
        for end in 0..good.len() {
            assert!(check_reply(&good[..end], 1, false).is_err());
        }
        let mut extra = good;
        extra.push(0);
        assert!(check_reply(&extra, 1, false).is_err());
    }
}
