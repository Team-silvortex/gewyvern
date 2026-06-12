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

mod core;
mod explain_compact;
mod explain_surface;
mod findings;
mod integration;
mod ir;
