use super::*;

use leselang_hir::{computation::ScalarType, lower};
use leselang_syntax::parse;

// Frozen reference signatures, independent of the live host catalog.
macro_rules! continuation_signatures {
    ($($variant:ident => $source:literal),+ $(,)?) => {
        const SIGNATURES: &[(Type, &str)] = &[$((Type::$variant, $source)),+];

        fn source_for_type(ty: Type) -> Option<&'static str> {
            match ty {
                $(Type::$variant => Some($source)),+,
                Type::Scalar(_) | Type::Structured => None,
            }
        }
    };
}

continuation_signatures! {
    RuntimeList => r#"runtime.list(environment: "dev", cluster: "lab", role: "edge")"#,
    RuntimeInspect => r#"runtime.inspect(runtime_id: "runtime-a")"#,
    RuntimeHistory => r#"runtime.history(runtime_id: "runtime-a")"#,
    RuntimeLogs => r#"runtime.logs(runtime_id: "runtime-a")"#,
    RuntimeRefresh => r#"runtime.refresh(runtime_id: "runtime-a")"#,
    RuntimeCapabilitiesRefresh => r#"runtime.refresh_capabilities(runtime_id: "runtime-a")"#,
    RuntimeDeploy => r#"runtime.deploy(runtime_id: "runtime-a", pipeline_kind: "http/request", target: none)"#,
    DebuggerCancel => r#"debugger.cancel(session_id: "session-a")"#,
    UiActivate => r#"ui.activate(node_id: "node-a")"#,
    UiFocus => r#"ui.focus(node_id: "node-a")"#,
    UiNavigateFocus => r#"ui.navigate_focus(node_id: "node-a", direction: "next")"#,
    UiScrollIntoView => r#"ui.scroll_into_view(node_id: "node-a")"#,
    UiAssertVisible => r#"ui.assert_visible(node_id: "node-a")"#,
    UiAssertHidden => r#"ui.assert_hidden(node_id: "node-a")"#,
    UiWaitHidden => r#"ui.wait_hidden(node_id: "node-a")"#,
    UiAssertRealized => r#"ui.assert_realized(node_id: "node-a")"#,
    UiWaitRealized => r#"ui.wait_realized(node_id: "node-a")"#,
    UiWaitVisible => r#"ui.wait_visible(node_id: "node-a")"#,
    UiWaitEnabled => r#"ui.wait_enabled(node_id: "node-a")"#,
    UiWaitDisabled => r#"ui.wait_disabled(node_id: "node-a")"#,
    UiOpenWindow => r#"ui.open_window(node_id: "node-a")"#,
    UiCloseWindow => r#"ui.close_window(node_id: "node-a")"#,
    UiAssertWindowOpen => r#"ui.assert_window_open(node_id: "node-a")"#,
    UiWaitWindowOpen => r#"ui.wait_window_open(node_id: "node-a")"#,
    UiAssertWindowClosed => r#"ui.assert_window_closed(node_id: "node-a")"#,
    UiWaitWindowClosed => r#"ui.wait_window_closed(node_id: "node-a")"#,
    UiWaitFocused => r#"ui.wait_focused(node_id: "node-a")"#,
    UiAssertFocused => r#"ui.assert_focused(node_id: "node-a")"#,
    UiWaitUnfocused => r#"ui.wait_unfocused(node_id: "node-a")"#,
    UiAssertUnfocused => r#"ui.assert_unfocused(node_id: "node-a")"#,
    UiAssertEnabled => r#"ui.assert_enabled(node_id: "node-a")"#,
    UiAssertDisabled => r#"ui.assert_disabled(node_id: "node-a")"#,
    UiAssertChildCount => r#"ui.assert_child_count(node_id: "node-a", count: "3")"#,
    UiWaitChildCount => r#"ui.wait_child_count(node_id: "node-a", count: "3")"#,
    UiSetSelection => r#"ui.set_selection(node_id: "node-a", state: "selected")"#,
    UiAssertSelection => r#"ui.assert_selection(node_id: "node-a", state: "selected")"#,
    UiWaitSelection => r#"ui.wait_selection(node_id: "node-a", state: "selected")"#,
    UiAssertText => r#"ui.assert_text(node_id: "node-a", expected: "text-a")"#,
    UiWaitText => r#"ui.wait_text(node_id: "node-a", expected: "text-a")"#,
    UiAssertAutomationId => r#"ui.assert_automation_id(node_id: "node-a", expected: "automation-a")"#,
    UiWaitAutomationId => r#"ui.wait_automation_id(node_id: "node-a", expected: "automation-a")"#,
    UiAssertNodeKind => r#"ui.assert_node_kind(node_id: "node-a", kind: "runtime_card")"#,
    UiWaitNodeKind => r#"ui.wait_node_kind(node_id: "node-a", kind: "runtime_card")"#,
    UiAssertActionKind => r#"ui.assert_action_kind(node_id: "node-a", kind: "runtime_inspect")"#,
    UiWaitActionKind => r#"ui.wait_action_kind(node_id: "node-a", kind: "runtime_inspect")"#,
    UiAssertActionLabel => r#"ui.assert_action_label(node_id: "node-a", expected: "text-a")"#,
    UiWaitActionLabel => r#"ui.wait_action_label(node_id: "node-a", expected: "text-a")"#,
    UiAssertActionAvailable => r#"ui.assert_action_available(node_id: "node-a")"#,
    UiWaitActionAvailable => r#"ui.wait_action_available(node_id: "node-a")"#,
    UiAssertActionUnavailableReason => r#"ui.assert_action_unavailable_reason(node_id: "node-a", expected: none)"#,
    UiWaitActionUnavailableReason => r#"ui.wait_action_unavailable_reason(node_id: "node-a", expected: none)"#,
    UiSubmitForm => r#"ui.submit_form(node_id: "node-a")"#,
    UiCancelForm => r#"ui.cancel_form(node_id: "node-a")"#,
    UiSetFormValue => r#"ui.set_form_value(node_id: "node-a", field: "field-a", value: "value-a")"#,
    UiAssertFormValue => r#"ui.assert_form_value(node_id: "node-a", field: "field-a", expected: "value-a")"#,
    UiWaitFormValue => r#"ui.wait_form_value(node_id: "node-a", field: "field-a", expected: "value-a")"#,
    UiAssertFormField => r#"ui.assert_form_field(node_id: "node-a", field: "field-a", expected: "value-a")"#,
    UiWaitFormField => r#"ui.wait_form_field(node_id: "node-a", field: "field-a", expected: "value-a")"#,
    UiAssertFormFieldInputKind => r#"ui.assert_form_field_input_kind(node_id: "node-a", field: "field-a", kind: "trimmed_text")"#,
    UiWaitFormFieldInputKind => r#"ui.wait_form_field_input_kind(node_id: "node-a", field: "field-a", kind: "trimmed_text")"#,
    UiAssertFormFieldRequired => r#"ui.assert_form_field_required(node_id: "node-a", field: "field-a", state: "required")"#,
    UiWaitFormFieldRequired => r#"ui.wait_form_field_required(node_id: "node-a", field: "field-a", state: "required")"#,
    UiAssertFormFieldMaxLength => r#"ui.assert_form_field_max_length(node_id: "node-a", field: "field-a", max_length: "3")"#,
    UiWaitFormFieldMaxLength => r#"ui.wait_form_field_max_length(node_id: "node-a", field: "field-a", max_length: "3")"#,
    UiAssertFormFieldPlaceholder => r#"ui.assert_form_field_placeholder(node_id: "node-a", field: "field-a", expected: none)"#,
    UiWaitFormFieldPlaceholder => r#"ui.wait_form_field_placeholder(node_id: "node-a", field: "field-a", expected: none)"#,
    UiAssertAccessibleName => r#"ui.assert_accessible_name(node_id: "node-a", expected: "text-a")"#,
    UiWaitAccessibleName => r#"ui.wait_accessible_name(node_id: "node-a", expected: "text-a")"#,
    UiAssertAccessibleDescription => r#"ui.assert_accessible_description(node_id: "node-a", expected: "text-a")"#,
    UiWaitAccessibleDescription => r#"ui.wait_accessible_description(node_id: "node-a", expected: "text-a")"#,
}

pub(super) fn requests() -> Vec<EffectRequest> {
    assert_eq!(SIGNATURES.len(), 70);
    SIGNATURES
        .iter()
        .enumerate()
        .map(|(index, &(result_type, source))| {
            assert_eq!(source_for_type(result_type), Some(source));
            let program = lower(&parse(&format!("fn main() = {source}"))).unwrap();
            assert_eq!(program.function.result_type, result_type, "{source}");
            Vm::build_effect_request(
                &program.function.effect,
                result_type,
                index as u64 + 1,
                Principal::new("operator").unwrap(),
                CapabilitySet::new([
                    CAPABILITY_UI_PRESENTATION,
                    CAPABILITY_RUNTIME_READ,
                    CAPABILITY_RUNTIME_REFRESH,
                    CAPABILITY_RUNTIME_DEPLOY,
                    CAPABILITY_DEBUGGER_CONTROL,
                ]),
                Some(Revision(7)),
                30_000,
                None,
                21,
            )
            .unwrap()
        })
        .collect()
}

fn mismatch_fault() -> Fault {
    Fault {
        code: "LSV2016".into(),
        message: "continuation result type does not match its pending effect".into(),
    }
}

#[test]
fn continuation_signatures_keep_all_seventy_wire_and_restore_round_trips() {
    let mut images = Vm::default();
    let mut envelopes = Vm::default();
    for request in requests() {
        let image = &request.continuation;
        assert_eq!(validate_image(image), Ok(()));
        let bytes = encode_continuation(image).unwrap();
        assert_eq!(bytes, serde_json::to_vec(image).unwrap());
        assert_eq!(decode_continuation(&bytes).unwrap(), *image);
        let wire = serde_json::to_vec(&request).unwrap();
        assert_eq!(
            serde_json::from_slice::<EffectRequest>(&wire).unwrap(),
            request
        );
        images.restore(image.clone()).unwrap();
        envelopes.restore_request(request.clone()).unwrap();
        envelopes.restore_request(request).unwrap();
    }
    assert_eq!(images.pending_count(), 70);
    assert_eq!(envelopes.pending_count(), 70);
    assert_eq!(
        images.pending_continuations(),
        envelopes.pending_continuations()
    );
    assert_eq!(images.next_effect_id, 71);
    assert_eq!(envelopes.next_effect_id, 71);
    assert_eq!(images.completed_count(), 0);
}

#[test]
fn continuation_signatures_reject_every_cross_operation_and_non_host_result_type() {
    let types: Vec<_> = SIGNATURES
        .iter()
        .map(|&(ty, _)| ty)
        .chain([
            Type::Structured,
            Type::Scalar(ScalarType::Integer),
            Type::Scalar(ScalarType::Boolean),
            Type::Scalar(ScalarType::String),
            Type::Scalar(ScalarType::None),
            Type::Scalar(ScalarType::OptionalString),
            Type::Scalar(ScalarType::StringList),
        ])
        .collect();
    for request in requests() {
        for &ty in &types {
            let mut torn = request.continuation.clone();
            torn.result_type = ty;
            assert_eq!(
                validate_image(&torn),
                if ty == request.continuation.result_type {
                    Ok(())
                } else {
                    Err(mismatch_fault())
                },
                "{:?} as {ty:?}",
                request.continuation.result_type
            );
        }
    }
}

#[test]
fn continuation_type_refusals_do_not_mutate_live_state_or_advance_identity() {
    let mut vm = Vm::default();
    for request in requests() {
        vm.restore_request(request.clone()).unwrap();
        let before = vm.pending_continuations();
        let next_id = vm.next_effect_id;
        let mut torn = request.clone();
        torn.continuation.result_type = Type::Structured;
        let bytes = serde_json::to_vec(&torn.continuation).unwrap();
        assert_eq!(
            encode_continuation(&torn.continuation),
            Err(mismatch_fault())
        );
        assert_eq!(decode_continuation(&bytes), Err(mismatch_fault()));
        assert_eq!(validate_effect_request(&torn), Err(mismatch_fault()));
        let wire = serde_json::to_vec(&torn).unwrap();
        assert!(
            serde_json::from_slice::<EffectRequest>(&wire)
                .unwrap_err()
                .to_string()
                .contains(&mismatch_fault().message)
        );
        assert_eq!(vm.restore(torn.continuation.clone()), Err(mismatch_fault()));
        assert_eq!(vm.restore_request(torn), Err(mismatch_fault()));
        assert_eq!(vm.pending_continuations(), before);
        assert_eq!(vm.next_effect_id, next_id);
        assert_eq!(vm.completed_count(), 0);
        vm.restore_request(request).unwrap();
    }
}

#[test]
fn continuation_signature_faults_keep_frame_before_type_before_budget_order() {
    let mut image = requests().remove(0).continuation;
    image.schema_version = 99;
    image.token = ContinuationToken::from_wire("invalid token".into());
    image.program_counter = 2;
    image.result_type = Type::Structured;
    image.fuel_remaining = MAX_EXECUTION_FUEL + 1;
    assert_eq!(validate_image(&image).unwrap_err().code, "LSV2001");
    image.schema_version = CONTINUATION_SCHEMA_VERSION;
    let token_fault = validate_image(&image).unwrap_err();
    assert_eq!(token_fault.code, "LSV2006");
    assert_eq!(token_fault.message, "invalid continuation token");
    image.token = ContinuationToken::from_wire("continuation-1".into());
    assert_eq!(validate_image(&image).unwrap_err().code, "LSV2014");
    image.program_counter = 1;
    assert_eq!(validate_image(&image), Err(mismatch_fault()));
    image.result_type = Type::RuntimeList;
    assert_eq!(validate_image(&image).unwrap_err().code, "LSV2007");
    image.fuel_remaining = 20;
    assert_eq!(validate_image(&image), Ok(()));
}

#[test]
fn continuation_signature_lookup_does_not_admit_structured_or_compute_frames() {
    let mut image = requests().remove(0).continuation;
    let compute = lower(&parse("fn main() = 1")).unwrap().function.effect;
    for effect in [
        Effect::All { branches: vec![] },
        Effect::Sequence { steps: vec![] },
        compute,
    ] {
        image.pending_effect = effect;
        image.result_type = Type::Structured;
        image.fuel_remaining = MAX_EXECUTION_FUEL + 1;
        let expected = Fault {
            code: "LSV2015".into(),
            message: "structured control flow cannot be restored as a single continuation".into(),
        };
        assert_eq!(validate_image(&image), Err(expected.clone()));
        assert_eq!(encode_continuation(&image), Err(expected.clone()));
        assert_eq!(
            decode_continuation(&serde_json::to_vec(&image).unwrap()),
            Err(expected)
        );
    }
}

#[test]
fn continuation_signature_lookup_leaves_canonical_validation_at_the_restore_boundary() {
    let mut image = requests()
        .into_iter()
        .find(|request| request.continuation.result_type == Type::UiFocus)
        .unwrap()
        .continuation;
    image.pending_effect = Effect::UiFocus {
        node_id: String::new(),
    };
    image.token = ContinuationToken::from_wire("continuation-01".into());
    assert_eq!(validate_image(&image), Ok(()));
    image.result_type = Type::Structured;
    assert_eq!(validate_restorable_image(&image), Err(mismatch_fault()));
    image.result_type = Type::UiFocus;
    assert_eq!(
        validate_restorable_image(&image).unwrap_err(),
        invalid_continuation_sequence_fault()
    );
    image.token = ContinuationToken::from_wire("continuation-1".into());
    assert_eq!(validate_image(&image), Ok(()));
    let error = validate_restorable_image(&image).unwrap_err();
    assert_eq!(error.code, "LSV2017");
    assert_eq!(
        error.message,
        "continuation pending effect is not canonical"
    );
}

#[cfg(feature = "sqlite-journal")]
mod durable {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    struct TempJournal(PathBuf);

    impl TempJournal {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(1);
            let root = std::env::temp_dir().join(format!(
                "leselang-continuation-signatures-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir(&root).unwrap();
            Self(root)
        }
    }

    impl Drop for TempJournal {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn continuation_signatures_reopen_all_operations_and_refuse_tampered_journal_types() {
        let directory = TempJournal::new();
        let path = directory.0.join("effects.sqlite3");
        let mut vm = Vm::open_journal(&path, DEFAULT_FUEL).unwrap();
        let cases = requests();
        for request in &cases {
            vm.restore_request(request.clone()).unwrap();
        }
        let before = vm.pending_continuations();
        drop(vm);
        let mut reopened = Vm::open_journal(&path, DEFAULT_FUEL).unwrap();
        assert_eq!(reopened.pending_continuations(), before);
        assert_eq!(reopened.next_effect_id, 71);
        for request in &cases {
            reopened.restore_request(request.clone()).unwrap();
        }
        drop(reopened);

        for request in &cases {
            let connection = rusqlite::Connection::open(&path).unwrap();
            let token = request.continuation.token.as_str();
            let original: Vec<u8> = connection
                .query_row(
                    "SELECT image FROM vm_effects WHERE token = ?1",
                    [token],
                    |row| row.get(0),
                )
                .unwrap();
            let mut torn = request.continuation.clone();
            torn.result_type = Type::Structured;
            let corrupted = serde_json::to_vec(&torn).unwrap();
            assert_eq!(
                connection
                    .execute(
                        "UPDATE vm_effects SET image = ?2 WHERE token = ?1",
                        rusqlite::params![token, corrupted],
                    )
                    .unwrap(),
                1
            );
            drop(connection);
            assert_eq!(
                Vm::open_journal(&path, DEFAULT_FUEL).err().unwrap(),
                mismatch_fault()
            );

            let connection = rusqlite::Connection::open(&path).unwrap();
            let after: Vec<u8> = connection
                .query_row(
                    "SELECT image FROM vm_effects WHERE token = ?1",
                    [token],
                    |row| row.get(0),
                )
                .unwrap();
            assert_eq!(after, corrupted);
            connection
                .execute(
                    "UPDATE vm_effects SET image = ?2 WHERE token = ?1",
                    rusqlite::params![token, original],
                )
                .unwrap();
        }
        let recovered = Vm::open_journal(&path, DEFAULT_FUEL).unwrap();
        assert_eq!(recovered.pending_continuations(), before);
        assert_eq!(recovered.next_effect_id, 71);
        assert_eq!(recovered.completed_count(), 0);
    }
}
