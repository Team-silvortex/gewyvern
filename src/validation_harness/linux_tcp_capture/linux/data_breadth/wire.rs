//! Small RESP2 and PostgreSQL v3 fixtures, not general database drivers.
use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use serde_json::{Value, json};
use socket2::Socket;

use super::{Protocol, ValidationError};

pub(super) const PASSWORD: &str = "gewyvern-data-lab-only";
pub(super) const READ_CHUNK: usize = 7;
const LIMIT: usize = 4096;
const BINARY_VALUE: &[u8] = b"\0\xff\r\n-ERR";

pub(super) fn invalid() -> ValidationError {
    ValidationError::new("invalid or unsupported bounded database/cache frame")
}

pub(super) struct Wire {
    stream: TcpStream,
    deadline: Instant,
    response: Vec<u8>,
}
impl Wire {
    pub(super) fn connect(socket: Socket, target: SocketAddr) -> Result<Self, ValidationError> {
        socket.connect_timeout(&target.into(), Duration::from_secs(2))?;
        Ok(Self {
            stream: socket.into(),
            deadline: Instant::now() + Duration::from_secs(12),
            response: Vec::new(),
        })
    }
    pub(super) fn close_write(&self) -> Result<(), ValidationError> {
        self.stream.shutdown(std::net::Shutdown::Write)?;
        Ok(())
    }
    fn remaining(&self) -> Result<Duration, ValidationError> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|d| !d.is_zero())
            .ok_or_else(|| ValidationError::new("data fixture exchange deadline exceeded"))
    }
    pub(super) fn read(&mut self, size: usize) -> Result<Vec<u8>, ValidationError> {
        if size > LIMIT.saturating_sub(self.response.len()) {
            return Err(invalid());
        }
        let mut bytes = vec![0; size];
        let mut offset = 0;
        while offset < size {
            self.stream.set_read_timeout(Some(self.remaining()?))?;
            // Exercise partial client reads without claiming these are TCP packet boundaries.
            let end = size.min(offset + READ_CHUNK);
            let count = self.stream.read(&mut bytes[offset..end])?;
            if count == 0 {
                return Err(invalid());
            }
            offset += count;
        }
        self.response.extend_from_slice(&bytes);
        Ok(bytes)
    }
    pub(super) fn write(&mut self, bytes: &[u8]) -> Result<(), ValidationError> {
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
    pub(super) fn finish(mut self) -> Result<Vec<u8>, ValidationError> {
        self.stream.set_read_timeout(Some(self.remaining()?))?;
        if self.stream.read(&mut [0])? != 0 {
            return Err(invalid());
        }
        Ok(self.response)
    }
    fn resp(&mut self, args: &[&str]) -> Result<Resp, ValidationError> {
        let args: Vec<_> = args.iter().map(|arg| arg.as_bytes()).collect();
        self.write(&redis_request(&args))?;
        self.resp_receive()
    }
    fn resp_receive(&mut self) -> Result<Resp, ValidationError> {
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
    fn pg_ready(&mut self, expected_state: u8) -> Result<(), ValidationError> {
        for _ in 0..32 {
            let (tag, body) = self.pg_receive()?;
            if tag == b'Z' {
                return if body == [expected_state] {
                    Ok(())
                } else {
                    Err(invalid())
                };
            }
        }
        Err(invalid())
    }
}

fn redis_request(args: &[&[u8]]) -> Vec<u8> {
    let mut request = format!("*{}\r\n", args.len()).into_bytes();
    for arg in args {
        request.extend_from_slice(format!("${}\r\n", arg.len()).as_bytes());
        request.extend_from_slice(arg);
        request.extend_from_slice(b"\r\n");
    }
    request
}

#[derive(Debug, PartialEq, Eq)]
enum Resp {
    Simple(String),
    Error(String),
    Bulk(Vec<u8>),
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
        if line.iter().any(|b| matches!(b, b'\r' | b'\n')) {
            return Err(invalid());
        }
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
            let value = bytes.get(*cursor..end).ok_or_else(invalid)?.to_vec();
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
    match protocol {
        Protocol::Mysql => return super::mysql::describe(bytes),
        Protocol::Memcached => return super::memcached::describe(bytes),
        Protocol::Mqtt => return super::mqtt::describe(bytes),
        Protocol::Nats => return super::nats::describe(bytes),
        _ => (),
    }
    if matches!(protocol, Protocol::Redis) {
        let mut cursor = 0;
        let mut replies = Vec::new();
        while cursor < bytes.len() && replies.len() < 32 {
            replies.push(match parse_resp(bytes, &mut cursor)? {
                Resp::Simple(value) => json!({"kind":"simple","value":value}),
                Resp::Bulk(value) => match std::str::from_utf8(&value) {
                    Ok(text) => json!({"kind":"bulk","value":text}),
                    Err(_) => {
                        let hex: String = value.iter().map(|byte| format!("{byte:02x}")).collect();
                        json!({"kind":"bulk","value_hex":hex,"bytes":value.len()})
                    }
                },
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
    let mut ready_states = Vec::new();
    let mut command_tags = Vec::new();
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
            b'Z' if matches!(body, b"I" | b"T" | b"E") => {
                ready_states.push(char::from(body[0]));
            }
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
                let length = i32::from_be_bytes(body[2..6].try_into().map_err(|_| invalid())?);
                if length == -1 && body.len() == 6 {
                    rows.push(None);
                } else if length >= 0 && body.len() - 6 == length as usize {
                    rows.push(Some(
                        std::str::from_utf8(&body[6..])
                            .map_err(|_| invalid())?
                            .to_owned(),
                    ));
                } else {
                    return Err(invalid());
                }
            }
            b'C' if matches!(body, b"SELECT 1\0" | b"BEGIN\0" | b"ROLLBACK\0") => {
                command_tags.push(
                    std::str::from_utf8(&body[..body.len() - 1])
                        .map_err(|_| invalid())?
                        .to_owned(),
                );
            }
            _ => return Err(invalid()),
        }
        frames += 1;
        cursor += size + 1;
    }
    if cursor != bytes.len() {
        return Err(invalid());
    }
    Ok(
        json!({"auth":auth,"sqlstates":errors,"rows":rows,"ready":ready_states.len(),
            "ready_states":ready_states,"completed_queries":command_tags.len(),"command_tags":command_tags}),
    )
}

pub(super) fn expected(protocol: Protocol, control: bool) -> Value {
    match (protocol, control) {
        (Protocol::Mysql, control) => super::mysql::expected(control),
        (Protocol::Memcached, control) => super::memcached::expected(control),
        (Protocol::Mqtt, control) => super::mqtt::expected(control),
        (Protocol::Nats, control) => super::nats::expected(control),
        (Protocol::Postgres, false) => {
            json!({"auth":[3],"sqlstates":["28P01"],"rows":[],"ready":0,"ready_states":[],"completed_queries":0,"command_tags":[]})
        }
        (Protocol::Postgres, true) => {
            json!({"auth":[3,0],"sqlstates":["22012","22012","25P02"],"rows":["42","43",null,"","45"],
                "ready":11,"ready_states":["I","I","I","I","I","I","T","E","E","I","I"],
                "completed_queries":7,"command_tags":["SELECT 1","SELECT 1","SELECT 1","SELECT 1","BEGIN","ROLLBACK","SELECT 1"]})
        }
        (Protocol::Redis, false) => {
            json!({"replies":[{"kind":"error","code":"NOAUTH"},{"kind":"error","code":"WRONGPASS"},{"kind":"simple","value":"OK"},{"kind":"simple","value":"OK"},{"kind":"bulk","value":"42"},{"kind":"error","code":"WRONGTYPE"},{"kind":"error","code":"ERR"},{"kind":"bulk","value":"42"},{"kind":"simple","value":"OK"}]})
        }
        (Protocol::Redis, true) => {
            json!({"replies":[{"kind":"simple","value":"OK"},{"kind":"nil"},
                {"kind":"simple","value":"OK"},{"kind":"bulk","value":"-ERR not-an-error"},
                {"kind":"simple","value":"OK"},{"kind":"bulk","value":""},
                {"kind":"simple","value":"OK"},{"kind":"bulk","value_hex":"00ff0d0a2d455252","bytes":8},
                {"kind":"bulk","value":"-ERR not-an-error"},{"kind":"nil"},{"kind":"bulk","value":""},
                {"kind":"simple","value":"OK"}]})
        }
    }
}

pub(super) fn exchange(
    socket: Socket,
    target: SocketAddr,
    protocol: Protocol,
    control: bool,
) -> Result<(Vec<u8>, Value), ValidationError> {
    let mut wire = Wire::connect(socket, target)?;
    match protocol {
        Protocol::Mysql => super::mysql::interact(&mut wire, control)?,
        Protocol::Memcached => super::memcached::interact(&mut wire, control)?,
        Protocol::Mqtt => super::mqtt::interact(&mut wire, control)?,
        Protocol::Nats => super::nats::interact(&mut wire, control)?,
        Protocol::Redis => {
            let commands: Vec<Vec<&str>> = if control {
                vec![
                    vec!["AUTH", PASSWORD],
                    vec!["GET", "missing"],
                    vec!["SET", "lookalike", "-ERR not-an-error"],
                    vec!["GET", "lookalike"],
                    vec!["SET", "empty", ""],
                    vec!["GET", "empty"],
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
            if control {
                wire.write(&redis_request(&[b"SET", b"binary", BINARY_VALUE]))?;
                wire.resp_receive()?;
                wire.resp(&["GET", "binary"])?;
                let mut pipeline = Vec::new();
                for key in ["lookalike", "missing", "empty"] {
                    pipeline.extend(redis_request(&[b"GET", key.as_bytes()]));
                }
                wire.write(&pipeline)?;
                for _ in 0..3 {
                    wire.resp_receive()?;
                }
                wire.resp(&["QUIT"])?;
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
                wire.pg_ready(b'I')?;
                for (sql, state) in [
                    ("SELECT 42::text\0", b'I'),
                    ("SELECT 1/0\0", b'I'),
                    ("SELECT 43::text\0", b'I'),
                    ("SELECT NULL::text\0", b'I'),
                    ("SELECT ''::text\0", b'I'),
                    ("BEGIN\0", b'T'),
                    ("SELECT 1/0\0", b'E'),
                    ("SELECT 44::text\0", b'E'),
                    ("ROLLBACK\0", b'I'),
                    ("SELECT 45::text\0", b'I'),
                ] {
                    wire.pg_send(b'Q', sql.as_bytes())?;
                    wire.pg_ready(state)?;
                }
                wire.pg_send(b'X', &[])?;
            } else if wire.pg_receive()?.0 != b'E' {
                return Err(invalid());
            }
        }
    }
    let response = wire.finish()?;
    let result = describe(protocol, &response)?;
    let matches = if matches!(protocol, Protocol::Nats) {
        super::nats::matches_expected(&result, control)
    } else {
        result == expected(protocol, control)
    };
    if !matches {
        return Err(ValidationError::new(format!(
            "unexpected real data target result: {result}"
        )));
    }
    Ok((response, result))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pg_frame(tag: u8, body: &[u8]) -> Vec<u8> {
        let mut bytes = vec![tag];
        bytes.extend_from_slice(&((body.len() + 4) as u32).to_be_bytes());
        bytes.extend_from_slice(body);
        bytes
    }

    #[test]
    fn postgres_null_empty_and_transaction_states_remain_distinct() {
        let mut bytes = pg_frame(b'D', &[0, 1, 255, 255, 255, 255]);
        bytes.extend(pg_frame(b'D', &[0, 1, 0, 0, 0, 0]));
        for state in *b"TEI" {
            bytes.extend(pg_frame(b'Z', &[state]));
        }
        bytes.extend(pg_frame(b'C', b"ROLLBACK\0"));
        let result = describe(Protocol::Postgres, &bytes).unwrap();
        assert_eq!(result["rows"], json!([null, ""]));
        assert_eq!(result["ready_states"], json!(["T", "E", "I"]));
        assert_eq!(result["command_tags"], json!(["ROLLBACK"]));
        for body in [
            &[0, 1, 255, 255, 255, 254][..],
            &[0, 1, 255, 255, 255, 255, 0],
            &[0, 1, 0, 0, 0, 1],
            &[0, 1, 0, 0, 0, 0, 0],
            &[0, 1, 127, 255, 255, 255],
        ] {
            assert!(describe(Protocol::Postgres, &pg_frame(b'D', body)).is_err());
        }
        for state in [b"X".as_slice(), b"", b"II"] {
            assert!(describe(Protocol::Postgres, &pg_frame(b'Z', state)).is_err());
        }
    }

    #[test]
    fn resp_empty_nil_binary_and_embedded_delimiters_remain_distinct() {
        let mut bytes = b"$0\r\n\r\n$-1\r\n$8\r\n".to_vec();
        bytes.extend_from_slice(BINARY_VALUE);
        bytes.extend_from_slice(b"\r\n+OK\r\n");
        assert_eq!(
            describe(Protocol::Redis, &bytes).unwrap(),
            json!({"replies":[
                {"kind":"bulk","value":""},{"kind":"nil"},
                {"kind":"bulk","value_hex":"00ff0d0a2d455252","bytes":8},
                {"kind":"simple","value":"OK"}
            ]})
        );
        assert_eq!(
            redis_request(&[b"SET", b"binary", BINARY_VALUE]),
            b"*3\r\n$3\r\nSET\r\n$6\r\nbinary\r\n$8\r\n\0\xff\r\n-ERR\r\n"
        );
        for invalid in [
            b"+O\rK\r\n".as_slice(),
            b"-ERR\ninvalid\r\n",
            b"$8\r\nshort\r\n",
        ] {
            assert!(describe(Protocol::Redis, invalid).is_err());
        }
    }

    #[test]
    fn client_reads_fragmented_and_pipelined_bulk_frames_without_text_splitting() {
        use std::net::TcpListener;
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let stream = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (mut peer, _) = listener.accept().unwrap();
        peer.set_write_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        peer.set_nodelay(true).unwrap();
        let server = std::thread::spawn(move || {
            for byte in b"$8\r\n\0\xff\r\n-ERR\r\n$0\r\n\r\n$-1\r\n" {
                peer.write_all(&[*byte]).unwrap();
            }
        });
        let mut wire = Wire {
            stream,
            deadline: Instant::now() + Duration::from_secs(2),
            response: Vec::new(),
        };
        assert_eq!(
            wire.resp_receive().unwrap(),
            Resp::Bulk(BINARY_VALUE.to_vec())
        );
        assert_eq!(wire.resp_receive().unwrap(), Resp::Bulk(Vec::new()));
        assert_eq!(wire.resp_receive().unwrap(), Resp::Nil);
        assert_eq!(
            describe(Protocol::Redis, &wire.finish().unwrap()).unwrap()["replies"]
                .as_array()
                .unwrap()
                .len(),
            3
        );
        server.join().unwrap();
    }
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
