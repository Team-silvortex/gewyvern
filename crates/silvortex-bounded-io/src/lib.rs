//! Product-neutral bounded I/O primitives for native service boundaries.

use std::fs::{File, Metadata, OpenOptions};
use std::io::{self, Read, Write};
use std::net::{TcpStream, ToSocketAddrs};
#[cfg(unix)]
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;
use std::time::{Duration, Instant};

pub const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
pub const MAX_RESOLVED_ADDRESSES: usize = 8;

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

        let allowed = usize::try_from(self.remaining.min(buffer.len() as u64))
            .expect("bounded read length fits usize");
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

pub fn open_bounded_regular_file(path: &Path, max_bytes: u64) -> io::Result<BoundedFile> {
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
    options.custom_flags(libc::O_NOFOLLOW);
    let file = options.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.file_type().is_file() || metadata.len() == 0 || metadata.len() > max_bytes {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "file must be regular, non-empty, and within the size limit",
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
    if timeout.is_zero() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "transport I/O timeout must be non-zero",
        ));
    }
    let deadline = Instant::now().checked_add(timeout).ok_or_else(|| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            "transport I/O timeout exceeds the supported duration",
        )
    })?;
    let stream = connect_with_deadline(address, timeout)?;
    DeadlineTcpStream::new(stream, deadline)
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
    fn connector_applies_a_nonzero_deadline_to_loopback() {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        assert!(connect_with_deadline(listener.local_addr().unwrap(), Duration::ZERO).is_err());
        assert!(
            connect_with_deadline(listener.local_addr().unwrap(), Duration::from_secs(1)).is_ok()
        );
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
        let link = root.join("link.pem");
        fs::write(&target, b"bounded").unwrap();
        symlink(&target, &link).unwrap();

        assert!(open_bounded_regular_file(&target, 7).is_ok());
        assert!(open_bounded_regular_file(&target, 6).is_err());
        assert!(open_bounded_regular_file(&target, 0).is_err());
        assert!(open_bounded_regular_file(&link, 7).is_err());
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
}
