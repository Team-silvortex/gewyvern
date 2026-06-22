use super::super::ShelfMatch;

pub(crate) fn amqp_shelf(entry: &str) -> Option<ShelfMatch> {
    const START: &[&str] = &["start"];
    const SESSION_PUBLISH: &[&str] = &["session", "publish"];
    const CONSUME: &[&str] = &["consume"];
    if START.contains(&entry) {
        Some((
            "start-negotiation",
            "Start And Negotiation",
            "docs/book/reference-amqp-start-surface.md",
            START,
        ))
    } else if SESSION_PUBLISH.contains(&entry) {
        Some((
            "session-publish",
            "Session And Publish",
            "docs/book/reference-amqp-session-surface.md",
            SESSION_PUBLISH,
        ))
    } else if CONSUME.contains(&entry) {
        Some((
            "consume",
            "Consume",
            "docs/book/reference-amqp-consume-surface.md",
            CONSUME,
        ))
    } else {
        None
    }
}

pub(crate) fn smtp_shelf(entry: &str) -> Option<ShelfMatch> {
    const SESSION: &[&str] = &["session", "auth", "auth-denied"];
    const ENVELOPE: &[&str] = &["mail", "rcpt", "rcpt-denied"];
    const DATA: &[&str] = &["data", "data-denied"];
    if SESSION.contains(&entry) {
        Some((
            "session-auth",
            "Session And Auth",
            "docs/book/reference-smtp-session-surface.md",
            SESSION,
        ))
    } else if ENVELOPE.contains(&entry) {
        Some((
            "envelope",
            "Envelope",
            "docs/book/reference-smtp-envelope-surface.md",
            ENVELOPE,
        ))
    } else if DATA.contains(&entry) {
        Some((
            "data",
            "Data",
            "docs/book/reference-smtp-data-surface.md",
            DATA,
        ))
    } else {
        None
    }
}
