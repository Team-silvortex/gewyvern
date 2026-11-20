//! Independent native client checks. No clock-setting calls or product decoder.
use super::ValidationError;
use serde::Serialize;

pub(super) fn query(version: u8, stamp: u64, short: bool) -> Vec<u8> {
    let mut bytes = vec![0; 48];
    bytes[0] = (version << 3) | 3;
    bytes[2] = 6;
    bytes[3] = (-20i8) as u8;
    bytes[40..48].copy_from_slice(&stamp.to_be_bytes());
    if short {
        bytes.truncate(12);
    }
    bytes
}

#[derive(Debug, PartialEq, Serialize)]
pub(super) struct Reply {
    pub version: u8,
    pub leap: u8,
    pub stratum: u8,
    pub reference_id: [u8; 4],
    pub origin_hex: String,
    pub receive_hex: String,
    pub transmit_hex: String,
    pub local_reference: bool,
}

pub(super) fn reply(bytes: &[u8], request: &[u8], local: bool) -> Result<Reply, ValidationError> {
    let decode = || -> Option<Reply> {
        if bytes.len() != 48 || request.len() != 48 {
            return None;
        }
        let timestamp = |offset| {
            Some(u64::from_be_bytes(
                bytes.get(offset..offset + 8)?.try_into().ok()?,
            ))
        };
        let version = (bytes[0] >> 3) & 7;
        let leap = bytes[0] >> 6;
        let stratum = bytes[1];
        let origin = timestamp(24)?;
        let receive = timestamp(32)?;
        let transmit = timestamp(40)?;
        if bytes[0] & 7 != 4
            || version != (request[0] >> 3) & 7
            || bytes[24..32] != request[40..48]
            || origin == 0
            || (local && (leap != 0 || stratum != 10 || receive == 0 || transmit < receive))
            || (!local && (leap != 3 || stratum != 0))
        {
            return None;
        }
        Some(Reply {
            version,
            leap,
            stratum,
            origin_hex: format!("{origin:016x}"),
            receive_hex: format!("{receive:016x}"),
            transmit_hex: format!("{transmit:016x}"),
            reference_id: bytes[12..16].try_into().ok()?,
            local_reference: local,
        })
    };
    decode().ok_or_else(|| {
        ValidationError::new(format!(
            "NTP reply differs from owned fixture or request origin: {bytes:02x?}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn origin_framing_status_and_version_are_independent_client_checks() {
        let request = query(4, 0x12345678, false);
        let mut bytes = vec![0; 48];
        bytes[0] = 0x24;
        bytes[1] = 10;
        bytes[24..32].copy_from_slice(&request[40..48]);
        bytes[39] = 1;
        bytes[47] = 2;
        assert!(reply(&bytes, &request, true).is_ok());
        let serialized = serde_json::to_value(reply(&bytes, &request, true).unwrap()).unwrap();
        assert_eq!(serialized["origin_hex"], "0000000012345678");
        assert!(reply(&bytes, &request, false).is_err());
        for (at, value) in [(0, 0x1c), (1, 0), (24, 1), (39, 3)] {
            let mut bad = bytes.clone();
            bad[at] = value;
            assert!(reply(&bad, &request, true).is_err());
        }
        for end in 0..48 {
            assert!(reply(&bytes[..end], &request, true).is_err());
        }
        bytes.push(0);
        assert!(reply(&bytes, &request, true).is_err());
    }
}
