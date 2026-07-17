use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use leserpent_adapters::{
    EnvironmentSecretStore, GewyvernDeploymentAdapter, GewyvernHealthAdapter,
    GewyvernStatusRefreshAdapter, GewyvernTarget, PlatformSecretStore, SecretKey, SecretStore,
};
use leserpent_runtime::ControlRuntime;
#[cfg(unix)]
use leserpentd::IpcServer;
use leserpentd::{AdapterRegistry, DaemonConfig, DaemonHost, RemoteServer};
use signal_hook::consts::{SIGINT, SIGTERM};

fn main() {
    if let Err(error) = run() {
        eprintln!("leserpentd: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut database = std::env::var_os("LESERPENT_DATABASE").map(PathBuf::from);
    let mut socket = None;
    let mut remote_listen = None;
    let mut remote_certificate = None;
    let mut remote_private_key = None;
    let mut gewyvern_targets = Vec::new();
    let mut gewyvern_https_targets = Vec::new();
    let mut gewyvern_admin_secret = None;
    let mut steps = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--database" => {
                database = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--database requires a path".to_string())?,
                ));
            }
            "--socket" => {
                socket = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--socket requires a path".to_string())?,
                ));
            }
            "--remote-listen" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--remote-listen requires an address".to_string())?;
                remote_listen = Some(
                    value
                        .parse::<SocketAddr>()
                        .map_err(|_| "--remote-listen address is invalid".to_string())?,
                );
            }
            "--remote-cert" => {
                remote_certificate =
                    Some(PathBuf::from(arguments.next().ok_or_else(|| {
                        "--remote-cert requires a path".to_string()
                    })?));
            }
            "--remote-key" => {
                remote_private_key = Some(PathBuf::from(
                    arguments
                        .next()
                        .ok_or_else(|| "--remote-key requires a path".to_string())?,
                ));
            }
            "--once" => steps = Some(1),
            "--gewyvern-target" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--gewyvern-target requires ID=LOOPBACK:PORT".to_string())?;
                gewyvern_targets.push(parse_gewyvern_target(&value)?);
            }
            "--gewyvern-https-target" => {
                let value = arguments.next().ok_or_else(|| {
                    "--gewyvern-https-target requires ID=HTTPS_ORIGIN,CA_PATH".to_string()
                })?;
                gewyvern_https_targets.push(parse_gewyvern_https_target(&value)?);
            }
            "--gewyvern-admin-secret" if gewyvern_admin_secret.is_none() => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--gewyvern-admin-secret requires KEY".to_string())?;
                gewyvern_admin_secret = Some(
                    SecretKey::new(value)
                        .map_err(|_| "--gewyvern-admin-secret KEY is invalid".to_string())?,
                );
            }
            "--gewyvern-admin-secret" => {
                return Err("--gewyvern-admin-secret was provided more than once".into());
            }
            "--steps" => {
                let value = arguments
                    .next()
                    .ok_or_else(|| "--steps requires a positive integer".to_string())?;
                let parsed = value
                    .parse::<u64>()
                    .map_err(|_| "--steps requires a positive integer".to_string())?;
                if parsed == 0 {
                    return Err("--steps requires a positive integer".into());
                }
                steps = Some(parsed);
            }
            "-h" | "--help" => {
                println!(
                    "Usage: leserpentd --database PATH [--socket PATH] \
                     [--remote-listen ADDR --remote-cert PATH --remote-key PATH] \
                     [--gewyvern-target ID=LOOPBACK:PORT] \
                     [--gewyvern-https-target ID=HTTPS_ORIGIN,CA_PATH] \
                     [--gewyvern-admin-secret KEY] [--once | --steps N]\n\
                     Environment: LESERPENT_DATABASE may provide the database path; \
                     LESERPENT_IPC_TOKEN is required when --socket is used; \
                     LESERPENT_REMOTE_TOKEN is required for the HTTPS remote endpoint; \
                     GEWY_API_ADMIN_TOKEN optionally authenticates Gewyvern targets"
                );
                return Ok(());
            }
            other => return Err(format!("unknown argument '{other}'")),
        }
    }
    let database = database.ok_or_else(|| {
        "database path is required via --database or LESERPENT_DATABASE".to_string()
    })?;
    let runtime = ControlRuntime::open(database).map_err(|error| error.to_string())?;
    let mut registry = AdapterRegistry::default();
    if !gewyvern_targets.is_empty() || !gewyvern_https_targets.is_empty() {
        let (configured_admin_secret, secrets): (Option<SecretKey>, Arc<dyn SecretStore>) =
            if let Some(admin_secret) = gewyvern_admin_secret {
                (
                    Some(admin_secret),
                    Arc::new(
                        PlatformSecretStore::new("org.gewyvern.leserpent.adapters")
                            .map_err(|error| format!("{error:?}"))?,
                    ),
                )
            } else {
                let admin_secret =
                    SecretKey::new("gewyvern-admin").map_err(|error| format!("{error:?}"))?;
                let configured = std::env::var_os("GEWY_API_ADMIN_TOKEN")
                    .is_some()
                    .then(|| admin_secret.clone());
                (
                    configured,
                    Arc::new(
                        EnvironmentSecretStore::new([(
                            admin_secret,
                            "GEWY_API_ADMIN_TOKEN".to_string(),
                        )])
                        .map_err(|error| format!("{error:?}"))?,
                    ),
                )
            };
        if !gewyvern_https_targets.is_empty() && configured_admin_secret.is_none() {
            return Err(
                "--gewyvern-https-target requires --gewyvern-admin-secret or GEWY_API_ADMIN_TOKEN"
                    .into(),
            );
        }
        let mut targets = gewyvern_targets
            .into_iter()
            .map(|(runtime_id, address)| {
                GewyvernTarget::loopback(address, configured_admin_secret.clone())
                    .map(|target| (runtime_id, target))
            })
            .collect::<Result<Vec<_>, _>>()?;
        for (runtime_id, origin, ca_path) in gewyvern_https_targets {
            let admin_secret = configured_admin_secret
                .clone()
                .expect("remote target credential was checked above");
            targets.push((
                runtime_id,
                GewyvernTarget::https(&origin, ca_path, admin_secret)?,
            ));
        }
        registry.register(GewyvernHealthAdapter::with_secret_store(
            targets.clone(),
            secrets.clone(),
        )?)?;
        if configured_admin_secret.is_some() {
            registry.register(GewyvernDeploymentAdapter::with_secret_store(
                targets.clone(),
                secrets.clone(),
            )?)?;
        }
        registry.register(GewyvernStatusRefreshAdapter::with_secret_store(
            targets, secrets,
        )?)?;
    }
    let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default())?;
    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, Arc::clone(&stop)).map_err(|error| error.to_string())?;
    signal_hook::flag::register(SIGTERM, Arc::clone(&stop)).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    let ipc = match socket {
        Some(path) => {
            let token = std::env::var("LESERPENT_IPC_TOKEN")
                .map_err(|_| "LESERPENT_IPC_TOKEN is required with --socket".to_string())?;
            Some(IpcServer::bind(path, &token)?)
        }
        None => None,
    };
    #[cfg(not(unix))]
    if socket.is_some() {
        return Err("--socket is currently supported only on Unix platforms".into());
    }
    let mut remote = match (remote_listen, remote_certificate, remote_private_key) {
        (None, None, None) => None,
        (Some(address), Some(certificate), Some(private_key)) => {
            let token = std::env::var("LESERPENT_REMOTE_TOKEN").map_err(|_| {
                "LESERPENT_REMOTE_TOKEN is required with --remote-listen".to_string()
            })?;
            Some(RemoteServer::bind(
                address,
                certificate,
                private_key,
                &token,
            )?)
        }
        _ => {
            return Err(
                "--remote-listen, --remote-cert, and --remote-key must be provided together".into(),
            );
        }
    };
    match steps {
        Some(steps) => {
            for _ in 0..steps {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                #[cfg(unix)]
                if let Some(ipc) = &ipc {
                    ipc.poll_once(host.runtime_mut())?;
                }
                if let Some(remote) = &mut remote {
                    remote.poll_once(host.runtime_mut())?;
                }
                host.run_steps_until(1, &stop)
                    .map_err(|error| error.to_string())?;
            }
        }
        None => {
            while !stop.load(Ordering::Acquire) {
                #[cfg(unix)]
                if let Some(ipc) = &ipc {
                    ipc.poll_once(host.runtime_mut())?;
                }
                if let Some(remote) = &mut remote {
                    remote.poll_once(host.runtime_mut())?;
                }
                host.run_steps_until(1, &stop)
                    .map_err(|error| error.to_string())?;
            }
        }
    }
    Ok(())
}

fn parse_gewyvern_target(value: &str) -> Result<(String, SocketAddr), String> {
    let (runtime_id, address) = value
        .split_once('=')
        .ok_or_else(|| "--gewyvern-target requires ID=LOOPBACK:PORT".to_string())?;
    if runtime_id.is_empty() {
        return Err("--gewyvern-target runtime ID must not be empty".into());
    }
    let address = address
        .parse::<SocketAddr>()
        .map_err(|_| "--gewyvern-target address is invalid".to_string())?;
    Ok((runtime_id.to_string(), address))
}

fn parse_gewyvern_https_target(value: &str) -> Result<(String, String, PathBuf), String> {
    let (runtime_id, target) = value
        .split_once('=')
        .ok_or_else(|| "--gewyvern-https-target requires ID=HTTPS_ORIGIN,CA_PATH".to_string())?;
    let (origin, ca_path) = target
        .split_once(',')
        .ok_or_else(|| "--gewyvern-https-target requires ID=HTTPS_ORIGIN,CA_PATH".to_string())?;
    if runtime_id.is_empty() || origin.is_empty() || ca_path.is_empty() {
        return Err("--gewyvern-https-target fields must not be empty".into());
    }
    Ok((
        runtime_id.to_string(),
        origin.to_string(),
        PathBuf::from(ca_path),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gewyvern_target_argument_supports_ipv4_and_ipv6_loopback_shapes() {
        assert_eq!(
            parse_gewyvern_target("runtime-a=127.0.0.1:9411").unwrap().0,
            "runtime-a"
        );
        assert!(parse_gewyvern_target("runtime-a=[::1]:9411").is_ok());
        assert!(parse_gewyvern_target("runtime-a").is_err());
    }

    #[test]
    fn gewyvern_https_target_argument_keeps_origin_and_ca_explicit() {
        let (runtime_id, origin, ca_path) =
            parse_gewyvern_https_target("runtime-a=https://gewyvern.example:9443,/etc/gewy/ca.pem")
                .unwrap();
        assert_eq!(runtime_id, "runtime-a");
        assert_eq!(origin, "https://gewyvern.example:9443");
        assert_eq!(ca_path, PathBuf::from("/etc/gewy/ca.pem"));
        assert!(parse_gewyvern_https_target("runtime-a=https://localhost").is_err());
        assert!(parse_gewyvern_https_target("=https://localhost,/ca.pem").is_err());
    }
}
