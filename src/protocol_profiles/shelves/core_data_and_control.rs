use super::super::ShelfMatch;

pub(crate) fn mdns_shelf(entry: &str) -> Option<ShelfMatch> {
    const QUERY: &[&str] = &["query"];
    if QUERY.contains(&entry) {
        Some((
            "query",
            "Query",
            "docs/book/reference-mdns-surface.md",
            QUERY,
        ))
    } else {
        None
    }
}

pub(crate) fn ssdp_shelf(entry: &str) -> Option<ShelfMatch> {
    const DISCOVERY: &[&str] = &["discovery"];
    if DISCOVERY.contains(&entry) {
        Some((
            "discovery",
            "Discovery",
            "docs/book/reference-ssdp-surface.md",
            DISCOVERY,
        ))
    } else {
        None
    }
}

pub(crate) fn quic_shelf(entry: &str) -> Option<ShelfMatch> {
    const INITIAL: &[&str] = &["initial"];
    const CRYPTO: &[&str] = &["crypto"];
    const STREAM: &[&str] = &["stream"];
    const BIDI: &[&str] = &["bidi"];
    if INITIAL.contains(&entry) {
        Some((
            "initial",
            "Initial",
            "docs/book/reference-quic-initial-surface.md",
            INITIAL,
        ))
    } else if CRYPTO.contains(&entry) {
        Some((
            "crypto",
            "Crypto Handshake",
            "docs/book/reference-quic-crypto-surface.md",
            CRYPTO,
        ))
    } else if STREAM.contains(&entry) {
        Some((
            "stream",
            "Outbound Stream",
            "docs/book/reference-quic-stream-surface.md",
            STREAM,
        ))
    } else if BIDI.contains(&entry) {
        Some((
            "bidi",
            "Bidirectional Stream",
            "docs/book/reference-quic-bidi-surface.md",
            BIDI,
        ))
    } else {
        None
    }
}

pub(crate) fn radius_shelf(entry: &str) -> Option<ShelfMatch> {
    const ACCESS: &[&str] = &["access"];
    const CHALLENGE: &[&str] = &["challenge"];
    const DENIED: &[&str] = &["denied"];
    if ACCESS.contains(&entry) {
        Some((
            "access",
            "Access",
            "docs/book/reference-radius-access-surface.md",
            ACCESS,
        ))
    } else if CHALLENGE.contains(&entry) {
        Some((
            "challenge",
            "Challenge",
            "docs/book/reference-radius-challenge-surface.md",
            CHALLENGE,
        ))
    } else if DENIED.contains(&entry) {
        Some((
            "denied",
            "Denied",
            "docs/book/reference-radius-denied-surface.md",
            DENIED,
        ))
    } else {
        None
    }
}

pub(crate) fn gtpu_shelf(entry: &str) -> Option<ShelfMatch> {
    const ECHO: &[&str] = &["echo"];
    if ECHO.contains(&entry) {
        Some(("echo", "Echo", "docs/book/reference-gtpu-surface.md", ECHO))
    } else {
        None
    }
}
