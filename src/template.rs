use crate::reason::{ReasonProfile, ReasonProfile::{HandshakeL1, UdpDatagramL1}};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WindowProfile {
    pub id: &'static str,
    pub duration_ms: u64,
    pub lateness_ms: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Template {
    pub id: &'static str,
    pub fragment_set: Vec<&'static str>,
    pub window_profile: Option<WindowProfile>,
    pub reason_profile: Option<ReasonProfile>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateError {
    MissingFragmentSet,
    MissingWindowProfile,
    MissingReasonProfile,
}

impl Template {
    pub fn validate(&self) -> Result<(), TemplateError> {
        if self.fragment_set.is_empty() {
            return Err(TemplateError::MissingFragmentSet);
        }
        if self.window_profile.is_none() {
            return Err(TemplateError::MissingWindowProfile);
        }
        if self.reason_profile.is_none() {
            return Err(TemplateError::MissingReasonProfile);
        }
        Ok(())
    }
}

pub fn default_5s_window() -> WindowProfile {
    WindowProfile {
        id: "default_5s",
        duration_ms: 5_000,
        lateness_ms: 200,
    }
}

pub fn handshake_debug_template() -> Template {
    Template {
        id: "handshake_debug",
        fragment_set: vec![
            "tcp_state_fragment",
            "tcp_packet_meta_fragment",
            "route_meta_fragment",
        ],
        window_profile: Some(default_5s_window()),
        reason_profile: Some(HandshakeL1),
    }
}

pub fn udp_debug_template() -> Template {
    Template {
        id: "udp_debug",
        fragment_set: vec![
            "udp_packet_meta_fragment",
            "route_meta_fragment",
        ],
        window_profile: Some(default_5s_window()),
        reason_profile: Some(UdpDatagramL1),
    }
}

pub fn udp_process_debug_template() -> Template {
    Template {
        id: "udp_process_debug",
        fragment_set: vec![
            "udp_packet_meta_fragment",
            "route_meta_fragment",
            "sock_lineage_fragment",
        ],
        window_profile: Some(default_5s_window()),
        reason_profile: Some(UdpDatagramL1),
    }
}
