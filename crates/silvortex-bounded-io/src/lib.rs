//! Product-neutral bounded I/O primitives for native service boundaries.

use std::fs::{self, File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, TcpStream, ToSocketAddrs};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

pub const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
pub const MAX_RESOLVED_ADDRESSES: usize = 8;
pub const MAX_HTTPS_AUTHORITY_BYTES: usize = 320;
const ATOMIC_WRITE_CREATE_ATTEMPTS: usize = 16;
static ATOMIC_WRITE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

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

/// Opens and exclusively locks a private regular file, waiting at most `timeout`.
///
/// The operating system releases the lock when the returned file is dropped or the process exits.
pub fn open_private_lock_file(path: &Path, timeout: Duration) -> io::Result<File> {
    let deadline = lock_deadline(timeout)?;

    #[cfg(not(unix))]
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "lock path must not be a symlink",
        ));
    }

    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true);
    configure_private_file_options(&mut options);
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "lock path must be a regular file",
        ));
    }
    validate_private_file_metadata(&metadata)?;
    lock_file_until(&file, deadline)?;
    Ok(file)
}

/// Appends bytes to a private regular file while holding a bounded exclusive lock.
pub fn append_bounded_private_file(
    path: &Path,
    contents: &[u8],
    max_bytes: u64,
    lock_timeout: Duration,
) -> io::Result<()> {
    validate_write_size(contents, max_bytes)?;
    let deadline = lock_deadline(lock_timeout)?;
    validate_real_parent(path)?;

    #[cfg(not(unix))]
    if fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "append path must not be a symlink",
        ));
    }

    let mut options = OpenOptions::new();
    options.write(true).append(true).create(true);
    configure_private_file_options(&mut options);
    let mut file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "append path must be a regular file",
        ));
    }
    validate_private_file_metadata(&metadata)?;
    lock_file_until(&file, deadline)?;

    let current_bytes = file.metadata()?.len();
    let appended_bytes = u64::try_from(contents.len()).unwrap_or(u64::MAX);
    if current_bytes
        .checked_add(appended_bytes)
        .is_none_or(|total| total > max_bytes)
    {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "appended file would exceed the size limit",
        ));
    }
    file.write_all(contents)?;
    file.sync_all()
}

fn lock_deadline(timeout: Duration) -> io::Result<Instant> {
    if timeout.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "file lock timeout must be non-zero",
        ));
    }
    Instant::now().checked_add(timeout).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "file lock timeout exceeds the supported duration",
        )
    })
}

fn lock_file_until(file: &File, deadline: Instant) -> io::Result<()> {
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(()),
            Err(fs::TryLockError::WouldBlock) => {
                let remaining = deadline.saturating_duration_since(Instant::now());
                if remaining.is_zero() {
                    return Err(io::Error::new(
                        io::ErrorKind::TimedOut,
                        "timed out waiting for the file lock",
                    ));
                }
                std::thread::sleep(remaining.min(Duration::from_millis(10)));
            }
            Err(fs::TryLockError::Error(error)) => return Err(error),
        }
    }
}

/// Atomically replaces a bounded file with private permissions in its existing real directory.
pub fn atomic_write_bounded_private_file(
    path: &Path,
    contents: &[u8],
    max_bytes: u64,
) -> io::Result<()> {
    validate_write_size(contents, max_bytes)?;
    let parent = validate_real_parent(path)?;

    let (mut temporary, temporary_path) = create_private_temporary_file(path, parent)?;
    let result = (|| {
        temporary.write_all(contents)?;
        temporary.sync_all()?;
        drop(temporary);
        replace_file(&temporary_path, path)?;
        sync_parent(path)
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary_path);
    }
    result
}

fn validate_write_size(contents: &[u8], max_bytes: u64) -> io::Result<()> {
    if max_bytes == 0 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "file size limit must be non-zero",
        ));
    }
    if u64::try_from(contents.len()).unwrap_or(u64::MAX) > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "file contents exceed the size limit",
        ));
    }
    Ok(())
}

fn validate_real_parent(path: &Path) -> io::Result<&Path> {
    let parent = atomic_file_parent(path)?;
    let metadata = fs::symlink_metadata(parent)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "file parent must be a real directory",
        ));
    }
    Ok(parent)
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

fn create_private_temporary_file(path: &Path, parent: &Path) -> io::Result<(File, PathBuf)> {
    path.file_name().ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "atomic file path must have a file name",
        )
    })?;
    let process_id = std::process::id();
    for _ in 0..ATOMIC_WRITE_CREATE_ATTEMPTS {
        let sequence = ATOMIC_WRITE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let unix_nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_nanos())
            .unwrap_or(0);
        let temporary_path = parent.join(format!(
            ".atomic-write.{process_id}.{unix_nanos}.{sequence}.tmp"
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        configure_private_file_options(&mut options);
        match options.open(&temporary_path) {
            Ok(file) => return Ok((file, temporary_path)),
            Err(error) if error.kind() == io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error),
        }
    }
    Err(io::Error::new(
        io::ErrorKind::AlreadyExists,
        "failed to reserve a unique atomic file path",
    ))
}

fn atomic_file_parent(path: &Path) -> io::Result<&Path> {
    match path.parent() {
        Some(parent) if parent.as_os_str().is_empty() => Ok(Path::new(".")),
        Some(parent) => Ok(parent),
        None => Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "atomic file path must have a parent directory",
        )),
    }
}

#[cfg(unix)]
fn configure_private_file_options(options: &mut OpenOptions) {
    options
        .mode(0o600)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
}

#[cfg(not(unix))]
fn configure_private_file_options(_: &mut OpenOptions) {}

#[cfg(unix)]
fn validate_private_file_metadata(metadata: &Metadata) -> io::Result<()> {
    use std::os::unix::fs::MetadataExt;

    if metadata.uid() != unsafe { libc::geteuid() } {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private file must be owned by the current user",
        ));
    }
    if metadata.mode() & 0o077 != 0 {
        return Err(io::Error::new(
            io::ErrorKind::PermissionDenied,
            "private file must not grant group or other permissions",
        ));
    }
    if metadata.nlink() != 1 {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "private file must have exactly one hard link",
        ));
    }
    Ok(())
}

#[cfg(not(unix))]
fn validate_private_file_metadata(_: &Metadata) -> io::Result<()> {
    Ok(())
}

#[cfg(not(windows))]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    fs::rename(source, destination)
}

#[cfg(windows)]
fn replace_file(source: &Path, destination: &Path) -> io::Result<()> {
    use std::os::windows::ffi::OsStrExt;

    const MOVEFILE_REPLACE_EXISTING: u32 = 0x1;
    const MOVEFILE_WRITE_THROUGH: u32 = 0x8;
    #[link(name = "kernel32")]
    unsafe extern "system" {
        #[link_name = "MoveFileExW"]
        fn move_file_ex_w(existing: *const u16, replacement: *const u16, flags: u32) -> i32;
    }
    let source = source
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let destination = destination
        .as_os_str()
        .encode_wide()
        .chain(Some(0))
        .collect::<Vec<_>>();
    let result = unsafe {
        move_file_ex_w(
            source.as_ptr(),
            destination.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if result == 0 {
        Err(io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> io::Result<()> {
    File::open(atomic_file_parent(path)?)?.sync_all()
}

#[cfg(not(unix))]
fn sync_parent(_: &Path) -> io::Result<()> {
    Ok(())
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

    fn temp_root(label: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "silvortex-bounded-io-{label}-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("system clock must follow the epoch")
                .as_nanos()
        ))
    }

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

    #[test]
    fn atomic_private_write_replaces_complete_files_and_preserves_on_rejection() {
        let root = temp_root("atomic-write");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("state.tsv");
        fs::write(&path, b"old").unwrap();

        atomic_write_bounded_private_file(&path, b"complete", 8).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"complete");

        let error = atomic_write_bounded_private_file(&path, b"oversized", 8).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(fs::read(&path).unwrap(), b"complete");
        assert!(fs::read_dir(&root).unwrap().all(|entry| {
            !entry
                .unwrap()
                .file_name()
                .to_string_lossy()
                .ends_with(".tmp")
        }));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn atomic_private_write_accepts_a_bare_relative_path() {
        let unique = temp_root("bare-relative");
        let path = PathBuf::from(unique.file_name().unwrap());

        let result = atomic_write_bounded_private_file(&path, b"complete", 8);
        let contents = fs::read(&path);
        let _ = fs::remove_file(&path);

        result.unwrap();
        assert_eq!(contents.unwrap(), b"complete");
    }

    #[cfg(unix)]
    #[test]
    fn atomic_private_write_replaces_symlinks_without_touching_their_targets() {
        use std::os::unix::fs::{MetadataExt, symlink};

        let root = temp_root("atomic-symlink");
        fs::create_dir_all(&root).unwrap();
        let outside = root.join("outside.tsv");
        let path = root.join("state.tsv");
        fs::write(&outside, b"outside").unwrap();
        symlink(&outside, &path).unwrap();

        atomic_write_bounded_private_file(&path, b"replacement", 32).unwrap();

        assert_eq!(fs::read(&outside).unwrap(), b"outside");
        assert_eq!(fs::read(&path).unwrap(), b"replacement");
        let metadata = fs::symlink_metadata(&path).unwrap();
        assert!(metadata.is_file());
        assert!(!metadata.file_type().is_symlink());
        assert_eq!(metadata.mode() & 0o777, 0o600);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn atomic_private_write_rejects_a_symlinked_parent() {
        use std::os::unix::fs::symlink;

        let root = temp_root("atomic-parent-symlink");
        let real_parent = root.join("real");
        let linked_parent = root.join("linked");
        fs::create_dir_all(&real_parent).unwrap();
        symlink(&real_parent, &linked_parent).unwrap();

        let error =
            atomic_write_bounded_private_file(&linked_parent.join("state.tsv"), b"replacement", 32)
                .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert!(!real_parent.join("state.tsv").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn bounded_private_append_preserves_bytes_and_enforces_the_total_limit() {
        let root = temp_root("bounded-append");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("events.log");

        append_bounded_private_file(&path, &[0xff, b'\n'], 8, Duration::from_millis(100)).unwrap();
        append_bounded_private_file(&path, b"next\n", 8, Duration::from_millis(100)).unwrap();
        assert_eq!(
            fs::read(&path).unwrap(),
            [0xff, b'\n', b'n', b'e', b'x', b't', b'\n']
        );

        let error =
            append_bounded_private_file(&path, b"xx", 8, Duration::from_millis(100)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(
            fs::read(&path).unwrap(),
            [0xff, b'\n', b'n', b'e', b'x', b't', b'\n']
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt;

            assert_eq!(fs::metadata(&path).unwrap().mode() & 0o777, 0o600);
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn bounded_private_append_rejects_symlinks_without_touching_targets() {
        use std::os::unix::fs::symlink;

        let root = temp_root("bounded-append-symlink");
        fs::create_dir_all(&root).unwrap();
        let outside = root.join("outside.log");
        let path = root.join("events.log");
        fs::write(&outside, b"outside").unwrap();
        symlink(&outside, &path).unwrap();

        assert!(
            append_bounded_private_file(&path, b"next", 32, Duration::from_millis(100)).is_err()
        );
        assert_eq!(fs::read(&outside).unwrap(), b"outside");
        assert!(
            fs::symlink_metadata(&path)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn bounded_private_append_rejects_hard_links_without_touching_targets() {
        let root = temp_root("bounded-append-hard-link");
        fs::create_dir_all(&root).unwrap();
        let outside = root.join("outside.log");
        let path = root.join("events.log");
        atomic_write_bounded_private_file(&outside, b"outside", 32).unwrap();
        fs::hard_link(&outside, &path).unwrap();

        let error = append_bounded_private_file(&path, b"next", 32, Duration::from_millis(100))
            .unwrap_err();

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
        assert_eq!(fs::read(&outside).unwrap(), b"outside");
        assert_eq!(fs::read(&path).unwrap(), b"outside");
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn bounded_private_append_times_out_on_a_competing_writer() {
        let root = temp_root("bounded-append-lock");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("events.log");
        atomic_write_bounded_private_file(&path, b"first\n", 32).unwrap();
        let lock = open_private_lock_file(&path, Duration::from_millis(100)).unwrap();

        let error = append_bounded_private_file(&path, b"next\n", 32, Duration::from_millis(25))
            .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        assert_eq!(fs::read(&path).unwrap(), b"first\n");

        drop(lock);
        append_bounded_private_file(&path, b"next\n", 32, Duration::from_millis(100)).unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"first\nnext\n");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_lock_deadlines_fail_before_creating_files() {
        let root = temp_root("invalid-lock-deadline");
        fs::create_dir_all(&root).unwrap();
        let lock_path = root.join("state.lock");
        let append_path = root.join("events.log");

        assert_eq!(
            open_private_lock_file(&lock_path, Duration::ZERO)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert_eq!(
            append_bounded_private_file(&append_path, b"event\n", 32, Duration::ZERO)
                .unwrap_err()
                .kind(),
            io::ErrorKind::InvalidInput
        );
        assert!(!lock_path.exists());
        assert!(!append_path.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn private_file_lock_times_out_and_recovers_after_release() {
        let root = temp_root("file-lock");
        fs::create_dir_all(&root).unwrap();
        let path = root.join("state.lock");
        let first = open_private_lock_file(&path, Duration::from_millis(100)).unwrap();

        let error = open_private_lock_file(&path, Duration::from_millis(25)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);

        drop(first);
        let second = open_private_lock_file(&path, Duration::from_millis(100)).unwrap();
        drop(second);
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn private_file_lock_is_private_and_rejects_symlinks() {
        use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};

        let root = temp_root("file-lock-symlink");
        fs::create_dir_all(&root).unwrap();
        let lock = root.join("state.lock");
        let file = open_private_lock_file(&lock, Duration::from_millis(100)).unwrap();
        assert_eq!(file.metadata().unwrap().mode() & 0o777, 0o600);
        drop(file);

        let target = root.join("target.lock");
        let link = root.join("linked.lock");
        fs::write(&target, []).unwrap();
        symlink(&target, &link).unwrap();
        assert!(open_private_lock_file(&link, Duration::from_millis(100)).is_err());

        let exposed = root.join("exposed.lock");
        fs::write(&exposed, []).unwrap();
        fs::set_permissions(&exposed, fs::Permissions::from_mode(0o644)).unwrap();
        assert_eq!(
            open_private_lock_file(&exposed, Duration::from_millis(100))
                .unwrap_err()
                .kind(),
            io::ErrorKind::PermissionDenied
        );
        fs::remove_dir_all(root).unwrap();
    }
}
