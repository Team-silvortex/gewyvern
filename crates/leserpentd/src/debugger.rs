use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use leselang_hir::{CAPABILITY_UI_PRESENTATION, lower};
use leselang_observe::{execute_debugger_cancel, waiting_debugger_projection};
use leselang_syntax::parse;
use leselang_ui::{DebuggerFaultSummary, DebuggerProjection, DebuggerState, debugger_document};
use leselang_vm::{CancellationReason, DEFAULT_FUEL, EffectRequest, Step, Vm};
use leserpent_domain::{
    CAPABILITY_DEBUGGER_CONTROL, CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ,
    CAPABILITY_RUNTIME_REFRESH, CapabilitySet, CommandEnvelope, CommandPlan, PlannedOperation,
    Principal, Revision, validate_debugger_session_id,
};
use leserpent_protocol::{
    DebuggerCancelResponse, DebuggerMutationStatus, DebuggerSessionResponse,
    DebuggerSessionStartRequest, DebuggerSessionView, DebuggerSessionsRequest,
    DebuggerSessionsResponse,
};
use ring::digest::{SHA256, digest};

const MAX_DEBUGGER_SESSIONS: usize = 32;
const MAX_RETAINED_DEBUGGER_JOURNALS: usize = 64;
const MAX_DEBUGGER_SOURCE_BYTES: usize = 64 * 1024;
const MIN_DEBUGGER_TIMEOUT_MS: u64 = 100;
const MAX_DEBUGGER_TIMEOUT_MS: u64 = 5 * 60 * 1_000;

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
                    session: view(&existing.current_projection)?,
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
        let Step::Effect(preflight_effect) = Vm::default().start_timed(
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
            Vm::open_journal(&journal_path, DEFAULT_FUEL).map_err(|_| DebuggerAuthorityError {
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
        };
        self.next_sequence = self.next_sequence.saturating_add(1);
        let response = DebuggerSessionResponse {
            session: view(&projection)?,
        };
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
            .map(|(_, session)| view(&session.current_projection))
            .collect::<Result<Vec<_>, _>>()?;
        if request.session_id.is_some() && sessions.is_empty() {
            return Err(DebuggerAuthorityError {
                code: "debugger_session_not_found",
                message: "debugger session was not found",
            });
        }
        Ok(DebuggerSessionsResponse { sessions })
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
            session: view(&projection)?,
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
        if let Some(session_id) = oldest {
            let session = self
                .sessions
                .remove(&session_id)
                .expect("selected debugger session remains present");
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
    view(&projection)?;
    session.current_projection = projection;
    Ok(())
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

fn view(projection: &DebuggerProjection) -> Result<DebuggerSessionView, DebuggerAuthorityError> {
    Ok(DebuggerSessionView {
        projection: projection.clone(),
        document: debugger_document(projection).map_err(|_| DebuggerAuthorityError {
            code: "debugger_projection_invalid",
            message: "debugger VM state could not be projected safely",
        })?,
    })
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
    digest(&SHA256, source.as_bytes())
        .as_ref()
        .try_into()
        .expect("SHA-256 output is always 32 bytes")
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
            "debugger_source_invalid"
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
        invalid.source = "fn main() = true".into();
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
