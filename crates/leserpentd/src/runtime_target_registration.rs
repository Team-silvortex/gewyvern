use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::sync::Arc;

use leserpent_adapters::{
    GewyvernTarget, GewyvernTargetCatalog, MutableSecretStore, SecretKey, SecretStoreError,
    SecretValue, validate_gewyvern_admin_secret,
};
use leserpent_domain::{
    CAPABILITY_RUNTIME_REGISTER, COMMAND_PLAN_SCHEMA_VERSION, CapabilitySet, Command,
    CommandEnvelope, CommandId, CommandOrigin, CommandPlan, Confirmation, IdempotencyKey,
    PlannedOperation, Principal, Revision, RuntimeId, RuntimeProjection, RuntimeTags,
    canonical_runtime_endpoint_identity, validate_registration_intent,
};
use leserpent_runtime::{
    ControlRuntime, PlanResult, RuntimeError, RuntimeTargetRegistrationAdmission,
    RuntimeTargetRegistrationRecord,
};
use serde::{Deserialize, Serialize};

use crate::wire::constant_time_equals;

const TARGET_BINDING_SCHEMA_VERSION: u32 = 1;
const OPERATION_PREFIX: &str = "web-register-";
const SECRET_PREFIX: &str = "runtime-target-secret-";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RuntimeTargetRegistrationAction {
    Create,
    Update,
}

pub(crate) struct RuntimeTargetDescriptor {
    pub(crate) runtime_id: RuntimeId,
    pub(crate) name: String,
    pub(crate) endpoint: String,
    pub(crate) sidecar_endpoint: Option<String>,
    pub(crate) tags: RuntimeTags,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RuntimeTargetRegistrationIntent {
    schema_version: u32,
    pub(crate) action: RuntimeTargetRegistrationAction,
    pub(crate) runtime_id: RuntimeId,
    pub(crate) name: String,
    pub(crate) endpoint: String,
    pub(crate) sidecar_endpoint: Option<String>,
    pub(crate) tags: RuntimeTags,
    pub(crate) expected_revision: Option<Revision>,
    pub(crate) plan_token: String,
}

impl RuntimeTargetRegistrationIntent {
    pub(crate) fn new(
        action: RuntimeTargetRegistrationAction,
        target: RuntimeTargetDescriptor,
        expected_revision: Option<Revision>,
        plan_token: String,
    ) -> Result<Self, RuntimeTargetRegistrationError> {
        let RuntimeTargetDescriptor {
            runtime_id,
            name,
            endpoint,
            sidecar_endpoint,
            tags,
        } = target;
        validate_plan_token(&plan_token)?;
        validate_registration_intent(&name, &endpoint, sidecar_endpoint.as_deref(), &tags)
            .map_err(|_| RuntimeTargetRegistrationError::invalid())?;
        loopback_address(&endpoint)?;
        if matches!(action, RuntimeTargetRegistrationAction::Create) != expected_revision.is_none()
        {
            return Err(RuntimeTargetRegistrationError::invalid());
        }
        Ok(Self {
            schema_version: TARGET_BINDING_SCHEMA_VERSION,
            action,
            runtime_id,
            name,
            endpoint,
            sidecar_endpoint,
            tags,
            expected_revision,
            plan_token,
        })
    }

    pub(crate) fn operation_id(&self) -> String {
        format!("{OPERATION_PREFIX}{}", self.plan_token)
    }

    fn secret_key(&self) -> Result<SecretKey, RuntimeTargetRegistrationError> {
        SecretKey::new(format!("{SECRET_PREFIX}{}", self.plan_token))
            .map_err(|_| RuntimeTargetRegistrationError::internal())
    }

    fn payload(&self) -> Result<Vec<u8>, RuntimeTargetRegistrationError> {
        serde_json::to_vec(self).map_err(|_| RuntimeTargetRegistrationError::internal())
    }

    fn target(&self) -> Result<GewyvernTarget, RuntimeTargetRegistrationError> {
        GewyvernTarget::loopback(loopback_address(&self.endpoint)?, Some(self.secret_key()?))
            .map_err(|_| RuntimeTargetRegistrationError::invalid())
    }

    fn matches_projection(&self, projection: &RuntimeProjection) -> bool {
        projection.id == self.runtime_id
            && projection.name == self.name
            && projection.endpoint == self.endpoint
            && projection.sidecar_endpoint == self.sidecar_endpoint
            && projection.tags == self.tags
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RuntimeTargetRegistrationErrorKind {
    Invalid,
    Conflict,
    Unavailable,
    Internal,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeTargetRegistrationError {
    pub(crate) kind: RuntimeTargetRegistrationErrorKind,
}

impl RuntimeTargetRegistrationError {
    fn invalid() -> Self {
        Self {
            kind: RuntimeTargetRegistrationErrorKind::Invalid,
        }
    }

    fn conflict() -> Self {
        Self {
            kind: RuntimeTargetRegistrationErrorKind::Conflict,
        }
    }

    fn unavailable() -> Self {
        Self {
            kind: RuntimeTargetRegistrationErrorKind::Unavailable,
        }
    }

    fn internal() -> Self {
        Self {
            kind: RuntimeTargetRegistrationErrorKind::Internal,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct RuntimeTargetRegistrationOutcome {
    pub(crate) projection: RuntimeProjection,
    pub(crate) registration_revision: Revision,
    pub(crate) replayed: bool,
}

#[derive(Clone)]
pub struct RuntimeTargetRegistrationAuthority {
    targets: GewyvernTargetCatalog,
    secrets: Arc<dyn MutableSecretStore>,
}

impl RuntimeTargetRegistrationAuthority {
    pub fn new(targets: GewyvernTargetCatalog, secrets: Arc<dyn MutableSecretStore>) -> Self {
        Self { targets, secrets }
    }

    pub(crate) fn validate_import_bindings(
        &self,
        runtime: &mut ControlRuntime,
        projections: &[RuntimeProjection],
    ) -> Result<Vec<String>, RuntimeTargetRegistrationError> {
        let projections = projections
            .iter()
            .map(|projection| (projection.id.as_str(), projection))
            .collect::<BTreeMap<_, _>>();
        for (runtime_id, endpoint) in self
            .targets
            .endpoint_origins()
            .map_err(|_| RuntimeTargetRegistrationError::unavailable())?
        {
            let projection = projections
                .get(runtime_id.as_str())
                .ok_or_else(RuntimeTargetRegistrationError::conflict)?;
            if canonical_runtime_endpoint_identity(&projection.endpoint)
                != canonical_runtime_endpoint_identity(&endpoint)
            {
                return Err(RuntimeTargetRegistrationError::conflict());
            }
        }
        let mut runtime_ids = Vec::new();
        for binding in runtime
            .runtime_target_bindings()
            .map_err(map_runtime_error)?
        {
            let intent = parse_payload(&binding.payload)?;
            validate_record_identity(
                &intent,
                &binding.operation_id,
                &binding.runtime_id,
                &binding.secret_key,
            )?;
            let projection = projections
                .get(binding.runtime_id.as_str())
                .ok_or_else(RuntimeTargetRegistrationError::conflict)?;
            if !intent.matches_projection(projection) {
                return Err(RuntimeTargetRegistrationError::conflict());
            }
            let secret_key = intent.secret_key()?;
            if self
                .secrets
                .load(&secret_key)
                .map_err(map_secret_error)?
                .is_none()
                || !self
                    .targets
                    .contains(&binding.runtime_id)
                    .map_err(|_| RuntimeTargetRegistrationError::unavailable())?
            {
                return Err(RuntimeTargetRegistrationError::unavailable());
            }
            runtime_ids.push(binding.runtime_id);
        }
        runtime_ids.sort();
        Ok(runtime_ids)
    }

    pub(crate) fn persisted_intent(
        &self,
        runtime: &mut ControlRuntime,
        operation_id: &str,
    ) -> Result<Option<RuntimeTargetRegistrationIntent>, RuntimeTargetRegistrationError> {
        let pending = runtime
            .pending_runtime_target_registrations()
            .map_err(map_runtime_error)?;
        if let Some(record) = pending
            .into_iter()
            .find(|record| record.operation_id == operation_id)
        {
            return parse_record(&record).map(Some);
        }
        runtime
            .runtime_target_bindings()
            .map_err(map_runtime_error)?
            .into_iter()
            .find(|record| record.operation_id == operation_id)
            .map(|record| {
                parse_payload(&record.payload).and_then(|intent| {
                    validate_record_identity(
                        &intent,
                        &record.operation_id,
                        &record.runtime_id,
                        &record.secret_key,
                    )?;
                    Ok(intent)
                })
            })
            .transpose()
    }

    pub(crate) fn execute(
        &self,
        runtime: &mut ControlRuntime,
        intent: &RuntimeTargetRegistrationIntent,
        supplied_secret: &SecretValue,
    ) -> Result<RuntimeTargetRegistrationOutcome, RuntimeTargetRegistrationError> {
        validate_gewyvern_admin_secret(supplied_secret)
            .map_err(|_| RuntimeTargetRegistrationError::invalid())?;
        let operation_id = intent.operation_id();
        let secret_key = intent.secret_key()?;
        let payload = intent.payload()?;
        let admission = runtime
            .begin_runtime_target_registration(
                &operation_id,
                &intent.runtime_id,
                secret_key.as_str(),
                &payload,
            )
            .map_err(map_runtime_error)?;
        self.ensure_secret(&secret_key, supplied_secret)?;

        let command_projection = match apply_registration(runtime, intent, &operation_id) {
            Ok(projection) => projection,
            Err(error)
                if error.kind == RuntimeTargetRegistrationErrorKind::Conflict
                    && admission != RuntimeTargetRegistrationAdmission::CommittedReplay =>
            {
                self.abort_conflicted_registration(runtime, &operation_id)?;
                return Err(error);
            }
            Err(error) => return Err(error),
        };
        self.targets
            .upsert(intent.runtime_id.as_str().to_string(), intent.target()?)
            .map_err(|_| RuntimeTargetRegistrationError::unavailable())?;
        let commit = runtime
            .commit_runtime_target_registration(&operation_id)
            .map_err(map_runtime_error)?;
        if commit.binding.runtime_id != intent.runtime_id.as_str()
            || commit.binding.secret_key != secret_key.as_str()
            || commit.binding.payload != payload
        {
            return Err(RuntimeTargetRegistrationError::internal());
        }
        self.collect_secrets(runtime)
            .map_err(|_| RuntimeTargetRegistrationError::unavailable())?;
        let registration_revision = command_projection.revision;
        let projection = runtime
            .runtime_projection(&intent.runtime_id)
            .filter(|projection| intent.matches_projection(projection))
            .cloned()
            .unwrap_or(command_projection);
        Ok(RuntimeTargetRegistrationOutcome {
            projection,
            registration_revision,
            replayed: admission == RuntimeTargetRegistrationAdmission::CommittedReplay
                || commit.replayed,
        })
    }

    pub fn recover(&self, runtime: &mut ControlRuntime) -> Result<(), String> {
        for binding in runtime
            .runtime_target_bindings()
            .map_err(|error| error.to_string())?
        {
            let intent = parse_payload(&binding.payload).map_err(recovery_error)?;
            validate_record_identity(
                &intent,
                &binding.operation_id,
                &binding.runtime_id,
                &binding.secret_key,
            )
            .map_err(recovery_error)?;
            let Some(projection) = runtime.runtime_projection(&intent.runtime_id) else {
                runtime
                    .retire_runtime_target_binding(&intent.runtime_id)
                    .map_err(|error| error.to_string())?;
                continue;
            };
            if !intent.matches_projection(projection) {
                return Err(
                    "persisted runtime target binding conflicts with runtime authority".into(),
                );
            }
            let secret_key = intent.secret_key().map_err(recovery_error)?;
            if self
                .secrets
                .load(&secret_key)
                .map_err(|_| "runtime target secret store is unavailable".to_string())?
                .is_none()
            {
                return Err("persisted runtime target secret is missing".into());
            }
            self.targets.upsert(
                intent.runtime_id.as_str().to_string(),
                intent.target().map_err(recovery_error)?,
            )?;
        }

        for record in runtime
            .pending_runtime_target_registrations()
            .map_err(|error| error.to_string())?
        {
            let intent = parse_record(&record).map_err(recovery_error)?;
            let secret_key = intent.secret_key().map_err(recovery_error)?;
            if self
                .secrets
                .load(&secret_key)
                .map_err(|_| "runtime target secret store is unavailable".to_string())?
                .is_none()
            {
                continue;
            }
            match apply_registration(runtime, &intent, &record.operation_id) {
                Ok(_) => {}
                Err(error) if error.kind == RuntimeTargetRegistrationErrorKind::Conflict => {
                    self.abort_conflicted_registration(runtime, &record.operation_id)
                        .map_err(recovery_error)?;
                    continue;
                }
                Err(error) => return Err(recovery_error(error)),
            }
            self.targets.upsert(
                intent.runtime_id.as_str().to_string(),
                intent.target().map_err(recovery_error)?,
            )?;
            runtime
                .commit_runtime_target_registration(&record.operation_id)
                .map_err(|error| error.to_string())?;
        }
        self.collect_secrets(runtime)
    }

    pub(crate) fn retire(
        &self,
        runtime: &mut ControlRuntime,
        runtime_id: &RuntimeId,
    ) -> Result<(), RuntimeTargetRegistrationError> {
        runtime
            .retire_runtime_target_binding(runtime_id)
            .map_err(map_runtime_error)?;
        self.targets
            .remove(runtime_id.as_str())
            .map_err(|_| RuntimeTargetRegistrationError::unavailable())?;
        self.collect_secrets(runtime)
            .map_err(|_| RuntimeTargetRegistrationError::unavailable())
    }

    fn ensure_secret(
        &self,
        key: &SecretKey,
        supplied: &SecretValue,
    ) -> Result<(), RuntimeTargetRegistrationError> {
        match self.secrets.load(key) {
            Ok(Some(existing)) => constant_time_equals(
                existing.expose_secret().as_bytes(),
                supplied.expose_secret().as_bytes(),
            )
            .then_some(())
            .ok_or_else(RuntimeTargetRegistrationError::conflict),
            Ok(None) => self
                .secrets
                .store_atomic(key, supplied)
                .map_err(map_secret_error),
            Err(error) => Err(map_secret_error(error)),
        }
    }

    fn abort_conflicted_registration(
        &self,
        runtime: &mut ControlRuntime,
        operation_id: &str,
    ) -> Result<(), RuntimeTargetRegistrationError> {
        if !runtime
            .abort_runtime_target_registration(operation_id)
            .map_err(map_runtime_error)?
        {
            return Err(RuntimeTargetRegistrationError::internal());
        }
        self.collect_secrets(runtime)
            .map_err(|_| RuntimeTargetRegistrationError::unavailable())
    }

    fn collect_secrets(&self, runtime: &mut ControlRuntime) -> Result<(), String> {
        loop {
            let keys = runtime
                .runtime_target_secret_gc_batch()
                .map_err(|error| error.to_string())?;
            if keys.is_empty() {
                return Ok(());
            }
            for key in keys {
                let secret_key = SecretKey::new(key.clone())
                    .map_err(|_| "persisted runtime target secret key is invalid".to_string())?;
                self.secrets
                    .remove(&secret_key)
                    .map_err(|_| "runtime target secret store is unavailable".to_string())?;
                if !runtime
                    .acknowledge_runtime_target_secret_gc(&key)
                    .map_err(|error| error.to_string())?
                {
                    return Err("runtime target secret cleanup acknowledgement was lost".into());
                }
            }
        }
    }
}

fn parse_record(
    record: &RuntimeTargetRegistrationRecord,
) -> Result<RuntimeTargetRegistrationIntent, RuntimeTargetRegistrationError> {
    let intent = parse_payload(&record.payload)?;
    validate_record_identity(
        &intent,
        &record.operation_id,
        &record.runtime_id,
        &record.secret_key,
    )?;
    Ok(intent)
}

fn parse_payload(
    payload: &[u8],
) -> Result<RuntimeTargetRegistrationIntent, RuntimeTargetRegistrationError> {
    let intent: RuntimeTargetRegistrationIntent =
        serde_json::from_slice(payload).map_err(|_| RuntimeTargetRegistrationError::internal())?;
    if intent.schema_version != TARGET_BINDING_SCHEMA_VERSION {
        return Err(RuntimeTargetRegistrationError::internal());
    }
    RuntimeTargetRegistrationIntent::new(
        intent.action,
        RuntimeTargetDescriptor {
            runtime_id: intent.runtime_id,
            name: intent.name,
            endpoint: intent.endpoint,
            sidecar_endpoint: intent.sidecar_endpoint,
            tags: intent.tags,
        },
        intent.expected_revision,
        intent.plan_token,
    )
}

fn validate_record_identity(
    intent: &RuntimeTargetRegistrationIntent,
    operation_id: &str,
    runtime_id: &str,
    secret_key: &str,
) -> Result<(), RuntimeTargetRegistrationError> {
    if intent.operation_id() != operation_id
        || intent.runtime_id.as_str() != runtime_id
        || intent.secret_key()?.as_str() != secret_key
    {
        return Err(RuntimeTargetRegistrationError::internal());
    }
    Ok(())
}

fn apply_registration(
    runtime: &mut ControlRuntime,
    intent: &RuntimeTargetRegistrationIntent,
    operation_id: &str,
) -> Result<RuntimeProjection, RuntimeTargetRegistrationError> {
    let command = match intent.action {
        RuntimeTargetRegistrationAction::Create => Command::RuntimeRegister {
            runtime_id: intent.runtime_id.clone(),
            name: intent.name.clone(),
            endpoint: intent.endpoint.clone(),
            sidecar_endpoint: intent.sidecar_endpoint.clone(),
            tags: intent.tags.clone(),
        },
        RuntimeTargetRegistrationAction::Update => Command::RuntimeRegistrationUpdate {
            runtime_id: intent.runtime_id.clone(),
            name: intent.name.clone(),
            endpoint: intent.endpoint.clone(),
            sidecar_endpoint: intent.sidecar_endpoint.clone(),
            tags: intent.tags.clone(),
        },
    };
    let envelope = CommandEnvelope {
        schema_version: leserpent_domain::DOMAIN_SCHEMA_VERSION,
        command_id: CommandId::new(operation_id)
            .map_err(|_| RuntimeTargetRegistrationError::internal())?,
        idempotency_key: IdempotencyKey::new(operation_id)
            .map_err(|_| RuntimeTargetRegistrationError::internal())?,
        expected_revision: intent.expected_revision,
        principal: Principal {
            id: "rust-web-console".into(),
        },
        capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
        origin: CommandOrigin::CompatibilityAdapter,
        confirmation: Confirmation::Confirmed,
        dry_run: false,
        command,
    };
    match runtime.execute_plan(CommandPlan {
        schema_version: COMMAND_PLAN_SCHEMA_VERSION,
        required_capability: CAPABILITY_RUNTIME_REGISTER.into(),
        operation: PlannedOperation::Command(envelope),
    }) {
        Ok(PlanResult::Command(result)) => Ok(result.runtime),
        Ok(PlanResult::Query(_)) => Err(RuntimeTargetRegistrationError::internal()),
        Err(error) => Err(map_runtime_error(error)),
    }
}

pub(crate) fn loopback_address(
    endpoint: &str,
) -> Result<SocketAddr, RuntimeTargetRegistrationError> {
    let authority = endpoint
        .strip_prefix("http://")
        .and_then(|value| value.strip_suffix('/'))
        .filter(|value| !value.is_empty() && !value.contains(['/', '?', '#', '@']))
        .ok_or_else(RuntimeTargetRegistrationError::invalid)?;
    let address = authority
        .parse::<SocketAddr>()
        .map_err(|_| RuntimeTargetRegistrationError::invalid())?;
    if !address.ip().is_loopback() || address.port() == 0 {
        return Err(RuntimeTargetRegistrationError::invalid());
    }
    Ok(address)
}

fn validate_plan_token(token: &str) -> Result<(), RuntimeTargetRegistrationError> {
    if token.len() != 64
        || !token
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
    {
        return Err(RuntimeTargetRegistrationError::invalid());
    }
    Ok(())
}

fn map_runtime_error(error: RuntimeError) -> RuntimeTargetRegistrationError {
    match error {
        RuntimeError::Storage(message)
            if matches!(
                message.as_str(),
                "runtime target secret key is already reserved"
                    | "runtime target registration is already pending"
                    | "runtime target registration operation identity conflicts"
            ) =>
        {
            RuntimeTargetRegistrationError::conflict()
        }
        RuntimeError::Storage(_) => RuntimeTargetRegistrationError::unavailable(),
        RuntimeError::Domain(_)
        | RuntimeError::InvalidPlan(_)
        | RuntimeError::AuthorityWriterFence(_) => RuntimeTargetRegistrationError::conflict(),
        _ => RuntimeTargetRegistrationError::internal(),
    }
}

fn map_secret_error(error: SecretStoreError) -> RuntimeTargetRegistrationError {
    match error {
        SecretStoreError::InvalidKey | SecretStoreError::InvalidValue => {
            RuntimeTargetRegistrationError::invalid()
        }
        SecretStoreError::InvalidEnvironmentName | SecretStoreError::Unavailable => {
            RuntimeTargetRegistrationError::unavailable()
        }
    }
}

fn recovery_error(_error: RuntimeTargetRegistrationError) -> String {
    "persisted runtime target registration is invalid".into()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    use leserpent_adapters::SecretStore;

    use super::*;

    #[derive(Default)]
    struct MemorySecretStore {
        values: Mutex<BTreeMap<String, String>>,
    }

    impl SecretStore for MemorySecretStore {
        fn load(&self, key: &SecretKey) -> Result<Option<SecretValue>, SecretStoreError> {
            self.values
                .lock()
                .map_err(|_| SecretStoreError::Unavailable)?
                .get(key.as_str())
                .map(|value| SecretValue::new(value.clone()))
                .transpose()
        }
    }

    impl MutableSecretStore for MemorySecretStore {
        fn store_atomic(
            &self,
            key: &SecretKey,
            value: &SecretValue,
        ) -> Result<(), SecretStoreError> {
            self.values
                .lock()
                .map_err(|_| SecretStoreError::Unavailable)?
                .insert(key.as_str().to_string(), value.expose_secret().to_string());
            Ok(())
        }

        fn remove(&self, key: &SecretKey) -> Result<bool, SecretStoreError> {
            Ok(self
                .values
                .lock()
                .map_err(|_| SecretStoreError::Unavailable)?
                .remove(key.as_str())
                .is_some())
        }
    }

    fn temp_database(label: &str) -> PathBuf {
        let nonce = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "leserpent-target-registration-{label}-{}-{nonce}.sqlite",
            std::process::id()
        ))
    }

    fn intent_for(
        token: char,
        runtime_id: &str,
        endpoint: &str,
    ) -> RuntimeTargetRegistrationIntent {
        RuntimeTargetRegistrationIntent::new(
            RuntimeTargetRegistrationAction::Create,
            RuntimeTargetDescriptor {
                runtime_id: RuntimeId::new(runtime_id).unwrap(),
                name: "Runtime Target A".into(),
                endpoint: endpoint.into(),
                sidecar_endpoint: None,
                tags: RuntimeTags::default(),
            },
            None,
            token.to_string().repeat(64),
        )
        .unwrap()
    }

    fn intent(token: char, endpoint: &str) -> RuntimeTargetRegistrationIntent {
        intent_for(token, "runtime-target-a", endpoint)
    }

    #[test]
    fn registration_never_persists_secret_and_replays_after_restart() {
        let path = temp_database("replay");
        let secrets = Arc::new(MemorySecretStore::default());
        let targets = GewyvernTargetCatalog::default();
        let authority = RuntimeTargetRegistrationAuthority::new(targets.clone(), secrets.clone());
        let registration = intent('a', "http://127.0.0.1:9411/");
        let supplied = SecretValue::new("pairing-super-secret").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();

        let first = authority
            .execute(&mut runtime, &registration, &supplied)
            .unwrap();
        assert!(!first.replayed);
        assert!(targets.contains("runtime-target-a").unwrap());
        let bindings = runtime.runtime_target_bindings().unwrap();
        assert_eq!(bindings.len(), 1);
        assert!(
            !bindings[0]
                .payload
                .windows("pairing-super-secret".len())
                .any(|window| window == b"pairing-super-secret")
        );
        let replay = authority
            .execute(&mut runtime, &registration, &supplied)
            .unwrap();
        assert!(replay.replayed);
        assert_eq!(replay.projection.id, first.projection.id);
        let wrong = SecretValue::new("different-pairing-secret").unwrap();
        assert_eq!(
            authority
                .execute(&mut runtime, &registration, &wrong)
                .unwrap_err()
                .kind,
            RuntimeTargetRegistrationErrorKind::Conflict
        );
        drop(runtime);

        let recovered_targets = GewyvernTargetCatalog::default();
        let recovered_authority =
            RuntimeTargetRegistrationAuthority::new(recovered_targets.clone(), secrets);
        let mut recovered = ControlRuntime::open(&path).unwrap();
        recovered_authority.recover(&mut recovered).unwrap();
        assert!(recovered_targets.contains("runtime-target-a").unwrap());
        assert_eq!(recovered.runtime_target_bindings().unwrap().len(), 1);
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn recovery_resumes_each_pre_commit_crash_window() {
        let path = temp_database("crash-windows");
        let secrets = Arc::new(MemorySecretStore::default());
        let registration = intent('b', "http://127.0.0.1:9412/");
        let operation_id = registration.operation_id();
        let secret_key = registration.secret_key().unwrap();
        let payload = registration.payload().unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();
        assert_eq!(
            runtime
                .begin_runtime_target_registration(
                    &operation_id,
                    &registration.runtime_id,
                    secret_key.as_str(),
                    &payload,
                )
                .unwrap(),
            RuntimeTargetRegistrationAdmission::Prepared
        );
        drop(runtime);

        let targets = GewyvernTargetCatalog::default();
        let authority = RuntimeTargetRegistrationAuthority::new(targets.clone(), secrets.clone());
        let mut recovered = ControlRuntime::open(&path).unwrap();
        authority.recover(&mut recovered).unwrap();
        assert_eq!(
            recovered
                .pending_runtime_target_registrations()
                .unwrap()
                .len(),
            1
        );
        assert!(
            recovered
                .runtime_projection(&registration.runtime_id)
                .is_none()
        );
        let supplied = SecretValue::new("crash-window-secret").unwrap();
        authority
            .ensure_secret(&secret_key, &supplied)
            .expect("secret write after intent must succeed");
        apply_registration(&mut recovered, &registration, &operation_id).unwrap();
        drop(recovered);

        let final_targets = GewyvernTargetCatalog::default();
        let final_authority =
            RuntimeTargetRegistrationAuthority::new(final_targets.clone(), secrets);
        let mut final_runtime = ControlRuntime::open(&path).unwrap();
        final_authority.recover(&mut final_runtime).unwrap();
        assert!(
            final_runtime
                .pending_runtime_target_registrations()
                .unwrap()
                .is_empty()
        );
        assert_eq!(final_runtime.runtime_target_bindings().unwrap().len(), 1);
        assert!(final_targets.contains("runtime-target-a").unwrap());
        drop(final_runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn deterministic_conflicts_abort_intent_and_collect_secret() {
        let path = temp_database("deterministic-conflict");
        let secrets = Arc::new(MemorySecretStore::default());
        let targets = GewyvernTargetCatalog::default();
        let authority = RuntimeTargetRegistrationAuthority::new(targets.clone(), secrets.clone());
        let registration = intent('c', "http://127.0.0.1:9413/");
        let competing = intent('d', "http://127.0.0.1:9414/");
        let secret_key = registration.secret_key().unwrap();
        let supplied = SecretValue::new("conflicted-secret").unwrap();
        let mut runtime = ControlRuntime::open(&path).unwrap();

        apply_registration(&mut runtime, &competing, &competing.operation_id()).unwrap();
        assert_eq!(
            authority
                .execute(&mut runtime, &registration, &supplied)
                .unwrap_err()
                .kind,
            RuntimeTargetRegistrationErrorKind::Conflict
        );
        assert!(
            runtime
                .pending_runtime_target_registrations()
                .unwrap()
                .is_empty()
        );
        assert!(runtime.runtime_target_secret_gc_batch().unwrap().is_empty());
        assert!(secrets.load(&secret_key).unwrap().is_none());
        assert!(!targets.contains("runtime-target-a").unwrap());

        let recovering = intent_for('e', "runtime-target-b", "http://127.0.0.1:9415/");
        let recovery_competitor = intent_for('f', "runtime-target-b", "http://127.0.0.1:9416/");
        let recovering_key = recovering.secret_key().unwrap();
        runtime
            .begin_runtime_target_registration(
                &recovering.operation_id(),
                &recovering.runtime_id,
                recovering_key.as_str(),
                &recovering.payload().unwrap(),
            )
            .unwrap();
        authority
            .ensure_secret(
                &recovering_key,
                &SecretValue::new("recovery-conflict-secret").unwrap(),
            )
            .unwrap();
        apply_registration(
            &mut runtime,
            &recovery_competitor,
            &recovery_competitor.operation_id(),
        )
        .unwrap();
        drop(runtime);

        let mut recovered = ControlRuntime::open(&path).unwrap();
        authority.recover(&mut recovered).unwrap();
        assert!(
            recovered
                .pending_runtime_target_registrations()
                .unwrap()
                .is_empty()
        );
        assert!(
            recovered
                .runtime_target_secret_gc_batch()
                .unwrap()
                .is_empty()
        );
        assert!(secrets.load(&recovering_key).unwrap().is_none());
        drop(recovered);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn secret_collection_drains_more_than_one_persistence_batch() {
        let path = temp_database("gc-batches");
        let secrets = Arc::new(MemorySecretStore::default());
        let authority = RuntimeTargetRegistrationAuthority::new(
            GewyvernTargetCatalog::default(),
            secrets.clone(),
        );
        let mut runtime = ControlRuntime::open(&path).unwrap();

        for index in 0..129 {
            let key = SecretKey::new(format!("queued-secret-{index}")).unwrap();
            secrets
                .store_atomic(&key, &SecretValue::new(format!("secret-{index}")).unwrap())
                .unwrap();
            let operation_id = format!("queued-operation-{index}");
            runtime
                .begin_runtime_target_registration(
                    &operation_id,
                    &RuntimeId::new(format!("runtime-gc-{index}")).unwrap(),
                    key.as_str(),
                    b"queued",
                )
                .unwrap();
            assert!(
                runtime
                    .abort_runtime_target_registration(&operation_id)
                    .unwrap()
            );
        }
        assert_eq!(runtime.runtime_target_secret_gc_batch().unwrap().len(), 128);

        authority.collect_secrets(&mut runtime).unwrap();
        assert!(runtime.runtime_target_secret_gc_batch().unwrap().is_empty());
        assert!(secrets.values.lock().unwrap().is_empty());
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn unsafe_admin_secret_is_rejected_before_intent_persistence() {
        let path = temp_database("unsafe-secret");
        let secrets = Arc::new(MemorySecretStore::default());
        let authority = RuntimeTargetRegistrationAuthority::new(
            GewyvernTargetCatalog::default(),
            secrets.clone(),
        );
        let registration = intent('9', "http://127.0.0.1:9417/");
        let mut runtime = ControlRuntime::open(&path).unwrap();

        assert_eq!(
            authority
                .execute(
                    &mut runtime,
                    &registration,
                    &SecretValue::new("unsafe token").unwrap(),
                )
                .unwrap_err()
                .kind,
            RuntimeTargetRegistrationErrorKind::Invalid
        );
        assert!(
            runtime
                .pending_runtime_target_registrations()
                .unwrap()
                .is_empty()
        );
        assert!(secrets.values.lock().unwrap().is_empty());
        drop(runtime);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn loopback_target_parser_rejects_ambient_remote_trust() {
        assert!(loopback_address("http://127.0.0.1:9411/").is_ok());
        assert!(loopback_address("http://[::1]:9411/").is_ok());
        assert!(loopback_address("http://192.0.2.1:9411/").is_err());
        assert!(loopback_address("https://127.0.0.1:9411/").is_err());
        assert!(loopback_address("http://127.0.0.1:9411/path").is_err());
    }
}
