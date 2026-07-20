use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use leserpent_protocol::{
    MAX_PROTOCOL_MESSAGE_BYTES, ResponseEnvelope, decode_request, encode_response,
};
use leserpent_runtime::ControlRuntime;
use serde::Deserialize;

use crate::wire::{
    MAX_AUTH_TOKEN_BYTES, constant_time_equals, error_response, execute_request,
    validate_auth_token,
};

const MAX_IPC_FRAME_BYTES: usize = MAX_PROTOCOL_MESSAGE_BYTES + 1024;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AuthenticatedRequest {
    token: String,
    request: serde_json::Value,
}

pub struct IpcServer {
    listener: UnixListener,
    path: PathBuf,
    socket_device: u64,
    socket_inode: u64,
    token: Vec<u8>,
}

impl IpcServer {
    pub fn bind(path: impl AsRef<Path>, token: &str) -> Result<Self, String> {
        validate_auth_token(token).map_err(|error| format!("IPC {error}"))?;
        let path = path.as_ref();
        if path.as_os_str().len() > 100 {
            return Err("IPC socket path is too long".into());
        }
        if fs::symlink_metadata(path).is_ok() {
            return Err("IPC socket path already exists".into());
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let listener = UnixListener::bind(path).map_err(|error| error.to_string())?;
        fs::set_permissions(path, fs::Permissions::from_mode(0o600))
            .map_err(|error| error.to_string())?;
        listener
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        let metadata = fs::symlink_metadata(path).map_err(|error| error.to_string())?;
        Ok(Self {
            listener,
            path: path.to_path_buf(),
            socket_device: metadata.dev(),
            socket_inode: metadata.ino(),
            token: token.as_bytes().to_vec(),
        })
    }

    pub fn poll_once(&self, runtime: &mut ControlRuntime) -> Result<bool, String> {
        let (stream, _) = match self.listener.accept() {
            Ok(connection) => connection,
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => return Ok(false),
            Err(error) => return Err(error.to_string()),
        };
        self.handle(stream, runtime)?;
        Ok(true)
    }

    fn handle(&self, mut stream: UnixStream, runtime: &mut ControlRuntime) -> Result<(), String> {
        stream
            .set_nonblocking(false)
            .map_err(|error| error.to_string())?;
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .map_err(|error| error.to_string())?;
        stream
            .set_write_timeout(Some(Duration::from_secs(2)))
            .map_err(|error| error.to_string())?;
        let mut frame = Vec::new();
        BufReader::new(&stream)
            .take((MAX_IPC_FRAME_BYTES + 1) as u64)
            .read_until(b'\n', &mut frame)
            .map_err(|error| error.to_string())?;
        let response = self.dispatch(&frame, runtime);
        let mut encoded = encode_response(&response).map_err(|error| error.to_string())?;
        encoded.push(b'\n');
        stream
            .write_all(&encoded)
            .map_err(|error| error.to_string())
    }

    fn dispatch(&self, frame: &[u8], runtime: &mut ControlRuntime) -> ResponseEnvelope {
        if frame.len() > MAX_IPC_FRAME_BYTES || !frame.ends_with(b"\n") {
            return error_response("invalid_frame", "IPC frame is missing or too large");
        }
        let authenticated: AuthenticatedRequest = match serde_json::from_slice(frame) {
            Ok(request) => request,
            Err(_) => return error_response("invalid_json", "IPC frame is not valid JSON"),
        };
        if authenticated.token.len() > MAX_AUTH_TOKEN_BYTES
            || !constant_time_equals(authenticated.token.as_bytes(), &self.token)
        {
            return error_response("unauthorized", "IPC authentication failed");
        }
        let request_bytes = match serde_json::to_vec(&authenticated.request) {
            Ok(bytes) => bytes,
            Err(_) => return error_response("invalid_request", "IPC request cannot be encoded"),
        };
        let request = match decode_request(&request_bytes) {
            Ok(request) => request,
            Err(_) => return error_response("invalid_request", "IPC protocol request is invalid"),
        };
        execute_request(runtime, request)
    }
}

impl Drop for IpcServer {
    fn drop(&mut self) {
        let Ok(metadata) = fs::symlink_metadata(&self.path) else {
            return;
        };
        if metadata.dev() == self.socket_device && metadata.ino() == self.socket_inode {
            let _ = fs::remove_file(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    use leserpent_domain::{
        CAPABILITY_ORCHESTRA_WRITE, CAPABILITY_RUNTIME_DEPLOY, CAPABILITY_RUNTIME_READ,
        CAPABILITY_RUNTIME_REGISTER, CapabilitySet, Command, CommandEnvelope, CommandId,
        CommandOrigin, CommandStatus, Confirmation, DOMAIN_SCHEMA_VERSION, IdempotencyKey,
        Principal, Query, QueryEnvelope, QueryResult, RUNTIME_DEPLOYMENT_EFFECT_KIND,
        RuntimeDeploymentOutcome, RuntimeDeploymentRequest, RuntimeId, RuntimeListFilter,
        RuntimeLogLevel, RuntimeTags,
    };
    use leserpent_protocol::{
        DeploymentReceiptRequest, DeploymentReceiptStatus, HealthRequest, OrchestraDeleteRequest,
        OrchestraHistoryRequest, OrchestraPersistenceRequest, PROTOCOL_SCHEMA_VERSION,
        ProtocolRequest, ProtocolResponse, RequestEnvelope, decode_response,
    };

    use super::*;

    const TOKEN: &str = "0123456789abcdef0123456789abcdef";

    fn temp_path(label: &str, extension: &str) -> PathBuf {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        PathBuf::from("/tmp").join(format!(
            "leserpentd-ipc-{label}-{}-{unique}.{extension}",
            std::process::id()
        ))
    }

    fn query_request() -> RequestEnvelope {
        RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query: Query::RuntimeList {
                    filter: RuntimeListFilter::default(),
                },
            }),
        }
    }

    fn send(
        server: &IpcServer,
        runtime: &mut ControlRuntime,
        socket: &Path,
        token: &str,
        protocol_request: RequestEnvelope,
    ) -> ResponseEnvelope {
        let request = serde_json::json!({ "token": token, "request": protocol_request });
        let mut client = UnixStream::connect(socket).unwrap();
        let mut encoded = serde_json::to_vec(&request).unwrap();
        encoded.push(b'\n');
        client.write_all(&encoded).unwrap();
        assert!(server.poll_once(runtime).unwrap());
        let mut response = Vec::new();
        client.read_to_end(&mut response).unwrap();
        decode_response(&response).unwrap()
    }

    #[test]
    fn authenticated_query_round_trips_over_private_socket() {
        let database = temp_path("roundtrip", "sqlite");
        let socket = temp_path("roundtrip", "sock");
        let mut runtime = ControlRuntime::open(&database).unwrap();
        let server = IpcServer::bind(&socket, TOKEN).unwrap();
        assert_eq!(
            fs::metadata(&socket).unwrap().permissions().mode() & 0o777,
            0o600
        );
        let response = send(&server, &mut runtime, &socket, TOKEN, query_request());
        assert!(matches!(response.response, ProtocolResponse::Query(_)));
        drop(server);
        assert!(!socket.exists());
        drop(runtime);
        fs::remove_file(database).unwrap();
    }

    #[test]
    fn authenticated_registration_is_durable_and_idempotent_over_ipc() {
        let database = temp_path("registration", "sqlite");
        let socket = temp_path("registration", "sock");
        let mut runtime = ControlRuntime::open(&database).unwrap();
        let server = IpcServer::bind(&socket, TOKEN).unwrap();
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
                    tags: RuntimeTags {
                        environment: Some("production".into()),
                        cluster: Some("east".into()),
                        role: Some("edge".into()),
                    },
                },
            }),
        };
        let first = send(&server, &mut runtime, &socket, TOKEN, request.clone());
        let replay = send(&server, &mut runtime, &socket, TOKEN, request);
        assert_eq!(first, replay);
        assert!(matches!(
            &first.response,
            ProtocolResponse::Command(result)
                if result.status == CommandStatus::Applied
                    && result.runtime.id.as_str() == "runtime-new"
                    && result.runtime.revision.0 == 1
        ));
        drop(server);
        drop(runtime);

        let mut restored = ControlRuntime::open(&database).unwrap();
        let result = restored
            .execute_plan(leserpent_domain::CommandPlan {
                schema_version: leserpent_domain::COMMAND_PLAN_SCHEMA_VERSION,
                required_capability: CAPABILITY_RUNTIME_READ.into(),
                operation: leserpent_domain::PlannedOperation::Query(QueryEnvelope {
                    schema_version: DOMAIN_SCHEMA_VERSION,
                    principal: Principal {
                        id: "web-bridge".into(),
                    },
                    capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                    query: Query::RuntimeList {
                        filter: RuntimeListFilter::default(),
                    },
                }),
            })
            .unwrap();
        assert!(matches!(
            result,
            leserpent_runtime::PlanResult::Query(QueryResult::RuntimeList { runtimes, .. })
                if runtimes.len() == 1 && runtimes[0].id.as_str() == "runtime-new"
        ));
        drop(restored);
        fs::remove_file(database).unwrap();
    }

    #[test]
    fn authenticated_runtime_logs_round_trip_without_endpoint_disclosure() {
        let database = temp_path("logs", "sqlite");
        let socket = temp_path("logs", "sock");
        let mut runtime = ControlRuntime::open(&database).unwrap();
        let runtime_id = RuntimeId::new("runtime-a").unwrap();
        runtime
            .register_runtime(
                runtime_id.clone(),
                "Runtime A",
                "https://secret-endpoint.invalid",
            )
            .unwrap();
        runtime
            .append_runtime_log(&runtime_id, RuntimeLogLevel::Warning, "bounded warning")
            .unwrap();
        let server = IpcServer::bind(&socket, TOKEN).unwrap();
        let request = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Query(QueryEnvelope {
                schema_version: DOMAIN_SCHEMA_VERSION,
                principal: Principal {
                    id: "operator".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
                query: Query::RuntimeLogs {
                    runtime_id,
                    after_sequence: None,
                    limit: 10,
                },
            }),
        };
        let response = send(&server, &mut runtime, &socket, TOKEN, request);
        assert!(matches!(
            &response.response,
            ProtocolResponse::Query(QueryResult::RuntimeLogs { entries, .. })
                if entries.len() == 1 && entries[0].message == "bounded warning"
        ));
        let encoded = serde_json::to_string(&response).unwrap();
        assert!(!encoded.contains("secret-endpoint"));
        assert!(!encoded.contains("endpoint"));
        drop(server);
        drop(runtime);
        fs::remove_file(database).unwrap();
    }

    #[test]
    fn invalid_token_is_rejected_without_dispatch() {
        let database = temp_path("auth", "sqlite");
        let socket = temp_path("auth", "sock");
        let mut runtime = ControlRuntime::open(&database).unwrap();
        let server = IpcServer::bind(&socket, TOKEN).unwrap();
        let response = send(
            &server,
            &mut runtime,
            &socket,
            "fedcba9876543210fedcba9876543210",
            query_request(),
        );
        assert!(matches!(
            response.response,
            ProtocolResponse::Error(ref error) if error.code == "unauthorized"
        ));
        drop(server);
        drop(runtime);
        fs::remove_file(database).unwrap();
    }

    #[test]
    fn authenticated_deployment_receipt_returns_only_the_bound_outcome() {
        let database = temp_path("deployment-receipt", "sqlite");
        let socket = temp_path("deployment-receipt", "sock");
        let mut runtime = ControlRuntime::open(&database).unwrap();
        let deployment = RuntimeDeploymentRequest {
            runtime_id: "runtime-a".into(),
            request_id: "deploy-1".into(),
            pipeline_kind: "http/request".into(),
            requested_by: "operator.example".into(),
            confirmed: true,
            target: None,
        };
        runtime
            .enqueue_effect(
                "deploy-command",
                RUNTIME_DEPLOYMENT_EFFECT_KIND,
                &serde_json::to_vec(&deployment).unwrap(),
                1,
            )
            .unwrap();
        let lease = runtime
            .claim_effect("worker-a", Duration::from_secs(30))
            .unwrap()
            .unwrap();
        let outcome = RuntimeDeploymentOutcome {
            deployment_id: "gdep-1".into(),
            request_id: "deploy-1".into(),
            pipeline_kind: "http/request".into(),
            requested_by: "operator.example".into(),
            status: "accepted".into(),
            accepted_unix_ms: 1_700_000_000_000,
            target: None,
            replayed: false,
        };
        runtime
            .complete_effect(&lease, &serde_json::to_vec(&outcome).unwrap())
            .unwrap();
        let server = IpcServer::bind(&socket, TOKEN).unwrap();
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
        let response = send(&server, &mut runtime, &socket, TOKEN, request.clone());
        assert!(matches!(
            response.response,
            ProtocolResponse::DeploymentReceipt(ref receipt)
                if receipt.status == DeploymentReceiptStatus::Completed
                    && receipt.outcome.as_ref() == Some(&outcome)
        ));

        let mut mismatched = request;
        let ProtocolRequest::DeploymentReceipt(receipt) = &mut mismatched.request else {
            unreachable!();
        };
        receipt.request_id = "deploy-other".into();
        let response = send(&server, &mut runtime, &socket, TOKEN, mismatched);
        assert!(matches!(
            response.response,
            ProtocolResponse::Error(ref error) if error.code == "invalid_request"
        ));
        drop(server);
        drop(runtime);
        fs::remove_file(database).unwrap();
    }

    #[test]
    fn authenticated_orchestra_persistence_is_atomic_and_idempotent() {
        let database = temp_path("orchestra-persistence", "sqlite");
        let socket = temp_path("orchestra-persistence", "sock");
        let mut runtime = ControlRuntime::open(&database).unwrap();
        let server = IpcServer::bind(&socket, TOKEN).unwrap();
        let envelope =
            leserpent_protocol::compatibility_v1::decode_orchestra_persistence(include_bytes!(
                "../../leserpent-protocol/tests/fixtures/legacy-orchestra-persistence-v1.json"
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
        for _ in 0..2 {
            let response = send(&server, &mut runtime, &socket, TOKEN, request.clone());
            assert!(matches!(
                response.response,
                ProtocolResponse::OrchestraPersisted(ref persisted)
                    if persisted.envelope == envelope && persisted.event_count == 1
            ));
        }

        let history = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraHistory(OrchestraHistoryRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                runtime_id: Some(envelope.run.runtime_id.clone()),
                run_id: Some(envelope.run.run_id.clone()),
                offset: 0,
                limit: 64,
            }),
        };
        let response = send(&server, &mut runtime, &socket, TOKEN, history);
        assert!(matches!(
            response.response,
            ProtocolResponse::OrchestraHistory(ref history)
                if history.runs.is_empty()
                    && history.events.len() == 1
                    && history.events[0].event_id == 1
                    && history.events[0].run_id == envelope.event.run_id
                    && history.next_offset.is_none()
        ));

        let mut drifted = request;
        let ProtocolRequest::OrchestraPersist(persistence) = &mut drifted.request else {
            unreachable!();
        };
        persistence.envelope.event.summary = "drifted retry".into();
        let response = send(&server, &mut runtime, &socket, TOKEN, drifted);
        assert!(matches!(
            response.response,
            ProtocolResponse::Error(ref error)
                if error.code == "orchestra_persistence_failed"
        ));
        let delete = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraDelete(OrchestraDeleteRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                runtime_ids: vec![envelope.run.runtime_id.clone()],
            }),
        };
        let response = send(&server, &mut runtime, &socket, TOKEN, delete);
        assert!(matches!(
            response.response,
            ProtocolResponse::OrchestraDeleted(ref deleted)
                if deleted.deleted_runtime_count == 1
                    && deleted.deleted_run_count == 1
                    && deleted.deleted_event_count == 1
        ));
        let history = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::OrchestraHistory(OrchestraHistoryRequest {
                principal: Principal {
                    id: "operator-a".into(),
                },
                capabilities: CapabilitySet::new([CAPABILITY_ORCHESTRA_WRITE]),
                runtime_id: Some(envelope.run.runtime_id.clone()),
                run_id: None,
                offset: 0,
                limit: 64,
            }),
        };
        let response = send(&server, &mut runtime, &socket, TOKEN, history);
        assert!(matches!(
            response.response,
            ProtocolResponse::OrchestraHistory(ref history)
                if history.runs.is_empty() && history.events.is_empty()
        ));
        drop(server);
        drop(runtime);
        fs::remove_file(database).unwrap();
    }

    #[test]
    fn authenticated_health_proves_runtime_authority() {
        let database = temp_path("health", "sqlite");
        let socket = temp_path("health", "sock");
        let mut runtime = ControlRuntime::open(&database).unwrap();
        runtime
            .enqueue_effect("health-visible", "test.effect", b"payload", 3)
            .unwrap();
        let server = IpcServer::bind(&socket, TOKEN).unwrap();
        let health = RequestEnvelope {
            schema_version: PROTOCOL_SCHEMA_VERSION,
            request: ProtocolRequest::Health(HealthRequest {}),
        };
        let response = send(&server, &mut runtime, &socket, TOKEN, health);
        assert!(matches!(
            response.response,
            ProtocolResponse::Health(ref health)
                if health.status == "ready"
                    && health.authority_owned
                    && health.protocol_schema_version == PROTOCOL_SCHEMA_VERSION
                    && health.effect_queue.as_ref().is_some_and(|queue|
                        queue.ready == 1
                            && queue.active == 1
                            && queue.terminal == 0
                            && !queue.saturated)
        ));
        drop(server);
        drop(runtime);
        fs::remove_file(database).unwrap();
    }

    #[test]
    fn malformed_or_unterminated_frames_fail_closed() {
        let database = temp_path("frame", "sqlite");
        let socket = temp_path("frame", "sock");
        let mut runtime = ControlRuntime::open(&database).unwrap();
        let server = IpcServer::bind(&socket, TOKEN).unwrap();
        let response = server.dispatch(br#"{"token":"ignored"}"#, &mut runtime);
        assert!(matches!(
            response.response,
            ProtocolResponse::Error(ref error) if error.code == "invalid_frame"
        ));
        let oversized = vec![b'x'; MAX_IPC_FRAME_BYTES + 1];
        let response = server.dispatch(&oversized, &mut runtime);
        assert!(matches!(
            response.response,
            ProtocolResponse::Error(ref error) if error.code == "invalid_frame"
        ));
        drop(server);
        drop(runtime);
        fs::remove_file(database).unwrap();
    }
}
