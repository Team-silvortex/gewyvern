use super::super::ShelfMatch;

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
