//! Opt-in, bounded Linux UDP collection for an installed service.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct UdpCapturePlan {
    pub schema_version: u32,
    pub cgroup: String,
    pub udp_port: u16,
    pub window_ms: u64,
    pub ring_kib: u32,
    pub max_facts: u32,
}

impl UdpCapturePlan {
    pub fn new(cgroup: impl Into<String>, udp_port: u16) -> Self {
        Self {
            schema_version: 1,
            cgroup: cgroup.into(),
            udp_port,
            window_ms: 1000,
            ring_kib: 1024,
            max_facts: 4096,
        }
    }

    pub fn validate(&self) -> Result<(), &'static str> {
        let scope = self.cgroup.strip_prefix("/sys/fs/cgroup/");
        if self.schema_version != 1
            || self.cgroup.len() > 4096
            || self.cgroup.chars().any(|c| c.is_control() || c == '\\')
            || scope.is_none_or(|suffix| {
                suffix
                    .split('/')
                    .any(|part| part.is_empty() || part == "." || part == "..")
            })
            || self.udp_port == 0
            || !(10..=60_000).contains(&self.window_ms)
            || !(4..=16_384).contains(&self.ring_kib)
            || !self.ring_kib.is_power_of_two()
            || !(1..=65_536).contains(&self.max_facts)
        {
            return Err("invalid bounded Linux UDP capture plan");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn scope_and_budgets_are_explicit_and_bounded() {
        let plan = UdpCapturePlan::new("/sys/fs/cgroup/example.service", 53);
        assert!(plan.validate().is_ok());
        for cgroup in [
            "/sys/fs/cgroup",
            "/sys/fs/cgroup/",
            "/tmp/test",
            "/sys/fs/cgroup/a/../b",
            "/sys/fs/cgroup/a//b",
            "/sys/fs/cgroup/a\n",
        ] {
            assert!(UdpCapturePlan::new(cgroup, 53).validate().is_err());
        }
        let mut invalid = plan.clone();
        invalid.udp_port = 0;
        assert!(invalid.validate().is_err());
        invalid = plan.clone();
        invalid.ring_kib = 5;
        assert!(invalid.validate().is_err());
        invalid = plan.clone();
        invalid.max_facts = 0;
        assert!(invalid.validate().is_err());
        invalid = plan.clone();
        invalid.window_ms = 0;
        assert!(invalid.validate().is_err());
        let mut value = serde_json::to_value(&plan).unwrap();
        value["command"] = serde_json::json!("ignored?");
        assert!(serde_json::from_value::<UdpCapturePlan>(value).is_err());
    }
}
