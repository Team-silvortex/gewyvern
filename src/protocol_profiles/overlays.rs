use super::ProtocolOverlaySummary;

pub(super) fn overlays_for_surface(protocol: &str, entry: &str) -> Vec<ProtocolOverlaySummary> {
    match (protocol, entry) {
        ("dns", "tcp") => vec![ProtocolOverlaySummary {
            key: "dot".into(),
            label: "DNS-Over-TLS".into(),
            kind: "encrypted_resolver_overlay".into(),
            operator_hint: "Treat this as the DNS TCP lookup shelf plus TLS client setup posture before trusting resolver reachability.".into(),
            aliases: vec!["dot".into(), "dns-over-tls".into(), "dns_over_tls".into()],
            companion_protocol: Some("tls".into()),
            companion_entry: Some("client".into()),
        }],
        ("http", "request") => vec![ProtocolOverlaySummary {
            key: "doh".into(),
            label: "DNS-Over-HTTPS".into(),
            kind: "resolver_payload_overlay".into(),
            operator_hint: "Treat this as the HTTP request shelf carrying DNS resolver intent inside the request and response payload path.".into(),
            aliases: vec!["doh".into(), "dns-over-https".into(), "dns_over_https".into()],
            companion_protocol: Some("dns".into()),
            companion_entry: Some("tcp".into()),
        }],
        ("http", "connect" | "auth-required" | "auth-tunnel" | "denied") => {
            vec![ProtocolOverlaySummary {
                key: "http-connect".into(),
                label: "HTTP CONNECT Tunnel".into(),
                kind: "proxy_tunnel_overlay".into(),
                operator_hint: "Treat this as an HTTP proxy tunnel surface where authority selection, proxy policy, and tunnel establishment matter more than plain request semantics.".into(),
                aliases: vec![
                    "http-connect".into(),
                    "http_connect".into(),
                    "http-connect-auth-required".into(),
                    "http_connect_auth_required".into(),
                    "http-connect-auth-tunnel".into(),
                    "http_connect_auth_tunnel".into(),
                    "http-connect-denied".into(),
                    "http_connect_denied".into(),
                ],
                companion_protocol: Some("tls".into()),
                companion_entry: Some("client".into()),
            }]
        }
        ("smtp" | "imap" | "pop3", "auth" | "auth-denied") => vec![ProtocolOverlaySummary {
            key: "starttls".into(),
            label: "STARTTLS Upgrade".into(),
            kind: "tls_upgrade_overlay".into(),
            operator_hint: "Treat this as a cleartext-to-TLS upgrade point where capability advertisement, policy requirements, and upgrade timing can block later authentication.".into(),
            aliases: Vec::new(),
            companion_protocol: Some("tls".into()),
            companion_entry: Some("client".into()),
        }],
        ("https", "connect") => vec![ProtocolOverlaySummary {
            key: "https".into(),
            label: "HTTPS Over TLS".into(),
            kind: "tls_application_overlay".into(),
            operator_hint: "Treat this as an HTTP request surface that depends on a healthy TLS client handshake before request semantics are trustworthy.".into(),
            aliases: Vec::new(),
            companion_protocol: Some("tls".into()),
            companion_entry: Some("client".into()),
        }],
        ("tls", "client") => vec![
            ProtocolOverlaySummary {
                key: "https".into(),
                label: "HTTPS Over TLS".into(),
                kind: "tls_application_overlay".into(),
                operator_hint: "This TLS client posture commonly fronts HTTPS; after handshake success, continue into the HTTPS connect shelf if request semantics matter.".into(),
                aliases: Vec::new(),
                companion_protocol: Some("https".into()),
                companion_entry: Some("connect".into()),
            },
            ProtocolOverlaySummary {
                key: "dot".into(),
                label: "DNS-Over-TLS".into(),
                kind: "encrypted_resolver_overlay".into(),
                operator_hint: "This TLS client posture can also carry DNS-over-TLS when the next layer is resolver traffic rather than web request intent.".into(),
                aliases: vec!["dot".into(), "dns-over-tls".into(), "dns_over_tls".into()],
                companion_protocol: Some("dns".into()),
                companion_entry: Some("tcp".into()),
            },
        ],
        ("http3", "request" | "server") => vec![ProtocolOverlaySummary {
            key: "http3".into(),
            label: "HTTP/3 Over QUIC".into(),
            kind: "quic_application_overlay".into(),
            operator_hint: "Treat this as HTTP semantics layered on top of QUIC setup; when request meaning is missing, inspect QUIC initial and crypto progress first.".into(),
            aliases: Vec::new(),
            companion_protocol: Some("quic".into()),
            companion_entry: Some("initial".into()),
        }],
        ("quic", "initial" | "crypto" | "stream" | "bidi") => vec![ProtocolOverlaySummary {
            key: "http3".into(),
            label: "HTTP/3 Over QUIC".into(),
            kind: "quic_application_overlay".into(),
            operator_hint: "This QUIC posture often carries HTTP/3; after transport health is established, continue into the HTTP/3 request or server shelf for payload semantics.".into(),
            aliases: Vec::new(),
            companion_protocol: Some("http3".into()),
            companion_entry: Some("request".into()),
        }],
        _ => Vec::new(),
    }
}

pub(super) fn selected_overlay_for_alias(protocol_alias: &str) -> Option<&'static str> {
    match protocol_alias {
        "dot" | "dns-over-tls" | "dns_over_tls" => Some("dot"),
        "doh" | "dns-over-https" | "dns_over_https" => Some("doh"),
        "http-connect"
        | "http_connect"
        | "http-connect-auth-required"
        | "http_connect_auth_required"
        | "http-connect-auth-tunnel"
        | "http_connect_auth_tunnel"
        | "http-connect-denied"
        | "http_connect_denied" => Some("http-connect"),
        _ => None,
    }
}
