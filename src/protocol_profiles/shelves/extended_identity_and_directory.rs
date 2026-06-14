use super::super::ShelfMatch;

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
