use std::process::Command;

fn gewyc(argument: &str) -> std::process::Output {
    Command::new(env!("CARGO_BIN_EXE_gewyc"))
        .arg(argument)
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .expect("gewyc should start")
}

#[test]
fn help_flags_write_usage_to_stdout_and_succeed() {
    for argument in ["-h", "--help"] {
        let output = gewyc(argument);
        assert!(output.status.success(), "{argument} should succeed");
        assert!(
            output.stderr.is_empty(),
            "{argument} should not write stderr"
        );
        let stdout = String::from_utf8(output.stdout).expect("help should be UTF-8");
        assert!(stdout.starts_with("usage: gewyc "));
        assert!(stdout.contains("gewyc init [dir]"));
    }
}

#[test]
fn version_flags_write_workspace_version_to_stdout_and_succeed() {
    for argument in ["-V", "--version"] {
        let output = gewyc(argument);
        assert!(output.status.success(), "{argument} should succeed");
        assert!(
            output.stderr.is_empty(),
            "{argument} should not write stderr"
        );
        assert_eq!(
            String::from_utf8(output.stdout)
                .expect("version should be UTF-8")
                .trim(),
            concat!("gewyc ", env!("CARGO_PKG_VERSION"))
        );
    }
}

#[test]
fn ir_command_emits_a_versioned_analysis_ir_surface() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("gewyc crate should live under crates/gewyc")
        .join("dsl/udp_process_debug.gewy");
    let output = Command::new(env!("CARGO_BIN_EXE_gewyc"))
        .arg("ir")
        .arg(path)
        .arg("--json")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .expect("gewyc ir should start");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("IR output should be UTF-8");
    assert!(stdout.contains("\"surface_id\":\"gewyc.ir\""));
    assert!(stdout.contains(
        "\"language_contract\":{\"language\":\"gewylang\",\"syntax_version\":1,\"stage\":\"analysis_ir\",\"stage_version\":1}"
    ));
}

#[test]
fn binding_command_emits_a_versioned_binding_ir_surface() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|path| path.parent())
        .expect("gewyc crate should live under crates/gewyc")
        .join("dsl/udp_process_debug.gewy");
    let output = Command::new(env!("CARGO_BIN_EXE_gewyc"))
        .arg("binding")
        .arg(path)
        .arg("--json")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .output()
        .expect("gewyc binding should start");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let stdout = String::from_utf8(output.stdout).expect("binding output should be UTF-8");
    assert!(stdout.contains("\"surface_id\":\"gewyc.binding\""));
    assert!(stdout.contains(
        "\"language_contract\":{\"language\":\"gewylang\",\"syntax_version\":1,\"stage\":\"binding_ir\",\"stage_version\":1}"
    ));
}
