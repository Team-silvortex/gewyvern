#![cfg(unix)]

use std::fs;
use std::io::{Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command as ProcessCommand, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use leserpent_domain::{
    CAPABILITY_RUNTIME_READ, CapabilitySet, DOMAIN_SCHEMA_VERSION, Principal, Query, QueryEnvelope,
    RuntimeListFilter,
};
use leserpent_protocol::{
    AuthorityWriterClaimRequest, CAPABILITY_AUTHORITY_WRITER, PROTOCOL_SCHEMA_VERSION,
    ProtocolRequest, ProtocolResponse, RequestEnvelope, ResponseEnvelope, decode_response,
    encode_request,
};
use rcgen::{CertifiedKey, generate_simple_self_signed};
use rusqlite::Connection;
use rustls::pki_types::{CertificateDer, ServerName};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};

const TOKEN: &str = "0123456789abcdef0123456789abcdef";
const WRITER_ID: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const FAIRNESS_WAVES: usize = 3;
const SLOW_IPC_PEERS_PER_WAVE: usize = 64;

struct TempRoot(PathBuf);

impl TempRoot {
    fn new() -> Self {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = PathBuf::from(format!(
            "/tmp/leserpent-cross-transport-{}-{unique}",
            std::process::id()
        ));
        fs::create_dir(&root).unwrap();
        Self(root)
    }
}

impl Drop for TempRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct DaemonProcess {
    child: Option<Child>,
}

impl DaemonProcess {
    fn spawn(
        binary: &Path,
        database: &Path,
        socket: &Path,
        remote_address: SocketAddr,
        certificate: &Path,
        private_key: &Path,
    ) -> Self {
        let mut child = ProcessCommand::new(binary)
            .args([
                "--database",
                database.to_str().unwrap(),
                "--socket",
                socket.to_str().unwrap(),
                "--remote-listen",
                &remote_address.to_string(),
                "--remote-cert",
                certificate.to_str().unwrap(),
                "--remote-key",
                private_key.to_str().unwrap(),
            ])
            .env("LESERPENT_IPC_TOKEN", TOKEN)
            .env("LESERPENT_REMOTE_TOKEN", TOKEN)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .unwrap();
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if socket.exists()
                && UnixStream::connect(socket).is_ok()
                && TcpStream::connect(remote_address).is_ok()
            {
                break;
            }
            if let Some(status) = child.try_wait().unwrap() {
                let mut stderr = String::new();
                child
                    .stderr
                    .take()
                    .unwrap()
                    .read_to_string(&mut stderr)
                    .unwrap();
                panic!("leserpentd exited before dual-transport readiness ({status}): {stderr}");
            }
            assert!(
                Instant::now() < deadline,
                "leserpentd did not expose both transports"
            );
            thread::sleep(Duration::from_millis(10));
        }
        Self { child: Some(child) }
    }

    fn stop(mut self) {
        let mut child = self.child.take().unwrap();
        assert_eq!(unsafe { libc::kill(child.id() as i32, libc::SIGTERM) }, 0);
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(
                    status.success(),
                    "leserpentd did not stop cleanly: {status}"
                );
                return;
            }
            if Instant::now() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                panic!("leserpentd did not stop after SIGTERM");
            }
            thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for DaemonProcess {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
}

fn writer_claim() -> RequestEnvelope {
    RequestEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        request: ProtocolRequest::AuthorityWriterClaim(AuthorityWriterClaimRequest {
            principal: Principal {
                id: "cross-transport-test".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_AUTHORITY_WRITER]),
            writer_id: WRITER_ID.into(),
        }),
    }
}

fn runtime_list_query() -> RequestEnvelope {
    RequestEnvelope {
        schema_version: PROTOCOL_SCHEMA_VERSION,
        request: ProtocolRequest::Query(QueryEnvelope {
            schema_version: DOMAIN_SCHEMA_VERSION,
            principal: Principal {
                id: "cross-transport-test".into(),
            },
            capabilities: CapabilitySet::new([CAPABILITY_RUNTIME_READ]),
            query: Query::RuntimeList {
                filter: RuntimeListFilter::default(),
            },
        }),
    }
}

fn send_ipc(socket: &Path, request: &RequestEnvelope) -> ResponseEnvelope {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(4)))
        .unwrap();
    let mut frame = serde_json::to_vec(&serde_json::json!({
        "token": TOKEN,
        "request": request,
    }))
    .unwrap();
    frame.push(b'\n');
    stream.write_all(&frame).unwrap();
    stream.shutdown(std::net::Shutdown::Write).unwrap();
    let mut response = Vec::new();
    stream.read_to_end(&mut response).unwrap();
    decode_response(&response).unwrap()
}

fn hold_ipc_accept(socket: &Path) -> UnixStream {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream.write_all(b"{").unwrap();
    stream.flush().unwrap();
    thread::sleep(Duration::from_millis(100));
    stream
}

fn queue_slow_ipc_peer(socket: &Path) -> UnixStream {
    let mut stream = UnixStream::connect(socket).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    stream.write_all(b"{").unwrap();
    stream.flush().unwrap();
    stream
}

fn read_http_body(stream: &mut impl Read) -> Vec<u8> {
    let mut response = Vec::new();
    while !response.windows(4).any(|window| window == b"\r\n\r\n") {
        let mut byte = [0_u8; 1];
        stream.read_exact(&mut byte).unwrap();
        response.push(byte[0]);
    }
    let header_end = response
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .unwrap()
        + 4;
    let header = std::str::from_utf8(&response[..header_end]).unwrap();
    assert!(header.starts_with("HTTP/1.1 200 OK\r\n"));
    let content_length = header
        .split("\r\n")
        .find_map(|line| {
            line.strip_prefix("Content-Length: ")
                .and_then(|value| value.parse::<usize>().ok())
        })
        .unwrap();
    let mut body = vec![0_u8; content_length];
    stream.read_exact(&mut body).unwrap();
    body
}

fn spawn_https_query(
    address: SocketAddr,
    certificate: CertificateDer<'static>,
) -> thread::JoinHandle<(Duration, ResponseEnvelope)> {
    thread::spawn(move || {
        let started = Instant::now();
        let mut roots = RootCertStore::empty();
        roots.add(certificate).unwrap();
        let provider = Arc::new(rustls::crypto::ring::default_provider());
        let mut config = ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .unwrap()
            .with_root_certificates(roots)
            .with_no_client_auth();
        config.alpn_protocols = vec![b"http/1.1".to_vec()];
        let connection =
            ClientConnection::new(Arc::new(config), ServerName::try_from("localhost").unwrap())
                .unwrap();
        let socket = TcpStream::connect(address).unwrap();
        socket
            .set_read_timeout(Some(Duration::from_secs(5)))
            .unwrap();
        let mut stream = StreamOwned::new(connection, socket);
        let body = encode_request(&runtime_list_query()).unwrap();
        write!(
            stream,
            "POST /v1/wire HTTP/1.1\r\nHost: localhost\r\nAuthorization: Bearer {TOKEN}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n",
            body.len()
        )
        .unwrap();
        stream.write_all(&body).unwrap();
        let response = decode_response(&read_http_body(&mut stream)).unwrap();
        (started.elapsed(), response)
    })
}

fn inspect_owner_lease(database: &Path) -> (String, i64) {
    Connection::open(database)
        .unwrap()
        .query_row(
            "SELECT owner_token, lease_expires_at_unix_ms FROM runtime_owner WHERE id = 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

fn wait_for_owner_lease_extension(database: &Path, owner_token: &str, previous: i64) -> i64 {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let (current_owner, current_expiry) = inspect_owner_lease(database);
        assert_eq!(current_owner, owner_token);
        if current_expiry > previous {
            return current_expiry;
        }
        assert!(
            Instant::now() < deadline,
            "maintenance did not advance the owner heartbeat"
        );
        thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn https_and_maintenance_progress_across_repeated_saturated_ipc_waves() {
    let binary = PathBuf::from(env!("CARGO_BIN_EXE_leserpentd"));
    let root = TempRoot::new();
    let database = root.0.join("cross-transport.sqlite");
    let socket = root.0.join("cross-transport.sock");
    let certificate_path = root.0.join("remote.crt");
    let private_key_path = root.0.join("remote.key");
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".to_string()]).unwrap();
    fs::write(&certificate_path, cert.pem()).unwrap();
    fs::write(&private_key_path, signing_key.serialize_pem()).unwrap();
    fs::set_permissions(&private_key_path, fs::Permissions::from_mode(0o600)).unwrap();
    let address = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap();

    let daemon = DaemonProcess::spawn(
        &binary,
        &database,
        &socket,
        address,
        &certificate_path,
        &private_key_path,
    );
    let claim = send_ipc(&socket, &writer_claim());
    assert!(matches!(
        claim.response,
        ProtocolResponse::AuthorityWriterClaimed(ref claim)
            if claim.generation == 1 && claim.writer_id == WRITER_ID && !claim.replayed
    ));
    let (owner_token, mut lease_expiry) = inspect_owner_lease(&database);
    let mut https_elapsed = Vec::with_capacity(FAIRNESS_WAVES);
    let mut wave_elapsed = Vec::with_capacity(FAIRNESS_WAVES);

    for _ in 0..FAIRNESS_WAVES {
        let gate = hold_ipc_accept(&socket);
        let (_, gated_expiry) = inspect_owner_lease(&database);
        assert!(gated_expiry >= lease_expiry);
        let slow_peers = (0..SLOW_IPC_PEERS_PER_WAVE)
            .map(|_| queue_slow_ipc_peer(&socket))
            .collect::<Vec<_>>();
        let https = spawn_https_query(address, cert.der().clone());
        let started = Instant::now();
        drop(gate);

        let (remote_elapsed, response) = https.join().unwrap();
        assert!(matches!(response.response, ProtocolResponse::Query(_)));
        assert!(
            remote_elapsed <= Duration::from_secs(5),
            "HTTPS query starved behind saturated IPC: {remote_elapsed:?}"
        );
        https_elapsed.push(remote_elapsed);
        for mut slow in slow_peers {
            let mut response = Vec::new();
            slow.read_to_end(&mut response).unwrap();
            assert!(response.is_empty());
        }
        let elapsed = started.elapsed();
        assert!(
            elapsed <= Duration::from_secs(5),
            "cross-transport fairness wave exceeded its budget: {elapsed:?}"
        );
        wave_elapsed.push(elapsed);
        lease_expiry = wait_for_owner_lease_extension(&database, &owner_token, gated_expiry);
    }

    let final_fence = Connection::open(&database)
        .unwrap()
        .query_row(
            "SELECT generation, writer_id FROM authority_writer_fence WHERE id = 1",
            [],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?)),
        )
        .unwrap();
    assert_eq!(final_fence, (1, WRITER_ID.to_string()));
    println!(
        "cross-transport-fairness waves={} slow_ipc_peers_per_wave={} https_queries={} https_elapsed_ms={:?} wave_elapsed_ms={:?} maintenance_heartbeat_advanced_each_wave=true final_generation=1",
        FAIRNESS_WAVES,
        SLOW_IPC_PEERS_PER_WAVE,
        FAIRNESS_WAVES,
        https_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>(),
        wave_elapsed
            .iter()
            .map(Duration::as_millis)
            .collect::<Vec<_>>()
    );
    daemon.stop();
    assert!(!socket.exists());
}
