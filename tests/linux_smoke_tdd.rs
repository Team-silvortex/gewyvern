#[cfg(target_os = "linux")]
use gewyvern::loader::{LINUX_SMOKE_FRAGMENT_ID, linux_tracepoint_smoke_failures};
#[cfg(target_os = "linux")]
use gewyvern::runtime::{RuntimeSession, SessionConfig};
#[cfg(target_os = "linux")]
use gewyvern::template::{handshake_debug_template, udp_debug_template};

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Linux eBPF-capable environment"]
fn linux_tracepoint_attach_smoke() {
    let failures = linux_tracepoint_smoke_failures("syscalls/sys_enter_nanosleep").unwrap();
    assert!(failures.is_empty(), "linux attach smoke should succeed");
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Linux eBPF-capable environment"]
fn linux_tracepoint_attach_failure_becomes_structured_record() {
    let failures =
        linux_tracepoint_smoke_failures("syscalls/definitely_missing_smoke_event").unwrap();

    assert_eq!(failures.len(), 1);
    assert_eq!(failures[0].fragment_id, LINUX_SMOKE_FRAGMENT_ID);
    assert_eq!(
        failures[0].hookpoint.label(),
        "tracepoint:syscalls/definitely_missing_smoke_event"
    );
    assert!(!failures[0].error.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Linux eBPF-capable environment"]
fn runtime_session_start_can_probe_linux_loader_success() {
    let config = SessionConfig::for_template(handshake_debug_template()).unwrap();
    let session =
        RuntimeSession::start_with_linux_tracepoint_smoke(config, "syscalls/sys_enter_nanosleep")
            .unwrap();

    let export = session.export_bundle();
    assert!(export.attach_report.hookpoints_failed.is_empty());
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Linux eBPF-capable environment"]
fn runtime_session_start_can_probe_linux_loader_failure() {
    let config = SessionConfig::for_template(handshake_debug_template()).unwrap();
    let session = RuntimeSession::start_with_linux_tracepoint_smoke(
        config,
        "syscalls/definitely_missing_smoke_event",
    )
    .unwrap();

    let export = session.export_bundle();
    assert_eq!(
        export.attach_report.hookpoints_failed,
        vec![
            "linux_tracepoint_smoke_fragment@tracepoint:syscalls/definitely_missing_smoke_event"
                .to_string()
        ]
    );
    assert!(
        export
            .attach_report
            .hookpoints_attached
            .contains(&"tcp_state_fragment@tracepoint:sock/inet_sock_set_state".to_string())
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Linux eBPF-capable environment"]
fn runtime_session_can_probe_real_tcp_state_fragment_attach() {
    let mut template = handshake_debug_template();
    template.id = "tcp_state_probe".to_string();
    template.fragment_set = vec!["tcp_state_fragment".into()];
    let config = SessionConfig::for_template(template).unwrap();
    let session = RuntimeSession::start_with_linux_tracepoint_probes(config).unwrap();

    let export = session.export_bundle();
    assert!(export.attach_report.hookpoints_failed.is_empty());
    assert_eq!(
        export.attach_report.hookpoints_attached,
        vec!["tcp_state_fragment@tracepoint:sock/inet_sock_set_state".to_string()]
    );
    assert_eq!(
        export.attach_report.fragments_loaded,
        vec!["tcp_state_fragment".to_string()]
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Linux eBPF-capable environment"]
fn runtime_session_can_probe_real_route_meta_fragment_attach() {
    let mut template = handshake_debug_template();
    template.id = "route_meta_probe".to_string();
    template.fragment_set = vec!["tcp_state_fragment".into(), "route_meta_fragment".into()];
    let config = SessionConfig::for_template(template).unwrap();
    let session = RuntimeSession::start_with_linux_kernel_probes(config).unwrap();

    let export = session.export_bundle();
    assert!(export.attach_report.hookpoints_failed.is_empty());
    assert_eq!(
        export.attach_report.hookpoints_attached,
        vec![
            "tcp_state_fragment@tracepoint:sock/inet_sock_set_state".to_string(),
            "route_meta_fragment@kprobe:ip_route_output_flow".to_string()
        ]
    );
    assert_eq!(
        export.attach_report.fragments_loaded,
        vec![
            "tcp_state_fragment".to_string(),
            "route_meta_fragment".to_string()
        ]
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Linux eBPF-capable environment"]
fn runtime_session_can_probe_real_tcp_packet_meta_fragment_attach() {
    let mut template = handshake_debug_template();
    template.id = "packet_meta_probe".to_string();
    template.fragment_set = vec![
        "tcp_state_fragment".into(),
        "tcp_packet_meta_fragment".into(),
    ];
    let config = SessionConfig::for_template(template).unwrap();
    let session = RuntimeSession::start_with_linux_kernel_probes(config).unwrap();

    let export = session.export_bundle();
    assert!(export.attach_report.hookpoints_failed.is_empty());
    assert_eq!(
        export.attach_report.hookpoints_attached,
        vec![
            "tcp_state_fragment@tracepoint:sock/inet_sock_set_state".to_string(),
            "tcp_packet_meta_fragment@tc:ingress".to_string()
        ]
    );
    assert_eq!(
        export.attach_report.fragments_loaded,
        vec![
            "tcp_state_fragment".to_string(),
            "tcp_packet_meta_fragment".to_string()
        ]
    );
}

#[cfg(target_os = "linux")]
#[test]
#[ignore = "requires a Linux eBPF-capable environment"]
fn runtime_session_can_probe_real_udp_template_attach() {
    let config = SessionConfig::for_template(udp_debug_template()).unwrap();
    let session = RuntimeSession::start_with_linux_kernel_probes(config).unwrap();

    let export = session.export_bundle();
    assert!(export.attach_report.hookpoints_failed.is_empty());
    assert_eq!(
        export.attach_report.hookpoints_attached,
        vec![
            "udp_packet_meta_fragment@tc:ingress".to_string(),
            "route_meta_fragment@kprobe:ip_route_output_flow".to_string()
        ]
    );
    assert_eq!(
        export.attach_report.fragments_loaded,
        vec![
            "udp_packet_meta_fragment".to_string(),
            "route_meta_fragment".to_string()
        ]
    );
    assert_eq!(export.debug_summary.fragments_loaded, 2);
    assert!(!export.debug_summary.degraded);
}
