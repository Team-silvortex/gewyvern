use super::*;
use gewyvern::gewyc::{
    RenderFormat, compile_binding_report_file, compile_envelope_file, compile_explain_report_file,
    compile_frontend_report_file, render_binding_report, render_envelope_report,
    render_explain_report, render_explain_report_with_focus, render_explain_report_with_options,
    render_frontend_report, render_frontend_report_with_focus, render_frontend_report_with_options,
};
use std::fs;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("gewyc crate should live under crates/gewyc")
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

fn protocol_fixture_path(relative: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("gewyc crate should live under crates/gewyc")
        .join("protocols")
        .join(relative)
        .to_string_lossy()
        .into_owned()
}

fn temp_test_dir(prefix: &str) -> std::path::PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!("gewyc-{prefix}-{}-{nonce}", std::process::id()));
    fs::create_dir_all(&root).expect("create temp test dir");
    root
}
#[test]
fn parse_cli_defaults_to_compile_command() {
    let cli = parse_cli(
        vec!["gewyc".into(), "dsl/udp_process_debug.gewy".into()],
        UiLocale::En,
    )
    .unwrap();
    assert_eq!(
        cli,
        Cli {
            command: Command::Compile,
            emit: EmitTarget::Binding,
            path: "dsl/udp_process_debug.gewy".into(),
            output: OutputMode::Text,
            focus: None,
            compact: false,
            out: None,
        }
    );
}

#[test]
fn parse_cli_accepts_diagnostics_json_mode() {
    let cli = parse_cli(
        vec![
            "gewyc".into(),
            "diagnostics".into(),
            "dsl/udp_process_debug.gewy".into(),
            "--json".into(),
        ],
        UiLocale::En,
    )
    .unwrap();
    assert_eq!(cli.command, Command::Diagnostics);
    assert_eq!(cli.emit, EmitTarget::Diagnostics);
    assert_eq!(cli.output, OutputMode::Json);
}

#[test]
fn parse_cli_accepts_frontend_command() {
    let cli = parse_cli(
        vec![
            "gewyc".into(),
            "frontend".into(),
            "dsl/udp_process_debug.gewy".into(),
            "--json".into(),
        ],
        UiLocale::En,
    )
    .unwrap();
    assert_eq!(cli.command, Command::Frontend);
    assert_eq!(cli.emit, EmitTarget::Frontend);
    assert_eq!(cli.output, OutputMode::Json);
}

#[test]
fn parse_cli_accepts_explain_command() {
    let cli = parse_cli(
        vec![
            "gewyc".into(),
            "explain".into(),
            "dsl/udp_process_debug.gewy".into(),
            "--json".into(),
        ],
        UiLocale::En,
    )
    .unwrap();
    assert_eq!(cli.command, Command::Explain);
    assert_eq!(cli.emit, EmitTarget::Explain);
    assert_eq!(cli.output, OutputMode::Json);
}

#[test]
fn parse_cli_accepts_explain_focus() {
    let cli = parse_cli(
        vec![
            "gewyc".into(),
            "explain".into(),
            "dsl/udp_process_debug.gewy".into(),
            "--focus".into(),
            "validation".into(),
        ],
        UiLocale::En,
    )
    .unwrap();
    assert_eq!(cli.focus.as_deref(), Some("validation"));
}

#[test]
fn parse_cli_accepts_explain_binding_focus() {
    let cli = parse_cli(
        vec![
            "gewyc".into(),
            "explain".into(),
            "dsl/udp_process_debug.gewy".into(),
            "--focus".into(),
            "binding".into(),
        ],
        UiLocale::En,
    )
    .unwrap();
    assert_eq!(cli.focus.as_deref(), Some("binding"));
}

#[test]
fn parse_cli_accepts_frontend_focus() {
    let cli = parse_cli(
        vec![
            "gewyc".into(),
            "frontend".into(),
            "dsl/udp_process_debug.gewy".into(),
            "--focus".into(),
            "graph".into(),
        ],
        UiLocale::En,
    )
    .unwrap();
    assert_eq!(cli.focus.as_deref(), Some("graph"));
}

#[test]
fn parse_cli_accepts_compact_for_explain() {
    let cli = parse_cli(
        vec![
            "gewyc".into(),
            "explain".into(),
            "dsl/udp_process_debug.gewy".into(),
            "--compact".into(),
        ],
        UiLocale::En,
    )
    .unwrap();
    assert!(cli.compact);
}

#[test]
fn parse_cli_rejects_focus_outside_explain() {
    let err = parse_cli(
        vec![
            "gewyc".into(),
            "diagnostics".into(),
            "dsl/udp_process_debug.gewy".into(),
            "--focus".into(),
            "parse".into(),
        ],
        UiLocale::En,
    )
    .unwrap_err();
    assert!(err.contains("--focus is only valid with explain/frontend"));
}

#[test]
fn parse_cli_rejects_compact_outside_explain() {
    let err = parse_cli(
        vec![
            "gewyc".into(),
            "diagnostics".into(),
            "dsl/udp_process_debug.gewy".into(),
            "--compact".into(),
        ],
        UiLocale::En,
    )
    .unwrap_err();
    assert!(err.contains("--compact is only valid with explain/frontend"));
}

#[test]
fn parse_cli_accepts_emit_and_out_flags() {
    let cli = parse_cli(
        vec![
            "gewyc".into(),
            "dsl/udp_process_debug.gewy".into(),
            "--emit".into(),
            "diagnostics".into(),
            "--out".into(),
            "/tmp/gewyc-out.json".into(),
        ],
        UiLocale::En,
    )
    .unwrap();
    assert_eq!(cli.command, Command::Compile);
    assert_eq!(cli.emit, EmitTarget::Diagnostics);
    assert_eq!(cli.out.as_deref(), Some("/tmp/gewyc-out.json"));
}

#[test]
fn parse_cli_accepts_findings_command() {
    let cli = parse_cli(
        vec![
            "gewyc".into(),
            "findings".into(),
            "dsl/udp_process_debug.gewy".into(),
            "--json".into(),
        ],
        UiLocale::En,
    )
    .unwrap();
    assert_eq!(cli.command, Command::Findings);
    assert_eq!(cli.emit, EmitTarget::Findings);
    assert_eq!(cli.output, OutputMode::Json);
}

#[test]
fn parse_cli_accepts_stages_command() {
    let cli = parse_cli(
        vec![
            "gewyc".into(),
            "stages".into(),
            "dsl/udp_process_debug.gewy".into(),
            "--json".into(),
        ],
        UiLocale::En,
    )
    .unwrap();
    assert_eq!(cli.command, Command::Stages);
    assert_eq!(cli.emit, EmitTarget::Stages);
    assert_eq!(cli.output, OutputMode::Json);
}

#[test]
fn parse_cli_accepts_envelope_command() {
    let cli = parse_cli(
        vec![
            "gewyc".into(),
            "envelope".into(),
            "dsl/udp_process_debug.gewy".into(),
            "--json".into(),
        ],
        UiLocale::En,
    )
    .unwrap();
    assert_eq!(cli.command, Command::Envelope);
    assert_eq!(cli.emit, EmitTarget::Envelope);
    assert_eq!(cli.output, OutputMode::Json);
}

#[test]
fn parse_cli_accepts_init_without_path() {
    let cli = parse_cli(vec!["gewyc".into(), "init".into()], UiLocale::En).unwrap();
    assert_eq!(cli.command, Command::Init);
    assert_eq!(cli.path, ".");
}

#[test]
fn parse_cli_accepts_lock_without_path() {
    let cli = parse_cli(vec!["gewyc".into(), "lock".into()], UiLocale::En).unwrap();
    assert_eq!(cli.command, Command::Lock);
    assert_eq!(cli.path, ".");
}

#[test]
fn binding_json_mentions_template_id() {
    let report = compile_binding_report_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let json = render_binding_report(&report, RenderFormat::Json);
    assert!(json.contains("\"surface_id\":\"gewyc.binding\""));
    assert!(json.contains(
        "\"schema_hint\":{\"family\":\"gewyc\",\"surface\":\"binding\",\"schema_version\":1}"
    ));
    assert!(json.contains("\"template_id\":\"udp_process_debug\""));
    assert!(json.contains("\"program_model\""));
}

#[test]
fn cli_envelope_collects_binding_and_stages_from_shared_entrypoint() {
    let envelope = compile_envelope_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    assert_eq!(
        envelope
            .binding
            .as_ref()
            .map(|report| report.template_id.as_str()),
        Some("udp_process_debug")
    );
    assert!(envelope.stages.parse.ok);
    assert!(envelope.stages.validation.ok);
}

#[test]
fn envelope_json_mentions_all_surfaces() {
    let envelope = compile_envelope_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let json = render_envelope_report(&envelope, RenderFormat::Json);
    assert!(json.contains("\"surface_id\":\"gewyc.envelope\""));
    assert!(json.contains(
        "\"schema_hint\":{\"family\":\"gewyc\",\"surface\":\"envelope\",\"schema_version\":1}"
    ));
    assert!(json.contains("\"binding\":"));
    assert!(json.contains("\"diagnostics\":"));
    assert!(json.contains("\"findings\":{\"summary\":"));
    assert!(json.contains("\"findings\":[]"));
    assert!(json.contains("\"stages\":"));
}

#[test]
fn init_templates_include_manifest_and_main_entry() {
    assert!(render_init_manifest("demo").contains("# gewylang package manifest"));
    assert!(render_init_manifest("demo").contains("entry=main.gewy"));
    assert!(render_init_entry("demo").contains("# main.gewy is the single package entrypoint."));
    assert!(render_init_entry("demo").contains("template :demo"));
    assert!(render_init_entry("demo").contains("|> include \"./module.gewy\""));
    assert!(render_init_entry("demo").contains("|> use :network_module"));
    assert!(render_init_module("demo").contains("# module.gewy is for reusable function units."));
    assert!(render_init_module("demo").contains("fn network_module() ="));
    assert!(render_init_module("demo").contains("  let model_name = :demo_model"));
    assert!(render_init_module("demo").contains("|> fragment :udp_packet_meta_fragment"));
    assert!(render_init_module("demo").contains("|> operation ${op_name}"));
    assert!(render_init_module("demo").contains("|> program_model ${model_name}"));
}

#[test]
fn init_normalizes_package_name_for_generated_template_ids() {
    assert_eq!(normalize_package_name("My Demo-App"), "my_demo_app");
    assert_eq!(normalize_package_name("2026 proto"), "gewy_2026_proto");
    assert_eq!(normalize_package_name("___"), "");
}

#[test]
fn initialize_package_creates_compilable_scaffold() {
    let root = temp_test_dir("init-compile").join("My Demo-App");
    initialize_package(root.to_str().unwrap()).unwrap();

    let envelope = compile_envelope_file(root.join("main.gewy").to_str().unwrap()).unwrap();
    assert_eq!(
        envelope
            .binding
            .as_ref()
            .map(|binding| binding.template_id.as_str()),
        Some("my_demo_app")
    );
    assert!(envelope.stages.parse.ok);
    assert!(envelope.stages.validation.ok);
    assert!(envelope.stages.diagnostics.ok);
}

#[test]
fn initialize_package_preserves_existing_files() {
    let root = temp_test_dir("init-preserve");
    let manifest = root.join("gewy.pkg");
    let entry = root.join("main.gewy");
    let module = root.join("module.gewy");
    fs::write(&manifest, "name=custom\nentry=main.gewy\n").unwrap();
    fs::write(&entry, "template(:custom)\n").unwrap();
    fs::write(
        &module,
        "fn custom() =\n  |> fragment(:udp_packet_meta_fragment)\n",
    )
    .unwrap();

    initialize_package(root.to_str().unwrap()).unwrap();

    assert_eq!(
        fs::read_to_string(&manifest).unwrap(),
        "name=custom\nentry=main.gewy\n"
    );
    assert_eq!(fs::read_to_string(&entry).unwrap(), "template(:custom)\n");
    assert_eq!(
        fs::read_to_string(&module).unwrap(),
        "fn custom() =\n  |> fragment(:udp_packet_meta_fragment)\n"
    );
}

#[test]
fn frontend_command_renders_pipeline_graph_summary() {
    let report = compile_frontend_report_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let text = render_frontend_report(&report, RenderFormat::Text);
    let json = render_frontend_report(&report, RenderFormat::Json);
    assert!(text.contains("kind=pipeline"));
    assert!(text.contains("function_nodes:"));
    assert!(text.contains("graph_nodes:"));
    assert!(text.contains("graph_edges:"));
    assert!(json.contains("\"surface_id\":\"gewyc.frontend\""));
    assert!(json.contains(
        "\"schema_hint\":{\"family\":\"gewyc\",\"surface\":\"frontend\",\"schema_version\":1}"
    ));
    assert!(json.contains("\"kind\":\"pipeline\""));
    assert!(json.contains("\"graph_edges\""));
}

#[test]
fn frontend_command_focuses_graph_section() {
    let report = compile_frontend_report_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let text =
        render_frontend_report_with_focus(&report, RenderFormat::Text, Some(FrontendFocus::Graph));
    let json =
        render_frontend_report_with_focus(&report, RenderFormat::Json, Some(FrontendFocus::Graph));
    assert!(text.contains("focus=graph"));
    assert!(text.contains("graph_nodes:"));
    assert!(text.contains("graph_edges:"));
    assert!(json.contains("\"focus\":\"graph\""));
    assert!(json.contains("\"focused_report\""));
}

#[test]
fn frontend_command_focuses_expansion_section() {
    let report = compile_frontend_report_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let text = render_frontend_report_with_focus(
        &report,
        RenderFormat::Text,
        Some(FrontendFocus::Expansion),
    );
    let json = render_frontend_report_with_focus(
        &report,
        RenderFormat::Json,
        Some(FrontendFocus::Expansion),
    );
    assert!(text.contains("focus=expansion"));
    assert!(text.contains("expansion_previews:"));
    assert!(json.contains("\"focus\":\"expansion\""));
    assert!(json.contains("\"expansion_previews\""));
}

#[test]
fn frontend_command_compact_text_stays_short() {
    let report = compile_frontend_report_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let text = render_frontend_report_with_options(&report, RenderFormat::Text, None, true);
    assert!(text.contains("kind=pipeline"));
    assert!(text.contains("includes="));
    assert!(!text.contains("function_nodes:"));
    assert!(!text.contains("graph_nodes:"));
}

#[test]
fn explain_command_renders_human_oriented_compiler_summary() {
    let report = compile_explain_report_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let text = render_explain_report(&report, RenderFormat::Text);
    let json = render_explain_report(&report, RenderFormat::Json);
    assert!(text.contains("surface=explain"));
    assert!(text.contains("frontend:"));
    assert!(text.contains("validation:"));
    assert!(json.contains("\"surface_id\":\"gewyc.explain\""));
    assert!(json.contains(
        "\"schema_hint\":{\"family\":\"gewyc\",\"surface\":\"explain\",\"schema_version\":1}"
    ));
    assert!(json.contains("\"summary\""));
    assert!(json.contains("\"findings\""));
}

#[test]
fn explain_command_focuses_validation_section() {
    let report = compile_explain_report_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let text = render_explain_report_with_focus(
        &report,
        RenderFormat::Text,
        Some(ExplainFocus::Validation),
    );
    let json = render_explain_report_with_focus(
        &report,
        RenderFormat::Json,
        Some(ExplainFocus::Validation),
    );
    assert!(text.contains("focus=validation"));
    assert!(text.contains("unsupported_payload_offsets="));
    assert!(json.contains("\"focus\":\"validation\""));
    assert!(json.contains("\"focused_report\""));
}

#[test]
fn explain_command_focuses_binding_section() {
    let report = compile_explain_report_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let text =
        render_explain_report_with_focus(&report, RenderFormat::Text, Some(ExplainFocus::Binding));
    let json =
        render_explain_report_with_focus(&report, RenderFormat::Json, Some(ExplainFocus::Binding));
    assert!(text.contains("focus=binding"));
    assert!(text.contains("lowered_binding_summary="));
    assert!(text.contains("binding_delta"));
    assert!(text.contains("binding_note="));
    assert!(json.contains("\"focus\":\"binding\""));
    assert!(json.contains("\"lowered_binding_summary\""));
    assert!(json.contains("\"frontend_lowering_delta\""));
    assert!(json.contains("\"binding_shape_note\""));
}

#[test]
fn explain_command_compact_text_stays_short() {
    let report = compile_explain_report_file(&dsl_fixture_path("udp_process_debug.gewy")).unwrap();
    let text = render_explain_report_with_options(&report, RenderFormat::Text, None, true);
    assert!(text.contains("surface=explain ok=true"));
    assert!(text.contains("template=udp_process_debug"));
    assert!(!text.contains("frontend:"));
    assert!(!text.contains("validation:"));
}

#[test]
fn lock_writes_resolved_dependency_and_source_entries() {
    let root = std::env::temp_dir().join(format!("gewy-lock-{}", std::process::id()));
    let app_dir = root.join("app");
    let registry_dir = root.join("registry");
    let dep_dir = registry_dir.join("udp_stdlib");
    std::fs::create_dir_all(&app_dir).unwrap();
    std::fs::create_dir_all(&dep_dir).unwrap();
    std::fs::write(
        app_dir.join("gewy.pkg"),
        format!(
            "name=lock_demo\nversion=0.1.0\nentry=main.gewy\nsource.local={}\ndep.std=source:local/udp_stdlib\n",
            registry_dir.to_string_lossy()
        ),
    )
    .unwrap();
    std::fs::write(app_dir.join("main.gewy"), "template(:lock_demo)\n").unwrap();

    let lock = gewyvern::dsl::build_lockfile(app_dir.to_str().unwrap()).unwrap();
    assert!(lock.contains("name=lock_demo"));
    assert!(lock.contains("source.local="));
    assert!(lock.contains("dep.std="));
}

#[test]
#[ignore = "benchmark"]
fn benchmark_gewyc_binding_report_udp_process_debug() {
    let path = dsl_fixture_path("udp_process_debug.gewy");
    let start = Instant::now();
    let mut total_rules = 0usize;
    for _ in 0..200 {
        let report = compile_binding_report_file(&path).unwrap();
        total_rules += report
            .program_model
            .as_ref()
            .map(|model| model.rules)
            .unwrap_or(0);
    }
    let elapsed = start.elapsed();
    assert!(total_rules > 0);
    eprintln!(
        "benchmark_gewyc_binding_report_udp_process_debug: iterations=200 total_rules={} elapsed_ms={:.3}",
        total_rules,
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_gewyc_frontend_report_udp_process_debug() {
    let path = dsl_fixture_path("udp_process_debug.gewy");
    let start = Instant::now();
    let mut total_functions = 0usize;
    for _ in 0..200 {
        let report = compile_frontend_report_file(&path).unwrap();
        total_functions += report.function_count;
    }
    let elapsed = start.elapsed();
    assert!(total_functions > 0);
    eprintln!(
        "benchmark_gewyc_frontend_report_udp_process_debug: iterations=200 total_functions={} elapsed_ms={:.3}",
        total_functions,
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_gewyc_explain_report_udp_process_debug() {
    let path = dsl_fixture_path("udp_process_debug.gewy");
    let start = Instant::now();
    let mut total_findings = 0usize;
    for _ in 0..100 {
        let report = compile_explain_report_file(&path).unwrap();
        total_findings += report.findings.findings.len();
    }
    let elapsed = start.elapsed();
    eprintln!(
        "benchmark_gewyc_explain_report_udp_process_debug: iterations=100 total_findings={} elapsed_ms={:.3}",
        total_findings,
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_gewyc_envelope_report_udp_process_debug() {
    let path = dsl_fixture_path("udp_process_debug.gewy");
    let start = Instant::now();
    let mut total_stage_count = 0usize;
    for _ in 0..100 {
        let report = compile_envelope_file(&path).unwrap();
        total_stage_count += report.stages.parse.ok as usize;
        total_stage_count += report.stages.validation.ok as usize;
        total_stage_count += report.stages.diagnostics.ok as usize;
    }
    let elapsed = start.elapsed();
    assert!(total_stage_count > 0);
    eprintln!(
        "benchmark_gewyc_envelope_report_udp_process_debug: iterations=100 stage_flags={} elapsed_ms={:.3}",
        total_stage_count,
        elapsed.as_secs_f64() * 1000.0
    );
}

#[test]
#[ignore = "benchmark"]
fn benchmark_gewyc_lockfile_protocol_publish_package() {
    let root = protocol_fixture_path("amqp/publish");
    let start = Instant::now();
    let mut total_len = 0usize;
    for _ in 0..100 {
        let lock = gewyvern::dsl::build_lockfile(&root).unwrap();
        total_len += lock.len();
    }
    let elapsed = start.elapsed();
    assert!(total_len > 0);
    eprintln!(
        "benchmark_gewyc_lockfile_protocol_publish_package: iterations=100 total_len={} elapsed_ms={:.3}",
        total_len,
        elapsed.as_secs_f64() * 1000.0
    );
}
