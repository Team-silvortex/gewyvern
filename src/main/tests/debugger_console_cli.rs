use super::*;

#[test]
fn cli_accepts_local_debugger_console_mode() {
    let cli = Cli::from_args(["--scan-all".to_string(), "--debugger-console".to_string()])
        .expect("debugger console mode should parse");
    assert!(cli.scan_all);
    assert!(cli.debugger_console);
}

#[test]
fn cli_accepts_local_debug_session_mode() {
    let cli = Cli::from_args(["--scan-all".to_string(), "--debug-session".to_string()])
        .expect("debug session mode should parse");
    assert!(cli.scan_all);
    assert!(cli.debug_session);
}

#[test]
fn cli_rejects_debugger_console_with_other_output_modes() {
    let err = Cli::from_args([
        "--debugger-console".to_string(),
        "--http-transactions".to_string(),
    ])
    .unwrap_err();
    assert!(err.contains("--debugger-console"));
    assert!(err.contains("--http-transactions"));
}

#[test]
fn cli_rejects_overlapping_local_debugger_modes() {
    let err = Cli::from_args([
        "--debugger-console".to_string(),
        "--debug-session".to_string(),
    ])
    .unwrap_err();
    assert!(err.contains("--debugger-console"));
    assert!(err.contains("--debug-session"));
}

#[test]
fn local_debugger_console_renders_machine_and_human_views() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http request dsl should compile");
    let export = run_binding_demo(binding);
    let outputs = vec![("scan:http:request".to_string(), export)];

    let json_cli = Cli::from_args(["--debugger-console".to_string(), "--json".to_string()])
        .expect("json debugger console should parse");
    let json = render_debugger_console_outputs(&json_cli, &outputs);
    assert!(json.contains("\"surface\":\"local_debugger_console\""));
    assert!(json.contains("\"recommended_focus\":{\"name\":\"scan:http:request\""));
    assert!(json.contains("\"targets\":["));

    let text_cli =
        Cli::from_args(["--debugger-console".to_string()]).expect("text debugger console parses");
    let text = render_debugger_console_outputs(&text_cli, &outputs);
    assert!(text.contains("debugger_console: targets=1"));
    assert!(text.contains("recommended_focus: scan:http:request"));
}

#[test]
fn local_debug_session_renders_machine_and_human_views() {
    let binding = compile_file(&dsl_fixture_path("http_request_path.gewy"))
        .expect("http request dsl should compile");
    let export = run_binding_demo(binding);
    let outputs = vec![("scan:http:request".to_string(), export)];

    let json_cli = Cli::from_args(["--debug-session".to_string(), "--json".to_string()])
        .expect("json debug session should parse");
    let json = render_debug_session_outputs(&json_cli, &outputs);
    assert!(json.contains("\"surface\":\"local_debug_session\""));
    assert!(json.contains("\"recommended_focus\":{\"name\":\"scan:http:request\""));
    assert!(json.contains("\"failure_spine\":{"));
    assert!(json.contains("\"debugger_posture\":{"));
    assert!(json.contains("\"state\":\"healthy\""));
    assert!(json.contains("\"recommended_action\":\"observe_stable_baseline\""));
    assert!(json.contains("\"next_steps\":["));
    assert!(json.contains("\"kind\":\"read_protocol_plan\""));

    let text_cli =
        Cli::from_args(["--debug-session".to_string()]).expect("text debug session parses");
    let text = render_debug_session_outputs(&text_cli, &outputs);
    assert!(text.contains("debug_session: targets=1"));
    assert!(text.contains("focus: scan:http:request"));
    assert!(text.contains("posture=healthy"));
}
