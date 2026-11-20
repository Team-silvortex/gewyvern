//! Independent native client oracle; product predicates never decode these outcomes.
use super::ValidationError;
use serde::Serialize;
use std::net::{Ipv4Addr, SocketAddr};

pub(super) fn query(id: u8, kind: u8) -> Vec<u8> {
    let mut bytes = vec![0, 1, 0, 0, 0x21, 0x12, 0xa4, 0x42];
    bytes.extend([id; 12]);
    match kind {
        1 => bytes.extend([0x77, 0x77, 0, 0]),
        2 => bytes.truncate(2),
        3 => bytes.extend([0xff, 0xff, 0, 1, 0x41, 0xaa, 0xbb, 0xcc]),
        _ => {}
    }
    if bytes.len() >= 20 {
        let size = (bytes.len() - 20) as u16;
        bytes[2..4].copy_from_slice(&size.to_be_bytes());
    }
    bytes
}

#[derive(Debug, PartialEq, Serialize)]
pub(super) struct Reply {
    pub transaction_hex: String,
    pub error_code: Option<u16>,
    pub mapped_address: Option<String>,
    pub unknown_attributes: Vec<u16>,
}

pub(super) fn reply(
    bytes: &[u8],
    request: &[u8],
    expected: SocketAddr,
    denied: bool,
) -> Result<Reply, ValidationError> {
    let decode = || -> Option<Reply> {
        if bytes.len() < 20
            || bytes.len() > 128
            || request.len() < 20
            || bytes[4..8] != [0x21, 0x12, 0xa4, 0x42]
            || bytes[8..20] != request[8..20]
            || usize::from(u16::from_be_bytes([bytes[2], bytes[3]])) + 20 != bytes.len()
            || !bytes.len().is_multiple_of(4)
            || bytes[..2] != if denied { [1, 0x11] } else { [1, 1] }
        {
            return None;
        }
        let mut mapped = None;
        let mut error = None;
        let mut unknown = Vec::new();
        let mut at = 20;
        while at < bytes.len() {
            let head = bytes.get(at..at + 4)?;
            let tag = u16::from_be_bytes([head[0], head[1]]);
            let size = usize::from(u16::from_be_bytes([head[2], head[3]]));
            let body = bytes.get(at + 4..at + 4 + size)?;
            at += 4 + ((size + 3) & !3);
            if at > bytes.len() {
                return None;
            }
            match tag {
                0x0020 if mapped.is_none() => {
                    if body.len() != 8 || body[1] != 1 {
                        return None;
                    }
                    let port = u16::from_be_bytes([body[2], body[3]]) ^ 0x2112;
                    let ip = Ipv4Addr::from(
                        u32::from_be_bytes(body[4..8].try_into().ok()?) ^ 0x2112a442,
                    );
                    mapped = Some(SocketAddr::from((ip, port)));
                }
                0x0009 if error.is_none() => {
                    if body.len() < 4 {
                        return None;
                    }
                    error = Some(u16::from(body[2] & 7) * 100 + u16::from(body[3]));
                }
                0x000a => {
                    if size % 2 != 0 {
                        return None;
                    }
                    unknown.extend(
                        body.as_chunks::<2>()
                            .0
                            .iter()
                            .map(|b| u16::from_be_bytes([b[0], b[1]])),
                    );
                }
                _ => {}
            }
        }
        if denied {
            if error != Some(420) || !unknown.contains(&0x7777) || mapped.is_some() {
                return None;
            }
        } else if mapped != Some(expected) || error.is_some() {
            return None;
        }
        Some(Reply {
            transaction_hex: bytes[8..20].iter().map(|b| format!("{b:02x}")).collect(),
            error_code: error,
            mapped_address: mapped.map(|a| a.to_string()),
            unknown_attributes: unknown,
        })
    };
    decode().ok_or_else(|| {
        ValidationError::new(format!(
            "STUN reply differs from native transaction/address oracle: {bytes:02x?}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn client_checks_transaction_address_and_error_details() {
        let request = query(1, 0);
        let mut response = request.clone();
        response[0] = 1;
        response[3] = 12;
        response.extend([0, 0x20, 0, 8, 0, 1, 0x32, 0x9a, 0x5e, 0x12, 0xa4, 0x43]);
        let address = "127.0.0.1:5000".parse().unwrap();
        assert!(reply(&response, &request, address, false).is_ok());
        for at in [0, 4, 8, 27, 31] {
            let mut bad = response.clone();
            bad[at] ^= 1;
            assert!(reply(&bad, &request, address, false).is_err());
        }
        for n in 0..response.len() {
            assert!(reply(&response[..n], &request, address, false).is_err());
        }
        assert!(reply(&response, &request, address, true).is_err());
    }
}
