//! Fixed, independent protocol-4.1 client for a disposable plaintext fixture.
use super::*;
use ring::digest::{SHA1_FOR_LEGACY_USE_ONLY as SHA1, digest};
const LARGE: usize = 99000;
fn packet(s: &mut TcpStream, seq: u8) -> Vec<u8> {
    let mut h = [0; 4];
    s.read_exact(&mut h).unwrap();
    assert_eq!(h[3], seq);
    h[3] = 0;
    let n = u32::from_le_bytes(h) as usize;
    assert!((1..=256 * 1024).contains(&n));
    let mut body = vec![0; n];
    s.read_exact(&mut body).unwrap();
    body
}
fn send(s: &mut TcpStream, seq: u8, body: &[u8]) {
    let mut h = (body.len() as u32).to_le_bytes();
    h[3] = seq;
    let mut buf = h.to_vec();
    buf.extend_from_slice(body);
    s.write_all(&buf).unwrap();
}
pub(super) struct Client {
    s: TcpStream,
    salt: [u8; 20],
}
impl Client {
    pub fn open() -> Self {
        let mut s = TcpStream::connect(("127.0.0.1", 3306)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(5))).unwrap();
        s.set_write_timeout(Some(Duration::from_secs(5))).unwrap();
        let g = packet(&mut s, 0);
        assert_eq!(g[0], 10);
        let v = g[1..].iter().position(|v| *v == 0).unwrap();
        let b = &g[2 + v..];
        let mut salt = [0; 20];
        salt[..8].copy_from_slice(&b[4..12]);
        salt[8..].copy_from_slice(&b[31..43]);
        assert_eq!(b[12], 0);
        assert_eq!(b[20], 21);
        assert_eq!(&b[44..], b"mysql_native_password\0");
        Self { s, salt }
    }
    pub fn login(&mut self, pw: &str, allow: bool) {
        let first = digest(&SHA1, pw.as_bytes());
        let second = digest(&SHA1, first.as_ref());
        let third = digest(&SHA1, &[&self.salt[..], second.as_ref()].concat());
        let caps: u32 = 1 | 4 | 8 | 0x200 | 0x2000 | 0x8000 | 0x80000;
        let mut b = caps.to_le_bytes().to_vec();
        b.extend((2 * 1024 * 1024u32).to_le_bytes());
        b.push(45);
        b.extend([0; 23]);
        b.extend(b"gewy\0");
        b.push(20);
        b.extend(
            first
                .as_ref()
                .iter()
                .zip(third.as_ref())
                .map(|(a, b)| a ^ b),
        );
        b.extend(b"gewy\0mysql_native_password\0");
        send(&mut self.s, 1, &b);
        let body = packet(&mut self.s, 2);
        if allow {
            assert_eq!(body, [0, 0, 0, 2, 0, 0, 0]);
        } else {
            error(&body, 1045, b"28000");
        }
    }
    fn query(&mut self, sql: &str) {
        send(&mut self.s, 0, &[&[3], sql.as_bytes()].concat());
    }
    fn rows(&mut self, sql: &str, expected: &[Option<&[u8]>]) {
        self.query(sql);
        let cols = expected.len() as u8;
        assert_eq!(packet(&mut self.s, 1), [cols]);
        for i in 0..cols {
            assert!(packet(&mut self.s, 2 + i).starts_with(b"\x03def"));
        }
        let eof = [254, 0, 0, 2, 0];
        assert_eq!(packet(&mut self.s, 2 + cols), eof);
        let mut want = vec![];
        for value in expected {
            let Some(value) = value else {
                want.push(251);
                continue;
            };
            let n = value.len();
            match n {
                0..=250 => want.push(n as u8),
                251..=65535 => {
                    want.push(252);
                    want.extend((n as u16).to_le_bytes());
                }
                _ => {
                    want.push(253);
                    want.extend(&(n as u32).to_le_bytes()[..3]);
                }
            }
            want.extend(*value);
        }
        assert!(packet(&mut self.s, 3 + cols) == want, "text row differs");
        assert_eq!(packet(&mut self.s, 4 + cols), eof);
    }
    fn query_err(&mut self, sql: &str, errno: u16, state: &[u8]) {
        self.query(sql);
        error(&packet(&mut self.s, 1), errno, state);
    }
    pub fn run(&mut self, case: &str) {
        match case {
            "mixed" => {
                self.rows("SELECT NULL", &[None]);
                self.rows("SELECT ''", &[Some(b"")]);
                self.rows(
                    "SELECT UNHEX('FF1504233238303030')",
                    &[Some(b"\xff\x15\x04#28000")],
                );
                self.rows("SELECT REPEAT('x',251)", &[Some(&[b'x'; 251])]);
                let value = b"\xff\x28\x04#42000".repeat(11000);
                assert_eq!(value.len(), LARGE);
                self.rows(
                    "SELECT REPEAT(UNHEX('FF2804233432303030'),11000)",
                    &[Some(&value)],
                );
                self.query_err("GEWY_BAD", 1064, b"42000");
                self.query_err("SELECT `28000`", 1054, b"42S22");
                self.query_err("SIGNAL SQLSTATE '28000' SET MYSQL_ERRNO=1045, MESSAGE_TEXT='fixture command error'",1045,b"28000");
                self.rows("SELECT 'after'", &[Some(b"after")]);
                self.ping();
            }
            "columns" => self.rows("SELECT 1,2", &[Some(b"1"), Some(b"2")]),
            "limit32" | "overflow33" => {
                for _ in 0..if case == "limit32" { 30 } else { 31 } {
                    self.ping();
                }
            }
            "preexisting" => self.ping(),
            _ => panic!("unknown case"),
        }
        send(&mut self.s, 0, &[1]);
    }
    fn ping(&mut self) {
        send(&mut self.s, 0, &[14]);
        assert_eq!(packet(&mut self.s, 1), [0, 0, 0, 2, 0, 0, 0]);
    }
}
fn error(b: &[u8], errno: u16, state: &[u8]) {
    assert_eq!(b[0], 255);
    assert_eq!(u16::from_le_bytes([b[1], b[2]]), errno);
    assert_eq!(b[3], b'#');
    assert_eq!(&b[4..9], state);
    assert!(b.len() > 9);
}
pub(super) fn expected(case: &str) -> Vec<Value> {
    let mut e = vec![json!({"type":"greeting_v10"})];
    let err = |phase, errno, bucket| json!({"type":"error","phase":phase,"errno":errno,"sqlstate_bucket":bucket});
    if case == "denied" {
        e.push(err("authentication", 1045, "28000"));
        return e;
    }
    e.push(json!({"type":"auth_ok"}));
    if case == "limit32" {
        e.extend(vec![json!({"type":"command_ok"}); 30]);
        return e;
    }
    for n in [None, Some(0), Some(9), Some(251), Some(LARGE)] {
        e.extend([
            json!({"type":"row","value_bytes":n}),
            json!({"type":"result_end"}),
        ]);
    }
    e.extend([
        err("command", 1064, "42000"),
        err("command", 1054, "other"),
        err("command", 1045, "28000"),
        json!({"type":"row","value_bytes":5}),
        json!({"type":"result_end"}),
        json!({"type":"command_ok"}),
    ]);
    e
}
