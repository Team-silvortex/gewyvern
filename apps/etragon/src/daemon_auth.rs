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
        .map(|value| value == expected_token)
        .unwrap_or(false)
}

fn request_header_value<'a>(request_text: &'a str, header_name: &str) -> Option<&'a str> {
    request_text.lines().skip(1).find_map(|line| {
        let (name, value) = line.split_once(':')?;
        if name.trim().eq_ignore_ascii_case(header_name) {
            Some(value.trim())
        } else {
            None
        }
    })
}
