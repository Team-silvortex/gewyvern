//! Bounded core NATS client: byte-counted MSG bodies never become control lines.
use super::ValidationError;
use super::wire::{PASSWORD, Wire, invalid};
use serde_json::{Value, json};

const VALUES: [&[u8]; 3] = [b"\0\xff\r\n-ERR", b"", b"-ERR not-an-error"];
const PERMISSION: &str = "Permissions Violation for Publish to \"denied\"";
fn line<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], ValidationError> {
    let end = bytes
        .windows(2)
        .position(|pair| pair == b"\r\n")
        .filter(|end| *end <= 1024)
        .ok_or_else(invalid)?;
    let result = &bytes[..end];
    *bytes = &bytes[end + 2..];
    Ok(result)
}
fn body_size(header: &[u8]) -> Result<Option<usize>, ValidationError> {
    if !header.starts_with(b"MSG ") {
        return Ok(None);
    }
    let header = std::str::from_utf8(header).map_err(|_| invalid())?;
    let parts: Vec<_> = header.split(' ').collect();
    if parts.len() != 4
        || parts[..3] != ["MSG", "gewy.lab", "1"]
        || parts[3].is_empty()
        || !parts[3].bytes().all(|b| b.is_ascii_digit())
    {
        return Err(invalid());
    }
    let size = parts[3].parse::<usize>().map_err(|_| invalid())?;
    if size > 1024 {
        return Err(invalid());
    }
    Ok(Some(size))
}
fn decode(bytes: &mut &[u8]) -> Result<Value, ValidationError> {
    let header = line(bytes)?;
    if let Some(size) = body_size(header)? {
        let body = bytes.get(..size).ok_or_else(invalid)?;
        if bytes.get(size..size + 2) != Some(b"\r\n") {
            return Err(invalid());
        }
        let value = json!({"kind":"message","subject":"gewy.lab","sid":1,"value_hex":body.iter().map(|b|format!("{b:02x}")).collect::<String>(),"value_bytes":size});
        *bytes = &bytes[size + 2..];
        return Ok(value);
    }
    if let Some(info) = header.strip_prefix(b"INFO ") {
        let info: Value = serde_json::from_slice(info)?;
        if info["auth_required"] != true || info["max_payload"] != 1024 || info["port"] != 4222 {
            return Err(invalid());
        }
        return Ok(json!({"kind":"info","auth_required":true,"max_payload":1024}));
    }
    if header == b"PONG" {
        return Ok(json!({"kind":"pong"}));
    }
    if let Some(message) = header
        .strip_prefix(b"-ERR '")
        .and_then(|rest| rest.strip_suffix(b"'"))
    {
        return Ok(
            json!({"kind":"error","message":std::str::from_utf8(message).map_err(|_|invalid())?}),
        );
    }
    Err(invalid())
}
fn read(wire: &mut Wire) -> Result<Value, ValidationError> {
    let mut bytes = Vec::new();
    while !bytes.ends_with(b"\r\n") && bytes.len() < 1026 {
        bytes.extend(wire.read(1)?);
    }
    if !bytes.ends_with(b"\r\n") {
        return Err(invalid());
    }
    if let Some(size) = body_size(&bytes[..bytes.len() - 2])? {
        bytes.extend(wire.read(size + 2)?);
    }
    let mut cursor = bytes.as_slice();
    let result = decode(&mut cursor)?;
    if !cursor.is_empty() {
        return Err(invalid());
    }
    Ok(result)
}
fn expect(wire: &mut Wire, wanted: Value) -> Result<(), ValidationError> {
    for _ in 0..5 {
        let actual = read(wire)?;
        if actual == wanted {
            return Ok(());
        }
        // Protocol-1 INFO updates can interleave with replies; keep their raw bytes.
        if actual["kind"] == "info" && wanted["kind"] != "info" {
            continue;
        }
        return Err(ValidationError::new(format!(
            "unexpected NATS response: {actual}, expected {wanted}"
        )));
    }
    Err(ValidationError::new("NATS INFO update bound exceeded"))
}
fn message(value: &[u8]) -> Value {
    json!({"kind":"message","subject":"gewy.lab","sid":1,"value_hex":value.iter().map(|b|format!("{b:02x}")).collect::<String>(),"value_bytes":value.len()})
}
fn publish(wire: &mut Wire, value: &[u8]) -> Result<(), ValidationError> {
    let mut request = format!("PUB gewy.lab {}\r\n", value.len()).into_bytes();
    request.extend_from_slice(value);
    request.extend_from_slice(b"\r\nPING\r\n");
    wire.write(&request)?;
    expect(wire, message(value))?;
    expect(wire, json!({"kind":"pong"}))
}
pub(super) fn interact(wire: &mut Wire, control: bool) -> Result<(), ValidationError> {
    expect(
        wire,
        json!({"kind":"info","auth_required":true,"max_payload":1024}),
    )?;
    let options = json!({"verbose":false,"pedantic":true,"protocol":1,"echo":true,"user":"gewy","pass":if control {PASSWORD}else{"invalid-lab-password"}});
    wire.write(format!("CONNECT {options}\r\nPING\r\n").as_bytes())?;
    if !control {
        return expect(
            wire,
            json!({"kind":"error","message":"Authorization Violation"}),
        );
    }
    expect(wire, json!({"kind":"pong"}))?;
    wire.write(b"SUB gewy.lab 1\r\nPING\r\n")?;
    expect(wire, json!({"kind":"pong"}))?;
    // Negative bodies precede the real error so stage deduplication cannot hide a false match.
    for value in VALUES {
        publish(wire, value)?;
    }
    wire.write(b"PUB denied 1\r\nx\r\nPING\r\n")?;
    expect(wire, json!({"kind":"error","message":PERMISSION}))?;
    expect(wire, json!({"kind":"pong"}))?;
    publish(wire, b"43")?;
    wire.close_write()
}
pub(super) fn describe(mut bytes: &[u8]) -> Result<Value, ValidationError> {
    let mut replies = Vec::new();
    while !bytes.is_empty() && replies.len() < 32 {
        replies.push(decode(&mut bytes)?);
    }
    if !bytes.is_empty() || replies.is_empty() {
        return Err(invalid());
    }
    Ok(json!({"replies":replies}))
}
pub(super) fn expected(control: bool) -> Value {
    let mut replies = vec![json!({"kind":"info","auth_required":true,"max_payload":1024})];
    if control {
        replies.extend([json!({"kind":"pong"}), json!({"kind":"pong"})]);
        for value in VALUES {
            replies.extend([message(value), json!({"kind":"pong"})]);
        }
        replies.extend([
            json!({"kind":"error","message":PERMISSION}),
            json!({"kind":"pong"}),
            message(b"43"),
            json!({"kind":"pong"}),
        ]);
    } else {
        replies.push(json!({"kind":"error","message":"Authorization Violation"}));
    }
    json!({"replies":replies})
}

pub(super) fn matches_expected(result: &Value, control: bool) -> bool {
    let Some(replies) = result["replies"].as_array() else {
        return false;
    };
    let info_count = replies
        .iter()
        .filter(|reply| reply["kind"] == "info")
        .count();
    (1..=4).contains(&info_count)
        && json!({"replies":replies.iter().enumerate()
        .filter(|(index,reply)|*index==0 || reply["kind"]!="info")
        .map(|(_,reply)|reply).collect::<Vec<_>>()})
            == expected(control)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn asynchronous_info_is_preserved_but_cannot_replace_a_command_reply() {
        let mut transcript = expected(true);
        let info = transcript["replies"][0].clone();
        transcript["replies"]
            .as_array_mut()
            .unwrap()
            .insert(1, info.clone());
        assert!(matches_expected(&transcript, true));
        assert_eq!(transcript["replies"][1], info);
        transcript["replies"][2] = info.clone();
        assert!(!matches_expected(&transcript, true));
        transcript = expected(true);
        for _ in 0..4 {
            transcript["replies"]
                .as_array_mut()
                .unwrap()
                .push(info.clone());
        }
        assert!(!matches_expected(&transcript, true));
    }
    #[test]
    fn nats_byte_counted_messages_preserve_control_looking_and_binary_bodies() {
        for value in VALUES {
            let mut bytes = format!("MSG gewy.lab 1 {}\r\n", value.len()).into_bytes();
            bytes.extend_from_slice(value);
            bytes.extend_from_slice(b"\r\nPONG\r\n");
            assert_eq!(
                describe(&bytes).unwrap(),
                json!({"replies":[message(value),{"kind":"pong"}]})
            );
        }
        for bytes in [
            b"MSG gewy.lab 1 1025\r\n".as_slice(),
            b"MSG gewy.lab 2 0\r\n\r\n",
            b"MSG gewy.lab 1 -1\r\n",
            b"MSG gewy.lab 1 2\r\nx\r\n",
            b"-ERR unquoted\r\n",
            b"PONG\r",
        ] {
            assert!(describe(bytes).is_err(), "{bytes:?}");
        }
    }
}
