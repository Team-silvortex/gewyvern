use std::net::{Ipv4Addr, Ipv6Addr};
use std::str::FromStr;

pub const SILVORTEX_ISSUER_KEY: &str = "LeserpentSilvortexIssuer";
pub const MAX_SILVORTEX_ISSUER_BYTES: usize = 2048;

pub fn is_canonical_https_origin(value: &str) -> bool {
    if value.len() > MAX_SILVORTEX_ISSUER_BYTES
        || !value.is_ascii()
        || value
            .bytes()
            .any(|byte| byte.is_ascii_control() || byte.is_ascii_whitespace())
    {
        return false;
    }
    let Ok(uri) = value.parse::<http::Uri>() else {
        return false;
    };
    uri.scheme_str() == Some("https")
        && uri.authority().is_some_and(|authority| {
            is_canonical_authority(authority)
                && value
                    .strip_prefix("https://")
                    .and_then(|remainder| remainder.strip_suffix('/'))
                    == Some(authority.as_str())
        })
        && uri.path() == "/"
        && uri.query().is_none()
}

fn is_canonical_authority(authority: &http::uri::Authority) -> bool {
    if authority.as_str().contains('@') || !is_canonical_host(authority.host()) {
        return false;
    }
    match authority.port() {
        Some(port) => port
            .as_str()
            .parse::<u16>()
            .is_ok_and(|value| value != 0 && value != 443 && value.to_string() == port.as_str()),
        None => true,
    }
}

fn is_canonical_host(host: &str) -> bool {
    if let Some(address) = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
    {
        return Ipv6Addr::from_str(address).is_ok_and(|parsed| parsed.to_string() == address);
    }
    if host.split('.').all(is_legacy_numeric_label) {
        return !host.contains("0x")
            && Ipv4Addr::from_str(host).is_ok_and(|parsed| parsed.to_string() == host);
    }
    host.len() <= 253
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
                && !label.starts_with('-')
                && !label.ends_with('-')
        })
}

fn is_legacy_numeric_label(label: &str) -> bool {
    !label.is_empty()
        && (label.bytes().all(|byte| byte.is_ascii_digit())
            || label.strip_prefix("0x").is_some_and(|hex| {
                !hex.is_empty() && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
            }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_canonical_public_https_origins() {
        assert!(is_canonical_https_origin("https://id.example.invalid/"));
        assert!(is_canonical_https_origin(
            "https://id.example.invalid:8443/"
        ));
        assert!(is_canonical_https_origin("https://127.0.0.1:8443/"));
        assert!(is_canonical_https_origin("https://[2001:db8::1]:8443/"));
        for invalid in [
            "http://id.example.invalid/",
            "https://id.example.invalid/path",
            "https://user@id.example.invalid/",
            "https://id.example.invalid/?query=1",
            "https://id.example.invalid/#fragment",
            "https://id.example.invalid/<key>",
            "https://foo&bar/",
            "https://foo=bar/",
            "https://foo;bar/",
            "https://UPPER.example.invalid/",
            "https://under_score.example.invalid/",
            "https://-prefix.example.invalid/",
            "https://suffix-.example.invalid/",
            "https://999.0.0.1/",
            "https://123/",
            "https://2130706433/",
            "https://0x7f000001/",
            "https://0x7f.0.0.1/",
            "https://0177.0.0.1/",
            "https://id.example.invalid:0/",
            "https://id.example.invalid:0443/",
            "https://id.example.invalid:443/",
            "https://[2001:0db8::1]/",
        ] {
            assert!(!is_canonical_https_origin(invalid), "{invalid}");
        }
    }
}
