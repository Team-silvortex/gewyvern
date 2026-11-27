use super::*;
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

#[derive(Default)]
#[repr(C)]
struct Flow {
    busy: u64,
    raced: u64,
    origin: u32,
    next: u32,
    port: u32,
    fault: u32,
    phase: u32,
    expected: u32,
    header: u32,
    length: u32,
    pos: u32,
    skip: u32,
    kind: u32,
    number: u32,
    mask: u32,
    width: u32,
    count: u32,
    closed: u32,
    segments: u32,
    max_batch: u32,
    items: [u32; 32],
}
const _: () = assert!(size_of::<Flow>() == 216);
fn event(item: u32) -> Option<Value> {
    let kind = item >> 24;
    let detail = item & 0xffffff;
    Some(match (kind, detail) {
        (1, 0) => json!({"type":"greeting_v10"}),
        (2, 0) => json!({"type":"auth_ok"}),
        (3, 0) => json!({"type":"command_ok"}),
        (4 | 5, n) if n & 65535 != 0 => {
            let bucket = match n >> 16 {
                0 => "other",
                1 => "28000",
                2 => "42000",
                _ => return None,
            };
            json!({"type":"error","phase":if kind==4 {"authentication"}else{"command"},"errno":n&65535,"sqlstate_bucket":bucket})
        }
        (6, n) if n < 2 * 1024 * 1024 => json!({"type":"row","value_bytes":n}),
        (7, 0) => json!({"type":"row","value_bytes":null}),
        (8, 0) => json!({"type":"result_end"}),
        _ => return None,
    })
}
fn render(cookie: u64, row: Flow) -> (Value, bool) {
    tcp_observer::EventStream {
        conflicting: row.busy != 0 || row.raced != 0,
        partial: row.header != 0 || !matches!(row.phase, 2 | 6),
        port: row.port,
        fault: row.fault,
        segments: row.segments,
        count: row.count,
        items: &row.items,
    }
    .render(cookie, event)
}
pub fn observe_mysql(
    path: &Path,
    port: u16,
    seconds: u32,
    stop: &AtomicBool,
    ready: impl FnOnce(),
) -> Result<(Value, bool), CaptureError> {
    let spec = tcp_observer::Spec {
        kind: "mysql_classic_single_column",
        code: include_str!("../../../ebpf/production/mysql.bpf.c"),
        headers: tcp_observer::STREAM_HEADERS,
        port_define: "MYSQL_PORT",
        program: c"mysql_classic_in",
        rows: c"flows",
    };
    // SAFETY: Flow matches struct my_flow, keyed by u64 socket cookies.
    unsafe { tcp_observer::observe(spec, (path, port, seconds), stop, ready, render) }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn unfinished_result_or_auth_exchange_withholds_past_events() {
        for phase in [0, 1, 3, 4, 5] {
            let mut row = Flow {
                phase,
                count: 1,
                ..Flow::default()
            };
            row.items[0] = 1 << 24;
            let (report, incomplete) = render(3, row);
            assert!(incomplete);
            assert_eq!(report["events"], json!([]));
        }
        assert_ne!(event(6 << 24), event(7 << 24));
        assert_eq!(event(5 << 24 | 3 << 16 | 1064), None);
    }
}
