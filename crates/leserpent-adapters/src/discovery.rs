use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use leserpent_domain::{
    RUNTIME_CAPABILITY_DISCOVERY_EFFECT_KIND, Revision, RuntimeCapabilityObservation,
    RuntimeCapabilityRefreshRequest, RuntimeCapabilitySnapshot,
};
use leserpent_runtime::EffectExecution;
use serde::Deserialize;
use serde_json::Value;

use crate::gewyvern::{GewyvernTarget, get_json, normalize_targets};
use crate::{EffectAdapter, EmptySecretStore, SecretStore, validate_id};

pub const GEWYVERN_DISCOVERY_EFFECT_KIND: &str = RUNTIME_CAPABILITY_DISCOVERY_EFFECT_KIND;
const MAX_CAPABILITY_ENDPOINTS: usize = 128;
const MAX_CAPABILITY_EXTENSIONS: usize = 64;

pub type GewyvernDiscoveryRequest = RuntimeCapabilityRefreshRequest;
pub type GewyvernCapabilityObservation = RuntimeCapabilityObservation;

#[derive(Deserialize)]
struct CapabilityDocument {
    service: String,
    version: String,
    latest_snapshot: bool,
    authenticated_deployment: bool,
    serve_required: bool,
    external_sidecar_context: bool,
    target_path_segment_encoding: String,
    target_direct_path_chars: String,
    endpoints: Vec<String>,
    #[serde(flatten)]
    extensions: BTreeMap<String, Value>,
}

pub struct GewyvernDiscoveryAdapter {
    targets: BTreeMap<String, GewyvernTarget>,
    secrets: Arc<dyn SecretStore>,
    timeout: Duration,
}

impl GewyvernDiscoveryAdapter {
    pub fn new(
        targets: impl IntoIterator<Item = (String, GewyvernTarget)>,
    ) -> Result<Self, String> {
        Self::with_secret_store(targets, Arc::new(EmptySecretStore))
    }

    pub fn with_secret_store(
        targets: impl IntoIterator<Item = (String, GewyvernTarget)>,
        secrets: Arc<dyn SecretStore>,
    ) -> Result<Self, String> {
        Ok(Self {
            targets: normalize_targets(targets)?,
            secrets,
            timeout: Duration::from_secs(3),
        })
    }
}

impl EffectAdapter for GewyvernDiscoveryAdapter {
    fn kind(&self) -> &str {
        GEWYVERN_DISCOVERY_EFFECT_KIND
    }

    fn execute(&mut self, payload: &[u8]) -> EffectExecution {
        let request: GewyvernDiscoveryRequest = match serde_json::from_slice(payload) {
            Ok(request) => request,
            Err(_) => return reject("invalid Gewyvern discovery payload"),
        };
        if validate_id("runtime_id", &request.runtime_id).is_err() {
            return reject("invalid Gewyvern discovery runtime_id");
        }
        let Some(target) = self.targets.get(&request.runtime_id) else {
            return reject("Gewyvern discovery runtime is not configured");
        };
        let response = match get_json(
            target,
            self.secrets.as_ref(),
            "/v1/capabilities",
            self.timeout,
        ) {
            Ok(response) => response,
            Err(error) => {
                return EffectExecution::Retry {
                    error,
                    after: Duration::from_secs(1),
                };
            }
        };
        if response.status != 200 {
            return match response.status {
                500..=599 => EffectExecution::Retry {
                    error: "Gewyvern capability service is unavailable".into(),
                    after: Duration::from_secs(1),
                },
                _ => reject("Gewyvern capability request was rejected"),
            };
        }
        let observation = match normalize_document(
            &request.runtime_id,
            request.expected_revision,
            &response.body,
        ) {
            Ok(observation) => observation,
            Err(error) => return reject(error),
        };
        match serde_json::to_vec(&observation) {
            Ok(body) => EffectExecution::Complete(body),
            Err(_) => reject("Gewyvern capability observation cannot be encoded"),
        }
    }
}

fn normalize_document(
    runtime_id: &str,
    expected_revision: Revision,
    body: &[u8],
) -> Result<GewyvernCapabilityObservation, &'static str> {
    let document: CapabilityDocument =
        serde_json::from_slice(body).map_err(|_| "Gewyvern capability response JSON is invalid")?;
    if document.service != "gewyvern-api"
        || !valid_version(&document.version)
        || document.target_path_segment_encoding != "percent-encoding"
        || document.target_direct_path_chars.is_empty()
        || document.target_direct_path_chars.len() > 64
        || !document
            .target_direct_path_chars
            .bytes()
            .all(|byte| byte.is_ascii_graphic() || byte == b' ')
        || document.endpoints.is_empty()
        || document.endpoints.len() > MAX_CAPABILITY_ENDPOINTS
        || document.extensions.len() > MAX_CAPABILITY_EXTENSIONS
    {
        return Err("Gewyvern capability response is invalid");
    }
    let endpoints = document
        .endpoints
        .into_iter()
        .map(|endpoint| validate_endpoint(endpoint).ok_or(()))
        .collect::<Result<BTreeSet<_>, _>>()
        .map_err(|_| "Gewyvern capability endpoint is invalid")?;
    if !endpoints.contains("/v1/capabilities")
        || document.authenticated_deployment != endpoints.contains("/v1/deployments")
    {
        return Err("Gewyvern capability claims are inconsistent");
    }
    let extensions = document
        .extensions
        .into_iter()
        .map(|(key, value)| {
            if !valid_extension_key(&key) {
                return Err("Gewyvern capability extension name is invalid");
            }
            let value = value
                .as_bool()
                .ok_or("Gewyvern capability extension is not boolean")?;
            Ok((key, value))
        })
        .collect::<Result<BTreeMap<_, _>, _>>()?;
    Ok(GewyvernCapabilityObservation {
        runtime_id: runtime_id.to_string(),
        expected_revision,
        capabilities: RuntimeCapabilitySnapshot {
            source: "gewyvern-api".into(),
            service: document.service,
            version: document.version,
            latest_snapshot: document.latest_snapshot,
            authenticated_deployment: document.authenticated_deployment,
            serve_required: document.serve_required,
            external_sidecar_context: document.external_sidecar_context,
            target_path_segment_encoding: document.target_path_segment_encoding,
            target_direct_path_chars: document.target_direct_path_chars,
            endpoints: endpoints.into_iter().collect(),
            extensions,
        },
    })
}

fn valid_version(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'+'))
}

fn validate_endpoint(value: String) -> Option<String> {
    (value.len() <= 256
        && value.starts_with('/')
        && !value.starts_with("//")
        && !value.contains(['?', '#'])
        && value.bytes().all(|byte| byte.is_ascii_graphic()))
    .then_some(value)
}

fn valid_extension_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'_' | b'-')
        })
}

fn reject(error: &str) -> EffectExecution {
    EffectExecution::Reject {
        error: error.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    fn capability_body() -> &'static [u8] {
        br#"{"service":"gewyvern-api","version":"1.2.0","latest_snapshot":true,"authenticated_deployment":true,"serve_required":true,"external_sidecar_context":true,"target_path_segment_encoding":"percent-encoding","target_direct_path_chars":"A-Z a-z 0-9 . _ ~ :","endpoints":["/v1/deployments","/v1/capabilities","/v1/capabilities"],"protocol_catalog":true}"#
    }

    #[test]
    fn discovery_queries_only_the_configured_capability_endpoint() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 2048];
            let read = stream.read(&mut request).unwrap();
            let request = std::str::from_utf8(&request[..read]).unwrap();
            assert!(request.starts_with("GET /v1/capabilities HTTP/1.1\r\n"));
            let body = capability_body();
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
                body.len()
            )
            .unwrap();
            stream.write_all(body).unwrap();
        });
        let target = GewyvernTarget::loopback(address, None).unwrap();
        let mut adapter =
            GewyvernDiscoveryAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        let EffectExecution::Complete(body) =
            adapter.execute(br#"{"runtime_id":"runtime-a","expected_revision":1}"#)
        else {
            panic!("discovery should complete");
        };
        let observation: GewyvernCapabilityObservation = serde_json::from_slice(&body).unwrap();
        assert_eq!(
            observation.capabilities.endpoints,
            vec!["/v1/capabilities", "/v1/deployments"]
        );
        assert_eq!(
            observation.capabilities.extensions.get("protocol_catalog"),
            Some(&true)
        );
        server.join().unwrap();
    }

    #[test]
    fn discovery_rejects_ambient_targets_and_inconsistent_claims_before_use() {
        let target = GewyvernTarget::loopback("127.0.0.1:9".parse().unwrap(), None).unwrap();
        let mut adapter =
            GewyvernDiscoveryAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        assert!(matches!(
            adapter.execute(br#"{"runtime_id":"runtime-missing","expected_revision":1}"#),
            EffectExecution::Reject { .. }
        ));
        assert!(matches!(
            adapter.execute(
                br#"{"runtime_id":"runtime-a","expected_revision":1,"subnet":"192.168.1.0/24"}"#
            ),
            EffectExecution::Reject { .. }
        ));

        let inconsistent = br#"{"service":"gewyvern-api","version":"1.2.0","latest_snapshot":true,"authenticated_deployment":true,"serve_required":true,"external_sidecar_context":true,"target_path_segment_encoding":"percent-encoding","target_direct_path_chars":"A-Z a-z 0-9 . _ ~ :","endpoints":["/v1/capabilities"]}"#;
        assert!(normalize_document("runtime-a", Revision(1), inconsistent).is_err());
    }

    #[test]
    fn discovery_extensions_are_boolean_and_bounded() {
        let non_boolean = br#"{"service":"gewyvern-api","version":"1.2.0","latest_snapshot":true,"authenticated_deployment":false,"serve_required":true,"external_sidecar_context":true,"target_path_segment_encoding":"percent-encoding","target_direct_path_chars":"A-Z a-z 0-9 . _ ~ :","endpoints":["/v1/capabilities"],"future":"unsafe"}"#;
        assert!(normalize_document("runtime-a", Revision(1), non_boolean).is_err());
        let invalid_endpoint = br#"{"service":"gewyvern-api","version":"1.2.0","latest_snapshot":true,"authenticated_deployment":false,"serve_required":true,"external_sidecar_context":true,"target_path_segment_encoding":"percent-encoding","target_direct_path_chars":"A-Z a-z 0-9 . _ ~ :","endpoints":["/v1/capabilities?scan=all"]}"#;
        assert!(normalize_document("runtime-a", Revision(1), invalid_endpoint).is_err());
    }
}
