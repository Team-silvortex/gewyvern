use std::fs;
use std::path::PathBuf;

use leselang_ui::{
    DebuggerEffectKind, DebuggerFrame, DebuggerPendingEffect, DebuggerProjection, DebuggerState,
    UiDocument, UiPatch, debugger_document, diff,
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
        .expect("usage: render_debugger_conformance_fixture OUTPUT");
    let previous = debugger_document(&debugger(Revision(1), true, 0)).unwrap();
    let next = debugger_document(&debugger(Revision(2), false, 1)).unwrap();
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

fn debugger(revision: Revision, waiting: bool, frame_start: u32) -> DebuggerProjection {
    DebuggerProjection {
        revision,
        session_id: "session-a".into(),
        state: if waiting {
            DebuggerState::WaitingEffect
        } else {
            DebuggerState::Yielded
        },
        program_counter: if waiting { 7 } else { 8 },
        fuel_remaining: if waiting { 899 } else { 898 },
        deadline_remaining_ms: Some(5_000),
        pending_effect: waiting.then(|| DebuggerPendingEffect {
            effect_id: "effect-7".into(),
            kind: DebuggerEffectKind::RuntimeInspect,
            runtime_id: Some(RuntimeId::new("runtime-a").unwrap()),
        }),
        frames: (frame_start..frame_start + 40)
            .map(|instruction| DebuggerFrame {
                frame_id: format!("frame-{instruction}"),
                instruction,
                display: format!("logical frame {instruction}"),
            })
            .collect(),
        fault: None,
    }
}
