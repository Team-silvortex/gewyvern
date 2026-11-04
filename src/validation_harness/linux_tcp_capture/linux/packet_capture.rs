use std::ffi::CString;
use std::fs::{self, File};
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use super::super::packet_evidence::PacketEvent;
use super::{Counts, Pins, ValidationError, map_dump, map_entries, repo_root, tool};

pub(super) struct Scope {
    original: PathBuf,
    owned: PathBuf,
    moved: bool,
}

fn original_cgroup(input: &str) -> Result<PathBuf, ValidationError> {
    let value = input
        .strip_prefix("0::")
        .map(str::trim_end)
        .filter(|value| value.starts_with('/') && !value.contains('\n'))
        .ok_or_else(|| ValidationError::new("capture requires a unified cgroup v2 hierarchy"))?;
    if Path::new(value)
        .components()
        .any(|part| !matches!(part, Component::RootDir | Component::Normal(_)))
    {
        return Err(ValidationError::new("unsafe original cgroup path"));
    }
    Ok(Path::new("/sys/fs/cgroup").join(value.trim_start_matches('/')))
}

impl Scope {
    pub(super) fn enter(token: &str) -> Result<Self, ValidationError> {
        if token.is_empty() || !token.bytes().all(|b| b.is_ascii_digit() || b == b'-') {
            return Err(ValidationError::new("invalid capture cgroup token"));
        }
        let original = original_cgroup(&fs::read_to_string("/proc/self/cgroup")?)?;
        let owned = Path::new("/sys/fs/cgroup").join(format!("gewyvern-capture-{token}"));
        fs::create_dir(&owned)?;
        let mut scope = Self {
            original,
            owned,
            moved: false,
        };
        fs::write(
            scope.owned.join("cgroup.procs"),
            std::process::id().to_string(),
        )?;
        scope.moved = true;
        Ok(scope)
    }

    pub(super) fn cleanup(&mut self) -> Result<(), ValidationError> {
        if self.moved {
            fs::write(
                self.original.join("cgroup.procs"),
                std::process::id().to_string(),
            )?;
            self.moved = false;
        }
        if self.owned.exists() {
            fs::remove_dir(&self.owned)?;
        }
        Ok(())
    }
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Err(error) = self.cleanup() {
            eprintln!("capture cgroup cleanup failed: {error}");
        }
    }
}

// Stable Linux UAPI, with zeroed trailing fields. Link ownership is an fd, so
// process exit detaches it; unlike legacy cgroup attach, no permanent hook remains.
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

fn bpf_fd<T>(command: u32, attr: &T) -> Result<OwnedFd, ValidationError> {
    // SAFETY: callers provide initialized repr(C) Linux UAPI attributes, and all
    // referenced buffers/fds outlive this synchronous syscall. Success owns one fd.
    let fd = unsafe { libc::syscall(libc::SYS_bpf, command, attr as *const T, size_of::<T>()) };
    if fd < 0 {
        return Err(std::io::Error::last_os_error().into());
    }
    // SAFETY: the successful BPF syscall returned a new, uniquely owned fd.
    Ok(unsafe { OwnedFd::from_raw_fd(fd as i32) })
}

pub(super) struct Capture {
    link: Option<OwnedFd>,
    pins: Pins,
}

fn packet_counts(pins: &Pins, out: &Path) -> Result<Counts, ValidationError> {
    let bytes = map_dump(pins, "counts", out)?;
    fs::write(out.join("packet-counts-raw.json"), &bytes)?;
    let entries = map_entries::<Counts>(&bytes)?;
    match entries.as_slice() {
        [entry] if entry.key == 0 => Ok(entry.value),
        _ => Err(ValidationError::new("invalid packet capture counters")),
    }
}
impl Capture {
    pub(super) fn attach(
        scope: &Scope,
        expected: &[(u64, u16, u16, u32); 2],
        addresses: ([u8; 4], [u8; 4]),
        out: &Path,
    ) -> Result<Self, ValidationError> {
        let pins = Pins::new()?;
        let object = out.join("http-prefix.bpf.o");
        tool(
            Command::new("clang")
                .args(["-O2", "-g", "-target", "bpf", "-I/usr/include"])
                .arg(format!(
                    "-I/usr/include/{}-linux-gnu",
                    std::env::consts::ARCH
                ))
                .arg(format!("-DCOOKIE_A={}ULL", expected[0].0))
                .arg(format!("-DCOOKIE_B={}ULL", expected[1].0))
                .arg(format!("-DPORT_A={}", expected[0].1))
                .arg(format!("-DPORT_B={}", expected[1].1))
                .arg(format!(
                    "-DSOURCE_IPV4={}U",
                    u32::from_be_bytes(addresses.0)
                ))
                .arg(format!(
                    "-DDESTINATION_IPV4={}U",
                    u32::from_be_bytes(addresses.1)
                ))
                .arg("-c")
                .arg(repo_root().join("ebpf/validation/http_prefix.bpf.c"))
                .arg("-o")
                .arg(&object),
            out,
        )?;
        let program = pins.0.join("program");
        tool(
            Command::new("bpftool")
                .args(["prog", "load"])
                .arg(&object)
                .arg(&program)
                .arg("pinmaps")
                .arg(pins.0.join("maps")),
            out,
        )?;
        tool(
            Command::new("bpftool")
                .args(["map", "update", "pinned"])
                .arg(pins.0.join("maps/counts"))
                .args(["key", "hex", "00", "00", "00", "00", "value", "hex"])
                .args(["00"; 16]),
            out,
        )?;
        let path = CString::new(program.as_os_str().as_bytes())
            .map_err(|e| ValidationError::new(e.to_string()))?;
        let prog = bpf_fd(
            7,
            &ObjectGet {
                pathname: path.as_ptr() as u64,
                bpf_fd: 0,
                file_flags: 0,
            },
        )?;
        let cgroup = File::open(&scope.owned)?;
        let link = bpf_fd(
            28,
            &LinkCreate {
                prog_fd: prog.as_raw_fd() as u32,
                target_fd: cgroup.as_raw_fd() as u32,
                attach_type: 0,
                flags: 0,
            },
        )?;
        Ok(Self {
            link: Some(link),
            pins,
        })
    }

    pub(super) fn finish(
        mut self,
        out: &Path,
    ) -> Result<(Vec<PacketEvent>, Counts), ValidationError> {
        self.link.take();
        let before = packet_counts(&self.pins, out)?;
        let bytes = map_dump(&self.pins, "events", out)?;
        fs::write(out.join("packet-prefix-raw.json"), &bytes)?;
        let after = packet_counts(&self.pins, out)?;
        if before != after || after.total == 0 || after.total > 128 || after.dropped != 0 {
            return Err(ValidationError::new(
                "incomplete or changing packet capture",
            ));
        }
        let mut events = Vec::new();
        for entry in map_entries::<PacketEvent>(&bytes)? {
            if entry.key >= 128 || entry.value.sequence != u64::from(entry.key) + 1 {
                return Err(ValidationError::new("packet capture slot mismatch"));
            }
            events.push(entry.value);
        }
        events.sort_by_key(|e| e.sequence);
        if events.len() as u64 != after.total {
            return Err(ValidationError::new("incomplete packet snapshot"));
        }
        self.pins.cleanup()?;
        Ok((events, after))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cgroup_path_is_confined_to_unified_mount() {
        assert_eq!(
            original_cgroup("0::/user.slice/test.scope\n").unwrap(),
            Path::new("/sys/fs/cgroup/user.slice/test.scope")
        );
        for input in ["1:cpu:/x", "0::/../etc", "0::relative", "0::/x\n0::/y"] {
            assert!(original_cgroup(input).is_err(), "{input}");
        }
    }
}
