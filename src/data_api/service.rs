use std::io::{BufReader, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use rustls::pki_types::{CertificateDer, PrivateKeyDer, pem::PemObject};
use rustls::{ServerConfig, ServerConnection, StreamOwned};
use silvortex_bounded_io::open_bounded_regular_file;

use super::{
    API_CLIENT_READ_TIMEOUT, API_CLIENT_WRITE_TIMEOUT, API_MAX_CONCURRENT_CLIENTS, ApiAccessPolicy,
    ApiSnapshot, ApiState, EVENT_API_CLIENT_ACCEPT_FAILED, EVENT_API_CLIENT_OVERLOAD_REJECTED,
    EVENT_API_LISTENER_BIND_FAILED, api_client_is_loopback, handle_api_client, log_error_event,
    log_warn_event, normalize_api_admin_token,
};

pub struct ApiService {
    state: ApiState,
    #[cfg(test)]
    local_addr: std::net::SocketAddr,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

#[derive(Clone)]
enum ApiTransport {
    Plain,
    Tls(Arc<ServerConfig>),
}

impl ApiService {
    pub fn state(&self) -> &ApiState {
        &self.state
    }

    #[cfg(test)]
    pub fn local_addr(&self) -> std::net::SocketAddr {
        self.local_addr
    }

    pub fn shutdown(&mut self) {
        self.shutdown.store(true, Ordering::Release);
        if let Some(handle) = self.handle.take() {
            let _ = handle.join();
        }
    }
}

impl Drop for ApiService {
    fn drop(&mut self) {
        self.shutdown();
    }
}

pub fn start_api_service(addr: &str, allow_remote_bind: bool) -> Result<ApiService, String> {
    let access_policy = ApiAccessPolicy::from_env(allow_remote_bind);
    start_api_service_with(addr, access_policy, ApiTransport::Plain).inspect_err(|message| {
        log_error_event(
            "api",
            EVENT_API_LISTENER_BIND_FAILED,
            &[("socket", addr.to_string()), ("error", message.clone())],
            "refused unsafe api listener bind",
        );
    })
}

pub fn start_api_service_with_admin_token(
    addr: &str,
    allow_remote_bind: bool,
    admin_token: Option<&str>,
) -> Result<ApiService, String> {
    let access_policy = ApiAccessPolicy {
        allow_remote_bind,
        admin_token: admin_token.and_then(normalize_api_admin_token),
        require_token: false,
    };
    start_api_service_with(addr, access_policy, ApiTransport::Plain).inspect_err(|message| {
        log_error_event(
            "api",
            EVENT_API_LISTENER_BIND_FAILED,
            &[("socket", addr.to_string()), ("error", message.clone())],
            "refused unsafe api listener bind",
        );
    })
}

pub fn start_tls_api_service(
    addr: &str,
    certificate_path: impl AsRef<Path>,
    private_key_path: impl AsRef<Path>,
    admin_token: &str,
) -> Result<ApiService, String> {
    let admin_token = normalize_api_admin_token(admin_token)
        .ok_or_else(|| "Gewyvern HTTPS admin token is invalid".to_string())?;
    let tls = load_tls_server_config(certificate_path.as_ref(), private_key_path.as_ref())?;
    start_api_service_with(
        addr,
        ApiAccessPolicy {
            allow_remote_bind: true,
            admin_token: Some(admin_token),
            require_token: true,
        },
        ApiTransport::Tls(Arc::new(tls)),
    )
}

fn start_api_service_with(
    addr: &str,
    access_policy: ApiAccessPolicy,
    transport: ApiTransport,
) -> Result<ApiService, String> {
    validate_api_bind_addr(addr, &access_policy)?;
    let listener = TcpListener::bind(addr)
        .map_err(|error| format!("failed to bind api socket {addr}: {error}"))?;
    listener
        .set_nonblocking(true)
        .map_err(|error| format!("failed to configure api socket {addr}: {error}"))?;
    #[cfg(test)]
    let local_addr = listener
        .local_addr()
        .map_err(|error| format!("failed to inspect api socket {addr}: {error}"))?;
    let state = Arc::new(std::sync::Mutex::new(Arc::new(ApiSnapshot::default())));
    let deployments = Arc::new(std::sync::Mutex::new(Default::default()));
    let thread_state = Arc::clone(&state);
    let thread_deployments = Arc::clone(&deployments);
    let active_clients = Arc::new(AtomicUsize::new(0));
    let shutdown = Arc::new(AtomicBool::new(false));
    let thread_shutdown = Arc::clone(&shutdown);
    let thread_access_policy = access_policy.clone();
    let handle = thread::spawn(move || {
        while !thread_shutdown.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((mut stream, remote_addr)) => {
                    let previous = active_clients.fetch_add(1, Ordering::AcqRel);
                    if previous >= API_MAX_CONCURRENT_CLIENTS {
                        active_clients.fetch_sub(1, Ordering::AcqRel);
                        log_warn_event(
                            "api",
                            EVENT_API_CLIENT_OVERLOAD_REJECTED,
                            &[
                                ("limit", API_MAX_CONCURRENT_CLIENTS.to_string()),
                                ("active_clients", previous.to_string()),
                            ],
                            "rejected api client because concurrency limit was reached",
                        );
                        if matches!(&transport, ApiTransport::Plain) {
                            reject_api_client_overload(stream);
                        }
                        continue;
                    }
                    let client_state = Arc::clone(&thread_state);
                    let client_deployments = Arc::clone(&thread_deployments);
                    let client_counter = Arc::clone(&active_clients);
                    let client_access_policy = thread_access_policy.clone();
                    let client_transport = transport.clone();
                    thread::spawn(move || {
                        let _guard = ActiveApiClientGuard(client_counter);
                        if stream
                            .set_nonblocking(false)
                            .and_then(|()| stream.set_read_timeout(Some(API_CLIENT_READ_TIMEOUT)))
                            .and_then(|()| stream.set_write_timeout(Some(API_CLIENT_WRITE_TIMEOUT)))
                            .is_err()
                        {
                            return;
                        }
                        match client_transport {
                            ApiTransport::Plain => handle_api_client(
                                &mut stream,
                                remote_addr.ip(),
                                client_state,
                                client_deployments,
                                client_access_policy,
                            ),
                            ApiTransport::Tls(config) => {
                                let Ok(mut connection) = ServerConnection::new(config) else {
                                    return;
                                };
                                while connection.is_handshaking() {
                                    if let Err(error) = connection.complete_io(&mut stream) {
                                        log_warn_event(
                                            "api",
                                            EVENT_API_CLIENT_ACCEPT_FAILED,
                                            &[("error", error.to_string())],
                                            "rejected invalid API TLS peer",
                                        );
                                        return;
                                    }
                                }
                                let mut stream = StreamOwned::new(connection, stream);
                                handle_api_client(
                                    &mut stream,
                                    remote_addr.ip(),
                                    client_state,
                                    client_deployments,
                                    client_access_policy,
                                );
                                stream.conn.send_close_notify();
                                let _ = stream.flush();
                            }
                        }
                    });
                }
                Err(err) => {
                    if err.kind() == std::io::ErrorKind::WouldBlock {
                        thread::sleep(Duration::from_millis(50));
                        continue;
                    }
                    log_warn_event(
                        "api",
                        EVENT_API_CLIENT_ACCEPT_FAILED,
                        &[("error", err.to_string())],
                        "failed to accept api client",
                    );
                }
            }
        }
    });
    Ok(ApiService {
        state,
        #[cfg(test)]
        local_addr,
        shutdown,
        handle: Some(handle),
    })
}

fn load_tls_server_config(
    certificate_path: &Path,
    private_key_path: &Path,
) -> Result<ServerConfig, String> {
    let certificate_file = open_bounded_regular_file(certificate_path, 1024 * 1024)
        .map_err(|_| "Gewyvern TLS certificate must be a bounded regular file".to_string())?;
    let private_key_file = open_bounded_regular_file(private_key_path, 64 * 1024)
        .map_err(|_| "Gewyvern TLS private key must be a bounded regular file".to_string())?;
    #[cfg(unix)]
    if private_key_file
        .metadata()
        .map_err(|_| "Gewyvern TLS private key metadata is unavailable".to_string())?
        .permissions()
        .mode()
        & 0o077
        != 0
    {
        return Err("Gewyvern TLS private key must not be accessible by group or others".into());
    }
    let certificates = CertificateDer::pem_reader_iter(&mut BufReader::new(certificate_file))
        .collect::<Result<Vec<_>, _>>()
        .map_err(|_| "Gewyvern TLS certificate contains invalid PEM".to_string())?;
    if certificates.is_empty() {
        return Err("Gewyvern TLS certificate contains no certificates".into());
    }
    let private_key = PrivateKeyDer::from_pem_reader(&mut BufReader::new(private_key_file))
        .map_err(|_| "Gewyvern TLS private key contains invalid PEM".to_string())?;
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let mut config = ServerConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|_| "Gewyvern TLS protocol configuration failed".to_string())?
        .with_no_client_auth()
        .with_single_cert(certificates, private_key)
        .map_err(|_| "Gewyvern TLS certificate and private key do not match".to_string())?;
    config.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(config)
}

fn validate_api_bind_addr(bind_addr: &str, access_policy: &ApiAccessPolicy) -> Result<(), String> {
    let resolved = bind_addr
        .to_socket_addrs()
        .map_err(|err| format!("failed to resolve api bind address '{bind_addr}': {err}"))?
        .collect::<Vec<_>>();
    if resolved.is_empty() {
        return Err(format!(
            "api bind address '{bind_addr}' did not resolve to any socket addresses"
        ));
    }
    if resolved
        .iter()
        .any(|socket_addr| !api_client_is_loopback(socket_addr.ip()))
    {
        if !access_policy.allow_remote_bind {
            return Err(format!(
                "api bind address '{bind_addr}' is not loopback-only; pass --allow-remote-api for explicit remote exposure"
            ));
        }
        if access_policy.admin_token.is_none() {
            return Err(format!(
                "api bind address '{bind_addr}' is remote; set GEWY_API_ADMIN_TOKEN, runtime.api_admin_token, or pass --api-admin-token before exposing the runtime API"
            ));
        }
    }
    Ok(())
}

struct ActiveApiClientGuard(Arc<AtomicUsize>);

impl Drop for ActiveApiClientGuard {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::AcqRel);
    }
}

fn reject_api_client_overload(mut stream: TcpStream) {
    let _ = stream.set_write_timeout(Some(API_CLIENT_WRITE_TIMEOUT));
    let _ = write!(stream, "{}", service_busy_response());
}

fn service_busy_response() -> &'static str {
    "HTTP/1.1 503 Service Unavailable\r\nContent-Type: application/json; charset=utf-8\r\nContent-Length: 55\r\nConnection: close\r\n\r\n{\"error\":\"service_busy\",\"retry_after\":\"short_backoff\"}"
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::{Read, Write};
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    use rcgen::{CertifiedKey, generate_simple_self_signed};
    use rustls::pki_types::{CertificateDer, ServerName, pem::PemObject};
    use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

    use super::*;

    fn https_request(address: std::net::SocketAddr, ca_pem: &str, token: &str) -> String {
        let certificate = CertificateDer::pem_slice_iter(ca_pem.as_bytes())
            .next()
            .unwrap()
            .unwrap();
        let mut roots = RootCertStore::empty();
        roots.add(certificate).unwrap();
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let config = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
        let socket = TcpStream::connect(address).unwrap();
        let connection = ClientConnection::new(
            Arc::new(config),
            ServerName::try_from("localhost").unwrap().to_owned(),
        )
        .unwrap();
        let mut stream = StreamOwned::new(connection, socket);
        write!(
            stream,
            "GET /health HTTP/1.1\r\nHost: localhost\r\nX-Gewyvern-Admin-Token: {token}\r\nConnection: close\r\n\r\n"
        )
        .unwrap();
        stream.flush().unwrap();
        let mut response = Vec::new();
        let mut chunk = [0_u8; 1024];
        let mut expected_length = None;
        loop {
            let read = match stream.read(&mut chunk) {
                Ok(read) => read,
                Err(error)
                    if error.kind() == std::io::ErrorKind::UnexpectedEof
                        && !response.is_empty() =>
                {
                    break;
                }
                Err(error) => panic!(
                    "HTTPS response read failed after {} bytes: {error}",
                    response.len()
                ),
            };
            assert_ne!(read, 0, "HTTPS response ended before its declared body");
            response.extend_from_slice(&chunk[..read]);
            if expected_length.is_none()
                && let Some(header_start) =
                    response.windows(4).position(|window| window == b"\r\n\r\n")
            {
                let header_end = header_start + 4;
                let headers = std::str::from_utf8(&response[..header_start]).unwrap();
                let body_length = headers
                    .lines()
                    .find_map(|line| {
                        line.split_once(':').and_then(|(name, value)| {
                            name.eq_ignore_ascii_case("content-length")
                                .then(|| value.trim().parse::<usize>().unwrap())
                        })
                    })
                    .unwrap();
                expected_length = Some(header_end + body_length);
            }
            if expected_length.is_some_and(|length| response.len() >= length) {
                response.truncate(expected_length.unwrap());
                break;
            }
        }
        let response = String::from_utf8(response).unwrap();
        let (headers, body) = response.split_once("\r\n\r\n").unwrap();
        let content_length = headers
            .lines()
            .find_map(|line| {
                line.split_once(':').and_then(|(name, value)| {
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().unwrap())
                })
            })
            .unwrap();
        assert_eq!(body.len(), content_length);
        response
    }

    #[test]
    fn active_api_client_guard_releases_slot_on_drop() {
        let counter = Arc::new(AtomicUsize::new(1));
        {
            let _guard = ActiveApiClientGuard(Arc::clone(&counter));
            counter.fetch_add(1, Ordering::AcqRel);
        }
        assert_eq!(counter.load(Ordering::Acquire), 1);
    }

    #[test]
    fn overload_rejection_uses_503_service_busy_response() {
        let response = service_busy_response();
        assert!(response.starts_with("HTTP/1.1 503 Service Unavailable"));
        assert!(response.contains("\"error\":\"service_busy\""));
        assert!(response.contains("\"retry_after\":\"short_backoff\""));
    }

    #[test]
    fn api_bind_guard_rejects_remote_bind_without_explicit_flag() {
        let err = validate_api_bind_addr(
            "0.0.0.0:9100",
            &ApiAccessPolicy {
                allow_remote_bind: false,
                admin_token: Some("secret-token".into()),
                require_token: false,
            },
        )
        .expect_err("remote bind should require explicit flag");
        assert!(err.contains("--allow-remote-api"));
    }

    #[test]
    fn api_bind_guard_rejects_remote_bind_without_token() {
        let err = validate_api_bind_addr(
            "0.0.0.0:9100",
            &ApiAccessPolicy {
                allow_remote_bind: true,
                admin_token: None,
                require_token: false,
            },
        )
        .expect_err("remote bind should require admin token");
        assert!(err.contains("GEWY_API_ADMIN_TOKEN"));
    }

    #[test]
    fn api_bind_guard_accepts_remote_bind_with_explicit_flag_and_token() {
        validate_api_bind_addr(
            "0.0.0.0:9100",
            &ApiAccessPolicy {
                allow_remote_bind: true,
                admin_token: Some("secret-token".into()),
                require_token: false,
            },
        )
        .expect("remote bind should succeed with explicit flag and token");
    }

    #[test]
    fn tls_api_requires_the_configured_token_even_from_loopback() {
        let root = std::env::temp_dir().join(format!(
            "gewyvern-tls-api-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir(&root).unwrap();
        let CertifiedKey { cert, signing_key } =
            generate_simple_self_signed(vec!["localhost".into()]).unwrap();
        let certificate = root.join("server.crt");
        let private_key = root.join("server.key");
        fs::write(&certificate, cert.pem()).unwrap();
        fs::write(&private_key, signing_key.serialize_pem()).unwrap();
        #[cfg(unix)]
        fs::set_permissions(&private_key, fs::Permissions::from_mode(0o600)).unwrap();
        let token = "0123456789abcdef0123456789abcdef";
        let service =
            start_tls_api_service("127.0.0.1:0", &certificate, &private_key, token).unwrap();

        let denied = https_request(
            service.local_addr(),
            &cert.pem(),
            "abcdef0123456789abcdef0123456789",
        );
        assert!(denied.starts_with("HTTP/1.1 403 Forbidden"));
        let ready = https_request(service.local_addr(), &cert.pem(), token);
        assert!(ready.starts_with("HTTP/1.1 200 OK"));
        assert!(ready.contains("\"ok\":true"));

        drop(service);
        fs::remove_dir_all(root).unwrap();
    }
}
