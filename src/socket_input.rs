use crate::export::{ExportBundle, ExportError, fact_from_json};
use crate::runtime::{RuntimeError, RuntimeSession, SessionConfig};
use crate::template::{Template, TemplateBinding};
use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;
use std::time::SystemTime;

const MAX_FACT_LINE_BYTES: usize = 64 * 1024;
const MAX_FACT_COUNT: usize = 100_000;

#[derive(Debug)]
pub enum SocketInputError {
    UnsupportedPlatform,
    BindFailed(String),
    AcceptFailed(String),
    ReadFailed(String),
    LimitExceeded(String),
    ParseFailed(ExportError),
    Runtime(RuntimeError),
}

#[cfg(target_family = "unix")]
pub fn run_unix_socket_session(
    socket_path: &str,
    template: Template,
) -> Result<ExportBundle, SocketInputError> {
    remove_unix_socket_file(socket_path)?;
    let listener = bind_unix_socket_listener(socket_path)?;
    let export = run_unix_socket_session_on_listener(&listener, template)?;
    remove_unix_socket_file(socket_path)?;
    Ok(export)
}

#[cfg(target_family = "unix")]
pub fn run_unix_socket_session_with_binding(
    socket_path: &str,
    binding: TemplateBinding,
) -> Result<ExportBundle, SocketInputError> {
    remove_unix_socket_file(socket_path)?;
    let listener = bind_unix_socket_listener(socket_path)?;
    let export = run_unix_socket_session_on_listener_with_binding(&listener, binding)?;
    remove_unix_socket_file(socket_path)?;
    Ok(export)
}

#[cfg(target_family = "unix")]
pub fn remove_unix_socket_file(socket_path: &str) -> Result<(), SocketInputError> {
    use std::fs;
    use std::os::unix::fs::FileTypeExt;

    match fs::symlink_metadata(socket_path) {
        Ok(metadata) if metadata.file_type().is_socket() => fs::remove_file(socket_path)
            .map_err(|err| SocketInputError::BindFailed(err.to_string())),
        Ok(_) => Err(SocketInputError::BindFailed(format!(
            "refusing to remove non-socket path '{}'",
            socket_path
        ))),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(err) => Err(SocketInputError::BindFailed(err.to_string())),
    }
}

#[cfg(target_family = "unix")]
pub fn bind_unix_socket_listener(
    socket_path: &str,
) -> Result<std::os::unix::net::UnixListener, SocketInputError> {
    use std::os::unix::net::UnixListener;

    UnixListener::bind(socket_path).map_err(|err| SocketInputError::BindFailed(err.to_string()))
}

#[cfg(target_family = "unix")]
pub fn run_unix_socket_session_on_listener(
    listener: &std::os::unix::net::UnixListener,
    template: Template,
) -> Result<ExportBundle, SocketInputError> {
    let (stream, _) = listener
        .accept()
        .map_err(|err| SocketInputError::AcceptFailed(err.to_string()))?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    run_stream_session(
        BufReader::new(stream),
        SessionConfig::for_template(template).map_err(SocketInputError::Runtime)?,
    )
}

#[cfg(target_family = "unix")]
pub fn run_unix_socket_session_on_listener_with_binding(
    listener: &std::os::unix::net::UnixListener,
    binding: TemplateBinding,
) -> Result<ExportBundle, SocketInputError> {
    let (stream, _) = listener
        .accept()
        .map_err(|err| SocketInputError::AcceptFailed(err.to_string()))?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    run_stream_session(
        BufReader::new(stream),
        SessionConfig::for_binding(binding).map_err(SocketInputError::Runtime)?,
    )
}

pub fn run_tcp_socket_session(
    bind_addr: &str,
    template: Template,
) -> Result<ExportBundle, SocketInputError> {
    let listener = TcpListener::bind(bind_addr)
        .map_err(|err| SocketInputError::BindFailed(err.to_string()))?;
    run_tcp_socket_session_on_listener(&listener, template)
}

pub fn run_tcp_socket_session_with_binding(
    bind_addr: &str,
    binding: TemplateBinding,
) -> Result<ExportBundle, SocketInputError> {
    let listener = TcpListener::bind(bind_addr)
        .map_err(|err| SocketInputError::BindFailed(err.to_string()))?;
    run_tcp_socket_session_on_listener_with_binding(&listener, binding)
}

pub fn run_tcp_socket_session_on_listener(
    listener: &TcpListener,
    template: Template,
) -> Result<ExportBundle, SocketInputError> {
    let (stream, _) = listener
        .accept()
        .map_err(|err| SocketInputError::AcceptFailed(err.to_string()))?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    run_stream_session(
        BufReader::new(stream),
        SessionConfig::for_template(template).map_err(SocketInputError::Runtime)?,
    )
}

pub fn run_tcp_socket_session_on_listener_with_binding(
    listener: &TcpListener,
    binding: TemplateBinding,
) -> Result<ExportBundle, SocketInputError> {
    let (stream, _) = listener
        .accept()
        .map_err(|err| SocketInputError::AcceptFailed(err.to_string()))?;
    let _ = stream.set_read_timeout(Some(std::time::Duration::from_secs(5)));
    run_stream_session(
        BufReader::new(stream),
        SessionConfig::for_binding(binding).map_err(SocketInputError::Runtime)?,
    )
}

fn run_stream_session<R: Read>(
    reader: BufReader<R>,
    config: SessionConfig,
) -> Result<ExportBundle, SocketInputError> {
    let mut session = RuntimeSession::start(config).map_err(SocketInputError::Runtime)?;
    let mut window_end = SystemTime::UNIX_EPOCH;
    let mut line = String::new();
    let mut fact_count = 0usize;

    let mut reader = reader;
    loop {
        line.clear();
        let read = reader
            .read_line(&mut line)
            .map_err(|err| SocketInputError::ReadFailed(err.to_string()))?;
        if read == 0 {
            break;
        }
        if line.len() > MAX_FACT_LINE_BYTES {
            return Err(SocketInputError::LimitExceeded(format!(
                "fact line exceeded {} bytes",
                MAX_FACT_LINE_BYTES
            )));
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        fact_count += 1;
        if fact_count > MAX_FACT_COUNT {
            return Err(SocketInputError::LimitExceeded(format!(
                "fact count exceeded {} records",
                MAX_FACT_COUNT
            )));
        }
        let fact = fact_from_json(trimmed).map_err(SocketInputError::ParseFailed)?;
        window_end = window_end.max(fact.ts);
        session.ingest(fact);
    }

    session.freeze(window_end);
    Ok(session.export_bundle())
}

#[cfg(not(target_family = "unix"))]
pub fn remove_unix_socket_file(_socket_path: &str) -> Result<(), SocketInputError> {
    Err(SocketInputError::UnsupportedPlatform)
}

#[cfg(not(target_family = "unix"))]
pub fn run_unix_socket_session(
    _socket_path: &str,
    _template: Template,
) -> Result<ExportBundle, SocketInputError> {
    Err(SocketInputError::UnsupportedPlatform)
}

#[cfg(not(target_family = "unix"))]
pub fn run_unix_socket_session_with_binding(
    _socket_path: &str,
    _binding: TemplateBinding,
) -> Result<ExportBundle, SocketInputError> {
    Err(SocketInputError::UnsupportedPlatform)
}

#[cfg(test)]
mod tests {
    use super::{MAX_FACT_COUNT, MAX_FACT_LINE_BYTES, SocketInputError, run_stream_session};
    use crate::export::fact_to_json;
    use crate::ledger::{
        CpuId, FactEnvelope, FactId, FactKind, PacketDir, PacketMetaFact, SessionId,
    };
    use crate::runtime::SessionConfig;
    use crate::template::udp_debug_template;
    use std::io::BufReader;
    use std::time::{Duration, SystemTime};

    fn valid_fact_line() -> String {
        let packet = FactEnvelope {
            id: FactId(1),
            ts: SystemTime::UNIX_EPOCH + Duration::from_millis(10),
            cpu: CpuId(0),
            ifindex: Some(2),
            session: SessionId(1),
            fragment_id: "udp_packet_meta_fragment".into(),
            kind: FactKind::PacketMeta(PacketMetaFact {
                netns: 1,
                sk_cookie: Some(123),
                dir: PacketDir::Egress,
                local_port: None,
                remote_port: None,
                payload_byte0: None,
                payload_byte1: None,
                payload_prefix2: None,
                payload_prefix4: None,
                payload_byte4: None,
                payload_byte5: None,
                payload_byte9: None,
                payload_byte10: None,
                payload_byte13: None,
                l3_proto: 0x0800,
                l4_proto: 17,
                tot_len: 88,
                tcp_flags: 0,
                seq: None,
                ack: None,
                window: None,
            }),
        };
        format!("{}\n", fact_to_json(&packet))
    }

    #[test]
    fn run_stream_session_rejects_oversized_fact_line() {
        let oversized = format!("{}\n", "x".repeat(MAX_FACT_LINE_BYTES + 1));
        let err = run_stream_session(
            BufReader::new(std::io::Cursor::new(oversized.into_bytes())),
            SessionConfig::for_template(udp_debug_template()).unwrap(),
        )
        .unwrap_err();
        assert!(
            matches!(err, SocketInputError::LimitExceeded(message) if message.contains("fact line exceeded"))
        );
    }

    #[test]
    fn run_stream_session_rejects_excessive_fact_count() {
        let mut input = String::new();
        for _ in 0..=MAX_FACT_COUNT {
            input.push_str(&valid_fact_line());
        }
        let err = run_stream_session(
            BufReader::new(std::io::Cursor::new(input.into_bytes())),
            SessionConfig::for_template(udp_debug_template()).unwrap(),
        )
        .unwrap_err();
        assert!(
            matches!(err, SocketInputError::LimitExceeded(message) if message.contains("fact count exceeded"))
        );
    }
}
