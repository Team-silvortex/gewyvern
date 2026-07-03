use super::ProtocolAlias;

pub(crate) const PROTOCOL_ENTRY_ALIASES: &[ProtocolAlias] = &[
    ProtocolAlias {
        alias: "login",
        protocol: "ftp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "control",
        protocol: "ftp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "directory",
        protocol: "ftp",
        entry: Some("list"),
    },
    ProtocolAlias {
        alias: "download",
        protocol: "ftp",
        entry: Some("retr"),
    },
    ProtocolAlias {
        alias: "upload",
        protocol: "ftp",
        entry: Some("stor"),
    },
    ProtocolAlias {
        alias: "active-directory",
        protocol: "ftp",
        entry: Some("active-list"),
    },
    ProtocolAlias {
        alias: "active-download",
        protocol: "ftp",
        entry: Some("active-retr"),
    },
    ProtocolAlias {
        alias: "active-upload",
        protocol: "ftp",
        entry: Some("active-stor"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "ftp",
        entry: Some("denied"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "kerberos",
        entry: Some("as"),
    },
    ProtocolAlias {
        alias: "initial-auth",
        protocol: "kerberos",
        entry: Some("as"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "kerberos",
        entry: Some("as-error"),
    },
    ProtocolAlias {
        alias: "initial-auth-error",
        protocol: "kerberos",
        entry: Some("as-error"),
    },
    ProtocolAlias {
        alias: "ticket",
        protocol: "kerberos",
        entry: Some("tgs"),
    },
    ProtocolAlias {
        alias: "service-ticket",
        protocol: "kerberos",
        entry: Some("tgs"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "ldap",
        entry: Some("bind"),
    },
    ProtocolAlias {
        alias: "auth",
        protocol: "ldap",
        entry: Some("bind"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "ldap",
        entry: Some("bind-denied"),
    },
    ProtocolAlias {
        alias: "auth-denied",
        protocol: "ldap",
        entry: Some("bind-denied"),
    },
    ProtocolAlias {
        alias: "directory",
        protocol: "ldap",
        entry: Some("search"),
    },
    ProtocolAlias {
        alias: "query",
        protocol: "ldap",
        entry: Some("search"),
    },
    ProtocolAlias {
        alias: "directory-session",
        protocol: "ldap",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "replication",
        protocol: "ldap",
        entry: Some("sync"),
    },
    ProtocolAlias {
        alias: "connect",
        protocol: "amqp",
        entry: Some("session"),
    },
    ProtocolAlias {
        alias: "login",
        protocol: "amqp",
        entry: Some("start"),
    },
    ProtocolAlias {
        alias: "negotiate",
        protocol: "amqp",
        entry: Some("start"),
    },
    ProtocolAlias {
        alias: "login-denied",
        protocol: "amqp",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "negotiate-denied",
        protocol: "amqp",
        entry: Some("auth-denied"),
    },
    ProtocolAlias {
        alias: "send",
        protocol: "amqp",
        entry: Some("publish"),
    },
    ProtocolAlias {
        alias: "receive",
        protocol: "amqp",
        entry: Some("consume"),
    },
    ProtocolAlias {
        alias: "deliver",
        protocol: "amqp",
        entry: Some("consume"),
    },
];
