use gewyvern::fragment::EvidenceTier;
use gewyvern::ledger::FactKindTag;
use gewyvern::template::{
    FragmentParamValue, Template, TemplateError, handshake_debug_template, udp_debug_template,
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
    assert_eq!(
        template.validate(),
        Err(TemplateError::MissingWindowProfile)
    );
}

#[test]
fn template_requires_reason_profile() {
    let mut template = handshake_debug_template();
    template.reason_profile = None;
    assert_eq!(
        template.validate(),
        Err(TemplateError::MissingReasonProfile)
    );
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

#[test]
fn template_can_compile_into_binding_with_fragment_params() {
    let binding = udp_process_debug_template()
        .bind()
        .with_fragment_param(
            "udp_packet_meta_fragment",
            "l4_proto_hint",
            FragmentParamValue::U64(17),
        )
        .with_fragment_param(
            "sock_lineage_fragment",
            "capture_comm",
            FragmentParamValue::Bool(true),
        );

    assert_eq!(binding.validate(), Ok(()));
    assert_eq!(
        binding.fragment_params["udp_packet_meta_fragment"]["l4_proto_hint"],
        FragmentParamValue::U64(17)
    );
    assert_eq!(
        binding.fragment_params["sock_lineage_fragment"]["capture_comm"],
        FragmentParamValue::Bool(true)
    );
}

#[test]
fn template_binding_can_override_evidence_tiers() {
    let binding = udp_process_debug_template()
        .bind()
        .with_evidence_tier(FactKindTag::SockLineage, EvidenceTier::CoreRequirement);

    assert_eq!(binding.validate(), Ok(()));
    assert_eq!(
        binding.evidence_overrides.get(&FactKindTag::SockLineage),
        Some(&EvidenceTier::CoreRequirement)
    );
}
