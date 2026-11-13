//! Bounded SMTP submission fixture, not a mail client or delivery oracle.
use super::ValidationError;
use super::wire::{Wire, invalid};
use serde_json::{Value, json};

pub(super) const IMAGE: &str =
    "axllent/mailpit@sha256:9b3a463be15354c0f0eba91697d80e3e0d8a5623ef0e627cacc836bc17fd5231";
const AUTH_OK: &[u8] = b"AUTH PLAIN AGdld3kAZ2V3eXZlcm4tZGF0YS1sYWItb25seQ==\r\n";
const AUTH_BAD: &[u8] = b"AUTH PLAIN AGdld3kAd3Jvbmc=\r\n";
const MESSAGE: &[u8] = b"From: sender@example.test\r\nTo: inbox@gewy.test\r\nSubject: bounded fixture\r\n\r\n550 this is body text, not a server reply\r\n..dot-stuffed line\r\n.\r\n";

pub(super) fn seed_pop3(
    socket: socket2::Socket,
    target: std::net::SocketAddr,
) -> Result<(), ValidationError> {
    let mut wire = Wire::connect(socket, target)?;
    expect(&mut wire, 220)?;
    command(&mut wire, b"EHLO gewy.test\r\n", 250)?;
    command(&mut wire, AUTH_OK, 235)?;
    for (index, body) in [super::pop3::FIRST_BODY, super::pop3::SECOND_BODY]
        .into_iter()
        .enumerate()
    {
        command(&mut wire, b"MAIL FROM:<sender@example.test>\r\n", 250)?;
        command(&mut wire, b"RCPT TO:<inbox@gewy.test>\r\n", 250)?;
        command(&mut wire, b"DATA\r\n", 354)?;
        let mut message = format!("From: sender@example.test\r\nTo: inbox@gewy.test\r\nSubject: pop3-fixture-{index}\r\nContent-Type: text/plain\r\n\r\n").into_bytes();
        for line in body.split_inclusive(|b| *b == b'\n') {
            if line.starts_with(b".") {
                message.push(b'.');
            }
            message.extend_from_slice(line);
        }
        message.extend_from_slice(b".\r\n");
        command(&mut wire, &message, 250)?;
    }
    command(&mut wire, b"QUIT\r\n", 221)?;
    wire.finish()?;
    Ok(())
}

fn reply(
    mut line: impl FnMut() -> Result<Vec<u8>, ValidationError>,
) -> Result<Value, ValidationError> {
    let mut code = None;
    let mut lines = Vec::new();
    for _ in 0..16 {
        let bytes = line()?;
        if bytes.len() < 6
            || bytes.len() > 512
            || !bytes.ends_with(b"\r\n")
            || !matches!(bytes[0], b'2'..=b'5')
            || !bytes[1..3].iter().all(u8::is_ascii_digit)
            || !matches!(bytes[3], b' ' | b'-')
            || !bytes[4..bytes.len() - 2]
                .iter()
                .all(|b| matches!(b, b'\t' | 32..=126))
        {
            return Err(invalid());
        }
        let current = u16::from(bytes[0] - b'0') * 100
            + u16::from(bytes[1] - b'0') * 10
            + u16::from(bytes[2] - b'0');
        if code.is_some_and(|previous| previous != current) {
            return Err(invalid());
        }
        code = Some(current);
        lines.push(String::from_utf8(bytes[4..bytes.len() - 2].to_vec()).map_err(|_| invalid())?);
        if bytes[3] == b' ' {
            return Ok(json!({"code":current,"lines":lines}));
        }
    }
    Err(invalid())
}
fn read(wire: &mut Wire) -> Result<Value, ValidationError> {
    reply(|| {
        let mut bytes = Vec::new();
        while bytes.len() < 512 && !bytes.ends_with(b"\r\n") {
            bytes.extend(wire.read(1)?);
        }
        Ok(bytes)
    })
}
fn expect(wire: &mut Wire, code: u16) -> Result<(), ValidationError> {
    let actual = read(wire)?;
    if actual["code"] != code {
        return Err(ValidationError::new(format!(
            "unexpected SMTP reply {actual}, expected {code}"
        )));
    }
    Ok(())
}
fn command(wire: &mut Wire, bytes: &[u8], code: u16) -> Result<(), ValidationError> {
    wire.write(bytes)?;
    expect(wire, code)
}
pub(super) fn interact(wire: &mut Wire, control: bool) -> Result<(), ValidationError> {
    expect(wire, 220)?;
    command(wire, b"EHLO gewy.test\r\n", 250)?;
    command(
        wire,
        if control { AUTH_OK } else { AUTH_BAD },
        if control { 235 } else { 535 },
    )?;
    if control {
        command(wire, b"DATA\r\n", 503)?;
        command(wire, b"MAIL FROM:<sender@example.test>\r\n", 250)?;
        command(wire, b"RCPT TO:<denied@example.test>\r\n", 550)?;
        command(wire, b"RCPT TO:<inbox@gewy.test>\r\n", 250)?;
        command(wire, b"DATA\r\n", 354)?;
        command(wire, MESSAGE, 250)?;
        command(wire, b"RSET\r\n", 250)?;
        command(wire, b"MAIL FROM:<>\r\n", 250)?;
        command(wire, b"RCPT TO:<inbox@gewy.test>\r\n", 250)?;
        command(wire, b"DATA\r\n", 354)?;
        command(
            wire,
            b"From: sender@example.test\r\nSubject: recovery\r\n\r\n42\r\n.\r\n",
            250,
        )?;
    } else {
        command(wire, b"MAIL FROM:<sender@example.test>\r\n", 530)?;
        command(wire, b"NOOP\r\n", 250)?;
    }
    command(wire, b"QUIT\r\n", 221)
}
pub(super) fn describe(mut bytes: &[u8]) -> Result<Value, ValidationError> {
    let mut replies = Vec::new();
    while !bytes.is_empty() && replies.len() < 32 {
        replies.push(reply(|| {
            let end = bytes
                .windows(2)
                .position(|b| b == b"\r\n")
                .ok_or_else(invalid)?
                + 2;
            let line = bytes[..end].to_vec();
            bytes = &bytes[end..];
            Ok(line)
        })?);
    }
    if !bytes.is_empty() || replies.is_empty() {
        return Err(invalid());
    }
    Ok(json!({"replies":replies}))
}
pub(super) fn expected(control: bool) -> Value {
    if control {
        json!([
            220, 250, 235, 503, 250, 550, 250, 354, 250, 250, 250, 250, 354, 250, 221
        ])
    } else {
        json!([220, 250, 535, 530, 250, 221])
    }
}
pub(super) fn matches_expected(actual: &Value, control: bool) -> bool {
    let Some(replies) = actual["replies"].as_array() else {
        return false;
    };
    let codes: Vec<_> = replies.iter().map(|reply| reply["code"].clone()).collect();
    let first_line = |index: usize| {
        replies
            .get(index)
            .and_then(|reply| reply["lines"][0].as_str())
    };
    let envelope = if control {
        first_line(3).is_some_and(|s| s.starts_with("5.5.1 "))
            && first_line(4) == Some("2.1.0 Ok")
            && first_line(5).is_some_and(|s| s.starts_with("5.1.0 "))
            && first_line(6) == Some("2.1.5 Ok")
            && [8, 13].into_iter().all(|index| {
                first_line(index).is_some_and(|s| {
                    s.strip_prefix("2.0.0 Ok: queued as ").is_some_and(|id| {
                        !id.is_empty() && id.bytes().all(|b| b.is_ascii_alphanumeric())
                    })
                })
            })
            && first_line(9) == Some("2.0.0 Ok")
            && first_line(10) == Some("2.1.0 Ok")
            && first_line(11) == Some("2.1.5 Ok")
    } else {
        first_line(2).is_some_and(|s| s.starts_with("5.7.8 "))
            && first_line(3).is_some_and(|s| s.starts_with("5.7.0 "))
            && first_line(4) == Some("2.0.0 Ok")
    };
    envelope
        && json!(codes) == expected(control)
        && replies
            .get(1)
            .and_then(|reply| reply["lines"].as_array())
            .is_some_and(|lines| {
                lines.len() > 1
                    && lines.iter().any(|line| {
                        line.as_str()
                            .is_some_and(|s| s.starts_with("AUTH ") && s.contains("PLAIN"))
                    })
            })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn multiline_replies_keep_boundaries_and_reject_crossed_or_truncated_codes() {
        let parsed = describe(b"220 ready\r\n250-host\r\n250-AUTH PLAIN LOGIN\r\n250 SIZE 1024\r\n535 5.7.8 bad login\r\n").unwrap();
        assert_eq!(parsed["replies"].as_array().unwrap().len(), 3);
        assert_eq!(parsed["replies"][1]["lines"].as_array().unwrap().len(), 3);
        for bad in [
            b"250-more\r\n".as_slice(),
            b"250-more\r\n550 denied\r\n",
            b"250 OK\n",
            b"250 OK\r\ntrailing",
            b"250XOK\r\n",
            b"200 bad\0text\r\n",
            b"99x wrong\r\n",
        ] {
            assert!(describe(bad).is_err(), "{bad:?}");
        }
        assert!(describe(b"250-more\r\n".repeat(17).as_slice()).is_err());
        assert!(describe(format!("250 {}\r\n", "x".repeat(507)).as_bytes()).is_err());
    }
    #[test]
    fn equal_codes_do_not_replace_a_multiline_ehlo_capability_reply() {
        let mut replies: Vec<_> = expected(false)
            .as_array()
            .unwrap()
            .iter()
            .map(|code| json!({"code":code,"lines":["OK"]}))
            .collect();
        assert!(!matches_expected(&json!({"replies":replies}), false));
        replies[1]["lines"] = json!(["host", "AUTH PLAIN LOGIN"]);
        replies[2]["lines"] = json!(["5.7.8 Authentication credentials invalid"]);
        replies[3]["lines"] = json!(["5.7.0 Authentication required"]);
        replies[4]["lines"] = json!(["2.0.0 Ok"]);
        assert!(matches_expected(&json!({"replies":replies}), false));
        assert!(!matches_expected(&json!({"replies":replies}), true));
    }
}
