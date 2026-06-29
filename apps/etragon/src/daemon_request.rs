use super::*;

pub(super) const DAEMON_REQUEST_LIMIT_BYTES: usize = 64 * 1024;

pub(super) enum DaemonRequestRead {
    Complete(String),
    TooLarge,
}

pub(super) fn read_daemon_request(stream: &mut TcpStream) -> Result<DaemonRequestRead, String> {
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .map_err(|err| format!("failed to configure daemon request timeout: {err}"))?;

    let mut request = Vec::new();
    let mut chunk = [0u8; 2048];
    loop {
        match stream.read(&mut chunk) {
            Ok(0) => break,
            Ok(size) => {
                request.extend_from_slice(&chunk[..size]);
                if request.len() > DAEMON_REQUEST_LIMIT_BYTES {
                    return Ok(DaemonRequestRead::TooLarge);
                }
                if daemon_request_is_complete(&request) {
                    break;
                }
            }
            Err(err)
                if matches!(
                    err.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut
                ) && !request.is_empty() =>
            {
                break;
            }
            Err(err) => return Err(format!("failed to read daemon request: {err}")),
        }
    }

    Ok(DaemonRequestRead::Complete(
        String::from_utf8_lossy(&request).to_string(),
    ))
}

fn daemon_request_is_complete(request: &[u8]) -> bool {
    let Some(header_end) = header_end_index(request) else {
        return false;
    };
    let header_text = String::from_utf8_lossy(&request[..header_end]);
    let content_length = header_text.lines().find_map(|line| {
        let (name, value) = line.split_once(':')?;
        name.trim()
            .eq_ignore_ascii_case("content-length")
            .then(|| value.trim().parse::<usize>().ok())
            .flatten()
    });
    match content_length {
        Some(expected) => request.len().saturating_sub(header_end) >= expected,
        None => true,
    }
}

fn header_end_index(request: &[u8]) -> Option<usize> {
    request
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .map(|index| index + 4)
        .or_else(|| {
            request
                .windows(2)
                .position(|window| window == b"\n\n")
                .map(|index| index + 2)
        })
}
