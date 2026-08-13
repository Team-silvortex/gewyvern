use super::Cli;

#[test]
fn cli_rejects_external_engine_bin_that_looks_like_next_flag() {
    let err =
        Cli::from_args(["--external-engine-bin".to_string(), "--json".to_string()]).unwrap_err();
    assert!(err.contains("--external-engine-bin"));
    assert!(err.contains("non-empty path"));
}

#[test]
fn cli_rejects_blank_external_engine_worker_path() {
    let err = Cli::from_args([
        "--external-engine-bin".to_string(),
        "/opt/gewy/external-engine".to_string(),
        "--external-engine-worker".to_string(),
        "   ".to_string(),
    ])
    .unwrap_err();
    assert!(err.contains("--external-engine-worker"));
    assert!(err.contains("non-empty path"));
}

#[test]
fn cli_accepts_explicit_external_engine_paths() {
    let cli = Cli::from_args([
        "--external-engine-bin".to_string(),
        "/opt/gewy/external-engine".to_string(),
        "--external-engine-worker".to_string(),
        "/opt/gewy/worker.py".to_string(),
        "--external-engine-python-bin".to_string(),
        "/usr/bin/python3".to_string(),
    ])
    .unwrap();
    let config = cli.external_analysis_config().expect("external config");
    assert_eq!(config.engine_bin, "/opt/gewy/external-engine");
    assert_eq!(config.python_worker.as_deref(), Some("/opt/gewy/worker.py"));
    assert_eq!(config.python_bin.as_deref(), Some("/usr/bin/python3"));
}

#[test]
fn cli_rejects_external_engine_path_without_separator() {
    let err = Cli::from_args(["--external-engine-bin".to_string(), "python3".to_string()])
        .unwrap_err();
    assert!(err.contains("filesystem path"));
}
