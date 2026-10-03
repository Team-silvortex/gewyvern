use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use leselang_hir::{CAPABILITY_UI_PRESENTATION, lower, validate_ui_node_id};
use leselang_observe::{execute_debugger_cancel, waiting_debugger_projection};
use leselang_syntax::parse;
use leselang_ui::{DebuggerFaultSummary, DebuggerProjection, DebuggerState, debugger_document};
use leselang_vm::{
    CancellationReason, DEFAULT_FUEL, EffectOperation, EffectRequest, EffectResult,
    PresentationOperation, PresentationResult, SchedulerLimits, Step, Vm,
};
use leserpent_domain::{
    CAPABILITY_DEBUGGER_CONTROL, CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ,
    CAPABILITY_RUNTIME_REFRESH, CapabilitySet, CommandEnvelope, CommandPlan, PlannedOperation,
    Principal, Revision, validate_debugger_session_id,
};
use leserpent_protocol::{
    DebuggerCancelResponse, DebuggerMutationStatus, DebuggerPresentationAcknowledgeRequest,
    DebuggerPresentationOutcome, DebuggerPresentationResponse, DebuggerPresentationStatus,
    DebuggerSessionResponse, DebuggerSessionStartRequest, DebuggerSessionView,
    DebuggerSessionsRequest, DebuggerSessionsResponse,
};
use ring::digest::{SHA256, digest};

const MAX_DEBUGGER_SESSIONS: usize = 32;
const MAX_RETAINED_DEBUGGER_JOURNALS: usize = 64;
const MAX_DEBUGGER_SOURCE_BYTES: usize = 64 * 1024;
const MIN_DEBUGGER_TIMEOUT_MS: u64 = 100;
const MAX_DEBUGGER_TIMEOUT_MS: u64 = 5 * 60 * 1_000;
const DEBUGGER_SCHEDULER_LIMITS: SchedulerLimits = SchedulerLimits {
    max_pending_dispatches: leselang_vm::MAX_MERGE_BRANCHES,
    max_active_leases: 1,
};

pub type SharedDebuggerAuthority = Arc<Mutex<DebuggerAuthority>>;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebuggerAuthorityError {
    code: &'static str,
    message: &'static str,
}

impl DebuggerAuthorityError {
    pub fn code(&self) -> &'static str {
        self.code
    }

    pub fn message(&self) -> &'static str {
        self.message
    }
}

struct DebuggerSession {
    principal_id: String,
    source_digest: [u8; 32],
    expected_revision: Option<Revision>,
    timeout_ms: u64,
    sequence: u64,
    journal_path: PathBuf,
    vm: Vm,
    request: EffectRequest,
    waiting_projection: DebuggerProjection,
    current_projection: DebuggerProjection,
    applied_cancel: Option<(CommandEnvelope, DebuggerCancelResponse)>,
    last_presentation: Option<(
        DebuggerPresentationAcknowledgeRequest,
        DebuggerPresentationResponse,
    )>,
}

pub struct DebuggerAuthority {
    journal_root: PathBuf,
    sessions: BTreeMap<String, DebuggerSession>,
    next_sequence: u64,
}

impl DebuggerAuthority {
    pub fn for_database(database: impl AsRef<Path>) -> Result<Self, String> {
        let database = database.as_ref();
        let file_name = database
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| "debugger journal database path is invalid".to_string())?;
        Self::open(database.with_file_name(format!("{file_name}.leselang-debugger")))
    }

    pub fn open(journal_root: impl AsRef<Path>) -> Result<Self, String> {
        let journal_root = journal_root.as_ref();
        fs::create_dir_all(journal_root)
            .map_err(|_| "cannot create debugger journal directory".to_string())?;
        let metadata = fs::symlink_metadata(journal_root)
            .map_err(|_| "cannot inspect debugger journal directory".to_string())?;
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return Err("debugger journal path must be a real directory".into());
        }
        #[cfg(unix)]
        fs::set_permissions(journal_root, fs::Permissions::from_mode(0o700))
            .map_err(|_| "cannot protect debugger journal directory".to_string())?;
        Ok(Self {
            journal_root: journal_root.to_path_buf(),
            sessions: BTreeMap::new(),
            next_sequence: 1,
        })
    }

    pub fn start_session(
        &mut self,
        request: DebuggerSessionStartRequest,
    ) -> Result<DebuggerSessionResponse, DebuggerAuthorityError> {
        authorize(&request.principal, &request.capabilities)?;
        validate_debugger_session_id(&request.session_id).map_err(|_| invalid_session())?;
        if request.source.is_empty()
            || request.source.len() > MAX_DEBUGGER_SOURCE_BYTES
            || request.source.contains('\0')
            || request.timeout_ms < MIN_DEBUGGER_TIMEOUT_MS
            || request.timeout_ms > MAX_DEBUGGER_TIMEOUT_MS
            || request
                .expected_revision
                .is_some_and(|revision| revision.0 == 0)
        {
            return Err(invalid_start());
        }
        let source_digest = source_digest(&request.source);
        let observed_at_ms = now_ms()?;
        if let Some(existing) = self.sessions.get_mut(&request.session_id) {
            refresh_session_at(existing, observed_at_ms)?;
            if existing.principal_id == request.principal.id
                && existing.source_digest == source_digest
                && existing.expected_revision == request.expected_revision
                && existing.timeout_ms == request.timeout_ms
            {
                return Ok(DebuggerSessionResponse {
                    session: view_session(existing)?,
                });
            }
            return Err(DebuggerAuthorityError {
                code: "debugger_session_conflict",
                message: "debugger session identity was reused with different input",
            });
        }
        self.refresh_sessions_at(observed_at_ms)?;
        if self.sessions.len() >= MAX_DEBUGGER_SESSIONS
            && self
                .sessions
                .values()
                .all(|session| session.current_projection.state == DebuggerState::WaitingEffect)
        {
            return Err(DebuggerAuthorityError {
                code: "debugger_session_capacity",
                message: "debugger session capacity is exhausted",
            });
        }

        let program = lower(&parse(&request.source)).map_err(|_| DebuggerAuthorityError {
            code: "debugger_source_invalid",
            message: "Leselang debugger source is invalid",
        })?;
        let mut preflight =
            Vm::new_with_limits(DEFAULT_FUEL, DEBUGGER_SCHEDULER_LIMITS).map_err(|_| {
                DebuggerAuthorityError {
                    code: "debugger_scheduler_invalid",
                    message: "debugger scheduler policy is invalid",
                }
            })?;
        let Step::Effect(preflight_effect) = preflight.start_timed(
            &program,
            request.principal.clone(),
            vm_capabilities(),
            request.expected_revision,
            observed_at_ms,
            request.timeout_ms,
        ) else {
            return Err(DebuggerAuthorityError {
                code: "debugger_session_not_suspended",
                message: "Leselang source did not suspend at one debuggable effect",
            });
        };
        waiting_debugger_projection(
            &preflight_effect,
            &request.session_id,
            request.expected_revision.unwrap_or(Revision(1)),
            observed_at_ms,
        )
        .map_err(|_| DebuggerAuthorityError {
            code: "debugger_projection_invalid",
            message: "debugger VM state could not be projected safely",
        })?;
        let journal_path = self
            .journal_root
            .join(format!("{}.sqlite", request.session_id));
        if journal_artifacts_exist(&journal_path)? {
            return Err(DebuggerAuthorityError {
                code: "debugger_session_recovery_required",
                message: "an unclaimed debugger journal already exists for this session",
            });
        }
        self.prune_terminal_session()?;
        if self.sessions.len() >= MAX_DEBUGGER_SESSIONS {
            return Err(DebuggerAuthorityError {
                code: "debugger_session_capacity",
                message: "debugger session capacity is exhausted",
            });
        }
        self.ensure_journal_capacity()?;
        let mut vm =
            Vm::open_journal_with_limits(&journal_path, DEFAULT_FUEL, DEBUGGER_SCHEDULER_LIMITS)
                .map_err(|_| DebuggerAuthorityError {
                    code: "debugger_journal_unavailable",
                    message: "debugger VM journal could not be opened",
                })?;
        let step = vm.start_timed(
            &program,
            request.principal.clone(),
            vm_capabilities(),
            request.expected_revision,
            observed_at_ms,
            request.timeout_ms,
        );
        let Step::Effect(effect) = step else {
            drop(vm);
            remove_journal_files(&journal_path)?;
            return Err(DebuggerAuthorityError {
                code: "debugger_session_not_suspended",
                message: "Leselang source did not suspend at one debuggable effect",
            });
        };
        let effect = *effect;
        if &effect != preflight_effect.as_ref() {
            drop(vm);
            remove_journal_files(&journal_path)?;
            return Err(DebuggerAuthorityError {
                code: "debugger_session_nondeterministic",
                message: "debugger VM preflight and durable start diverged",
            });
        }
        let projection = match waiting_debugger_projection(
            &effect,
            &request.session_id,
            request.expected_revision.unwrap_or(Revision(1)),
            observed_at_ms,
        ) {
            Ok(projection) => projection,
            Err(_) => {
                drop(vm);
                remove_journal_files(&journal_path)?;
                return Err(DebuggerAuthorityError {
                    code: "debugger_projection_invalid",
                    message: "debugger VM state could not be projected safely",
                });
            }
        };
        let response = DebuggerSessionResponse {
            session: view(&projection, Some(&effect))?,
        };
        let session = DebuggerSession {
            principal_id: request.principal.id,
            source_digest,
            expected_revision: request.expected_revision,
            timeout_ms: request.timeout_ms,
            sequence: self.next_sequence,
            journal_path,
            vm,
            request: effect,
            waiting_projection: projection.clone(),
            current_projection: projection.clone(),
            applied_cancel: None,
            last_presentation: None,
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        self.sessions.insert(request.session_id, session);
        Ok(response)
    }

    pub fn sessions(
        &mut self,
        request: DebuggerSessionsRequest,
    ) -> Result<DebuggerSessionsResponse, DebuggerAuthorityError> {
        authorize(&request.principal, &request.capabilities)?;
        if let Some(session_id) = &request.session_id {
            validate_debugger_session_id(session_id).map_err(|_| invalid_session())?;
        }
        self.refresh_sessions_at(now_ms()?)?;
        let sessions = self
            .sessions
            .iter()
            .filter(|(session_id, _)| {
                request
                    .session_id
                    .as_ref()
                    .is_none_or(|expected| expected == *session_id)
            })
            .map(|(_, session)| view_session(session))
            .collect::<Result<Vec<_>, _>>()?;
        if request.session_id.is_some() && sessions.is_empty() {
            return Err(DebuggerAuthorityError {
                code: "debugger_session_not_found",
                message: "debugger session was not found",
            });
        }
        Ok(DebuggerSessionsResponse { sessions })
    }

    pub fn acknowledge_presentation(
        &mut self,
        request: DebuggerPresentationAcknowledgeRequest,
    ) -> Result<DebuggerPresentationResponse, DebuggerAuthorityError> {
        authorize(&request.principal, &request.capabilities)?;
        if !request.capabilities.contains(CAPABILITY_UI_PRESENTATION)
            || request.expected_revision.0 == 0
            || validate_debugger_session_id(&request.session_id).is_err()
            || !valid_debugger_identifier(&request.effect_id)
        {
            return Err(DebuggerAuthorityError {
                code: "debugger_presentation_unauthorized",
                message: "live presentation requires explicit UI authority",
            });
        }

        let observed_at_ms = now_ms()?;
        let session = self
            .sessions
            .get_mut(&request.session_id)
            .ok_or(DebuggerAuthorityError {
                code: "debugger_session_not_found",
                message: "debugger session was not found",
            })?;
        refresh_session_at(session, observed_at_ms)?;
        if let Some((applied, response)) = &session.last_presentation
            && applied == &request
        {
            return Ok(response.clone());
        }
        if session.principal_id != request.principal.id {
            return Err(DebuggerAuthorityError {
                code: "debugger_presentation_unauthorized",
                message: "live presentation is bound to the issuing principal",
            });
        }
        if session.current_projection.state != DebuggerState::WaitingEffect {
            return Err(DebuggerAuthorityError {
                code: "debugger_session_not_waiting",
                message: "debugger session is no longer waiting",
            });
        }
        if session.current_projection.revision != request.expected_revision
            || session.request.effect_id != request.effect_id
        {
            return Err(DebuggerAuthorityError {
                code: "debugger_presentation_conflict",
                message: "live presentation coordinates changed before acknowledgement",
            });
        }
        let operation = pending_presentation(&session.request).ok_or(DebuggerAuthorityError {
            code: "debugger_effect_not_presentation",
            message: "pending debugger effect is not a UI presentation operation",
        })?;
        let expected_node_id = presentation_node_id(&operation)?;
        let next_revision = Revision(session.current_projection.revision.0.checked_add(1).ok_or(
            DebuggerAuthorityError {
                code: "debugger_revision_exhausted",
                message: "debugger session revision is exhausted",
            },
        )?);
        let effect_id = request.effect_id.clone();

        let (status, step, rejection_code) = match &request.outcome {
            DebuggerPresentationOutcome::Applied {
                node_id,
                focused_node_id,
            } => {
                if node_id != &expected_node_id {
                    return Err(DebuggerAuthorityError {
                        code: "debugger_presentation_conflict",
                        message: "live presentation target changed before acknowledgement",
                    });
                }
                let result = presentation_result(&operation, node_id, focused_node_id.as_deref())?;
                let continuation = session.request.continuation.clone();
                (
                    DebuggerPresentationStatus::Applied,
                    session.vm.resume_at(
                        &continuation,
                        observed_at_ms,
                        EffectResult::Presentation(result),
                    ),
                    None,
                )
            }
            DebuggerPresentationOutcome::Rejected { node_id, code } => {
                if node_id != &expected_node_id || !valid_failure_code(code) {
                    return Err(DebuggerAuthorityError {
                        code: "debugger_presentation_rejection_invalid",
                        message: "live presentation rejection is invalid",
                    });
                }
                let continuation = session.request.continuation.clone();
                let step = session.vm.cancel_effect(&continuation, observed_at_ms);
                if !matches!(
                    step,
                    Step::Cancelled(ref cancellation)
                        if cancellation.reason == CancellationReason::Requested
                ) {
                    return Err(DebuggerAuthorityError {
                        code: "debugger_presentation_rejection_failed",
                        message: "live presentation rejection did not consume the continuation",
                    });
                }
                (
                    DebuggerPresentationStatus::Rejected,
                    step,
                    Some(code.as_str()),
                )
            }
        };

        let mut next_request = None;
        let projection = match (status, step) {
            (DebuggerPresentationStatus::Applied, Step::Done(_)) => terminal_projection(
                &session.current_projection,
                next_revision,
                DebuggerState::Completed,
                None,
            )?,
            (DebuggerPresentationStatus::Applied, Step::Effect(effect)) => {
                let effect = *effect;
                let projection = waiting_debugger_projection(
                    &effect,
                    &request.session_id,
                    next_revision,
                    observed_at_ms,
                )
                .map_err(|_| DebuggerAuthorityError {
                    code: "debugger_presentation_reentry_failed",
                    message: "live presentation could not project the next VM effect",
                })?;
                next_request = Some(effect);
                projection
            }
            (DebuggerPresentationStatus::Applied, _) => terminal_projection(
                &session.current_projection,
                next_revision,
                DebuggerState::Failed,
                Some(DebuggerFaultSummary {
                    code: "debugger_presentation_reentry_failed".into(),
                    display: "live presentation did not re-enter the VM safely".into(),
                }),
            )?,
            (DebuggerPresentationStatus::Rejected, _) => terminal_projection(
                &session.current_projection,
                next_revision,
                DebuggerState::Failed,
                Some(DebuggerFaultSummary {
                    code: format!(
                        "debugger_presentation_{}",
                        rejection_code.unwrap_or("rejected")
                    ),
                    display: "the GUI adapter rejected the presentation operation".into(),
                }),
            )?,
        };
        let response = DebuggerPresentationResponse {
            effect_id,
            status,
            session: view(&projection, next_request.as_ref())?,
            acknowledged_at_ms: observed_at_ms,
        };
        session.current_projection = projection.clone();
        if let Some(effect) = next_request {
            session.request = effect;
            session.waiting_projection = projection;
        }
        session.last_presentation = Some((request, response.clone()));
        Ok(response)
    }

    pub fn cancel(
        &mut self,
        command: CommandEnvelope,
    ) -> Result<DebuggerCancelResponse, DebuggerAuthorityError> {
        let leserpent_domain::Command::DebuggerCancel { session_id } = &command.command else {
            return Err(invalid_session());
        };
        let observed_at_ms = now_ms()?;
        let session = self
            .sessions
            .get_mut(session_id)
            .ok_or(DebuggerAuthorityError {
                code: "debugger_session_not_found",
                message: "debugger session was not found",
            })?;
        refresh_session_at(session, observed_at_ms)?;
        if let Some((applied, response)) = &session.applied_cancel {
            return if applied == &command {
                Ok(response.clone())
            } else {
                Err(DebuggerAuthorityError {
                    code: "debugger_session_not_waiting",
                    message: "debugger session is no longer waiting",
                })
            };
        }
        if session.current_projection.state != DebuggerState::WaitingEffect {
            return Err(DebuggerAuthorityError {
                code: "debugger_session_not_waiting",
                message: "debugger session is no longer waiting",
            });
        }
        let plan = CommandPlan {
            schema_version: leserpent_domain::COMMAND_PLAN_SCHEMA_VERSION,
            required_capability: CAPABILITY_DEBUGGER_CONTROL.to_string(),
            operation: PlannedOperation::Command(command.clone()),
        };
        let result = execute_debugger_cancel(
            &plan,
            &session.waiting_projection,
            &session.request.continuation,
            &mut session.vm,
            observed_at_ms,
        )
        .map_err(|_| DebuggerAuthorityError {
            code: "debugger_cancel_rejected",
            message: "debugger cancellation was rejected",
        })?;
        let status = match result.status {
            leselang_observe::DebuggerMutationStatus::Planned => DebuggerMutationStatus::Planned,
            leselang_observe::DebuggerMutationStatus::Applied => DebuggerMutationStatus::Applied,
        };
        let projection = if status == DebuggerMutationStatus::Applied {
            let mut projection = session.waiting_projection.clone();
            projection.revision = Revision(projection.revision.0.checked_add(1).ok_or(
                DebuggerAuthorityError {
                    code: "debugger_revision_exhausted",
                    message: "debugger session revision is exhausted",
                },
            )?);
            projection.state = DebuggerState::Cancelled;
            projection.pending_effect = None;
            projection.deadline_remaining_ms = None;
            projection
        } else {
            session.waiting_projection.clone()
        };
        let response = DebuggerCancelResponse {
            command_id: command.command_id.clone(),
            status,
            session: view(
                &projection,
                (status == DebuggerMutationStatus::Planned).then_some(&session.request),
            )?,
            audited_at_ms: result.audited_at_ms,
        };
        if status == DebuggerMutationStatus::Applied {
            session.current_projection = projection;
            session.applied_cancel = Some((command, response.clone()));
        }
        Ok(response)
    }

    fn refresh_sessions_at(&mut self, observed_at_ms: u64) -> Result<(), DebuggerAuthorityError> {
        for session in self.sessions.values_mut() {
            refresh_session_at(session, observed_at_ms)?;
        }
        Ok(())
    }

    fn prune_terminal_session(&mut self) -> Result<(), DebuggerAuthorityError> {
        if self.sessions.len() < MAX_DEBUGGER_SESSIONS {
            return Ok(());
        }
        let oldest = self
            .sessions
            .iter()
            .filter(|(_, session)| session.current_projection.state != DebuggerState::WaitingEffect)
            .min_by_key(|(_, session)| session.sequence)
            .map(|(session_id, _)| session_id.clone());
        if let Some(session_id) = oldest
            && let Some(session) = self.sessions.remove(&session_id)
        {
            let journal_path = session.journal_path.clone();
            drop(session);
            remove_journal_files(&journal_path)?;
        }
        Ok(())
    }

    fn ensure_journal_capacity(&self) -> Result<(), DebuggerAuthorityError> {
        let active = self
            .sessions
            .values()
            .map(|session| session.journal_path.clone())
            .collect::<BTreeSet<_>>();
        let entries = fs::read_dir(&self.journal_root).map_err(|_| journal_cleanup_error())?;
        let mut total = 0usize;
        let mut removable = Vec::new();
        for entry in entries {
            let path = entry.map_err(|_| journal_cleanup_error())?.path();
            if path.extension().and_then(|extension| extension.to_str()) != Some("sqlite") {
                continue;
            }
            let metadata = fs::symlink_metadata(&path).map_err(|_| journal_cleanup_error())?;
            if !metadata.is_file() && !metadata.file_type().is_symlink() {
                return Err(journal_cleanup_error());
            }
            total = total.saturating_add(1);
            if !active.contains(&path) {
                let modified_at = metadata
                    .modified()
                    .ok()
                    .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
                    .map(|elapsed| elapsed.as_nanos())
                    .unwrap_or(0);
                removable.push((modified_at, path));
            }
        }
        let remove_count = total
            .saturating_add(1)
            .saturating_sub(MAX_RETAINED_DEBUGGER_JOURNALS);
        if remove_count == 0 {
            return Ok(());
        }
        removable.sort();
        if removable.len() < remove_count {
            return Err(DebuggerAuthorityError {
                code: "debugger_journal_capacity",
                message: "debugger journal retention capacity is exhausted",
            });
        }
        for (_, path) in removable.into_iter().take(remove_count) {
            remove_journal_files(&path)?;
        }
        Ok(())
    }
}

fn refresh_session_at(
    session: &mut DebuggerSession,
    observed_at_ms: u64,
) -> Result<(), DebuggerAuthorityError> {
    if session.current_projection.state != DebuggerState::WaitingEffect {
        return Ok(());
    }
    if session
        .request
        .continuation
        .deadline_at_ms
        .is_none_or(|deadline| observed_at_ms < deadline)
    {
        return Ok(());
    }

    let mut projection = waiting_debugger_projection(
        &session.request,
        &session.current_projection.session_id,
        session.current_projection.revision,
        observed_at_ms,
    )
    .map_err(|_| DebuggerAuthorityError {
        code: "debugger_projection_invalid",
        message: "debugger VM state could not be projected safely",
    })?;
    let step = session
        .vm
        .cancel_effect(&session.request.continuation, observed_at_ms);
    if !matches!(
        step,
        Step::Cancelled(ref cancellation)
            if cancellation.reason == CancellationReason::DeadlineExceeded
    ) {
        return Err(DebuggerAuthorityError {
            code: "debugger_deadline_convergence_failed",
            message: "debugger VM deadline did not converge",
        });
    }
    projection.revision = Revision(projection.revision.0.checked_add(1).ok_or(
        DebuggerAuthorityError {
            code: "debugger_revision_exhausted",
            message: "debugger session revision is exhausted",
        },
    )?);
    projection.state = DebuggerState::Failed;
    projection.pending_effect = None;
    projection.deadline_remaining_ms = None;
    projection.fault = Some(DebuggerFaultSummary {
        code: "debugger_deadline_exceeded".into(),
        display: "debugger effect deadline exceeded".into(),
    });
    view(&projection, None)?;
    session.current_projection = projection;
    Ok(())
}

fn terminal_projection(
    current: &DebuggerProjection,
    revision: Revision,
    state: DebuggerState,
    fault: Option<DebuggerFaultSummary>,
) -> Result<DebuggerProjection, DebuggerAuthorityError> {
    let mut projection = current.clone();
    projection.revision = revision;
    projection.state = state;
    projection.pending_effect = None;
    projection.deadline_remaining_ms = None;
    projection.fault = fault;
    view(&projection, None)?;
    Ok(projection)
}

fn presentation_node_id(
    operation: &PresentationOperation,
) -> Result<String, DebuggerAuthorityError> {
    let value = serde_json::to_value(operation).map_err(|_| presentation_result_error())?;
    let node_id = value
        .get("node_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(presentation_result_error)?;
    if !validate_ui_node_id(node_id) {
        return Err(presentation_result_error());
    }
    Ok(node_id.to_string())
}

fn presentation_result(
    operation: &PresentationOperation,
    node_id: &str,
    focused_node_id: Option<&str>,
) -> Result<PresentationResult, DebuggerAuthorityError> {
    let mut value = serde_json::to_value(operation).map_err(|_| presentation_result_error())?;
    let object = value
        .as_object_mut()
        .ok_or_else(presentation_result_error)?;
    if object.get("node_id").and_then(serde_json::Value::as_str) != Some(node_id)
        || !validate_ui_node_id(node_id)
    {
        return Err(presentation_result_error());
    }
    if matches!(operation, PresentationOperation::NavigateFocus { .. }) {
        let focused_node_id = focused_node_id
            .filter(|value| validate_ui_node_id(value))
            .ok_or_else(presentation_result_error)?;
        object.insert(
            "focused_node_id".into(),
            serde_json::Value::String(focused_node_id.to_string()),
        );
    } else if focused_node_id.is_some() {
        return Err(presentation_result_error());
    }
    serde_json::from_value(value).map_err(|_| presentation_result_error())
}

fn presentation_result_error() -> DebuggerAuthorityError {
    DebuggerAuthorityError {
        code: "debugger_presentation_result_invalid",
        message: "live presentation result does not match the pending VM effect",
    }
}

fn valid_debugger_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.' | b':'))
}

fn valid_failure_code(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .bytes()
            .next()
            .is_some_and(|byte| byte.is_ascii_lowercase())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

fn journal_artifacts_exist(path: &Path) -> Result<bool, DebuggerAuthorityError> {
    for candidate in journal_artifacts(path) {
        match fs::symlink_metadata(candidate) {
            Ok(_) => return Ok(true),
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(_) => {
                return Err(DebuggerAuthorityError {
                    code: "debugger_journal_unavailable",
                    message: "debugger VM journal path could not be inspected",
                });
            }
        }
    }
    Ok(false)
}

fn remove_journal_files(path: &Path) -> Result<(), DebuggerAuthorityError> {
    for candidate in journal_artifacts(path) {
        let metadata = match fs::symlink_metadata(&candidate) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => continue,
            Err(_) => return Err(journal_cleanup_error()),
        };
        if !metadata.is_file() && !metadata.file_type().is_symlink() {
            return Err(journal_cleanup_error());
        }
        fs::remove_file(candidate).map_err(|_| journal_cleanup_error())?;
    }
    Ok(())
}

fn journal_artifacts(path: &Path) -> [PathBuf; 4] {
    [
        path.to_path_buf(),
        journal_sidecar(path, "-journal"),
        journal_sidecar(path, "-wal"),
        journal_sidecar(path, "-shm"),
    ]
}

fn journal_sidecar(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_os_string();
    value.push(suffix);
    PathBuf::from(value)
}

fn journal_cleanup_error() -> DebuggerAuthorityError {
    DebuggerAuthorityError {
        code: "debugger_journal_cleanup_failed",
        message: "debugger journal retention could not be enforced safely",
    }
}

fn authorize(
    principal: &Principal,
    capabilities: &CapabilitySet,
) -> Result<(), DebuggerAuthorityError> {
    if principal.id.trim().is_empty() || !capabilities.contains(CAPABILITY_DEBUGGER_CONTROL) {
        return Err(DebuggerAuthorityError {
            code: "debugger_unauthorized",
            message: "debugger session access requires explicit authority",
        });
    }
    Ok(())
}

fn view_session(session: &DebuggerSession) -> Result<DebuggerSessionView, DebuggerAuthorityError> {
    view(
        &session.current_projection,
        (session.current_projection.state == DebuggerState::WaitingEffect)
            .then_some(&session.request),
    )
}

fn view(
    projection: &DebuggerProjection,
    request: Option<&EffectRequest>,
) -> Result<DebuggerSessionView, DebuggerAuthorityError> {
    Ok(DebuggerSessionView {
        projection: projection.clone(),
        document: debugger_document(projection).map_err(|_| DebuggerAuthorityError {
            code: "debugger_projection_invalid",
            message: "debugger VM state could not be projected safely",
        })?,
        pending_presentation: request.and_then(pending_presentation),
    })
}

fn pending_presentation(request: &EffectRequest) -> Option<PresentationOperation> {
    match &request.operation {
        EffectOperation::Presentation(envelope) => Some(envelope.operation.clone()),
        EffectOperation::Query(_) | EffectOperation::Command(_) => None,
    }
}

fn vm_capabilities() -> CapabilitySet {
    CapabilitySet::new([
        CAPABILITY_RUNTIME_READ,
        CAPABILITY_RUNTIME_REFRESH,
        CAPABILITY_RUNTIME_DEPLOY,
        CAPABILITY_DEBUGGER_CONTROL,
        CAPABILITY_UI_PRESENTATION,
    ])
}

fn source_digest(source: &str) -> [u8; 32] {
    let digest = digest(&SHA256, source.as_bytes());
    let mut output = [0_u8; 32];
    if let Some(bytes) = digest.as_ref().get(..output.len()) {
        output.copy_from_slice(bytes);
    }
    output
}

fn now_ms() -> Result<u64, DebuggerAuthorityError> {
    let elapsed =
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|_| DebuggerAuthorityError {
                code: "debugger_clock_invalid",
                message: "debugger authority clock is invalid",
            })?;
    u64::try_from(elapsed.as_millis()).map_err(|_| DebuggerAuthorityError {
        code: "debugger_clock_invalid",
        message: "debugger authority clock is invalid",
    })
}

fn invalid_session() -> DebuggerAuthorityError {
    DebuggerAuthorityError {
        code: "debugger_session_invalid",
        message: "debugger session identity is invalid",
    }
}

fn invalid_start() -> DebuggerAuthorityError {
    DebuggerAuthorityError {
        code: "debugger_start_invalid",
        message: "debugger session start request is invalid",
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU64, Ordering};

    use leserpent_domain::{
        Command, CommandId, CommandOrigin, Confirmation, DOMAIN_SCHEMA_VERSION, IdempotencyKey,
    };

    use super::*;

    static NEXT_ROOT: AtomicU64 = AtomicU64::new(1);

    struct TempRoot(PathBuf);

    impl TempRoot {
        fn new(label: &str) -> Self {
            let path = std::env::temp_dir().join(format!(
                "leserpent-debugger-{label}-{}-{}",
                std::process::id(),
                NEXT_ROOT.fetch_add(1, Ordering::Relaxed)
            ));
            Self(path)
        }
    }

    impl Drop for TempRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn start_request(session_id: &str) -> DebuggerSessionStartRequest {
        DebuggerSessionStartRequest {
            principal: Principal {
                id: "debugger-operator".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_DEBUGGER_CONTROL]),
            session_id: session_id.into(),
            source: "fn main() = runtime.inspect(runtime_id: \"runtime-a\")".into(),
            expected_revision: Some(Revision(7)),
            timeout_ms: 300_000,
        }
    }

    fn cancel_command(session_id: &str, dry_run: bool) -> CommandEnvelope {
        CommandEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            command_id: CommandId::new("debugger-command-a").unwrap(),
            idempotency_key: IdempotencyKey::new("debugger-idempotency-a").unwrap(),
            expected_revision: Some(Revision(7)),
            principal: Principal {
                id: "debugger-operator".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_DEBUGGER_CONTROL]),
            origin: CommandOrigin::Gui,
            confirmation: if dry_run {
                Confirmation::NotRequired
            } else {
                Confirmation::Confirmed
            },
            dry_run,
            command: Command::DebuggerCancel {
                session_id: session_id.into(),
            },
        }
    }

    fn presentation_acknowledgement(
        session_id: &str,
        outcome: DebuggerPresentationOutcome,
    ) -> DebuggerPresentationAcknowledgeRequest {
        DebuggerPresentationAcknowledgeRequest {
            principal: Principal {
                id: "debugger-operator".into(),
            },
            capabilities: CapabilitySet::new([
                CAPABILITY_DEBUGGER_CONTROL,
                CAPABILITY_UI_PRESENTATION,
            ]),
            session_id: session_id.into(),
            effect_id: "effect-1".into(),
            expected_revision: Revision(7),
            outcome,
        }
    }

    #[test]
    fn real_vm_session_projects_plans_cancels_and_replays() {
        let root = TempRoot::new("vertical");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let started = authority.start_session(start_request("session-a")).unwrap();
        assert_eq!(
            started.session.projection.state,
            DebuggerState::WaitingEffect
        );
        assert_eq!(started.session.projection.revision, Revision(7));
        assert_eq!(started.session.document.revision, Revision(7));
        assert!(
            serde_json::to_string(&started.session.document)
                .unwrap()
                .contains("debugger_cancel")
        );

        let listed = authority
            .sessions(DebuggerSessionsRequest {
                principal: Principal {
                    id: "debugger-operator".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_DEBUGGER_CONTROL]),
                session_id: Some("session-a".into()),
            })
            .unwrap();
        assert_eq!(listed.sessions, vec![started.session.clone()]);

        let planned = authority.cancel(cancel_command("session-a", true)).unwrap();
        assert_eq!(planned.status, DebuggerMutationStatus::Planned);
        assert_eq!(
            planned.session.projection.state,
            DebuggerState::WaitingEffect
        );
        assert!(planned.audited_at_ms.is_none());

        let command = cancel_command("session-a", false);
        let applied = authority.cancel(command.clone()).unwrap();
        assert_eq!(applied.status, DebuggerMutationStatus::Applied);
        assert_eq!(applied.session.projection.state, DebuggerState::Cancelled);
        assert_eq!(applied.session.projection.revision, Revision(8));
        assert!(applied.audited_at_ms.is_some());
        assert_eq!(authority.cancel(command).unwrap(), applied);

        let mut conflicting = cancel_command("session-a", false);
        conflicting.command_id = CommandId::new("debugger-command-b").unwrap();
        conflicting.idempotency_key = IdempotencyKey::new("debugger-idempotency-b").unwrap();
        assert_eq!(
            authority.cancel(conflicting).unwrap_err().code(),
            "debugger_session_not_waiting"
        );
    }

    #[test]
    fn live_presentation_reenters_the_rust_vm_and_replays_idempotently() {
        let root = TempRoot::new("live-presentation");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-presentation");
        request.source = "fn main() = ui.assert_visible(node_id: \"remote-fleet\")".into();
        let started = authority.start_session(request).unwrap();
        assert_eq!(
            started.session.pending_presentation,
            Some(PresentationOperation::AssertVisible {
                node_id: "remote-fleet".into(),
            })
        );

        let acknowledgement = presentation_acknowledgement(
            "session-presentation",
            DebuggerPresentationOutcome::Applied {
                node_id: "remote-fleet".into(),
                focused_node_id: None,
            },
        );
        let advanced = authority
            .acknowledge_presentation(acknowledgement.clone())
            .unwrap();
        assert_eq!(advanced.status, DebuggerPresentationStatus::Applied);
        assert_eq!(advanced.effect_id, "effect-1");
        assert_eq!(advanced.session.projection.state, DebuggerState::Completed);
        assert_eq!(advanced.session.projection.revision, Revision(8));
        assert!(advanced.session.pending_presentation.is_none());
        assert_eq!(
            authority.acknowledge_presentation(acknowledgement).unwrap(),
            advanced
        );

        let mut unauthorized = presentation_acknowledgement(
            "session-presentation",
            DebuggerPresentationOutcome::Applied {
                node_id: "remote-fleet".into(),
                focused_node_id: None,
            },
        );
        unauthorized.capabilities = CapabilitySet::new([CAPABILITY_DEBUGGER_CONTROL]);
        assert_eq!(
            authority
                .acknowledge_presentation(unauthorized)
                .unwrap_err()
                .code(),
            "debugger_presentation_unauthorized"
        );
    }

    #[test]
    fn sequential_gui_program_reenters_one_native_operation_at_a_time() {
        let root = TempRoot::new("sequential-presentation");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-sequence");
        request.source = r#"fn main() = seq(
            focus: ui.focus(node_id: "runtime-a"),
            verify: repeat(times: 2, body: ui.assert_visible(node_id: "runtime-a"))
        )"#
        .into();
        let mut view = authority.start_session(request).unwrap().session;
        assert!(matches!(
            view.pending_presentation,
            Some(PresentationOperation::Focus { .. })
        ));
        for index in 0..3 {
            assert_eq!(view.projection.state, DebuggerState::WaitingEffect);
            let mut acknowledgement = presentation_acknowledgement(
                "session-sequence",
                DebuggerPresentationOutcome::Applied {
                    node_id: "runtime-a".into(),
                    focused_node_id: None,
                },
            );
            acknowledgement.effect_id = authority.sessions["session-sequence"]
                .request
                .effect_id
                .clone();
            acknowledgement.expected_revision = view.projection.revision;
            let response = authority
                .acknowledge_presentation(acknowledgement.clone())
                .unwrap();
            assert_eq!(
                authority.acknowledge_presentation(acknowledgement).unwrap(),
                response
            );
            view = response.session;
            assert_eq!(view.projection.revision, Revision(8 + index));
            if index < 2 {
                assert!(matches!(
                    view.pending_presentation,
                    Some(PresentationOperation::AssertVisible { .. })
                ));
            }
        }
        assert_eq!(view.projection.state, DebuggerState::Completed);
        assert!(view.pending_presentation.is_none());
        assert_eq!(authority.sessions["session-sequence"].vm.pending_count(), 0);
    }

    #[test]
    fn computed_condition_selects_one_gui_flow_through_native_acknowledgements() {
        let root = TempRoot::new("computed-presentation");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-computed");
        request.source = r#"fn main() = bind(count: add(left: 2, right: 3), body:
            choose(when: eq(left: count, right: 5),
                then: seq(focus: ui.focus(node_id: "runtime-a"), verify: ui.assert_visible(node_id: "runtime-a")),
                otherwise: seq(unselected: ui.focus(node_id: "never"))))"#.into();
        let mut view = authority.start_session(request).unwrap().session;
        assert!(
            matches!(view.pending_presentation, Some(PresentationOperation::Focus { ref node_id }) if node_id == "runtime-a")
        );
        for _ in 0..2 {
            let mut acknowledgement = presentation_acknowledgement(
                "session-computed",
                DebuggerPresentationOutcome::Applied {
                    node_id: "runtime-a".into(),
                    focused_node_id: None,
                },
            );
            acknowledgement.effect_id = authority.sessions["session-computed"]
                .request
                .effect_id
                .clone();
            acknowledgement.expected_revision = view.projection.revision;
            view = authority
                .acknowledge_presentation(acknowledgement)
                .unwrap()
                .session;
        }
        assert_eq!(view.projection.state, DebuggerState::Completed);
        assert_eq!(authority.sessions["session-computed"].vm.pending_count(), 0);
        let mut pure = start_request("session-pure");
        pure.source = "fn main() = add(left: 2, right: 3)".into();
        assert_eq!(
            authority.start_session(pure).unwrap_err().code(),
            "debugger_session_not_suspended"
        );
    }

    #[test]
    fn computed_node_arguments_use_the_existing_native_acknowledgement_fence() {
        let root = TempRoot::new("computed-arguments");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-arguments");
        request.source = r#"fn main() = bind(node: concat(left: "runtime-", right: "a"), body: ui.focus(node_id: node))"#.into();
        let started = authority.start_session(request.clone()).unwrap();
        assert_eq!(authority.start_session(request).unwrap(), started);
        assert!(
            matches!(&started.session.pending_presentation, Some(PresentationOperation::Focus { node_id }) if node_id == "runtime-a")
        );
        let mut acknowledgement = presentation_acknowledgement(
            "session-arguments",
            DebuggerPresentationOutcome::Applied {
                node_id: "wrong-node".into(),
                focused_node_id: None,
            },
        );
        assert_eq!(
            authority
                .acknowledge_presentation(acknowledgement.clone())
                .unwrap_err()
                .code(),
            "debugger_presentation_conflict"
        );
        acknowledgement.outcome = DebuggerPresentationOutcome::Applied {
            node_id: "runtime-a".into(),
            focused_node_id: None,
        };
        let completed = authority
            .acknowledge_presentation(acknowledgement.clone())
            .unwrap();
        assert_eq!(
            authority.acknowledge_presentation(acknowledgement).unwrap(),
            completed
        );
        assert_eq!(completed.session.projection.state, DebuggerState::Completed);
        assert_eq!(
            authority.sessions["session-arguments"].vm.pending_count(),
            0
        );

        let mut invalid = start_request("session-invalid-arguments");
        invalid.source =
            r#"fn main() = ui.focus(node_id: concat(left: "bad", right: " node"))"#.into();
        assert!(authority.start_session(invalid).is_err());
        assert!(!authority.sessions.contains_key("session-invalid-arguments"));
        assert!(
            !journal_artifacts_exist(
                &authority
                    .journal_root
                    .join("session-invalid-arguments.sqlite")
            )
            .unwrap()
        );
    }

    #[test]
    fn native_navigation_result_reenters_typed_computation_without_exposing_locals() {
        let root = TempRoot::new("result-binding");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-result-binding");
        request.source = r#"fn main() = bind(expected: "runtime-b", body:
            bind(moved: ui.navigate_focus(node_id: "runtime-a", direction: "next"), body:
                eq(left: field(value: moved, name: "focused_node_id"), right: expected)))"#
            .into();
        let started = authority.start_session(request).unwrap();
        assert!(matches!(
            started.session.pending_presentation,
            Some(PresentationOperation::NavigateFocus { .. })
        ));
        let image = authority.sessions["session-result-binding"]
            .request
            .continuation
            .clone();
        assert!(image.result_binding.is_some());
        let mut acknowledgement = presentation_acknowledgement(
            "session-result-binding",
            DebuggerPresentationOutcome::Applied {
                node_id: "wrong-origin".into(),
                focused_node_id: Some("runtime-b".into()),
            },
        );
        assert_eq!(
            authority
                .acknowledge_presentation(acknowledgement.clone())
                .unwrap_err()
                .code(),
            "debugger_presentation_conflict"
        );
        assert_eq!(
            authority.sessions["session-result-binding"]
                .vm
                .pending_count(),
            1
        );
        acknowledgement.outcome = DebuggerPresentationOutcome::Applied {
            node_id: "runtime-a".into(),
            focused_node_id: Some("runtime-b".into()),
        };
        let completed = authority
            .acknowledge_presentation(acknowledgement.clone())
            .unwrap();
        assert_eq!(
            authority.acknowledge_presentation(acknowledgement).unwrap(),
            completed
        );
        assert_eq!(completed.session.projection.state, DebuggerState::Completed);
        let encoded = serde_json::to_string(&completed).unwrap();
        assert!(!encoded.contains("result_binding"));
        assert!(!encoded.contains("focused_node_id"));
        let session = authority
            .sessions
            .get_mut("session-result-binding")
            .unwrap();
        assert_eq!(session.vm.pending_count(), 0);
        assert_eq!(
            session.vm.resume_at(
                &image,
                image.deadline_at_ms.unwrap() - 1,
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "ignored-replay".into()
                })
            ),
            Step::Done(leselang_vm::Value::Scalar {
                value: leselang_vm::ScalarValue::Boolean(true)
            })
        );
    }

    #[test]
    fn native_result_loops_complete_or_fail_without_another_presentation() {
        for (destination, expected) in [
            ("runtime-b", DebuggerState::Completed),
            ("runtime-c", DebuggerState::Failed),
        ] {
            let root = TempRoot::new("result-loop");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-result-loop");
            request.source = r#"fn main() = bind(moved:
                ui.navigate_focus(node_id: "runtime-a", direction: "next"), body:
                loop(matched: false, while: not(value: matched),
                    next: eq(left: field(value: moved, name: "focused_node_id"), right: "runtime-b"),
                    limit: 1))"#.into();
            authority.start_session(request).unwrap();
            let image = authority.sessions["session-result-loop"]
                .request
                .continuation
                .clone();
            let acknowledgement = presentation_acknowledgement(
                "session-result-loop",
                DebuggerPresentationOutcome::Applied {
                    node_id: "runtime-a".into(),
                    focused_node_id: Some(destination.into()),
                },
            );
            let terminal = authority
                .acknowledge_presentation(acknowledgement.clone())
                .unwrap();
            assert_eq!(terminal.session.projection.state, expected);
            assert!(terminal.session.pending_presentation.is_none());
            assert_eq!(
                authority.acknowledge_presentation(acknowledgement).unwrap(),
                terminal
            );
            let session = authority.sessions.get_mut("session-result-loop").unwrap();
            assert_eq!(session.vm.pending_count(), 0);
            let replay = session.vm.resume_at(
                &image,
                image.deadline_at_ms.unwrap() - 1,
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "ignored".into(),
                }),
            );
            if expected == DebuggerState::Completed {
                assert_eq!(
                    replay,
                    Step::Done(leselang_vm::Value::Scalar {
                        value: leselang_vm::ScalarValue::Boolean(true)
                    })
                );
            } else {
                assert!(matches!(replay, Step::Fault(fault) if fault.code == "LSV1406"));
            }
        }
    }

    #[test]
    fn native_result_driven_successor_advances_with_correlated_gui_acknowledgements() {
        let root = TempRoot::new("result-successor");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-successor");
        request.source = r#"fn main() = bind(moved:
            ui.navigate_focus(node_id: "runtime-a", direction: "next"), body:
            ui.focus(node_id: field(value: moved, name: "focused_node_id")))"#
            .into();
        let started = authority.start_session(request).unwrap();
        let first_ack = presentation_acknowledgement(
            "session-successor",
            DebuggerPresentationOutcome::Applied {
                node_id: "runtime-a".into(),
                focused_node_id: Some("runtime-b".into()),
            },
        );
        let advanced = authority
            .acknowledge_presentation(first_ack.clone())
            .unwrap();
        assert_eq!(
            advanced.session.projection.revision,
            Revision(started.session.projection.revision.0 + 1)
        );
        assert!(matches!(&advanced.session.pending_presentation,
            Some(PresentationOperation::Focus { node_id }) if node_id == "runtime-b"));
        assert_eq!(
            authority
                .acknowledge_presentation(first_ack.clone())
                .unwrap(),
            advanced
        );
        let mut stale = first_ack.clone();
        stale.outcome = DebuggerPresentationOutcome::Applied {
            node_id: "runtime-a".into(),
            focused_node_id: Some("runtime-c".into()),
        };
        assert_eq!(
            authority
                .acknowledge_presentation(stale)
                .unwrap_err()
                .code(),
            "debugger_presentation_conflict"
        );
        let mut next_ack = presentation_acknowledgement(
            "session-successor",
            DebuggerPresentationOutcome::Applied {
                node_id: "runtime-b".into(),
                focused_node_id: None,
            },
        );
        next_ack.effect_id = authority.sessions["session-successor"]
            .request
            .effect_id
            .clone();
        assert_ne!(next_ack.effect_id, first_ack.effect_id);
        next_ack.expected_revision = advanced.session.projection.revision;
        let completed = authority
            .acknowledge_presentation(next_ack.clone())
            .unwrap();
        assert_eq!(completed.session.projection.state, DebuggerState::Completed);
        assert!(completed.session.pending_presentation.is_none());
        assert_eq!(
            authority.acknowledge_presentation(next_ack).unwrap(),
            completed
        );
        assert_eq!(
            authority.sessions["session-successor"].vm.pending_count(),
            0
        );
        assert!(
            !serde_json::to_string(&advanced)
                .unwrap()
                .contains("result_binding")
        );
    }

    #[test]
    fn native_dataflow_chain_reuses_prior_results_and_fences_each_gui_step() {
        let root = TempRoot::new("dataflow");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-dataflow");
        request.source = r#"fn main() = bind(first:
            ui.navigate_focus(node_id: "runtime-a", direction: "next"), body:
            bind(second: ui.focus(node_id: field(value: first, name: "focused_node_id")), body:
                bind(third: ui.focus(node_id: field(value: first, name: "node_id")), body:
                    eq(left: field(value: second, name: "node_id"), right: field(value: first, name: "focused_node_id")))))"#.into();
        let mut view = authority.start_session(request).unwrap().session;
        let mut prior_ack: Option<DebuggerPresentationAcknowledgeRequest> = None;
        for (index, (node, destination)) in [
            ("runtime-a", Some("runtime-b")),
            ("runtime-b", None),
            ("runtime-a", None),
        ]
        .into_iter()
        .enumerate()
        {
            let current = &authority.sessions["session-dataflow"].request;
            assert_eq!(current.continuation.schema_version, 4);
            let mut ack = presentation_acknowledgement(
                "session-dataflow",
                DebuggerPresentationOutcome::Applied {
                    node_id: node.into(),
                    focused_node_id: destination.map(str::to_string),
                },
            );
            ack.effect_id = current.effect_id.clone();
            ack.expected_revision = view.projection.revision;
            if let Some(mut stale) = prior_ack.take() {
                stale.outcome = DebuggerPresentationOutcome::Applied {
                    node_id: "wrong".into(),
                    focused_node_id: None,
                };
                assert_eq!(
                    authority
                        .acknowledge_presentation(stale)
                        .unwrap_err()
                        .code(),
                    "debugger_presentation_conflict"
                );
            }
            let response = authority.acknowledge_presentation(ack.clone()).unwrap();
            assert_eq!(
                authority.acknowledge_presentation(ack.clone()).unwrap(),
                response
            );
            assert_eq!(
                response.session.projection.revision,
                Revision(8 + index as u64)
            );
            assert!(
                !serde_json::to_string(&response)
                    .unwrap()
                    .contains("result_binding")
            );
            prior_ack = Some(ack);
            view = response.session;
        }
        assert_eq!(view.projection.state, DebuggerState::Completed);
        assert!(view.pending_presentation.is_none());
        assert_eq!(authority.sessions["session-dataflow"].vm.pending_count(), 0);
    }

    #[test]
    fn native_conditional_exit_completes_without_a_phantom_presentation() {
        for destination in ["runtime-home", "runtime-b"] {
            let root = TempRoot::new("conditional-exit");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-conditional");
            request.source = r#"fn main() = bind(moved:
                ui.navigate_focus(node_id: "runtime-a", direction: "next"), body:
                choose(when: eq(left: field(value: moved, name: "focused_node_id"), right: "runtime-home"), then: true,
                    otherwise: bind(focused: ui.focus(node_id: field(value: moved, name: "focused_node_id")), body:
                        eq(left: field(value: focused, name: "node_id"), right: field(value: moved, name: "focused_node_id")))))"#.into();
            let initial = authority.start_session(request).unwrap().session;
            let current = &authority.sessions["session-conditional"].request;
            assert_eq!(
                current.continuation.schema_version,
                leselang_vm::CONDITIONAL_CONTINUATION_SCHEMA_VERSION
            );
            let mut ack = presentation_acknowledgement(
                "session-conditional",
                DebuggerPresentationOutcome::Applied {
                    node_id: "runtime-a".into(),
                    focused_node_id: Some(destination.into()),
                },
            );
            ack.effect_id = current.effect_id.clone();
            ack.expected_revision = initial.projection.revision;
            let mut response = authority.acknowledge_presentation(ack.clone()).unwrap();
            assert_eq!(
                authority.acknowledge_presentation(ack.clone()).unwrap(),
                response
            );
            assert_eq!(response.session.projection.revision, Revision(8));
            if destination == "runtime-b" {
                assert!(response.session.pending_presentation.is_some());
                let current = &authority.sessions["session-conditional"].request;
                let mut next = presentation_acknowledgement(
                    "session-conditional",
                    DebuggerPresentationOutcome::Applied {
                        node_id: destination.into(),
                        focused_node_id: None,
                    },
                );
                next.effect_id = current.effect_id.clone();
                next.expected_revision = response.session.projection.revision;
                response = authority.acknowledge_presentation(next.clone()).unwrap();
                assert_eq!(authority.acknowledge_presentation(next).unwrap(), response);
                assert_eq!(response.session.projection.revision, Revision(9));
            }
            ack.outcome = DebuggerPresentationOutcome::Applied {
                node_id: "runtime-a".into(),
                focused_node_id: Some("changed".into()),
            };
            assert_eq!(
                authority.acknowledge_presentation(ack).unwrap_err().code(),
                "debugger_session_not_waiting"
            );
            assert_eq!(response.session.projection.state, DebuggerState::Completed);
            assert!(response.session.pending_presentation.is_none());
            assert_eq!(
                authority.sessions["session-conditional"].vm.pending_count(),
                0
            );
            assert!(
                !serde_json::to_string(&response)
                    .unwrap()
                    .contains("result_binding")
            );
        }
    }

    #[test]
    fn native_numeric_result_converts_into_form_text_with_correlated_acknowledgements() {
        let root = TempRoot::new("conversion");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-conversion");
        request.source = r#"fn main() = bind(counted:
            ui.assert_child_count(node_id: "rows", count: "7"), body:
            ui.set_form_value(node_id: "form", field: "replicas",
                value: to_string(value: add(left: field(value: counted, name: "count"), right: 1))))"#.into();
        let started = authority.start_session(request).unwrap();
        assert!(matches!(&started.session.pending_presentation,
            Some(PresentationOperation::AssertChildCount { node_id, count }) if node_id == "rows" && *count == 7));
        let mut first_ack = presentation_acknowledgement(
            "session-conversion",
            DebuggerPresentationOutcome::Applied {
                node_id: "rows".into(),
                focused_node_id: None,
            },
        );
        first_ack.effect_id = authority.sessions["session-conversion"]
            .request
            .effect_id
            .clone();
        first_ack.expected_revision = started.session.projection.revision;
        let advanced = authority
            .acknowledge_presentation(first_ack.clone())
            .unwrap();
        assert_eq!(advanced.session.projection.revision, Revision(8));
        assert!(matches!(&advanced.session.pending_presentation,
            Some(PresentationOperation::SetFormValue { node_id, field, value })
                if node_id == "form" && field == "replicas" && value == "8"));
        assert_eq!(
            authority
                .acknowledge_presentation(first_ack.clone())
                .unwrap(),
            advanced
        );
        let current = &authority.sessions["session-conversion"].request;
        assert_ne!(current.effect_id, first_ack.effect_id);
        let mut next_ack = presentation_acknowledgement(
            "session-conversion",
            DebuggerPresentationOutcome::Applied {
                node_id: "wrong-form".into(),
                focused_node_id: None,
            },
        );
        next_ack.effect_id = current.effect_id.clone();
        next_ack.expected_revision = advanced.session.projection.revision;
        assert_eq!(
            authority
                .acknowledge_presentation(next_ack.clone())
                .unwrap_err()
                .code(),
            "debugger_presentation_conflict"
        );
        next_ack.outcome = DebuggerPresentationOutcome::Applied {
            node_id: "form".into(),
            focused_node_id: None,
        };
        let completed = authority
            .acknowledge_presentation(next_ack.clone())
            .unwrap();
        assert_eq!(
            authority.acknowledge_presentation(next_ack).unwrap(),
            completed
        );
        assert_eq!(completed.session.projection.revision, Revision(9));
        assert_eq!(completed.session.projection.state, DebuggerState::Completed);
        assert!(completed.session.pending_presentation.is_none());
        assert_eq!(
            authority.sessions["session-conversion"].vm.pending_count(),
            0
        );
        assert!(
            !serde_json::to_string(&advanced)
                .unwrap()
                .contains("result_binding")
        );
    }

    #[test]
    fn native_pure_helpers_connect_result_projection_to_correlated_form_actions() {
        let root = TempRoot::new("pure-helpers");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-helpers");
        request.source =
            r#"fn next_count(count: integer) = to_string(value: add(left: count, right: 1))
            fn main() = bind(counted: ui.assert_child_count(node_id: "rows", count: "7"),
                body: ui.set_form_value(node_id: "form", field: "replicas",
                    value: next_count(count: field(value: counted, name: "count"))))"#
                .into();
        let started = authority.start_session(request).unwrap();
        assert!(matches!(&started.session.pending_presentation,
            Some(PresentationOperation::AssertChildCount { node_id, count }) if node_id == "rows" && *count == 7));
        let mut first_ack = presentation_acknowledgement(
            "session-helpers",
            DebuggerPresentationOutcome::Applied {
                node_id: "rows".into(),
                focused_node_id: None,
            },
        );
        first_ack.effect_id = authority.sessions["session-helpers"]
            .request
            .effect_id
            .clone();
        first_ack.expected_revision = started.session.projection.revision;
        let advanced = authority
            .acknowledge_presentation(first_ack.clone())
            .unwrap();
        assert!(matches!(&advanced.session.pending_presentation,
            Some(PresentationOperation::SetFormValue { node_id, field, value })
                if node_id == "form" && field == "replicas" && value == "8"));
        assert_eq!(
            authority.acknowledge_presentation(first_ack).unwrap(),
            advanced
        );
        let current = &authority.sessions["session-helpers"].request;
        let mut final_ack = presentation_acknowledgement(
            "session-helpers",
            DebuggerPresentationOutcome::Applied {
                node_id: "wrong-form".into(),
                focused_node_id: None,
            },
        );
        final_ack.effect_id = current.effect_id.clone();
        final_ack.expected_revision = advanced.session.projection.revision;
        assert_eq!(
            authority
                .acknowledge_presentation(final_ack.clone())
                .unwrap_err()
                .code(),
            "debugger_presentation_conflict"
        );
        final_ack.outcome = DebuggerPresentationOutcome::Applied {
            node_id: "form".into(),
            focused_node_id: None,
        };
        let terminal = authority
            .acknowledge_presentation(final_ack.clone())
            .unwrap();
        assert_eq!(terminal.session.projection.state, DebuggerState::Completed);
        assert_eq!(
            authority.acknowledge_presentation(final_ack).unwrap(),
            terminal
        );
        assert_eq!(authority.sessions["session-helpers"].vm.pending_count(), 0);
        assert!(
            !serde_json::to_string(&advanced)
                .unwrap()
                .contains("result_binding")
        );
    }

    #[test]
    fn native_boolean_projections_select_actions_with_correlated_acknowledgements() {
        for (selection, requirement, destination) in [
            ("selected", "optional", "run"),
            ("unselected", "required", "skip"),
        ] {
            let root = TempRoot::new("boolean-projections");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-booleans");
            request.source = format!(
                r#"fn target(selected: boolean, required: boolean) =
                    choose(when: and(left: selected, right: not(value: required)), then: "run", otherwise: "skip")
                fn main() = bind(selection: ui.set_selection(node_id: "toggle", state: "{selection}"), body:
                    bind(requirement: ui.assert_form_field_required(node_id: "form", field: "name", state: "{requirement}"),
                        body: ui.focus(node_id: target(selected: field(value: selection, name: "selected"),
                            required: field(value: requirement, name: "required")))))"#
            );
            let mut response = authority.start_session(request).unwrap();
            assert!(matches!(
                response.session.pending_presentation,
                Some(PresentationOperation::SetSelection { .. })
            ));
            for (index, node) in ["toggle", "form", destination].into_iter().enumerate() {
                let mut acknowledgement = presentation_acknowledgement(
                    "session-booleans",
                    DebuggerPresentationOutcome::Applied {
                        node_id: node.into(),
                        focused_node_id: None,
                    },
                );
                acknowledgement.effect_id = authority.sessions["session-booleans"]
                    .request
                    .effect_id
                    .clone();
                acknowledgement.expected_revision = response.session.projection.revision;
                let mut wrong = acknowledgement.clone();
                wrong.outcome = DebuggerPresentationOutcome::Applied {
                    node_id: "wrong-target".into(),
                    focused_node_id: None,
                };
                assert_eq!(
                    authority
                        .acknowledge_presentation(wrong)
                        .unwrap_err()
                        .code(),
                    "debugger_presentation_conflict"
                );
                let advanced = authority
                    .acknowledge_presentation(acknowledgement.clone())
                    .unwrap();
                assert_eq!(
                    authority.acknowledge_presentation(acknowledgement).unwrap(),
                    advanced
                );
                response.session = advanced.session;
                assert_eq!(
                    response.session.projection.revision,
                    Revision(8 + index as u64)
                );
                if index == 0 {
                    assert!(matches!(
                        response.session.pending_presentation,
                        Some(PresentationOperation::AssertFormFieldRequired { .. })
                    ));
                } else if index == 1 {
                    assert!(matches!(&response.session.pending_presentation,
                        Some(PresentationOperation::Focus { node_id }) if node_id == destination));
                }
            }
            assert_eq!(response.session.projection.state, DebuggerState::Completed);
            assert!(response.session.pending_presentation.is_none());
            assert_eq!(authority.sessions["session-booleans"].vm.pending_count(), 0);
            assert!(
                !serde_json::to_string(&response)
                    .unwrap()
                    .contains("result_binding")
            );
        }
    }

    #[test]
    fn native_text_receipts_feed_form_fields_without_exposing_private_frames() {
        let root = TempRoot::new("text-projections");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-text");
        request.source = r#"fn suffix(text: string) = concat(left: text, right: "-copy")
            fn main() = bind(written: ui.set_form_value(node_id: "form", field: "name", value: "alpha"), body:
                bind(alias: written, body:
                    bind(checked: ui.wait_form_value(node_id: field(value: written, name: "node_id"), field: field(value: alias, name: "field"), expected: field(value: alias, name: "value")), body:
                        bind(last: ui.set_form_value(node_id: "form", field: "label", value: suffix(text: field(value: checked, name: "expected"))), body:
                            eq(left: field(value: last, name: "value"), right: suffix(text: field(value: written, name: "value")))))))"#.into();
        let mut view = authority.start_session(request).unwrap().session;
        let first = authority.sessions["session-text"]
            .request
            .continuation
            .clone();
        for index in 0..3 {
            let current = &authority.sessions["session-text"].request;
            if index > 0 {
                assert!(
                    current
                        .continuation
                        .result_binding
                        .as_ref()
                        .unwrap()
                        .results
                        .iter()
                        .all(|saved| saved.result.projection_version == 3)
                );
            }
            if index == 1 {
                assert!(
                    matches!(&view.pending_presentation, Some(PresentationOperation::WaitFormValue { field, expected, .. }) if field == "name" && expected == "alpha")
                );
            } else if index == 2 {
                assert!(
                    matches!(&view.pending_presentation, Some(PresentationOperation::SetFormValue { field, value, .. }) if field == "label" && value == "alpha-copy")
                );
            }
            let mut ack = presentation_acknowledgement(
                "session-text",
                DebuggerPresentationOutcome::Applied {
                    node_id: "wrong-node".into(),
                    focused_node_id: None,
                },
            );
            ack.effect_id = current.effect_id.clone();
            ack.expected_revision = view.projection.revision;
            assert_eq!(
                authority
                    .acknowledge_presentation(ack.clone())
                    .unwrap_err()
                    .code(),
                "debugger_presentation_conflict"
            );
            ack.outcome = DebuggerPresentationOutcome::Applied {
                node_id: "form".into(),
                focused_node_id: None,
            };
            let response = authority.acknowledge_presentation(ack.clone()).unwrap();
            assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
            assert_eq!(response.session.projection.revision, Revision(8 + index));
            let public = serde_json::to_string(&response).unwrap();
            assert!(!public.contains("result_binding") && !public.contains("projection_version"));
            view = response.session;
        }
        assert_eq!(view.projection.state, DebuggerState::Completed);
        assert!(view.pending_presentation.is_none());
        let session = authority.sessions.get_mut("session-text").unwrap();
        assert_eq!(session.vm.pending_count(), 0);
        assert_eq!(
            session.vm.resume_at(
                &first,
                now_ms().unwrap(),
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "ignored".into()
                })
            ),
            Step::Done(leselang_vm::Value::Scalar {
                value: leselang_vm::ScalarValue::Boolean(true)
            })
        );
    }

    #[test]
    fn native_kind_receipts_drive_typed_successors_without_exposing_private_frames() {
        let root = TempRoot::new("kind-projections");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-kind");
        request.source = r#"fn destination(kind: string) = choose(when: eq(left: kind, right: "runtime_refresh"), then: "run", otherwise: "skip")
            fn main() = bind(first: ui.assert_action_kind(node_id: "action", kind: "runtime_refresh"), body:
                bind(alias: first, body: bind(second: ui.wait_action_kind(node_id: "action", kind: field(value: alias, name: "kind")), body:
                    bind(last: ui.focus(node_id: destination(kind: field(value: second, name: "kind"))), body:
                        eq(left: field(value: first, name: "kind"), right: field(value: second, name: "kind"))))))"#.into();
        let mut view = authority.start_session(request).unwrap().session;
        for (index, node) in ["action", "action", "run"].into_iter().enumerate() {
            let current = &authority.sessions["session-kind"].request;
            if index > 0 {
                assert!(
                    current
                        .continuation
                        .result_binding
                        .as_ref()
                        .unwrap()
                        .results
                        .iter()
                        .all(|saved| saved.result.projection_version == 4)
                );
            }
            if index == 1 {
                assert!(
                    matches!(&view.pending_presentation, Some(PresentationOperation::WaitActionKind { expected_kind, .. })
                    if *expected_kind == leselang_hir::UiSemanticActionKind::RuntimeRefresh)
                );
            } else if index == 2 {
                assert!(
                    matches!(&view.pending_presentation, Some(PresentationOperation::Focus { node_id }) if node_id == "run")
                );
            }
            let mut ack = presentation_acknowledgement(
                "session-kind",
                DebuggerPresentationOutcome::Applied {
                    node_id: "wrong".into(),
                    focused_node_id: None,
                },
            );
            ack.effect_id = current.effect_id.clone();
            ack.expected_revision = view.projection.revision;
            assert_eq!(
                authority
                    .acknowledge_presentation(ack.clone())
                    .unwrap_err()
                    .code(),
                "debugger_presentation_conflict"
            );
            ack.outcome = DebuggerPresentationOutcome::Applied {
                node_id: node.into(),
                focused_node_id: None,
            };
            let response = authority.acknowledge_presentation(ack.clone()).unwrap();
            assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
            assert_eq!(
                response.session.projection.revision,
                Revision(8 + index as u64)
            );
            let public = serde_json::to_string(&response).unwrap();
            assert!(!public.contains("result_binding") && !public.contains("projection_version"));
            view = response.session;
        }
        assert_eq!(view.projection.state, DebuggerState::Completed);
        assert!(view.pending_presentation.is_none());
        assert_eq!(authority.sessions["session-kind"].vm.pending_count(), 0);
    }

    #[test]
    fn native_text_inspection_selects_form_prefix_without_private_frames() {
        for (text, prefix) in [
            ("ready!", "r"),
            ("Ready!", "default"),
            ("other", "default"),
            ("", "default"),
        ] {
            let root = TempRoot::new("text-inspection");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-text-inspection");
            request.source = format!(
                r#"fn ready(text: string) = and(left: starts_with(left: text, right: "ready"), right: and(left: contains(left: text, right: "ad"), right: ends_with(left: text, right: "!")))
                fn main() = bind(first: ui.assert_text(node_id: "status", expected: {}), body:
                    bind(last: ui.set_form_value(node_id: "form", field: "prefix", value: choose(when: ready(text: field(value: first, name: "expected")), then: value_or(left: char_at(left: field(value: first, name: "expected"), right: 0), right: "default"), otherwise: "default")), body:
                        eq(left: field(value: last, name: "value"), right: {})))"#,
                serde_json::to_string(text).unwrap(),
                serde_json::to_string(prefix).unwrap()
            );
            let mut view = authority.start_session(request).unwrap().session;
            let first = authority.sessions["session-text-inspection"]
                .request
                .continuation
                .clone();
            for (index, node) in ["status", "form"].into_iter().enumerate() {
                if index == 1 {
                    assert!(
                        matches!(&view.pending_presentation, Some(PresentationOperation::SetFormValue { value, .. }) if value == prefix)
                    );
                }
                let current = &authority.sessions["session-text-inspection"].request;
                let mut ack = presentation_acknowledgement(
                    "session-text-inspection",
                    DebuggerPresentationOutcome::Applied {
                        node_id: "wrong".into(),
                        focused_node_id: None,
                    },
                );
                ack.effect_id = current.effect_id.clone();
                ack.expected_revision = view.projection.revision;
                assert_eq!(
                    authority
                        .acknowledge_presentation(ack.clone())
                        .unwrap_err()
                        .code(),
                    "debugger_presentation_conflict"
                );
                ack.outcome = DebuggerPresentationOutcome::Applied {
                    node_id: node.into(),
                    focused_node_id: None,
                };
                let response = authority.acknowledge_presentation(ack.clone()).unwrap();
                assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
                assert_eq!(
                    response.session.projection.revision,
                    Revision(8 + index as u64)
                );
                let public = serde_json::to_string(&response).unwrap();
                assert!(
                    !public.contains("result_binding") && !public.contains("projection_version")
                );
                view = response.session;
            }
            assert_eq!(view.projection.state, DebuggerState::Completed);
            assert!(view.pending_presentation.is_none());
            let session = authority
                .sessions
                .get_mut("session-text-inspection")
                .unwrap();
            assert_eq!(session.vm.pending_count(), 0);
            assert_eq!(
                session.vm.resume_at(
                    &first,
                    now_ms().unwrap(),
                    EffectResult::Presentation(PresentationResult::Focus {
                        node_id: "ignored".into()
                    })
                ),
                Step::Done(leselang_vm::Value::Scalar {
                    value: leselang_vm::ScalarValue::Boolean(true)
                })
            );
        }
    }

    #[test]
    fn native_collection_fold_filters_confirmed_text_before_form_acknowledgement() {
        let root = TempRoot::new("string-list-fold");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-string-list");
        request.source = r#"fn main() = bind(first: ui.assert_text(node_id: "status", expected: "ready-a,skip,ready-b"), body:
            bind(selected: fold(output: strings(), items: split(left: field(value: first, name: "expected"), right: ","), item: "part", next: choose(when: starts_with(left: part, right: "ready-"), then: append(left: output, right: part), otherwise: output), limit: 64), body:
                bind(last: ui.set_form_value(node_id: "form", field: "selected", value: join(left: selected, right: ";")), body: selected)))"#.into();
        let mut view = authority.start_session(request).unwrap().session;
        let first = authority.sessions["session-string-list"]
            .request
            .continuation
            .clone();
        for (index, node) in ["status", "form"].into_iter().enumerate() {
            if index == 1 {
                assert!(
                    matches!(&view.pending_presentation, Some(PresentationOperation::SetFormValue { value, .. }) if value == "ready-a;ready-b")
                );
            }
            let current = &authority.sessions["session-string-list"].request;
            let mut ack = presentation_acknowledgement(
                "session-string-list",
                DebuggerPresentationOutcome::Applied {
                    node_id: "wrong".into(),
                    focused_node_id: None,
                },
            );
            ack.effect_id = current.effect_id.clone();
            ack.expected_revision = view.projection.revision;
            assert_eq!(
                authority
                    .acknowledge_presentation(ack.clone())
                    .unwrap_err()
                    .code(),
                "debugger_presentation_conflict"
            );
            ack.outcome = DebuggerPresentationOutcome::Applied {
                node_id: node.into(),
                focused_node_id: None,
            };
            let response = authority.acknowledge_presentation(ack.clone()).unwrap();
            assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
            assert_eq!(
                response.session.projection.revision,
                Revision(8 + index as u64)
            );
            let public = serde_json::to_string(&response).unwrap();
            assert!(!public.contains("result_binding") && !public.contains("projection_version"));
            view = response.session;
        }
        assert_eq!(view.projection.state, DebuggerState::Completed);
        assert!(view.pending_presentation.is_none());
        let session = authority.sessions.get_mut("session-string-list").unwrap();
        assert_eq!(session.vm.pending_count(), 0);
        assert_eq!(
            session.vm.resume_at(
                &first,
                now_ms().unwrap(),
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "ignored".into()
                })
            ),
            Step::Done(leselang_vm::Value::Scalar {
                value: leselang_vm::ScalarValue::StringList(
                    leselang_hir::computation::StringListValue(vec![
                        "ready-a".into(),
                        "ready-b".into()
                    ])
                )
            })
        );
    }

    #[test]
    fn native_effectful_function_returns_to_caller_form_without_call_frames() {
        for skip in [false, true] {
            let root = TempRoot::new("effectful-function");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-function");
            request.source = format!(
                r#"fn host_ready(node: string, skip: boolean) = choose(when: skip, then: false, otherwise:
                bind(result: ui.assert_text(node_id: node, expected: "ready"), body: starts_with(left: field(value: result, name: "expected"), right: "ready")))
                fn main() = bind(ok: host_ready(node: "status", skip: {skip}), body:
                    bind(written: ui.set_form_value(node_id: "form", field: "ready", value: to_string(value: ok)), body: ok))"#
            );
            let mut view = authority.start_session(request).unwrap().session;
            let first = authority.sessions["session-function"]
                .request
                .continuation
                .clone();
            let nodes: &[&str] = if skip { &["form"] } else { &["status", "form"] };
            for (index, node) in nodes.iter().enumerate() {
                if *node == "form" {
                    assert!(
                        matches!(&view.pending_presentation, Some(PresentationOperation::SetFormValue { value, .. }) if value == if skip { "false" } else { "true" })
                    );
                }
                let current = &authority.sessions["session-function"].request;
                let mut ack = presentation_acknowledgement(
                    "session-function",
                    DebuggerPresentationOutcome::Applied {
                        node_id: "wrong".into(),
                        focused_node_id: None,
                    },
                );
                ack.effect_id = current.effect_id.clone();
                ack.expected_revision = view.projection.revision;
                assert_eq!(
                    authority
                        .acknowledge_presentation(ack.clone())
                        .unwrap_err()
                        .code(),
                    "debugger_presentation_conflict"
                );
                ack.outcome = DebuggerPresentationOutcome::Applied {
                    node_id: (*node).into(),
                    focused_node_id: None,
                };
                let response = authority.acknowledge_presentation(ack.clone()).unwrap();
                assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
                assert_eq!(
                    response.session.projection.revision,
                    Revision(8 + index as u64)
                );
                let public = serde_json::to_string(&response).unwrap();
                assert!(
                    !public.contains("result_binding") && !public.contains("projection_version")
                );
                view = response.session;
            }
            assert_eq!(view.projection.state, DebuggerState::Completed);
            assert!(view.pending_presentation.is_none());
            let session = authority.sessions.get_mut("session-function").unwrap();
            assert_eq!(session.vm.pending_count(), 0);
            assert_eq!(
                session.vm.resume_at(
                    &first,
                    now_ms().unwrap(),
                    EffectResult::Presentation(PresentationResult::Focus {
                        node_id: "ignored".into()
                    })
                ),
                Step::Done(leselang_vm::Value::Scalar {
                    value: leselang_vm::ScalarValue::Boolean(!skip)
                })
            );
        }
    }

    #[test]
    fn native_prepared_helper_group_reaches_form_only_after_all_correlated_receipts() {
        let root = TempRoot::new("prepared-helper-group");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-prepared-group");
        request.source = r#"fn check(node: string) = ui.assert_text(node_id: node, expected: concat(left: "re", right: "ady"))
            fn main() = bind(group: seq(first: check(node: "a"), second: check(node: "b")), body:
                bind(written: ui.set_form_value(node_id: "form", field: "ready", value: concat(left: field(value: member(value: group, name: "first"), name: "expected"), right: field(value: member(value: group, name: "second"), name: "expected"))), body:
                    eq(left: field(value: written, name: "value"), right: "readyready")))"#.into();
        let mut view = authority.start_session(request).unwrap().session;
        let first = authority.sessions["session-prepared-group"]
            .request
            .continuation
            .clone();
        for (index, node) in ["a", "b", "form"].into_iter().enumerate() {
            if node == "form" {
                assert!(
                    matches!(&view.pending_presentation, Some(PresentationOperation::SetFormValue { value, .. }) if value == "readyready")
                );
            } else {
                assert!(
                    matches!(&view.pending_presentation, Some(PresentationOperation::AssertText { node_id, .. }) if node_id == node)
                );
            }
            let current = &authority.sessions["session-prepared-group"].request;
            let mut ack = presentation_acknowledgement(
                "session-prepared-group",
                DebuggerPresentationOutcome::Applied {
                    node_id: "wrong".into(),
                    focused_node_id: None,
                },
            );
            ack.effect_id = current.effect_id.clone();
            ack.expected_revision = view.projection.revision;
            assert_eq!(
                authority
                    .acknowledge_presentation(ack.clone())
                    .unwrap_err()
                    .code(),
                "debugger_presentation_conflict"
            );
            ack.outcome = DebuggerPresentationOutcome::Applied {
                node_id: node.into(),
                focused_node_id: None,
            };
            let response = authority.acknowledge_presentation(ack.clone()).unwrap();
            assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
            assert_eq!(
                response.session.projection.revision,
                Revision(8 + index as u64)
            );
            let public = serde_json::to_string(&response).unwrap();
            assert!(!public.contains("result_binding") && !public.contains("projection_version"));
            view = response.session;
        }
        assert_eq!(view.projection.state, DebuggerState::Completed);
        assert!(view.pending_presentation.is_none());
        let session = authority
            .sessions
            .get_mut("session-prepared-group")
            .unwrap();
        assert_eq!(session.vm.pending_count(), 0);
        assert_eq!(
            session.vm.resume_at(
                &first,
                now_ms().unwrap(),
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "ignored".into()
                })
            ),
            Step::Done(leselang_vm::Value::Scalar {
                value: leselang_vm::ScalarValue::Boolean(true)
            })
        );
    }

    #[test]
    fn native_selected_group_members_drive_forms_through_correlated_acknowledgements() {
        for alternate in [false, true] {
            let root = TempRoot::new("selected-helper-group");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-selected-group");
            request.source = format!(
                r#"fn rows(alternate: boolean) = choose(when: alternate,
                then: seq(first: ui.focus(node_id: "a"), second: ui.assert_text(node_id: "a", expected: "ready")),
                otherwise: seq(first: ui.focus(node_id: "b"), second: ui.assert_text(node_id: "b", expected: concat(left: "re", right: "ady"))))
                fn main() = bind(group: rows(alternate: {alternate}), body:
                    bind(written: ui.set_form_value(node_id: "form", field: "selected", value: concat(left: field(value: member(value: group, name: "first"), name: "node_id"), right: field(value: member(value: group, name: "second"), name: "expected"))), body: field(value: written, name: "value")))"#
            );
            let mut view = authority.start_session(request).unwrap().session;
            let selected = if alternate { "a" } else { "b" };
            for (index, node) in [selected, selected, "form"].into_iter().enumerate() {
                match &view.pending_presentation {
                    Some(PresentationOperation::Focus { node_id }) if index == 0 => {
                        assert_eq!(node_id, selected)
                    }
                    Some(PresentationOperation::AssertText { node_id, .. }) if index == 1 => {
                        assert_eq!(node_id, selected)
                    }
                    Some(PresentationOperation::SetFormValue { value, .. }) if index == 2 => {
                        assert_eq!(value, &format!("{selected}ready"))
                    }
                    other => panic!("{other:?}"),
                }
                let current = &authority.sessions["session-selected-group"].request;
                let mut ack = presentation_acknowledgement(
                    "session-selected-group",
                    DebuggerPresentationOutcome::Applied {
                        node_id: "wrong".into(),
                        focused_node_id: None,
                    },
                );
                ack.effect_id = current.effect_id.clone();
                ack.expected_revision = view.projection.revision;
                assert_eq!(
                    authority
                        .acknowledge_presentation(ack.clone())
                        .unwrap_err()
                        .code(),
                    "debugger_presentation_conflict"
                );
                ack.outcome = DebuggerPresentationOutcome::Applied {
                    node_id: node.into(),
                    focused_node_id: None,
                };
                let response = authority.acknowledge_presentation(ack.clone()).unwrap();
                assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
                assert_eq!(
                    response.session.projection.revision,
                    Revision(8 + index as u64)
                );
                let public = serde_json::to_string(&response).unwrap();
                assert!(
                    !public.contains("result_binding") && !public.contains("projection_version")
                );
                view = response.session;
            }
            assert_eq!(view.projection.state, DebuggerState::Completed);
            assert_eq!(
                authority.sessions["session-selected-group"]
                    .vm
                    .pending_count(),
                0
            );
        }
    }

    #[test]
    fn native_selected_parallel_groups_still_fail_preflight_before_session_journals() {
        let root = TempRoot::new("selected-parallel-group");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-selected-parallel");
        request.source = r#"fn rows(alternate: boolean) = choose(when: alternate,
            then: all(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "a")),
            otherwise: all(first: ui.focus(node_id: "b"), second: ui.focus(node_id: "b")))
            fn main() = bind(group: rows(alternate: false), body: field(value: member(value: group, name: "first"), name: "node_id"))"#.into();
        lower(&parse(&request.source)).unwrap();
        assert_eq!(
            authority.start_session(request).unwrap_err().code(),
            "debugger_session_not_suspended"
        );
        assert!(authority.sessions.is_empty());
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
    }

    #[test]
    fn native_prepared_parallel_helpers_fail_preflight_without_creating_journals() {
        let root = TempRoot::new("prepared-parallel-helpers");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-prepared-parallel");
        request.source =
            r#"fn focus(node: string) = ui.focus(node_id: concat(left: node, right: "-prepared"))
            fn main() = all(first: focus(node: "a"), second: focus(node: "b"))"#
                .into();
        lower(&parse(&request.source)).unwrap();
        assert_eq!(
            authority.start_session(request.clone()).unwrap_err().code(),
            "debugger_session_not_suspended"
        );
        assert!(authority.sessions.is_empty());
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
        request.source = r#"fn main() = ui.focus(node_id: "a")"#.into();
        assert_eq!(
            authority
                .start_session(request)
                .unwrap()
                .session
                .projection
                .state,
            DebuggerState::WaitingEffect
        );
    }

    #[test]
    fn native_prepared_result_selection_keeps_correlated_captures_and_early_exits() {
        for ready in [false, true] {
            let root = TempRoot::new("prepared-result-selection");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-prepared-result");
            let expected = if ready { "ready" } else { "skip" };
            request.source = format!(
                r#"fn main() = bind(seed: ui.assert_text(node_id: "status", expected: "{expected}"), body:
                bind(selected: bind(target: concat(left: "node-", right: field(value: seed, name: "expected")), body:
                    choose(when: starts_with(left: target, right: "node-ready"), then: ui.focus(node_id: target), otherwise: ui.focus(node_id: "fallback"))), body:
                        choose(when: eq(left: field(value: selected, name: "node_id"), right: "fallback"), then: false, otherwise:
                            bind(written: ui.set_form_value(node_id: "form", field: "target", value: field(value: selected, name: "node_id")), body: true))))"#
            );
            let mut view = authority.start_session(request).unwrap().session;
            let first = authority.sessions["session-prepared-result"]
                .request
                .continuation
                .clone();
            let nodes = if ready {
                vec!["status", "node-ready", "form"]
            } else {
                vec!["status", "fallback"]
            };
            for (index, node) in nodes.into_iter().enumerate() {
                match &view.pending_presentation {
                    Some(PresentationOperation::AssertText { node_id, .. }) if index == 0 => {
                        assert_eq!(node_id, node)
                    }
                    Some(PresentationOperation::Focus { node_id }) if index == 1 => {
                        assert_eq!(node_id, node)
                    }
                    Some(PresentationOperation::SetFormValue { value, .. }) if index == 2 => {
                        assert_eq!(value, "node-ready")
                    }
                    other => panic!("{other:?}"),
                }
                let current = &authority.sessions["session-prepared-result"].request;
                let mut ack = presentation_acknowledgement(
                    "session-prepared-result",
                    DebuggerPresentationOutcome::Applied {
                        node_id: "wrong".into(),
                        focused_node_id: None,
                    },
                );
                ack.effect_id = current.effect_id.clone();
                ack.expected_revision = view.projection.revision;
                assert_eq!(
                    authority
                        .acknowledge_presentation(ack.clone())
                        .unwrap_err()
                        .code(),
                    "debugger_presentation_conflict"
                );
                ack.outcome = DebuggerPresentationOutcome::Applied {
                    node_id: node.into(),
                    focused_node_id: None,
                };
                let response = authority.acknowledge_presentation(ack.clone()).unwrap();
                assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
                assert_eq!(
                    response.session.projection.revision,
                    Revision(8 + index as u64)
                );
                let public = serde_json::to_string(&response).unwrap();
                assert!(
                    !public.contains("result_binding") && !public.contains("projection_version")
                );
                view = response.session;
            }
            assert_eq!(view.projection.state, DebuggerState::Completed);
            let session = authority
                .sessions
                .get_mut("session-prepared-result")
                .unwrap();
            assert_eq!(session.vm.pending_count(), 0);
            assert_eq!(
                session.vm.resume_at(
                    &first,
                    now_ms().unwrap(),
                    EffectResult::Presentation(PresentationResult::Focus {
                        node_id: "ignored".into()
                    })
                ),
                Step::Done(leselang_vm::Value::Scalar {
                    value: leselang_vm::ScalarValue::Boolean(ready)
                })
            );
        }
    }

    #[test]
    fn native_session_vm_has_bounded_dispatch_policy_without_changing_sequential_ui() {
        let root = TempRoot::new("native-scheduler-limits");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-scheduler");
        request.source = r#"fn main() = bind(group: seq(first: ui.focus(node_id: "first"), second: ui.focus(node_id: "second")), body: true)"#.into();
        let response = authority.start_session(request).unwrap();
        assert_eq!(
            response.session.projection.state,
            DebuggerState::WaitingEffect
        );
        let pressure = authority.sessions["session-scheduler"]
            .vm
            .scheduler_pressure(now_ms().unwrap())
            .unwrap();
        assert_eq!(pressure.limits, DEBUGGER_SCHEDULER_LIMITS);
        assert_eq!(pressure.pending_dispatches, 2);
        assert_eq!(pressure.active_leases, 0);
    }

    #[test]
    fn independent_native_sessions_scope_colliding_effect_ids_and_cancellation() {
        let root = TempRoot::new("isolated-native-sessions");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        for (id, label) in [("session-lane-a", "alpha"), ("session-lane-b", "bravo")] {
            let mut request = start_request(id);
            request.source = format!(
                r#"fn main() = bind(result: ui.assert_text(node_id: "{label}", expected: "{label}"), body: bind(written: ui.set_form_value(node_id: "form-{label}", field: "answer", value: field(value: result, name: "expected")), body: true))"#
            );
            authority.start_session(request).unwrap();
            assert_eq!(authority.sessions[id].request.effect_id, "effect-1");
        }
        let first_b = view_session(&authority.sessions["session-lane-b"]).unwrap();
        let mut ack = presentation_acknowledgement(
            "session-lane-b",
            DebuggerPresentationOutcome::Applied {
                node_id: "alpha".into(),
                focused_node_id: None,
            },
        );
        assert_eq!(
            authority
                .acknowledge_presentation(ack.clone())
                .unwrap_err()
                .code(),
            "debugger_presentation_conflict"
        );
        ack.session_id = "session-lane-a".into();
        let next_a = authority.acknowledge_presentation(ack).unwrap().session;
        assert_eq!(next_a.projection.revision, Revision(8));
        assert_eq!(
            view_session(&authority.sessions["session-lane-b"]).unwrap(),
            first_b
        );
        let mut cancel = cancel_command("session-lane-a", false);
        cancel.expected_revision = Some(next_a.projection.revision);
        assert_eq!(
            authority.cancel(cancel).unwrap().session.projection.state,
            DebuggerState::Cancelled
        );
        assert_eq!(
            view_session(&authority.sessions["session-lane-b"]).unwrap(),
            first_b
        );
        let ack = presentation_acknowledgement(
            "session-lane-b",
            DebuggerPresentationOutcome::Applied {
                node_id: "bravo".into(),
                focused_node_id: None,
            },
        );
        let next_b = authority.acknowledge_presentation(ack).unwrap().session;
        assert!(
            matches!(&next_b.pending_presentation, Some(PresentationOperation::SetFormValue { value, .. }) if value == "bravo")
        );
        let mut ack = presentation_acknowledgement(
            "session-lane-b",
            DebuggerPresentationOutcome::Applied {
                node_id: "form-bravo".into(),
                focused_node_id: None,
            },
        );
        ack.effect_id = authority.sessions["session-lane-b"]
            .request
            .effect_id
            .clone();
        ack.expected_revision = next_b.projection.revision;
        assert_eq!(
            authority
                .acknowledge_presentation(ack)
                .unwrap()
                .session
                .projection
                .state,
            DebuggerState::Completed
        );
    }

    #[test]
    fn native_selected_data_functions_return_through_correlated_caller_acknowledgements() {
        for selected in ["single()", "gathered()", "\"ready\""] {
            let root = TempRoot::new("selected-function-return");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-selected-function");
            request.source = format!(
                r#"fn single() = bind(result: ui.assert_text(node_id: "single", expected: "ready"), body: field(value: result, name: "expected"))
                fn gathered() = bind(group: seq(first: ui.assert_text(node_id: "left", expected: "re"), second: ui.assert_text(node_id: "right", expected: "ady")), body: concat(left: field(value: member(value: group, name: "first"), name: "expected"), right: field(value: member(value: group, name: "second"), name: "expected")))
                fn main() = bind(answer: choose(when: false, then: single(), otherwise: {selected}), body:
                    bind(written: ui.set_form_value(node_id: "form", field: "answer", value: answer), body: eq(left: field(value: written, name: "value"), right: "ready")))"#
            );
            let mut view = authority.start_session(request).unwrap().session;
            let first = authority.sessions["session-selected-function"]
                .request
                .continuation
                .clone();
            let nodes = match selected {
                "single()" => vec!["single", "form"],
                "gathered()" => vec!["left", "right", "form"],
                _ => vec!["form"],
            };
            for (index, node) in nodes.into_iter().enumerate() {
                if node == "form" {
                    assert!(
                        matches!(&view.pending_presentation, Some(PresentationOperation::SetFormValue { value, .. }) if value == "ready")
                    );
                } else {
                    assert!(
                        matches!(&view.pending_presentation, Some(PresentationOperation::AssertText { node_id, .. }) if node_id == node)
                    );
                }
                let current = &authority.sessions["session-selected-function"].request;
                let mut ack = presentation_acknowledgement(
                    "session-selected-function",
                    DebuggerPresentationOutcome::Applied {
                        node_id: "wrong".into(),
                        focused_node_id: None,
                    },
                );
                ack.effect_id = current.effect_id.clone();
                ack.expected_revision = view.projection.revision;
                assert_eq!(
                    authority
                        .acknowledge_presentation(ack.clone())
                        .unwrap_err()
                        .code(),
                    "debugger_presentation_conflict"
                );
                ack.outcome = DebuggerPresentationOutcome::Applied {
                    node_id: node.into(),
                    focused_node_id: None,
                };
                let response = authority.acknowledge_presentation(ack.clone()).unwrap();
                assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
                assert_eq!(
                    response.session.projection.revision,
                    Revision(8 + index as u64)
                );
                let public = serde_json::to_string(&response).unwrap();
                assert!(!public.contains("result_binding") && !public.contains("call_frame"));
                view = response.session;
            }
            assert_eq!(view.projection.state, DebuggerState::Completed);
            let session = authority
                .sessions
                .get_mut("session-selected-function")
                .unwrap();
            assert_eq!(session.vm.pending_count(), 0);
            assert_eq!(
                session.vm.resume_at(
                    &first,
                    now_ms().unwrap(),
                    EffectResult::Presentation(PresentationResult::Focus {
                        node_id: "ignored".into()
                    })
                ),
                Step::Done(leselang_vm::Value::Scalar {
                    value: leselang_vm::ScalarValue::Boolean(true)
                })
            );
        }
    }

    #[test]
    fn native_selected_parallel_data_function_fails_preflight_without_a_journal() {
        let root = TempRoot::new("selected-parallel-function");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-selected-parallel-function");
        let source = |selected| {
            format!(
                r#"fn gathered() = bind(group: all(first: ui.focus(node_id: "a"), second: ui.focus(node_id: "b")), body: true)
            fn main() = bind(answer: choose(when: {selected}, then: gathered(), otherwise: false), body: ui.set_form_value(node_id: "form", field: "answer", value: to_string(value: answer)))"#
            )
        };
        request.source = source(true);
        lower(&parse(&request.source)).unwrap();
        assert_eq!(
            authority.start_session(request.clone()).unwrap_err().code(),
            "debugger_session_not_suspended"
        );
        assert!(authority.sessions.is_empty());
        assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
        request.source = source(false);
        let view = authority.start_session(request).unwrap().session;
        assert!(
            matches!(view.pending_presentation, Some(PresentationOperation::SetFormValue { value, .. }) if value == "false")
        );
    }

    #[test]
    fn native_nullable_receipts_preserve_none_empty_and_explicit_defaults() {
        for expected in [None, Some(""), Some("hint")] {
            let root = TempRoot::new("optional-projections");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-optional");
            let text = expected.map_or_else(
                || "none".into(),
                |text| serde_json::to_string(text).unwrap(),
            );
            request.source = format!(
                r#"fn defaulted(value: optional_string) = value_or(left: value, right: "default")
                fn main() = bind(first: ui.assert_form_field_placeholder(node_id: "form", field: "name", expected: {text}), body:
                    bind(second: ui.wait_form_field_placeholder(node_id: "form", field: "name", expected: field(value: first, name: "optional_expected")), body:
                        bind(last: ui.set_form_value(node_id: "form", field: "label", value: defaulted(value: field(value: second, name: "optional_expected"))), body:
                            has_value(value: field(value: first, name: "optional_expected")))))"#
            );
            let mut view = authority.start_session(request).unwrap().session;
            let first = authority.sessions["session-optional"]
                .request
                .continuation
                .clone();
            for index in 0..3 {
                let current = &authority.sessions["session-optional"].request;
                if index > 0 {
                    assert!(
                        current
                            .continuation
                            .result_binding
                            .as_ref()
                            .unwrap()
                            .results
                            .iter()
                            .all(|saved| saved.result.projection_version == 5)
                    );
                }
                if index == 1 {
                    assert!(
                        matches!(&view.pending_presentation, Some(PresentationOperation::WaitFormFieldPlaceholder { expected: value, .. }) if value.as_deref() == expected)
                    );
                } else if index == 2 {
                    assert!(
                        matches!(&view.pending_presentation, Some(PresentationOperation::SetFormValue { value, .. }) if value == expected.unwrap_or("default"))
                    );
                }
                let mut ack = presentation_acknowledgement(
                    "session-optional",
                    DebuggerPresentationOutcome::Applied {
                        node_id: "wrong".into(),
                        focused_node_id: None,
                    },
                );
                ack.effect_id = current.effect_id.clone();
                ack.expected_revision = view.projection.revision;
                assert_eq!(
                    authority
                        .acknowledge_presentation(ack.clone())
                        .unwrap_err()
                        .code(),
                    "debugger_presentation_conflict"
                );
                ack.outcome = DebuggerPresentationOutcome::Applied {
                    node_id: "form".into(),
                    focused_node_id: None,
                };
                let response = authority.acknowledge_presentation(ack.clone()).unwrap();
                assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
                assert_eq!(response.session.projection.revision, Revision(8 + index));
                let public = serde_json::to_string(&response).unwrap();
                assert!(
                    !public.contains("result_binding") && !public.contains("projection_version")
                );
                view = response.session;
            }
            assert_eq!(view.projection.state, DebuggerState::Completed);
            assert!(view.pending_presentation.is_none());
            let session = authority.sessions.get_mut("session-optional").unwrap();
            assert_eq!(session.vm.pending_count(), 0);
            assert_eq!(
                session.vm.resume_at(
                    &first,
                    now_ms().unwrap(),
                    EffectResult::Presentation(PresentationResult::Focus {
                        node_id: "ignored".into()
                    })
                ),
                Step::Done(leselang_vm::Value::Scalar {
                    value: leselang_vm::ScalarValue::Boolean(expected.is_some())
                })
            );
        }
    }

    #[test]
    fn native_recovery_fills_form_defaults_without_swallowing_presentation_rejection() {
        for (node, value, rejected) in [("7", "7", false), ("bad", "3", false), ("bad", "", true)] {
            let root = TempRoot::new("recovery");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-recovery");
            request.source = format!(
                r#"fn main() = bind(r: ui.focus(node_id: "{node}"), body:
                ui.set_form_value(node_id: "form", field: "replicas", value: to_string(value:
                    recover(value: parse_integer(value: field(value: r, name: "node_id")), fallback: 3))))"#
            );
            let started = authority.start_session(request).unwrap();
            let outcome = if rejected {
                DebuggerPresentationOutcome::Rejected {
                    node_id: node.into(),
                    code: "target_not_visible".into(),
                }
            } else {
                DebuggerPresentationOutcome::Applied {
                    node_id: node.into(),
                    focused_node_id: None,
                }
            };
            let mut first_ack = presentation_acknowledgement("session-recovery", outcome);
            first_ack.effect_id = authority.sessions["session-recovery"]
                .request
                .effect_id
                .clone();
            first_ack.expected_revision = started.session.projection.revision;
            let advanced = authority
                .acknowledge_presentation(first_ack.clone())
                .unwrap();
            assert_eq!(
                authority
                    .acknowledge_presentation(first_ack.clone())
                    .unwrap(),
                advanced
            );
            assert_eq!(advanced.session.projection.revision, Revision(8));
            if rejected {
                assert_eq!(advanced.session.projection.state, DebuggerState::Failed);
                assert!(advanced.session.pending_presentation.is_none());
                assert_eq!(authority.sessions["session-recovery"].vm.pending_count(), 0);
                continue;
            }
            assert!(matches!(&advanced.session.pending_presentation,
                Some(PresentationOperation::SetFormValue { node_id, field, value: actual })
                    if node_id == "form" && field == "replicas" && actual == value));
            let mut next_ack = presentation_acknowledgement(
                "session-recovery",
                DebuggerPresentationOutcome::Applied {
                    node_id: "form".into(),
                    focused_node_id: None,
                },
            );
            next_ack.effect_id = authority.sessions["session-recovery"]
                .request
                .effect_id
                .clone();
            next_ack.expected_revision = advanced.session.projection.revision;
            assert_ne!(next_ack.effect_id, first_ack.effect_id);
            first_ack.outcome = DebuggerPresentationOutcome::Applied {
                node_id: "changed".into(),
                focused_node_id: None,
            };
            assert_eq!(
                authority
                    .acknowledge_presentation(first_ack)
                    .unwrap_err()
                    .code(),
                "debugger_presentation_conflict"
            );
            let completed = authority
                .acknowledge_presentation(next_ack.clone())
                .unwrap();
            assert_eq!(
                authority.acknowledge_presentation(next_ack).unwrap(),
                completed
            );
            assert_eq!(completed.session.projection.revision, Revision(9));
            assert_eq!(completed.session.projection.state, DebuggerState::Completed);
            assert!(completed.session.pending_presentation.is_none());
            assert_eq!(authority.sessions["session-recovery"].vm.pending_count(), 0);
            assert!(
                !serde_json::to_string(&advanced)
                    .unwrap()
                    .contains("result_binding")
            );
        }
    }

    #[test]
    fn native_group_results_compute_only_after_correlated_presentations_complete() {
        let root = TempRoot::new("group-results");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-group-results");
        request.source = r#"fn main() = bind(g: seq(
            focus: ui.focus(node_id: "runtime-a"),
            verify: ui.assert_visible(node_id: "runtime-a")), body:
            eq(left: field(value: member(value: g, name: "focus"), name: "node_id"),
               right: field(value: member(value: g, name: "verify"), name: "node_id")))"#
            .into();
        let mut view = authority.start_session(request).unwrap().session;
        let first_image = authority.sessions["session-group-results"]
            .request
            .continuation
            .clone();
        assert_eq!(first_image.schema_version, 6);
        for (index, expected_revision) in [Revision(8), Revision(9)].into_iter().enumerate() {
            let current = &authority.sessions["session-group-results"].request;
            let mut ack = presentation_acknowledgement(
                "session-group-results",
                DebuggerPresentationOutcome::Applied {
                    node_id: "wrong-node".into(),
                    focused_node_id: None,
                },
            );
            ack.effect_id = current.effect_id.clone();
            ack.expected_revision = view.projection.revision;
            assert_eq!(
                authority
                    .acknowledge_presentation(ack.clone())
                    .unwrap_err()
                    .code(),
                "debugger_presentation_conflict"
            );
            ack.outcome = DebuggerPresentationOutcome::Applied {
                node_id: "runtime-a".into(),
                focused_node_id: None,
            };
            let response = authority.acknowledge_presentation(ack.clone()).unwrap();
            assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
            assert_eq!(response.session.projection.revision, expected_revision);
            if index == 0 {
                assert!(
                    matches!(&response.session.pending_presentation, Some(PresentationOperation::AssertVisible { node_id }) if node_id == "runtime-a")
                );
            }
            view = response.session;
        }
        assert_eq!(view.projection.state, DebuggerState::Completed);
        assert!(view.pending_presentation.is_none());
        let session = authority.sessions.get_mut("session-group-results").unwrap();
        assert_eq!(session.vm.pending_count(), 0);
        assert_eq!(
            session.vm.resume_at(
                &first_image,
                now_ms().unwrap(),
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "ignored".into()
                })
            ),
            Step::Done(leselang_vm::Value::Scalar {
                value: leselang_vm::ScalarValue::Boolean(true)
            })
        );
        let public = serde_json::to_string(&view).unwrap();
        assert!(!public.contains("group_result"));
        assert!(!public.contains("result_binding"));
    }

    #[test]
    fn native_group_results_drive_a_correlated_atomic_tail_without_public_frames() {
        let root = TempRoot::new("group-tail");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-group-tail");
        request.source = r#"fn main() = bind(g: seq(
            move: ui.navigate_focus(node_id: "runtime-a", direction: "next"),
            verify: ui.assert_visible(node_id: "runtime-b")), body:
            ui.focus(node_id: field(value: member(value: g, name: "move"), name: "focused_node_id")))"#.into();
        let mut view = authority.start_session(request).unwrap().session;
        let first = authority.sessions["session-group-tail"]
            .request
            .continuation
            .clone();
        assert_eq!(first.schema_version, 7);
        for (index, node) in ["runtime-a", "runtime-b", "runtime-b"]
            .into_iter()
            .enumerate()
        {
            if index == 2 {
                assert!(matches!(&view.pending_presentation,
                    Some(PresentationOperation::Focus { node_id }) if node_id == "runtime-b"));
            }
            let current = &authority.sessions["session-group-tail"].request;
            assert_eq!(current.continuation.schema_version, 7);
            assert_eq!(current.continuation.group_result, first.group_result);
            let mut ack = presentation_acknowledgement(
                "session-group-tail",
                DebuggerPresentationOutcome::Applied {
                    node_id: "wrong-node".into(),
                    focused_node_id: None,
                },
            );
            ack.effect_id = current.effect_id.clone();
            ack.expected_revision = view.projection.revision;
            assert_eq!(
                authority
                    .acknowledge_presentation(ack.clone())
                    .unwrap_err()
                    .code(),
                "debugger_presentation_conflict"
            );
            ack.outcome = DebuggerPresentationOutcome::Applied {
                node_id: node.into(),
                focused_node_id: (index == 0).then(|| "runtime-b".into()),
            };
            let response = authority.acknowledge_presentation(ack.clone()).unwrap();
            assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
            assert_eq!(
                response.session.projection.revision,
                Revision(8 + index as u64)
            );
            let public = serde_json::to_string(&response).unwrap();
            assert!(!public.contains("group_result"));
            assert!(!public.contains("result_binding"));
            view = response.session;
        }
        assert_eq!(view.projection.state, DebuggerState::Completed);
        assert!(view.pending_presentation.is_none());
        let session = authority.sessions.get_mut("session-group-tail").unwrap();
        assert_eq!(session.vm.pending_count(), 0);
        assert_eq!(
            session.vm.resume_at(
                &first,
                now_ms().unwrap(),
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "ignored".into()
                })
            ),
            Step::Done(leselang_vm::Value::UiFocus {
                node_id: "runtime-b".into()
            })
        );
    }

    #[test]
    fn native_debugger_fences_parallel_group_tails_before_creating_session_journals() {
        for body in [
            r#"ui.focus(node_id: "runtime-b")"#,
            r#"bind(r: ui.focus(node_id: "runtime-b"), body: true)"#,
            r#"bind(r: ui.focus(node_id: "runtime-b"), body: bind(s: ui.focus(node_id: "runtime-b"), body: true))"#,
            r#"choose(when: true, then: true, otherwise: bind(r: ui.focus(node_id: "runtime-b"), body: true))"#,
        ] {
            let root = TempRoot::new("parallel-group-tail");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-parallel-tail");
            request.source = format!(
                r#"fn main() = bind(g: all(a: ui.focus(node_id: "runtime-a"), b: ui.focus(node_id: "runtime-b")), body: {body})"#
            );
            lower(&parse(&request.source)).unwrap();
            assert_eq!(
                authority.start_session(request.clone()).unwrap_err().code(),
                "debugger_session_not_suspended"
            );
            assert!(authority.sessions.is_empty());
            assert_eq!(fs::read_dir(&root.0).unwrap().count(), 0);
            request.source = r#"fn main() = ui.focus(node_id: "runtime-a")"#.into();
            assert_eq!(
                authority
                    .start_session(request)
                    .unwrap()
                    .session
                    .projection
                    .state,
                DebuggerState::WaitingEffect
            );
        }
    }

    #[test]
    fn native_group_conditional_exits_finish_without_extra_ui_steps() {
        for stop_at in 0..=2 {
            let root = TempRoot::new("group-conditional");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-group-conditional");
            request.source = r#"fn main() = bind(g: seq(
                move: ui.navigate_focus(node_id: "runtime-a", direction: "next"),
                check: ui.assert_visible(node_id: "runtime-a")), body:
                bind(alias: g, body:
                    choose(when: eq(left: field(value: member(value: alias, name: "move"), name: "focused_node_id"), right: "runtime-home"), then: 0, otherwise:
                        bind(first: ui.navigate_focus(node_id: "runtime-a", direction: "next"), body:
                            choose(when: eq(left: field(value: first, name: "focused_node_id"), right: "runtime-home"), then: 1, otherwise:
                                bind(last: ui.assert_visible(node_id: field(value: first, name: "focused_node_id")), body: 2))))))"#.into();
            let mut view = authority.start_session(request).unwrap().session;
            let first = authority.sessions["session-group-conditional"]
                .request
                .continuation
                .clone();
            assert_eq!(first.schema_version, 11);
            for index in 0..2 + stop_at {
                let current = &authority.sessions["session-group-conditional"].request;
                assert_eq!(current.continuation.schema_version, 11);
                assert_eq!(current.continuation.group_result, first.group_result);
                if index >= 2 {
                    let saved = current.continuation.result_binding.as_ref().unwrap();
                    assert_eq!(saved.body.is_pure(), index == 3);
                    assert_eq!(saved.groups.len(), 2);
                }
                let mut ack = presentation_acknowledgement(
                    "session-group-conditional",
                    DebuggerPresentationOutcome::Applied {
                        node_id: "wrong-node".into(),
                        focused_node_id: None,
                    },
                );
                ack.effect_id = current.effect_id.clone();
                ack.expected_revision = view.projection.revision;
                assert_eq!(
                    authority
                        .acknowledge_presentation(ack.clone())
                        .unwrap_err()
                        .code(),
                    "debugger_presentation_conflict"
                );
                ack.outcome = DebuggerPresentationOutcome::Applied {
                    node_id: if index == 3 { "runtime-b" } else { "runtime-a" }.into(),
                    focused_node_id: matches!(index, 0 | 2).then(|| {
                        if (index == 0 && stop_at == 0) || (index == 2 && stop_at == 1) {
                            "runtime-home".into()
                        } else {
                            "runtime-b".into()
                        }
                    }),
                };
                let response = authority.acknowledge_presentation(ack.clone()).unwrap();
                assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
                assert_eq!(response.session.projection.revision, Revision(8 + index));
                let public = serde_json::to_string(&response).unwrap();
                assert!(!public.contains("group_result"));
                assert!(!public.contains("result_binding"));
                view = response.session;
            }
            assert_eq!(view.projection.state, DebuggerState::Completed);
            assert!(view.pending_presentation.is_none());
            let session = authority
                .sessions
                .get_mut("session-group-conditional")
                .unwrap();
            assert_eq!(session.vm.pending_count(), 0);
            assert_eq!(
                session.vm.resume_at(
                    &first,
                    now_ms().unwrap(),
                    EffectResult::Presentation(PresentationResult::Focus {
                        node_id: "ignored".into()
                    }),
                ),
                Step::Done(leselang_vm::Value::Scalar {
                    value: leselang_vm::ScalarValue::Integer(stop_at),
                }),
            );
        }
    }

    #[test]
    fn native_group_dataflow_preserves_correlated_ui_steps_and_private_frames() {
        for failed in [false, true] {
            let root = TempRoot::new("group-dataflow");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-group-dataflow");
            let final_body = if failed {
                "div(left: 1, right: 0)"
            } else {
                r#"eq(left: field(value: verified, name: "node_id"), right: field(value: member(value: alias, name: "move"), name: "focused_node_id"))"#
            };
            request.source = format!(
                r#"fn main() = bind(g: seq(
                move: ui.navigate_focus(node_id: "runtime-a", direction: "next"),
                check: ui.assert_visible(node_id: "runtime-b")), body:
                bind(focused: ui.focus(node_id: field(value: member(value: g, name: "move"), name: "focused_node_id")), body:
                    bind(alias: g, body: bind(verified: ui.assert_visible(node_id: field(value: focused, name: "node_id")), body: {final_body}))))"#
            );
            let mut view = authority.start_session(request).unwrap().session;
            let first = authority.sessions["session-group-dataflow"]
                .request
                .continuation
                .clone();
            assert_eq!(first.schema_version, 10);
            for (index, node) in ["runtime-a", "runtime-b", "runtime-b", "runtime-b"]
                .into_iter()
                .enumerate()
            {
                let current = &authority.sessions["session-group-dataflow"].request;
                assert_eq!(current.continuation.schema_version, 10);
                assert_eq!(current.continuation.group_result, first.group_result);
                if index >= 2 {
                    let saved = current.continuation.result_binding.as_ref().unwrap();
                    assert_eq!(saved.body.is_pure(), index == 3);
                    assert!(!saved.groups.is_empty());
                }
                let mut ack = presentation_acknowledgement(
                    "session-group-dataflow",
                    DebuggerPresentationOutcome::Applied {
                        node_id: "wrong-node".into(),
                        focused_node_id: None,
                    },
                );
                ack.effect_id = current.effect_id.clone();
                ack.expected_revision = view.projection.revision;
                assert_eq!(
                    authority
                        .acknowledge_presentation(ack.clone())
                        .unwrap_err()
                        .code(),
                    "debugger_presentation_conflict"
                );
                ack.outcome = DebuggerPresentationOutcome::Applied {
                    node_id: node.into(),
                    focused_node_id: (index == 0).then(|| "runtime-b".into()),
                };
                let response = authority.acknowledge_presentation(ack.clone()).unwrap();
                assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
                assert_eq!(
                    response.session.projection.revision,
                    Revision(8 + index as u64)
                );
                let public = serde_json::to_string(&response).unwrap();
                assert!(!public.contains("group_result"));
                assert!(!public.contains("result_binding"));
                view = response.session;
            }
            assert_eq!(
                view.projection.state,
                if failed {
                    DebuggerState::Failed
                } else {
                    DebuggerState::Completed
                }
            );
            assert!(view.pending_presentation.is_none());
            let session = authority
                .sessions
                .get_mut("session-group-dataflow")
                .unwrap();
            assert_eq!(session.vm.pending_count(), 0);
            let replay = session.vm.resume_at(
                &first,
                now_ms().unwrap(),
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "ignored".into(),
                }),
            );
            if failed {
                assert!(matches!(replay, Step::Fault(fault) if fault.code == "LSV1401"));
            } else {
                assert_eq!(
                    replay,
                    Step::Done(leselang_vm::Value::Scalar {
                        value: leselang_vm::ScalarValue::Boolean(true)
                    })
                );
            }
        }
    }

    #[test]
    fn native_group_capture_keeps_private_member_frames_and_commits_a_final_scalar() {
        for failed in [false, true] {
            let root = TempRoot::new("group-capture");
            let mut authority = DebuggerAuthority::open(&root.0).unwrap();
            let mut request = start_request("session-group-capture");
            let body = if failed {
                "div(left: 1, right: 0)"
            } else {
                r#"eq(left: field(value: focused, name: "node_id"), right: field(value: member(value: g, name: "move"), name: "focused_node_id"))"#
            };
            request.source = format!(
                r#"fn main() = bind(g: seq(
                move: ui.navigate_focus(node_id: "runtime-a", direction: "next"),
                verify: ui.assert_visible(node_id: "runtime-b")), body:
                bind(focused: ui.focus(node_id: field(value: member(value: g, name: "move"), name: "focused_node_id")), body: {body}))"#
            );
            let mut view = authority.start_session(request).unwrap().session;
            let first = authority.sessions["session-group-capture"]
                .request
                .continuation
                .clone();
            assert_eq!(first.schema_version, 8);
            for (index, node) in ["runtime-a", "runtime-b", "runtime-b"]
                .into_iter()
                .enumerate()
            {
                let current = &authority.sessions["session-group-capture"].request;
                assert_eq!(current.continuation.schema_version, 8);
                assert_eq!(current.continuation.group_result, first.group_result);
                if index == 2 {
                    assert!(
                        current
                            .continuation
                            .result_binding
                            .as_ref()
                            .is_some_and(|binding| !binding.groups.is_empty())
                    );
                }
                let mut ack = presentation_acknowledgement(
                    "session-group-capture",
                    DebuggerPresentationOutcome::Applied {
                        node_id: "wrong-node".into(),
                        focused_node_id: None,
                    },
                );
                ack.effect_id = current.effect_id.clone();
                ack.expected_revision = view.projection.revision;
                assert_eq!(
                    authority
                        .acknowledge_presentation(ack.clone())
                        .unwrap_err()
                        .code(),
                    "debugger_presentation_conflict"
                );
                ack.outcome = DebuggerPresentationOutcome::Applied {
                    node_id: node.into(),
                    focused_node_id: (index == 0).then(|| "runtime-b".into()),
                };
                let response = authority.acknowledge_presentation(ack.clone()).unwrap();
                assert_eq!(authority.acknowledge_presentation(ack).unwrap(), response);
                assert_eq!(
                    response.session.projection.revision,
                    Revision(8 + index as u64)
                );
                let public = serde_json::to_string(&response).unwrap();
                assert!(!public.contains("group_result"));
                assert!(!public.contains("result_binding"));
                view = response.session;
            }
            assert_eq!(
                view.projection.state,
                if failed {
                    DebuggerState::Failed
                } else {
                    DebuggerState::Completed
                }
            );
            assert!(view.pending_presentation.is_none());
            let session = authority.sessions.get_mut("session-group-capture").unwrap();
            assert_eq!(session.vm.pending_count(), 0);
            let result = session.vm.resume_at(
                &first,
                now_ms().unwrap(),
                EffectResult::Presentation(PresentationResult::Focus {
                    node_id: "ignored".into(),
                }),
            );
            if failed {
                assert!(matches!(result, Step::Fault(fault) if fault.code == "LSV1401"));
            } else {
                assert_eq!(
                    result,
                    Step::Done(leselang_vm::Value::Scalar {
                        value: leselang_vm::ScalarValue::Boolean(true)
                    })
                );
            }
        }
    }

    #[test]
    fn computed_group_arguments_advance_native_steps_without_rebinding_targets() {
        let root = TempRoot::new("computed-group-arguments");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-group-arguments");
        request.source = r#"fn main() = bind(node: concat(left: "runtime-", right: "a"), body: seq(
            focus: ui.focus(node_id: node),
            verify: repeat(times: 2, body: ui.assert_visible(node_id: node))))"#
            .into();
        let mut view = authority.start_session(request).unwrap().session;
        let mut previous_acknowledgement: Option<DebuggerPresentationAcknowledgeRequest> = None;
        for index in 0..3 {
            if let Some(mut stale) = previous_acknowledgement.take() {
                // A stale coordinate with a changed payload is not an idempotent replay.
                stale.outcome = DebuggerPresentationOutcome::Applied {
                    node_id: "wrong-target".into(),
                    focused_node_id: None,
                };
                assert_eq!(
                    authority
                        .acknowledge_presentation(stale)
                        .unwrap_err()
                        .code(),
                    "debugger_presentation_conflict"
                );
            }
            assert!(matches!(&view.pending_presentation,
                Some(PresentationOperation::Focus { node_id } | PresentationOperation::AssertVisible { node_id }) if node_id == "runtime-a"));
            let mut acknowledgement = presentation_acknowledgement(
                "session-group-arguments",
                DebuggerPresentationOutcome::Applied {
                    node_id: "runtime-a".into(),
                    focused_node_id: None,
                },
            );
            acknowledgement.effect_id = authority.sessions["session-group-arguments"]
                .request
                .effect_id
                .clone();
            acknowledgement.expected_revision = view.projection.revision;
            let response = authority
                .acknowledge_presentation(acknowledgement.clone())
                .unwrap();
            assert_eq!(
                authority
                    .acknowledge_presentation(acknowledgement.clone())
                    .unwrap(),
                response
            );
            previous_acknowledgement = Some(acknowledgement);
            view = response.session;
            assert_eq!(view.projection.revision, Revision(8 + index));
        }
        assert_eq!(view.projection.state, DebuggerState::Completed);
        assert_eq!(
            authority.sessions["session-group-arguments"]
                .vm
                .pending_count(),
            0
        );

        let mut invalid = start_request("session-invalid-group");
        invalid.source = r#"fn main() = seq(first: ui.focus(node_id: "runtime-a"), later: ui.focus(node_id: concat(left: "bad", right: " node")))"#.into();
        assert!(authority.start_session(invalid).is_err());
        assert!(!authority.sessions.contains_key("session-invalid-group"));
        assert!(
            !journal_artifacts_exist(&authority.journal_root.join("session-invalid-group.sqlite"))
                .unwrap()
        );
    }

    #[test]
    fn sequential_gui_cancellation_uses_current_session_revision_and_fences_remaining_steps() {
        let root = TempRoot::new("sequential-cancel");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-sequence-cancel");
        request.source =
            r#"fn main() = repeat(times: 3, body: ui.focus(node_id: "runtime-a"))"#.into();
        authority.start_session(request).unwrap();
        let mut ack = presentation_acknowledgement(
            "session-sequence-cancel",
            DebuggerPresentationOutcome::Applied {
                node_id: "runtime-a".into(),
                focused_node_id: None,
            },
        );
        ack.effect_id = authority.sessions["session-sequence-cancel"]
            .request
            .effect_id
            .clone();
        let view = authority.acknowledge_presentation(ack).unwrap().session;
        assert_eq!(view.projection.revision, Revision(8));
        let mut command = cancel_command("session-sequence-cancel", false);
        assert!(authority.cancel(command.clone()).is_err());
        command.expected_revision = Some(view.projection.revision);
        let cancelled = authority.cancel(command.clone()).unwrap();
        assert_eq!(cancelled.session.projection.state, DebuggerState::Cancelled);
        assert_eq!(authority.cancel(command).unwrap(), cancelled);
        assert_eq!(
            authority.sessions["session-sequence-cancel"]
                .vm
                .pending_count(),
            0
        );
        let path = authority.sessions["session-sequence-cancel"]
            .journal_path
            .clone();
        drop(authority);
        let mut recovered = Vm::open_journal(path, DEFAULT_FUEL).unwrap();
        assert_eq!(recovered.pending_count(), 0);
        assert!(
            recovered
                .claim_effect(now_ms().unwrap(), 100)
                .unwrap()
                .is_none()
        );
    }

    #[test]
    fn focus_navigation_requires_and_reenters_with_the_adapter_destination() {
        let root = TempRoot::new("focus-navigation");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-navigation");
        request.source =
            "fn main() = ui.navigate_focus(node_id: \"remote-fleet\", direction: \"next\")".into();
        let started = authority.start_session(request).unwrap();
        assert!(matches!(
            started.session.pending_presentation,
            Some(PresentationOperation::NavigateFocus {
                ref node_id,
                direction: leselang_hir::UiFocusNavigationDirection::Next,
            }) if node_id == "remote-fleet"
        ));

        let missing_destination = presentation_acknowledgement(
            "session-navigation",
            DebuggerPresentationOutcome::Applied {
                node_id: "remote-fleet".into(),
                focused_node_id: None,
            },
        );
        assert_eq!(
            authority
                .acknowledge_presentation(missing_destination)
                .unwrap_err()
                .code(),
            "debugger_presentation_result_invalid"
        );

        let advanced = authority
            .acknowledge_presentation(presentation_acknowledgement(
                "session-navigation",
                DebuggerPresentationOutcome::Applied {
                    node_id: "remote-fleet".into(),
                    focused_node_id: Some("runtime-card-a".into()),
                },
            ))
            .unwrap();
        assert_eq!(advanced.status, DebuggerPresentationStatus::Applied);
        assert_eq!(advanced.session.projection.state, DebuggerState::Completed);
        assert!(advanced.session.pending_presentation.is_none());
    }

    #[test]
    fn rejected_live_presentation_converges_to_a_visible_terminal_failure() {
        let root = TempRoot::new("rejected-presentation");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let mut request = start_request("session-presentation-rejected");
        request.source = "fn main() = ui.assert_visible(node_id: \"remote-fleet\")".into();
        authority.start_session(request).unwrap();

        let rejected = authority
            .acknowledge_presentation(presentation_acknowledgement(
                "session-presentation-rejected",
                DebuggerPresentationOutcome::Rejected {
                    node_id: "remote-fleet".into(),
                    code: "target_not_visible".into(),
                },
            ))
            .unwrap();
        assert_eq!(rejected.status, DebuggerPresentationStatus::Rejected);
        assert_eq!(rejected.session.projection.state, DebuggerState::Failed);
        assert_eq!(
            rejected
                .session
                .projection
                .fault
                .as_ref()
                .map(|fault| fault.code.as_str()),
            Some("debugger_presentation_target_not_visible")
        );
        assert!(rejected.session.pending_presentation.is_none());
    }

    #[test]
    fn session_start_is_bounded_idempotent_and_secret_free() {
        let root = TempRoot::new("bounds");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        let request = start_request("session-b");
        let started = authority.start_session(request.clone()).unwrap();
        assert_eq!(authority.start_session(request).unwrap(), started);
        let encoded = serde_json::to_string(&started).unwrap();
        assert!(!encoded.contains("fn main"));
        assert!(!encoded.contains("debugger-operator"));
        assert!(!encoded.contains("idempotency"));

        let mut drifted = start_request("session-b");
        drifted.source = "fn main() = runtime.list()".into();
        assert_eq!(
            authority.start_session(drifted).unwrap_err().code(),
            "debugger_session_conflict"
        );
        let mut completed = start_request("session-complete");
        completed.source = "fn main() = true".into();
        assert_eq!(
            authority.start_session(completed).unwrap_err().code(),
            "debugger_session_not_suspended"
        );
        assert!(!root.0.join("session-complete.sqlite").exists());

        fs::write(root.0.join("session-sidecar.sqlite-wal"), b"stale").unwrap();
        assert_eq!(
            authority
                .start_session(start_request("session-sidecar"))
                .unwrap_err()
                .code(),
            "debugger_session_recovery_required"
        );
        assert!(!root.0.join("session-sidecar.sqlite").exists());
    }

    #[test]
    fn expired_sessions_converge_and_release_bounded_capacity() {
        let root = TempRoot::new("deadline-capacity");
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        for index in 0..MAX_DEBUGGER_SESSIONS {
            authority
                .start_session(start_request(&format!("session-{index:02}")))
                .unwrap();
        }
        let deadline = authority
            .sessions
            .values()
            .filter_map(|session| session.request.continuation.deadline_at_ms)
            .max()
            .unwrap();
        authority.refresh_sessions_at(deadline).unwrap();
        assert!(authority.sessions.values().all(|session| {
            session.current_projection.state == DebuggerState::Failed
                && session.current_projection.pending_effect.is_none()
                && session.current_projection.deadline_remaining_ms.is_none()
                && session
                    .current_projection
                    .fault
                    .as_ref()
                    .is_some_and(|fault| fault.code == "debugger_deadline_exceeded")
        }));

        let retired_journal = authority.sessions["session-00"].journal_path.clone();
        let mut invalid = start_request("session-invalid");
        invalid.source = "fn main() = add(left: true, right: 1)".into();
        assert_eq!(
            authority.start_session(invalid).unwrap_err().code(),
            "debugger_source_invalid"
        );
        assert!(authority.sessions.contains_key("session-00"));
        assert!(retired_journal.exists());

        authority
            .start_session(start_request("session-replacement"))
            .unwrap();
        assert_eq!(authority.sessions.len(), MAX_DEBUGGER_SESSIONS);
        assert!(!authority.sessions.contains_key("session-00"));
        assert!(!retired_journal.exists());
    }

    #[test]
    fn stale_journal_retention_is_bounded_across_processes() {
        let root = TempRoot::new("journal-retention");
        fs::create_dir_all(&root.0).unwrap();
        for index in 0..MAX_RETAINED_DEBUGGER_JOURNALS {
            fs::write(root.0.join(format!("retained-{index:02}.sqlite")), []).unwrap();
        }
        let mut authority = DebuggerAuthority::open(&root.0).unwrap();
        authority
            .start_session(start_request("session-new"))
            .unwrap();
        let journal_count = fs::read_dir(&root.0)
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| {
                entry.path().extension().and_then(|value| value.to_str()) == Some("sqlite")
            })
            .count();
        assert_eq!(journal_count, MAX_RETAINED_DEBUGGER_JOURNALS);
        assert!(root.0.join("session-new.sqlite").exists());
    }
}
