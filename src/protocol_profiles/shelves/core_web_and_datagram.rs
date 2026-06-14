use super::super::ShelfMatch;
use super::GENERIC_SURFACE_PAGE;

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
        Some(("connect", "Connect", GENERIC_SURFACE_PAGE, CONNECT))
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
        Some(("auth", "Auth", GENERIC_SURFACE_PAGE, AUTH))
    } else if RELAY.contains(&entry) {
        Some(("relay", "Relay", GENERIC_SURFACE_PAGE, RELAY))
    } else {
        None
    }
}

pub(crate) fn tls_shelf(entry: &str) -> Option<ShelfMatch> {
    const CLIENT: &[&str] = &["client"];
    if CLIENT.contains(&entry) {
        Some(("client", "Client", GENERIC_SURFACE_PAGE, CLIENT))
    } else {
        None
    }
}

pub(crate) fn stun_shelf(entry: &str) -> Option<ShelfMatch> {
    const BINDING: &[&str] = &["binding"];
    const ALLOCATE: &[&str] = &["allocate"];
    const REFRESH: &[&str] = &["refresh"];
    if BINDING.contains(&entry) {
        Some(("binding", "Binding", GENERIC_SURFACE_PAGE, BINDING))
    } else if ALLOCATE.contains(&entry) {
        Some(("allocate", "Allocate", GENERIC_SURFACE_PAGE, ALLOCATE))
    } else if REFRESH.contains(&entry) {
        Some(("refresh", "Refresh", GENERIC_SURFACE_PAGE, REFRESH))
    } else {
        None
    }
}

pub(crate) fn coap_shelf(entry: &str) -> Option<ShelfMatch> {
    const GET: &[&str] = &["get"];
    const POST: &[&str] = &["post"];
    const PUT: &[&str] = &["put"];
    const DELETE: &[&str] = &["delete"];
    if GET.contains(&entry) {
        Some(("get", "Get", GENERIC_SURFACE_PAGE, GET))
    } else if POST.contains(&entry) {
        Some(("post", "Post", GENERIC_SURFACE_PAGE, POST))
    } else if PUT.contains(&entry) {
        Some(("put", "Put", GENERIC_SURFACE_PAGE, PUT))
    } else if DELETE.contains(&entry) {
        Some(("delete", "Delete", GENERIC_SURFACE_PAGE, DELETE))
    } else {
        None
    }
}

pub(crate) fn ntp_shelf(entry: &str) -> Option<ShelfMatch> {
    const CLIENT: &[&str] = &["client"];
    const QUERY: &[&str] = &["query"];
    const SYNC: &[&str] = &["sync"];
    if CLIENT.contains(&entry) {
        Some(("client", "Client", GENERIC_SURFACE_PAGE, CLIENT))
    } else if QUERY.contains(&entry) {
        Some(("query", "Query", GENERIC_SURFACE_PAGE, QUERY))
    } else if SYNC.contains(&entry) {
        Some(("sync", "Sync", GENERIC_SURFACE_PAGE, SYNC))
    } else {
        None
    }
}

pub(crate) fn dhcp_shelf(entry: &str) -> Option<ShelfMatch> {
    const CLIENT: &[&str] = &["client"];
    const DISCOVER: &[&str] = &["discover"];
    const REQUEST: &[&str] = &["request"];
    if CLIENT.contains(&entry) {
        Some(("client", "Client", GENERIC_SURFACE_PAGE, CLIENT))
    } else if DISCOVER.contains(&entry) {
        Some(("discover", "Discover", GENERIC_SURFACE_PAGE, DISCOVER))
    } else if REQUEST.contains(&entry) {
        Some(("request", "Request", GENERIC_SURFACE_PAGE, REQUEST))
    } else {
        None
    }
}

pub(crate) fn wireguard_shelf(entry: &str) -> Option<ShelfMatch> {
    const HANDSHAKE: &[&str] = &["handshake"];
    if HANDSHAKE.contains(&entry) {
        Some(("handshake", "Handshake", GENERIC_SURFACE_PAGE, HANDSHAKE))
    } else {
        None
    }
}

pub(crate) fn snmp_shelf(entry: &str) -> Option<ShelfMatch> {
    const GET: &[&str] = &["get"];
    const GET_NEXT: &[&str] = &["get-next"];
    const SET: &[&str] = &["set"];
    if GET.contains(&entry) {
        Some(("get", "Get", GENERIC_SURFACE_PAGE, GET))
    } else if GET_NEXT.contains(&entry) {
        Some(("get-next", "Get Next", GENERIC_SURFACE_PAGE, GET_NEXT))
    } else if SET.contains(&entry) {
        Some(("set", "Set", GENERIC_SURFACE_PAGE, SET))
    } else {
        None
    }
}
