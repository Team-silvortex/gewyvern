use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpStream};
use std::time::Duration;

use leserpent_domain::{
    RUNTIME_STATUS_REFRESH_EFFECT_KIND, Revision, RuntimeStatusObservation, RuntimeStatusSnapshot,
};
use leserpent_runtime::EffectExecution;
use serde::{Deserialize, Serialize};

use crate::{EffectAdapter, validate_id};

pub const GEWYVERN_HEALTH_EFFECT_KIND: &str = "gewyvern.health.check";
pub const GEWYVERN_STATUS_REFRESH_EFFECT_KIND: &str = RUNTIME_STATUS_REFRESH_EFFECT_KIND;
const MAX_HTTP_RESPONSE_BYTES: u64 = 64 * 1024;

#[derive(Clone, Debug)]
pub struct GewyvernTarget {
    address: SocketAddr,
    admin_token: Option<String>,
}

impl GewyvernTarget {
    pub fn loopback(address: SocketAddr, admin_token: Option<String>) -> Result<Self, String> {
        if !address.ip().is_loopback() {
            return Err("Gewyvern adapter currently permits loopback targets only".into());
        }
        if admin_token.as_ref().is_some_and(|token| {
            token.is_empty() || token.len() > 256 || token.contains('\r') || token.contains('\n')
        }) {
            return Err("Gewyvern admin token is invalid".into());
        }
        Ok(Self {
            address,
            admin_token,
        })
    }
}

pub struct GewyvernHealthAdapter {
    targets: BTreeMap<String, GewyvernTarget>,
    timeout: Duration,
}

pub struct GewyvernStatusRefreshAdapter {
    targets: BTreeMap<String, GewyvernTarget>,
    timeout: Duration,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GewyvernStatusRefreshRequest {
    pub runtime_id: String,
    pub expected_revision: Revision,
}

pub type GewyvernStatusObservation = RuntimeStatusObservation;

impl GewyvernHealthAdapter {
    pub fn new(
        targets: impl IntoIterator<Item = (String, GewyvernTarget)>,
    ) -> Result<Self, String> {
        Ok(Self {
            targets: normalize_targets(targets)?,
            timeout: Duration::from_secs(3),
        })
    }
}

impl GewyvernStatusRefreshAdapter {
    pub fn new(
        targets: impl IntoIterator<Item = (String, GewyvernTarget)>,
    ) -> Result<Self, String> {
        Ok(Self {
            targets: normalize_targets(targets)?,
            timeout: Duration::from_secs(3),
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct HealthPayload {
    runtime_id: String,
}

impl EffectAdapter for GewyvernHealthAdapter {
    fn kind(&self) -> &str {
        GEWYVERN_HEALTH_EFFECT_KIND
    }

    fn execute(&mut self, payload: &[u8]) -> EffectExecution {
        let request: HealthPayload = match serde_json::from_slice(payload) {
            Ok(request) => request,
            Err(_) => return reject("invalid Gewyvern health payload"),
        };
        if validate_id("runtime_id", &request.runtime_id).is_err() {
            return reject("invalid Gewyvern runtime_id");
        }
        let Some(target) = self.targets.get(&request.runtime_id) else {
            return reject("Gewyvern runtime is not configured");
        };
        match fetch_health(target, self.timeout) {
            Ok(body) => EffectExecution::Complete(body),
            Err(error) => EffectExecution::Retry {
                error,
                after: Duration::from_secs(1),
            },
        }
    }
}

impl EffectAdapter for GewyvernStatusRefreshAdapter {
    fn kind(&self) -> &str {
        GEWYVERN_STATUS_REFRESH_EFFECT_KIND
    }

    fn execute(&mut self, payload: &[u8]) -> EffectExecution {
        let request: GewyvernStatusRefreshRequest = match serde_json::from_slice(payload) {
            Ok(request) => request,
            Err(_) => return reject("invalid Gewyvern status refresh payload"),
        };
        if validate_id("runtime_id", &request.runtime_id).is_err() {
            return reject("invalid Gewyvern runtime_id");
        }
        let Some(target) = self.targets.get(&request.runtime_id) else {
            return reject("Gewyvern runtime is not configured");
        };
        match fetch_status(target, self.timeout, &request) {
            Ok(observation) => match serde_json::to_vec(&observation) {
                Ok(encoded) => EffectExecution::Complete(encoded),
                Err(_) => reject("Gewyvern status observation cannot be encoded"),
            },
            Err(error) => EffectExecution::Retry {
                error,
                after: Duration::from_secs(1),
            },
        }
    }
}

fn normalize_targets(
    targets: impl IntoIterator<Item = (String, GewyvernTarget)>,
) -> Result<BTreeMap<String, GewyvernTarget>, String> {
    let mut normalized = BTreeMap::new();
    for (runtime_id, target) in targets {
        validate_id("runtime_id", &runtime_id)?;
        if normalized.insert(runtime_id.clone(), target).is_some() {
            return Err(format!("duplicate Gewyvern runtime '{runtime_id}'"));
        }
    }
    if normalized.is_empty() {
        return Err("Gewyvern adapter requires at least one target".into());
    }
    Ok(normalized)
}

fn fetch_health(target: &GewyvernTarget, timeout: Duration) -> Result<Vec<u8>, String> {
    let body = fetch_json(target, "/health", timeout)?;
    let health: serde_json::Value = serde_json::from_slice(&body)
        .map_err(|_| "Gewyvern health response JSON is invalid".to_string())?;
    if health.get("ok") != Some(&serde_json::Value::Bool(true)) {
        return Err("Gewyvern health response is not ready".into());
    }
    Ok(body)
}

#[derive(Deserialize)]
struct HealthDocument {
    ok: bool,
    has_snapshot: bool,
    kind: Option<String>,
    updated_unix_ms: u128,
    #[serde(default)]
    resilience_degraded: bool,
}

#[derive(Default, Deserialize)]
struct MetaDocument {
    target_count: Option<u64>,
    #[serde(default)]
    has_summary_json: bool,
    #[serde(default)]
    has_analysis_json: bool,
    #[serde(default)]
    has_training_example_json: bool,
    #[serde(default)]
    has_export_json: bool,
    #[serde(default)]
    has_report_json: bool,
    #[serde(default)]
    has_report_html: bool,
    #[serde(default)]
    has_external_sidecar_context: bool,
    #[serde(default)]
    has_external_evidence_chain_enrichment: bool,
    #[serde(default)]
    has_external_diagnostic_opinion: bool,
}

fn fetch_status(
    target: &GewyvernTarget,
    timeout: Duration,
    request: &GewyvernStatusRefreshRequest,
) -> Result<GewyvernStatusObservation, String> {
    let health: HealthDocument =
        serde_json::from_slice(&fetch_json(target, "/health", timeout)?)
            .map_err(|_| "Gewyvern health response JSON is invalid".to_string())?;
    if !health.ok {
        return Err("Gewyvern health response is not ready".into());
    }
    let meta: MetaDocument = if health.has_snapshot {
        serde_json::from_slice(&fetch_json(target, "/v1/latest/meta", timeout)?)
            .map_err(|_| "Gewyvern snapshot metadata JSON is invalid".to_string())?
    } else {
        MetaDocument::default()
    };
    Ok(GewyvernStatusObservation {
        runtime_id: request.runtime_id.clone(),
        expected_revision: request.expected_revision,
        status: RuntimeStatusSnapshot {
            status_source: "gewyvern-api".into(),
            status_fetched_at: Some(health.updated_unix_ms.to_string()),
            status_fetch_error: None,
            has_latest_snapshot: health.has_snapshot,
            snapshot_kind: health.kind,
            target_count: meta.target_count,
            has_summary_json: meta.has_summary_json,
            has_analysis_json: meta.has_analysis_json,
            has_training_example_json: meta.has_training_example_json,
            has_training_dataset_manifest: false,
            has_export_json: meta.has_export_json,
            has_report_json: meta.has_report_json,
            has_report_html: meta.has_report_html,
            has_external_sidecar_context: meta.has_external_sidecar_context,
            has_external_evidence_chain_enrichment: meta.has_external_evidence_chain_enrichment,
            has_external_diagnostic_opinion: meta.has_external_diagnostic_opinion,
            resilience_degraded: health.resilience_degraded,
            resilience_status: None,
            resilience_summary: None,
            socket_service_status: None,
            socket_consecutive_idle_timeouts: None,
            socket_total_idle_timeouts: None,
        },
    })
}

fn fetch_json(target: &GewyvernTarget, path: &str, timeout: Duration) -> Result<Vec<u8>, String> {
    let mut stream = TcpStream::connect_timeout(&target.address, timeout)
        .map_err(|_| "Gewyvern API connection failed".to_string())?;
    stream
        .set_read_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(timeout))
        .map_err(|error| error.to_string())?;
    let mut request = format!(
        "GET {path} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\n",
        target.address,
    );
    if let Some(token) = &target.admin_token {
        request.push_str("X-Gewyvern-Admin-Token: ");
        request.push_str(token);
        request.push_str("\r\n");
    }
    request.push_str("\r\n");
    stream
        .write_all(request.as_bytes())
        .map_err(|_| "Gewyvern API write failed".to_string())?;
    let mut response = Vec::new();
    stream
        .take(MAX_HTTP_RESPONSE_BYTES + 1)
        .read_to_end(&mut response)
        .map_err(|_| "Gewyvern API read failed".to_string())?;
    if response.len() as u64 > MAX_HTTP_RESPONSE_BYTES {
        return Err("Gewyvern API response is too large".into());
    }
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| "Gewyvern API response is malformed".to_string())?;
    let headers = std::str::from_utf8(&response[..header_end])
        .map_err(|_| "Gewyvern API response headers are invalid".to_string())?;
    if !headers
        .lines()
        .next()
        .is_some_and(|line| line.starts_with("HTTP/1.1 200 "))
    {
        return Err("Gewyvern API request was rejected".into());
    }
    let body = &response[header_end + 4..];
    Ok(body.to_vec())
}

fn reject(error: &str) -> EffectExecution {
    EffectExecution::Reject {
        error: error.into(),
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr, TcpListener};
    use std::thread;

    use super::*;

    fn serve_json(stream: &mut impl Write, body: &[u8]) {
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(body).unwrap();
    }

    #[test]
    fn health_adapter_calls_bounded_loopback_api_with_configured_token() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let read = stream.read(&mut request).unwrap();
            let request = std::str::from_utf8(&request[..read]).unwrap();
            assert!(request.starts_with("GET /health HTTP/1.1\r\n"));
            assert!(request.contains("X-Gewyvern-Admin-Token: test-token\r\n"));
            let body = br#"{"ok":true,"has_snapshot":false}"#;
            serve_json(&mut stream, body);
        });
        let target = GewyvernTarget::loopback(address, Some("test-token".into())).unwrap();
        let mut adapter = GewyvernHealthAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        let result = adapter.execute(br#"{"runtime_id":"runtime-a"}"#);
        let EffectExecution::Complete(body) = result else {
            panic!("health request should complete");
        };
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&body).unwrap()["ok"],
            true
        );
        server.join().unwrap();
    }

    #[test]
    fn status_adapter_combines_health_and_snapshot_metadata() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let responses: [&[u8]; 2] = [
                br#"{"ok":true,"has_snapshot":true,"kind":"multi","updated_unix_ms":1234,"resilience_degraded":true}"#,
                br#"{"target_count":2,"has_summary_json":true,"has_analysis_json":true,"has_training_example_json":true,"has_export_json":true,"has_report_json":true,"has_report_html":false,"has_external_sidecar_context":true,"has_external_evidence_chain_enrichment":false,"has_external_diagnostic_opinion":true}"#,
            ];
            let paths = ["/health", "/v1/latest/meta"];
            for (path, body) in paths.into_iter().zip(responses) {
                let (mut stream, _) = listener.accept().unwrap();
                let mut request = [0_u8; 4096];
                let read = stream.read(&mut request).unwrap();
                let request = std::str::from_utf8(&request[..read]).unwrap();
                assert!(request.starts_with(&format!("GET {path} HTTP/1.1\r\n")));
                assert!(request.contains("X-Gewyvern-Admin-Token: test-token\r\n"));
                serve_json(&mut stream, body);
            }
        });
        let target = GewyvernTarget::loopback(address, Some("test-token".into())).unwrap();
        let mut adapter =
            GewyvernStatusRefreshAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        let result = adapter.execute(br#"{"runtime_id":"runtime-a","expected_revision":7}"#);
        let EffectExecution::Complete(body) = result else {
            panic!("status refresh should complete");
        };
        let observation: GewyvernStatusObservation = serde_json::from_slice(&body).unwrap();
        assert_eq!(observation.runtime_id, "runtime-a");
        assert_eq!(observation.expected_revision, Revision(7));
        assert_eq!(observation.status.status_source, "gewyvern-api");
        assert_eq!(
            observation.status.status_fetched_at.as_deref(),
            Some("1234")
        );
        assert!(observation.status.has_latest_snapshot);
        assert_eq!(observation.status.snapshot_kind.as_deref(), Some("multi"));
        assert_eq!(observation.status.target_count, Some(2));
        assert!(observation.status.has_summary_json);
        assert!(observation.status.has_external_sidecar_context);
        assert!(observation.status.has_external_diagnostic_opinion);
        assert!(observation.status.resilience_degraded);
        assert!(!observation.status.has_report_html);
        server.join().unwrap();
    }

    #[test]
    fn status_payload_validation_fails_before_network_access() {
        let target = GewyvernTarget::loopback("127.0.0.1:9".parse().unwrap(), None).unwrap();
        let mut adapter =
            GewyvernStatusRefreshAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        assert!(matches!(
            adapter.execute(br#"{"runtime_id":"runtime-a","expected_revision":1,"extra":true}"#),
            EffectExecution::Reject { .. }
        ));
        assert!(matches!(
            adapter.execute(br#"{"runtime_id":"missing","expected_revision":1}"#),
            EffectExecution::Reject { .. }
        ));
    }

    #[test]
    fn status_adapter_accepts_empty_runtime_without_requesting_metadata() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let read = stream.read(&mut request).unwrap();
            assert!(
                std::str::from_utf8(&request[..read])
                    .unwrap()
                    .starts_with("GET /health HTTP/1.1\r\n")
            );
            serve_json(
                &mut stream,
                br#"{"ok":true,"has_snapshot":false,"kind":null,"updated_unix_ms":0}"#,
            );
        });
        let target = GewyvernTarget::loopback(address, None).unwrap();
        let mut adapter =
            GewyvernStatusRefreshAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        let EffectExecution::Complete(body) =
            adapter.execute(br#"{"runtime_id":"runtime-a","expected_revision":1}"#)
        else {
            panic!("empty runtime status should complete");
        };
        let observation: GewyvernStatusObservation = serde_json::from_slice(&body).unwrap();
        assert!(!observation.status.has_latest_snapshot);
        assert_eq!(observation.status.target_count, None);
        server.join().unwrap();
    }

    #[test]
    fn target_and_payload_validation_fail_before_network_access() {
        let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)), 9411);
        assert!(GewyvernTarget::loopback(remote, None).is_err());

        let target = GewyvernTarget::loopback("127.0.0.1:9".parse().unwrap(), None).unwrap();
        let mut adapter = GewyvernHealthAdapter::new([("runtime-a".to_string(), target)]).unwrap();
        assert!(matches!(
            adapter.execute(br#"{"runtime_id":"missing","extra":true}"#),
            EffectExecution::Reject { .. }
        ));
        assert!(matches!(
            adapter.execute(br#"{"runtime_id":"runtime-missing"}"#),
            EffectExecution::Reject { .. }
        ));
    }
}
