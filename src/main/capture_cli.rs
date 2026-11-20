use gewyvern::live_capture::CaptureOptions;
use std::path::PathBuf;

pub(crate) const HELP: &str = "Live Linux UDP capture: --capture-cgroup <non-root-cgroup-v2> --capture-udp-port <port> --out <absolute-latest-snapshot> [--capture-seconds <1..86400> | --serve] [--capture-window-ms <10..60000>] [--capture-max-facts <1..65536>] [--capture-ring-kib <4..16384-power-of-two>] [--capture-status <absolute-path>]. IPv4/IPv6 direct UDP only; root/BPF privileges, clang and libbpf.so.1 required. Exit 3 means accounted but incomplete capture. --serve uses bounded independent windows; --api-socket publishes the latest export.";

#[derive(Debug)]
pub(crate) struct CaptureArgs {
    pub cgroup: Option<PathBuf>,
    pub port: u16,
    pub seconds: Option<u64>,
    pub window_ms: u64,
    pub max_facts: usize,
    pub ring_kib: u32,
    pub status: Option<PathBuf>,
    touched: bool,
}
impl Default for CaptureArgs {
    fn default() -> Self {
        Self {
            cgroup: None,
            port: 0,
            seconds: None,
            window_ms: 1000,
            max_facts: 4096,
            ring_kib: 1024,
            status: None,
            touched: false,
        }
    }
}
impl CaptureArgs {
    pub fn parse(&mut self, flag: &str, value: String) -> Result<(), String> {
        self.touched = true;
        let invalid = || format!("invalid value for {flag}");
        match flag {
            "--capture-cgroup" => self.cgroup = Some(value.into()),
            "--capture-udp-port" => self.port = value.parse().map_err(|_| invalid())?,
            "--capture-seconds" => self.seconds = Some(value.parse().map_err(|_| invalid())?),
            "--capture-window-ms" => self.window_ms = value.parse().map_err(|_| invalid())?,
            "--capture-max-facts" => self.max_facts = value.parse().map_err(|_| invalid())?,
            "--capture-ring-kib" => self.ring_kib = value.parse().map_err(|_| invalid())?,
            "--capture-status" => self.status = Some(value.into()),
            _ => return Err(format!("unknown capture option {flag}")),
        }
        Ok(())
    }
    pub fn options(&self) -> Result<CaptureOptions, String> {
        let options = CaptureOptions {
            cgroup: self.cgroup.clone().ok_or("--capture-cgroup is required")?,
            udp_port: self.port,
            ring_bytes: self
                .ring_kib
                .checked_mul(1024)
                .ok_or("capture ring size overflow")?,
            max_facts: self.max_facts,
        };
        options.validate().map_err(|e| e.to_string())?;
        Ok(options)
    }
    pub fn validate(&self, serve: bool) -> Result<(), String> {
        if !self.touched {
            return Ok(());
        }
        self.options()?;
        if !(10..=60_000).contains(&self.window_ms)
            || self.seconds.is_some_and(|n| n == 0 || n > 86400)
        {
            return Err("capture window must be 10..60000ms; duration 1..86400 seconds".into());
        }
        if !serve && self.seconds.is_none() {
            return Err(
                "one-shot capture requires --capture-seconds; use --serve for continuous capture"
                    .into(),
            );
        }
        if self.status.as_ref().is_some_and(|path| !path.is_absolute()) {
            return Err("--capture-status must be absolute".into());
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cli_capture_requires_explicit_bounded_nonconflicting_input() {
        let base = [
            "--capture-cgroup",
            "/sys/fs/cgroup/test",
            "--capture-udp-port",
            "53",
            "--capture-seconds",
            "1",
            "--out",
            "/tmp/capture.json",
        ];
        let parse = |extra: &[&str]| {
            crate::Cli::from_args(base.iter().chain(extra).map(|arg| (*arg).to_string()))
        };
        assert!(parse(&[]).is_ok());
        assert!(parse(&["--serve"]).is_ok());
        for extra in [
            vec!["--tcp-socket", "127.0.0.1:9000"],
            vec!["--scan-all"],
            vec!["--template", "tcp"],
            vec!["--out", "relative.json"],
            vec!["--pid", "1"],
            vec!["--capture-max-facts", "0"],
            vec!["--capture-ring-kib", "5"],
            vec!["--capture-window-ms", "0"],
            vec!["--capture-status", "relative.json"],
            vec!["--ingest-mode", "remote-advisory"],
            vec!["--serve", "--max-sessions", "0"],
        ] {
            assert!(parse(&extra).is_err(), "accepted {extra:?}");
        }
    }

    #[test]
    fn capture_never_silently_falls_back_to_demo() {
        let mut args = CaptureArgs::default();
        args.parse("--capture-seconds", "3".into()).unwrap();
        assert!(args.validate(false).is_err());
        args.parse("--capture-cgroup", "/sys/fs/cgroup/test".into())
            .unwrap();
        args.parse("--capture-udp-port", "53".into()).unwrap();
        assert!(args.validate(false).is_ok());
        args.seconds = None;
        assert!(args.validate(false).is_err());
        assert!(args.validate(true).is_ok());
    }
}
