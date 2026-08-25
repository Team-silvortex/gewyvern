use super::*;
use std::fs;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

#[cfg(target_family = "unix")]
fn write_test_script(body: &str) -> std::path::PathBuf {
    use std::os::unix::fs::PermissionsExt;

    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock should be after unix epoch")
        .as_nanos();
    let script_path = std::env::temp_dir().join(format!("external-analysis-test-{unique}.sh"));
    fs::write(&script_path, format!("#!/bin/sh\n{body}\n"))
        .expect("test script should be writable");
    let mut permissions = fs::metadata(&script_path)
        .expect("test script should exist")
        .permissions();
    permissions.set_mode(0o700);
    fs::set_permissions(&script_path, permissions).expect("test script should be executable");
    script_path
}

#[test]
fn read_capped_stream_rejects_oversized_output() {
    let result = read_capped_stream("abcdef".as_bytes(), 4, "stdout");
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("exceeded"));
}

#[test]
fn repeated_failures_open_temporary_circuit() {
    let _guard = test_guard();
    reset_external_fault_state();
    note_external_analysis_failure("engine-bin", "timeout");
    note_external_analysis_failure("engine-bin", "timeout");
    assert!(current_external_circuit_block("engine-bin").is_none());
    note_external_analysis_failure("engine-bin", "timeout");
    let reason = current_external_circuit_block("engine-bin")
        .expect("circuit should open after repeated failures");
    assert!(reason.contains("temporarily bypassed"));
    reset_external_fault_state();
}

#[cfg(target_family = "unix")]
#[test]
fn run_external_command_enforces_timeout() {
    let script_path = write_test_script("sleep 1\nprintf 'late\\n'");
    let result = run_external_command(
        Command::new(&script_path),
        None,
        Duration::from_millis(100),
        1024,
        1024,
    );
    let _ = fs::remove_file(&script_path);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("timed out"));
}

#[cfg(target_family = "unix")]
#[test]
fn run_external_command_timeout_still_fires_when_engine_never_reads_stdin() {
    let script_path = write_test_script("sleep 1\nprintf 'late\\n'");
    let payload = vec![b'x'; 1024 * 1024];
    let result = run_external_command(
        Command::new(&script_path),
        Some(&payload),
        Duration::from_millis(100),
        1024,
        1024,
    );
    let _ = fs::remove_file(&script_path);
    assert!(result.is_err());
    assert!(result.unwrap_err().contains("timed out"));
}

#[cfg(target_family = "unix")]
#[test]
fn query_external_capabilities_rejects_oversized_stdout() {
    let script_path = write_test_script(
        "if [ \"$1\" = \"protocol-capabilities\" ]; then\ndd if=/dev/zero bs=1024 count=1025 2>/dev/null | tr '\\000' 'x'\nfi",
    );
    let profile = query_external_capabilities(&ExternalAnalysisConfig {
        engine_bin: script_path.to_string_lossy().into_owned(),
        python_worker: None,
        python_bin: None,
    });
    let _ = fs::remove_file(&script_path);
    assert!(profile.is_none());
}

#[test]
fn validate_external_analysis_binary_rejects_path_search_names() {
    let err = validate_external_analysis_binary("python3")
        .expect_err("bare executable name should be rejected");
    assert!(err.contains("filesystem path"));
}

#[test]
fn validate_external_analysis_path_argument_rejects_control_characters() {
    let err = validate_external_analysis_path_argument("python\nbin")
        .expect_err("path arguments must reject control characters");
    assert!(err.contains("control"));
}
