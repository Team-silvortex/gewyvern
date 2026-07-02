use super::super::ShelfMatch;

pub(crate) fn kafka_shelf(entry: &str) -> Option<ShelfMatch> {
    const METADATA: &[&str] = &["metadata", "api-versions"];
    const PRODUCE: &[&str] = &["produce"];
    const FETCH: &[&str] = &["fetch"];
    if METADATA.contains(&entry) {
        Some((
            "metadata",
            "Metadata",
            "docs/book/reference-kafka-metadata-surface.md",
            METADATA,
        ))
    } else if PRODUCE.contains(&entry) {
        Some((
            "produce",
            "Produce",
            "docs/book/reference-kafka-produce-surface.md",
            PRODUCE,
        ))
    } else if FETCH.contains(&entry) {
        Some((
            "fetch",
            "Fetch",
            "docs/book/reference-kafka-fetch-surface.md",
            FETCH,
        ))
    } else {
        None
    }
}

pub(crate) fn nats_shelf(entry: &str) -> Option<ShelfMatch> {
    const SESSION: &[&str] = &["connect"];
    const PUB: &[&str] = &["pub"];
    const SUB: &[&str] = &["sub"];
    const ERROR: &[&str] = &["error"];
    if SESSION.contains(&entry) {
        Some((
            "session",
            "Session",
            "docs/book/reference-nats-session-surface.md",
            SESSION,
        ))
    } else if PUB.contains(&entry) {
        Some((
            "publish",
            "Publish",
            "docs/book/reference-nats-publish-surface.md",
            PUB,
        ))
    } else if SUB.contains(&entry) {
        Some((
            "subscribe",
            "Subscribe",
            "docs/book/reference-nats-subscribe-surface.md",
            SUB,
        ))
    } else if ERROR.contains(&entry) {
        Some((
            "error",
            "Error",
            "docs/book/reference-nats-error-surface.md",
            ERROR,
        ))
    } else {
        None
    }
}
