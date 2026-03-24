use crate::reason::{ReasonProfile, ReasonProfile::HandshakeL1};

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

#[cfg(test)]
mod tests {
    use super::{handshake_debug_template, Template, TemplateError};

    #[test]
    fn template_requires_fragment_set() {
        let mut template = handshake_debug_template();
        template.fragment_set.clear();
        assert_eq!(template.validate(), Err(TemplateError::MissingFragmentSet));
    }

    #[test]
    fn template_requires_window_profile() {
        let mut template = handshake_debug_template();
        template.window_profile = None;
        assert_eq!(template.validate(), Err(TemplateError::MissingWindowProfile));
    }

    #[test]
    fn template_requires_reason_profile() {
        let mut template = handshake_debug_template();
        template.reason_profile = None;
        assert_eq!(template.validate(), Err(TemplateError::MissingReasonProfile));
    }

    #[test]
    fn handshake_template_is_valid() {
        let template: Template = handshake_debug_template();
        assert_eq!(template.validate(), Ok(()));
    }
}
