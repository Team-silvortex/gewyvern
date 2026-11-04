mod support;

use gewyvern::export::ExportBundle;
use gewyvern::flow::{ModuleSeverity, ProgramFindingCause, ProgramOperation};
use gewyvern::fragment::{AttachFailure, HookPoint, builtin_registry};
use gewyvern::ledger::FactKind;
use gewyvern::loader::StaticFailureLoader;
use gewyvern::program::{ProgramModel, ProgramNarrative, ProgramPredicate, ProgramRule};
use gewyvern::runtime::{
    RejectedFactReason, RuntimeError, RuntimeSession, SessionConfig, build_flow_snapshots,
};
use gewyvern::template::{
    FragmentParamValue, TemplateError, handshake_debug_template, udp_debug_template,
    udp_process_debug_template,
};
use std::collections::BTreeMap;
use std::time::{Duration, SystemTime};
use support::{
    packet_fact, route_fact, run_handshake_session, run_udp_process_session, run_udp_session,
    sock_lineage_fact, tcp_state_fact, udp_packet_fact,
};

#[path = "runtime_tdd/part_001.rs"]
mod part_001;
#[path = "runtime_tdd/part_002.rs"]
mod part_002;
#[path = "runtime_tdd/performance.rs"]
mod performance;

#[test]
fn tcp_lifecycle_uses_state_meaning_not_numeric_order() {
    for state in 1..=12 {
        let facts = [tcp_state_fact(1, 42, 1, state)];
        let flows = build_flow_snapshots(&facts);
        assert_eq!(
            flows[0].lifecycle.terminated,
            matches!(state, 6 | 7),
            "state {state}"
        );
        assert_eq!(
            flows[0].lifecycle.termination_fact.is_some(),
            matches!(state, 6 | 7)
        );
    }
}

#[test]
fn a_socket_reconnecting_after_close_does_not_remain_terminated() {
    let facts = [
        tcp_state_fact(1, 42, 2, 7),
        tcp_state_fact(2, 42, 7, 2),
        tcp_state_fact(3, 42, 2, 1),
    ];
    let flows = build_flow_snapshots(&facts);
    assert_eq!(flows.len(), 1);
    assert!(!flows[0].lifecycle.terminated);
    assert_eq!(flows[0].lifecycle.termination_fact, None);
}

#[test]
fn runtime_start_revalidates_public_session_config_without_panicking() {
    for (template, expected) in [
        (
            {
                let mut template = handshake_debug_template();
                template.window_profile = None;
                template
            },
            TemplateError::MissingWindowProfile,
        ),
        (
            {
                let mut template = handshake_debug_template();
                template.reason_profile = None;
                template
            },
            TemplateError::MissingReasonProfile,
        ),
        (
            {
                let mut template = handshake_debug_template();
                template.program_model = None;
                template
            },
            TemplateError::MissingProgramModel,
        ),
    ] {
        let result = RuntimeSession::start(SessionConfig {
            template,
            registry: builtin_registry(),
            attach_failures: Vec::new(),
            fragment_params: BTreeMap::new(),
            evidence_overrides: BTreeMap::new(),
        });

        let error = result.expect_err("invalid public config must be rejected");
        assert_eq!(error, RuntimeError::InvalidTemplate(expected));
        assert_eq!(error.code(), "runtime_template_invalid");
        assert_eq!(
            error.category(),
            gewyvern::machine_error::ErrorCategory::Input
        );
        assert!(!error.retryable());
        let machine_error = gewyvern::machine_error::MachineError::from(error);
        assert_eq!(machine_error.code, "runtime_template_invalid");
        assert_eq!(
            machine_error.category,
            gewyvern::machine_error::ErrorCategory::Input
        );
        assert_eq!(machine_error.exit_code, 2);
    }
}
