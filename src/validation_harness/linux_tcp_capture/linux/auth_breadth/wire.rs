//! Deliberately bounded FTP control and LDAP simple-bind clients, not general clients.
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::{Duration, Instant};

use socket2::Socket;

use super::{Protocol, ValidationError};

pub(super) const PASSWORD: &str = "gewyvern-lab-only";
const DN: &str = "cn=admin,dc=example,dc=org";
const LIMIT: usize = 4096;

struct Wire {
    stream: TcpStream,
    deadline: Instant,
    response: Vec<u8>,
}

impl Wire {
    fn remaining(&self) -> Result<Duration, ValidationError> {
        self.deadline
            .checked_duration_since(Instant::now())
            .filter(|duration| !duration.is_zero())
            .ok_or_else(|| ValidationError::new("authentication fixture deadline exceeded"))
    }
    fn read(&mut self, length: usize) -> Result<Vec<u8>, ValidationError> {
        if length > LIMIT.saturating_sub(self.response.len()) {
            return Err(ValidationError::new(
                "authentication response exceeds 4 KiB",
            ));
        }
        let mut bytes = vec![0; length];
        let mut received = 0;
        while received < length {
            self.stream.set_read_timeout(Some(self.remaining()?))?;
            let count = self.stream.read(&mut bytes[received..])?;
            if count == 0 {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "incomplete authentication reply",
                )
                .into());
            }
            received += count;
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
                return Err(std::io::Error::new(
                    std::io::ErrorKind::WriteZero,
                    "incomplete authentication request",
                )
                .into());
            }
            sent += count;
        }
        Ok(())
    }
    fn ftp_reply(&mut self) -> Result<u16, ValidationError> {
        let mut line = Vec::new();
        while line.len() < 512 {
            line.extend(self.read(1)?);
            if line.ends_with(b"\r\n") {
                return ftp_code(&line);
            }
        }
        Err(ValidationError::new(
            "FTP fixture requires a bounded single-line reply",
        ))
    }
    fn ldap_reply(&mut self) -> Result<u16, ValidationError> {
        let mut frame = self.read(2)?;
        let length = if frame[1] < 128 {
            usize::from(frame[1])
        } else {
            let count = usize::from(frame[1] & 127);
            if !(1..=2).contains(&count) {
                return Err(ValidationError::new("unsupported LDAP BER length"));
            }
            let bytes = self.read(count)?;
            frame.extend_from_slice(&bytes);
            bytes
                .into_iter()
                .fold(0usize, |n, b| (n << 8) | usize::from(b))
        };
        frame.extend(self.read(length)?);
        ldap_code(&frame)
    }
    fn finish(mut self) -> Result<Vec<u8>, ValidationError> {
        self.stream.set_read_timeout(Some(self.remaining()?))?;
        let mut extra = [0];
        if self.stream.read(&mut extra)? != 0 {
            return Err(ValidationError::new(
                "unexpected bytes after authentication fixture close",
            ));
        }
        Ok(self.response)
    }
}

fn ftp_code(line: &[u8]) -> Result<u16, ValidationError> {
    if line.len() < 6
        || !line.ends_with(b"\r\n")
        || line[3] != b' '
        || !line[..3].iter().all(u8::is_ascii_digit)
    {
        return Err(ValidationError::new(
            "invalid or unsupported FTP reply framing",
        ));
    }
    Ok(
        u16::from(line[0] - b'0') * 100
            + u16::from(line[1] - b'0') * 10
            + u16::from(line[2] - b'0'),
    )
}

fn tlv<'a>(bytes: &mut &'a [u8], expected: u8) -> Result<&'a [u8], ValidationError> {
    if bytes.len() < 2 || bytes[0] != expected {
        return Err(ValidationError::new("unexpected LDAP BER tag"));
    }
    let (header, length) = if bytes[1] < 128 {
        (2, usize::from(bytes[1]))
    } else {
        let count = usize::from(bytes[1] & 127);
        if !(1..=2).contains(&count) || bytes.len() < 2 + count {
            return Err(ValidationError::new("unsupported LDAP BER length"));
        }
        (
            2 + count,
            bytes[2..2 + count]
                .iter()
                .fold(0usize, |n, b| (n << 8) | usize::from(*b)),
        )
    };
    if length > LIMIT || bytes.len() < header + length {
        return Err(ValidationError::new(
            "truncated or oversized LDAP BER value",
        ));
    }
    let value = &bytes[header..header + length];
    *bytes = &bytes[header + length..];
    Ok(value)
}

fn ldap_code(bytes: &[u8]) -> Result<u16, ValidationError> {
    let mut input = bytes;
    let mut message = tlv(&mut input, 0x30)?;
    if !input.is_empty() || tlv(&mut message, 0x02)? != [1] {
        return Err(ValidationError::new(
            "LDAP response message ID or boundary mismatch",
        ));
    }
    let mut result = tlv(&mut message, 0x61)?;
    let code = tlv(&mut result, 0x0a)?;
    if !message.is_empty() || code.len() != 1 {
        return Err(ValidationError::new("unsupported LDAP bind result"));
    }
    tlv(&mut result, 0x04)?;
    tlv(&mut result, 0x04)?;
    if !result.is_empty() {
        return Err(ValidationError::new("unsupported LDAP bind extension"));
    }
    Ok(u16::from(code[0]))
}

pub(super) fn response_code(protocol: Protocol, bytes: &[u8]) -> Result<u16, ValidationError> {
    match protocol {
        Protocol::Ftp => {
            let mut lines = bytes.split_inclusive(|b| *b == b'\n');
            let mut next = || ftp_code(lines.next().unwrap_or_default());
            if next()? != 220 || next()? != 331 {
                return Err(ValidationError::new("FTP banner/auth sequence mismatch"));
            }
            let code = next()?;
            if ![230, 530].contains(&code) || next()? != 221 || lines.next().is_some() {
                return Err(ValidationError::new(
                    "FTP final auth/close sequence mismatch",
                ));
            }
            Ok(code)
        }
        Protocol::Ldap => ldap_code(bytes),
    }
}

pub(super) fn exchange(
    socket: Socket,
    target: SocketAddr,
    protocol: Protocol,
    accepted: bool,
) -> Result<(Vec<u8>, u16), ValidationError> {
    socket.connect_timeout(&target.into(), Duration::from_secs(2))?;
    let mut wire = Wire {
        stream: socket.into(),
        deadline: Instant::now() + Duration::from_secs(12),
        response: Vec::new(),
    };
    let password = if accepted {
        PASSWORD
    } else {
        "invalid-lab-password"
    };
    let code = match protocol {
        Protocol::Ftp => {
            if wire.ftp_reply()? != 220 {
                return Err(ValidationError::new("FTP banner missing"));
            }
            wire.write(b"USER gewy\r\n")?;
            if wire.ftp_reply()? != 331 {
                return Err(ValidationError::new("FTP password prompt missing"));
            }
            wire.write(format!("PASS {password}\r\n").as_bytes())?;
            let code = wire.ftp_reply()?;
            wire.write(b"QUIT\r\n")?;
            if wire.ftp_reply()? != 221 {
                return Err(ValidationError::new("FTP close reply missing"));
            }
            code
        }
        Protocol::Ldap => {
            let mut bind = vec![0x02, 1, 3, 0x04, DN.len() as u8];
            bind.extend_from_slice(DN.as_bytes());
            bind.extend_from_slice(&[0x80, password.len() as u8]);
            bind.extend_from_slice(password.as_bytes());
            let mut message = vec![
                0x30,
                (5 + bind.len()) as u8,
                0x02,
                1,
                1,
                0x60,
                bind.len() as u8,
            ];
            message.extend(bind);
            wire.write(&message)?;
            let code = wire.ldap_reply()?;
            wire.write(&[0x30, 5, 0x02, 1, 2, 0x42, 0])?;
            code
        }
    };
    let response = wire.finish()?;
    if code != protocol.code(accepted) || response_code(protocol, &response)? != code {
        return Err(ValidationError::new(
            "authentication target response disagrees with fixture",
        ));
    }
    Ok((response, code))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn partial_reads_do_not_restart_the_total_exchange_deadline() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let client = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let worker = std::thread::spawn(move || {
            let (mut server, _) = listener.accept().unwrap();
            for _ in 0..16 {
                if server.write_all(&[1]).is_err() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        });
        let start = Instant::now();
        let mut wire = Wire {
            stream: client,
            deadline: start + Duration::from_millis(80),
            response: Vec::new(),
        };
        assert!(wire.read(16).is_err());
        assert!(start.elapsed() < Duration::from_secs(2));
        drop(wire);
        worker.join().unwrap();
    }
    #[test]
    fn binary_bind_result_requires_framing_tag_message_and_exact_extent() {
        let valid = [0x30, 12, 2, 1, 1, 0x61, 7, 0x0a, 1, 49, 4, 0, 4, 0];
        assert_eq!(ldap_code(&valid).unwrap(), 49);
        let mut success = valid;
        success[9] = 0;
        assert_eq!(ldap_code(&success).unwrap(), 0);
        for (index, byte) in [(0, 0x31), (1, 0x80), (4, 2), (5, 0x65), (7, 2), (8, 2)] {
            let mut invalid = valid;
            invalid[index] = byte;
            assert!(ldap_code(&invalid).is_err());
        }
        assert!(ldap_code(&valid[..13]).is_err());
        assert!(ldap_code(&[valid.as_slice(), &[0]].concat()).is_err());
        let long = [&[0x30, 0x81, 12], &valid[2..]].concat();
        assert_eq!(ldap_code(&long).unwrap(), 49);
    }
    #[test]
    fn ftp_requires_real_reply_boundaries_not_embedded_status_text() {
        let valid = b"220 Ready\r\n331 Password\r\n230 Welcome 530 fake\r\n221 Bye\r\n";
        assert_eq!(response_code(Protocol::Ftp, valid).unwrap(), 230);
        for invalid in [
            b"530 fake".as_slice(),
            b"220-unsupported\r\n",
            b"220 Ready\n",
            b"hello 530 denied\r\n",
        ] {
            assert!(response_code(Protocol::Ftp, invalid).is_err());
        }
    }
}
