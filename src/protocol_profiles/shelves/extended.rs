use super::ShelfMatch;

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
    const SESSION: &[&str] = &["session", "auth"];
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

pub(crate) fn kerberos_shelf(entry: &str) -> Option<ShelfMatch> {
    const AS: &[&str] = &["as", "as-error"];
    const TGS: &[&str] = &["tgs"];
    if AS.contains(&entry) {
        Some((
            "as",
            "AS Exchange",
            "docs/book/reference-protocol-surface.md",
            AS,
        ))
    } else if TGS.contains(&entry) {
        Some((
            "tgs",
            "TGS Exchange",
            "docs/book/reference-protocol-surface.md",
            TGS,
        ))
    } else {
        None
    }
}

pub(crate) fn rtsp_shelf(entry: &str) -> Option<ShelfMatch> {
    const OPTIONS: &[&str] = &["options"];
    const DESCRIBE: &[&str] = &["describe"];
    const SETUP: &[&str] = &["setup"];
    const PLAY: &[&str] = &["play"];
    if OPTIONS.contains(&entry) {
        Some((
            "options",
            "Options",
            "docs/book/reference-rtsp-options-surface.md",
            OPTIONS,
        ))
    } else if DESCRIBE.contains(&entry) {
        Some((
            "describe",
            "Describe",
            "docs/book/reference-rtsp-describe-surface.md",
            DESCRIBE,
        ))
    } else if SETUP.contains(&entry) {
        Some((
            "setup",
            "Setup",
            "docs/book/reference-rtsp-setup-surface.md",
            SETUP,
        ))
    } else if PLAY.contains(&entry) {
        Some((
            "play",
            "Play",
            "docs/book/reference-rtsp-play-surface.md",
            PLAY,
        ))
    } else {
        None
    }
}

pub(crate) fn ssh_shelf(entry: &str) -> Option<ShelfMatch> {
    const SESSION: &[&str] = &["session"];
    const AUTH: &[&str] = &["auth", "auth-denied"];
    const CHANNEL: &[&str] = &["channel"];
    if SESSION.contains(&entry) {
        Some((
            "session",
            "Session",
            "docs/book/reference-ssh-session-surface.md",
            SESSION,
        ))
    } else if AUTH.contains(&entry) {
        Some((
            "auth",
            "Auth",
            "docs/book/reference-ssh-auth-surface.md",
            AUTH,
        ))
    } else if CHANNEL.contains(&entry) {
        Some((
            "channel",
            "Channel",
            "docs/book/reference-ssh-channel-surface.md",
            CHANNEL,
        ))
    } else {
        None
    }
}

pub(crate) fn imap_shelf(entry: &str) -> Option<ShelfMatch> {
    const AUTH: &[&str] = &["auth", "auth-denied"];
    const SELECT: &[&str] = &["select"];
    if AUTH.contains(&entry) {
        Some((
            "auth",
            "Auth",
            "docs/book/reference-imap-auth-surface.md",
            AUTH,
        ))
    } else if SELECT.contains(&entry) {
        Some((
            "select",
            "Mailbox Select",
            "docs/book/reference-imap-select-surface.md",
            SELECT,
        ))
    } else {
        None
    }
}

pub(crate) fn pop3_shelf(entry: &str) -> Option<ShelfMatch> {
    const AUTH: &[&str] = &["auth", "auth-denied"];
    const LIST: &[&str] = &["list"];
    if AUTH.contains(&entry) {
        Some((
            "auth",
            "Auth",
            "docs/book/reference-pop3-auth-surface.md",
            AUTH,
        ))
    } else if LIST.contains(&entry) {
        Some((
            "list",
            "Mailbox List",
            "docs/book/reference-pop3-list-surface.md",
            LIST,
        ))
    } else {
        None
    }
}

pub(crate) fn socks5_shelf(entry: &str) -> Option<ShelfMatch> {
    const SESSION: &[&str] = &["session"];
    const AUTH: &[&str] = &["auth", "auth-denied"];
    const DENIED: &[&str] = &["auth-connect-denied", "denied"];
    if SESSION.contains(&entry) {
        Some((
            "session",
            "Session",
            "docs/book/reference-socks5-session-surface.md",
            SESSION,
        ))
    } else if AUTH.contains(&entry) {
        Some((
            "auth",
            "Auth",
            "docs/book/reference-socks5-auth-surface.md",
            AUTH,
        ))
    } else if DENIED.contains(&entry) {
        Some((
            "denied",
            "Denied",
            "docs/book/reference-socks5-denied-surface.md",
            DENIED,
        ))
    } else {
        None
    }
}

pub(crate) fn sip_shelf(entry: &str) -> Option<ShelfMatch> {
    const REGISTER: &[&str] = &["register"];
    const INVITE: &[&str] = &["invite"];
    const BYE: &[&str] = &["bye"];
    if REGISTER.contains(&entry) {
        Some((
            "register",
            "Register",
            "docs/book/reference-sip-register-surface.md",
            REGISTER,
        ))
    } else if INVITE.contains(&entry) {
        Some((
            "invite",
            "Invite",
            "docs/book/reference-sip-invite-surface.md",
            INVITE,
        ))
    } else if BYE.contains(&entry) {
        Some(("bye", "Bye", "docs/book/reference-sip-bye-surface.md", BYE))
    } else {
        None
    }
}

pub(crate) fn ldap_shelf(entry: &str) -> Option<ShelfMatch> {
    const BIND: &[&str] = &["bind", "bind-denied"];
    const SEARCH: &[&str] = &["search", "session"];
    const WRITE: &[&str] = &["modify", "denied", "constraint", "write", "sync"];
    if BIND.contains(&entry) {
        Some((
            "bind",
            "Bind",
            "docs/book/reference-ldap-bind-surface.md",
            BIND,
        ))
    } else if SEARCH.contains(&entry) {
        Some((
            "search-session",
            "Search And Session",
            "docs/book/reference-ldap-search-surface.md",
            SEARCH,
        ))
    } else if WRITE.contains(&entry) {
        Some((
            "write-sync",
            "Write And Sync",
            "docs/book/reference-ldap-write-surface.md",
            WRITE,
        ))
    } else {
        None
    }
}
