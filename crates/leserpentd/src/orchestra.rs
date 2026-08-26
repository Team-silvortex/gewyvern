use leserpent_domain::{
    CAPABILITY_ORCHESTRA_WRITE, CAPABILITY_RUNTIME_REFRESH, COMMAND_PLAN_SCHEMA_VERSION,
    CapabilitySet, Command, CommandEnvelope, CommandId, CommandOrigin, CommandPlan, Confirmation,
    DOMAIN_SCHEMA_VERSION, DomainError, IdempotencyKey, PlannedOperation, Revision, RuntimeId,
    RuntimeProjection,
};
use leserpent_protocol::compatibility_v1::{
    LegacyOrchestraEvent, LegacyOrchestraRun, LegacyOrchestraStep,
};
use leserpent_protocol::{
    OrchestraCancelCommandRequest, OrchestraControlOperation, OrchestraPlan,
    OrchestraPlanCatalogRequest, OrchestraPlanCatalogResponse, OrchestraPlanStep,
    OrchestraRetryCommandRequest, OrchestraRunCommandRequest, OrchestraRunReceiptResponse,
};
use leserpent_runtime::{ControlRuntime, OrchestraEffectStatusRecord, PlanResult, RuntimeError};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

const PLAN_POLICY_VERSION: &str = "orchestra-v1";
const MAX_CONTROL_COMMAND_ID_BYTES: usize = 96;
const MAX_RECONCILE_RUNS: u16 = 64;
const CANCEL_ERROR_PREFIX: &str = "orchestra_cancelled:";

struct StartRunIntent {
    command_id: CommandId,
    runtime_id: RuntimeId,
    plan_id: String,
    expected_plan_revision: String,
    confirmed: bool,
    approved_by: Option<String>,
    approval_note: Option<String>,
    retried_from: Option<LegacyOrchestraRun>,
    operation: OrchestraControlOperation,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum OrchestraAuthorityError {
    Unauthorized,
    RuntimeNotFound,
    PlanNotFound,
    PlanNotExecutable,
    PlanRevisionChanged,
    ConfirmationRequired,
    InvalidApproval,
    InvalidCommand,
    RequestConflict,
    RuntimeBusy,
    RunNotFound,
    RunNotTerminal,
    RunAlreadyTerminal,
    RunNotCancelable,
    PersistenceFailed,
    InvalidPersistedState,
}

impl OrchestraAuthorityError {
    pub(crate) fn code(self) -> &'static str {
        match self {
            Self::Unauthorized => "orchestra_unauthorized",
            Self::RuntimeNotFound => "runtime_not_found",
            Self::PlanNotFound => "orchestra_plan_not_found",
            Self::PlanNotExecutable => "orchestra_plan_not_executable",
            Self::PlanRevisionChanged => "orchestra_plan_revision_changed",
            Self::ConfirmationRequired => "orchestra_confirmation_required",
            Self::InvalidApproval => "invalid_orchestra_approval",
            Self::InvalidCommand => "invalid_orchestra_command",
            Self::RequestConflict => "orchestra_request_conflict",
            Self::RuntimeBusy => "orchestra_runtime_busy",
            Self::RunNotFound => "orchestra_run_not_found",
            Self::RunNotTerminal => "orchestra_run_not_terminal",
            Self::RunAlreadyTerminal => "orchestra_run_already_terminal",
            Self::RunNotCancelable => "orchestra_run_not_cancelable",
            Self::PersistenceFailed => "orchestra_persistence_failed",
            Self::InvalidPersistedState => "orchestra_state_invalid",
        }
    }

    pub(crate) fn message(self) -> &'static str {
        match self {
            Self::Unauthorized => "Orchestra authority was rejected",
            Self::RuntimeNotFound => "runtime was not found",
            Self::PlanNotFound => "Orchestra plan was not found",
            Self::PlanNotExecutable => "Orchestra plan is not executable by the Rust authority",
            Self::PlanRevisionChanged => "Orchestra plan revision changed",
            Self::ConfirmationRequired => "Orchestra command requires explicit confirmation",
            Self::InvalidApproval => "Orchestra approval metadata is invalid",
            Self::InvalidCommand => "Orchestra command identity is invalid",
            Self::RequestConflict => "Orchestra command identity was reused with different input",
            Self::RuntimeBusy => "runtime already has an active Orchestra run",
            Self::RunNotFound => "Orchestra run was not found",
            Self::RunNotTerminal => "Orchestra run is not terminal",
            Self::RunAlreadyTerminal => "Orchestra run is already terminal",
            Self::RunNotCancelable => "Orchestra run has no safely cancelable queued work",
            Self::PersistenceFailed => "Orchestra persistence transaction failed",
            Self::InvalidPersistedState => "persisted Orchestra state failed validation",
        }
    }
}

pub(crate) fn plan_catalog(
    runtime: &ControlRuntime,
    request: &OrchestraPlanCatalogRequest,
) -> Result<OrchestraPlanCatalogResponse, OrchestraAuthorityError> {
    authorize(&request.principal.id, &request.capabilities)?;
    let projection = runtime
        .runtime_projection(&request.runtime_id)
        .ok_or(OrchestraAuthorityError::RuntimeNotFound)?;
    Ok(build_plan_catalog(projection))
}

pub(crate) fn run_command(
    runtime: &mut ControlRuntime,
    request: OrchestraRunCommandRequest,
) -> Result<OrchestraRunReceiptResponse, OrchestraAuthorityError> {
    authorize(&request.principal.id, &request.capabilities)?;
    start_run(
        runtime,
        StartRunIntent {
            command_id: request.command_id,
            runtime_id: request.runtime_id,
            plan_id: request.plan_id,
            expected_plan_revision: request.expected_plan_revision,
            confirmed: request.confirmed,
            approved_by: request.approved_by,
            approval_note: request.approval_note,
            retried_from: None,
            operation: OrchestraControlOperation::Run,
        },
    )
}

pub(crate) fn retry_command(
    runtime: &mut ControlRuntime,
    request: OrchestraRetryCommandRequest,
) -> Result<OrchestraRunReceiptResponse, OrchestraAuthorityError> {
    authorize(&request.principal.id, &request.capabilities)?;
    require_control_id(request.command_id.as_str())?;
    require_identifier(&request.run_id)?;
    reconcile_run(runtime, request.runtime_id.as_str(), &request.run_id)?;
    let (previous, _) = load_run(runtime, request.runtime_id.as_str(), &request.run_id)?;
    if !terminal_outcome(&previous.outcome) {
        return Err(OrchestraAuthorityError::RunNotTerminal);
    }
    if previous.attempt >= 1_000_000 {
        return Err(OrchestraAuthorityError::InvalidPersistedState);
    }
    let plan_id = previous.plan_id.clone();
    start_run(
        runtime,
        StartRunIntent {
            command_id: request.command_id,
            runtime_id: request.runtime_id,
            plan_id,
            expected_plan_revision: request.expected_plan_revision,
            confirmed: request.confirmed,
            approved_by: request.approved_by,
            approval_note: request.approval_note,
            retried_from: Some(previous),
            operation: OrchestraControlOperation::Retry,
        },
    )
}

pub(crate) fn cancel_command(
    runtime: &mut ControlRuntime,
    request: OrchestraCancelCommandRequest,
) -> Result<OrchestraRunReceiptResponse, OrchestraAuthorityError> {
    authorize(&request.principal.id, &request.capabilities)?;
    require_control_id(request.command_id.as_str())?;
    require_identifier(&request.run_id)?;
    if !request.confirmed {
        return Err(OrchestraAuthorityError::ConfirmationRequired);
    }
    reconcile_run(runtime, request.runtime_id.as_str(), &request.run_id)?;
    let (run, events) = load_run(runtime, request.runtime_id.as_str(), &request.run_id)?;
    let cancellation_event = cancellation_event_type(request.command_id.as_str())?;
    if terminal_outcome(&run.outcome) {
        if run.outcome == "cancelled"
            && events
                .iter()
                .any(|event| event.event_type == cancellation_event)
        {
            return Ok(OrchestraRunReceiptResponse {
                command_id: request.command_id,
                operation: OrchestraControlOperation::Cancel,
                run,
                replayed: true,
            });
        }
        return Err(OrchestraAuthorityError::RunAlreadyTerminal);
    }
    let effect_ids = plan_effect_ids(&run.run_id, &run.plan_id)?;
    let cancellation = runtime
        .cancel_ready_orchestra_effects(&run.run_id, request.command_id.as_str(), &effect_ids)
        .map_err(map_runtime_error)?;
    if cancellation.cancelled_effect_count == 0 && !cancellation.replayed {
        return Err(OrchestraAuthorityError::RunNotCancelable);
    }
    reconcile_run(runtime, request.runtime_id.as_str(), &request.run_id)?;
    let (run, _) = load_run(runtime, request.runtime_id.as_str(), &request.run_id)?;
    if run.outcome != "cancelled" {
        return Err(OrchestraAuthorityError::InvalidPersistedState);
    }
    Ok(OrchestraRunReceiptResponse {
        command_id: request.command_id,
        operation: OrchestraControlOperation::Cancel,
        run,
        replayed: cancellation.replayed,
    })
}

pub(crate) fn reconcile_scope(
    runtime: &mut ControlRuntime,
    runtime_id: Option<&str>,
    run_id: Option<&str>,
) -> Result<(), OrchestraAuthorityError> {
    if let Some(run_id) = run_id {
        let runtime_id = runtime_id.ok_or(OrchestraAuthorityError::InvalidCommand)?;
        return reconcile_run(runtime, runtime_id, run_id).map(|_| ());
    }
    let history = runtime
        .load_orchestra_history(runtime_id, None, 0, MAX_RECONCILE_RUNS)
        .map_err(map_runtime_error)?;
    let runs = history
        .runs
        .iter()
        .map(|bytes| decode_run(bytes))
        .collect::<Result<Vec<_>, _>>()?;
    for run in runs {
        if active_outcome(&run.outcome) {
            reconcile_run(runtime, &run.runtime_id, &run.run_id)?;
        }
    }
    Ok(())
}

fn start_run(
    runtime: &mut ControlRuntime,
    intent: StartRunIntent,
) -> Result<OrchestraRunReceiptResponse, OrchestraAuthorityError> {
    let StartRunIntent {
        command_id,
        runtime_id,
        plan_id,
        expected_plan_revision,
        confirmed,
        approved_by,
        approval_note,
        retried_from,
        operation,
    } = intent;
    require_control_id(command_id.as_str())?;
    require_identifier(&plan_id)?;
    if !confirmed {
        return Err(OrchestraAuthorityError::ConfirmationRequired);
    }
    let replay_approved_by = normalize_text(approved_by.as_deref(), 80)?;
    let replay_approval_note = normalize_text(approval_note.as_deref(), 500)?;
    if let Some(existing) =
        find_run_by_request_id(runtime, runtime_id.as_str(), command_id.as_str())?
    {
        let expected_parent = retried_from.as_ref().map(|run| run.run_id.as_str());
        if existing.plan_id != plan_id
            || existing.retried_from_run_id.as_deref() != expected_parent
            || existing.plan_revision.as_deref() != Some(expected_plan_revision.as_str())
            || existing.approved_by != replay_approved_by
            || existing.approval_note != replay_approval_note
        {
            return Err(OrchestraAuthorityError::RequestConflict);
        }
        ensure_initial_effect(runtime, &existing)?;
        return Ok(OrchestraRunReceiptResponse {
            command_id,
            operation,
            run: existing,
            replayed: true,
        });
    }
    let projection = runtime
        .runtime_projection(&runtime_id)
        .ok_or(OrchestraAuthorityError::RuntimeNotFound)?;
    let catalog = build_plan_catalog(projection);
    let plan = catalog
        .plans
        .iter()
        .find(|plan| plan.plan_id == plan_id)
        .ok_or(OrchestraAuthorityError::PlanNotFound)?;
    if plan.execution_mode != "automatic" {
        return Err(OrchestraAuthorityError::PlanNotExecutable);
    }
    if plan.revision != expected_plan_revision {
        return Err(OrchestraAuthorityError::PlanRevisionChanged);
    }
    let (approved_by, approval_note) = validate_approval(
        plan,
        confirmed,
        approved_by.as_deref(),
        approval_note.as_deref(),
    )?;
    let run_id = run_id(command_id.as_str())?;
    let executed_at = timestamp_after(None)?;
    let attempt = retried_from
        .as_ref()
        .map_or(1, |run| run.attempt.saturating_add(1));
    let run = LegacyOrchestraRun {
        run_id: run_id.clone(),
        runtime_id: runtime_id.as_str().into(),
        plan_id,
        outcome: "queued".into(),
        executed_at: executed_at.clone(),
        steps: Vec::new(),
        completed_at: None,
        attempt,
        retried_from_run_id: retried_from.as_ref().map(|run| run.run_id.clone()),
        approved_by,
        approval_note,
        plan_revision: Some(plan.revision.clone()),
        request_id: Some(command_id.as_str().into()),
    };
    let event = LegacyOrchestraEvent {
        event_id: 0,
        run_id: run_id.clone(),
        runtime_id: runtime_id.as_str().into(),
        event_type: "run_queued".into(),
        from_outcome: None,
        to_outcome: "queued".into(),
        summary: "Rust Orchestra authority queued the plan".into(),
        recorded_at: executed_at,
    };
    let run_bytes =
        serde_json::to_vec(&run).map_err(|_| OrchestraAuthorityError::InvalidPersistedState)?;
    let event_bytes =
        serde_json::to_vec(&event).map_err(|_| OrchestraAuthorityError::InvalidPersistedState)?;
    let receipt = runtime
        .persist_orchestra_run_event_start(
            &run_id,
            runtime_id.as_str(),
            command_id.as_str(),
            "run_queued",
            "queued",
            &event.recorded_at,
            &run_bytes,
            &event_bytes,
        )
        .map_err(map_runtime_error)?;
    let persisted = decode_run(&receipt.run)?;
    if persisted != run {
        return Err(OrchestraAuthorityError::InvalidPersistedState);
    }
    ensure_initial_effect(runtime, &persisted)?;
    Ok(OrchestraRunReceiptResponse {
        command_id,
        operation,
        run: persisted,
        replayed: false,
    })
}

fn reconcile_run(
    runtime: &mut ControlRuntime,
    runtime_id: &str,
    run_id: &str,
) -> Result<LegacyOrchestraRun, OrchestraAuthorityError> {
    let (mut run, mut events) = load_run(runtime, runtime_id, run_id)?;
    if !native_control_run(&run) {
        return Ok(run);
    }
    if terminal_outcome(&run.outcome) {
        return Ok(run);
    }
    let marker_id = cancel_marker_id(run_id)?;
    if let Some(marker) = runtime
        .orchestra_effect_status(&marker_id)
        .map_err(map_runtime_error)?
    {
        let command_id = marker
            .last_error
            .as_deref()
            .and_then(|error| error.strip_prefix(CANCEL_ERROR_PREFIX))
            .ok_or(OrchestraAuthorityError::InvalidPersistedState)?;
        require_control_id(command_id)?;
        settle_cancelled_status_refresh(runtime, &run, command_id)?;
        return transition_run(
            runtime,
            run,
            &events,
            "cancelled",
            &cancellation_event_type(command_id)?,
            vec![LegacyOrchestraStep {
                step: "cancel".into(),
                outcome: "cancelled".into(),
                summary: "queued Orchestra work was cancelled by the operator".into(),
            }],
        );
    }

    let initial_id = initial_effect_id(run_id, &run.plan_id)?;
    let initial = runtime
        .orchestra_effect_status(&initial_id)
        .map_err(map_runtime_error)?;
    let initial = match initial {
        Some(initial) => initial,
        None => {
            ensure_initial_effect(runtime, &run)?;
            runtime
                .orchestra_effect_status(&initial_id)
                .map_err(map_runtime_error)?
                .ok_or(OrchestraAuthorityError::InvalidPersistedState)?
        }
    };
    if initial.state == "failed" {
        let step = initial_step(&run.plan_id);
        return transition_failed_effect(runtime, run, &events, &initial, step);
    }
    if matches!(initial.state.as_str(), "leased" | "completed") && run.outcome == "queued" {
        run = transition_run(runtime, run, &events, "running", "run_started", Vec::new())?;
        events = load_run(runtime, runtime_id, run_id)?.1;
    }
    if initial.state != "completed" {
        return Ok(run);
    }

    if run.plan_id == "analysis_recovery" {
        let status_id = status_effect_id(run_id)?;
        let status = runtime
            .orchestra_effect_status(&status_id)
            .map_err(map_runtime_error)?;
        let Some(status) = status else {
            let projection = runtime
                .runtime_projection(
                    &RuntimeId::new(runtime_id)
                        .map_err(|_| OrchestraAuthorityError::InvalidPersistedState)?,
                )
                .ok_or(OrchestraAuthorityError::RuntimeNotFound)?;
            schedule_refresh_command(runtime, &status_id, runtime_id, projection.revision, false)?;
            return Ok(run);
        };
        if status.state == "failed" {
            return transition_failed_effect(runtime, run, &events, &status, "refresh_status");
        }
        if status.state != "completed" {
            return Ok(run);
        }
        return transition_run(
            runtime,
            run,
            &events,
            "succeeded",
            "run_completed",
            vec![
                LegacyOrchestraStep {
                    step: "refresh_capabilities".into(),
                    outcome: "ok".into(),
                    summary: "runtime capabilities refreshed".into(),
                },
                LegacyOrchestraStep {
                    step: "refresh_status".into(),
                    outcome: "ok".into(),
                    summary: "runtime status refreshed".into(),
                },
            ],
        );
    }

    transition_run(
        runtime,
        run,
        &events,
        "succeeded",
        "run_completed",
        vec![LegacyOrchestraStep {
            step: "refresh_status".into(),
            outcome: "ok".into(),
            summary: "runtime status refreshed".into(),
        }],
    )
}

fn settle_cancelled_status_refresh(
    runtime: &mut ControlRuntime,
    run: &LegacyOrchestraRun,
    cancellation_command_id: &str,
) -> Result<(), OrchestraAuthorityError> {
    let effect_id = status_effect_id(&run.run_id)?;
    runtime
        .settle_cancelled_orchestra_status_effect(&effect_id, cancellation_command_id)
        .map_err(map_runtime_error)?;
    Ok(())
}

fn transition_failed_effect(
    runtime: &mut ControlRuntime,
    run: LegacyOrchestraRun,
    events: &[LegacyOrchestraEvent],
    effect: &OrchestraEffectStatusRecord,
    step: &str,
) -> Result<LegacyOrchestraRun, OrchestraAuthorityError> {
    if let Some(command_id) = effect
        .last_error
        .as_deref()
        .and_then(|error| error.strip_prefix(CANCEL_ERROR_PREFIX))
    {
        require_control_id(command_id)?;
        return transition_run(
            runtime,
            run,
            events,
            "cancelled",
            &cancellation_event_type(command_id)?,
            vec![LegacyOrchestraStep {
                step: "cancel".into(),
                outcome: "cancelled".into(),
                summary: "queued Orchestra work was cancelled by the operator".into(),
            }],
        );
    }
    transition_run(
        runtime,
        run,
        events,
        "failed",
        "run_failed",
        vec![LegacyOrchestraStep {
            step: step.into(),
            outcome: "failed".into(),
            summary: "Orchestra adapter execution failed".into(),
        }],
    )
}

fn transition_run(
    runtime: &mut ControlRuntime,
    mut run: LegacyOrchestraRun,
    events: &[LegacyOrchestraEvent],
    outcome: &str,
    event_type: &str,
    steps: Vec<LegacyOrchestraStep>,
) -> Result<LegacyOrchestraRun, OrchestraAuthorityError> {
    let previous = run.outcome.clone();
    let recorded_at = timestamp_after(events.last().map(|event| event.recorded_at.as_str()))?;
    run.outcome = outcome.into();
    run.steps = steps;
    if terminal_outcome(outcome) {
        run.completed_at = Some(recorded_at.clone());
    }
    let event = LegacyOrchestraEvent {
        event_id: 0,
        run_id: run.run_id.clone(),
        runtime_id: run.runtime_id.clone(),
        event_type: event_type.into(),
        from_outcome: Some(previous.clone()),
        to_outcome: outcome.into(),
        summary: transition_summary(outcome).into(),
        recorded_at,
    };
    let run_bytes =
        serde_json::to_vec(&run).map_err(|_| OrchestraAuthorityError::InvalidPersistedState)?;
    let event_bytes =
        serde_json::to_vec(&event).map_err(|_| OrchestraAuthorityError::InvalidPersistedState)?;
    let receipt = runtime
        .persist_orchestra_run_event(
            &run.run_id,
            &run.runtime_id,
            run.request_id.as_deref(),
            &event.event_type,
            Some(&previous),
            outcome,
            outcome,
            &event.recorded_at,
            &run_bytes,
            &event_bytes,
        )
        .map_err(map_runtime_error)?;
    decode_run(&receipt.run)
}

fn build_plan_catalog(runtime: &RuntimeProjection) -> OrchestraPlanCatalogResponse {
    let mut reasons: Vec<String> = Vec::new();
    if runtime.status.status_fetch_error.is_some() {
        reasons.push("status_fetch_failed".into());
    }
    if !runtime.status.has_latest_snapshot {
        reasons.push("no_latest_snapshot".into());
    }
    if !runtime.status.has_analysis_json {
        reasons.push("no_analysis_json".into());
    }
    if runtime.status.resilience_degraded {
        reasons.push("resilience_degraded".into());
    }
    if runtime
        .sidecar_status
        .as_ref()
        .is_some_and(|status| status.status_fetch_error.is_some())
    {
        reasons.push("sidecar_status_fetch_failed".into());
    }
    if runtime.capabilities.is_unobserved() {
        reasons.push("capabilities_unobserved".into());
    }
    reasons.sort();
    reasons.dedup();
    let severe = reasons.iter().any(|reason| {
        matches!(
            reason.as_str(),
            "status_fetch_failed" | "resilience_degraded" | "sidecar_status_fetch_failed"
        )
    });
    let needs_attention = !reasons.is_empty();
    let attention_severity = if severe {
        "critical"
    } else if needs_attention {
        "warning"
    } else {
        "healthy"
    };
    let triage_approval = if severe {
        "operator_confirmation"
    } else {
        "none"
    };
    let mut plans = vec![
        plan(
            runtime.revision,
            "runtime_triage",
            "triage",
            "Refresh and verify runtime posture",
            "Refresh the authoritative runtime status before deeper action.",
            if severe { "medium" } else { "low" },
            "ready_now",
            "automatic",
            triage_approval,
            reasons.clone(),
            Vec::new(),
            vec![OrchestraPlanStep {
                key: "refresh_status".into(),
                title: "Refresh runtime status".into(),
                detail: "Run the bounded native status adapter and commit its observation.".into(),
                kind: "refresh".into(),
            }],
        ),
        plan(
            runtime.revision,
            "analysis_recovery",
            "recover_analysis",
            "Recover analysis-ready runtime evidence",
            "Refresh capabilities first, then re-enter with the new revision for status evidence.",
            "medium",
            "ready_now",
            "automatic",
            "operator_confirmation",
            reasons.clone(),
            Vec::new(),
            vec![
                OrchestraPlanStep {
                    key: "refresh_capabilities".into(),
                    title: "Refresh capabilities".into(),
                    detail: "Discover capabilities through the configured native adapter.".into(),
                    kind: "refresh".into(),
                },
                OrchestraPlanStep {
                    key: "refresh_status".into(),
                    title: "Refresh runtime status".into(),
                    detail: "Use the post-capability revision for a fenced status observation."
                        .into(),
                    kind: "refresh".into(),
                },
            ],
        ),
    ];
    if runtime.sidecar_endpoint.is_some() {
        plans.push(plan(
            runtime.revision,
            "sidecar_coordination",
            "coordinate_sidecar",
            "Coordinate runtime and sidecar posture",
            "Sidecar coordination remains guided until a Rust sidecar effect adapter is present.",
            "medium",
            "review_first",
            "guided",
            "operator_confirmation",
            reasons.clone(),
            vec!["sidecar_status".into()],
            vec![OrchestraPlanStep {
                key: "review_sidecar".into(),
                title: "Review sidecar posture".into(),
                detail: "Use a guided surface without inventing native execution authority.".into(),
                kind: "review".into(),
            }],
        ));
    }
    plans.push(plan(
        runtime.revision,
        "session_preparation",
        "prepare_session",
        "Prepare a session handoff",
        "Session creation remains guided until its Rust command authority is sealed.",
        "medium",
        "review_first",
        "guided",
        "operator_confirmation",
        Vec::new(),
        Vec::new(),
        vec![OrchestraPlanStep {
            key: "review_session".into(),
            title: "Review session requirements".into(),
            detail: "Use the session surface without claiming an executable native plan.".into(),
            kind: "review".into(),
        }],
    ));
    OrchestraPlanCatalogResponse {
        runtime_id: runtime.id.clone(),
        runtime_name: runtime.name.clone(),
        runtime_revision: runtime.revision,
        status_source: runtime.status.status_source.clone(),
        attention_severity: attention_severity.into(),
        needs_attention,
        attention_reasons: reasons,
        plans,
    }
}

#[allow(clippy::too_many_arguments)]
fn plan(
    revision: Revision,
    plan_id: &str,
    intent: &str,
    title: &str,
    summary: &str,
    risk_level: &str,
    execution_readiness: &str,
    execution_mode: &str,
    approval_mode: &str,
    reasons: Vec<String>,
    required_capabilities: Vec<String>,
    steps: Vec<OrchestraPlanStep>,
) -> OrchestraPlan {
    OrchestraPlan {
        plan_id: plan_id.into(),
        intent: intent.into(),
        title: title.into(),
        summary: summary.into(),
        risk_level: risk_level.into(),
        execution_readiness: execution_readiness.into(),
        execution_mode: execution_mode.into(),
        approval_mode: approval_mode.into(),
        revision: plan_revision(revision, plan_id),
        reasons,
        required_capabilities,
        steps,
    }
}

fn plan_revision(revision: Revision, plan_id: &str) -> String {
    format!("{PLAN_POLICY_VERSION}-{}-{plan_id}", revision.0)
}

fn authorize(
    principal: &str,
    capabilities: &leserpent_domain::CapabilitySet,
) -> Result<(), OrchestraAuthorityError> {
    if principal.trim().is_empty() || !capabilities.contains(CAPABILITY_ORCHESTRA_WRITE) {
        return Err(OrchestraAuthorityError::Unauthorized);
    }
    Ok(())
}

fn validate_approval(
    plan: &OrchestraPlan,
    confirmed: bool,
    approved_by: Option<&str>,
    approval_note: Option<&str>,
) -> Result<(Option<String>, Option<String>), OrchestraAuthorityError> {
    if !confirmed {
        return Err(OrchestraAuthorityError::ConfirmationRequired);
    }
    let approved_by = normalize_text(approved_by, 80)?;
    let approval_note = normalize_text(approval_note, 500)?;
    if plan.approval_mode == "operator_confirmation"
        && (approved_by.is_none() || approval_note.is_none())
    {
        return Err(OrchestraAuthorityError::InvalidApproval);
    }
    Ok((approved_by, approval_note))
}

fn normalize_text(
    value: Option<&str>,
    maximum: usize,
) -> Result<Option<String>, OrchestraAuthorityError> {
    let Some(value) = value else {
        return Ok(None);
    };
    if value.is_empty()
        || value.len() > maximum
        || value != value.trim()
        || value.chars().any(char::is_control)
    {
        return Err(OrchestraAuthorityError::InvalidApproval);
    }
    Ok(Some(value.into()))
}

fn ensure_initial_effect(
    runtime: &mut ControlRuntime,
    run: &LegacyOrchestraRun,
) -> Result<(), OrchestraAuthorityError> {
    let effect_id = initial_effect_id(&run.run_id, &run.plan_id)?;
    if runtime
        .orchestra_effect_status(&effect_id)
        .map_err(map_runtime_error)?
        .is_some()
    {
        return Ok(());
    }
    let expected_revision = parse_plan_revision(run)?;
    schedule_refresh_command(
        runtime,
        &effect_id,
        &run.runtime_id,
        expected_revision,
        run.plan_id == "analysis_recovery",
    )
}

fn schedule_refresh_command(
    runtime: &mut ControlRuntime,
    effect_id: &str,
    runtime_id: &str,
    expected_revision: Revision,
    capabilities: bool,
) -> Result<(), OrchestraAuthorityError> {
    let runtime_id =
        RuntimeId::new(runtime_id).map_err(|_| OrchestraAuthorityError::InvalidPersistedState)?;
    let command_id = leserpent_domain::CommandId::new(effect_id)
        .map_err(|_| OrchestraAuthorityError::InvalidPersistedState)?;
    let command = if capabilities {
        Command::RuntimeCapabilitiesRefresh {
            runtime_id: runtime_id.clone(),
        }
    } else {
        Command::RuntimeRefresh {
            runtime_id: runtime_id.clone(),
        }
    };
    let result = runtime
        .execute_plan(CommandPlan {
            schema_version: COMMAND_PLAN_SCHEMA_VERSION,
            required_capability: CAPABILITY_RUNTIME_REFRESH.into(),
            operation: PlannedOperation::Command(CommandEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                command_id: command_id.clone(),
                idempotency_key: IdempotencyKey::new(effect_id)
                    .map_err(|_| OrchestraAuthorityError::InvalidPersistedState)?,
                expected_revision: Some(expected_revision),
                principal: leserpent_domain::Principal {
                    id: "leserpentd.orchestra".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
                origin: CommandOrigin::Gui,
                confirmation: Confirmation::NotRequired,
                dry_run: false,
                command,
            }),
        })
        .map_err(map_runtime_error)?;
    let PlanResult::Command(result) = result else {
        return Err(OrchestraAuthorityError::InvalidPersistedState);
    };
    if result.command_id != command_id || result.runtime.id != runtime_id {
        return Err(OrchestraAuthorityError::InvalidPersistedState);
    }
    Ok(())
}

fn parse_plan_revision(run: &LegacyOrchestraRun) -> Result<Revision, OrchestraAuthorityError> {
    let encoded = run
        .plan_revision
        .as_deref()
        .ok_or(OrchestraAuthorityError::InvalidPersistedState)?;
    let prefix = format!("{PLAN_POLICY_VERSION}-");
    let suffix = format!("-{}", run.plan_id);
    let revision = encoded
        .strip_prefix(&prefix)
        .and_then(|value| value.strip_suffix(&suffix))
        .and_then(|value| value.parse::<u64>().ok())
        .map(Revision)
        .ok_or(OrchestraAuthorityError::InvalidPersistedState)?;
    if plan_revision(revision, &run.plan_id) != encoded {
        return Err(OrchestraAuthorityError::InvalidPersistedState);
    }
    Ok(revision)
}

fn plan_effect_ids(run_id: &str, plan_id: &str) -> Result<Vec<String>, OrchestraAuthorityError> {
    match plan_id {
        "runtime_triage" => Ok(vec![status_effect_id(run_id)?]),
        "analysis_recovery" => Ok(vec![
            capabilities_effect_id(run_id)?,
            status_effect_id(run_id)?,
        ]),
        _ => Err(OrchestraAuthorityError::InvalidPersistedState),
    }
}

fn initial_effect_id(run_id: &str, plan_id: &str) -> Result<String, OrchestraAuthorityError> {
    match plan_id {
        "runtime_triage" => status_effect_id(run_id),
        "analysis_recovery" => capabilities_effect_id(run_id),
        _ => Err(OrchestraAuthorityError::InvalidPersistedState),
    }
}

fn run_id(command_id: &str) -> Result<String, OrchestraAuthorityError> {
    require_control_id(command_id)?;
    let run_id = format!("orun-{command_id}");
    require_identifier(&run_id)?;
    Ok(run_id)
}

fn capabilities_effect_id(run_id: &str) -> Result<String, OrchestraAuthorityError> {
    effect_id(run_id, "capabilities")
}

fn status_effect_id(run_id: &str) -> Result<String, OrchestraAuthorityError> {
    effect_id(run_id, "status")
}

fn cancel_marker_id(run_id: &str) -> Result<String, OrchestraAuthorityError> {
    effect_id(run_id, "cancel")
}

fn effect_id(run_id: &str, suffix: &str) -> Result<String, OrchestraAuthorityError> {
    let effect_id = format!("{run_id}.{suffix}");
    require_identifier(&effect_id)?;
    Ok(effect_id)
}

fn cancellation_event_type(command_id: &str) -> Result<String, OrchestraAuthorityError> {
    require_control_id(command_id)?;
    let event_type = format!("run_cancelled.{command_id}");
    require_identifier(&event_type)?;
    Ok(event_type)
}

fn require_control_id(value: &str) -> Result<(), OrchestraAuthorityError> {
    if value.len() > MAX_CONTROL_COMMAND_ID_BYTES {
        return Err(OrchestraAuthorityError::InvalidCommand);
    }
    require_identifier(value)
}

fn require_identifier(value: &str) -> Result<(), OrchestraAuthorityError> {
    if value.is_empty()
        || value.len() > 128
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'))
    {
        return Err(OrchestraAuthorityError::InvalidCommand);
    }
    Ok(())
}

fn find_run_by_request_id(
    runtime: &mut ControlRuntime,
    runtime_id: &str,
    request_id: &str,
) -> Result<Option<LegacyOrchestraRun>, OrchestraAuthorityError> {
    let history = runtime
        .load_orchestra_history(Some(runtime_id), None, 0, MAX_RECONCILE_RUNS)
        .map_err(map_runtime_error)?;
    history
        .runs
        .iter()
        .map(|bytes| decode_run(bytes))
        .find(|run| {
            run.as_ref()
                .is_ok_and(|run| run.request_id.as_deref() == Some(request_id))
        })
        .transpose()
}

fn load_run(
    runtime: &mut ControlRuntime,
    runtime_id: &str,
    run_id: &str,
) -> Result<(LegacyOrchestraRun, Vec<LegacyOrchestraEvent>), OrchestraAuthorityError> {
    require_identifier(runtime_id)?;
    require_identifier(run_id)?;
    let run_history = runtime
        .load_orchestra_history(Some(runtime_id), None, 0, MAX_RECONCILE_RUNS)
        .map_err(map_runtime_error)?;
    let run = run_history
        .runs
        .iter()
        .map(|bytes| decode_run(bytes))
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .find(|run| run.run_id == run_id)
        .ok_or(OrchestraAuthorityError::RunNotFound)?;
    let event_history = runtime
        .load_orchestra_history(Some(runtime_id), Some(run_id), 0, 64)
        .map_err(map_runtime_error)?;
    let events = event_history
        .events
        .iter()
        .map(|(_, bytes)| {
            serde_json::from_slice(bytes)
                .map_err(|_| OrchestraAuthorityError::InvalidPersistedState)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok((run, events))
}

fn decode_run(bytes: &[u8]) -> Result<LegacyOrchestraRun, OrchestraAuthorityError> {
    serde_json::from_slice(bytes).map_err(|_| OrchestraAuthorityError::InvalidPersistedState)
}

fn timestamp_after(previous: Option<&str>) -> Result<String, OrchestraAuthorityError> {
    let mut timestamp = OffsetDateTime::now_utc();
    if let Some(previous) = previous {
        let previous = OffsetDateTime::parse(previous, &Rfc3339)
            .map_err(|_| OrchestraAuthorityError::InvalidPersistedState)?;
        if timestamp < previous {
            timestamp = previous;
        }
    }
    timestamp
        .format(&Rfc3339)
        .map_err(|_| OrchestraAuthorityError::InvalidPersistedState)
}

fn active_outcome(outcome: &str) -> bool {
    matches!(outcome, "queued" | "running")
}

fn native_control_run(run: &LegacyOrchestraRun) -> bool {
    matches!(run.plan_id.as_str(), "runtime_triage" | "analysis_recovery")
        && run.request_id.is_some()
        && parse_plan_revision(run).is_ok()
}

fn terminal_outcome(outcome: &str) -> bool {
    matches!(
        outcome,
        "succeeded" | "degraded" | "failed" | "cancelled" | "ok"
    )
}

fn initial_step(plan_id: &str) -> &'static str {
    if plan_id == "analysis_recovery" {
        "refresh_capabilities"
    } else {
        "refresh_status"
    }
}

fn transition_summary(outcome: &str) -> &'static str {
    match outcome {
        "running" => "Rust Orchestra authority started the plan",
        "succeeded" => "Rust Orchestra authority completed the plan",
        "cancelled" => "Rust Orchestra authority cancelled queued work",
        _ => "Rust Orchestra authority recorded a terminal failure",
    }
}

fn map_runtime_error(error: RuntimeError) -> OrchestraAuthorityError {
    match error {
        RuntimeError::Domain(DomainError::RevisionConflict { .. }) => {
            OrchestraAuthorityError::PlanRevisionChanged
        }
        RuntimeError::Domain(DomainError::IdempotencyConflict { .. }) => {
            OrchestraAuthorityError::RequestConflict
        }
        RuntimeError::Domain(DomainError::RuntimeNotFound { .. }) => {
            OrchestraAuthorityError::RuntimeNotFound
        }
        RuntimeError::Storage(message)
            if message.contains("already has an active Orchestra run")
                || message.contains("already has an active run") =>
        {
            OrchestraAuthorityError::RuntimeBusy
        }
        RuntimeError::Storage(message)
            if message.contains("identity was reused")
                || message.contains("UNIQUE constraint failed") =>
        {
            OrchestraAuthorityError::RequestConflict
        }
        RuntimeError::Storage(_) => OrchestraAuthorityError::PersistenceFailed,
        _ => OrchestraAuthorityError::InvalidPersistedState,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use leserpent_domain::{
        CapabilitySet, CommandId, Principal, RefreshStatus, RuntimeCapabilityObservation,
        RuntimeCapabilityRefreshRequest, RuntimeCapabilitySnapshot, RuntimeStatusObservation,
        RuntimeStatusRefreshRequest,
    };
    use leserpent_runtime::EffectExecution;
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{Duration, SystemTime, UNIX_EPOCH};

    fn temp_database(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "leserpent-orchestra-{label}-{}-{nonce}.sqlite",
            std::process::id()
        ))
    }

    fn request(runtime_id: RuntimeId, revision: String) -> OrchestraRunCommandRequest {
        OrchestraRunCommandRequest {
            principal: Principal {
                id: "operator-a".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
            command_id: CommandId::new("orchestra-run-0001").unwrap(),
            runtime_id,
            plan_id: "runtime_triage".into(),
            expected_plan_revision: revision,
            confirmed: true,
            approved_by: None,
            approval_note: None,
        }
    }

    #[test]
    fn planner_is_revision_fenced_and_marks_guided_plans_honestly() {
        let mut runtime = ControlRuntime::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        runtime
            .register_runtime(runtime_id.clone(), "Runtime A", "https://runtime-a.invalid")
            .unwrap();
        let catalog = plan_catalog(
            &runtime,
            &OrchestraPlanCatalogRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                runtime_id,
            },
        )
        .unwrap();
        assert_eq!(catalog.plans.len(), 3);
        assert_eq!(catalog.plans[0].execution_mode, "automatic");
        assert_eq!(catalog.plans[1].approval_mode, "operator_confirmation");
        assert_eq!(catalog.plans[2].execution_mode, "guided");
        assert!(catalog.plans.iter().all(|plan| {
            plan.revision.starts_with(&format!(
                "{PLAN_POLICY_VERSION}-{}-",
                catalog.runtime_revision.0
            ))
        }));
    }

    #[test]
    fn run_requires_persistence_and_exact_plan_revision() {
        let mut runtime = ControlRuntime::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        runtime
            .register_runtime(runtime_id.clone(), "Runtime A", "https://runtime-a.invalid")
            .unwrap();
        assert_eq!(
            run_command(&mut runtime, request(runtime_id.clone(), "stale".into())),
            Err(OrchestraAuthorityError::PersistenceFailed)
        );

        let path = temp_database("revision-fence");
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(runtime_id.clone(), "Runtime A", "https://runtime-a.invalid")
            .unwrap();
        assert_eq!(
            run_command(&mut runtime, request(runtime_id, "stale".into())),
            Err(OrchestraAuthorityError::PlanRevisionChanged)
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn control_ids_leave_room_for_derived_run_effect_and_event_ids() {
        let accepted = "a".repeat(MAX_CONTROL_COMMAND_ID_BYTES);
        assert!(run_id(&accepted).is_ok());
        assert!(status_effect_id(&run_id(&accepted).unwrap()).is_ok());
        assert!(cancellation_event_type(&accepted).is_ok());
        assert_eq!(
            require_control_id(&"a".repeat(MAX_CONTROL_COMMAND_ID_BYTES + 1)),
            Err(OrchestraAuthorityError::InvalidCommand)
        );
    }

    #[test]
    fn native_run_is_atomic_replayable_and_converges_from_effect_to_history() {
        let path = temp_database("native-run");
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(runtime_id.clone(), "Runtime A", "https://runtime-a.invalid")
            .unwrap();
        let revision = plan_revision(
            runtime.runtime_projection(&runtime_id).unwrap().revision,
            "runtime_triage",
        );
        let accepted =
            run_command(&mut runtime, request(runtime_id.clone(), revision.clone())).unwrap();
        assert_eq!(accepted.run.outcome, "queued");
        assert!(!accepted.replayed);
        let replay = run_command(&mut runtime, request(runtime_id.clone(), revision)).unwrap();
        assert_eq!(replay.run.run_id, accepted.run.run_id);
        assert!(replay.replayed);

        let competing_revision = plan_revision(
            runtime.runtime_projection(&runtime_id).unwrap().revision,
            "runtime_triage",
        );
        let competing = OrchestraRunCommandRequest {
            command_id: CommandId::new("orchestra-run-0002").unwrap(),
            ..request(runtime_id.clone(), competing_revision)
        };
        assert_eq!(
            run_command(&mut runtime, competing),
            Err(OrchestraAuthorityError::RuntimeBusy)
        );

        let lease = runtime
            .claim_effect("orchestra-worker", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        assert_eq!(
            lease.effect_id,
            status_effect_id(&accepted.run.run_id).unwrap()
        );
        let effect_request: RuntimeStatusRefreshRequest =
            serde_json::from_slice(&lease.payload).unwrap();
        let mut status = runtime
            .runtime_projection(&runtime_id)
            .unwrap()
            .status
            .clone();
        status.status_source = "gewyvern-api".into();
        status.status_fetched_at = Some("2026-08-26T08:00:00Z".into());
        status.status_fetch_error = None;
        status.has_latest_snapshot = true;
        status.has_analysis_json = true;
        let outcome = RuntimeStatusObservation {
            runtime_id: runtime_id.as_str().into(),
            expected_revision: effect_request.expected_revision,
            status,
        };
        let settled = runtime
            .settle_effect(
                &lease,
                EffectExecution::Complete(serde_json::to_vec(&outcome).unwrap()),
            )
            .unwrap();
        let settled_status = runtime.orchestra_effect_status(&lease.effect_id).unwrap();
        assert!(
            matches!(settled, leserpent_runtime::WorkerStep::Completed { .. }),
            "status effect did not complete: {settled:?} {settled_status:?}"
        );
        reconcile_scope(&mut runtime, Some(runtime_id.as_str()), None).unwrap();
        let (completed, events) =
            load_run(&mut runtime, runtime_id.as_str(), &accepted.run.run_id).unwrap();
        assert_eq!(completed.outcome, "succeeded");
        assert_eq!(completed.steps.len(), 1);
        assert_eq!(events.len(), 3);
        drop(runtime);

        let mut recovered = ControlRuntime::open(&path).unwrap();
        let (completed_after_restart, _) =
            load_run(&mut recovered, runtime_id.as_str(), &accepted.run.run_id).unwrap();
        assert_eq!(completed_after_restart, completed);
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn analysis_recovery_reenters_with_post_capability_revision() {
        let path = temp_database("analysis-recovery");
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(runtime_id.clone(), "Runtime A", "https://runtime-a.invalid")
            .unwrap();
        let revision = plan_revision(
            runtime.runtime_projection(&runtime_id).unwrap().revision,
            "analysis_recovery",
        );
        let accepted = run_command(
            &mut runtime,
            OrchestraRunCommandRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                command_id: CommandId::new("orchestra-analysis-0001").unwrap(),
                runtime_id: runtime_id.clone(),
                plan_id: "analysis_recovery".into(),
                expected_plan_revision: revision,
                confirmed: true,
                approved_by: Some("operator-a".into()),
                approval_note: Some("recover current evidence".into()),
            },
        )
        .unwrap();

        let capability_lease = runtime
            .claim_effect("orchestra-worker", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        assert_eq!(
            capability_lease.effect_id,
            capabilities_effect_id(&accepted.run.run_id).unwrap()
        );
        let capability_request: RuntimeCapabilityRefreshRequest =
            serde_json::from_slice(&capability_lease.payload).unwrap();
        let capability_outcome = RuntimeCapabilityObservation {
            runtime_id: runtime_id.as_str().into(),
            expected_revision: capability_request.expected_revision,
            capabilities: RuntimeCapabilitySnapshot {
                source: "gewyvern-api".into(),
                service: "gewyvern-api".into(),
                version: "1.17.4".into(),
                latest_snapshot: true,
                authenticated_deployment: true,
                serve_required: true,
                external_sidecar_context: false,
                target_path_segment_encoding: "percent-encoding".into(),
                target_direct_path_chars: "A-Z a-z 0-9 . _ ~ :".into(),
                endpoints: vec!["/v1/capabilities".into(), "/v1/deployments".into()],
                extensions: BTreeMap::new(),
            },
        };
        runtime
            .settle_effect(
                &capability_lease,
                EffectExecution::Complete(serde_json::to_vec(&capability_outcome).unwrap()),
            )
            .unwrap();
        reconcile_scope(&mut runtime, Some(runtime_id.as_str()), None).unwrap();

        let status_lease = runtime
            .claim_effect("orchestra-worker", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        assert_eq!(
            status_lease.effect_id,
            status_effect_id(&accepted.run.run_id).unwrap()
        );
        let status_request: RuntimeStatusRefreshRequest =
            serde_json::from_slice(&status_lease.payload).unwrap();
        assert!(status_request.expected_revision > capability_request.expected_revision);
        let mut status = runtime
            .runtime_projection(&runtime_id)
            .unwrap()
            .status
            .clone();
        status.status_source = "gewyvern-api".into();
        status.status_fetched_at = Some("2026-08-26T08:00:00Z".into());
        status.status_fetch_error = None;
        status.has_latest_snapshot = true;
        status.has_analysis_json = true;
        runtime
            .settle_effect(
                &status_lease,
                EffectExecution::Complete(
                    serde_json::to_vec(&RuntimeStatusObservation {
                        runtime_id: runtime_id.as_str().into(),
                        expected_revision: status_request.expected_revision,
                        status,
                    })
                    .unwrap(),
                ),
            )
            .unwrap();
        reconcile_scope(&mut runtime, Some(runtime_id.as_str()), None).unwrap();
        let (completed, events) =
            load_run(&mut runtime, runtime_id.as_str(), &accepted.run.run_id).unwrap();
        assert_eq!(completed.outcome, "succeeded");
        assert_eq!(completed.steps.len(), 2);
        assert_eq!(events.len(), 3);
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn queued_cancel_is_idempotent_and_terminal_run_can_retry() {
        let path = temp_database("cancel-retry");
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        runtime
            .register_runtime(runtime_id.clone(), "Runtime A", "https://runtime-a.invalid")
            .unwrap();
        let revision = plan_revision(
            runtime.runtime_projection(&runtime_id).unwrap().revision,
            "runtime_triage",
        );
        let accepted =
            run_command(&mut runtime, request(runtime_id.clone(), revision.clone())).unwrap();
        let cancel_request = OrchestraCancelCommandRequest {
            principal: Principal {
                id: "operator-a".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
            command_id: CommandId::new("orchestra-cancel-0001").unwrap(),
            runtime_id: runtime_id.clone(),
            run_id: accepted.run.run_id.clone(),
            confirmed: true,
        };
        let cancelled = cancel_command(&mut runtime, cancel_request.clone()).unwrap();
        assert_eq!(cancelled.run.outcome, "cancelled");
        assert!(!cancelled.replayed);
        assert_eq!(
            runtime
                .runtime_projection(&runtime_id)
                .unwrap()
                .refresh_status,
            RefreshStatus::Failed
        );
        drop(runtime);

        let mut runtime = ControlRuntime::open(&path).unwrap();
        assert_eq!(
            runtime
                .runtime_projection(&runtime_id)
                .unwrap()
                .refresh_status,
            RefreshStatus::Failed
        );
        let replay = cancel_command(&mut runtime, cancel_request).unwrap();
        assert_eq!(replay.run, cancelled.run);
        assert!(replay.replayed);

        let retry_revision = plan_revision(
            runtime.runtime_projection(&runtime_id).unwrap().revision,
            "runtime_triage",
        );
        let retry = retry_command(
            &mut runtime,
            OrchestraRetryCommandRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                command_id: CommandId::new("orchestra-retry-0001").unwrap(),
                runtime_id: runtime_id.clone(),
                run_id: cancelled.run.run_id.clone(),
                expected_plan_revision: retry_revision,
                confirmed: true,
                approved_by: None,
                approval_note: None,
            },
        )
        .unwrap();
        assert_eq!(retry.run.outcome, "queued");
        assert_eq!(retry.run.attempt, 2);
        assert_eq!(
            retry.run.retried_from_run_id.as_deref(),
            Some(cancelled.run.run_id.as_str())
        );
        drop(runtime);
        fs::remove_file(path).unwrap();
    }
}
