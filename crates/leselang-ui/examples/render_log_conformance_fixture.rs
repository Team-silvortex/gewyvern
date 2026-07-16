use std::fs;
use std::path::PathBuf;

use leselang_ui::{
    RuntimeLogEntry, RuntimeLogLevel, RuntimeLogProjection, UiDocument, UiPatch, diff,
    runtime_log_document,
};
use leserpent_domain::{Revision, RuntimeId};
use serde::Serialize;

#[derive(Serialize)]
struct Fixture<'a> {
    schema_version: u32,
    previous: &'a UiDocument,
    patch: &'a UiPatch,
    next: &'a UiDocument,
}

fn main() {
    let output = std::env::args_os()
        .nth(1)
        .map(PathBuf::from)
        .expect("usage: render_log_conformance_fixture OUTPUT");
    let previous = runtime_log_document(&logs(Revision(1), 0, 48)).unwrap();
    let next = runtime_log_document(&logs(Revision(2), 1, 48)).unwrap();
    let patch = diff(&previous, &next).unwrap();
    let bytes = serde_json::to_vec_pretty(&Fixture {
        schema_version: 1,
        previous: &previous,
        patch: &patch,
        next: &next,
    })
    .unwrap();
    if let Some(parent) = output.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(output, bytes).unwrap();
}

fn logs(revision: Revision, start: u64, count: u64) -> RuntimeLogProjection {
    RuntimeLogProjection {
        revision,
        runtime_id: RuntimeId::new("runtime-logs").unwrap(),
        runtime_name: "Log Runtime".into(),
        entries: (start..start + count)
            .map(|sequence| RuntimeLogEntry {
                sequence,
                level: match sequence % 5 {
                    0 => RuntimeLogLevel::Trace,
                    1 => RuntimeLogLevel::Debug,
                    2 => RuntimeLogLevel::Info,
                    3 => RuntimeLogLevel::Warning,
                    _ => RuntimeLogLevel::Error,
                },
                display: format!("sanitized event {sequence}"),
            })
            .collect(),
    }
}
