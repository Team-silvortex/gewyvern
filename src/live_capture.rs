//! Product capture lifecycle. No dependency on validation fixtures or Docker.
use std::path::PathBuf;

use serde::{Deserialize, Serialize};

mod fact_buffer;
pub use fact_buffer::{
    CaptureFactBuffer, FactBufferAccounting, UDP_OFFLOAD_ID_WINDOW, UDP_OFFLOAD_WAIT,
};

#[cfg(target_os = "linux")]
mod linux;
#[cfg(target_os = "linux")]
pub use linux::Collector;

#[derive(Clone, Debug)]
pub struct CaptureOptions {
    pub cgroup: PathBuf,
    pub udp_port: u16,
    pub ring_bytes: u32,
    pub max_facts: usize,
}

impl CaptureOptions {
    pub fn validate(&self) -> Result<(), CaptureError> {
        if !self.cgroup.is_absolute()
            || self.udp_port == 0
            || !(4096..=16 * 1024 * 1024).contains(&self.ring_bytes)
            || !self.ring_bytes.is_power_of_two()
            || !(1..=65536).contains(&self.max_facts)
        {
            return Err(CaptureError::new(
                "capture_options_invalid",
                "absolute cgroup, nonzero UDP port, power-of-two 4KiB..16MiB ring and 1..65536 facts required",
            ));
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct CaptureError {
    pub code: &'static str,
    pub detail: String,
}
impl CaptureError {
    pub fn new(code: &'static str, detail: impl Into<String>) -> Self {
        Self {
            code,
            detail: detail.into(),
        }
    }
}
impl std::fmt::Display for CaptureError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.code, self.detail)
    }
}
impl std::error::Error for CaptureError {}
impl From<std::io::Error> for CaptureError {
    fn from(error: std::io::Error) -> Self {
        Self::new("capture_io_failed", error.to_string())
    }
}

// Decimal strings retain exact u64 counts in JavaScript/UI consumers.
mod decimal {
    use serde::{Deserialize, Deserializer, Serializer};
    pub fn serialize<S: Serializer>(value: &u64, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&value.to_string())
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
        let text = String::deserialize(deserializer)?;
        let value = text.parse::<u64>().map_err(serde::de::Error::custom)?;
        if value.to_string() != text {
            return Err(serde::de::Error::custom("noncanonical counter"));
        }
        Ok(value)
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KernelCounters {
    #[serde(with = "decimal")]
    pub matched: u64,
    #[serde(with = "decimal")]
    pub ring_dropped: u64,
    #[serde(with = "decimal")]
    pub unsupported: u64,
    #[serde(with = "decimal")]
    pub malformed: u64,
    #[serde(with = "decimal")]
    pub no_cookie: u64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CaptureSnapshot {
    pub schema_version: u8,
    pub run_id: String,
    pub cgroup: String,
    pub udp_port: u16,
    pub state: CaptureState,
    pub kernel: KernelCounters,
    #[serde(with = "decimal")]
    pub received: u64,
    #[serde(with = "decimal")]
    pub retained: u64,
    #[serde(with = "decimal")]
    pub userspace_dropped: u64,
    #[serde(with = "decimal")]
    pub decode_rejected: u64,
    pub final_accounted: bool,
    pub incomplete: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CaptureState {
    Running,
    Stopped,
    Failed,
}

impl CaptureSnapshot {
    pub fn reconciles(&self) -> bool {
        self.received.checked_add(self.kernel.ring_dropped) == Some(self.kernel.matched)
            && self
                .retained
                .checked_add(self.userspace_dropped)
                .and_then(|n| n.checked_add(self.decode_rejected))
                == Some(self.received)
    }
    pub fn has_loss(&self) -> bool {
        self.kernel.ring_dropped != 0
            || self.kernel.unsupported != 0
            || self.kernel.malformed != 0
            || self.kernel.no_cookie != 0
            || self.userspace_dropped != 0
            || self.decode_rejected != 0
    }
}

/// Only UDP packet evidence is actually attached. Do not advertise route,
/// lineage, TCP state or QUIC observations that this producer cannot supply.
pub fn session_config(
    mut config: crate::runtime::SessionConfig,
    options: &CaptureOptions,
) -> Result<crate::runtime::SessionConfig, CaptureError> {
    options.validate()?;
    use crate::fragment::{AttachFailure, FragmentRegistry, HookPoint, MapKind, MapSpec};
    use crate::ledger::FactKindTag;
    if !config
        .template
        .fragment_set
        .iter()
        .any(|id| id == "udp_packet_meta_fragment")
    {
        return Err(CaptureError::new(
            "capture_template_incompatible",
            "capture requires a UDP packet template",
        ));
    }
    let mut registry = FragmentRegistry::new();
    for id in &config.template.fragment_set {
        let mut descriptor = config
            .registry
            .descriptor(id)
            .ok_or_else(|| CaptureError::new("capture_fragment_missing", id))?
            .clone();
        if id == "udp_packet_meta_fragment" {
            descriptor.hookpoints = vec![HookPoint::CgroupSkbIngress, HookPoint::CgroupSkbEgress];
            descriptor.sampled_payload_offsets = (0..128).collect();
            descriptor
                .emits
                .retain(|kind| *kind == FactKindTag::PacketMeta);
            descriptor
                .evidence_classes
                .retain(|item| item.fact_kind == FactKindTag::PacketMeta);
            descriptor.maps = vec![MapSpec {
                name: "events".into(),
                kind: MapKind::RingBuf,
                max_entries: options.ring_bytes,
            }];
        } else {
            config
                .attach_failures
                .extend(descriptor.hookpoints.iter().map(|hookpoint| AttachFailure {
                    fragment_id: id.clone(),
                    hookpoint: hookpoint.clone(),
                    error: "not supplied by live UDP capture".into(),
                }));
        }
        registry
            .register(descriptor)
            .map_err(|error| CaptureError::new("capture_registry_invalid", format!("{error:?}")))?;
    }
    config.registry = registry;
    Ok(config)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn capture_registry_advertises_only_the_real_udp_surface() {
        let options = CaptureOptions {
            cgroup: "/sys/fs/cgroup/test".into(),
            udp_port: 53,
            ring_bytes: 8192,
            max_facts: 16,
        };
        let config =
            crate::runtime::SessionConfig::for_template(crate::template::udp_debug_template())
                .unwrap();
        let config = session_config(config, &options).unwrap();
        let udp = config
            .registry
            .descriptor("udp_packet_meta_fragment")
            .unwrap();
        assert_eq!(udp.maps[0].max_entries, 8192);
        assert_eq!(udp.sampled_payload_offsets, (0..128).collect::<Vec<_>>());
        assert_eq!(udp.emits, vec![crate::ledger::FactKindTag::PacketMeta]);
        for id in &config.template.fragment_set {
            if id != "udp_packet_meta_fragment" {
                let descriptor = config.registry.descriptor(id).unwrap();
                for hook in &descriptor.hookpoints {
                    assert!(config.attach_failures.iter().any(|failure|
                        &failure.fragment_id == id && &failure.hookpoint == hook));
                }
            }
        }
        let tcp = crate::runtime::SessionConfig::for_template(
            crate::template::handshake_debug_template(),
        )
        .unwrap();
        assert_eq!(
            session_config(tcp, &options).unwrap_err().code,
            "capture_template_incompatible"
        );
    }

    #[test]
    fn options_bound_memory_and_scope() {
        let mut opts = CaptureOptions {
            cgroup: "/sys/fs/cgroup/test".into(),
            udp_port: 53,
            ring_bytes: 4096,
            max_facts: 1,
        };
        assert!(opts.validate().is_ok());
        opts.ring_bytes = 8193;
        assert!(opts.validate().is_err());
        opts.ring_bytes = 4096;
        opts.max_facts = 0;
        assert!(opts.validate().is_err());
        opts.max_facts = 1;
        opts.udp_port = 0;
        assert!(opts.validate().is_err());
    }
    #[test]
    fn kernel_and_userspace_loss_are_separate_and_exact() {
        let mut s = CaptureSnapshot {
            schema_version: 1,
            run_id: "test".into(),
            cgroup: "/test".into(),
            udp_port: 53,
            state: CaptureState::Stopped,
            kernel: KernelCounters {
                matched: 10,
                ring_dropped: 2,
                ..Default::default()
            },
            received: 8,
            retained: 5,
            userspace_dropped: 2,
            decode_rejected: 1,
            final_accounted: true,
            incomplete: true,
        };
        assert!(s.reconciles() && s.has_loss());
        s.retained += 1;
        assert!(!s.reconciles());
        s.received = u64::MAX;
        assert!(!s.reconciles());
        let json = serde_json::to_string(&s).unwrap();
        assert_eq!(serde_json::from_str::<CaptureSnapshot>(&json).unwrap(), s);
    }
}
