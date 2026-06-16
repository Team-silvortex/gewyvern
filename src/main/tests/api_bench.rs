use super::*;

#[test]
fn api_snapshot_meta_and_routes_cover_single_export() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
        .expect("http_request_path DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "dsl_demo".into(),
            primary_module_family: analysis_snapshot(&export).primary_module_family,
            evidence_posture: analysis_snapshot(&export).evidence_posture,
            automation_outcome: analysis_snapshot(&export).automation_outcome,
            summary_text: summary_line("dsl_demo", &export),
            summary_json: summary_json("dsl_demo", &export),
            findings_json: findings_json("dsl_demo", &export),
            analysis_json: analysis_snapshot_json(&analysis_snapshot(&export)),
            training_example_json: training_example_json("dsl_demo", &export),
            has_external_sidecar_context: false,
            has_external_evidence_chain_enrichment: false,
            has_external_diagnostic_opinion: false,
            has_external_capability_profile: false,
            external_capability_status: None,
            external_hint_status: None,
            external_context_status: None,
            external_sidecar_trust_level: None,
            external_sidecar_consumption_mode: None,
            export_json: export.to_json(),
            report_json: scan_report_json(&[("dsl_demo".to_string(), export.clone())]),
            report_html: scan_report_html(&[("dsl_demo".to_string(), export.clone())]),
        },
    );
    let snapshot = state.lock().unwrap().clone();

    let meta = api_snapshot_meta_json(&snapshot);
    assert!(meta.contains("\"kind\":\"single\""));
    assert!(meta.contains("\"name\":\"dsl_demo\""));
    assert!(meta.contains("\"target_names\":[\"dsl_demo\"]"));
    assert!(meta.contains("\"has_analysis_json\":true"));
    assert!(meta.contains("\"has_training_example_json\":true"));
    assert!(meta.contains("\"has_export_json\":true"));
    assert!(meta.contains("\"has_external_sidecar_context\":false"));
    assert!(meta.contains("\"has_external_evidence_chain_enrichment\":false"));
    assert!(meta.contains("\"has_external_diagnostic_opinion\":false"));
    assert!(meta.contains("\"external_sidecar_trust_level\":null"));
    assert!(meta.contains("\"external_context_status\":null"));
    assert!(meta.contains("\"external_sidecar_consumption_mode\":null"));

    let (_, _, targets_body) = api_response_for_request("/v1/latest/targets", &snapshot);
    assert!(targets_body.contains("\"targets\":[\"dsl_demo\"]"));
    assert!(targets_body.contains("\"has_external_sidecar_context\":false"));
    assert!(targets_body.contains("\"has_external_evidence_chain_enrichment\":false"));
    assert!(targets_body.contains("\"has_external_diagnostic_opinion\":false"));
    assert!(targets_body.contains("\"external_sidecar_trust_level\":null"));
    assert!(targets_body.contains("\"external_context_status\":null"));
    assert!(targets_body.contains("\"external_sidecar_consumption_mode\":null"));
    assert!(targets_body.contains("\"has_protocol_surface\":false"));

    let (_, _, summary_body) = api_response_for_request("/v1/latest/summary.json", &snapshot);
    assert!(summary_body.contains("\"demo\":\"dsl_demo\""));
    let (_, _, analysis_body) = api_response_for_request("/v1/latest/analysis.json", &snapshot);
    assert!(analysis_body.contains("\"primary_module_kind\""));
    assert!(analysis_body.contains("\"protocol_flows\""));
    assert!(analysis_body.contains("\"augmentations\":["));
    assert!(analysis_body.contains("\"name\":\"automation_recommendation\""));
    let (_, _, training_body) =
        api_response_for_request("/v1/latest/training-example.json", &snapshot);
    assert!(training_body.contains("\"kind\":\"training_example\""));
    let (_, _, dataset_body) =
        api_response_for_request("/v1/latest/training-dataset.json", &snapshot);
    assert!(dataset_body.contains("\"kind\":\"training_dataset_manifest\""));

    let (_, _, export_body) = api_response_for_request("/v1/latest/export.json", &snapshot);
    assert!(export_body.contains("\"template_id\""));

    let (_, _, target_summary_body) =
        api_response_for_request("/v1/latest/targets/dsl_demo/summary.json", &snapshot);
    assert!(target_summary_body.contains("\"demo\":\"dsl_demo\""));
    let (_, _, target_analysis_body) =
        api_response_for_request("/v1/latest/targets/dsl_demo/analysis.json", &snapshot);
    assert!(target_analysis_body.contains("\"primary_failure_mode\""));
    let (surface_status, _, surface_body) = api_response_for_request(
        "/v1/latest/targets/dsl_demo/protocol-surface.json",
        &snapshot,
    );
    assert_eq!(surface_status, 404);
    assert!(surface_body.contains("no protocol surface available"));
}

#[test]
fn api_snapshot_routes_cover_scan_export() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
        .expect("http_request_path DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let outputs = vec![("scan:http:request".to_string(), export)];
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    let rendered_targets = outputs
        .iter()
        .map(|(name, export)| {
            let analysis = analysis_snapshot(export);
            let (
                has_external_sidecar_context,
                has_external_evidence_chain_enrichment,
                has_external_diagnostic_opinion,
            ) = crate::diagnosis_runtime::external_sidecar_presence(&analysis);
            ApiRenderedTarget {
                name: name.clone(),
                primary_module_family: analysis.primary_module_family.clone(),
                evidence_posture: analysis.evidence_posture.clone(),
                automation_outcome: analysis.automation_outcome.clone(),
                summary_text: summary_line(name, export),
                summary_json: summary_json(name, export),
                findings_json: findings_json(name, export),
                analysis_json: analysis_snapshot_json(&analysis),
                training_example_json: training_example_json(name, export),
                has_external_sidecar_context,
                has_external_evidence_chain_enrichment,
                has_external_diagnostic_opinion,
                has_external_capability_profile: false,
                external_capability_status: None,
                external_hint_status: None,
                external_context_status: None,
                external_sidecar_trust_level: None,
                external_sidecar_consumption_mode: None,
                export_json: export.to_json(),
                report_json: scan_report_json(&[(name.clone(), export.clone())]),
                report_html: scan_report_html(&[(name.clone(), export.clone())]),
            }
        })
        .collect::<Vec<_>>();
    update_api_snapshot_for_scan(
        &state,
        rendered_targets,
        scan_report_text(&outputs),
        scan_report_json(&outputs),
        format!(
            "[{}]",
            outputs
                .iter()
                .map(|(name, export)| format!(
                    "{{\"target\":\"{}\",\"analysis\":{}}}",
                    name.replace('\\', "\\\\").replace('"', "\\\""),
                    analysis_snapshot_json(&analysis_snapshot(export)),
                ))
                .collect::<Vec<_>>()
                .join(",")
        ),
        training_example_json_array(
            &outputs,
            &outputs
                .iter()
                .map(|(_, export)| analysis_snapshot(export))
                .collect::<Vec<_>>(),
        ),
        scan_report_json(&outputs),
        scan_report_html(&outputs),
    );
    let snapshot = state.lock().unwrap().clone();

    let (health_status, _, health_body) = api_response_for_request("/health", &snapshot);
    assert_eq!(health_status, 200);
    assert!(health_body.contains("\"has_snapshot\":true"));

    let (cap_status, _, cap_body) = api_response_for_request("/v1/capabilities", &snapshot);
    assert_eq!(cap_status, 200);
    assert!(cap_body.contains("\"service\":\"gewyvern-api\""));
    assert!(cap_body.contains(&format!("\"version\":\"{}\"", env!("CARGO_PKG_VERSION"))));
    assert!(cap_body.contains("\"external_sidecar_context\":true"));
    assert!(cap_body.contains("\"training_dataset_manifest\":true"));
    assert!(cap_body.contains("\"external_capability_profile\":true"));
    assert!(cap_body.contains("\"external_sidecar_trust_level\":true"));
    assert!(cap_body.contains("\"external_sidecar_consumption_mode\":true"));

    let (targets_status, _, targets_body) =
        api_response_for_request("/v1/latest/targets", &snapshot);
    assert_eq!(targets_status, 200);
    assert!(targets_body.contains("\"targets\":[\"scan:http:request\"]"));
    assert!(targets_body.contains("\"has_protocol_surface\":true"));
    assert!(targets_body.contains("\"protocol\":\"http\""));
    assert!(targets_body.contains("\"entry\":\"request\""));
    assert!(targets_body.contains("\"default_entry\":\"request\""));
    assert!(targets_body.contains("\"selected_is_default\":true"));
    let (analysis_status, _, analysis_body) =
        api_response_for_request("/v1/latest/analysis.json", &snapshot);
    assert_eq!(analysis_status, 200);
    assert!(analysis_body.contains("\"target\":\"scan:http:request\""));
    assert!(analysis_body.contains("\"augmentations\":["));
    assert!(analysis_body.contains("\"name\":\"automation_recommendation\""));
    let (training_status, _, training_body) =
        api_response_for_request("/v1/latest/training-example.json", &snapshot);
    assert_eq!(training_status, 200);
    assert!(training_body.contains("\"kind\":\"training_example\""));
    let (dataset_status, _, dataset_body) =
        api_response_for_request("/v1/latest/training-dataset.json", &snapshot);
    assert_eq!(dataset_status, 200);
    assert!(dataset_body.contains("\"kind\":\"training_dataset_manifest\""));

    let (report_status, _, report_body) =
        api_response_for_request("/v1/latest/report.json", &snapshot);
    assert_eq!(report_status, 200);
    assert!(report_body.contains("\"scan_all\":true"));

    let (target_status, _, target_body) = api_response_for_request(
        "/v1/latest/targets/scan:http:request/report.json",
        &snapshot,
    );
    assert_eq!(target_status, 200);
    assert!(target_body.contains("\"target\":\"scan:http:request\""));
    let (target_analysis_status, _, target_analysis_body) = api_response_for_request(
        "/v1/latest/targets/scan:http:request/analysis.json",
        &snapshot,
    );
    assert_eq!(target_analysis_status, 200);
    assert!(target_analysis_body.contains("\"primary_module_kind\""));
    let (surface_status, _, surface_body) = api_response_for_request(
        "/v1/latest/targets/scan:http:request/protocol-surface.json",
        &snapshot,
    );
    assert_eq!(surface_status, 200);
    assert!(surface_body.contains("\"protocol\":\"http\""));
    assert!(surface_body.contains("\"entry\":\"request\""));
    assert!(surface_body.contains("\"default_entry\":\"request\""));
    assert!(surface_body.contains("\"selected_is_default\":true"));

    let (findings_status, _, _) = api_response_for_request("/v1/latest/findings.json", &snapshot);
    assert_eq!(findings_status, 404);
}

#[test]
fn api_target_list_exposes_url_safe_path_segments() {
    let _guard = test_guard();
    set_external_analysis_config(None);
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
        .expect("http_request_path DSL should compile");
    let export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let state = Arc::new(Mutex::new(Arc::new(ApiSnapshot::default())));
    update_api_snapshot_for_single(
        &state,
        ApiRenderedTarget {
            name: "scan:http request/%".into(),
            primary_module_family: analysis_snapshot(&export).primary_module_family,
            evidence_posture: analysis_snapshot(&export).evidence_posture,
            automation_outcome: analysis_snapshot(&export).automation_outcome,
            summary_text: summary_line("scan:http request/%", &export),
            summary_json: summary_json("scan:http request/%", &export),
            findings_json: findings_json("scan:http request/%", &export),
            analysis_json: analysis_snapshot_json(&analysis_snapshot(&export)),
            training_example_json: training_example_json("scan:http request/%", &export),
            has_external_sidecar_context: false,
            has_external_evidence_chain_enrichment: false,
            has_external_diagnostic_opinion: false,
            has_external_capability_profile: false,
            external_capability_status: None,
            external_hint_status: None,
            external_context_status: None,
            external_sidecar_trust_level: None,
            external_sidecar_consumption_mode: None,
            export_json: export.to_json(),
            report_json: scan_report_json(&[("scan:http request/%".to_string(), export.clone())]),
            report_html: scan_report_html(&[("scan:http request/%".to_string(), export.clone())]),
        },
    );
    let snapshot = state.lock().unwrap().clone();

    let (_, _, meta_body) = api_response_for_request("/v1/latest/meta", &snapshot);
    assert!(meta_body.contains("\"path_segment\":\"scan:http%20request%2F%25\""));

    let (_, _, targets_body) = api_response_for_request("/v1/latest/targets", &snapshot);
    assert!(targets_body.contains("\"path_segment_encoding\":\"percent-encoding\""));
    assert!(targets_body.contains("\"url_path\":\"/v1/latest/targets/scan:http%20request%2F%25\""));
    assert!(targets_body.contains("\"has_external_sidecar_context\":false"));
    assert!(targets_body.contains("\"has_protocol_surface\":false"));

    let (target_status, _, target_body) = api_response_for_request(
        "/v1/latest/targets/scan:http%20request%2F%25/summary.json",
        &snapshot,
    );
    assert_eq!(target_status, 200);
    assert!(target_body.contains("\"demo\":\"scan:http request/%\""));
    let (analysis_status, _, analysis_body) = api_response_for_request(
        "/v1/latest/targets/scan:http%20request%2F%25/analysis.json",
        &snapshot,
    );
    assert_eq!(analysis_status, 200);
    assert!(analysis_body.contains("\"primary_module_kind\""));
}

#[test]
fn api_rejects_invalid_target_path_percent_encoding() {
    let snapshot = ApiSnapshot::default();
    let (status, _, body) =
        api_response_for_request("/v1/latest/targets/bad%2/report.json", &snapshot);
    assert_eq!(status, 400);
    assert!(body.contains("\"error\":\"invalid_target_path_segment\""));
}

#[test]
fn api_rejects_oversized_report_bodies() {
    let mut snapshot = ApiSnapshot::default();
    snapshot.report_json = Some("x".repeat((512 * 1024) + 32));

    let (status, content_type, body) =
        api_response_for_request("/v1/latest/report.json", &snapshot);
    assert_eq!(status, 503);
    assert_eq!(content_type, "application/json; charset=utf-8");
    assert!(body.contains("\"error\":\"response_too_large\""));
    assert!(body.contains("\"path\":\"/v1/latest/report.json\""));
}

#[test]
#[ignore = "benchmark"]
fn benchmark_summary_json_large_protocol_flow_export() {
    let export = synthesize_large_protocol_flow_export();
    let start = Instant::now();
    let mut total_len = 0usize;
    for _ in 0..200 {
        total_len += summary_json("bench", &export).len();
    }
    let elapsed = start.elapsed();
    assert!(total_len > 0);
    eprintln!(
        "benchmark_summary_json_large_protocol_flow_export: iterations=200 flows={} findings={} elapsed_ms={:.3}",
        export.program_flows.len(),
        export.program_findings.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_summary_line_large_protocol_flow_export() {
    let export = synthesize_large_protocol_flow_export();
    let start = Instant::now();
    let mut total_len = 0usize;
    for _ in 0..200 {
        total_len += summary_line("bench", &export).len();
    }
    let elapsed = start.elapsed();
    assert!(total_len > 0);
    eprintln!(
        "benchmark_summary_line_large_protocol_flow_export: iterations=200 flows={} findings={} elapsed_ms={:.3}",
        export.program_flows.len(),
        export.program_findings.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_analysis_snapshot_large_protocol_flow_export() {
    let export = synthesize_large_protocol_flow_export();
    let start = Instant::now();
    let mut total_flows = 0usize;
    for _ in 0..200 {
        total_flows += analysis_snapshot(&export).protocol_flows.len();
    }
    let elapsed = start.elapsed();
    assert!(total_flows > 0);
    eprintln!(
        "benchmark_analysis_snapshot_large_protocol_flow_export: iterations=200 flows={} findings={} elapsed_ms={:.3}",
        export.program_flows.len(),
        export.program_findings.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_analysis_snapshot_json_large_protocol_flow_export() {
    let export = synthesize_large_protocol_flow_export();
    let snapshot = analysis_snapshot(&export);
    let start = Instant::now();
    let mut total_len = 0usize;
    for _ in 0..200 {
        total_len += analysis_snapshot_json(&snapshot).len();
    }
    let elapsed = start.elapsed();
    assert!(total_len > 0);
    eprintln!(
        "benchmark_analysis_snapshot_json_large_protocol_flow_export: iterations=200 flows={} findings={} elapsed_ms={:.3}",
        export.program_flows.len(),
        export.program_findings.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_findings_json_large_protocol_flow_export() {
    let export = synthesize_large_protocol_flow_export();
    let analysis = analysis_snapshot(&export);
    let start = Instant::now();
    let mut total_len = 0usize;
    for _ in 0..200 {
        total_len += findings_json_with_analysis("bench", &export, &analysis).len();
    }
    let elapsed = start.elapsed();
    assert!(total_len > 0);
    eprintln!(
        "benchmark_findings_json_large_protocol_flow_export: iterations=200 flows={} findings={} elapsed_ms={:.3}",
        export.program_flows.len(),
        export.program_findings.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_http_transactions_json_large_view() {
    let transactions = synthesize_large_http_transactions();
    let start = Instant::now();
    let mut total_len = 0usize;
    for _ in 0..200 {
        total_len += http_transactions_json(&transactions).len();
    }
    let elapsed = start.elapsed();
    assert!(total_len > 0);
    eprintln!(
        "benchmark_http_transactions_json_large_view: iterations=200 transactions={} elapsed_ms={:.3}",
        transactions.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_http_transactions_text_large_view() {
    let transactions = synthesize_large_http_transactions();
    let start = Instant::now();
    let mut total_len = 0usize;
    for _ in 0..200 {
        total_len += http_transactions_text(&transactions).len();
    }
    let elapsed = start.elapsed();
    assert!(total_len > 0);
    eprintln!(
        "benchmark_http_transactions_text_large_view: iterations=200 transactions={} elapsed_ms={:.3}",
        transactions.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_scan_report_json_large_protocol_flow_export() {
    let outputs = synthesize_large_scan_outputs(24);
    let start = Instant::now();
    let mut total_len = 0usize;
    for _ in 0..40 {
        total_len += scan_report_json(&outputs).len();
    }
    let elapsed = start.elapsed();
    assert!(total_len > 0);
    eprintln!(
        "benchmark_scan_report_json_large_protocol_flow_export: iterations=40 targets={} flows_per_target={} elapsed_ms={:.3}",
        outputs.len(),
        outputs[0].1.program_flows.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_scan_report_text_large_protocol_flow_export() {
    let outputs = synthesize_large_scan_outputs(24);
    let start = Instant::now();
    let mut total_len = 0usize;
    for _ in 0..40 {
        total_len += scan_report_text(&outputs).len();
    }
    let elapsed = start.elapsed();
    assert!(total_len > 0);
    eprintln!(
        "benchmark_scan_report_text_large_protocol_flow_export: iterations=40 targets={} flows_per_target={} elapsed_ms={:.3}",
        outputs.len(),
        outputs[0].1.program_flows.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_scan_report_html_large_protocol_flow_export() {
    let outputs = synthesize_large_scan_outputs(12);
    let start = Instant::now();
    let mut total_len = 0usize;
    for _ in 0..10 {
        total_len += scan_report_html(&outputs).len();
    }
    let elapsed = start.elapsed();
    assert!(total_len > 0);
    eprintln!(
        "benchmark_scan_report_html_large_protocol_flow_export: iterations=10 targets={} flows_per_target={} elapsed_ms={:.3}",
        outputs.len(),
        outputs[0].1.program_flows.len(),
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
fn pid_filter_keeps_only_target_process_view() {
    let target = synthetic_process_view(9101, "curl");
    let other = synthetic_process_view(9102, "dig");

    let target_export = coerce_export_process(
        annotate_export_trust(
            run_binding_demo(
                compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
                    .expect("http_request_path DSL should compile"),
            ),
            &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
        ),
        &target,
    );
    let other_export = coerce_export_process(
        annotate_export_trust(
            run_binding_demo(
                compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/dns_udp_process.gewy")
                    .expect("dns_udp_process DSL should compile"),
            ),
            &Cli::from_args(["--demo".to_string(), "udp".to_string()]).unwrap(),
        ),
        &other,
    );

    let combined = merge_exports_for_tests(vec![target_export, other_export]);
    let filtered = filter_export_by_pid(&combined, target.pid);

    assert!(
        !combined.program_flows.is_empty()
            && combined.program_flows.len() > filtered.program_flows.len(),
        "pid filtering should remove at least one non-target program flow"
    );
    assert!(
        !filtered.program_flows.is_empty(),
        "pid filtering should keep target program flows"
    );
    assert!(
        filtered
            .flows
            .iter()
            .all(|flow| process_matches_pid(flow.process.as_ref(), target.pid)),
        "all filtered flows should point at the requested pid"
    );
    assert!(
        filtered
            .program_flows
            .iter()
            .all(|flow| process_matches_pid(flow.process.as_ref(), target.pid)),
        "all filtered program flows should point at the requested pid"
    );
    assert!(
        filtered
            .program_findings
            .iter()
            .all(|finding| process_matches_pid(finding.process.as_ref(), target.pid)),
        "all filtered program findings should point at the requested pid"
    );
    assert!(
        filtered
            .module_findings
            .iter()
            .all(|finding| process_matches_pid(finding.process.as_ref(), target.pid)),
        "all filtered module findings should point at the requested pid"
    );
    assert!(
        filtered
            .reasons
            .iter()
            .all(|reason| filtered.flows.iter().any(|flow| {
                flow.id == reason.flow && process_matches_pid(flow.process.as_ref(), target.pid)
            })),
        "all filtered reasons should still be anchored to target-owned flows"
    );
    assert!(
        filtered.facts.len() < combined.facts.len(),
        "pid filtering should narrow the fact set"
    );
}
