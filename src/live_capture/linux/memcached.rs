use super::*;
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

#[repr(C)]
#[derive(Default, Clone, Copy)]
struct Item {
    op_status: u32,
    value_bytes: u32,
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
    pos: u32,
    op: u32,
    extras: u32,
    status: u32,
    length: u32,
    left: u32,
    count: u32,
    closed: u32,
    segments: u32,
    max_batch: u32,
    items: [Item; 32],
}
const _: () = assert!(size_of::<Flow>() == 328);

fn event(item: Item) -> Option<Value> {
    let op = item.op_status >> 16;
    let status = item.op_status & 0xffff;
    let operation = match op {
        0 => "get",
        1 => "set",
        2 => "add",
        3 => "replace",
        4 => "delete",
        9 => "getq",
        10 => "noop",
        14 => "append",
        15 => "prepend",
        _ => return None,
    };
    if matches!(op, 0 | 9) && status == 0 {
        return (item.value_bytes <= 16*1024*1024-4).then(||json!({"operation":operation,"status":0,"result":"hit","value_bytes":item.value_bytes}));
    }
    if item.value_bytes != 0 {
        return None;
    }
    let result = match (op, status) {
        (0 | 9, 1) => "miss",
        (1 | 2 | 3 | 14 | 15, 0) => "stored",
        (4, 0) => "deleted",
        (10, 0) => "ok",
        (_, 1) => "not_found",
        (_, 2) => "key_exists",
        (_, 3) => "too_large",
        (_, 5) => "not_stored",
        _ => "error",
    };
    Some(json!({"operation":operation,"status":status,"result":result}))
}
fn render(cookie: u64, row: Flow) -> (Value, bool) {
    let (mut report, incomplete) = tcp_observer::EventStream {
        conflicting: row.busy != 0 || row.raced != 0,
        partial: row.pos != 0 || row.left != 0,
        port: row.port,
        fault: row.fault,
        segments: row.segments,
        count: row.count,
        items: &row.items,
    }
    .render(cookie, event);
    report["max_replies_per_segment"] = json!(row.max_batch);
    (report, incomplete)
}
pub fn observe_memcached(
    path: &Path,
    port: u16,
    seconds: u32,
    stop: &AtomicBool,
    ready: impl FnOnce(),
) -> Result<(Value, bool), CaptureError> {
    let spec = tcp_observer::Spec {
        kind: "memcached_binary_replies",
        code: include_str!("../../../ebpf/production/memcached.bpf.c"),
        headers: tcp_observer::STREAM_HEADERS,
        port_define: "MC_PORT",
        program: c"mc_binary_in",
        rows: c"flows",
    };
    // SAFETY: Flow matches struct mc_flow and uses u64 socket cookie keys.
    unsafe { tcp_observer::observe(spec, (path, port, seconds), stop, ready, render) }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn an_empty_value_is_not_a_miss_or_an_error_body() {
        let decode = |op, status, bytes| {
            event(Item {
                op_status: op << 16 | status,
                value_bytes: bytes,
            })
        };
        assert_eq!(decode(0u32, 0u32, 0).unwrap()["result"], "hit");
        assert_eq!(decode(0, 1, 0).unwrap()["result"], "miss");
        assert_eq!(decode(4, 1, 0).unwrap()["result"], "not_found");
        assert_eq!(decode(1, 0, 1), None);
        assert_eq!(decode(0, 1, 4), None);
        assert_eq!(decode(11, 0, 0), None);
        assert_eq!(decode(0, 0, 16 * 1024 * 1024), None);
        assert_eq!(decode(1, 0x1234, 0).unwrap()["status"], 0x1234);
    }
}
