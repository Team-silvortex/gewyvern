//! Packet-local RTSP/1.0 candidates. Never search message bodies or join packets.
use super::RtspResponseKind;
use crate::ledger::PacketMetaFact;

pub(crate) const MAX_HEADER_BYTES: u16 = 512;

pub(super) fn matches(packet: &PacketMetaFact, kind: RtspResponseKind) -> bool {
    let mut prefix = [0u8; MAX_HEADER_BYTES as usize];
    for offset in 0..MAX_HEADER_BYTES {
        let Some(byte) = packet.payload_bytes.get(&offset) else {
            return false;
        };
        let end = usize::from(offset) + 1;
        prefix[end - 1] = *byte;
        if prefix[..end].ends_with(b"\r\n\r\n") {
            return matches_header(&prefix[..end], kind);
        }
    }
    false
}

fn matches_header(bytes: &[u8], kind: RtspResponseKind) -> bool {
    let Ok(text) = std::str::from_utf8(bytes) else {
        return false;
    };
    if bytes.len() > usize::from(MAX_HEADER_BYTES) || !text.ends_with("\r\n\r\n") {
        return false;
    }
    let mut lines = text[..text.len() - 4].split("\r\n");
    let Some(status) = lines.next() else {
        return false;
    };
    if !status.starts_with("RTSP/1.0 200 ") || !status.bytes().all(|b| (32..=126).contains(&b)) {
        return false;
    }
    let mut headers: [Option<(&str, &str)>; 32] = [None; 32];
    for (index, line) in lines.enumerate() {
        if index >= headers.len() {
            return false;
        }
        let Some((name, value)) = line.split_once(':') else {
            return false;
        };
        if name.is_empty()
            || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
            || !value.bytes().all(|b| b == b'\t' || (32..=126).contains(&b))
            || headers[..index]
                .iter()
                .flatten()
                .any(|(old, _)| name.eq_ignore_ascii_case(old))
        {
            return false;
        }
        headers[index] = Some((name, value.trim_matches([' ', '\t'])));
    }
    let get = |name: &str| {
        headers
            .iter()
            .flatten()
            .find_map(|(key, value)| key.eq_ignore_ascii_case(name).then_some(*value))
    };
    let decimal = |v: &str| {
        !v.is_empty() && v.bytes().all(|b| b.is_ascii_digit()) && v.parse::<u32>().is_ok()
    };
    if !get("CSeq").is_some_and(decimal) {
        return false;
    }
    if get("Content-Length").is_some_and(|v| !decimal(v)) {
        return false;
    }
    let session = get("Session").is_some_and(|v| {
        let id = v.split(';').next().unwrap_or("");
        !id.is_empty()
            && id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'_' | b'.' | b'+'))
    });
    match kind {
        RtspResponseKind::Options => get("Public").is_some_and(|v| {
            v.split(',').all(|method| {
                !method.trim().is_empty()
                    && method
                        .trim()
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b == b'_')
            }) && v.split(',').any(|method| {
                matches!(
                    method.trim(),
                    "OPTIONS"
                        | "DESCRIBE"
                        | "SETUP"
                        | "PLAY"
                        | "PAUSE"
                        | "TEARDOWN"
                        | "ANNOUNCE"
                        | "RECORD"
                        | "GET_PARAMETER"
                        | "SET_PARAMETER"
                )
            })
        }),
        RtspResponseKind::Describe => {
            get("Content-Type").is_some_and(|v| {
                v.split(';')
                    .next()
                    .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/sdp"))
            }) && get("Content-Length")
                .and_then(|v| v.parse::<u32>().ok())
                .is_some_and(|n| n > 0)
        }
        RtspResponseKind::Setup => {
            session
                && get("Transport").is_some_and(|v| {
                    v.split(';').next().is_some_and(|profile| {
                        matches!(profile.trim(), "RTP/AVP" | "RTP/AVP/TCP" | "RTP/AVP/UDP")
                    })
                })
        }
        RtspResponseKind::Play => {
            session && get("RTP-Info").is_some_and(|v| v.starts_with("url=") && v.len() > 4)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn headers_are_order_independent_and_names_case_insensitive() {
        assert!(matches_header(
            b"RTSP/1.0 200 OK\r\nServer: test\r\npUbLiC: OPTIONS, DESCRIBE\r\ncSeQ: 12345\r\n\r\n",
            RtspResponseKind::Options
        ));
        assert!(matches_header(b"RTSP/1.0 200 Fine\r\nContent-Length: 30\r\nCSeq: 0\r\nContent-Type: APPLICATION/SDP; charset=utf-8\r\n\r\n",RtspResponseKind::Describe));
        assert!(matches_header(b"RTSP/1.0 200 OK\r\nTransport: RTP/AVP/TCP;unicast\r\nSession: aB-1;timeout=60\r\nCSeq: 9\r\n\r\n",RtspResponseKind::Setup));
        assert!(matches_header(b"RTSP/1.0 200 OK\r\nCSeq: 10\r\nSession: aB-1\r\nRTP-Info: url=rtsp://test/track\r\n\r\n",RtspResponseKind::Play));
    }
    #[test]
    fn malformed_ambiguous_and_body_only_headers_are_not_candidates() {
        for bytes in [
            b"RTSP/1.0 404 Not Found\r\nCSeq: 1\r\nPublic: OPTIONS\r\n\r\n".as_slice(),
            b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nCSEQ: 2\r\nPublic: OPTIONS\r\n\r\n",
            b"RTSP/1.0 200 OK\r\nCSeq: +1\r\nPublic: OPTIONS\r\n\r\n",
            b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nPublic: XOPTIONS\r\n\r\n",
            b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nPublic: OPTIONS\r\n",
        ] {
            assert!(
                !matches_header(bytes, RtspResponseKind::Options),
                "{bytes:?}"
            );
        }
        assert!(!matches_header(
            b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nSession: x\r\nRange: npt=0-\r\n\r\n",
            RtspResponseKind::Play
        ));
        assert!(!matches_header(
            b"RTSP/1.0 200 OK\r\nCSeq: 1\r\nSession: x\r\n\r\n",
            RtspResponseKind::Setup
        ));
    }
}
