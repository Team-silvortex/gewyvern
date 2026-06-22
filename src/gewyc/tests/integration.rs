use super::*;

#[test]
fn envelope_json_contains_all_frontend_surfaces() {
    let input =
        crate::dsl::read_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
            .unwrap();
    let envelope = compile_envelope_str(&input);
    let json = render_envelope_report(&envelope, RenderFormat::Json);
    assert!(json.contains("\"binding\":"));
    assert!(json.contains("\"diagnostics\":"));
    assert!(json.contains("\"findings\":{\"findings\":[]}"));
    assert!(json.contains("\"stages\":"));
    assert!(json.contains("\"template_id\":\"udp_process_debug\""));
}

#[test]
fn compile_frontend_report_file_materializes_pipeline_summary() {
    let report = compile_frontend_report_file(
        "/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy",
    )
    .unwrap();
    assert_eq!(report.kind, "pipeline");
    assert!(!report.function_nodes.is_empty());
    assert!(!report.graph_nodes.is_empty());
    assert!(!report.graph_edges.is_empty());
    assert!(!report.expansion_previews.is_empty());
    assert_eq!(report.expansion_previews[0].scope, "entry");
    assert!(
        report.expansion_previews[0]
            .steps
            .iter()
            .any(|step| step.starts_with("use("))
    );
}

#[test]
fn stages_json_includes_parse_and_diagnostics_sections() {
    let report =
        compile_stages_report_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
            .unwrap();
    let json = render_stages_report(&report, RenderFormat::Json);
    assert!(json.contains("\"parse\":{\"ok\":true"));
    assert!(json.contains("\"frontend\":"));
    assert!(json.contains("\"function_nodes\""));
    assert!(json.contains("\"use_edges\""));
    assert!(json.contains("\"graph_nodes\""));
    assert!(json.contains("\"graph_edges\""));
    assert!(json.contains("\"validation\":{\"ok\":true"));
    assert!(json.contains("\"registry\":\"builtin\""));
    assert!(json.contains(
        "\"checks\":[\"binding_schema\",\"fragment_params\",\"rule_evidence\",\"payload_offsets\"]"
    ));
    assert!(json.contains("\"sampled_payload_offsets\":[0,1,4,5,9,10,13]"));
    assert!(json.contains("\"required_payload_offsets\":[]"));
    assert!(json.contains("\"unsupported_payload_offsets\":[]"));
    assert!(json.contains("\"finding\":null"));
    assert!(json.contains("\"diagnostics\":{\"ok\":true"));
    assert!(json.contains("\"report\":"));
    assert!(json.contains("\"template_id\":\"udp_process_debug\""));
}

#[test]
fn stages_report_includes_pipeline_frontend_summary() {
    let report = compile_stages_report_str(
        r#"
fn udp_rules() {
  |> operation(:datagram_exchange)
  |> program_model(:frontend_summary_model)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :frontend_summary, phase: :bind)
}

fn udp_core() {
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> use(:udp_rules)
}

template(:frontend_summary)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core)
"#,
    );
    let frontend = report.parse.frontend.as_ref().unwrap();
    assert_eq!(frontend.kind, "pipeline");
    assert_eq!(frontend.function_count, 2);
    assert_eq!(
        frontend.function_nodes,
        vec![
            FrontendFunctionReport {
                name: "udp_core".to_string(),
                signature: "udp_core()".to_string(),
                step_count: 3,
                source_id: "entry".to_string(),
                package_scope: "inline".to_string(),
                params: Vec::new(),
            },
            FrontendFunctionReport {
                name: "udp_rules".to_string(),
                signature: "udp_rules()".to_string(),
                step_count: 3,
                source_id: "entry".to_string(),
                package_scope: "inline".to_string(),
                params: Vec::new(),
            }
        ]
    );
    assert_eq!(frontend.merged_step_count, 9);
    assert_eq!(
        frontend.use_edges,
        vec![
            FrontendUseEdgeReport {
                from: "entry".to_string(),
                to: "udp_core".to_string(),
                line: 17,
            },
            FrontendUseEdgeReport {
                from: "udp_core".to_string(),
                to: "udp_rules".to_string(),
                line: 11,
            }
        ]
    );
    assert_eq!(
        frontend.graph_nodes,
        vec![
            FrontendGraphNodeReport {
                id: "entry".to_string(),
                kind: "entry".to_string(),
                label: "entry".to_string(),
                package_scope: "inline".to_string(),
                step_count: Some(3),
            },
            FrontendGraphNodeReport {
                id: "fn:udp_core".to_string(),
                kind: "function".to_string(),
                label: "udp_core".to_string(),
                package_scope: "inline".to_string(),
                step_count: Some(3),
            },
            FrontendGraphNodeReport {
                id: "fn:udp_rules".to_string(),
                kind: "function".to_string(),
                label: "udp_rules".to_string(),
                package_scope: "inline".to_string(),
                step_count: Some(3),
            }
        ]
    );
    assert_eq!(
        frontend.graph_edges,
        vec![
            FrontendGraphEdgeReport {
                from: "entry".to_string(),
                to: "fn:udp_core".to_string(),
                kind: "use".to_string(),
                line: 17,
            },
            FrontendGraphEdgeReport {
                from: "fn:udp_core".to_string(),
                to: "fn:udp_rules".to_string(),
                kind: "use".to_string(),
                line: 11,
            }
        ]
    );
}

#[test]
fn stages_report_lists_include_sources_in_parse_frontend_summary() {
    let package_dir =
        std::env::temp_dir().join(format!("gewyc-frontend-summary-{}", std::process::id()));
    std::fs::create_dir_all(&package_dir).unwrap();
    std::fs::write(
        package_dir.join("gewy.pkg"),
        "name=frontend_summary_pkg\nversion=0.1.0\nentry=main.gewy\n",
    )
    .unwrap();
    std::fs::write(
        package_dir.join("main.gewy"),
        r#"
template(:frontend_summary_pkg)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("./module.gewy")
|> use(:udp_core)
"#,
    )
    .unwrap();
    std::fs::write(
        package_dir.join("module.gewy"),
        r#"
fn udp_core() {
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> operation(:datagram_exchange)
  |> program_model(:frontend_summary_pkg_model)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :frontend_summary_pkg, phase: :bind)
}
"#,
    )
    .unwrap();

    let report = compile_stages_report_file(package_dir.to_str().unwrap()).unwrap();
    let frontend = report.parse.frontend.as_ref().unwrap();
    assert_eq!(frontend.kind, "pipeline");
    assert_eq!(frontend.function_count, 1);
    assert_eq!(frontend.function_nodes.len(), 1);
    assert_eq!(frontend.function_nodes[0].name, "udp_core");
    assert_eq!(frontend.function_nodes[0].step_count, 5);
    assert_eq!(
        frontend.function_nodes[0].package_scope,
        "frontend_summary_pkg"
    );
    assert!(frontend.function_nodes[0].params.is_empty());
    assert!(
        frontend.function_nodes[0]
            .source_id
            .ends_with("module.gewy")
    );
    assert_eq!(frontend.include_sources.len(), 1);
    assert_eq!(frontend.include_sources[0].request, "./module.gewy");
    assert_eq!(frontend.include_sources[0].kind, "local");
    assert_eq!(frontend.include_sources[0].dependency, None);
    assert_eq!(
        frontend.include_sources[0].package_scope,
        "frontend_summary_pkg"
    );
    assert!(
        frontend.include_sources[0]
            .resolved_path
            .ends_with("module.gewy")
    );
    assert_eq!(
        frontend.use_edges,
        vec![FrontendUseEdgeReport {
            from: "entry".to_string(),
            to: "udp_core".to_string(),
            line: 6,
        }]
    );
    assert!(frontend.graph_nodes.iter().any(|node| node.kind == "entry"));
    assert!(frontend.graph_nodes.iter().any(|node| node.kind == "file"));
    assert!(
        frontend
            .graph_nodes
            .iter()
            .any(|node| node.label == "module.gewy")
    );
    assert!(
        frontend
            .graph_nodes
            .iter()
            .any(|node| node.kind == "file" && node.package_scope == "frontend_summary_pkg")
    );
    assert!(
        frontend
            .graph_edges
            .iter()
            .any(|edge| edge.kind == "include" && edge.line == 5)
    );
}

#[test]
fn stages_report_infers_pipeline_function_parameter_kinds() {
    let report = compile_stages_report_str(
        r#"
fn udp_core(model_name, op_name = :datagram_exchange, dedupe_flag = true, duration_ms = 5000) =
  |> window(duration_ms: ${duration_ms}, lateness_ms: 200)
  |> operation(${op_name})
  |> program_model(${model_name})
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: ${dedupe_flag}, module: :frontend_summary, phase: :bind)

template(:frontend_summary)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, :typed_model)
"#,
    );
    let frontend = report.parse.frontend.as_ref().unwrap();
    let function = frontend
        .function_nodes
        .iter()
        .find(|node| node.name == "udp_core")
        .unwrap();
    assert_eq!(
        function.signature,
        "udp_core(model_name, op_name = :datagram_exchange, dedupe_flag = true, duration_ms = 5000)"
    );
    assert_eq!(
        function.params,
        vec![
            FrontendFunctionParamReport {
                name: "model_name".to_string(),
                has_default: false,
                declared_kind: None,
                effective_kind: Some("atom".to_string()),
            },
            FrontendFunctionParamReport {
                name: "op_name".to_string(),
                has_default: true,
                declared_kind: None,
                effective_kind: Some("atom".to_string()),
            },
            FrontendFunctionParamReport {
                name: "dedupe_flag".to_string(),
                has_default: true,
                declared_kind: None,
                effective_kind: Some("bool".to_string()),
            },
            FrontendFunctionParamReport {
                name: "duration_ms".to_string(),
                has_default: true,
                declared_kind: None,
                effective_kind: Some("u64".to_string()),
            },
        ]
    );
}

#[test]
fn stages_report_surfaces_declared_and_effective_pipeline_parameter_kinds() {
    let report = compile_stages_report_str(
        r#"
fn udp_core(model_name: atom, dedupe_flag: bool = true, duration_ms: u64 = 5000) =
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> window(duration_ms: $duration_ms, lateness_ms: 200)
  |> operation(:datagram_exchange)
  |> program_model($model_name)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: $dedupe_flag, module: :frontend_summary, phase: :bind)

template(:frontend_summary_typed)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, :typed_model)
"#,
    );
    let frontend = report.parse.frontend.as_ref().unwrap();
    let function = frontend
        .function_nodes
        .iter()
        .find(|node| node.name == "udp_core")
        .unwrap();
    assert_eq!(
        function.signature,
        "udp_core(model_name: atom, dedupe_flag: bool = true, duration_ms: u64 = 5000)"
    );
    assert_eq!(
        function.params,
        vec![
            FrontendFunctionParamReport {
                name: "model_name".to_string(),
                has_default: false,
                declared_kind: Some("atom".to_string()),
                effective_kind: Some("atom".to_string()),
            },
            FrontendFunctionParamReport {
                name: "dedupe_flag".to_string(),
                has_default: true,
                declared_kind: Some("bool".to_string()),
                effective_kind: Some("bool".to_string()),
            },
            FrontendFunctionParamReport {
                name: "duration_ms".to_string(),
                has_default: true,
                declared_kind: Some("u64".to_string()),
                effective_kind: Some("u64".to_string()),
            },
        ]
    );
}

#[test]
fn explain_report_uses_default_pipeline_function_arguments() {
    let report = compile_explain_report_str(
        r#"
fn udp_client(model_name = :default_model, op_name = :datagram_exchange) =
  let module_name = :udp_client
  |> fragment(:udp_packet_meta_fragment)
  |> operation(${op_name})
  |> program_model(${model_name})

template(:frontend_defaults)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_client)
"#,
    );
    assert!(report.ok);
    let binding = report.binding.as_ref().unwrap();
    let model = binding.program_model.as_ref().unwrap();
    assert_eq!(model.id, "default_model");
    assert_eq!(model.operation, "datagram_exchange");
}

#[test]
fn explain_report_allows_partial_override_of_default_pipeline_function_arguments() {
    let report = compile_explain_report_str(
        r#"
fn udp_client(model_name = :default_model, op_name = :datagram_exchange) =
  let module_name = :udp_client
  |> fragment(:udp_packet_meta_fragment)
  |> operation(${op_name})
  |> program_model(${model_name})

template(:frontend_defaults)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_client, :custom_model)
"#,
    );
    assert!(report.ok);
    let binding = report.binding.as_ref().unwrap();
    let model = binding.program_model.as_ref().unwrap();
    assert_eq!(model.id, "custom_model");
    assert_eq!(model.operation, "datagram_exchange");
}

#[test]
fn explain_report_supports_named_pipeline_function_arguments() {
    let report = compile_explain_report_str(
        r#"
fn udp_client(model_name = :default_model, op_name = :datagram_exchange) =
  let module_name = :udp_client
  |> fragment(:udp_packet_meta_fragment)
  |> operation(${op_name})
  |> program_model(${model_name})

template(:frontend_defaults)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_client, op_name: :custom_exchange, model_name: :named_model)
"#,
    );
    assert!(report.ok);
    let binding = report.binding.as_ref().unwrap();
    let model = binding.program_model.as_ref().unwrap();
    assert_eq!(model.id, "named_model");
    assert_eq!(model.operation, "custom_exchange");
}

#[test]
fn explain_report_supports_positional_then_named_pipeline_function_arguments() {
    let report = compile_explain_report_str(
        r#"
fn udp_client(model_name, op_name = :datagram_exchange) =
  |> fragment(:udp_packet_meta_fragment)
  |> operation(${op_name})
  |> program_model(${model_name})

template(:frontend_defaults)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_client, :mixed_model, op_name: :custom_exchange)
"#,
    );
    assert!(report.ok);
    let binding = report.binding.as_ref().unwrap();
    let model = binding.program_model.as_ref().unwrap();
    assert_eq!(model.id, "mixed_model");
    assert_eq!(model.operation, "custom_exchange");
}

#[test]
fn explain_report_rejects_atom_inference_mismatches_for_pipeline_arguments() {
    let report = compile_explain_report_str(
        r#"
fn udp_client(model_name, op_name = :datagram_exchange) =
  |> fragment(:udp_packet_meta_fragment)
  |> operation(${op_name})
  |> program_model(${model_name})

template(:frontend_defaults)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_client, "unsafe model")
"#,
    );
    assert!(!report.ok);
    let text = render_explain_report(&report, RenderFormat::Text);
    assert!(text.contains("expects atom-like identifier value"));
    assert!(text.contains("model_name"));
}

#[test]
fn explain_report_rejects_predicate_inference_mismatches_for_pipeline_arguments() {
    let report = compile_explain_report_str(
        r#"
fn rule_module(predicate_name = :process_bound) =
  |> program_model(:predicate_model)
  |> operation(:datagram_exchange)
  |> program_rule(predicate: ${predicate_name}, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :predicate_module, phase: :bind)

template(:frontend_defaults)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:rule_module, predicate_name: :not_a_real_predicate)
"#,
    );
    assert!(!report.ok);
    let text = render_explain_report(&report, RenderFormat::Text);
    assert!(text.contains("expects predicate-compatible value"));
    assert!(text.contains("predicate_name"));
}

#[test]
fn explain_report_rejects_narrative_inference_mismatches_for_pipeline_arguments() {
    let report = compile_explain_report_str(
        r#"
fn rule_module(narrative_value = :process_bound) =
  |> program_model(:narrative_model)
  |> operation(:datagram_exchange)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: ${narrative_value}, dedupe: true, module: :predicate_module, phase: :bind)

template(:frontend_defaults)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:rule_module, narrative_value: "free text")
"#,
    );
    assert!(!report.ok);
    let text = render_explain_report(&report, RenderFormat::Text);
    assert!(text.contains("expects narrative-compatible value"));
    assert!(text.contains("narrative_value"));
    assert!(text.contains("static:"));
}

#[test]
fn explain_report_rejects_stage_inference_mismatches_for_pipeline_arguments() {
    let report = compile_explain_report_str(
        r#"
fn stage_module(stage_value = :process_bound) =
  |> program_model(:stage_model)
  |> operation(:datagram_exchange)
  |> program_rule(predicate: :process_bound, stage: ${stage_value}, narrative: :process_bound, dedupe: true, module: :stage_module, phase: :bind)

template(:frontend_defaults)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:stage_module, stage_value: :not_a_real_stage)
"#,
    );
    assert!(!report.ok);
    let text = render_explain_report(&report, RenderFormat::Text);
    assert!(text.contains("expects stage-compatible value"));
    assert!(text.contains("stage_value"));
}

#[test]
fn explain_report_rejects_key_event_inference_mismatches_for_pipeline_arguments() {
    let report = compile_explain_report_str(
        r#"
fn reason_module(event_value = :process_identified) =
  |> reason_model(:reason_model)
  |> reason_rule(predicate: :process_bound, key_event: ${event_value}, narrative: :process_bound, dedupe: true, module: :reason_module, phase: :bind)

template(:frontend_defaults)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:reason_module, event_value: :not_a_real_event)
"#,
    );
    assert!(!report.ok);
    let text = render_explain_report(&report, RenderFormat::Text);
    assert!(text.contains("expects key_event-compatible value"));
    assert!(text.contains("event_value"));
}

#[test]
fn explain_report_rejects_phase_inference_mismatches_for_pipeline_arguments() {
    let report = compile_explain_report_str(
        r#"
fn phase_module(phase_value = :send_request) =
  |> program_model(:phase_model)
  |> operation(:datagram_exchange)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :phase_module, phase: ${phase_value})

template(:frontend_defaults)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:phase_module, phase_value: :send-request)
"#,
    );
    assert!(!report.ok);
    let text = render_explain_report(&report, RenderFormat::Text);
    assert!(text.contains("expects phase-compatible value"));
    assert!(text.contains("phase_value"));
    assert!(text.contains("snake_case"));
}

#[test]
fn stages_report_summarizes_payload_offset_support() {
    let report =
        compile_stages_report_file("/Users/Shared/chroot/dev/gewyvern/dsl/snmp_get_path.gewy")
            .unwrap();
    assert_eq!(
        report.validation.sampled_payload_offsets,
        vec![0, 1, 4, 5, 9, 10, 13]
    );
    assert_eq!(report.validation.required_payload_offsets, vec![13]);
    assert_eq!(
        report.validation.unsupported_payload_offsets,
        Vec::<u16>::new()
    );
}

#[test]
fn envelope_json_is_valid_for_stable_subset_entry() {
    let report =
        compile_envelope_file("/Users/Shared/chroot/dev/gewyvern/dsl/http_request_path.gewy")
            .unwrap();
    let json = render_envelope_report(&report, RenderFormat::Json);
    assert_valid_json_document(&json);
}

#[test]
fn envelope_json_is_valid_for_registry_amqp_publish_entry() {
    let report =
        compile_envelope_file("/Users/Shared/chroot/dev/gewyvern/protocols/amqp/publish/main.gewy")
            .unwrap();
    let json = render_envelope_report(&report, RenderFormat::Json);
    assert_valid_json_document(&json);
}
