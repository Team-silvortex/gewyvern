use std::collections::BTreeMap;
use std::ffi::{CString, c_int, c_void};
use std::fs::{self, File};
use std::net::{IpAddr, SocketAddr};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::ptr::NonNull;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use super::{
    CaptureError, CaptureFactBuffer, CaptureOptions, CaptureSnapshot, CaptureState,
    FactBufferAccounting, KernelCounters,
};
use crate::ledger::{FactEnvelope, FactId, PacketDir, SessionId, UdpOffloadSegment};
use crate::packet_capture::{CapturedUdpDatagram, PacketObservation};

mod api;
mod http_status;
mod redis_resp;
mod tcp_observer;
use api::Api;
pub use http_status::observe_http_status;
pub use redis_resp::observe_redis_resp;

#[repr(C)]
#[derive(Clone, Copy)]
struct Record {
    monotonic_ns: u64,
    cookie: u64,
    netns_cookie: u64,
    cpu: u32,
    ifindex: u32,
    total_len: u32,
    sport: u16,
    dport: u16,
    payload_len: u16,
    prefix_len: u16,
    source: [u8; 16],
    destination: [u8; 16],
    version: u8,
    ip_version: u8,
    ip_header_len: u8,
    outbound: u8,
    prefix: [u8; 128],
    aggregate_id: u64,
    aggregate_payload_len: u32,
    segment_size: u32,
    segment_index: u32,
    segment_count: u32,
}

impl Record {
    fn offload(&self) -> Result<Option<UdpOffloadSegment>, CaptureError> {
        let invalid = || CaptureError::new("capture_record_invalid", "invalid offload provenance");
        if self.aggregate_id == 0 {
            if self.aggregate_payload_len != 0
                || self.segment_size != 0
                || self.segment_index != 0
                || self.segment_count != 0
            {
                return Err(invalid());
            }
            return Ok(None);
        }
        Ok(Some(UdpOffloadSegment {
            aggregate_id: self.aggregate_id,
            aggregate_payload_len: self.aggregate_payload_len,
            segment_size: self.segment_size.try_into().map_err(|_| invalid())?,
            segment_index: self.segment_index.try_into().map_err(|_| invalid())?,
            segment_count: self.segment_count.try_into().map_err(|_| invalid())?,
            ip_header_len: self.ip_header_len,
        }))
    }
}

struct CallbackState {
    records: Vec<Record>,
    invalid: u64,
}
unsafe extern "C" fn receive(context: *mut c_void, data: *mut c_void, len: usize) -> c_int {
    // SAFETY: boxed state outlives synchronous callbacks. Integer-only records
    // can be read unaligned; reject wrong lengths before touching the buffer.
    let state = unsafe { &mut *context.cast::<CallbackState>() };
    if data.is_null() || len != size_of::<Record>() || state.records.len() >= 512 {
        state.invalid += 1;
        return 0;
    }
    let record = unsafe { data.cast::<Record>().read_unaligned() };
    // Capacity is preallocated, so the C callback never allocates or calls user code.
    state.records.push(record);
    0
}

struct Scratch(PathBuf);
impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn compile_source(code: &str, defines: &[String]) -> Result<(Scratch, PathBuf), CaptureError> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| CaptureError::new("capture_clock_invalid", e.to_string()))?
        .as_nanos();
    let path = Path::new("/tmp").join(format!("gewyvern-bpf-{}-{nonce}", std::process::id()));
    fs::DirBuilder::new().mode(0o700).create(&path)?;
    let scratch = Scratch(path);
    let source = scratch.0.join("capture.c");
    let object = scratch.0.join("capture.o");
    fs::write(&source, code)?;
    let output = crate::bounded_process::run_command_output(
        Command::new("/usr/bin/clang")
            .env_clear()
            .env("PATH", "/usr/bin:/bin")
            .args(["-O2", "-g", "-target", "bpf", "-I/usr/include"])
            .arg(format!(
                "-I/usr/include/{}-linux-gnu",
                std::env::consts::ARCH
            ))
            .args(defines)
            .arg("-c")
            .arg(source)
            .arg("-o")
            .arg(&object),
        Duration::from_secs(30),
        crate::bounded_process::OutputLimits::new(32768, 32768),
        "product BPF compile",
    )?;
    if !output.status.success() {
        return Err(CaptureError::new(
            "capture_compile_failed",
            String::from_utf8_lossy(&output.stderr),
        ));
    }
    Ok((scratch, object))
}

fn compile(options: &CaptureOptions) -> Result<(Scratch, PathBuf), CaptureError> {
    compile_source(
        include_str!("../../ebpf/production/udp.bpf.c"),
        &[
            format!("-DUDP_PORT={}", options.udp_port),
            format!("-DRING_BYTES={}", options.ring_bytes),
            format!(
                "-DOFFLOAD_CORE={}",
                u8::from(size_of::<usize>() == 8 && Path::new("/sys/kernel/btf/vmlinux").is_file())
            ),
        ],
    )
}

fn open_scope(path: &Path) -> Result<(File, PathBuf), CaptureError> {
    let canonical = fs::canonicalize(path)?;
    let root = Path::new("/sys/fs/cgroup");
    if canonical == root
        || !canonical.starts_with(root)
        || canonical != path
        || !canonical.join("cgroup.procs").is_file()
    {
        return Err(CaptureError::new(
            "capture_scope_invalid",
            "use a canonical existing non-root cgroup v2 directory",
        ));
    }
    let file = File::open(&canonical)?;
    let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
    // SAFETY: initialized output storage and live directory fd.
    if unsafe { libc::fstatfs(file.as_raw_fd(), &mut stat) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    if stat.f_type != 0x63677270 {
        return Err(CaptureError::new("capture_scope_invalid", "not cgroup v2"));
    }
    if unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) } != 0 {
        return Err(CaptureError::new(
            "capture_scope_busy",
            "another collector owns this cgroup",
        ));
    }
    Ok((file, canonical))
}

fn link(program: c_int, scope: c_int, direction: u32) -> Result<OwnedFd, CaptureError> {
    #[repr(C)]
    struct Link {
        program: u32,
        target: u32,
        attach_type: u32,
        flags: u32,
    }
    let attr = Link {
        program: program as u32,
        target: scope as u32,
        attach_type: direction,
        flags: 0,
    };
    // SAFETY: Linux BPF_LINK_CREATE UAPI, both fds are live. No legacy attachment
    // fallback: a process-owned link must disappear even on SIGKILL.
    let fd = unsafe { libc::syscall(libc::SYS_bpf, 28, &attr, size_of::<Link>()) };
    if fd < 0 {
        return Err(CaptureError::new(
            "capture_attach_failed",
            std::io::Error::last_os_error().to_string(),
        ));
    }
    Ok(unsafe { OwnedFd::from_raw_fd(fd as c_int) })
}

fn clock() -> Result<u64, CaptureError> {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    if unsafe { libc::clock_gettime(libc::CLOCK_MONOTONIC, &mut time) } != 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(time.tv_sec as u64 * 1_000_000_000 + time.tv_nsec as u64)
}

pub struct Collector {
    api: Api,
    object: Option<NonNull<c_void>>,
    ring: Option<NonNull<c_void>>,
    links: Vec<OwnedFd>,
    scope: File,
    scope_path: PathBuf,
    state: Box<CallbackState>,
    counters_fd: c_int,
    namespaces: BTreeMap<u64, u32>,
    anchor: (u64, SystemTime),
    snapshot: CaptureSnapshot,
    facts: CaptureFactBuffer,
    next_id: u64,
}

impl Collector {
    pub fn start(options: CaptureOptions) -> Result<Self, CaptureError> {
        options.validate()?;
        let (scope, scope_path) = open_scope(&options.cgroup)?;
        let (scratch, object_path) = compile(&options)?;
        let api = Api::new()?;
        let anchor = (clock()?, SystemTime::now());
        let run_id = format!("{}-{}", std::process::id(), anchor.0);
        let mut collector = Self {
            api,
            object: None,
            ring: None,
            links: Vec::new(),
            scope,
            scope_path,
            counters_fd: -1,
            state: Box::new(CallbackState {
                records: Vec::with_capacity(512),
                invalid: 0,
            }),
            namespaces: BTreeMap::new(),
            anchor,
            snapshot: CaptureSnapshot {
                schema_version: 1,
                run_id,
                cgroup: options.cgroup.display().to_string(),
                udp_port: options.udp_port,
                state: CaptureState::Running,
                kernel: Default::default(),
                received: 0,
                retained: 0,
                userspace_dropped: 0,
                decode_rejected: 0,
                final_accounted: false,
                incomplete: false,
            },
            facts: CaptureFactBuffer::new(options.max_facts)?,
            next_id: 1,
        };
        let path = CString::new(object_path.as_os_str().as_bytes())
            .map_err(|e| CaptureError::new("capture_path_invalid", e.to_string()))?;
        let object = collector.api.pointer(
            unsafe { (collector.api.open)(path.as_ptr(), std::ptr::null()) },
            "capture_object_open_failed",
        )?;
        collector.object = Some(object);
        if unsafe { (collector.api.load)(object.as_ptr()) } != 0 {
            return Err(CaptureError::new(
                "capture_object_load_failed",
                std::io::Error::last_os_error().to_string(),
            ));
        }
        let events = unsafe { (collector.api.map_fd)(object.as_ptr(), c"events".as_ptr()) };
        collector.counters_fd =
            unsafe { (collector.api.map_fd)(object.as_ptr(), c"counts".as_ptr()) };
        if events < 0 || collector.counters_fd < 0 {
            return Err(CaptureError::new(
                "capture_maps_missing",
                "product object maps unavailable",
            ));
        }
        collector.ring = Some(collector.api.pointer(
            unsafe {
                (collector.api.ring_new)(
                    events,
                    receive,
                    (&mut *collector.state as *mut CallbackState).cast(),
                    std::ptr::null(),
                )
            },
            "capture_ring_open_failed",
        )?);
        for (name, direction) in [(c"udp_ingress", 0), (c"udp_egress", 1)] {
            let program = collector.api.pointer(
                unsafe { (collector.api.find_program)(object.as_ptr(), name.as_ptr()) },
                "capture_program_missing",
            )?;
            let fd = unsafe { (collector.api.program_fd)(program.as_ptr()) };
            if fd < 0 {
                return Err(CaptureError::new(
                    "capture_program_missing",
                    "invalid program fd",
                ));
            }
            collector
                .links
                .push(link(fd, collector.scope.as_raw_fd(), direction)?);
        }
        drop(scratch);
        Ok(collector)
    }

    pub fn scope_alive(&self) -> bool {
        fs::metadata(&self.scope_path)
            .ok()
            .zip(self.scope.metadata().ok())
            .is_some_and(|(a, b)| a.ino() == b.ino() && a.dev() == b.dev())
    }

    pub fn consume(&mut self) -> Result<usize, CaptureError> {
        let now = Instant::now();
        let expired = self.facts.expire(now);
        self.account_facts(expired);
        let ring = self
            .ring
            .ok_or_else(|| CaptureError::new("capture_ring_closed", "reader closed"))?;
        let count = unsafe { (self.api.consume)(ring.as_ptr(), 512) };
        if count < 0 {
            return Err(CaptureError::new(
                "capture_consume_failed",
                format!("libbpf {count}"),
            ));
        }
        self.snapshot.received += self.state.invalid;
        self.snapshot.decode_rejected += self.state.invalid;
        self.state.invalid = 0;
        // Move the fixed callback buffer out without losing its allocation.
        let mut records = std::mem::take(&mut self.state.records);
        for record in records.drain(..) {
            self.snapshot.received += 1;
            match self.decode(record) {
                Ok(fact) => {
                    let delta = self.facts.push(fact, now);
                    self.account_facts(delta);
                }
                Err(_) => self.snapshot.decode_rejected += 1,
            }
        }
        self.state.records = records;
        Ok(count as usize)
    }

    fn account_facts(&mut self, delta: FactBufferAccounting) {
        self.snapshot.retained += delta.retained;
        self.snapshot.userspace_dropped += delta.userspace_dropped;
        self.snapshot.decode_rejected += delta.decode_rejected;
    }

    fn decode(&mut self, record: Record) -> Result<FactEnvelope, CaptureError> {
        if record.version != 2
            || record.outbound > 1
            || ![4, 6].contains(&record.ip_version)
            || record.prefix_len > 128
            || record.prefix_len > record.payload_len
            || record.netns_cookie == 0
            || (record.ip_version == 4
                && (record.source[4..] != [0; 12] || record.destination[4..] != [0; 12]))
        {
            return Err(CaptureError::new(
                "capture_record_invalid",
                "invalid product event ABI",
            ));
        }
        let offload = record.offload()?;
        let next = u32::try_from(self.namespaces.len() + 1).unwrap_or(u32::MAX);
        if next > 4096 && !self.namespaces.contains_key(&record.netns_cookie) {
            return Err(CaptureError::new(
                "capture_namespace_limit",
                "more than 4096 network namespaces",
            ));
        }
        let netns = *self.namespaces.entry(record.netns_cookie).or_insert(next);
        let delta = record
            .monotonic_ns
            .checked_sub(self.anchor.0)
            .ok_or_else(|| {
                CaptureError::new("capture_clock_invalid", "record before attachment")
            })?;
        let ts = self
            .anchor
            .1
            .checked_add(Duration::from_nanos(delta))
            .ok_or_else(|| CaptureError::new("capture_clock_invalid", "clock overflow"))?;
        let ts = UNIX_EPOCH
            + Duration::from_millis(
                u64::try_from(
                    ts.duration_since(UNIX_EPOCH)
                        .map_err(|e| CaptureError::new("capture_clock_invalid", e.to_string()))?
                        .as_millis(),
                )
                .map_err(|e| CaptureError::new("capture_clock_invalid", e.to_string()))?,
            );
        let address = |bytes: [u8; 16]| {
            if record.ip_version == 6 {
                IpAddr::V6(bytes.into())
            } else {
                IpAddr::V4([bytes[0], bytes[1], bytes[2], bytes[3]].into())
            }
        };
        let datagram = CapturedUdpDatagram {
            source: SocketAddr::new(address(record.source), record.sport),
            destination: SocketAddr::new(address(record.destination), record.dport),
            ip_header_len: record.ip_header_len,
            total_len: record.total_len,
            payload_len: record.payload_len,
            payload_prefix: &record.prefix[..usize::from(record.prefix_len)],
        };
        let observation = PacketObservation {
            id: FactId(self.next_id),
            session: SessionId(1),
            ts,
            cpu: record.cpu,
            ifindex: record.ifindex,
            netns,
            socket_cookie: record.cookie,
            direction: if record.outbound == 1 {
                PacketDir::Egress
            } else {
                PacketDir::Ingress
            },
        };
        let fact = match offload {
            Some(metadata) => datagram.into_offload_segment_fact(observation, metadata),
            None => datagram.into_fact(observation),
        }
        .map_err(|e| CaptureError::new(e.code(), e.to_string()))?;
        self.next_id = self
            .next_id
            .checked_add(1)
            .filter(|id| *id <= i64::MAX as u64)
            .ok_or_else(|| CaptureError::new("capture_id_exhausted", "fact ID space exhausted"))?;
        Ok(fact)
    }

    pub fn take_facts(&mut self) -> Vec<FactEnvelope> {
        self.facts.take_facts()
    }
    pub fn snapshot(&mut self) -> Result<CaptureSnapshot, CaptureError> {
        let zero = 0u32;
        let mut counts = KernelCounters::default();
        if unsafe {
            (self.api.lookup)(
                self.counters_fd,
                (&zero as *const u32).cast(),
                (&mut counts as *mut KernelCounters).cast(),
            )
        } != 0
        {
            return Err(CaptureError::new(
                "capture_counter_read_failed",
                std::io::Error::last_os_error().to_string(),
            ));
        }
        self.snapshot.kernel = counts;
        self.snapshot.incomplete =
            self.snapshot.has_loss() || self.snapshot.state == CaptureState::Failed;
        Ok(self.snapshot.clone())
    }
    pub fn finish(&mut self) -> Result<CaptureSnapshot, CaptureError> {
        self.links.clear();
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.consume()? != 0 {
            if Instant::now() >= deadline {
                return Err(CaptureError::new(
                    "capture_drain_timeout",
                    "ring did not drain after detach",
                ));
            }
        }
        let unfinished = self.facts.finish();
        self.account_facts(unfinished);
        self.snapshot.state = CaptureState::Stopped;
        self.snapshot()?;
        self.snapshot.final_accounted = self.snapshot.reconciles();
        if !self.snapshot.final_accounted {
            return Err(CaptureError::new(
                "capture_counts_unreconciled",
                "detached counters differ from consumed records",
            ));
        }
        Ok(self.snapshot.clone())
    }
}
impl Drop for Collector {
    fn drop(&mut self) {
        self.links.clear();
        // SAFETY: resources are uniquely owned; callbacks stop before state or
        // object maps are freed, and Api outlives these explicit destructors.
        unsafe {
            if let Some(ring) = self.ring.take() {
                (self.api.ring_free)(ring.as_ptr());
            }
            if let Some(object) = self.object.take() {
                (self.api.close)(object.as_ptr());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn product_record_layout_and_scope_admission() {
        assert_eq!(size_of::<Record>(), 232);
        assert_eq!(std::mem::offset_of!(Record, prefix), 80);
        assert_eq!(std::mem::offset_of!(Record, aggregate_id), 208);
        assert_eq!(std::mem::offset_of!(Record, segment_count), 228);
        assert!(open_scope(Path::new("/sys/fs/cgroup")).is_err());
        assert!(open_scope(Path::new("/tmp")).is_err());
    }

    #[test]
    fn product_record_provenance_is_not_truncated_or_implicitly_ordinary() {
        // All fields are integers and byte arrays, including the ABI padding.
        let mut record: Record = unsafe { std::mem::zeroed() };
        assert_eq!(record.offload().unwrap(), None);
        record.segment_count = 4;
        assert!(record.offload().is_err());
        record.aggregate_id = 1;
        record.aggregate_payload_len = 3089;
        record.segment_size = 1024;
        record.ip_header_len = 20;
        assert_eq!(record.offload().unwrap().unwrap().segment_count, 4);
        for field in ["size", "index", "count"] {
            let mut bad = record;
            match field {
                "size" => bad.segment_size = 65536 + 1024,
                "index" => bad.segment_index = 65536,
                _ => bad.segment_count = 65536 + 4,
            }
            assert!(bad.offload().is_err());
        }
    }
}
