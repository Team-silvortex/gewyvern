use crate::flow::{ProgramOperation, ProgramStageKind};
use crate::program::{ProgramModel, ProgramNarrative, ProgramPredicate, ProgramRule};
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
    pub program_model: Option<ProgramModel>,
}

#[derive(Debug, Eq, PartialEq)]
pub enum TemplateError {
    MissingFragmentSet,
    MissingWindowProfile,
    MissingReasonProfile,
    MissingProgramModel,
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
        if self.program_model.is_none() {
            return Err(TemplateError::MissingProgramModel);
        }
        Ok(())
    }
}

pub fn connect_flow_model() -> ProgramModel {
    ProgramModel {
        id: "connect_flow_v1",
        operation: ProgramOperation::ConnectFlow,
        rules: vec![
            ProgramRule {
                predicate: ProgramPredicate::ProcessBound,
                stage: Some(ProgramStageKind::ProcessBound),
                narrative: ProgramNarrative::ProcessBound,
                dedupe: true,
            },
            ProgramRule {
                predicate: ProgramPredicate::SocketStateObserved,
                stage: Some(ProgramStageKind::SocketStateTransition),
                narrative: ProgramNarrative::None,
                dedupe: false,
            },
            ProgramRule {
                predicate: ProgramPredicate::RouteResolved,
                stage: Some(ProgramStageKind::RouteResolved),
                narrative: ProgramNarrative::Static("program resolved a route for this network flow"),
                dedupe: true,
            },
        ],
    }
}

pub fn datagram_exchange_model() -> ProgramModel {
    ProgramModel {
        id: "datagram_exchange_v1",
        operation: ProgramOperation::DatagramExchange,
        rules: vec![
            ProgramRule {
                predicate: ProgramPredicate::ProcessBound,
                stage: Some(ProgramStageKind::ProcessBound),
                narrative: ProgramNarrative::ProcessBound,
                dedupe: true,
            },
            ProgramRule {
                predicate: ProgramPredicate::DatagramObserved { l4_proto: 17 },
                stage: Some(ProgramStageKind::DatagramObserved),
                narrative: ProgramNarrative::Static("program emitted or received a UDP datagram"),
                dedupe: true,
            },
            ProgramRule {
                predicate: ProgramPredicate::RouteResolved,
                stage: Some(ProgramStageKind::RouteResolved),
                narrative: ProgramNarrative::Static("program resolved a route for this network flow"),
                dedupe: true,
            },
        ],
    }
}

pub fn default_program_model_for_reason_profile(profile: &ReasonProfile) -> ProgramModel {
    match profile {
        ReasonProfile::HandshakeL1 => connect_flow_model(),
        ReasonProfile::UdpDatagramL1 => datagram_exchange_model(),
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
        program_model: Some(connect_flow_model()),
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
        program_model: Some(datagram_exchange_model()),
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
        program_model: Some(datagram_exchange_model()),
    }
}
