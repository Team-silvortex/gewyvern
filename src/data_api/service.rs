use std::io::Write;
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::thread;
use std::thread::JoinHandle;
use std::time::Duration;

use super::{
    API_CLIENT_WRITE_TIMEOUT, API_MAX_CONCURRENT_CLIENTS, ApiAccessPolicy, ApiSnapshot, ApiState,
    EVENT_API_CLIENT_ACCEPT_FAILED, EVENT_API_CLIENT_OVERLOAD_REJECTED,
    EVENT_API_LISTENER_BIND_FAILED, api_client_is_loopback, handle_api_client, log_error_event,
    log_warn_event,
};

pub struct ApiService {
    state: ApiState,
    shutdown: Arc<AtomicBool>,
    handle: Option<JoinHandle<()>>,
}

impl ApiService {
    pub fn state(&self) -> &ApiState {
        &self.state
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

pub fn start_api_service(addr: &str, allow_remote_bind: bool) -> ApiService {
    let access_policy = ApiAccessPolicy::from_env(allow_remote_bind);
    validate_api_bind_addr(addr, &access_policy).unwrap_or_else(|message| {
        log_error_event(
            "api",
            EVENT_API_LISTENER_BIND_FAILED,
            &[("socket", addr.to_string()), ("error", message.clone())],
            "refused unsafe api listener bind",
        );
        eprintln!("{message}");
        std::process::exit(1);
    });
    let listener = TcpListener::bind(addr).unwrap_or_else(|err| {
        log_error_event(
            "api",
            EVENT_API_LISTENER_BIND_FAILED,
            &[("socket", addr.to_string()), ("error", err.to_string())],
            "failed to bind api listener",
        );
        eprintln!("failed to bind api socket {}: {}", addr, err);
        std::process::exit(1);
    });
    listener.set_nonblocking(true).unwrap_or_else(|err| {
        log_error_event(
            "api",
            EVENT_API_LISTENER_BIND_FAILED,
            &[("socket", addr.to_string()), ("error", err.to_string())],
            "failed to configure api listener",
        );
        eprintln!("failed to configure api socket {}: {}", addr, err);
        std::process::exit(1);
    });
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
                Ok((stream, remote_addr)) => {
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
                        reject_api_client_overload(stream);
                        continue;
                    }
                    let client_state = Arc::clone(&thread_state);
                    let client_deployments = Arc::clone(&thread_deployments);
                    let client_counter = Arc::clone(&active_clients);
                    let client_access_policy = thread_access_policy.clone();
                    thread::spawn(move || {
                        let _guard = ActiveApiClientGuard(client_counter);
                        handle_api_client(
                            stream,
                            remote_addr.ip(),
                            client_state,
                            client_deployments,
                            client_access_policy,
                        );
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
    ApiService {
        state,
        shutdown,
        handle: Some(handle),
    }
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
    use super::*;

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
            },
        )
        .expect("remote bind should succeed with explicit flag and token");
    }
}
