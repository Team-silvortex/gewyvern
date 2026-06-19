use std::thread;
use std::time::Duration;

use crate::runtime_events::EVENT_SOCKET_SERVICE_RECOVERED;
use crate::runtime_logging::{log_error_event, log_info_event};

const DEFAULT_SOCKET_FAILURE_BACKOFF_BASE_MS: u64 = 100;
const DEFAULT_SOCKET_FAILURE_BACKOFF_CAP_MS: u64 = 2_000;
const SOCKET_FAILURE_BACKOFF_BASE_ENV: &str = "GEWY_SOCKET_FAILURE_BACKOFF_BASE_MS";
const SOCKET_FAILURE_BACKOFF_CAP_ENV: &str = "GEWY_SOCKET_FAILURE_BACKOFF_CAP_MS";

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct SocketResilienceStatus {
    pub(crate) consecutive_failures: usize,
    pub(crate) total_failures: usize,
    pub(crate) current_backoff_ms: u128,
    pub(crate) backoff_base_ms: u64,
    pub(crate) backoff_cap_ms: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct SocketFailureReport {
    pub(super) consecutive_failures: usize,
    pub(super) total_failures: usize,
    pub(super) backoff: Option<Duration>,
}

#[derive(Default)]
pub(crate) struct SocketLoopHealth {
    consecutive_failures: usize,
    total_failures: usize,
}

impl SocketLoopHealth {
    pub(crate) fn record_failure(&mut self) -> SocketFailureReport {
        self.consecutive_failures += 1;
        self.total_failures += 1;
        let report = SocketFailureReport {
            consecutive_failures: self.consecutive_failures,
            total_failures: self.total_failures,
            backoff: socket_failure_backoff(self.consecutive_failures),
        };
        publish_socket_resilience_failure(report);
        report
    }

    pub(crate) fn record_success(&mut self) -> Option<usize> {
        let recovered = self.consecutive_failures;
        self.consecutive_failures = 0;
        publish_socket_resilience_success(self.total_failures);
        if recovered > 0 {
            Some(recovered)
        } else {
            None
        }
    }
}

pub(super) fn log_socket_session_failure(
    event: &str,
    transport: &str,
    endpoint: &str,
    error: &str,
    report: SocketFailureReport,
) {
    let mut fields = vec![
        ("transport", transport.to_string()),
        ("endpoint", endpoint.to_string()),
        ("error", error.to_string()),
        (
            "consecutive_failures",
            report.consecutive_failures.to_string(),
        ),
        ("total_failures", report.total_failures.to_string()),
    ];
    if let Some(backoff) = report.backoff {
        fields.push(("backoff_ms", backoff.as_millis().to_string()));
    }
    log_error_event("serve", event, &fields, "socket session failure");
}

pub(super) fn log_socket_loop_recovered(
    transport: &str,
    endpoint: &str,
    recovered_after_failures: usize,
) {
    log_info_event(
        "serve",
        EVENT_SOCKET_SERVICE_RECOVERED,
        &[
            ("transport", transport.to_string()),
            ("endpoint", endpoint.to_string()),
            (
                "recovered_after_failures",
                recovered_after_failures.to_string(),
            ),
        ],
        "socket service recovered after prior session failures",
    );
}

pub(super) fn apply_socket_failure_backoff(report: SocketFailureReport) {
    if let Some(backoff) = report.backoff {
        thread::sleep(backoff);
    }
}

fn socket_failure_counter() -> &'static std::sync::atomic::AtomicUsize {
    static COUNTER: std::sync::OnceLock<std::sync::atomic::AtomicUsize> = std::sync::OnceLock::new();
    COUNTER.get_or_init(|| std::sync::atomic::AtomicUsize::new(0))
}

fn socket_total_failure_counter() -> &'static std::sync::atomic::AtomicUsize {
    static COUNTER: std::sync::OnceLock<std::sync::atomic::AtomicUsize> = std::sync::OnceLock::new();
    COUNTER.get_or_init(|| std::sync::atomic::AtomicUsize::new(0))
}

fn socket_current_backoff_ms() -> &'static std::sync::atomic::AtomicUsize {
    static COUNTER: std::sync::OnceLock<std::sync::atomic::AtomicUsize> = std::sync::OnceLock::new();
    COUNTER.get_or_init(|| std::sync::atomic::AtomicUsize::new(0))
}

fn publish_socket_resilience_failure(report: SocketFailureReport) {
    use std::sync::atomic::Ordering;
    socket_failure_counter().store(report.consecutive_failures, Ordering::Release);
    socket_total_failure_counter().store(report.total_failures, Ordering::Release);
    socket_current_backoff_ms().store(
        report.backoff.map(|value| value.as_millis() as usize).unwrap_or(0),
        Ordering::Release,
    );
}

fn publish_socket_resilience_success(total_failures: usize) {
    use std::sync::atomic::Ordering;
    socket_failure_counter().store(0, Ordering::Release);
    socket_total_failure_counter().store(total_failures, Ordering::Release);
    socket_current_backoff_ms().store(0, Ordering::Release);
}

pub(crate) fn current_socket_resilience_status() -> SocketResilienceStatus {
    use std::sync::atomic::Ordering;
    SocketResilienceStatus {
        consecutive_failures: socket_failure_counter().load(Ordering::Acquire),
        total_failures: socket_total_failure_counter().load(Ordering::Acquire),
        current_backoff_ms: socket_current_backoff_ms().load(Ordering::Acquire) as u128,
        backoff_base_ms: socket_failure_backoff_base_ms(),
        backoff_cap_ms: socket_failure_backoff_cap_ms(),
    }
}

#[cfg(test)]
pub(crate) fn reset_socket_resilience_status() {
    use std::sync::atomic::Ordering;
    socket_failure_counter().store(0, Ordering::Release);
    socket_total_failure_counter().store(0, Ordering::Release);
    socket_current_backoff_ms().store(0, Ordering::Release);
}

fn socket_failure_backoff(consecutive_failures: usize) -> Option<Duration> {
    if consecutive_failures < 2 {
        return None;
    }
    let base_ms = socket_failure_backoff_base_ms();
    let cap_ms = socket_failure_backoff_cap_ms().max(base_ms);
    let exponent = (consecutive_failures - 2).min(8) as u32;
    let backoff_ms = (base_ms << exponent).min(cap_ms);
    Some(Duration::from_millis(backoff_ms))
}

fn socket_failure_backoff_base_ms() -> u64 {
    std::env::var(SOCKET_FAILURE_BACKOFF_BASE_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SOCKET_FAILURE_BACKOFF_BASE_MS)
}

fn socket_failure_backoff_cap_ms() -> u64 {
    std::env::var(SOCKET_FAILURE_BACKOFF_CAP_ENV)
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(DEFAULT_SOCKET_FAILURE_BACKOFF_CAP_MS)
}

#[cfg(test)]
mod tests {
    use super::{SocketLoopHealth, socket_failure_backoff};
    use std::time::Duration;

    #[test]
    fn socket_failure_backoff_grows_and_caps() {
        assert_eq!(socket_failure_backoff(1), None);
        assert_eq!(socket_failure_backoff(2), Some(Duration::from_millis(100)));
        assert_eq!(socket_failure_backoff(3), Some(Duration::from_millis(200)));
        assert_eq!(socket_failure_backoff(6), Some(Duration::from_millis(1600)));
        assert_eq!(socket_failure_backoff(7), Some(Duration::from_millis(2000)));
        assert_eq!(socket_failure_backoff(12), Some(Duration::from_millis(2000)));
    }

    #[test]
    fn loop_health_resets_after_success() {
        let mut health = SocketLoopHealth::default();
        let first = health.record_failure();
        let second = health.record_failure();
        assert_eq!(first.consecutive_failures, 1);
        assert_eq!(second.consecutive_failures, 2);
        assert_eq!(health.record_success(), Some(2));
        let next = health.record_failure();
        assert_eq!(next.consecutive_failures, 1);
    }
}
