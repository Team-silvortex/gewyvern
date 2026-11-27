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
    state: u32,
    tag: u32,
    nbytes: u32,
    length: u32,
    left: u32,
    started: u32,
    ready: u32,
    field: u32,
    seen: u32,
    pos: u32,
    mask: u32,
    count: u32,
    closed: u32,
    segments: u32,
    max_batch: u32,
    reserved: u32,
    items: [u32; 32],
}
const _: () = assert!(size_of::<Flow>() == 224);

fn event(n: u32) -> Option<Value> {
    Some(match n {
        1 => json!({"type":"data_row"}),
        2..=4 => json!({"type":"ready","state":(["I","T","E"][(n-2) as usize])}),
        8..=15 => {
            json!({"type":"error_response","phase":if n<12 {"before_first_ready"} else {"after_first_ready"},
            "sqlstate_bucket":(["other","28P01","22012","25P02"][(n%4) as usize])})
        }
        _ => return None,
    })
}

fn render(cookie: u64, row: Flow) -> (Value, bool) {
    tcp_observer::EventStream {
        conflicting: row.busy != 0 || row.raced != 0,
        partial: row.state != 0,
        fault: row.fault,
        port: row.port,
        segments: row.segments,
        count: row.count,
        items: &row.items,
    }
    .render(cookie, event)
}

pub fn observe_postgres(
    path: &Path,
    port: u16,
    seconds: u32,
    stop: &AtomicBool,
    ready: impl FnOnce(),
) -> Result<(Value, bool), CaptureError> {
    let spec = tcp_observer::Spec {
        kind: "postgres_v3_backend_events",
        code: include_str!("../../../ebpf/production/postgres.bpf.c"),
        headers: tcp_observer::STREAM_HEADERS,
        port_define: "PG_PORT",
        program: c"pg_wire_in",
        rows: c"flows",
    };
    // SAFETY: Flow matches struct pg_flow, keyed by u64 socket cookies.
    unsafe { tcp_observer::observe(spec, (path, port, seconds), stop, ready, render) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn uncertain_rows_never_publish_a_prefix() {
        for mut r in [
            Flow {
                state: 2,
                ..Default::default()
            },
            Flow {
                raced: 1,
                ..Default::default()
            },
            Flow {
                fault: 4,
                ..Default::default()
            },
            Flow {
                count: 33,
                ..Default::default()
            },
            Flow {
                count: 2,
                ..Default::default()
            },
        ] {
            r.items[0] = 9;
            r.count = r.count.max(1);
            let (v, incomplete) = render(7, r);
            assert!(incomplete);
            assert_eq!(v["events"], json!([]));
        }
    }
}
