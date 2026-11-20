//! Keep successful inactive units observable without changing their service policy.
use std::ffi::{CString, c_char, c_int, c_void};
use std::ptr::NonNull;

struct Library(NonNull<c_void>);
impl Drop for Library {
    fn drop(&mut self) {
        // SAFETY: this wrapper owns the library handle, after bus shutdown.
        unsafe {
            libc::dlclose(self.0.as_ptr());
        }
    }
}

pub(super) struct UnitReference {
    bus: *mut c_void,
    close: unsafe extern "C" fn(*mut c_void) -> *mut c_void,
    _library: Library,
}

impl UnitReference {
    pub(super) fn new(unit: &str) -> Self {
        let unit = CString::new(unit).unwrap();
        // SAFETY: documented sd-bus ABI; synchronous calls on this thread only.
        // All strings and the shared library outlive calls, and Drop closes the
        // connection to release RefUnit even if a field assertion unwinds.
        unsafe {
            let library = Library(
                NonNull::new(libc::dlopen(
                    c"libsystemd.so.0".as_ptr(),
                    libc::RTLD_NOW | libc::RTLD_LOCAL,
                ))
                .expect("host libsystemd required"),
            );
            macro_rules! symbol {
                ($name:literal, $ty:ty) => {{
                    let pointer =
                        libc::dlsym(library.0.as_ptr(), concat!($name, "\0").as_ptr().cast());
                    assert!(!pointer.is_null(), "missing sd-bus symbol {}", $name);
                    std::mem::transmute::<*mut c_void, $ty>(pointer)
                }};
            }
            let open = symbol!(
                "sd_bus_open_system",
                unsafe extern "C" fn(*mut *mut c_void) -> c_int
            );
            let timeout = symbol!(
                "sd_bus_set_method_call_timeout",
                unsafe extern "C" fn(*mut c_void, u64) -> c_int
            );
            let call = symbol!(
                "sd_bus_call_method",
                unsafe extern "C" fn(
                    *mut c_void,
                    *const c_char,
                    *const c_char,
                    *const c_char,
                    *const c_char,
                    *mut c_void,
                    *mut *mut c_void,
                    *const c_char,
                    ...
                ) -> c_int
            );
            let close = symbol!(
                "sd_bus_flush_close_unref",
                unsafe extern "C" fn(*mut c_void) -> *mut c_void
            );
            let mut reference = Self {
                bus: std::ptr::null_mut(),
                close,
                _library: library,
            };
            assert!(open(&mut reference.bus) >= 0, "open system bus");
            assert!(timeout(reference.bus, 5_000_000) >= 0, "set bus deadline");
            let result = call(
                reference.bus,
                c"org.freedesktop.systemd1".as_ptr(),
                c"/org/freedesktop/systemd1".as_ptr(),
                c"org.freedesktop.systemd1.Manager".as_ptr(),
                c"RefUnit".as_ptr(),
                std::ptr::null_mut(),
                std::ptr::null_mut(),
                c"s".as_ptr(),
                unit.as_ptr(),
            );
            assert!(result >= 0, "RefUnit failed: {result}");
            reference
        }
    }
}

impl Drop for UnitReference {
    fn drop(&mut self) {
        // SAFETY: owned sd-bus connection; NULL is accepted on failed open.
        unsafe {
            (self.close)(self.bus);
        }
    }
}
