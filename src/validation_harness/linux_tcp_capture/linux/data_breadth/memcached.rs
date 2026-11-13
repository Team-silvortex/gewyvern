//! Fixed binary-protocol exchanges with opaque correlation; not a general cache client.
use super::ValidationError;
use super::wire::{Wire, invalid};
use serde_json::{Value, json};

const BINARY: &[u8] = b"\0\xff\r\n-ERR";
pub(super) const IMAGE: &str =
    "memcached@sha256:084fb554292d1447bc2c68b80c2331b98834514f56c866bc21a61be3c2ee376d";
struct Request {
    op: u8,
    key: &'static [u8],
    extras: Vec<u8>,
    value: Vec<u8>,
}
fn request(op: u8, key: &'static [u8], value: &[u8]) -> Request {
    Request {
        op,
        key,
        extras: if op == 1 { vec![0; 8] } else { Vec::new() },
        value: value.to_vec(),
    }
}
fn requests(control: bool) -> Vec<Request> {
    if control {
        vec![
            request(1, b"empty", b""),
            request(0, b"empty", b""),
            request(1, b"binary", BINARY),
            request(0, b"binary", b""),
            request(7, b"", b""),
        ]
    } else {
        vec![
            request(0, b"missing", b""),
            request(14, b"missing", b"x"),
            request(1, b"oversized", &vec![b'x'; 1536]),
            request(1, b"lab", b"42"),
            request(0, b"lab", b""),
            request(127, b"", b""),
            request(0, b"lab", b""),
            request(7, b"", b""),
        ]
    }
}
fn encode(request: &Request, opaque: u32) -> Vec<u8> {
    let mut bytes = vec![0; 24];
    bytes[0] = 128;
    bytes[1] = request.op;
    bytes[2..4].copy_from_slice(&(request.key.len() as u16).to_be_bytes());
    bytes[4] = request.extras.len() as u8;
    bytes[8..12].copy_from_slice(
        &((request.key.len() + request.extras.len() + request.value.len()) as u32).to_be_bytes(),
    );
    bytes[12..16].copy_from_slice(&opaque.to_be_bytes());
    bytes.extend_from_slice(&request.extras);
    bytes.extend_from_slice(request.key);
    bytes.extend_from_slice(&request.value);
    bytes
}
fn body_length(header: &[u8]) -> Result<usize, ValidationError> {
    if header.len() != 24 || header[0] != 129 || header[5] != 0 {
        return Err(invalid());
    }
    let size = u32::from_be_bytes(header[8..12].try_into().map_err(|_| invalid())?) as usize;
    if size > 2048
        || usize::from(header[4]) + usize::from(u16::from_be_bytes([header[2], header[3]])) > size
    {
        return Err(invalid());
    }
    Ok(size)
}
fn decode(header: &[u8], body: &[u8], opaque: u32) -> Result<Value, ValidationError> {
    if body_length(header)? != body.len()
        || u32::from_be_bytes(header[12..16].try_into().map_err(|_| invalid())?) != opaque
        || header[2..4] != [0, 0]
    {
        return Err(invalid());
    }
    let status = u16::from_be_bytes([header[6], header[7]]);
    let mut result = json!({"opcode":header[1],"status":status,"opaque":opaque});
    if header[1] == 0 && status == 0 {
        if header[4] != 4 || body[..4] != [0; 4] {
            return Err(invalid());
        }
        result["value_hex"] = json!(
            body[4..]
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        );
        result["value_bytes"] = json!(body.len() - 4);
    } else if header[4] != 0 || (status == 0 && !body.is_empty()) {
        return Err(invalid());
    }
    Ok(result)
}
pub(super) fn interact(wire: &mut Wire, control: bool) -> Result<(), ValidationError> {
    for (index, request) in requests(control).iter().enumerate() {
        let opaque = index as u32 + 1;
        wire.write(&encode(request, opaque))?;
        let header = wire.read(24)?;
        let body = wire.read(body_length(&header)?)?;
        let result = decode(&header, &body, opaque)?;
        if result["opcode"] != request.op {
            return Err(invalid());
        }
    }
    Ok(())
}
pub(super) fn describe(mut bytes: &[u8]) -> Result<Value, ValidationError> {
    let mut replies = Vec::new();
    while !bytes.is_empty() && replies.len() < 32 {
        let header = bytes.get(..24).ok_or_else(invalid)?;
        let size = body_length(header)?;
        let body = bytes.get(24..24 + size).ok_or_else(invalid)?;
        replies.push(decode(header, body, replies.len() as u32 + 1)?);
        bytes = &bytes[24 + size..];
    }
    if !bytes.is_empty() || replies.is_empty() {
        return Err(invalid());
    }
    Ok(json!({"replies":replies}))
}
pub(super) fn expected(control: bool) -> Value {
    let rows: Vec<(u8, u16, Option<&[u8]>)> = if control {
        vec![
            (1, 0, None),
            (0, 0, Some(b"")),
            (1, 0, None),
            (0, 0, Some(BINARY)),
            (7, 0, None),
        ]
    } else {
        vec![
            (0, 1, None),
            (14, 5, None),
            (1, 3, None),
            (1, 0, None),
            (0, 0, Some(b"42")),
            (127, 129, None),
            (0, 0, Some(b"42")),
            (7, 0, None),
        ]
    };
    json!({"replies":rows.into_iter().enumerate().map(|(index,(op,status,value))|{
        let mut result=json!({"opcode":op,"status":status,"opaque":index+1});
        if let Some(value)=value { result["value_hex"]=json!(value.iter().map(|b|format!("{b:02x}")).collect::<String>()); result["value_bytes"]=json!(value.len()); }
        result
    }).collect::<Vec<_>>()})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binary_response_status_opaque_and_body_lengths_are_checked() {
        let mut header = vec![0; 24];
        header[0] = 129;
        header[4] = 4;
        header[11] = 4;
        header[15] = 1;
        assert_eq!(decode(&header, &[0; 4], 1).unwrap()["value_hex"], "");
        assert!(decode(&header, &[0; 3], 1).is_err());
        assert!(decode(&header, &[0; 4], 2).is_err());
        header[0] = 128;
        assert!(body_length(&header).is_err());
        header[0] = 129;
        header[4] = 5;
        assert!(body_length(&header).is_err());
        for len in 0..24 {
            assert!(body_length(&header[..len]).is_err());
        }
    }
}
