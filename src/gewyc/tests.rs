use super::*;
use std::io::Write;
use std::process::{Command, Stdio};

pub(super) fn assert_valid_json_document(json: &str) {
    let mut child = Command::new("python3")
        .args(["-c", "import json, sys; json.load(sys.stdin)"])
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(json.as_bytes())
        .unwrap();
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "invalid json: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub(super) fn dsl_fixture_path(name: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("dsl")
        .join(name)
        .to_string_lossy()
        .into_owned()
}

pub(super) fn protocol_fixture_path(relative: &str) -> String {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("protocols")
        .join(relative)
        .to_string_lossy()
        .into_owned()
}

mod contract;
mod core;
mod explain_compact;
mod explain_surface;
mod findings;
mod fixture_contract;
mod frontend_surface;
mod integration;
mod integration_validation;
mod ir;
