//! Owns fixture startup, uncaptured setup and teardown, never evidence classification.
use super::super::docker_target::{OwnedResource, docker, target_addresses};
use super::super::{owned_socket_on, tool};
use super::{Protocol, ValidationError, mqtt, pop3, rtsp, smtp, wire};
use serde_json::{Value, json};
use std::fs;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;
use std::time::{Duration, Instant};

pub(super) struct Target {
    // Fields drop in this order on errors: publisher, container, then its network.
    publisher: Option<wire::Wire>,
    container: OwnedResource,
    network: OwnedResource,
    pub(super) source: Ipv4Addr,
    pub(super) destination: Ipv4Addr,
    pub(super) address: SocketAddr,
    pub(super) image_id: String,
    pub(super) container_id: Value,
    pub(super) network_id: Value,
    pub(super) volumes: Vec<String>,
}

impl Target {
    pub(super) fn start(
        protocol: Protocol,
        token: &str,
        out: &Path,
    ) -> Result<Self, ValidationError> {
        let image_id = tool(
            docker().args(["image", "inspect", protocol.image(), "--format", "{{.Id}}"]),
            out,
        )?;
        let network =
            OwnedResource::create("network", token, &["--driver", "bridge", "--internal"], out)?;
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
        let container = OwnedResource::create("container", token, &args, out)?;
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
                out,
            )?;
        }
        tool(docker().args(["container", "start", &container.name]), out)?;
        let container_info = container.inspect()?;
        if container_info["State"]["Running"] != true {
            fs::write(
                out.join("target-startup.log"),
                tool(
                    docker().args(["logs", "--tail", "100", &container.name]),
                    out,
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
        let (source, destination) =
            target_addresses(&container_info, &network_info, &network.name)?;
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
                            out,
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

        Ok(Self {
            publisher,
            container,
            network,
            source,
            destination,
            address: target,
            image_id: String::from_utf8_lossy(&image_id).trim().to_owned(),
            container_id: container_info["Id"].clone(),
            network_id: network_info["Id"].clone(),
            volumes,
        })
    }

    pub(super) fn finish_traffic(&mut self) {
        self.publisher.take();
    }

    pub(super) fn cleanup(&mut self, out: &Path) -> Result<(), ValidationError> {
        self.finish_traffic();
        self.container.cleanup()?;
        for volume in &self.volumes {
            let remaining = tool(
                docker().args([
                    "volume",
                    "ls",
                    "--quiet",
                    "--filter",
                    &format!("name=^{volume}$"),
                ]),
                out,
            )?;
            if !remaining.iter().all(u8::is_ascii_whitespace) {
                return Err(ValidationError::new(
                    "owned anonymous target volume survived cleanup",
                ));
            }
        }
        self.network.cleanup()?;

        Ok(())
    }
}
