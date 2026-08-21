use std::collections::BTreeSet;
use std::path::PathBuf;
use std::process::Command;

use gewyvern::project_status::{
    ContractStability, EvidenceKind, EvidenceState, Independence, Maturity, STATUS_SCHEMA_VERSION,
    StatusCatalog, default_catalog_path,
};
use serde_json::json;

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn json_string_set(value: &serde_json::Value, field: &str) -> BTreeSet<String> {
    value[field]
        .as_array()
        .unwrap_or_else(|| panic!("{field} must be an array"))
        .iter()
        .map(|value| {
            value
                .as_str()
                .unwrap_or_else(|| panic!("{field} entries must be strings"))
                .to_string()
        })
        .collect()
}

fn collect_csharp_mutation_routes(root: &std::path::Path) -> BTreeSet<String> {
    fn visit(path: &std::path::Path, routes: &mut BTreeSet<String>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                visit(&path, routes);
                continue;
            }
            if path.extension().and_then(|value| value.to_str()) != Some("cs") {
                continue;
            }
            let source = std::fs::read_to_string(&path).unwrap();
            let mut remaining = source.as_str();
            while let Some(index) = remaining.find("app.Map") {
                remaining = &remaining[index + "app.Map".len()..];
                let method_end = remaining.find('(').unwrap_or_else(|| {
                    panic!("incomplete app.Map invocation in {}", path.display())
                });
                let method = &remaining[..method_end];
                remaining = &remaining[method_end + 1..];
                if matches!(method, "OpenApi" | "StaticAssets" | "FallbackToFile") {
                    continue;
                }
                let route_start = remaining.find("\"/v1/").unwrap_or_else(|| {
                    panic!(
                        "app.Map{method} in {} has no literal /v1 route",
                        path.display()
                    )
                });
                let route = &remaining[route_start + 1..];
                let route_end = route.find('"').unwrap();
                match method {
                    "Get" | "Head" | "Options" => {}
                    "Post" | "Put" | "Delete" | "Patch" => {
                        assert!(
                            routes.insert(route[..route_end].to_string()),
                            "duplicate C# mutation route {}",
                            &route[..route_end]
                        );
                    }
                    _ => panic!(
                        "unsupported /v1 route helper app.Map{method} in {}; classify it before merging",
                        path.display()
                    ),
                }
                remaining = &route[route_end + 1..];
            }
        }
    }

    let mut routes = BTreeSet::new();
    visit(root, &mut routes);
    routes
}

fn collect_rust_remote_routes(source: &str) -> BTreeSet<String> {
    let table_start = source
        .find("let route = match parts[1] {")
        .expect("remote route match must exist");
    let table = &source[table_start..];
    let table_end = table
        .find("\n    };")
        .expect("remote route match must be bounded");
    let mut table = &table[..table_end];
    let mut routes = BTreeSet::new();
    while let Some(start) = table.find("\"/v1/") {
        table = &table[start + 1..];
        let end = table.find('"').unwrap();
        assert!(routes.insert(table[..end].to_string()));
        table = &table[end + 1..];
    }
    routes
}

#[test]
fn project_status_catalog_is_protocolized_and_valid() {
    let root = repository_root();
    let schema: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join("project/status/schema.json"))
            .expect("status schema must exist"),
    )
    .expect("status schema must be JSON");
    assert_eq!(
        schema["$schema"],
        "https://json-schema.org/draft/2020-12/schema"
    );
    assert_eq!(
        schema["properties"]["schema_version"]["const"],
        STATUS_SCHEMA_VERSION
    );

    let catalog = StatusCatalog::load(default_catalog_path()).expect("catalog must decode");
    catalog.validate(&root).expect("catalog must validate");
    assert!(catalog.dimensions.architectures.len() >= 6);
    assert!(catalog.dimensions.modules.len() >= 21);
    assert!(catalog.dimensions.features.len() >= 23);
    assert!(catalog.coverage_requirements.len() >= 28);
    assert!(catalog.cells.len() >= 23);

    for cell in &catalog.cells {
        assert!(!cell.contract.id.is_empty());
        assert!(!cell.contract.version.is_empty());
        assert!(!cell.contract.surfaces.is_empty());
        assert!(!cell.evidence.is_empty());
        assert!(!cell.next_gate.is_empty());
    }
}

#[test]
fn control_plane_writer_inventory_is_exhaustive_across_csharp_and_rust_routes() {
    let inventory: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            repository_root().join("docs/contracts/leserpent-control-plane-mutations-v1.json"),
        )
        .expect("control-plane mutation inventory must exist"),
    )
    .expect("control-plane mutation inventory must decode");
    assert_eq!(inventory["version"], "1.24.0");
    let mut expected_csharp_routes = json_string_set(&inventory, "mutation_routes");
    expected_csharp_routes.extend(json_string_set(&inventory, "read_only_post_allowlist"));
    assert_eq!(
        collect_csharp_mutation_routes(&repository_root().join("apps/leserpent/src/Leserpent")),
        expected_csharp_routes,
        "every C# non-read /v1 endpoint must be inventoried"
    );
    let writer_fence = &inventory["rust_authority_writer_fence"];
    let covered = writer_fence["covered_ipc_mutations"]
        .as_array()
        .expect("covered IPC mutations must be an array");
    let remaining = writer_fence["remaining_routes"]
        .as_array()
        .expect("remaining routes must be an array");
    for mutation in [
        "runtime_deploy",
        "runtime_refresh",
        "runtime_capabilities_refresh",
        "orchestra_persist",
        "orchestra_delete",
        "orchestra_delete_command",
        "orchestra_delete_replay_checkpoint",
        "bootstrap_session_bind",
        "bootstrap_v1",
        "provisioning_v1",
        "retirement_v1",
        "daemon_retirement_v1",
    ] {
        assert!(covered.iter().any(|value| value == mutation));
        assert!(!remaining.iter().any(|value| value == mutation));
    }
    assert!(remaining.is_empty());
    assert_eq!(
        collect_rust_remote_routes(
            &std::fs::read_to_string(repository_root().join("crates/leserpentd/src/remote.rs"))
                .unwrap()
        ),
        json_string_set(writer_fence, "remote_routes"),
        "every Rust HTTPS route must be inventoried"
    );
    let remote = writer_fence["covered_remote_mutations"]
        .as_array()
        .expect("covered remote mutations must be an array");
    for route in [
        "/v1/wire",
        "/v1/bootstrap",
        "/v1/provisioning",
        "/v1/retirement",
        "/v1/daemon-retirement",
    ] {
        assert!(remote.iter().any(|value| value == route));
    }
    assert_eq!(
        writer_fence["remote_headers"]["writer_id"],
        "X-Leserpent-Authority-Writer-Id"
    );
    assert_eq!(
        writer_fence["remote_headers"]["generation"],
        "X-Leserpent-Authority-Writer-Generation"
    );
    assert_eq!(
        writer_fence["claim_crash_proof"]["physical_linux_evidence"],
        "docs/fixtures/leserpent_authority_writer_claim_crash_linux_x86_64_20260801.json"
    );
    assert_eq!(
        writer_fence["claim_response_loss_proof"]["physical_linux_evidence"],
        "docs/fixtures/leserpent_authority_writer_lost_response_race_linux_x86_64_20260801.json"
    );
    assert_eq!(
        writer_fence["claim_cold_restart_replay_proof"]["physical_linux_evidence"],
        "docs/fixtures/leserpent_authority_writer_lost_response_cold_replay_linux_x86_64_20260801.json"
    );
    assert_eq!(
        writer_fence["claim_unclean_recovery_proof"]["physical_linux_evidence"],
        "docs/fixtures/leserpent_authority_writer_lost_response_sigkill_recovery_linux_x86_64_20260802.json"
    );
    assert_eq!(
        writer_fence["claim_repeated_unclean_recovery_proof"]["physical_linux_evidence"],
        "docs/fixtures/leserpent_authority_writer_repeated_unclean_recovery_linux_x86_64_20260802.json"
    );
    assert_eq!(
        writer_fence["claim_post_recovery_contention_proof"]["physical_linux_evidence"],
        "docs/fixtures/leserpent_authority_writer_post_recovery_contention_linux_x86_64_20260802.json"
    );
    assert_eq!(
        writer_fence["claim_post_recovery_duplicate_retry_proof"]["physical_linux_evidence"],
        "docs/fixtures/leserpent_authority_writer_post_recovery_duplicate_retry_linux_x86_64_20260802.json"
    );
    assert_eq!(
        writer_fence["claim_post_recovery_mixed_peer_proof"]["physical_linux_evidence"],
        "docs/fixtures/leserpent_authority_writer_post_recovery_mixed_peer_linux_x86_64_20260802.json"
    );
    let hostile_lifecycle = &writer_fence["claim_hostile_lifecycle_proof"];
    assert_eq!(hostile_lifecycle["repeated_batches"]["count"], 2);
    assert_eq!(hostile_lifecycle["repeated_batches"]["peers_per_batch"], 64);
    assert_eq!(hostile_lifecycle["shutdown"]["active_slow_peers"], 64);
    assert_eq!(
        hostile_lifecycle["shutdown"]["frame_read_poll_interval_ms"],
        100
    );
    assert_eq!(hostile_lifecycle["shutdown"]["sigterm_budget_ms"], 1000);
    assert_eq!(
        hostile_lifecycle["local_evidence"],
        "crates/leserpentd/tests/authority_writer_takeover_vertical.rs#repeated_hostile_batches_preserve_owner_heartbeat_and_bounded_sigterm"
    );
    assert_eq!(
        hostile_lifecycle["physical_linux_evidence"],
        "docs/fixtures/leserpent_authority_writer_repeated_hostile_lifecycle_linux_x86_64_20260802.json"
    );
    let hostile_resources = &writer_fence["claim_hostile_resource_retention_proof"];
    assert_eq!(hostile_resources["cycles"], 3);
    assert_eq!(hostile_resources["completed_batch_peers_per_cycle"], 64);
    assert_eq!(hostile_resources["shutdown_batch_peers_per_cycle"], 64);
    assert_eq!(
        hostile_resources["linux_proc_observation"],
        json!(["fd", "task"])
    );
    assert_eq!(
        hostile_resources["local_evidence"],
        "crates/leserpentd/tests/authority_writer_takeover_vertical.rs#repeated_hostile_shutdown_restart_cycles_bound_process_resources"
    );
    assert_eq!(
        hostile_resources["physical_linux_evidence"],
        "docs/fixtures/leserpent_authority_writer_hostile_resource_cycles_linux_x86_64_20260802.json"
    );
    let reconnect_fairness = &writer_fence["claim_hostile_reconnect_fairness_proof"];
    assert_eq!(reconnect_fairness["waves"], 3);
    assert_eq!(reconnect_fairness["peers_per_wave"], 64);
    assert_eq!(reconnect_fairness["slow_peers_per_wave"], 60);
    assert_eq!(reconnect_fairness["valid_reconnects_per_wave"], 4);
    assert_eq!(reconnect_fairness["total_valid_reconnects"], 12);
    assert_eq!(reconnect_fairness["ready_prefix_budget_ms"], 1_000);
    assert_eq!(reconnect_fairness["reconnect_budget_ms"], 3_000);
    assert_eq!(
        reconnect_fairness["local_evidence"],
        json!([
            "crates/leserpentd/src/ipc.rs#batch_dispatches_ready_prefix_before_later_slow_reader",
            "crates/leserpentd/tests/authority_writer_takeover_vertical.rs#burst_reconnects_remain_fair_across_repeated_saturated_hostile_waves"
        ])
    );
    assert_eq!(
        reconnect_fairness["physical_linux_evidence"],
        "docs/fixtures/leserpent_authority_writer_hostile_reconnect_fairness_linux_x86_64_20260802.json"
    );
    let cross_transport = &writer_fence["cross_transport_fairness_proof"];
    assert_eq!(cross_transport["waves"], 3);
    assert_eq!(cross_transport["slow_ipc_peers_per_wave"], 64);
    assert_eq!(cross_transport["authenticated_https_queries"], 3);
    assert_eq!(cross_transport["https_budget_ms"], 5_000);
    assert_eq!(cross_transport["wave_budget_ms"], 5_000);
    assert_eq!(
        cross_transport["schedule"],
        "maintenance-first-then-alternating-unix-ipc-and-https-priority"
    );
    assert_eq!(
        cross_transport["local_evidence"],
        json!([
            "crates/leserpentd/src/main.rs#transport_scheduler_alternates_local_and_remote_priority",
            "crates/leserpentd/tests/cross_transport_fairness_vertical.rs#https_and_maintenance_progress_across_repeated_saturated_ipc_waves"
        ])
    );
    assert_eq!(
        cross_transport["physical_linux_evidence"],
        "docs/fixtures/leserpent_cross_transport_fairness_linux_x86_64_20260802.json"
    );
    let slow_https = &writer_fence["slow_https_cross_transport_fairness_proof"];
    assert_eq!(slow_https["waves"], 3);
    assert_eq!(slow_https["authenticated_slow_https_peers_per_wave"], 1);
    assert_eq!(slow_https["ipc_queries_per_wave"], 4);
    assert_eq!(slow_https["total_ipc_queries"], 12);
    assert_eq!(slow_https["remote_read_timeout_ms"], 3_000);
    assert_eq!(slow_https["remote_budget_consumption_floor_ms"], 2_500);
    assert_eq!(slow_https["ipc_query_budget_ms"], 5_000);
    assert_eq!(slow_https["wave_budget_ms"], 5_000);
    assert_eq!(
        slow_https["local_evidence"],
        "crates/leserpentd/tests/cross_transport_fairness_vertical.rs#ipc_and_maintenance_progress_across_repeated_authenticated_slow_https_waves"
    );
    assert_eq!(
        slow_https["physical_linux_evidence"],
        "docs/fixtures/leserpent_slow_https_cross_transport_fairness_linux_x86_64_20260802.json"
    );
    let slow_https_shutdown = &writer_fence["slow_https_shutdown_proof"];
    assert_eq!(
        slow_https_shutdown["active_authenticated_slow_https_peers"],
        1
    );
    assert_eq!(slow_https_shutdown["connection_read_poll_interval_ms"], 100);
    assert_eq!(slow_https_shutdown["connection_hard_deadline_ms"], 3_000);
    assert_eq!(slow_https_shutdown["sigterm_budget_ms"], 1_000);
    assert_eq!(
        slow_https_shutdown["local_evidence"],
        "crates/leserpentd/tests/cross_transport_fairness_vertical.rs#sigterm_cancels_authenticated_slow_https_and_allows_immediate_restart"
    );
    assert_eq!(
        slow_https_shutdown["physical_linux_evidence"],
        "docs/fixtures/leserpent_slow_https_sigterm_linux_x86_64_20260802.json"
    );
    let phase_shutdown = &writer_fence["remote_read_phase_shutdown_proof"];
    assert_eq!(
        phase_shutdown["phases"],
        json!([
            "incomplete-tls-handshake",
            "incomplete-authenticated-http-header",
            "authenticated-incomplete-body"
        ])
    );
    assert_eq!(phase_shutdown["daemon_processes"], 4);
    assert_eq!(phase_shutdown["physical_repetitions"], 3);
    assert_eq!(phase_shutdown["connection_read_poll_interval_ms"], 100);
    assert_eq!(phase_shutdown["connection_hard_deadline_ms"], 3_000);
    assert_eq!(phase_shutdown["sigterm_budget_ms"], 1_000);
    assert!(
        phase_shutdown["transport_cancellation"]
            .as_str()
            .unwrap()
            .contains("nonretryable-connection-aborted")
    );
    assert!(
        phase_shutdown["timeout_response"]
            .as_str()
            .unwrap()
            .contains("first-nonblocking-http-error-write")
    );
    assert_eq!(
        phase_shutdown["local_evidence"],
        "crates/leserpentd/tests/cross_transport_fairness_vertical.rs#repeated_remote_read_phase_shutdowns_preserve_process_resource_baselines"
    );
    assert_eq!(
        phase_shutdown["physical_linux_evidence"],
        "docs/fixtures/leserpent_remote_read_phase_shutdown_linux_x86_64_20260802.json"
    );
    let backlog_shutdown = &writer_fence["remote_backlog_shutdown_proof"];
    assert_eq!(
        backlog_shutdown["active_phases"],
        json!([
            "incomplete-tls-handshake",
            "incomplete-authenticated-http-header",
            "authenticated-incomplete-body"
        ])
    );
    assert_eq!(backlog_shutdown["daemon_processes"], 4);
    assert_eq!(
        backlog_shutdown["incomplete_tls_backlog_peers_per_phase"],
        64
    );
    assert_eq!(backlog_shutdown["connection_read_poll_interval_ms"], 100);
    assert_eq!(backlog_shutdown["connection_hard_deadline_ms"], 3_000);
    assert_eq!(backlog_shutdown["sigterm_budget_ms"], 1_000);
    assert!(
        backlog_shutdown["backlog_process_resources"]
            .as_str()
            .unwrap()
            .contains("zero-daemon-fd-or-task-amplification")
    );
    assert!(
        backlog_shutdown["authority"]
            .as_str()
            .unwrap()
            .contains("replayed-across-restarts")
    );
    assert_eq!(
        backlog_shutdown["local_evidence"],
        "crates/leserpentd/tests/cross_transport_fairness_vertical.rs#mixed_remote_read_phases_with_listener_backlog_preserve_bounded_shutdown_and_authority"
    );
    assert_eq!(
        backlog_shutdown["physical_linux_evidence"],
        "docs/fixtures/leserpent_remote_backlog_shutdown_linux_x86_64_20260802.json"
    );
    let event_shutdown = &writer_fence["maximum_event_session_shutdown_proof"];
    assert_eq!(event_shutdown["authenticated_websocket_event_sessions"], 32);
    assert_eq!(event_shutdown["active_authenticated_stalled_requests"], 1);
    assert_eq!(event_shutdown["daemon_processes"], 2);
    assert_eq!(event_shutdown["event_route"], "/v1/events");
    assert_eq!(event_shutdown["event_subprotocol"], "leserpent.events.v1");
    assert_eq!(event_shutdown["sigterm_budget_ms"], 1_000);
    assert!(
        event_shutdown["event_session_process_resources"]
            .as_str()
            .unwrap()
            .contains("38-open-fds-and-1-task")
    );
    assert!(
        event_shutdown["saturated_process_resources"]
            .as_str()
            .unwrap()
            .contains("39-open-fds-and-1-task")
    );
    assert!(
        event_shutdown["event_delivery"]
            .as_str()
            .unwrap()
            .contains("zero-application-events-after-sigterm")
    );
    assert!(
        event_shutdown["pre_sigterm_event_queue"]
            .as_str()
            .unwrap()
            .contains("after-stalled-request-is-active")
    );
    assert_eq!(
        event_shutdown["local_evidence"],
        "crates/leserpentd/tests/cross_transport_fairness_vertical.rs#maximum_event_sessions_with_stalled_request_preserve_bounded_shutdown_and_resources"
    );
    assert_eq!(
        event_shutdown["physical_linux_evidence"],
        "docs/fixtures/leserpent_max_event_session_shutdown_linux_x86_64_20260802.json"
    );
}

#[test]
fn maximum_event_session_cycle_contract_tracks_reclamation_before_admission() {
    let inventory: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            repository_root().join("docs/contracts/leserpent-control-plane-mutations-v1.json"),
        )
        .expect("control-plane mutation inventory must exist"),
    )
    .expect("control-plane mutation inventory must decode");
    let proof = &inventory["rust_authority_writer_fence"]["maximum_event_session_cycle_proof"];
    assert_eq!(proof["cycles"], 3);
    assert_eq!(
        proof["authenticated_websocket_event_sessions_per_cycle"],
        32
    );
    assert_eq!(proof["maximum_capacity_window_sessions"], 96);
    assert_eq!(proof["immediate_post_disconnect_reclamation_probes"], 3);
    assert_eq!(proof["total_authenticated_websocket_event_sessions"], 99);
    assert_eq!(proof["runtime_mutations"], 3);
    assert_eq!(proof["runtime_snapshot_events"], 96);
    assert!(
        proof["cross_transport_progress"]
            .as_str()
            .unwrap()
            .contains("ipc-query-and-one-authenticated-https-query")
    );
    assert!(
        proof["reconnect_ordering"]
            .as_str()
            .unwrap()
            .contains("before-a-new-connection")
    );
    assert!(
        proof["remaining_physical_proof"]
            .as_str()
            .unwrap()
            .contains("physical-linux-x86-64")
    );
    assert_eq!(
        proof["local_evidence"],
        "crates/leserpentd/tests/cross_transport_fairness_vertical.rs#maximum_event_session_cycles_reclaim_slots_and_preserve_cross_transport_progress"
    );

    let remote = std::fs::read_to_string(repository_root().join("crates/leserpentd/src/remote.rs"))
        .expect("remote server source must exist");
    let reclamation = remote
        .find("// Reclaim closed event sessions before applying the capacity limit")
        .expect("remote scheduler must document pre-admission reclamation");
    let admission = remote[reclamation..]
        .find("let accepted = match self.listener.accept()")
        .expect("remote scheduler must retain listener admission after reclamation");
    assert!(admission > 0);
}

#[test]
fn slow_event_session_contract_is_bounded_and_non_vacuous() {
    let inventory: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            repository_root().join("docs/contracts/leserpent-control-plane-mutations-v1.json"),
        )
        .expect("control-plane mutation inventory must exist"),
    )
    .expect("control-plane mutation inventory must decode");
    let proof = &inventory["rust_authority_writer_fence"]["slow_event_session_isolation_proof"];
    assert_eq!(proof["seeded_runtime_projections"], 128);
    assert_eq!(proof["minimum_initial_snapshot_bytes"], 32 * 1_024);
    assert_eq!(proof["maximum_event_sessions"], 32);
    assert_eq!(proof["nonreading_event_sessions"], 1);
    assert_eq!(proof["healthy_event_sessions"], 31);
    assert_eq!(proof["authority_revisions"], 24);
    assert_eq!(proof["healthy_runtime_snapshot_events"], 744);
    assert_eq!(proof["event_write_buffer_limit_bytes"], 1_049_600);
    assert!(
        proof["slow_session_policy"]
            .as_str()
            .unwrap()
            .contains("drop-only-the-slow-session")
    );
    assert!(
        proof["remaining_physical_proof"]
            .as_str()
            .unwrap()
            .contains("physical-linux-x86-64")
    );
    assert_eq!(
        proof["local_evidence"],
        "crates/leserpentd/tests/cross_transport_fairness_vertical.rs#slow_event_session_is_bounded_without_blocking_healthy_fanout_or_transports"
    );

    let events = std::fs::read_to_string(repository_root().join("crates/leserpentd/src/events.rs"))
        .expect("event session source must exist");
    let bounded_buffer = events
        .find(".max_write_buffer_size(MAX_PROTOCOL_MESSAGE_BYTES + 1024)")
        .expect("event sessions must retain a bounded write buffer");
    let slow_session_drop = events[bounded_buffer..]
        .find("Err(_) => false")
        .expect("event sessions must isolate a terminal slow writer");
    assert!(slow_session_drop > 0);
}

#[test]
fn retained_linux_authority_writer_cold_response_replay_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_authority_writer_lost_response_cold_replay_linux_x86_64_20260801.json",
        ))
        .expect("authority writer cold response replay evidence must exist"),
    )
    .expect("authority writer cold response replay evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.1.46");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(
        evidence["test"]["name"],
        "lost_final_claim_response_replays_after_cold_restart_before_queued_competitor"
    );
    assert_eq!(
        evidence["sequence"]["first_daemon"]["lost_response_generation"],
        2
    );
    assert_eq!(
        evidence["sequence"]["first_daemon"]["response_bytes_decoded"],
        false
    );
    assert_eq!(
        evidence["sequence"]["second_daemon"]["replay_result"]["replayed"],
        true
    );
    assert_eq!(
        evidence["sequence"]["second_daemon"]["competitor_result"]["generation"],
        3
    );
    assert_eq!(evidence["sequence"]["third_daemon"]["generation"], 3);
    assert_eq!(evidence["sequence"]["third_daemon"]["replayed"], true);
    for check in [
        "production_daemon_ipc_path",
        "lost_claim_commit_survives_cold_restart",
        "old_socket_removed_before_restart",
        "actual_listener_connect_readiness",
        "replay_precedes_queued_competitor",
        "competitor_advances_exactly_once",
        "stale_replayed_ticket_rejected_after_competitor",
        "final_ticket_mutation_applied",
        "second_cold_restart_replay_stable",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_authority_writer_unclean_response_recovery_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_authority_writer_lost_response_sigkill_recovery_linux_x86_64_20260802.json",
        ))
        .expect("authority writer unclean response recovery evidence must exist"),
    )
    .expect("authority writer unclean response recovery evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.45");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(
        evidence["test"]["name"],
        "lost_claim_response_survives_sigkill_lease_expiry_and_same_socket_recovery"
    );
    assert_eq!(evidence["test"]["owner_lease_duration_ms"], 30_000);
    assert_eq!(
        evidence["sequence"]["unclean_owner"]["lost_response_generation"],
        2
    );
    assert_eq!(
        evidence["sequence"]["unclean_owner"]["termination"],
        "SIGKILL"
    );
    assert_eq!(
        evidence["sequence"]["pre_expiry"]["replacement_admitted"],
        false
    );
    assert_eq!(
        evidence["sequence"]["natural_recovery"]["replay_result"]["replayed"],
        true
    );
    assert_eq!(
        evidence["sequence"]["natural_recovery"]["competitor_result"]["generation"],
        3
    );
    for check in [
        "production_daemon_ipc_path",
        "lost_response_and_sigkill_combined",
        "sqlite_generation_2_durable",
        "pre_expiry_owner_rejected",
        "pre_expiry_socket_not_removed",
        "natural_owner_lease_expiry_observed",
        "same_socket_path_recovered",
        "live_listener_nonreplacement_unit_proof",
        "nonsocket_and_symlink_rejection_unit_proof",
        "insecure_socket_rejection_unit_proof",
        "same_writer_generation_2_replayed",
        "competitor_generation_3_nonreplayed",
        "stale_ticket_rejected",
        "final_ticket_mutation_applied",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_authority_writer_repeated_unclean_recovery_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_authority_writer_repeated_unclean_recovery_linux_x86_64_20260802.json",
        ))
        .expect("repeated authority writer unclean recovery evidence must exist"),
    )
    .expect("repeated authority writer unclean recovery evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.45");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(
        evidence["test"]["name"],
        "repeated_unclean_response_recovery_preserves_generations_and_same_socket"
    );
    assert_eq!(evidence["test"]["owner_lease_duration_ms"], 30_000);
    assert_eq!(evidence["test"]["unclean_cycles"], 2);
    assert_eq!(
        evidence["sequence"]["first_cycle"]["lost_response_generation"],
        2
    );
    assert_eq!(
        evidence["sequence"]["first_cycle"]["competitor_generation"],
        3
    );
    assert_eq!(
        evidence["sequence"]["second_cycle"]["lost_response_generation"],
        4
    );
    assert_eq!(
        evidence["sequence"]["second_cycle"]["competitor_generation"],
        5
    );
    assert_eq!(evidence["sequence"]["final"]["generation"], 5);
    assert_eq!(evidence["sequence"]["final"]["replayed"], true);
    assert_eq!(
        evidence["sequence"]["final"]["stale_generations_rejected"],
        serde_json::json!([3, 4])
    );
    for check in [
        "production_daemon_ipc_path",
        "two_unread_response_sigkill_cycles",
        "two_pre_expiry_owner_rejections",
        "two_natural_owner_lease_expiries",
        "two_same_socket_path_recoveries",
        "contiguous_generations_1_through_5",
        "same_id_replay_stable_each_cycle",
        "competitor_advances_exactly_once_each_cycle",
        "all_prior_tickets_stale",
        "final_ticket_mutation_applied",
        "final_ticket_replay_stable",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_authority_writer_post_recovery_contention_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_authority_writer_post_recovery_contention_linux_x86_64_20260802.json",
        ))
        .expect("post-recovery writer contention evidence must exist"),
    )
    .expect("post-recovery writer contention evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.45");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(
        evidence["test"]["name"],
        "post_recovery_writer_contention_is_bounded_and_generation_contiguous"
    );
    assert_eq!(evidence["test"]["owner_lease_duration_ms"], 30_000);
    assert_eq!(evidence["test"]["concurrent_contenders"], 64);
    assert_eq!(evidence["test"]["claim_budget_ms"], 5_000);
    assert_eq!(evidence["test"]["production_ipc_batch_limit"], 64);
    assert_eq!(
        evidence["sequence"]["unclean_owner"]["lost_response_generation"],
        2
    );
    assert_eq!(evidence["sequence"]["natural_recovery"]["generation"], 2);
    assert_eq!(evidence["sequence"]["natural_recovery"]["replayed"], true);
    assert_eq!(
        evidence["sequence"]["contention"]["independent_writer_ids"],
        64
    );
    assert_eq!(evidence["sequence"]["contention"]["first_generation"], 3);
    assert_eq!(evidence["sequence"]["contention"]["final_generation"], 66);
    assert_eq!(evidence["sequence"]["final"]["generation"], 66);
    assert_eq!(evidence["sequence"]["final"]["replayed"], true);
    assert_eq!(
        evidence["sequence"]["final"]["stale_generations_rejected"],
        serde_json::json!([2, 65])
    );
    for check in [
        "production_daemon_ipc_path",
        "unread_response_sigkill_recovery",
        "pre_expiry_owner_and_socket_fence",
        "natural_owner_lease_expiry",
        "same_socket_path_recovery",
        "production_batch_limit_saturated",
        "all_64_claims_complete_within_budget",
        "unique_writer_admission",
        "contiguous_generations_3_through_66",
        "no_false_replays_under_contention",
        "recovered_ticket_stale",
        "penultimate_ticket_stale",
        "final_ticket_mutation_applied",
        "final_ticket_replay_stable",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_authority_writer_post_recovery_duplicate_retry_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_authority_writer_post_recovery_duplicate_retry_linux_x86_64_20260802.json",
        ))
        .expect("post-recovery duplicate retry evidence must exist"),
    )
    .expect("post-recovery duplicate retry evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.45");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(
        evidence["test"]["name"],
        "post_recovery_saturated_duplicate_retries_survive_abandoned_responses"
    );
    assert_eq!(evidence["test"]["owner_lease_duration_ms"], 30_000);
    assert_eq!(evidence["test"]["claim_budget_ms"], 5_000);
    assert_eq!(evidence["test"]["production_ipc_batch_limit"], 64);
    assert_eq!(evidence["test"]["duplicate_groups"], 16);
    assert_eq!(evidence["test"]["abandoned_responses"], 16);
    assert_eq!(evidence["test"]["readable_retries"], 48);
    assert_eq!(evidence["sequence"]["natural_recovery"]["generation"], 2);
    assert_eq!(evidence["sequence"]["natural_recovery"]["replayed"], true);
    assert_eq!(evidence["sequence"]["saturated_batch"]["total_claims"], 64);
    assert_eq!(evidence["sequence"]["saturated_batch"]["group_count"], 16);
    assert_eq!(
        evidence["sequence"]["saturated_batch"]["abandoned_response_claims"],
        16
    );
    assert_eq!(
        evidence["sequence"]["saturated_batch"]["readable_retry_replays"],
        48
    );
    assert_eq!(
        evidence["sequence"]["saturated_batch"]["first_generation"],
        3
    );
    assert_eq!(
        evidence["sequence"]["saturated_batch"]["final_generation"],
        18
    );
    assert!(
        evidence["sequence"]["saturated_batch"]["observed_elapsed_ms"]
            .as_u64()
            .unwrap()
            <= 5_000
    );
    assert_eq!(evidence["sequence"]["final"]["generation"], 18);
    assert_eq!(evidence["sequence"]["final"]["replayed"], true);
    assert_eq!(
        evidence["sequence"]["final"]["stale_generations_rejected"],
        serde_json::json!([2, 17])
    );
    for check in [
        "production_daemon_ipc_path",
        "unread_response_sigkill_recovery",
        "pre_expiry_owner_and_socket_fence",
        "natural_owner_lease_expiry",
        "same_socket_path_recovery",
        "production_batch_limit_saturated",
        "sixteen_response_read_halves_closed",
        "sixteen_abandoned_claims_committed",
        "forty_eight_same_id_retries_replayed",
        "peer_response_failures_isolated",
        "all_readable_claims_complete_within_budget",
        "contiguous_generations_3_through_18",
        "recovered_ticket_stale",
        "penultimate_ticket_stale",
        "final_ticket_mutation_applied",
        "final_ticket_replay_stable",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_authority_writer_post_recovery_mixed_peer_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_authority_writer_post_recovery_mixed_peer_linux_x86_64_20260802.json",
        ))
        .expect("post-recovery mixed peer evidence must exist"),
    )
    .expect("post-recovery mixed peer evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.45");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(
        evidence["test"]["name"],
        "post_recovery_mixed_hostile_and_slow_peers_preserve_valid_claim_progress"
    );
    assert_eq!(evidence["test"]["owner_lease_duration_ms"], 30_000);
    assert_eq!(evidence["test"]["claim_budget_ms"], 5_000);
    assert_eq!(evidence["test"]["production_ipc_batch_limit"], 64);
    assert_eq!(evidence["test"]["peer_read_timeout_ms"], 2_000);
    assert_eq!(evidence["test"]["malformed_peers"], 16);
    assert_eq!(evidence["test"]["unauthorized_peers"], 16);
    assert_eq!(evidence["test"]["slow_timeout_peers"], 16);
    assert_eq!(evidence["test"]["valid_claim_peers"], 16);
    assert_eq!(evidence["sequence"]["mixed_batch"]["total_peers"], 64);
    assert_eq!(evidence["sequence"]["mixed_batch"]["group_count"], 16);
    assert_eq!(
        evidence["sequence"]["mixed_batch"]["malformed_error"],
        "invalid_json"
    );
    assert_eq!(
        evidence["sequence"]["mixed_batch"]["unauthorized_error"],
        "unauthorized"
    );
    assert_eq!(
        evidence["sequence"]["mixed_batch"]["slow_peer_response_bytes"],
        0
    );
    assert_eq!(
        evidence["sequence"]["mixed_batch"]["valid_first_generation"],
        3
    );
    assert_eq!(
        evidence["sequence"]["mixed_batch"]["valid_final_generation"],
        18
    );
    assert!(
        evidence["sequence"]["mixed_batch"]["observed_elapsed_ms"]
            .as_u64()
            .unwrap()
            <= 5_000
    );
    assert_eq!(evidence["sequence"]["final"]["generation"], 18);
    assert_eq!(evidence["sequence"]["final"]["replayed"], true);
    assert_eq!(
        evidence["sequence"]["final"]["stale_generations_rejected"],
        serde_json::json!([2, 17])
    );
    for check in [
        "production_daemon_ipc_path",
        "unread_response_sigkill_recovery",
        "pre_expiry_owner_and_socket_fence",
        "natural_owner_lease_expiry",
        "same_socket_path_recovery",
        "production_batch_limit_saturated",
        "parallel_batch_frame_reads",
        "accept_order_serial_dispatch",
        "sixteen_malformed_peers_fixed_error",
        "sixteen_unauthorized_peers_fixed_error",
        "sixteen_slowloris_peers_timeout_together",
        "slow_peer_responses_empty",
        "invalid_peers_allocate_no_generation",
        "sixteen_valid_claims_complete_within_budget",
        "contiguous_generations_3_through_18",
        "recovered_ticket_stale",
        "penultimate_ticket_stale",
        "final_ticket_mutation_applied",
        "final_ticket_replay_stable",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn local_repeated_hostile_lifecycle_proof_is_non_vacuous() {
    let ipc = std::fs::read_to_string(repository_root().join("crates/leserpentd/src/ipc.rs"))
        .expect("IPC authority source must exist");
    for contract in [
        "const IPC_FRAME_READ_TIMEOUT: Duration = Duration::from_secs(2);",
        "const IPC_FRAME_READ_POLL_INTERVAL: Duration = Duration::from_millis(100);",
        "pub fn poll_batch_until(",
        "read_frame_until(stream, cancelled)",
        "if cancelled.load(Ordering::Acquire)",
        "for reader in readers",
        "ready accepted prefix",
        "fn batch_dispatches_ready_prefix_before_later_slow_reader()",
        "fn trickle_frame_cannot_extend_the_total_read_deadline()",
    ] {
        assert!(ipc.contains(contract), "missing IPC contract {contract}");
    }
    let main = std::fs::read_to_string(repository_root().join("crates/leserpentd/src/main.rs"))
        .expect("daemon entrypoint source must exist");
    assert!(main.matches("ipc.poll_batch_until(").count() >= 2);
    let vertical = std::fs::read_to_string(
        repository_root().join("crates/leserpentd/tests/authority_writer_takeover_vertical.rs"),
    )
    .expect("authority writer vertical proof must exist");
    for contract in [
        "const REPEATED_HOSTILE_BATCHES: usize = 2;",
        "const RESOURCE_LIFECYCLE_CYCLES: usize = 3;",
        "const RECONNECT_FAIRNESS_WAVES: usize = 3;",
        "const RECONNECTS_PER_FAIRNESS_WAVE: usize = 4;",
        "const SLOW_PEERS_PER_RECONNECT_GROUP: usize = 15;",
        "const RECONNECT_FAIRNESS_BUDGET: Duration = Duration::from_secs(5);",
        "fn repeated_hostile_batches_preserve_owner_heartbeat_and_bounded_sigterm()",
        "fn repeated_hostile_shutdown_restart_cycles_bound_process_resources()",
        "fn burst_reconnects_remain_fair_across_repeated_saturated_hostile_waves()",
        "run_saturated_reconnect_fairness_wave",
        "valid reconnect starved behind a saturated hostile wave",
        "wait_for_owner_lease_extension",
        "wait_for_saturated_reader_resources",
        "assert_process_resources_released",
        "post-batch process resources drifted across cycles",
        "daemon.stop_with_budget(Duration::from_secs(2))",
        "shutdown_elapsed < Duration::from_secs(1)",
        "SELECT COUNT(*) FROM runtime_owner",
        "owner_lease_released=true immediate_restart=true generation=1",
        "authority-writer-hostile-resource-cycles cycles={}",
    ] {
        assert!(
            vertical.contains(contract),
            "missing hostile lifecycle proof contract {contract}"
        );
    }
}

#[test]
fn local_cross_transport_fairness_contract_is_non_vacuous() {
    let main = std::fs::read_to_string(repository_root().join("crates/leserpentd/src/main.rs"))
        .expect("daemon entrypoint source must exist");
    let turn = main
        .split_once("fn run_fair_daemon_turn(")
        .expect("fair daemon turn must exist")
        .1
        .split_once("\nfn run()")
        .expect("fair daemon turn must end before run")
        .0;
    let maintenance = turn
        .find("host.run_steps_until(1, stop)")
        .expect("fair turn must run maintenance");
    let transport_order = turn
        .find("if remote_first")
        .expect("fair turn must choose transport order");
    assert!(
        maintenance < transport_order,
        "maintenance must precede both transport order branches"
    );
    for contract in [
        "struct TransportScheduler",
        "self.remote_first = !self.remote_first;",
        "fn transport_scheduler_alternates_local_and_remote_priority()",
        "run_fair_daemon_turn(",
    ] {
        assert!(
            main.contains(contract),
            "missing scheduler contract {contract}"
        );
    }

    let remote = std::fs::read_to_string(repository_root().join("crates/leserpentd/src/remote.rs"))
        .expect("remote transport source must exist");
    for contract in [
        "const CONNECTION_TIMEOUT: Duration = Duration::from_secs(3);",
        "const CONNECTION_READ_POLL_INTERVAL: Duration = Duration::from_millis(100);",
        "struct CancellableTransport",
        "if self.cancelled.load(Ordering::Acquire)",
        "if Instant::now() >= self.deadline",
        "std::io::ErrorKind::ConnectionAborted",
        "fn cancelled_transport_uses_a_nonretryable_error_kind()",
        "std::thread::sleep(CONNECTION_READ_POLL_INTERVAL)",
        "pub fn poll_once_until(",
        "if cancelled.load(Ordering::Acquire)",
        "remote request cancelled",
        ".set_nonblocking(true)",
        "stream.sock.into_inner()",
    ] {
        assert!(
            remote.contains(contract),
            "missing cancellable remote-read contract {contract}"
        );
    }
    assert!(main.matches("remote.poll_once_until(").count() >= 2);

    let vertical = std::fs::read_to_string(
        repository_root().join("crates/leserpentd/tests/cross_transport_fairness_vertical.rs"),
    )
    .expect("cross-transport vertical proof must exist");
    for contract in [
        "const FAIRNESS_WAVES: usize = 3;",
        "const SLOW_IPC_PEERS_PER_WAVE: usize = 64;",
        "const SLOW_HTTPS_FAIRNESS_WAVES: usize = 3;",
        "const IPC_QUERIES_PER_SLOW_HTTPS_WAVE: usize = 4;",
        "const REMOTE_READ_SHUTDOWN_PHASES: usize = 3;",
        "const REMOTE_BACKLOG_PEERS_PER_PHASE: usize = 64;",
        "const MAXIMUM_EVENT_SESSIONS: usize = 32;",
        "static NEXT_TEMP_ROOT: AtomicU64 = AtomicU64::new(0);",
        "NEXT_TEMP_ROOT.fetch_add(1, Ordering::Relaxed)",
        "enum RemoteReadPhase",
        "TlsHandshake",
        "HttpHeader",
        "AuthenticatedBody",
        "struct ProcessResources",
        "fn wait_for_idle_process_resources(",
        "fn wait_for_stalled_remote_resource(",
        "fn wait_for_event_session_resources(",
        "fn sample_unchanged_process_resources(",
        "target.contains(\"=socket:[\")",
        "target.ends_with(\"-journal\")",
        "--remote-listen",
        "fn spawn_https_query(",
        "fn spawn_authenticated_slow_https(",
        "fn queue_incomplete_tls_backlog_peer(",
        "fn connect_event_session(",
        "fn assert_event_session_closed_without_application_event(",
        "fn drain_event_session_before_shutdown(",
        "Authorization: Bearer {TOKEN}",
        "Content-Length: 1\\r\\n\\r\\n",
        "Query::RuntimeList",
        "fn https_and_maintenance_progress_across_repeated_saturated_ipc_waves()",
        "fn ipc_and_maintenance_progress_across_repeated_authenticated_slow_https_waves()",
        "fn sigterm_cancels_authenticated_slow_https_and_allows_immediate_restart()",
        "fn repeated_remote_read_phase_shutdowns_preserve_process_resource_baselines()",
        "fn mixed_remote_read_phases_with_listener_backlog_preserve_bounded_shutdown_and_authority()",
        "fn maximum_event_sessions_with_stalled_request_preserve_bounded_shutdown_and_resources()",
        "daemon.stop_with_budget(Duration::from_secs(1))",
        "application_response_suppressed=true immediate_restart=true generation=1",
        "stable_fd_task_baselines=true proc_released_each_phase=true",
        "listener_backlog_zero_daemon_fd_amplification=true",
        "zero_authority_generation_allocation=true",
        "exact_session_fd_accounting=true",
        "late_application_events_suppressed=true",
        "pre_shutdown_event_queue_drained=true",
        "all_event_sessions_closed=true",
        "phases.map(RemoteReadPhase::label)",
        "let readiness_fence = send_ipc(&socket, &runtime_list_query());",
        "slow HTTPS peer did not consume the remote read budget",
        "total_ipc_queries={}",
        "maintenance_heartbeat_advanced_each_wave=true final_generation=1",
    ] {
        assert!(
            vertical.contains(contract),
            "missing cross-transport proof contract {contract}"
        );
    }
}

#[test]
fn retained_linux_repeated_hostile_lifecycle_proof_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_authority_writer_repeated_hostile_lifecycle_linux_x86_64_20260802.json",
        ))
        .expect("repeated hostile lifecycle Linux evidence must exist"),
    )
    .expect("repeated hostile lifecycle Linux evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.24");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(evidence["target"]["kernel"], "Linux 7.0.0-28-generic");
    assert_eq!(
        evidence["test"]["name"],
        "repeated_hostile_batches_preserve_owner_heartbeat_and_bounded_sigterm"
    );
    assert_eq!(evidence["test"]["repeated_hostile_batches"], 2);
    assert_eq!(evidence["test"]["peers_per_batch"], 64);
    assert_eq!(evidence["test"]["frame_read_poll_interval_ms"], 100);
    assert_eq!(evidence["test"]["sigterm_budget_ms"], 1_000);
    assert_eq!(
        evidence["sequence"]["hostile_batches"][0]["observed_elapsed_ms"],
        2_234
    );
    assert_eq!(
        evidence["sequence"]["hostile_batches"][1]["observed_elapsed_ms"],
        2_209
    );
    assert_eq!(evidence["sequence"]["shutdown"]["active_slow_peers"], 64);
    assert_eq!(evidence["sequence"]["shutdown"]["observed_elapsed_ms"], 165);
    assert_eq!(evidence["sequence"]["restart"]["generation"], 1);
    assert_eq!(evidence["sequence"]["restart"]["replayed"], true);
    for check in [
        "production_daemon_ipc_path",
        "two_repeated_hostile_batches",
        "production_batch_limit_saturated",
        "same_owner_token_across_batches",
        "owner_lease_refreshed_after_each_batch",
        "valid_claims_are_stable_replays",
        "hostile_batches_allocate_no_writer_generation",
        "each_batch_completes_within_budget",
        "signal_cancellable_parallel_frame_reads",
        "sixty_four_active_slow_peers_at_sigterm",
        "sigterm_completes_within_budget",
        "runtime_owner_row_released",
        "unix_socket_released",
        "immediate_same_path_restart",
        "writer_generation_replay_stable",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_hostile_resource_cycle_proof_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_authority_writer_hostile_resource_cycles_linux_x86_64_20260802.json",
        ))
        .expect("hostile resource cycle Linux evidence must exist"),
    )
    .expect("hostile resource cycle Linux evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.24");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(evidence["target"]["kernel"], "Linux 7.0.0-28-generic");
    assert_eq!(
        evidence["test"]["name"],
        "repeated_hostile_shutdown_restart_cycles_bound_process_resources"
    );
    assert_eq!(evidence["test"]["cycles"], 3);
    assert_eq!(evidence["test"]["completed_batch_peers_per_cycle"], 64);
    assert_eq!(evidence["test"]["shutdown_batch_peers_per_cycle"], 64);
    assert_eq!(evidence["test"]["sigterm_budget_ms"], 1_000);
    assert_eq!(
        evidence["resources"]["returned_after_completed_batch_per_cycle"],
        json!([
            { "open_fds": 5, "tasks": 1 },
            { "open_fds": 5, "tasks": 1 },
            { "open_fds": 5, "tasks": 1 }
        ])
    );
    assert_eq!(
        evidence["resources"]["active_shutdown_wave_per_cycle"],
        json!([
            { "open_fds": 69, "tasks": 65 },
            { "open_fds": 69, "tasks": 65 },
            { "open_fds": 69, "tasks": 65 }
        ])
    );
    assert_eq!(
        evidence["resources"]["active_delta_from_returned_baseline"],
        json!({ "open_fds": 64, "tasks": 64 })
    );
    assert_eq!(
        evidence["timing"]["completed_batch_elapsed_ms"],
        json!([2_250, 2_241, 2_240])
    );
    assert_eq!(
        evidence["timing"]["sigterm_elapsed_ms"],
        json!([216, 207, 208])
    );
    assert_eq!(evidence["authority"]["generation"], 1);
    assert_eq!(evidence["authority"]["restart_replays"], 2);
    for check in [
        "production_daemon_ipc_path",
        "three_shutdown_restart_cycles",
        "completed_hostile_batch_each_cycle",
        "shutdown_slow_peer_batch_each_cycle",
        "stable_returned_fd_baseline",
        "stable_returned_task_baseline",
        "sixty_four_accepted_peer_fds_observed",
        "sixty_four_scoped_reader_tasks_observed",
        "all_scoped_reader_tasks_joined",
        "each_completed_batch_within_budget",
        "each_sigterm_within_budget",
        "proc_resources_released_each_cycle",
        "runtime_owner_row_released_each_cycle",
        "unix_socket_released_each_cycle",
        "writer_generation_stable_across_restarts",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_hostile_reconnect_fairness_proof_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_authority_writer_hostile_reconnect_fairness_linux_x86_64_20260802.json",
        ))
        .expect("hostile reconnect fairness Linux evidence must exist"),
    )
    .expect("hostile reconnect fairness Linux evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.25");
    assert_eq!(evidence["target"]["endpoint"], "kyuubiki-lab.local");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(evidence["target"]["kernel"], "Linux 7.0.0-28-generic");
    assert_eq!(
        evidence["ready_prefix_test"]["name"],
        "ipc::tests::batch_dispatches_ready_prefix_before_later_slow_reader"
    );
    assert_eq!(evidence["ready_prefix_test"]["observed_duration_ms"], 70);
    assert_eq!(evidence["ready_prefix_test"]["budget_ms"], 1_000);
    assert_eq!(
        evidence["reconnect_fairness_test"]["name"],
        "burst_reconnects_remain_fair_across_repeated_saturated_hostile_waves"
    );
    assert_eq!(evidence["reconnect_fairness_test"]["waves"], 3);
    assert_eq!(evidence["reconnect_fairness_test"]["peers_per_wave"], 64);
    assert_eq!(
        evidence["reconnect_fairness_test"]["slow_peers_per_wave"],
        60
    );
    assert_eq!(
        evidence["reconnect_fairness_test"]["valid_reconnects_per_wave"],
        4
    );
    assert_eq!(
        evidence["reconnect_fairness_test"]["total_valid_reconnects"],
        12
    );
    assert_eq!(
        evidence["timing"]["wave_elapsed_ms"],
        json!([2_225, 2_197, 2_195])
    );
    assert_eq!(
        evidence["timing"]["reconnect_elapsed_ms"],
        json!([
            [2_218, 2_222, 2_223, 2_224],
            [2_193, 2_194, 2_195, 2_197],
            [2_186, 2_188, 2_193, 2_195]
        ])
    );
    assert_eq!(evidence["timing"]["maximum_reconnect_elapsed_ms"], 2_224);
    assert_eq!(evidence["authority"]["generation"], 1);
    assert_eq!(evidence["authority"]["stable_replays"], 12);
    for check in [
        "production_daemon_ipc_path",
        "strict_accept_order_dispatch",
        "ready_prefix_not_blocked_by_later_slow_reader",
        "three_repeated_saturated_waves",
        "sixty_slowloris_peers_per_wave",
        "four_valid_reconnects_per_wave",
        "all_twelve_reconnects_received_responses",
        "each_reconnect_completed_within_budget",
        "no_reconnect_starvation",
        "owner_heartbeat_advanced_after_each_wave",
        "writer_generation_stable",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_cross_transport_fairness_proof_is_non_vacuous() {
    let evidence: serde_json::Value =
        serde_json::from_str(
            &std::fs::read_to_string(repository_root().join(
                "docs/fixtures/leserpent_cross_transport_fairness_linux_x86_64_20260802.json",
            ))
            .expect("cross-transport fairness Linux evidence must exist"),
        )
        .expect("cross-transport fairness Linux evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.25");
    assert_eq!(evidence["target"]["endpoint"], "kyuubiki-lab.local");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(evidence["target"]["kernel"], "Linux 7.0.0-28-generic");
    assert_eq!(
        evidence["scheduler_test"]["name"],
        "tests::transport_scheduler_alternates_local_and_remote_priority"
    );
    assert_eq!(
        evidence["cross_transport_test"]["name"],
        "https_and_maintenance_progress_across_repeated_saturated_ipc_waves"
    );
    assert_eq!(evidence["cross_transport_test"]["waves"], 3);
    assert_eq!(
        evidence["cross_transport_test"]["slow_ipc_peers_per_wave"],
        64
    );
    assert_eq!(
        evidence["cross_transport_test"]["authenticated_https_queries"],
        3
    );
    assert_eq!(
        evidence["timing"]["https_elapsed_ms"],
        json!([2_264, 2_241, 2_226])
    );
    assert_eq!(
        evidence["timing"]["wave_elapsed_ms"],
        json!([2_265, 2_241, 2_227])
    );
    assert_eq!(evidence["authority"]["writer_generation"], 1);
    for check in [
        "production_daemon_dual_transport_path",
        "real_tls_http_request",
        "authenticated_read_only_query",
        "maintenance_precedes_transport_polling",
        "alternating_transport_priority",
        "three_repeated_saturated_ipc_waves",
        "sixty_four_slow_ipc_peers_per_wave",
        "each_https_query_completed_within_budget",
        "each_cross_transport_wave_completed_within_budget",
        "no_https_starvation",
        "owner_heartbeat_advanced_after_each_wave",
        "writer_generation_stable",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_slow_https_cross_transport_fairness_proof_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_slow_https_cross_transport_fairness_linux_x86_64_20260802.json",
        ))
        .expect("slow HTTPS cross-transport Linux evidence must exist"),
    )
    .expect("slow HTTPS cross-transport Linux evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.25");
    assert_eq!(evidence["target"]["endpoint"], "kyuubiki-lab.local");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(evidence["target"]["kernel"], "Linux 7.0.0-28-generic");
    assert_eq!(
        evidence["test"]["name"],
        "ipc_and_maintenance_progress_across_repeated_authenticated_slow_https_waves"
    );
    assert_eq!(evidence["test"]["waves"], 3);
    assert_eq!(evidence["test"]["ipc_queries_per_wave"], 4);
    assert_eq!(evidence["test"]["total_ipc_queries"], 12);
    assert_eq!(evidence["test"]["remote_read_timeout_ms"], 3_000);
    assert_eq!(evidence["slow_https_request"]["declared_content_length"], 1);
    assert_eq!(evidence["slow_https_request"]["provided_body_bytes"], 0);
    assert_eq!(evidence["slow_https_request"]["response_status"], 400);
    assert_eq!(
        evidence["timing"]["ipc_elapsed_ms"],
        json!([
            [3_042, 3_043, 3_045, 3_046],
            [3_195, 3_198, 3_199, 3_197],
            [3_143, 3_145, 3_146, 3_148]
        ])
    );
    assert_eq!(
        evidence["timing"]["slow_https_elapsed_ms"],
        json!([3_108, 3_156, 3_119])
    );
    assert_eq!(
        evidence["timing"]["wave_elapsed_ms"],
        json!([3_046, 3_199, 3_148])
    );
    assert_eq!(evidence["authority"]["writer_generation"], 1);
    for check in [
        "production_daemon_dual_transport_path",
        "real_tls_http_request",
        "valid_bearer_header",
        "incomplete_body_consumed_remote_read_budget",
        "three_repeated_slow_https_waves",
        "four_concurrent_ipc_queries_per_wave",
        "all_twelve_ipc_queries_received_responses",
        "each_ipc_query_completed_within_budget",
        "each_slow_https_peer_failed_within_budget",
        "each_cross_transport_wave_completed_within_budget",
        "no_ipc_starvation",
        "owner_heartbeat_advanced_after_each_wave",
        "writer_generation_stable",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_slow_https_sigterm_proof_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            repository_root()
                .join("docs/fixtures/leserpent_slow_https_sigterm_linux_x86_64_20260802.json"),
        )
        .expect("slow HTTPS SIGTERM Linux evidence must exist"),
    )
    .expect("slow HTTPS SIGTERM Linux evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.25");
    assert_eq!(evidence["target"]["endpoint"], "kyuubiki-lab.local");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(evidence["target"]["kernel"], "Linux 7.0.0-28-generic");
    assert_eq!(
        evidence["test"]["name"],
        "sigterm_cancels_authenticated_slow_https_and_allows_immediate_restart"
    );
    assert_eq!(evidence["test"]["connection_read_poll_interval_ms"], 100);
    assert_eq!(evidence["test"]["connection_hard_deadline_ms"], 3_000);
    assert_eq!(evidence["test"]["sigterm_budget_ms"], 1_000);
    assert_eq!(evidence["test"]["observed_shutdown_ms"], 10);
    assert_eq!(
        evidence["slow_https_request"]["application_response_suppressed"],
        true
    );
    assert_eq!(evidence["recovery"]["restart_database"], "same");
    assert_eq!(evidence["recovery"]["restart_unix_socket"], "same");
    assert_eq!(evidence["recovery"]["writer_generation"], 1);
    assert_eq!(evidence["recovery"]["writer_claim_replayed"], true);
    for check in [
        "production_daemon_dual_transport_path",
        "real_tls_http_request",
        "valid_bearer_header",
        "authenticated_body_read_active_before_sigterm",
        "cooperative_remote_read_cancellation",
        "hard_connection_deadline_preserved",
        "sigterm_completed_within_budget",
        "application_response_suppressed_after_cancellation",
        "runtime_owner_row_released",
        "unix_socket_released",
        "immediate_same_database_socket_restart",
        "writer_generation_replayed_without_allocation",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_remote_read_phase_shutdown_proof_is_non_vacuous() {
    let evidence: serde_json::Value =
        serde_json::from_str(
            &std::fs::read_to_string(repository_root().join(
                "docs/fixtures/leserpent_remote_read_phase_shutdown_linux_x86_64_20260802.json",
            ))
            .expect("remote read phase shutdown Linux evidence must exist"),
        )
        .expect("remote read phase shutdown Linux evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.25");
    assert_eq!(evidence["target"]["endpoint"], "kyuubiki-lab.local");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(evidence["target"]["kernel"], "Linux 7.0.0-28-generic");
    assert_eq!(
        evidence["test"]["name"],
        "repeated_remote_read_phase_shutdowns_preserve_process_resource_baselines"
    );
    assert_eq!(evidence["test"]["remote_read_phases"], 3);
    assert_eq!(evidence["test"]["daemon_processes"], 4);
    assert_eq!(evidence["test"]["physical_repetitions"], 3);
    assert_eq!(evidence["test"]["connection_read_poll_interval_ms"], 100);
    assert_eq!(evidence["test"]["connection_hard_deadline_ms"], 3_000);
    assert_eq!(evidence["test"]["sigterm_budget_ms"], 1_000);
    assert_eq!(
        evidence["phases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|phase| phase["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["tls-handshake", "http-header", "authenticated-body"]
    );
    assert_eq!(
        evidence["phases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|phase| phase["observed_shutdown_ms"].as_u64().unwrap())
            .collect::<Vec<_>>(),
        vec![104, 115, 110]
    );
    assert_eq!(
        evidence["timing"]["shutdown_runs_ms"],
        json!([[115, 110, 114], [113, 110, 104], [104, 115, 110]])
    );
    assert_eq!(evidence["timing"]["observed_minimum_ms"], 104);
    assert_eq!(evidence["timing"]["observed_maximum_ms"], 115);
    assert_eq!(
        evidence["final_full_group_confirmation"]["phase_shutdown_ms"],
        json!([112, 114, 105])
    );
    assert_eq!(
        evidence["final_full_group_confirmation"]["slow_https_timeout_response"],
        "http-400"
    );
    assert_eq!(
        evidence["process_resources"]["idle_open_fds"],
        json!([6, 6, 6, 6])
    );
    assert_eq!(
        evidence["process_resources"]["idle_tasks"],
        json!([1, 1, 1, 1])
    );
    assert_eq!(
        evidence["process_resources"]["active_open_fds"],
        json!([7, 7, 7])
    );
    assert_eq!(
        evidence["process_resources"]["active_tasks"],
        json!([1, 1, 1])
    );
    assert_eq!(evidence["process_resources"]["connection_fd_delta"], 1);
    assert_eq!(evidence["process_resources"]["task_delta"], 0);
    assert_eq!(evidence["recovery"]["writer_generation"], 1);
    assert_eq!(
        evidence["recovery"]["writer_claim_replayed_across_restarts"],
        true
    );
    for check in [
        "production_daemon_dual_transport_path",
        "incomplete_tls_handshake_read_active_before_sigterm",
        "incomplete_authenticated_http_header_read_active_before_sigterm",
        "authenticated_body_read_active_before_sigterm",
        "cancellation_wrapper_below_rustls",
        "would_block_absorbed_before_rustls",
        "nonretryable_connection_aborted_cancellation",
        "all_phase_shutdowns_completed_within_budget",
        "three_consecutive_physical_runs_completed",
        "final_production_cross_transport_group_passed",
        "application_response_suppressed_after_each_cancellation",
        "stable_idle_fd_baseline_across_four_processes",
        "one_active_connection_fd_per_phase",
        "no_task_amplification_per_phase",
        "proc_directory_released_after_each_phase",
        "runtime_owner_row_released_after_each_phase",
        "unix_socket_released_after_each_phase",
        "immediate_same_database_socket_restart_after_each_phase",
        "writer_generation_replayed_without_allocation",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_remote_backlog_shutdown_proof_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            repository_root()
                .join("docs/fixtures/leserpent_remote_backlog_shutdown_linux_x86_64_20260802.json"),
        )
        .expect("remote backlog shutdown Linux evidence must exist"),
    )
    .expect("remote backlog shutdown Linux evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.25");
    assert_eq!(evidence["target"]["endpoint"], "kyuubiki-lab.local");
    assert_eq!(evidence["target"]["hostname"], "kyuubiki-lab");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(evidence["target"]["kernel"], "Linux 7.0.0-28-generic");
    assert_eq!(
        evidence["test"]["name"],
        "mixed_remote_read_phases_with_listener_backlog_preserve_bounded_shutdown_and_authority"
    );
    assert_eq!(evidence["test"]["remote_read_phases"], 3);
    assert_eq!(evidence["test"]["daemon_processes"], 4);
    assert_eq!(evidence["test"]["backlog_peers_per_phase"], 64);
    assert_eq!(evidence["test"]["connection_read_poll_interval_ms"], 100);
    assert_eq!(evidence["test"]["connection_hard_deadline_ms"], 3_000);
    assert_eq!(evidence["test"]["sigterm_budget_ms"], 1_000);
    assert_eq!(
        evidence["phases"]
            .as_array()
            .unwrap()
            .iter()
            .map(|phase| phase["name"].as_str().unwrap())
            .collect::<Vec<_>>(),
        vec!["tls-handshake", "http-header", "authenticated-body"]
    );
    assert!(
        evidence["phases"]
            .as_array()
            .unwrap()
            .iter()
            .all(|phase| phase["backlog_peers"] == 64)
    );
    assert_eq!(
        evidence["timing"]["shutdown_runs_ms"],
        json!([93, 110, 100])
    );
    assert_eq!(evidence["timing"]["observed_minimum_ms"], 93);
    assert_eq!(evidence["timing"]["observed_maximum_ms"], 110);
    assert_eq!(
        evidence["process_resources"]["idle_open_fds"],
        json!([6, 6, 6, 6])
    );
    assert_eq!(
        evidence["process_resources"]["idle_tasks"],
        json!([1, 1, 1, 1])
    );
    assert_eq!(
        evidence["process_resources"]["active_open_fds"],
        json!([7, 7, 7])
    );
    assert_eq!(
        evidence["process_resources"]["backlog_open_fds"],
        json!([7, 7, 7])
    );
    assert_eq!(
        evidence["process_resources"]["backlog_tasks"],
        json!([1, 1, 1])
    );
    assert_eq!(
        evidence["process_resources"]["active_connection_fd_delta"],
        1
    );
    assert_eq!(evidence["process_resources"]["backlog_daemon_fd_delta"], 0);
    assert_eq!(evidence["process_resources"]["task_delta"], 0);
    assert_eq!(evidence["recovery"]["writer_generation_before_backlog"], 1);
    assert_eq!(evidence["recovery"]["writer_generation_after_backlog"], 1);
    assert_eq!(evidence["recovery"]["writer_generation_after_restart"], 1);
    for check in [
        "production_daemon_dual_transport_path",
        "active_read_phase_rotated_across_tls_header_and_body",
        "sixty_four_incomplete_tls_peers_queued_per_phase",
        "all_phase_shutdowns_completed_within_budget",
        "application_response_suppressed_after_each_cancellation",
        "stable_idle_fd_baseline_across_four_processes",
        "one_active_connection_fd_per_phase",
        "listener_backlog_added_zero_daemon_fds",
        "listener_backlog_added_zero_daemon_tasks",
        "proc_directory_released_after_each_phase",
        "runtime_owner_row_released_after_each_phase",
        "unix_socket_released_after_each_phase",
        "immediate_same_database_socket_restart_after_each_phase",
        "backlog_allocated_zero_authority_generations",
        "writer_generation_replayed_without_allocation",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_maximum_event_session_shutdown_proof_is_non_vacuous() {
    let evidence: serde_json::Value =
        serde_json::from_str(
            &std::fs::read_to_string(repository_root().join(
                "docs/fixtures/leserpent_max_event_session_shutdown_linux_x86_64_20260802.json",
            ))
            .expect("maximum event session shutdown Linux evidence must exist"),
        )
        .expect("maximum event session shutdown Linux evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.124.25");
    assert_eq!(evidence["target"]["endpoint"], "kyuubiki-lab.local");
    assert_eq!(evidence["target"]["hostname"], "kyuubiki-lab");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(evidence["target"]["kernel"], "Linux 7.0.0-28-generic");
    assert_eq!(
        evidence["test"]["name"],
        "maximum_event_sessions_with_stalled_request_preserve_bounded_shutdown_and_resources"
    );
    assert_eq!(evidence["test"]["authenticated_event_sessions"], 32);
    assert_eq!(evidence["test"]["active_stalled_requests"], 1);
    assert_eq!(evidence["test"]["daemon_processes"], 2);
    assert_eq!(evidence["test"]["connection_read_poll_interval_ms"], 100);
    assert_eq!(evidence["test"]["connection_hard_deadline_ms"], 3_000);
    assert_eq!(evidence["test"]["sigterm_budget_ms"], 1_000);
    assert_eq!(evidence["event_sessions"]["transport"], "tls-websocket");
    assert_eq!(evidence["event_sessions"]["route"], "/v1/events");
    assert_eq!(
        evidence["event_sessions"]["subprotocol"],
        "leserpent.events.v1"
    );
    assert_eq!(evidence["event_sessions"]["initial_events_consumed"], 32);
    assert_eq!(
        evidence["event_sessions"]["queued_application_events_drained_before_sigterm"],
        0
    );
    assert_eq!(
        evidence["event_sessions"]["application_events_after_sigterm"],
        0
    );
    assert_eq!(evidence["timing"]["observed_shutdown_ms"], 111);
    assert_eq!(evidence["timing"]["sigterm_budget_ms"], 1_000);
    assert_eq!(evidence["process_resources"]["idle_open_fds"], 6);
    assert_eq!(evidence["process_resources"]["idle_tasks"], 1);
    assert_eq!(
        evidence["process_resources"]["maximum_event_session_open_fds"],
        38
    );
    assert_eq!(
        evidence["process_resources"]["maximum_event_session_tasks"],
        1
    );
    assert_eq!(evidence["process_resources"]["event_session_fd_delta"], 32);
    assert_eq!(evidence["process_resources"]["event_session_task_delta"], 0);
    assert_eq!(
        evidence["process_resources"]["event_sessions_plus_stalled_request_open_fds"],
        39
    );
    assert_eq!(
        evidence["process_resources"]["event_sessions_plus_stalled_request_tasks"],
        1
    );
    assert_eq!(evidence["process_resources"]["stalled_request_fd_delta"], 1);
    assert_eq!(evidence["process_resources"]["restart_idle_open_fds"], 6);
    assert_eq!(evidence["process_resources"]["restart_idle_tasks"], 1);
    assert_eq!(evidence["recovery"]["writer_generation_before_sessions"], 1);
    assert_eq!(
        evidence["recovery"]["writer_generation_during_saturation"],
        1
    );
    assert_eq!(evidence["recovery"]["writer_generation_after_restart"], 1);
    for check in [
        "production_daemon_dual_transport_path",
        "maximum_32_authenticated_event_sessions_established",
        "all_initial_runtime_snapshots_consumed",
        "pre_sigterm_application_event_queue_drained",
        "exact_one_fd_per_event_session",
        "zero_task_amplification_at_maximum_sessions",
        "stalled_request_added_exactly_one_fd",
        "stalled_request_added_zero_tasks",
        "sigterm_completed_within_budget",
        "stalled_application_response_suppressed",
        "late_application_events_suppressed",
        "all_event_sessions_closed_after_sigterm",
        "proc_directory_released",
        "runtime_owner_row_released",
        "unix_socket_released",
        "immediate_same_database_socket_restart",
        "restart_returned_to_idle_resource_baseline",
        "zero_authority_generation_allocation",
        "writer_generation_replayed_without_allocation",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_authority_writer_lost_response_race_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_authority_writer_lost_response_race_linux_x86_64_20260801.json",
        ))
        .expect("authority writer lost-response evidence must exist"),
    )
    .expect("authority writer lost-response evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.1.46");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(
        evidence["test"]["name"],
        "lost_claim_response_race_is_linearizable_for_same_and_competing_writers"
    );
    assert_eq!(evidence["fault"]["initial_generation"], 1);
    assert_eq!(evidence["fault"]["response_bytes_decoded"], false);
    assert_eq!(evidence["race"]["observed_order"], "competing-then-same");
    assert_eq!(evidence["race"]["competing_result"]["generation"], 2);
    assert_eq!(evidence["race"]["same_writer_result"]["generation"], 3);
    assert_eq!(evidence["race"]["same_writer_result"]["replayed"], false);
    assert_eq!(evidence["race"]["final_generation"], 3);
    for check in [
        "production_daemon_ipc_path",
        "initial_claim_durable_without_caller_response",
        "simultaneous_independent_clients",
        "same_writer_not_false_replay_after_competitor",
        "unique_generation_writer_order",
        "stale_competing_ticket_rejected",
        "final_ticket_mutation_applied",
        "final_same_id_replay_stable",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_authority_writer_claim_crash_evidence_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_authority_writer_claim_crash_linux_x86_64_20260801.json",
        ))
        .expect("authority writer claim crash evidence must exist"),
    )
    .expect("authority writer claim crash evidence must decode");
    assert_eq!(evidence["target"]["host"], "192.168.1.46");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(
        evidence["test"]["name"],
        "sigkill_at_writer_claim_commit_preserves_an_atomic_generation"
    );
    assert_eq!(evidence["test"]["owner_lease_duration_ms"], 30_000);
    assert_eq!(
        evidence["boundaries"]["pre_commit"]["recovered_generation"],
        1
    );
    assert_eq!(evidence["boundaries"]["natural_takeover"]["generation"], 2);
    assert_eq!(
        evidence["boundaries"]["post_commit"]["recovered_generation"],
        3
    );
    for check in [
        "production_control_runtime_claim_path",
        "deterministic_pre_commit_block",
        "rollback_journal_recovery",
        "sqlite_integrity_check_ok",
        "pre_expiry_replacement_rejected",
        "natural_owner_lease_expiry_observed",
        "stale_generation_rejected_after_takeover",
        "post_commit_generation_durable_after_sigkill",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
}

#[test]
fn retained_linux_avalonia_presentation_native_aot_evidence_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_avalonia_presentation_native_aot_linux_x86_64_20260809.json",
        ))
        .expect("Avalonia Linux NativeAOT presentation evidence must exist"),
    )
    .expect("Avalonia Linux NativeAOT presentation evidence must decode");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(
        evidence["proof"],
        "leserpent-avalonia-presentation-native-aot-linux-x86_64"
    );
    assert_eq!(evidence["target"]["host"], "192.168.124.22");
    assert_eq!(evidence["target"]["hostname"], "kyuubiki-lab");
    assert_eq!(evidence["target"]["kernel"], "Linux 7.0.0-28-generic");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(evidence["target"]["execution"], "physical-host-xvfb");
    assert_eq!(evidence["toolchain"]["dotnet_sdk"], "10.0.110");
    assert_eq!(evidence["toolchain"]["rid"], "linux-x64");
    assert_eq!(
        evidence["toolchain"]["restore"],
        "locked-complete-dual-rid-graph"
    );

    let sha256 = evidence["artifact"]["sha256"]
        .as_str()
        .expect("artifact SHA-256 must be a string");
    assert_eq!(sha256.len(), 64);
    assert!(sha256.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(evidence["artifact"]["bytes"].as_u64().unwrap() > 20_000_000);
    assert_eq!(evidence["artifact"]["stripped"], true);
    assert_eq!(evidence["suites"]["rust_unit_tests"], 281);
    assert_eq!(evidence["suites"]["lifecycle_gate_tests"], 14);
    assert_eq!(evidence["suites"]["status_gate_tests"], 54);
    assert_eq!(evidence["suites"]["repository_gate_tests"], 68);
    assert_eq!(evidence["suites"]["presentation_atoms"], 54);
    assert_eq!(evidence["suites"]["presentation_atom_profiles"], 54);
    assert_eq!(evidence["suites"]["renderer_conformance"], true);
    assert_eq!(evidence["observations"]["wait_unfocused_timeout"], true);
    assert_eq!(
        evidence["observations"]["wait_unfocused_external_deactivation"],
        false
    );
    assert_eq!(
        evidence["observations"]["external_deactivation_required"],
        false
    );
    for check in [
        "locked_dual_rid_restore",
        "linux_x64_native_aot_publish",
        "xvfb_real_gui_execution",
        "strict_cross_language_codec",
        "all_required_presentation_markers_passed",
        "window_lifecycle_passed",
        "focus_navigation_passed",
        "virtualization_passed",
        "child_count_assertion_passed",
        "child_count_external_patch_wait_passed",
        "child_count_persistent_mismatch_timeout_passed",
        "child_count_virtualization_preserved",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
    assert_eq!(evidence["result"], "passed");
}

#[test]
fn retained_linux_avalonia_activation_native_aot_evidence_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_avalonia_activation_native_aot_linux_x86_64_20260809.json",
        ))
        .expect("Avalonia Linux NativeAOT activation evidence must exist"),
    )
    .expect("Avalonia Linux NativeAOT activation evidence must decode");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(
        evidence["proof"],
        "leserpent-avalonia-activation-native-aot-linux-x86_64"
    );
    assert_eq!(evidence["target"]["host"], "192.168.124.22");
    assert_eq!(evidence["target"]["hostname"], "kyuubiki-lab");
    assert_eq!(evidence["target"]["kernel"], "Linux 7.0.0-28-generic");
    assert_eq!(evidence["target"]["execution"], "physical-host-xvfb");
    assert_eq!(evidence["toolchain"]["dotnet_sdk"], "10.0.110");
    assert_eq!(evidence["toolchain"]["rid"], "linux-x64");
    assert_eq!(
        evidence["artifact"]["sha256"],
        "7b3d5590ceee5ddfb5f706845dcce7882d5089ab600b34f3a36c3499f87a8b71"
    );
    assert!(evidence["artifact"]["bytes"].as_u64().unwrap() > 20_000_000);
    assert_eq!(evidence["artifact"]["debug_symbol_bytes"], 0);
    assert_eq!(evidence["artifact"]["stripped"], true);
    assert_eq!(evidence["suites"]["rust_unit_tests"], 286);
    assert_eq!(evidence["suites"]["lifecycle_gate_tests"], 14);
    assert_eq!(
        evidence["suites"]["native_validation_harness_gate_tests"],
        29
    );
    assert_eq!(evidence["suites"]["status_gate_tests"], 55);
    assert_eq!(evidence["suites"]["repository_gate_tests"], 69);
    assert_eq!(evidence["suites"]["presentation_atoms"], 55);
    assert_eq!(evidence["suites"]["presentation_atom_profiles"], 55);
    assert_eq!(evidence["suites"]["native_aot_control_fixtures"], 4);
    assert_eq!(evidence["suites"]["native_aot_presentation_fixture"], true);
    assert_eq!(evidence["suites"]["renderer_conformance"], true);
    for observation in [
        "presentation_activate",
        "native_click_exactly_once",
        "unavailable_action_rejected",
        "hidden_action_rejected",
        "non_action_rejected",
        "missing_action_rejected",
        "wait_unfocused_timeout",
    ] {
        assert_eq!(
            evidence["observations"][observation], true,
            "missing observation {observation}"
        );
    }
    assert_eq!(
        evidence["observations"]["wait_unfocused_external_deactivation"],
        false
    );
    assert_eq!(
        evidence["observations"]["external_deactivation_required"],
        false
    );
    for check in [
        "locked_dual_rid_restore",
        "linux_x64_native_aot_publish",
        "xvfb_real_gui_execution",
        "strict_cross_language_codec",
        "complete_55_atom_manifest",
        "mode_scoped_control_fixture",
        "native_presentation_fixture",
        "native_action_activation",
        "single_native_click_route",
        "invalid_activation_fail_closed",
        "all_required_presentation_markers_passed",
        "window_lifecycle_passed",
        "focus_navigation_passed",
        "virtualization_passed",
        "child_count_virtualization_preserved",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
    assert_eq!(evidence["result"], "passed");
}

#[test]
fn retained_linux_avalonia_window_reopen_native_aot_evidence_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_avalonia_window_reopen_native_aot_linux_x86_64_20260809.json",
        ))
        .expect("Avalonia Linux NativeAOT window-reopen evidence must exist"),
    )
    .expect("Avalonia Linux NativeAOT window-reopen evidence must decode");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(
        evidence["proof"],
        "leserpent-avalonia-window-reopen-native-aot-linux-x86_64"
    );
    assert_eq!(evidence["source"]["renderer_contract"], "1.86.0");
    for source_hash in ["renderer_sha256", "window_probe_sha256"] {
        let hash = evidence["source"][source_hash]
            .as_str()
            .unwrap_or_else(|| panic!("source hash {source_hash} must be a string"));
        assert_eq!(hash.len(), 64, "source hash {source_hash} must be SHA-256");
        assert!(
            hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "source hash {source_hash} must be hexadecimal"
        );
    }
    assert_eq!(evidence["target"]["host"], "192.168.124.22");
    assert_eq!(evidence["target"]["endpoint"], "kyuubiki-lab.local");
    assert_eq!(evidence["target"]["alias"], "kyuubiki-lab");
    assert_eq!(evidence["target"]["hostname"], "kyuubiki-lab");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(evidence["target"]["execution"], "physical-host-xvfb");
    assert_eq!(evidence["toolchain"]["dotnet_sdk"], "10.0.110");
    assert_eq!(evidence["toolchain"]["rid"], "linux-x64");

    let artifact_sha256 = evidence["artifact"]["sha256"]
        .as_str()
        .expect("artifact SHA-256 must be a string");
    assert_eq!(artifact_sha256.len(), 64);
    assert!(artifact_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(evidence["artifact"]["bytes"].as_u64().unwrap() > 20_000_000);
    assert!(evidence["artifact"]["files"].as_u64().unwrap() <= 12);
    assert_eq!(evidence["artifact"]["debug_symbol_bytes"], 0);
    assert_eq!(evidence["artifact"]["stripped"], true);
    assert_eq!(evidence["suites"]["lifecycle_gate_tests"], 14);
    assert_eq!(
        evidence["suites"]["native_validation_harness_gate_tests"],
        29
    );
    assert_eq!(evidence["suites"]["status_gate_tests"], 55);
    assert_eq!(evidence["suites"]["presentation_atoms"], 55);
    assert_eq!(evidence["suites"]["presentation_atom_profiles"], 55);
    assert_eq!(evidence["suites"]["native_aot_control_fixtures"], 4);
    assert_eq!(evidence["suites"]["native_aot_presentation_fixture"], true);
    for observation in [
        "presentation_activate",
        "native_click_exactly_once",
        "open_window",
        "close_window",
        "reopen_window",
        "reclose_window",
        "window_lifecycle_idempotent",
        "window_reopen_fresh_native_window",
        "window_semantic_tree_rematerialized",
        "window_lifecycle_state_observed",
    ] {
        assert_eq!(
            evidence["observations"][observation], true,
            "missing observation {observation}"
        );
    }
    for check in [
        "source_hash_match",
        "locked_dual_rid_restore",
        "linux_x64_native_aot_publish",
        "xvfb_real_gui_execution",
        "strict_cross_language_codec",
        "complete_55_atom_manifest",
        "four_independent_control_fixtures",
        "native_presentation_fixture",
        "native_open_close_reopen_reclose",
        "duplicate_window_lifecycle_idempotency",
        "fresh_native_window_reopen",
        "stable_semantic_identity_after_rematerialization",
        "all_required_presentation_markers_passed",
        "secret_free_evidence",
    ] {
        assert_eq!(evidence["checks"][check], true, "missing check {check}");
    }
    assert_eq!(evidence["result"], "passed");
}

#[test]
fn retained_linux_silvortex_oidc_provider_shadow_evidence_is_non_vacuous() {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(
            "docs/fixtures/leserpent_silvortex_oidc_provider_shadow_linux_x86_64_20260810.json",
        ))
        .expect("Silvortex OIDC provider shadow evidence must exist"),
    )
    .expect("Silvortex OIDC provider shadow evidence must decode");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(
        evidence["proof"],
        "leserpent-silvortex-reviewed-oidc-provider-shadow-linux-x86_64"
    );
    assert_eq!(evidence["source"]["renderer_contract"], "1.87.0");
    for source_hash in [
        "silvortex_application_profile_migration_sha256",
        "silvortex_identity_shadow_smoke_sha256",
        "silvortex_identity_policy_sha256",
        "leserpent_account_session_sha256",
    ] {
        let hash = evidence["source"][source_hash]
            .as_str()
            .unwrap_or_else(|| panic!("source hash {source_hash} must be a string"));
        assert_eq!(hash.len(), 64, "source hash {source_hash} must be SHA-256");
        assert!(
            hash.bytes().all(|byte| byte.is_ascii_hexdigit()),
            "source hash {source_hash} must be hexadecimal"
        );
    }
    assert_eq!(evidence["source"]["remote_source_hashes_match"], true);
    assert_eq!(evidence["target"]["host"], "192.168.124.22");
    assert_eq!(evidence["target"]["endpoint"], "kyuubiki-lab.local");
    assert_eq!(evidence["target"]["alias"], "kyuubiki-lab");
    assert_eq!(evidence["target"]["hostname"], "kyuubiki-lab");
    assert_eq!(evidence["target"]["architecture"], "x86_64");
    assert_eq!(
        evidence["target"]["execution"],
        "physical-host-disposable-docker"
    );
    assert_eq!(evidence["registration"]["application_key"], "leserpent");
    assert_eq!(
        evidence["registration"]["client_profile"],
        "leserpent_desktop"
    );
    assert_eq!(
        evidence["registration"]["client_id"],
        "svx_client_leserpent_desktop"
    );
    assert_eq!(evidence["registration"]["client_kind"], "native");
    assert_eq!(evidence["registration"]["confidential"], false);
    assert_eq!(evidence["registration"]["client_secret_present"], false);
    assert_eq!(
        evidence["registration"]["redirect_uris"],
        serde_json::json!(["http://127.0.0.1:43817/oidc/callback"])
    );
    assert_eq!(
        evidence["registration"]["scopes"],
        serde_json::json!(["openid", "profile", "email", "offline_access"])
    );
    assert_eq!(evidence["suite"]["exit_code"], 0);
    assert_eq!(evidence["suite"]["checks"], 21);
    assert_eq!(evidence["suite"]["isolated_resources_cleaned"], true);
    let log_sha256 = evidence["suite"]["log_sha256"]
        .as_str()
        .expect("retained remote log SHA-256 must be a string");
    assert_eq!(log_sha256.len(), 64);
    assert!(log_sha256.bytes().all(|byte| byte.is_ascii_hexdigit()));
    for observation in [
        "reviewed_profile_migration_applied",
        "reviewed_static_client_selected",
        "oidc_discovery",
        "exact_redirect_validation",
        "authorization_code_pkce_s256",
        "rs256_signed_id_token",
        "mfa_assurance",
        "userinfo_subject_binding",
        "scope_narrowing_refresh_rotation",
        "refresh_replay_containment",
        "public_client_secret_rejected",
        "consent_revocation",
    ] {
        assert_eq!(
            evidence["observations"][observation], true,
            "missing provider observation {observation}"
        );
    }
    assert_eq!(evidence["boundaries"]["provider_shadow_only"], true);
    for boundary in [
        "native_system_browser_proof",
        "platform_credential_vault_restore_proof",
        "native_logout_proof",
        "account_identity_replaces_daemon_authority",
        "retained_secrets",
    ] {
        assert_eq!(
            evidence["boundaries"][boundary], false,
            "boundary {boundary} must remain false"
        );
    }
    assert_eq!(evidence["result"], "passed");
}

#[test]
fn retained_system_profile_bootstrap_retirement_evidence_is_non_vacuous() {
    let root = repository_root();
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            root.join("docs/fixtures/leserpent_real_ssh_system_profile_retirement_20260728.json"),
        )
        .expect("system-profile bootstrap retirement evidence must exist"),
    )
    .expect("system-profile bootstrap retirement evidence must decode");

    assert_eq!(evidence["target"]["os"], "Ubuntu 24.04.4");
    assert_eq!(evidence["target"]["arch"], "x86_64");
    assert_eq!(evidence["target"]["service_manager"], "systemd-system");
    assert_eq!(evidence["target"]["install_profile"], "system");
    assert_eq!(
        evidence["transport"]["system_elevation_command"],
        "/usr/bin/sudo -n --"
    );
    assert_eq!(evidence["transport"]["password_over_stdin"], false);
    assert_eq!(evidence["transport"]["secret_in_argv"], false);
    assert_eq!(
        evidence["privilege_policy"]["general_shell_authority"],
        false
    );
    assert_eq!(
        evidence["privilege_policy"]["general_systemctl_authority"],
        false
    );
    assert_eq!(
        evidence["privilege_policy"]["policy_removed_after_proof"],
        true
    );
    assert_eq!(
        evidence["privilege_policy"]["passwordless_sudo_exit_after_revocation"],
        1
    );
    for field in [
        "service_ready_before_retirement",
        "session_bound_before_retirement",
        "trust_persistence_failure_withheld_authority",
        "one_millisecond_timeout_rejected",
        "forged_generation_rejected",
        "identity_bound_retirement_succeeded",
        "exact_retirement_replay_succeeded",
        "retired_generation_reinstall_rejected",
        "system_service_descriptor_absent",
        "service_inactive",
        "service_disabled_or_absent",
        "daemon_process_absent",
        "endpoint_port_clear",
        "staging_absent",
        "remote_source_removed",
        "local_artifact_removed",
        "local_trust_root_removed",
    ] {
        assert_eq!(
            evidence["proof"][field], true,
            "missing proof field {field}"
        );
    }
    assert_eq!(evidence["proof"]["test_identity_residue_after_cleanup"], 0);
    assert_eq!(evidence["secrets_retained"], false);
}

#[test]
fn retained_runtime_deletion_crash_evidence_is_non_vacuous() {
    assert_runtime_deletion_crash_evidence(
        "docs/fixtures/leserpent_runtime_deletion_crash_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_crash_evidence(
        "docs/fixtures/leserpent_runtime_deletion_crash_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_checkpoint_worker_duplicate_host_evidence_is_non_vacuous() {
    let root = repository_root();
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(root.join(
            "docs/fixtures/leserpent_checkpoint_worker_duplicate_host_linux_x86_64_20260727.json",
        ))
        .expect("checkpoint worker duplicate-host evidence must exist"),
    )
    .expect("checkpoint worker duplicate-host evidence must decode");
    assert_eq!(evidence["schemaVersion"], 2);
    assert_eq!(
        evidence["campaign"],
        "leserpent_checkpoint_worker_duplicate_host"
    );
    assert_eq!(evidence["architecture"], "x64");
    assert_eq!(evidence["firstHost"]["workerState"], "owner");
    assert_eq!(evidence["firstHost"]["leaseHeld"], true);
    assert_eq!(evidence["secondHost"]["workerState"], "standby");
    assert_eq!(evidence["secondHost"]["leaseHeld"], false);
    assert_eq!(
        evidence["standbyAfterOwnerTermination"]["workerState"],
        "standby"
    );
    assert_eq!(evidence["freshProcessTakeover"]["workerState"], "owner");
    assert_eq!(evidence["freshProcessTakeover"]["leaseHeld"], true);
    assert_eq!(
        evidence["controlPlaneWriter"]["firstHost"]["state"],
        "owner"
    );
    assert_eq!(
        evidence["controlPlaneWriter"]["firstHost"]["saveStatus"],
        200
    );
    assert_eq!(
        evidence["controlPlaneWriter"]["secondHost"]["state"],
        "standby"
    );
    assert_eq!(
        evidence["controlPlaneWriter"]["secondHost"]["saveStatus"],
        409
    );
    assert_eq!(
        evidence["controlPlaneWriter"]["standbyAfterOwnerTermination"]["saveStatus"],
        409
    );
    assert_eq!(
        evidence["controlPlaneWriter"]["freshProcessTakeover"]["state"],
        "owner"
    );
    assert_eq!(
        evidence["controlPlaneWriter"]["freshProcessTakeover"]["saveStatus"],
        200
    );
    assert_eq!(
        evidence["controlPlaneWriter"]["fixedStandbyError"],
        "control_plane_writer_standby"
    );
    assert_eq!(evidence["ownerCountBeforeTermination"], 1);
    assert_eq!(evidence["authenticatedHealthEndpoint"], true);
    assert_eq!(evidence["secretFreeHealthPayload"], true);
}

#[test]
fn retained_runtime_deletion_fault_campaigns_are_non_vacuous() {
    assert_runtime_deletion_fault_campaign(
        "docs/fixtures/leserpent_runtime_deletion_fault_campaign_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_fault_campaign(
        "docs/fixtures/leserpent_runtime_deletion_fault_campaign_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_concurrency_campaigns_are_non_vacuous() {
    assert_runtime_deletion_concurrency_campaign(
        "docs/fixtures/leserpent_runtime_deletion_concurrency_campaign_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_concurrency_campaign(
        "docs/fixtures/leserpent_runtime_deletion_concurrency_campaign_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_daemon_restart_campaigns_are_non_vacuous() {
    assert_runtime_deletion_daemon_restart_campaign(
        "docs/fixtures/leserpent_runtime_deletion_daemon_restart_campaign_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_daemon_restart_campaign(
        "docs/fixtures/leserpent_runtime_deletion_daemon_restart_campaign_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_unclean_takeovers_are_non_vacuous() {
    assert_runtime_deletion_unclean_takeover(
        "docs/fixtures/leserpent_runtime_deletion_unclean_takeover_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_unclean_takeover(
        "docs/fixtures/leserpent_runtime_deletion_unclean_takeover_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_overlapping_takeovers_are_non_vacuous() {
    assert_runtime_deletion_overlapping_takeover(
        "docs/fixtures/leserpent_runtime_deletion_overlapping_takeover_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_overlapping_takeover(
        "docs/fixtures/leserpent_runtime_deletion_overlapping_takeover_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_repeated_takeovers_are_non_vacuous() {
    assert_runtime_deletion_repeated_takeover(
        "docs/fixtures/leserpent_runtime_deletion_repeated_takeover_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_repeated_takeover(
        "docs/fixtures/leserpent_runtime_deletion_repeated_takeover_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_poison_isolation_is_non_vacuous() {
    assert_runtime_deletion_poison_isolation(
        "docs/fixtures/leserpent_runtime_deletion_poison_isolation_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_poison_isolation(
        "docs/fixtures/leserpent_runtime_deletion_poison_isolation_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_high_cardinality_evidence_is_non_vacuous() {
    assert_runtime_deletion_high_cardinality(
        "docs/fixtures/leserpent_runtime_deletion_high_cardinality_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_high_cardinality(
        "docs/fixtures/leserpent_runtime_deletion_high_cardinality_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_batch_persistence_evidence_is_non_vacuous() {
    assert_runtime_deletion_batch_persistence(
        "docs/fixtures/leserpent_runtime_deletion_batch_persistence_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_batch_persistence(
        "docs/fixtures/leserpent_runtime_deletion_batch_persistence_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_saturated_queue_evidence_is_non_vacuous() {
    assert_runtime_deletion_saturated_queue(
        "docs/fixtures/leserpent_runtime_deletion_saturated_queue_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_saturated_queue(
        "docs/fixtures/leserpent_runtime_deletion_saturated_queue_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_retry_claim_races_are_non_vacuous() {
    assert_runtime_deletion_retry_claim_race(
        "docs/fixtures/leserpent_runtime_deletion_retry_claim_race_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_retry_claim_race(
        "docs/fixtures/leserpent_runtime_deletion_retry_claim_race_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_retry_crashes_are_non_vacuous() {
    assert_runtime_deletion_retry_crash(
        "docs/fixtures/leserpent_runtime_deletion_retry_crash_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_retry_crash(
        "docs/fixtures/leserpent_runtime_deletion_retry_crash_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_lost_acknowledgements_are_non_vacuous() {
    assert_runtime_deletion_lost_acknowledgement(
        "docs/fixtures/leserpent_runtime_deletion_lost_ack_20260726.json",
        "Arm64",
    );
    assert_runtime_deletion_lost_acknowledgement(
        "docs/fixtures/leserpent_runtime_deletion_lost_ack_linux_x86_64_20260726.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_replay_horizon_fail_closed_is_non_vacuous() {
    assert_runtime_deletion_replay_horizon(
        "docs/fixtures/leserpent_runtime_deletion_replay_horizon_20260726.json",
        "Arm64",
    );
    assert_runtime_deletion_replay_horizon(
        "docs/fixtures/leserpent_runtime_deletion_replay_horizon_linux_x86_64_20260726.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_reconciliation_commits_are_non_vacuous() {
    assert_runtime_deletion_reconciliation_commit(
        "docs/fixtures/leserpent_runtime_deletion_reconciliation_commit_20260726.json",
        "Arm64",
    );
    assert_runtime_deletion_reconciliation_commit(
        "docs/fixtures/leserpent_runtime_deletion_reconciliation_commit_linux_x86_64_20260726.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_cross_authority_convergence_is_non_vacuous() {
    assert_runtime_deletion_cross_authority(
        "docs/fixtures/leserpent_runtime_deletion_cross_authority_20260726.json",
        "Arm64",
    );
    assert_runtime_deletion_cross_authority(
        "docs/fixtures/leserpent_runtime_deletion_cross_authority_linux_x86_64_20260726.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_retry_rollovers_are_non_vacuous() {
    assert_runtime_deletion_retry_rollover(
        "docs/fixtures/leserpent_runtime_deletion_retry_rollover_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_retry_rollover(
        "docs/fixtures/leserpent_runtime_deletion_retry_rollover_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_retry_atomic_rollovers_are_non_vacuous() {
    assert_runtime_deletion_retry_atomic_rollover(
        "docs/fixtures/leserpent_runtime_deletion_retry_atomic_rollover_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_retry_atomic_rollover(
        "docs/fixtures/leserpent_runtime_deletion_retry_atomic_rollover_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_retry_atomic_backups_are_non_vacuous() {
    assert_runtime_deletion_retry_atomic_backup(
        "docs/fixtures/leserpent_runtime_deletion_retry_atomic_backup_20260723.json",
        "Arm64",
    );
    assert_runtime_deletion_retry_atomic_backup(
        "docs/fixtures/leserpent_runtime_deletion_retry_atomic_backup_linux_x86_64_20260723.json",
        "X64",
    );
}

#[test]
fn retained_runtime_deletion_retry_post_recovery_writes_are_non_vacuous() {
    assert_runtime_deletion_retry_post_recovery_write(
        "docs/fixtures/leserpent_runtime_deletion_retry_post_recovery_write_20260723.json",
        "Arm64",
        "invalid_json",
    );
    assert_runtime_deletion_retry_post_recovery_write(
        "docs/fixtures/leserpent_runtime_deletion_retry_post_recovery_write_linux_x86_64_20260723.json",
        "X64",
        "invalid_json",
    );
}

#[test]
fn retained_runtime_deletion_retry_semantic_generations_are_non_vacuous() {
    assert_runtime_deletion_retry_post_recovery_write(
        "docs/fixtures/leserpent_runtime_deletion_retry_semantic_generation_20260723.json",
        "Arm64",
        "semantic_invalid",
    );
    assert_runtime_deletion_retry_post_recovery_write(
        "docs/fixtures/leserpent_runtime_deletion_retry_semantic_generation_linux_x86_64_20260723.json",
        "X64",
        "semantic_invalid",
    );
}

fn assert_runtime_deletion_crash_evidence(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion crash evidence must exist"),
    )
    .expect("runtime deletion crash evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert!(
        evidence["observed_at"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(
        evidence["platform"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(evidence["architecture"], expected_architecture);
    for check in [
        "real_leserpentd",
        "daemon_unregistration_committed",
        "host_process_force_killed",
        "durable_intent_restored",
        "protected_runtime_restored",
        "background_recovery_converged",
        "daemon_and_compatibility_state_absent",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained crash proof {check}"
        );
    }
}

fn assert_runtime_deletion_fault_campaign(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion fault campaign evidence must exist"),
    )
    .expect("runtime deletion fault campaign evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert!(
        evidence["observed_at"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert!(
        evidence["platform"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(evidence["architecture"], expected_architecture);
    assert!(
        evidence["iterations_per_phase"]
            .as_u64()
            .is_some_and(|value| value >= 3)
    );
    assert_eq!(
        evidence["total_forced_terminations"].as_u64(),
        evidence["iterations_per_phase"]
            .as_u64()
            .map(|iterations| iterations * 3)
    );
    assert_eq!(
        evidence["phases"],
        serde_json::json!([
            "intent_persisted",
            "daemon_committed",
            "local_cleanup_persisted"
        ])
    );
    for check in [
        "real_leserpentd",
        "every_durable_transition_covered",
        "every_host_process_force_killed",
        "every_intent_restored",
        "every_protected_runtime_rejected_new_work",
        "every_background_recovery_converged",
        "every_daemon_and_compatibility_state_absent",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained fault campaign proof {check}"
        );
    }
}

fn assert_runtime_deletion_concurrency_campaign(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion concurrency campaign evidence must exist"),
    )
    .expect("runtime deletion concurrency campaign evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert!(
        evidence["observed_at"]
            .as_str()
            .is_some_and(|value| !value.is_empty())
    );
    assert_eq!(evidence["architecture"], expected_architecture);
    let iterations = evidence["iterations_per_phase"]
        .as_u64()
        .expect("concurrency campaign iteration count must be numeric");
    assert!(iterations >= 3);
    assert_eq!(evidence["total_forced_terminations"], iterations * 3);
    assert_eq!(evidence["interference_runtimes_per_scenario"], 8);
    assert_eq!(
        evidence["total_interference_registrations"],
        iterations * 3 * 8
    );
    assert_eq!(
        evidence["phases"],
        serde_json::json!([
            "intent_persisted",
            "daemon_committed",
            "local_cleanup_persisted"
        ])
    );
    for check in [
        "real_leserpentd",
        "every_durable_transition_covered",
        "concurrent_registration_and_state_save_traffic",
        "traffic_before_and_after_daemon_commit",
        "local_cleanup_raced_with_normal_writes",
        "every_unrelated_runtime_survived_in_memory",
        "every_unrelated_runtime_survived_disk_reload",
        "every_unrelated_daemon_registration_survived",
        "every_deletion_recovery_converged",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained concurrency campaign proof {check}"
        );
    }
}

fn assert_runtime_deletion_daemon_restart_campaign(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion daemon restart evidence must exist"),
    )
    .expect("runtime deletion daemon restart evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    let iterations = evidence["iterations_per_phase"]
        .as_u64()
        .expect("daemon restart iteration count must be numeric");
    assert!(iterations >= 3);
    let scenarios = iterations * 3;
    assert_eq!(evidence["total_forced_host_terminations"], scenarios);
    assert_eq!(evidence["total_controlled_daemon_restarts"], scenarios);
    assert_eq!(evidence["observed_failed_recovery_attempts"], scenarios);
    assert_eq!(evidence["total_interference_registrations"], scenarios * 8);
    for check in [
        "real_leserpentd",
        "same_daemon_database_reopened",
        "every_durable_transition_covered",
        "every_daemon_stopped_with_sigterm",
        "every_owner_lease_released_before_restart",
        "every_offline_recovery_attempt_failed",
        "every_failed_claim_was_released_for_retry",
        "concurrent_registration_and_state_save_traffic",
        "every_unrelated_runtime_survived_disk_reload",
        "every_unrelated_daemon_registration_survived",
        "every_post_restart_recovery_converged",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained daemon restart proof {check}"
        );
    }
}

fn assert_runtime_deletion_unclean_takeover(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion unclean takeover evidence must exist"),
    )
    .expect("runtime deletion unclean takeover evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["owner_lease_duration_ms"], 30_000);
    assert_eq!(evidence["total_forced_host_terminations"], 3);
    assert_eq!(evidence["total_sigkill_daemon_terminations"], 3);
    assert_eq!(evidence["observed_owner_lease_rejections"], 3);
    assert_eq!(evidence["interference_runtimes_per_scenario"], 8);
    assert_eq!(evidence["total_interference_registrations"], 24);
    assert_eq!(
        evidence["phases"],
        serde_json::json!([
            "intent_persisted",
            "daemon_committed",
            "local_cleanup_persisted"
        ])
    );
    let latencies = evidence["takeover_latencies_ms"]
        .as_array()
        .expect("takeover latencies must be an array");
    assert_eq!(latencies.len(), 3);
    let latencies = latencies
        .iter()
        .map(|value| value.as_u64().expect("takeover latency must be numeric"))
        .collect::<Vec<_>>();
    assert!(
        latencies
            .iter()
            .all(|latency| (20_000..=45_000).contains(latency))
    );
    assert_eq!(
        evidence["min_takeover_latency_ms"],
        *latencies.iter().min().expect("latencies must not be empty")
    );
    assert_eq!(
        evidence["max_takeover_latency_ms"],
        *latencies.iter().max().expect("latencies must not be empty")
    );
    let expected_average = latencies.iter().sum::<u64>() as f64 / latencies.len() as f64;
    let retained_average = evidence["average_takeover_latency_ms"]
        .as_f64()
        .expect("average takeover latency must be numeric");
    assert!((retained_average - expected_average).abs() < 0.001);
    for check in [
        "real_leserpentd",
        "every_durable_transition_covered",
        "every_daemon_terminated_uncleanly",
        "every_pre_expiry_start_rejected",
        "every_takeover_waited_for_natural_owner_lease_expiry",
        "same_daemon_database_reopened",
        "every_failed_claim_was_released_for_retry",
        "concurrent_registration_and_state_save_traffic",
        "every_unrelated_runtime_survived_disk_reload",
        "every_unrelated_daemon_registration_survived",
        "every_post_takeover_recovery_converged",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained unclean takeover proof {check}"
        );
    }
}

fn assert_runtime_deletion_overlapping_takeover(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion overlapping takeover evidence must exist"),
    )
    .expect("runtime deletion overlapping takeover evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["owner_lease_duration_ms"], 30_000);
    assert_eq!(evidence["forced_host_terminations"], 1);
    assert_eq!(evidence["sigkill_daemon_terminations"], 1);
    assert_eq!(evidence["overlapping_intent_count"], 3);
    assert_eq!(evidence["interference_runtime_count"], 8);
    assert!(
        evidence["takeover_latency_ms"]
            .as_u64()
            .is_some_and(|latency| (20_000..=45_000).contains(&latency))
    );
    assert_eq!(
        evidence["intent_boundaries"],
        serde_json::json!([
            "intent_persisted",
            "daemon_committed",
            "local_cleanup_persisted"
        ])
    );
    for check in [
        "real_leserpentd",
        "mixed_durable_boundaries_shared_one_state",
        "all_intents_restored_independently",
        "all_initial_offline_attempts_failed",
        "all_failed_claims_released_for_retry",
        "pre_expiry_replacement_rejected",
        "takeover_waited_for_natural_owner_lease_expiry",
        "same_daemon_database_reopened",
        "all_retries_succeeded",
        "all_intents_converged",
        "concurrent_registration_and_state_save_traffic",
        "every_unrelated_runtime_survived_disk_reload",
        "every_unrelated_daemon_registration_survived",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained overlapping takeover proof {check}"
        );
    }
}

fn assert_runtime_deletion_repeated_takeover(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion repeated takeover evidence must exist"),
    )
    .expect("runtime deletion repeated takeover evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["owner_lease_duration_ms"], 30_000);
    assert_eq!(evidence["forced_host_terminations"], 1);
    assert_eq!(evidence["sigkill_daemon_terminations"], 2);
    assert_eq!(evidence["owner_lease_takeovers"], 2);
    assert_eq!(evidence["overlapping_intent_count"], 3);
    assert_eq!(evidence["partially_converged_intent_count"], 1);
    assert_eq!(evidence["pending_intents_after_second_termination"], 2);
    assert_eq!(evidence["interference_runtime_count"], 8);
    let latencies = evidence["takeover_latencies_ms"]
        .as_array()
        .expect("repeated takeover latencies must be an array");
    assert_eq!(latencies.len(), 2);
    assert!(latencies.iter().all(|latency| {
        latency
            .as_u64()
            .is_some_and(|value| (20_000..=45_000).contains(&value))
    }));
    assert_eq!(
        evidence["intent_boundaries"],
        serde_json::json!([
            "intent_persisted",
            "daemon_committed",
            "local_cleanup_persisted"
        ])
    );
    for check in [
        "real_leserpentd",
        "mixed_durable_boundaries_shared_one_state",
        "all_initial_offline_attempts_failed",
        "first_retry_committed_before_second_sigkill",
        "first_local_cleanup_completed_after_second_sigkill",
        "partial_progress_remained_durable",
        "remaining_attempts_observed_second_outage",
        "all_failed_claims_released_for_retry",
        "both_pre_expiry_replacements_rejected",
        "both_takeovers_waited_for_natural_owner_lease_expiry",
        "same_daemon_database_reopened_twice",
        "remaining_retries_succeeded_after_second_takeover",
        "all_intents_converged",
        "concurrent_registration_and_state_save_traffic",
        "every_unrelated_runtime_survived_disk_reload",
        "every_unrelated_daemon_registration_survived",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained repeated takeover proof {check}"
        );
    }
}

fn assert_runtime_deletion_poison_isolation(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion poison isolation evidence must exist"),
    )
    .expect("runtime deletion poison isolation evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["overlapping_intent_count"], 3);
    assert_eq!(evidence["poison_intent_count"], 1);
    assert_eq!(evidence["healthy_intent_count"], 2);
    assert_eq!(evidence["interference_runtime_count"], 8);
    assert!(
        evidence["poison_attempt_count"]
            .as_u64()
            .is_some_and(|attempts| attempts >= 3)
    );
    assert!(
        evidence["healthy_convergence_latency_ms"]
            .as_u64()
            .is_some_and(|latency| latency <= 5_000)
    );
    for check in [
        "real_leserpentd",
        "poison_intent_was_queue_head",
        "poison_failure_was_target_scoped",
        "healthy_intents_converged_while_poison_remained_pending",
        "poison_intent_retried_without_busy_loop",
        "poison_runtime_remained_protected",
        "poison_intent_survived_disk_reload",
        "poison_runtime_remained_protected_after_reload",
        "repaired_authority_converged_poison_intent",
        "concurrent_registration_and_state_save_traffic",
        "every_unrelated_runtime_survived_disk_reload",
        "every_unrelated_daemon_registration_survived",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained poison isolation proof {check}"
        );
    }
}

fn assert_runtime_deletion_high_cardinality(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion high-cardinality evidence must exist"),
    )
    .expect("runtime deletion high-cardinality evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["queue_intent_count"], 32);
    assert_eq!(evidence["poison_stride"], 8);
    assert_eq!(evidence["poison_intent_count"], 4);
    assert_eq!(evidence["healthy_intent_count"], 28);
    assert_eq!(evidence["first_pass_converged_intent_count"], 28);
    assert_eq!(evidence["first_pass_pending_intent_count"], 4);
    assert_eq!(evidence["recovery_passes_observed"], 3);
    assert_eq!(evidence["interference_runtime_count"], 8);
    let poison_attempts = evidence["poison_attempt_counts"]
        .as_array()
        .expect("poison attempt counts must be an array");
    assert_eq!(poison_attempts.len(), 4);
    assert!(
        poison_attempts
            .iter()
            .all(|attempts| attempts.as_u64().is_some_and(|attempts| attempts >= 3))
    );
    let first_pass_latency = evidence["first_pass_latency_ms"]
        .as_u64()
        .expect("first-pass latency must be numeric");
    let authority_phase_latency = evidence["authority_phase_latency_ms"]
        .as_u64()
        .expect("authority-phase latency must be numeric");
    let local_batch_latency = evidence["local_batch_latency_ms"]
        .as_u64()
        .expect("local-batch latency must be numeric");
    let retry_window = evidence["poison_retry_window_ms"]
        .as_u64()
        .expect("poison retry window must be numeric");
    assert_eq!(evidence["recovery_batch_size"], 32);
    assert_eq!(evidence["max_concurrent_authority_mutations"], 8);
    assert_eq!(evidence["max_ipc_connections_per_daemon_tick"], 64);
    assert_eq!(
        first_pass_latency,
        authority_phase_latency + local_batch_latency
    );
    assert!(first_pass_latency < 3_000);
    assert!(local_batch_latency <= 500);
    assert!(retry_window >= first_pass_latency + 1_800);
    assert!(retry_window <= first_pass_latency + 5_000);
    for check in [
        "real_leserpentd",
        "bounded_recovery_claim_batch",
        "bounded_concurrent_authority_mutations",
        "bounded_daemon_ipc_drain",
        "deterministic_durable_queue_order",
        "sparse_poison_failures_were_target_scoped",
        "first_pass_made_bounded_healthy_progress",
        "every_healthy_intent_converged_in_first_pass",
        "first_pass_latency_under_3000_ms",
        "successful_local_convergence_used_one_strict_batch",
        "every_poison_intent_retried_without_busy_loop",
        "poison_reservations_survived_disk_reload",
        "poison_runtimes_remained_protected_after_reload",
        "repaired_authority_converged_every_poison_intent",
        "concurrent_registration_and_state_save_traffic",
        "every_unrelated_runtime_survived_disk_reload",
        "every_unrelated_daemon_registration_survived",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained high-cardinality proof {check}"
        );
    }
}

fn assert_runtime_deletion_batch_persistence(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion batch-persistence evidence must exist"),
    )
    .expect("runtime deletion batch-persistence evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["runtime_intent_count"], 2);
    assert_eq!(
        evidence["authority_attempt_counts"],
        serde_json::json!([2, 2])
    );
    assert_eq!(evidence["orchestra_delete_batch_count"], 2);
    let first_failure_latency = evidence["first_failure_latency_ms"]
        .as_u64()
        .expect("first failure latency must be numeric");
    let retry_delay = evidence["retry_delay_ms"]
        .as_u64()
        .expect("retry delay must be numeric");
    let convergence_latency = evidence["convergence_latency_ms"]
        .as_u64()
        .expect("convergence latency must be numeric");
    assert!(first_failure_latency < 1_000);
    assert!((750..=2_000).contains(&retry_delay));
    assert!(convergence_latency >= first_failure_latency + retry_delay);
    assert!(convergence_latency < 5_000);
    for check in [
        "real_leserpentd",
        "daemon_mutations_committed_before_local_failure",
        "strict_local_batch_save_failed",
        "runtime_projection_rolled_back",
        "session_projection_rolled_back",
        "orchestra_projection_rolled_back",
        "recovery_activity_projection_rolled_back",
        "deletion_intents_rolled_back",
        "deleting_reservations_remained_protected",
        "failed_pass_state_survived_disk_reload",
        "retries_were_paced",
        "daemon_unregistration_replayed_idempotently",
        "orchestra_cleanup_replayed_idempotently",
        "next_pass_converged",
        "converged_state_survived_disk_reload",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained batch-persistence proof {check}"
        );
    }
}

fn assert_runtime_deletion_saturated_queue(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion saturated-queue evidence must exist"),
    )
    .expect("runtime deletion saturated-queue evidence must be JSON");

    assert_eq!(evidence["schema_version"], 3);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["queue_intent_count"], 128);
    assert_eq!(evidence["recovery_batch_size"], 32);
    assert_eq!(evidence["max_concurrent_authority_mutations"], 8);
    assert_eq!(evidence["poison_stride"], 16);
    assert_eq!(evidence["poison_intent_count"], 8);
    assert_eq!(evidence["slow_intent_count"], 17);
    assert_eq!(evidence["cancellation_started_call_count"], 8);
    assert_eq!(evidence["cancellation_cancelled_call_count"], 8);
    assert_eq!(evidence["observed_max_concurrency"], 8);
    assert_eq!(evidence["orchestra_delete_batch_count"], 5);
    assert_eq!(evidence["retry_backoff_seconds"], 1);
    assert!(
        evidence["cancellation_latency_ms"]
            .as_u64()
            .is_some_and(|latency| latency < 1_000)
    );
    assert_eq!(
        evidence["pending_counts_after_pass"],
        serde_json::json!([98, 68, 38, 8])
    );
    assert_eq!(
        evidence["poison_attempt_counts"],
        serde_json::json!([1, 1, 1, 1, 1, 1, 1, 1])
    );
    assert_eq!(
        evidence["persisted_attempt_counts"],
        serde_json::json!([1, 1, 1, 1, 1, 1, 1, 1])
    );
    assert_eq!(
        evidence["persisted_failure_codes"],
        serde_json::json!([
            "authority_failure",
            "authority_failure",
            "authority_failure",
            "authority_failure",
            "authority_failure",
            "authority_failure",
            "authority_failure",
            "authority_failure"
        ])
    );
    assert_eq!(
        evidence["retry_now_resulting_revisions"],
        serde_json::json!([3, 3, 3, 3, 3, 3, 3, 3])
    );
    assert_eq!(evidence["retained_retry_audit_count"], 8);
    assert_eq!(evidence["retry_now_replay_observed"], true);
    assert!(
        evidence["retry_now_repair_latency_ms"]
            .as_u64()
            .is_some_and(|latency| latency < 1_000)
    );
    let pass_latencies = evidence["pass_latencies_ms"]
        .as_array()
        .expect("pass latencies must be an array");
    assert_eq!(pass_latencies.len(), 4);
    let pass_latencies = pass_latencies
        .iter()
        .map(|latency| latency.as_u64().expect("pass latency must be numeric"))
        .collect::<Vec<_>>();
    assert!(pass_latencies.windows(2).all(|pair| pair[0] < pair[1]));
    assert!(pass_latencies[3] < 5_000);
    for check in [
        "saturated_durable_queue",
        "bounded_recovery_claim_batch",
        "bounded_authority_concurrency",
        "all_authority_slots_were_saturated",
        "cooperative_cancellation_reached_every_blocked_call",
        "shutdown_latency_under_1000_ms",
        "cancelled_pass_preserved_every_intent",
        "cancelled_pass_released_every_claim",
        "deterministic_four_pass_progress",
        "mixed_slow_and_failing_authority_operations",
        "poison_failures_were_target_scoped",
        "every_healthy_intent_converged",
        "deferred_poison_did_not_consume_ready_claim_slots",
        "retry_attempt_metadata_was_durable",
        "persisted_failure_codes_were_safe",
        "retry_deadline_blocked_premature_claim",
        "stale_retry_revision_was_rejected",
        "retry_now_revision_advanced",
        "retry_now_audit_survived_convergence",
        "retry_now_request_was_idempotent",
        "retry_now_repair_latency_under_1000_ms",
        "poison_reservations_survived_disk_reload",
        "repaired_poison_intents_converged",
        "converged_state_survived_disk_reload",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained saturated-queue proof {check}"
        );
    }
}

fn assert_runtime_deletion_retry_claim_race(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion retry/claim race evidence must exist"),
    )
    .expect("runtime deletion retry/claim race evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["total_rounds"], 48);
    assert_eq!(evidence["retry_contenders_per_round"], 8);
    assert_eq!(evidence["forced_worker_first_rounds"], 8);
    assert_eq!(evidence["forced_operator_first_rounds"], 8);
    assert_eq!(evidence["simultaneous_start_rounds"], 32);
    assert_eq!(evidence["authority_call_count"], 48);
    assert_eq!(evidence["converged_runtime_count"], 48);
    assert_eq!(evidence["unexpected_result_count"], 0);
    assert_eq!(
        evidence["retained_retry_audit_count"],
        evidence["accepted_retry_count"]
    );
    assert!(
        evidence["simultaneous_accepted_retry_count"]
            .as_u64()
            .is_some_and(|count| count <= 32)
    );
    let classified_results = evidence["accepted_retry_count"]
        .as_u64()
        .expect("accepted retry count must be numeric")
        + evidence["in_progress_conflict_count"]
            .as_u64()
            .expect("in-progress conflict count must be numeric")
        + evidence["revision_conflict_count"]
            .as_u64()
            .expect("revision conflict count must be numeric");
    assert_eq!(classified_results, 48 * 8);
    for check in [
        "both_forced_interleavings_exercised",
        "simultaneous_start_campaign_is_non_vacuous",
        "at_most_one_retry_won_each_round",
        "worker_claim_rejected_every_late_retry",
        "revision_fence_rejected_every_losing_operator",
        "accepted_retry_audit_was_durable",
        "exactly_one_authority_mutation_per_runtime",
        "every_runtime_converged_after_race",
        "conflict_results_were_deterministic",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained retry/claim race proof {check}"
        );
    }
}

fn assert_runtime_deletion_retry_crash(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion retry crash evidence must exist"),
    )
    .expect("runtime deletion retry crash evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["iterations_per_phase"], 3);
    assert_eq!(
        evidence["phases"],
        serde_json::json!(["retry_acknowledged", "retry_daemon_committed"])
    );
    assert_eq!(evidence["total_forced_terminations"], 6);
    assert_eq!(evidence["daemon_committed_before_termination_count"], 3);
    assert_eq!(evidence["recovery_authority_call_count"], 6);
    for check in [
        "real_leserpentd",
        "retry_acknowledgement_boundary_covered",
        "retry_daemon_commit_boundary_covered",
        "every_host_process_force_killed",
        "every_revision_and_audit_restored",
        "every_pending_runtime_remained_protected",
        "committed_mutation_replayed_idempotently",
        "exactly_one_recovery_authority_call_per_scenario",
        "every_retry_request_replayed_after_convergence",
        "every_daemon_and_compatibility_state_converged",
        "every_audit_survived_convergence_and_reload",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained retry crash proof {check}"
        );
    }
}

fn assert_runtime_deletion_lost_acknowledgement(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion lost-ack evidence must exist"),
    )
    .expect("runtime deletion lost-ack evidence must be JSON");

    assert_eq!(evidence["schema_version"], 2);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["iterations"], 3);
    assert_eq!(evidence["total_forced_host_terminations"], 3);
    assert_eq!(evidence["receipt_lookup_call_count"], 3);
    assert_eq!(evidence["post_restart_unregistration_mutation_count"], 0);
    assert!(
        evidence["minimum_operation_generation"]
            .as_u64()
            .is_some_and(|generation| generation > 0)
    );
    assert!(
        evidence["maximum_operation_generation"]
            .as_u64()
            .zip(evidence["minimum_operation_generation"].as_u64())
            .is_some_and(|(maximum, minimum)| maximum >= minimum)
    );
    assert_eq!(
        evidence["minimum_replay_horizon_floor"],
        evidence["minimum_operation_generation"]
    );
    assert_eq!(
        evidence["maximum_replay_horizon_floor"],
        evidence["maximum_operation_generation"]
    );
    for check in [
        "real_leserpentd",
        "schema_v5_command_identity_and_replay_floor_restored",
        "daemon_commit_preceded_host_termination",
        "acknowledgement_withheld_from_recovery_worker",
        "every_host_process_force_killed",
        "every_restart_performed_receipt_lookup",
        "zero_post_restart_unregistration_mutations",
        "receipt_generation_stable_across_recovery",
        "every_daemon_and_compatibility_state_converged",
        "every_converged_state_survived_disk_reload",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained lost-ack proof {check}"
        );
    }
}

fn assert_runtime_deletion_replay_horizon(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion replay-horizon evidence must exist"),
    )
    .expect("runtime deletion replay-horizon evidence must be JSON");

    assert_eq!(evidence["schema_version"], 2);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["forced_host_termination_count"], 1);
    assert_eq!(evidence["replay_horizon_capacity"], 256);
    assert_eq!(evidence["receipt_lookup_call_count"], 1);
    assert_eq!(evidence["post_restart_unregistration_mutation_count"], 0);
    assert!(
        evidence["reconciliation_daemon_revision"]
            .as_u64()
            .expect("reconciliation daemon revision must be retained")
            > 0
    );
    let floor = evidence["persisted_replay_horizon_floor"]
        .as_u64()
        .expect("replay floor must be retained");
    let evicted_through = evidence["evicted_through_generation"]
        .as_u64()
        .expect("evicted generation must be retained");
    assert!(floor > 0);
    assert!(evicted_through >= floor);
    for check in [
        "real_leserpentd",
        "schema_v5_replay_floor_persisted_before_mutation",
        "daemon_commit_preceded_host_termination",
        "acknowledgement_withheld_from_recovery_worker",
        "complete_replay_horizon_rollover",
        "original_receipt_was_evicted",
        "typed_miss_was_classified_ambiguous",
        "zero_post_restart_unregistration_mutations",
        "local_runtime_projection_was_preserved",
        "ambiguous_intent_survived_disk_reload",
        "reappeared_identity_blocked_reconciliation",
        "absence_snapshot_permitted_convergence",
        "atomic_local_cleanup_and_audit_survived_reload",
        "reconciliation_replayed_after_restart",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained replay-horizon proof {check}"
        );
    }
}

fn assert_runtime_deletion_reconciliation_commit(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion reconciliation commit evidence must exist"),
    )
    .expect("runtime deletion reconciliation commit evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["iterations_per_strategy"], 3);
    assert_eq!(
        evidence["strategies"],
        serde_json::json!(["BeforeWrite", "DuringTempWrite", "AfterCommit"])
    );
    assert_eq!(evidence["total_forced_terminations"], 9);
    assert!(
        evidence["daemon_revision"]
            .as_u64()
            .is_some_and(|revision| revision > 0)
    );
    assert_eq!(evidence["retry_audit_retention_limit"], 256);
    assert_eq!(evidence["temp_artifact_observed_count"], 3);
    let previous = evidence["previous_generation_count"]
        .as_u64()
        .expect("previous generation count must be numeric");
    let replacement = evidence["replacement_generation_count"]
        .as_u64()
        .expect("replacement generation count must be numeric");
    assert!(previous > 0);
    assert!(replacement > 0);
    assert_eq!(previous + replacement, 9);
    for check in [
        "real_leserpentd_snapshot_used",
        "before_write_restored_complete_previous_generation",
        "every_temp_write_was_observed",
        "after_commit_restored_complete_replacement_generation",
        "every_restart_observed_old_or_new_generation",
        "no_torn_runtime_session_intent_or_audit_generation",
        "every_previous_generation_retry_converged",
        "every_reconciliation_audit_survived_reload",
        "every_request_replayed_after_restart",
        "every_restart_preserved_retry_audit_window",
        "every_final_state_converged",
        "both_atomic_outcomes_were_exercised",
        "every_host_process_force_killed",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained reconciliation commit proof {check}"
        );
    }
}

fn assert_runtime_deletion_cross_authority(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion cross-authority evidence must exist"),
    )
    .expect("runtime deletion cross-authority evidence must be JSON");

    assert_eq!(evidence["schema_version"], 3);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["iterations_per_strategy"], 3);
    assert_eq!(
        evidence["strategies"],
        serde_json::json!([
            "AfterOrchestraCleanup",
            "DuringControlTempWrite",
            "AfterControlCommit"
        ])
    );
    assert_eq!(evidence["total_forced_terminations"], 9);
    assert!(
        evidence["daemon_revision"]
            .as_u64()
            .is_some_and(|revision| revision > 0)
    );
    assert_eq!(evidence["control_temp_artifact_observed_count"], 3);
    assert_eq!(evidence["cleanup_checkpoint_race_rounds"], 2);
    let races = evidence["cleanup_checkpoint_races"]
        .as_array()
        .expect("cleanup/checkpoint races must be an array");
    assert_eq!(races.len(), 2);
    assert_eq!(races[0]["order"], "CleanupFirst");
    assert_eq!(races[1]["order"], "CheckpointFirst");
    for race in races {
        assert_eq!(race["available_before_race"], 1);
        let checkpoint = race["checkpoint_generation"]
            .as_u64()
            .expect("checkpoint generation must be numeric");
        let cleanup = race["cleanup_generation"]
            .as_u64()
            .expect("cleanup generation must be numeric");
        assert_eq!(cleanup, checkpoint + 1);
    }
    let previous = evidence["previous_generation_count"]
        .as_u64()
        .expect("previous generation count must be numeric");
    let replacement = evidence["replacement_generation_count"]
        .as_u64()
        .expect("replacement generation count must be numeric");
    assert!(previous > 0);
    assert!(replacement > 0);
    assert_eq!(previous + replacement, 9);
    let checkpoint_restart = &evidence["audit_checkpoint_daemon_restart"];
    assert!(
        checkpoint_restart["checkpoint_lag_before_daemon_restart"]
            .as_u64()
            .is_some_and(|lag| lag > 0)
    );
    assert_eq!(checkpoint_restart["checkpoint_lag_after_daemon_restart"], 0);
    assert_eq!(
        checkpoint_restart["audit_generation"],
        checkpoint_restart["checkpointed_through_generation"]
    );
    for check in [
        "real_leserpentd_orchestra_authority_used",
        "orchestra_cleanup_committed_before_every_termination",
        "target_history_absent_before_every_termination",
        "unrelated_run_and_event_preserved",
        "after_orchestra_cleanup_restored_previous_control_generation",
        "every_control_temp_write_was_observed",
        "after_control_commit_restored_replacement_generation",
        "every_restart_observed_old_or_new_control_generation",
        "no_torn_control_generation",
        "every_previous_generation_retried_absent_target_cleanup",
        "every_final_state_converged",
        "every_final_state_retained_one_reconciliation_audit",
        "every_request_replayed_after_restart",
        "every_cleanup_receipt_replayed_same_generation",
        "every_audit_checkpoint_protected_cleanup_replay_horizon",
        "every_pre_saturation_critical_warning_visible",
        "cleanup_first_race_exercised",
        "checkpoint_first_race_exercised",
        "every_raced_cleanup_committed",
        "every_raced_checkpoint_committed",
        "every_race_observed_expected_completion_order",
        "every_cleanup_checkpoint_race_admission_safe",
        "audit_driven_checkpoint_advanced_after_daemon_restart",
        "checkpoint_lag_was_visible_before_daemon_restart",
        "checkpoint_lag_converged_to_zero_after_daemon_restart",
        "automatic_checkpoint_status_reported",
        "both_control_generation_outcomes_were_exercised",
        "every_host_process_force_killed",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained cross-authority proof {check}"
        );
    }
}

fn assert_runtime_deletion_retry_rollover(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion retry rollover evidence must exist"),
    )
    .expect("runtime deletion retry rollover evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["audit_entry_count"], 272);
    assert_eq!(evidence["audit_retention_limit"], 256);
    assert_eq!(evidence["initial_evicted_entry_count"], 16);
    assert_eq!(evidence["wave_sizes"], serde_json::json!([128, 128, 16]));
    assert_eq!(evidence["recovery_batch_size"], 32);
    assert_eq!(evidence["max_concurrent_authority_mutations"], 8);
    assert_eq!(evidence["observed_max_concurrency"], 8);
    assert_eq!(evidence["authority_call_count"], 273);
    assert!(
        evidence["elapsed_ms"]
            .as_u64()
            .is_some_and(|elapsed| elapsed < 30_000)
    );
    for check in [
        "concurrent_operator_worker_campaign",
        "full_pending_waves_converged",
        "audit_timestamps_followed_linearization_order",
        "retention_bound_was_exact",
        "oldest_entries_were_evicted_first",
        "retained_request_replayed_after_convergence",
        "evicted_request_was_outside_replay_horizon",
        "evicted_request_id_was_reusable",
        "reuse_evicted_the_next_oldest_record",
        "every_runtime_received_one_authority_mutation",
        "no_pending_intent_starved_or_was_lost",
        "rollover_state_survived_disk_reload",
        "bounded_authority_concurrency",
        "campaign_completed_under_30000_ms",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained retry rollover proof {check}"
        );
    }
}

fn assert_runtime_deletion_retry_atomic_rollover(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion retry atomic-rollover evidence must exist"),
    )
    .expect("runtime deletion retry atomic-rollover evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["iterations_per_strategy"], 3);
    assert_eq!(
        evidence["strategies"],
        serde_json::json!(["BeforeWrite", "DuringTempWrite", "AfterCommit"])
    );
    assert_eq!(evidence["total_forced_terminations"], 9);
    assert_eq!(evidence["audit_retention_limit"], 256);
    assert_eq!(evidence["runtime_ids_per_audit_record"], 128);
    assert_eq!(evidence["temp_artifact_observed_count"], 3);
    let previous = evidence["previous_window_count"]
        .as_u64()
        .expect("previous window count must be numeric");
    let replacement = evidence["replacement_window_count"]
        .as_u64()
        .expect("replacement window count must be numeric");
    assert_eq!(previous + replacement, 9);
    assert!(previous > 0);
    assert!(replacement > 0);
    for check in [
        "before_write_restored_complete_previous_window",
        "every_temp_write_was_observed",
        "after_commit_restored_complete_replacement_window",
        "every_restart_loaded_exactly_256_records",
        "every_restart_observed_old_or_new_window",
        "no_torn_or_reordered_window",
        "both_atomic_outcomes_were_exercised",
        "every_host_process_force_killed",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained atomic-rollover proof {check}"
        );
    }
}

fn assert_runtime_deletion_retry_atomic_backup(path: &str, expected_architecture: &str) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion retry atomic-backup evidence must exist"),
    )
    .expect("runtime deletion retry atomic-backup evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["iterations_per_strategy"], 3);
    assert_eq!(
        evidence["strategies"],
        serde_json::json!(["BeforeWrite", "DuringBackupTempWrite", "AfterCommit"])
    );
    assert_eq!(evidence["total_forced_terminations"], 9);
    assert_eq!(evidence["audit_retention_limit"], 256);
    assert_eq!(evidence["runtime_ids_per_audit_record"], 128);
    assert_eq!(evidence["deliberately_corrupted_primary_count"], 9);
    assert_eq!(evidence["complete_previous_window_recovery_count"], 9);
    assert_eq!(evidence["typed_backup_recovery_provenance_count"], 9);
    assert_eq!(evidence["backup_temp_artifact_observed_count"], 3);
    for check in [
        "backup_refresh_used_unique_temp_file",
        "every_backup_temp_write_was_observed",
        "every_primary_was_deliberately_corrupted",
        "every_fallback_loaded_exactly_256_records",
        "every_fallback_restored_complete_previous_window",
        "every_fallback_reported_backup_source",
        "every_fallback_reported_recovered_outcome",
        "every_primary_failure_reported_invalid_json",
        "no_backup_failure_was_reported",
        "recovery_provenance_was_secret_free",
        "no_truncated_or_mixed_backup_window",
        "every_host_process_force_killed",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained atomic-backup proof {check}"
        );
    }
}

fn assert_runtime_deletion_retry_post_recovery_write(
    path: &str,
    expected_architecture: &str,
    expected_failure_code: &str,
) {
    let evidence: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(repository_root().join(path))
            .expect("runtime deletion retry post-recovery-write evidence must exist"),
    )
    .expect("runtime deletion retry post-recovery-write evidence must be JSON");

    assert_eq!(evidence["schema_version"], 1);
    assert_eq!(evidence["architecture"], expected_architecture);
    assert_eq!(evidence["iterations_per_strategy"], 3);
    assert_eq!(
        evidence["strategies"],
        serde_json::json!(["BeforeWrite", "DuringPrimaryTempWrite", "AfterCommit"])
    );
    assert_eq!(evidence["total_forced_terminations"], 9);
    assert_eq!(evidence["audit_retention_limit"], 256);
    assert_eq!(evidence["runtime_ids_per_audit_record"], 128);
    assert_eq!(evidence["primary_failure_code"], expected_failure_code);
    assert_eq!(
        evidence["active_previous_window_count"]
            .as_u64()
            .expect("previous window count must be numeric")
            + evidence["active_replacement_window_count"]
                .as_u64()
                .expect("replacement window count must be numeric"),
        9
    );
    assert_eq!(evidence["known_good_backup_preserved_count"], 9);
    assert_eq!(evidence["backup_temp_artifact_absent_count"], 9);
    assert_eq!(evidence["primary_temp_artifact_observed_count"], 3);
    for check in [
        "recovery_started_from_corrupted_primary",
        "first_post_recovery_write_skipped_backup_refresh",
        "every_primary_temp_write_was_observed",
        "every_restart_loaded_exactly_256_records",
        "every_backup_retained_exactly_256_records",
        "every_backup_preserved_complete_previous_window",
        "precommit_restart_reported_backup_recovery",
        "postcommit_restart_reported_clean_primary",
        "every_precommit_failure_reported_expected_code",
        "every_restart_observed_complete_old_or_new_window",
        "no_corrupted_primary_was_copied_into_backup",
        "every_host_process_force_killed",
    ] {
        assert_eq!(
            evidence["checks"][check], true,
            "missing retained post-recovery-write proof {check}"
        );
    }
}

#[test]
fn etragon_stays_downweighted_until_the_deep_learning_stack_is_proven() {
    let catalog = StatusCatalog::load(default_catalog_path()).expect("catalog must decode");
    let etragon = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "etragon/learning-sidecar/advisory-learning")
        .expect("Etragon advisory-learning cell must exist");

    assert_eq!(etragon.maturity, Maturity::Incubating);
    assert!(etragon.completion <= 45);
    assert!(etragon.blockers.iter().any(|blocker| {
        blocker.id == "deep-learning-stack-not-integrated"
            && blocker.summary.contains("inference evidence")
    }));
}

#[test]
fn tensor_tracks_reuse_development_and_leserpent_two_gates() {
    let catalog = StatusCatalog::load(default_catalog_path()).expect("catalog must decode");
    let summary = catalog.summary(20);

    assert!(
        summary
            .independently_usable
            .iter()
            .any(|cell| cell.independence == Independence::StandaloneTool)
    );
    assert!(
        summary
            .in_development
            .iter()
            .any(|cell| cell.architecture == "leserpent-2")
    );

    let domain = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/domain-model/command-query-kernel")
        .expect("Leserpent Gate 1 cell must exist");
    assert_eq!(domain.maturity, Maturity::Mature);
    assert_eq!(domain.completion, 100);
    assert_eq!(domain.independence, Independence::ReusableLibrary);
    assert_eq!(domain.contract.stability, ContractStability::Stable);
    assert!(
        domain
            .evidence
            .iter()
            .any(|item| item.kind == EvidenceKind::Source && item.state == EvidenceState::Present)
    );

    let language = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/language-vm/effect-reentry")
        .expect("Leserpent Gate 2 cell must exist");
    assert_eq!(language.maturity, Maturity::Mature);
    assert_eq!(language.completion, 100);
    assert_eq!(language.contract.stability, ContractStability::Stable);
    assert!(language.blockers.is_empty());

    let linux_attach = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "gewyvern-core/linux-ebpf/linux-attach")
        .expect("Gewyvern Linux attach cell must exist");
    assert_eq!(linux_attach.completion, 98);
    assert_eq!(linux_attach.contract.version, "1.5.1");
    assert_eq!(linux_attach.blockers.len(), 1);
    for surface in [
        "dedicated-project-ssh-alias",
        "project-owned-remote-workspace-root",
        "generated-output-sync-exclusion",
        "stale-excluded-output-deletion",
        "bounded-nonce-ssh-control-socket",
        "shell-quoted-control-path-override",
        "dotnet10-rid-neutral-locked-aot-restore",
        "dynamic-writer-fence-sqlite-activation",
        "physical-linux-helper-version-upgrade-proof",
        "shared-native-bounded-process-guard",
        "bounded-local-smoke-subprocesses",
        "bounded-smoke-command-output-capture",
    ] {
        assert!(
            linux_attach
                .contract
                .surfaces
                .iter()
                .any(|item| item == surface)
        );
    }
    assert!(linux_attach.evidence.iter().any(|item| {
        item.path == "docs/fixtures/linux_attach_pinned_source_root.json"
            && item.state == EvidenceState::Present
    }));
    assert!(linux_attach.evidence.iter().any(|item| {
        item.path == "src/bounded_process.rs" && item.state == EvidenceState::Present
    }));

    let gewylang = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "gewylang/compiler/parser-lowering")
        .expect("GewyLang compiler cell must exist");
    assert_eq!(gewylang.maturity, Maturity::Mature);
    assert_eq!(gewylang.completion, 100);
    assert_eq!(gewylang.contract.stability, ContractStability::Stable);
    assert!(gewylang.blockers.is_empty());
    assert!(
        language
            .evidence
            .iter()
            .filter(|item| item.kind == EvidenceKind::Source)
            .all(|item| item.state == EvidenceState::Present)
    );

    let avalonia = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/ui-renderers/avalonia-renderer")
        .expect("Leserpent Gate 4 renderer cell must exist");
    assert_eq!(avalonia.maturity, Maturity::Mature);
    assert_eq!(avalonia.completion, 100);
    assert_eq!(avalonia.contract.stability, ContractStability::Stable);
    assert_eq!(avalonia.contract.version, "1.104.0");
    assert!(
        avalonia
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "avalonia-generation-horizon-classification")
    );
    for surface in [
        "native-hub-topology-filter",
        "hub-filter-keyboard-navigation",
        "hub-filter-focus-recovery",
        "native-hub-refresh-all-control",
        "global-topology-refresh-single-flight",
        "per-authority-refresh-join",
        "operator-refresh-live-status",
        "shared-topology-refresh-policy-consumer",
        "zero-avalonia-refresh-concurrency-ownership",
        "shared-workspace-launch-coordinator-consumer",
        "zero-avalonia-workspace-launch-policy-ownership",
        "shared-mutation-lifecycle-coordinator-consumer",
        "zero-avalonia-mutation-state-ownership",
        "authoritative-snapshot-action-gate",
        "malformed-mutation-response-observation-fence",
        "shared-mutation-failure-classification",
        "zero-avalonia-mutation-failure-branching",
        "stale-mutation-completion-suppression",
        "bounded-mutation-failure-diagnostics",
        "shared-authority-health-lifecycle-coordinator-consumer",
        "zero-avalonia-health-state-ownership",
        "authority-health-stop-generation-fence",
        "shared-health-failure-classification",
        "shared-event-lifecycle-consumer",
        "event-run-generation-handle",
        "event-disposal-single-flight",
        "subscriber-failure-isolation",
        "bounded-subscriber-failure-telemetry",
        "zero-avalonia-event-lifecycle-ownership",
        "shared-typed-ui-action-router-consumer",
        "source-bound-rendered-action-invocation",
        "multi-workspace-action-source-identity",
        "retired-workspace-action-source-fence",
        "zero-node-id-business-routing",
        "source-renderer-form-registration",
        "offline-native-learning-center",
        "hub-visible-tutorial-entry",
        "macos-learning-center-menu",
        "singleton-tutorial-window",
        "tutorial-auxiliary-window-classification",
        "tutorial-keyboard-navigation",
        "tutorial-automation-contract",
    ] {
        assert!(
            avalonia
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing Avalonia renderer surface {surface}"
        );
    }
    for surface in [
        "native-selection-state-mutation",
        "idempotent-selection-mutation",
        "reversible-selection-mutation",
        "selection-mutation-no-action-activation",
        "selection-mutation-focus-preservation",
        "production-parameter-form-scope-binding",
        "scoped-native-form-window-registration",
        "native-form-submit-mutation",
        "native-form-cancel-mutation",
        "disabled-form-submit-rejection",
        "form-lifecycle-single-click",
        "closed-form-replay-rejection",
        "form-lifecycle-no-semantic-action-activation",
        "native-form-value-mutation",
        "scoped-native-form-field-registration",
        "duplicate-form-field-registration-rejection",
        "form-value-mutation-idempotency",
        "form-value-mutation-no-action-activation",
        "form-value-mutation-focus-preservation",
        "native-form-value-assertion",
        "form-value-mismatch-presentation-rejection",
        "unregistered-form-field-presentation-rejection",
        "form-field-scope-disposal",
        "dispatcher-yielding-form-value-wait",
        "external-form-value-transition-wait",
        "persistent-form-value-mismatch-timeout",
        "optional-team-silvortex-account",
        "oidc-native-code-pkce",
        "system-browser-loopback-callback",
        "strict-oidc-discovery-origin",
        "rs256-jwks-id-token-verification",
        "mfa-assurance-required",
        "userinfo-subject-binding",
        "rotating-refresh-token-vault",
        "offline-daemon-independence",
        "account-control-automation",
        "duplicate-callback-parameter-rejection",
        "public-client-no-secret",
        "reviewed-silvortex-application-profile",
        "statically-registered-native-oidc-client",
        "reviewed-native-oidc-client-default",
        "physical-linux-oidc-provider-shadow-proof",
        "packaged-native-account-proof-runner",
        "reviewed-account-proof-configuration-fence",
        "preexisting-account-credential-refusal",
        "fresh-session-vault-refresh-proof",
        "refresh-credential-rotation-proof",
        "revocation-attempt-observation",
        "private-atomic-account-proof-evidence",
        "failed-proof-local-credential-cleanup",
        "identity-free-account-proof",
        "native-aot-account-proof-fence",
        "bundle-owned-silvortex-issuer",
        "canonical-public-issuer-origin",
        "finder-account-configuration",
        "bounded-info-plist-account-reader",
        "plist-account-entity-rejection",
        "packaged-account-environment-override-rejection",
        "release-preflight-account-config-validation",
        "macos-proof-packaged-config-fence",
        "strict-ui-adapter-manifest-codec",
        "developer-owned-adapter-manifest-validation",
        "generated-binding-manifest-validation",
        "complete-presentation-atom-manifest-validation",
        "adapter-manifest-unknown-field-rejection",
        "adapter-manifest-numeric-enum-rejection",
        "invalid-adapter-binding-kind-rejection",
        "invalid-presentation-atom-rejection",
        "adapter-manifest-profile-validation",
        "adapter-manifest-profile-enum-codec",
        "canonical-presentation-atom-profile-validation",
        "strict-unregistration-receipt-client",
        "retained-receipt-horizon-binding",
        "native-action-activation",
        "native-click-event-route",
        "unavailable-action-activation-rejection",
        "hidden-action-activation-rejection",
        "single-invocation-activation",
        "mode-scoped-presentation-probes",
        "native-aot-presentation-probe",
        "physical-linux-action-activation-evidence",
        "native-enabled-state-assertion",
        "disabled-target-presentation-rejection",
        "native-disabled-state-assertion",
        "enabled-target-disabled-assertion-rejection",
        "native-hidden-state-assertion",
        "visible-target-hidden-assertion-rejection",
        "dispatcher-yielding-hidden-wait",
        "external-hidden-transition-wait",
        "persistent-visible-hidden-wait-timeout",
        "native-text-assertion",
        "text-mismatch-presentation-rejection",
        "dispatcher-yielding-text-wait",
        "external-text-transition-wait",
        "persistent-text-mismatch-timeout",
        "dispatcher-yielding-accessible-name-wait",
        "external-accessible-name-transition-wait",
        "persistent-accessible-name-mismatch-timeout",
        "dispatcher-yielding-accessible-description-wait",
        "external-accessible-description-transition-wait",
        "persistent-accessible-description-mismatch-timeout",
        "native-automation-id-assertion",
        "automation-id-mismatch-presentation-rejection",
        "dispatcher-yielding-automation-id-wait",
        "external-automation-id-transition-wait",
        "persistent-automation-id-mismatch-timeout",
        "automation-id-wait-no-focus-or-action-mutation",
        "semantic-node-kind-assertion",
        "node-kind-mismatch-presentation-rejection",
        "dispatcher-yielding-node-kind-wait",
        "persistent-node-kind-mismatch-timeout",
        "semantic-action-kind-assertion",
        "action-kind-mismatch-presentation-rejection",
        "dispatcher-yielding-action-kind-wait",
        "persistent-action-kind-mismatch-timeout",
        "semantic-action-label-assertion",
        "dispatcher-yielding-action-label-wait",
        "persistent-action-label-mismatch-timeout",
        "action-label-mismatch-presentation-rejection",
        "action-label-wait-no-action-activation",
        "semantic-action-available-assertion",
        "dispatcher-yielding-action-available-wait",
        "external-action-available-transition-wait",
        "persistent-action-unavailable-timeout",
        "action-available-wait-no-action-activation",
        "semantic-action-unavailable-reason-assertion",
        "action-unavailable-reason-mismatch-presentation-rejection",
        "dispatcher-yielding-action-unavailable-reason-wait",
        "external-action-unavailable-reason-transition-wait",
        "persistent-action-unavailable-reason-timeout",
        "semantic-form-field-assertion",
        "form-field-mismatch-presentation-rejection",
        "semantic-form-field-input-kind-assertion",
        "form-field-input-kind-mismatch-presentation-rejection",
        "semantic-form-field-required-assertion",
        "form-field-required-mismatch-presentation-rejection",
        "semantic-form-field-max-length-assertion",
        "form-field-max-length-mismatch-presentation-rejection",
        "semantic-form-field-placeholder-assertion",
        "form-field-placeholder-mismatch-presentation-rejection",
        "dispatcher-yielding-form-field-wait",
        "persistent-form-field-mismatch-timeout",
        "dispatcher-yielding-form-field-input-kind-wait",
        "persistent-form-field-input-kind-mismatch-timeout",
        "dispatcher-yielding-form-field-required-wait",
        "persistent-form-field-required-mismatch-timeout",
        "dispatcher-yielding-form-field-max-length-wait",
        "persistent-form-field-max-length-mismatch-timeout",
        "dispatcher-yielding-form-field-placeholder-wait",
        "external-form-field-placeholder-transition-wait",
        "persistent-form-field-placeholder-mismatch-timeout",
        "native-accessible-name-assertion",
        "accessible-name-mismatch-presentation-rejection",
        "native-accessible-description-assertion",
        "accessible-description-mismatch-presentation-rejection",
        "native-realized-state-assertion",
        "unrealized-target-presentation-rejection",
        "dispatcher-yielding-realization-wait",
        "natural-layout-realization-wait",
        "persistent-unrealized-timeout",
        "dispatcher-yielding-visibility-wait",
        "natural-layout-visibility-wait",
        "persistent-invisible-timeout",
        "dispatcher-yielding-enabled-wait",
        "external-enabled-transition-wait",
        "persistent-disabled-timeout",
        "dispatcher-yielding-disabled-wait",
        "external-disabled-transition-wait",
        "persistent-enabled-disabled-wait-timeout",
        "native-window-open-mutation",
        "native-window-close-mutation",
        "native-window-reopen-mutation",
        "native-window-reclose-mutation",
        "idempotent-window-lifecycle",
        "duplicate-window-open-close-idempotency",
        "fresh-native-window-reopen",
        "semantic-tree-rematerialization-after-window-close",
        "visible-window-state-fence",
        "non-activating-window-open",
        "window-lifecycle-state-observation",
        "desktop-focus-deactivation-aware-verification",
        "native-window-open-assertion",
        "dispatcher-yielding-window-open-wait",
        "window-open-wait-timeout-validation",
        "native-window-closed-state-assertion",
        "dispatcher-yielding-window-closed-wait",
        "detached-surface-window-closed-wait",
        "persistent-open-window-closed-wait-timeout",
        "window-closed-wait-no-window-mutation",
        "dispatcher-yielding-focused-wait",
        "external-focused-transition-wait",
        "persistent-realized-unfocused-timeout",
        "focused-wait-no-focus-mutation",
        "native-unfocused-state-assertion",
        "dispatcher-yielding-unfocused-wait",
        "external-unfocused-transition-wait",
        "persistent-focused-unfocused-wait-timeout",
        "unfocused-wait-no-focus-mutation",
        "native-focus-navigation",
        "stable-focus-navigation-destination",
        "stable-index-focus-boundary-navigation",
        "focus-first-last-stable-destination",
        "focus-navigation-failure-atomicity",
        "focus-navigation-no-action-activation",
        "native-selection-state-assertion",
        "selection-mismatch-presentation-rejection",
        "selectionless-target-presentation-rejection",
        "dispatcher-yielding-selection-wait",
        "native-selection-wait",
        "persistent-selection-mismatch-timeout",
        "selection-probe-focus-preservation",
        "semantic-child-count-assertion",
        "dispatcher-yielding-child-count-wait",
        "external-child-count-patch-transition",
        "persistent-child-count-mismatch-timeout",
        "virtualization-preserving-child-count-observation",
    ] {
        assert!(
            avalonia
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface)
        );
    }
    assert!(avalonia.evidence.iter().any(|item| {
        item.kind == EvidenceKind::Test
            && item.path
                == "docs/fixtures/leserpent_avalonia_presentation_native_aot_linux_x86_64_20260809.json"
            && item.state == EvidenceState::Present
    }));
    assert!(avalonia.evidence.iter().any(|item| {
        item.kind == EvidenceKind::Test
            && item.path
                == "docs/fixtures/leserpent_silvortex_oidc_provider_shadow_linux_x86_64_20260810.json"
            && item.state == EvidenceState::Present
    }));
    assert!(avalonia.evidence.iter().any(|item| {
        item.kind == EvidenceKind::Source
            && item.path
                == "apps/leserpent-avalonia/src/Leserpent.Avalonia/SilvortexAccountProof.cs"
            && item.state == EvidenceState::Present
    }));
    assert!(avalonia.evidence.iter().any(|item| {
        item.kind == EvidenceKind::Source
            && item.path
                == "apps/leserpent-avalonia/src/Leserpent.Avalonia/SilvortexAccountConfiguration.cs"
            && item.state == EvidenceState::Present
    }));
    assert!(avalonia.evidence.iter().any(|item| {
        item.kind == EvidenceKind::Source
            && item.path == "src/leserpent_account_config.rs"
            && item.state == EvidenceState::Present
    }));
    assert!(avalonia.next_gate.contains("--silvortex-issuer"));
    assert!(avalonia.next_gate.contains("--prove-silvortex-account"));
    assert!(avalonia.next_gate.contains("macOS NativeAOT application"));
    assert!(avalonia.next_gate.contains("system-browser"));
    assert!(avalonia.next_gate.contains("credential-vault"));
    assert!(avalonia.next_gate.contains("local logout"));
    assert!(avalonia.next_gate.contains("Linux Secret Service"));
    assert!(avalonia.blockers.is_empty());

    let transport = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/transport-protocol/wire-compatibility")
        .expect("Leserpent Gate 6 transport cell must exist");
    assert_eq!(transport.maturity, Maturity::Mature);
    assert_eq!(transport.completion, 100);
    assert_eq!(transport.contract.stability, ContractStability::Stable);
    assert_eq!(transport.contract.version, "1.15.0");
    for surface in [
        "optional-unregistration-replay-horizon-health",
        "legacy-horizon-free-health-decode",
        "strict-avalonia-horizon-health-decode",
        "optional-runtime-unregistration-operation-generation",
        "legacy-generation-free-receipt-decode",
        "typed-runtime-unregistration-receipt-lookup",
        "atomic-receipt-horizon-response",
        "typed-null-receipt-miss",
        "canonical-authority-writer-http-headers",
        "paired-unique-writer-header-validation",
        "post-bearer-writer-ticket-validation",
        "authenticated-https-generation-fence",
        "exhaustive-request-fence-classification",
        "runtime-refresh-generation-fence",
        "bootstrap-session-bind-generation-fence",
    ] {
        assert!(
            transport
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing replay horizon transport surface {surface}"
        );
    }
    assert!(transport.blockers.is_empty());

    let cli = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/native-cli/cli-parity")
        .expect("Leserpent native CLI cell must exist");
    assert_eq!(cli.maturity, Maturity::Mature);
    assert_eq!(cli.completion, 100);
    assert_eq!(cli.contract.stability, ContractStability::Stable);
    assert_eq!(cli.contract.version, "1.9.0");
    assert!(
        cli.contract
            .surfaces
            .iter()
            .any(|surface| surface == "native-cli-generation-bound-unregistration-receipt")
    );
    assert!(
        cli.contract
            .surfaces
            .iter()
            .any(|surface| surface == "runtime-unregister-receipt-command")
    );
    for surface in [
        "runtime-provision-command",
        "runtime-retire-command",
        "authenticated-retirement-ipc-https",
        "stable-retirement-identity-replay",
        "credential-free-retirement-progress",
        "retirement-terminal-exit-code",
        "native-cli-confirmed-daemon-retirement",
        "authenticated-cli-daemon-retirement-ipc-https",
        "authority-omitting-cli-daemon-retirement-request",
        "bounded-cli-daemon-retirement-progress",
        "daemon-retirement-terminal-exit-codes",
        "credential-free-daemon-retirement-output",
        "explicit-cli-daemon-retirement-confirmation",
        "authenticated-https-writer-ticket-headers",
        "remote-writer-ticket-forwarding",
        "transport-neutral-writer-ticket-environment",
    ] {
        assert!(
            cli.contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing CLI retirement surface {surface}"
        );
    }
    assert!(cli.blockers.is_empty());

    let runtime = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/control-runtime/durable-authority")
        .expect("Leserpent Gate 5 runtime cell must exist");
    assert_eq!(runtime.maturity, Maturity::Mature);
    assert_eq!(runtime.completion, 100);
    assert_eq!(runtime.contract.stability, ContractStability::Stable);
    assert_eq!(runtime.contract.version, "1.15.0");
    for surface in [
        "durable-sidecar-endpoint",
        "atomic-sidecar-registration-update",
        "secret-free-sidecar-projection",
        "journal-authority-timestamps",
        "snapshot-persisted-authority-timestamps",
        "replay-stable-authority-timestamps",
        "idempotent-timestamp-preservation",
        "legacy-optional-authority-timestamps",
        "timestamp-free-command-outcomes",
        "typed-sidecar-status-observation",
        "sanitized-sidecar-failure-posture",
        "durable-sidecar-status",
        "sidecar-status-replay",
        "validated-runtime-status-posture",
        "sanitized-runtime-status-failure",
        "shared-status-observation-validation",
        "sqlite-v14-runtime-unregistration",
        "sqlite-v15-unregistration-generation",
        "schema-owned-unregistration-replay-horizon",
        "queryable-unregistration-horizon-health",
        "durable-unregistration-generation-receipt",
        "first-replay-generation-identity",
        "validated-unregistration-receipt-lookup",
        "atomic-receipt-horizon-snapshot",
        "projection-tombstone-lookup-fence",
        "atomic-unregistration-orchestra-cleanup",
        "durable-unregistration-idempotency",
        "restart-safe-unregistration-replay",
        "atomic-authority-writer-claim-transaction",
        "deterministic-claim-precommit-crash-rollback",
        "postcommit-claim-durability",
        "natural-owner-lease-recovery-after-sigkill",
        "physical-linux-claim-crash-proof",
        "lost-claim-response-idempotency",
        "concurrent-writer-claim-linearization",
        "unique-generation-writer-order",
        "final-ticket-authority-proof",
        "physical-linux-lost-response-race-proof",
        "durable-lost-claim-response-replay",
        "queued-claim-order-linearization",
        "fresh-process-writer-generation-continuity",
        "final-ticket-second-restart-replay",
        "physical-linux-cold-response-replay-proof",
        "combined-lost-response-sigkill-recovery",
        "pre-expiry-owner-and-socket-fence",
        "natural-lease-generation-replay",
        "same-path-authority-rebind",
        "physical-linux-unclean-response-recovery-proof",
        "repeated-unclean-response-recovery",
        "two-natural-owner-lease-expiry-cycles",
        "contiguous-authority-generations-1-through-5",
        "repeated-same-path-authority-rebind",
        "physical-linux-repeated-unclean-recovery-proof",
        "post-recovery-64-claim-transaction-contention",
        "bounded-authority-claim-completion",
        "contiguous-authority-generations-3-through-66",
        "no-false-replay-under-contention",
        "physical-linux-post-recovery-contention-proof",
        "post-recovery-duplicate-claim-linearization",
        "read-half-abandoned-response-commit",
        "contiguous-authority-generations-3-through-18",
        "48-stable-idempotent-replays",
        "physical-linux-saturated-duplicate-retry-proof",
        "parallel-ipc-frame-intake",
        "accept-order-serial-authority-dispatch",
        "invalid-peers-zero-generation-allocation",
        "bounded-valid-claim-progress-under-slowloris",
        "physical-linux-hostile-peer-progress-proof",
        "repeated-hostile-batch-lifecycle-proof",
        "owner-heartbeat-continuity-under-hostile-load",
        "signal-cancellable-ipc-frame-read",
        "hard-total-frame-read-deadline",
        "bounded-sigterm-under-slowloris",
        "graceful-owner-and-socket-release",
        "immediate-same-path-restart",
        "physical-linux-repeated-hostile-lifecycle-proof",
        "three-cycle-hostile-resource-retention",
        "stable-post-batch-fd-task-baseline",
        "scoped-reader-thread-join",
        "saturated-plus-64-fd-task-proof",
        "per-cycle-proc-owner-socket-release",
        "physical-linux-hostile-resource-retention-proof",
        "accept-order-ready-prefix-dispatch",
        "later-reader-nonblocking-response",
        "three-saturated-reconnect-fairness-waves",
        "60-slow-4-valid-reconnect-wave",
        "twelve-bounded-reconnect-replays",
        "owner-heartbeat-under-reconnect-waves",
        "physical-linux-hostile-reconnect-fairness-proof",
        "maintenance-first-daemon-turn",
        "alternating-unix-ipc-https-priority",
        "three-saturated-cross-transport-waves",
        "bounded-authenticated-https-under-ipc-saturation",
        "owner-heartbeat-under-cross-transport-load",
        "stable-writer-generation-under-cross-transport-load",
        "physical-linux-cross-transport-fairness-proof",
        "authenticated-slow-https-body-timeout",
        "three-slow-https-cross-transport-waves",
        "four-concurrent-ipc-queries-per-wave",
        "twelve-bounded-ipc-queries-under-https-pressure",
        "owner-heartbeat-under-slow-https-pressure",
        "stable-writer-generation-under-slow-https-pressure",
        "physical-linux-symmetric-cross-transport-fairness-proof",
        "hard-total-tls-http-read-deadline",
        "signal-cancellable-remote-request-read",
        "100ms-remote-read-stop-poll",
        "pre-authority-cancellation-fence",
        "cancelled-response-suppression",
        "bounded-sigterm-under-authenticated-slow-https",
        "remote-owner-socket-release",
        "immediate-post-https-cancellation-restart",
        "physical-linux-slow-https-sigterm-proof",
        "repeated-remote-read-phase-shutdown",
        "incomplete-tls-handshake-cancellation",
        "incomplete-http-header-cancellation",
        "authenticated-body-cancellation-cycle",
        "stable-remote-idle-fd-task-baseline",
        "single-remote-connection-fd-accounting",
        "zero-remote-task-amplification",
        "per-phase-proc-owner-socket-release",
        "immediate-cross-phase-restart",
        "physical-linux-remote-read-phase-resource-proof",
        "nonblocking-tcp-rustls-underlay",
        "wouldblock-absorbed-before-rustls",
        "nonretryable-connection-aborted-cancellation",
        "sqlite-journal-aware-idle-baseline",
        "three-run-physical-phase-stability",
        "post-read-deadline-immediate-http-error",
        "collision-free-parallel-lifecycle-fixtures",
    ] {
        assert!(
            runtime
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing durable sidecar authority surface {surface}"
        );
    }
    assert!(runtime.blockers.is_empty());
    assert!(runtime.evidence.iter().any(|item| {
        item.path == "crates/leserpentd/tests/authority_writer_takeover_vertical.rs"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_claim_crash_linux_x86_64_20260801.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_lost_response_race_linux_x86_64_20260801.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_lost_response_cold_replay_linux_x86_64_20260801.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_lost_response_sigkill_recovery_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_repeated_unclean_recovery_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_post_recovery_contention_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_post_recovery_duplicate_retry_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_post_recovery_mixed_peer_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_repeated_hostile_lifecycle_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_hostile_resource_cycles_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_hostile_reconnect_fairness_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path == "crates/leserpentd/tests/cross_transport_fairness_vertical.rs"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_cross_transport_fairness_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_slow_https_cross_transport_fairness_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_slow_https_sigterm_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_remote_read_phase_shutdown_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_remote_backlog_shutdown_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_max_event_session_shutdown_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(runtime.next_gate.contains("physical Linux x86_64 evidence"));

    let daemon_lifecycle = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/daemon-host/daemon-lifecycle")
        .expect("Leserpent daemon lifecycle cell must exist");
    assert_eq!(daemon_lifecycle.contract.version, "1.21.0");
    assert_eq!(
        daemon_lifecycle.contract.stability,
        ContractStability::Stable
    );
    for surface in [
        "same-owner-stale-unix-socket-reclamation",
        "live-unix-listener-replacement-rejection",
        "nonsocket-and-symlink-preservation",
        "insecure-socket-replacement-rejection",
        "device-inode-revalidation-before-unlink",
        "unclean-same-path-restart",
        "physical-linux-unclean-restart-proof",
        "repeated-unclean-same-path-restart",
        "two-cycle-socket-reclamation",
        "physical-linux-repeated-unclean-restart-proof",
        "post-recovery-production-ipc-batch-saturation",
        "64-connection-claim-admission",
        "bounded-contention-completion",
        "physical-linux-post-recovery-contention-proof",
        "post-recovery-mixed-response-saturation",
        "16-abandoned-response-claims",
        "48-readable-retry-replays",
        "isolated-peer-response-failure",
        "physical-linux-saturated-duplicate-retry-proof",
        "bounded-parallel-ipc-frame-read",
        "16-slowloris-timeout-isolation",
        "16-malformed-16-unauthorized-peer-isolation",
        "two-wave-batch-progress-under-5000ms",
        "physical-linux-hostile-peer-progress-proof",
        "repeated-hostile-batch-lifecycle-proof",
        "owner-heartbeat-continuity-under-hostile-load",
        "signal-cancellable-ipc-frame-read",
        "hard-total-frame-read-deadline",
        "bounded-sigterm-under-slowloris",
        "graceful-owner-and-socket-release",
        "immediate-same-path-restart",
        "physical-linux-repeated-hostile-lifecycle-proof",
        "three-cycle-hostile-resource-retention",
        "stable-post-batch-fd-task-baseline",
        "scoped-reader-thread-join",
        "saturated-plus-64-fd-task-proof",
        "per-cycle-proc-owner-socket-release",
        "physical-linux-hostile-resource-retention-proof",
        "accept-order-ready-prefix-dispatch",
        "later-reader-nonblocking-response",
        "three-saturated-reconnect-fairness-waves",
        "60-slow-4-valid-reconnect-wave",
        "twelve-bounded-reconnect-replays",
        "owner-heartbeat-under-reconnect-waves",
        "physical-linux-hostile-reconnect-fairness-proof",
        "maintenance-first-daemon-turn",
        "alternating-unix-ipc-https-priority",
        "three-saturated-cross-transport-waves",
        "bounded-authenticated-https-under-ipc-saturation",
        "owner-heartbeat-under-cross-transport-load",
        "stable-writer-generation-under-cross-transport-load",
        "physical-linux-cross-transport-fairness-proof",
        "authenticated-slow-https-body-timeout",
        "three-slow-https-cross-transport-waves",
        "four-concurrent-ipc-queries-per-wave",
        "twelve-bounded-ipc-queries-under-https-pressure",
        "owner-heartbeat-under-slow-https-pressure",
        "stable-writer-generation-under-slow-https-pressure",
        "physical-linux-symmetric-cross-transport-fairness-proof",
        "hard-total-tls-http-read-deadline",
        "signal-cancellable-remote-request-read",
        "100ms-remote-read-stop-poll",
        "pre-authority-cancellation-fence",
        "cancelled-response-suppression",
        "bounded-sigterm-under-authenticated-slow-https",
        "remote-owner-socket-release",
        "immediate-post-https-cancellation-restart",
        "physical-linux-slow-https-sigterm-proof",
        "repeated-remote-read-phase-shutdown",
        "incomplete-tls-handshake-cancellation",
        "incomplete-http-header-cancellation",
        "authenticated-body-cancellation-cycle",
        "stable-remote-idle-fd-task-baseline",
        "single-remote-connection-fd-accounting",
        "zero-remote-task-amplification",
        "per-phase-proc-owner-socket-release",
        "immediate-cross-phase-restart",
        "physical-linux-remote-read-phase-resource-proof",
        "nonblocking-tcp-rustls-underlay",
        "wouldblock-absorbed-before-rustls",
        "nonretryable-connection-aborted-cancellation",
        "sqlite-journal-aware-idle-baseline",
        "three-run-physical-phase-stability",
        "post-read-deadline-immediate-http-error",
        "collision-free-parallel-lifecycle-fixtures",
    ] {
        assert!(
            daemon_lifecycle
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing daemon lifecycle surface {surface}"
        );
    }
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path == "crates/leserpentd/src/ipc.rs" && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_lost_response_sigkill_recovery_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_repeated_unclean_recovery_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_post_recovery_contention_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_post_recovery_duplicate_retry_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_post_recovery_mixed_peer_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_repeated_hostile_lifecycle_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_hostile_resource_cycles_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_hostile_reconnect_fairness_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path == "crates/leserpentd/tests/cross_transport_fairness_vertical.rs"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_cross_transport_fairness_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_slow_https_cross_transport_fairness_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_slow_https_sigterm_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_remote_read_phase_shutdown_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_remote_backlog_shutdown_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(daemon_lifecycle.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_max_event_session_shutdown_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(
        daemon_lifecycle
            .next_gate
            .contains("physical Linux x86_64 evidence")
    );

    let compatibility_control = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-1x/control-plane/orchestration-persistence")
        .expect("Leserpent compatibility control-plane cell must exist");
    assert_eq!(compatibility_control.contract.version, "1.49.1");
    assert!(
        compatibility_control
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "pending-writer-cold-start-empty-read")
    );
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "apps/leserpent/src/Leserpent/ControlPlane/SqliteOrchestraRunStore.cs"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "apps/leserpent/tests/Leserpent.SecurityTests/SqliteOrchestraRunStoreTests.cs"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "apps/leserpent/src/Leserpent/ControlPlane/DaemonAuthorityWriterSession.cs"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "apps/leserpent/src/Leserpent/ControlPlane/RuntimeDeletionCommandIdentity.cs"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "apps/leserpent/src/Leserpent/ControlPlane/RuntimeDeletionAuthorityWorkflow.cs"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "apps/leserpent/src/Leserpent/ControlPlane/OrchestraDeleteCheckpointService.cs"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "apps/leserpent/src/Leserpent/ControlPlane/OrchestraDeleteCheckpointWorkerLease.cs"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "scripts/validation/leserpent_runtime_deletion_replay_horizon.sh"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "apps/leserpent/tests/Leserpent.SecurityTests/RuntimeDeletionReconciliationEndpointTests.cs"
            && item.state == EvidenceState::Present
    }));
    for surface in [
        "daemon-authoritative-sidecar-endpoint",
        "daemon-authoritative-runtime-timestamps",
        "legacy-timestamp-field-fallback",
        "reversed-timestamp-rejection",
        "daemon-authoritative-sidecar-status",
        "legacy-sidecar-status-fallback",
        "sanitized-sidecar-failure-intake",
        "daemon-first-sidecar-refresh",
        "composed-daemon-discovery-intake",
        "sanitized-runtime-status-failure-intake",
        "daemon-first-runtime-refresh",
        "daemon-first-fleet-refresh",
        "daemon-first-orchestra-refresh",
        "daemon-first-runtime-unregistration",
        "runtime-deletion-reservation",
        "reserved-session-and-orchestra-rejection",
        "durable-runtime-deletion-intent",
        "strict-deletion-intent-persistence",
        "schema-v2-control-state",
        "deletion-intent-restart-recovery",
        "background-delete-convergence",
        "real-forced-termination-recovery-proof",
        "retained-crash-boundary-evidence",
        "physical-linux-x86-64-crash-recovery-proof",
        "three-phase-runtime-deletion-fault-campaign",
        "repeated-linux-crash-convergence",
        "retained-fault-campaign-aggregate",
        "concurrent-registration-delete-recovery",
        "state-save-interference-proof",
        "cross-authority-unrelated-runtime-preservation",
        "physical-linux-concurrency-fault-proof",
        "offline-recovery-retry-proof",
        "same-database-daemon-restart",
        "owner-lease-respecting-restart",
        "physical-linux-daemon-restart-recovery",
        "unclean-daemon-owner-lease-takeover",
        "pre-expiry-double-owner-rejection",
        "natural-lease-failover-latency-evidence",
        "physical-linux-sigkill-recovery-proof",
        "overlapping-durable-deletion-intents",
        "mixed-boundary-single-takeover-proof",
        "independent-intent-claim-retry",
        "physical-linux-overlapping-intent-proof",
        "repeated-unclean-daemon-takeover",
        "partial-deletion-progress-preservation",
        "second-outage-claim-release",
        "physical-linux-repeated-takeover-proof",
        "poison-intent-isolation",
        "recovery-queue-fairness",
        "durable-poison-reservation",
        "post-repair-poison-convergence",
        "physical-linux-poison-isolation-proof",
        "high-cardinality-recovery-queue",
        "sparse-poison-progress",
        "per-pass-recovery-evidence",
        "cross-platform-serial-recovery-baseline",
        "bounded-runtime-deletion-recovery-batch",
        "bounded-authority-mutation-concurrency",
        "bounded-daemon-ipc-drain",
        "atomic-successful-deletion-batch",
        "cross-platform-sub-3000ms-recovery-proof",
        "strict-batch-persistence-rollback",
        "idempotent-daemon-unregistration-replay",
        "idempotent-orchestra-cleanup-replay",
        "cross-platform-batch-persistence-fault-proof",
        "saturated-128-intent-recovery",
        "cooperative-saturated-shutdown",
        "deterministic-multi-batch-fairness",
        "mixed-authority-pressure-proof",
        "cross-platform-saturated-queue-proof",
        "schema-v3-control-state",
        "durable-per-intent-retry-metadata",
        "bounded-exponential-delete-backoff",
        "ready-intent-skip-ahead",
        "sanitized-deletion-failure-code",
        "operator-runtime-deletion-status-api",
        "cross-platform-durable-backoff-proof",
        "revision-fenced-runtime-deletion-retry",
        "idempotent-retry-now-request",
        "durable-retry-now-audit",
        "post-convergence-retry-audit",
        "signaled-recovery-wakeup",
        "guarded-retry-now-api",
        "cross-platform-retry-now-proof",
        "linearizable-retry-claim-boundary",
        "deterministic-retry-claim-conflicts",
        "single-authority-mutation-under-retry-race",
        "cross-platform-retry-claim-race-proof",
        "retry-acknowledgement-crash-recovery",
        "post-daemon-commit-retry-replay",
        "cross-platform-retry-crash-proof",
        "monotonic-retry-audit-linearization",
        "deterministic-oldest-first-audit-eviction",
        "explicit-retry-replay-horizon",
        "cross-platform-retry-rollover-proof",
        "atomic-retry-audit-rollover-persistence",
        "old-or-new-audit-window-recovery",
        "observed-temp-file-host-termination",
        "cross-platform-atomic-rollover-proof",
        "atomic-control-state-backup-refresh",
        "corrupted-primary-backup-recovery",
        "complete-prior-audit-window-fallback",
        "cross-platform-atomic-backup-proof",
        "typed-control-state-load-provenance",
        "secret-free-primary-load-failure-code",
        "health-degraded-backup-recovery",
        "cross-platform-recovery-provenance-proof",
        "post-recovery-known-good-backup-preservation",
        "corrupted-primary-backup-refresh-fence",
        "old-or-new-post-recovery-write",
        "cross-platform-post-recovery-write-proof",
        "shared-control-state-semantic-validator",
        "semantic-invalid-generation-fallback",
        "semantic-generation-promotion-fence",
        "cross-platform-semantic-generation-proof",
        "runtime-session-identity-uniqueness",
        "case-insensitive-topology-collision-fence",
        "session-runtime-referential-integrity",
        "pre-projection-import-validation",
        "legacy-orchestra-run-identity-uniqueness",
        "legacy-orchestra-runtime-referential-integrity",
        "pre-migration-orchestra-generation-fence",
        "no-silent-legacy-run-filtering",
        "per-runtime-orchestra-request-identity-uniqueness",
        "retained-orchestra-retry-lineage-validation",
        "retention-aware-retry-parent-validation",
        "pre-sqlite-lineage-fence",
        "orchestra-lifecycle-outcome-validation",
        "active-completion-consistency",
        "monotonic-orchestra-completion-time",
        "bounded-orchestra-step-payload",
        "legacy-null-completion-compatibility",
        "runtime-payload-required-field-validation",
        "monotonic-runtime-persistence-timestamps",
        "bounded-unique-runtime-capabilities",
        "session-payload-required-field-validation",
        "monotonic-session-persistence-timestamps",
        "bounded-unique-session-requirements",
        "validated-sidecar-memory-envelope",
        "pre-projection-runtime-session-payload-fence",
        "fixed-discovery-diagnostic-codes",
        "runtime-status-source-coherence",
        "sidecar-status-source-coherence",
        "bounded-persisted-diagnostic-text",
        "secret-free-persistence-health-errors",
        "pre-projection-diagnostic-fence",
        "shared-orchestra-envelope-validator",
        "bounded-orchestra-operator-metadata",
        "bounded-secret-free-step-and-event-summaries",
        "run-event-identity-outcome-coherence",
        "monotonic-orchestra-event-time",
        "fail-closed-orchestra-authority-read",
        "cross-language-orchestra-envelope-parity",
        "pre-sqlite-orchestra-metadata-fence",
        "deterministic-legacy-event-origin-backfill",
        "strict-monotonic-orchestra-event-id",
        "monotonic-orchestra-event-recording-time",
        "legal-orchestra-event-transition-chain",
        "terminal-run-event-correspondence",
        "corrupted-event-read-fail-closed",
        "atomic-sqlite-event-sequence-validation",
        "secret-free-event-history-unavailable-response",
        "rust-authority-event-append-sequence-validation",
        "transactional-previous-outcome-fence",
        "transactional-rfc3339-event-time-monotonicity",
        "idempotent-event-replay-before-sequence-check",
        "terminal-event-append-rejection",
        "cross-process-illegal-event-rejection",
        "rust-authority-history-row-validation",
        "transactional-history-read-snapshot",
        "complete-event-sequence-before-pagination",
        "sqlite-column-envelope-coherence",
        "bounded-orchestra-event-cardinality",
        "cross-process-corrupted-history-rejection",
        "batched-run-list-event-validation",
        "single-batch-event-query",
        "bounded-run-list-event-lookahead",
        "run-list-terminal-event-correspondence",
        "pagination-lookahead-corruption-fence",
        "cross-process-corrupted-run-list-rejection",
        "no-n-plus-one-history-validation",
        "transactional-append-envelope-column-coherence",
        "native-caller-poison-write-rejection",
        "canonical-zero-event-id-admission",
        "request-id-envelope-correspondence",
        "atomic-malformed-append-rollback",
        "transactional-retained-history-admission",
        "replay-on-corruption-rejection",
        "extension-on-corruption-rejection",
        "retained-request-id-column-coherence",
        "cross-process-corrupted-replay-rejection",
        "validated-predecessor-reuse",
        "shared-retained-run-validator",
        "history-request-id-column-coherence",
        "run-specific-request-id-drift-rejection",
        "run-list-request-id-drift-rejection",
        "pagination-lookahead-request-id-fence",
        "cross-process-request-id-drift-rejection",
        "post-append-snapshot-validation",
        "validated-persistence-receipt",
        "transaction-generation-receipt-binding",
        "post-write-column-drift-rollback",
        "post-write-generation-drift-rollback",
        "cross-process-post-write-rollback",
        "validated-replay-receipt-readback",
        "explicit-bounded-retention-plan",
        "monotonic-orchestra-transaction-generation",
        "clock-rollback-current-run-preservation",
        "complete-retained-run-validation",
        "batched-retained-event-validation",
        "runtime-event-cardinality-reconciliation",
        "validated-eviction-cascade",
        "retention-fault-atomic-rollback",
        "cross-process-retention-failure",
        "retention-retry-convergence",
        "set-based-multi-runtime-orchestra-delete",
        "bounded-pre-delete-structural-snapshot",
        "event-parent-runtime-ownership-fence",
        "malformed-envelope-safe-cleanup",
        "exact-cascade-mutation-budget",
        "post-delete-target-absence",
        "validated-orchestra-delete-receipt",
        "unrelated-runtime-mutation-fence",
        "shared-unregistration-delete-postconditions",
        "unregistration-delete-fault-rollback",
        "cross-process-delete-failure",
        "zero-count-delete-retry",
        "canonical-unregistration-operation-request",
        "bounded-unregistration-receipt-validation",
        "persisted-target-derived-tombstone",
        "transactional-unregistration-replay-snapshot",
        "complete-orchestra-replay-tombstone",
        "replay-on-tombstone-drift-rejection",
        "replay-on-operation-corruption-rejection",
        "repaired-tombstone-retry-convergence",
        "cross-process-unregistration-replay-failure",
        "fixed-unregistration-replay-error",
        "post-insert-unregistration-operation-readback",
        "canonical-runtime-journal-tombstones",
        "exact-journal-tombstone-cardinality",
        "ambiguous-journal-tombstone-rejection",
        "journal-tombstone-terminal-state-fence",
        "live-runtime-projection-tombstone",
        "unregistration-journal-fault-rollback",
        "compaction-preserved-unregistration-evidence",
        "cross-process-journal-replay-failure",
        "repaired-journal-replay-convergence",
        "fixed-256-unregistration-replay-horizon",
        "oldest-insertion-linearized-operation-eviction",
        "pure-replay-horizon-convergence",
        "atomic-operation-horizon-rollover",
        "operation-only-first-phase-eviction",
        "retained-operation-journal-protection",
        "fallback-snapshot-covered-tombstone-compaction",
        "bounded-1000-journal-compaction",
        "monotonic-unregistration-timestamp",
        "outside-horizon-command-id-reuse",
        "rollover-fault-rollback",
        "post-rollover-restart-proof",
        "sqlite-v15-unregistration-generations",
        "v14-insertion-order-generation-migration",
        "schema-owned-oldest-generation-eviction",
        "atomic-generation-allocation",
        "durable-eviction-high-water",
        "contiguous-replay-generation-window",
        "generation-state-fault-rollback",
        "queryable-replay-horizon-metadata",
        "authenticated-health-replay-horizon",
        "native-cli-replay-horizon",
        "strict-avalonia-replay-horizon",
        "daemon-generation-bound-unregistration-receipt",
        "legacy-generation-absence-without-zero",
        "read-only-unregistration-receipt-recovery",
        "cross-process-reconciliation-commit-crash-proof",
        "old-or-new-reconciliation-generation",
        "observed-reconciliation-temp-write-termination",
        "cross-platform-reconciliation-commit-proof",
        "cross-authority-reconciliation-crash-proof",
        "orchestra-before-control-commit-boundary",
        "idempotent-absent-history-retry",
        "unrelated-orchestra-history-preservation",
        "single-audit-cross-authority-convergence",
        "cross-platform-cross-authority-proof",
        "authenticated-https-authority-writer-headers",
        "remote-wire-generation-fence",
        "remote-specialized-route-predecode-fence",
        "read-only-remote-fence-exemptions",
        "source-scanned-csharp-mutation-inventory",
        "source-scanned-rust-https-inventory",
        "exhaustive-rust-request-fence-policy",
        "runtime-refresh-generation-fence",
        "bootstrap-session-bind-generation-fence",
        "real-daemon-cold-takeover-proof",
        "deterministic-precommit-writer-claim-sigkill",
        "rollback-journal-writer-claim-recovery",
        "natural-owner-lease-expiry-after-claim-crash",
        "postcommit-writer-claim-sigkill-durability",
        "physical-linux-writer-claim-crash-proof",
        "lost-writer-claim-response",
        "concurrent-same-and-competing-writer-linearization",
        "explicit-dual-claim-order-contract",
        "final-writer-ticket-mutation-proof",
        "physical-linux-lost-response-race-proof",
        "cold-restart-lost-claim-response-replay",
        "queued-replay-before-competing-claim",
        "connectable-listener-readiness-proof",
        "second-cold-restart-final-replay",
        "physical-linux-cold-response-replay-proof",
        "lost-response-sigkill-combination-proof",
        "pre-expiry-stale-socket-preservation",
        "natural-lease-same-socket-recovery",
        "stale-socket-safe-reclamation",
        "physical-linux-unclean-response-recovery-proof",
        "repeated-unclean-response-recovery",
        "two-natural-lease-expiry-cycles",
        "contiguous-writer-generations-1-through-5",
        "repeated-same-socket-reclamation",
        "physical-linux-repeated-unclean-response-proof",
        "post-recovery-64-writer-contention",
        "bounded-claim-admission-budget",
        "contiguous-writer-generations-3-through-66",
        "max-generation-sole-mutation-authority",
        "physical-linux-post-recovery-contention-proof",
        "saturated-duplicate-writer-retries",
        "abandoned-claim-response-recovery",
        "16-new-claims-48-stable-replays",
        "contiguous-writer-generations-3-through-18",
        "physical-linux-saturated-duplicate-retry-proof",
        "post-recovery-hostile-peer-admission",
        "malformed-and-unauthorized-claim-isolation",
        "16-valid-claims-under-48-invalid-or-slow-peers",
        "invalid-peers-zero-generation-allocation",
        "physical-linux-hostile-peer-progress-proof",
        "repeated-hostile-batch-lifecycle-proof",
        "owner-heartbeat-continuity-under-hostile-load",
        "signal-cancellable-ipc-frame-read",
        "hard-total-frame-read-deadline",
        "bounded-sigterm-under-slowloris",
        "graceful-owner-and-socket-release",
        "immediate-same-path-restart",
        "physical-linux-repeated-hostile-lifecycle-proof",
        "three-cycle-hostile-resource-retention",
        "stable-post-batch-fd-task-baseline",
        "scoped-reader-thread-join",
        "saturated-plus-64-fd-task-proof",
        "per-cycle-proc-owner-socket-release",
        "physical-linux-hostile-resource-retention-proof",
        "accept-order-ready-prefix-dispatch",
        "later-reader-nonblocking-response",
        "three-saturated-reconnect-fairness-waves",
        "60-slow-4-valid-reconnect-wave",
        "twelve-bounded-reconnect-replays",
        "owner-heartbeat-under-reconnect-waves",
        "physical-linux-hostile-reconnect-fairness-proof",
        "maintenance-first-daemon-turn",
        "alternating-unix-ipc-https-priority",
        "three-saturated-cross-transport-waves",
        "bounded-authenticated-https-under-ipc-saturation",
        "owner-heartbeat-under-cross-transport-load",
        "stable-writer-generation-under-cross-transport-load",
        "physical-linux-cross-transport-fairness-proof",
        "authenticated-slow-https-body-timeout",
        "three-slow-https-cross-transport-waves",
        "four-concurrent-ipc-queries-per-wave",
        "twelve-bounded-ipc-queries-under-https-pressure",
        "owner-heartbeat-under-slow-https-pressure",
        "stable-writer-generation-under-slow-https-pressure",
        "physical-linux-symmetric-cross-transport-fairness-proof",
        "hard-total-tls-http-read-deadline",
        "signal-cancellable-remote-request-read",
        "100ms-remote-read-stop-poll",
        "pre-authority-cancellation-fence",
        "cancelled-response-suppression",
        "bounded-sigterm-under-authenticated-slow-https",
        "remote-owner-socket-release",
        "immediate-post-https-cancellation-restart",
        "physical-linux-slow-https-sigterm-proof",
        "repeated-remote-read-phase-shutdown",
        "incomplete-tls-handshake-cancellation",
        "incomplete-http-header-cancellation",
        "authenticated-body-cancellation-cycle",
        "stable-remote-idle-fd-task-baseline",
        "single-remote-connection-fd-accounting",
        "zero-remote-task-amplification",
        "per-phase-proc-owner-socket-release",
        "immediate-cross-phase-restart",
        "physical-linux-remote-read-phase-resource-proof",
        "nonblocking-tcp-rustls-underlay",
        "wouldblock-absorbed-before-rustls",
        "nonretryable-connection-aborted-cancellation",
        "sqlite-journal-aware-idle-baseline",
        "three-run-physical-phase-stability",
        "post-read-deadline-immediate-http-error",
        "collision-free-parallel-lifecycle-fixtures",
    ] {
        assert!(
            compatibility_control
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing compatibility authority surface {surface}"
        );
    }
    assert_eq!(compatibility_control.contract.version, "1.49.1");
    assert!(
        compatibility_control
            .next_gate
            .contains("physical Linux x86_64 evidence")
    );
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "crates/leserpentd/tests/authority_writer_takeover_vertical.rs"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_claim_crash_linux_x86_64_20260801.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_lost_response_race_linux_x86_64_20260801.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_lost_response_cold_replay_linux_x86_64_20260801.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_lost_response_sigkill_recovery_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_repeated_unclean_recovery_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_post_recovery_contention_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_post_recovery_duplicate_retry_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_post_recovery_mixed_peer_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_repeated_hostile_lifecycle_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_hostile_resource_cycles_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_authority_writer_hostile_reconnect_fairness_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "crates/leserpentd/tests/cross_transport_fairness_vertical.rs"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_cross_transport_fairness_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path
            == "docs/fixtures/leserpent_slow_https_cross_transport_fairness_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_slow_https_sigterm_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_remote_read_phase_shutdown_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_remote_backlog_shutdown_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));
    assert!(compatibility_control.evidence.iter().any(|item| {
        item.path == "docs/fixtures/leserpent_max_event_session_shutdown_linux_x86_64_20260802.json"
            && item.state == EvidenceState::Present
    }));

    let reconciliation = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-1x/control-plane/runtime-deletion-reconciliation")
        .expect("runtime deletion reconciliation cell must exist");
    assert_eq!(reconciliation.maturity, Maturity::Mature);
    assert_eq!(reconciliation.completion, 100);
    assert_eq!(reconciliation.contract.stability, ContractStability::Stable);
    assert_eq!(reconciliation.contract.version, "1.13.0");
    for surface in [
        "schema-v6-control-state",
        "typed-daemon-reconciliation-snapshot",
        "revision-bound-runtime-deletion-reconciliation",
        "reappeared-runtime-identity-fence",
        "atomic-reconciliation-cleanup-audit",
        "bounded-reconciliation-audit-retention",
        "idempotent-reconciliation-request-replay",
        "guarded-runtime-deletion-reconciliation-api",
        "cross-platform-reconciliation-proof",
        "cross-process-reconciliation-commit-crash-proof",
        "old-or-new-reconciliation-generation",
        "observed-reconciliation-temp-write-termination",
        "cross-platform-reconciliation-commit-proof",
        "cross-authority-reconciliation-crash-proof",
        "orchestra-before-control-commit-boundary",
        "idempotent-absent-history-retry",
        "unrelated-orchestra-history-preservation",
        "single-audit-cross-authority-convergence",
        "cross-platform-cross-authority-proof",
        "intent-derived-orchestra-cleanup-command",
        "sqlite-v16-orchestra-delete-receipts",
        "atomic-cleanup-receipt-commit",
        "durable-cleanup-generation-replay",
        "cleanup-command-target-conflict-fence",
        "reconciliation-audit-cleanup-generation-binding",
        "cross-platform-cleanup-receipt-crash-proof",
        "sqlite-v17-orchestra-delete-replay-horizon",
        "fixed-4096-cleanup-receipt-capacity",
        "queryable-cleanup-replay-horizon",
        "authenticated-cleanup-horizon-query",
        "monotonic-audit-generation-checkpoint",
        "checkpoint-before-prefix-compaction",
        "durable-cleanup-eviction-high-water",
        "lossless-v16-cleanup-receipt-migration",
        "daemon-local-store-horizon-parity",
        "startup-audit-horizon-fence",
        "arm64-cleanup-horizon-crash-proof",
        "physical-linux-x64-cleanup-horizon-crash-proof",
        "typed-cleanup-replay-admission-posture",
        "queryable-cleanup-replay-available-capacity",
        "pinned-cleanup-horizon-saturation-diagnostic",
        "actionable-cleanup-checkpoint-remediation",
        "typed-cleanup-saturation-wire-error",
        "daemon-cli-local-store-saturation-parity",
        "checkpoint-restored-cleanup-admission",
        "protected-cleanup-horizon-pressure-model",
        "cleanup-horizon-warning-threshold-512",
        "cleanup-horizon-critical-threshold-128",
        "false-positive-free-unpinned-horizon",
        "cross-language-cleanup-pressure-parity",
        "pre-saturation-operator-remediation",
        "cleanup-checkpoint-race-linearization",
        "cleanup-first-race-proof",
        "checkpoint-first-race-proof",
        "cross-platform-cleanup-checkpoint-race-proof",
        "physical-linux-cleanup-checkpoint-race-proof",
        "sqlite-v18-cleanup-checkpoint-high-water",
        "conservative-v17-cleanup-checkpoint-migration",
        "sqlite-v5-local-cleanup-checkpoint-high-water",
        "exact-cleanup-checkpoint-lag",
        "cleanup-pressure-hysteresis",
        "cleanup-warning-recovery-threshold-768",
        "cleanup-critical-recovery-threshold-256",
        "audit-durable-automatic-cleanup-checkpoint",
        "replay-triggered-cleanup-checkpoint-repair",
        "startup-triggered-cleanup-checkpoint-repair",
        "queryable-cleanup-checkpoint-status",
        "source-generated-cleanup-status-json",
        "daemon-restart-checkpoint-convergence",
        "cross-platform-auto-checkpoint-restart-proof",
        "ipc-peer-disconnect-isolation",
        "physical-linux-peer-disconnect-proof",
        "schema-v7-durable-checkpoint-monitor",
        "full-state-prewrite-monitor-validation",
        "bounded-automatic-checkpoint-backoff",
        "automatic-checkpoint-retry-ceiling-30s",
        "durable-last-trusted-cleanup-horizon",
        "stale-cleanup-pressure-observation",
        "restart-safe-checkpoint-alert",
        "generation-fenced-alert-acknowledgement",
        "mutate-intent-alert-acknowledgement-api",
        "new-incident-acknowledgement-invalidation",
        "daemon-history-outage-degraded-startup",
        "prolonged-daemon-outage-pressure-proof",
        "schema-v8-durable-checkpoint-alert-outbox",
        "hosted-automatic-checkpoint-scheduler",
        "poll-free-daemon-recovery-convergence",
        "stable-checkpoint-alert-event-id",
        "attempt-before-alert-delivery",
        "at-least-once-checkpoint-alert-delivery",
        "bounded-alert-delivery-backoff",
        "restart-safe-alert-outbox-drain",
        "generation-bound-alert-outbox-validation",
        "structured-logging-alert-sink",
        "process-lifetime-checkpoint-worker-lease",
        "canonical-state-path-lease-identity",
        "pid-start-token-owner-record",
        "runtime-owner-token-revalidation",
        "owner-private-nonsymlink-lease",
        "live-duplicate-host-standby",
        "registry-checkpoint-ownership-fence",
        "single-checkpoint-authority-mutation",
        "single-alert-notification",
        "cross-process-checkpoint-lease-proof",
        "force-kill-stale-owner-recovery",
        "already-covered-checkpoint-suppression",
        "authenticated-https-alert-sink",
        "private-file-alert-token",
        "inline-alert-secret-rejection",
        "redirect-free-alert-delivery",
        "wire-v1-alert-envelope",
        "idempotency-key-alert-delivery",
        "authenticated-checkpoint-worker-health",
        "source-generated-checkpoint-worker-health",
        "secret-free-checkpoint-worker-health",
        "alert-sink-delivery-health",
        "linux-proc-start-identity",
        "legacy-linux-start-time-compatibility",
        "physical-linux-checkpoint-worker-duplicate-host-proof",
        "standby-non-reentry",
        "fresh-process-stale-owner-takeover",
        "process-wide-control-writer-lease",
        "fail-closed-http-mutation-policy",
        "registry-pre-mutation-writer-fence",
        "standby-startup-repair-suppression",
        "background-worker-writer-gate",
        "authenticated-control-writer-health",
        "fixed-standby-mutation-rejection",
        "fresh-process-control-writer-takeover",
    ] {
        assert!(
            reconciliation
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing reconciliation surface {surface}"
        );
    }
    assert!(reconciliation.blockers.is_empty());
    assert!(reconciliation.next_gate.contains("local and remote"));

    let bootstrap = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/deployment-bootstrap/reverse-bootstrap")
        .expect("reverse deployment bootstrap must be tracked independently");
    assert_eq!(bootstrap.maturity, Maturity::Mature);
    assert_eq!(bootstrap.completion, 100);
    assert_eq!(bootstrap.contract.stability, ContractStability::Stable);
    assert_eq!(bootstrap.contract.version, "1.0.1");
    for surface in [
        "bounded-native-service-manager-batch",
        "timed-out-service-manager-child-reaping",
    ] {
        assert!(
            bootstrap
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing reverse-bootstrap surface {surface}"
        );
    }
    assert!(
        bootstrap
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "native-rust-ssh-transport")
    );
    assert!(
        bootstrap
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "strict-bootstrap-origin-config-v1")
    );
    assert!(
        bootstrap
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "independent-authenticated-bootstrap-submission")
    );
    assert!(
        bootstrap
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "avalonia-phase-gated-session-binding")
    );
    assert!(
        bootstrap
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "session-bound-add-to-hub-gate")
    );
    for surface in [
        "independent-bootstrap-retirement-wire-v1",
        "five-identity-bootstrap-retirement-binding",
        "native-bootstrap-retire-v1-entry",
        "private-three-phase-bootstrap-retirement-marker",
        "generation-fenced-retirement-cleanup",
        "retained-bootstrap-state-and-logs",
        "macos-native-install-retirement-process-proof",
        "transport-returned-bootstrap-generation",
        "pinned-native-ssh-bootstrap-retirement",
        "operation-specific-retirement-staging",
        "forged-generation-retirement-rejection",
        "physical-linux-bootstrap-retirement",
        "cross-host-bootstrap-retirement-replay",
        "post-retirement-residue-audit",
        "retired-generation-reinstall-fence",
        "terminal-retirement-absence-proof",
        "exact-install-profile-policy",
        "fixed-noninteractive-system-profile-elevation",
        "privileged-system-profile-bootstrap-retirement",
        "system-wide-systemd-retirement-proof",
        "temporary-sudo-authority-revocation",
        "system-profile-residue-audit",
        "validated-install-generation-checkpoint",
        "install-profile-authority-binding",
        "legacy-checkpoint-retirement-ineligibility",
        "avalonia-retirement-authority-projection",
        "session-bound-daemon-retirement-admission",
        "checkpoint-derived-daemon-retirement-authority",
        "public-retirement-intent-authority-omission",
        "private-daemon-retirement-effect-v1",
        "typed-daemon-retirement-lifecycle",
        "policy-revalidated-daemon-retirement",
        "transport-response-rebinding",
        "runtime-schema-20-daemon-retirement-authority",
        "atomic-daemon-retirement-effect-checkpoint",
        "restart-safe-daemon-retirement-settlement",
        "independent-retirement-operation-kind",
        "checkpoint-derived-submission-effect",
        "production-daemon-retirement-adapter-registration",
        "explicit-daemon-retirement-ipc-route",
        "authenticated-daemon-retirement-https-route",
        "bounded-daemon-retirement-route-payload",
        "adapter-registration-daemon-retirement-route-gate",
        "retirement-operation-namespace-isolation",
        "daemon-retirement-auth-error-envelope",
        "native-cli-confirmed-daemon-retirement",
        "authenticated-cli-daemon-retirement-ipc-https",
        "authority-omitting-cli-daemon-retirement-request",
        "bounded-cli-daemon-retirement-progress",
        "daemon-retirement-terminal-exit-codes",
        "credential-free-daemon-retirement-output",
        "explicit-cli-daemon-retirement-confirmation",
        "avalonia-confirmed-daemon-retirement",
        "authenticated-avalonia-daemon-retirement-https",
        "authority-omitting-avalonia-daemon-retirement",
        "bootstrap-bound-avalonia-daemon-retirement",
        "identity-locked-avalonia-daemon-retirement-progress",
        "bounded-avalonia-daemon-retirement-poll",
        "credential-free-avalonia-daemon-retirement-status",
        "daemon-retirement-failure-recovery-guidance",
        "hub-separated-daemon-runtime-lifecycle-actions",
    ] {
        assert!(
            bootstrap
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing bootstrap retirement surface {surface}"
        );
    }
    assert!(bootstrap.blockers.is_empty());
    assert!(bootstrap.next_gate.contains("optional post-2.0"));
    assert!(bootstrap.evidence.iter().any(|evidence| {
        evidence.path == "docs/fixtures/leserpent_real_ssh_bootstrap_retirement_20260727.json"
    }));
    assert!(bootstrap.evidence.iter().any(|evidence| {
        evidence.path == "docs/fixtures/leserpent_real_ssh_system_profile_retirement_20260728.json"
            && evidence.state == EvidenceState::Present
    }));
    assert!(
        !bootstrap
            .blockers
            .iter()
            .any(|blocker| blocker.id == "bootstrap-production-entry-missing")
    );
    assert!(
        !bootstrap
            .blockers
            .iter()
            .any(|blocker| blocker.id.contains("gewyvern-retirement"))
    );
    assert!(
        !bootstrap
            .blockers
            .iter()
            .any(|blocker| blocker.id == "post-bind-gewyvern-ui-missing")
    );
    assert!(bootstrap.blockers.iter().all(|blocker| {
        !blocker
            .summary
            .contains("still needs service activation, registration proof")
    }));

    let desktop_localization = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/ui-renderers/desktop-localization")
        .expect("desktop localization contract must remain tracked");
    assert_eq!(desktop_localization.maturity, Maturity::Incubating);
    assert_eq!(desktop_localization.completion, 95);
    assert_eq!(desktop_localization.contract.version, "0.8.0");
    assert_eq!(desktop_localization.contract.stability, ContractStability::Draft);
    for surface in [
        "thirty-official-locale-identifiers",
        "private-atomic-language-preference",
        "native-language-settings-window",
        "ui-ir-localized-text-resolution",
        "deterministic-english-localization-fallback",
        "eight-complete-built-in-shell-catalogs",
        "built-in-shell-format-contract",
        "web-term-aligned-built-in-catalogs",
        "eight-built-in-language-selector-layout-envelopes",
        "seven-non-english-built-in-semantic-catalogs",
        "semantic-catalog-exact-key-set",
        "seven-built-in-localized-ui-ir-control-probes",
        "seven-built-in-connection-specialist-catalogs",
        "connection-catalog-exact-key-set",
        "localized-connection-and-forget-controls",
        "eight-built-in-connection-layout-envelopes",
        "live-connection-language-reprojection",
        "seven-built-in-bootstrap-specialist-catalogs",
        "bootstrap-catalog-exact-key-set",
        "localized-reverse-deployment-controls",
        "eight-built-in-bootstrap-layout-envelopes",
        "live-bootstrap-language-reprojection",
        "seven-built-in-provisioning-specialist-catalogs",
        "provisioning-catalog-exact-key-set",
        "localized-gewyvern-provisioning-controls",
        "eight-built-in-provisioning-layout-envelopes",
        "live-provisioning-language-reprojection",
        "seven-built-in-retirement-specialist-catalogs",
        "retirement-catalog-exact-key-set",
        "localized-gewyvern-retirement-controls",
        "eight-built-in-retirement-layout-envelopes",
        "live-retirement-language-reprojection",
        "shared-strict-domain-catalog-contract",
        "seven-built-in-daemon-retirement-specialist-catalogs",
        "daemon-retirement-catalog-exact-key-set",
        "localized-daemon-retirement-controls",
        "eight-built-in-daemon-retirement-layout-envelopes",
        "live-daemon-retirement-language-reprojection",
        "seven-built-in-startup-recovery-specialist-catalogs",
        "startup-recovery-catalog-exact-key-set",
        "localized-startup-recovery-controls",
        "token-redacted-startup-detail-preservation",
        "eight-built-in-startup-recovery-layout-envelopes",
        "live-startup-recovery-language-reprojection",
        "seven-built-in-account-specialist-catalogs",
        "account-catalog-exact-key-set",
        "typed-account-presentation-status",
        "account-phase-status-compatibility-fence",
        "localized-account-controls",
        "eight-built-in-account-layout-envelopes",
        "live-account-language-reprojection",
        "minimum-hub-layout-envelope",
        "zh-cn-complete-learning-center",
        "rtl-native-flow-direction",
    ] {
        assert!(
            desktop_localization
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing desktop localization surface {surface}"
        );
    }
    assert!(desktop_localization.blockers.iter().any(|blocker| {
        blocker.id == "desktop-long-tail-language-review"
            && blocker.summary.contains("eight built-in locales")
            && blocker.summary.contains("seven non-English built-ins")
            && blocker.summary.contains("exact 275-key semantic set")
            && blocker.summary.contains("daemon retirement")
            && blocker.summary.contains("startup recovery")
            && blocker.summary.contains("typed presentation status")
            && blocker.summary.contains("native-speaker review")
    }));
    assert!(desktop_localization.evidence.iter().any(|evidence| {
        evidence.path
            == "apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopLocalization.cs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(desktop_localization.evidence.iter().any(|evidence| {
        evidence.path
            == "apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopBuiltInShellCatalogs.cs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(desktop_localization.evidence.iter().any(|evidence| {
        evidence.path
            == "apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopBuiltInSemanticCatalogs.cs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(desktop_localization.evidence.iter().any(|evidence| {
        evidence.path
            == "apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopConnectionCatalogs.cs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(desktop_localization.evidence.iter().any(|evidence| {
        evidence.path
            == "apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopBootstrapDeploymentCatalogs.cs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(desktop_localization.evidence.iter().any(|evidence| {
        evidence.path
            == "apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopProvisioningCatalogs.cs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(desktop_localization.evidence.iter().any(|evidence| {
        evidence.path
            == "apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopRetirementCatalogs.cs"
            && evidence.state == EvidenceState::Present
    }));
    for path in [
        "apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopDomainCatalogContract.cs",
        "apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopDaemonRetirementCatalogs.cs",
        "apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopStartupRecoveryCatalogs.cs",
        "apps/leserpent-avalonia/src/Leserpent.Avalonia/DesktopAccountCatalogs.cs",
        "apps/leserpent-avalonia/src/Leserpent.Avalonia/DaemonRetirementWindow.cs",
        "apps/leserpent-avalonia/src/Leserpent.Avalonia/StartupErrorWindow.cs",
        "apps/leserpent-avalonia/src/Leserpent.Avalonia/SilvortexAccountControl.cs",
        "apps/leserpent-avalonia/src/Leserpent.Avalonia/HubWindow.cs",
    ] {
        assert!(desktop_localization.evidence.iter().any(|evidence| {
            evidence.path == path && evidence.state == EvidenceState::Present
        }));
    }

    let remote_console = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/remote-console/remote-mobile-console")
        .expect("remote/mobile console contract must remain tracked");
    assert_eq!(remote_console.completion, 99);
    assert_eq!(remote_console.contract.version, "0.69.0");
    for surface in [
        "renderer-neutral-runtime-search",
        "cross-authority-topology-search",
        "mobile-reusable-runtime-search",
        "frontend-independent-topology-refresh-coordinator",
        "bounded-topology-refresh-fanout",
        "mobile-reusable-topology-refresh-policy",
        "terminal-topology-refresh-summary",
        "cancelled-queued-topology-refresh-fence",
        "frontend-independent-workspace-launch-coordinator",
        "coalesced-workspace-launch-revision-fence",
        "bounded-active-pending-workspace-admission",
        "terminal-workspace-launch-cancellation",
        "mobile-reusable-workspace-launch-policy",
        "authoritative-runtime-removal-rejection",
        "frontend-independent-mutation-coordinator",
        "authoritative-snapshot-mutation-admission",
        "cached-heartbeat-mutation-rejection",
        "operation-token-stale-callback-fence",
        "malformed-response-unknown-outcome-fence",
        "frontend-independent-mutation-failure-classification",
        "mobile-reusable-mutation-failure-policy",
        "stale-mutation-completion-suppression",
        "bounded-mutation-failure-diagnostics",
        "mobile-reusable-mutation-lifecycle",
        "authoritative-snapshot-inspect-admission",
        "frontend-independent-authority-health-coordinator",
        "authority-health-single-flight",
        "authority-health-generation-fence",
        "authority-health-cancellation-restoration",
        "authority-health-bounded-failure-classification",
        "mobile-reusable-authority-health-lifecycle",
        "frontend-independent-event-lifecycle",
        "event-run-generation-handle",
        "idempotent-concurrent-event-shutdown",
        "mobile-reusable-event-lifecycle",
        "subscriber-failure-isolation",
        "bounded-subscriber-failure-count",
        "frontend-independent-typed-ui-action-router",
        "opaque-action-node-identities",
        "runtime-context-action-binding",
        "action-availability-routing",
        "deployment-form-event-routing",
        "deployment-submission-source-fence",
        "mobile-reusable-ui-action-routing",
        "strict-orchestra-delete-replay-health-codec",
        "visible-orchestra-delete-replay-pressure",
        "complete-runtime-projection-wire-coverage",
        "strict-runtime-authority-timestamp-codec",
        "renderer-neutral-mobile-layout-policy",
        "compact-medium-expanded-mobile-width-classes",
        "font-scale-aware-mobile-breakpoints",
        "safe-area-and-display-cutout-mobile-insets",
        "ime-aware-mobile-action-area",
        "allocation-free-mobile-layout-plan",
        "ime-structural-reflow-fence",
        "minimum-48dp-touch-targets",
        "runtime-first-mobile-onboarding",
        "collapsible-sensitive-mobile-setup",
        "bounded-mobile-content-width",
        "expanded-mobile-two-pane-layout",
        "adaptive-mobile-runtime-columns",
        "locked-android-arm64-x64-rid-graph",
        "host-rid-free-android-release",
        "standalone-debug-apk-opt-in",
        "dual-abi-android-aot-package",
        "android-brand-icon-package",
        "api36-arm64-emulator-runtime-proof",
        "compact-medium-expanded-emulator-matrix",
        "short-landscape-emulator-proof",
        "large-font-emulator-proof",
        "ime-action-visibility-emulator-proof",
        "release-flag-secure-emulator-proof",
        "cold-hot-relaunch-emulator-proof",
        "mobile-immutable-ui-document-binding",
        "exact-mobile-native-presentation-equivalence",
        "heartbeat-stable-mobile-native-render",
        "native-render-state-fence",
        "keychain-independent-mobile-conformance-certificate",
        "android-renderer-neutral-native-controls",
        "android-first-frame-ui-document",
        "android-native-workspace-query",
        "android-workspace-back-navigation",
        "android-parameterized-form-event-controls",
        "android-explicit-mutation-confirmation",
        "android-typed-mutation-transport",
        "android-shared-mutation-coordinator",
        "mobile-fixed-principal",
        "mobile-operation-generation-fence",
        "shared-mobile-connection-profile-store",
        "endpoint-hashed-mobile-ca-cache-paths",
        "atomic-mobile-ca-profile",
        "malformed-mobile-profile-fail-closed",
        "ios-application-entry",
        "ios-scene-lifecycle",
        "ios-app-switcher-privacy-shield",
        "ios-native-hub",
        "ios-renderer-neutral-native-controls",
        "ios-native-workspace-query",
        "ios-parameterized-form-event-controls",
        "ios-explicit-mutation-confirmation",
        "ios-shared-mutation-coordinator",
        "ios-adaptive-safe-area-layout",
        "ios-dynamic-type-layout",
        "ios-keyboard-layout-guide",
        "ios-brand-icon-package",
        "ios-debug-keychain-proof",
        "compact-ios-header-reflow",
        "native-ios-process-argument-proof-switches",
        "ios26-simulator-runtime-proof",
        "ios-keychain-simulator-proof",
        "ios-debug-simulator-package",
        "ios-release-aot-package",
        "ios-compact-expanded-simulator-matrix",
        "ios-large-font-simulator-proof",
        "ios-cold-hot-relaunch-simulator-proof",
    ] {
        assert!(
            remote_console
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing remote console surface {surface}"
        );
    }
    assert!(remote_console.evidence.iter().any(|evidence| {
        evidence.path == "apps/leserpent-mobile/src/Leserpent.MobileCore/MobileLayoutPolicy.cs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(remote_console.evidence.iter().any(|evidence| {
        evidence.path
            == "apps/leserpent-mobile/src/Leserpent.MobileCore/MobileUiDocumentBinding.cs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(remote_console.evidence.iter().any(|evidence| {
        evidence.path
            == "apps/leserpent-mobile/src/Leserpent.MobileCore/MobileNativeRenderGate.cs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(remote_console.evidence.iter().any(|evidence| {
        evidence.path
            == "apps/leserpent-mobile/src/Leserpent.MobileCore/MobileConnectionProfileStore.cs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(remote_console.evidence.iter().any(|evidence| {
        evidence.path
            == "docs/fixtures/leserpent_android_api36_emulator_macos_arm64_20260821.json"
            && evidence.state == EvidenceState::Present
    }));
    assert!(remote_console.evidence.iter().any(|evidence| {
        evidence.path
            == "apps/leserpent-mobile/src/Leserpent.Mobile.iOS/MobileHubViewController.cs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(remote_console.evidence.iter().any(|evidence| {
        evidence.path == "tests/ios_entry_contract_tdd.rs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(remote_console.evidence.iter().any(|evidence| {
        evidence.path == "tests/leserpent_ios_simulator_tdd.rs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(remote_console.evidence.iter().any(|evidence| {
        evidence.path
            == "docs/fixtures/leserpent_ios26_simulator_macos_arm64_20260821.json"
            && evidence.state == EvidenceState::Present
    }));

    let two_zero_seal = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/release-assurance/two-zero-seal")
        .expect("2.0 release seal must be tracked independently");
    assert_eq!(two_zero_seal.completion, 64);
    assert_eq!(two_zero_seal.contract.version, "0.16.0-draft");
    assert!(
        two_zero_seal
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "closed-core-capability-scope")
    );
    assert!(
        two_zero_seal
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "machine-validated-scope-freeze-manifest")
    );
    assert!(
        two_zero_seal
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "unified-leserpent-release-proof-stage")
    );
    assert!(
        two_zero_seal
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "current-run-schema-scope-artifact-index")
    );
    for surface in [
        "strict-release-readiness-mutation-gate",
        "atomic-apple-release-workflow",
    ] {
        assert!(
            two_zero_seal
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing 2.0 seal surface {surface}"
        );
    }
    assert!(two_zero_seal.evidence.iter().any(|evidence| {
        evidence.path == "project/release/leserpent-2-scope-freeze.json"
            && evidence.state == EvidenceState::Present
    }));
    assert!(two_zero_seal.evidence.iter().any(|evidence| {
        evidence.path == "src/validation_harness/release_gate.rs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(two_zero_seal.blockers.iter().any(|blocker| {
        blocker.id == "prior-gates-open"
            && blocker
                .summary
                .contains("unified current-run parity/schema release stage")
            && blocker.summary.contains("Apple-backed release evidence")
            && !blocker.summary.contains("desktop/remote conformance")
    }));

    let continuous_proof = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/release-assurance/continuous-proof")
        .expect("continuous proof contract must remain tracked");
    assert_eq!(continuous_proof.completion, 97);
    assert_eq!(continuous_proof.contract.version, "0.79.0");
    assert!(
        continuous_proof
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "bounded-validation-subprocess-proof")
    );
    assert!(
        continuous_proof
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "bounded-proof-output-capture")
    );
    for surface in [
        "hub-topology-filter-proof",
        "renderer-neutral-runtime-search-proof",
        "hub-refresh-all-control-proof",
        "topology-refresh-single-flight-proof",
        "shared-topology-refresh-boundary-proof",
        "mobile-topology-refresh-consumption-proof",
        "zero-avalonia-refresh-coordinator-duplication-proof",
        "shared-workspace-launch-boundary-proof",
        "mobile-workspace-launch-consumption-proof",
        "zero-avalonia-workspace-launch-policy-proof",
        "heartbeat-resistant-workspace-launch-proof",
        "shared-mutation-coordinator-boundary-proof",
        "mobile-mutation-coordinator-consumption-proof",
        "zero-avalonia-mutation-state-proof",
        "cached-heartbeat-mutation-negative-proof",
        "malformed-mutation-response-fence-proof",
        "shared-mutation-failure-classification-proof",
        "mobile-mutation-failure-policy-consumption-proof",
        "zero-avalonia-mutation-failure-branching-proof",
        "stale-mutation-completion-suppression-proof",
        "bounded-mutation-failure-diagnostics-proof",
        "shared-authority-health-coordinator-boundary-proof",
        "mobile-authority-health-coordinator-consumption-proof",
        "zero-avalonia-health-state-proof",
        "authority-health-stop-fence-proof",
        "authority-health-failure-classification-proof",
        "shared-event-lifecycle-boundary-proof",
        "event-generation-handle-proof",
        "event-disposal-single-flight-proof",
        "mobile-event-lifecycle-consumption-proof",
        "zero-avalonia-event-lifecycle-ownership-proof",
        "subscriber-failure-isolation-proof",
        "bounded-subscriber-failure-count-proof",
        "native-aot-event-lifecycle-proof",
        "typed-ui-action-routing-proof",
        "opaque-action-node-id-proof",
        "runtime-context-binding-proof",
        "deployment-form-routing-proof",
        "multi-workspace-action-source-proof",
        "retired-workspace-action-source-fence-proof",
        "mobile-ui-action-routing-consumption-proof",
        "native-aot-ui-action-routing-proof",
        "desktop-tutorial-native-control-proof",
        "hub-tutorial-entry-proof",
        "desktop-tutorial-lifecycle-proof",
        "desktop-tutorial-native-aot-proof",
    ] {
        assert!(
            continuous_proof
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing continuous proof surface {surface}"
        );
    }
    for surface in [
        "stale-aot-accessibility-proof-invalidation",
        "failed-aot-fixture-log-retention",
        "bounded-local-validation-helper-subprocesses",
        "contextual-host-permission-failure-guidance",
        "native-developer-workflow",
        "parallel-cross-stack-build",
        "smart-locked-dotnet-restore",
        "portable-package-build-lock",
        "reusable-release-binary-packaging",
        "one-command-macos-aot-bundle-install",
        "atomic-signed-desktop-artifact",
        "strict-apple-release-readiness-gate",
        "atomic-developer-id-notarization-workflow",
        "pending-bundle-release-isolation",
    ] {
        assert!(
            continuous_proof
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing continuous-proof surface {surface}"
        );
    }
    assert!(continuous_proof.evidence.iter().any(|evidence| {
        evidence.path == "src/validation_harness/command.rs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(continuous_proof.evidence.iter().any(|evidence| {
        evidence.path == "src/validation_harness/release_gate.rs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(continuous_proof.evidence.iter().any(|evidence| {
        evidence.path == "crates/gewyvern-dev/src/main.rs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(continuous_proof.evidence.iter().any(|evidence| {
        evidence.path == "src/bin/gewyvern_leserpent_release.rs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(continuous_proof.evidence.iter().any(|evidence| {
        evidence.path == "tests/leserpent_avalonia_lifecycle_tdd.rs"
            && evidence.state == EvidenceState::Present
    }));
    assert!(continuous_proof.evidence.iter().any(|evidence| {
        evidence.path == "docs/machine-contract.md" && evidence.state == EvidenceState::Present
    }));

    let provisioning = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/deployment-bootstrap/gewyvern-provisioning")
        .expect("Gewyvern provisioning must be tracked independently from pipeline deployment");
    assert_eq!(provisioning.maturity, Maturity::Mature);
    assert_eq!(provisioning.completion, 100);
    assert_eq!(provisioning.contract.stability, ContractStability::Stable);
    assert_eq!(provisioning.contract.version, "1.0.1");
    for surface in [
        "bounded-native-service-manager-batch",
        "timed-out-service-manager-child-reaping",
    ] {
        assert!(
            provisioning
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing Gewyvern provisioning surface {surface}"
        );
    }
    assert!(
        provisioning.contract.surfaces.iter().any(|surface| {
            surface == "planned-installing-service-ready-runtime-registered-state"
        })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "raw-provisioning-credential-field-rejection" })
    );
    assert!(
        !provisioning
            .blockers
            .iter()
            .any(|blocker| { blocker.id == "gewyvern-registration-handoff-missing" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "schema-13-shared-authority-checkpoint" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "authenticated-provisioning-https-route" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "daemon-provisioning-identity-fence" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "independent-gewyvern-installer-wire-v1" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "installer-request-ready-response-binding" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "native-gewyvern-install-v1-entrypoint" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "native-gewyvern-activate-v1-entrypoint" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "native-gewyvern-service-v1-entrypoint" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "installer-installed-only-preparation" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "host-key-pinned-gewyvern-ssh" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "ready-before-trust-before-receipt" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "tls-token-health-before-ready" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "activation-rollback-preserves-prior-service" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "daemon-derived-runtime-registration-proof" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "atomic-effect-registration-checkpoint" })
    );
    assert!(
        provisioning
            .contract
            .surfaces
            .iter()
            .any(|surface| { surface == "legacy-service-ready-promotion" })
    );
    for surface in [
        "native-cli-confirmed-runtime-provision",
        "explicit-provisioning-id-replay",
        "authenticated-cli-provisioning-ipc-https",
        "bounded-cli-provisioning-progress",
        "provisioning-terminal-exit-codes",
        "avalonia-confirmed-runtime-provision",
        "authority-scoped-avalonia-provisioning",
        "identity-locked-avalonia-progress",
        "bounded-avalonia-provisioning-poll",
        "explicit-new-attempt-retry-guidance",
        "optional-local-gewyvern-provisioning-origin",
        "independent-runtime-retirement-domain",
        "runtime-retire-capability",
        "service-retired-before-runtime-unregister",
        "retirement-failure-preserves-registration",
        "opaque-ssh-retirement-handle",
        "strict-retirement-wire-v1",
        "retirement-identity-binding",
        "restart-safe-retirement-checkpoint",
        "durable-retirement-authority",
        "atomic-retirement-effect-unregistration",
        "replayable-runtime-unregistration",
        "schema-12-to-13-authority-migration",
        "lost-retirement-lease-rollback",
        "opaque-retirement-secret-resolution",
        "adapter-gated-daemon-retirement-submission",
        "retirement-predispatch-registration-fence",
        "terminal-retirement-outcome-validation",
        "atomic-daemon-retirement-settlement",
        "forged-retirement-receipt-rejection",
        "strict-gewyvern-retirement-wire-v1",
        "native-gewyvern-retire-v1-entrypoint",
        "manifest-bound-target-retirement",
        "two-phase-retirement-recovery-marker",
        "runtime-scoped-retirement-deletion",
        "private-retirement-authority-permissions",
        "host-key-pinned-retirement-ssh",
        "shared-gewyvern-origin-retirement-policy",
        "production-retirement-adapter-registration",
        "explicit-retirement-ipc-route",
        "authenticated-retirement-https-route",
        "bounded-retirement-route-payload",
        "adapter-registration-retirement-route-gate",
        "native-cli-confirmed-runtime-retire",
        "explicit-retirement-id-replay",
        "authenticated-cli-retirement-ipc-https",
        "bounded-cli-retirement-progress",
        "retirement-terminal-exit-codes",
        "avalonia-confirmed-runtime-retire",
        "authority-scoped-avalonia-retirement",
        "provisioning-bound-avalonia-retirement",
        "identity-locked-avalonia-retirement-progress",
        "bounded-avalonia-retirement-poll",
        "credential-free-avalonia-retirement-status",
        "retirement-failure-registration-guidance",
        "physical-linux-native-ssh-retirement",
        "forged-provisioning-retirement-rejection",
        "idempotent-physical-retirement-replay",
        "zero-residue-systemd-user-retirement",
    ] {
        assert!(
            provisioning
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing provisioning client surface {surface}"
        );
    }
    assert!(provisioning.blockers.is_empty());
    assert!(provisioning.evidence.iter().any(|evidence| {
        evidence.path == "docs/fixtures/leserpent_real_ssh_retirement_20260723.json"
            && evidence.state == EvidenceState::Present
    }));
    assert!(
        !provisioning
            .blockers
            .iter()
            .any(|blocker| { blocker.id == "gewyvern-provisioning-client-controls-incomplete" })
    );
    assert!(
        !provisioning
            .blockers
            .iter()
            .any(|blocker| blocker.id == "gewyvern-provisioning-client-controls-missing")
    );

    let ui = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/ui-language/ui-ir-lowering")
        .expect("Leserpent Gate 4 UI language cell must exist");
    assert_eq!(ui.maturity, Maturity::Mature);
    assert_eq!(ui.completion, 100);
    assert_eq!(ui.contract.stability, ContractStability::Stable);
    assert_eq!(ui.contract.version, "1.54.0");
    for surface in [
        "ui-event-hir-effect-lowering",
        "hir-effect-ui-event-reverse-mapping",
        "canonical-event-leselang-export",
        "current-action-semantic-roundtrip",
        "ui-adapter-manifest",
        "complete-presentation-atom-manifest",
        "presentation-atom-profile-manifest",
        "presentation-atom-family-metadata",
        "presentation-atom-effect-metadata",
        "ui-activate-presentation-roundtrip",
        "interaction-presentation-atom-profile",
        "action-target-activation-validation",
        "developer-owned-adapter-kind",
        "generated-framework-binding-kind",
        "cross-language-adapter-manifest-fixture",
        "same-topology-linear-diff",
        "relative-patch-performance-fence",
        "ui-assert-enabled-presentation-roundtrip",
        "ui-assert-disabled-presentation-roundtrip",
        "ui-assert-hidden-presentation-roundtrip",
        "ui-wait-hidden-presentation-roundtrip",
        "fixed-hidden-wait-timeout",
        "ui-assert-text-presentation-roundtrip",
        "ui-wait-text-presentation-roundtrip",
        "fixed-text-wait-timeout",
        "ui-assert-automation-id-presentation-roundtrip",
        "ui-wait-automation-id-presentation-roundtrip",
        "fixed-automation-id-wait-timeout",
        "ui-assert-node-kind-presentation-roundtrip",
        "ui-wait-node-kind-presentation-roundtrip",
        "fixed-node-kind-wait-timeout",
        "ui-assert-action-kind-presentation-roundtrip",
        "ui-wait-action-kind-presentation-roundtrip",
        "fixed-action-kind-wait-timeout",
        "action-kind-wait-target-validation",
        "ui-assert-action-label-presentation-roundtrip",
        "ui-wait-action-label-presentation-roundtrip",
        "fixed-action-label-wait-timeout",
        "action-label-target-validation",
        "ui-assert-action-available-presentation-roundtrip",
        "ui-wait-action-available-presentation-roundtrip",
        "fixed-action-available-wait-timeout",
        "action-available-target-validation",
        "ui-assert-action-unavailable-reason-presentation-roundtrip",
        "ui-wait-action-unavailable-reason-presentation-roundtrip",
        "fixed-action-unavailable-reason-wait-timeout",
        "action-unavailable-reason-target-validation",
        "ui-submit-form-presentation-roundtrip",
        "ui-cancel-form-presentation-roundtrip",
        "form-lifecycle-presentation-atom-profile",
        "parameterized-action-form-lifecycle-validation",
        "frontend-local-form-lifecycle",
        "ui-assert-form-field-presentation-roundtrip",
        "form-field-target-validation",
        "ui-assert-form-field-input-kind-presentation-roundtrip",
        "form-field-input-kind-target-validation",
        "ui-assert-form-field-required-presentation-roundtrip",
        "form-field-required-target-validation",
        "ui-assert-form-field-max-length-presentation-roundtrip",
        "form-field-max-length-target-validation",
        "ui-assert-form-field-placeholder-presentation-roundtrip",
        "form-field-placeholder-target-validation",
        "ui-wait-form-field-presentation-roundtrip",
        "fixed-form-field-wait-timeout",
        "ui-wait-form-field-input-kind-presentation-roundtrip",
        "fixed-form-field-input-kind-wait-timeout",
        "ui-wait-form-field-required-presentation-roundtrip",
        "fixed-form-field-required-wait-timeout",
        "ui-wait-form-field-max-length-presentation-roundtrip",
        "fixed-form-field-max-length-wait-timeout",
        "ui-wait-form-field-placeholder-presentation-roundtrip",
        "fixed-form-field-placeholder-wait-timeout",
        "ui-set-form-value-presentation-roundtrip",
        "ui-assert-form-value-presentation-roundtrip",
        "ui-wait-form-value-presentation-roundtrip",
        "fixed-form-value-wait-timeout",
        "form-value-presentation-atom-profile",
        "form-value-schema-bound-mutation",
        "ui-assert-accessible-name-presentation-roundtrip",
        "ui-wait-accessible-name-presentation-roundtrip",
        "fixed-accessible-name-wait-timeout",
        "ui-assert-accessible-description-presentation-roundtrip",
        "ui-wait-accessible-description-presentation-roundtrip",
        "fixed-accessible-description-wait-timeout",
        "ui-assert-realized-presentation-roundtrip",
        "ui-wait-realized-presentation-roundtrip",
        "fixed-realization-wait-timeout",
        "ui-wait-visible-presentation-roundtrip",
        "fixed-visibility-wait-timeout",
        "ui-wait-enabled-presentation-roundtrip",
        "fixed-enabled-wait-timeout",
        "ui-wait-disabled-presentation-roundtrip",
        "fixed-disabled-wait-timeout",
        "ui-open-window-presentation-roundtrip",
        "ui-close-window-presentation-roundtrip",
        "window-lifecycle-mutation-profile",
        "ui-assert-window-open-presentation-roundtrip",
        "ui-wait-window-open-presentation-roundtrip",
        "fixed-window-open-wait-timeout",
        "ui-assert-window-closed-presentation-roundtrip",
        "ui-wait-window-closed-presentation-roundtrip",
        "fixed-window-closed-wait-timeout",
        "ui-wait-focused-presentation-roundtrip",
        "fixed-focused-wait-timeout",
        "ui-assert-unfocused-presentation-roundtrip",
        "ui-wait-unfocused-presentation-roundtrip",
        "fixed-unfocused-wait-timeout",
        "ui-focus-navigation-presentation-roundtrip",
        "explicit-focus-navigation-direction",
        "focus-navigation-first-last-roundtrip",
        "semantic-selection-state",
        "ui-set-selection-presentation-roundtrip",
        "selection-lifecycle-mutation-profile",
        "ui-assert-selection-presentation-roundtrip",
        "ui-wait-selection-presentation-roundtrip",
        "fixed-selection-wait-timeout",
        "ui-assert-child-count-presentation-roundtrip",
        "ui-wait-child-count-presentation-roundtrip",
        "fixed-child-count-wait-timeout",
        "structural-presentation-atom-profile",
    ] {
        assert!(
            ui.contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing UI performance surface {surface}"
        );
    }
    assert!(ui.blockers.is_empty());
    assert!(ui.next_gate.contains("presentation"));
    assert!(!ui.next_gate.contains("selection"));

    let syntax = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/language-syntax/lossless-frontend")
        .expect("Leserpent language syntax cell must exist");
    assert_eq!(syntax.maturity, Maturity::Mature);
    assert_eq!(syntax.completion, 100);
    assert_eq!(syntax.contract.stability, ContractStability::Stable);
    assert_eq!(syntax.contract.version, "1.0.1");
    for surface in [
        "unescaped-string-fast-path",
        "bounded-language-pipeline-benchmark",
    ] {
        assert!(
            syntax
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing syntax performance surface {surface}"
        );
    }
    assert!(syntax.blockers.is_empty());

    let hir = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/language-hir/typed-effects")
        .expect("Leserpent language HIR cell must exist");
    assert_eq!(hir.contract.version, "0.65.0");
    for surface in [
        "debugger-cancel-effect",
        "ui-activate-effect",
        "ui-focus-effect",
        "ui-focus-navigation-effect",
        "typed-focus-navigation-direction",
        "expanded-focus-navigation-directions",
        "ui-scroll-into-view-effect",
        "ui-assert-visible-effect",
        "ui-assert-hidden-effect",
        "ui-wait-hidden-effect",
        "fixed-hidden-wait-policy",
        "ui-assert-focused-effect",
        "ui-assert-enabled-effect",
        "ui-assert-disabled-effect",
        "ui-assert-text-effect",
        "ui-wait-text-effect",
        "fixed-text-wait-policy",
        "ui-assert-automation-id-effect",
        "ui-wait-automation-id-effect",
        "fixed-automation-id-wait-policy",
        "ui-assert-node-kind-effect",
        "ui-wait-node-kind-effect",
        "fixed-node-kind-wait-policy",
        "ui-assert-action-kind-effect",
        "ui-wait-action-kind-effect",
        "fixed-action-kind-wait-policy",
        "ui-assert-action-label-effect",
        "ui-wait-action-label-effect",
        "fixed-action-label-wait-policy",
        "ui-assert-action-available-effect",
        "ui-wait-action-available-effect",
        "fixed-action-available-wait-policy",
        "ui-assert-action-unavailable-reason-effect",
        "ui-wait-action-unavailable-reason-effect",
        "fixed-action-unavailable-reason-wait-policy",
        "ui-submit-form-effect",
        "ui-cancel-form-effect",
        "distinct-form-lifecycle-mutations",
        "ui-assert-form-field-effect",
        "ui-assert-form-field-input-kind-effect",
        "ui-assert-form-field-required-effect",
        "ui-assert-form-field-max-length-effect",
        "ui-assert-form-field-placeholder-effect",
        "ui-wait-form-field-effect",
        "fixed-form-field-wait-policy",
        "ui-wait-form-field-input-kind-effect",
        "fixed-form-field-input-kind-wait-policy",
        "ui-wait-form-field-required-effect",
        "fixed-form-field-required-wait-policy",
        "ui-wait-form-field-max-length-effect",
        "fixed-form-field-max-length-wait-policy",
        "ui-wait-form-field-placeholder-effect",
        "fixed-form-field-placeholder-wait-policy",
        "ui-set-form-value-effect",
        "ui-assert-form-value-effect",
        "ui-wait-form-value-effect",
        "fixed-form-value-wait-policy",
        "shared-ui-form-value-validation",
        "ui-assert-accessible-name-effect",
        "ui-wait-accessible-name-effect",
        "fixed-accessible-name-wait-policy",
        "ui-assert-accessible-description-effect",
        "ui-wait-accessible-description-effect",
        "fixed-accessible-description-wait-policy",
        "ui-assert-realized-effect",
        "ui-wait-realized-effect",
        "fixed-realization-wait-policy",
        "ui-wait-visible-effect",
        "fixed-visibility-wait-policy",
        "ui-wait-enabled-effect",
        "fixed-enabled-wait-policy",
        "ui-wait-disabled-effect",
        "fixed-disabled-wait-policy",
        "ui-open-window-effect",
        "ui-close-window-effect",
        "ui-assert-window-open-effect",
        "ui-wait-window-open-effect",
        "fixed-window-open-wait-policy",
        "ui-assert-window-closed-effect",
        "ui-wait-window-closed-effect",
        "fixed-window-closed-wait-policy",
        "ui-set-selection-effect",
        "ui-assert-selection-effect",
        "ui-wait-selection-effect",
        "typed-ui-selection-state",
        "fixed-selection-wait-policy",
        "ui-assert-child-count-effect",
        "ui-wait-child-count-effect",
        "bounded-ui-child-count",
        "ui-wait-focused-effect",
        "fixed-focused-wait-policy",
        "ui-assert-unfocused-effect",
        "ui-wait-unfocused-effect",
        "fixed-unfocused-wait-policy",
        "ui-presentation-capability",
        "typed-form-input-kind",
        "typed-form-requirement-state",
        "bounded-form-field-key-validation",
        "bounded-form-max-length-string",
        "optional-form-placeholder-expected",
        "optional-action-unavailable-reason-expected",
        "canonical-effect-roundtrip",
        "single-allocation-name-deduplication",
        "bounded-language-pipeline-benchmark",
    ] {
        assert!(
            hir.contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing HIR performance surface {surface}"
        );
    }

    let vm = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/language-vm/effect-reentry")
        .expect("Leserpent language VM cell must exist");
    assert_eq!(vm.contract.version, "1.50.0");
    for surface in [
        "typed-debugger-cancel-result",
        "restart-safe-debugger-cancel-dispatch",
        "typed-presentation-envelope",
        "typed-ui-activate-result",
        "activate-request-result-binding",
        "typed-ui-focus-result",
        "typed-ui-focus-navigation-result",
        "focus-navigation-start-direction-result-binding",
        "focus-navigation-first-last-result-binding",
        "typed-ui-set-selection-result",
        "selection-mutation-request-result-binding",
        "typed-ui-selection-result",
        "selection-state-request-result-binding",
        "fixed-ui-selection-wait-deadline",
        "typed-ui-assert-child-count-result",
        "typed-ui-wait-child-count-result",
        "child-count-request-result-binding",
        "fixed-ui-child-count-wait-deadline",
        "typed-ui-scroll-into-view-result",
        "typed-ui-assert-visible-result",
        "typed-ui-assert-hidden-result",
        "typed-ui-wait-hidden-result",
        "fixed-ui-hidden-wait-deadline",
        "typed-ui-assert-focused-result",
        "typed-ui-assert-enabled-result",
        "typed-ui-assert-disabled-result",
        "typed-ui-assert-text-result",
        "typed-ui-wait-text-result",
        "text-wait-request-result-binding",
        "fixed-ui-text-wait-deadline",
        "typed-ui-assert-automation-id-result",
        "automation-id-request-result-binding",
        "typed-ui-wait-automation-id-result",
        "automation-id-wait-request-result-binding",
        "fixed-ui-automation-id-wait-deadline",
        "typed-ui-assert-node-kind-result",
        "node-kind-request-result-binding",
        "typed-ui-wait-node-kind-result",
        "fixed-ui-node-kind-wait-deadline",
        "typed-ui-assert-action-kind-result",
        "action-kind-request-result-binding",
        "typed-ui-wait-action-kind-result",
        "fixed-ui-action-kind-wait-deadline",
        "typed-ui-assert-action-label-result",
        "typed-ui-wait-action-label-result",
        "fixed-ui-action-label-wait-deadline",
        "typed-ui-assert-action-available-result",
        "typed-ui-wait-action-available-result",
        "fixed-ui-action-available-wait-deadline",
        "typed-ui-assert-action-unavailable-reason-result",
        "action-unavailable-reason-request-result-binding",
        "typed-ui-wait-action-unavailable-reason-result",
        "action-unavailable-reason-wait-request-result-binding",
        "fixed-ui-action-unavailable-reason-wait-deadline",
        "typed-ui-submit-form-result",
        "form-submit-request-result-binding",
        "typed-ui-cancel-form-result",
        "form-cancel-request-result-binding",
        "distinct-form-lifecycle-mutation-kind",
        "typed-ui-assert-form-field-result",
        "form-field-request-result-binding",
        "typed-ui-assert-form-field-input-kind-result",
        "form-field-input-kind-request-result-binding",
        "typed-ui-assert-form-field-required-result",
        "form-field-required-request-result-binding",
        "typed-ui-assert-form-field-max-length-result",
        "form-field-max-length-request-result-binding",
        "typed-ui-assert-form-field-placeholder-result",
        "form-field-placeholder-request-result-binding",
        "typed-ui-wait-form-field-result",
        "fixed-ui-form-field-wait-deadline",
        "typed-ui-wait-form-field-input-kind-result",
        "fixed-ui-form-field-input-kind-wait-deadline",
        "typed-ui-wait-form-field-required-result",
        "fixed-ui-form-field-required-wait-deadline",
        "typed-ui-wait-form-field-max-length-result",
        "fixed-ui-form-field-max-length-wait-deadline",
        "typed-ui-wait-form-field-placeholder-result",
        "form-field-placeholder-wait-request-result-binding",
        "fixed-ui-form-field-placeholder-wait-deadline",
        "typed-ui-set-form-value-result",
        "form-value-mutation-request-result-binding",
        "typed-ui-assert-form-value-result",
        "typed-ui-wait-form-value-result",
        "form-value-observation-request-result-binding",
        "fixed-ui-form-value-wait-deadline",
        "typed-ui-assert-accessible-name-result",
        "typed-ui-wait-accessible-name-result",
        "accessible-name-wait-request-result-binding",
        "fixed-ui-accessible-name-wait-deadline",
        "typed-ui-assert-accessible-description-result",
        "typed-ui-wait-accessible-description-result",
        "accessible-description-wait-request-result-binding",
        "fixed-ui-accessible-description-wait-deadline",
        "typed-ui-assert-realized-result",
        "typed-ui-wait-realized-result",
        "fixed-ui-realization-wait-deadline",
        "typed-ui-wait-visible-result",
        "fixed-ui-visibility-wait-deadline",
        "typed-ui-wait-enabled-result",
        "fixed-ui-enabled-wait-deadline",
        "typed-ui-wait-disabled-result",
        "fixed-ui-disabled-wait-deadline",
        "typed-ui-open-window-result",
        "window-open-request-result-binding",
        "typed-ui-close-window-result",
        "window-close-request-result-binding",
        "typed-ui-assert-window-open-result",
        "typed-ui-wait-window-open-result",
        "fixed-ui-window-open-wait-deadline",
        "typed-ui-assert-window-closed-result",
        "typed-ui-wait-window-closed-result",
        "fixed-ui-window-closed-wait-deadline",
        "typed-ui-wait-focused-result",
        "fixed-ui-focused-wait-deadline",
        "typed-ui-assert-unfocused-result",
        "typed-ui-wait-unfocused-result",
        "fixed-ui-unfocused-wait-deadline",
        "presentation-operation-identity-binding",
        "allocation-free-continuation-size-validation",
        "bounded-language-pipeline-benchmark",
    ] {
        assert!(
            vm.contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing VM performance surface {surface}"
        );
    }
    for cell in [syntax, hir, vm] {
        assert!(cell.evidence.iter().any(|evidence| {
            evidence.path == "crates/leselang-vm/examples/language_benchmark.rs"
                && evidence.state == EvidenceState::Present
        }));
    }

    let command = catalog
        .cells
        .iter()
        .find(|cell| cell.id == "leserpent-2/command-lowering/command-plan-lowering")
        .expect("Leserpent command lowering cell must exist");
    assert_eq!(command.contract.version, "0.57.0");
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "action-activation-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "multi-presentation-effect-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "realization-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "visibility-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "enabled-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "focused-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "unfocused-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "unfocused-assert-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "focus-navigation-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "form-field-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "form-field-input-kind-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "form-field-required-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "form-field-max-length-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "form-field-placeholder-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "form-field-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "form-field-input-kind-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "form-field-required-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "form-field-max-length-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "form-field-placeholder-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "selection-command-rejection")
    );
    for surface in [
        "child-count-assert-command-rejection",
        "child-count-wait-command-rejection",
    ] {
        assert!(
            command
                .contract
                .surfaces
                .iter()
                .any(|candidate| candidate == surface),
            "missing child-count command fence {surface}"
        );
    }
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "automation-id-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "node-kind-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "node-kind-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "action-kind-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "action-kind-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "action-label-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "action-label-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "action-available-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "action-available-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "action-unavailable-reason-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "text-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "accessible-name-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "accessible-description-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "action-unavailable-reason-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "disabled-assert-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "hidden-assert-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "hidden-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "disabled-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "window-lifecycle-open-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "window-lifecycle-close-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "window-open-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "window-open-wait-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "window-closed-command-rejection")
    );
    assert!(
        command
            .contract
            .surfaces
            .iter()
            .any(|surface| surface == "window-closed-wait-command-rejection")
    );

    let required_boundaries = [
        "boundary-leselang-syntax",
        "boundary-leselang-hir",
        "boundary-leselang-vm",
        "boundary-leselang-command",
        "boundary-leselang-ui",
        "boundary-leserpent-domain",
        "boundary-leserpent-runtime",
        "boundary-leserpent-protocol",
        "boundary-leserpent-adapters",
        "boundary-leserpent-cli",
        "boundary-leserpentd",
        "boundary-leserpent-bootstrap",
        "boundary-gewyvern-runtime-evidence",
        "boundary-gewyvern-linux-ebpf",
        "boundary-gewylang-compiler",
        "boundary-gewylang-protocol-packages",
        "boundary-leserpent-1x-control-plane",
        "boundary-leserpent-1x-web-console",
        "boundary-etragon-learning-sidecar",
        "boundary-project-status-governance",
    ];
    for requirement in required_boundaries {
        assert!(
            catalog
                .coverage_requirements
                .iter()
                .any(|item| item.id == requirement),
            "missing architecture boundary {requirement}"
        );
    }
    for gate in 1..=7 {
        let gate_name = ["one", "two", "three", "four", "five", "six", "seven"][gate - 1];
        assert!(
            catalog
                .coverage_requirements
                .iter()
                .any(|item| item.id.starts_with(&format!("gate-{gate_name}-"))),
            "missing roadmap gate {gate}"
        );
    }

    for architecture in catalog
        .dimensions
        .architectures
        .iter()
        .map(|entry| entry.id.as_str())
    {
        for cell in catalog
            .cells
            .iter()
            .filter(|cell| cell.architecture == architecture)
        {
            assert!(
                catalog.coverage_requirements.iter().any(|requirement| {
                    requirement.architecture == architecture && requirement.cells.contains(&cell.id)
                }),
                "cell '{}' is missing authoritative coverage",
                cell.id
            );
        }
    }
}

#[test]
fn coverage_manifest_rejects_unknown_mappings_and_orphan_cells() {
    let mut catalog = StatusCatalog::load(default_catalog_path()).expect("catalog must decode");
    let proof = catalog
        .coverage_requirements
        .iter_mut()
        .find(|item| item.id == "proof-continuous-shelves")
        .expect("continuous proof requirement must exist");
    proof.cells = vec!["leserpent-2/release-assurance/missing".into()];

    let errors = catalog
        .validate(repository_root())
        .expect_err("invalid coverage mapping must be rejected")
        .join("\n");
    assert!(errors.contains("references unknown cell"));
    assert!(errors.contains("continuous-proof' is missing"));
}

#[test]
fn expanded_coverage_manifest_rejects_gewyvern_core_orphans() {
    let mut catalog = StatusCatalog::load(default_catalog_path()).expect("catalog must decode");
    catalog
        .coverage_requirements
        .retain(|item| item.id != "boundary-gewyvern-linux-ebpf");

    let errors = catalog
        .validate(repository_root())
        .expect_err("unmapped Gewyvern Core cells must be rejected")
        .join("\n");
    assert!(errors.contains(
        "cell 'gewyvern-core/linux-ebpf/linux-attach' is missing from the 'gewyvern-core' coverage manifest"
    ));
}

#[test]
fn coverage_manifest_is_required_for_every_architecture_with_cells() {
    let mut catalog = StatusCatalog::load(default_catalog_path()).expect("catalog must decode");
    catalog
        .coverage_requirements
        .retain(|item| item.architecture != "etragon");

    let errors = catalog
        .validate(repository_root())
        .expect_err("architectures without coverage requirements must be rejected")
        .join("\n");
    assert!(errors.contains("architecture 'etragon' has cells but no coverage requirements"));
}

#[test]
fn native_status_cli_exposes_human_and_machine_views() {
    let binary = env!("CARGO_BIN_EXE_gewyvern_status");
    let validate = Command::new(binary)
        .arg("validate")
        .output()
        .expect("status validate must run");
    assert!(validate.status.success());
    assert!(String::from_utf8_lossy(&validate.stdout).contains("status catalog valid"));

    let summary = Command::new(binary)
        .args(["summary", "--json", "--limit", "3"])
        .output()
        .expect("status summary must run");
    assert!(summary.status.success());
    let payload: serde_json::Value =
        serde_json::from_slice(&summary.stdout).expect("summary must be JSON");
    assert_eq!(payload["schema_version"], STATUS_SCHEMA_VERSION);
    assert_eq!(payload["coverage"]["requirement_count"], 29);
    assert_eq!(payload["coverage"]["architecture_count"], 6);
    assert_eq!(payload["coverage"]["ownership_boundary_count"], 21);
    assert_eq!(payload["coverage"]["roadmap_gate_count"], 7);
    assert_eq!(payload["coverage"]["proof_shelf_count"], 1);
    assert_eq!(payload["weakest"].as_array().unwrap().len(), 3);
    assert!(payload["lifecycles"].as_array().unwrap().len() >= 3);
    assert!(payload["architectures"].as_array().unwrap().len() >= 6);
    assert!(payload["modules"].as_array().unwrap().len() >= 12);

    let developing = Command::new(binary)
        .args(["developing", "--architecture", "leserpent-2", "--json"])
        .output()
        .expect("status developing query must run");
    assert!(developing.status.success());
    let payload: serde_json::Value =
        serde_json::from_slice(&developing.stdout).expect("developing view must be JSON");
    assert!(
        payload
            .as_array()
            .unwrap()
            .iter()
            .all(|cell| cell["architecture"] == "leserpent-2")
    );

    let help = Command::new(binary)
        .arg("--help")
        .output()
        .expect("status help must run");
    assert!(help.status.success());
    assert!(String::from_utf8_lossy(&help.stdout).contains("usage: gewyvern_status"));

    let tensor_slice = Command::new(binary)
        .args([
            "weakest",
            "--architecture",
            "leserpent-2",
            "--module",
            "language-vm",
            "--feature",
            "effect-reentry",
            "--lifecycle",
            "target",
            "--maturity",
            "mature",
            "--json",
        ])
        .output()
        .expect("three-dimensional status slice must run");
    assert!(tensor_slice.status.success());
    let payload: serde_json::Value =
        serde_json::from_slice(&tensor_slice.stdout).expect("tensor slice must be JSON");
    assert_eq!(payload.as_array().unwrap().len(), 1);
    assert_eq!(payload[0]["feature"], "effect-reentry");
}
