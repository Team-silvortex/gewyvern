use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use serde::{Deserialize, Serialize};

pub const DOMAIN_SCHEMA_VERSION: u32 = 1;
pub const CAPABILITY_RUNTIME_READ: &str = "runtime.read";
pub const CAPABILITY_RUNTIME_REFRESH: &str = "runtime.refresh";

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct RuntimeId(String);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CommandId(String);

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct IdempotencyKey(String);

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct Revision(pub u64);

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct Principal {
    pub id: String,
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(transparent)]
pub struct CapabilitySet(BTreeSet<String>);

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandOrigin {
    Gui,
    Cli,
    Leselang,
    Model,
    CompatibilityAdapter,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Confirmation {
    NotRequired,
    Confirmed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Command {
    RuntimeRefresh { runtime_id: RuntimeId },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Query {
    RuntimeList,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandEnvelope {
    pub schema_version: u32,
    pub command_id: CommandId,
    pub idempotency_key: IdempotencyKey,
    pub expected_revision: Option<Revision>,
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub origin: CommandOrigin,
    pub confirmation: Confirmation,
    pub dry_run: bool,
    pub command: Command,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct QueryEnvelope {
    pub schema_version: u32,
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub query: Query,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RuntimeProjection {
    pub id: RuntimeId,
    pub name: String,
    pub endpoint: String,
    pub revision: Revision,
    pub refresh_count: u64,
    pub refresh_status: RefreshStatus,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RefreshStatus {
    NeverRequested,
    Pending,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DomainEvent {
    RuntimeRefreshRequested {
        runtime_id: RuntimeId,
        revision: Revision,
        command_id: CommandId,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus {
    Planned,
    Applied,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct CommandResult {
    pub command_id: CommandId,
    pub status: CommandStatus,
    pub runtime: RuntimeProjection,
    pub events: Vec<DomainEvent>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum QueryResult {
    RuntimeList {
        revision: Revision,
        runtimes: Vec<RuntimeProjection>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DomainError {
    InvalidIdentifier {
        field: &'static str,
    },
    InvalidSchemaVersion {
        actual: u32,
        expected: u32,
    },
    Unauthorized {
        capability: &'static str,
    },
    RuntimeNotFound {
        runtime_id: String,
    },
    RevisionConflict {
        expected: Revision,
        actual: Revision,
    },
    IdempotencyConflict {
        key: String,
    },
}

#[derive(Clone, Debug)]
struct AppliedCommand {
    command: Command,
    result: CommandResult,
}

#[derive(Default)]
pub struct InMemoryControlPlane {
    revision: u64,
    runtimes: BTreeMap<RuntimeId, RuntimeProjection>,
    applied: BTreeMap<(String, IdempotencyKey), AppliedCommand>,
}

impl RuntimeId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        validated_identifier("runtime_id", value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CommandId {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        validated_identifier("command_id", value.into()).map(Self)
    }
}

impl IdempotencyKey {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        validated_identifier("idempotency_key", value.into()).map(Self)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl CapabilitySet {
    pub fn new(values: impl IntoIterator<Item = impl Into<String>>) -> Self {
        Self(values.into_iter().map(Into::into).collect())
    }

    pub fn contains(&self, capability: &str) -> bool {
        self.0.contains(capability)
    }
}

impl InMemoryControlPlane {
    pub fn register_runtime(
        &mut self,
        id: RuntimeId,
        name: impl Into<String>,
        endpoint: impl Into<String>,
    ) -> RuntimeProjection {
        self.revision += 1;
        let projection = RuntimeProjection {
            id: id.clone(),
            name: name.into(),
            endpoint: endpoint.into(),
            revision: Revision(self.revision),
            refresh_count: 0,
            refresh_status: RefreshStatus::NeverRequested,
        };
        self.runtimes.insert(id, projection.clone());
        projection
    }

    pub fn query(&self, envelope: QueryEnvelope) -> Result<QueryResult, DomainError> {
        validate_schema(envelope.schema_version)?;
        validate_principal(&envelope.principal)?;
        require_capability(&envelope.capabilities, CAPABILITY_RUNTIME_READ)?;
        match envelope.query {
            Query::RuntimeList => Ok(QueryResult::RuntimeList {
                revision: Revision(self.revision),
                runtimes: self.runtimes.values().cloned().collect(),
            }),
        }
    }

    pub fn execute(&mut self, envelope: CommandEnvelope) -> Result<CommandResult, DomainError> {
        validate_schema(envelope.schema_version)?;
        validate_principal(&envelope.principal)?;
        match &envelope.command {
            Command::RuntimeRefresh { .. } => {
                require_capability(&envelope.capabilities, CAPABILITY_RUNTIME_REFRESH)?;
            }
        }

        let idempotency_scope = (
            envelope.principal.id.clone(),
            envelope.idempotency_key.clone(),
        );
        if let Some(applied) = self.applied.get(&idempotency_scope) {
            return if applied.command == envelope.command {
                Ok(applied.result.clone())
            } else {
                Err(DomainError::IdempotencyConflict {
                    key: envelope.idempotency_key.as_str().to_string(),
                })
            };
        }

        match envelope.command.clone() {
            Command::RuntimeRefresh { runtime_id } => {
                let current = self.runtimes.get(&runtime_id).cloned().ok_or_else(|| {
                    DomainError::RuntimeNotFound {
                        runtime_id: runtime_id.as_str().to_string(),
                    }
                })?;
                if let Some(expected) = envelope.expected_revision
                    && expected != current.revision
                {
                    return Err(DomainError::RevisionConflict {
                        expected,
                        actual: current.revision,
                    });
                }

                let mut next = current;
                next.revision = Revision(self.revision + 1);
                next.refresh_count += 1;
                next.refresh_status = RefreshStatus::Pending;
                let event = DomainEvent::RuntimeRefreshRequested {
                    runtime_id: runtime_id.clone(),
                    revision: next.revision,
                    command_id: envelope.command_id.clone(),
                };
                let result = CommandResult {
                    command_id: envelope.command_id,
                    status: if envelope.dry_run {
                        CommandStatus::Planned
                    } else {
                        CommandStatus::Applied
                    },
                    runtime: next.clone(),
                    events: vec![event],
                };

                if !envelope.dry_run {
                    self.revision += 1;
                    self.runtimes.insert(runtime_id, next);
                    self.applied.insert(
                        idempotency_scope,
                        AppliedCommand {
                            command: envelope.command,
                            result: result.clone(),
                        },
                    );
                }
                Ok(result)
            }
        }
    }
}

impl fmt::Display for DomainError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidIdentifier { field } => write!(formatter, "invalid {field}"),
            Self::InvalidSchemaVersion { actual, expected } => {
                write!(
                    formatter,
                    "unsupported schema version {actual}, expected {expected}"
                )
            }
            Self::Unauthorized { capability } => {
                write!(formatter, "missing capability '{capability}'")
            }
            Self::RuntimeNotFound { runtime_id } => {
                write!(formatter, "runtime '{runtime_id}' was not found")
            }
            Self::RevisionConflict { expected, actual } => write!(
                formatter,
                "revision conflict: expected {}, actual {}",
                expected.0, actual.0
            ),
            Self::IdempotencyConflict { key } => {
                write!(formatter, "idempotency key '{key}' was reused")
            }
        }
    }
}

impl std::error::Error for DomainError {}

fn validated_identifier(field: &'static str, value: String) -> Result<String, DomainError> {
    let valid = !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b':' | b'.'));
    valid
        .then_some(value)
        .ok_or(DomainError::InvalidIdentifier { field })
}

fn validate_schema(actual: u32) -> Result<(), DomainError> {
    (actual == DOMAIN_SCHEMA_VERSION)
        .then_some(())
        .ok_or(DomainError::InvalidSchemaVersion {
            actual,
            expected: DOMAIN_SCHEMA_VERSION,
        })
}

fn validate_principal(principal: &Principal) -> Result<(), DomainError> {
    validated_identifier("principal.id", principal.id.clone()).map(|_| ())
}

fn require_capability(
    capabilities: &CapabilitySet,
    capability: &'static str,
) -> Result<(), DomainError> {
    capabilities
        .contains(capability)
        .then_some(())
        .ok_or(DomainError::Unauthorized { capability })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refresh_envelope(runtime_id: RuntimeId, command_id: &str, key: &str) -> CommandEnvelope {
        CommandEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            command_id: CommandId::new(command_id).unwrap(),
            idempotency_key: IdempotencyKey::new(key).unwrap(),
            expected_revision: Some(Revision(1)),
            principal: Principal {
                id: "operator".to_string(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REFRESH]),
            origin: CommandOrigin::Cli,
            confirmation: Confirmation::NotRequired,
            dry_run: false,
            command: Command::RuntimeRefresh { runtime_id },
        }
    }

    #[test]
    fn runtime_list_is_sorted_and_capability_gated() {
        let mut control = InMemoryControlPlane::default();
        control.register_runtime(RuntimeId::new("runtime-b").unwrap(), "B", "http://b");
        control.register_runtime(RuntimeId::new("runtime-a").unwrap(), "A", "http://a");

        let result = control
            .query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".to_string(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query: Query::RuntimeList,
            })
            .unwrap();
        let QueryResult::RuntimeList { runtimes, .. } = result;
        assert_eq!(runtimes[0].id.as_str(), "runtime-a");
        assert_eq!(runtimes[1].id.as_str(), "runtime-b");
    }

    #[test]
    fn runtime_refresh_is_idempotent_and_revision_checked() {
        let mut control = InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(runtime_id.clone(), "A", "http://a");
        let other_runtime_id = RuntimeId::new("runtime-b").unwrap();
        control.register_runtime(other_runtime_id.clone(), "B", "http://b");
        let command = refresh_envelope(runtime_id.clone(), "command-1", "refresh-a");
        let mut command = command;
        command.expected_revision = Some(Revision(1));

        let first = control.execute(command.clone()).unwrap();
        let replay = control.execute(command).unwrap();
        assert_eq!(first, replay);
        assert_eq!(first.runtime.refresh_count, 1);

        let mut conflicting = refresh_envelope(other_runtime_id, "command-2", "refresh-a");
        conflicting.expected_revision = Some(Revision(2));
        assert!(matches!(
            control.execute(conflicting),
            Err(DomainError::IdempotencyConflict { .. })
        ));
    }

    #[test]
    fn dry_run_does_not_consume_revision_or_idempotency_key() {
        let mut control = InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(runtime_id.clone(), "A", "http://a");
        let mut command = refresh_envelope(runtime_id, "command-1", "refresh-a");
        command.dry_run = true;

        let preview = control.execute(command.clone()).unwrap();
        assert_eq!(preview.status, CommandStatus::Planned);
        command.dry_run = false;
        let applied = control.execute(command).unwrap();
        assert_eq!(applied.status, CommandStatus::Applied);
        assert_eq!(applied.runtime.revision, Revision(2));
    }

    #[test]
    fn runtime_refresh_rejects_missing_capability_and_stale_revision() {
        let mut control = InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(runtime_id.clone(), "A", "http://a");

        let mut unauthorized = refresh_envelope(runtime_id.clone(), "command-1", "refresh-a");
        unauthorized.capabilities = CapabilitySet::default();
        assert!(matches!(
            control.execute(unauthorized),
            Err(DomainError::Unauthorized {
                capability: CAPABILITY_RUNTIME_REFRESH
            })
        ));

        let mut stale = refresh_envelope(runtime_id, "command-2", "refresh-b");
        stale.expected_revision = Some(Revision(99));
        assert!(matches!(
            control.execute(stale),
            Err(DomainError::RevisionConflict {
                expected: Revision(99),
                actual: Revision(1)
            })
        ));
    }

    #[test]
    fn idempotency_keys_are_scoped_to_the_principal() {
        let mut control = InMemoryControlPlane::default();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        control.register_runtime(runtime_id.clone(), "A", "http://a");

        let first = refresh_envelope(runtime_id.clone(), "command-1", "shared-key");
        control.execute(first).unwrap();

        let mut second = refresh_envelope(runtime_id, "command-2", "shared-key");
        second.principal.id = "another-operator".to_string();
        second.expected_revision = Some(Revision(2));
        let result = control.execute(second).unwrap();
        assert_eq!(result.runtime.revision, Revision(3));
        assert_eq!(result.runtime.refresh_count, 2);
    }
}
