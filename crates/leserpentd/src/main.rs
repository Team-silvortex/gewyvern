use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[cfg(feature = "native-ssh")]
use leserpent_adapters::{BootstrapTrustStore, FileBootstrapTrustStore};
use leserpent_adapters::{
    DAEMON_RETIREMENT_EFFECT_KIND, GEWYVERN_PROVISIONING_EFFECT_KIND,
    GEWYVERN_RETIREMENT_EFFECT_KIND, HOST_BOOTSTRAP_EFFECT_KIND,
};
use leserpent_adapters::{
    EnvironmentSecretStore, GewyvernDeploymentAdapter, GewyvernDiscoveryAdapter,
    GewyvernHealthAdapter, GewyvernStatusRefreshAdapter, GewyvernTarget, PlatformSecretStore,
    SecretKey, SecretStore,
};
use leserpent_domain::RuntimeId;
use leserpent_protocol::AuthorityWriterFence;
use leserpent_runtime::ControlRuntime;
use leserpentd::{
    AdapterRegistry, BootstrapSessionVerifier, DaemonConfig, DaemonHost, DebuggerAuthority,
    NativeBootstrapSessionVerifier, RemoteServer, load_remote_token_file,
};
#[cfg(feature = "native-ssh")]
use leserpentd::{BootstrapOriginConfig, GewyvernOriginConfig};
#[cfg(unix)]
use leserpentd::{IpcServer, MAX_IPC_CONNECTIONS_PER_TICK};
use ring::rand::{SecureRandom, SystemRandom};
use signal_hook::consts::{SIGINT, SIGTERM};
use zeroize::Zeroizing;

fn main() {
    if let Err(error) = run() {
        eprintln!("leserpentd: {error}");
        std::process::exit(1);
    }
}

#[derive(Default)]
struct TransportScheduler {
    remote_first: bool,
}

impl TransportScheduler {
    fn next_remote_first(&mut self) -> bool {
        let remote_first = self.remote_first;
        self.remote_first = !self.remote_first;
        remote_first
    }
}

fn run_fair_daemon_turn(
    host: &mut DaemonHost,
    #[cfg(unix)] ipc: Option<&IpcServer>,
    remote: Option<&mut RemoteServer>,
    stop: &AtomicBool,
    remote_first: bool,
) -> Result<(), String> {
    // Maintenance leads every turn so neither transport can defer owner
    // heartbeat or durable worker progress indefinitely.
    host.run_steps_until(1, stop)
        .map_err(|error| error.to_string())?;
    if remote_first {
        if let Some(remote) = remote {
            remote.poll_once_until(host.runtime_mut(), stop)?;
        }
        #[cfg(unix)]
        if let Some(ipc) = ipc {
            ipc.poll_batch_until(host.runtime_mut(), MAX_IPC_CONNECTIONS_PER_TICK, stop)?;
        }
    } else {
        #[cfg(unix)]
        if let Some(ipc) = ipc {
            ipc.poll_batch_until(host.runtime_mut(), MAX_IPC_CONNECTIONS_PER_TICK, stop)?;
        }
        if let Some(remote) = remote {
            remote.poll_once_until(host.runtime_mut(), stop)?;
        }
    }
    Ok(())
}

fn run() -> Result<(), String> {
    let arguments = std::env::args().skip(1).collect::<Vec<_>>();
    if arguments.first().map(String::as_str) == Some("bootstrap-install-v1") {
        if arguments.len() != 1 {
            return Err("bootstrap-install-v1 accepts no command-line arguments".into());
        }
        return leserpentd::bootstrap_install::run_bootstrap_install_stdio()
            .map_err(|error| error.to_string());
    }
    if arguments.first().map(String::as_str) == Some("bootstrap-activate-v1") {
        if arguments.len() != 1 {
            return Err("bootstrap-activate-v1 accepts no command-line arguments".into());
        }
        return leserpentd::bootstrap_install::run_bootstrap_activate_stdio()
            .map_err(|error| error.to_string());
    }
    if arguments.first().map(String::as_str) == Some("bootstrap-retire-v1") {
        if arguments.len() != 1 {
            return Err("bootstrap-retire-v1 accepts no command-line arguments".into());
        }
        return leserpentd::bootstrap_install::run_bootstrap_retire_stdio()
            .map_err(|error| error.to_string());
    }
    let mut database = std::env::var_os("LESERPENT_DATABASE").map(PathBuf::from);
    let mut socket = None;
    let mut remote_listen = None;
    let mut remote_certificate = None;
    let mut remote_private_key = None;
    let mut remote_token_file = None;
    let mut web_console_writer = false;
    let mut gewyvern_targets = Vec::new();
    let mut gewyvern_https_targets = Vec::new();
    let mut gewyvern_admin_secret = None;
    let mut bootstrap_config = None;
    let mut gewyvern_provisioning_config = None;
    let mut bootstrap_trust_root = None;
    let mut steps = None;
    let mut arguments = arguments.into_iter();
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
            "--remote-token-file" if remote_token_file.is_none() => {
                remote_token_file =
                    Some(PathBuf::from(arguments.next().ok_or_else(|| {
                        "--remote-token-file requires a path".to_string()
                    })?));
            }
            "--remote-token-file" => {
                return Err("--remote-token-file was provided more than once".into());
            }
            "--web-console-writer" if !web_console_writer => {
                web_console_writer = true;
            }
            "--web-console-writer" => {
                return Err("--web-console-writer was provided more than once".into());
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
            "--bootstrap-trust-root" if bootstrap_trust_root.is_none() => {
                bootstrap_trust_root =
                    Some(PathBuf::from(arguments.next().ok_or_else(|| {
                        "--bootstrap-trust-root requires a path".to_string()
                    })?));
            }
            "--bootstrap-trust-root" => {
                return Err("--bootstrap-trust-root was provided more than once".into());
            }
            "--bootstrap-config" if bootstrap_config.is_none() => {
                bootstrap_config =
                    Some(PathBuf::from(arguments.next().ok_or_else(|| {
                        "--bootstrap-config requires a path".to_string()
                    })?));
            }
            "--bootstrap-config" => {
                return Err("--bootstrap-config was provided more than once".into());
            }
            "--gewyvern-provisioning-config" if gewyvern_provisioning_config.is_none() => {
                gewyvern_provisioning_config =
                    Some(PathBuf::from(arguments.next().ok_or_else(|| {
                        "--gewyvern-provisioning-config requires a path".to_string()
                    })?));
            }
            "--gewyvern-provisioning-config" => {
                return Err("--gewyvern-provisioning-config was provided more than once".into());
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
                     [--remote-listen ADDR --remote-cert PATH --remote-key PATH \
                      [--remote-token-file PATH] [--web-console-writer]] \
                     [--gewyvern-target ID=LOOPBACK:PORT] \
                     [--gewyvern-https-target ID=HTTPS_ORIGIN,CA_PATH] \
                     [--gewyvern-admin-secret KEY] [--bootstrap-trust-root PATH] \
                     [--bootstrap-config PATH] [--gewyvern-provisioning-config PATH] \
                     [--once | --steps N]\n\
                     Environment: LESERPENT_DATABASE may provide the database path; \
                     LESERPENT_IPC_TOKEN is required when --socket is used; \
                     LESERPENT_REMOTE_TOKEN or --remote-token-file is required for HTTPS; \
                     the HTTPS origin serves Rust Web read projections by default; \
                     --web-console-writer explicitly grants it daemon-owned mutations; \
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
    if web_console_writer
        && (remote_listen.is_none() || remote_certificate.is_none() || remote_private_key.is_none())
    {
        return Err(
            "--web-console-writer requires --remote-listen, --remote-cert, and --remote-key".into(),
        );
    }
    #[cfg(not(feature = "native-ssh"))]
    if bootstrap_config.is_some() || gewyvern_provisioning_config.is_some() {
        return Err(
            "SSH origin configuration requires a leserpentd build with the native-ssh feature"
                .into(),
        );
    }
    #[cfg(feature = "native-ssh")]
    let bootstrap_origin = bootstrap_config
        .map(BootstrapOriginConfig::load)
        .transpose()?;
    #[cfg(feature = "native-ssh")]
    let gewyvern_origin = gewyvern_provisioning_config
        .map(GewyvernOriginConfig::load)
        .transpose()?;
    #[cfg(feature = "native-ssh")]
    if (bootstrap_origin.is_some() || gewyvern_origin.is_some()) && bootstrap_trust_root.is_none() {
        return Err("SSH origin configuration requires --bootstrap-trust-root".into());
    }
    let debugger_authority = Arc::new(Mutex::new(DebuggerAuthority::for_database(&database)?));
    let mut runtime = ControlRuntime::open(database).map_err(|error| error.to_string())?;
    let web_console_writer_fence = if web_console_writer {
        let writer_id = new_authority_writer_id()?;
        let claim = runtime
            .claim_authority_writer(&writer_id)
            .map_err(|error| format!("cannot claim Rust Web writer authority: {error}"))?;
        Some(AuthorityWriterFence {
            generation: claim.generation,
            writer_id,
        })
    } else {
        None
    };
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
        let registrations = gewyvern_targets
            .iter()
            .map(|(runtime_id, address)| (runtime_id.clone(), format!("http://{address}")))
            .chain(
                gewyvern_https_targets
                    .iter()
                    .map(|(runtime_id, origin, _)| (runtime_id.clone(), origin.clone())),
            )
            .collect::<Vec<_>>();
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
        registry.register(GewyvernDiscoveryAdapter::with_secret_store(
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
        for (runtime_id, endpoint) in registrations {
            let id = RuntimeId::new(runtime_id.clone())
                .map_err(|_| format!("configured runtime ID '{runtime_id}' is invalid"))?;
            runtime
                .ensure_runtime_registered(id, runtime_id, endpoint)
                .map_err(|error| error.to_string())?;
        }
    }
    #[cfg(feature = "native-ssh")]
    let bootstrap_secret_service = bootstrap_origin
        .as_ref()
        .map(BootstrapOriginConfig::secret_service)
        .unwrap_or("org.gewyvern.leserpent.adapters");
    #[cfg(not(feature = "native-ssh"))]
    let bootstrap_secret_service = "org.gewyvern.leserpent.adapters";
    let bootstrap_secrets: Option<Arc<dyn SecretStore>> = bootstrap_trust_root
        .as_ref()
        .map(|_| {
            PlatformSecretStore::new(bootstrap_secret_service)
                .map(|store| Arc::new(store) as Arc<dyn SecretStore>)
                .map_err(|error| format!("cannot open bootstrap session store: {error:?}"))
        })
        .transpose()?;
    #[cfg(feature = "native-ssh")]
    if let Some(origin) = bootstrap_origin {
        let secrets = bootstrap_secrets
            .as_ref()
            .expect("bootstrap origin trust-root requirement was checked")
            .clone();
        let trust: Arc<dyn BootstrapTrustStore> = Arc::new(
            FileBootstrapTrustStore::new(
                bootstrap_trust_root
                    .as_ref()
                    .expect("bootstrap origin trust-root requirement was checked"),
            )
            .map_err(|error| format!("cannot open bootstrap trust store: {error:?}"))?,
        );
        let (bootstrap, retirement) = origin.into_native_adapters(secrets, trust)?;
        registry.register(bootstrap)?;
        registry.register(retirement)?;
    }
    #[cfg(feature = "native-ssh")]
    if let Some(origin) = gewyvern_origin {
        let secrets: Arc<dyn SecretStore> = Arc::new(
            PlatformSecretStore::new(origin.secret_service())
                .map_err(|error| format!("cannot open Gewyvern provisioning store: {error:?}"))?,
        );
        let trust: Arc<dyn BootstrapTrustStore> = Arc::new(
            FileBootstrapTrustStore::new(
                bootstrap_trust_root
                    .as_ref()
                    .expect("Gewyvern origin trust-root requirement was checked"),
            )
            .map_err(|error| format!("cannot open Gewyvern trust store: {error:?}"))?,
        );
        let (provisioning, retirement) = origin.into_native_adapters(secrets, trust)?;
        registry.register(provisioning)?;
        registry.register(retirement)?;
    }
    let bootstrap_verifier: Option<Arc<dyn BootstrapSessionVerifier>> = bootstrap_trust_root
        .map(|trust_root| {
            NativeBootstrapSessionVerifier::new(
                bootstrap_secrets
                    .as_ref()
                    .expect("bootstrap trust root always initializes a secret store")
                    .clone(),
                trust_root,
            )
            .map(|verifier| Arc::new(verifier) as Arc<dyn BootstrapSessionVerifier>)
        })
        .transpose()?;
    let bootstrap_submission_enabled = registry.contains_kind(HOST_BOOTSTRAP_EFFECT_KIND);
    let provisioning_submission_enabled = registry.contains_kind(GEWYVERN_PROVISIONING_EFFECT_KIND);
    let retirement_submission_enabled = registry.contains_kind(GEWYVERN_RETIREMENT_EFFECT_KIND);
    let daemon_retirement_submission_enabled =
        registry.contains_kind(DAEMON_RETIREMENT_EFFECT_KIND);
    let mut host = DaemonHost::new(runtime, registry, DaemonConfig::default())?;
    let stop = Arc::new(AtomicBool::new(false));
    signal_hook::flag::register(SIGINT, Arc::clone(&stop)).map_err(|error| error.to_string())?;
    signal_hook::flag::register(SIGTERM, Arc::clone(&stop)).map_err(|error| error.to_string())?;
    #[cfg(unix)]
    let ipc = match socket {
        Some(path) => {
            let token = std::env::var("LESERPENT_IPC_TOKEN")
                .map_err(|_| "LESERPENT_IPC_TOKEN is required with --socket".to_string())?;
            let server = IpcServer::bind(path, &token)?;
            let server = server.with_debugger_authority(Arc::clone(&debugger_authority));
            let server = match &bootstrap_verifier {
                Some(verifier) => server.with_bootstrap_verifier(Arc::clone(verifier)),
                None => server,
            };
            let server = if bootstrap_submission_enabled {
                server.with_bootstrap_submission()
            } else {
                server
            };
            let server = if provisioning_submission_enabled {
                server.with_provisioning_submission()
            } else {
                server
            };
            let server = if retirement_submission_enabled {
                server.with_retirement_submission()
            } else {
                server
            };
            Some(if daemon_retirement_submission_enabled {
                server.with_daemon_retirement_submission()
            } else {
                server
            })
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
            let environment_token = std::env::var("LESERPENT_REMOTE_TOKEN").ok();
            if remote_token_file.is_some() && environment_token.is_some() {
                return Err(
                    "--remote-token-file and LESERPENT_REMOTE_TOKEN are mutually exclusive".into(),
                );
            }
            let token = match (remote_token_file.as_ref(), environment_token) {
                (Some(path), None) => load_remote_token_file(path)?,
                (None, Some(token)) => Zeroizing::new(token),
                (None, None) => {
                    return Err(
                        "LESERPENT_REMOTE_TOKEN or --remote-token-file is required with --remote-listen"
                            .into(),
                    );
                }
                (Some(_), Some(_)) => unreachable!("mutual exclusion was checked"),
            };
            let server = RemoteServer::bind(address, certificate, private_key, &token)?;
            let server = match &web_console_writer_fence {
                Some(writer_fence) => server.with_web_console_writer(writer_fence.clone()),
                None => server,
            };
            let server = server.with_debugger_authority(Arc::clone(&debugger_authority));
            let server = match &bootstrap_verifier {
                Some(verifier) => server.with_bootstrap_verifier(Arc::clone(verifier)),
                None => server,
            };
            let server = if bootstrap_submission_enabled {
                server.with_bootstrap_submission()
            } else {
                server
            };
            let server = if provisioning_submission_enabled {
                server.with_provisioning_submission()
            } else {
                server
            };
            let server = if retirement_submission_enabled {
                server.with_retirement_submission()
            } else {
                server
            };
            Some(if daemon_retirement_submission_enabled {
                server.with_daemon_retirement_submission()
            } else {
                server
            })
        }
        _ => {
            return Err(
                "--remote-listen, --remote-cert, and --remote-key must be provided together".into(),
            );
        }
    };
    if remote.is_none() && remote_token_file.is_some() {
        return Err("--remote-token-file requires the remote HTTPS options".into());
    }
    let mut transport_scheduler = TransportScheduler::default();
    match steps {
        Some(steps) => {
            for _ in 0..steps {
                if stop.load(Ordering::Acquire) {
                    break;
                }
                run_fair_daemon_turn(
                    &mut host,
                    #[cfg(unix)]
                    ipc.as_ref(),
                    remote.as_mut(),
                    &stop,
                    transport_scheduler.next_remote_first(),
                )?;
            }
        }
        None => {
            while !stop.load(Ordering::Acquire) {
                run_fair_daemon_turn(
                    &mut host,
                    #[cfg(unix)]
                    ipc.as_ref(),
                    remote.as_mut(),
                    &stop,
                    transport_scheduler.next_remote_first(),
                )?;
            }
        }
    }
    Ok(())
}

fn new_authority_writer_id() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    SystemRandom::new()
        .fill(&mut bytes)
        .map_err(|_| "cannot generate Rust Web writer identity".to_string())?;
    let mut writer_id = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        write!(&mut writer_id, "{byte:02x}")
            .expect("writing a fixed authority writer ID cannot fail");
    }
    Ok(writer_id)
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

    #[test]
    fn transport_scheduler_alternates_local_and_remote_priority() {
        let mut scheduler = TransportScheduler::default();
        assert_eq!(
            (0..6)
                .map(|_| scheduler.next_remote_first())
                .collect::<Vec<_>>(),
            [false, true, false, true, false, true]
        );
    }

    #[test]
    fn web_console_writer_identity_is_fresh_private_hex() {
        let first = new_authority_writer_id().unwrap();
        let second = new_authority_writer_id().unwrap();
        assert_eq!(first.len(), 32);
        assert!(first.bytes().all(|byte| byte.is_ascii_hexdigit()));
        assert_ne!(first, second);
    }
}
