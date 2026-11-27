//! These are controlled server bytes, not live mysqld results. Ack after each
//! read to force packet boundaries without relying on the timing of writes.
use super::*;
fn packet(seq: u8, body: &[u8]) -> Vec<u8> {
    let mut h = (body.len() as u32).to_le_bytes();
    h[3] = seq;
    [&h, body].concat()
}
fn chunks(case: &str) -> Vec<Vec<u8>> {
    let mut hello = b"\x0a8.0-test\0".to_vec();
    hello.extend([1, 0, 0, 0]);
    hello.extend(b"12345678\0");
    hello.extend([0xff, 0xf7, 45, 2, 0, 0xff, 0x81, 21]);
    hello.extend([0; 10]);
    hello.extend(b"123456789abc\0mysql_native_password\0");
    let ok = [0, 0, 0, 2, 0, 0, 0];
    let mut parts = vec![packet(0, &hello), packet(2, &ok)];
    match case {
        "control-split" => {
            let err = packet(1, b"\xff\x28\x04#42000synthetic");
            parts.extend([
                err[..2].to_vec(),
                err[2..7].to_vec(),
                err[7..10].to_vec(),
                err[10..].to_vec(),
                packet(1, &ok),
            ]);
        }
        "control-sequence" => parts.push(packet(2, &ok)),
        "control-marker" => parts.push(packet(1, b"\xff\x15\x04!28000synthetic")),
        "control-no-end" | "control-row-length" => {
            let mut col = b"\x03def\0\0\0\x01x\0".to_vec();
            col.extend([12, 45, 0, 1, 0, 0, 0, 253, 0, 0, 0, 0, 0]);
            parts.extend([
                packet(1, &[1]),
                packet(2, &col),
                packet(3, &[254, 0, 0, 2, 0]),
                packet(
                    4,
                    if case == "control-no-end" {
                        b"\x01x"
                    } else {
                        b"\xfc\x01\0x"
                    },
                ),
            ]);
        }
        _ => unreachable!(),
    }
    parts
}
pub(super) fn receive(port: u16, case: &str) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
    for expected in chunks(case) {
        let mut got = vec![0; expected.len()];
        s.read_exact(&mut got).unwrap();
        assert_eq!(got, expected);
        s.write_all(&[1]).unwrap();
    }
    assert_eq!(s.read(&mut [0]).unwrap(), 0);
}
pub(super) fn send(l: TcpListener, case: &str) {
    l.set_nonblocking(true).unwrap();
    let mut peer = None;
    until(|| {
        peer = l.accept().ok();
        peer.is_some()
    });
    let mut s = peer.unwrap().0;
    s.set_nodelay(true).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
    for part in chunks(case) {
        s.write_all(&part).unwrap();
        let mut ack = [0];
        s.read_exact(&mut ack).unwrap();
        assert_eq!(ack, [1]);
    }
    s.shutdown(std::net::Shutdown::Write).unwrap();
}
