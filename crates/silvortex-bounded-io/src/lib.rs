//! Product-neutral bounded I/O primitives for native service boundaries.

use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, TcpStream, ToSocketAddrs};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

pub const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
pub const MAX_RESOLVED_ADDRESSES: usize = 8;
pub const MAX_HTTPS_AUTHORITY_BYTES: usize = 320;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HttpsOrigin<'a> {
    authority: &'a str,
    host: &'a str,
    port: u16,
    ipv6: bool,
}

impl<'a> HttpsOrigin<'a> {
    pub fn authority(self) -> &'a str {
        self.authority
    }

    pub fn host(self) -> &'a str {
        self.host
    }

    pub fn port(self) -> u16 {
        self.port
    }

    pub fn host_header(self) -> String {
        if self.ipv6 {
            format!("[{}]:{}", self.host, self.port)
        } else {
            format!("{}:{}", self.host, self.port)
        }
    }
}

pub struct BoundedFile {
    file: File,
    remaining: u64,
}

pub struct DeadlineTcpStream {
    stream: TcpStream,
    deadline: Instant,
}

impl DeadlineTcpStream {
    fn new(stream: TcpStream, deadline: Instant) -> io::Result<Self> {
        let transport = Self { stream, deadline };
        transport.refresh_read_timeout()?;
        transport.refresh_write_timeout()?;
        Ok(transport)
    }

    fn remaining(&self) -> io::Result<Duration> {
        let remaining = self.deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Err(io::Error::new(
                io::ErrorKind::TimedOut,
                "transport I/O deadline elapsed",
            ));
        }
        Ok(remaining)
    }

    fn refresh_read_timeout(&self) -> io::Result<()> {
        self.stream.set_read_timeout(Some(self.remaining()?))
    }

    fn refresh_write_timeout(&self) -> io::Result<()> {
        self.stream.set_write_timeout(Some(self.remaining()?))
    }
}

impl Read for DeadlineTcpStream {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        self.refresh_read_timeout()?;
        self.stream.read(buffer)
    }
}

impl Write for DeadlineTcpStream {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        self.refresh_write_timeout()?;
        self.stream.write(buffer)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.refresh_write_timeout()?;
        self.stream.flush()
    }
}

impl BoundedFile {
    pub fn metadata(&self) -> io::Result<Metadata> {
        self.file.metadata()
    }
}

impl Read for BoundedFile {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        if self.remaining == 0 {
            let mut overflow = [0_u8; 1];
            return match self.file.read(&mut overflow)? {
                0 => Ok(0),
                _ => Err(io::Error::new(
                    io::ErrorKind::InvalidData,
                    "file grew beyond the configured size limit",
                )),
            };
        }

        let remaining = usize::try_from(self.remaining).unwrap_or(usize::MAX);
        let allowed = remaining.min(buffer.len());
        let read = self.file.read(&mut buffer[..allowed])?;
        self.remaining -= read as u64;
        Ok(read)
    }
}

pub fn is_http_header_name(name: &str) -> bool {
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

pub fn parse_http_content_length(value: &str) -> Option<usize> {
    if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

pub fn parse_http_status_code(value: &str) -> Option<u16> {
    if value.len() != 3 || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value
        .parse()
        .ok()
        .filter(|status| (100..=599).contains(status))
}

pub fn parse_https_origin(value: &str) -> Option<HttpsOrigin<'_>> {
    let authority = value.strip_prefix("https://")?;
    if authority.is_empty()
        || authority.len() > MAX_HTTPS_AUTHORITY_BYTES
        || !authority.is_ascii()
        || authority.bytes().any(|byte| {
            byte <= 0x20 || byte == 0x7f || matches!(byte, b'/' | b'?' | b'#' | b'@' | b'\\')
        })
    {
        return None;
    }

    if let Some(bracketed) = authority.strip_prefix('[') {
        let (host, suffix) = bracketed.split_once(']')?;
        let port = match suffix.strip_prefix(':') {
            Some(value) => parse_https_port(value)?,
            None if suffix.is_empty() => 443,
            None => return None,
        };
        host.parse::<Ipv6Addr>().ok()?;
        return Some(HttpsOrigin {
            authority,
            host,
            port,
            ipv6: true,
        });
    }

    if authority.matches(':').count() > 1 {
        return None;
    }
    let (host, port) = match authority.rsplit_once(':') {
        Some((host, value)) => (host, parse_https_port(value)?),
        None => (authority, 443),
    };
    if !valid_https_host(host) {
        return None;
    }
    Some(HttpsOrigin {
        authority,
        host,
        port,
        ipv6: false,
    })
}

fn parse_https_port(value: &str) -> Option<u16> {
    if value.is_empty()
        || !value.bytes().all(|byte| byte.is_ascii_digit())
        || (value.len() > 1 && value.starts_with('0'))
    {
        return None;
    }
    value.parse().ok().filter(|port| *port != 0)
}

fn valid_https_host(host: &str) -> bool {
    if host.is_empty() || host.len() > 253 || host.ends_with('.') {
        return false;
    }
    if host
        .bytes()
        .all(|byte| byte.is_ascii_digit() || byte == b'.')
    {
        return host.parse::<Ipv4Addr>().is_ok();
    }
    host.split('.').all(|label| {
        !label.is_empty()
            && label.len() <= 63
            && label
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
            && label
                .as_bytes()
                .first()
                .is_some_and(u8::is_ascii_alphanumeric)
            && label
                .as_bytes()
                .last()
                .is_some_and(u8::is_ascii_alphanumeric)
    })
}

pub fn open_bounded_regular_file(path: &Path, max_bytes: u64) -> io::Result<BoundedFile> {
    open_bounded_regular_file_inner(path, max_bytes, false)
}

/// Opens a regular, non-symlink file with a hard read limit while permitting empty files.
pub fn open_bounded_regular_file_allow_empty(
    path: &Path,
    max_bytes: u64,
) -> io::Result<BoundedFile> {
    open_bounded_regular_file_inner(path, max_bytes, true)
}

/// Reads a regular, non-symlink UTF-8 file without trusting path metadata or file size races.
///
/// Empty text files are accepted; callers that require content should validate it separately.
pub fn read_bounded_utf8_regular_file(path: &Path, max_bytes: u64) -> io::Result<String> {
    let mut file = open_bounded_regular_file_inner(path, max_bytes, true)?;
    let capacity = usize::try_from(file.metadata()?.len()).unwrap_or(0);
    let mut bytes = Vec::with_capacity(capacity);
    file.read_to_end(&mut bytes)?;
    String::from_utf8(bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))
}

fn open_bounded_regular_file_inner(
    path: &Path,
    max_bytes: u64,
    allow_empty: bool,
) -> io::Result<BoundedFile> {
    if max_bytes == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "file size limit must be non-zero",
        ));
    }

    #[cfg(not(unix))]
    if std::fs::symlink_metadata(path)?.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "path must not be a symlink",
        ));
    }

    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file()
        || (!allow_empty && metadata.len() == 0)
        || metadata.len() > max_bytes
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            if allow_empty {
                "file must be regular and within the size limit"
            } else {
                "file must be regular, non-empty, and within the size limit"
            },
        ));
    }
    Ok(BoundedFile {
        file,
        remaining: max_bytes,
    })
}

pub fn connect_with_deadline(
    address: impl ToSocketAddrs,
    timeout: Duration,
) -> io::Result<TcpStream> {
    if timeout.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "connection timeout must be non-zero",
        ));
    }
    let started_at = Instant::now();
    let deadline = started_at.checked_add(timeout).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "connection timeout exceeds the supported duration",
        )
    })?;
    let addresses = address.to_socket_addrs()?;
    let mut last_error = None;
    for address in addresses.take(MAX_RESOLVED_ADDRESSES) {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            break;
        }
        match TcpStream::connect_timeout(&address, remaining) {
            Ok(stream) => {
                stream.set_read_timeout(Some(timeout))?;
                stream.set_write_timeout(Some(timeout))?;
                return Ok(stream);
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| {
        io::Error::new(
            io::ErrorKind::AddrNotAvailable,
            "endpoint resolved no reachable addresses",
        )
    }))
}

pub fn connect_with_io_deadline(
    address: impl ToSocketAddrs,
    timeout: Duration,
) -> io::Result<DeadlineTcpStream> {
    let deadline = io_deadline_after(timeout)?;
    let stream = connect_with_deadline(address, timeout)?;
    DeadlineTcpStream::new(stream, deadline)
}

pub fn wrap_tcp_stream_with_io_deadline(
    stream: TcpStream,
    timeout: Duration,
) -> io::Result<DeadlineTcpStream> {
    DeadlineTcpStream::new(stream, io_deadline_after(timeout)?)
}

fn io_deadline_after(timeout: Duration) -> io::Result<Instant> {
    if timeout.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "transport I/O timeout must be non-zero",
        ));
    }
    Instant::now().checked_add(timeout).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "transport I/O timeout exceeds the supported duration",
        )
    })
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::fs::OpenOptions;
    use std::io::{Read, Write};
    use std::net::{SocketAddr, TcpListener, ToSocketAddrs};
    use std::thread;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    #[test]
    fn header_names_follow_the_http_token_grammar() {
        for valid in ["Content-Length", "x_custom", "x.y", "x~y"] {
            assert!(is_http_header_name(valid), "{valid}");
        }
        for invalid in ["", "Transfer-Encoding ", "bad:name", "x y", "雪"] {
            assert!(!is_http_header_name(invalid), "{invalid}");
        }
    }

    #[test]
    fn http_numbers_require_the_wire_decimal_grammar() {
        assert_eq!(parse_http_content_length("0"), Some(0));
        assert_eq!(parse_http_content_length("0012"), Some(12));
        assert_eq!(parse_http_content_length("12"), Some(12));
        assert_eq!(parse_http_status_code("200"), Some(200));
        assert_eq!(parse_http_status_code("599"), Some(599));

        for invalid in ["", "+0", "-0", " 0", "0 ", "1_0", "184467440737095516160"] {
            assert_eq!(parse_http_content_length(invalid), None, "{invalid:?}");
        }
        for invalid in ["", "20", "0200", "+200", "600", "999", "2O0"] {
            assert_eq!(parse_http_status_code(invalid), None, "{invalid:?}");
        }
    }

    #[test]
    fn https_origins_have_one_strict_cross_plane_grammar() {
        let cases = [
            ("https://localhost", "localhost", 443, "localhost:443"),
            (
                "https://host.example:7443",
                "host.example",
                7443,
                "host.example:7443",
            ),
            ("https://127.0.0.1:443", "127.0.0.1", 443, "127.0.0.1:443"),
            ("https://[::1]", "::1", 443, "[::1]:443"),
            (
                "https://[2001:db8::1]:9443",
                "2001:db8::1",
                9443,
                "[2001:db8::1]:9443",
            ),
        ];
        for (value, host, port, host_header) in cases {
            let origin = parse_https_origin(value).unwrap();
            assert_eq!(origin.authority(), value.strip_prefix("https://").unwrap());
            assert_eq!(origin.host(), host);
            assert_eq!(origin.port(), port);
            assert_eq!(origin.host_header(), host_header);
        }

        for invalid in [
            "http://host.example",
            "https://",
            "https://host.example/",
            "https://host.example?secret",
            "https://user@host.example",
            "https://host\\example",
            "https://host.example\u{7f}",
            "https://host.example:0",
            "https://host.example:+443",
            "https://host.example:0443",
            "https://host.example:65536",
            "https://::1",
            "https://[::1",
            "https://[::1]suffix",
            "https://127.1",
            "https://999.1.1.1",
            "https://-host.example",
            "https://host-.example",
            "https://host..example",
            "https://host.example.",
            "https://host_example",
            "https://host.例子",
        ] {
            assert!(parse_https_origin(invalid).is_none(), "{invalid:?}");
        }
    }

    #[test]
    fn connector_applies_a_nonzero_deadline_to_loopback() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        assert!(connect_with_deadline(listener.local_addr().unwrap(), Duration::ZERO).is_err());
        assert!(
            connect_with_deadline(listener.local_addr().unwrap(), Duration::from_secs(1)).is_ok()
        );
    }

    #[test]
    fn accepted_stream_wrapper_rejects_a_zero_io_deadline() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let peer = TcpStream::connect(listener.local_addr().unwrap()).unwrap();
        let (stream, _) = listener.accept().unwrap();

        assert!(wrap_tcp_stream_with_io_deadline(stream, Duration::ZERO).is_err());
        drop(peer);
    }

    #[test]
    fn address_resolution_consumes_the_connection_budget() {
        struct SlowAddress(SocketAddr);

        impl ToSocketAddrs for SlowAddress {
            type Iter = std::vec::IntoIter<SocketAddr>;

            fn to_socket_addrs(&self) -> io::Result<Self::Iter> {
                thread::sleep(Duration::from_millis(20));
                Ok(vec![self.0].into_iter())
            }
        }

        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        assert!(
            connect_with_deadline(
                SlowAddress(listener.local_addr().unwrap()),
                Duration::from_millis(1)
            )
            .is_err()
        );
    }

    #[test]
    fn io_deadline_is_absolute_across_trickled_bytes() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap();
        let peer = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            for _ in 0..32 {
                if stream.write_all(b"x").is_err() {
                    break;
                }
                thread::sleep(Duration::from_millis(10));
            }
        });
        let mut stream = connect_with_io_deadline(address, Duration::from_millis(50)).unwrap();
        let mut received = 0;
        let error = loop {
            match stream.read(&mut [0_u8; 1]) {
                Ok(1) => received += 1,
                Ok(_) => panic!("trickle peer ended before the I/O deadline"),
                Err(error) => break error,
            }
        };

        assert!(matches!(
            error.kind(),
            io::ErrorKind::TimedOut | io::ErrorKind::WouldBlock
        ));
        assert!(received < 32, "trickled bytes extended the I/O deadline");
        drop(stream);
        peer.join().unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn bounded_file_open_rejects_symlinks_atomically() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "silvortex-bounded-io-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("target.pem");
        let empty = root.join("empty.pem");
        let link = root.join("link.pem");
        fs::write(&target, b"bounded").unwrap();
        fs::write(&empty, []).unwrap();
        symlink(&target, &link).unwrap();

        assert!(open_bounded_regular_file(&target, 7).is_ok());
        assert!(open_bounded_regular_file(&target, 6).is_err());
        assert!(open_bounded_regular_file(&target, 0).is_err());
        assert!(open_bounded_regular_file(&link, 7).is_err());
        assert!(open_bounded_regular_file(&empty, 7).is_err());
        assert!(open_bounded_regular_file_allow_empty(&empty, 7).is_ok());
        assert!(open_bounded_regular_file_allow_empty(&link, 7).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn bounded_file_open_rejects_fifo_without_waiting_for_a_writer() {
        use std::ffi::CString;
        use std::os::unix::ffi::OsStrExt;

        let root = std::env::temp_dir().join(format!(
            "silvortex-bounded-io-fifo-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let fifo = root.join("input.pipe");
        let fifo_path = CString::new(fifo.as_os_str().as_bytes()).unwrap();
        // SAFETY: fifo_path is a live NUL-terminated path and the requested mode is valid.
        assert_eq!(unsafe { libc::mkfifo(fifo_path.as_ptr(), 0o600) }, 0);

        let started = Instant::now();
        assert!(open_bounded_regular_file(&fifo, 1024).is_err());
        assert!(started.elapsed() < Duration::from_secs(5));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bounded_file_read_rejects_growth_after_open() {
        let path = std::env::temp_dir().join(format!(
            "silvortex-bounded-io-growth-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::write(&path, b"safe").unwrap();
        let mut file = open_bounded_regular_file(&path, 4).unwrap();
        OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(b"overflow")
            .unwrap();

        let mut bytes = Vec::new();
        assert!(file.read_to_end(&mut bytes).is_err());
        assert_eq!(bytes, b"safe");
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn bounded_utf8_reader_accepts_empty_text_and_rejects_oversized_or_invalid_input() {
        let root = std::env::temp_dir().join(format!(
            "silvortex-bounded-utf8-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let empty = root.join("empty.txt");
        let valid = root.join("valid.txt");
        let oversized = root.join("oversized.txt");
        let invalid = root.join("invalid.txt");
        fs::write(&empty, []).unwrap();
        fs::write(&valid, "hello 世界").unwrap();
        fs::write(&oversized, b"12345").unwrap();
        fs::write(&invalid, [0xff, 0xfe]).unwrap();

        assert_eq!(read_bounded_utf8_regular_file(&empty, 16).unwrap(), "");
        assert_eq!(
            read_bounded_utf8_regular_file(&valid, 32).unwrap(),
            "hello 世界"
        );
        assert!(read_bounded_utf8_regular_file(&oversized, 4).is_err());
        assert_eq!(
            read_bounded_utf8_regular_file(&invalid, 4)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidData
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn bounded_utf8_reader_rejects_symlinks() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join(format!(
            "silvortex-bounded-utf8-link-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&root).unwrap();
        let target = root.join("target.txt");
        let link = root.join("link.txt");
        fs::write(&target, "bounded").unwrap();
        symlink(&target, &link).unwrap();

        assert!(read_bounded_utf8_regular_file(&link, 16).is_err());
        fs::remove_dir_all(root).unwrap();
    }
}
