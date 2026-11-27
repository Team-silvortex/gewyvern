use super::*;
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

#[derive(Clone, Copy, Default)]
#[repr(C)]
struct Item {
    descriptor: u32,
    extra: u32,
}
#[derive(Default)]
#[repr(C)]
struct Flow {
    busy: u64,
    raced: u64,
    origin: u32,
    next: u32,
    port: u32,
    fault: u32,
    state: u32,
    session: u32,
    header: u32,
    digits: u32,
    length: u32,
    left: u32,
    topic: u32,
    id: u32,
    pos: u32,
    extra: u32,
    count: u32,
    closed: u32,
    segments: u32,
    max_batch: u32,
    items: [Item; 32],
}
const _: () = assert!(size_of::<Flow>() == 344);
fn event(item: Item) -> Option<Value> {
    let h = item.descriptor >> 24;
    let n = (item.descriptor >> 16) & 255;
    let id = item.descriptor & 65535;
    let extra = item.extra;
    if h != 0x90 && n != 0 {
        return None;
    }
    Some(match h {
        0x20 if id <= 5 && extra <= 1 && (id == 0 || extra == 0) => {
            json!({"type":"connack","return_code":id,"session_present":extra==1})
        }
        0x30..=0x3f => {
            let qos = (h >> 1) & 3;
            if qos == 3
                || (qos == 0 && (id != 0 || h & 8 != 0))
                || (qos != 0 && id == 0)
                || extra > 1024 * 1024
            {
                return None;
            }
            json!({"type":"publish","qos":qos,"dup":h&8!=0,"retain":h&1!=0,"packet_id":(qos!=0).then_some(id),"payload_bytes":extra})
        }
        0x90 if id != 0 && (1..=8).contains(&n) && extra >> (n * 2) == 0 => {
            let codes: Vec<_> = (0..n)
                .map(|i| match (extra >> (2 * i)) & 3 {
                    3 => 128,
                    x => x,
                })
                .collect();
            json!({"type":"suback","packet_id":id,"codes":codes})
        }
        0x40 | 0x50 | 0x62 | 0x70 | 0xb0 if id != 0 && extra == 0 => {
            let kind = match h {
                0x40 => "puback",
                0x50 => "pubrec",
                0x62 => "pubrel",
                0x70 => "pubcomp",
                _ => "unsuback",
            };
            json!({"type":kind,"packet_id":id})
        }
        0xd0 if id == 0 && extra == 0 => json!({"type":"pingresp"}),
        _ => return None,
    })
}
fn render(cookie: u64, row: Flow) -> (Value, bool) {
    tcp_observer::EventStream {
        conflicting: row.busy != 0 || row.raced != 0,
        partial: row.state != 0 || row.session == 0,
        fault: row.fault,
        port: row.port,
        segments: row.segments,
        count: row.count,
        items: &row.items,
    }
    .render(cookie, event)
}
pub fn observe_mqtt(
    path: &Path,
    port: u16,
    seconds: u32,
    stop: &AtomicBool,
    ready: impl FnOnce(),
) -> Result<(Value, bool), CaptureError> {
    let spec = tcp_observer::Spec {
        kind: "mqtt311_broker_frames",
        code: include_str!("../../../ebpf/production/mqtt.bpf.c"),
        headers: tcp_observer::STREAM_HEADERS,
        port_define: "MQTT_PORT",
        program: c"mqtt311_in",
        rows: c"flows",
    };
    // SAFETY: Flow matches struct mq_flow, keyed by u64 socket cookies.
    unsafe { tcp_observer::observe(spec, (path, port, seconds), stop, ready, render) }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_session_flags_and_identifiers_have_no_interpretation() {
        let decode = |h: u32, n: u32, id: u32, extra| {
            event(Item {
                descriptor: h << 24 | n << 16 | id,
                extra,
            })
        };
        assert_eq!(decode(0x20, 0, 2, 1), None);
        assert_eq!(decode(0x38, 0, 0, 0), None);
        assert_eq!(decode(0x32, 0, 0, 0), None);
        assert_eq!(decode(0x40, 0, 0, 0), None);
        assert_eq!(
            decode(0x90, 3, 1, 0b11_10_01).unwrap()["codes"],
            json!([1, 2, 128])
        );
        assert_eq!(decode(0x90, 1, 1, 4), None);
        assert_eq!(decode(0x62, 0, 65535, 0).unwrap()["packet_id"], 65535);
    }
}
