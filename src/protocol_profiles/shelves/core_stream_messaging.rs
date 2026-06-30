use super::super::ShelfMatch;

pub(crate) fn kafka_shelf(entry: &str) -> Option<ShelfMatch> {
    const METADATA: &[&str] = &["metadata"];
    const STREAM: &[&str] = &["produce", "fetch"];
    if METADATA.contains(&entry) {
        Some((
            "metadata",
            "Metadata",
            "docs/book/reference-kafka-metadata-surface.md",
            METADATA,
        ))
    } else if STREAM.contains(&entry) {
        Some((
            "stream",
            "Stream",
            "docs/book/reference-kafka-stream-surface.md",
            STREAM,
        ))
    } else {
        None
    }
}

pub(crate) fn nats_shelf(entry: &str) -> Option<ShelfMatch> {
    const SESSION: &[&str] = &["connect"];
    const PUBSUB: &[&str] = &["pub", "sub"];
    const ERROR: &[&str] = &["error"];
    if SESSION.contains(&entry) {
        Some((
            "session",
            "Session",
            "docs/book/reference-nats-session-surface.md",
            SESSION,
        ))
    } else if PUBSUB.contains(&entry) {
        Some((
            "pubsub",
            "Publish And Subscribe",
            "docs/book/reference-nats-pubsub-surface.md",
            PUBSUB,
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
