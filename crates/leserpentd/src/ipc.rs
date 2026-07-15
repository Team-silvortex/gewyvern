use std::fs;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::time::Duration;

use leserpent_domain::{
    CAPABILITY_RUNTIME_READ, CAPABILITY_RUNTIME_REFRESH, Command, CommandPlan, PlannedOperation,
    Query,
};
use leserpent_protocol::{
    EffectQueueHealth, HealthResponse, MAX_PROTOCOL_MESSAGE_BYTES, PROTOCOL_SCHEMA_VERSION,
    ProtocolError, ProtocolRequest, ProtocolResponse, RequestEnvelope, ResponseEnvelope,
    decode_request, encode_response,
};
use leserpent_runtime::{ControlRuntime, PlanResult, RuntimeError};
use serde::Deserialize;

const MAX_AUTH_TOKEN_BYTES: usize = 256;
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
        validate_token(token)?;
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

fn execute_request(runtime: &mut ControlRuntime, request: RequestEnvelope) -> ResponseEnvelope {
    let request = match request.request {
        ProtocolRequest::Health(_) => {
            return match runtime
                .heartbeat()
                .and_then(|()| runtime.effect_queue_stats())
            {
                Ok(queue) => response(ProtocolResponse::Health(HealthResponse {
                    status: "ready".into(),
                    authority_owned: true,
                    protocol_schema_version: PROTOCOL_SCHEMA_VERSION,
                    effect_queue: Some(EffectQueueHealth {
                        ready: queue.ready,
                        leased: queue.leased,
                        completed: queue.completed,
                        failed: queue.failed,
                        active: queue.active(),
                        terminal: queue.terminal(),
                        capacity: queue.capacity,
                        saturated: queue.saturated(),
                    }),
                })),
                Err(_) => error_response("runtime_unavailable", "runtime authority is unavailable"),
            };
        }
        request => request,
    };
    let required_capability = match &request {
        ProtocolRequest::Query(query) => match query.query {
            Query::RuntimeList { .. } => CAPABILITY_RUNTIME_READ,
        },
        ProtocolRequest::Command(command) => match command.command {
            Command::RuntimeRefresh { .. } => CAPABILITY_RUNTIME_REFRESH,
        },
        ProtocolRequest::Health(_) => unreachable!(),
    };
    let operation = match request {
        ProtocolRequest::Query(query) => PlannedOperation::Query(query),
        ProtocolRequest::Command(command) => PlannedOperation::Command(command),
        ProtocolRequest::Health(_) => unreachable!(),
    };
    match runtime.execute_plan(CommandPlan {
        schema_version: leserpent_domain::COMMAND_PLAN_SCHEMA_VERSION,
        required_capability: required_capability.to_string(),
        operation,
    }) {
        Ok(PlanResult::Query(result)) => response(ProtocolResponse::Query(result)),
        Ok(PlanResult::Command(result)) => response(ProtocolResponse::Command(Box::new(result))),
        Err(RuntimeError::Domain(error)) => leserpent_protocol::domain_error_response(&error),
        Err(RuntimeError::InvalidPlan(_)) => {
            error_response("invalid_request", "IPC command plan is invalid")
        }
        Err(_) => error_response("runtime_failed", "runtime request failed"),
    }
}

fn response(response: ProtocolResponse) -> ResponseEnvelope {
    ResponseEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        response,
    }
}

fn error_response(code: &str, message: &str) -> ResponseEnvelope {
    response(ProtocolResponse::Error(ProtocolError {
        code: code.to_string(),
        message: message.to_string(),
    }))
}

fn validate_token(token: &str) -> Result<(), String> {
    if token.len() < 32
        || token.len() > MAX_AUTH_TOKEN_BYTES
        || token.bytes().any(|byte| byte <= 0x20)
    {
        return Err("IPC token must contain 32 to 256 non-whitespace bytes".into());
    }
    Ok(())
}

fn constant_time_equals(left: &[u8], right: &[u8]) -> bool {
    let mut difference = left.len() ^ right.len();
    let length = left.len().max(right.len());
    for index in 0..length {
        difference |= usize::from(
            left.get(index).copied().unwrap_or(0) ^ right.get(index).copied().unwrap_or(0),
        );
    }
    difference == 0
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::os::unix::fs::PermissionsExt;
    use std::time::{SystemTime, UNIX_EPOCH};

    use leserpent_domain::{
        CAPABILITY_RUNTIME_READ, CapabilitySet, DOMAIN_SCHEMA_VERSION, Principal, Query,
        QueryEnvelope, RuntimeListFilter,
    };
    use leserpent_protocol::{
        HealthRequest, PROTOCOL_SCHEMA_VERSION, ProtocolRequest, ProtocolResponse, RequestEnvelope,
        decode_response,
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
