use super::*;
use std::sync::atomic::AtomicBool;

#[repr(C)]
#[derive(Default)]
struct Prefix {
    origin: u32,
    local_port: u32,
    conflicting_origin: u64,
    sealed: u64,
    pieces: u64,
    slots: [u64; 13],
}

impl Prefix {
    fn classify(&self) -> (&'static str, Option<u16>) {
        if self.conflicting_origin != 0 || self.slots.iter().any(|v| v.count_ones() > 1) {
            return ("conflicting", None);
        }
        if self.slots.contains(&0) {
            return ("incomplete_prefix", None);
        }
        let mut status = 0u16;
        for (i, &v) in self.slots.iter().enumerate() {
            if (9..12).contains(&i) {
                let digit = v.trailing_zeros() as u16;
                if digit > 9 || (i == 9 && !(1..=5).contains(&digit)) {
                    return ("not_http11", None);
                }
                status = status * 10 + digit;
            } else if v != 1 {
                return ("not_http11", None);
            }
        }
        ("verified_prefix", Some(status))
    }
}

pub fn observe_http_status(
    path: &Path,
    port: u16,
    seconds: u32,
    stop: &AtomicBool,
    ready: impl FnOnce(),
) -> Result<(serde_json::Value, bool), CaptureError> {
    let spec = tcp_observer::Spec {
        kind: "http11_first_response_status",
        code: include_str!("../../../ebpf/production/http_status.bpf.c"),
        port_define: "HTTP_PORT",
        program: c"http_status_in",
        rows: c"prefixes",
    };
    // SAFETY: Prefix matches struct prefix in the embedded program.
    unsafe {
        tcp_observer::observe(
            spec,
            (path, port, seconds),
            stop,
            ready,
            |key, row: Prefix| {
                let (outcome, status) = row.classify();
                (
                    serde_json::json!({"socket_cookie":key.to_string(),"local_port":row.local_port,
          "outcome":outcome,"status":status,"prefix_segments":row.pieces}),
                    matches!(outcome, "conflicting" | "incomplete_prefix"),
                )
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn gaps_conflicts_and_invalid_digits_never_produce_status() {
        let mut row = Prefix {
            slots: [1; 13],
            ..Default::default()
        };
        row.slots[9] = 1 << 4;
        row.slots[11] = 1 << 3;
        assert_eq!(row.classify(), ("verified_prefix", Some(403)));
        for (i, mask, outcome) in [
            (0, 0, "incomplete_prefix"),
            (10, 3, "conflicting"),
            (9, 64, "not_http11"),
            (12, 1024, "not_http11"),
        ] {
            let old = std::mem::replace(&mut row.slots[i], mask);
            assert_eq!(row.classify(), (outcome, None));
            row.slots[i] = old;
        }
        row.conflicting_origin = 1;
        assert_eq!(row.classify(), ("conflicting", None));
        assert_eq!(size_of::<Prefix>(), 136);
    }
}
