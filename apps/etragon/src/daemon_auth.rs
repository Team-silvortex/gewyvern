use super::*;

pub(super) fn daemon_request_is_authorized(
    remote_ip: IpAddr,
    request_text: &str,
    access_policy: &DaemonAccessPolicy,
) -> bool {
    if daemon_client_is_loopback(remote_ip) {
        return true;
    }
    let Some(expected_token) = access_policy.admin_token.as_deref() else {
        return false;
    };
    request_header_value(request_text, ETRAGON_ADMIN_TOKEN_HEADER)
        .map(|value| token_equals(value, expected_token))
        .unwrap_or(false)
}

fn token_equals(supplied: &str, expected: &str) -> bool {
    let supplied = supplied.as_bytes();
    let expected = expected.as_bytes();
    let max_len = supplied.len().max(expected.len());
    let mut diff = supplied.len() ^ expected.len();
    for index in 0..max_len {
        let left = supplied.get(index).copied().unwrap_or(0);
        let right = expected.get(index).copied().unwrap_or(0);
        diff |= (left ^ right) as usize;
    }
    diff == 0
}

fn request_header_value<'a>(request_text: &'a str, header_name: &str) -> Option<&'a str> {
    let mut matched = None;
    for line in request_text.lines().skip(1) {
        let Some((name, value)) = line.split_once(':') else {
            continue;
        };
        if name.trim().eq_ignore_ascii_case(header_name) {
            if matched.is_some() {
                return None;
            }
            matched = Some(value.trim());
        }
    }
    matched
}
