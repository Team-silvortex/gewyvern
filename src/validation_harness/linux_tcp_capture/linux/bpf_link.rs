use std::ffi::CString;
use std::fs::File;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

use super::ValidationError;

#[repr(C)]
struct ObjectGet {
    pathname: u64,
    bpf_fd: u32,
    file_flags: u32,
}
#[repr(C)]
struct LinkCreate {
    prog_fd: u32,
    target_fd: u32,
    attach_type: u32,
    flags: u32,
}
#[repr(C)]
struct InfoGet {
    bpf_fd: u32,
    info_len: u32,
    info: u64,
}

fn call<T>(command: u32, attr: &mut T) -> Result<libc::c_long, ValidationError> {
    // SAFETY: the private callers pass initialized Linux UAPI structs and keep
    // referenced buffers/fds alive for this synchronous call.
    let result = unsafe { libc::syscall(libc::SYS_bpf, command, attr as *mut T, size_of::<T>()) };
    if result < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    Ok(result)
}

fn owned_fd<T>(command: u32, attr: &mut T) -> Result<OwnedFd, ValidationError> {
    let fd = call(command, attr)?;
    // SAFETY: OBJ_GET and LINK_CREATE return new uniquely owned descriptors.
    Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
}

pub(super) fn open_pinned(path: &Path) -> Result<OwnedFd, ValidationError> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|e| ValidationError::new(e.to_string()))?;
    owned_fd(
        7,
        &mut ObjectGet {
            pathname: path.as_ptr() as u64,
            bpf_fd: 0,
            file_flags: 0,
        },
    )
}

pub(super) fn attach_cgroup(program: &Path, cgroup: &Path) -> Result<OwnedFd, ValidationError> {
    attach_cgroup_direction(program, cgroup, 0)
}

pub(super) fn attach_cgroup_egress(
    program: &Path,
    cgroup: &Path,
) -> Result<OwnedFd, ValidationError> {
    attach_cgroup_direction(program, cgroup, 1)
}

fn attach_cgroup_direction(
    program: &Path,
    cgroup: &Path,
    attach_type: u32,
) -> Result<OwnedFd, ValidationError> {
    let program = open_pinned(program)?;
    let cgroup = File::open(cgroup)?;
    owned_fd(
        28,
        &mut LinkCreate {
            prog_fd: program.as_raw_fd() as u32,
            target_fd: cgroup.as_raw_fd() as u32,
            attach_type,
            flags: 0,
        },
    )
}

pub(super) fn id(link: &OwnedFd) -> Result<u32, ValidationError> {
    // The first three u32 fields of Linux bpf_link_info are type, id and prog_id.
    let mut info = [0u32; 3];
    call(
        15,
        &mut InfoGet {
            bpf_fd: link.as_raw_fd() as u32,
            info_len: size_of_val(&info) as u32,
            info: info.as_mut_ptr() as u64,
        },
    )?;
    if info[1] == 0 {
        return Err(ValidationError::new("BPF link has no kernel identity"));
    }
    Ok(info[1])
}
