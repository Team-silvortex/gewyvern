use super::*;
use std::fs;
use std::net::{TcpListener, TcpStream};
use std::path::Path;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

mod cli;
mod core;
mod daemon;
mod federation;

fn default_worker_args() -> Vec<String> {
    vec![
        "--python-worker".to_string(),
        default_python_worker_script().to_string_lossy().to_string(),
    ]
}

fn fixture(name: &str) -> String {
    fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests")
            .join("fixtures")
            .join(name),
    )
    .expect("fixture should read")
}

fn reserve_bind_addr() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("listener should bind");
    let addr = listener
        .local_addr()
        .expect("listener should have local addr");
    drop(listener);
    addr.to_string()
}

fn daemon_test_guard() -> &'static Mutex<()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(()))
}

fn lock_daemon_test_guard() -> std::sync::MutexGuard<'static, ()> {
    daemon_test_guard()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn wait_for_body<F>(url: &str, predicate: F) -> Option<String>
where
    F: Fn(&str) -> bool,
{
    for _ in 0..4800 {
        if let Ok(body) = read_url(url) {
            if predicate(&body) {
                return Some(body);
            }
        }
        thread::sleep(Duration::from_millis(25));
    }
    None
}

fn wait_for_daemon_health(bind_addr: &str) -> Option<String> {
    wait_for_body(&format!("http://{}/health", bind_addr), |body| {
        body.contains("\"status\":\"ok\"")
    })
}

fn wait_for_daemon_ready(bind_addr: &str) -> Option<String> {
    wait_for_body(&format!("http://{}/v1/latest/status", bind_addr), |body| {
        body.contains("\"status\":\"ready\"") && !body.contains("\"last_success_unix_ms\":null")
    })
}

fn post_json(url: &str, body: &str) -> Result<String, String> {
    let (host, port, path) = parse_http_url(url)?;
    let mut stream = TcpStream::connect((host.as_str(), port))
        .map_err(|err| format!("failed to connect to {}:{}: {err}", host, port))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|err| format!("failed to configure read timeout: {err}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|err| format!("failed to configure write timeout: {err}"))?;
    let request = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nConnection: close\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
        path,
        host,
        body.len(),
        body
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|err| format!("failed to send request: {err}"))?;
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .map_err(|err| format!("failed to read response: {err}"))?;
    let (head, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| "invalid HTTP response: missing header separator".to_string())?;
    let status_line = head
        .lines()
        .next()
        .ok_or_else(|| "invalid HTTP response: missing status line".to_string())?;
    if !status_line.contains(" 200 ") {
        return Err(format!("unexpected HTTP response: {status_line}"));
    }
    Ok(body.to_string())
}
