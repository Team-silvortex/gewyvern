use super::*;

#[test]
fn dsl_accepts_shorthand_parameterized_pipeline_function_units() {
    let binding = compile_str(
        r#"
fn udp_core(model_name, op_name) {
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> operation($op_name)
  |> program_model($model_name)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: $model_name, phase: :bind)
}

template(:pipeline_shorthand_fn_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, :pipeline_shorthand_fn_udp_model, :datagram_exchange)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_shorthand_fn_udp");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "pipeline_shorthand_fn_udp_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
}

#[test]
fn dsl_accepts_explicit_pipeline_parameter_kinds() {
    let binding = compile_str(
        r#"
fn udp_core(model_name: atom, op_name: atom = :datagram_exchange, dedupe_flag: bool = true, duration_ms: u64 = 5000) =>
  |> window(duration_ms: $duration_ms, lateness_ms: 200)
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> operation($op_name)
  |> program_model($model_name)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: $dedupe_flag, module: :pipeline_typed_fn_udp, phase: :bind)

template(:pipeline_typed_fn_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, :pipeline_typed_fn_udp_model)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_typed_fn_udp");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "pipeline_typed_fn_udp_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
}

#[test]
fn dsl_accepts_expression_style_pipeline_function_units() {
    let binding = compile_str(
        r#"
fn udp_core() =
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> operation(:datagram_exchange)
  |> program_model(:pipeline_expr_fn_udp_model)

template(:pipeline_expr_fn_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_expr_fn_udp");
    assert_eq!(
        binding.template.fragment_set,
        vec![
            "udp_packet_meta_fragment",
            "route_meta_fragment",
            "sock_lineage_fragment"
        ]
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "pipeline_expr_fn_udp_model"
    );
}

#[test]
fn dsl_accepts_parameterized_expression_style_pipeline_function_units() {
    let binding = compile_str(
        r#"
fn udp_core(model_name, op_name) =>
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> operation($op_name)
  |> program_model($model_name)

template(:pipeline_expr_param_fn_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, :pipeline_expr_param_fn_udp_model, :datagram_exchange)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_expr_param_fn_udp");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "pipeline_expr_param_fn_udp_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
}

#[test]
fn dsl_accepts_pipeline_function_local_let_bindings() {
    let binding = compile_str(
        r#"
fn udp_core() =
  let model_name = :pipeline_let_fn_udp_model
  let op_name = :datagram_exchange
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> operation($op_name)
  |> program_model($model_name)

template(:pipeline_let_fn_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_let_fn_udp");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "pipeline_let_fn_udp_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
}

#[test]
fn dsl_accepts_parameterized_pipeline_function_local_let_bindings() {
    let binding = compile_str(
        r#"
fn udp_core(model_name) {
  let op_name = :datagram_exchange
  let phase_module = $model_name
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> operation($op_name)
  |> program_model($model_name)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: $phase_module, phase: :bind)
}

template(:pipeline_param_let_fn_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, :pipeline_param_let_fn_udp_model)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_param_let_fn_udp");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "pipeline_param_let_fn_udp_model"
    );
}

#[test]
fn dsl_accepts_shorthand_parameterized_pipeline_function_local_let_bindings() {
    let binding = compile_str(
        r#"
fn udp_core(model_name) {
  let op_name = :datagram_exchange
  let phase_module = $model_name
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> operation($op_name)
  |> program_model($model_name)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: $phase_module, phase: :bind)
}

template(:pipeline_shorthand_param_let_fn_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, :pipeline_shorthand_param_let_fn_udp_model)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_shorthand_param_let_fn_udp");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "pipeline_shorthand_param_let_fn_udp_model"
    );
}

#[test]
fn dsl_accepts_nested_pipeline_function_use_units() {
    let binding = compile_str(
        r#"
fn udp_rules() {
  |> operation(:datagram_exchange)
  |> program_model(:pipeline_nested_fn_udp_model)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :pipeline_nested_fn_udp, phase: :bind)
}

fn udp_core() {
  |> fragment(:udp_packet_meta_fragment)
  |> fragment(:route_meta_fragment)
  |> fragment(:sock_lineage_fragment)
  |> use(:udp_rules)
}

template(:pipeline_nested_fn_udp)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core)
"#,
    )
    .unwrap();

    assert_eq!(binding.template.id, "pipeline_nested_fn_udp");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "pipeline_nested_fn_udp_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
}

#[test]
fn dsl_reports_unknown_pipeline_function_with_known_candidates() {
    let err = compile_str(
        r#"
fn udp_core() =>
  |> operation(:datagram_exchange)
  |> program_model(:udp_core_model)

template(:pipeline_unknown_fn)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_cor)
"#,
    )
    .expect_err("unknown function should fail");

    let message = format!("{:?}", err.root());
    assert!(message.contains("unknown pipeline function 'udp_cor'"));
    assert!(message.contains("Declared pipeline functions in this module: udp_core."));
}

#[test]
fn dsl_reports_unknown_pipeline_step_with_available_candidates() {
    let err = compile_str(
        r#"
template(:pipeline_unknown_step)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> operashun(:datagram_exchange)
"#,
    )
    .expect_err("unknown step should fail");

    let message = format!("{:?}", err.root());
    assert!(message.contains("unknown pipeline DSL step 'operashun'"));
    assert!(message.contains(
        "Available pipeline steps: template, window, reason, reason_model, fragment, operation, program_model, param, evidence, program_rule, reason_rule, include, use."
    ));
}

#[test]
fn dsl_reports_unknown_named_parameter_with_signature_context() {
    let err = compile_str(
        r#"
fn udp_core(model_name, op_name = :datagram_exchange) =>
  |> operation($op_name)
  |> program_model($model_name)

template(:pipeline_unknown_param)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, model_name: :udp_model, mode_name: :oops)
"#,
    )
    .expect_err("unknown named parameter should fail");

    let message = format!("{:?}", err.root());
    assert!(message.contains(
        "pipeline function call does not match udp_core(model_name, op_name = :datagram_exchange)"
    ));
    assert!(message.contains("unknown named parameter 'mode_name'"));
    assert!(message.contains(
        "Declared parameters for udp_core(model_name, op_name = :datagram_exchange): model_name, op_name."
    ));
}

#[test]
fn dsl_reports_unknown_placeholder_with_in_scope_names() {
    let err = compile_str(
        r#"
fn udp_core(model_name) {
  let op_name = :datagram_exchange
  |> operation($op_name)
  |> program_model($model_nam)
}

template(:pipeline_unknown_placeholder)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, :udp_model)
"#,
    )
    .expect_err("unknown placeholder should fail");

    let message = format!("{:?}", err.root());
    assert!(message.contains(
        "unknown pipeline placeholder '$model_nam' while expanding program_model while expanding udp_core(model_name)"
    ));
    assert!(message.contains(
        "Names in scope for program_model while expanding udp_core(model_name): model_name, op_name."
    ));
}

#[test]
fn dsl_rejects_pipeline_parameter_kind_annotation_that_conflicts_with_usage() {
    let err = compile_str(
        r#"
fn udp_core(model_name: bool) =>
  |> operation(:datagram_exchange)
  |> program_model($model_name)

template(:pipeline_kind_conflict)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, true)
"#,
    )
    .expect_err("conflicting explicit kind should fail");

    let message = format!("{:?}", err.root());
    assert!(message.contains(
        "pipeline parameter 'model_name' in udp_core(model_name: bool) declares kind 'bool' but is used like 'atom'"
    ));
}

#[test]
fn dsl_reports_inconsistent_pipeline_parameter_inference_with_signature_context() {
    let err = compile_str(
        r#"
fn udp_core(shared_value) =>
  |> program_model($shared_value)
  |> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: $shared_value, module: :frontend_summary, phase: :bind)

template(:pipeline_inferred_kind_conflict)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> use(:udp_core, :udp_model)
"#,
    )
    .expect_err("inconsistent inferred kind should fail");

    let message = format!("{:?}", err.root());
    assert!(message.contains(
        "pipeline parameter 'shared_value' in udp_core(shared_value) is inferred inconsistently as both atom and bool"
    ));
}

#[test]
fn dsl_package_entry_compiles_from_manifest_directory_and_merges_pipeline_includes() {
    let package_dir =
        std::env::temp_dir().join(format!("gewy-package-{}-manifest-dir", std::process::id()));
    fs::create_dir_all(&package_dir).unwrap();
    fs::write(
        package_dir.join("gewy.pkg"),
        "name=package_udp_debug\nversion=0.1.0\nentry=main.gewy\n",
    )
    .unwrap();
    fs::write(
        package_dir.join("main.gewy"),
        r#"
template(:package_udp_debug)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("./partials.gewy")
"#,
    )
    .unwrap();
    fs::write(
        package_dir.join("partials.gewy"),
        r#"
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_model(:package_udp_debug_model)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :package_udp_debug, phase: :bind)
|> param(:sock_lineage_fragment.capture_comm, true)
"#,
    )
    .unwrap();

    let binding = compile_file(package_dir.to_str().unwrap()).unwrap();
    assert_eq!(binding.template.id, "package_udp_debug");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "package_udp_debug_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
    assert_eq!(
        binding.fragment_params["sock_lineage_fragment"]["capture_comm"],
        FragmentParamValue::Bool(true)
    );
}

#[test]
fn dsl_package_entry_can_include_pipeline_module_from_local_dependency() {
    let root = std::env::temp_dir().join(format!("gewy-package-{}-deps", std::process::id()));
    let app_dir = root.join("app");
    let dep_dir = root.join("udp_stdlib");
    fs::create_dir_all(&app_dir).unwrap();
    fs::create_dir_all(&dep_dir).unwrap();

    fs::write(
        app_dir.join("gewy.pkg"),
        format!(
            "name=app_with_dep\nversion=0.1.0\nentry=main.gewy\ndep.std={}\n",
            dep_dir.to_string_lossy()
        ),
    )
    .unwrap();
    fs::write(
        app_dir.join("main.gewy"),
        r#"
template(:app_with_dep)
|> window(:default_5s)
|> reason(:udp_datagram_l1)
|> include("std:udp_module.gewy")
"#,
    )
    .unwrap();
    fs::write(
        dep_dir.join("udp_module.gewy"),
        r#"
|> fragment(:udp_packet_meta_fragment)
|> fragment(:route_meta_fragment)
|> fragment(:sock_lineage_fragment)
|> operation(:datagram_exchange)
|> program_model(:app_with_dep_model)
|> program_rule(predicate: :process_bound, stage: :process_bound, narrative: :process_bound, dedupe: true, module: :app_with_dep, phase: :bind)
"#,
    )
    .unwrap();

    let binding = compile_file(app_dir.to_str().unwrap()).unwrap();
    assert_eq!(binding.template.id, "app_with_dep");
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().id,
        "app_with_dep_model"
    );
    assert_eq!(
        binding.template.program_model.as_ref().unwrap().operation,
        ProgramOperation::DatagramExchange
    );
}
