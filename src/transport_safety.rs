use std::io;
use std::net::{TcpStream, ToSocketAddrs};
use std::time::{Duration, Instant};

pub const MAX_RESOLVED_ADDRESSES: usize = 8;

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
    let deadline = Instant::now().checked_add(timeout).ok_or_else(|| {
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
            Ok(stream) => return Ok(stream),
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
