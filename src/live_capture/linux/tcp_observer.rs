use super::*;
use serde::Serialize;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Default, Serialize)]
#[repr(C)]
struct Counts {
    unanchored_payload_packets: u64,
    missing_cookie: u64,
    admission_failures: u64,
    invalid_packets: u64,
}
const _: () = assert!(size_of::<Counts>() == 32);

struct Object<'a>(&'a Api, NonNull<c_void>);
impl Drop for Object<'_> {
    fn drop(&mut self) {
        unsafe { (self.0.close)(self.1.as_ptr()) }
    }
}
impl Object<'_> {
    fn fd(&self, name: &std::ffi::CStr) -> Result<c_int, CaptureError> {
        let fd = unsafe { (self.0.map_fd)(self.1.as_ptr(), name.as_ptr()) };
        (fd >= 0).then_some(fd).ok_or_else(|| {
            CaptureError::new("capture_maps_missing", "TCP observer map unavailable")
        })
    }
}

// SAFETY: fd must be a live map with exactly the K/V layouts, and both types
// must accept arbitrary kernel-written bit patterns (integer-only here).
unsafe fn lookup<K, V: Default>(api: &Api, fd: c_int, key: &K) -> Result<V, CaptureError> {
    let mut v = V::default();
    // The two call sites pair the embedded object's map with its repr(C) value.
    if unsafe {
        (api.lookup)(
            fd,
            std::ptr::from_ref(key).cast(),
            std::ptr::from_mut(&mut v).cast(),
        )
    } != 0
    {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(v)
}

pub(super) const STREAM_HEADERS: &[(&str, &str)] = &[(
    "scoped_stream.inc.c",
    include_str!("../../../ebpf/production/scoped_stream.inc.c"),
)];

pub(super) struct EventStream<'a, T> {
    pub conflicting: bool,
    pub partial: bool,
    pub fault: u32,
    pub port: u32,
    pub segments: u32,
    pub count: u32,
    pub items: &'a [T],
}
impl<T: Copy> EventStream<'_, T> {
    pub fn render(
        &self,
        cookie: u64,
        decode: impl Fn(T) -> Option<serde_json::Value>,
    ) -> (serde_json::Value, bool) {
        use serde_json::json;
        let mut outcome = match self.fault {
            _ if self.conflicting => "conflicting",
            0 if self.partial => "partial_frame",
            0 => "framed",
            1 => "sequence_discontinuity",
            2 => "malformed_frame",
            3 => "unsupported_type",
            4 => "limit_reached",
            5 => "segment_budget_exhausted",
            _ => "read_failure",
        };
        let mut events = Vec::new();
        if outcome == "framed" {
            if let Some(decoded) = self
                .items
                .get(..self.count as usize)
                .and_then(|v| v.iter().copied().map(decode).collect::<Option<Vec<_>>>())
            {
                events = decoded;
            } else {
                outcome = "invalid_map_record";
            }
        }
        (
            json!({"socket_cookie":cookie.to_string(),"local_port":self.port,"outcome":outcome,
        "events":events,"server_payload_segments":self.segments}),
            outcome != "framed",
        )
    }
}

pub(super) struct Spec {
    pub kind: &'static str,
    pub code: &'static str,
    pub headers: &'static [(&'static str, &'static str)],
    pub port_define: &'static str,
    pub program: &'static std::ffi::CStr,
    pub rows: &'static std::ffi::CStr,
}

// SAFETY: Row must have the exact integer-only layout of spec.rows. The
// embedded object's counts map and u64 row keys must match this reader.
pub(super) unsafe fn observe<Row: Default>(
    spec: Spec,
    target: (&Path, u16, u32),
    stop: &AtomicBool,
    ready: impl FnOnce(),
    render: impl Fn(u64, Row) -> (serde_json::Value, bool),
) -> Result<(serde_json::Value, bool), CaptureError> {
    let (path, port, seconds) = target;
    if port == 0 || !(1..=300).contains(&seconds) {
        return Err(CaptureError::new(
            "capture_options_invalid",
            "nonzero TCP port and 1..300 seconds required",
        ));
    }
    let (scope, canonical) = open_scope(path)?;
    let (scratch, object_path) = compile_source(
        spec.code,
        &["-mcpu=v3".into(), format!("-D{}={port}", spec.port_define)],
        spec.headers,
    )?;
    let api = Api::new()?;
    let name = CString::new(object_path.as_os_str().as_bytes())
        .map_err(|e| CaptureError::new("capture_path_invalid", e.to_string()))?;
    let object = Object(
        &api,
        api.pointer(
            unsafe { (api.open)(name.as_ptr(), std::ptr::null()) },
            "capture_object_open_failed",
        )?,
    );
    if unsafe { (api.load)(object.1.as_ptr()) } != 0 {
        return Err(CaptureError::new(
            "capture_object_load_failed",
            std::io::Error::last_os_error().to_string(),
        ));
    }
    let prefixes = object.fd(spec.rows)?;
    let counters = object.fd(c"counts")?;
    let prog = api.pointer(
        unsafe { (api.find_program)(object.1.as_ptr(), spec.program.as_ptr()) },
        "capture_program_missing",
    )?;
    let ingress = link(
        unsafe { (api.program_fd)(prog.as_ptr()) },
        scope.as_raw_fd(),
        0,
    )?;
    drop(scratch);
    ready();
    let start = Instant::now();
    let scope_id = scope.metadata()?;
    while !stop.load(Ordering::Relaxed) && start.elapsed() < Duration::from_secs(seconds.into()) {
        if !fs::metadata(&canonical)
            .is_ok_and(|m| m.dev() == scope_id.dev() && m.ino() == scope_id.ino())
        {
            return Err(CaptureError::new(
                "capture_scope_removed",
                "target cgroup removed or replaced",
            ));
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    drop(ingress);
    let counts: Counts = unsafe { lookup(&api, counters, &0u32) }?;
    let mut incomplete = counts.unanchored_payload_packets != 0
        || counts.admission_failures != 0
        || counts.invalid_packets != 0
        || counts.missing_cookie != 0;
    let mut rows = BTreeMap::new();
    let mut last: Option<u64> = None;
    loop {
        let mut key = 0u64;
        let prev = last
            .as_ref()
            .map_or(std::ptr::null(), |v| std::ptr::from_ref(v).cast());
        let rc = unsafe { (api.next_key)(prefixes, prev, (&raw mut key).cast()) };
        if rc != 0 {
            let err = std::io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ENOENT) {
                break;
            }
            return Err(err.into());
        }
        if rows.len() >= 64 || rows.contains_key(&key) {
            return Err(CaptureError::new(
                "capture_map_unstable",
                "unexpected TCP observer map enumeration",
            ));
        }
        let (value, partial) = render(key, unsafe { lookup(&api, prefixes, &key) }?);
        incomplete |= partial;
        rows.insert(key, value);
        last = Some(key);
    }
    Ok((
        serde_json::json!({"schema_version":1,"kind":spec.kind,
        "scope":"ipv4_loopback_client_ingress","cgroup":canonical,"remote_tcp_port":port,
        "incomplete":incomplete,"counters":counts,"connections":rows.into_values().collect::<Vec<_>>() }),
        incomplete,
    ))
}
