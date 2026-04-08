use crate::export::{fact_from_json, ExportBundle, ExportError};
use crate::runtime::{RuntimeError, RuntimeSession, SessionConfig};
use crate::template::Template;
use std::io::{BufRead, BufReader, Read};
use std::net::TcpListener;
use std::time::SystemTime;

#[derive(Debug)]
pub enum SocketInputError {
    UnsupportedPlatform,
    BindFailed(String),
    AcceptFailed(String),
    ReadFailed(String),
    ParseFailed(ExportError),
    Runtime(RuntimeError),
}

#[cfg(target_family = "unix")]
pub fn run_unix_socket_session(
    socket_path: &str,
    template: Template,
) -> Result<ExportBundle, SocketInputError> {
    use std::fs;

    let _ = fs::remove_file(socket_path);
    let listener = bind_unix_socket_listener(socket_path)?;
    let export = run_unix_socket_session_on_listener(&listener, template)?;
    let _ = fs::remove_file(socket_path);
    Ok(export)
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
    run_stream_session(BufReader::new(stream), template)
}

pub fn run_tcp_socket_session(
    bind_addr: &str,
    template: Template,
) -> Result<ExportBundle, SocketInputError> {
    let listener =
        TcpListener::bind(bind_addr).map_err(|err| SocketInputError::BindFailed(err.to_string()))?;
    run_tcp_socket_session_on_listener(&listener, template)
}

pub fn run_tcp_socket_session_on_listener(
    listener: &TcpListener,
    template: Template,
) -> Result<ExportBundle, SocketInputError> {
    let (stream, _) = listener
        .accept()
        .map_err(|err| SocketInputError::AcceptFailed(err.to_string()))?;
    run_stream_session(BufReader::new(stream), template)
}

fn run_stream_session<R: Read>(
    reader: BufReader<R>,
    template: Template,
) -> Result<ExportBundle, SocketInputError> {
    let config = SessionConfig::for_template(template).map_err(SocketInputError::Runtime)?;
    let mut session = RuntimeSession::start(config).map_err(SocketInputError::Runtime)?;
    let mut window_end = SystemTime::UNIX_EPOCH;

    for line in reader.lines() {
        let line = line.map_err(|err| SocketInputError::ReadFailed(err.to_string()))?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let fact = fact_from_json(line).map_err(SocketInputError::ParseFailed)?;
        window_end = window_end.max(fact.ts);
        session.ingest(fact);
    }

    session.freeze(window_end);
    Ok(session.export_bundle())
}

#[cfg(not(target_family = "unix"))]
pub fn run_unix_socket_session(
    _socket_path: &str,
    _template: Template,
) -> Result<ExportBundle, SocketInputError> {
    Err(SocketInputError::UnsupportedPlatform)
}
