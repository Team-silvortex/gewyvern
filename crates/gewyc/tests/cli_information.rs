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
