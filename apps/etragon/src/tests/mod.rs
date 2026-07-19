use super::*;
use std::fs;
use std::fs::OpenOptions;
use std::net::{TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::{
    Mutex, OnceLock,
    atomic::{AtomicU16, Ordering},
};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

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
    static NEXT_TEST_PORT: AtomicU16 = AtomicU16::new(43000);
    for _ in 0..1024 {
        let port = NEXT_TEST_PORT.fetch_add(1, Ordering::Relaxed);
        let addr = format!("127.0.0.1:{port}");
        if let Ok(listener) = TcpListener::bind(&addr) {
            drop(listener);
            return addr;
        }
    }
    panic!("failed to reserve a daemon test bind address after scanning 1024 ports");
}

fn daemon_test_guard() -> &'static Mutex<()> {
    static GUARD: OnceLock<Mutex<()>> = OnceLock::new();
    GUARD.get_or_init(|| Mutex::new(()))
}

struct DaemonTestGuard {
    _local_guard: std::sync::MutexGuard<'static, ()>,
    lock_path: PathBuf,
}

impl Drop for DaemonTestGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.lock_path);
    }
}

fn daemon_test_lock_path() -> PathBuf {
    std::env::temp_dir().join("gewyvern-etragon-daemon-tests.lock")
}

fn daemon_test_lock_is_stale(lock_path: &Path) -> bool {
    fs::metadata(lock_path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|modified| SystemTime::now().duration_since(modified).ok())
        // Some daemon tests intentionally allow several minutes for worker
        // convergence. Never steal a lock from a test that is still active.
        .map(|age| age > Duration::from_secs(10 * 60))
        .unwrap_or(false)
}

fn lock_daemon_test_guard() -> DaemonTestGuard {
    let local_guard = daemon_test_guard()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let lock_path = daemon_test_lock_path();
    let deadline = std::time::Instant::now() + Duration::from_secs(30);
    loop {
        match OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&lock_path)
        {
            Ok(_) => {
                return DaemonTestGuard {
                    _local_guard: local_guard,
                    lock_path,
                };
            }
            Err(err)
                if err.kind() == std::io::ErrorKind::AlreadyExists
                    && std::time::Instant::now() < deadline =>
            {
                if daemon_test_lock_is_stale(&lock_path) {
                    let _ = fs::remove_file(&lock_path);
                    continue;
                }
                thread::sleep(Duration::from_millis(25));
            }
            Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                panic!(
                    "timed out waiting for cross-process daemon test lock '{}': {}",
                    lock_path.display(),
                    err
                );
            }
            Err(err) => {
                panic!(
                    "failed to acquire cross-process daemon test lock '{}': {}",
                    lock_path.display(),
                    err
                );
            }
        }
    }
}

fn wait_for_body<F>(url: &str, predicate: F) -> Option<String>
where
    F: Fn(&str) -> bool,
{
    for _ in 0..4800 {
        if let Ok(body) = read_url(url)
            && predicate(&body)
        {
            return Some(body);
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
