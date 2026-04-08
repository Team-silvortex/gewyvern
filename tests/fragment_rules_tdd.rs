use gewyvern::fragment::{
    AttachFailure, CapabilityFlag, FragmentDescriptor, FragmentRegistry, HookPoint, MapKind,
    MapSpec, RegistryError, builtin_registry,
};
use gewyvern::ledger::FactKindTag;

#[test]
fn builtin_handshake_plan_has_full_coverage() {
    let registry = builtin_registry();
    let plan = registry
        .plan([
            "tcp_state_fragment",
            "tcp_packet_meta_fragment",
            "route_meta_fragment",
        ])
        .unwrap();

    assert!(plan.coverage.missing.is_empty());
    assert_eq!(plan.fragments.len(), 3);
    assert_eq!(plan.hook_graph.len(), 3);
}

#[test]
fn registry_rejects_hookpoint_conflicts() {
    let mut registry = FragmentRegistry::new();
    let first = test_fragment("a", HookPoint::TCIngress, FactKindTag::TcpState, vec![]);
    let second = test_fragment("b", HookPoint::TCIngress, FactKindTag::PacketMeta, vec![]);
    registry.register(first).unwrap();
    registry.register(second).unwrap();

    let err = registry.plan(["a", "b"]).unwrap_err();
    assert!(matches!(err, RegistryError::HookConflict(_)));
}

#[test]
fn registry_rejects_missing_required_fact_coverage() {
    let mut registry = FragmentRegistry::new();
    let fragment = test_fragment(
        "needs_route",
        HookPoint::TCEgress,
        FactKindTag::PacketMeta,
        vec![FactKindTag::RouteDecision],
    );
    registry.register(fragment).unwrap();

    let err = registry.plan(["needs_route"]).unwrap_err();
    assert_eq!(
        err,
        RegistryError::MissingCoverage(vec![FactKindTag::RouteDecision])
    );
}

#[test]
fn attach_report_tracks_failed_hookpoints_separately() {
    let registry = builtin_registry();
    let plan = registry
        .plan([
            "tcp_state_fragment",
            "tcp_packet_meta_fragment",
            "route_meta_fragment",
        ])
        .unwrap();

    let report = registry.attach_report_with_failures(
        &plan,
        ["route_meta_fragment@kprobe:ip_route_output_flow".to_string()],
    );

    assert_eq!(report.hookpoints_failed.len(), 1);
    assert_eq!(
        report.hookpoints_failed,
        vec!["route_meta_fragment@kprobe:ip_route_output_flow".to_string()]
    );
    assert_eq!(report.hookpoints_attached.len(), 2);
    assert!(!report
        .hookpoints_attached
        .contains(&"route_meta_fragment@kprobe:ip_route_output_flow".to_string()));
    assert_eq!(report.fragments_loaded.len(), 2);
    assert!(!report
        .fragments_loaded
        .contains(&"route_meta_fragment".to_string()));
    assert_eq!(report.ringbuf_stats.maps, 2);
    assert_eq!(report.ringbuf_stats.total_max_entries, 8_192);
}

#[test]
fn attach_report_accepts_structured_failure_records() {
    let registry = builtin_registry();
    let plan = registry
        .plan([
            "tcp_state_fragment",
            "tcp_packet_meta_fragment",
            "route_meta_fragment",
        ])
        .unwrap();

    let report = registry.attach_report_with_failure_records(
        &plan,
        [AttachFailure {
            fragment_id: "route_meta_fragment",
            hookpoint: HookPoint::KProbe("ip_route_output_flow"),
            error: "mock attach failure".into(),
        }],
    );

    assert_eq!(
        report.hookpoints_failed,
        vec!["route_meta_fragment@kprobe:ip_route_output_flow".to_string()]
    );
    assert_eq!(report.fragments_loaded.len(), 2);
}

#[test]
fn attach_report_keeps_failures_that_are_outside_the_plan() {
    let registry = builtin_registry();
    let plan = registry
        .plan([
            "tcp_state_fragment",
            "tcp_packet_meta_fragment",
            "route_meta_fragment",
        ])
        .unwrap();

    let report = registry.attach_report_with_failure_records(
        &plan,
        [AttachFailure {
            fragment_id: "linux_tracepoint_smoke_fragment",
            hookpoint: HookPoint::TracePoint("syscalls/definitely_missing_smoke_event"),
            error: "mock attach failure".into(),
        }],
    );

    assert_eq!(
        report.hookpoints_failed,
        vec!["linux_tracepoint_smoke_fragment@tracepoint:syscalls/definitely_missing_smoke_event".to_string()]
    );
    assert_eq!(report.fragments_loaded.len(), 3);
}

fn test_fragment(
    id: &'static str,
    hookpoint: HookPoint,
    emits: FactKindTag,
    requires: Vec<FactKindTag>,
) -> FragmentDescriptor {
    FragmentDescriptor {
        id,
        version: 1,
        hookpoints: vec![hookpoint],
        emits: vec![emits],
        requires,
        maps: vec![MapSpec {
            name: "events",
            kind: MapKind::RingBuf,
            max_entries: 1024,
        }],
        capabilities: vec![CapabilityFlag::TcpState],
    }
}
