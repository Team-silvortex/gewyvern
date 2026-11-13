use std::fs;
use std::os::fd::OwnedFd;
use std::path::Path;
use std::process::Command;

use super::super::super::udp_evidence::Datagram;
use super::super::packet_capture::Scope;
use super::super::{Pins, bpf_link, map_dump, map_entries, repo_root, tool};
use super::{Counts, ValidationError};

pub(super) struct Capture {
    link: Option<OwnedFd>,
    pins: Pins,
}

impl Capture {
    pub fn attach(
        scope: &Scope,
        expected: &[(u64, u16); 2],
        addresses: ([u8; 4], [u8; 4]),
        egress: bool,
        out: &Path,
    ) -> Result<Self, ValidationError> {
        let pins = Pins::new()?;
        let object = out.join("udp-dns.bpf.o");
        let mut compiler = Command::new("clang");
        compiler
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
            ));
        if egress {
            compiler.arg("-DCAPTURE_EGRESS=1");
        }
        tool(
            compiler
                .arg("-c")
                .arg(repo_root().join("ebpf/validation/udp_dns.bpf.c"))
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
        let link = if egress {
            bpf_link::attach_cgroup_egress(&program, scope.path())?
        } else {
            bpf_link::attach_cgroup(&program, scope.path())?
        };
        Ok(Self {
            link: Some(link),
            pins,
        })
    }
    pub fn id(&self) -> Result<u32, ValidationError> {
        bpf_link::id(
            self.link
                .as_ref()
                .ok_or_else(|| ValidationError::new("UDP capture detached"))?,
        )
    }
    fn counts(&self, out: &Path) -> Result<Counts, ValidationError> {
        let bytes = map_dump(&self.pins, "counts", out)?;
        fs::write(out.join("counts-raw.json"), &bytes)?;
        let entries = map_entries::<Counts>(&bytes)?;
        match entries.as_slice() {
            [entry] if entry.key == 0 => Ok(entry.value),
            _ => Err(ValidationError::new("invalid UDP capture counter map")),
        }
    }
    pub fn finish(mut self, out: &Path) -> Result<(Vec<Datagram>, Counts), ValidationError> {
        self.link.take();
        let counts = self.counts(out)?;
        if counts.total == 0 || counts.total > 128 || counts.dropped != 0 {
            return Err(ValidationError::new(
                "UDP capture empty, over capacity or reports loss",
            ));
        }
        let bytes = map_dump(&self.pins, "events", out)?;
        fs::write(out.join("events-raw.json"), &bytes)?;
        let mut events = Vec::new();
        for entry in map_entries::<Datagram>(&bytes)? {
            if entry.key >= 128 || entry.value.sequence != u64::from(entry.key) + 1 {
                return Err(ValidationError::new("invalid UDP event slot"));
            }
            events.push(entry.value);
        }
        events.sort_by_key(|event| event.sequence);
        if counts != self.counts(out)?
            || events.len() as u64 != counts.total
            || events
                .iter()
                .enumerate()
                .any(|(i, e)| e.sequence != i as u64 + 1)
        {
            return Err(ValidationError::new("UDP snapshot incomplete or unstable"));
        }
        self.pins.cleanup()?;
        Ok((events, counts))
    }
}
