use super::super::ShelfMatch;

pub(crate) fn grpc_shelf(entry: &str) -> Option<ShelfMatch> {
    const CALL: &[&str] = &["call"];
    const STATUS: &[&str] = &["status"];
    const STREAM: &[&str] = &["stream"];
    if CALL.contains(&entry) {
        Some((
            "call",
            "Unary Call",
            "docs/book/reference-grpc-call-surface.md",
            CALL,
        ))
    } else if STATUS.contains(&entry) {
        Some((
            "status",
            "Status Trailer",
            "docs/book/reference-grpc-status-surface.md",
            STATUS,
        ))
    } else if STREAM.contains(&entry) {
        Some((
            "stream",
            "Streaming RPC",
            "docs/book/reference-grpc-stream-surface.md",
            STREAM,
        ))
    } else {
        None
    }
}

pub(crate) fn websocket_shelf(entry: &str) -> Option<ShelfMatch> {
    const UPGRADE: &[&str] = &["upgrade"];
    const FRAME: &[&str] = &["frame"];
    const CLOSE: &[&str] = &["close"];
    if UPGRADE.contains(&entry) {
        Some((
            "upgrade",
            "HTTP Upgrade",
            "docs/book/reference-websocket-upgrade-surface.md",
            UPGRADE,
        ))
    } else if FRAME.contains(&entry) {
        Some((
            "frame",
            "Data Frame",
            "docs/book/reference-websocket-frame-surface.md",
            FRAME,
        ))
    } else if CLOSE.contains(&entry) {
        Some((
            "close",
            "Close Control",
            "docs/book/reference-websocket-close-surface.md",
            CLOSE,
        ))
    } else {
        None
    }
}

pub(crate) fn graphql_shelf(entry: &str) -> Option<ShelfMatch> {
    const QUERY: &[&str] = &["query"];
    const MUTATION: &[&str] = &["mutation"];
    const SUBSCRIPTION: &[&str] = &["subscription"];
    if QUERY.contains(&entry) {
        Some((
            "query",
            "Query",
            "docs/book/reference-graphql-query-surface.md",
            QUERY,
        ))
    } else if MUTATION.contains(&entry) {
        Some((
            "mutation",
            "Mutation",
            "docs/book/reference-graphql-mutation-surface.md",
            MUTATION,
        ))
    } else if SUBSCRIPTION.contains(&entry) {
        Some((
            "subscription",
            "Subscription",
            "docs/book/reference-graphql-subscription-surface.md",
            SUBSCRIPTION,
        ))
    } else {
        None
    }
}
