//! Fixed POP3 mailbox transaction with bounded multiline and dot-stuffing handling.
use super::ValidationError;
use super::wire::{PASSWORD, Wire, invalid};
use serde_json::{Value, json};
use socket2::Socket;
use std::net::SocketAddr;

pub(super) const FIRST_BODY: &[u8] =
    b"first\r\n. dot\r\n.\r\n-ERR authentication text only\r\n+OK not a status\r\n";
pub(super) const SECOND_BODY: &[u8] = b"second\r\n";

struct Reply {
    ok: bool,
    status: String,
    body: Vec<u8>,
}
impl Reply {
    fn value(&self) -> Value {
        json!({"ok":self.ok,"status":self.status,"body_bytes":self.body.len(),
            "body_hex":self.body.iter().map(|b|format!("{b:02x}")).collect::<String>()})
    }
}
fn parse(
    mut line: impl FnMut() -> Result<Vec<u8>, ValidationError>,
    multiline: bool,
) -> Result<Reply, ValidationError> {
    let bytes = line()?;
    if bytes.len() > 512 || !bytes.ends_with(b"\r\n") {
        return Err(invalid());
    }
    let status = std::str::from_utf8(&bytes[..bytes.len() - 2]).map_err(|_| invalid())?;
    let ok = status == "+OK" || status.starts_with("+OK ");
    if (!ok && status != "-ERR" && !status.starts_with("-ERR "))
        || !status.bytes().all(|b| (32..=126).contains(&b))
    {
        return Err(invalid());
    }
    let mut reply = Reply {
        ok,
        status: status.to_owned(),
        body: Vec::new(),
    };
    if !ok || !multiline {
        return Ok(reply);
    }
    for _ in 0..64 {
        let bytes = line()?;
        if bytes.len() > 1024 || !bytes.ends_with(b"\r\n") {
            return Err(invalid());
        }
        if bytes == b".\r\n" {
            return Ok(reply);
        }
        let unstuffed = bytes.strip_prefix(b".").unwrap_or(&bytes);
        if reply.body.len() + unstuffed.len() > 2048 {
            return Err(invalid());
        }
        reply.body.extend_from_slice(unstuffed);
    }
    Err(invalid())
}
fn read(wire: &mut Wire, multiline: bool) -> Result<Reply, ValidationError> {
    parse(
        || {
            let mut bytes = Vec::new();
            while bytes.len() < 1024 && !bytes.ends_with(b"\r\n") {
                bytes.extend(wire.read(1)?);
            }
            Ok(bytes)
        },
        multiline,
    )
}
fn expect(wire: &mut Wire, ok: bool, multiline: bool) -> Result<(), ValidationError> {
    let reply = read(wire, multiline)?;
    if reply.ok != ok {
        return Err(ValidationError::new(format!(
            "unexpected POP3 reply: {}",
            reply.status
        )));
    }
    Ok(())
}
fn command(
    wire: &mut Wire,
    request: &str,
    ok: bool,
    multiline: bool,
) -> Result<(), ValidationError> {
    wire.write(request.as_bytes())?;
    expect(wire, ok, multiline)
}
pub(super) fn probe(socket: Socket, target: SocketAddr) -> Result<(), ValidationError> {
    let mut wire = Wire::connect(socket, target)?;
    expect(&mut wire, true, false)?;
    command(&mut wire, "QUIT\r\n", true, false)?;
    wire.finish()?;
    Ok(())
}
pub(super) fn interact(wire: &mut Wire, control: bool) -> Result<(), ValidationError> {
    expect(wire, true, false)?;
    command(wire, "USER gewy\r\n", true, false)?;
    command(
        wire,
        &format!("PASS {}\r\n", if control { PASSWORD } else { "wrong" }),
        control,
        false,
    )?;
    if control {
        for (request, ok, multiline) in [
            ("STAT\r\n", true, false),
            ("LIST\r\n", true, true),
            ("UIDL\r\n", true, true),
            ("RETR 99\r\n", false, false),
            ("NOOP\r\n", true, false),
            ("RETR 1\r\n", true, true),
            ("RETR 2\r\n", true, true),
            ("DELE 1\r\n", true, false),
            ("STAT\r\n", true, false),
            ("RETR 1\r\n", false, false),
            ("RSET\r\n", true, false),
            ("STAT\r\n", true, false),
            ("LIST\r\n", true, true),
        ] {
            command(wire, request, ok, multiline)?;
        }
    } else {
        command(wire, "STAT\r\n", false, false)?;
    }
    command(wire, "QUIT\r\n", true, false)
}
fn decode(mut bytes: &[u8]) -> Result<Vec<Reply>, ValidationError> {
    let mut replies: Vec<Reply> = Vec::new();
    while !bytes.is_empty() && replies.len() < 32 {
        let control = replies.get(2).is_some_and(|reply| reply.ok);
        let multiline = control && matches!(replies.len(), 4 | 5 | 8 | 9 | 15);
        replies.push(parse(
            || {
                let end = bytes
                    .windows(2)
                    .position(|b| b == b"\r\n")
                    .ok_or_else(invalid)?
                    + 2;
                let line = bytes[..end].to_vec();
                bytes = &bytes[end..];
                Ok(line)
            },
            multiline,
        )?);
    }
    if !bytes.is_empty() || replies.is_empty() {
        return Err(invalid());
    }
    Ok(replies)
}
pub(super) fn describe(bytes: &[u8]) -> Result<Value, ValidationError> {
    let replies = decode(bytes)?;
    let retrievals: Vec<_> = replies
        .iter()
        .enumerate()
        .filter(|(i, _)| matches!(i, 8 | 9))
        .map(|(i, reply)| {
            json!({"reply_index":i,
            "declared_octets":retrieval_size(&reply.status),
            "unstuffed_octets":reply.body.len(),
            "size_conformant":retrieval_size(&reply.status)==Some(reply.body.len()),
            "known_mailpit_extra_crlf":retrieval_size(&reply.status)
                .is_some_and(|size| known_mailpit_size(size, &reply.body))})
        })
        .collect();
    Ok(
        json!({"replies":replies.iter().map(Reply::value).collect::<Vec<_>>(),
        "retrievals":retrievals,
        "fixture_size_policy":"pinned_Mailpit_1.33.0_extra_CRLF_reported_not_removed"}),
    )
}
fn retrieval_size(status: &str) -> Option<usize> {
    status
        .strip_prefix("+OK ")?
        .strip_suffix(" octets")?
        .parse()
        .ok()
}
fn known_mailpit_size(declared: usize, raw: &[u8]) -> bool {
    // Mailpit v1.33.0 sendData appends CRLF after the already CRLF-terminated raw message.
    // Keep those bytes in all client/capture evidence; this is not POP3 conformance.
    declared.checked_add(2) == Some(raw.len()) && raw.ends_with(b"\r\n\r\n")
}
pub(super) fn expected(control: bool) -> Value {
    if control {
        json!([
            true, true, true, true, true, true, false, true, true, true, true, true, false, true,
            true, true, true
        ])
    } else {
        json!([true, true, false, false, true])
    }
}
fn unhex(value: &Value) -> Option<Vec<u8>> {
    let value = value.as_str()?;
    if !value.len().is_multiple_of(2) || value.len() > 4096 || !value.is_ascii() {
        return None;
    }
    (0..value.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&value[i..i + 2], 16).ok())
        .collect()
}
fn numbers(value: &str) -> Option<Vec<usize>> {
    value
        .split_ascii_whitespace()
        .map(|s| s.parse::<usize>().ok())
        .collect()
}
pub(super) fn matches_expected(actual: &Value, control: bool) -> bool {
    let Some(replies) = actual["replies"].as_array() else {
        return false;
    };
    let statuses: Vec<_> = replies.iter().map(|r| r["ok"].clone()).collect();
    if json!(statuses) != expected(control) {
        return false;
    }
    if !control {
        return true;
    }
    let Some(body1) = unhex(&replies[8]["body_hex"]) else {
        return false;
    };
    let Some(body2) = unhex(&replies[9]["body_hex"]) else {
        return false;
    };
    let body = |raw: &[u8]| {
        raw.windows(4)
            .position(|b| b == b"\r\n\r\n")
            .map(|i| raw[i + 4..].to_vec())
    };
    let contents = [body(&body1), body(&body2)];
    if !contents.contains(&Some([FIRST_BODY, b"\r\n"].concat()))
        || !contents.contains(&Some([SECOND_BODY, b"\r\n"].concat()))
    {
        return false;
    }
    let stat = |index: usize| {
        replies[index]["status"]
            .as_str()
            .and_then(|s| s.strip_prefix("+OK "))
            .and_then(numbers)
    };
    let size = |index: usize| replies[index]["status"].as_str().and_then(retrieval_size);
    let (Some(size1), Some(size2)) = (size(8), size(9)) else {
        return false;
    };
    if !known_mailpit_size(size1, &body1) || !known_mailpit_size(size2, &body2) {
        return false;
    }
    let sizes = [size1, size2];
    if stat(3) != Some(vec![2, sizes.iter().sum()])
        || stat(11) != Some(vec![1, sizes[1]])
        || stat(14) != stat(3)
    {
        return false;
    }
    let Some(list) = unhex(&replies[4]["body_hex"]) else {
        return false;
    };
    let expected_list = format!("1 {}\r\n2 {}\r\n", sizes[0], sizes[1]);
    if list != expected_list.as_bytes() || replies[15]["body_hex"] != replies[4]["body_hex"] {
        return false;
    }
    let Some(uidl) = unhex(&replies[5]["body_hex"]) else {
        return false;
    };
    let Ok(uidl) = std::str::from_utf8(&uidl) else {
        return false;
    };
    let entries: Vec<_> = uidl
        .lines()
        .map(|l| l.split_ascii_whitespace().collect::<Vec<_>>())
        .collect();
    entries.len() == 2
        && entries[0].len() == 2
        && entries[1].len() == 2
        && entries[0][0] == "1"
        && entries[1][0] == "2"
        && entries[0][1] != entries[1][1]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pinned_server_size_quirk_is_explicit_and_strictly_bounded() {
        let raw = b"body\r\n\r\n";
        assert!(known_mailpit_size(6, raw));
        for size in [0, 5, 7, 8, usize::MAX] {
            assert!(!known_mailpit_size(size, raw));
        }
        assert!(!known_mailpit_size(6, b"body!!!!"));
        assert_eq!(retrieval_size("+OK 355 octets"), Some(355));
        assert_eq!(retrieval_size("+OK 2 messages (355 octets)"), None);
    }
    fn fixture() -> Value {
        let mut replies: Vec<_> = expected(true)
            .as_array()
            .unwrap()
            .iter()
            .map(|ok| {
                Reply {
                    ok: ok.as_bool().unwrap(),
                    status: String::new(),
                    body: vec![],
                }
                .value()
            })
            .collect();
        let mut sizes = Vec::new();
        for (index, body) in [(8, FIRST_BODY), (9, SECOND_BODY)] {
            let raw = [b"Subject: test\r\n\r\n".as_slice(), body, b"\r\n"].concat();
            sizes.push(raw.len() - 2);
            replies[index] = Reply {
                ok: true,
                status: format!("+OK {} octets", raw.len() - 2),
                body: raw,
            }
            .value();
        }
        replies[3]["status"] = json!(format!("+OK 2 {}", sizes.iter().sum::<usize>()));
        replies[11]["status"] = json!(format!("+OK 1 {}", sizes[1]));
        replies[14] = replies[3].clone();
        replies[4] = Reply {
            ok: true,
            status: String::new(),
            body: format!("1 {}\r\n2 {}\r\n", sizes[0], sizes[1]).into_bytes(),
        }
        .value();
        replies[15] = replies[4].clone();
        replies[5] = Reply {
            ok: true,
            status: String::new(),
            body: b"1 unique-a\r\n2 unique-b\r\n".to_vec(),
        }
        .value();
        json!({"replies":replies})
    }
    #[test]
    fn mailbox_oracle_requires_body_size_list_reset_and_unique_ids() {
        let valid = fixture();
        assert!(matches_expected(&valid, true));
        for (index, field, value) in [
            (8, "body_hex", json!("626f64790d0a0d0a")),
            (8, "status", json!("+OK 999 octets")),
            (11, "status", json!("+OK 2 999")),
            (14, "status", json!("+OK 1 999")),
            (15, "body_hex", json!("")),
            (5, "body_hex", json!("3120610d0a3220610d0a")),
            (12, "ok", json!(true)),
        ] {
            let mut invalid = valid.clone();
            invalid["replies"][index][field] = value;
            assert!(!matches_expected(&invalid, true), "{index} {field}");
        }
    }
    fn multiline(bytes: &[u8]) -> Result<Reply, ValidationError> {
        let mut bytes = bytes;
        parse(
            || {
                let end = bytes
                    .windows(2)
                    .position(|b| b == b"\r\n")
                    .ok_or_else(invalid)?
                    + 2;
                let line = bytes[..end].to_vec();
                bytes = &bytes[end..];
                Ok(line)
            },
            true,
        )
    }
    #[test]
    fn body_status_lookalikes_and_dot_lines_do_not_become_replies() {
        let parsed =
            multiline(b"+OK follows\r\n..\r\n..leading\r\n-ERR auth\r\n+OK status\r\n.\r\n")
                .unwrap();
        assert_eq!(parsed.body, b".\r\n.leading\r\n-ERR auth\r\n+OK status\r\n");
        assert!(parsed.ok);
        let denied = multiline(b"-ERR no message\r\n").unwrap();
        assert!(!denied.ok);
        assert!(denied.body.is_empty());
    }
    #[test]
    fn multiline_is_bounded_and_requires_terminator_and_real_status_token() {
        for invalid in [
            b"+OK follows\r\nbody\r\n".as_slice(),
            b"+OKX invalid\r\n",
            b"-ERRX invalid\r\n",
            b"+OK\n",
        ] {
            assert!(multiline(invalid).is_err());
        }
        assert!(multiline(format!("+OK {}\r\n.\r\n", "x".repeat(510)).as_bytes()).is_err());
        assert!(
            multiline(format!("+OK follows\r\n{}\r\n.\r\n", "x".repeat(1024)).as_bytes()).is_err()
        );
        assert!(
            multiline(format!("+OK follows\r\n{}.\r\n", "x\r\n".repeat(65)).as_bytes()).is_err()
        );
    }
}
