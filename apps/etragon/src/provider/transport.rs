use super::{MAX_RESPONSE_BYTES, ProviderConfig, ProviderError};
use http::Uri;
use rustls::pki_types::{CertificateDer, ServerName, pem::PemObject};
use rustls::{ClientConfig, ClientConnection, RootCertStore, StreamOwned};
use silvortex_bounded_io::{connect_with_io_deadline, read_bounded_utf8_regular_file};
use std::io::{self, BufRead, BufReader, Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, ToSocketAddrs};
use std::path::Path;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
    mpsc,
};
use std::time::{Duration, Instant};

const MAX_HEADERS: usize = 16 * 1024;
const MAX_WIRE_BYTES: usize = MAX_RESPONSE_BYTES + 64 * 1024;
static RESOLVERS: AtomicUsize = AtomicUsize::new(0);

struct Endpoint {
    authority: String,
    host: String,
    port: u16,
    path: String,
}

impl Endpoint {
    fn parse(config: &ProviderConfig) -> Result<Self, ProviderError> {
        Self::from_url(
            &config.endpoint,
            config.ca_bundle.is_some(),
            config.allow_remote,
        )
    }

    fn from_url(url: &str, has_ca: bool, allow_remote: bool) -> Result<Self, ProviderError> {
        if url.len() > 2048 || url.contains('#') {
            return Err(ProviderError::InvalidConfig);
        }
        let uri: Uri = url.parse().map_err(|_| ProviderError::InvalidConfig)?;
        let tls = match uri.scheme_str() {
            Some("https") => true,
            Some("http") => false,
            _ => return Err(ProviderError::InvalidConfig),
        };
        let authority = uri
            .authority()
            .ok_or(ProviderError::InvalidConfig)?
            .as_str();
        if authority.contains('@') || uri.query().is_some() {
            return Err(ProviderError::InvalidConfig);
        }
        let host = uri
            .host()
            .ok_or(ProviderError::InvalidConfig)?
            .trim_matches(['[', ']']);
        let local = host.eq_ignore_ascii_case("localhost")
            || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback());
        if !local && !allow_remote {
            return Err(ProviderError::RemoteNotAllowed);
        }
        if (!local && !tls) || (tls != has_ca) {
            return Err(ProviderError::InvalidConfig);
        }
        let host_end = if authority.starts_with('[') {
            authority.find(']').ok_or(ProviderError::InvalidConfig)? + 1
        } else {
            authority.find(':').unwrap_or(authority.len())
        };
        let port = match &authority[host_end..] {
            "" => {
                if tls {
                    443
                } else {
                    80
                }
            }
            suffix => suffix
                .strip_prefix(':')
                .and_then(|text| text.parse::<u16>().ok())
                .filter(|port| *port != 0)
                .ok_or(ProviderError::InvalidConfig)?,
        };
        if tls && ServerName::try_from(host.to_owned()).is_err() {
            return Err(ProviderError::InvalidConfig);
        }
        Ok(Self {
            authority: authority.to_owned(),
            host: host.to_owned(),
            port,
            path: uri.path().to_owned(),
        })
    }
}

pub(super) fn validate_endpoint(config: &ProviderConfig) -> Result<(), ProviderError> {
    Endpoint::parse(config).map(|_| ())
}

fn io_error(error: io::Error) -> ProviderError {
    match error.kind() {
        io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock => ProviderError::Timeout,
        _ => ProviderError::Transport,
    }
}

fn remaining(deadline: Instant) -> Result<Duration, ProviderError> {
    let time = deadline.saturating_duration_since(Instant::now());
    if time.is_zero() {
        Err(ProviderError::Timeout)
    } else {
        Ok(time)
    }
}

fn resolve(endpoint: &Endpoint, deadline: Instant) -> Result<Vec<SocketAddr>, ProviderError> {
    if endpoint.host.eq_ignore_ascii_case("localhost") {
        return Ok(vec![SocketAddr::new(
            Ipv4Addr::LOCALHOST.into(),
            endpoint.port,
        )]);
    }
    if let Ok(ip) = endpoint.host.parse::<IpAddr>() {
        return Ok(vec![SocketAddr::new(ip, endpoint.port)]);
    }
    // OS DNS calls cannot be cancelled. Bound detached lookups so repeated timeouts
    // cannot create unbounded threads; localhost/literal IPs need no resolver thread.
    RESOLVERS
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |n| {
            (n < 4).then_some(n + 1)
        })
        .map_err(|_| ProviderError::Transport)?;
    struct Permit;
    impl Drop for Permit {
        fn drop(&mut self) {
            RESOLVERS.fetch_sub(1, Ordering::AcqRel);
        }
    }
    let permit = Permit;
    let host = endpoint.host.clone();
    let port = endpoint.port;
    let (sender, receiver) = mpsc::sync_channel(1);
    std::thread::Builder::new()
        .name("etragon-dns".into())
        .spawn(move || {
            let _permit = permit;
            let result = (host.as_str(), port)
                .to_socket_addrs()
                .map(|addresses| addresses.take(16).collect::<Vec<_>>());
            let _ = sender.send(result);
        })
        .map_err(|_| ProviderError::Transport)?;
    receiver
        .recv_timeout(remaining(deadline)?)
        .map_err(|error| match error {
            mpsc::RecvTimeoutError::Timeout => ProviderError::Timeout,
            mpsc::RecvTimeoutError::Disconnected => ProviderError::Transport,
        })?
        .map_err(io_error)
}

fn tls_config(path: &Path) -> Result<Arc<ClientConfig>, ProviderError> {
    let pem = read_bounded_utf8_regular_file(path, 1024 * 1024).map_err(|_| ProviderError::Tls)?;
    let mut roots = RootCertStore::empty();
    for certificate in CertificateDer::pem_slice_iter(pem.as_bytes()) {
        roots
            .add(certificate.map_err(|_| ProviderError::Tls)?)
            .map_err(|_| ProviderError::Tls)?;
    }
    if roots.is_empty() {
        return Err(ProviderError::Tls);
    }
    let mut client =
        ClientConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))
            .with_safe_default_protocol_versions()
            .map_err(|_| ProviderError::Tls)?
            .with_root_certificates(roots)
            .with_no_client_auth();
    client.alpn_protocols = vec![b"http/1.1".to_vec()];
    Ok(Arc::new(client))
}

pub(super) fn post_json(config: &ProviderConfig, body: &[u8]) -> Result<Vec<u8>, ProviderError> {
    config.validate()?;
    let endpoint = Endpoint::parse(config)?;
    let authorization = match &config.api_key_env {
        None => String::new(),
        Some(key) => {
            let token = std::env::var(key).map_err(|_| ProviderError::MissingCredential)?;
            validate_token(&token)?;
            format!("Authorization: Bearer {token}\r\n")
        }
    };
    let header = format!(
        "POST {} HTTP/1.1\r\nHost: {}\r\nContent-Type: application/json\r\nAccept: application/json\r\nAccept-Encoding: identity\r\nConnection: close\r\n{authorization}Content-Length: {}\r\n\r\n",
        endpoint.path,
        endpoint.authority,
        body.len(),
    );
    request_json(
        endpoint,
        config.ca_bundle.as_deref(),
        config.timeout_ms,
        &header,
        body,
    )
}

pub(super) fn validate_source(url: &str, ca: Option<&Path>) -> Result<(), ProviderError> {
    Endpoint::from_url(url, ca.is_some(), true).map(|_| ())
}

pub(super) fn get_snapshot(url: &str, ca: Option<&Path>) -> Result<String, ProviderError> {
    let endpoint = Endpoint::from_url(url, ca.is_some(), true)?;
    let token_header = match std::env::var("ETRAGON_SOURCE_ADMIN_TOKEN") {
        Ok(token) => {
            validate_token(&token)?;
            format!("X-Gewyvern-Admin-Token: {token}\r\n")
        }
        Err(std::env::VarError::NotPresent) => String::new(),
        Err(_) => return Err(ProviderError::MissingCredential),
    };
    let header = format!(
        "GET {} HTTP/1.1\r\nHost: {}\r\nAccept: application/json\r\nAccept-Encoding: identity\r\nConnection: close\r\n{token_header}\r\n",
        endpoint.path, endpoint.authority,
    );
    let bytes = request_json(endpoint, ca, 5000, &header, &[])?;
    String::from_utf8(bytes).map_err(|_| ProviderError::InvalidInput)
}

fn validate_token(token: &str) -> Result<(), ProviderError> {
    if token.is_empty() || token.len() > 8192 || !token.bytes().all(|c| c.is_ascii_graphic()) {
        return Err(ProviderError::MissingCredential);
    }
    Ok(())
}

fn request_json(
    endpoint: Endpoint,
    ca: Option<&Path>,
    timeout_ms: u64,
    header: &str,
    body: &[u8],
) -> Result<Vec<u8>, ProviderError> {
    let tls = ca.map(tls_config).transpose()?;
    let deadline = Instant::now() + Duration::from_millis(timeout_ms);
    let addresses = resolve(&endpoint, deadline)?;
    let mut socket = None;
    let mut last_error = ProviderError::Transport;
    for address in addresses {
        match connect_with_io_deadline(address, remaining(deadline)?) {
            Ok(stream) => {
                socket = Some(stream);
                break;
            }
            Err(error) => {
                last_error = io_error(error);
            }
        }
    }
    let socket = socket.ok_or(last_error)?;
    if let Some(tls) = tls {
        let name = ServerName::try_from(endpoint.host).map_err(|_| ProviderError::InvalidConfig)?;
        let connection = ClientConnection::new(tls, name).map_err(|_| ProviderError::Tls)?;
        exchange(&mut StreamOwned::new(connection, socket), header, body)
    } else {
        exchange(&mut { socket }, header, body)
    }
}

fn exchange(
    stream: &mut (impl Read + Write),
    header: &str,
    body: &[u8],
) -> Result<Vec<u8>, ProviderError> {
    stream
        .write_all(header.as_bytes())
        .and_then(|()| stream.write_all(body))
        .and_then(|()| stream.flush())
        .map_err(io_error)?;
    read_response(stream)
}

fn line(reader: &mut impl BufRead) -> Result<Vec<u8>, ProviderError> {
    let mut bytes = Vec::new();
    reader.read_until(b'\n', &mut bytes).map_err(io_error)?;
    if bytes.len() > MAX_HEADERS {
        return Err(ProviderError::TooLarge);
    }
    if !bytes.ends_with(b"\r\n") {
        return Err(ProviderError::InvalidResponse);
    }
    bytes.truncate(bytes.len() - 2);
    Ok(bytes)
}

fn read_response(stream: &mut impl Read) -> Result<Vec<u8>, ProviderError> {
    let mut reader = BufReader::new(stream.take(MAX_WIRE_BYTES as u64));
    let status = line(&mut reader)?;
    let status = std::str::from_utf8(&status).map_err(|_| ProviderError::InvalidResponse)?;
    let parts: Vec<_> = status.splitn(3, ' ').collect();
    if parts.len() < 2
        || !["HTTP/1.1", "HTTP/1.0"].contains(&parts[0])
        || parts[1].len() != 3
        || !parts[1].bytes().all(|c| c.is_ascii_digit())
    {
        return Err(ProviderError::InvalidResponse);
    }
    let status = parts[1]
        .parse::<u16>()
        .map_err(|_| ProviderError::InvalidResponse)?;
    if status != 200 {
        return Err(ProviderError::HttpStatus(status));
    }
    let mut size = 0;
    let mut length = None;
    let mut chunked = false;
    let mut json_type = false;
    loop {
        let raw = line(&mut reader)?;
        size += raw.len() + 2;
        if size > MAX_HEADERS {
            return Err(ProviderError::TooLarge);
        }
        if raw.is_empty() {
            break;
        }
        let text = std::str::from_utf8(&raw).map_err(|_| ProviderError::InvalidResponse)?;
        let (name, value) = text.split_once(':').ok_or(ProviderError::InvalidResponse)?;
        if !silvortex_bounded_io::is_http_header_name(name)
            || value.bytes().any(|c| c < 32 && c != b'\t')
        {
            return Err(ProviderError::InvalidResponse);
        }
        let value = value.trim();
        match name.to_ascii_lowercase().as_str() {
            "content-length" => {
                if length.is_some() {
                    return Err(ProviderError::InvalidResponse);
                }
                length = Some(
                    silvortex_bounded_io::parse_http_content_length(value)
                        .ok_or(ProviderError::InvalidResponse)?,
                );
            }
            "transfer-encoding" => {
                if chunked || !value.eq_ignore_ascii_case("chunked") {
                    return Err(ProviderError::InvalidResponse);
                }
                chunked = true;
            }
            "content-type" => {
                if json_type
                    || !value
                        .split(';')
                        .next()
                        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
                {
                    return Err(ProviderError::InvalidResponse);
                }
                json_type = true;
            }
            "content-encoding" if !value.eq_ignore_ascii_case("identity") => {
                return Err(ProviderError::InvalidResponse);
            }
            _ => (),
        }
    }
    if !json_type || (chunked && length.is_some()) {
        return Err(ProviderError::InvalidResponse);
    }
    let mut output = Vec::new();
    if chunked {
        loop {
            let raw = line(&mut reader)?;
            let digits = raw
                .split(|&c| c == b';')
                .next()
                .ok_or(ProviderError::InvalidResponse)?;
            if digits.is_empty() || digits.len() > 8 || !digits.iter().all(u8::is_ascii_hexdigit) {
                return Err(ProviderError::InvalidResponse);
            }
            let count = usize::from_str_radix(
                std::str::from_utf8(digits).map_err(|_| ProviderError::InvalidResponse)?,
                16,
            )
            .map_err(|_| ProviderError::InvalidResponse)?;
            if count == 0 {
                // No trailers are part of this small inference transport contract.
                if !line(&mut reader)?.is_empty() {
                    return Err(ProviderError::InvalidResponse);
                }
                break;
            }
            append_exact(&mut reader, &mut output, count)?;
            if !line(&mut reader)?.is_empty() {
                return Err(ProviderError::InvalidResponse);
            }
        }
    } else if let Some(length) = length {
        append_exact(&mut reader, &mut output, length)?;
    } else {
        reader
            .take(MAX_RESPONSE_BYTES as u64 + 1)
            .read_to_end(&mut output)
            .map_err(io_error)?;
        if output.len() > MAX_RESPONSE_BYTES {
            return Err(ProviderError::TooLarge);
        }
    }
    Ok(output)
}

fn append_exact(
    reader: &mut impl Read,
    output: &mut Vec<u8>,
    count: usize,
) -> Result<(), ProviderError> {
    if count > MAX_RESPONSE_BYTES.saturating_sub(output.len()) {
        return Err(ProviderError::TooLarge);
    }
    let start = output.len();
    output.resize(start + count, 0);
    reader.read_exact(&mut output[start..]).map_err(io_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_http_framing() {
        let cases = [
            (
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}",
                true,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\n2\r\n{}\r\n0\r\n\r\n",
                true,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nContent-Length: 2\r\n\r\n{}",
                false,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\nTransfer-Encoding: chunked\r\n\r\n",
                false,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 999999\r\n\r\n",
                false,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nTransfer-Encoding: chunked\r\n\r\nffffffff\r\n",
                false,
            ),
            (
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{",
                false,
            ),
            ("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\n\r\n", false),
            (
                "HTTP/1.1 302 Found\r\nLocation: https://secret.invalid/\r\n\r\n",
                false,
            ),
        ];
        for (wire, success) in cases {
            assert_eq!(
                read_response(&mut wire.as_bytes()).is_ok(),
                success,
                "{wire}"
            );
        }
    }
}
