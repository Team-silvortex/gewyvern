use super::super::ShelfMatch;
pub(crate) fn dns_shelf(entry: &str) -> Option<ShelfMatch> {
    const UDP: &[&str] = &["udp"];
    const TCP: &[&str] = &["tcp"];
    if UDP.contains(&entry) {
        Some((
            "udp",
            "UDP Lookup",
            "docs/book/reference-dns-udp-surface.md",
            UDP,
        ))
    } else if TCP.contains(&entry) {
        Some((
            "tcp",
            "TCP Query",
            "docs/book/reference-dns-tcp-surface.md",
            TCP,
        ))
    } else {
        None
    }
}

pub(crate) fn https_shelf(entry: &str) -> Option<ShelfMatch> {
    const CONNECT: &[&str] = &["connect"];
    if CONNECT.contains(&entry) {
        Some((
            "connect",
            "Connect",
            "docs/book/reference-https-surface.md",
            CONNECT,
        ))
    } else {
        None
    }
}

pub(crate) fn http_shelf(entry: &str) -> Option<ShelfMatch> {
    const MESSAGE: &[&str] = &["request", "response"];
    const CONNECT: &[&str] = &["connect", "denied"];
    const CONNECT_AUTH: &[&str] = &["auth-required", "auth-tunnel"];
    if MESSAGE.contains(&entry) {
        Some((
            "message",
            "Message",
            "docs/book/reference-http-message-surface.md",
            MESSAGE,
        ))
    } else if CONNECT.contains(&entry) {
        Some((
            "connect",
            "Connect Tunnel",
            "docs/book/reference-http-connect-surface.md",
            CONNECT,
        ))
    } else if CONNECT_AUTH.contains(&entry) {
        Some((
            "connect-auth",
            "Connect Auth",
            "docs/book/reference-http-connect-auth-surface.md",
            CONNECT_AUTH,
        ))
    } else {
        None
    }
}

pub(crate) fn hy2_shelf(entry: &str) -> Option<ShelfMatch> {
    const AUTH: &[&str] = &["auth"];
    const RELAY: &[&str] = &["udp", "tcp"];
    const CLOSE: &[&str] = &["close"];
    const TCP_CLOSE: &[&str] = &["tcp-close"];
    const UDP_CLOSE: &[&str] = &["udp-close"];
    if AUTH.contains(&entry) {
        Some((
            "auth",
            "Auth",
            "docs/book/reference-hy2-auth-surface.md",
            AUTH,
        ))
    } else if RELAY.contains(&entry) {
        Some((
            "relay",
            "Relay",
            "docs/book/reference-hy2-relay-surface.md",
            RELAY,
        ))
    } else if CLOSE.contains(&entry) {
        Some((
            "close",
            "Session Close",
            "docs/book/reference-hy2-close-surface.md",
            CLOSE,
        ))
    } else if TCP_CLOSE.contains(&entry) {
        Some((
            "tcp-close",
            "TCP Relay Close",
            "docs/book/reference-hy2-tcp-close-surface.md",
            TCP_CLOSE,
        ))
    } else if UDP_CLOSE.contains(&entry) {
        Some((
            "udp-close",
            "UDP Relay Close",
            "docs/book/reference-hy2-udp-close-surface.md",
            UDP_CLOSE,
        ))
    } else {
        None
    }
}

pub(crate) fn tls_shelf(entry: &str) -> Option<ShelfMatch> {
    const CLIENT: &[&str] = &["client"];
    const SERVER: &[&str] = &["server"];
    if CLIENT.contains(&entry) {
        Some((
            "client",
            "Client",
            "docs/book/reference-tls-client-surface.md",
            CLIENT,
        ))
    } else if SERVER.contains(&entry) {
        Some((
            "server",
            "Server",
            "docs/book/reference-tls-server-surface.md",
            SERVER,
        ))
    } else {
        None
    }
}

pub(crate) fn stun_shelf(entry: &str) -> Option<ShelfMatch> {
    const BINDING: &[&str] = &["binding", "binding-error"];
    const RELAY: &[&str] = &["allocate", "refresh"];
    if BINDING.contains(&entry) {
        Some((
            "binding",
            "Binding",
            "docs/book/reference-stun-binding-surface.md",
            BINDING,
        ))
    } else if RELAY.contains(&entry) {
        Some((
            "relay",
            "Relay Control",
            "docs/book/reference-stun-relay-surface.md",
            RELAY,
        ))
    } else {
        None
    }
}

pub(crate) fn coap_shelf(entry: &str) -> Option<ShelfMatch> {
    const GET: &[&str] = &["get"];
    const WRITE: &[&str] = &["post", "put", "delete"];
    if GET.contains(&entry) {
        Some(("get", "Get", "docs/book/reference-coap-get-surface.md", GET))
    } else if WRITE.contains(&entry) {
        Some((
            "write",
            "Write Methods",
            "docs/book/reference-coap-write-surface.md",
            WRITE,
        ))
    } else {
        None
    }
}

pub(crate) fn ntp_shelf(entry: &str) -> Option<ShelfMatch> {
    const CLIENT: &[&str] = &["client"];
    const QUERY: &[&str] = &["query"];
    const SYNC: &[&str] = &["sync"];
    if CLIENT.contains(&entry) {
        Some((
            "client",
            "Client",
            "docs/book/reference-ntp-client-surface.md",
            CLIENT,
        ))
    } else if QUERY.contains(&entry) {
        Some((
            "query",
            "Query",
            "docs/book/reference-ntp-time-surface.md",
            QUERY,
        ))
    } else if SYNC.contains(&entry) {
        Some((
            "sync",
            "Sync",
            "docs/book/reference-ntp-time-surface.md",
            SYNC,
        ))
    } else {
        None
    }
}

pub(crate) fn dhcp_shelf(entry: &str) -> Option<ShelfMatch> {
    const CLIENT: &[&str] = &["client"];
    const LEASE: &[&str] = &["discover", "nak", "request"];
    if CLIENT.contains(&entry) {
        Some((
            "client",
            "Client",
            "docs/book/reference-dhcp-client-surface.md",
            CLIENT,
        ))
    } else if LEASE.contains(&entry) {
        Some((
            "lease",
            "Lease Negotiation",
            "docs/book/reference-dhcp-lease-surface.md",
            LEASE,
        ))
    } else {
        None
    }
}

pub(crate) fn arp_shelf(entry: &str) -> Option<ShelfMatch> {
    const REQUEST: &[&str] = &["request"];
    const REPLY: &[&str] = &["reply"];
    if REQUEST.contains(&entry) {
        Some((
            "request",
            "Who-Has Request",
            "docs/book/reference-arp-request-surface.md",
            REQUEST,
        ))
    } else if REPLY.contains(&entry) {
        Some((
            "reply",
            "Is-At Reply",
            "docs/book/reference-arp-reply-surface.md",
            REPLY,
        ))
    } else {
        None
    }
}

pub(crate) fn icmp_shelf(entry: &str) -> Option<ShelfMatch> {
    const ECHO: &[&str] = &["echo"];
    const FAILURE: &[&str] = &["unreachable"];
    if ECHO.contains(&entry) {
        Some((
            "echo",
            "Echo Reachability",
            "docs/book/reference-icmp-echo-surface.md",
            ECHO,
        ))
    } else if FAILURE.contains(&entry) {
        Some((
            "failure",
            "Reachability Failure",
            "docs/book/reference-icmp-failure-surface.md",
            FAILURE,
        ))
    } else {
        None
    }
}

pub(crate) fn icmpv6_shelf(entry: &str) -> Option<ShelfMatch> {
    const ECHO: &[&str] = &["echo"];
    const FAILURE: &[&str] = &["unreachable"];
    if ECHO.contains(&entry) {
        Some((
            "echo",
            "Echo Reachability",
            "docs/book/reference-icmpv6-echo-surface.md",
            ECHO,
        ))
    } else if FAILURE.contains(&entry) {
        Some((
            "failure",
            "Reachability Failure",
            "docs/book/reference-icmpv6-failure-surface.md",
            FAILURE,
        ))
    } else {
        None
    }
}

pub(crate) fn ndp_shelf(entry: &str) -> Option<ShelfMatch> {
    const SOLICIT: &[&str] = &["solicit"];
    const ADVERTISE: &[&str] = &["advertise"];
    if SOLICIT.contains(&entry) {
        Some((
            "solicit",
            "Neighbor Solicitation",
            "docs/book/reference-ndp-solicit-surface.md",
            SOLICIT,
        ))
    } else if ADVERTISE.contains(&entry) {
        Some((
            "advertise",
            "Neighbor Advertisement",
            "docs/book/reference-ndp-advertise-surface.md",
            ADVERTISE,
        ))
    } else {
        None
    }
}

pub(crate) fn bgp_shelf(entry: &str) -> Option<ShelfMatch> {
    const SESSION: &[&str] = &["open", "keepalive"];
    if SESSION.contains(&entry) {
        Some((
            "session",
            "Session",
            "docs/book/reference-bgp-session-surface.md",
            SESSION,
        ))
    } else {
        None
    }
}

pub(crate) fn ospf_shelf(entry: &str) -> Option<ShelfMatch> {
    const NEIGHBOR: &[&str] = &["hello"];
    const DATABASE: &[&str] = &["dbdesc"];
    if NEIGHBOR.contains(&entry) {
        Some((
            "neighbor",
            "Neighbor",
            "docs/book/reference-ospf-neighbor-surface.md",
            NEIGHBOR,
        ))
    } else if DATABASE.contains(&entry) {
        Some((
            "database",
            "Database Description",
            "docs/book/reference-ospf-database-surface.md",
            DATABASE,
        ))
    } else {
        None
    }
}

pub(crate) fn gre_shelf(entry: &str) -> Option<ShelfMatch> {
    const TUNNEL: &[&str] = &["encap", "keepalive"];
    if TUNNEL.contains(&entry) {
        Some((
            "tunnel",
            "Tunnel",
            "docs/book/reference-gre-tunnel-surface.md",
            TUNNEL,
        ))
    } else {
        None
    }
}

pub(crate) fn vxlan_shelf(entry: &str) -> Option<ShelfMatch> {
    const OVERLAY: &[&str] = &["encap", "vni"];
    if OVERLAY.contains(&entry) {
        Some((
            "overlay",
            "Overlay",
            "docs/book/reference-vxlan-overlay-surface.md",
            OVERLAY,
        ))
    } else {
        None
    }
}

pub(crate) fn geneve_shelf(entry: &str) -> Option<ShelfMatch> {
    const OVERLAY: &[&str] = &["encap", "options"];
    if OVERLAY.contains(&entry) {
        Some((
            "overlay",
            "Overlay",
            "docs/book/reference-geneve-overlay-surface.md",
            OVERLAY,
        ))
    } else {
        None
    }
}

pub(crate) fn l2tp_shelf(entry: &str) -> Option<ShelfMatch> {
    const TUNNEL: &[&str] = &["control", "session"];
    if TUNNEL.contains(&entry) {
        Some((
            "tunnel",
            "Tunnel",
            "docs/book/reference-l2tp-tunnel-surface.md",
            TUNNEL,
        ))
    } else {
        None
    }
}

pub(crate) fn pptp_shelf(entry: &str) -> Option<ShelfMatch> {
    const TUNNEL: &[&str] = &["control", "data"];
    if TUNNEL.contains(&entry) {
        Some((
            "tunnel",
            "Tunnel",
            "docs/book/reference-pptp-tunnel-surface.md",
            TUNNEL,
        ))
    } else {
        None
    }
}

pub(crate) fn ipsec_shelf(entry: &str) -> Option<ShelfMatch> {
    const SECURITY: &[&str] = &["esp", "ah"];
    if SECURITY.contains(&entry) {
        Some((
            "security",
            "Security",
            "docs/book/reference-ipsec-security-surface.md",
            SECURITY,
        ))
    } else {
        None
    }
}

pub(crate) fn wireguard_shelf(entry: &str) -> Option<ShelfMatch> {
    const HANDSHAKE: &[&str] = &["handshake"];
    const COOKIE: &[&str] = &["cookie"];
    const TRANSPORT: &[&str] = &["transport"];
    if HANDSHAKE.contains(&entry) {
        Some((
            "handshake",
            "Handshake",
            "docs/book/reference-wireguard-handshake-surface.md",
            HANDSHAKE,
        ))
    } else if COOKIE.contains(&entry) {
        Some((
            "cookie",
            "Cookie Reply",
            "docs/book/reference-wireguard-cookie-surface.md",
            COOKIE,
        ))
    } else if TRANSPORT.contains(&entry) {
        Some((
            "transport",
            "Transport",
            "docs/book/reference-wireguard-transport-surface.md",
            TRANSPORT,
        ))
    } else {
        None
    }
}

pub(crate) fn snmp_shelf(entry: &str) -> Option<ShelfMatch> {
    const READ: &[&str] = &["bulk", "get", "get-next"];
    const SET: &[&str] = &["set"];
    const NOTIFY: &[&str] = &["trap", "inform"];
    const SECURITY: &[&str] = &["v3-auth", "v3-priv"];
    const MANAGE: &[&str] = &["engine-sync", "trap-recv"];
    const RESULT: &[&str] = &["report", "unauthorized"];
    if READ.contains(&entry) {
        Some((
            "read",
            "Read",
            "docs/book/reference-snmp-read-surface.md",
            READ,
        ))
    } else if SET.contains(&entry) {
        Some(("set", "Set", "docs/book/reference-snmp-set-surface.md", SET))
    } else if NOTIFY.contains(&entry) {
        Some((
            "notify",
            "Notify",
            "docs/book/reference-snmp-notify-surface.md",
            NOTIFY,
        ))
    } else if SECURITY.contains(&entry) {
        Some((
            "security",
            "Security",
            "docs/book/reference-snmp-security-surface.md",
            SECURITY,
        ))
    } else if MANAGE.contains(&entry) {
        Some((
            "manage",
            "Manage",
            "docs/book/reference-snmp-manage-surface.md",
            MANAGE,
        ))
    } else if RESULT.contains(&entry) {
        Some((
            "result",
            "Result",
            "docs/book/reference-snmp-result-surface.md",
            RESULT,
        ))
    } else {
        None
    }
}
