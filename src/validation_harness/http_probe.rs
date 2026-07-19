use std::io::{Read, Write};
use std::net::{Shutdown, TcpStream, ToSocketAddrs};
use std::time::Duration;

use super::command::ValidationError;
use crate::transport_safety::connect_with_deadline;

pub(super) const MAX_HTTP_RESPONSE_BYTES: usize = 1024 * 1024;
const HTTP_IO_TIMEOUT: Duration = Duration::from_secs(2);

pub(super) fn bounded_tcp_connect(addr: impl ToSocketAddrs) -> Result<TcpStream, ValidationError> {
    bounded_tcp_connect_with_timeout(addr, HTTP_IO_TIMEOUT)
}

fn bounded_tcp_connect_with_timeout(
    addr: impl ToSocketAddrs,
    timeout: Duration,
) -> Result<TcpStream, ValidationError> {
    let stream = connect_with_deadline(addr, timeout).map_err(|error| {
        ValidationError::new(format!("cannot connect to TCP endpoint: {error}"))
    })?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    Ok(stream)
}

pub(super) fn bounded_http_get(addr: &str, path: &str) -> Result<String, ValidationError> {
    Ok(bounded_http_request(addr, addr, path, &[])?.raw)
}

pub(super) fn bounded_http_get_body(
    endpoint: impl ToSocketAddrs,
    authority: &str,
    path: &str,
    headers: &[(&str, &str)],
) -> Result<String, ValidationError> {
    Ok(bounded_http_request(endpoint, authority, path, headers)?.body)
}

struct HttpProbeResponse {
    raw: String,
    body: String,
}

fn bounded_http_request(
    endpoint: impl ToSocketAddrs,
    authority: &str,
    path: &str,
    headers: &[(&str, &str)],
) -> Result<HttpProbeResponse, ValidationError> {
    validate_request_component(authority, "authority", false)?;
    validate_request_component(path, "path", true)?;
    for (name, value) in headers {
        if !is_http_header_name(name) || value.bytes().any(|byte| byte.is_ascii_control()) {
            return Err(ValidationError::new("HTTP request header is invalid"));
        }
    }

    let mut stream = bounded_tcp_connect(endpoint)?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {authority}\r\nConnection: close\r\n"
    )?;
    for (name, value) in headers {
        write!(stream, "{name}: {value}\r\n")?;
    }
    write!(stream, "\r\n")?;
    stream.shutdown(Shutdown::Write).ok();

    let mut response_bytes = Vec::new();
    Read::by_ref(&mut stream)
        .take((MAX_HTTP_RESPONSE_BYTES + 1) as u64)
        .read_to_end(&mut response_bytes)?;
    if response_bytes.len() > MAX_HTTP_RESPONSE_BYTES {
        return Err(ValidationError::new(format!(
            "HTTP response exceeds {MAX_HTTP_RESPONSE_BYTES} bytes"
        )));
    }
    let response = String::from_utf8(response_bytes)
        .map_err(|_| ValidationError::new("HTTP response is not valid UTF-8"))?;
    let status = response.lines().next().unwrap_or("<missing status line>");
    let mut parts = status.split_ascii_whitespace();
    if !matches!(parts.next(), Some("HTTP/1.1" | "HTTP/1.0")) || parts.next() != Some("200") {
        return Err(ValidationError::new(format!(
            "HTTP endpoint did not return 200: {status}"
        )));
    }
    let (response_headers, body) = response
        .split_once("\r\n\r\n")
        .ok_or_else(|| ValidationError::new("HTTP response is missing the header terminator"))?;
    let mut transfer_encoding = None;
    for line in response_headers.split("\r\n").skip(1) {
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| ValidationError::new("HTTP response header is malformed"))?;
        if !is_http_header_name(name) {
            return Err(ValidationError::new("HTTP response header name is invalid"));
        }
        if name.eq_ignore_ascii_case("transfer-encoding")
            && transfer_encoding.replace(value.trim()).is_some()
        {
            return Err(ValidationError::new(
                "HTTP response repeats Transfer-Encoding",
            ));
        }
    }
    let body = match transfer_encoding {
        None => body.to_string(),
        Some(value) if value.eq_ignore_ascii_case("chunked") => decode_chunked_body(body)?,
        Some(_) => {
            return Err(ValidationError::new(
                "HTTP response uses an unsupported Transfer-Encoding",
            ));
        }
    };
    Ok(HttpProbeResponse {
        raw: response,
        body,
    })
}

fn is_http_header_name(name: &str) -> bool {
    !name.is_empty()
        && name.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(
                    byte,
                    b'!' | b'#'
                        | b'$'
                        | b'%'
                        | b'&'
                        | b'\''
                        | b'*'
                        | b'+'
                        | b'-'
                        | b'.'
                        | b'^'
                        | b'_'
                        | b'`'
                        | b'|'
                        | b'~'
                )
        })
}

fn decode_chunked_body(body: &str) -> Result<String, ValidationError> {
    let mut remaining = body;
    let mut decoded = String::new();
    loop {
        let Some((size_hex, after_size)) = remaining.split_once("\r\n") else {
            return Err(ValidationError::new("invalid chunked HTTP body"));
        };
        let size = usize::from_str_radix(size_hex.trim(), 16)
            .map_err(|err| ValidationError::new(format!("invalid chunk size: {err}")))?;
        if size == 0 {
            return Ok(decoded);
        }
        let framed_size = size
            .checked_add(2)
            .ok_or_else(|| ValidationError::new("chunk size exceeds platform limits"))?;
        if after_size.len() < framed_size {
            return Err(ValidationError::new("truncated chunked HTTP body"));
        }
        let chunk = after_size
            .get(..size)
            .ok_or_else(|| ValidationError::new("chunk boundary splits UTF-8 data"))?;
        if after_size.as_bytes().get(size..framed_size) != Some(b"\r\n") {
            return Err(ValidationError::new("chunk is missing its CRLF terminator"));
        }
        decoded.push_str(chunk);
        remaining = &after_size[framed_size..];
    }
}

fn validate_request_component(
    value: &str,
    name: &str,
    require_slash: bool,
) -> Result<(), ValidationError> {
    if value.is_empty()
        || (require_slash && !value.starts_with('/'))
        || value.bytes().any(|byte| {
            !byte.is_ascii()
                || byte.is_ascii_control()
                || byte.is_ascii_whitespace()
                || byte == b'#'
        })
    {
        return Err(ValidationError::new(format!(
            "HTTP request {name} is invalid"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, ToSocketAddrs};
    use std::thread;

    use super::*;

    fn serve_response(chunks: Vec<Vec<u8>>) -> (String, thread::JoinHandle<()>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap().to_string();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = Vec::new();
            Read::by_ref(&mut stream)
                .take(8192)
                .read_to_end(&mut request)
                .unwrap();
            assert!(request.windows(4).any(|window| window == b"\r\n\r\n"));
            for chunk in chunks {
                if stream.write_all(&chunk).is_err() {
                    break;
                }
            }
        });
        (addr, server)
    }

    #[test]
    fn request_components_reject_injection_and_ambiguous_paths() {
        assert!(validate_request_component("127.0.0.1:8080", "authority", false).is_ok());
        assert!(validate_request_component("/health?full=true", "path", true).is_ok());
        for authority in ["", "host name:80", "host:80\r\nInjected:x"] {
            assert!(validate_request_component(authority, "authority", false).is_err());
        }
        for path in ["health", "/a b", "/a#fragment", "/雪"] {
            assert!(validate_request_component(path, "path", true).is_err());
        }
    }

    #[test]
    fn address_resolution_consumes_the_shared_connect_budget() {
        struct SlowAddress(SocketAddr);

        impl ToSocketAddrs for SlowAddress {
            type Iter = std::vec::IntoIter<SocketAddr>;

            fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
                thread::sleep(Duration::from_millis(20));
                Ok(vec![self.0].into_iter())
            }
        }

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        assert!(
            bounded_tcp_connect_with_timeout(
                SlowAddress(listener.local_addr().unwrap()),
                Duration::from_millis(1)
            )
            .is_err()
        );
    }

    #[test]
    fn connector_never_attempts_more_than_eight_resolved_addresses() {
        struct ManyAddresses(Vec<SocketAddr>);

        impl ToSocketAddrs for ManyAddresses {
            type Iter = std::vec::IntoIter<SocketAddr>;

            fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
                Ok(self.0.clone().into_iter())
            }
        }

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let unreachable = "127.0.0.1:0".parse().unwrap();
        let mut addresses = vec![unreachable; crate::transport_safety::MAX_RESOLVED_ADDRESSES];
        addresses.push(listener.local_addr().unwrap());

        assert!(
            bounded_tcp_connect_with_timeout(ManyAddresses(addresses), Duration::from_secs(1))
                .is_err()
        );
    }

    #[test]
    fn accepts_a_fragmented_valid_response() {
        let (addr, server) = serve_response(vec![
            b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n".to_vec(),
            b"\r\n{}".to_vec(),
        ]);

        let response = bounded_http_get(&addr, "/health").unwrap();
        server.join().unwrap();
        assert!(response.ends_with("{}"));
    }

    #[test]
    fn rejects_an_ambiguous_success_status() {
        let (addr, server) = serve_response(vec![
            b"HTTP/1.1 200evil NOPE\r\nContent-Length: 2\r\n\r\n{}".to_vec(),
        ]);

        let result = bounded_http_get(&addr, "/health");
        server.join().unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn rejects_an_oversized_response() {
        let mut response = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\n".to_vec();
        response.resize(MAX_HTTP_RESPONSE_BYTES + 1, b'x');
        let (addr, server) = serve_response(vec![response]);

        let result = bounded_http_get(&addr, "/health");
        server.join().unwrap();
        assert!(result.is_err());
    }

    #[test]
    fn chunk_decoder_rejects_overflow_bad_terminators_and_utf8_splits() {
        let overflow = format!("{:x}\r\n", usize::MAX);
        assert!(decode_chunked_body(&overflow).is_err());
        assert!(decode_chunked_body("1\r\naXX0\r\n\r\n").is_err());
        assert!(decode_chunked_body("1\r\né\r\n0\r\n\r\n").is_err());
    }
}
