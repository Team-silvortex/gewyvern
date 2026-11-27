use super::*;
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;

#[repr(C)]
#[derive(Default)]
struct Flow {
    busy: u64,
    raced: u64,
    origin: u32,
    next: u32,
    port: u32,
    fault: u32,
    state: u32,
    tag: u32,
    digits: u32,
    negative: u32,
    length: u32,
    left: u32,
    array_left: u32,
    array_len: u32,
    inner_left: u32,
    array_errors: u32,
    count: u32,
    closed: u32,
    segments: u32,
    max_batch: u32,
    items: [u32; 32],
}
const _: () = assert!(size_of::<Flow>() == 216);

fn reply(item: u32) -> Option<Value> {
    let size = item >> 4;
    Some(match item & 15 {
        1 if size == 0 => json!({"type":"simple_string"}),
        2 if size == 0 => json!({"type":"error"}),
        3 if size == 0 => json!({"type":"integer"}),
        4 if size <= 16 * 1024 * 1024 => json!({"type":"bulk_string","bytes":size}),
        5 if size == 0 => json!({"type":"null_bulk"}),
        6 => {
            let elements = size & 255;
            let errors = size >> 8;
            if elements > 128 || errors > elements * 128 {
                return None;
            }
            json!({"type":"array","elements":elements,"error_frames":errors})
        }
        7 if size == 0 => json!({"type":"null_array"}),
        _ => return None,
    })
}

fn render(cookie: u64, row: Flow) -> (Value, bool) {
    let mut outcome = if row.raced != 0 || row.busy != 0 {
        "conflicting"
    } else {
        match row.fault {
            0 if row.state != 0 || row.array_left != 0 || row.inner_left != 0 => "partial_frame",
            0 => "framed",
            1 => "sequence_discontinuity",
            2 => "malformed_frame",
            3 => "unsupported_type",
            4 => "limit_reached",
            5 => "segment_budget_exhausted",
            7 => "array_depth_exceeded",
            _ => "read_failure",
        }
    };
    if row.count as usize > row.items.len() {
        outcome = "invalid_map_record";
    }
    let mut replies = Vec::new();
    if outcome == "framed" {
        for &item in row.items.iter().take(row.count as usize) {
            let Some(value) = reply(item) else {
                outcome = "invalid_map_record";
                replies.clear();
                break;
            };
            replies.push(value);
        }
    }
    (
        json!({"socket_cookie":cookie.to_string(),"local_port":row.port,"outcome":outcome,
        "server_payload_segments":row.segments,"max_replies_per_segment":row.max_batch,
        "replies":replies}),
        outcome != "framed",
    )
}

pub fn observe_redis_resp(
    path: &Path,
    port: u16,
    seconds: u32,
    stop: &AtomicBool,
    ready: impl FnOnce(),
) -> Result<(Value, bool), CaptureError> {
    let spec = tcp_observer::Spec {
        kind: "redis_resp2_reply_frames",
        code: include_str!("../../../ebpf/production/redis_resp.bpf.c"),
        headers: tcp_observer::STREAM_HEADERS,
        port_define: "REDIS_PORT",
        program: c"redis_resp_in",
        rows: c"flows",
    };
    // SAFETY: Flow matches struct resp in the embedded object, with u64 cookie keys.
    unsafe { tcp_observer::observe(spec, (path, port, seconds), stop, ready, render) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn array_counts_are_independent_and_bounded() {
        let array = |n, errors| 6u32 | n << 4 | errors << 12;
        assert_eq!(
            reply(array(128u32, 16384u32)),
            Some(json!({"type":"array","elements":128,"error_frames":16384}))
        );
        for tag in [
            array(129, 0),
            array(0, 1),
            array(1, 129),
            0x12,
            ((16 * 1024 * 1024 + 1) << 4) | 4,
        ] {
            assert_eq!(reply(tag), None);
        }
    }

    #[test]
    fn uncertain_or_invalid_map_rows_never_expose_a_verified_prefix() {
        for mut row in [
            Flow {
                busy: 1,
                ..Flow::default()
            },
            Flow {
                raced: 1,
                ..Flow::default()
            },
            Flow {
                state: 5,
                ..Flow::default()
            },
            Flow {
                array_left: 1,
                ..Flow::default()
            },
            Flow {
                inner_left: 1,
                ..Flow::default()
            },
            Flow {
                fault: 1,
                ..Flow::default()
            },
            Flow {
                count: 33,
                ..Flow::default()
            },
            Flow {
                count: 2,
                ..Flow::default()
            },
        ] {
            row.items[0] = 2;
            row.count = row.count.max(1);
            let (report, incomplete) = render(42, row);
            assert!(incomplete);
            assert_eq!(report["replies"], json!([]));
        }
    }
}
