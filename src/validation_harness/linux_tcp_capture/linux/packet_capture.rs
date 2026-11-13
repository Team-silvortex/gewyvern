use std::fs;
use std::os::fd::OwnedFd;
use std::path::{Component, Path, PathBuf};
use std::process::Command;

use super::super::packet_evidence::PacketEvent;
use super::bpf_link;
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
    pub(super) fn path(&self) -> &Path {
        &self.owned
    }
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
    pub(super) fn detach(&mut self) {
        self.link.take();
    }
    pub(super) fn stream_map(&self) -> Result<OwnedFd, ValidationError> {
        bpf_link::open_pinned(&self.pins.0.join("maps/events"))
    }
    pub(super) fn stream_counts<T: serde::de::DeserializeOwned>(
        &self,
        out: &Path,
    ) -> Result<T, ValidationError> {
        let bytes = map_dump(&self.pins, "counts", out)?;
        fs::write(out.join("stream-counts-raw.json"), &bytes)?;
        let mut entries = map_entries::<T>(&bytes)?;
        if entries.len() != 1 || entries[0].key != 0 {
            return Err(ValidationError::new("invalid stream capture counters"));
        }
        Ok(entries.remove(0).value)
    }
    pub(super) fn sample_counts(&self, out: &Path) -> Result<Counts, ValidationError> {
        packet_counts(&self.pins, out)
    }
    pub(super) fn path(&self) -> &Path {
        &self.pins.0
    }
    pub(super) fn link_id(&self) -> Result<u32, ValidationError> {
        bpf_link::id(
            self.link
                .as_ref()
                .ok_or_else(|| ValidationError::new("packet capture is detached"))?,
        )
    }
    pub(super) fn remove_event_for_validation(
        &mut self,
        out: &Path,
    ) -> Result<(), ValidationError> {
        self.link.take();
        tool(
            Command::new("bpftool")
                .args(["map", "delete", "pinned"])
                .arg(self.pins.0.join("maps/events"))
                .args(["key", "hex", "00", "00", "00", "00"]),
            out,
        )?;
        Ok(())
    }
    pub(super) fn attach(
        scope: &Scope,
        expected: &[(u64, u16, u16, u32); 2],
        addresses: ([u8; 4], [u8; 4]),
        out: &Path,
    ) -> Result<Self, ValidationError> {
        Self::attach_with_capacity(scope, expected, addresses, out, 128)
    }

    pub(super) fn attach_with_capacity(
        scope: &Scope,
        expected: &[(u64, u16, u16, u32); 2],
        addresses: ([u8; 4], [u8; 4]),
        out: &Path,
        capacity: u32,
    ) -> Result<Self, ValidationError> {
        if !(1..=128).contains(&capacity) {
            return Err(ValidationError::new("invalid validation map capacity"));
        }
        Self::load(scope, expected, addresses, out, capacity, false, false)
    }

    pub(super) fn attach_data_breadth(
        scope: &Scope,
        expected: &[(u64, u16, u16, u32); 2],
        addresses: ([u8; 4], [u8; 4]),
        out: &Path,
    ) -> Result<Self, ValidationError> {
        Self::load(scope, expected, addresses, out, 128, false, true)
    }

    pub(super) fn attach_stream(
        scope: &Scope,
        expected: &[(u64, u16, u16, u32); 2],
        out: &Path,
    ) -> Result<Self, ValidationError> {
        Self::load(
            scope,
            expected,
            ([127, 0, 0, 1], [127, 0, 0, 1]),
            out,
            128,
            true,
            false,
        )
    }

    fn load(
        scope: &Scope,
        expected: &[(u64, u16, u16, u32); 2],
        addresses: ([u8; 4], [u8; 4]),
        out: &Path,
        capacity: u32,
        streaming: bool,
        data_breadth: bool,
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
                .arg(format!("-DREMOTE_PORT_A={}", expected[0].2))
                .arg(format!("-DREMOTE_PORT_B={}", expected[1].2))
                .arg(format!("-DCAPTURE_MAP_CAPACITY={capacity}"))
                .args(if data_breadth {
                    vec!["-DCAPTURE_DATA_BREADTH=1"]
                } else {
                    vec![]
                })
                .args(if streaming {
                    vec!["-DCAPTURE_STREAMING=1"]
                } else {
                    vec![]
                })
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
                .args(vec!["00"; if streaming { 24 } else { 16 }]),
            out,
        )?;
        let link = bpf_link::attach_cgroup(&program, &scope.owned)?;
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
        // Refuse known loss before dumping a full map or constructing facts.
        if let Some(code) = count_rejection(before) {
            return reject(out, code, before, None);
        }
        let bytes = map_dump(&self.pins, "events", out)?;
        fs::write(out.join("packet-prefix-raw.json"), &bytes)?;
        let after = packet_counts(&self.pins, out)?;
        if before != after {
            return reject(out, "unstable_snapshot", after, None);
        }
        let mut events = Vec::new();
        for entry in map_entries::<PacketEvent>(&bytes)? {
            if entry.key >= 128 || entry.value.sequence != u64::from(entry.key) + 1 {
                return reject(out, "invalid_slot", after, Some(events.len()));
            }
            events.push(entry.value);
        }
        events.sort_by_key(|e| e.sequence);
        if events.len() as u64 != after.total {
            return reject(out, "snapshot_incomplete", after, Some(events.len()));
        }
        self.pins.cleanup()?;
        Ok((events, after))
    }
}

fn count_rejection(counts: Counts) -> Option<&'static str> {
    if counts.total > 128 {
        Some("capacity_exceeded")
    } else if counts.dropped > 0 {
        Some("reported_loss")
    } else if counts.total == 0 {
        Some("empty_capture")
    } else {
        None
    }
}

fn reject(
    out: &Path,
    code: &str,
    counts: Counts,
    observed: Option<usize>,
) -> Result<(Vec<PacketEvent>, Counts), ValidationError> {
    fs::write(
        out.join("packet-rejection.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "schema_version":1,"accepted":false,"code":code,"counts":counts,"observed_entries":observed,
            "packet_export_produced":false
        }))?,
    )?;
    Err(ValidationError::new(format!(
        "packet capture rejected: {code}"
    )))
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
    #[test]
    fn loss_and_capacity_are_distinct_from_an_exactly_full_capture() {
        assert_eq!(
            count_rejection(Counts {
                total: 128,
                dropped: 0
            }),
            None
        );
        assert_eq!(
            count_rejection(Counts {
                total: 129,
                dropped: 1
            }),
            Some("capacity_exceeded")
        );
        assert_eq!(
            count_rejection(Counts {
                total: 6,
                dropped: 2
            }),
            Some("reported_loss")
        );
        assert_eq!(
            count_rejection(Counts {
                total: 0,
                dropped: 0
            }),
            Some("empty_capture")
        );
    }
}
