use super::{ErrorCategory, MachineError};
use std::path::PathBuf;

pub(super) const HELP: &str = "gewyvern capture-http-status --capture-cgroup <absolute-client-cgroup-v2> --capture-tcp-port <port> --capture-seconds <1..300> [--json]\n  Linux IPv4 loopback only. JSON: first HTTP/1.1 response status per new connection, never headers or body. 64 connections maximum; exit 3 means incomplete.";

pub(super) const REDIS_HELP: &str = "gewyvern capture-redis-resp --capture-cgroup <absolute-client-cgroup-v2> --capture-tcp-port <port> --capture-seconds <1..300> [--json]\n  Linux IPv4 loopback only. JSON: ordered RESP2 reply types, lengths and array error counts, never commands or values. 64 connections, 32 replies each, up to two array levels; exit 3 means incomplete.";
#[derive(Clone, Copy)]
pub(super) enum Mode {
    Http,
    Redis,
    Postgres,
    Memcached,
    Mysql,
    Mqtt,
}
pub(super) const MQTT_HELP: &str = "gewyvern capture-mqtt --capture-cgroup <absolute-client-cgroup-v2> --capture-tcp-port <port> --capture-seconds <1..300> [--json]\n  Linux IPv4 loopback, plaintext MQTT 3.1.1 broker frames. JSON: session-present, QoS acknowledgements, packet IDs and publish lengths; no topics or payloads. 32 events/socket; exit 3 means incomplete.";
pub(super) const MYSQL_HELP: &str = "gewyvern capture-mysql --capture-cgroup <absolute-client-cgroup-v2> --capture-tcp-port <port> --capture-seconds <1..300> [--json]\n  Linux IPv4 loopback, plaintext direct-auth classic MySQL: one-column text rows and legacy EOF. Reports error phase, number and SQLSTATE bucket; no SQL or values. 32 events per socket; exit 3 means incomplete.";
pub(super) const MC_HELP: &str = "gewyvern capture-memcached --capture-cgroup <absolute-client-cgroup-v2> --capture-tcp-port <port> --capture-seconds <1..300> [--json]\n  Linux IPv4 loopback, classic binary replies only. JSON: operation, status and hit length; no keys or values. 64 connections, 32 replies each; exit 3 means incomplete.";
pub(super) const PG_HELP: &str = "gewyvern capture-postgres --capture-cgroup <absolute-client-cgroup-v2> --capture-tcp-port <port> --capture-seconds <1..300> [--json]\n  Plaintext PostgreSQL v3 simple-query backend frames on Linux IPv4 loopback. JSON: rows, ready states and error buckets; no SQL or values. 64 connections, 32 events each; exit 3 means incomplete.";
impl Mode {
    fn help(self) -> &'static str {
        match self {
            Self::Http => HELP,
            Self::Redis => REDIS_HELP,
            Self::Postgres => PG_HELP,
            Self::Memcached => MC_HELP,
            Self::Mysql => MYSQL_HELP,
            Self::Mqtt => MQTT_HELP,
        }
    }
}

fn options(mode: Mode, args: &[String]) -> Result<(PathBuf, u16, u32), MachineError> {
    let invalid = || {
        MachineError::new(
            match mode {
                Mode::Http => "http_status_options_invalid",
                Mode::Redis => "redis_resp_options_invalid",
                Mode::Postgres => "postgres_options_invalid",
                Mode::Memcached => "memcached_options_invalid",
                Mode::Mysql => "mysql_options_invalid",
                Mode::Mqtt => "mqtt_options_invalid",
            },
            ErrorCategory::Input,
            mode.help(),
            false,
            2,
        )
    };
    let (mut scope, mut port, mut seconds, mut json) = (None, None, None, false);
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--capture-cgroup" if scope.is_none() => {
                scope = Some(PathBuf::from(iter.next().ok_or_else(invalid)?))
            }
            "--capture-tcp-port" if port.is_none() => {
                port = Some(
                    iter.next()
                        .and_then(|v| v.parse::<u16>().ok())
                        .filter(|v| *v > 0)
                        .ok_or_else(invalid)?,
                )
            }
            "--capture-seconds" if seconds.is_none() => {
                seconds = Some(
                    iter.next()
                        .and_then(|v| v.parse::<u32>().ok())
                        .filter(|v| (1..=300).contains(v))
                        .ok_or_else(invalid)?,
                )
            }
            "--json" if !json => json = true,
            _ => return Err(invalid()),
        }
    }
    Ok((
        scope.filter(|v| v.is_absolute()).ok_or_else(invalid)?,
        port.ok_or_else(invalid)?,
        seconds.ok_or_else(invalid)?,
    ))
}

pub(super) fn run(mode: Mode, args: &[String]) -> Result<i32, MachineError> {
    if matches!(args, [x] if x == "--help" || x == "-h") {
        println!("{}", mode.help());
        return Ok(0);
    }
    let opts = options(mode, args)?;
    #[cfg(target_os = "linux")]
    {
        use std::sync::{Arc, atomic::AtomicBool};
        let stop = Arc::new(AtomicBool::new(false));
        let signals = [libc::SIGINT, libc::SIGTERM]
            .map(|s| signal_hook::flag::register(s, Arc::clone(&stop)));
        let result = (|| {
            for signal in &signals {
                if let Err(e) = signal {
                    return Err(MachineError::new(
                        "capture_signal_failed",
                        ErrorCategory::Environment,
                        e.to_string(),
                        false,
                        1,
                    ));
                }
            }
            use gewyvern::live_capture::{observe_http_status, observe_redis_resp};
            let (report, incomplete) = match mode {
                Mode::Mqtt => {
                    gewyvern::live_capture::observe_mqtt(&opts.0, opts.1, opts.2, &stop, || {
                        eprintln!("capture-mqtt: ready")
                    })
                }
                Mode::Mysql => {
                    gewyvern::live_capture::observe_mysql(&opts.0, opts.1, opts.2, &stop, || {
                        eprintln!("capture-mysql: ready")
                    })
                }
                Mode::Memcached => gewyvern::live_capture::observe_memcached(
                    &opts.0,
                    opts.1,
                    opts.2,
                    &stop,
                    || eprintln!("capture-memcached: ready"),
                ),
                Mode::Http => observe_http_status(&opts.0, opts.1, opts.2, &stop, || {
                    eprintln!("capture-http-status: ready")
                }),
                Mode::Redis => observe_redis_resp(&opts.0, opts.1, opts.2, &stop, || {
                    eprintln!("capture-redis-resp: ready")
                }),
                Mode::Postgres => {
                    gewyvern::live_capture::observe_postgres(&opts.0, opts.1, opts.2, &stop, || {
                        eprintln!("capture-postgres: ready")
                    })
                }
            }
            .map_err(|e| {
                MachineError::new(e.code, ErrorCategory::Environment, e.detail, true, 1)
            })?;
            println!("{report}");
            Ok(if incomplete { 3 } else { 0 })
        })();
        for id in signals.into_iter().flatten() {
            signal_hook::low_level::unregister(id);
        }
        result
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = opts;
        Err(MachineError::new(
            "capture_platform_unsupported",
            ErrorCategory::Environment,
            "TCP response capture requires Linux",
            false,
            2,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_explicit_scope_port_and_bounded_duration() {
        let parse = |s: &str| {
            options(
                Mode::Http,
                &s.split_whitespace().map(str::to_owned).collect::<Vec<_>>(),
            )
        };
        let base =
            "--capture-seconds 3 --capture-cgroup /sys/fs/cgroup/private --capture-tcp-port 8080";
        assert_eq!(parse(base).unwrap().1, 8080);
        for bad in [
            base.replace("/sys/fs/cgroup/private", "relative"),
            base.replace("8080", "0"),
            base.replace("seconds 3", "seconds 301"),
            format!("{base} --capture-seconds 4"),
            format!("{base} --serve"),
            "--json".into(),
        ] {
            assert!(parse(&bad).is_err());
        }
    }
}
