use std::ffi::{c_char, c_int, c_void};
use std::ptr::NonNull;

use super::CaptureError;
pub(super) type Callback = unsafe extern "C" fn(*mut c_void, *mut c_void, usize) -> c_int;

pub(super) struct Api {
    library: NonNull<c_void>,
    pub open: unsafe extern "C" fn(*const c_char, *const c_void) -> *mut c_void,
    pub load: unsafe extern "C" fn(*mut c_void) -> c_int,
    pub close: unsafe extern "C" fn(*mut c_void),
    pub find_program: unsafe extern "C" fn(*const c_void, *const c_char) -> *mut c_void,
    pub program_fd: unsafe extern "C" fn(*const c_void) -> c_int,
    pub map_fd: unsafe extern "C" fn(*const c_void, *const c_char) -> c_int,
    pub lookup: unsafe extern "C" fn(c_int, *const c_void, *mut c_void) -> c_int,
    pub next_key: unsafe extern "C" fn(c_int, *const c_void, *mut c_void) -> c_int,
    pub ring_new: unsafe extern "C" fn(c_int, Callback, *mut c_void, *const c_void) -> *mut c_void,
    pub consume: unsafe extern "C" fn(*mut c_void, usize) -> c_int,
    pub ring_free: unsafe extern "C" fn(*mut c_void),
    pub get_error: unsafe extern "C" fn(*const c_void) -> libc::c_long,
}
impl Api {
    pub fn new() -> Result<Self, CaptureError> {
        // SAFETY: constant C strings and documented libbpf ABI signatures. The
        // library stays loaded until after every object/ring is destroyed.
        unsafe {
            let library = NonNull::new(libc::dlopen(
                c"libbpf.so.1".as_ptr(),
                libc::RTLD_NOW | libc::RTLD_LOCAL,
            ))
            .ok_or_else(|| CaptureError::new("capture_libbpf_missing", "libbpf.so.1 required"))?;
            macro_rules! symbol {
                ($name:literal, $ty:ty) => {{
                    let pointer =
                        libc::dlsym(library.as_ptr(), concat!($name, "\0").as_ptr().cast());
                    if pointer.is_null() {
                        libc::dlclose(library.as_ptr());
                        return Err(CaptureError::new("capture_libbpf_symbol_missing", $name));
                    }
                    std::mem::transmute::<*mut c_void, $ty>(pointer)
                }};
            }
            Ok(Self {
                library,
                open: symbol!(
                    "bpf_object__open_file",
                    unsafe extern "C" fn(*const c_char, *const c_void) -> *mut c_void
                ),
                load: symbol!(
                    "bpf_object__load",
                    unsafe extern "C" fn(*mut c_void) -> c_int
                ),
                close: symbol!("bpf_object__close", unsafe extern "C" fn(*mut c_void)),
                find_program: symbol!(
                    "bpf_object__find_program_by_name",
                    unsafe extern "C" fn(*const c_void, *const c_char) -> *mut c_void
                ),
                program_fd: symbol!(
                    "bpf_program__fd",
                    unsafe extern "C" fn(*const c_void) -> c_int
                ),
                map_fd: symbol!(
                    "bpf_object__find_map_fd_by_name",
                    unsafe extern "C" fn(*const c_void, *const c_char) -> c_int
                ),
                lookup: symbol!(
                    "bpf_map_lookup_elem",
                    unsafe extern "C" fn(c_int, *const c_void, *mut c_void) -> c_int
                ),
                next_key: symbol!(
                    "bpf_map_get_next_key",
                    unsafe extern "C" fn(c_int, *const c_void, *mut c_void) -> c_int
                ),
                ring_new: symbol!(
                    "ring_buffer__new",
                    unsafe extern "C" fn(
                        c_int,
                        Callback,
                        *mut c_void,
                        *const c_void,
                    ) -> *mut c_void
                ),
                consume: symbol!(
                    "ring_buffer__consume_n",
                    unsafe extern "C" fn(*mut c_void, usize) -> c_int
                ),
                ring_free: symbol!("ring_buffer__free", unsafe extern "C" fn(*mut c_void)),
                get_error: symbol!(
                    "libbpf_get_error",
                    unsafe extern "C" fn(*const c_void) -> libc::c_long
                ),
            })
        }
    }
    pub fn pointer(
        &self,
        pointer: *mut c_void,
        code: &'static str,
    ) -> Result<NonNull<c_void>, CaptureError> {
        // SAFETY: get_error accepts null and libbpf error pointers.
        let error = unsafe { (self.get_error)(pointer) };
        if error != 0 || pointer.is_null() {
            return Err(CaptureError::new(
                code,
                format!("libbpf error {error}: {}", std::io::Error::last_os_error()),
            ));
        }
        NonNull::new(pointer).ok_or_else(|| CaptureError::new(code, "null libbpf pointer"))
    }
}
impl Drop for Api {
    fn drop(&mut self) {
        // SAFETY: owner destroys every libbpf object before dropping Api.
        unsafe {
            libc::dlclose(self.library.as_ptr());
        }
    }
}
