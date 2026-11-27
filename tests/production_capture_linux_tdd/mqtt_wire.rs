//! Independent MQTT client: asserts the actual bytes and records the expected
//! broker events by local port. The product does not see the client requests.
use super::*;
const TOPIC: &[u8] = b"gewy/offline";
const ID: &[u8] = b"gewy-persistent";
fn string(b: &mut Vec<u8>, s: &[u8]) {
    b.extend((s.len() as u16).to_be_bytes());
    b.extend(s);
}
fn frame(h: u8, b: &[u8]) -> Vec<u8> {
    let mut v = vec![h];
    let mut n = b.len();
    loop {
        let d = (n % 128) as u8;
        n /= 128;
        v.push(if n == 0 { d } else { d | 128 });
        if n == 0 {
            break;
        }
    }
    v.extend(b);
    v
}
pub(super) struct Wire {
    s: TcpStream,
    port: u16,
    events: Vec<Value>,
}
impl Wire {
    pub fn open() -> Self {
        let s = TcpStream::connect(("127.0.0.1", 1883)).unwrap();
        s.set_read_timeout(Some(Duration::from_secs(4))).unwrap();
        s.set_write_timeout(Some(Duration::from_secs(4))).unwrap();
        Self {
            port: s.local_addr().unwrap().port(),
            s,
            events: Vec::new(),
        }
    }
    fn send(&mut self, h: u8, b: &[u8]) {
        self.s.write_all(&frame(h, b)).unwrap();
    }
    fn read(&mut self, h: u8) -> Vec<u8> {
        let mut byte = [0];
        self.s.read_exact(&mut byte).unwrap();
        assert_eq!(byte, [h]);
        let mut n = 0;
        for i in 0..4 {
            self.s.read_exact(&mut byte).unwrap();
            n |= usize::from(byte[0] & 127) << (7 * i);
            if byte[0] & 128 == 0 {
                assert!(n <= 128 * 1024);
                let mut buf = vec![0; n];
                self.s.read_exact(&mut buf).unwrap();
                return buf;
            }
        }
        panic!("invalid wire length")
    }
    fn expect(&mut self, h: u8, b: &[u8], event: Value) {
        assert_eq!(self.read(h), b);
        self.events.push(event);
    }
    pub fn connect(&mut self, id: &[u8], clean: bool, sp: bool, code: u8) {
        let mut b = b"\0\x04MQTT".to_vec();
        b.extend([4, if clean { 2 } else { 0 }, 0, 30]);
        string(&mut b, id);
        self.send(0x10, &b);
        self.expect(
            0x20,
            &[u8::from(sp), code],
            json!({"type":"connack","session_present":sp,"return_code":code}),
        );
    }
    fn subscribe(&mut self) {
        let mut b = vec![0x12, 0x34];
        string(&mut b, TOPIC);
        b.push(2);
        string(&mut b, b"gewy/another");
        b.push(1);
        self.send(0x82, &b);
        self.expect(
            0x90,
            &[0x12, 0x34, 2, 1],
            json!({"type":"suback","packet_id":0x1234,"codes":[2,1]}),
        );
    }
    fn publish(&mut self, topic: &[u8], qos: u8, id: u16, data: &[u8]) {
        let mut b = Vec::new();
        string(&mut b, topic);
        if qos != 0 {
            b.extend(id.to_be_bytes());
        }
        b.extend(data);
        self.send(0x30 | qos << 1, &b);
        match qos {
            0 => {}
            1 => self.expect(
                0x40,
                &id.to_be_bytes(),
                json!({"type":"puback","packet_id":id}),
            ),
            2 => {
                self.expect(
                    0x50,
                    &id.to_be_bytes(),
                    json!({"type":"pubrec","packet_id":id}),
                );
                self.send(0x62, &id.to_be_bytes());
                self.expect(
                    0x70,
                    &id.to_be_bytes(),
                    json!({"type":"pubcomp","packet_id":id}),
                );
            }
            _ => unreachable!(),
        }
    }
    fn delivery(&mut self, qos: u8, dup: bool, data: &[u8]) -> Option<u16> {
        let b = self.read(0x30 | qos << 1 | u8::from(dup) << 3);
        let mut prefix = vec![];
        string(&mut prefix, TOPIC);
        assert!(b.starts_with(&prefix));
        let mut body = &b[prefix.len()..];
        let id = (qos != 0).then(|| {
            let n = u16::from_be_bytes(body[..2].try_into().unwrap());
            assert_ne!(n, 0);
            body = &body[2..];
            n
        });
        assert!(body == data, "binary delivery differs");
        self.events.push(json!({"type":"publish","qos":qos,"dup":dup,"retain":false,"packet_id":id,"payload_bytes":data.len()}));
        id
    }
    fn ping(&mut self) {
        self.send(0xc0, &[]);
        self.expect(0xd0, &[], json!({"type":"pingresp"}));
    }
    fn finish(mut self, disconnect: bool) -> Value {
        if disconnect {
            self.send(0xe0, &[]);
            assert_eq!(self.s.read(&mut [0]).unwrap(), 0);
        }
        json!({"port":self.port,"events":self.events})
    }
}
pub(super) fn neighbor() {
    let mut w = Wire::open();
    w.connect(b"gewy-other", true, false, 0);
    w.ping();
    w.finish(true);
}
fn resume() -> Vec<Value> {
    let mut rows = vec![];
    let mut subscriber = Wire::open();
    subscriber.connect(ID, false, false, 0);
    subscriber.subscribe();
    rows.push(subscriber.finish(true));
    let data: Vec<_> = b"\0\xff\r\n\x20\x02\x01\0\x40\x02\x12\x34\xd0\0"
        .iter()
        .copied()
        .cycle()
        .take(96 * 1024)
        .collect();
    let mut publisher = Wire::open();
    publisher.connect(b"gewy-publisher", true, false, 0);
    publisher.publish(b"gewy/out", 1, 0xe123, b"qos1");
    publisher.publish(TOPIC, 2, 0xbeef, &data);
    let mut first = Wire::open();
    first.connect(ID, false, true, 0);
    let pending = first.delivery(2, false, &data).unwrap();
    // Deliberately close without PUBREC. This leaves an inflight QoS 2 publication.
    rows.push(first.finish(false));
    let mut resumed = Wire::open();
    resumed.connect(ID, false, true, 0);
    let retry = resumed.delivery(2, true, &data).unwrap();
    assert_eq!(retry, pending);
    resumed.send(0x50, &retry.to_be_bytes());
    resumed.expect(
        0x62,
        &retry.to_be_bytes(),
        json!({"type":"pubrel","packet_id":retry}),
    );
    resumed.send(0x70, &retry.to_be_bytes());
    let binary = b"\xff\0\xd0\0";
    publisher.publish(TOPIC, 1, 0xface, binary);
    let received = resumed.delivery(1, false, binary).unwrap();
    resumed.send(0x40, &received.to_be_bytes());
    publisher.publish(TOPIC, 0, 0, b"");
    resumed.delivery(0, false, b"");
    let mut unsub = vec![0x23, 0x45];
    string(&mut unsub, TOPIC);
    resumed.send(0xa2, &unsub);
    resumed.expect(
        0xb0,
        &[0x23, 0x45],
        json!({"type":"unsuback","packet_id":0x2345}),
    );
    resumed.ping();
    rows.push(resumed.finish(true));
    rows.push(publisher.finish(true));
    let mut clean = Wire::open();
    clean.connect(ID, true, false, 0);
    rows.push(clean.finish(true));
    rows
}
pub(super) fn scenario(case: &str, pre: Option<Wire>) -> Vec<Value> {
    if case == "resume" {
        return resume();
    }
    let mut w = pre.unwrap_or_else(Wire::open);
    if case == "denied" {
        w.connect(b"", false, false, 2);
        return vec![w.finish(false)];
    }
    if case != "preexisting" {
        w.connect(b"gewy-limit", true, false, 0);
    }
    let n = match case {
        "limit32" => 31,
        "overflow33" => 32,
        "preexisting" => 1,
        _ => unreachable!(),
    };
    for _ in 0..n {
        w.ping();
    }
    vec![w.finish(true)]
}
