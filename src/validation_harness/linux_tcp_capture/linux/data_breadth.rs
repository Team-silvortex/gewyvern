//! Real private protocol targets -> bounded kernel bytes -> package candidates -> replay.
use super::super::packet_evidence::reconstruct_with_prefix;
use super::bounded_stream::captured_response;
use super::docker_target::{OwnedResource, docker, target_addresses};
use super::http_evidence::sha256;
use super::packet_capture::{Capture, Scope};
use super::{
    ValidationError, ValidationReport, attach_capture_at, export_stage_at, network_namespace,
    owned_socket_on, port, prepare_output, repo_root, snapshot, tool, wait_state,
};
use crate::export::ExportBundle;
use crate::ledger::FactKind;
use serde_json::{Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
mod memcached;
mod mqtt;
mod mysql;
mod nats;
mod pop3;
mod rtsp;
mod smtp;
mod wire;

#[derive(Clone, Copy)]
enum Protocol {
    Postgres,
    Redis,
    Mysql,
    Memcached,
    Mqtt,
    Nats,
    Smtp,
    Pop3,
    Rtsp,
}
impl Protocol {
    fn name(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
            Self::Redis => "redis",
            Self::Mysql => "mysql",
            Self::Memcached => "memcached",
            Self::Mqtt => "mqtt",
            Self::Nats => "nats",
            Self::Smtp => "smtp",
            Self::Pop3 => "pop3",
            Self::Rtsp => "rtsp",
        }
    }
    fn port(self) -> u16 {
        match self {
            Self::Postgres => 5432,
            Self::Redis => 6379,
            Self::Mysql => 3306,
            Self::Memcached => 11211,
            Self::Mqtt => 1883,
            Self::Nats => 4222,
            Self::Smtp => 25,
            Self::Pop3 => 110,
            Self::Rtsp => 554,
        }
    }
    fn image(self) -> &'static str {
        match self {
            Self::Rtsp => rtsp::IMAGE,
            Self::Smtp | Self::Pop3 => smtp::IMAGE,
            Self::Mqtt => {
                "eclipse-mosquitto@sha256:75d6662dee52c947dc9dd9c29f1d8d7ffef78ddaf38001caa09755681e8bbc98"
            }
            Self::Nats => {
                "nats@sha256:558394a23dd3c36f2d3a64b51865fd09908010e80a542cea5e75c2af543ed62a"
            }
            Self::Mysql => {
                "mysql@sha256:7dcddc01f13bab2f15cde676d44d01f61fc9f99fe7785e86196dfc07d358ae2b"
            }
            Self::Memcached => memcached::IMAGE,
            Self::Postgres => {
                "postgres@sha256:57c72fd2a128e416c7fcc499958864df5301e940bca0a56f58fddf30ffc07777"
            }
            Self::Redis => {
                "redis@sha256:7958cfbb9d152f87dadfbc89f1b0b15938f0b81ff8982feb327476f4911935c3"
            }
        }
    }
    fn rules(self) -> Vec<(&'static str, &'static str, &'static str, [bool; 2])> {
        match self {
            Self::Rtsp => vec![
                (
                    "protocols/rtsp/options/main.gewy",
                    include_str!("../../../../protocols/rtsp/options/main.gewy"),
                    "receive_options_ok",
                    [true, true],
                ),
                (
                    "protocols/rtsp/describe/main.gewy",
                    include_str!("../../../../protocols/rtsp/describe/main.gewy"),
                    "receive_describe_ok",
                    [false, true],
                ),
                (
                    "protocols/rtsp/setup/main.gewy",
                    include_str!("../../../../protocols/rtsp/setup/main.gewy"),
                    "receive_setup_ok",
                    [false, true],
                ),
                (
                    "protocols/rtsp/play/main.gewy",
                    include_str!("../../../../protocols/rtsp/play/main.gewy"),
                    "receive_play_ok",
                    [false, true],
                ),
            ],
            Self::Pop3 => vec![
                (
                    "protocols/pop3/auth/main.gewy",
                    include_str!("../../../../protocols/pop3/auth/main.gewy"),
                    "receive_auth_ok",
                    [false, true],
                ),
                (
                    "protocols/pop3/auth-denied/main.gewy",
                    include_str!("../../../../protocols/pop3/auth-denied/main.gewy"),
                    "receive_auth_denied",
                    [true, false],
                ),
                (
                    "protocols/pop3/list/main.gewy",
                    include_str!("../../../../protocols/pop3/list/main.gewy"),
                    "receive_list_ready",
                    [false, true],
                ),
            ],
            Self::Smtp => vec![
                (
                    "protocols/smtp/session/main.gewy",
                    include_str!("../../../../protocols/smtp/session/main.gewy"),
                    "receive_banner",
                    [true, true],
                ),
                (
                    "protocols/smtp/auth/main.gewy",
                    include_str!("../../../../protocols/smtp/auth/main.gewy"),
                    "receive_auth_ok",
                    [false, true],
                ),
                (
                    "protocols/smtp/auth-denied/main.gewy",
                    include_str!("../../../../protocols/smtp/auth-denied/main.gewy"),
                    "receive_auth_denied",
                    [true, false],
                ),
                (
                    "protocols/smtp/mail/main.gewy",
                    include_str!("../../../../protocols/smtp/mail/main.gewy"),
                    "receive_mail_ok",
                    [false, true],
                ),
                (
                    "protocols/smtp/rcpt/main.gewy",
                    include_str!("../../../../protocols/smtp/rcpt/main.gewy"),
                    "receive_rcpt_ok",
                    [false, true],
                ),
                (
                    "protocols/smtp/rcpt-denied/main.gewy",
                    include_str!("../../../../protocols/smtp/rcpt-denied/main.gewy"),
                    "receive_rcpt_denied",
                    [false, true],
                ),
                (
                    "protocols/smtp/data/main.gewy",
                    include_str!("../../../../protocols/smtp/data/main.gewy"),
                    "receive_message_queued",
                    [false, true],
                ),
                (
                    "protocols/smtp/data-denied/main.gewy",
                    include_str!("../../../../protocols/smtp/data-denied/main.gewy"),
                    "receive_message_denied",
                    [false, false],
                ),
            ],
            Self::Mqtt => vec![
                (
                    "protocols/mqtt/connect/main.gewy",
                    include_str!("../../../../protocols/mqtt/connect/main.gewy"),
                    "receive_connack",
                    [false, true],
                ),
                (
                    "protocols/mqtt/connack/main.gewy",
                    include_str!("../../../../protocols/mqtt/connack/main.gewy"),
                    "receive_connack",
                    [true, true],
                ),
                (
                    "protocols/mqtt/publish/main.gewy",
                    include_str!("../../../../protocols/mqtt/publish/main.gewy"),
                    "receive_puback",
                    [false, true],
                ),
                (
                    "protocols/mqtt/pubrec/main.gewy",
                    include_str!("../../../../protocols/mqtt/pubrec/main.gewy"),
                    "receive_pubrec",
                    [false, true],
                ),
                (
                    "protocols/mqtt/pubcomp/main.gewy",
                    include_str!("../../../../protocols/mqtt/pubcomp/main.gewy"),
                    "receive_pubcomp",
                    [false, true],
                ),
                (
                    "protocols/mqtt/subscribe/main.gewy",
                    include_str!("../../../../protocols/mqtt/subscribe/main.gewy"),
                    "receive_suback",
                    [false, true],
                ),
            ],
            Self::Nats => vec![
                (
                    "protocols/nats/connect/main.gewy",
                    include_str!("../../../../protocols/nats/connect/main.gewy"),
                    "receive_info",
                    [true, true],
                ),
                (
                    "protocols/nats/error/main.gewy",
                    include_str!("../../../../protocols/nats/error/main.gewy"),
                    "receive_error",
                    [true, true],
                ),
                (
                    "protocols/nats/sub/main.gewy",
                    include_str!("../../../../protocols/nats/sub/main.gewy"),
                    "receive_message",
                    [false, true],
                ),
            ],
            Self::Mysql => vec![
                (
                    "protocols/mysql/auth-denied/main.gewy",
                    include_str!("../../../../protocols/mysql/auth-denied/main.gewy"),
                    "receive_auth_denied",
                    [true, false],
                ),
                (
                    "protocols/mysql/error/main.gewy",
                    include_str!("../../../../protocols/mysql/error/main.gewy"),
                    "receive_error",
                    [false, true],
                ),
                (
                    "protocols/mysql/auth/main.gewy",
                    include_str!("../../../../protocols/mysql/auth/main.gewy"),
                    "receive_ok",
                    [false, true],
                ),
            ],
            Self::Memcached => vec![
                (
                    "protocols/memcached/get/main.gewy",
                    include_str!("../../../../protocols/memcached/get/main.gewy"),
                    "receive_value",
                    [true, true],
                ),
                (
                    "protocols/memcached/set/main.gewy",
                    include_str!("../../../../protocols/memcached/set/main.gewy"),
                    "receive_stored",
                    [true, true],
                ),
                (
                    "protocols/memcached/miss/main.gewy",
                    include_str!("../../../../protocols/memcached/miss/main.gewy"),
                    "receive_not_found",
                    [true, false],
                ),
                (
                    "protocols/memcached/not-stored/main.gewy",
                    include_str!("../../../../protocols/memcached/not-stored/main.gewy"),
                    "receive_not_stored",
                    [true, false],
                ),
            ],
            Self::Postgres => vec![
                (
                    "protocols/postgres/auth-denied/main.gewy",
                    include_str!("../../../../protocols/postgres/auth-denied/main.gewy"),
                    "receive_auth_denied",
                    [true, false],
                ),
                (
                    "protocols/postgres/error/main.gewy",
                    include_str!("../../../../protocols/postgres/error/main.gewy"),
                    "receive_error",
                    [false, true],
                ),
                (
                    "protocols/postgres/auth/main.gewy",
                    include_str!("../../../../protocols/postgres/auth/main.gewy"),
                    "receive_auth",
                    [true, true],
                ),
            ],
            Self::Redis => vec![
                (
                    "protocols/redis/auth-denied/main.gewy",
                    include_str!("../../../../protocols/redis/auth-denied/main.gewy"),
                    "receive_auth_denied",
                    [true, false],
                ),
                (
                    "protocols/redis/error/main.gewy",
                    include_str!("../../../../protocols/redis/error/main.gewy"),
                    "receive_error",
                    [true, false],
                ),
                (
                    "protocols/redis/get/main.gewy",
                    include_str!("../../../../protocols/redis/get/main.gewy"),
                    "receive_bulk",
                    [true, true],
                ),
            ],
        }
    }
}

fn valid_candidate(
    protocol: Protocol,
    phase: &str,
    packet: &crate::ledger::PacketMetaFact,
) -> bool {
    let at = |offset| {
        packet
            .payload_bytes
            .iter()
            .find_map(|(at, value)| (*at == offset).then_some(*value))
    };
    match protocol {
        Protocol::Rtsp => rtsp::candidate(packet, phase),
        Protocol::Pop3 => {
            let starts = |offset: u16, text: &[u8]| {
                text.iter()
                    .enumerate()
                    .all(|(i, b)| at(offset + i as u16) == Some(*b))
            };
            let text: &[u8] = match phase {
                "receive_banner" => b"+OK Mailpit POP3 server\r\n",
                "receive_user_ok" => b"+OK User accepted\r\n",
                "receive_auth_ok" => b"+OK signed in\r\n",
                "receive_auth_denied" => b"-ERR invalid password\r\n",
                "receive_list_ready" => {
                    return starts(0, b"+OK ")
                        && (1..=3).any(|digits| {
                            (4..4 + digits)
                                .all(|offset| at(offset).is_some_and(|b| b.is_ascii_digit()))
                                && starts(4 + digits, b" messages (")
                        });
                }
                _ => return false,
            };
            starts(0, text)
        }
        Protocol::Smtp => {
            if phase == "receive_rcpt_denied" {
                return [b"550 5.1.0 ", b"550 5.1.1 "].iter().any(|prefix| {
                    prefix
                        .iter()
                        .enumerate()
                        .all(|(offset, value)| at(offset as u16) == Some(*value))
                });
            }
            let prefix: &[u8] = match phase {
                "receive_banner" => b"220 ",
                "receive_ehlo_ok" => b"250-",
                "receive_auth_ok" => b"235 ",
                "receive_auth_denied" => b"535 ",
                "receive_mail_ok" => b"250 2.1.0 ",
                "receive_rcpt_ok" => b"250 2.1.5 ",
                "receive_data_ready" => b"354 ",
                "receive_message_queued" => b"250 2.0.0 Ok: queued as ",
                "receive_message_denied" => b"550 5.7.1 Message rejected",
                _ => return false,
            };
            prefix
                .iter()
                .enumerate()
                .all(|(offset, value)| at(offset as u16) == Some(*value))
        }
        Protocol::Mqtt => match phase {
            "receive_connack" => {
                at(0) == Some(32)
                    && at(1) == Some(2)
                    && matches!(at(2), Some(0..=1))
                    && matches!(at(3), Some(0..=5))
            }
            "receive_puback" | "receive_pubrec" | "receive_pubcomp" => {
                let kind = match phase {
                    "receive_puback" => 64,
                    "receive_pubrec" => 80,
                    _ => 112,
                };
                at(0) == Some(kind)
                    && at(1) == Some(2)
                    && at(2).zip(at(3)).is_some_and(|id| id != (0, 0))
            }
            "receive_suback" => {
                at(0) == Some(144)
                    && at(1) == Some(3)
                    && at(2).zip(at(3)).is_some_and(|id| id != (0, 0))
                    && at(4) == Some(0)
            }
            _ => false,
        },
        Protocol::Nats => {
            let prefix: &[u8] = match phase {
                "receive_info" => b"INFO ",
                "receive_error" => b"-ERR '",
                "receive_message" => b"MSG ",
                _ => return false,
            };
            prefix
                .iter()
                .enumerate()
                .all(|(offset, value)| at(offset as u16) == Some(*value))
        }
        Protocol::Mysql => match phase {
            "receive_auth_denied" => {
                at(4) == Some(255)
                    && at(5) == Some(0x15)
                    && at(6) == Some(4)
                    && (7..13)
                        .zip(b"#28000")
                        .all(|(offset, value)| at(offset) == Some(*value))
            }
            "receive_error" => {
                at(3) == Some(1) && at(4) == Some(255) && at(7) == Some(b'#') && at(12).is_some()
            }
            "receive_ok" => at(3) == Some(2) && at(4) == Some(0),
            _ => false,
        },
        Protocol::Memcached => {
            let status = at(6).zip(at(7)).map(|(a, b)| u16::from_be_bytes([a, b]));
            at(0) == Some(129)
                && at(23).is_some()
                && match phase {
                    "receive_value" => at(1) == Some(0) && at(4) == Some(4) && status == Some(0),
                    "receive_stored" => at(1) == Some(1) && status == Some(0),
                    "receive_not_found" => at(1) == Some(0) && status == Some(1),
                    "receive_not_stored" => matches!(at(1), Some(1 | 14 | 15)) && status == Some(5),
                    _ => false,
                }
        }
        _ => true,
    }
}

fn stage_cookies(
    export: &ExportBundle,
    phase: &str,
    protocol: Protocol,
) -> Result<BTreeSet<u64>, ValidationError> {
    let mut cookies = BTreeSet::new();
    for stage in export
        .program_flows
        .iter()
        .flat_map(|flow| &flow.stages)
        .filter(|stage| {
            stage.phase.as_deref() == Some(phase)
                || (matches!(protocol, Protocol::Smtp | Protocol::Pop3 | Protocol::Rtsp)
                    && stage
                        .phase
                        .as_deref()
                        .is_some_and(|p| p.starts_with("receive_")))
        })
    {
        let packet = export
            .facts
            .iter()
            .find_map(|fact| match &fact.kind {
                FactKind::PacketMeta(packet) if fact.id == stage.at => Some(packet),
                _ => None,
            })
            .ok_or_else(|| ValidationError::new("database/cache stage has no packet provenance"))?;
        let actual_phase = stage.phase.as_deref().unwrap_or(phase);
        if !valid_candidate(protocol, actual_phase, packet) {
            return Err(ValidationError::new(format!(
                "{} {actual_phase} matched the wrong actual packet at fact {:?}, cookie {:?}, bytes {:?}",
                protocol.name(),
                stage.at,
                packet.sk_cookie,
                packet.payload_bytes.iter().take(26).collect::<Vec<_>>()
            )));
        }
        let cookie = packet
            .sk_cookie
            .ok_or_else(|| ValidationError::new("candidate has no owned cookie"))?;
        if actual_phase == phase {
            cookies.insert(cookie);
        }
    }
    Ok(cookies)
}

fn phase(out: &Path, protocol: Protocol) -> Result<Value, ValidationError> {
    let out = prepare_output(Some(out.to_owned()), "data-breadth")?;
    let image_id = tool(
        docker().args(["image", "inspect", protocol.image(), "--format", "{{.Id}}"]),
        &out,
    )?;
    let token = format!(
        "{}-{}",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| ValidationError::new(e.to_string()))?
            .as_nanos()
    );
    let mut network = OwnedResource::create(
        "network",
        &token,
        &["--driver", "bridge", "--internal"],
        &out,
    )?;
    let network_info = network.inspect()?;
    let mut args = vec![
        "--pull=never",
        "--network",
        &network.name,
        "--cpus=1",
        if matches!(protocol, Protocol::Mysql) {
            "--memory=1g"
        } else {
            "--memory=512m"
        },
        "--pids-limit=128",
        "--cap-drop=ALL",
        "--cap-add=CHOWN",
        "--cap-add=DAC_OVERRIDE",
        "--cap-add=FOWNER",
        "--cap-add=SETGID",
        "--cap-add=SETUID",
        "--cap-add=SYS_CHROOT",
        "--cap-add=NET_BIND_SERVICE",
        "--security-opt=no-new-privileges",
    ];
    // Disposable test credentials only. No host account or operator credential enters the fixture.
    let password = format!("POSTGRES_PASSWORD={}", wire::PASSWORD);
    let mysql_password = format!("MYSQL_PASSWORD={}", wire::PASSWORD);
    let mysql_root_password = format!("MYSQL_ROOT_PASSWORD={}", wire::PASSWORD);
    let smtp_password = format!("MP_SMTP_AUTH=gewy:{}", wire::PASSWORD);
    let pop3_password = format!("MP_POP3_AUTH=gewy:{}", wire::PASSWORD);
    match protocol {
        Protocol::Rtsp => args.extend([
            "--read-only",
            "--env",
            "MTX_RTSPADDRESS=:554",
            "--env",
            "MTX_RTSPTRANSPORTS=tcp",
            "--env",
            "MTX_RTMP=false",
            "--env",
            "MTX_HLS=false",
            "--env",
            "MTX_WEBRTC=false",
            "--env",
            "MTX_SRT=false",
            "--env",
            "MTX_MOQ=false",
            "--env",
            "MTX_API=false",
            "--env",
            "MTX_METRICS=false",
            "--env",
            "MTX_PPROF=false",
            "--env",
            "MTX_PLAYBACK=false",
            "--env",
            "MTX_WRITEQUEUESIZE=16",
            "--env",
            "MTX_PATHDEFAULTS_RECORD=false",
        ]),
        Protocol::Smtp | Protocol::Pop3 => args.extend([
            "--env",
            &smtp_password,
            "--tmpfs",
            "/tmp:rw,nosuid,nodev,size=32m",
        ]),
        Protocol::Postgres => args.extend([
            "--env",
            "POSTGRES_USER=gewy",
            "--env",
            "POSTGRES_DB=gewy",
            "--env",
            &password,
            "--env",
            "POSTGRES_HOST_AUTH_METHOD=password",
            "--env",
            "POSTGRES_INITDB_ARGS=--encoding=UTF8 --locale=C",
            "--tmpfs",
            "/var/lib/postgresql/data:rw,nosuid,nodev,size=128m",
        ]),
        Protocol::Redis => args.extend(["--tmpfs", "/data:rw,nosuid,nodev,size=32m"]),
        Protocol::Mysql => args.extend([
            "--env",
            "MYSQL_USER=gewy",
            "--env",
            "MYSQL_DATABASE=gewy",
            "--env",
            &mysql_password,
            "--env",
            &mysql_root_password,
            "--tmpfs",
            "/var/lib/mysql:rw,nosuid,nodev,size=512m",
        ]),
        Protocol::Memcached | Protocol::Mqtt | Protocol::Nats => (),
    }
    if matches!(protocol, Protocol::Pop3) {
        args.extend(["--env", &pop3_password]);
    }
    args.push(protocol.image());
    match protocol {
        Protocol::Rtsp => (),
        Protocol::Smtp | Protocol::Pop3 => args.extend([
            "--smtp",
            "0.0.0.0:25",
            "--listen",
            "127.0.0.1:8025",
            "--database",
            "/tmp/mailpit.db",
            "--smtp-auth-allow-insecure",
            "--smtp-allowed-recipients",
            "@gewy[.]test$",
            "--smtp-disable-rdns",
            "--max",
            "16",
            "--max-message-size",
            "1",
            "--disable-version-check",
        ]),
        Protocol::Mqtt => args.extend(["mosquitto", "-c", "/tmp/gewyvern.conf"]),
        Protocol::Nats => args.extend(["-c", "/tmp/gewyvern.conf"]),
        Protocol::Mysql => args.extend([
            "mysqld",
            "--default-authentication-plugin=mysql_native_password",
            "--performance-schema=OFF",
            "--innodb-buffer-pool-size=32M",
            "--innodb-log-buffer-size=8M",
            "--innodb-redo-log-capacity=8M",
            "--max-connections=16",
            "--skip-log-bin",
        ]),
        Protocol::Memcached => args.extend([
            "memcached",
            "-m",
            "16",
            "-c",
            "64",
            "-t",
            "1",
            "-I",
            "1k",
            "-o",
            "slab_chunk_max=1",
            "-U",
            "0",
            "-B",
            "binary",
        ]),
        Protocol::Postgres => args.extend([
            "postgres",
            "-c",
            "shared_buffers=16MB",
            "-c",
            "max_connections=16",
        ]),
        Protocol::Redis => args.extend([
            "redis-server",
            "--save",
            "",
            "--appendonly",
            "no",
            "--requirepass",
            wire::PASSWORD,
        ]),
    }
    if matches!(protocol, Protocol::Pop3) {
        args.extend(["--pop3", "0.0.0.0:110"]);
    }
    let mut container = OwnedResource::create("container", &token, &args, &out)?;
    let broker_config = match protocol {
        Protocol::Mqtt => Some("listener 1883 0.0.0.0\nallow_anonymous true\npersistence false\nmax_connections 16\nlog_dest stdout\n".to_owned()),
        Protocol::Nats => Some(json!({"port":4222,"max_payload":1024,"max_connections":16,"authorization":{"users":[{"user":"gewy","password":wire::PASSWORD,"permissions":{"publish":{"allow":["gewy.>"]},"subscribe":{"allow":["gewy.>"]}}}]}}).to_string()),
        _ => None,
    };
    if let Some(config) = broker_config {
        let path = out.join("broker.conf");
        fs::write(&path, config)?;
        tool(
            docker()
                .arg("cp")
                .arg(&path)
                .arg(format!("{}:/tmp/gewyvern.conf", container.name)),
            &out,
        )?;
    }
    tool(docker().args(["container", "start", &container.name]), &out)?;
    let container_info = container.inspect()?;
    if container_info["State"]["Running"] != true {
        fs::write(
            out.join("target-startup.log"),
            tool(
                docker().args(["logs", "--tail", "100", &container.name]),
                &out,
            )?,
        )?;
        return Err(ValidationError::new(format!(
            "{} target stopped before capture; inspect target-startup.log and tools.log",
            protocol.name()
        )));
    }
    let volumes: Vec<_> = container_info["Mounts"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|mount| mount["Type"] == "volume")
        .map(|mount| mount["Name"].as_str().unwrap_or_default().to_owned())
        .collect();
    if volumes
        .iter()
        .any(|name| name.len() != 64 || !name.bytes().all(|b| b.is_ascii_hexdigit()))
    {
        return Err(ValidationError::new(
            "database/cache target has an unexpected named volume",
        ));
    }
    let (source, destination) = target_addresses(&container_info, &network_info, &network.name)?;
    let target = SocketAddr::from((destination, protocol.port()));
    let deadline = Instant::now() + Duration::from_secs(60);
    loop {
        let ready = if matches!(protocol, Protocol::Mqtt) {
            mqtt::probe(owned_socket_on(source)?, target)
        } else if matches!(protocol, Protocol::Pop3) {
            pop3::probe(owned_socket_on(source)?, target)
        } else if matches!(protocol, Protocol::Rtsp) {
            rtsp::probe(owned_socket_on(source)?, target)
        } else {
            wire::exchange(owned_socket_on(source)?, target, protocol, true).map(|_| ())
        };
        match ready {
            Ok(()) => break,
            Err(error) if Instant::now() >= deadline => {
                fs::write(
                    out.join("target-startup.log"),
                    tool(
                        docker().args(["logs", "--tail", "100", &container.name]),
                        &out,
                    )?,
                )?;
                return Err(ValidationError::new(format!(
                    "{} readiness failed: {error}",
                    protocol.name()
                )));
            }
            Err(_) => std::thread::sleep(Duration::from_millis(250)),
        }
    }
    if matches!(protocol, Protocol::Mqtt) {
        mqtt::seed(owned_socket_on(source)?, target)?;
    }
    if matches!(protocol, Protocol::Pop3) {
        smtp::seed_pop3(
            owned_socket_on(source)?,
            SocketAddr::from((destination, 25)),
        )?;
    }
    let publisher = if matches!(protocol, Protocol::Rtsp) {
        Some(rtsp::publish(owned_socket_on(source)?, target)?)
    } else {
        None
    };
    let mut scope = Scope::enter(&token)?;
    let sockets = [owned_socket_on(source)?, owned_socket_on(source)?];
    let cookies = [sockets[0].cookie()?, sockets[1].cookie()?];
    let netns = network_namespace()?;
    let expected = [
        (cookies[0], port(&sockets[0])?, protocol.port(), netns),
        (cookies[1], port(&sockets[1])?, protocol.port(), netns),
    ];
    let addresses = (source.octets(), destination.octets());
    let (mut states_capture, clock) = attach_capture_at(cookies, &out, addresses)?;
    let packets_capture = Capture::attach_data_breadth(&scope, &expected, addresses, &out)?;
    let packet_ownership = (
        packets_capture.path().to_owned(),
        packets_capture.link_id()?,
    );
    let mut responses = Vec::new();
    for (index, socket) in sockets.into_iter().enumerate() {
        let (response, code) = wire::exchange(socket, target, protocol, index == 1)?;
        wait_state(
            &states_capture,
            &out,
            &format!("closed-{index}"),
            cookies[index],
            7,
        )?;
        fs::write(out.join(format!("client-response-{index}.bin")), &response)?;
        responses.push((response, code));
    }
    let neighbor = owned_socket_on(source)?;
    let neighbor_cookie = neighbor.cookie()?;
    let (_, neighbor_code) = wire::exchange(neighbor, target, protocol, false)?;
    drop(publisher);
    states_capture.detach();
    let (states, state_counts) = snapshot(&states_capture, &out, "final")?;
    let tcp = export_stage_at(
        &states,
        state_counts,
        &expected,
        clock,
        &out,
        "final",
        addresses,
    )?;
    let (packets, packet_counts) = packets_capture.finish(&out)?;
    if packets.iter().any(|p| p.cookie == neighbor_cookie)
        || states.iter().any(|s| s.cookie == neighbor_cookie)
    {
        return Err(ValidationError::new(
            "unselected database/cache neighbor entered capture",
        ));
    }
    let mut observations = Vec::new();
    for (index, (response, code)) in responses.iter().enumerate() {
        let captured = captured_response(&packets, cookies[index])?;
        if &captured != response || wire::describe(protocol, &captured)? != *code {
            return Err(ValidationError::new(
                "captured database/cache bytes disagree with independent client",
            ));
        }
        observations.push(
            json!({"cookie":cookies[index],"control":index==1,"client_result":code,
            "captured_result":wire::describe(protocol,&captured)?,"response_bytes":response.len(),
            "response_sha256":sha256(response),"captured_sha256":sha256(&captured)}),
        );
    }
    let mut packages = Vec::new();
    let mut package_failures = Vec::new();
    fs::write(
        out.join("client-observations.json"),
        serde_json::to_vec_pretty(&observations)?,
    )?;
    fs::write(
        out.join("packet-events.json"),
        serde_json::to_vec_pretty(&packets)?,
    )?;
    for (index, (path, rules, phase, matches)) in protocol.rules().into_iter().enumerate() {
        if fs::read(repo_root().join(path))? != rules.as_bytes() {
            return Err(ValidationError::new(
                "built-in database/cache rules differ from workspace",
            ));
        }
        let export = reconstruct_with_prefix(&tcp, &states, &packets, clock, rules, 1024)?;
        fs::write(
            out.join(format!("package-{index}-export.json")),
            export.to_json(),
        )?;
        let observed = match stage_cookies(&export, phase, protocol) {
            Ok(cookies) => cookies,
            Err(error) => {
                package_failures.push(error.to_string());
                BTreeSet::new()
            }
        };
        for id in ["route_meta_fragment", "sock_lineage_fragment"] {
            if export
                .attach_report
                .fragments_loaded
                .iter()
                .any(|name| name == id)
                || (export
                    .attach_plan
                    .fragments
                    .iter()
                    .any(|fragment| fragment.id == id)
                    && !export
                        .attach_report
                        .hookpoints_failed
                        .iter()
                        .any(|name| name.starts_with(id)))
            {
                return Err(ValidationError::new(
                    "unobserved protocol fragment reported as loaded",
                ));
            }
        }
        let wanted: BTreeSet<_> = cookies
            .into_iter()
            .zip(matches)
            .filter_map(|(cookie, yes)| yes.then_some(cookie))
            .collect();
        if observed != wanted
            || export.flows.iter().any(|flow| flow.process.is_some())
            || export
                .facts
                .iter()
                .any(|fact| !matches!(fact.kind, FactKind::TcpState(_) | FactKind::PacketMeta(_)))
        {
            package_failures.push(format!(
                "data package {path} phase {phase}: observed {observed:?}, expected {wanted:?}; check packet candidates versus framed response"
            ));
        }
        packages.push(json!({"path":path,"source_sha256":sha256(rules.as_bytes()),"phase":phase,
            "matched_cookies":observed,"replay_identical":true,"facts":export.facts.len(),
            "unobserved_fragments_explicit":true,"process_attribution":false,"request_payload_observed":false}));
    }
    let links: Vec<Value> = serde_json::from_slice(&tool(
        Command::new("bpftool").args(["-j", "link", "show"]),
        &out,
    )?)?;
    if packet_ownership.0.exists() || links.iter().any(|link| link["id"] == packet_ownership.1) {
        return Err(ValidationError::new(
            "database/cache packet link survived cleanup",
        ));
    }
    states_capture.cleanup()?;
    scope.cleanup()?;
    container.cleanup()?;
    for volume in &volumes {
        let remaining = tool(
            docker().args([
                "volume",
                "ls",
                "--quiet",
                "--filter",
                &format!("name=^{volume}$"),
            ]),
            &out,
        )?;
        if !remaining.iter().all(u8::is_ascii_whitespace) {
            return Err(ValidationError::new(
                "owned anonymous target volume survived cleanup",
            ));
        }
    }
    network.cleanup()?;
    let report = json!({"schema_version":2,"protocol":protocol.name(),"ok":package_failures.is_empty(),"image":protocol.image(),
        "image_id":String::from_utf8_lossy(&image_id).trim(),"container_id":container_info["Id"],"network_id":network_info["Id"],
        "internal_network":true,"published_ports":false,"selected_cookies":cookies,
        "unselected_neighbor_cookie":neighbor_cookie,"unselected_neighbor_result":neighbor_code,
        "client_read_chunk_bytes":wire::READ_CHUNK,"client_reads_are_packet_boundaries":false,
        "selected_query_or_command_count":match protocol { Protocol::Postgres => 10, Protocol::Redis => 21, Protocol::Mysql => 5, Protocol::Memcached => 13, Protocol::Mqtt=>9, Protocol::Nats=>16, Protocol::Smtp=>19, Protocol::Pop3=>20, Protocol::Rtsp=>11 },
        "redis_pipeline_commands":if matches!(protocol, Protocol::Redis) {3} else {0},
        "observations":observations,"packages":packages,"packet_counts":packet_counts,
        "tcp_state_counts":state_counts,"owned_resources_removed":true,"anonymous_volume_ids":volumes,
        "anonymous_volumes_removed_with_container":true,"package_failures":package_failures});
    fs::write(out.join("result.json"), serde_json::to_vec_pretty(&report)?)?;
    if !package_failures.is_empty() {
        return Err(ValidationError::new(package_failures.join("; ")));
    }
    Ok(report)
}

pub(super) fn run(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    run_group(out, Group::Data)
}

pub(super) fn run_mysql_memcached(
    out: Option<PathBuf>,
) -> Result<ValidationReport, ValidationError> {
    run_group(out, Group::MysqlMemcached)
}

pub(super) fn run_messaging(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    run_group(out, Group::Messaging)
}
enum Group {
    Data,
    MysqlMemcached,
    Messaging,
    Smtp,
    Pop3,
    Rtsp,
}

pub(super) fn run_smtp(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    run_group(out, Group::Smtp)
}

pub(super) fn run_pop3(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    run_group(out, Group::Pop3)
}

pub(super) fn run_rtsp(out: Option<PathBuf>) -> Result<ValidationReport, ValidationError> {
    run_group(out, Group::Rtsp)
}

fn run_group(out: Option<PathBuf>, group: Group) -> Result<ValidationReport, ValidationError> {
    let (mode, protocols): (&str, &[Protocol]) = match group {
        Group::MysqlMemcached => ("mysql-memcached", &[Protocol::Mysql, Protocol::Memcached]),
        Group::Messaging => ("messaging-breadth", &[Protocol::Mqtt, Protocol::Nats]),
        Group::Data => ("data-breadth", &[Protocol::Postgres, Protocol::Redis]),
        Group::Smtp => ("smtp-breadth", &[Protocol::Smtp]),
        Group::Pop3 => ("pop3-breadth", &[Protocol::Pop3]),
        Group::Rtsp => ("rtsp-breadth", &[Protocol::Rtsp]),
    };
    let out = prepare_output(out, "linux-data-breadth")?;
    let mut phases = Vec::new();
    for &protocol in protocols {
        eprintln!("capture protocol breadth: {}", protocol.name());
        phases.push(phase(&out.join(protocol.name()), protocol)?);
    }
    super::http_boundaries::run(Some(out.join("after-data-restart")))?;
    let checks = if matches!(group, Group::Rtsp) {
        vec![
            "real_MediaMTX_RTSP_1_0",
            "uncaptured_native_publisher_and_synthetic_PCMU_seed",
            "OPTIONS_and_missing_DESCRIBE_recovery",
            "SDP_and_content_length_checked",
            "unsupported_UDP_then_TCP_SETUP",
            "PLAY_keepalive_PAUSE_resume_TEARDOWN",
            "CSeq_and_session_identity_checked",
            "kernel_bytes_match_independent_client",
            "same_target_unselected_neighbor_excluded",
            "existing_protocol_packages_and_replay",
            "owned_internal_network_cleanup",
            "fresh_HTTP_capture_after_data",
        ]
    } else if matches!(group, Group::Pop3) {
        vec![
            "real_Mailpit_POP3",
            "uncaptured_SMTP_seed_then_captured_mailbox_retrieval",
            "auth_denial_and_unauthorized_STAT",
            "LIST_STAT_and_RETR_declared_sizes_agree",
            "Mailpit_extra_CRLF_disclosed_and_raw_bytes_preserved",
            "UIDL_unique_and_RETR_bodies_match_seed",
            "matched_packet_status_cross_check",
            "missing_and_deleted_message_errors",
            "dot_unstuffed_body_not_status_lines",
            "delete_mark_reset_and_restored_list",
            "kernel_bytes_match_independent_client",
            "same_target_unselected_neighbor_excluded",
            "existing_protocol_packages_and_replay",
            "owned_internal_network_cleanup",
            "fresh_HTTP_capture_after_data",
        ]
    } else if matches!(group, Group::Smtp) {
        vec![
            "real_Mailpit_SMTP",
            "auth_denial_and_unauthenticated_submission_refusal",
            "multiline_EHLO_and_bounded_CRLF_replies",
            "bad_sequence_and_recipient_rejection_then_same_connection_submission",
            "dot_stuffed_body_and_second_transaction",
            "matched_packet_status_cross_check",
            "kernel_bytes_match_independent_client",
            "same_target_unselected_neighbor_excluded",
            "existing_protocol_packages_and_replay",
            "owned_internal_network_cleanup",
            "fresh_HTTP_capture_after_data",
        ]
    } else if matches!(group, Group::Messaging) {
        vec![
            "real_Mosquitto_and_NATS",
            "MQTT_identifier_refusal_persistent_session_resume_and_QoS_acks",
            "NATS_auth_denial_permission_error_then_same_connection_recovery",
            "binary_empty_and_control_looking_message_bodies",
            "matched_packet_status_cross_check",
            "kernel_bytes_match_independent_client",
            "same_target_unselected_neighbor_excluded",
            "existing_protocol_packages_and_replay",
            "owned_internal_network_and_anonymous_volume_cleanup",
            "fresh_HTTP_capture_after_data",
        ]
    } else if matches!(group, Group::MysqlMemcached) {
        vec![
            "real_MySQL_and_Memcached",
            "MySQL_native_auth_query_error_and_recovery",
            "Memcached_binary_miss_not_stored_oversized_and_recovery",
            "matched_packet_status_cross_check",
            "kernel_bytes_match_independent_client",
            "same_target_unselected_neighbor_excluded",
            "existing_protocol_packages_and_replay",
            "owned_internal_network_and_anonymous_volume_cleanup",
            "fresh_HTTP_capture_after_data",
        ]
    } else {
        vec![
            "real_PostgreSQL_and_Redis",
            "auth_denial_query_error_then_recovery",
            "PostgreSQL_failed_transaction_rollback_and_null_empty",
            "Redis_empty_nil_binary_and_pipelined_replies",
            "bounded_partial_client_reads_not_packet_segmentation",
            "same_target_unselected_denial_excluded",
            "kernel_bytes_match_independent_client",
            "existing_protocol_packages_and_replay",
            "unobserved_process_route_and_egress_not_invented",
            "owned_internal_network_and_anonymous_volume_cleanup",
            "fresh_HTTP_capture_after_data",
        ]
    };
    fs::write(
        out.join("summary.json"),
        serde_json::to_vec_pretty(&json!({"schema_version":2,"ok":true,
        "mode":mode,"phases":phases,"checks":checks,"ingest_trust_mode":"unverified-local",
        "not_proven":["TLS_decryption","arbitrary_protocol_detection","general_TCP_reassembly",
            "vulnerability_exploitation_detection","PostgreSQL_TLS_SCRAM_extended_query","Redis_cluster_RESP3","MySQL_caching_sha2_auth_switch_prepared_statements","Memcached_text_meta_SASL_UDP",
            "MQTT5_authentication_TLS_offline_delivery_or_crash_durability","NATS_JetStream_cluster_headers_TLS",
            "SMTP_STARTTLS_delivery_to_external_mailbox_or_crash_durability",
            "POP3_TLS_APOP_SASL_external_delivery_or_delete_commit_durability",
            "RTSP_TLS_authentication_RTP_delivery_quality_UDP_multicast_or_general_stream_decoding",
            "PID_or_container_process_attribution","request_payload_capture","IPv6_or_UDP","multiple_hosts_or_kernels"]}))?,
    )?;
    Ok(ValidationReport {
        name: format!("Linux real {mode} capture breadth"),
        out_dir: out,
        checks: checks.into_iter().map(str::to_owned).collect(),
    })
}
