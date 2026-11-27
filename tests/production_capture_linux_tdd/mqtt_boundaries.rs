//! Read acknowledgements force fragment boundaries. These are not Mosquitto bytes.
use super::*;
fn chunks(case: &str) -> Vec<Vec<u8>> {
    if case == "control-session" {
        return vec![vec![0x20, 2, 1, 2]];
    }
    let mut v = vec![vec![0x20, 2, 0, 0]];
    match case {
        "control-split" | "control-truncated" => {
            // 2 topic-length bytes + 1 topic byte + 2 id bytes + 128 body bytes.
            v.extend([
                vec![0x33, 0x85],
                vec![1, 0],
                vec![1, b't', 0xab],
                vec![0xcd, 0xd0, 0],
            ]);
            let mut tail = vec![0xff; 126];
            if case == "control-split" {
                tail.extend([0xd0, 0]);
            } else {
                tail.pop();
            }
            v.push(tail);
        }
        "control-zero-id" => v.push(vec![0x40, 2, 0, 0]),
        "control-length" => v.push(vec![0xd0, 0x80, 0]),
        "control-flags" => v.push(vec![0x60, 2, 0, 1]),
        "control-too-large" => v.push(vec![0x30, 0x81, 0x80, 0x40]), // 1 MiB + 1
        _ => unreachable!(),
    }
    v
}
pub(super) fn receive(port: u16, case: &str) {
    let mut s = TcpStream::connect(("127.0.0.1", port)).unwrap();
    s.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
    for chunk in chunks(case) {
        let mut got = vec![0; chunk.len()];
        s.read_exact(&mut got).unwrap();
        assert_eq!(got, chunk);
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
    for chunk in chunks(case) {
        s.write_all(&chunk).unwrap();
        let mut ack = [0];
        s.read_exact(&mut ack).unwrap();
        assert_eq!(ack, [1]);
    }
    s.shutdown(std::net::Shutdown::Write).unwrap();
}
pub(super) fn check(row: &Value, case: &str) {
    if case == "control-split" {
        assert_eq!(row["outcome"], "framed");
        assert!(row["server_payload_segments"].as_u64().unwrap() >= 5);
        assert_eq!(
            row["events"],
            json!([{"type":"connack","session_present":false,"return_code":0},
            {"type":"publish","packet_id":0xabcd,"qos":1,"retain":true,"dup":false,"payload_bytes":128},{"type":"pingresp"}])
        );
    } else {
        assert_eq!(row["events"], json!([]));
        assert_eq!(
            row["outcome"],
            match case {
                "control-truncated" => "partial_frame",
                "control-flags" => "unsupported_type",
                "control-too-large" => "limit_reached",
                _ => "malformed_frame",
            }
        );
    }
}
