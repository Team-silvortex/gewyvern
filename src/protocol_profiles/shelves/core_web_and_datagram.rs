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
    } else {
        None
    }
}

pub(crate) fn tls_shelf(entry: &str) -> Option<ShelfMatch> {
    const CLIENT: &[&str] = &["client"];
    if CLIENT.contains(&entry) {
        Some((
            "client",
            "Client",
            "docs/book/reference-tls-surface.md",
            CLIENT,
        ))
    } else {
        None
    }
}

pub(crate) fn stun_shelf(entry: &str) -> Option<ShelfMatch> {
    const BINDING: &[&str] = &["binding"];
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
    const LEASE: &[&str] = &["discover", "request"];
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

pub(crate) fn wireguard_shelf(entry: &str) -> Option<ShelfMatch> {
    const HANDSHAKE: &[&str] = &["handshake"];
    if HANDSHAKE.contains(&entry) {
        Some((
            "handshake",
            "Handshake",
            "docs/book/reference-wireguard-surface.md",
            HANDSHAKE,
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
    } else {
        None
    }
}
