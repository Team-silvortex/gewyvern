use gewyvern::template::{
    Template, TemplateError, handshake_debug_template, udp_debug_template,
    udp_process_debug_template,
};

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
fn template_requires_program_model() {
    let mut template = handshake_debug_template();
    template.program_model = None;
    assert_eq!(template.validate(), Err(TemplateError::MissingProgramModel));
}

#[test]
fn handshake_template_is_valid() {
    let template: Template = handshake_debug_template();
    assert_eq!(template.validate(), Ok(()));
}

#[test]
fn udp_template_is_valid() {
    let template: Template = udp_debug_template();
    assert_eq!(template.validate(), Ok(()));
}

#[test]
fn udp_process_template_is_valid() {
    let template: Template = udp_process_debug_template();
    assert_eq!(template.validate(), Ok(()));
}
