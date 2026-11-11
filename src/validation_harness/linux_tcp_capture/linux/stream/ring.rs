//! A thread-confined libbpf reader. No build-time libbpf dependency or unbounded queue.
use std::ffi::{c_int, c_void};
use std::os::fd::{AsRawFd, OwnedFd};
use std::ptr::NonNull;

use super::ValidationError;

#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub(super) struct Record {
    pub sequence: u64,
    pub monotonic_ns: u64,
    pub cookie: u64,
    pub cpu: u32,
    pub ifindex: u32,
    pub source: u32,
    pub destination: u32,
    pub seq: u32,
    pub ack: u32,
    pub sport: u16,
    pub dport: u16,
    pub total_len: u16,
    pub window: u16,
    pub payload_len: u16,
    pub prefix_len: u16,
    pub flags: u8,
    pub ip_header_len: u8,
    pub tcp_header_len: u8,
    pub prefix: [u8; 64],
}

type Callback = unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> c_int;
type New = unsafe extern "C" fn(c_int, Callback, *mut c_void, *const c_void) -> *mut c_void;
type Consume = unsafe extern "C" fn(*mut c_void, usize) -> c_int;
type Free = unsafe extern "C" fn(*mut c_void);

struct Library(NonNull<c_void>);
impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: this uniquely owned loader handle outlives all calls and ring destruction.
        unsafe {
            libc::dlclose(self.0.as_ptr());
        }
    }
}

struct Api {
    new: New,
    consume: Consume,
    free: Free,
    _library: Library,
}
impl Api {
    fn load() -> Result<Self, ValidationError> {
        // SAFETY: a static NUL-terminated library name and valid loader flags.
        let handle =
            unsafe { libc::dlopen(c"libbpf.so.1".as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
        let library = Library(
            NonNull::new(handle)
                .ok_or_else(|| ValidationError::new("stream capture requires libbpf.so.1"))?,
        );
        macro_rules! symbol {
            ($name:expr, $ty:ty) => {{
                // SAFETY: signatures match libbpf.h. A null symbol is never called.
                let pointer = unsafe { libc::dlsym(library.0.as_ptr(), $name.as_ptr()) };
                if pointer.is_null() {
                    return Err(ValidationError::new(format!(
                        "libbpf lacks {}",
                        $name.to_string_lossy()
                    )));
                }
                unsafe { std::mem::transmute::<*mut c_void, $ty>(pointer) }
            }};
        }
        Ok(Self {
            new: symbol!(c"ring_buffer__new", New),
            consume: symbol!(c"ring_buffer__consume_n", Consume),
            free: symbol!(c"ring_buffer__free", Free),
            _library: library,
        })
    }
}

struct State<F> {
    handler: F,
    error: Option<String>,
}

unsafe extern "C" fn callback<F>(context: *mut c_void, data: *mut c_void, len: usize) -> c_int
where
    F: FnMut(&Record) -> Result<(), ValidationError>,
{
    // SAFETY: Ring owns the boxed context and libbpf invokes it synchronously on this thread.
    let state = unsafe { &mut *context.cast::<State<F>>() };
    if data.is_null() || len != size_of::<Record>() {
        state.error = Some("invalid stream record ABI length".into());
        return -libc::EINVAL;
    }
    // SAFETY: libbpf guarantees len readable bytes until callback return; all fields are integers.
    let record = unsafe { data.cast::<Record>().read_unaligned() };
    if record.prefix_len > 64 || record.prefix_len > record.payload_len {
        state.error = Some("invalid stream record prefix bounds".into());
        return -libc::EINVAL;
    }
    // A host callback must not unwind across the C boundary.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| (state.handler)(&record))) {
        Ok(Ok(())) => 0,
        Ok(Err(error)) => {
            state.error = Some(error.to_string());
            -libc::EINVAL
        }
        Err(_) => {
            state.error = Some("stream consumer callback panicked".into());
            -libc::EINVAL
        }
    }
}

pub(super) struct Ring<F> {
    ring: NonNull<c_void>,
    state: Box<State<F>>,
    map: OwnedFd,
    api: Api,
}

impl<F: FnMut(&Record) -> Result<(), ValidationError>> Ring<F> {
    pub(super) fn new(map: OwnedFd, handler: F) -> Result<Self, ValidationError> {
        let api = Api::load()?;
        let mut state = Box::new(State {
            handler,
            error: None,
        });
        // SAFETY: map is held open, callback/context remain valid until free, null opts uses defaults.
        let ring = unsafe {
            (api.new)(
                map.as_raw_fd(),
                callback::<F>,
                (&mut *state as *mut State<F>).cast(),
                std::ptr::null(),
            )
        };
        let ring = NonNull::new(ring).ok_or_else(|| {
            ValidationError::new(format!(
                "ring reader creation failed: {}",
                std::io::Error::last_os_error()
            ))
        })?;
        Ok(Self {
            ring,
            state,
            map,
            api,
        })
    }

    pub(super) fn consume(&mut self) -> Result<usize, ValidationError> {
        // SAFETY: uniquely borrowed live ring; bounded batches keep cancellation observable.
        let result = unsafe { (self.api.consume)(self.ring.as_ptr(), 512) };
        if let Some(error) = self.state.error.take() {
            return Err(ValidationError::new(error));
        }
        if result < 0 {
            return Err(ValidationError::new(format!(
                "ring consume failed: {result}"
            )));
        }
        Ok(result as usize)
    }

    pub(super) fn wait(&self) -> Result<(), ValidationError> {
        let mut fd = libc::pollfd {
            fd: self.map.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        // SAFETY: one valid pollfd; wait is bounded even with no producer activity.
        let result = unsafe { libc::poll(&mut fd, 1, 5) };
        if result < 0 && std::io::Error::last_os_error().raw_os_error() != Some(libc::EINTR) {
            return Err(std::io::Error::last_os_error().into());
        }
        if fd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) != 0 {
            return Err(ValidationError::new("ring poll failed"));
        }
        Ok(())
    }
}
impl<F> Drop for Ring<F> {
    fn drop(&mut self) {
        // SAFETY: free runs once, before the context, map fd and loader handle are dropped.
        unsafe {
            (self.api.free)(self.ring.as_ptr());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn wire_layout_matches_bpf_and_bad_length_cannot_reach_sink() {
        assert_eq!(size_of::<Record>(), 128);
        assert_eq!(std::mem::offset_of!(Record, prefix), 63);
        let mut state = State {
            handler: |_: &Record| -> Result<(), ValidationError> { panic!("must not run") },
            error: None,
        };
        invoke(&mut state, std::ptr::null_mut(), 127);
        assert_eq!(
            state.error.as_deref(),
            Some("invalid stream record ABI length")
        );
    }
    fn invoke<F: FnMut(&Record) -> Result<(), ValidationError>>(
        state: &mut State<F>,
        data: *mut c_void,
        len: usize,
    ) {
        // SAFETY: tests provide the matching context type; null/short records are rejected before read.
        assert_eq!(
            unsafe { callback::<F>((state as *mut State<F>).cast(), data, len) },
            -libc::EINVAL
        );
    }
    #[test]
    fn callback_errors_and_panics_do_not_cross_ffi() {
        // SAFETY: all-zero is a valid representation for this integer-only record.
        let mut record: Record = unsafe { std::mem::zeroed() };
        let mut state = State {
            handler: |_: &Record| -> Result<(), ValidationError> {
                Err(ValidationError::new("sink failed"))
            },
            error: None,
        };
        invoke(&mut state, (&mut record as *mut Record).cast(), 128);
        assert_eq!(state.error.as_deref(), Some("sink failed"));
        let mut state = State {
            handler: |_: &Record| -> Result<(), ValidationError> { panic!("test panic") },
            error: None,
        };
        invoke(&mut state, (&mut record as *mut Record).cast(), 128);
        assert_eq!(
            state.error.as_deref(),
            Some("stream consumer callback panicked")
        );
    }
}
