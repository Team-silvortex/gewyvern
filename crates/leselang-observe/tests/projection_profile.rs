use leselang_command::reference_request::validate_effect_request;
use leselang_command::{
    ContinuationImage, ContinuationToken, EffectOperation, EffectRequest, LoweringContext,
    plan_debugger_cancel,
};
use leselang_observe::{
    DebuggerMutationInspection, DebuggerMutationResult, DebuggerMutationStatus,
    MAX_RUNTIME_LOG_SOURCE_ENTRIES, ObserveError, inspect_debugger_cancel_plan,
    runtime_log_projection, waiting_debugger_projection,
};
use leselang_ui::{
    DebuggerEffectKind, DebuggerState, MAX_RUNTIME_LOG_DISPLAY_BYTES, MAX_RUNTIME_LOG_ENTRIES,
    debugger_document, runtime_log_document,
};
use leserpent_domain::{
    CAPABILITY_DEBUGGER_CONTROL, CAPABILITY_RUNTIME_READ, CapabilitySet, CommandId, CommandOrigin,
    Confirmation, DOMAIN_SCHEMA_VERSION, IdempotencyKey, MAX_RUNTIME_LOG_MESSAGE_BYTES,
    MAX_RUNTIME_LOG_QUERY_ENTRIES, PlannedOperation, Principal, Query, QueryEnvelope, Revision,
    RuntimeId, RuntimeLogLevel, RuntimeLogRecord,
};
use leserpent_hir::{Effect, Type};

// Received protocol data, deliberately constructed without starting or linking a VM.
fn request(logs: bool) -> EffectRequest {
    let runtime_id = RuntimeId::new("runtime-a").unwrap();
    let continuation = ContinuationImage {
        schema_version: 1,
        token: ContinuationToken::from_wire("continuation-7".into()),
        program_counter: 1,
        expected_revision: Some(Revision(7)),
        result_type: if logs {
            Type::RuntimeLogs
        } else {
            Type::RuntimeInspect
        },
        pending_effect: if logs {
            Effect::RuntimeLogs {
                runtime_id: runtime_id.clone(),
            }
        } else {
            Effect::RuntimeInspect {
                runtime_id: runtime_id.clone(),
            }
        },
        fuel_remaining: 99,
        deadline_ms: 30_000,
        deadline_at_ms: Some(31_000),
        max_output_items: 10_000,
        result_binding: None,
        group_result: None,
    };
    EffectRequest {
        effect_id: "effect-7".into(),
        required_capability: CAPABILITY_RUNTIME_READ.into(),
        operation: EffectOperation::Query(QueryEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            principal: Principal::new("private-principal").unwrap(),
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            query: if logs {
                Query::RuntimeLogs {
                    runtime_id,
                    after_sequence: None,
                    limit: MAX_RUNTIME_LOG_QUERY_ENTRIES,
                }
            } else {
                Query::RuntimeInspect { runtime_id }
            },
        }),
        budget: continuation.resource_budget(),
        continuation,
    }
}

fn context(dry_run: bool, confirmation: Confirmation) -> LoweringContext {
    LoweringContext {
        principal: Principal::new("operator").unwrap(),
        capabilities: CapabilitySet::new([CAPABILITY_DEBUGGER_CONTROL]),
        expected_revision: Some(Revision(8)),
        command_id: CommandId::new("cancel-a").unwrap(),
        idempotency_key: IdempotencyKey::new("private-replay-key").unwrap(),
        origin: CommandOrigin::Gui,
        confirmation,
        dry_run,
    }
}

#[test]
fn received_requests_project_without_an_evaluator_or_private_fields() {
    for logs in [false, true] {
        let request = request(logs);
        let bytes = serde_json::to_vec(&request).unwrap();
        let decoded: EffectRequest = serde_json::from_slice(&bytes).unwrap();
        for now in [0, 1_250, 31_000, u64::MAX] {
            let projection =
                waiting_debugger_projection(&decoded, "session-a", Revision(8), now).unwrap();
            assert_eq!(projection.revision, Revision(8));
            assert_eq!(projection.state, DebuggerState::WaitingEffect);
            assert_eq!(projection.program_counter, 1);
            assert_eq!(projection.fuel_remaining, 99);
            assert_eq!(
                projection.deadline_remaining_ms,
                Some(31_000u64.saturating_sub(now))
            );
            let pending = projection.pending_effect.as_ref().unwrap();
            assert_eq!(pending.effect_id, "effect-7");
            assert_eq!(pending.runtime_id.as_ref().unwrap().as_str(), "runtime-a");
            assert_eq!(
                pending.kind,
                if logs {
                    DebuggerEffectKind::RuntimeLogs
                } else {
                    DebuggerEffectKind::RuntimeInspect
                }
            );
            assert_eq!(
                projection.frames[0].display,
                if logs {
                    "await runtime logs"
                } else {
                    "await runtime inspect"
                }
            );
            debugger_document(&projection).unwrap();
            let wire = serde_json::to_string(&projection).unwrap();
            for private in [
                "private-principal",
                "continuation-7",
                CAPABILITY_RUNTIME_READ,
                "deadline_at_ms",
                "result_binding",
            ] {
                assert!(!wire.contains(private), "{private}");
            }
        }
        assert_eq!(serde_json::to_vec(&decoded).unwrap(), bytes);
        let mut untimed = decoded;
        untimed.continuation.deadline_at_ms = None;
        untimed.budget = untimed.continuation.resource_budget();
        let projection =
            waiting_debugger_projection(&untimed, "session-a", Revision(8), u64::MAX).unwrap();
        assert_eq!(projection.deadline_remaining_ms, Some(30_000));
    }
}

#[test]
fn projection_rechecks_mutable_data_in_the_original_admission_order() {
    let original = request(false);
    for case in 0..6 {
        let mut request = original.clone();
        match case {
            0 => {
                request.continuation.schema_version = 99;
                request.effect_id = "wrong".into();
            }
            1 => {
                request.continuation.token = ContinuationToken::from_wire("continuation-007".into())
            }
            2 => request.budget.fuel_remaining = 0,
            3 => request.required_capability = "wrong".into(),
            4 => {
                let EffectOperation::Query(query) = &mut request.operation else {
                    unreachable!()
                };
                query.query = Query::RuntimeList {
                    filter: Default::default(),
                };
            }
            5 => request.continuation.result_type = Type::Structured,
            _ => unreachable!(),
        }
        let before = serde_json::to_vec(&request).unwrap();
        let fault = validate_effect_request(&request).unwrap_err();
        assert_eq!(
            waiting_debugger_projection(&request, "", Revision(8), 1_250),
            Err(ObserveError::InvalidEffectRequest(fault))
        );
        assert_eq!(serde_json::to_vec(&request).unwrap(), before);
    }
    assert!(matches!(
        waiting_debugger_projection(&original, "", Revision(8), 1_250),
        Err(ObserveError::InvalidProjection(_))
    ));
}

#[test]
fn plan_inspection_retains_revision_session_and_confirmation_checks_without_mutation() {
    let projection =
        waiting_debugger_projection(&request(false), "session-a", Revision(8), 1_250).unwrap();
    for dry_run in [false, true] {
        for confirmation in [Confirmation::NotRequired, Confirmation::Confirmed] {
            let plan = plan_debugger_cancel("session-a", &context(dry_run, confirmation)).unwrap();
            let before = serde_json::to_vec(&plan).unwrap();
            let result = inspect_debugger_cancel_plan(&plan, &projection);
            if !dry_run && confirmation != Confirmation::Confirmed {
                assert_eq!(result, Err(ObserveError::DebuggerConfirmationRequired));
            } else {
                assert_eq!(
                    result.unwrap(),
                    DebuggerMutationInspection {
                        command_id: "cancel-a".into(),
                        session_id: "session-a".into(),
                        expected_revision: Revision(8),
                        dry_run,
                    }
                );
            }
            assert_eq!(serde_json::to_vec(&plan).unwrap(), before);
        }
    }
    let plan = plan_debugger_cancel("session-a", &context(false, Confirmation::Confirmed)).unwrap();
    let mut wrong = projection.clone();
    wrong.session_id = "session-b".into();
    assert_eq!(
        inspect_debugger_cancel_plan(&plan, &wrong),
        Err(ObserveError::DebuggerSessionMismatch)
    );
    wrong = projection.clone();
    wrong.state = DebuggerState::Cancelled;
    assert_eq!(
        inspect_debugger_cancel_plan(&plan, &wrong),
        Err(ObserveError::DebuggerSessionMismatch)
    );
    let mut stale = plan.clone();
    let PlannedOperation::Command(command) = &mut stale.operation else {
        unreachable!()
    };
    command.expected_revision = Some(Revision(9));
    assert_eq!(
        inspect_debugger_cancel_plan(&stale, &projection),
        Err(ObserveError::RevisionMismatch {
            expected: Revision(8),
            actual: Revision(9)
        })
    );
    let PlannedOperation::Command(command) = &mut stale.operation else {
        unreachable!()
    };
    command.expected_revision = None;
    assert_eq!(
        inspect_debugger_cancel_plan(&stale, &projection),
        Err(ObserveError::MissingDebuggerRevision)
    );
    stale.schema_version = u32::MAX;
    assert!(matches!(
        inspect_debugger_cancel_plan(&stale, &wrong),
        Err(ObserveError::InvalidDebuggerPlan(_))
    ));
}

#[test]
fn logs_keep_bounded_ordered_sanitized_windows_without_runtime_storage() {
    let runtime_id = RuntimeId::new("runtime-a").unwrap();
    let project = |entries: &[RuntimeLogRecord]| {
        runtime_log_projection(Revision(8), runtime_id.clone(), "Runtime\nA", entries)
    };
    let mut entries: Vec<_> = (1..=300)
        .map(|sequence| RuntimeLogRecord {
            sequence,
            level: RuntimeLogLevel::Info,
            message: format!("event {sequence}"),
        })
        .collect();
    entries.last_mut().unwrap().message = format!("failed\nrequest\t{}", "\u{706b}".repeat(400));
    let projection = project(&entries).unwrap();
    assert_eq!(projection.entries.len(), MAX_RUNTIME_LOG_ENTRIES);
    assert_eq!(projection.entries[0].sequence, 45);
    assert_eq!(projection.runtime_name, "Runtime A");
    let last = projection.entries.last().unwrap();
    assert!(last.display.starts_with("failed request "));
    assert!(last.display.len() <= MAX_RUNTIME_LOG_DISPLAY_BYTES);
    assert!(!last.display.chars().any(char::is_control));
    runtime_log_document(&projection).unwrap();
    entries[1].sequence = 1;
    assert_eq!(project(&entries), Err(ObserveError::InvalidLogSequence));
    entries[0].message = "x".repeat(MAX_RUNTIME_LOG_MESSAGE_BYTES + 1);
    assert_eq!(project(&entries), Err(ObserveError::LogSourceLimitExceeded));
    let oversized = vec![entries[2].clone(); MAX_RUNTIME_LOG_SOURCE_ENTRIES + 1];
    assert_eq!(
        project(&oversized),
        Err(ObserveError::LogSourceLimitExceeded)
    );
}

#[test]
fn mutation_observation_dtos_remain_available_and_closed_without_the_executor() {
    let data = DebuggerMutationResult {
        inspection: DebuggerMutationInspection {
            command_id: "cancel-a".into(),
            session_id: "session-a".into(),
            expected_revision: Revision(8),
            dry_run: false,
        },
        status: DebuggerMutationStatus::Applied,
        state: DebuggerState::Cancelled,
        audited_at_ms: Some(1_250),
    };
    let bytes = serde_json::to_vec(&data).unwrap();
    assert_eq!(
        serde_json::from_slice::<DebuggerMutationResult>(&bytes).unwrap(),
        data
    );
    let mut unknown = serde_json::to_value(&data).unwrap();
    unknown["execution_grant"] = true.into();
    assert!(serde_json::from_value::<DebuggerMutationResult>(unknown).is_err());
    let mut unknown = serde_json::to_value(&data).unwrap();
    unknown["inspection"]["execution_grant"] = true.into();
    assert!(serde_json::from_value::<DebuggerMutationResult>(unknown).is_err());
}

#[cfg(feature = "vm-bridge")]
mod bridge {
    use super::*;
    use leselang_observe::{execute_debugger_cancel, execute_debugger_cancel_effect};
    use leserpent_vm::{Step, Vm, encode_continuation};

    fn live() -> (Vm, EffectRequest) {
        let program = leserpent_hir::lower(&leselang_syntax::parse(
            "fn main() = runtime.inspect(runtime_id: \"runtime-a\")",
        ))
        .unwrap();
        let mut vm = Vm::default();
        let Step::Effect(request) = vm.start_timed(
            &program,
            Principal::new("private-principal").unwrap(),
            CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            Some(Revision(7)),
            1_000,
            30_000,
        ) else {
            panic!("expected suspended request")
        };
        (vm, *request)
    }

    #[test]
    fn dry_run_keeps_original_image_encoding_checks_and_never_writes_audit() {
        let (mut vm, request) = live();
        let projection =
            waiting_debugger_projection(&request, "session-a", Revision(8), 1_250).unwrap();
        let plan =
            plan_debugger_cancel("session-a", &context(true, Confirmation::NotRequired)).unwrap();
        for case in 0..3 {
            let mut image = request.continuation.clone();
            match case {
                0 => image.schema_version = 99,
                1 => image.token = ContinuationToken::from_wire("bad/token".into()),
                2 => image.deadline_at_ms = Some(u64::MAX),
                _ => unreachable!(),
            }
            let error = encode_continuation(&image).unwrap_err();
            assert_eq!(
                execute_debugger_cancel(&plan, &projection, &image, &mut vm, 1_250),
                Err(ObserveError::InvalidDebuggerContinuation(error))
            );
            assert_eq!(
                vm.pending_continuations(),
                vec![request.continuation.clone()]
            );
            assert_eq!(
                vm.debugger_audit(&CommandId::new("cancel-a").unwrap())
                    .unwrap(),
                None
            );
        }
        assert_eq!(
            execute_debugger_cancel(&plan, &projection, &request.continuation, &mut vm, 1_250)
                .unwrap()
                .status,
            DebuggerMutationStatus::Planned
        );
        assert_eq!(
            execute_debugger_cancel_effect(
                &plan,
                &projection,
                &request.continuation,
                &mut vm,
                1_250
            ),
            Err(ObserveError::UnexpectedDebuggerStep)
        );
        assert_eq!(vm.pending_count(), 1);
        assert_eq!(vm.completed_count(), 0);
    }

    #[test]
    fn static_projection_and_inspection_do_not_authorize_cancellation() {
        let mut vm = Vm::default();
        let request = request(false);
        let projection =
            waiting_debugger_projection(&request, "session-a", Revision(8), 1_250).unwrap();
        let plan =
            plan_debugger_cancel("session-a", &context(false, Confirmation::Confirmed)).unwrap();
        inspect_debugger_cancel_plan(&plan, &projection).unwrap();
        assert!(
            matches!(execute_debugger_cancel(&plan, &projection, &request.continuation, &mut vm, 1_250), Err(ObserveError::InvalidDebuggerContinuation(error)) if error.code == "LSV2004")
        );
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(vm.completed_count(), 0);
        assert_eq!(
            vm.debugger_audit(&CommandId::new("cancel-a").unwrap())
                .unwrap(),
            None
        );

        let (mut vm, request) = live();
        let mut forged = request.clone();
        forged.continuation.fuel_remaining -= 1;
        forged.budget = forged.continuation.resource_budget();
        let projection =
            waiting_debugger_projection(&forged, "session-a", Revision(8), 1_250).unwrap();
        inspect_debugger_cancel_plan(&plan, &projection).unwrap();
        assert!(
            matches!(execute_debugger_cancel(&plan, &projection, &forged.continuation, &mut vm, 1_250), Err(ObserveError::InvalidDebuggerContinuation(error)) if error.code == "LSV2005")
        );
        assert_eq!(
            vm.pending_continuations(),
            vec![request.continuation.clone()]
        );
        assert_eq!(
            vm.debugger_audit(&CommandId::new("cancel-a").unwrap())
                .unwrap(),
            None
        );
        let projection =
            waiting_debugger_projection(&request, "session-a", Revision(8), 1_250).unwrap();
        let result = execute_debugger_cancel_effect(
            &plan,
            &projection,
            &request.continuation,
            &mut vm,
            1_250,
        )
        .unwrap();
        assert_eq!(result.observed_at_ms, 1_250);
        assert_eq!(vm.pending_count(), 0);
        assert_eq!(
            execute_debugger_cancel_effect(
                &plan,
                &projection,
                &request.continuation,
                &mut vm,
                1_500
            )
            .unwrap(),
            result
        );
    }
}
