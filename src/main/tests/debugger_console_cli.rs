use super::*;

#[test]
fn cli_accepts_local_debugger_console_mode() {
    let cli = Cli::from_args(["--scan-all".to_string(), "--debugger-console".to_string()])
        .expect("debugger console mode should parse");
    assert!(cli.scan_all);
    assert!(cli.debugger_console);
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
