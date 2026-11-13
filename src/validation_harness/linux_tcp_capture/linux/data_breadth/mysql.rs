//! Bounded MySQL 4.1/native-password fixture; no TLS, compression or auth switching.
use super::ValidationError;
use super::wire::{PASSWORD, Wire, invalid};
use serde_json::{Value, json};

const CAPS: u32 = 1 | 4 | 8 | 0x200 | 0x2000 | 0x8000 | 0x80000;
const QUERIES: [&str; 5] = [
    "SELECT '42'",
    "GEWY_BAD",
    "SELECT NULL",
    "SELECT ''",
    "SELECT '43'",
];

fn take<'a>(bytes: &mut &'a [u8], count: usize) -> Result<&'a [u8], ValidationError> {
    let value = bytes.get(..count).ok_or_else(invalid)?;
    *bytes = &bytes[count..];
    Ok(value)
}
fn nul<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], ValidationError> {
    let size = bytes.iter().position(|b| *b == 0).ok_or_else(invalid)?;
    let value = take(bytes, size)?;
    take(bytes, 1)?;
    Ok(value)
}
fn greeting(mut bytes: &[u8]) -> Result<[u8; 20], ValidationError> {
    if take(&mut bytes, 1)? != [10] || nul(&mut bytes)?.is_empty() {
        return Err(invalid());
    }
    take(&mut bytes, 4)?;
    let mut salt = [0; 20];
    salt[..8].copy_from_slice(take(&mut bytes, 8)?);
    if take(&mut bytes, 1)? != [0] {
        return Err(invalid());
    }
    let lower = u16::from_le_bytes(take(&mut bytes, 2)?.try_into().map_err(|_| invalid())?);
    take(&mut bytes, 3)?;
    let upper = u16::from_le_bytes(take(&mut bytes, 2)?.try_into().map_err(|_| invalid())?);
    if (u32::from(lower) | (u32::from(upper) << 16)) & CAPS != CAPS
        || take(&mut bytes, 1)? != [21]
        || take(&mut bytes, 10)? != [0; 10]
    {
        return Err(invalid());
    }
    salt[8..].copy_from_slice(take(&mut bytes, 12)?);
    if take(&mut bytes, 1)? != [0]
        || nul(&mut bytes)? != b"mysql_native_password"
        || !bytes.is_empty()
    {
        return Err(ValidationError::new(
            "fixture requires direct mysql_native_password handshake",
        ));
    }
    Ok(salt)
}
fn token(password: &[u8], salt: &[u8; 20]) -> [u8; 20] {
    use ring::digest::{SHA1_FOR_LEGACY_USE_ONLY, digest};
    let first = digest(&SHA1_FOR_LEGACY_USE_ONLY, password);
    let second = digest(&SHA1_FOR_LEGACY_USE_ONLY, first.as_ref());
    let mut input = salt.to_vec();
    input.extend_from_slice(second.as_ref());
    let third = digest(&SHA1_FOR_LEGACY_USE_ONLY, &input);
    let mut response = [0; 20];
    for (index, byte) in response.iter_mut().enumerate() {
        *byte = first.as_ref()[index] ^ third.as_ref()[index];
    }
    response
}
fn length(header: &[u8], sequence: u8) -> Result<usize, ValidationError> {
    if header.len() != 4 || header[3] != sequence {
        return Err(ValidationError::new("MySQL fixture sequence mismatch"));
    }
    let count =
        usize::from(header[0]) | (usize::from(header[1]) << 8) | (usize::from(header[2]) << 16);
    if !(1..=2048).contains(&count) {
        return Err(invalid());
    }
    Ok(count)
}
fn read(wire: &mut Wire, sequence: u8) -> Result<Vec<u8>, ValidationError> {
    let count = length(&wire.read(4)?, sequence)?;
    wire.read(count)
}
fn send(wire: &mut Wire, sequence: u8, body: &[u8]) -> Result<(), ValidationError> {
    if body.is_empty() || body.len() > 2048 {
        return Err(invalid());
    }
    let len = body.len().to_le_bytes();
    let mut bytes = vec![len[0], len[1], len[2], sequence];
    bytes.extend_from_slice(body);
    wire.write(&bytes)
}
fn error(body: &[u8]) -> Result<Value, ValidationError> {
    if body.len() < 9
        || body[0] != 255
        || body[3] != b'#'
        || !body[4..9]
            .iter()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
    {
        return Err(invalid());
    }
    Ok(
        json!({"error":u16::from_le_bytes([body[1],body[2]]),"sqlstate":std::str::from_utf8(&body[4..9]).map_err(|_| invalid())?}),
    )
}
fn ok(body: &[u8]) -> Result<Value, ValidationError> {
    if body.len() != 7 || body[..3] != [0, 0, 0] || body[3..] != [2, 0, 0, 0] {
        return Err(invalid());
    }
    Ok(json!({"ok":true}))
}
fn short_string<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], ValidationError> {
    let length = usize::from(take(bytes, 1)?[0]);
    if length >= 251 {
        return Err(invalid());
    }
    take(bytes, length)
}
fn result(
    next: &mut impl FnMut(u8) -> Result<Vec<u8>, ValidationError>,
) -> Result<Value, ValidationError> {
    let body = next(1)?;
    match body.first() {
        Some(255) => error(&body),
        Some(0) => ok(&body),
        Some(1) if body.len() == 1 => {
            let definition = next(2)?;
            let mut fields = definition.as_slice();
            for _ in 0..6 {
                short_string(&mut fields)?;
            }
            if fields.len() != 13 || fields[0] != 12 || fields[11..] != [0, 0] {
                return Err(invalid());
            }
            let eof = |bytes: Vec<u8>| {
                if bytes == [254, 0, 0, 2, 0] {
                    Ok(())
                } else {
                    Err(invalid())
                }
            };
            eof(next(3)?)?;
            let row = next(4)?;
            let value = if row == [251] {
                Value::Null
            } else {
                let mut row = row.as_slice();
                let value = std::str::from_utf8(short_string(&mut row)?)
                    .map_err(|_| invalid())?
                    .to_owned();
                if !row.is_empty() {
                    return Err(invalid());
                }
                json!(value)
            };
            eof(next(5)?)?;
            Ok(json!({"row":value}))
        }
        _ => Err(invalid()),
    }
}
pub(super) fn interact(wire: &mut Wire, control: bool) -> Result<(), ValidationError> {
    let salt = greeting(&read(wire, 0)?)?;
    let mut login = CAPS.to_le_bytes().to_vec();
    login.extend_from_slice(&4096u32.to_le_bytes());
    login.push(45);
    login.extend_from_slice(&[0; 23]);
    login.extend_from_slice(b"gewy\0");
    login.push(20);
    login.extend_from_slice(&token(
        if control {
            PASSWORD.as_bytes()
        } else {
            b"invalid-lab-password"
        },
        &salt,
    ));
    login.extend_from_slice(b"gewy\0mysql_native_password\0");
    send(wire, 1, &login)?;
    let auth = read(wire, 2)?;
    if !control {
        error(&auth)?;
        return Ok(());
    }
    ok(&auth)?;
    for query in QUERIES {
        let mut body = vec![3];
        body.extend_from_slice(query.as_bytes());
        send(wire, 0, &body)?;
        result(&mut |seq| read(wire, seq))?;
    }
    send(wire, 0, &[14])?;
    ok(&read(wire, 1)?)?;
    send(wire, 0, &[1])
}
pub(super) fn describe(mut bytes: &[u8]) -> Result<Value, ValidationError> {
    let mut next = |seq| {
        let count = length(take(&mut bytes, 4)?, seq)?;
        Ok(take(&mut bytes, count)?.to_vec())
    };
    greeting(&next(0)?)?;
    let auth = next(2)?;
    let report = if auth.first() == Some(&255) {
        json!({"auth":error(&auth)?,"results":[],"ping":false})
    } else {
        ok(&auth)?;
        let mut results = Vec::new();
        for _ in QUERIES {
            results.push(result(&mut next)?);
        }
        ok(&next(1)?)?;
        json!({"auth":{"ok":true},"results":results,"ping":true})
    };
    if !bytes.is_empty() {
        return Err(invalid());
    }
    Ok(report)
}
pub(super) fn expected(control: bool) -> Value {
    if control {
        json!({"auth":{"ok":true},"results":[{"row":"42"},{"error":1064,"sqlstate":"42000"},{"row":null},{"row":""},{"row":"43"}],"ping":true})
    } else {
        json!({"auth":{"error":1045,"sqlstate":"28000"},"results":[],"ping":false})
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mysql_frames_reject_sequence_length_and_sqlstate_confusion() {
        assert_eq!(length(&[9, 0, 0, 2], 2).unwrap(), 9);
        for header in [[0, 0, 0, 2], [9, 0, 0, 1], [1, 16, 0, 2]] {
            assert!(length(&header, 2).is_err());
        }
        assert_eq!(
            error(b"\xff\x15\x04#28000query text").unwrap()["error"],
            1045
        );
        for body in [
            b"\xff".as_slice(),
            b"\xff\x15\x04!28000",
            b"\xff\x15\x04#28\0xx",
        ] {
            assert!(error(body).is_err());
        }
        assert!(ok(&[0, 0, 0]).is_err());
    }
    #[test]
    fn mysql_result_frames_distinguish_null_empty_and_nonempty_rows() {
        for (row, expected) in [
            (vec![251], Value::Null),
            (vec![0], json!("")),
            (vec![2, b'4', b'2'], json!("42")),
        ] {
            let mut column = vec![0; 6];
            column.extend([12, 45, 0, 1, 0, 0, 0, 253, 0, 0, 0, 0, 0]);
            let frames = [
                vec![1],
                column,
                vec![254, 0, 0, 2, 0],
                row,
                vec![254, 0, 0, 2, 0],
            ];
            let mut calls = 0;
            let value = result(&mut |seq| {
                calls += 1;
                assert_eq!(seq, calls);
                Ok(frames[usize::from(seq) - 1].clone())
            })
            .unwrap();
            assert_eq!(value["row"], expected);
        }
    }
}
