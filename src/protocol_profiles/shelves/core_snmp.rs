use super::super::ShelfMatch;

pub(crate) fn snmp_shelf(entry: &str) -> Option<ShelfMatch> {
    const READ: &[&str] = &["bulk", "get", "get-next"];
    const SET: &[&str] = &["set"];
    const NOTIFY: &[&str] = &["trap", "inform"];
    const SECURITY: &[&str] = &["v3-auth", "v3-priv"];
    const MANAGE: &[&str] = &["engine-sync", "trap-recv"];
    const RESULT: &[&str] = &["report", "unauthorized"];
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
    } else if RESULT.contains(&entry) {
        Some((
            "result",
            "Result",
            "docs/book/reference-snmp-result-surface.md",
            RESULT,
        ))
    } else {
        None
    }
}
