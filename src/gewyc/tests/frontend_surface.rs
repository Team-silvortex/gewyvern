use super::*;

fn typed_frontend_report() -> FrontendReport {
    compile_frontend_report_str(
        r#"
fn udp_core(model_name: atom, dedupe_flag: bool = true, duration_ms = 5000) =
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
    )
    .unwrap()
}

#[test]
fn frontend_functions_text_uses_signature_and_only_shows_extra_param_notes() {
    let report = typed_frontend_report();
    let text = render_frontend_report_with_focus(
        &report,
        RenderFormat::Text,
        Some(FrontendFocus::Functions),
    );
    assert!(text.contains(
        "udp_core(model_name: atom, dedupe_flag: bool = true, duration_ms = 5000)"
    ));
    assert!(text.contains("param_notes: duration_ms <inferred u64>"));
    assert!(!text.contains("dedupe_flag <bool, default>"));
}

#[test]
fn frontend_functions_compact_text_uses_signature_and_note_delta_only() {
    let report = typed_frontend_report();
    let text = render_frontend_report_with_options(
        &report,
        RenderFormat::Text,
        Some(FrontendFocus::Functions),
        true,
    );
    assert!(text.contains(
        "udp_core(model_name: atom, dedupe_flag: bool = true, duration_ms = 5000):7@entry#inline"
    ));
    assert!(text.contains("{notes: duration_ms <inferred u64>}"));
    assert!(!text.contains("model_name <atom>"));
}
