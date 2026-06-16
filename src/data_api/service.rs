use std::io::Write;
use std::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use super::{
    API_CLIENT_WRITE_TIMEOUT, API_MAX_CONCURRENT_CLIENTS, ApiSnapshot, ApiState,
    EVENT_API_CLIENT_ACCEPT_FAILED, EVENT_API_CLIENT_OVERLOAD_REJECTED,
    EVENT_API_LISTENER_BIND_FAILED, handle_api_client, log_error_event, log_warn_event,
};

pub fn start_api_service(addr: &str) -> ApiState {
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
    let state = Arc::new(std::sync::Mutex::new(Arc::new(ApiSnapshot::default())));
    let thread_state = Arc::clone(&state);
    let active_clients = Arc::new(AtomicUsize::new(0));
    thread::spawn(move || {
        for stream in listener.incoming() {
            match stream {
                Ok(stream) => {
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
                    let client_counter = Arc::clone(&active_clients);
                    thread::spawn(move || {
                        let _guard = ActiveApiClientGuard(client_counter);
                        handle_api_client(stream, client_state);
                    });
                }
                Err(err) => {
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
    state
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
}
