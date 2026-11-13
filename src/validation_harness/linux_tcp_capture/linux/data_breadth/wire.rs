//! Small RESP2 and PostgreSQL v3 fixtures, not general database drivers.
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use socket2::Socket;

use super::{Protocol, ValidationError};

pub(super) const PASSWORD: &str = "gewyvern-data-lab-only";
const LIMIT: usize = 4096;

fn invalid() -> ValidationError {
    ValidationError::new("invalid or unsupported bounded database/cache frame")
}

struct Wire {
    stream: TcpStream,
    deadline: Instant,
    response: Vec<u8>,
}
impl Wire {
    fn remaining(&self) -> Result<Duration, ValidationError> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| ValidationError::new("data fixture exchange deadline exceeded"))
    }
    fn read(&mut self, size: usize) -> Result<Vec<u8>, ValidationError> {
        if size > LIMIT.saturating_sub(self.response.len()) {
            return Err(invalid());
        }
        let mut bytes = vec![0; size];
        let mut offset = 0;
        while offset < size {
            self.stream.set_read_timeout(Some(self.remaining()?))?;
            let count = self.stream.read(&mut bytes[offset..])?;
            if count == 0 {
                return Err(invalid());
            }
            offset += count;
        }
        self.response.extend_from_slice(&bytes);
        Ok(bytes)
    }
    fn write(&mut self, bytes: &[u8]) -> Result<(), ValidationError> {
        let mut sent = 0;
        while sent < bytes.len() {
            self.stream.set_write_timeout(Some(self.remaining()?))?;
            let count = self.stream.write(&bytes[sent..])?;
            if count == 0 {
                return Err(invalid());
            }
            sent += count;
        }
        Ok(())
    }
    fn finish(mut self) -> Result<Vec<u8>, ValidationError> {
        self.stream.set_read_timeout(Some(self.remaining()?))?;
        if self.stream.read(&mut [0])? != 0 {
            return Err(invalid());
        }
        Ok(self.response)
    }
    fn resp(&mut self, args: &[&str]) -> Result<Resp, ValidationError> {
        let mut request = format!("*{}\r\n", args.len());
        for arg in args {
            request.push_str(&format!("${}\r\n{arg}\r\n", arg.len()));
        }
        self.write(request.as_bytes())?;
        let mut bytes = Vec::new();
        while !bytes.ends_with(b"\r\n") && bytes.len() < 512 {
            bytes.extend(self.read(1)?);
        }
        if !bytes.ends_with(b"\r\n") {
            return Err(invalid());
        }
        if bytes[0] == b'$' {
            let size = number(&bytes[1..bytes.len() - 2])?;
            if size >= 0 {
                if size > 1024 {
                    return Err(invalid());
                }
                bytes.extend(self.read(size as usize + 2)?);
            }
        }
        let mut cursor = 0;
        let response = parse_resp(&bytes, &mut cursor)?;
        if cursor != bytes.len() {
            return Err(invalid());
        }
        Ok(response)
    }
    fn pg_send(&mut self, tag: u8, payload: &[u8]) -> Result<(), ValidationError> {
        let mut bytes = vec![tag];
        bytes.extend_from_slice(&((payload.len() + 4) as u32).to_be_bytes());
        bytes.extend_from_slice(payload);
        self.write(&bytes)
    }
    fn pg_receive(&mut self) -> Result<(u8, Vec<u8>), ValidationError> {
        let header = self.read(5)?;
        let size = u32::from_be_bytes(header[1..5].try_into().map_err(|_| invalid())?) as usize;
        if !(4..=2048).contains(&size) {
            return Err(invalid());
        }
        Ok((header[0], self.read(size - 4)?))
    }
    fn pg_ready(&mut self) -> Result<(), ValidationError> {
        for _ in 0..32 {
            let (tag, body) = self.pg_receive()?;
            if tag == b'Z' {
                return if body == b"I" { Ok(()) } else { Err(invalid()) };
            }
        }
        Err(invalid())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Resp {
    Simple(String),
    Error(String),
    Bulk(String),
    Nil,
    Integer(i64),
}
fn number(bytes: &[u8]) -> Result<i64, ValidationError> {
    let value = std::str::from_utf8(bytes).map_err(|_| invalid())?;
    let number = value.parse::<i64>().map_err(|_| invalid())?;
    if number.to_string() != value {
        return Err(invalid());
    }
    Ok(number)
}
fn parse_resp(bytes: &[u8], cursor: &mut usize) -> Result<Resp, ValidationError> {
    let rest = bytes.get(*cursor..).ok_or_else(invalid)?;
    let end = rest
        .windows(2)
        .position(|b| b == b"\r\n")
        .ok_or_else(invalid)?;
    if end == 0 || end > 512 {
        return Err(invalid());
    }
    let line = &rest[1..end];
    *cursor += end + 2;
    let text = || {
        std::str::from_utf8(line)
            .map(str::to_owned)
            .map_err(|_| invalid())
    };
    match rest[0] {
        b'+' => Ok(Resp::Simple(text()?)),
        b'-' => Ok(Resp::Error(text()?)),
        b':' => Ok(Resp::Integer(number(line)?)),
        b'$' => {
            let size = number(line)?;
            if size == -1 {
                return Ok(Resp::Nil);
            }
            if !(0..=1024).contains(&size) {
                return Err(invalid());
            }
            let end = *cursor + size as usize;
            if bytes.get(end..end + 2) != Some(b"\r\n") {
                return Err(invalid());
            }
            let value = std::str::from_utf8(bytes.get(*cursor..end).ok_or_else(invalid)?)
                .map_err(|_| invalid())?
                .to_owned();
            *cursor = end + 2;
            Ok(Resp::Bulk(value))
        }
        _ => Err(invalid()),
    }
}

fn cstring<'a>(bytes: &mut &'a [u8]) -> Result<&'a [u8], ValidationError> {
    let size = bytes.iter().position(|b| *b == 0).ok_or_else(invalid)?;
    let value = &bytes[..size];
    *bytes = &bytes[size + 1..];
    Ok(value)
}
fn error_fields(mut body: &[u8]) -> Result<BTreeMap<u8, String>, ValidationError> {
    let mut fields = BTreeMap::new();
    for _ in 0..32 {
        if body == [0] {
            return if fields.contains_key(&b'C') {
                Ok(fields)
            } else {
                Err(invalid())
            };
        }
        let key = *body.first().ok_or_else(invalid)?;
        if !key.is_ascii_alphabetic() {
            return Err(invalid());
        }
        body = &body[1..];
        let value = std::str::from_utf8(cstring(&mut body)?).map_err(|_| invalid())?;
        if fields.insert(key, value.into()).is_some() {
            return Err(invalid());
        }
    }
    Err(invalid())
}

pub(super) fn describe(protocol: Protocol, bytes: &[u8]) -> Result<Value, ValidationError> {
    if bytes.is_empty() || bytes.len() > LIMIT {
        return Err(invalid());
    }
    if matches!(protocol, Protocol::Redis) {
        let mut cursor = 0;
        let mut replies = Vec::new();
        while cursor < bytes.len() && replies.len() < 32 {
            replies.push(match parse_resp(bytes, &mut cursor)? {
                Resp::Simple(value) => json!({"kind":"simple","value":value}),
                Resp::Bulk(value) => json!({"kind":"bulk","value":value}),
                Resp::Error(value) => {
                    json!({"kind":"error","code":value.split(' ').next().ok_or_else(invalid)?})
                }
                Resp::Nil => json!({"kind":"nil"}),
                Resp::Integer(value) => json!({"kind":"integer","value":value}),
            });
        }
        if cursor != bytes.len() {
            return Err(invalid());
        }
        return Ok(json!({"replies":replies}));
    }
    let mut cursor = 0;
    let mut auth = Vec::new();
    let mut errors = Vec::new();
    let mut rows = Vec::new();
    let mut ready = 0;
    let mut complete = 0;
    let mut frames = 0;
    while cursor < bytes.len() && frames < 64 {
        let header = bytes.get(cursor..cursor + 5).ok_or_else(invalid)?;
        let size = u32::from_be_bytes(header[1..5].try_into().map_err(|_| invalid())?) as usize;
        if !(4..=2048).contains(&size) {
            return Err(invalid());
        }
        let mut body = bytes
            .get(cursor + 5..cursor + 1 + size)
            .ok_or_else(invalid)?;
        match header[0] {
            b'R' if body.len() == 4 => {
                auth.push(u32::from_be_bytes(body.try_into().map_err(|_| invalid())?))
            }
            b'Z' if body == b"I" => ready += 1,
            b'E' => {
                let fields = error_fields(body)?;
                let code = fields.get(&b'C').ok_or_else(invalid)?;
                if code.len() != 5
                    || !code
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
                {
                    return Err(invalid());
                }
                errors.push(code.clone());
            }
            b'S' => {
                cstring(&mut body)?;
                cstring(&mut body)?;
                if !body.is_empty() {
                    return Err(invalid());
                }
            }
            b'K' if body.len() == 8 => (),
            b'T' => {
                if body.get(..2) != Some(&[0, 1]) {
                    return Err(invalid());
                }
                body = &body[2..];
                cstring(&mut body)?;
                if body.len() != 18 || body[16..] != [0, 0] {
                    return Err(invalid());
                }
            }
            b'D' => {
                if body.len() < 6 || body[..2] != [0, 1] {
                    return Err(invalid());
                }
                let length =
                    u32::from_be_bytes(body[2..6].try_into().map_err(|_| invalid())?) as usize;
                if body.len() != 6 + length {
                    return Err(invalid());
                }
                rows.push(
                    std::str::from_utf8(&body[6..])
                        .map_err(|_| invalid())?
                        .to_owned(),
                );
            }
            b'C' if body == b"SELECT 1\0" => complete += 1,
            _ => return Err(invalid()),
        }
        frames += 1;
        cursor += size + 1;
    }
    if cursor != bytes.len() {
        return Err(invalid());
    }
    Ok(
        json!({"auth":auth,"sqlstates":errors,"rows":rows,"ready":ready,"completed_queries":complete}),
    )
}

pub(super) fn expected(protocol: Protocol, control: bool) -> Value {
    match (protocol, control) {
        (Protocol::Postgres, false) => {
            json!({"auth":[3],"sqlstates":["28P01"],"rows":[],"ready":0,"completed_queries":0})
        }
        (Protocol::Postgres, true) => {
            json!({"auth":[3,0],"sqlstates":["22012"],"rows":["42","43"],"ready":4,"completed_queries":2})
        }
        (Protocol::Redis, false) => {
            json!({"replies":[{"kind":"error","code":"NOAUTH"},{"kind":"error","code":"WRONGPASS"},{"kind":"simple","value":"OK"},{"kind":"simple","value":"OK"},{"kind":"bulk","value":"42"},{"kind":"error","code":"WRONGTYPE"},{"kind":"error","code":"ERR"},{"kind":"bulk","value":"42"},{"kind":"simple","value":"OK"}]})
        }
        (Protocol::Redis, true) => {
            json!({"replies":[{"kind":"simple","value":"OK"},{"kind":"nil"},{"kind":"simple","value":"OK"},{"kind":"bulk","value":"-ERR not-an-error"},{"kind":"simple","value":"OK"}]})
        }
    }
}

pub(super) fn exchange(
    socket: Socket,
    target: SocketAddr,
    protocol: Protocol,
    control: bool,
) -> Result<(Vec<u8>, Value), ValidationError> {
    socket.connect_timeout(&target.into(), Duration::from_secs(2))?;
    let mut wire = Wire {
        stream: socket.into(),
        deadline: Instant::now() + Duration::from_secs(12),
        response: Vec::new(),
    };
    match protocol {
        Protocol::Redis => {
            let commands: Vec<Vec<&str>> = if control {
                vec![
                    vec!["AUTH", PASSWORD],
                    vec!["GET", "missing"],
                    vec!["SET", "lookalike", "-ERR not-an-error"],
                    vec!["GET", "lookalike"],
                    vec!["QUIT"],
                ]
            } else {
                vec![
                    vec!["PING"],
                    vec!["AUTH", "invalid-lab-password"],
                    vec!["AUTH", PASSWORD],
                    vec!["SET", "lab", "42"],
                    vec!["GET", "lab"],
                    vec!["LPUSH", "lab", "x"],
                    vec!["GEWY_BAD"],
                    vec!["GET", "lab"],
                    vec!["QUIT"],
                ]
            };
            for command in commands {
                wire.resp(&command)?;
            }
        }
        Protocol::Postgres => {
            let mut startup = 196608u32.to_be_bytes().to_vec();
            startup.extend_from_slice(b"user\0gewy\0database\0gewy\0\0");
            let mut bytes = ((startup.len() + 4) as u32).to_be_bytes().to_vec();
            bytes.extend(startup);
            wire.write(&bytes)?;
            if wire.pg_receive()? != (b'R', 3u32.to_be_bytes().to_vec()) {
                return Err(invalid());
            }
            wire.pg_send(
                b'p',
                format!(
                    "{}\0",
                    if control {
                        PASSWORD
                    } else {
                        "invalid-lab-password"
                    }
                )
                .as_bytes(),
            )?;
            if control {
                wire.pg_ready()?;
                for sql in [
                    b"SELECT 42::text\0".as_slice(),
                    b"SELECT 1/0\0",
                    b"SELECT 43::text\0",
                ] {
                    wire.pg_send(b'Q', sql)?;
                    wire.pg_ready()?;
                }
                wire.pg_send(b'X', &[])?;
            } else if wire.pg_receive()?.0 != b'E' {
                return Err(invalid());
            }
        }
    }
    let response = wire.finish()?;
    let result = describe(protocol, &response)?;
    if result != expected(protocol, control) {
        return Err(ValidationError::new(format!(
            "unexpected real data target result: {result}"
        )));
    }
    Ok((response, result))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resp_bulk_text_is_not_an_error_and_boundaries_are_strict() {
        assert_eq!(
            describe(Protocol::Redis, b"$17\r\n-ERR not-an-error\r\n$-1\r\n").unwrap(),
            json!({"replies":[{"kind":"bulk","value":"-ERR not-an-error"},{"kind":"nil"}]})
        );
        for bytes in [
            b"$16\r\n-ERR".as_slice(),
            b"$-2\r\n",
            b"$1025\r\n",
            b"+OK\r\n?",
            b"$01\r\nx\r\n",
        ] {
            assert!(describe(Protocol::Redis, bytes).is_err());
        }
    }
    #[test]
    fn postgres_error_sqlstate_is_framed_not_a_text_search() {
        let body = b"SERROR\0C22012\0M28P01 is text\0\0";
        let mut frame = vec![b'E'];
        frame.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
        frame.extend_from_slice(body);
        assert_eq!(
            describe(Protocol::Postgres, &frame).unwrap()["sqlstates"],
            json!(["22012"])
        );
        for length in 0..frame.len() {
            assert!(describe(Protocol::Postgres, &frame[..length]).is_err());
        }
        frame.push(0);
        assert!(describe(Protocol::Postgres, &frame).is_err());
        assert!(error_fields(b"C22012\0C28P01\0\0").is_err());
    }
}
