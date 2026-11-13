//! Bounded RTSP/1.0 control fixture; not a media-quality or general RTP decoder.
use super::{
    ValidationError,
    wire::{Wire, invalid},
};
use serde_json::{Value, json};
use socket2::Socket;
use std::{collections::BTreeMap, net::SocketAddr};

pub(super) const IMAGE: &str =
    "bluenviron/mediamtx@sha256:59e43552f3be43695b70c19424dc2a42fcec18075f88d91561a24ac5d70a586c";
const SDP: &[u8] = b"v=0\r\no=- 0 0 IN IP4 127.0.0.1\r\ns=Gewyvern bounded fixture\r\nc=IN IP4 127.0.0.1\r\nt=0 0\r\na=control:*\r\nm=audio 0 RTP/AVP 0\r\na=rtpmap:0 PCMU/8000\r\na=control:trackID=0\r\n";

fn header(bytes: &[u8]) -> Result<(u16, BTreeMap<String, String>, usize), ValidationError> {
    if bytes.len() > 2048 || !bytes.ends_with(b"\r\n\r\n") {
        return Err(invalid());
    }
    let text = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    let mut lines = text[..text.len() - 4].split("\r\n");
    let status = lines.next().ok_or_else(invalid)?;
    let code = status
        .strip_prefix("RTSP/1.0 ")
        .and_then(|s| s.get(..3))
        .filter(|s| s.bytes().all(|b| b.is_ascii_digit()))
        .and_then(|s| s.parse::<u16>().ok())
        .filter(|v| (100..600).contains(v))
        .ok_or_else(invalid)?;
    if status.as_bytes().get(12) != Some(&b' ') {
        return Err(invalid());
    }
    let mut fields = BTreeMap::new();
    for line in lines {
        if fields.len() >= 24 || line.len() > 512 {
            return Err(invalid());
        }
        let (name, value) = line.split_once(':').ok_or_else(invalid)?;
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || !value.bytes().all(|b| b == b'\t' || (32..=126).contains(&b))
            || fields
                .insert(name.to_ascii_lowercase(), value.trim().to_owned())
                .is_some()
        {
            return Err(invalid());
        }
    }
    let size = fields
        .get("content-length")
        .map(|s| s.parse::<usize>())
        .transpose()
        .map_err(|_| invalid())?
        .unwrap_or(0);
    if size > 1024 {
        return Err(invalid());
    }
    if fields
        .get("cseq")
        .and_then(|s| s.parse::<u32>().ok())
        .is_none()
    {
        return Err(invalid());
    }
    Ok((code, fields, size))
}
fn reply_value(code: u16, fields: BTreeMap<String, String>, body: &[u8]) -> Value {
    json!({"code":code,"cseq":fields["cseq"].parse::<u32>().unwrap_or(0),
        "headers":fields,"body_bytes":body.len(),
        "body_hex":body.iter().map(|b|format!("{b:02x}")).collect::<String>()})
}
fn read(wire: &mut Wire) -> Result<Value, ValidationError> {
    let mut bytes = Vec::new();
    while bytes.len() < 2048 && !bytes.ends_with(b"\r\n\r\n") {
        bytes.extend(wire.read(1)?);
    }
    let (code, fields, size) = header(&bytes)?;
    let body = wire.read(size)?;
    Ok(reply_value(code, fields, &body))
}
fn command(
    wire: &mut Wire,
    method: &str,
    url: &str,
    cseq: u32,
    extra: &str,
    body: &[u8],
    code: u16,
) -> Result<Value, ValidationError> {
    let mut request = format!("{method} {url} RTSP/1.0\r\nCSeq: {cseq}\r\n{extra}").into_bytes();
    if !body.is_empty() {
        request.extend(format!("Content-Length: {}\r\n", body.len()).as_bytes());
    }
    request.extend(b"\r\n");
    request.extend(body);
    wire.write(&request)?;
    let reply = read(wire)
        .map_err(|error| ValidationError::new(format!("RTSP {method} CSeq {cseq}: {error}")))?;
    if reply["code"] != code || reply["cseq"] != cseq {
        return Err(ValidationError::new(format!(
            "unexpected RTSP {method} reply {reply}; expected {code}/{cseq}"
        )));
    }
    Ok(reply)
}
fn session(reply: &Value) -> Result<String, ValidationError> {
    let id = reply["headers"]["session"]
        .as_str()
        .and_then(|s| s.split(';').next())
        .ok_or_else(invalid)?;
    if id.is_empty()
        || id.len() > 128
        || !id.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    {
        return Err(invalid());
    }
    Ok(id.to_owned())
}
pub(super) fn probe(socket: Socket, target: SocketAddr) -> Result<(), ValidationError> {
    let mut wire = Wire::connect(socket, target)?;
    command(
        &mut wire,
        "OPTIONS",
        &format!("rtsp://{target}/fixture"),
        1,
        "",
        &[],
        200,
    )?;
    wire.close_write()?;
    wire.finish()?;
    Ok(())
}
pub(super) fn publish(socket: Socket, target: SocketAddr) -> Result<Wire, ValidationError> {
    let mut wire = Wire::connect(socket, target)?;
    let url = format!("rtsp://{target}/fixture");
    command(
        &mut wire,
        "ANNOUNCE",
        &url,
        1,
        "Content-Type: application/sdp\r\n",
        SDP,
        200,
    )?;
    let reply = command(
        &mut wire,
        "SETUP",
        &format!("{url}/trackID=0"),
        2,
        "Transport: RTP/AVP/TCP;unicast;interleaved=0-1;mode=record\r\n",
        &[],
        200,
    )?;
    let id = session(&reply)?;
    command(
        &mut wire,
        "RECORD",
        &url,
        3,
        &format!("Session: {id}\r\n"),
        &[],
        200,
    )?;
    // One private synthetic PCMU packet seeds RTP-Info; no user media is used.
    let mut packet = vec![b'$', 0, 0, 172, 0x80, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 1];
    packet.extend([0xff; 160]);
    wire.write(&packet)?;
    Ok(wire)
}
pub(super) fn interact(
    wire: &mut Wire,
    target: SocketAddr,
    control: bool,
) -> Result<(), ValidationError> {
    let url = format!("rtsp://{target}/fixture");
    command(wire, "OPTIONS", &url, 1, "", &[], 200)?;
    if control {
        command(
            wire,
            "DESCRIBE",
            &url,
            2,
            "Accept: application/sdp\r\n",
            &[],
            200,
        )?;
        command(
            wire,
            "SETUP",
            &format!("{url}/trackID=0"),
            3,
            "Transport: RTP/AVP;unicast;client_port=30000-30001\r\n",
            &[],
            461,
        )?;
        let setup = command(
            wire,
            "SETUP",
            &format!("{url}/trackID=0"),
            4,
            "Transport: RTP/AVP/TCP;unicast;interleaved=0-1\r\n",
            &[],
            200,
        )?;
        let id = session(&setup)?;
        let extra = format!("Session: {id}\r\n");
        for (seq, method) in [
            (5, "PLAY"),
            (6, "GET_PARAMETER"),
            (7, "PAUSE"),
            (8, "PLAY"),
            (9, "TEARDOWN"),
        ] {
            command(wire, method, &url, seq, &extra, &[], 200)?;
        }
    } else {
        command(
            wire,
            "DESCRIBE",
            &format!("rtsp://{target}/missing"),
            2,
            "Accept: application/sdp\r\n",
            &[],
            404,
        )?;
    }
    wire.close_write()
}
pub(super) fn describe(mut bytes: &[u8]) -> Result<Value, ValidationError> {
    let mut replies = Vec::new();
    while !bytes.is_empty() && replies.len() < 16 {
        let end = bytes
            .windows(4)
            .position(|b| b == b"\r\n\r\n")
            .ok_or_else(invalid)?
            + 4;
        let (code, fields, size) = header(&bytes[..end])?;
        let body = bytes.get(end..end + size).ok_or_else(invalid)?;
        replies.push(reply_value(code, fields, body));
        bytes = &bytes[end + size..];
    }
    if !bytes.is_empty() || replies.is_empty() {
        return Err(invalid());
    }
    Ok(json!({"replies":replies}))
}
pub(super) fn expected(control: bool) -> Value {
    if control {
        json!([200, 200, 461, 200, 200, 200, 200, 200, 200])
    } else {
        json!([200, 404])
    }
}
pub(super) fn candidate(packet: &crate::ledger::PacketMetaFact, phase: &str) -> bool {
    let bytes: Vec<_> = (0..1024)
        .map_while(|offset| packet.payload_bytes.get(&offset).copied())
        .collect();
    let Some(end) = bytes.windows(4).position(|b| b == b"\r\n\r\n") else {
        return false;
    };
    let Ok((200, fields, length)) = header(&bytes[..end + 4]) else {
        return false;
    };
    match phase {
        "receive_options_ok" => fields
            .get("public")
            .is_some_and(|v| v.split(',').any(|m| m.trim() == "DESCRIBE")),
        "receive_describe_ok" => {
            fields
                .get("content-type")
                .is_some_and(|v| v == "application/sdp")
                && length > 0
        }
        "receive_setup_ok" => {
            fields.contains_key("session")
                && fields
                    .get("transport")
                    .is_some_and(|v| v.starts_with("RTP/AVP/TCP;"))
        }
        "receive_play_ok" => {
            fields.contains_key("session")
                && fields
                    .get("rtp-info")
                    .is_some_and(|v| v.starts_with("url=rtsp://"))
        }
        _ => false,
    }
}
pub(super) fn matches_expected(value: &Value, control: bool) -> bool {
    let Some(replies) = value["replies"].as_array() else {
        return false;
    };
    if json!(
        replies
            .iter()
            .map(|r| r["code"].clone())
            .collect::<Vec<_>>()
    ) != expected(control)
        || replies.iter().enumerate().any(|(i, r)| r["cseq"] != i + 1)
    {
        return false;
    }
    if !control {
        return true;
    }
    let describe = &replies[1];
    let Some(body) = describe["body_hex"].as_str() else {
        return false;
    };
    let needle = |s: &[u8]| s.iter().map(|b| format!("{b:02x}")).collect::<String>();
    describe["headers"]["content-type"] == "application/sdp"
        && body.contains(&needle(b"m=audio "))
        && body.contains(&needle(b"PCMU/8000"))
        && session(&replies[3]).is_ok()
        && replies[3]["headers"]["transport"]
            .as_str()
            .is_some_and(|s| s.contains("RTP/AVP/TCP") && s.contains("interleaved=0-1"))
        && [4, 5, 6, 7]
            .iter()
            .all(|&i| session(&replies[i]).ok() == session(&replies[3]).ok())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn bounded_rtsp_frames_preserve_body_and_case_insensitive_headers() {
        let v = describe(b"RTSP/1.0 200 OK\r\ncSeQ: 1\r\nContent-Length: 12\r\n\r\nRTSP/1.0 404")
            .unwrap();
        assert_eq!(v["replies"][0]["code"], 200);
        assert_eq!(v["replies"][0]["body_bytes"], 12);
        let v =
            describe(b"RTSP/1.0 200 OK\r\ncSeQ: 1\r\nContent-Length: 12\r\n\r\nRTSP/1.0 404!\r\n")
                .unwrap_err();
        assert!(!v.to_string().is_empty());
        let v = describe(b"RTSP/1.0 200 OK\r\ncSeQ: 1\r\nContent-Length: 11\r\n\r\nRTSP/1.0 40")
            .unwrap();
        assert_eq!(v["replies"][0]["body_bytes"], 11);
    }
    #[test]
    fn rtsp_parser_rejects_duplicate_truncated_oversized_and_interleaved_frames() {
        for bytes in [
            b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nCSEQ: 2\r\n\r\n".as_slice(),
            b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nContent-Length: 9999\r\n\r\n",
            b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nContent-Length: 2\r\n\r\nx",
            b"RTSP/1.0 2000 OK\r\nCSeq: 1\r\n\r\n",
            b"$\x00\x00\x01x",
            b"RTSP/1.0 200 OK\r\nCSeq: 1\r\n",
        ] {
            assert!(describe(bytes).is_err(), "{bytes:?}");
        }
    }
}
