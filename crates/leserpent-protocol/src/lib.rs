use leserpent_domain::{
    CommandEnvelope, CommandResult, DOMAIN_SCHEMA_VERSION, DomainError, QueryEnvelope, QueryResult,
};
use serde::{Deserialize, Serialize};

pub const PROTOCOL_SCHEMA_VERSION: u32 = 1;
pub const MAX_PROTOCOL_MESSAGE_BYTES: usize = 1024 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum ProtocolRequest {
    Command(CommandEnvelope),
    Query(QueryEnvelope),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RequestEnvelope {
    pub schema_version: u32,
    pub request: ProtocolRequest,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(tag = "kind", content = "payload", rename_all = "snake_case")]
pub enum ProtocolResponse {
    Command(CommandResult),
    Query(QueryResult),
    Error(ProtocolError),
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ResponseEnvelope {
    pub schema_version: u32,
    pub response: ProtocolResponse,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ProtocolError {
    pub code: String,
    pub message: String,
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

pub fn domain_error_response(error: &DomainError) -> ResponseEnvelope {
    let code = match error {
        DomainError::InvalidIdentifier { .. } => "invalid_identifier",
        DomainError::InvalidSchemaVersion { .. } => "invalid_domain_schema_version",
        DomainError::Unauthorized { .. } => "unauthorized",
        DomainError::RuntimeNotFound { .. } => "runtime_not_found",
        DomainError::RevisionConflict { .. } => "revision_conflict",
        DomainError::IdempotencyConflict { .. } => "idempotency_conflict",
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
    use leserpent_domain::{
        CAPABILITY_RUNTIME_READ, CapabilitySet, Principal, Query, QueryEnvelope,
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
                query: Query::RuntimeList,
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

        let source = br#"{"schema_version":2,"request":{"kind":"query","payload":{"schema_version":1,"principal":{"id":"operator"},"capabilities":["runtime.read"],"query":{"kind":"runtime_list"}}}}"#;
        assert!(matches!(
            decode_request(source),
            Err(DecodeError::InvalidSchemaVersion { .. })
        ));
    }
}
