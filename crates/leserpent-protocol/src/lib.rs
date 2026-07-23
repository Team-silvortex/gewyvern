use leserpent_domain::bootstrap::{BootstrapId, DeploymentBootstrapSnapshot};
use leserpent_domain::{
    CapabilitySet, CommandEnvelope, CommandId, CommandResult, DOMAIN_SCHEMA_VERSION, DomainError,
    Principal, QueryEnvelope, QueryResult, RefreshStatus, Revision, RuntimeCapabilitySnapshot,
    RuntimeDeploymentOutcome, RuntimeId, RuntimeProjection, RuntimeStatusSnapshot, RuntimeTags,
};
use serde::{Deserialize, Serialize};

pub mod bootstrap;
pub mod bootstrap_installer;
pub mod compatibility_v1;
pub mod gewyvern_installer;
pub mod gewyvern_retirement;
pub mod provisioning;
pub mod retirement;
pub mod transport_safety;

pub const PROTOCOL_SCHEMA_VERSION: u32 = 1;
pub const EVENT_SCHEMA_VERSION: u32 = 1;
pub const MAX_PROTOCOL_MESSAGE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
// Preserve the v1 Rust and wire request shape until the v2 schema seal.
#[allow(clippy::large_enum_variant)]
pub enum ProtocolRequest {
    Command(CommandEnvelope),
    Query(QueryEnvelope),
    Health(HealthRequest),
    DeploymentReceipt(DeploymentReceiptRequest),
    OrchestraPersist(OrchestraPersistenceRequest),
    OrchestraHistory(OrchestraHistoryRequest),
    OrchestraDelete(OrchestraDeleteRequest),
    BootstrapHandoff(BootstrapHandoffRequest),
    BootstrapSessionBind(BootstrapSessionBindRequest),
}

#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HealthRequest {}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentReceiptRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub command_id: CommandId,
    pub request_id: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraPersistenceRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub envelope: compatibility_v1::LegacyOrchestraPersistenceEnvelope,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraHistoryRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub runtime_id: Option<String>,
    pub run_id: Option<String>,
    pub offset: u32,
    pub limit: u16,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraDeleteRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub runtime_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapHandoffRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub bootstrap_id: BootstrapId,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BootstrapSessionBindRequest {
    pub principal: Principal,
    pub capabilities: CapabilitySet,
    pub bootstrap_id: BootstrapId,
    pub confirmed: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RequestEnvelope {
    pub schema_version: u32,
    pub request: ProtocolRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum ProtocolResponse {
    Command(Box<CommandResult>),
    Query(QueryResult),
    Health(HealthResponse),
    DeploymentReceipt(DeploymentReceiptResponse),
    OrchestraPersisted(OrchestraPersistenceResponse),
    OrchestraHistory(OrchestraHistoryResponse),
    OrchestraDeleted(OrchestraDeleteResponse),
    BootstrapHandoff(DeploymentBootstrapSnapshot),
    Error(ProtocolError),
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DeploymentReceiptStatus {
    Pending,
    Completed,
    Failed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct DeploymentReceiptResponse {
    pub command_id: CommandId,
    pub request_id: String,
    pub status: DeploymentReceiptStatus,
    pub attempt: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome: Option<RuntimeDeploymentOutcome>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraPersistenceResponse {
    pub envelope: compatibility_v1::LegacyOrchestraPersistenceEnvelope,
    pub event_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraHistoryResponse {
    pub runs: Vec<compatibility_v1::LegacyOrchestraRun>,
    pub events: Vec<compatibility_v1::LegacyOrchestraEvent>,
    pub next_offset: Option<u32>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OrchestraDeleteResponse {
    pub deleted_runtime_count: u32,
    pub deleted_run_count: u64,
    pub deleted_event_count: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct HealthResponse {
    pub status: String,
    pub authority_owned: bool,
    pub protocol_schema_version: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effect_queue: Option<EffectQueueHealth>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectQueueHealth {
    pub ready: u64,
    pub leased: u64,
    pub completed: u64,
    pub failed: u64,
    pub active: u64,
    pub terminal: u64,
    pub capacity: u64,
    pub saturated: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ResponseEnvelope {
    pub schema_version: u32,
    pub response: ProtocolResponse,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ProtocolError {
    pub code: String,
    pub message: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventEnvelope {
    pub schema_version: u32,
    pub event: ProtocolEvent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum ProtocolEvent {
    RuntimeSnapshot {
        revision: Revision,
        resumed_after: Option<Revision>,
        runtimes: Vec<RemoteRuntimeProjection>,
    },
    Heartbeat {
        revision: Revision,
    },
    ResyncRequired {
        requested_after: Revision,
        current_revision: Revision,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RemoteRuntimeProjection {
    pub id: RuntimeId,
    pub name: String,
    pub revision: Revision,
    pub refresh_count: u64,
    pub refresh_status: RefreshStatus,
    pub tags: RuntimeTags,
    pub status: RuntimeStatusSnapshot,
    #[serde(
        default,
        skip_serializing_if = "RuntimeCapabilitySnapshot::is_unobserved"
    )]
    pub capabilities: RuntimeCapabilitySnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capabilities_observed_for_revision: Option<Revision>,
}

impl From<RuntimeProjection> for RemoteRuntimeProjection {
    fn from(runtime: RuntimeProjection) -> Self {
        Self {
            id: runtime.id,
            name: runtime.name,
            revision: runtime.revision,
            refresh_count: runtime.refresh_count,
            refresh_status: runtime.refresh_status,
            tags: runtime.tags,
            status: runtime.status,
            capabilities: runtime.capabilities,
            capabilities_observed_for_revision: runtime.capabilities_observed_for_revision,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DecodeError {
    Oversized { size: usize, limit: usize },
    InvalidJson(String),
    InvalidSchemaVersion { actual: u32, expected: u32 },
    InvalidDomainSchemaVersion { actual: u32, expected: u32 },
}

pub fn decode_request(bytes: &[u8]) -> Result<RequestEnvelope, DecodeError> {
    if bytes.len() > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err(DecodeError::Oversized {
            size: bytes.len(),
            limit: MAX_PROTOCOL_MESSAGE_BYTES,
        });
    }
    let envelope: RequestEnvelope =
        serde_json::from_slice(bytes).map_err(|err| DecodeError::InvalidJson(err.to_string()))?;
    if envelope.schema_version != PROTOCOL_SCHEMA_VERSION {
        return Err(DecodeError::InvalidSchemaVersion {
            actual: envelope.schema_version,
            expected: PROTOCOL_SCHEMA_VERSION,
        });
    }
    let domain_version = match &envelope.request {
        ProtocolRequest::Command(command) => command.schema_version,
        ProtocolRequest::Query(query) => query.schema_version,
        ProtocolRequest::Health(_)
        | ProtocolRequest::DeploymentReceipt(_)
        | ProtocolRequest::OrchestraPersist(_)
        | ProtocolRequest::OrchestraHistory(_)
        | ProtocolRequest::OrchestraDelete(_)
        | ProtocolRequest::BootstrapHandoff(_)
        | ProtocolRequest::BootstrapSessionBind(_) => DOMAIN_SCHEMA_VERSION,
    };
    if domain_version != DOMAIN_SCHEMA_VERSION {
        return Err(DecodeError::InvalidDomainSchemaVersion {
            actual: domain_version,
            expected: DOMAIN_SCHEMA_VERSION,
        });
    }
    Ok(envelope)
}

pub fn encode_request(envelope: &RequestEnvelope) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(envelope)
}

pub fn encode_response(envelope: &ResponseEnvelope) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(envelope)
}

pub fn decode_response(bytes: &[u8]) -> Result<ResponseEnvelope, DecodeError> {
    if bytes.len() > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err(DecodeError::Oversized {
            size: bytes.len(),
            limit: MAX_PROTOCOL_MESSAGE_BYTES,
        });
    }
    let envelope: ResponseEnvelope =
        serde_json::from_slice(bytes).map_err(|err| DecodeError::InvalidJson(err.to_string()))?;
    if envelope.schema_version != PROTOCOL_SCHEMA_VERSION {
        return Err(DecodeError::InvalidSchemaVersion {
            actual: envelope.schema_version,
            expected: PROTOCOL_SCHEMA_VERSION,
        });
    }
    Ok(envelope)
}

pub fn encode_event(envelope: &EventEnvelope) -> Result<Vec<u8>, serde_json::Error> {
    serde_json::to_vec(envelope)
}

pub fn decode_event(bytes: &[u8]) -> Result<EventEnvelope, DecodeError> {
    if bytes.len() > MAX_PROTOCOL_MESSAGE_BYTES {
        return Err(DecodeError::Oversized {
            size: bytes.len(),
            limit: MAX_PROTOCOL_MESSAGE_BYTES,
        });
    }
    let envelope: EventEnvelope =
        serde_json::from_slice(bytes).map_err(|err| DecodeError::InvalidJson(err.to_string()))?;
    if envelope.schema_version != EVENT_SCHEMA_VERSION {
        return Err(DecodeError::InvalidSchemaVersion {
            actual: envelope.schema_version,
            expected: EVENT_SCHEMA_VERSION,
        });
    }
    Ok(envelope)
}

pub fn domain_error_response(error: &DomainError) -> ResponseEnvelope {
    let code = match error {
        DomainError::InvalidIdentifier { .. } => "invalid_identifier",
        DomainError::InvalidSchemaVersion { .. } => "invalid_domain_schema_version",
        DomainError::Unauthorized { .. } => "unauthorized",
        DomainError::RuntimeNotFound { .. } => "runtime_not_found",
        DomainError::RuntimeAlreadyExists { .. } => "runtime_already_exists",
        DomainError::RuntimeEndpointConflict { .. } => "runtime_endpoint_conflict",
        DomainError::RevisionConflict { .. } => "revision_conflict",
        DomainError::IdempotencyConflict { .. } => "idempotency_conflict",
        DomainError::ConfirmationRequired => "confirmation_required",
        DomainError::InvalidQuery { .. } => "invalid_query",
    };
    ResponseEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        response: ProtocolResponse::Error(ProtocolError {
            code: code.to_string(),
            message: error.to_string(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use leserpent_domain::bootstrap::{
        BootstrapId, BootstrapPhase, BootstrapTarget, BootstrapTransport, CAPABILITY_HOST_BOOTSTRAP,
    };
    use leserpent_domain::{
        CAPABILITY_ORCHESTRA_WRITE, CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ,
        CAPABILITY_RUNTIME_REGISTER, CapabilitySet, Command, CommandEnvelope, CommandId,
        CommandOrigin, Confirmation, IdempotencyKey, Principal, Query, QueryEnvelope, RuntimeId,
        RuntimeListFilter, RuntimeTags,
    };

    use super::*;

    #[test]
    fn runtime_list_request_round_trips_with_explicit_versions() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".to_string(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query: Query::RuntimeList {
                    filter: RuntimeListFilter::default(),
                },
            }),
        };
        let bytes = encode_request(&request).unwrap();
        assert_eq!(decode_request(&bytes).unwrap(), request);
    }

    #[test]
    fn runtime_registration_request_round_trips_as_a_strict_typed_command() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Command(CommandEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                command_id: CommandId::new("register-command").unwrap(),
                idempotency_key: IdempotencyKey::new("register-request").unwrap(),
                expected_revision: None,
                principal: Principal {
                    id: "web-bridge".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
                origin: CommandOrigin::CompatibilityAdapter,
                confirmation: Confirmation::Confirmed,
                dry_run: false,
                command: Command::RuntimeRegister {
                    runtime_id: RuntimeId::new("runtime-new").unwrap(),
                    name: "Runtime New".into(),
                    endpoint: "https://127.0.0.1:9443".into(),
                    tags: RuntimeTags::default(),
                },
            }),
        };
        let bytes = encode_request(&request).unwrap();
        assert_eq!(decode_request(&bytes).unwrap(), request);

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["request"]["payload"]["command"]["pairing_token"] =
            serde_json::Value::String("must-not-cross-the-domain-boundary".into());
        assert!(matches!(
            decode_request(&serde_json::to_vec(&value).unwrap()),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn runtime_registration_update_round_trips_with_an_explicit_revision_fence() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Command(CommandEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                command_id: CommandId::new("registration-update-command").unwrap(),
                idempotency_key: IdempotencyKey::new("registration-update-request").unwrap(),
                expected_revision: Some(Revision(7)),
                principal: Principal {
                    id: "web-bridge".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
                origin: CommandOrigin::CompatibilityAdapter,
                confirmation: Confirmation::Confirmed,
                dry_run: false,
                command: Command::RuntimeRegistrationUpdate {
                    runtime_id: RuntimeId::new("runtime-existing").unwrap(),
                    name: "Runtime Updated".into(),
                    endpoint: "https://127.0.0.1:9553".into(),
                    tags: RuntimeTags::default(),
                },
            }),
        };
        let bytes = encode_request(&request).unwrap();
        assert_eq!(decode_request(&bytes).unwrap(), request);

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["request"]["payload"]["command"]["admin_token"] =
            serde_json::Value::String("must-not-cross-the-domain-boundary".into());
        assert!(matches!(
            decode_request(&serde_json::to_vec(&value).unwrap()),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn runtime_discovery_intake_round_trips_without_secret_or_raw_payload_fields() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Command(CommandEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                command_id: CommandId::new("discovery-intake-command").unwrap(),
                idempotency_key: IdempotencyKey::new("discovery-intake-request").unwrap(),
                expected_revision: Some(Revision(2)),
                principal: Principal {
                    id: "web-bridge".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_REGISTER]),
                origin: CommandOrigin::CompatibilityAdapter,
                confirmation: Confirmation::Confirmed,
                dry_run: false,
                command: Command::RuntimeDiscoveryIntake {
                    runtime_id: RuntimeId::new("runtime-existing").unwrap(),
                    capabilities: Some(Box::new(RuntimeCapabilitySnapshot {
                        source: "gewyvern-api".into(),
                        service: "gewyvern-api".into(),
                        version: "1.2.0".into(),
                        latest_snapshot: true,
                        authenticated_deployment: true,
                        serve_required: true,
                        external_sidecar_context: true,
                        target_path_segment_encoding: "percent-encoding".into(),
                        target_direct_path_chars: "A-Z a-z 0-9 . _ ~ :".into(),
                        endpoints: vec!["/v1/capabilities".into(), "/v1/deployments".into()],
                        extensions: BTreeMap::from([("protocol_catalog".into(), true)]),
                    })),
                    status: None,
                },
            }),
        };
        let bytes = encode_request(&request).unwrap();
        assert_eq!(decode_request(&bytes).unwrap(), request);

        let mut value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        value["request"]["payload"]["command"]["pairing_token"] =
            serde_json::Value::String("must-not-cross-the-domain-boundary".into());
        assert!(matches!(
            decode_request(&serde_json::to_vec(&value).unwrap()),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn runtime_logs_request_round_trips_with_bounded_cursor() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".to_string(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query: Query::RuntimeLogs {
                    runtime_id: leserpent_domain::RuntimeId::new("runtime-a").unwrap(),
                    after_sequence: Some(41),
                    limit: 128,
                },
            }),
        };
        let bytes = encode_request(&request).unwrap();
        assert_eq!(decode_request(&bytes).unwrap(), request);
    }

    #[test]
    fn canonical_runtime_list_fixtures_decode() {
        let request = decode_request(include_bytes!(
            "../tests/fixtures/runtime-list-request-v1.json"
        ))
        .expect("request fixture should decode");
        assert!(matches!(request.request, ProtocolRequest::Query(_)));

        let response = decode_response(include_bytes!(
            "../tests/fixtures/runtime-list-response-v1.json"
        ))
        .expect("response fixture should decode");
        assert!(matches!(response.response, ProtocolResponse::Query(_)));
    }

    #[test]
    fn decoder_rejects_oversized_and_unknown_version_messages() {
        let oversized = vec![b' '; MAX_PROTOCOL_MESSAGE_BYTES + 1];
        assert!(matches!(
            decode_request(&oversized),
            Err(DecodeError::Oversized { .. })
        ));

        let source = br#"{"schema_version":2,"request":{"kind":"query","payload":{"schema_version":1,"principal":{"id":"operator"},"capabilities":["runtime.read"],"query":{"kind":"runtime_list","filter":{"environment":null,"cluster":null,"role":null}}}}}"#;
        assert!(matches!(
            decode_request(source),
            Err(DecodeError::InvalidSchemaVersion { .. })
        ));
    }

    #[test]
    fn endpoint_conflict_uses_a_typed_non_disclosing_wire_error() {
        let response = domain_error_response(&DomainError::RuntimeEndpointConflict {
            runtime_id: "runtime-owner".into(),
        });
        assert!(matches!(
            response.response,
            ProtocolResponse::Error(ProtocolError { code, message })
                if code == "runtime_endpoint_conflict"
                    && message.contains("runtime-owner")
                    && !message.contains("https://")
        ));
    }

    #[test]
    fn decoders_reject_unknown_versioned_envelope_fields() {
        let request =
            br#"{"schema_version":1,"request":{"kind":"health","payload":{}},"schemaVersion":1}"#;
        assert!(matches!(
            decode_request(request),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));

        let response = br#"{"schema_version":1,"response":{"kind":"health","payload":{"status":"ready","authority_owned":true,"protocol_schema_version":1}},"request_id":"ignored"}"#;
        assert!(matches!(
            decode_response(response),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));

        let event = br#"{"schema_version":1,"event":{"kind":"heartbeat","payload":{"revision":1}},"cursor":1}"#;
        assert!(matches!(
            decode_event(event),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn health_payload_rejects_unknown_fields() {
        let request =
            br#"{"schema_version":1,"request":{"kind":"health","payload":{"admin":true}}}"#;
        assert!(matches!(
            decode_request(request),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));

        let response = br#"{"schema_version":1,"response":{"kind":"health","payload":{"status":"ready","authority_owned":true,"protocol_schema_version":1,"effect_queue":null,"healthy":true}}}"#;
        assert!(matches!(
            decode_response(response),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));
    }

    #[test]
    fn health_request_round_trips_without_domain_payload() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Health(HealthRequest {}),
        };
        assert_eq!(
            decode_request(&encode_request(&request).unwrap()).unwrap(),
            request
        );
    }

    #[test]
    fn deployment_receipt_round_trips_as_a_typed_terminal_response() {
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::DeploymentReceipt(DeploymentReceiptRequest {
                principal: Principal {
                    id: "operator.example".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_DEPLOY]),
                command_id: CommandId::new("deploy-command").unwrap(),
                request_id: "deploy-1".into(),
            }),
        };
        assert_eq!(
            decode_request(&encode_request(&request).unwrap()).unwrap(),
            request
        );

        let response = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::DeploymentReceipt(DeploymentReceiptResponse {
                command_id: CommandId::new("deploy-command").unwrap(),
                request_id: "deploy-1".into(),
                status: DeploymentReceiptStatus::Completed,
                attempt: 1,
                outcome: Some(RuntimeDeploymentOutcome {
                    deployment_id: "gdep-1".into(),
                    request_id: "deploy-1".into(),
                    pipeline_kind: "http/request".into(),
                    requested_by: "operator.example".into(),
                    status: "accepted".into(),
                    accepted_unix_ms: 1_700_000_000_000,
                    target: None,
                    replayed: false,
                }),
                error: None,
            }),
        };
        assert_eq!(
            decode_response(&encode_response(&response).unwrap()).unwrap(),
            response
        );

        let history = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraHistory(OrchestraHistoryRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                runtime_id: Some("runtime-a".into()),
                run_id: Some("orun-1".into()),
                offset: 0,
                limit: 64,
            }),
        };
        assert_eq!(
            decode_request(&encode_request(&history).unwrap()).unwrap(),
            history
        );
        let delete = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraDelete(OrchestraDeleteRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                runtime_ids: vec!["runtime-a".into()],
            }),
        };
        assert_eq!(
            decode_request(&encode_request(&delete).unwrap()).unwrap(),
            delete
        );
    }

    #[test]
    fn bootstrap_handoff_wire_accepts_only_an_id_and_public_state() {
        let bootstrap_id = BootstrapId::new("bootstrap-wire-1").unwrap();
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::BootstrapSessionBind(BootstrapSessionBindRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_HOST_BOOTSTRAP]),
                bootstrap_id: bootstrap_id.clone(),
                confirmed: true,
            }),
        };
        let encoded = encode_request(&request).unwrap();
        assert_eq!(decode_request(&encoded).unwrap(), request);
        let mut forged: serde_json::Value = serde_json::from_slice(&encoded).unwrap();
        forged["request"]["payload"]["authority_owned"] = serde_json::Value::Bool(true);
        assert!(matches!(
            decode_request(&serde_json::to_vec(&forged).unwrap()),
            Err(DecodeError::InvalidJson(message)) if message.contains("unknown field")
        ));

        let response = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::BootstrapHandoff(DeploymentBootstrapSnapshot {
                bootstrap_id,
                phase: BootstrapPhase::Failed,
                target: BootstrapTarget {
                    transport: BootstrapTransport::Ssh,
                    host: "host.example".into(),
                    port: 22,
                },
                bootstrap_credential_present: false,
                daemon_id: None,
                endpoint: None,
                session_credential_handle: None,
                trust_credential_handle: None,
                fault_code: Some("transport_failure".into()),
                mutation_authorized: false,
            }),
        };
        assert_eq!(
            decode_response(&encode_response(&response).unwrap()).unwrap(),
            response
        );
    }

    #[test]
    fn orchestra_persistence_round_trips_the_atomic_legacy_envelope() {
        let envelope = compatibility_v1::decode_orchestra_persistence(include_bytes!(
            "../tests/fixtures/legacy-orchestra-persistence-v1.json"
        ))
        .unwrap();
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraPersist(OrchestraPersistenceRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                envelope: envelope.clone(),
            }),
        };
        assert_eq!(
            decode_request(&encode_request(&request).unwrap()).unwrap(),
            request
        );
        let response = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::OrchestraPersisted(OrchestraPersistenceResponse {
                envelope,
                event_count: 1,
            }),
        };
        assert_eq!(
            decode_response(&encode_response(&response).unwrap()).unwrap(),
            response
        );
    }

    #[test]
    fn health_response_accepts_legacy_payload_and_round_trips_queue_pressure() {
        let legacy = br#"{"schema_version":1,"response":{"kind":"health","payload":{"status":"ready","authority_owned":true,"protocol_schema_version":1}}}"#;
        let decoded = decode_response(legacy).unwrap();
        let ProtocolResponse::Health(health) = decoded.response else {
            panic!("legacy response should remain health");
        };
        assert_eq!(health.effect_queue, None);

        let response = ResponseEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            response: ProtocolResponse::Health(HealthResponse {
                status: "ready".into(),
                authority_owned: true,
                protocol_schema_version: PROTOCOL_SCHEMA_VERSION,
                effect_queue: Some(EffectQueueHealth {
                    ready: 3,
                    leased: 1,
                    completed: 5,
                    failed: 2,
                    active: 4,
                    terminal: 7,
                    capacity: 10_000,
                    saturated: false,
                }),
            }),
        };
        assert_eq!(
            decode_response(&encode_response(&response).unwrap()).unwrap(),
            response
        );
    }

    #[test]
    fn event_snapshot_round_trips_without_runtime_endpoint_disclosure() {
        let mut control = leserpent_domain::InMemoryControlPlane::default();
        let runtime = control.register_runtime(
            leserpent_domain::RuntimeId::new("runtime-a").unwrap(),
            "Runtime A",
            "https://secret-endpoint.invalid",
        );
        let event = EventEnvelope {
            schema_version: EVENT_SCHEMA_VERSION,
            event: ProtocolEvent::RuntimeSnapshot {
                revision: Revision(1),
                resumed_after: Some(Revision(0)),
                runtimes: vec![RemoteRuntimeProjection::from(runtime)],
            },
        };
        let encoded = encode_event(&event).unwrap();
        assert_eq!(decode_event(&encoded).unwrap(), event);
        let encoded = String::from_utf8(encoded).unwrap();
        assert!(!encoded.contains("secret-endpoint"));
        assert!(!encoded.contains("endpoint"));
    }
}
