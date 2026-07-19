use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use leserpent_domain::validate_deployment_intent;
pub use leserpent_domain::{
    RUNTIME_DEPLOYMENT_EFFECT_KIND as GEWYVERN_DEPLOYMENT_EFFECT_KIND,
    RuntimeDeploymentOutcome as GewyvernDeploymentResponse,
    RuntimeDeploymentRequest as GewyvernDeploymentRequest,
};
use leserpent_runtime::EffectExecution;
use serde::Serialize;

use crate::gewyvern::{GewyvernTarget, HttpJsonResponse, normalize_targets, post_json};
use crate::{EffectAdapter, EmptySecretStore, SecretStore, validate_id};

pub struct GewyvernDeploymentAdapter {
    targets: BTreeMap<String, GewyvernTarget>,
    secrets: Arc<dyn SecretStore>,
    timeout: Duration,
}

impl GewyvernDeploymentAdapter {
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

impl EffectAdapter for GewyvernDeploymentAdapter {
    fn kind(&self) -> &str {
        GEWYVERN_DEPLOYMENT_EFFECT_KIND
    }

    fn execute(&mut self, payload: &[u8]) -> EffectExecution {
        let request: GewyvernDeploymentRequest = match serde_json::from_slice(payload) {
            Ok(request) => request,
            Err(_) => return reject("invalid Gewyvern deployment payload"),
        };
        if validate_request(&request).is_err() {
            return reject("invalid Gewyvern deployment payload");
        }
        let Some(target) = self.targets.get(&request.runtime_id) else {
            return reject("Gewyvern deployment runtime is not configured");
        };
        if !target.is_authenticated() {
            return reject("Gewyvern deployment target is not authenticated");
        }
        let body = match deployment_body(&request) {
            Ok(body) => body,
            Err(error) => return reject(error),
        };
        let response = match post_json(
            target,
            self.secrets.as_ref(),
            "/v1/deployments",
            &body,
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
        settle_response(&request, response)
    }
}

fn settle_response(
    request: &GewyvernDeploymentRequest,
    response: HttpJsonResponse,
) -> EffectExecution {
    match response.status {
        200 | 202 => validate_response(request, response.status, &response.body)
            .map(EffectExecution::Complete)
            .unwrap_or_else(reject),
        400 | 413 => reject("Gewyvern deployment request was invalid"),
        403 => reject("Gewyvern deployment authentication was rejected"),
        409 => reject("Gewyvern deployment request conflicts with an existing request"),
        500..=599 => EffectExecution::Retry {
            error: "Gewyvern deployment service is unavailable".into(),
            after: Duration::from_secs(1),
        },
        _ => reject("Gewyvern deployment returned an unsupported status"),
    }
}

#[derive(Serialize)]
struct DeploymentBody<'a> {
    request_id: &'a str,
    pipeline_kind: &'a str,
    requested_by: &'a str,
    confirmed: bool,
    target: Option<&'a str>,
}

fn deployment_body(request: &GewyvernDeploymentRequest) -> Result<Vec<u8>, &'static str> {
    serde_json::to_vec(&DeploymentBody {
        request_id: &request.request_id,
        pipeline_kind: &request.pipeline_kind,
        requested_by: &request.requested_by,
        confirmed: true,
        target: request.target.as_deref(),
    })
    .map_err(|_| "Gewyvern deployment payload cannot be encoded")
}

fn validate_request(request: &GewyvernDeploymentRequest) -> Result<(), ()> {
    validate_id("runtime_id", &request.runtime_id).map_err(|_| ())?;
    if !request.confirmed
        || validate_id("request_id", &request.request_id).is_err()
        || validate_id("requested_by", &request.requested_by).is_err()
        || validate_deployment_intent(&request.pipeline_kind, request.target.as_deref()).is_err()
    {
        return Err(());
    }
    Ok(())
}

fn validate_response(
    request: &GewyvernDeploymentRequest,
    http_status: u16,
    body: &[u8],
) -> Result<Vec<u8>, &'static str> {
    let response: GewyvernDeploymentResponse =
        serde_json::from_slice(body).map_err(|_| "Gewyvern deployment response JSON is invalid")?;
    if validate_id("deployment_id", &response.deployment_id).is_err()
        || response.request_id != request.request_id
        || response.pipeline_kind != request.pipeline_kind
        || response.requested_by != request.requested_by
        || response.target != request.target
        || response.status != "accepted"
        || (http_status == 200) != response.replayed
        || (http_status == 202) == response.replayed
    {
        return Err("Gewyvern deployment response does not match the request");
    }
    Ok(body.to_vec())
}

fn reject(error: &str) -> EffectExecution {
    EffectExecution::Reject {
        error: error.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::net::TcpListener;
    use std::path::PathBuf;
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use crate::{ConfiguredSecretStore, SecretKey, SecretValue};
    use rcgen::{CertifiedKey, generate_simple_self_signed};
    use rustls::pki_types::{PrivateKeyDer, PrivatePkcs8KeyDer};
    use rustls::{ServerConfig, ServerConnection, StreamOwned};

    use super::*;

    fn request_payload(confirmed: bool) -> Vec<u8> {
        serde_json::to_vec(&GewyvernDeploymentRequest {
            runtime_id: "runtime-a".into(),
            request_id: "deploy-1".into(),
            pipeline_kind: "http/request".into(),
            requested_by: "operator.example".into(),
            confirmed,
            target: Some("pid:42".into()),
        })
        .unwrap()
    }

    fn temp_path(label: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "leserpent-deployment-{label}-{}-{unique}.pem",
            std::process::id()
        ))
    }

    fn accepted_response(stream: &mut impl Write) {
        let response = br#"{"deployment_id":"gdep_0001","request_id":"deploy-1","pipeline_kind":"http/request","requested_by":"operator.example","status":"accepted","accepted_unix_ms":1700000000000,"target":"pid:42","replayed":false}"#;
        write!(
            stream,
            "HTTP/1.1 202 Accepted\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response.len()
        )
        .unwrap();
        stream.write_all(response).unwrap();
    }

    #[test]
    fn authenticated_deployment_posts_only_the_typed_intent() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = vec![0_u8; 4096];
            let read = stream.read(&mut request).unwrap();
            let request = std::str::from_utf8(&request[..read]).unwrap();
            assert!(request.starts_with("POST /v1/deployments HTTP/1.1\r\n"));
            assert!(request.contains("X-Gewyvern-Admin-Token: test-token\r\n"));
            assert!(request.contains("Content-Type: application/json\r\n"));
            let body = request.split_once("\r\n\r\n").unwrap().1;
            assert!(!body.contains("test-token"));
            assert!(!body.contains("command"));
            accepted_response(&mut stream);
        });
        let key = SecretKey::new("runtime-a-admin").unwrap();
        let secrets = Arc::new(
            ConfiguredSecretStore::new([(key.clone(), SecretValue::new("test-token").unwrap())])
                .unwrap(),
        );
        let target = GewyvernTarget::loopback(address, Some(key)).unwrap();
        let mut adapter = GewyvernDeploymentAdapter::with_secret_store(
            [("runtime-a".to_string(), target)],
            secrets,
        )
        .unwrap();
        let EffectExecution::Complete(body) = adapter.execute(&request_payload(true)) else {
            panic!("deployment should complete");
        };
        let result: GewyvernDeploymentResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(result.deployment_id, "gdep_0001");
        assert!(!result.replayed);
        server.join().unwrap();
    }

    #[test]
    fn authenticated_deployment_round_trips_over_verified_https() {
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
        let ca_path = temp_path("ca");
        fs::write(&ca_path, cert.pem()).unwrap();
        let private_key =
            PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(signing_key.serialize_der()));
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = ServerConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_no_client_auth()
            .with_single_cert(vec![cert.der().clone()], private_key)
            .unwrap();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (socket, _) = listener.accept().unwrap();
            let connection = ServerConnection::new(Arc::new(config)).unwrap();
            let mut stream = StreamOwned::new(connection, socket);
            let mut request = [0_u8; 4096];
            let read = stream.read(&mut request).unwrap();
            let request = std::str::from_utf8(&request[..read]).unwrap();
            assert!(request.starts_with("POST /v1/deployments HTTP/1.1\r\n"));
            assert!(request.contains("X-Gewyvern-Admin-Token: test-token\r\n"));
            accepted_response(&mut stream);
            stream.conn.send_close_notify();
            stream.flush().unwrap();
        });
        let key = SecretKey::new("runtime-a-admin").unwrap();
        let secrets = Arc::new(
            ConfiguredSecretStore::new([(key.clone(), SecretValue::new("test-token").unwrap())])
                .unwrap(),
        );
        let target = GewyvernTarget::https(
            &format!("https://localhost:{}", address.port()),
            &ca_path,
            key,
        )
        .unwrap();
        let mut adapter = GewyvernDeploymentAdapter::with_secret_store(
            [("runtime-a".to_string(), target)],
            secrets,
        )
        .unwrap();
        assert!(matches!(
            adapter.execute(&request_payload(true)),
            EffectExecution::Complete(_)
        ));
        server.join().unwrap();
        fs::remove_file(ca_path).unwrap();
    }

    #[test]
    fn invalid_or_unauthenticated_deployments_fail_before_network_access() {
        let target = GewyvernTarget::loopback("127.0.0.1:9".parse().unwrap(), None).unwrap();
        let mut adapter =
            GewyvernDeploymentAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        assert!(matches!(
            adapter.execute(&request_payload(false)),
            EffectExecution::Reject { .. }
        ));
        assert!(matches!(
            adapter.execute(&request_payload(true)),
            EffectExecution::Reject { ref error }
                if error == "Gewyvern deployment target is not authenticated"
        ));
        let mut extra = request_payload(true);
        extra.pop();
        extra.extend_from_slice(br#","command":"sh"}"#);
        assert!(matches!(
            adapter.execute(&extra),
            EffectExecution::Reject { .. }
        ));
    }

    #[test]
    fn request_validation_reuses_domain_identity_and_deployment_rules() {
        let mut request: GewyvernDeploymentRequest =
            serde_json::from_slice(&request_payload(true)).unwrap();
        request.request_id = "deploy:one".into();
        assert!(validate_request(&request).is_ok());

        request.request_id = "bad/request".into();
        assert!(validate_request(&request).is_err());
        request.request_id = "deploy:one".into();
        request.requested_by = "operator@example".into();
        assert!(validate_request(&request).is_err());
        request.requested_by = "operator.example".into();
        request.pipeline_kind = "bad kind".into();
        assert!(validate_request(&request).is_err());
        request.pipeline_kind = "http/request".into();
        request.target = Some(" bad".into());
        assert!(validate_request(&request).is_err());
    }

    #[test]
    fn response_must_echo_idempotency_and_replay_semantics() {
        let request: GewyvernDeploymentRequest =
            serde_json::from_slice(&request_payload(true)).unwrap();
        let mismatched = br#"{"deployment_id":"gdep_1","request_id":"other","pipeline_kind":"http/request","requested_by":"operator.example","status":"accepted","accepted_unix_ms":1,"target":"pid:42","replayed":false}"#;
        assert!(validate_response(&request, 202, mismatched).is_err());
        let wrong_replay = br#"{"deployment_id":"gdep_1","request_id":"deploy-1","pipeline_kind":"http/request","requested_by":"operator.example","status":"accepted","accepted_unix_ms":1,"target":"pid:42","replayed":true}"#;
        assert!(validate_response(&request, 202, wrong_replay).is_err());
    }

    #[test]
    fn conflict_is_permanent_and_service_failure_is_retryable() {
        let request: GewyvernDeploymentRequest =
            serde_json::from_slice(&request_payload(true)).unwrap();
        assert!(matches!(
            settle_response(
                &request,
                HttpJsonResponse {
                    status: 409,
                    body: b"{}".to_vec(),
                },
            ),
            EffectExecution::Reject { .. }
        ));
        assert!(matches!(
            settle_response(
                &request,
                HttpJsonResponse {
                    status: 503,
                    body: b"{}".to_vec(),
                },
            ),
            EffectExecution::Retry { .. }
        ));
    }
}
