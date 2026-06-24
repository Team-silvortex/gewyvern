use super::*;
use crate::gewyc::{IrModelReport, IrReport, IrRuleReport};

#[test]
fn ir_model_report_helpers_summarize_supported_rules_and_metadata() {
    let model = IrModelReport {
        kind: "program_model".into(),
        id: "demo_model".into(),
        operation: Some("demo_op".into()),
        rules: vec![
            IrRuleReport {
                rule_index: 0,
                predicate: "process_bound".into(),
                signal: None,
                narrative: "process_bound".into(),
                dedupe: true,
                module: Some("alpha".into()),
                phase: Some("bind".into()),
                phase_kind: Some("bind_process".into()),
                required_facts: vec![],
                supporting_fragments: vec![],
                missing_facts: vec![],
                unsupported_payload_offsets: vec![],
                supported: true,
            },
            IrRuleReport {
                rule_index: 1,
                predicate: "route_resolved".into(),
                signal: None,
                narrative: "route_changed".into(),
                dedupe: true,
                module: Some("alpha".into()),
                phase: Some("resolve".into()),
                phase_kind: Some("resolve_route".into()),
                required_facts: vec![],
                supporting_fragments: vec![],
                missing_facts: vec![],
                unsupported_payload_offsets: vec![8],
                supported: false,
            },
        ],
    };
    let report = IrReport {
        template_id: "demo_template".into(),
        program_model: Some(model.clone()),
        reason_model: None,
    };

    assert_eq!(model.supported_rule_count(), 1);
    assert_eq!(model.unsupported_rule_count(), 1);
    assert_eq!(model.modules(), vec!["alpha".to_string()]);
    assert_eq!(
        model.phases(),
        vec!["bind".to_string(), "resolve".to_string()]
    );
    assert_eq!(report.model_entries().len(), 1);
    assert_eq!(report.model_entries()[0].0, "program_model");
    assert_eq!(model.rules[0].module_name(), Some("alpha"));
    assert_eq!(model.rules[1].phase_name(), Some("resolve"));
    assert_eq!(model.rules[1].phase_kind_name(), Some("resolve_route"));
    assert_eq!(model.rules[0].support_shape().required_facts.len(), 0);
    assert_eq!(
        model.rules[1].support_shape().unsupported_payload_offsets,
        &[8]
    );
    assert!(model.rules[0].support_shape().supported);
    assert!(!model.rules[1].support_shape().supported);
    assert!(!model.rules[0].has_unsupported_payload_offsets());
    assert!(model.rules[1].has_unsupported_payload_offsets());
}

#[test]
fn ir_report_compare_models_summarizes_alignment_and_deltas() {
    let program = IrModelReport {
        kind: "program_model".into(),
        id: "program_demo".into(),
        operation: Some("demo_op".into()),
        rules: vec![IrRuleReport {
            rule_index: 0,
            predicate: "process_bound".into(),
            signal: Some("process_identified".into()),
            narrative: "process_bound".into(),
            dedupe: true,
            module: Some("shared".into()),
            phase: Some("bind".into()),
            phase_kind: Some("bind_process".into()),
            required_facts: vec![],
            supporting_fragments: vec![],
            missing_facts: vec![],
            unsupported_payload_offsets: vec![],
            supported: true,
        }],
    };
    let reason = IrModelReport {
        kind: "declarative_reason_model".into(),
        id: "reason_demo".into(),
        operation: None,
        rules: vec![
            IrRuleReport {
                rule_index: 0,
                predicate: "process_bound".into(),
                signal: Some("process_identified".into()),
                narrative: "process_bound".into(),
                dedupe: true,
                module: Some("shared".into()),
                phase: Some("bind".into()),
                phase_kind: Some("bind_process".into()),
                required_facts: vec![],
                supporting_fragments: vec![],
                missing_facts: vec![],
                unsupported_payload_offsets: vec![],
                supported: true,
            },
            IrRuleReport {
                rule_index: 1,
                predicate: "route_resolved".into(),
                signal: None,
                narrative: "route_changed".into(),
                dedupe: true,
                module: Some("reason_only".into()),
                phase: Some("resolve".into()),
                phase_kind: Some("resolve_route".into()),
                required_facts: vec![],
                supporting_fragments: vec![],
                missing_facts: vec![],
                unsupported_payload_offsets: vec![],
                supported: false,
            },
        ],
    };
    let report = IrReport {
        template_id: "compare_demo".into(),
        program_model: Some(program),
        reason_model: Some(reason),
    };

    let compare = report
        .compare_models()
        .expect("compare summary should exist");
    assert_eq!(compare.program_rule_count, 1);
    assert_eq!(compare.reason_rule_count, 2);
    assert_eq!(compare.rule_count_delta, -1);
    assert_eq!(compare.program_supported_rule_count, 1);
    assert_eq!(compare.reason_supported_rule_count, 1);
    assert_eq!(compare.supported_rule_count_delta, 0);
    assert_eq!(compare.shared_modules, vec!["shared".to_string()]);
    assert_eq!(compare.program_only_modules, Vec::<String>::new());
    assert_eq!(compare.reason_only_modules, vec!["reason_only".to_string()]);
    assert_eq!(compare.shared_phases, vec!["bind".to_string()]);
    assert_eq!(compare.program_only_phases, Vec::<String>::new());
    assert_eq!(compare.reason_only_phases, vec!["resolve".to_string()]);

    let history = report.history_snapshot();
    assert_eq!(history.template_id, "compare_demo");
    assert_eq!(history.operation, Some("demo_op".to_string()));
    assert_eq!(
        history.program_model.as_ref().map(|m| m.rule_count),
        Some(1)
    );
    assert_eq!(history.reason_model.as_ref().map(|m| m.rule_count), Some(2));
    assert_eq!(
        history
            .model_compare
            .as_ref()
            .map(|compare| compare.rule_count_delta),
        Some(-1)
    );
    assert_eq!(
        history
            .model_compare
            .as_ref()
            .map(|compare| compare.reason_only_modules.clone()),
        Some(vec!["reason_only".to_string()])
    );
}

#[test]
fn explain_ir_focus_reports_builtin_reason_and_lowered_program_shape() {
    let report =
        compile_explain_report_file("/Users/Shared/chroot/dev/gewyvern/dsl/udp_process_debug.gewy")
            .unwrap();
    let text =
        render_explain_report_with_focus(&report, RenderFormat::Text, Some(ExplainFocus::Ir));
    let json =
        render_explain_report_with_focus(&report, RenderFormat::Json, Some(ExplainFocus::Ir));

    assert!(text.contains("focus=ir"));
    assert!(text.contains("ir_delta.frontend_functions=1"));
    assert!(text.contains("ir_delta.lowered_program_rules=3"));
    assert!(text.contains("ir_delta.lowered_reason_rules=0"));
    assert!(text.contains("program_model=udp_process_debug_dsl_model"));
    assert!(text.contains("program_model_operation=datagram_exchange"));
    assert!(text.contains("reason_model=udp_datagram_l1 kind=builtin_reason_profile rules=0"));
    assert!(json.contains("\"kind\":\"ir\""));
    assert!(json.contains("\"ir_lowering_delta\":{"));
    assert!(json.contains("\"frontend_function_count\":1"));
    assert!(json.contains("\"id\":\"udp_process_debug_dsl_model\""));
    assert!(json.contains("\"kind\":\"builtin_reason_profile\""));
}

#[test]
fn ir_json_surfaces_status_count_and_analysis_groups() {
    let report = compile_ir_report_file(
        "/Users/Shared/chroot/dev/gewyvern/protocols/amqp/publish/main.gewy",
    )
    .unwrap();
    let json = render_explain_report_with_focus(
        &compile_explain_report_file(
            "/Users/Shared/chroot/dev/gewyvern/protocols/amqp/publish/main.gewy",
        )
        .unwrap(),
        RenderFormat::Json,
        Some(ExplainFocus::Ir),
    );

    assert_eq!(report.template_id, "amqp_basic_publish_path");
    assert!(json.contains("\"status\":{\"present\":true}"));
    assert!(json.contains("\"analysis\":{\"ir_lowering_delta\":"));
    assert!(json.contains("\"report\":{\"template_id\":\"amqp_basic_publish_path\""));
    assert!(json.contains("\"counts\":{\"program_rules\":"));
    assert!(json.contains("\"analysis\":{\"model_compare\":"));
}

#[test]
fn compile_ir_report_file_materializes_ir_surface_directly() {
    let report = compile_ir_report_file(
        "/Users/Shared/chroot/dev/gewyvern/protocols/amqp/publish/main.gewy",
    )
    .unwrap();

    assert_eq!(report.template_id, "amqp_basic_publish_path");
    assert_eq!(
        report.program_model.as_ref().map(|model| model.id.as_str()),
        Some("amqp_basic_publish_path_dsl_model")
    );
    assert_eq!(
        report.reason_model.as_ref().map(|model| model.id.as_str()),
        Some("amqp_basic_publish_path_reason")
    );
}

#[test]
fn render_ir_history_snapshot_exposes_archival_shape() {
    let report = compile_explain_report_file(
        "/Users/Shared/chroot/dev/gewyvern/protocols/amqp/publish/main.gewy",
    )
    .unwrap();
    let ir_report = report.ir_report.as_ref().expect("ir report should exist");
    let text = render_ir_history_snapshot(ir_report, RenderFormat::Text);
    let json = render_ir_history_snapshot(ir_report, RenderFormat::Json);

    assert!(text.contains("template=amqp_basic_publish_path"));
    assert!(text.contains("operation=amqp_basic_publish"));
    assert!(text.contains("program_model.id=amqp_basic_publish_path_dsl_model"));
    assert!(text.contains("reason_model.id=amqp_basic_publish_path_reason"));
    assert!(text.contains("model_compare.rule_count_delta=0"));
    assert!(text.contains("model_compare.shared_modules=amqp_basic_publish_path"));
    assert!(json.contains("\"template_id\":\"amqp_basic_publish_path\""));
    assert!(json.contains("\"operation\":\"amqp_basic_publish\""));
    assert!(json.contains("\"program_model\":{"));
    assert!(json.contains("\"reason_model\":{"));
    assert!(json.contains("\"model_compare\":{"));
}

#[test]
fn explain_ir_focus_reports_protocol_phase_and_support_details() {
    let report = compile_explain_report_file(
        "/Users/Shared/chroot/dev/gewyvern/protocols/amqp/publish/main.gewy",
    )
    .unwrap();
    let text =
        render_explain_report_with_focus(&report, RenderFormat::Text, Some(ExplainFocus::Ir));
    let json =
        render_explain_report_with_focus(&report, RenderFormat::Json, Some(ExplainFocus::Ir));

    assert!(text.contains("focus=ir"));
    assert!(text.contains("ir_delta.lowered_modules=amqp_basic_publish_path"));
    assert!(text.contains(
        "ir_delta.lowered_phases=bind,connect,establish,receive_ack,resolve_upstream,send_publish"
    ));
    assert!(text.contains(
        "ir_delta.lowered_phase_kinds=bind_process,emit_payload,establish_connection,initiate_connection,receive_payload,resolve_route"
    ));
    assert!(text.contains("program_model_operation=amqp_basic_publish"));
    assert!(text.contains("phase=send_publish phase_kind=emit_payload"));
    assert!(text.contains("supporting=tcp_packet_meta_fragment"));
    assert!(text.contains(
        "reason_model=amqp_basic_publish_path_reason kind=declarative_reason_model rules=6"
    ));
    assert!(text.contains("ir_delta.model.program_model.id=amqp_basic_publish_path_dsl_model"));
    assert!(text.contains("ir_delta.model.program_model.modules=amqp_basic_publish_path"));
    assert!(text.contains("ir_delta.model.reason_model.id=amqp_basic_publish_path_reason"));
    assert!(text.contains("ir_compare.program_rules=6"));
    assert!(text.contains("ir_compare.reason_rules=6"));
    assert!(text.contains("ir_compare.shared_modules=amqp_basic_publish_path"));
    assert!(json.contains("\"lowered_modules\":[\"amqp_basic_publish_path\"]"));
    assert!(json.contains(
        "\"lowered_phase_kinds\":[\"bind_process\",\"emit_payload\",\"establish_connection\",\"initiate_connection\",\"receive_payload\",\"resolve_route\"]"
    ));
    assert!(json.contains("\"lowered_models\":[{"));
    assert!(json.contains("\"label\":\"program_model\""));
    assert!(json.contains("\"supported_rule_count\":6"));
    assert!(json.contains("\"label\":\"reason_model\""));
    assert!(json.contains("\"phase_kind\":\"emit_payload\""));
    assert!(json.contains("\"supporting_fragments\":[\"tcp_packet_meta_fragment\"]"));
    assert!(json.contains("\"kind\":\"declarative_reason_model\""));
    assert!(json.contains("\"model_compare\":{"));
    assert!(json.contains("\"history_snapshot\":{"));
    assert!(json.contains("\"template_id\":\"amqp_basic_publish_path\""));
    assert!(json.contains("\"program_rule_count\":6"));
    assert!(json.contains("\"shared_modules\":[\"amqp_basic_publish_path\"]"));
    assert!(json.contains("\"operation\":\"amqp_basic_publish\""));
    assert!(json.contains("\"reason_only_modules\":[]"));
}
