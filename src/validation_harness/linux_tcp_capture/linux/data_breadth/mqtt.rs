//! Fixed MQTT 3.1.1 exchanges; session resume and packet IDs are checked independently.
use super::ValidationError;
use super::wire::{Wire, invalid};
use serde_json::{Value, json};
use socket2::Socket;
use std::net::SocketAddr;

const TOPIC: &[u8] = b"gewy/in";
const BINARY: &[u8] = b"\0\xff\r\n-ERR";

fn string(bytes: &mut Vec<u8>, value: &[u8]) {
    bytes.extend_from_slice(&(value.len() as u16).to_be_bytes());
    bytes.extend_from_slice(value);
}
fn send(wire: &mut Wire, header: u8, body: &[u8]) -> Result<(), ValidationError> {
    if body.len() >= 128 {
        return Err(invalid());
    }
    let mut frame = vec![header, body.len() as u8];
    frame.extend_from_slice(body);
    wire.write(&frame)
}
fn connect(wire: &mut Wire, client: &[u8], clean: bool) -> Result<(), ValidationError> {
    let mut body = b"\0\x04MQTT".to_vec();
    body.extend([4, if clean { 2 } else { 0 }, 0, 10]);
    string(&mut body, client);
    send(wire, 16, &body)
}
fn decode(header: u8, body: &[u8]) -> Result<Value, ValidationError> {
    match header {
        32 if body.len() == 2 && body[0] <= 1 && body[1] <= 5 && (body[1] == 0 || body[0] == 0) => {
            Ok(json!({"kind":"connack","session_present":body[0]==1,"code":body[1]}))
        }
        64 | 80 | 112 if body.len() == 2 && body != [0, 0] => Ok(
            json!({"kind":match header {64=>"puback",80=>"pubrec",_=>"pubcomp"},"id":u16::from_be_bytes([body[0],body[1]])}),
        ),
        144 if body.len() == 3 && body[..2] != [0, 0] && matches!(body[2], 0..=2 | 128) => {
            Ok(json!({"kind":"suback","id":u16::from_be_bytes([body[0],body[1]]),"code":body[2]}))
        }
        208 if body.is_empty() => Ok(json!({"kind":"pingresp"})),
        48 if body.len() >= 2 => {
            let len = usize::from(u16::from_be_bytes([body[0], body[1]]));
            if body.get(2..2 + len) != Some(TOPIC) {
                return Err(invalid());
            }
            let value = &body[2 + len..];
            Ok(
                json!({"kind":"publish","topic":"gewy/in","value_hex":value.iter().map(|b|format!("{b:02x}")).collect::<String>(),"value_bytes":value.len()}),
            )
        }
        _ => Err(invalid()),
    }
}
fn read(wire: &mut Wire) -> Result<Value, ValidationError> {
    let header = wire.read(2)?;
    // These fixed frames use only the canonical one-byte Remaining Length form.
    if header[1] >= 128 {
        return Err(invalid());
    }
    decode(header[0], &wire.read(usize::from(header[1]))?)
}
fn expect(wire: &mut Wire, expected: Value) -> Result<(), ValidationError> {
    let actual = read(wire)?;
    if actual != expected {
        return Err(ValidationError::new(format!(
            "unexpected MQTT frame: {actual}, expected {expected}"
        )));
    }
    Ok(())
}
fn subscribe(wire: &mut Wire, id: u16) -> Result<(), ValidationError> {
    let mut body = id.to_be_bytes().to_vec();
    string(&mut body, TOPIC);
    body.push(0);
    send(wire, 130, &body)?;
    expect(wire, json!({"kind":"suback","id":id,"code":0}))
}
pub(super) fn seed(socket: Socket, target: SocketAddr) -> Result<(), ValidationError> {
    let mut wire = Wire::connect(socket, target)?;
    connect(&mut wire, b"gewy-persistent", false)?;
    expect(
        &mut wire,
        json!({"kind":"connack","session_present":false,"code":0}),
    )?;
    subscribe(&mut wire, 1)?;
    send(&mut wire, 224, &[])?;
    wire.finish()?;
    Ok(())
}
pub(super) fn interact(wire: &mut Wire, control: bool) -> Result<(), ValidationError> {
    connect(wire, if control { b"gewy-persistent" } else { b"" }, false)?;
    expect(
        wire,
        json!({"kind":"connack","session_present":control,"code":if control {0}else{2}}),
    )?;
    if !control {
        return Ok(());
    }
    for (header, id) in [(50, 7u16), (52, 8)] {
        let mut body = Vec::new();
        string(&mut body, b"gewy/out");
        body.extend_from_slice(&id.to_be_bytes());
        body.extend_from_slice(b"42");
        send(wire, header, &body)?;
        expect(
            wire,
            json!({"kind":if id==7 {"puback"}else{"pubrec"},"id":id}),
        )?;
        if id == 8 {
            send(wire, 98, &id.to_be_bytes())?;
            expect(wire, json!({"kind":"pubcomp","id":id}))?;
        }
    }
    let mut body = Vec::new();
    string(&mut body, TOPIC);
    body.extend_from_slice(BINARY);
    send(wire, 48, &body)?;
    expect(
        wire,
        json!({"kind":"publish","topic":"gewy/in","value_hex":"00ff0d0a2d455252","value_bytes":8}),
    )?;
    subscribe(wire, 9)?;
    send(wire, 192, &[])?;
    expect(wire, json!({"kind":"pingresp"}))?;
    send(wire, 224, &[])
}
pub(super) fn describe(mut bytes: &[u8]) -> Result<Value, ValidationError> {
    let mut replies = Vec::new();
    while !bytes.is_empty() && replies.len() < 32 {
        let header = bytes.get(..2).ok_or_else(invalid)?;
        if header[1] >= 128 {
            return Err(invalid());
        }
        let end = 2 + usize::from(header[1]);
        replies.push(decode(header[0], bytes.get(2..end).ok_or_else(invalid)?)?);
        bytes = &bytes[end..];
    }
    if !bytes.is_empty() || replies.is_empty() {
        return Err(invalid());
    }
    Ok(json!({"replies":replies}))
}
pub(super) fn expected(control: bool) -> Value {
    if !control {
        return json!({"replies":[{"kind":"connack","session_present":false,"code":2}]});
    }
    json!({"replies":[
        {"kind":"connack","session_present":true,"code":0},
        {"kind":"puback","id":7},{"kind":"pubrec","id":8},{"kind":"pubcomp","id":8},
        {"kind":"publish","topic":"gewy/in","value_hex":"00ff0d0a2d455252","value_bytes":8},
        {"kind":"suback","id":9,"code":0},{"kind":"pingresp"}
    ]})
}

pub(super) fn probe(socket: Socket, target: SocketAddr) -> Result<(), ValidationError> {
    let mut wire = Wire::connect(socket, target)?;
    connect(&mut wire, b"gewy-ready", true)?;
    expect(
        &mut wire,
        json!({"kind":"connack","session_present":false,"code":0}),
    )?;
    send(&mut wire, 224, &[])?;
    wire.finish()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mqtt_frames_reject_bad_flags_lengths_ids_and_refusal_session_state() {
        assert_eq!(
            describe(&[32, 2, 1, 0]).unwrap()["replies"][0]["session_present"],
            true
        );
        for bytes in [
            &[32, 2, 1, 1][..],
            &[32, 2, 2, 0],
            &[33, 2, 0, 0],
            &[32, 2, 0],
            &[32, 128],
            &[80, 2, 0, 0],
            &[112, 3, 0, 1, 0],
            &[48, 3, 0, 7, 0],
        ] {
            assert!(describe(bytes).is_err(), "{bytes:?}");
        }
        assert_eq!(
            describe(&[144, 3, 0, 9, 128]).unwrap()["replies"][0]["code"],
            128
        );
    }
}
