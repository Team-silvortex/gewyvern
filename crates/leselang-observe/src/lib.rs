use leselang_command::CommandPlan;
use leselang_hir::Effect;
use leselang_ui::{
    DebuggerEffectKind, DebuggerFrame, DebuggerPendingEffect, DebuggerProjection, DebuggerState,
    MAX_RUNTIME_LOG_DISPLAY_BYTES, MAX_RUNTIME_LOG_ENTRIES, MAX_UI_TEXT_BYTES, RuntimeLogEntry,
    RuntimeLogLevel as UiRuntimeLogLevel, RuntimeLogProjection, UiError, debugger_document,
    runtime_log_document,
};
use leselang_vm::{
    ContinuationImage, DebuggerAuditContext, DebuggerCancelResult, EffectRequest, Fault, Vm,
    encode_continuation, validate_effect_request,
};
use leserpent_domain::{
    Command, CommandPlanError, Confirmation, MAX_RUNTIME_LOG_MESSAGE_BYTES, PlannedOperation,
    Revision, RuntimeId, RuntimeLogLevel, RuntimeLogRecord,
};
use serde::{Deserialize, Serialize};

const PENDING_FRAME_ID: &str = "pending-effect";
pub const MAX_RUNTIME_LOG_SOURCE_ENTRIES: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ObserveError {
    InvalidEffectRequest(Fault),
    RevisionMismatch {
        expected: Revision,
        actual: Revision,
    },
    LogSourceLimitExceeded,
    InvalidLogSequence,
    InvalidDebuggerPlan(CommandPlanError),
    DebuggerSessionMismatch,
    MissingDebuggerRevision,
    DebuggerConfirmationRequired,
    InvalidDebuggerContinuation(Fault),
    UnexpectedDebuggerStep,
    InvalidProjection(UiError),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DebuggerMutationStatus {
    Planned,
    Applied,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerMutationInspection {
    pub command_id: String,
    pub session_id: String,
    pub expected_revision: Revision,
    pub dry_run: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DebuggerMutationResult {
    pub inspection: DebuggerMutationInspection,
    pub status: DebuggerMutationStatus,
    pub state: DebuggerState,
    pub audited_at_ms: Option<u64>,
}

pub fn inspect_debugger_cancel_plan(
    plan: &CommandPlan,
    projection: &DebuggerProjection,
) -> Result<DebuggerMutationInspection, ObserveError> {
    plan.validate().map_err(ObserveError::InvalidDebuggerPlan)?;
    let PlannedOperation::Command(command) = &plan.operation else {
        return Err(ObserveError::DebuggerSessionMismatch);
    };
    let Command::DebuggerCancel { session_id } = &command.command else {
        return Err(ObserveError::DebuggerSessionMismatch);
    };
    if session_id != &projection.session_id || projection.state != DebuggerState::WaitingEffect {
        return Err(ObserveError::DebuggerSessionMismatch);
    }
    let Some(expected_revision) = command.expected_revision else {
        return Err(ObserveError::MissingDebuggerRevision);
    };
    if expected_revision != projection.revision {
        return Err(ObserveError::RevisionMismatch {
            expected: projection.revision,
            actual: expected_revision,
        });
    }
    if !command.dry_run && command.confirmation != Confirmation::Confirmed {
        return Err(ObserveError::DebuggerConfirmationRequired);
    }
    Ok(DebuggerMutationInspection {
        command_id: command.command_id.as_str().to_string(),
        session_id: session_id.clone(),
        expected_revision,
        dry_run: command.dry_run,
    })
}

pub fn execute_debugger_cancel(
    plan: &CommandPlan,
    projection: &DebuggerProjection,
    image: &ContinuationImage,
    vm: &mut Vm,
    now_ms: u64,
) -> Result<DebuggerMutationResult, ObserveError> {
    let inspection = inspect_debugger_cancel_plan(plan, projection)?;
    encode_continuation(image).map_err(ObserveError::InvalidDebuggerContinuation)?;
    if image.program_counter != projection.program_counter
        || image.fuel_remaining != projection.fuel_remaining
        || image
            .expected_revision
            .is_some_and(|revision| revision != projection.revision)
    {
        return Err(ObserveError::DebuggerSessionMismatch);
    }
    if inspection.dry_run {
        return Ok(DebuggerMutationResult {
            inspection,
            status: DebuggerMutationStatus::Planned,
            state: projection.state,
            audited_at_ms: None,
        });
    }

    let PlannedOperation::Command(command) = &plan.operation else {
        return Err(ObserveError::DebuggerSessionMismatch);
    };
    let audit = DebuggerAuditContext {
        command_id: command.command_id.clone(),
        idempotency_key: command.idempotency_key.clone(),
        principal: command.principal.clone(),
        origin: command.origin,
        session_id: inspection.session_id.clone(),
        expected_revision: inspection.expected_revision,
    };
    let record = vm
        .cancel_effect_audited(image, now_ms, &audit)
        .map_err(ObserveError::InvalidDebuggerContinuation)?;
    Ok(DebuggerMutationResult {
        inspection,
        status: DebuggerMutationStatus::Applied,
        state: DebuggerState::Cancelled,
        audited_at_ms: Some(record.observed_at_ms),
    })
}

pub fn execute_debugger_cancel_effect(
    plan: &CommandPlan,
    projection: &DebuggerProjection,
    image: &ContinuationImage,
    vm: &mut Vm,
    now_ms: u64,
) -> Result<DebuggerCancelResult, ObserveError> {
    let mutation = execute_debugger_cancel(plan, projection, image, vm, now_ms)?;
    if mutation.status != DebuggerMutationStatus::Applied {
        return Err(ObserveError::UnexpectedDebuggerStep);
    }
    let PlannedOperation::Command(command) = &plan.operation else {
        return Err(ObserveError::DebuggerSessionMismatch);
    };
    Ok(DebuggerCancelResult {
        command_id: command.command_id.clone(),
        session_id: mutation.inspection.session_id,
        observed_at_ms: mutation
            .audited_at_ms
            .ok_or(ObserveError::UnexpectedDebuggerStep)?,
    })
}

/// Converts one bounded authoritative log window into renderer-neutral state.
/// Source-specific fields cannot cross this deliberately narrow input type.
pub fn runtime_log_projection(
    revision: Revision,
    runtime_id: RuntimeId,
    runtime_name: &str,
    entries: &[RuntimeLogRecord],
) -> Result<RuntimeLogProjection, ObserveError> {
    if entries.len() > MAX_RUNTIME_LOG_SOURCE_ENTRIES
        || entries
            .iter()
            .any(|entry| entry.message.len() > MAX_RUNTIME_LOG_MESSAGE_BYTES)
    {
        return Err(ObserveError::LogSourceLimitExceeded);
    }
    if entries
        .windows(2)
        .any(|pair| pair[0].sequence >= pair[1].sequence)
    {
        return Err(ObserveError::InvalidLogSequence);
    }

    let start = entries.len().saturating_sub(MAX_RUNTIME_LOG_ENTRIES);
    let projected_entries = entries[start..]
        .iter()
        .map(|entry| RuntimeLogEntry {
            sequence: entry.sequence,
            level: match entry.level {
                RuntimeLogLevel::Trace => UiRuntimeLogLevel::Trace,
                RuntimeLogLevel::Debug => UiRuntimeLogLevel::Debug,
                RuntimeLogLevel::Info => UiRuntimeLogLevel::Info,
                RuntimeLogLevel::Warning => UiRuntimeLogLevel::Warning,
                RuntimeLogLevel::Error => UiRuntimeLogLevel::Error,
            },
            display: sanitize_display(&entry.message, MAX_RUNTIME_LOG_DISPLAY_BYTES),
        })
        .collect();
    let projection = RuntimeLogProjection {
        revision,
        runtime_id,
        runtime_name: sanitize_display(runtime_name, MAX_UI_TEXT_BYTES),
        entries: projected_entries,
    };
    runtime_log_document(&projection).map_err(ObserveError::InvalidProjection)?;
    Ok(projection)
}

fn sanitize_display(value: &str, max_bytes: usize) -> String {
    let mut output = String::with_capacity(value.len().min(max_bytes));
    let mut pending_space = false;
    for character in value.chars() {
        if character.is_control() {
            pending_space = !output.is_empty();
            continue;
        }
        if pending_space && output.len() < max_bytes {
            output.push(' ');
            pending_space = false;
        }
        if output.len() + character.len_utf8() > max_bytes {
            break;
        }
        output.push(character);
    }
    let trimmed = output.trim();
    if trimmed.is_empty() {
        "(empty log message)".to_string()
    } else if trimmed.len() == output.len() {
        output
    } else {
        trimmed.to_string()
    }
}

/// Builds the public debugger state for one suspended synchronous VM effect.
/// Authentication and continuation fields are intentionally never copied.
pub fn waiting_debugger_projection(
    request: &EffectRequest,
    session_id: impl Into<String>,
    revision: Revision,
    now_ms: u64,
) -> Result<DebuggerProjection, ObserveError> {
    validate_effect_request(request).map_err(ObserveError::InvalidEffectRequest)?;

    if let Some(expected) = request.continuation.expected_revision
        && expected != revision
    {
        return Err(ObserveError::RevisionMismatch {
            expected,
            actual: revision,
        });
    }

    let (kind, runtime_id, display) = match &request.continuation.pending_effect {
        Effect::RuntimeList { .. } => (DebuggerEffectKind::RuntimeList, None, "runtime list"),
        Effect::RuntimeInspect { runtime_id } => (
            DebuggerEffectKind::RuntimeInspect,
            Some(runtime_id.clone()),
            "runtime inspect",
        ),
        Effect::RuntimeHistory { runtime_id } => (
            DebuggerEffectKind::RuntimeHistory,
            Some(runtime_id.clone()),
            "runtime history",
        ),
        Effect::RuntimeLogs { runtime_id } => (
            DebuggerEffectKind::RuntimeLogs,
            Some(runtime_id.clone()),
            "runtime logs",
        ),
        Effect::RuntimeRefresh { runtime_id } => (
            DebuggerEffectKind::RuntimeRefresh,
            Some(runtime_id.clone()),
            "runtime refresh",
        ),
        Effect::RuntimeCapabilitiesRefresh { runtime_id } => (
            DebuggerEffectKind::RuntimeCapabilitiesRefresh,
            Some(runtime_id.clone()),
            "runtime capabilities refresh",
        ),
        Effect::RuntimeDeploy { runtime_id, .. } => (
            DebuggerEffectKind::RuntimeDeploy,
            Some(runtime_id.clone()),
            "runtime deploy",
        ),
        Effect::DebuggerCancel { .. } => {
            (DebuggerEffectKind::DebuggerCancel, None, "debugger cancel")
        }
        Effect::UiFocus { .. } => (DebuggerEffectKind::UiFocus, None, "UI focus"),
        Effect::UiNavigateFocus { .. } => (
            DebuggerEffectKind::UiNavigateFocus,
            None,
            "UI navigate focus",
        ),
        Effect::UiScrollIntoView { .. } => (
            DebuggerEffectKind::UiScrollIntoView,
            None,
            "UI scroll into view",
        ),
        Effect::UiAssertVisible { .. } => (
            DebuggerEffectKind::UiAssertVisible,
            None,
            "UI assert visible",
        ),
        Effect::UiAssertHidden { .. } => {
            (DebuggerEffectKind::UiAssertHidden, None, "UI assert hidden")
        }
        Effect::UiWaitHidden { .. } => (DebuggerEffectKind::UiWaitHidden, None, "UI wait hidden"),
        Effect::UiAssertRealized { .. } => (
            DebuggerEffectKind::UiAssertRealized,
            None,
            "UI assert realized",
        ),
        Effect::UiWaitRealized { .. } => {
            (DebuggerEffectKind::UiWaitRealized, None, "UI wait realized")
        }
        Effect::UiWaitVisible { .. } => {
            (DebuggerEffectKind::UiWaitVisible, None, "UI wait visible")
        }
        Effect::UiWaitEnabled { .. } => {
            (DebuggerEffectKind::UiWaitEnabled, None, "UI wait enabled")
        }
        Effect::UiWaitDisabled { .. } => {
            (DebuggerEffectKind::UiWaitDisabled, None, "UI wait disabled")
        }
        Effect::UiOpenWindow { .. } => (DebuggerEffectKind::UiOpenWindow, None, "UI open window"),
        Effect::UiCloseWindow { .. } => {
            (DebuggerEffectKind::UiCloseWindow, None, "UI close window")
        }
        Effect::UiAssertWindowOpen { .. } => (
            DebuggerEffectKind::UiAssertWindowOpen,
            None,
            "UI assert window open",
        ),
        Effect::UiWaitWindowOpen { .. } => (
            DebuggerEffectKind::UiWaitWindowOpen,
            None,
            "UI wait window open",
        ),
        Effect::UiAssertWindowClosed { .. } => (
            DebuggerEffectKind::UiAssertWindowClosed,
            None,
            "UI assert window closed",
        ),
        Effect::UiWaitWindowClosed { .. } => (
            DebuggerEffectKind::UiWaitWindowClosed,
            None,
            "UI wait window closed",
        ),
        Effect::UiWaitFocused { .. } => {
            (DebuggerEffectKind::UiWaitFocused, None, "UI wait focused")
        }
        Effect::UiAssertFocused { .. } => (
            DebuggerEffectKind::UiAssertFocused,
            None,
            "UI assert focused",
        ),
        Effect::UiWaitUnfocused { .. } => (
            DebuggerEffectKind::UiWaitUnfocused,
            None,
            "UI wait unfocused",
        ),
        Effect::UiAssertUnfocused { .. } => (
            DebuggerEffectKind::UiAssertUnfocused,
            None,
            "UI assert unfocused",
        ),
        Effect::UiAssertEnabled { .. } => (
            DebuggerEffectKind::UiAssertEnabled,
            None,
            "UI assert enabled",
        ),
        Effect::UiAssertDisabled { .. } => (
            DebuggerEffectKind::UiAssertDisabled,
            None,
            "UI assert disabled",
        ),
        Effect::UiAssertSelection { .. } => (
            DebuggerEffectKind::UiAssertSelection,
            None,
            "UI assert selection",
        ),
        Effect::UiWaitSelection { .. } => (
            DebuggerEffectKind::UiWaitSelection,
            None,
            "UI wait selection",
        ),
        Effect::UiAssertText { .. } => (DebuggerEffectKind::UiAssertText, None, "UI assert text"),
        Effect::UiWaitText { .. } => (DebuggerEffectKind::UiWaitText, None, "UI wait text"),
        Effect::UiAssertAutomationId { .. } => (
            DebuggerEffectKind::UiAssertAutomationId,
            None,
            "UI assert automation id",
        ),
        Effect::UiAssertNodeKind { .. } => (
            DebuggerEffectKind::UiAssertNodeKind,
            None,
            "UI assert node kind",
        ),
        Effect::UiWaitNodeKind { .. } => (
            DebuggerEffectKind::UiWaitNodeKind,
            None,
            "UI wait node kind",
        ),
        Effect::UiAssertActionKind { .. } => (
            DebuggerEffectKind::UiAssertActionKind,
            None,
            "UI assert action kind",
        ),
        Effect::UiWaitActionKind { .. } => (
            DebuggerEffectKind::UiWaitActionKind,
            None,
            "UI wait action kind",
        ),
        Effect::UiAssertActionLabel { .. } => (
            DebuggerEffectKind::UiAssertActionLabel,
            None,
            "UI assert action label",
        ),
        Effect::UiWaitActionLabel { .. } => (
            DebuggerEffectKind::UiWaitActionLabel,
            None,
            "UI wait action label",
        ),
        Effect::UiAssertActionAvailable { .. } => (
            DebuggerEffectKind::UiAssertActionAvailable,
            None,
            "UI assert action available",
        ),
        Effect::UiWaitActionAvailable { .. } => (
            DebuggerEffectKind::UiWaitActionAvailable,
            None,
            "UI wait action available",
        ),
        Effect::UiAssertActionUnavailableReason { .. } => (
            DebuggerEffectKind::UiAssertActionUnavailableReason,
            None,
            "UI assert action unavailable reason",
        ),
        Effect::UiWaitActionUnavailableReason { .. } => (
            DebuggerEffectKind::UiWaitActionUnavailableReason,
            None,
            "UI wait action unavailable reason",
        ),
        Effect::UiAssertFormField { .. } => (
            DebuggerEffectKind::UiAssertFormField,
            None,
            "UI assert form field",
        ),
        Effect::UiAssertFormFieldInputKind { .. } => (
            DebuggerEffectKind::UiAssertFormFieldInputKind,
            None,
            "UI assert form field input kind",
        ),
        Effect::UiAssertFormFieldRequired { .. } => (
            DebuggerEffectKind::UiAssertFormFieldRequired,
            None,
            "UI assert form field required",
        ),
        Effect::UiAssertFormFieldMaxLength { .. } => (
            DebuggerEffectKind::UiAssertFormFieldMaxLength,
            None,
            "UI assert form field max length",
        ),
        Effect::UiAssertFormFieldPlaceholder { .. } => (
            DebuggerEffectKind::UiAssertFormFieldPlaceholder,
            None,
            "UI assert form field placeholder",
        ),
        Effect::UiWaitFormField { .. } => (
            DebuggerEffectKind::UiWaitFormField,
            None,
            "UI wait form field",
        ),
        Effect::UiWaitFormFieldInputKind { .. } => (
            DebuggerEffectKind::UiWaitFormFieldInputKind,
            None,
            "UI wait form field input kind",
        ),
        Effect::UiWaitFormFieldRequired { .. } => (
            DebuggerEffectKind::UiWaitFormFieldRequired,
            None,
            "UI wait form field required",
        ),
        Effect::UiWaitFormFieldMaxLength { .. } => (
            DebuggerEffectKind::UiWaitFormFieldMaxLength,
            None,
            "UI wait form field max length",
        ),
        Effect::UiWaitFormFieldPlaceholder { .. } => (
            DebuggerEffectKind::UiWaitFormFieldPlaceholder,
            None,
            "UI wait form field placeholder",
        ),
        Effect::UiAssertAccessibleName { .. } => (
            DebuggerEffectKind::UiAssertAccessibleName,
            None,
            "UI assert accessible name",
        ),
        Effect::UiWaitAccessibleName { .. } => (
            DebuggerEffectKind::UiWaitAccessibleName,
            None,
            "UI wait accessible name",
        ),
        Effect::UiAssertAccessibleDescription { .. } => (
            DebuggerEffectKind::UiAssertAccessibleDescription,
            None,
            "UI assert accessible description",
        ),
        Effect::UiWaitAccessibleDescription { .. } => (
            DebuggerEffectKind::UiWaitAccessibleDescription,
            None,
            "UI wait accessible description",
        ),
        Effect::All { .. } => {
            return Err(ObserveError::InvalidEffectRequest(Fault {
                code: "LSO1001".to_string(),
                message: "structured effect cannot be suspended as one request".to_string(),
            }));
        }
    };
    let deadline_remaining_ms = request
        .continuation
        .deadline_at_ms
        .map(|deadline| deadline.saturating_sub(now_ms))
        .or(Some(request.continuation.deadline_ms));
    let projection = DebuggerProjection {
        revision,
        session_id: session_id.into(),
        state: DebuggerState::WaitingEffect,
        program_counter: request.continuation.program_counter,
        fuel_remaining: request.continuation.fuel_remaining,
        deadline_remaining_ms,
        pending_effect: Some(DebuggerPendingEffect {
            effect_id: request.effect_id.clone(),
            kind,
            runtime_id,
        }),
        frames: vec![DebuggerFrame {
            frame_id: PENDING_FRAME_ID.to_string(),
            instruction: request.continuation.program_counter,
            display: format!("await {display}"),
        }],
        fault: None,
    };
    debugger_document(&projection).map_err(ObserveError::InvalidProjection)?;
    Ok(projection)
}

#[cfg(test)]
mod tests {
    use leselang_command::{LoweringContext, LoweringError, plan_debugger_cancel};
    use leselang_hir::lower;
    use leselang_syntax::parse;
    use leselang_vm::{EffectOperation, Step, Vm};
    use leserpent_domain::{
        CAPABILITY_DEBUGGER_CONTROL, CAPABILITY_RUNTIME_READ, CapabilitySet, CommandId,
        CommandOrigin, Confirmation, IdempotencyKey, Principal, Query, Revision,
    };

    use super::*;

    fn inspect_request() -> EffectRequest {
        inspect_vm_and_request().1
    }

    fn logs_request() -> EffectRequest {
        let syntax = parse("fn logs() = runtime.logs(runtime_id: \"runtime-a\")");
        let program = lower(&syntax).expect("program lowers");
        match Vm::default().start_timed(
            &program,
            Principal {
                id: "debugger-principal".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            Some(Revision(7)),
            1_000,
            30_000,
        ) {
            Step::Effect(request) => *request,
            other => panic!("expected effect, got {other:?}"),
        }
    }

    fn inspect_vm_and_request() -> (Vm, EffectRequest) {
        let syntax = parse("fn inspect() = runtime.inspect(runtime_id: \"runtime-a\")");
        let program = lower(&syntax).expect("program lowers");
        let mut vm = Vm::default();
        let request = match vm.start_timed(
            &program,
            Principal {
                id: "debugger-principal".to_string(),
            },
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            Some(Revision(7)),
            1_000,
            30_000,
        ) {
            Step::Effect(request) => *request,
            other => panic!("expected effect, got {other:?}"),
        };
        (vm, request)
    }

    fn debugger_context(dry_run: bool, confirmation: Confirmation) -> LoweringContext {
        LoweringContext {
            principal: Principal {
                id: "debugger-operator".to_string(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_DEBUGGER_CONTROL]),
            expected_revision: Some(Revision(7)),
            command_id: CommandId::new("debugger-command-a").unwrap(),
            idempotency_key: IdempotencyKey::new("debugger-effect-a").unwrap(),
            origin: CommandOrigin::Gui,
            confirmation,
            dry_run,
        }
    }

    #[test]
    fn produces_a_valid_sanitized_waiting_projection() {
        let request = inspect_request();
        let projection =
            waiting_debugger_projection(&request, "session-a", Revision(7), 1_250).unwrap();

        assert_eq!(projection.state, DebuggerState::WaitingEffect);
        assert_eq!(projection.deadline_remaining_ms, Some(29_750));
        assert_eq!(
            projection.pending_effect.as_ref().unwrap().kind,
            DebuggerEffectKind::RuntimeInspect
        );
        assert_eq!(
            projection
                .pending_effect
                .as_ref()
                .unwrap()
                .runtime_id
                .as_ref()
                .unwrap()
                .as_str(),
            "runtime-a"
        );
        debugger_document(&projection).unwrap();

        let encoded = serde_json::to_string(&projection).unwrap();
        assert!(!encoded.contains(request.continuation.token.as_str()));
        assert!(!encoded.contains("debugger-principal"));
        assert!(!encoded.contains(CAPABILITY_RUNTIME_READ));
        assert!(!encoded.contains("leselang-effect-"));
        assert!(!encoded.contains("31000"));
    }

    #[test]
    fn projects_runtime_logs_as_a_typed_waiting_effect() {
        let projection =
            waiting_debugger_projection(&logs_request(), "session-logs", Revision(7), 1_250)
                .unwrap();
        let pending = projection.pending_effect.as_ref().unwrap();
        assert_eq!(pending.kind, DebuggerEffectKind::RuntimeLogs);
        assert_eq!(pending.runtime_id.as_ref().unwrap().as_str(), "runtime-a");
        assert_eq!(projection.frames[0].display, "await runtime logs");
        debugger_document(&projection).unwrap();
    }

    #[test]
    fn rejects_a_torn_control_plane_revision() {
        let error =
            waiting_debugger_projection(&inspect_request(), "session-a", Revision(8), 1_250)
                .unwrap_err();
        assert_eq!(
            error,
            ObserveError::RevisionMismatch {
                expected: Revision(7),
                actual: Revision(8),
            }
        );
    }

    #[test]
    fn rejects_a_forged_vm_request_before_projection() {
        let mut request = inspect_request();
        let EffectOperation::Query(query) = &mut request.operation else {
            panic!("inspect must be a query");
        };
        query.query = Query::RuntimeList {
            filter: Default::default(),
        };

        assert!(matches!(
            waiting_debugger_projection(&request, "session-a", Revision(7), 1_250),
            Err(ObserveError::InvalidEffectRequest(_))
        ));
    }

    #[test]
    fn produces_a_sanitized_newest_runtime_log_window() {
        let mut entries = (1..=300)
            .map(|sequence| RuntimeLogRecord {
                sequence,
                level: RuntimeLogLevel::Info,
                message: format!("event {sequence}"),
            })
            .collect::<Vec<_>>();
        entries[299].level = RuntimeLogLevel::Error;
        entries[299].message = format!("failed\nrequest\t{}", "火".repeat(400));

        let projection = runtime_log_projection(
            Revision(9),
            RuntimeId::new("runtime-a").unwrap(),
            "Runtime\nA",
            &entries,
        )
        .unwrap();

        assert_eq!(projection.entries.len(), MAX_RUNTIME_LOG_ENTRIES);
        assert_eq!(projection.entries.first().unwrap().sequence, 45);
        assert_eq!(projection.entries.last().unwrap().sequence, 300);
        assert_eq!(projection.runtime_name, "Runtime A");
        let last = projection.entries.last().unwrap();
        assert_eq!(last.level, UiRuntimeLogLevel::Error);
        assert!(last.display.starts_with("failed request "));
        assert!(last.display.len() <= MAX_RUNTIME_LOG_DISPLAY_BYTES);
        assert!(!last.display.chars().any(char::is_control));
        runtime_log_document(&projection).unwrap();

        let encoded = serde_json::to_string(&projection).unwrap();
        assert!(!encoded.contains("endpoint"));
        assert!(!encoded.contains("transport"));
    }

    #[test]
    fn rejects_reordered_or_duplicate_runtime_log_sequences() {
        let entries = vec![
            RuntimeLogRecord {
                sequence: 2,
                level: RuntimeLogLevel::Warning,
                message: "second".to_string(),
            },
            RuntimeLogRecord {
                sequence: 2,
                level: RuntimeLogLevel::Debug,
                message: "duplicate".to_string(),
            },
        ];

        assert_eq!(
            runtime_log_projection(
                Revision(1),
                RuntimeId::new("runtime-a").unwrap(),
                "Runtime A",
                &entries,
            ),
            Err(ObserveError::InvalidLogSequence)
        );
    }

    #[test]
    fn rejects_oversized_runtime_log_source_messages() {
        let entries = vec![RuntimeLogRecord {
            sequence: 1,
            level: RuntimeLogLevel::Trace,
            message: "x".repeat(MAX_RUNTIME_LOG_MESSAGE_BYTES + 1),
        }];

        assert_eq!(
            runtime_log_projection(
                Revision(1),
                RuntimeId::new("runtime-a").unwrap(),
                "Runtime A",
                &entries,
            ),
            Err(ObserveError::LogSourceLimitExceeded)
        );
    }

    #[test]
    fn debugger_cancel_dry_run_is_inspectable_and_non_mutating() {
        let (mut vm, request) = inspect_vm_and_request();
        let projection =
            waiting_debugger_projection(&request, "session-a", Revision(7), 1_250).unwrap();
        let plan = plan_debugger_cancel(
            "session-a",
            &debugger_context(true, Confirmation::NotRequired),
        )
        .unwrap();

        let result =
            execute_debugger_cancel(&plan, &projection, &request.continuation, &mut vm, 1_250)
                .unwrap();
        assert_eq!(result.status, DebuggerMutationStatus::Planned);
        assert_eq!(result.state, DebuggerState::WaitingEffect);
        assert_eq!(result.audited_at_ms, None);
        assert_eq!(vm.pending_count(), 1);
        assert_eq!(
            vm.debugger_audit(&CommandId::new("debugger-command-a").unwrap())
                .unwrap(),
            None
        );
        let encoded = serde_json::to_string(&result).unwrap();
        assert!(!encoded.contains(request.continuation.token.as_str()));
        assert!(!encoded.contains("debugger-effect-a"));
    }

    #[test]
    fn debugger_cancel_requires_capability_and_confirmation_then_applies() {
        let mut missing = debugger_context(false, Confirmation::Confirmed);
        missing.capabilities = CapabilitySet::default();
        assert_eq!(
            plan_debugger_cancel("session-a", &missing),
            Err(LoweringError::MissingCapability {
                capability: CAPABILITY_DEBUGGER_CONTROL,
            })
        );

        let (mut vm, request) = inspect_vm_and_request();
        let projection =
            waiting_debugger_projection(&request, "session-a", Revision(7), 1_250).unwrap();
        let unconfirmed = plan_debugger_cancel(
            "session-a",
            &debugger_context(false, Confirmation::NotRequired),
        )
        .unwrap();
        assert_eq!(
            execute_debugger_cancel(
                &unconfirmed,
                &projection,
                &request.continuation,
                &mut vm,
                1_250,
            ),
            Err(ObserveError::DebuggerConfirmationRequired)
        );
        assert_eq!(vm.pending_count(), 1);

        let confirmed = plan_debugger_cancel(
            "session-a",
            &debugger_context(false, Confirmation::Confirmed),
        )
        .unwrap();
        let result = execute_debugger_cancel(
            &confirmed,
            &projection,
            &request.continuation,
            &mut vm,
            1_250,
        )
        .unwrap();
        assert_eq!(result.status, DebuggerMutationStatus::Applied);
        assert_eq!(result.state, DebuggerState::Cancelled);
        assert_eq!(result.audited_at_ms, Some(1_250));
        assert_eq!(vm.pending_count(), 0);
        let command_id = CommandId::new("debugger-command-a").unwrap();
        let audit = vm.debugger_audit(&command_id).unwrap().unwrap();
        assert_eq!(audit.command_id, command_id);
        assert_eq!(audit.session_id, "session-a");
        assert_eq!(audit.observed_at_ms, 1_250);

        let replay = execute_debugger_cancel(
            &confirmed,
            &projection,
            &request.continuation,
            &mut vm,
            1_500,
        )
        .unwrap();
        assert_eq!(replay.audited_at_ms, Some(1_250));
        let encoded = serde_json::to_string(&replay).unwrap();
        assert!(!encoded.contains(request.continuation.token.as_str()));
        assert!(!encoded.contains("debugger-effect-a"));
        assert_eq!(
            execute_debugger_cancel_effect(
                &confirmed,
                &projection,
                &request.continuation,
                &mut vm,
                1_750,
            )
            .unwrap(),
            DebuggerCancelResult {
                command_id: CommandId::new("debugger-command-a").unwrap(),
                session_id: "session-a".into(),
                observed_at_ms: 1_250,
            }
        );
    }

    #[test]
    fn debugger_cancel_rejects_stale_or_rebound_sessions() {
        let (mut vm, request) = inspect_vm_and_request();
        let projection =
            waiting_debugger_projection(&request, "session-a", Revision(7), 1_250).unwrap();
        let rebound = plan_debugger_cancel(
            "session-b",
            &debugger_context(false, Confirmation::Confirmed),
        )
        .unwrap();
        assert_eq!(
            execute_debugger_cancel(&rebound, &projection, &request.continuation, &mut vm, 1_250,),
            Err(ObserveError::DebuggerSessionMismatch)
        );

        let mut stale_context = debugger_context(false, Confirmation::Confirmed);
        stale_context.expected_revision = Some(Revision(8));
        let stale = plan_debugger_cancel("session-a", &stale_context).unwrap();
        assert!(matches!(
            execute_debugger_cancel(&stale, &projection, &request.continuation, &mut vm, 1_250,),
            Err(ObserveError::RevisionMismatch { .. })
        ));
    }
}
