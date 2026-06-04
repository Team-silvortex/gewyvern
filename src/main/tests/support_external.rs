use super::*;

pub(super) fn with_fake_etragon_hook<T>(output_json: &str, test: impl FnOnce() -> T) -> T {
    struct FakeEtragonHookGuard {
        script_path: std::path::PathBuf,
    }

    impl Drop for FakeEtragonHookGuard {
        fn drop(&mut self) {
            set_external_analysis_config(None);
            let _ = fs::remove_file(&self.script_path);
        }
    }

    let _guard = test_guard();
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let script_path = std::env::temp_dir().join(format!("fake-etragon-{unique}.sh"));
    fs::write(
        &script_path,
        format!(
            "#!/bin/sh\ncat >/dev/null\nprintf '%s\\n' '{}'\n",
            output_json
        ),
    )
    .expect("fake etragon hook should be writable");
    let mut permissions = fs::metadata(&script_path)
        .expect("fake etragon hook should exist")
        .permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&script_path, permissions).expect("fake etragon hook should be executable");
    set_external_analysis_config(Some(ExternalAnalysisConfig {
        engine_bin: script_path.to_string_lossy().into_owned(),
        python_worker: None,
        python_bin: None,
    }));
    let cleanup = FakeEtragonHookGuard {
        script_path: script_path.clone(),
    };
    let outcome = test();
    drop(cleanup);
    outcome
}

pub(super) fn synthesize_large_protocol_flow_export() -> gewyvern::export::ExportBundle {
    let binding = compile_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
        .expect("http_request_path DSL should compile");
    let mut export = annotate_export_trust(
        run_binding_demo(binding),
        &Cli::from_args(["--demo".to_string(), "tcp".to_string()]).unwrap(),
    );
    let base_flow = export.program_flows[0].clone();
    let base_process = base_flow.process.clone();
    let flow_count = 256u64;

    export.program_flows = (0..flow_count)
        .map(|offset| {
            let mut flow = base_flow.clone();
            flow.id = gewyvern::flow::ProgramFlowId(offset + 1);
            flow.process = base_process.clone();
            flow
        })
        .collect();
    export.program_findings = export
        .program_flows
        .iter()
        .map(|flow| ProgramFinding {
            program_flow: flow.id,
            process: flow.process.clone(),
            operation: flow.operation.clone(),
            module_label: "http_request_path".into(),
            network_module_kind: "http_request_response".into(),
            phase: Some("receive_response".into()),
            phase_kind: Some("receive_payload".into()),
            phase_transition: Some("send_request->receive_response".into()),
            phase_transition_kind: Some("emit_payload->receive_payload".into()),
            suspect_area: "transport_io".into(),
            cause: ProgramFindingCause::MissingCoreStage,
            summary: "synthetic missing response".into(),
            supporting_fragments: vec!["tcp_packet_meta_fragment".into()],
            evidence_trace: vec!["missing_signal:packet_observed".into()],
        })
        .collect();
    export.debug_summary.program_flows = export.program_flows.len() as u64;
    export.debug_summary.program_findings = export.program_findings.len() as u64;
    export.debug_summary.module_findings = export.module_findings.len() as u64;
    export
}

pub(super) fn synthesize_large_scan_outputs(
    target_count: usize,
) -> Vec<(String, gewyvern::export::ExportBundle)> {
    let export = synthesize_large_protocol_flow_export();
    (0..target_count)
        .map(|index| (format!("scan:http:request:{index}"), export.clone()))
        .collect()
}

pub(super) fn synthesize_large_http_transactions() -> Vec<HttpTransactionView> {
    let transaction_count = 256u64;
    (0..transaction_count)
        .map(|offset| HttpTransactionView {
            id: HttpTransactionId(offset + 1),
            client_process: Some(ProcessView {
                pid: 10_000 + offset as u32,
                tid: 10_000 + offset as u32,
                cgroup_id: 4242,
                comm: format!("curl-{offset}"),
            }),
            server_process: Some(ProcessView {
                pid: 20_000 + offset as u32,
                tid: 20_000 + offset as u32,
                cgroup_id: 4343,
                comm: format!("nginx-{offset}"),
            }),
            components: vec![
                HttpComponentRef {
                    template_id: format!("dns-{offset}"),
                    kind: HttpComponentKind::DnsLookup,
                    operation: ProgramOperation::Custom("dns_lookup".into()),
                },
                HttpComponentRef {
                    template_id: format!("http-request-{offset}"),
                    kind: HttpComponentKind::ClientRequest,
                    operation: ProgramOperation::Custom("http_request".into()),
                },
                HttpComponentRef {
                    template_id: format!("http-response-{offset}"),
                    kind: HttpComponentKind::ServerResponse,
                    operation: ProgramOperation::Custom("http_server_response".into()),
                },
            ],
            phases: vec![
                "resolve_upstream".into(),
                "connect".into(),
                "send_request".into(),
                "receive_response".into(),
            ],
            phase_kinds: vec![
                "route".into(),
                "connect".into(),
                "emit_payload".into(),
                "receive_payload".into(),
            ],
            verdict: HttpTransactionVerdict::HealthyRequestResponsePath,
            severity: None,
            degraded: false,
            suspect_sides: Vec::new(),
            finding_summaries: Vec::new(),
            summaries: vec![format!("synthetic http transaction {offset}")],
        })
        .collect()
}
